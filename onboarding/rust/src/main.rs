use gibson::{cell::Style, context::{Context, RenderMode}, node::Node};

fn main() -> std::io::Result<()> {
    let mut ctx = Context::headless(RenderMode::Fullscreen, 40, 8);
    ctx.set_root(Node::col().child(Node::text("Hello, LibGibson", Style::default())));
    ctx.render()?;
    assert_eq!(ctx.frame_size(), Some((40, 8)));
    assert_eq!(ctx.frame_cell(0, 0).map(|c| c.glyph), Some("H"));
    println!("Rust headless frame 40x8 OK");
    Ok(())
}
