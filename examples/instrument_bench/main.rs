//! `instrument_bench` — truthful measurement wired into the real instrument loop
//! (v0.6 R6.5, #143). A thin shell over [`bench`]: parse a workload, run it
//! through the real renderer via `TerminalSession::headless`, and print a receipt
//! whose every number is scoped to exactly what it witnessed.
//!
//! Usage:
//!   cargo run --example instrument_bench -- [--workload sparse|moderate|full-churn|backpressure|all]
//!       [--ticks N] [--size WxH] [--color mono|ansi16|ansi256|truecolor]
//!       [--stall-stage frame-generation|terminal-write|input-dispatch] [--stall-ms N]
//!   cargo run --example instrument_bench -- --selfcheck
//!
//! `--selfcheck` drives every workload and asserts the honesty invariants live,
//! exiting non-zero on the first violation.

mod bench;

use std::process::ExitCode;
use std::time::Duration;

use bench::{format_report, run_workload, selfcheck, Stall, Workload};
use gibson::instrument_telemetry::Stage;
use gibson::ColorDepth;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .find_map(|s| s.strip_prefix(&format!("{name}=")).map(str::to_owned))
        .or_else(|| args.windows(2).find(|p| p[0] == name).map(|p| p[1].clone()))
}

fn parse_stage(s: &str) -> Option<Stage> {
    Some(match s {
        "input-dispatch" => Stage::InputDispatch,
        "snapshot-age" => Stage::SnapshotAge,
        "frame-generation" => Stage::FrameGeneration,
        "raster-conversion" => Stage::RasterConversion,
        "terminal-write" => Stage::TerminalWrite,
        _ => return None,
    })
}

fn parse_color(s: &str) -> Option<ColorDepth> {
    Some(match s {
        "mono" => ColorDepth::Mono,
        "ansi16" => ColorDepth::Ansi16,
        "ansi256" => ColorDepth::Ansi256,
        "truecolor" | "true" => ColorDepth::TrueColor,
        _ => return None,
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--help") {
        println!(
            "instrument_bench — truthful measurement wired into the real instrument loop (#143)\n\
             --workload sparse|moderate|full-churn|backpressure|all   (default all)\n\
             --ticks N          rendered frames (default 64)\n\
             --size WxH         headless terminal geometry (default 80x24)\n\
             --color mono|ansi16|ansi256|truecolor   (default truecolor)\n\
             --stall-stage S --stall-ms N   inject a disclosed stall into one stage\n\
             --selfcheck        assert the honesty invariants and exit non-zero on violation"
        );
        return ExitCode::SUCCESS;
    }

    if args.iter().any(|a| a == "--selfcheck") {
        let bad = selfcheck();
        if bad.is_empty() {
            println!("instrument_bench --selfcheck: all honesty invariants hold.");
            return ExitCode::SUCCESS;
        }
        eprintln!("instrument_bench --selfcheck: {} VIOLATION(S):", bad.len());
        for b in &bad {
            eprintln!("  x {b}");
        }
        return ExitCode::FAILURE;
    }

    // Default 64 so p95 and p99 select distinct samples (at n=16 both round to the
    // max sample, over-suggesting a tail resolution the run does not have).
    let ticks: u64 = arg(&args, "--ticks")
        .and_then(|s| s.parse().ok())
        .unwrap_or(64);
    let (cols, rows) = arg(&args, "--size")
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .unwrap_or((80u16, 24u16));
    let color = arg(&args, "--color")
        .and_then(|s| parse_color(&s))
        .unwrap_or(ColorDepth::TrueColor);
    let stall = match (arg(&args, "--stall-stage"), arg(&args, "--stall-ms")) {
        (Some(s), Some(ms)) => match (parse_stage(&s), ms.parse::<u64>()) {
            (Some(stage), Ok(ms)) => Some(Stall {
                stage,
                extra: Duration::from_millis(ms),
            }),
            _ => {
                eprintln!("invalid --stall-stage/--stall-ms");
                return ExitCode::FAILURE;
            }
        },
        _ => None,
    };

    let which = arg(&args, "--workload").unwrap_or_else(|| "all".to_string());
    let workloads: Vec<Workload> = if which == "all" {
        Workload::ALL.to_vec()
    } else {
        match Workload::parse(&which) {
            Some(w) => vec![w],
            None => {
                eprintln!("unknown --workload {which}");
                return ExitCode::FAILURE;
            }
        }
    };

    for w in workloads {
        let r = run_workload(w, ticks, cols, rows, color, stall);
        print!("{}", format_report(&r));
        println!();
    }
    ExitCode::SUCCESS
}
