use gibson::cell::Style;
use gibson::context::Context;
use gibson::node::Node;
use gibson::renderer::Renderer;
use gibson::session::TerminalSession;

#[test]
fn test_commit_discards_from_live_surface() {
    let mut session = TerminalSession::new().unwrap();
    let mut renderer = Renderer::new(gibson::RenderMode::Inline, false);

    // Initial live render
    let mut root = Node::col().child(Node::text("Active Live Region", Style::default()));
    let _ = renderer.render(&mut root, &mut session).unwrap();

    assert!(renderer.live_region_height > 0);

    // Commit content to scrollback
    renderer
        .commit("Committed Immutable Response 1", &mut session)
        .unwrap();

    // After commit, live region MUST be completely reset!
    assert_eq!(renderer.live_region_height, 0);
    assert_eq!(renderer.last_cursor_y, 0);

    // Subsequent live render should start fresh
    let mut next_root = Node::col().child(Node::text("New Prompt Below", Style::default()));
    let (dirty, _total, _bytes, is_full) = renderer.render(&mut next_root, &mut session).unwrap();

    // Since previous surface was reset on commit, this is a clean first frame of the new live region
    assert!(is_full);
    assert!(dirty > 0);
}

#[test]
fn test_scrollback_invariance_diff_cost() {
    let mut ctx = Context::inline().unwrap();

    // Simulate 100 commits to scrollback
    for i in 0..100 {
        ctx.commit(&format!("Historic transcript line #{:04}", i))
            .unwrap();
    }

    // Measure diff metrics on a small live region
    let spinner_root = Node::col().child(Node::text("⠋ Live status", Style::default()));
    ctx.set_root(spinner_root);
    ctx.render().unwrap();

    let stats_1 = ctx.stats();

    // Tick the spinner
    let spinner_tick = Node::col().child(Node::text("⠙ Live status", Style::default()));
    ctx.set_root(spinner_tick);
    ctx.render().unwrap();

    let stats_2 = ctx.stats();

    let delta_dirty = stats_2.dirty_cells - stats_1.dirty_cells;
    // Only the single changed spinner glyph should be dirty (1 cell)
    assert_eq!(
        delta_dirty, 1,
        "Only 1 dirty cell should be patched regardless of transcript size!"
    );
}

#[test]
fn test_insert_before_live_preserves_live_surface() {
    let mut session = TerminalSession::new().unwrap();
    let mut renderer = Renderer::new(gibson::RenderMode::Inline, false);

    // Initial live render: prompt with input
    let mut root = Node::col().child(Node::text("Active Live Input Prompt", Style::default()));
    let (dirty1, _total, _bytes, is_full1) = renderer.render(&mut root, &mut session).unwrap();
    assert!(is_full1);
    assert!(dirty1 > 0);
    let original_height = renderer.live_region_height;
    assert!(original_height > 0);

    // Now insert a committed log ABOVE the active live region
    renderer
        .insert_before_live(&["[background] Task completed successfully"], &mut session)
        .unwrap();

    // After insert_before_live, live_region_height MUST be preserved!
    assert_eq!(
        renderer.live_region_height, original_height,
        "Live region height must be preserved after insert_before_live!"
    );

    // Next render of the same root should emit 0 dirty cells because previous_surface was preserved!
    let mut next_root = Node::col().child(Node::text("Active Live Input Prompt", Style::default()));
    let (dirty2, _total, bytes2, is_full2) = renderer.render(&mut next_root, &mut session).unwrap();
    assert!(
        !is_full2,
        "Should NOT be a full repaint; surface was preserved!"
    );
    assert_eq!(dirty2, 0, "Zero dirty cells since prompt didn't change!");
    assert_eq!(bytes2, 0, "Zero bytes emitted when content is identical!");
}
