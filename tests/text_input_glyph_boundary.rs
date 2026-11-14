//! RXV Gate A — the text-input glyph boundary (issue #252).
//!
//! The single-line editor spans five layers that must agree on where every grapheme sits:
//! the logical buffer (`TextInputState::text`), grapheme segmentation, the cursor/width
//! accounting (`cursor_display_column`, `total_display_width`), the painter
//! (`paint_text_input` → `Surface::set_cell`), and the bytes the ANSI compiler emits. Issue #252
//! showed they did not: a pasted TAB was counted as 1 column by the cursor, 0 by the total, 1 by
//! the painter, 0 by the compiler and 8 by the terminal — and `paint_text_input` handed raw control
//! characters straight to `set_cell`, bypassing the guard `Surface::print_str` applies, so the
//! compiler emitted them verbatim.
//!
//! The contract pinned here (see `TextInputState` docs):
//!
//! * **Insertion** (`handle_event` key/paste, `insert_str`, `insert_char`, `with_text`) is
//!   single-line and control-free: line-break runs → one space, TAB → one space, every other
//!   control character (C0, DEL, C1) is dropped.
//! * **Painting** is total over arbitrary strings (consumers may write the public `text` field,
//!   call `Node::text_input` directly, or come through the C ABI): a grapheme the cell model cannot
//!   place faithfully — one containing a control character, or one of zero display width standing
//!   alone — is painted as ONE `U+FFFD` cell. Every grapheme is therefore a visible, one- or
//!   two-cell cursor stop, and no control byte from content ever reaches the terminal.
//! * **Width** is one function: a grapheme occupies `min(width, 2)` cells, or 1 when it is
//!   substituted. Cursor column, total width, painter placement, and emitted columns all use it.
//!
//! Oracles are independent of the implementation: a spec model of the painter and of insertion
//! written here, a strict byte grammar for the renderer's output, and a `vt100` virtual terminal
//! (screen contents, cursor, and a callback recorder that sees any title/clipboard/bell/OSC side
//! effect a hostile sequence would trigger).

use std::ffi::CString;
use std::ptr;

use gibson::capability::ColorDepth;
use gibson::cell::Style;
use gibson::context::{Context, RenderMode};
use gibson::input::{Event, KeyCode, KeyEvent, KeyModifiers, TextInputState};
use gibson::node::Node;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const SUB: &str = "\u{FFFD}";

/// The terminal oracle's view of the substitute. vt100 0.16.2 never draws U+FFFD — its
/// `perform.rs` routes it to `unhandled_char`, reserving it as vte's invalid-UTF-8 marker — while
/// real emulators draw it in one cell. So the *screen* oracle reads the stream through a
/// byte-exact lens mapping each `EF BF BD` to `¤` (U+00A4, also one cell, absent from every
/// payload). The *byte* oracle (`assert_renderer_grammar`) always sees the unmodified stream.
const LENS: &str = "\u{A4}";

fn vt_lens(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(SUB.as_bytes()) {
            out.extend_from_slice(LENS.as_bytes());
            i += SUB.len();
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

fn on_screen(model: &str) -> String {
    model.replace(SUB, LENS)
}

// ---------------------------------------------------------------------------------------------
// Spec models (independent of src/)
// ---------------------------------------------------------------------------------------------

/// Every line or paragraph break the single-line editor flattens.
const BREAKS: [char; 7] = [
    '\r', '\n', '\u{0B}', '\u{0C}', '\u{85}', '\u{2028}', '\u{2029}',
];

/// The cells `(text, width)` the painter must place for grapheme `g`.
fn model_cells(g: &str) -> Vec<(String, usize)> {
    let sub = vec![(SUB.to_string(), 1)];
    if g.chars().any(|c| c.is_control() || BREAKS.contains(&c)) || g.width() == 0 {
        return sub;
    }
    if g.width() <= 2 {
        return vec![(g.to_string(), g.width())];
    }
    // Wider than any cell: one cell per spacing codepoint, zero-width ones riding on it.
    let mut cells: Vec<(String, usize)> = Vec::new();
    for c in g.chars() {
        let w = c.width().unwrap_or(0);
        match cells.last_mut() {
            Some(last) if w == 0 || last.1 == 0 => {
                last.0.push(c);
                last.1 += w;
            }
            _ => cells.push((c.to_string(), w)),
        }
    }
    if cells.iter().any(|(t, _)| !(1..=2).contains(&t.width())) {
        return sub;
    }
    cells
        .into_iter()
        .map(|(t, _)| {
            let w = t.width();
            (t, w)
        })
        .collect()
}

/// What the painter must show for `text`, one entry per grapheme.
fn painted_model(text: &str) -> Vec<String> {
    text.graphemes(true)
        .map(|g| model_cells(g).into_iter().map(|(t, _)| t).collect())
        .collect()
}

fn model_width(g: &str) -> usize {
    model_cells(g).iter().map(|(_, w)| w).sum()
}

fn model_col(text: &str, k: usize) -> usize {
    text.graphemes(true).take(k).map(model_width).sum()
}

/// Single-line insertion sanitizer: break runs → one space, TAB → space, other controls dropped.
fn sanitized_model(s: &str) -> String {
    let mut out = String::new();
    let mut in_break = false;
    for c in s.chars() {
        if BREAKS.contains(&c) {
            if !in_break {
                out.push(' ');
                in_break = true;
            }
        } else if c == '\t' {
            out.push(' ');
            in_break = false;
        } else if c.is_control() {
            // dropped; does not end a break run
        } else {
            out.push(c);
            in_break = false;
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Terminal oracles
// ---------------------------------------------------------------------------------------------

/// Records every side effect a hostile escape sequence could have on a real terminal.
#[derive(Default, Debug, Clone, PartialEq)]
struct Effects {
    bells: usize,
    titles: Vec<Vec<u8>>,
    clipboard: usize,
    resizes: usize,
    unhandled_osc: usize,
    unhandled_char: Vec<char>,
    unhandled_control: Vec<u8>,
    unhandled_escape: Vec<u8>,
    unhandled_csi: Vec<String>,
}

/// Private modes the renderer itself emits that vt100 does not model (synchronized update and
/// DECAWM). Anything else unhandled is foreign.
const RENDERER_PRIVATE_MODES: [&str; 4] = [
    "Some('?')[[2026]]h",
    "Some('?')[[2026]]l",
    "Some('?')[[7]]h",
    "Some('?')[[7]]l",
];

impl Effects {
    /// The effects not explained by the renderer's own vocabulary.
    fn foreign(&self) -> Effects {
        let mut e = self.clone();
        e.unhandled_csi
            .retain(|s| !RENDERER_PRIVATE_MODES.contains(&s.as_str()));
        e
    }
}

impl vt100::Callbacks for Effects {
    fn audible_bell(&mut self, _: &mut vt100::Screen) {
        self.bells += 1;
    }
    fn visual_bell(&mut self, _: &mut vt100::Screen) {
        self.bells += 1;
    }
    fn resize(&mut self, _: &mut vt100::Screen, _: (u16, u16)) {
        self.resizes += 1;
    }
    fn set_window_icon_name(&mut self, _: &mut vt100::Screen, t: &[u8]) {
        self.titles.push(t.to_vec());
    }
    fn set_window_title(&mut self, _: &mut vt100::Screen, t: &[u8]) {
        self.titles.push(t.to_vec());
    }
    fn copy_to_clipboard(&mut self, _: &mut vt100::Screen, _: &[u8], _: &[u8]) {
        self.clipboard += 1;
    }
    fn paste_from_clipboard(&mut self, _: &mut vt100::Screen, _: &[u8]) {
        self.clipboard += 1;
    }
    fn unhandled_control(&mut self, _: &mut vt100::Screen, b: u8) {
        self.unhandled_control.push(b);
    }
    fn unhandled_escape(&mut self, _: &mut vt100::Screen, _: Option<u8>, _: Option<u8>, b: u8) {
        self.unhandled_escape.push(b);
    }
    fn unhandled_csi(
        &mut self,
        _: &mut vt100::Screen,
        i1: Option<u8>,
        _: Option<u8>,
        params: &[&[u16]],
        c: char,
    ) {
        self.unhandled_csi
            .push(format!("{:?}{:?}{c}", i1.map(char::from), params));
    }
    fn unhandled_char(&mut self, _: &mut vt100::Screen, c: char) {
        self.unhandled_char.push(c);
    }
    fn unhandled_osc(&mut self, _: &mut vt100::Screen, _: &[&[u8]]) {
        self.unhandled_osc += 1;
    }
}

/// Strict renderer-output grammar: valid UTF-8; the only escape form is CSI
/// (`ESC [ params/intermediates final`); the only raw controls are CR and LF. No OSC, DCS, APC,
/// PM, SOS, lone ESC, BEL, TAB, NUL, BS, DEL or C1 code point may appear.
fn assert_renderer_grammar(bytes: &[u8], what: &str) {
    let s = std::str::from_utf8(bytes).unwrap_or_else(|e| panic!("{what}: output not UTF-8: {e}"));
    let mut it = s.char_indices();
    while let Some((i, c)) = it.next() {
        match c {
            '\x1b' => match it.next() {
                Some((_, '[')) => loop {
                    let (_, p) = it
                        .next()
                        .unwrap_or_else(|| panic!("{what}: truncated CSI at byte {i}"));
                    if ('\x40'..='\x7e').contains(&p) {
                        break;
                    }
                    assert!(
                        ('\x20'..='\x3f').contains(&p),
                        "{what}: illegal byte {p:?} inside CSI at byte {i}"
                    );
                },
                other => panic!(
                    "{what}: non-CSI escape ESC {other:?} at byte {i} — the renderer never emits \
                     OSC/DCS/APC or bare ESC"
                ),
            },
            '\r' | '\n' => {}
            c if c.is_control() => {
                panic!("{what}: raw control U+{:04X} at byte {i}", c as u32)
            }
            _ => {}
        }
    }
}

struct Shot {
    bytes: Vec<u8>,
    lines: Vec<String>,
    cursor: Option<(u16, u16)>,
    parser: vt100::Parser<Effects>,
}

impl Shot {
    fn row(&self, y: u16) -> String {
        let cols = self.parser.screen().size().1.get();
        self.parser
            .screen()
            .rows(0, cols)
            .nth(y as usize)
            .unwrap_or_default()
    }
    fn vt_cursor(&self) -> (u16, u16) {
        let (r, c) = self.parser.screen().cursor_position();
        (c, r)
    }
    fn effects(&self) -> &Effects {
        self.parser.callbacks()
    }
}

fn shoot_ctx(mut ctx: Context, root: Node, cols: u16, rows: u16) -> Shot {
    ctx.set_root(root);
    let paint = ctx.render().expect("headless render");
    let bytes = ctx.rendered_bytes().to_vec();
    let mut parser = vt100::Parser::new_with_callbacks(
        core::num::NonZeroU16::new(rows).expect("vt100 size is non-zero"),
        core::num::NonZeroU16::new(cols).expect("vt100 size is non-zero"),
        0,
        Effects::default(),
    );
    parser.process(&vt_lens(&bytes));
    Shot {
        bytes,
        lines: ctx.last_frame_lines(),
        cursor: paint.cursor_position,
        parser,
    }
}

fn shoot(root: Node, cols: u16, rows: u16) -> Shot {
    shoot_ctx(
        Context::headless(RenderMode::Inline, cols, rows),
        root,
        cols,
        rows,
    )
}

fn input_node(text: &str, cursor: usize) -> Node {
    Node::col().child(Node::text_input(text, cursor, None, Style::default()).height(1.0))
}

fn state_node(s: &TextInputState) -> Node {
    input_node(&s.text, s.cursor_grapheme)
}

/// The full agreement check for one painted input row at x = 0, no scroll.
fn assert_agreement(shot: &Shot, text: &str, cursor: usize, what: &str) {
    let model = painted_model(text);
    let expect_row: String = model.concat();
    assert_renderer_grammar(&shot.bytes, what);
    assert_eq!(
        shot.effects().foreign(),
        Effects::default(),
        "{what}: the frame triggered terminal side effects"
    );
    // vt100 reconstruction == painter's own frame lines == spec model.
    assert_eq!(
        shot.row(0).trim_end(),
        on_screen(&expect_row).trim_end(),
        "{what}: vt100 row"
    );
    assert_eq!(
        shot.lines[0].trim_end(),
        expect_row.trim_end(),
        "{what}: last_frame_lines row"
    );
    // Cursor: painter == spec model == the terminal's hardware cursor.
    let col = model_col(text, cursor) as u16;
    assert_eq!(shot.cursor, Some((col, 0)), "{what}: painter cursor");
    assert_eq!(shot.vt_cursor(), (col, 0), "{what}: vt100 hardware cursor");
}

// ---------------------------------------------------------------------------------------------
// A1 — the issue #252 reproduction, verbatim, plus its positive controls
// ---------------------------------------------------------------------------------------------

#[test]
fn issue_252_pasted_tab_keeps_width_cursor_and_terminal_columns_in_agreement() {
    let mut state = TextInputState::new();
    state.handle_event(&Event::Paste("foo\tbar".to_string()));

    assert_eq!(state.text, "foo bar", "TAB normalizes to one space");
    assert_eq!(state.cursor_display_column(), state.total_display_width());
    assert_eq!(state.cursor_display_column(), 7);

    let mut ctx = Context::headless(RenderMode::Inline, 40, 4);
    ctx.set_root(Node::col().child(
        Node::text_input(&state.text, state.cursor_grapheme, None, Style::default()).height(1.0),
    ));
    ctx.render().unwrap();
    let mut parser = vt100::Parser::new(
        core::num::NonZeroU16::new(4).expect("vt100 size is non-zero"),
        core::num::NonZeroU16::new(40).expect("vt100 size is non-zero"),
        0,
    );
    parser.process(ctx.rendered_bytes());
    let row0 = parser
        .screen()
        .contents()
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    assert_eq!(row0, "foo bar");
    assert!(
        !ctx.rendered_bytes().contains(&b'\t'),
        "no raw TAB may be emitted"
    );
}

#[test]
fn positive_controls_newline_ascii_backspace_wide_cjk_combining_zwj() {
    // Existing newline normalization (the issue's positive control).
    let mut s = TextInputState::new();
    s.handle_event(&Event::Paste("foo\nbar".to_string()));
    assert_eq!(s.text, "foo bar");
    assert_eq!(s.cursor_display_column(), 7);
    assert_eq!(s.total_display_width(), 7);
    let shot = shoot(state_node(&s), 40, 4);
    assert_agreement(&shot, &s.text, s.cursor_grapheme, "newline paste");

    // Normal ASCII typing.
    let mut s = TextInputState::new();
    for c in "hello".chars() {
        s.handle_event(&Event::Key(KeyEvent::char(c)));
    }
    assert_eq!((s.text.as_str(), s.cursor_grapheme), ("hello", 5));
    assert_agreement(&shoot(state_node(&s), 40, 4), &s.text, 5, "ascii typing");

    // Wide emoji, CJK, combining marks, a valid ZWJ cluster: widths survive unchanged.
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    for (text, width) in [
        ("🦀", 2usize),
        ("漢字", 4),
        ("e\u{301}x\u{308}", 2),
        (family, 2),
        ("a🦀漢e\u{301}", 6),
    ] {
        let s = TextInputState::with_text(text);
        assert_eq!(s.text, text, "{text:?}: printable text is never rewritten");
        assert_eq!(s.total_display_width(), width, "{text:?} total");
        assert_eq!(s.cursor_display_column(), width, "{text:?} cursor at end");
    }

    // Grapheme-aware backspace through a mixed buffer.
    let mut s = TextInputState::with_text(format!("a{family}e\u{301}漢"));
    let mut widths = vec![s.total_display_width()];
    while s.backspace() {
        assert_eq!(s.cursor_display_column(), s.total_display_width());
        widths.push(s.total_display_width());
    }
    assert_eq!(
        widths,
        vec![6, 4, 3, 1, 0],
        "one whole cluster per backspace"
    );
}

// ---------------------------------------------------------------------------------------------
// A2 — hostile control emission across every entry path
// ---------------------------------------------------------------------------------------------

/// Bounded hostile payloads. Each carries a recognisable marker so an *interpreted* sequence
/// would be visible as missing text on the reconstructed screen.
fn hostile_payloads() -> Vec<(&'static str, String)> {
    vec![
        ("tab", "foo\tbar".into()),
        ("cr", "a\rb".into()),
        ("lf", "a\nb".into()),
        ("crlf", "a\r\nb".into()),
        ("lone esc", "x\x1by".into()),
        ("csi private", "pre\x1b[?1337;4242hpost".into()),
        ("csi erase", "pre\x1b[2Jpost".into()),
        ("csi sgr", "pre\x1b[31;1mRED\x1b[0mpost".into()),
        ("csi dsr", "pre\x1b[6npost".into()),
        ("osc title bel", "pre\x1b]0;PWNED\x07post".into()),
        ("osc 52 st", "pre\x1b]52;c;UFdORUQ=\x1b\\post".into()),
        ("dcs", "pre\x1bP+q544e\x1b\\post".into()),
        ("nul", "a\0b".into()),
        ("bel", "a\x07b".into()),
        ("bs", "a\x08b".into()),
        ("del", "a\x7fb".into()),
        ("c1 csi", "pre\u{9b}?1337;4242hpost".into()),
        ("c1 osc", "pre\u{9d}0;PWNED\u{9c}post".into()),
        ("c1 nel", "a\u{85}b".into()),
        ("vt ff", "a\x0bb\x0cc".into()),
        ("unicode separators", "l1\u{2028}l2\u{2029}l3".into()),
    ]
}

/// Runs `check` on every hostile payload and reports ALL failures (not just the first), so the
/// red/green evidence is a complete per-payload table.
fn run_matrix(label: &str, mut check: impl FnMut(&str, &str)) {
    let mut failures = Vec::new();
    let payloads = hostile_payloads();
    for (name, payload) in &payloads {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(name, payload)));
        if let Err(e) = r {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            failures.push(format!("{name}: {}", msg.lines().next().unwrap_or("")));
        }
    }
    assert!(
        failures.is_empty(),
        "{label}: {}/{} payloads failed:\n  {}",
        failures.len(),
        payloads.len(),
        failures.join("\n  ")
    );
}

#[test]
fn hostile_controls_never_reach_the_terminal_through_any_entry_path() {
    run_matrix("direct+state paths", |name, payload| {
        let payload = payload.to_string();
        let clean = sanitized_model(&payload);
        let n = payload.graphemes(true).count();

        // (1) Direct `Node::text_input` with untrusted content: the painter alone must hold.
        let shot = shoot(input_node(&payload, n), 80, 4);
        assert_agreement(&shot, &payload, n, &format!("{name}: direct node"));

        // (2) Bracketed paste through the state.
        let mut s = TextInputState::new();
        s.handle_event(&Event::Paste(payload.clone()));
        assert_eq!(s.text, clean, "{name}: paste sanitizes");

        // (3) Programmatic insert_str (mid-buffer, so boundaries on both sides are exercised).
        let mut t = TextInputState::with_text("[]");
        t.move_left();
        t.insert_str(&payload);
        assert_eq!(t.text, format!("[{clean}]"), "{name}: insert_str sanitizes");

        // (4) Key-by-key: every char of the payload typed as a key event.
        let mut k = TextInputState::new();
        for c in payload.chars() {
            k.handle_event(&Event::Key(KeyEvent::char(c)));
        }
        let typed: String = payload
            .chars()
            .map(|c| sanitized_model(&c.to_string()))
            .collect();
        assert_eq!(k.text, typed, "{name}: typed keys sanitize");

        // (5) Construction.
        assert_eq!(
            TextInputState::with_text(payload.clone()).text,
            clean,
            "{name}: with_text sanitizes"
        );

        for (path, st) in [("paste", &s), ("insert_str", &t), ("keys", &k)] {
            assert!(
                !st.text.chars().any(char::is_control),
                "{name}/{path}: control left in buffer"
            );
            assert_eq!(
                st.cursor_display_column(),
                model_col(&st.text, st.cursor_grapheme),
                "{name}/{path}: cursor column"
            );
            let shot = shoot(state_node(st), 80, 4);
            assert_agreement(
                &shot,
                &st.text,
                st.cursor_grapheme,
                &format!("{name}: {path}"),
            );
        }
    });
}

#[test]
fn hostile_controls_through_the_c_abi_text_input_are_painted_as_replacement_cells() {
    use gibson::ffi::*;
    run_matrix("C ABI path", |name, payload| {
        // A C string cannot carry NUL; that payload is unrepresentable at this boundary.
        let Ok(value) = CString::new(payload) else {
            return;
        };
        let n = payload.graphemes(true).count();
        let bytes = unsafe {
            let mut ctx: *mut GibsonContextOpaque = ptr::null_mut();
            assert_eq!(
                gibson_create_headless_context(GIBSON_MODE_INLINE, 80, 4, &mut ctx),
                GibsonStatus::Ok
            );
            let style = GibsonStyle::default();
            let mut col = ptr::null_mut();
            assert_eq!(gibson_node_box_col(&mut col), GibsonStatus::Ok);
            let mut input = ptr::null_mut();
            assert_eq!(
                gibson_node_text_input(value.as_ptr(), n as u32, ptr::null(), &style, &mut input),
                GibsonStatus::Ok,
                "{name}: the ABI accepts arbitrary UTF-8 content"
            );
            assert_eq!(gibson_node_add_child(col, input), GibsonStatus::Ok);
            assert_eq!(gibson_set_root_node(ctx, col), GibsonStatus::Ok);
            assert_eq!(gibson_render(ctx), GibsonStatus::Ok);
            let mut need = 0usize;
            assert_eq!(
                gibson_get_rendered_output(ctx, ptr::null_mut(), 0, &mut need),
                GibsonStatus::Ok
            );
            let mut buf = vec![0u8; need];
            let mut got = 0usize;
            assert_eq!(
                gibson_get_rendered_output(ctx, buf.as_mut_ptr(), buf.len(), &mut got),
                GibsonStatus::Ok
            );
            buf.truncate(got);
            gibson_destroy_context(ctx);
            buf
        };
        let what = format!("{name}: C ABI");
        assert_renderer_grammar(&bytes, &what);
        let mut parser = vt100::Parser::new_with_callbacks(
            core::num::NonZeroU16::new(4).expect("vt100 size is non-zero"),
            core::num::NonZeroU16::new(80).expect("vt100 size is non-zero"),
            0,
            Effects::default(),
        );
        parser.process(&vt_lens(&bytes));
        assert_eq!(
            parser.callbacks().foreign(),
            Effects::default(),
            "{what}: side effects"
        );
        let row = parser.screen().rows(0, 80).next().unwrap_or_default();
        assert_eq!(
            row.trim_end(),
            on_screen(&painted_model(payload).concat()).trim_end(),
            "{what}: vt100 row"
        );
        let (r, c) = parser.screen().cursor_position();
        assert_eq!(
            (c, r),
            (model_col(payload, n) as u16, 0),
            "{what}: hardware cursor"
        );
    });
}

// ---------------------------------------------------------------------------------------------
// A3 — one width contract, at every cursor position, including zero-width standalone input
// ---------------------------------------------------------------------------------------------

/// Strings whose graphemes a per-codepoint terminal model (vt100, most emulators) places exactly
/// as the cell model does — so the hardware-cursor oracle is meaningful at every position.
fn width_corpus() -> Vec<String> {
    vec![
        "hello".into(),
        "foo\tbar".into(),
        "漢字テスト".into(),
        "e\u{301}e\u{301}x".into(),
        "a🦀b🦀".into(),
        "한국어".into(),
        "\u{200B}ab".into(), // ZERO WIDTH SPACE: a standalone zero-width grapheme
        "\u{301}ab".into(),  // a combining mark with no base
        "\u{FEFF}x\u{202E}y".into(), // BOM and a bidi override: zero-width format chars
        "x\u{FE0F}".into(),  // variation selector extends x (renderable)
        "\u{FE0F}".into(),   // ... but alone it is zero-width
        "a\x1b[2Jb".into(),
        "a\u{9b}31mb".into(),
        "\r\nend".into(),
        // Graphemes wider than two cells (#252 review F1): laid codepoint by codepoint.
        "क्षि x".into(), // Devanagari conjunct + spacing vowel sign: 3 cells
        "भोः!".into(),  // base + two spacing marks: 3 cells
        "\u{1100}\u{1100}xyz".into(), // GB6 joins leading jamo: one grapheme, 4 cells
        "स्त्री".into(),  // 4 cells
        "a\u{17D8}b".into(), // a single codepoint 3 wide: no cell can hold it
        "l1\u{2028}l2".into(), // a line separator in a single-line editor
    ]
}

#[test]
fn cursor_total_painter_and_terminal_agree_at_every_cursor_position() {
    for text in width_corpus() {
        let n = text.graphemes(true).count();
        let state = TextInputState {
            text: text.clone(),
            cursor_grapheme: n,
            scroll_offset: 0,
        };
        assert_eq!(
            state.cursor_display_column(),
            state.total_display_width(),
            "{text:?}: cursor at end must equal total width"
        );
        assert_eq!(
            state.total_display_width(),
            model_col(&text, n),
            "{text:?}: total width"
        );
        for k in 0..=n {
            let st = TextInputState {
                text: text.clone(),
                cursor_grapheme: k,
                scroll_offset: 0,
            };
            assert_eq!(
                st.cursor_display_column(),
                model_col(&text, k),
                "{text:?}@{k}: cursor column"
            );
            let shot = shoot(input_node(&text, k), 40, 3);
            assert_agreement(&shot, &text, k, &format!("{text:?}@{k}"));
        }
    }
}

#[test]
fn every_painted_cell_holds_its_grapheme_at_the_modelled_column() {
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    let flags = "\u{1F1EF}\u{1F1F5}\u{1F1EB}\u{1F1F7}";
    let mut corpus = width_corpus();
    corpus.push(format!("a{family}b"));
    corpus.push(format!("{flags}z"));
    corpus.push("क्षि x".into());
    for text in corpus {
        for depth in [ColorDepth::Mono, ColorDepth::TrueColor] {
            let mut ctx = Context::headless(RenderMode::Inline, 40, 3);
            ctx.set_color_depth(depth);
            ctx.set_root(input_node(&text, 0));
            ctx.render().expect("render");
            let mut col = 0u16;
            for (j, g) in text.graphemes(true).enumerate() {
                for (piece, w) in model_cells(g) {
                    let cell = ctx.frame_cell(col, 0).expect("in bounds");
                    assert_eq!(
                        cell.glyph,
                        piece.as_str(),
                        "{text:?} {depth:?}: grapheme {j} cell at column {col}"
                    );
                    col += w as u16;
                }
            }
        }
    }
}

#[test]
fn standalone_zero_width_input_is_a_visible_editable_cursor_stop() {
    // Policy: a zero-width grapheme that cannot attach to a base is KEPT (rejecting it would make
    // editing order-dependent: typing the base afterwards legitimately merges it) and painted as
    // one U+FFFD cell, so the cursor visibly steps over it and Backspace can remove it.
    let mut s = TextInputState::new();
    s.handle_event(&Event::Key(KeyEvent::char('\u{301}')));
    assert_eq!(s.text, "\u{301}");
    assert_eq!((s.cursor_display_column(), s.total_display_width()), (1, 1));
    assert_agreement(&shoot(state_node(&s), 20, 3), &s.text, 1, "lone mark");

    // Typing a base BEFORE it merges into one renderable cluster of width 1.
    s.move_to_start();
    s.handle_event(&Event::Key(KeyEvent::char('e')));
    assert_eq!(s.text, "e\u{301}");
    assert_eq!(s.grapheme_count(), 1);
    assert_eq!(s.total_display_width(), 1);
    assert_agreement(
        &shoot(state_node(&s), 20, 3),
        &s.text,
        s.cursor_grapheme,
        "merged",
    );

    // A zero-width space stands alone and is removed by exactly one Backspace.
    let mut z = TextInputState::new();
    z.insert_str("a\u{200B}b");
    assert_eq!(z.total_display_width(), 3);
    z.move_left();
    assert!(z.backspace());
    assert_eq!(z.text, "ab");
    assert_eq!(z.cursor_display_column(), 1);
}

// ---------------------------------------------------------------------------------------------
// A5 — editing, scrolling and deletion stay coherent; narrow mono stays readable
// ---------------------------------------------------------------------------------------------

#[test]
fn scrolling_keeps_the_cursor_on_its_grapheme_in_a_narrow_input() {
    let text = "ab漢字cd🦀e\u{301}f\u{200B}ghक्षि\u{1b}ij";
    let n = text.graphemes(true).count();
    let graphemes: Vec<&str> = text.graphemes(true).collect();
    let width = 6u16;
    for k in 0..=n {
        let root = Node::col()
            .child(Node::text_input(text, k, None, Style::default()).width(width as f32));
        let mut ctx = Context::headless(RenderMode::Inline, 20, 3);
        ctx.set_root(root);
        let paint = ctx.render().expect("render");
        let bytes = ctx.rendered_bytes().to_vec();
        assert_renderer_grammar(&bytes, &format!("scroll@{k}"));
        let scroll = paint.text_scroll_offset.expect("input reports its scroll");
        let (cx, cy) = paint.cursor_position.expect("cursor");
        let col = model_col(text, k);
        assert!(
            cx < width,
            "@{k}: cursor x {cx} escaped the {width}-cell input"
        );
        assert_eq!(cx as usize, col - scroll, "@{k}: cursor = column - scroll");
        let mut parser = vt100::Parser::new(
            core::num::NonZeroU16::new(3).expect("vt100 size is non-zero"),
            core::num::NonZeroU16::new(20).expect("vt100 size is non-zero"),
            0,
        );
        parser.process(&vt_lens(&bytes));
        let (r, c) = parser.screen().cursor_position();
        assert_eq!((c, r), (cx, cy), "@{k}: vt100 cursor");
        // Past the last grapheme (k == n) there is no glyph under the cursor; otherwise every
        // cell of the cursor grapheme is on screen, starting at the cursor.
        if let Some(g) = graphemes.get(k) {
            let mut x = c;
            for (piece, w) in model_cells(g) {
                let under = parser.screen().cell(r, x).expect("cell").contents();
                assert_eq!(
                    under,
                    on_screen(&piece),
                    "@{k}: grapheme {k} cell at column {x}"
                );
                x += w as u16;
            }
            assert!(x <= width, "@{k}: the cursor grapheme overran the input");
        }
    }
}

#[test]
fn deletion_from_either_end_preserves_agreement() {
    let start = TextInputState::with_text("a\tb漢\u{200B}e\u{301}🦀\x1b]0;x\x07z");
    assert!(!start.text.chars().any(char::is_control));

    let mut back = start.clone();
    loop {
        assert_eq!(back.cursor_display_column(), back.total_display_width());
        assert_agreement(
            &shoot(state_node(&back), 40, 3),
            &back.text,
            back.cursor_grapheme,
            &format!("backspace {:?}", back.text),
        );
        if !back.backspace() {
            break;
        }
    }
    assert!(back.text.is_empty());

    let mut fwd = start.clone();
    fwd.move_to_start();
    loop {
        assert_eq!(fwd.cursor_display_column(), 0);
        assert_agreement(
            &shoot(state_node(&fwd), 40, 3),
            &fwd.text,
            0,
            &format!("delete {:?}", fwd.text),
        );
        if !fwd.delete() {
            break;
        }
    }
    assert!(fwd.text.is_empty());

    // Ctrl+U / Ctrl+K cut at grapheme boundaries and keep the contract.
    let mut cut = start.clone();
    cut.cursor_grapheme = 4;
    cut.handle_event(&Event::Key(KeyEvent::new(
        KeyCode::Char('k'),
        KeyModifiers::CONTROL,
    )));
    assert_eq!(cut.cursor_display_column(), cut.total_display_width());
    cut.handle_event(&Event::Key(KeyEvent::new(
        KeyCode::Char('u'),
        KeyModifiers::CONTROL,
    )));
    assert!(cut.text.is_empty());
}

#[test]
fn monochrome_40x15_editing_with_hostile_paste_stays_readable() {
    let mut s = TextInputState::with_text("query: ");
    s.handle_event(&Event::Paste(
        "rm -rf\t/tmp/x\x1b]0;PWNED\x07\n\x1b[2Jdone".into(),
    ));
    assert_eq!(s.text, "query: rm -rf /tmp/x]0;PWNED [2Jdone");
    let mut ctx = Context::headless(RenderMode::Fullscreen, 40, 15);
    ctx.set_color_depth(ColorDepth::Mono);
    let root = Node::col()
        .child(Node::text("EDITOR", Style::new().bold()))
        .child(
            Node::text_input(&s.text, s.cursor_grapheme, Some("type"), Style::default())
                .height(1.0),
        );
    let shot = shoot_ctx(ctx, root, 40, 15);
    assert_renderer_grammar(&shot.bytes, "mono 40x15");
    assert_eq!(shot.effects().foreign(), Effects::default());
    assert_eq!(shot.row(0).trim_end(), "EDITOR");
    assert_eq!(shot.row(1).trim_end(), s.text);
    assert_eq!(shot.vt_cursor(), (s.text.chars().count() as u16, 1));
    assert!(
        !String::from_utf8_lossy(&shot.bytes).contains("38;"),
        "mono emits no foreground colour"
    );
}

#[test]
fn a_grapheme_wider_than_two_cells_never_overruns_its_neighbours() {
    // GB6 joins any run of Hangul leading jamo into ONE grapheme: 40 of them are 80 columns.
    let mut s = TextInputState::new();
    s.handle_event(&Event::Paste("\u{1100}".repeat(40)));
    assert_eq!(s.grapheme_count(), 1);
    assert_eq!(s.total_display_width(), 80);
    for cursor in [0, 1] {
        let root = Node::row()
            .child(
                Node::text_input(&s.text, cursor, None, Style::default())
                    .width(20.0)
                    .height(1.0),
            )
            .child(Node::text("|RIGHT", Style::default()));
        let shot = shoot(root, 40, 3);
        let what = format!("80-column grapheme, cursor {cursor}");
        assert_renderer_grammar(&shot.bytes, &what);
        let right: String = (20..26)
            .map(|x| {
                shot.parser
                    .screen()
                    .cell(0, x)
                    .expect("cell")
                    .contents()
                    .to_string()
            })
            .collect();
        assert_eq!(
            right, "|RIGHT",
            "{what}: the neighbouring widget was overwritten"
        );
        let (cx, cy) = shot.cursor.expect("cursor");
        assert!(cx < 20, "{what}: cursor left the input");
        assert_eq!(shot.vt_cursor(), (cx, cy), "{what}: vt100 cursor");
    }

    // A realistic wide cluster beside a neighbour, at every cursor position.
    let text = "नमस्ते क्षि स्त्री";
    let n = text.graphemes(true).count();
    for k in 0..=n {
        let root = Node::row()
            .child(
                Node::text_input(text, k, None, Style::default())
                    .width(8.0)
                    .height(1.0),
            )
            .child(Node::text("|R", Style::default()));
        let shot = shoot(root, 20, 3);
        let what = format!("devanagari@{k}");
        assert_renderer_grammar(&shot.bytes, &what);
        let right: String = (8..10)
            .map(|x| {
                shot.parser
                    .screen()
                    .cell(0, x)
                    .expect("cell")
                    .contents()
                    .to_string()
            })
            .collect();
        assert_eq!(right, "|R", "{what}: neighbour overwritten");
        assert_eq!(
            shot.vt_cursor(),
            shot.cursor.expect("cursor"),
            "{what}: cursor"
        );
    }
}

#[test]
fn numeric_set_text_keeps_invalid_text_invalid() {
    // The editor's insertion normalization must not turn rejected numeric text into a different,
    // valid number (#252 review F3).
    use gibson::edit::EditId;
    use gibson::numeric::{NumericEdit, NumericError, NumericField, NumericInput};
    let mut n = NumericInput::new(NumericField::new());
    assert!(n.begin(EditId(1), 5.0).is_some());
    for raw in ["5\u{0}0", "1\u{8}3", "4\t2", "7\n1"] {
        assert!(
            matches!(
                n.set_text(raw),
                Some(NumericEdit::Invalid {
                    error: NumericError::Unparseable,
                    ..
                })
            ),
            "{raw:?} must stay unparseable"
        );
    }
    assert!(matches!(
        n.set_text("50"),
        Some(NumericEdit::Preview { value, .. }) if value == 50.0
    ));
}

/// Terminal-model note, measured not assumed: a ZWJ emoji sequence is one grapheme of width 2 in
/// the cell model, but a per-codepoint terminal (vt100 here; real emulators vary) draws each
/// pictograph separately. That divergence predates #252, applies equally to `Node::text`, and is
/// documented in the release contract; this test pins that the text input does not make it worse
/// (the cell model still places the cluster at the modelled column, asserted above).
#[test]
fn zwj_terminal_model_divergence_is_measured() {
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    let per_char: usize = family.chars().filter_map(|c| c.width()).sum();
    assert_eq!(family.width(), 2, "cell model: one wide cluster");
    assert_eq!(
        per_char, 6,
        "per-codepoint terminal model: three wide pictographs"
    );
}
