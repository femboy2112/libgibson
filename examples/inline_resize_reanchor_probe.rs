//! Issue #403 probe: an inline live region that fills the terminal (`rows - 1` rows) across
//! resizes, above one committed history line.
//!
//! Every frame `n` shows rows `f<n>-r0`..`f<n>-r<rows-2>`. A delivered `Resize` starts frame
//! `n + 1` at the new height. With `--clear`, `Context::clear_live_region` runs after each
//! resize, before the new frame (the cleanup pattern from the issue). `--reclaim` opts into
//! `InlineResizePolicy::ReclaimLiveRows`; without it the default `PreserveAbove` applies. `q`
//! exits.
//!
//! Driven by `tests/pty_inline_resize_reanchor.rs` (vt100 as the terminal) and by the ignored
//! tmux test in the same file (tmux as the terminal).

use gibson::cell::Style;
use gibson::context::Context;
use gibson::input::{Event, KeyCode};
use gibson::node::Node;
use gibson::InlineResizePolicy;
use std::time::{Duration, Instant};

const HISTORY: &str = "SYNTHETIC-HISTORY";

fn frame(step: usize, rows: u16) -> Node {
    Node::col().children(
        (0..rows.saturating_sub(1).max(1))
            .map(|i| Node::text(format!("f{step}-r{i}"), Style::default()))
            .collect(),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let clear = std::env::args().any(|a| a == "--clear");
    let reclaim = std::env::args().any(|a| a == "--reclaim");
    let deadline = Instant::now() + Duration::from_secs(60);
    let (_, rows) = crossterm::terminal::size()?;

    let mut ctx = Context::inline()?;
    if reclaim {
        ctx.set_inline_resize_policy(InlineResizePolicy::ReclaimLiveRows);
    }
    ctx.commit_text(HISTORY)?;
    let mut step = 0;
    ctx.set_root(frame(step, rows));
    ctx.request_render();

    while Instant::now() < deadline {
        match ctx.run_once(Duration::from_millis(50))? {
            Some(Event::Key(k)) if matches!(k.code, KeyCode::Char('q') | KeyCode::Esc) => break,
            Some(Event::Resize(_, rows)) => {
                step += 1;
                if clear {
                    ctx.clear_live_region()?;
                }
                ctx.set_root(frame(step, rows));
                ctx.request_render();
            }
            _ => {}
        }
    }
    ctx.restore()?;
    Ok(())
}
