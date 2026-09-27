//! External-presentation cadence beacon for temporal-rendering research.
//!
//! This example does **not** infer display refresh from PTY timing. It emits a
//! bounded, high-contrast stimulus at a phase-locked application cadence so an
//! external high-speed camera / photodiode workflow can measure which application
//! frames actually become distinct terminal presentations — the observable
//! LibGibson cannot see through the PTY.
//!
//! Examples:
//!   cargo run --release --example temporal_cadence_beacon -- --fps=120 --seconds=5
//!   cargo run --release --example temporal_cadence_beacon -- --fps=120 --no-sync
//!   cargo run --release --example temporal_cadence_beacon -- --fps=120 --photodiode \
//!       --region-size=4 --pattern=gray --csv=/tmp/beacon.csv
//!
//! The Gray-code strip changes one bit per sequential frame (minimal wire churn),
//! so a slow-motion capture can decode the emitted frame index. `--photodiode`
//! adds a patch that inverts every frame — a clean square wave at fps/2 for a
//! photodiode aimed at it. `--csv` logs, per frame, the intended deadline, the
//! actual emission time, the split generation/write durations, bytes emitted,
//! exact changed cells and any missed phase-locked deadlines, so the emitted
//! record can be correlated against the externally captured presentation.
//!
//! This is a high-contrast dynamic stimulus. Keep the region bounded and stop if
//! it is uncomfortable.

use gibson::{Cell, Color, Context, FramePacing, Node, Style, Surface};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

const BITS: usize = 12;

#[derive(Clone, Copy)]
enum Pattern {
    /// Reflected binary (one bit changes per step): decodable, minimal churn.
    Gray,
    /// Plain binary counter (many bits change per step): a churn stress pattern.
    Counter,
}

fn arg_u32(name: &str, default: u32) -> u32 {
    let prefix = format!("--{name}=");
    std::env::args()
        .find_map(|arg| arg.strip_prefix(&prefix).and_then(|v| v.parse().ok()))
        .unwrap_or(default)
}

fn arg_str(name: &str) -> Option<String> {
    let prefix = format!("--{name}=");
    std::env::args().find_map(|arg| arg.strip_prefix(&prefix).map(|v| v.to_string()))
}

fn has(flag: &str) -> bool {
    std::env::args().any(|arg| arg == flag)
}

fn beacon_surface(frame: u64, cell: u16, pattern: Pattern, photodiode: bool) -> Surface {
    let width = BITS as u16 * cell;
    let strip_h = cell;
    let patch_h = if photodiode { cell } else { 0 };
    let mut surface = Surface::new(width, strip_h + patch_h);

    let code = match pattern {
        Pattern::Gray => frame ^ (frame >> 1),
        Pattern::Counter => frame,
    };
    for bit in 0..BITS {
        let on = (code >> bit) & 1 != 0;
        let level = if on { 220 } else { 28 };
        let style = Style::default().bg(Color::Rgb(level, level, level));
        // Bit 0 on the right, so the strip reads like a binary number.
        let x0 = (BITS - 1 - bit) as u16 * cell;
        for y in 0..strip_h {
            for dx in 0..cell {
                surface.set_cell(x0 + dx, y, Cell::space(style));
            }
        }
    }

    if photodiode {
        // A patch that inverts every frame: a square wave at fps/2 giving a
        // photodiode a clean per-frame edge to count.
        let level = if frame % 2 == 0 { 255 } else { 0 };
        let style = Style::default().bg(Color::Rgb(level, level, level));
        for y in strip_h..strip_h + patch_h {
            for x in 0..width {
                surface.set_cell(x, y, Cell::space(style));
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
    let cell = arg_u32("region-size", 2).clamp(1, 8) as u16;
    let photodiode = has("--photodiode");
    let pattern = match arg_str("pattern").as_deref() {
        Some("counter") => Pattern::Counter,
        _ => Pattern::Gray,
    };
    let csv_path = arg_str("csv");

    eprintln!(
        "temporal cadence beacon: {fps} fps for {seconds}s, sync_updates={sync}, \
         cell={cell}, photodiode={photodiode}; film the {}x{}-cell region",
        BITS as u16 * cell,
        cell + if photodiode { cell } else { 0 }
    );

    let mut csv = match csv_path {
        Some(path) => {
            let mut w = BufWriter::new(File::create(path)?);
            writeln!(
                w,
                "frame,deadline_ms,emit_ms,gen_us,write_us,bytes,exact_changed,missed_periods"
            )?;
            Some(w)
        }
        None => None,
    };

    let mut ctx = Context::fullscreen()?;
    if !ctx.is_interactive() {
        return Err(io::Error::other(
            "temporal_cadence_beacon requires an interactive TTY; presentation cadence must be measured externally",
        ));
    }

    ctx.set_sync_updates(sync);
    ctx.set_max_fps(fps);
    ctx.set_frame_pacing(FramePacing::PhaseLocked);

    let budget = Duration::from_nanos(1_000_000_000u64 / fps as u64);
    let start = Instant::now();

    for frame in 0..frames {
        ctx.set_root(Node::surface(Arc::new(beacon_surface(
            frame, cell, pattern, photodiode,
        ))));

        while !ctx.render_if_due()? {
            let wait = ctx.time_until_next_frame();
            if wait.is_zero() {
                std::thread::yield_now();
            } else {
                std::thread::sleep(wait.min(Duration::from_millis(2)));
            }
        }

        if let Some(w) = csv.as_mut() {
            let emit = start.elapsed();
            let deadline = budget * frame as u32;
            let report = ctx.last_frame_report();
            writeln!(
                w,
                "{frame},{:.3},{:.3},{},{},{},{},{}",
                deadline.as_secs_f64() * 1000.0,
                emit.as_secs_f64() * 1000.0,
                report.generation_duration.as_micros(),
                report.write_duration.as_micros(),
                report.bytes_emitted,
                report.exact_changed_cells,
                ctx.missed_periods_last_frame(),
            )?;
        }
    }

    if let Some(mut w) = csv {
        w.flush()?;
    }
    ctx.restore()?;
    Ok(())
}
