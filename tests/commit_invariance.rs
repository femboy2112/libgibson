use gibson::cell::Style;
use gibson::node::Node;
use gibson::renderer::Renderer;
use gibson::session::TerminalSession;

fn headless() -> (TerminalSession, Renderer) {
    (
        TerminalSession::headless(80, 24),
        Renderer::new(gibson::RenderMode::Inline),
    )
}

#[test]
fn test_commit_discards_from_live_surface() {
    let (mut session, mut renderer) = headless();
    let mut out: Vec<u8> = Vec::new();
    let mut target = renderer.bind_target(&mut session, &mut out).unwrap();

    let mut root = Node::col().child(Node::text("Active Live Region", Style::default()));
    let _ = target.render(&mut root).unwrap();
    assert!(target.live_region_height() > 0);

    target
        .commit_text("Committed Immutable Response 1")
        .unwrap();

    assert_eq!(target.live_region_height(), 0);
    assert_eq!(target.last_cursor_y(), 0);

    let mut next_root = Node::col().child(Node::text("New Prompt Below", Style::default()));
    let (dirty, _total, _bytes, is_full, _) = target.render(&mut next_root).unwrap();
    assert!(
        is_full,
        "after commit the next live frame is a clean first frame"
    );
    assert!(dirty > 0);
}

#[test]
fn test_scrollback_invariance_diff_cost() {
    // Simulate a long transcript in scrollback, then verify a one-cell live
    // mutation still costs exactly one dirty cell (O(1) w.r.t. transcript size).
    let (mut session, mut renderer) = headless();
    let mut out: Vec<u8> = Vec::new();
    let mut target = renderer.bind_target(&mut session, &mut out).unwrap();

    for i in 0..200 {
        target
            .commit_text(&format!("Historic transcript line #{:04}", i))
            .unwrap();
    }

    let mut root = Node::col().child(Node::text("⠋ Live status", Style::default()));
    target.render(&mut root).unwrap();

    let mut tick = Node::col().child(Node::text("⠙ Live status", Style::default()));
    let (dirty, _total, _bytes, _full, _) = target.render(&mut tick).unwrap();
    assert_eq!(
        dirty, 1,
        "Only the single changed spinner glyph should be dirty regardless of transcript size"
    );
}

#[test]
fn test_insert_before_live_preserves_live_surface() {
    let (mut session, mut renderer) = headless();
    let mut out: Vec<u8> = Vec::new();
    let mut target = renderer.bind_target(&mut session, &mut out).unwrap();

    let mut root = Node::col().child(Node::text("Active Live Input Prompt", Style::default()));
    let (dirty1, _total, _bytes, is_full1, _) = target.render(&mut root).unwrap();
    assert!(is_full1);
    assert!(dirty1 > 0);
    let original_height = target.live_region_height();
    assert!(original_height > 0);

    let (_bytes, _strategy) = target
        .insert_raw_lines_before_live_unchecked(&["[background] Task completed successfully"])
        .unwrap();

    assert_eq!(
        target.live_region_height(),
        original_height,
        "Live region height must be preserved after insert_before_live"
    );

    let mut next_root = Node::col().child(Node::text("Active Live Input Prompt", Style::default()));
    let (dirty2, _total, bytes2, is_full2, _) = target.render(&mut next_root).unwrap();
    assert!(
        !is_full2,
        "Should NOT be a full repaint; surface was preserved"
    );
    assert_eq!(dirty2, 0, "Zero dirty cells since prompt did not change");
    assert_eq!(bytes2, 0, "Zero bytes emitted when content is identical");
}

#[test]
fn test_commit_does_not_interpret_control_characters() {
    // Structured commit_text must neutralize ESC/OSC/CSI rather than pass them through.
    let (mut session, mut renderer) = headless();
    let mut out: Vec<u8> = Vec::new();
    renderer
        .bind_target(&mut session, &mut out)
        .unwrap()
        .commit_text("safe \x1b[31mRED\x1b[0m \x1b]0;pwn\x07")
        .unwrap();
    // The output must contain the literal word RED but no SGR red or OSC introducer.
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("RED"));
    assert!(
        !s.contains("\x1b[31m"),
        "structured text must not emit injected SGR"
    );
    assert!(
        !s.contains("\x1b]"),
        "structured text must not emit injected OSC"
    );
}
