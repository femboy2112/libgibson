//! Gate F1 — a FAILED color-depth repaint must not mark the depth as physically emitted.
//!
//! `Renderer::render` once advanced `last_emitted_depth` *before* committing the output
//! transaction. A write error on the depth-change frame then left the renderer believing the new
//! depth was on screen: the next render computed `restyle == false`, emitted nothing (the RAW diff
//! is unchanged), and the stale old-depth SGR survived — a failed write marked a depth physically
//! emitted when it was not. The fix advances `last_emitted_depth` only AFTER a successful commit
//! (exactly as `previous_surface` already is), so a failed write is re-detected and restyled on
//! the retry.
//!
//! Driven through the real `Renderer -> TerminalTransaction` byte stream into a `vt100` virtual
//! terminal; a fail-once writer injects the write error. Proven for Inline and Fullscreen.

use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;

use gibson::capability::ColorDepth;
use gibson::cell::{Color, Style};
use gibson::node::{Node, WrapMode};
use gibson::renderer::Renderer;
use gibson::session::TerminalSession;

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    needle.len() <= hay.len() && hay.windows(needle.len()).any(|w| w == needle)
}

fn styled_root() -> Node {
    let style = Style::new()
        .fg(Color::Rgb(255, 128, 0))
        .bg(Color::Rgb(16, 32, 64))
        .bold();
    Node::col().child(Node::text_wrapped("Static", style, WrapMode::NoWrap))
}

/// A writer that fails its first `write` call once — nothing reaches the sink — then behaves
/// normally. Models a transient write/flush error on exactly one frame, with that frame's bytes
/// never reaching the terminal (so the virtual-terminal oracle's state stays physically truthful).
#[derive(Default)]
struct WriterState {
    armed: bool,
    sink: Vec<u8>,
}
struct SharedWriter(Rc<RefCell<WriterState>>);
impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut state = self.0.borrow_mut();
        if state.armed {
            state.armed = false;
            return Err(io::Error::other("injected one-shot write failure"));
        }
        state.sink.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Locate `needle` on the virtual screen, returning `(row, col)` of its first cell.
fn find_on_screen(screen: &vt100::Screen, cols: u16, needle: &str) -> Option<(u16, u16)> {
    let rows = screen.size().0;
    for r in 0..rows {
        let line: String = screen.rows(0, cols).nth(r as usize).unwrap_or_default();
        if let Some(byte_col) = line.find(needle) {
            // The content is ASCII and left-aligned, so the char index is the column.
            return Some((r, byte_col as u16));
        }
    }
    None
}

fn fail_once_retry_restyles(mode: gibson::RenderMode) {
    let cols = 24u16;
    let rows = 4u16;
    let mut renderer = Renderer::new(mode);
    let mut session = TerminalSession::headless(cols, rows); // defaults to truecolor
    let mut parser = vt100::Parser::new(rows, cols, 0);

    // A single writer represents the one virtual terminal across every frame.
    // Its shared control changes only failure behavior, not the destination.
    let state = Rc::new(RefCell::new(WriterState::default()));
    let mut writer = SharedWriter(Rc::clone(&state));
    let mut bound = renderer.bind_target(&mut session, &mut writer).unwrap();

    // --- Frame A at truecolor: establish the live frame on the virtual terminal. ---
    bound.render(&mut styled_root()).unwrap();
    let out_a = std::mem::take(&mut state.borrow_mut().sink);
    parser.process(&out_a);
    assert!(
        contains(&out_a, b"38;2;255;128;0"),
        "frame A must paint truecolor fg"
    );

    // --- The depth changes mid-session; the repaint frame's write FAILS. ---
    bound.set_color_depth(ColorDepth::Ansi16);
    state.borrow_mut().armed = true;
    let failed = bound.render(&mut styled_root());
    assert!(
        failed.is_err(),
        "the injected write failure must surface as a render error"
    );
    assert!(
        state.borrow().sink.is_empty(),
        "nothing reached the terminal on the failed frame"
    );

    // Retry the same frame at the same new depth on the same bound destination.
    // If a failed frame had advanced last_emitted_depth, the retry would skip
    // the restyle; the VT and byte assertions below would catch that regression.
    bound.render(&mut styled_root()).unwrap();
    let out_c = std::mem::take(&mut state.borrow_mut().sink);
    parser.process(&out_c);

    assert!(
        contains(&out_c, b"Static"),
        "the retry must re-emit the content (restyle after a failed depth write); got {} bytes\n{}",
        out_c.len(),
        String::from_utf8_lossy(&out_c)
    );
    assert!(
        !contains(&out_c, b"38;2;") && !contains(&out_c, b"48;2;"),
        "the restyle must drop truecolor SGR for Ansi16\n{}",
        String::from_utf8_lossy(&out_c)
    );

    // --- vt100 oracle: the content survived AND its fg is now a non-truecolor (Ansi16) color. ---
    let screen = parser.screen();
    let (r, c) = find_on_screen(screen, cols, "Static")
        .expect("virtual terminal must still show the content after the retry");
    let fg = screen.cell(r, c).expect("content cell exists").fgcolor();
    assert!(
        !matches!(fg, vt100::Color::Rgb(_, _, _)),
        "after the restyle the on-screen cell must carry an Ansi16 color, not truecolor; got {fg:?}"
    );
}

#[test]
fn inline_failed_depth_write_is_restyled_on_retry() {
    fail_once_retry_restyles(gibson::RenderMode::Inline);
}

#[test]
fn fullscreen_failed_depth_write_is_restyled_on_retry() {
    fail_once_retry_restyles(gibson::RenderMode::Fullscreen);
}
