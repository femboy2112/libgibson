//! LibGibson Runtime Observatory — workstream D, chunks 1-2.
//!
//! A NASA-control-room instrument for watching the runtime's own honesty
//! guarantees at work, instead of just reading about them in a probe's stdout
//! dump. Chunk 1 shipped the shared scaffold (bounded diagnostic log,
//! restrained mission-control rendering, `--dump` path) plus Million-Tick.
//! Chunk 2 adds Ghost-Key: a pure replay of the REAL crossterm #1126 evidence
//! from `docs/fixtures/event-pressure/` through the same scaffold. Other
//! modes are still NOT built here — pretending otherwise in this file would
//! be exactly the kind of fabrication the rest of this tool refuses to do.
//!
//! Usage:
//!   cargo run --release --example runtime_observatory -- \
//!     --mode million-tick --dump --retention all --ticks 100000 \
//!     [--width 120] [--height 32] [--color mono|ansi16|ansi256|truecolor]
//!
//!   cargo run --release --example runtime_observatory -- \
//!     --mode ghost-key --dump --gate crossterm|raw [--freeze-at US]
//!     [--width 120] [--height 32] [--color mono|ansi16|ansi256|truecolor]

#[path = "runtime_observatory/diag.rs"]
mod diag;
#[path = "runtime_observatory/ghost_key.rs"]
mod ghost_key;
#[path = "runtime_observatory/million_tick.rs"]
mod million_tick;
#[path = "runtime_observatory/visual.rs"]
mod visual;

use gibson::ColorDepth;
use std::io;

fn option(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .find_map(|s| s.strip_prefix(&format!("{name}=")).map(str::to_owned))
        .or_else(|| args.windows(2).find(|p| p[0] == name).map(|p| p[1].clone()))
}
fn number(args: &[String], name: &str, default: u64) -> io::Result<u64> {
    option(args, name).map_or(Ok(default), |s| {
        s.parse()
            .map_err(|_| io::Error::other(format!("invalid {name}")))
    })
}

const HELP: &str = "LibGibson Runtime Observatory (workstream D, chunks 1-2: scaffold + Million-Tick + Ghost-Key)\n  --mode million-tick --dump [--width W] [--height H] [--color mono|ansi16|ansi256|truecolor]\n           [--ticks N] [--retention all|bounded:CAP|disabled]\n  --mode ghost-key --dump [--gate crossterm|raw] [--freeze-at US]\n           [--width W] [--height H] [--color mono|ansi16|ansi256|truecolor]\nRenders one deterministic frame to stdout and exits. Pure render: no state\nmutation happens at paint time (Ghost-Key doesn't drive anything at all — it\nreplays real recorded receipts from docs/fixtures/event-pressure/). Other\nmodes (workstream D chunks 3+) are not built yet; --mode anything else errors\nrather than faking a frame.";

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help") {
        println!("{HELP}");
        return Ok(());
    }

    let mode = option(&args, "--mode").unwrap_or_else(|| "million-tick".to_string());
    if mode != "million-tick" && mode != "ghost-key" {
        return Err(io::Error::other(format!(
            "--mode {mode} is not built yet (only million-tick and ghost-key exist so far)"
        )));
    }

    if !args.iter().any(|s| s == "--dump") {
        return Err(io::Error::other(
            "this chunk only implements the --dump path; pass --dump (a live interactive loop is future work, not faked here)",
        ));
    }

    let depth = match option(&args, "--color").as_deref() {
        Some("mono") => ColorDepth::Mono,
        Some("ansi16") => ColorDepth::Ansi16,
        Some("ansi256") => ColorDepth::Ansi256,
        _ => ColorDepth::TrueColor,
    };
    let width = number(&args, "--width", 120)?.clamp(1, 240) as u16;
    let height = number(&args, "--height", 32)?.clamp(1, 80) as u16;

    let text = if mode == "million-tick" {
        let ticks = number(&args, "--ticks", 100_000)?.max(1);
        let retention_arg = option(&args, "--retention").unwrap_or_else(|| "all".to_string());
        let retention = million_tick::parse_retention(&retention_arg).map_err(io::Error::other)?;
        million_tick::dump(ticks, retention, width, height, depth)
    } else {
        let gate_arg = option(&args, "--gate").unwrap_or_else(|| "crossterm".to_string());
        let gate = ghost_key::parse_gate(&gate_arg).map_err(io::Error::other)?;
        let freeze_at = option(&args, "--freeze-at")
            .map(|s| {
                s.parse::<u64>()
                    .map_err(|_| io::Error::other("invalid --freeze-at"))
            })
            .transpose()?;
        ghost_key::dump(gate, freeze_at, width, height, depth)
    };
    print!("{text}");
    Ok(())
}
