//! LibGibson Runtime Observatory — workstream D, chunk 1.
//!
//! A NASA-control-room instrument for watching the runtime's own honesty
//! guarantees at work, instead of just reading about them in a probe's stdout
//! dump. This chunk ships the shared scaffold (bounded diagnostic log,
//! restrained mission-control rendering, `--dump` path) plus exactly one
//! mode: Million-Tick, which is issue #10 (unbounded `StoryTrace`) staged as
//! a picture instead of a paragraph. Other modes are NOT built here — that's
//! the next chunk, and pretending otherwise in this file would be exactly the
//! kind of fabrication the rest of this tool refuses to do.
//!
//! Usage:
//!   cargo run --release --example runtime_observatory -- \
//!     --mode million-tick --dump --retention all --ticks 100000 \
//!     [--width 120] [--height 32] [--color mono|ansi16|ansi256|truecolor]

#[path = "runtime_observatory/diag.rs"]
mod diag;
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

const HELP: &str = "LibGibson Runtime Observatory (workstream D, chunk 1: scaffold + Million-Tick)\n  --mode million-tick --dump [--width W] [--height H] [--color mono|ansi16|ansi256|truecolor]\n           [--ticks N] [--retention all|bounded:CAP|disabled]\nRenders one deterministic frame to stdout and exits. Pure render: no state\nmutation happens at paint time. Other modes (workstream D chunks 2+) are not\nbuilt yet; --mode anything else errors rather than faking a frame.";

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help") {
        println!("{HELP}");
        return Ok(());
    }

    let mode = option(&args, "--mode").unwrap_or_else(|| "million-tick".to_string());
    if mode != "million-tick" {
        return Err(io::Error::other(format!(
            "--mode {mode} is not built in this chunk (only million-tick exists so far)"
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
    let ticks = number(&args, "--ticks", 100_000)?.max(1);
    let retention_arg = option(&args, "--retention").unwrap_or_else(|| "all".to_string());
    let retention = million_tick::parse_retention(&retention_arg).map_err(io::Error::other)?;

    let text = million_tick::dump(ticks, retention, width, height, depth);
    print!("{text}");
    Ok(())
}
