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
