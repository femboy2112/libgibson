use gibson::cell::Style;
use gibson::node::Node;
use gibson::renderer::Renderer;
use gibson::session::TerminalSession;

#[test]
fn test_non_tty_suppresses_interactive_ansi() {
    let mut session = TerminalSession::new().unwrap();
    // Explicitly simulate non-interactive / redirected environment
    session.is_tty = false;

    let mut renderer = Renderer::new(gibson::RenderMode::Inline, false);

    let mut root = Node::col().child(Node::text("Active Live Frame", Style::default()));

    // In non-TTY mode, live interactive frames return 0 emitted bytes
    let (_dirty, _total, bytes, _full) = renderer.render(&mut root, &mut session).unwrap();
    assert_eq!(
        bytes, 0,
        "Non-interactive stdout must not emit live ANSI frames"
    );
}
