//! Issue #264: an idle `Context::run_once(max_wait)` loop must sleep about `max_wait`
//! per call, while a pending frame and arriving input are still served on time.
//!
//! Each case runs `examples/idle_run_once_probe` in a child process (on a real PTY, or
//! headless) and reads the child's own measurements: how many times `run_once` returned
//! and how much CPU time the process used across the timed loop.
//!
//! The thresholds are wide and one-sided against load. A loaded machine can only make
//! the child do *less*: fewer returns and less CPU in a fixed wall-clock window, since
//! every idle call waits for its `max_wait`. The defect does more, by five orders of
//! magnitude: about 470,000 returns and 98% of a core per second at `082d324`.
//! The latency cases bound only the side the defect breaks: a pending frame must not
//! wait out a 5 s `max_wait`, and a key must end an 8 s wait early.
#![cfg(unix)]

use portable_pty::CommandBuilder;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[path = "common/pty_capture.rs"]
mod pty_capture;

const PROBE: &str = "idle_run_once_probe";

/// Parses the child's `RESULT k=v ...` line.
fn result_fields(output: &[u8]) -> BTreeMap<String, u64> {
    let text = String::from_utf8_lossy(output);
    let line = text
        .lines()
        .find_map(|l| {
            l.split_once("RESULT ")
                .map(|(_, rest)| rest.trim().to_owned())
        })
        .unwrap_or_else(|| panic!("no RESULT line in probe output: {text:?}"));
    line.split_whitespace()
        .filter_map(|kv| kv.split_once('='))
        .filter_map(|(k, v)| v.parse().ok().map(|v| (k.to_owned(), v)))
        .collect()
}

fn run_on_pty(args: &[&str]) -> BTreeMap<String, u64> {
    let mut cmd = CommandBuilder::new(pty_capture::example_path(PROBE));
    cmd.args(args);
    let mut cap = pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_secs(30));
    // The whole RESULT line: its terminating newline has arrived.
    cap.collect_until(|b| {
        let t = String::from_utf8_lossy(b);
        t.split_once("RESULT ")
            .is_some_and(|(_, rest)| rest.contains('\n'))
    })
    .expect("probe should report a RESULT");
    result_fields(&cap.finish())
}

fn run_headless(args: &[&str]) -> BTreeMap<String, u64> {
    let out = std::process::Command::new(pty_capture::example_path(PROBE))
        .args(args)
        .arg("--headless")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("run headless probe");
    assert!(out.status.success(), "headless probe failed: {out:?}");
    result_fields(&out.stdout)
}

/// The idle law for one configuration: over a 1 s window with `max_wait` = 100 ms, the
/// loop returns about 10 times and uses well under a quarter of a core.
fn assert_idle(label: &str, r: &BTreeMap<String, u64>) {
    let (calls, cpu_ms, wall_ms) = (r["calls"], r["cpu_ms"], r["wall_ms"]);
    assert!(
        calls <= 40,
        "{label}: run_once returned {calls} times in {wall_ms} ms with max_wait 100 ms \
         and nothing pending; an idle call must wait out max_wait (#264). Result: {r:?}"
    );
    assert!(
        calls >= 3,
        "{label}: only {calls} returns in {wall_ms} ms: the loop did not run. {r:?}"
    );
    assert!(
        cpu_ms * 4 < wall_ms,
        "{label}: an idle loop used {cpu_ms} ms of CPU in {wall_ms} ms (#264). {r:?}"
    );
    assert_eq!(
        r["frames"], 0,
        "{label}: nothing was pending, so nothing rendered"
    );
}

#[test]
fn idle_tty_loop_sleeps_instead_of_spinning_completion_relative() {
    let r = run_on_pty(&["idle", "--fps", "60"]);
    assert_idle("pty idle 60 fps", &r);
    // The permitted-frame clock still reads zero: the fix does not hide it, it stops
    // bounding an idle wait by it.
    assert_eq!(r["min_until_us"], 0, "{r:?}");
}

#[test]
fn idle_tty_loop_sleeps_instead_of_spinning_phase_locked() {
    assert_idle(
        "pty idle phase-locked",
        &run_on_pty(&["idle", "--phase-locked"]),
    );
}

#[test]
fn idle_tty_loop_sleeps_at_high_and_low_fps() {
    assert_idle("pty idle 1000 fps", &run_on_pty(&["idle", "--fps", "1000"]));
    assert_idle("pty idle 1 fps", &run_on_pty(&["idle", "--fps", "1"]));
}

#[test]
fn rootless_tty_loop_sleeps() {
    // No frame is ever recorded here, so the permitted-frame clock was zero from the
    // first call: the shape of `examples/hangup_probe`.
    assert_idle("pty no-root", &run_on_pty(&["no-root"]));
}

#[test]
fn idle_headless_loop_sleeps_off_tty() {
    // The off-TTY branch of run_once sleeps for the computed wait instead of polling;
    // it spun at about 4.7 million returns per second at 082d324.
    assert_idle("headless idle", &run_headless(&["idle"]));
    assert_idle("headless no-root", &run_headless(&["no-root"]));
    assert_idle(
        "headless idle phase-locked",
        &run_headless(&["idle", "--phase-locked"]),
    );
}

#[test]
fn a_pending_frame_renders_at_its_budget_not_at_max_wait() {
    // 10 fps: a 100 ms budget. A change lands right after a frame, and run_once is
    // called with max_wait = 5 s. The frame must render once its budget allows: not
    // after max_wait (the over-correction), and not before the budget (pacing).
    for (label, extra) in [
        ("completion-relative", None),
        ("phase-locked", Some("--phase-locked")),
    ] {
        let mut args = vec!["dirty", "--fps", "10", "--wait-ms", "5000"];
        args.extend(extra);
        let r = run_on_pty(&args);
        assert_eq!(
            r["frames"], 1,
            "{label}: the pending frame must render. {r:?}"
        );
        assert!(
            r["rendered_after_us"] < 2_000_000,
            "{label}: the pending frame waited {} us; it must render near its 100 ms \
             budget, far below max_wait = 5 s. {r:?}",
            r["rendered_after_us"]
        );
        assert!(
            r["rendered_after_us"] >= 50_000,
            "{label}: rendered after {} us, before the 100 ms budget allowed. {r:?}",
            r["rendered_after_us"]
        );
    }
}

#[test]
fn a_pending_frame_renders_on_time_headless() {
    let r = run_headless(&["dirty", "--fps", "10", "--wait-ms", "5000"]);
    assert_eq!(r["frames"], 1, "{r:?}");
    assert!(r["rendered_after_us"] < 2_000_000, "{r:?}");
    assert!(r["rendered_after_us"] >= 50_000, "{r:?}");
}

#[test]
fn arriving_input_ends_an_idle_wait_promptly() {
    // max_wait = 8 s: a key sent while run_once idles must come back long before that.
    let mut cmd = CommandBuilder::new(pty_capture::example_path(PROBE));
    cmd.args(["wake", "--wait-ms", "8000"]);
    let mut cap = pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_secs(30));
    cap.collect_until(|b| String::from_utf8_lossy(b).contains("READY"))
        .expect("probe should reach READY");
    // Let the child settle into its idle wait first.
    cap.collect_for(Duration::from_millis(300)).expect("settle");
    let sent = Instant::now();
    cap.write(b"k").expect("send key");
    cap.collect_until(|b| String::from_utf8_lossy(b).contains("GOT k"))
        .expect("probe should report the key");
    let latency = sent.elapsed();
    assert!(
        latency < Duration::from_secs(4),
        "a key took {latency:?} to end an 8 s idle wait; input must wake run_once promptly"
    );
    let _ = cap.finish();
}

#[test]
fn a_rootless_context_marked_dirty_goes_idle_after_one_attempt() {
    // A resize (or `mark_dirty`) on a context with no root leaves a pending frame that
    // can never be presented. The rootless render attempt clears it; left pending, the
    // wait would stay zero and the loop would spin (in-process, headless).
    let mut ctx = gibson::Context::headless(gibson::RenderMode::Inline, 40, 8);
    ctx.mark_dirty();
    assert_eq!(ctx.time_until_frame_needed(), Some(Duration::ZERO));
    let t0 = Instant::now();
    let mut calls = 0u32;
    while t0.elapsed() < Duration::from_millis(300) {
        ctx.run_once(Duration::from_millis(50)).expect("run_once");
        calls += 1;
    }
    assert!(
        calls <= 12,
        "{calls} returns in 300 ms with max_wait 50 ms: a rootless pending frame kept the \
         wait at zero (#264)"
    );
    assert_eq!(
        ctx.empty_root_renders(),
        1,
        "one rootless attempt, then idle"
    );
    assert_eq!(ctx.time_until_frame_needed(), None);
}
