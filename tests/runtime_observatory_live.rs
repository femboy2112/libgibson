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
