//! Issue #79: `Context::run_once` must RETURN on a terminal hangup (SIGHUP) rather
//! than busy-spin at ~66% CPU forever. We deliver a real hangup by dropping the
//! kernel's last reference to the PTY master (`Capture::hang_up`), with the child
//! bound to the slave as its controlling terminal.
#![cfg(unix)]

use portable_pty::CommandBuilder;
use std::sync::atomic::{AtomicU32, Ordering};
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
#[ignore = "timing-sensitive PTY test; run serially in the dedicated PTY CI job via --include-ignored (parallel `cargo test` starves its liveness deadline under load)"]
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
#[ignore = "timing-sensitive PTY test; run serially in the dedicated PTY CI job via --include-ignored (parallel `cargo test` starves its liveness deadline under load)"]
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

#[test]
#[ignore = "timing-sensitive PTY test; run serially in the dedicated PTY CI job via --include-ignored (parallel `cargo test` starves its liveness deadline under load)"]
fn between_poll_hangup_still_returns_brokenpipe_when_isatty_now_false() {
    // The deterministic sibling of the mid-poll test above: the hangup lands *between*
    // two polls, after which `isatty` reports non-tty. A dispatcher that re-probes
    // `isatty` on every call would demote the next poll to the crossterm spin path —
    // the identity-vs-liveness regression (#79). The latched tty identity keeps the
    // hangup-aware path. A filesystem barrier forces the ordering, so this is a real
    // counterexample rather than a timing coincidence.
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let tag = format!(
        "{}_{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let dir = std::env::temp_dir().join(format!("gibson_hangup_bp_{tag}"));
    std::fs::create_dir_all(&dir).expect("create barrier dir");
    let release = dir.join("release");
    let result = dir.join("result");

    let mut cmd = CommandBuilder::new(pty_capture::example_path("hangup_between_polls_probe"));
    cmd.arg(release.to_str().expect("utf-8 path"));
    cmd.arg(result.to_str().expect("utf-8 path"));
    let mut cap = pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_secs(12));

    cap.collect_until(|b| String::from_utf8_lossy(b).contains("READY"))
        .expect("probe should reach READY");
    cap.collect_until(|b| String::from_utf8_lossy(b).contains("ARMED"))
        .expect("probe should park on the barrier (ARMED)");

    // The child is now parked OUTSIDE poll_event, on the file barrier. Deliver the
    // hangup FIRST, then release the child into its next poll — so the hangup strictly
    // precedes that poll. No sleep-for-luck; the ordering is forced.
    cap.hang_up();
    std::fs::write(&release, b"go").expect("write release barrier");

    let code = wait_exit_code(&mut cap, Duration::from_millis(10_000));
    assert_eq!(
        code,
        Some(3),
        "a between-polls hangup must still surface as BrokenPipe (exit 3) even though \
         isatty now reports non-tty; None means the dispatcher demoted to the crossterm \
         spin path — the identity-vs-liveness regression (#79)"
    );

    // Non-vacuity: prove the demotion condition was genuinely armed — the child saw
    // isatty()==0 at the between-polls moment. If this is not 0 the test did not
    // exercise the regression, so a green result would be vacuous.
    let observed = std::fs::read_to_string(&result).unwrap_or_default();
    assert!(
        observed.contains("isatty=0"),
        "expected the probe to observe isatty==0 after master close (got {observed:?}); \
         the falsifier's demotion condition was not present"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
