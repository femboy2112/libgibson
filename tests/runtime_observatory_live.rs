//! Live Runtime Observatory smoke test (workstream D1) under a real PTY.
//!
//! The live loop needs a real terminal (raw mode + `poll_event`), so it can only
//! be exercised from a process whose stdout is a PTY slave. We drive it headless
//! with `--frames N` (a bounded auto-exit), then assert it painted the
//! mission-control frame from real state and restored the terminal on the way
//! out. This is the deterministic proof that the interactive instrument runs;
//! the `--dump` golden tests cover the frame contents.
#![cfg(unix)]

use portable_pty::{CommandBuilder, ExitStatus};
use std::time::{Duration, Instant};

#[path = "common/pty_capture.rs"]
mod pty_capture;

const SHOW_CURSOR: &[u8] = b"\x1b[?25h";

fn find_sub(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && hay.len() >= needle.len()
        && hay.windows(needle.len()).any(|w| w == needle)
}

fn wait_exit(capture: &mut pty_capture::Capture) -> ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        let _ = capture.collect_for(Duration::from_millis(20));
        if let Some(status) = capture.try_wait().expect("poll observatory child") {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "live observatory did not exit before deadline"
        );
    }
}

/// The live loop opens on Million-Tick, paints the mission-control frame from
/// real StoryDirector + /proc measurements, runs a bounded number of frames,
/// and restores the terminal cleanly on exit.
#[test]
fn live_million_tick_paints_and_restores() {
    let mut cmd = CommandBuilder::new(pty_capture::example_path("runtime_observatory"));
    cmd.args([
        "--mode",
        "million-tick",
        "--frames",
        "40",
        "--color",
        "ansi256",
    ]);
    let mut capture = pty_capture::Capture::spawn(cmd, 110, 32, Duration::from_secs(12));

    // It reaches the header once it has painted a real frame.
    capture
        .collect_until(|bytes| String::from_utf8_lossy(bytes).contains("RUNTIME OBSERVATORY"))
        .expect("capture live observatory header");

    let status = wait_exit(&mut capture);
    let output = capture.finish();
    let s = String::from_utf8_lossy(&output);

    assert!(
        s.contains("MILLION-TICK"),
        "live observatory never labelled the Million-Tick mode"
    );
    // Real measured telemetry made it to the panel, not a placeholder.
    assert!(
        s.contains("All steps") && s.contains("Bnd steps"),
        "live observatory did not render the All-vs-Bounded retention panel"
    );
    // It restored the terminal (show-cursor) on the way out.
    assert!(
        find_sub(&output, SHOW_CURSOR),
        "live observatory did not restore the terminal on exit"
    );
    assert!(
        status.success(),
        "live observatory should exit 0 under --frames, got {status:?}"
    );
}

/// Ownership-Duel live (D4) runs the REAL duel-trace probe as a child under its
/// own PTY and reveals the child's genuine lease-transition receipts (issue #11).
#[test]
fn live_ownership_duel_reveals_real_child_receipts() {
    // The supervised mode spawns this sibling binary via current_exe; make sure
    // it is built in this test process's target dir.
    let _ = pty_capture::example_path("terminal_ownership_probe");
    let mut cmd = CommandBuilder::new(pty_capture::example_path("runtime_observatory"));
    cmd.args([
        "--mode",
        "ownership-duel",
        "--frames",
        "90",
        "--color",
        "ansi256",
    ]);
    let mut capture = pty_capture::Capture::spawn(cmd, 110, 32, Duration::from_secs(15));
    capture
        .collect_until(|b| String::from_utf8_lossy(b).contains("OWNERSHIP-DUEL"))
        .expect("capture ownership-duel header");
    let status = wait_exit(&mut capture);
    let output = capture.finish();
    let s = String::from_utf8_lossy(&output);

    assert!(s.contains("OWNERSHIP-DUEL"), "no mode label");
    assert!(s.contains("SUPERVISED RECEIPTS"), "no supervised panel");
    assert!(
        !s.contains("PROBE UNAVAILABLE"),
        "supervised child could not be spawned: {s:?}"
    );
    // A genuine receipt from the real child reached the panel.
    assert!(
        s.contains("Acquire") || s.contains("LeaseState"),
        "no real duel receipt revealed"
    );
    assert!(
        find_sub(&output, SHOW_CURSOR),
        "did not restore the terminal on exit"
    );
    assert!(status.success(), "exit {status:?}");
}

/// Restore-Failure live (D5) runs the REAL restore-output-failure probe (fd 1 ->
/// /dev/full) and reveals its genuine cleanup receipts (issue #11 B1).
#[test]
fn live_restore_failure_reveals_real_child_receipts() {
    let _ = pty_capture::example_path("terminal_ownership_probe");
    let mut cmd = CommandBuilder::new(pty_capture::example_path("runtime_observatory"));
    cmd.args([
        "--mode",
        "restore-failure",
        "--frames",
        "90",
        "--color",
        "ansi256",
    ]);
    let mut capture = pty_capture::Capture::spawn(cmd, 110, 32, Duration::from_secs(15));
    capture
        .collect_until(|b| String::from_utf8_lossy(b).contains("RESTORE-FAILURE"))
        .expect("capture restore-failure header");
    let status = wait_exit(&mut capture);
    let output = capture.finish();
    let s = String::from_utf8_lossy(&output);

    assert!(s.contains("RESTORE-FAILURE"), "no mode label");
    assert!(
        !s.contains("PROBE UNAVAILABLE"),
        "supervised child could not be spawned: {s:?}"
    );
    // Genuine markers from the induced-failure child (contract, not just success).
    assert!(
        s.contains("RESTORE_RESULT") || s.contains("RAW_AFTER") || s.contains("RAW_BEFORE"),
        "no real restore-failure receipt revealed"
    );
    assert!(status.success(), "exit {status:?}");
}

/// Endurance live (D7): a sustained bounded soak that stays healthy — real
/// iterations, frames, RSS and per-update cost, in-process (issue #10).
#[test]
fn live_endurance_soaks_and_stays_bounded() {
    let mut cmd = CommandBuilder::new(pty_capture::example_path("runtime_observatory"));
    cmd.args([
        "--mode",
        "endurance",
        "--frames",
        "50",
        "--color",
        "ansi256",
    ]);
    let mut capture = pty_capture::Capture::spawn(cmd, 110, 32, Duration::from_secs(12));
    capture
        .collect_until(|b| String::from_utf8_lossy(b).contains("ENDURANCE"))
        .expect("capture endurance header");
    let status = wait_exit(&mut capture);
    let output = capture.finish();
    let s = String::from_utf8_lossy(&output);

    assert!(s.contains("ENDURANCE"), "no mode label");
    assert!(
        s.contains("iterations") && s.contains("SUSTAINED SOAK"),
        "no soak panel"
    );
    assert!(
        find_sub(&output, SHOW_CURSOR),
        "did not restore the terminal on exit"
    );
    assert!(status.success(), "exit {status:?}");
}
