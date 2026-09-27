//! External-presentation cadence beacon for temporal-rendering research.
//!
//! This example does **not** infer display refresh from PTY timing. It emits a
//! bounded Gray-code strip at a phase-locked application cadence so an external
//! high-speed camera/photodiode workflow can measure which application frames
//! actually become distinct terminal presentations.
//!
//! Example:
//!   cargo run --release --example temporal_cadence_beacon -- --fps=120 --seconds=5
//!   cargo run --release --example temporal_cadence_beacon -- --fps=120 --seconds=5 --no-sync
//!
//! The strip changes one Gray-code bit per sequential frame, minimizing wire
//! traffic and reducing the chance that the measurement itself becomes the
//! bottleneck. This is still a high-contrast dynamic research stimulus; keep the
//! region bounded and stop if it is uncomfortable.

use gibson::{
    Cell, Color, Context, FramePacing, Node, Style, Surface,
};
use std::io;
use std::sync::Arc;
use std::time::Duration;

const BITS: usize = 12;
const CELL_W: u16 = 2;
const CELL_H: u16 = 2;

fn arg_u32(name: &str, default: u32) -> u32 {
    let prefix = format!("--{name}=");
    std::env::args()
        .find_map(|arg| arg.strip_prefix(&prefix).and_then(|v| v.parse().ok()))
        .unwrap_or(default)
}

fn has(flag: &str) -> bool {
    std::env::args().any(|arg| arg == flag)
}

fn beacon_surface(frame: u64) -> Surface {
    let width = BITS as u16 * CELL_W;
    let height = CELL_H;
    let mut surface = Surface::new(width, height);
    let gray = frame ^ (frame >> 1);

    for bit in 0..BITS {
        let on = (gray >> bit) & 1 != 0;
        let level = if on { 220 } else { 28 };
        let style = Style::default().bg(Color::Rgb(level, level, level));
        let x0 = (BITS - 1 - bit) as u16 * CELL_W;
        for y in 0..CELL_H {
            for dx in 0..CELL_W {
                surface.set_cell(x0 + dx, y, Cell::space(style));
            }
        }
    }
    surface
}

fn main() -> io::Result<()> {
    let fps = arg_u32("fps", 120).clamp(1, 1000);
    let seconds = arg_u32("seconds", 5).clamp(1, 60);
    let frames = fps as u64 * seconds as u64;
    let sync = !has("--no-sync");

    eprintln!(
        "temporal cadence beacon: {fps} fps for {seconds}s, sync_updates={sync}; film the {}x{}-cell Gray-code strip",
        BITS as u16 * CELL_W,
        CELL_H
    );

    let mut ctx = Context::fullscreen()?;
    if !ctx.is_interactive() {
        return Err(io::Error::other(
            "temporal_cadence_beacon requires an interactive TTY; presentation cadence must be measured externally",
        ));
    }

    ctx.set_sync_updates(sync);
    ctx.set_max_fps(fps);
    ctx.set_frame_pacing(FramePacing::PhaseLocked);

    for frame in 0..frames {
        ctx.set_root(Node::surface(Arc::new(beacon_surface(frame))));

        while !ctx.render_if_due()? {
            let wait = ctx.time_until_next_frame();
            if wait.is_zero() {
                std::thread::yield_now();
            } else {
                std::thread::sleep(wait.min(Duration::from_millis(2)));
            }
        }
    }

    ctx.restore()?;
    Ok(())
}
