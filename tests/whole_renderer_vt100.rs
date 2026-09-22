//! Whole-renderer tests: `Renderer` -> `TerminalTransaction` -> complete byte
//! stream -> `vt100` virtual terminal.
//!
//! Unlike `screen_state_vt100.rs` (which exercises `Surface -> Diff -> AnsiCompiler`
//! in isolation), these tests drive the *entire* renderer through a controlled,
//! headless terminal geometry and assert on the resulting virtual screen.

use gibson::cell::{Color, Line, RichText, Span, Style};
use gibson::node::{Node, WrapMode};
use gibson::renderer::{InsertStrategy, Renderer};
use gibson::session::TerminalSession;

struct Harness {
    renderer: Renderer,
    session: TerminalSession,
    parser: vt100::Parser,
}

impl Harness {
    fn new(cols: u16, rows: u16) -> Self {
        Self {
            renderer: Renderer::new(gibson::RenderMode::Inline),
            session: TerminalSession::headless(cols, rows),
            parser: vt100::Parser::new(rows, cols, 0),
        }
    }

    fn render(&mut self, root: &mut Node) -> (usize, usize, bool) {
        let mut out = Vec::new();
        let (dirty, _total, bytes, full, _) = self
            .renderer
            .render(root, &mut self.session, &mut out)
            .unwrap();
        self.parser.process(&out);
        (dirty, bytes, full)
    }

    fn insert(&mut self, lines: &[&str]) -> (usize, InsertStrategy) {
        let mut out = Vec::new();
        let (bytes, strategy) = self
            .renderer
            .insert_before_live(lines, &mut self.session, &mut out)
            .unwrap();
        self.parser.process(&out);
        (bytes, strategy)
    }

    fn commit(&mut self, text: &str) -> usize {
        let mut out = Vec::new();
        let bytes = self
            .renderer
            .commit_text(text, &mut self.session, &mut out)
            .unwrap();
        self.parser.process(&out);
        bytes
    }

    fn screen_rows(&self) -> Vec<String> {
        let (_, cols) = (self.parser.screen().size().0, self.parser.screen().size().1);
        self.parser.screen().rows(0, cols).collect()
    }

    fn cursor(&self) -> (u16, u16) {
        self.parser.screen().cursor_position()
    }

    fn hide_cursor(&self) -> bool {
        self.parser.screen().hide_cursor()
    }
}

fn live_root() -> Node {
    Node::col()
        .child(Node::text("LIVE-TOP", Style::default()))
        .child(Node::text("LIVE-MID", Style::default()))
        .child(Node::text("LIVE-BOT", Style::default()))
}

#[test]
fn whole_renderer_first_inline_frame_appears() {
    let mut h = Harness::new(80, 24);
    let mut root = live_root();
    let (dirty, bytes, full) = h.render(&mut root);
    assert!(full, "first frame must be a full repaint");
    assert!(dirty > 0);
    assert!(bytes > 0);

    let rows = h.screen_rows();
    assert!(rows[0].starts_with("LIVE-TOP"), "rows: {:?}", &rows[..4]);
    assert!(rows[1].starts_with("LIVE-MID"));
    assert!(rows[2].starts_with("LIVE-BOT"));
    // Cursor parked at the bottom row of the live region, hidden.
    assert_eq!(h.cursor(), (2, 0));
    assert!(h.hide_cursor());
}

#[test]
fn whole_renderer_second_frame_one_cell_diff() {
    let mut h = Harness::new(80, 24);
    let mut root = Node::col().child(Node::text("SPINNER ⠋ status", Style::default()));
    h.render(&mut root);

    let mut tick = Node::col().child(Node::text("SPINNER ⠙ status", Style::default()));
    let (dirty, bytes, full) = h.render(&mut tick);
    assert!(!full);
    assert_eq!(dirty, 1, "exactly one glyph changed");
    assert!(bytes > 0);
    assert!(h.screen_rows()[0].contains("⠙"));
}

#[test]
fn whole_renderer_cursor_visibility_is_physical_truth() {
    use gibson::node::NodeKind;

    // Frame 1: two plain cells, no cursor requested -> hidden.
    let mut h = Harness::new(80, 24);
    let mut plain = Node::col().child(Node::text("ab", Style::default()));
    h.render(&mut plain);
    assert!(h.hide_cursor());

    // Frame 2: identical cells, but a cursor is requested on a cell that is
    // already a default space (cursor style == text style), so the framebuffer
    // diff is genuinely empty. We MUST still emit visibility/position.
    let mut input = Node::col().child(Node::new(NodeKind::TextInput {
        value: "ab".to_string(),
        cursor_grapheme: 2,
        placeholder: None,
        style: Style::default(),
        placeholder_style: Style::default(),
        cursor_style: Style::default(),
        scroll_offset: 0,
    }));
    let (dirty, bytes, full) = h.render(&mut input);
    assert!(!full);
    assert_eq!(dirty, 0, "framebuffer diff is empty...");
    assert!(
        bytes > 0,
        "...but cursor state changed so we must still emit"
    );
    assert!(!h.hide_cursor(), "cursor must now be visible");
}

#[test]
fn whole_renderer_live_region_growth_and_shrink() {
    let mut h = Harness::new(80, 24);
    let mut small = Node::col().child(Node::text("one", Style::default()));
    h.render(&mut small);
    assert_eq!(h.renderer.live_region_height, 1);

    let mut big = Node::col()
        .child(Node::text("one", Style::default()))
        .child(Node::text("two", Style::default()))
        .child(Node::text("three", Style::default()));
    h.render(&mut big);
    assert_eq!(h.renderer.live_region_height, 3);
    let rows = h.screen_rows();
    assert!(rows[0].starts_with("one"), "rows: {rows:?}");
    assert!(rows[2].starts_with("three"), "rows: {rows:?}");

    let mut small_again = Node::col().child(Node::text("one", Style::default()));
    h.render(&mut small_again);
    assert_eq!(h.renderer.live_region_height, 1);
    let rows = h.screen_rows();
    assert!(rows[0].starts_with("one"));
    // Rows below must be cleared, not left with stale content.
    assert!(
        !rows[2].contains("three"),
        "stale row not cleared: {:?}",
        rows[2]
    );
}

#[test]
fn whole_renderer_commit_moves_live_to_scrollback() {
    let mut h = Harness::new(80, 24);
    let mut root = live_root();
    h.render(&mut root);

    let bytes = h.commit("Committed result A");
    assert!(bytes > 0);
    assert_eq!(h.renderer.live_region_height, 0);

    let rows = h.screen_rows();
    let all = rows.join("\n");
    assert!(all.contains("Committed result A"));
    assert!(!all.contains("LIVE-TOP"), "live content must be purged");
}

// ---------------------------------------------------------------------------
// InsertLineFastPath: directly proven to avoid live repaint
// ---------------------------------------------------------------------------

#[test]
fn insert_line_fast_path_inserts_history_without_repainting_live() {
    let mut h = Harness::new(80, 24);
    let mut root = live_root();
    h.render(&mut root);
    let baseline = h.renderer.live_region_height;
    assert_eq!(baseline, 3);

    // Capture the insertion byte stream separately so we can prove the live
    // framebuffer was not re-emitted.
    let mut out: Vec<u8> = Vec::new();
    let (bytes, strategy) = h
        .renderer
        .insert_before_live(&["HISTORY-1"], &mut h.session, &mut out)
        .unwrap();
    assert_eq!(
        strategy,
        InsertStrategy::InsertLineFastPath,
        "must use fast path"
    );
    let s = String::from_utf8_lossy(&out);
    assert!(
        !s.contains("LIVE-TOP"),
        "fast path must NOT repaint live content"
    );
    assert!(
        !s.contains("LIVE-BOT"),
        "fast path must NOT repaint live content"
    );
    assert!(s.contains("HISTORY-1"));
    assert!(bytes > 0);
    h.parser.process(&out);

    // Virtual screen: history row directly above the (unchanged) live block.
    let rows = h.screen_rows();
    assert!(rows[0].starts_with("HISTORY-1"), "rows: {:?}", &rows[..5]);
    assert!(rows[1].starts_with("LIVE-TOP"));
    assert!(rows[2].starts_with("LIVE-MID"));
    assert!(rows[3].starts_with("LIVE-BOT"));

    // Cursor restored exactly to the previous relative position (region row 2).
    assert_eq!(h.cursor(), (3, 0));

    // previous_surface baseline preserved: next render emits nothing.
    let mut same = live_root();
    let (dirty2, bytes2, full2) = h.render(&mut same);
    assert!(!full2);
    assert_eq!(dirty2, 0);
    assert_eq!(bytes2, 0, "insertion must preserve the diff baseline");
}

#[test]
fn insert_line_fast_path_multiple_rows_and_cursor_restore() {
    let mut h = Harness::new(80, 24);
    // Live region with a visible cursor so we can prove exact restoration.
    let mut root = live_root();
    h.render(&mut root);
    // Now request a cursor in the middle row.
    let mut with_input = Node::col()
        .child(Node::text("LIVE-TOP", Style::default()))
        .child(Node::text_input("x", 1, None, Style::default()))
        .child(Node::text("LIVE-BOT", Style::default()));
    h.render(&mut with_input);
    let cursor_before = h.cursor();

    let (_, strategy) = h.insert(&["HIST-1", "HIST-2"]);
    assert_eq!(strategy, InsertStrategy::InsertLineFastPath);

    let rows = h.screen_rows();
    assert!(rows[0].starts_with("HIST-1"));
    assert!(rows[1].starts_with("HIST-2"));
    assert!(rows[2].starts_with("LIVE-TOP"));
    // Cursor was at row 1 relative to region; region top moved to row 2, so now row 3.
    assert_eq!(h.cursor().0, cursor_before.0 + 2);
    assert_eq!(h.cursor().1, cursor_before.1);
}

// ---------------------------------------------------------------------------
// RepaintFallback
// ---------------------------------------------------------------------------

#[test]
fn repaint_fallback_used_when_no_room_and_restores_live_and_cursor() {
    // Tiny terminal: cannot fit history + live, so the fallback must engage.
    let mut h = Harness::new(40, 6);
    let mut root = live_root();
    h.render(&mut root);
    assert_eq!(h.renderer.live_region_height, 3);

    let mut out: Vec<u8> = Vec::new();
    let (_bytes, strategy) = h
        .renderer
        .insert_before_live(&["A", "B", "C", "D", "E"], &mut h.session, &mut out)
        .unwrap();
    assert_eq!(strategy, InsertStrategy::RepaintFallback);
    let s = String::from_utf8_lossy(&out);
    // Fallback necessarily repaints the live framebuffer.
    assert!(s.contains("LIVE-TOP"), "fallback must repaint live content");
    h.parser.process(&out);

    let rows = h.screen_rows();
    let all = rows.join("\n");
    assert!(all.contains("LIVE-TOP"));
    assert!(all.contains("LIVE-BOT"));
    // The last committed history row should be visible above the live block.
    assert!(all.contains("E"), "expected last history line: {:?}", all);

    // The diff baseline is preserved, so the next identical frame emits nothing.
    let mut same = live_root();
    let (dirty2, bytes2, full2) = h.render(&mut same);
    assert!(!full2);
    assert_eq!(dirty2, 0);
    assert_eq!(bytes2, 0);
}

#[test]
fn insert_strategy_is_reported() {
    let mut h = Harness::new(80, 24);
    let mut root = live_root();
    h.render(&mut root);
    assert_eq!(h.renderer.last_insert_strategy, None);
    h.insert(&["x"]);
    assert_eq!(
        h.renderer.last_insert_strategy,
        Some(InsertStrategy::InsertLineFastPath)
    );
    assert_eq!(h.renderer.fast_insertions, 1);
}

// ---------------------------------------------------------------------------
// Resize resynchronization: anchor invalidation + re-establishment
// ---------------------------------------------------------------------------

#[test]
fn resize_invalidates_anchor_and_resyncs() {
    let mut h = Harness::new(120, 30);
    let mut root = live_root();
    h.render(&mut root);
    assert_eq!(h.renderer.anchor_resyncs, 0);

    // Simulate terminal resize and tell both the session and the virtual terminal.
    h.session.set_terminal_size(40, 12);
    h.parser.set_size(12, 40);

    let (_, _, full) = h.render(&mut root);
    assert!(full, "resize must force a full re-anchor repaint");
    assert_eq!(
        h.renderer.anchor_resyncs, 1,
        "anchor resync must be counted"
    );

    let rows = h.screen_rows();
    let all = rows.join("\n");
    assert!(all.contains("LIVE-TOP"));
    assert!(all.contains("LIVE-BOT"));

    // A further render at the same size is a normal differential frame.
    let (_, _, full2) = h.render(&mut root);
    assert!(!full2);
    assert_eq!(h.renderer.anchor_resyncs, 1);
}

#[test]
fn resize_to_narrow_width_reflows_without_panic() {
    let mut h = Harness::new(120, 30);
    let mut root = Node::col().child(Node::text_wrapped(
        "a very long line of text that must wrap when the terminal becomes narrow",
        Style::default(),
        WrapMode::WordWrap,
    ));
    h.render(&mut root);
    h.session.set_terminal_size(20, 8);
    h.parser.set_size(8, 20);
    h.render(&mut root);
    // No panic; all written rows fit within width.
    for row in h.screen_rows() {
        assert!(unicode_width::UnicodeWidthStr::width(row.as_str()) <= 20);
    }
}

// ---------------------------------------------------------------------------
// Unicode / right margin through the whole pipeline
// ---------------------------------------------------------------------------

#[test]
fn whole_renderer_right_margin_no_autowrap_glitch() {
    let mut h = Harness::new(20, 8);
    let mut root = Node::col().child(Node::text("X".repeat(20), Style::default()));
    h.render(&mut root);
    let rows = h.screen_rows();
    assert_eq!(&rows[0][..20], &"X".repeat(20));
}

#[test]
fn whole_renderer_cjk_and_emoji() {
    let mut h = Harness::new(40, 8);
    let mut root = Node::col().child(Node::rich_text(
        RichText::new().line(
            Line::new()
                .span(Span::styled("🦀 Rust ", Style::new().fg(Color::Green)))
                .span(Span::styled("你好世界", Style::new().bold())),
        ),
    ));
    h.render(&mut root);
    let rows = h.screen_rows();
    assert!(rows[0].contains("🦀"));
    assert!(rows[0].contains("你好世界"));
}

#[test]
fn whole_renderer_long_input_scrolls_with_hostile_unicode() {
    // A long mixed Unicode line in a narrow input must scroll smoothly and keep
    // the cursor inside the visible width.
    let text = "cargo clippy --all-targets --all-features 🦀 你好世界 e\u{0301} 🇺🇸";
    let graphemes = {
        use unicode_segmentation::UnicodeSegmentation;
        text.graphemes(true).count()
    };
    let mut h = Harness::new(24, 8);
    let mut root = Node::col()
        .child(Node::text_input(text, graphemes, Some("prompt"), Style::default()).width(20.0));
    let mut out = Vec::new();
    let (_d, _t, _b, _f, paint) = h
        .renderer
        .render(&mut root, &mut h.session, &mut out)
        .unwrap();
    h.parser.process(&out);
    let (cx, _cy) = paint.cursor_position.expect("cursor requested");
    assert!(cx < 20, "cursor x {} escaped the input width", cx);
}

#[test]
fn whole_renderer_long_input_survives_resize() {
    use unicode_segmentation::UnicodeSegmentation;
    let text = "cargo clippy --all-targets --all-features 🦀 你好世界 e\u{0301} 🇺🇸 tail";
    let graphemes = text.graphemes(true).count();

    let mut h = Harness::new(80, 24);
    let mut root = Node::col().child(Node::text_input(
        text,
        graphemes,
        Some("prompt"),
        Style::default(),
    ));
    h.render(&mut root);

    // Shrink dramatically; the input must re-scroll and keep the cursor in bounds.
    h.session.set_terminal_size(24, 8);
    h.parser.set_size(8, 24);
    let mut out = Vec::new();
    let (_d, _t, _b, _f, paint) = h
        .renderer
        .render(&mut root, &mut h.session, &mut out)
        .unwrap();
    let (cx, cy) = paint.cursor_position.expect("cursor requested");
    assert!(cx < 24, "cursor x {cx} escaped width after resize");
    assert!(cy < 8, "cursor y {cy} escaped height after resize");
    assert_eq!(h.renderer.anchor_resyncs, 1);
}
