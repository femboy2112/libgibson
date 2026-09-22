use gibson::{Context, RenderMode};
use gibson::node::Node;
use gibson::cell::{Style, Color};
use std::io;
use std::time::Duration;

fn main() -> io::Result<()> {
    let mut ctx = Context::new(RenderMode::Inline)?;
    let mut text = String::new();

    loop {
        let root = Node::col()
            .child(Node::text("Resize Torture App", Style::new().fg(Color::Yellow)))
            .child(Node::text_input(&text, text.len(), Some("Type here..."), Style::default()));

        ctx.set_root(root);
        let paint_ctx = ctx.render()?;
        
        
        if crossterm::event::poll(Duration::from_millis(50))? {
            if let crossterm::event::Event::Key(ke) = crossterm::event::read()? {
                if let crossterm::event::KeyCode::Char(c) = ke.code {
                    if c == 'q' { break; }
                    if c == 'i' {
                        ctx.insert_before_live(&[
                            "--- ASYNC INSERTION ---",
                            "Data received from network",
                        ])?;
                    } else {
                        text.push(c);
                    }
                }
            }
        }
    }
    
    ctx.commit("Session ended cleanly.")?;
    Ok(())
}
