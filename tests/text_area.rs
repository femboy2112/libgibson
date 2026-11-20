//! v0.9.1 `TextAreaState` / `text_area` (EXPERIMENTAL) — RED slice.
//!
//! Contract: `docs/design/v0.9.1-text-area.md`. The `red_` tests fail BY ASSERTION against the
//! stubs in `src/input/text_area.rs` (and the one-label lowering in `src/ui/compile.rs`); they go
//! green when the slice is implemented. The `guard_` tests pass today and must stay green: they
//! pin the single-line `TextInputState` contract (issue #252), the field set of `TextAreaState`
//! (no commit signal), and the runtime routing that is already real (Enter is text, never a
//! press; Esc and Ctrl/Alt+Enter stay the application's).
//!
//! Oracles are independent of the implementation under test: row geometry is checked as
//! invariants plus hand-computed pinned examples, grapheme widths come from the public
//! `TextInputState` width contract, and painting is compared cell for cell against the rows the
//! state itself reports.

use gibson::input::VisualRow;
use gibson::ui::prelude::*;
use gibson::ui::{compile_presented, Key, PresentationCx};
use gibson::{
    compute_layout, paint, ColorDepth, Event, KeyCode, KeyEvent, KeyModifiers, Surface,
    TextAreaState, TextInputState,
};
use std::time::Duration;
use unicode_segmentation::UnicodeSegmentation;

// ---------------------------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------------------------

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::empty()))
}

fn key_mod(code: KeyCode, modifiers: KeyModifiers) -> Event {
    Event::Key(KeyEvent::new(code, modifiers))
}

fn ch(c: char) -> Event {
    Event::Key(KeyEvent::char(c))
}

fn paste(s: &str) -> Event {
    Event::Paste(s.to_string())
}

/// A state holding `text` with the cursor at grapheme `cursor`.
fn state_at(text: &str, cursor: usize) -> TextAreaState {
    TextAreaState {
        text: text.to_string(),
        cursor_grapheme: cursor,
        ..TextAreaState::default()
    }
}

/// Cells of one grapheme under the editor width contract, read from the public single-line
/// editor (independent of the area's own width code). Never called on a line break.
fn gw(g: &str) -> usize {
    TextInputState::with_text(g).total_display_width()
}

/// `(start, end, width)` of every visual row.
fn rows_of(text: &str, width: u16) -> Vec<(usize, usize, usize)> {
    state_at(text, 0)
        .visual_rows(width)
        .iter()
        .map(|r| (r.start, r.end, r.width))
        .collect()
}

/// Deterministic ~8 KB multi-line buffer: ASCII words, blank lines, CJK, an emoji ZWJ sequence,
/// combining marks, and one 150-cell unbreakable run (forces hard breaks at every width).
fn corpus() -> String {
    const WORDS: [&str; 12] = [
        "alpha", "beta", "gamma", "delta", "lorem", "ipsum", "wrap", "cursor", "terminal",
        "grapheme", "prompt", "system",
    ];
    const EXOTIC: [&str; 4] = ["漢字", "👩\u{200D}💻", "e\u{0301}", "日本語"];
    let mut out = String::new();
    let mut seed: u32 = 0x2545_F491;
    while out.len() < 8192 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let pick = (seed >> 16) as usize;
        match pick % 11 {
            0 => out.push_str(EXOTIC[(pick / 11) % EXOTIC.len()]),
            1 => out.push('\n'),
            2 => out.push_str("\n\n"),
            _ => out.push_str(WORDS[pick % WORDS.len()]),
        }
        if pick % 7 != 0 {
            out.push(' ');
        }
        if out.len() > 4000 && out.len() < 4100 {
            out.push('\n');
            out.push_str(&"z".repeat(150));
            out.push('\n');
        }
    }
    out
}

/// A scripted edit sequence that walks the whole buffer, types, inserts newlines, pastes,
/// deletes, and pages.
fn script() -> Vec<Event> {
    let mut v = Vec::new();
    for _ in 0..8 {
        v.push(key(KeyCode::PageDown));
    }
    for _ in 0..40 {
        v.push(key(KeyCode::Down));
    }
    for c in "hello wrap".chars() {
        v.push(ch(c));
    }
    v.push(key(KeyCode::Enter));
    v.push(key(KeyCode::Enter));
    v.push(paste("pasted 漢字\r\nline\nthird 👩\u{200D}💻 line"));
    for _ in 0..25 {
        v.push(key(KeyCode::Up));
    }
    for _ in 0..30 {
        v.push(key(KeyCode::PageUp));
    }
    v.push(key(KeyCode::End));
    v.push(key(KeyCode::Home));
    for _ in 0..30 {
        v.push(key(KeyCode::Right));
    }
    for _ in 0..5 {
        v.push(key(KeyCode::Backspace));
    }
    for _ in 0..5 {
        v.push(key(KeyCode::Delete));
    }
    for _ in 0..150 {
        v.push(key(KeyCode::Down));
    }
    v.push(key(KeyCode::Enter));
    for _ in 0..4 {
        v.push(key(KeyCode::PageUp));
    }
    v
}

/// Renders a focused or unfocused `text_area` of `width` x `rows` cells into a `Surface`.
fn paint_area(state: &TextAreaState, width: u16, rows: u16, focused: bool) -> Surface {
    let env = UiEnvironment {
        width,
        height: rows,
        color_depth: ColorDepth::TrueColor,
        motion: MotionPreference::None,
        ..UiEnvironment::default()
    };
    let mut cx = PresentationCx::new(BuildCx::new(skins::SWISS_SIGNAL, env));
    if focused {
        cx.focused = Some(Key::from("area"));
    }
    let tree: Element<()> = text_area(state).key("area").rows(rows).width(width);
    let mut node = compile_presented(&tree, &cx).unwrap().node;
    compute_layout(&mut node, width, rows).unwrap();
    let mut surface = Surface::new(width, rows);
    paint(&node, &mut surface);
    surface
}

/// What the screen says on row `y`: lead cells only, trailing blanks trimmed.
fn painted_row(surface: &Surface, y: u16) -> String {
    (0..surface.width)
        .filter_map(|x| {
            let cell = surface.get(x, y).unwrap();
            (!cell.is_continuation).then_some(cell.glyph.grapheme.as_str())
        })
        .collect::<String>()
        .trim_end_matches(' ')
        .to_string()
}

/// Geometry invariants of `visual_rows`; returns the rows. Fails by assertion, never panics.
fn assert_row_partition(state: &TextAreaState, width: u16, ctx: &str) -> Vec<VisualRow> {
    let gs: Vec<&str> = state.text.graphemes(true).collect();
    let n = gs.len();
    let ws: Vec<usize> = gs
        .iter()
        .map(|g| if *g == "\n" { 0 } else { gw(g) })
        .collect();
    let limit = (width as usize).saturating_sub(1).max(1);
    let rows = state.visual_rows(width);
    assert!(!rows.is_empty(), "{ctx}: no rows");
    assert_eq!(
        rows[0].start, 0,
        "{ctx}: first row must start at grapheme 0"
    );
    for (i, r) in rows.iter().enumerate() {
        assert!(
            r.start <= r.end && r.end <= n,
            "{ctx}: row {i} out of range {r:?} (n={n})"
        );
        assert!(
            gs[r.start..r.end].iter().all(|g| *g != "\n"),
            "{ctx}: row {i} spans a line break {r:?}"
        );
        let w: usize = ws[r.start..r.end].iter().sum();
        assert_eq!(r.width, w, "{ctx}: row {i} width {r:?}");
        assert!(
            r.width <= limit,
            "{ctx}: row {i} wider than width-1={limit}: {r:?}"
        );
        match rows.get(i + 1) {
            Some(next) if next.start == r.end => {
                // Soft wrap: justified by a full row or by a word boundary (trailing space).
                assert!(
                    next.start < n,
                    "{ctx}: soft wrap into nothing after row {i}"
                );
                let ends_in_space = r.end > r.start && gs[r.end - 1] == " ";
                assert!(
                    ends_in_space || r.width + ws[next.start] > limit,
                    "{ctx}: row {i} {r:?} broke early with room left"
                );
            }
            Some(next) => {
                assert_eq!(
                    next.start,
                    r.end + 1,
                    "{ctx}: row {i} -> {} skips graphemes",
                    i + 1
                );
                assert!(
                    r.end < n && gs[r.end] == "\n",
                    "{ctx}: hard break after row {i} is not a newline"
                );
            }
            None => assert_eq!(r.end, n, "{ctx}: last row must end at the buffer end"),
        }
    }
    rows
}

/// The cursor's `(row, col)` derived from `rows` alone: the last row starting at or before the
/// cursor grapheme, and the summed width of the graphemes before the cursor on it.
fn oracle_rc(state: &TextAreaState, rows: &[VisualRow]) -> (usize, usize) {
    let gs: Vec<&str> = state.text.graphemes(true).collect();
    let g = state.cursor_grapheme.min(gs.len());
    let ri = rows.iter().rposition(|r| r.start <= g).unwrap_or(0);
    let start = rows[ri].start.min(g);
    let col: usize = gs[start..g].iter().map(|s| gw(s)).sum();
    (ri, col)
}

/// All per-edit invariants: geometry, cursor model, scroll window, painted surface.
fn check_all(state: &TextAreaState, width: u16, rows: u16, ctx: &str) {
    let vrows = assert_row_partition(state, width, ctx);

    let (r, c) = state.cursor_rc(width);
    assert_eq!(
        (r, c),
        oracle_rc(state, &vrows),
        "{ctx}: cursor_rc disagrees with visual_rows"
    );
    assert!(
        state.scroll_row <= r && r < state.scroll_row + rows as usize,
        "{ctx}: cursor row {r} outside scroll window [{}, {})",
        state.scroll_row,
        state.scroll_row + rows as usize
    );

    // Wrap agrees with the painter cell for cell.
    let on = paint_area(state, width, rows, true);
    let gs: Vec<&str> = state.text.graphemes(true).collect();
    for y in 0..rows {
        let expected = vrows
            .get(state.scroll_row + y as usize)
            .map(|vr| {
                gs[vr.start..vr.end]
                    .concat()
                    .trim_end_matches(' ')
                    .to_string()
            })
            .unwrap_or_default();
        assert_eq!(
            painted_row(&on, y),
            expected,
            "{ctx}: painted row {y} (visual row {}) disagrees with visual_rows",
            state.scroll_row + y as usize
        );
    }

    // The cursor cell: focusing changes the style of the cursor cell and nothing else.
    let off = paint_area(state, width, rows, false);
    let cursor_y = r.checked_sub(state.scroll_row).unwrap_or(usize::MAX);
    let mut differing = Vec::new();
    for y in 0..rows {
        for x in 0..width {
            if on.get(x, y).unwrap().style != off.get(x, y).unwrap().style {
                differing.push((x as usize, y as usize));
            }
        }
    }
    assert!(
        !differing.is_empty(),
        "{ctx}: focused paint shows no cursor cell"
    );
    assert!(
        differing
            .iter()
            .all(|&(x, y)| y == cursor_y && (c..c + 2).contains(&x)),
        "{ctx}: cursor cell not at (col {c}, row {cursor_y}); styled cells {differing:?}"
    );
}

fn run_script(width: u16, rows: u16) {
    let mut state = TextAreaState {
        text: corpus(),
        ..TextAreaState::default()
    };
    assert!(state.text.len() >= 8192, "corpus too small");
    check_all(&state, width, rows, "initial");
    for (i, ev) in script().iter().enumerate() {
        let handled = state.handle_event(ev, width, rows);
        state.update_scroll(width, rows);
        check_all(
            &state,
            width,
            rows,
            &format!("w{width} event {i} {ev:?} handled={handled}"),
        );
    }
}

// ---------------------------------------------------------------------------------------------
// RED: wrap rule (pinned, hand-computed)
// ---------------------------------------------------------------------------------------------

#[test]
fn red_wrap_word_boundary_fills_to_width_minus_one() {
    // Width 9 wraps at 8 cells (the last column is the cursor's). "aaa " + "bbb " fit; "ccc" does not.
    assert_eq!(rows_of("aaa bbb ccc", 9), vec![(0, 8, 8), (8, 11, 3)]);
    // "ab " then an over-long word: break before it, then hard-break by grapheme.
    assert_eq!(
        rows_of("ab cdefghij", 5),
        vec![(0, 3, 3), (3, 7, 4), (7, 11, 4)]
    );
}

#[test]
fn red_wrap_hard_breaks_long_words_by_grapheme() {
    assert_eq!(
        rows_of("abcdefghij", 5),
        vec![(0, 4, 4), (4, 8, 4), (8, 10, 2)]
    );
}

#[test]
fn red_wrap_never_splits_a_wide_grapheme() {
    // 漢字漢字 is four 2-cell graphemes; width 6 wraps at 5 cells: two per row, never 2.5.
    assert_eq!(rows_of("漢字漢字", 6), vec![(0, 2, 4), (2, 4, 4)]);
}

#[test]
fn red_wrap_newlines_end_rows_and_a_trailing_newline_opens_an_empty_row() {
    // Graphemes: a b \n c d \n  -> the '\n' graphemes sit at indices 2 and 5.
    assert_eq!(
        rows_of("ab\ncd\n", 10),
        vec![(0, 2, 2), (3, 5, 2), (6, 6, 0)]
    );
    assert_eq!(rows_of("", 10), vec![(0, 0, 0)]);
    assert_eq!(rows_of("a\n\nb", 10), vec![(0, 1, 1), (2, 2, 0), (3, 4, 1)]);
}

// ---------------------------------------------------------------------------------------------
// RED: cursor model, scrolling, movement
// ---------------------------------------------------------------------------------------------

#[test]
fn red_cursor_rc_pinned() {
    let rc = |text: &str, cursor: usize, width: u16| state_at(text, cursor).cursor_rc(width);
    // "aaa bbb ccc" at width 9 -> rows [0,8) and [8,11).
    assert_eq!(rc("aaa bbb ccc", 7, 9), (0, 7));
    // A soft-wrap boundary belongs to the next row.
    assert_eq!(rc("aaa bbb ccc", 8, 9), (1, 0));
    assert_eq!(rc("aaa bbb ccc", 9, 9), (1, 1));
    assert_eq!(rc("aaa bbb ccc", 11, 9), (1, 3));
    // A hard-break boundary: before the '\n' is the end of its row, after it the next row.
    assert_eq!(rc("ab\ncd", 2, 10), (0, 2));
    assert_eq!(rc("ab\ncd", 3, 10), (1, 0));
    // Columns are cells, not graphemes.
    assert_eq!(rc("漢字x", 2, 10), (0, 4));
}

#[test]
fn red_update_scroll_moves_minimally_and_keeps_the_cursor_row_visible() {
    let text = (0..100)
        .map(|i| format!("l{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut s = TextAreaState::with_text(text); // cursor at the end of "l99"
    assert_eq!(s.cursor_rc(20), (99, 3));
    s.update_scroll(20, 5);
    assert!(
        (95..=99).contains(&s.scroll_row),
        "cursor row 99 not inside the 5-row window starting at {}",
        s.scroll_row
    );
    // Cursor to the top: the window scrolls up just enough (cursor row becomes the first row).
    s.cursor_grapheme = 0;
    s.update_scroll(20, 5);
    assert_eq!(s.scroll_row, 0);
    // Down to row 10 (start of "l10"): scrolls down just enough (cursor row becomes the last row).
    s.cursor_grapheme = 30;
    assert_eq!(s.cursor_rc(20), (10, 0));
    s.update_scroll(20, 5);
    assert_eq!(s.scroll_row, 6);
    // Row 8 is inside [6, 11): nothing moves.
    s.cursor_grapheme = 24;
    s.update_scroll(20, 5);
    assert_eq!(s.scroll_row, 6);
}

#[test]
fn red_goal_column_survives_a_short_line() {
    // long, short, long: Down, Down returns to the original column.
    let mut s = state_at("aaaaaaaaaa\nbb\ncccccccccc", 6);
    assert!(s.handle_event(&key(KeyCode::Down), 40, 5));
    assert_eq!(s.cursor_rc(40), (1, 2));
    assert_eq!(s.cursor_grapheme, 13);
    assert_eq!(s.goal_col, Some(6));
    assert!(s.handle_event(&key(KeyCode::Down), 40, 5));
    assert_eq!(s.cursor_rc(40), (2, 6));
    assert_eq!(s.cursor_grapheme, 20);
    // And back up through the short line.
    assert!(s.handle_event(&key(KeyCode::Up), 40, 5));
    assert_eq!(s.cursor_rc(40), (1, 2));
    assert!(s.handle_event(&key(KeyCode::Up), 40, 5));
    assert_eq!(s.cursor_rc(40), (0, 6));
    assert_eq!(s.cursor_grapheme, 6);
    // A horizontal move forgets the goal column.
    assert!(s.handle_event(&key(KeyCode::Right), 40, 5));
    assert_eq!(s.goal_col, None);
}

#[test]
fn red_up_down_move_by_wrapped_visual_row() {
    // One logical line, two visual rows (width 9 wraps at 8): Down from row 0 lands on row 1.
    let mut s = state_at("aaa bbb ccc", 2);
    assert!(s.handle_event(&key(KeyCode::Down), 9, 5));
    assert_eq!(s.cursor_rc(9), (1, 2));
    assert_eq!(s.cursor_grapheme, 10);
    assert!(s.handle_event(&key(KeyCode::Up), 9, 5));
    assert_eq!(s.cursor_rc(9), (0, 2));
    assert_eq!(s.cursor_grapheme, 2);
}

#[test]
fn red_home_end_are_the_start_and_end_of_the_visual_row() {
    let mut s = state_at("aaa bbb ccc", 10); // row 1
    assert!(s.handle_event(&key(KeyCode::Home), 9, 5));
    assert_eq!(s.cursor_grapheme, 8);
    assert!(s.handle_event(&key(KeyCode::End), 9, 5));
    assert_eq!(s.cursor_grapheme, 11);
    // End on a soft-wrapped row stays on that row: on its last grapheme, not the next row's start.
    let mut s = state_at("aaa bbb ccc", 3);
    assert!(s.handle_event(&key(KeyCode::End), 9, 5));
    assert_eq!(s.cursor_grapheme, 7);
    assert_eq!(s.cursor_rc(9).0, 0);
    assert!(s.handle_event(&key(KeyCode::Home), 9, 5));
    assert_eq!(s.cursor_grapheme, 0);
    // End on a hard-ended row is after its last grapheme (before the '\n').
    let mut s = state_at("ab\ncd", 0);
    assert!(s.handle_event(&key(KeyCode::End), 10, 5));
    assert_eq!(s.cursor_grapheme, 2);
}

#[test]
fn red_page_keys_move_by_the_visible_rows() {
    let text = (0..100)
        .map(|i| format!("l{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut s = state_at(&text, 0);
    assert!(s.handle_event(&key(KeyCode::PageDown), 20, 5));
    assert_eq!(s.cursor_rc(20).0, 5);
    assert!(s.handle_event(&key(KeyCode::PageDown), 20, 5));
    assert_eq!(s.cursor_rc(20).0, 10);
    assert!(s.handle_event(&key(KeyCode::PageUp), 20, 5));
    assert_eq!(s.cursor_rc(20).0, 5);
}

// ---------------------------------------------------------------------------------------------
// RED: the 8 KB scripted multi-line buffer (cursor window, wrap == painter, cursor cell)
// ---------------------------------------------------------------------------------------------

#[test]
fn red_scripted_edits_80x24_cursor_visible_and_wrap_matches_painter() {
    run_script(80, 24);
}

#[test]
fn red_scripted_edits_40x24_cursor_visible_and_wrap_matches_painter() {
    run_script(40, 24);
}

// ---------------------------------------------------------------------------------------------
// RED: editing keys, Enter is text (never a commit)
// ---------------------------------------------------------------------------------------------

#[test]
fn red_enter_inserts_a_newline_and_reports_handled() {
    let mut s = state_at("ab", 1);
    assert!(
        s.handle_event(&key(KeyCode::Enter), 20, 4),
        "Enter must be handled"
    );
    assert_eq!(s.text, "a\nb");
    assert_eq!(s.cursor_grapheme, 2);
    // Shift+Enter is the same newline.
    assert!(s.handle_event(&key_mod(KeyCode::Enter, KeyModifiers::SHIFT), 20, 4));
    assert_eq!(s.text, "a\n\nb");
    // Ctrl/Alt+Enter are the application's (reportable only with the kitty protocol): not text.
    assert!(!s.handle_event(&key_mod(KeyCode::Enter, KeyModifiers::CONTROL), 20, 4));
    assert!(!s.handle_event(&key_mod(KeyCode::Enter, KeyModifiers::ALT), 20, 4));
    assert_eq!(s.text, "a\n\nb");
}

#[test]
fn red_typing_deleting_and_horizontal_moves() {
    let mut s = TextAreaState::new();
    for c in "ab".chars() {
        assert!(s.handle_event(&ch(c), 20, 4));
    }
    assert_eq!((s.text.as_str(), s.cursor_grapheme), ("ab", 2));
    assert!(s.handle_event(&key(KeyCode::Left), 20, 4));
    assert_eq!(s.cursor_grapheme, 1);
    assert!(s.handle_event(&key(KeyCode::Delete), 20, 4));
    assert_eq!((s.text.as_str(), s.cursor_grapheme), ("a", 1));
    assert!(s.handle_event(&key(KeyCode::Backspace), 20, 4));
    assert_eq!((s.text.as_str(), s.cursor_grapheme), ("", 0));
    // Edges are not "handled changes".
    assert!(!s.handle_event(&key(KeyCode::Backspace), 20, 4));
    assert!(!s.handle_event(&key(KeyCode::Left), 20, 4));
}

#[test]
fn red_backspace_removes_a_whole_grapheme_cluster() {
    let mut s = TextAreaState::new();
    assert!(s.handle_event(&ch('e'), 20, 4));
    assert!(s.handle_event(&ch('\u{0301}'), 20, 4));
    assert_eq!(s.grapheme_count(), 1);
    assert!(s.handle_event(&key(KeyCode::Backspace), 20, 4));
    assert_eq!(s.text, "");
}

#[test]
fn red_runtime_enter_is_an_edit_action_never_a_press() {
    let state = TextAreaState::with_text("a");
    let mut runtime = area_runtime(&state);
    let outcome = runtime.handle_event(&key(KeyCode::Enter));
    assert!(outcome.consumed);
    assert_eq!(outcome.actions, [Act::Edited("a\n".to_string())]);
}

// ---------------------------------------------------------------------------------------------
// RED: paste sanitization
// ---------------------------------------------------------------------------------------------

/// No terminal control byte may be in the buffer: no C0 except '\n', no DEL, no C1, no ESC.
fn assert_clean(text: &str, ctx: &str) {
    assert!(
        // TAB is kept (GOD #137 ruling i): the buffer must round-trip byte-exact.
        text.chars()
            .all(|c| c == '\n' || c == '\t' || !c.is_control()),
        "{ctx}: control character in buffer {text:?}"
    );
    assert!(
        !text.chars().any(|c| ('\u{7f}'..='\u{9f}').contains(&c)),
        "{ctx}: DEL/C1 in buffer {text:?}"
    );
    assert!(!text.contains('\u{1b}'), "{ctx}: ESC in buffer {text:?}");
}

#[test]
fn red_paste_keeps_newlines_and_normalizes_crlf_and_cr() {
    let mut s = TextAreaState::new();
    assert!(s.handle_event(&paste("x\r\ny\rz\n"), 20, 4));
    assert_eq!(s.text, "x\ny\nz\n");
    assert_eq!(s.cursor_grapheme, 6);
    assert_eq!(TextAreaState::normalize_paste("p\r\n\r\nq"), "p\n\nq");
}

#[test]
fn red_hostile_paste_leaves_no_control_byte_in_the_buffer() {
    let hostile = "a\u{1b}[31mb\u{1b}]0;title\u{7}c\r\nd\u{9b}2Je\u{0}";
    // Through the event path.
    let mut s = TextAreaState::new();
    s.handle_event(&paste(hostile), 20, 4);
    assert_clean(&s.text, "paste event");
    assert_eq!(s.text, "abc\nde");
    assert_eq!(s.cursor_grapheme, 6);
    // Through the public insertion path.
    let mut t = TextAreaState::new();
    t.insert_str(hostile);
    assert_clean(&t.text, "insert_str");
    assert_eq!(t.text, "abc\nde");
    // And at a cursor in the middle of existing text.
    let mut u = state_at("XY", 1);
    u.handle_event(&paste(hostile), 20, 4);
    assert_eq!(u.text, "Xabc\ndeY");
}

#[test]
fn red_paste_strips_every_c0_and_c1_control_but_newline() {
    for cp in 0u32..=0x9F {
        let c = char::from_u32(cp).unwrap();
        if !c.is_control() {
            continue;
        }
        let mut s = TextAreaState::new();
        s.handle_event(&paste(&format!("a{c}b")), 20, 4);
        assert_clean(&s.text, &format!("U+{cp:04X}"));
        // Introducers (ESC, C1 CSI/OSC/DCS/...) may swallow what follows them; the text
        // before the control always survives.
        assert!(
            s.text.starts_with('a'),
            "U+{cp:04X}: text before the control was lost: {:?}",
            s.text
        );
    }
}

#[test]
fn red_paste_escape_sequence_table() {
    // (input, expected). ESC- and C1-introduced sequences are swallowed through their
    // terminator; an interrupted or unterminated one is cut where it stops being a sequence.
    let cases: [(&str, &str); 13] = [
        ("x\u{1b}[31my", "xy"),
        ("x\u{1b}[1;2;3~y", "xy"),
        ("x\u{1b}]0;title\u{7}y", "xy"),
        ("x\u{1b}]52;c;AAAA\u{1b}\\y", "xy"),
        ("x\u{1b}P1;2|data\u{1b}\\y", "xy"),
        ("x\u{1b}cy", "xy"),
        ("x\u{1b}", "x"),
        ("x\u{1b}漢", "x漢"),
        ("x\u{1b}]0;never-terminated", "x"),
        ("x\u{1b}[31\ny", "x\ny"),
        ("x\u{9d}0;t\u{7}z", "xz"),
        ("x\u{90}data\u{9c}k", "xk"),
        ("d\u{9b}2Je", "de"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            TextAreaState::normalize_paste(input),
            expected,
            "normalize_paste({input:?})"
        );
        let mut s = TextAreaState::new();
        s.handle_event(&paste(input), 20, 4);
        assert_eq!(s.text, expected, "paste({input:?})");
    }
}

#[test]
fn red_paste_tab_and_unicode_separators() {
    // TAB is kept verbatim (GOD #137 ruling i: the field holds a system prompt and Reset/Save
    // must round-trip byte-exact); U+2028/U+2029 become newlines instead of painting as
    // U+FFFD; DEL is dropped.
    assert_eq!(TextAreaState::normalize_paste("a\tb"), "a\tb");
    let mut s = TextAreaState::new();
    assert!(s.handle_event(&paste("a\tb"), 20, 4));
    assert_eq!(s.text, "a\tb");
    assert_eq!(s.cursor_grapheme, 3);
    let prompt = "rules:\n\t1. be kind\n\t2. be exact\t(always)\n";
    assert_eq!(TextAreaState::with_text(prompt).text, prompt);
    let mut t = TextAreaState::new();
    t.insert_str(prompt);
    assert_eq!(t.text, prompt);
    assert_eq!(
        TextAreaState::normalize_paste("a\u{2028}b\u{2029}c"),
        "a\nb\nc"
    );
    assert_eq!(TextAreaState::normalize_paste("a\u{7f}b"), "ab");
}

#[test]
fn red_tab_is_one_grapheme_four_cells_wide() {
    // GOD #137 ruling i: a TAB is one grapheme of fixed width 4 (no tab stops) for wrap,
    // cursor and painter alike.
    assert_eq!(rows_of("a\tb", 20), vec![(0, 3, 6)]);
    assert_eq!(state_at("a\tb", 2).cursor_rc(20), (0, 5));
    // Fixed width, not a tab stop: a TAB after 3 cells is still 4 cells.
    assert_eq!(state_at("abc\tz", 4).cursor_rc(20), (0, 7));
    // Wrap at width 10 (limit 9): one unbreakable 12-cell token, hard-broken by grapheme.
    assert_eq!(rows_of("\t\t\t", 10), vec![(0, 2, 8), (2, 3, 4)]);
    // The painter shows the TAB as 4 blank cells and keeps the buffer byte-exact.
    let state = TextAreaState::with_text("a\tb");
    let surface = paint_area(&state, 20, 3, false);
    assert_eq!(painted_row(&surface, 0), "a    b");
    assert_eq!(state.text, "a\tb");
}

// ---------------------------------------------------------------------------------------------
// Runtime harness (real routing; the area's state methods are stubs until GREEN)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Act {
    Edited(String),
    Save,
}

fn area_runtime(state: &TextAreaState) -> UiRuntime<Act> {
    let tree: Element<Act> = text_area(state)
        .key("area")
        .width(20)
        .rows(3)
        .on_area_edit(|s: TextAreaState| Act::Edited(s.text))
        .on_press(Act::Save);
    let mut runtime = UiRuntime::new(skins::BLACK_ICE);
    runtime
        .frame(&tree, UiEnvironment::default(), Duration::ZERO)
        .unwrap();
    runtime
}

// ---------------------------------------------------------------------------------------------
// GUARDS (pass today; must stay green)
// ---------------------------------------------------------------------------------------------

#[test]
fn guard_text_input_still_flattens_a_pasted_newline_to_a_space() {
    // TextInputState is single-line by contract (issue #252): the area is additive.
    let mut input = TextInputState::new();
    assert!(input.handle_event(&paste("a\nb")));
    assert_eq!(input.text, "a b");
    let mut input = TextInputState::new();
    assert!(input.handle_event(&paste("a\r\nb\rc")));
    assert_eq!(input.text, "a b c");
}

#[test]
fn guard_text_input_enter_is_not_text() {
    // The single-line editor never inserts a break on Enter (the element commits via on_press).
    let mut input = TextInputState::with_text("ab");
    assert!(!input.handle_event(&key(KeyCode::Enter)));
    assert_eq!(input.text, "ab");
}

#[test]
fn guard_text_area_state_has_no_commit_field() {
    // Exhaustive destructure: adding a field (e.g. a `committed` flag) breaks this on purpose.
    let TextAreaState {
        text,
        cursor_grapheme,
        scroll_row,
        goal_col,
    } = TextAreaState::default();
    assert_eq!(text, "");
    assert_eq!((cursor_grapheme, scroll_row, goal_col), (0, 0, None));
}

#[test]
fn guard_text_area_element_is_a_focusable_editor() {
    let runtime = area_runtime(&TextAreaState::new());
    assert_eq!(runtime.focus(), Some(&Key::from("area")));
}

#[test]
fn guard_enter_and_space_never_activate_a_text_area_press() {
    let mut runtime = area_runtime(&TextAreaState::with_text("a"));
    for ev in [key(KeyCode::Enter), ch(' ')] {
        let outcome = runtime.handle_event(&ev);
        assert!(outcome.consumed, "{ev:?} must stay inside the editor");
        assert!(
            !outcome.actions.contains(&Act::Save),
            "{ev:?} fired the press action: a text area never commits"
        );
    }
}

#[test]
fn guard_esc_and_ctrl_alt_enter_stay_with_the_application() {
    let mut runtime = area_runtime(&TextAreaState::with_text("a"));
    for ev in [
        key(KeyCode::Esc),
        key_mod(KeyCode::Enter, KeyModifiers::CONTROL),
        key_mod(KeyCode::Enter, KeyModifiers::ALT),
    ] {
        let outcome = runtime.handle_event(&ev);
        assert!(!outcome.consumed, "{ev:?} must reach the application");
        assert!(outcome.actions.is_empty());
    }
}

// ---------------------------------------------------------------------------------------------
// Wide-grapheme cells (GOD #168): a direct check of the continuation cell, not via painted text
// ---------------------------------------------------------------------------------------------

/// Every painted wide lead at column `k` is followed by a continuation cell at `k + 1` inside the
/// surface, and no continuation cell appears without a wide lead before it. `painted_row` skips
/// continuation cells and trims the row end, so it cannot see a blank written in their place.
fn assert_wide_cells(surface: &Surface, ctx: &str) {
    for y in 0..surface.height {
        let mut x = 0;
        while x < surface.width {
            let cell = surface.get(x, y).unwrap();
            assert!(
                !cell.is_continuation,
                "{ctx}: orphan continuation cell at ({x},{y})"
            );
            if cell.glyph.display_width == 2 {
                assert!(
                    x + 1 < surface.width,
                    "{ctx}: wide lead {:?} at ({x},{y}) has no room for its continuation",
                    cell.glyph.grapheme
                );
                assert!(
                    surface.get(x + 1, y).unwrap().is_continuation,
                    "{ctx}: cell ({},{y}) after wide lead {:?} is not a continuation",
                    x + 1,
                    cell.glyph.grapheme
                );
                x += 2;
            } else {
                x += 1;
            }
        }
    }
}

/// Columns of the wide leads painted on row `y`.
fn wide_leads(surface: &Surface, y: u16) -> Vec<u16> {
    (0..surface.width)
        .filter(|&x| {
            let c = surface.get(x, y).unwrap();
            !c.is_continuation && c.glyph.display_width == 2
        })
        .collect()
}

#[test]
fn wide_lead_is_followed_by_a_continuation_cell() {
    for focused in [false, true] {
        for width in [5u16, 6, 7, 9, 12, 40] {
            for (text, cursor) in [
                ("漢字ab日本語", 0),
                ("a漢b字c", 2),
                ("日本語日本語日本語", 4),
            ] {
                let state = state_at(text, cursor);
                let s = paint_area(&state, width, 8, focused);
                let ctx = format!("{text:?} width {width} focused {focused}");
                assert_wide_cells(&s, &ctx);
                let painted: usize = (0..s.height).map(|y| wide_leads(&s, y).len()).sum();
                let expected = text.graphemes(true).filter(|g| gw(g) == 2).count();
                assert_eq!(painted, expected, "{ctx}: wide graphemes painted");
            }
        }
    }
}

#[test]
fn wide_grapheme_in_the_last_usable_column_fits_or_wraps_whole() {
    // Width 5 wraps at 4 cells (column 4 is the cursor's). "ab漢" is 4 cells: 漢 takes columns
    // 2 and 3, the last usable pair, and nothing touches column 4.
    assert_eq!(rows_of("ab漢", 5), vec![(0, 3, 4)]);
    let s = paint_area(&state_at("ab漢", 0), 5, 3, false);
    assert_wide_cells(&s, "ab漢 @5");
    assert_eq!(wide_leads(&s, 0), vec![2]);
    assert!(s.get(3, 0).unwrap().is_continuation);
    let col4 = s.get(4, 0).unwrap();
    assert!(!col4.is_continuation && col4.glyph.display_width < 2);

    // "abc漢" would need 5 cells: 漢 wraps whole to row 1 and no half of it is left on row 0.
    assert_eq!(rows_of("abc漢", 5), vec![(0, 3, 3), (3, 4, 2)]);
    let s = paint_area(&state_at("abc漢", 0), 5, 3, false);
    assert_wide_cells(&s, "abc漢 @5");
    assert_eq!(wide_leads(&s, 0), Vec::<u16>::new());
    assert!(!s.get(3, 0).unwrap().is_continuation);
    assert_eq!(wide_leads(&s, 1), vec![0]);
    assert!(s.get(1, 1).unwrap().is_continuation);
}

#[test]
#[should_panic(expected = "is not a continuation")]
fn mutant_blank_in_place_of_a_continuation_is_caught() {
    // The oracle above must fail on a painter that writes a plain blank where the continuation
    // belongs. `get_mut` bypasses the surface's own wide-glyph repair, as a buggy painter would.
    let mut s = paint_area(&state_at("ab漢", 0), 5, 3, false);
    assert!(s.get(3, 0).unwrap().is_continuation, "precondition");
    *s.get_mut(3, 0).unwrap() = gibson::Cell::new(gibson::Glyph::space(), gibson::Style::default());
    assert_wide_cells(&s, "mutant");
}
