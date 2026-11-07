//! Child probe for issue #264: an idle `Context::run_once(max_wait)` loop must wait
//! about `max_wait` per call instead of returning at once, while a pending frame and
//! arriving input are still served on time.
//!
//! argv: `idle_run_once_probe <scenario> [--fps N] [--phase-locked] [--wait-ms N] [--for-ms N]`
//!
//! Scenarios (each prints `READY` first, then exactly one `RESULT ...` line):
//!
//! * `idle` — render one frame, then loop `run_once(wait)` with nothing dirty for
//!   `--for-ms`. Reports the number of `run_once` returns and the process CPU time.
//! * `no-root` — like `idle`, but no root is ever set, so no frame is ever recorded.
//! * `dirty` — render one frame, mark the context dirty, then call `run_once` once
//!   with a long `max_wait` and no input. Reports how long that call took, whether it
//!   rendered, and the time since the first frame (the start of the budget).
//! * `wake` — idle-loop `run_once(wait)` until a key arrives, printing `GOT <char>` the
//!   moment `run_once` returns it, then exits. The parent measures the wake latency.
//!
//! `--headless` runs `idle` / `no-root` / `dirty` on a [`gibson::Context::headless`] session
//! instead of the controlling terminal (the off-TTY sleep branch of `run_once`).
//!
//! CPU is `getrusage(RUSAGE_SELF)` user+system time, measured across the timed loop only.

use gibson::cell::Style;
use gibson::context::{Context, RenderMode};
use gibson::node::Node;
use gibson::scheduler::FramePacing;
use gibson::{Event, KeyCode};
use std::io::Write;
use std::time::{Duration, Instant};

fn cpu_time() -> Duration {
    #[cfg(unix)]
    {
        // SAFETY: getrusage writes into a zeroed, correctly sized struct.
        let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
        unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) };
        let micros = (ru.ru_utime.tv_sec + ru.ru_stime.tv_sec) as u64 * 1_000_000
            + (ru.ru_utime.tv_usec + ru.ru_stime.tv_usec) as u64;
        Duration::from_micros(micros)
    }
    #[cfg(not(unix))]
    {
        Duration::ZERO
    }
}

fn arg_value(args: &[String], name: &str) -> Option<u64> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
}

fn say(line: &str) {
    // `\r\n`: the terminal is in raw mode once input has been polled.
    let mut out = std::io::stdout();
    let _ = write!(out, "{line}\r\n");
    let _ = out.flush();
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let scenario = args.get(1).cloned().unwrap_or_default();
    let fps = arg_value(&args, "--fps").unwrap_or(60) as u32;
    let wait = Duration::from_millis(arg_value(&args, "--wait-ms").unwrap_or(100));
    let run_for = Duration::from_millis(arg_value(&args, "--for-ms").unwrap_or(1000));
    let headless = args.iter().any(|a| a == "--headless");

    let mut ctx = if headless {
        Context::headless(RenderMode::Inline, 80, 24)
    } else {
        Context::new(RenderMode::Inline)?
    };
    ctx.set_max_fps(fps);
    if args.iter().any(|a| a == "--phase-locked") {
        ctx.set_frame_pacing(FramePacing::PhaseLocked);
    }

    match scenario.as_str() {
        "idle" | "no-root" => {
            if scenario == "idle" {
                ctx.set_root(Node::text("idle", Style::new()));
                ctx.render()?;
            }
            // Arm raw mode and the input source before the measurement starts.
            let _ = ctx.poll_event(Duration::ZERO)?;
            say("READY");
            let frames_before = ctx.stats().frames;
            let (t0, c0) = (Instant::now(), cpu_time());
            let mut calls = 0u64;
            let mut min_until = Duration::MAX;
            while t0.elapsed() < run_for {
                min_until = min_until.min(ctx.time_until_next_frame());
                let _ = ctx.run_once(wait)?;
                calls += 1;
            }
            let (wall, cpu) = (t0.elapsed(), cpu_time() - c0);
            let frames = ctx.stats().frames - frames_before;
            say(&format!(
                "RESULT scenario={scenario} calls={calls} wall_ms={} cpu_ms={} frames={frames} min_until_us={}",
                wall.as_millis(),
                cpu.as_millis(),
                min_until.as_micros()
            ));
        }
        "dirty" => {
            ctx.set_root(Node::text("frame-0", Style::new()));
            // Sampled BEFORE the render: the budget runs from when the scheduler records
            // that frame, which is later, so time measured from here can only overstate
            // the elapsed budget, never understate it. A correct next frame therefore
            // always shows `since_frame0` >= one budget, however the child is scheduled.
            let frame0 = Instant::now();
            ctx.render()?;
            let _ = ctx.poll_event(Duration::ZERO)?;
            say("READY");
            let frames_before = ctx.stats().frames;
            // A change lands right after a frame: it is pending, and the budget is not
            // yet spent. With no input, the one run_once call must wait until the budget
            // allows it and render it: not wait out max_wait, and not spin.
            ctx.set_root(Node::text("frame-1", Style::new()));
            let t0 = Instant::now();
            let _ = ctx.run_once(wait)?;
            let first_call = t0.elapsed();
            let first_call_frames = ctx.stats().frames - frames_before;
            let since_frame0 = frame0.elapsed();
            say(&format!(
                "RESULT scenario=dirty first_call_us={} first_call_frames={first_call_frames} \
                 since_frame0_us={} budget_us={}",
                first_call.as_micros(),
                since_frame0.as_micros(),
                ctx.frame_budget().as_micros()
            ));
        }
        "wake" => {
            ctx.set_root(Node::text("waiting", Style::new()));
            ctx.render()?;
            let _ = ctx.poll_event(Duration::ZERO)?;
            say("READY");
            let deadline = Instant::now() + Duration::from_secs(10);
            while Instant::now() < deadline {
                if let Some(Event::Key(k)) = ctx.run_once(wait)? {
                    if let KeyCode::Char(c) = k.code {
                        say(&format!("GOT {c}"));
                        break;
                    }
                }
            }
        }
        other => {
            eprintln!("unknown scenario {other:?}");
            std::process::exit(2);
        }
    }
    ctx.restore()?;
    Ok(())
}
