//! Issue #79: `Context::run_once` must RETURN on a terminal hangup (SIGHUP) rather
//! than busy-spin at ~66% CPU forever. We deliver a real hangup by dropping the
//! kernel's last reference to the PTY master (`Capture::hang_up`), with the child
//! bound to the slave as its controlling terminal.
#![cfg(unix)]

use portable_pty::CommandBuilder;
use std::time::{Duration, Instant};

#[path = "common/pty_capture.rs"]
mod pty_capture;

fn spawn(ignore_sighup: bool) -> pty_capture::Capture {
    let mut cmd = CommandBuilder::new(pty_capture::example_path("hangup_probe"));
    if ignore_sighup {
        cmd.arg("ignore-sighup");
    }
    pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_secs(12))
}

/// Waits up to `bound` for the child to exit; returns its exit code, or `None` if
/// it was still running — the pre-fix busy-spin failure mode.
fn wait_exit_code(cap: &mut pty_capture::Capture, bound: Duration) -> Option<u32> {
    let t0 = Instant::now();
    while t0.elapsed() < bound {
        if let Some(status) = cap.try_wait().expect("poll child") {
            return Some(status.exit_code());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

#[test]
fn run_once_returns_brokenpipe_on_hangup_instead_of_spinning() {
    let mut cap = spawn(/* ignore_sighup = */ true);
    cap.collect_until(|b| String::from_utf8_lossy(b).contains("READY"))
        .expect("probe should reach READY");
    // Park the child inside poll_event, mid-wait — the real mid-poll hangup case.
    cap.collect_for(Duration::from_millis(150)).expect("settle");

    cap.hang_up();

    // Liveness bound, deliberately generous: the #79 defect is an *infinite*
    // busy-spin, so any finite bound catches it; the number only needs to clear a
    // healthy-but-starved exit on a loaded CI runner. 10s is ~6x the slowest exit
    // seen under load. Polls try_wait(), independent of the capture budget — see
    // common/pty_capture.rs.
    let code = wait_exit_code(&mut cap, Duration::from_millis(10_000));
    assert_eq!(
        code,
        Some(3),
        "child must exit 3 (run_once returned BrokenPipe) within 10s of the \
         hangup; None means it busy-spun (the #79 bug), another code means it \
         died some other way"
    );
}

#[test]
fn control_a_real_hangup_is_delivered_and_differs_from_the_brokenpipe_path() {
    // Without ignoring SIGHUP, the hangup's default action terminates the child.
    // This proves hang_up() delivers a genuine hangup (so the test above cannot be
    // vacuously green) and that this death is distinct from the BrokenPipe exit.
    let mut cap = spawn(/* ignore_sighup = */ false);
    cap.collect_until(|b| String::from_utf8_lossy(b).contains("READY"))
        .expect("probe should reach READY");
    cap.collect_for(Duration::from_millis(150)).expect("settle");

    cap.hang_up();

    // Same generous liveness bound as the BrokenPipe test above: finite catches a
    // spin, the margin absorbs CI load.
    let code = wait_exit_code(&mut cap, Duration::from_millis(10_000));
    assert!(
        code.is_some(),
        "the child must terminate within 10s of a real hangup, not spin"
    );
    assert_ne!(
        code,
        Some(3),
        "the control must die by the default SIGHUP action, not the BrokenPipe path"
    );
}
