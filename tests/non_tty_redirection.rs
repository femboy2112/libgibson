use gibson::cell::Style;
use gibson::node::Node;
use gibson::renderer::Renderer;
use gibson::session::TerminalSession;

#[test]
fn test_non_tty_suppresses_interactive_ansi() {
    let mut session = TerminalSession::new().unwrap();
    // Explicitly simulate non-interactive / redirected environment
    session.is_tty = false;

    let mut renderer = Renderer::new(gibson::RenderMode::Inline);

    let mut root = Node::col().child(Node::text("Active Live Frame", Style::default()));

    // In non-TTY mode, live interactive frames return 0 emitted bytes
    let (_dirty, _total, bytes, _full, _paint_ctx) = renderer
        .render(&mut root, &mut session, &mut std::io::stdout())
        .unwrap();
    assert_eq!(
        bytes, 0,
        "Non-interactive stdout must not emit live ANSI frames"
    );
}

#[test]
fn test_non_tty_structured_commit_zero_escapes() {
    use gibson::cell::{Color, Line, RichText};
    use gibson::renderer::render_node_to_lines;

    let mut rich_node = Node::col()
        .child(Node::rule(
            Some("Build Finished"),
            Style::new().bold().fg(Color::Green),
        ))
        .child(
            Node::rail(Style::new().fg(Color::Cyan)).child(Node::rich_text(
                RichText::new()
                    .line(Line::styled(
                        "All targets passed without warnings.",
                        Style::new().bold(),
                    ))
                    .line(Line::styled(
                        "Artifact ready: target/release/app",
                        Style::new().dim(),
                    )),
            )),
        );

    // Render node to lines for non-TTY
    let lines = render_node_to_lines(&mut rich_node, false, 80).unwrap();
    assert!(!lines.is_empty());

    for line in &lines {
        // Absolutely zero escape sequences allowed in non-TTY output!
        assert!(
            !line.contains('\x1b'),
            "Line in non-TTY redirection contained escape byte: {}",
            line
        );
    }
}
