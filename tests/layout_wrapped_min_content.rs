//! #391: wrapped text must report its min-content width as its longest unbreakable
//! segment, not its unwrapped line width.
//!
//! Before the fix, a `flex_grow` column holding a long `text_wrapped` reply measured its
//! min-content at a 1000-column wrap width. So it could not shrink to its row, and it
//! overflowed the grid instead of wrapping. A `Node::stack` sibling (content-based minimum 0)
//! absorbed the overflow and collapsed to 0 columns. `NoWrap` text is unbreakable, so its
//! min-content stays its full width; that case is pinned here too.
use gibson::cell::Style;
use gibson::compute_layout;
use gibson::node::{Node, WrapMode};
use gibson::surface::{Rect, Surface};

/// About 140 columns of words, none longer than 8 columns.
fn reply() -> String {
    let words = [
        "the", "signal", "arrives", "before", "the", "frame,", "and", "nothing", "waits", "longer",
        "than", "needed.",
    ];
    (0..24)
        .map(|i| words[i % words.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

fn rect(n: &Node) -> Rect {
    n.computed_rect
}

/// `row(width cols)` = [fixed 24-column pane, flex_grow column holding `text`].
fn two_panes(cols: u16, rows: u16, pane: Node, text: Node) -> Node {
    let mut root = Node::row()
        .width(f32::from(cols))
        .height(f32::from(rows))
        .child(pane)
        .child(Node::col().flex_grow(1.0).child(text));
    compute_layout(&mut root, cols, rows).expect("layout");
    root
}

fn raster_pane() -> Node {
    Node::raster(Surface::new(24, 22))
}

#[test]
fn word_wrapped_reply_column_fits_its_row() {
    for (cols, rows) in [(80u16, 24u16), (120, 40), (40, 15)] {
        let root = two_panes(
            cols,
            rows,
            raster_pane(),
            Node::text_wrapped(reply(), Style::default(), WrapMode::WordWrap),
        );
        let (pane, col) = (rect(&root.children[0]), rect(&root.children[1]));
        assert_eq!(pane.width, 24, "{cols}x{rows}: raster pane width");
        assert_eq!(col.x, 24, "{cols}x{rows}: column x");
        assert_eq!(col.width, cols - 24, "{cols}x{rows}: column width");
        let text = rect(&root.children[1].children[0]);
        assert!(text.width <= cols - 24, "{cols}x{rows}: text {text:?}");
        assert!(
            text.height > 1,
            "{cols}x{rows}: reply must wrap, got {text:?}"
        );
    }
}

#[test]
fn stack_pane_beside_a_long_reply_keeps_its_declared_width() {
    for (cols, rows) in [(80u16, 24u16), (120, 40), (40, 15)] {
        let stack = Node::stack()
            .width(24.0)
            .height(f32::from(rows - 2))
            .child(raster_pane());
        let root = two_panes(
            cols,
            rows,
            stack,
            Node::text_wrapped(reply(), Style::default(), WrapMode::WordWrap),
        );
        assert_eq!(
            rect(&root.children[0]).width,
            24,
            "{cols}x{rows}: stack pane collapsed"
        );
        assert_eq!(rect(&root.children[1]).width, cols - 24, "{cols}x{rows}");
    }
}

#[test]
fn rich_wrapped_reply_column_fits_its_row() {
    let root = two_panes(
        80,
        24,
        raster_pane(),
        Node::rich_text_wrapped(reply(), WrapMode::WordWrap),
    );
    assert_eq!(rect(&root.children[1]).width, 56);
    assert!(rect(&root.children[1].children[0]).height > 1);
}

#[test]
fn char_wrapped_column_fits_its_row() {
    let long = "x".repeat(140);
    let root = two_panes(
        80,
        24,
        raster_pane(),
        Node::text_wrapped(long, Style::default(), WrapMode::CharWrap),
    );
    assert_eq!(rect(&root.children[1]).width, 56);
    // 140 columns at 56 per row: 3 rows.
    assert_eq!(rect(&root.children[1].children[0]).height, 3);
}

#[test]
fn word_wrap_min_content_is_the_longest_word() {
    // A row narrower than the longest word: the column cannot go below that word.
    let text = "ab abcdefghij abc"; // longest word: 10 columns
    let mut root = Node::row()
        .width(6.0)
        .height(5.0)
        .child(Node::col().child(Node::text_wrapped(
            text,
            Style::default(),
            WrapMode::WordWrap,
        )));
    compute_layout(&mut root, 6, 5).expect("layout");
    assert_eq!(rect(&root.children[0]).width, 10);
}

#[test]
fn no_wrap_text_still_reports_its_full_width() {
    // Unchanged by #391: NoWrap text is one unbreakable run.
    let root = two_panes(
        80,
        24,
        raster_pane(),
        Node::text("x".repeat(100), Style::default()),
    );
    assert_eq!(rect(&root.children[1].children[0]).width, 100);
}
