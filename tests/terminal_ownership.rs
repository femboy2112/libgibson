//! Terminal-ownership + restoration contract (issue #11) under a real PTY.
//!
//! `is_tty=true` is non-negotiable for the acquire path, so every ownership
//! scenario runs in a child bound to a PTY slave via the shared `pty_capture`
//! helper. The child is the patient; we read its vitals off the wire and bound
//! every wait so nothing is left open on the table. The one lease-invariant that
//! needs no terminal — headless never takes the lease — runs in-process.
#![cfg(unix)]

use portable_pty::{CommandBuilder, ExitStatus};
use std::time::{Duration, Instant};

#[path = "common/pty_capture.rs"]
mod pty_capture;

use gibson::TerminalSession;

/// Reset+show-cursor tail every restoration path (explicit, Drop, panic hook)
/// puts on the wire. Its presence is our proof the terminal was cleaned up.
const SHOW_CURSOR: &[u8] = b"\x1b[?25h";

fn spawn_probe(scenario: &str) -> pty_capture::Capture {
    let mut cmd = CommandBuilder::new(pty_capture::example_path("terminal_ownership_probe"));
    cmd.arg(scenario);
    pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_secs(10))
}

/// Drain and wait for the child to exit, bounded. Draining keeps the child from
/// blocking on a full PTY while we wait; the deadline guarantees no hang.
fn wait_exit(capture: &mut pty_capture::Capture) -> ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let _ = capture.collect_for(Duration::from_millis(20));
        if let Some(status) = capture.try_wait().expect("poll probe child") {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "terminal-ownership probe did not exit before deadline"
        );
    }
}

/// A second `TerminalSession::new()` in a process that already owns the terminal
/// is rejected with `AlreadyExists`; the child says so and exits clean.
#[test]
fn second_owner_errors() {
    let mut capture = spawn_probe("second-owner");
    capture
        .collect_until(|bytes| String::from_utf8_lossy(bytes).contains("SECOND_OWNER_REJECTED"))
        .expect("capture second-owner probe");
    let status = wait_exit(&mut capture);
    let output = capture.finish();

    assert!(
        String::from_utf8_lossy(&output).contains("SECOND_OWNER_REJECTED"),
        "probe never reported the second-owner rejection"
    );
    assert!(
        status.success(),
        "second-owner probe should exit 0, got {status:?}"
    );
}

/// Dropping the owner frees the lease; a fresh session reacquires it.
#[test]
fn drop_then_reacquire() {
    let mut capture = spawn_probe("drop-reacquire");
    capture
        .collect_until(|bytes| String::from_utf8_lossy(bytes).contains("REACQUIRED"))
        .expect("capture drop-reacquire probe");
    let status = wait_exit(&mut capture);
    let output = capture.finish();

    assert!(
        String::from_utf8_lossy(&output).contains("REACQUIRED"),
        "probe never reacquired the terminal after drop"
    );
    assert!(
        status.success(),
        "drop-reacquire probe should exit 0, got {status:?}"
    );
}

/// A panic while owning the terminal must trigger best-effort restoration via
/// the panic hook and let the process exit rather than hang.
#[test]
fn panic_restores_when_owned() {
    let mut capture = spawn_probe("panic-when-owned");
    capture
        .collect_until(|bytes| String::from_utf8_lossy(bytes).contains("OWNED_BEFORE_PANIC"))
        .expect("capture panic probe ownership marker");
    let status = wait_exit(&mut capture);
    let output = capture.finish();

    assert!(
        String::from_utf8_lossy(&output).contains("OWNED_BEFORE_PANIC"),
        "probe never reported ownership before panicking"
    );
    assert!(
        output.windows(SHOW_CURSOR.len()).any(|w| w == SHOW_CURSOR),
        "panic hook did not emit terminal restoration"
    );
    // A panic unwinds to a non-zero exit; the point is it exited at all.
    assert!(
        !status.success(),
        "panic probe should exit non-zero, got {status:?}"
    );
}

/// Headless sessions are mannequins: constructing many of them never errors and
/// never perturbs the process-global lease. Asserted as a *transition* (before ==
/// during == after), never as a hard global value, so concurrent tests in this
/// binary cannot make it flap. (Nothing in this process runs the acquire path in
/// parallel — the PTY probes are separate processes — but the transition form is
/// the robust contract regardless.)
#[test]
fn headless_does_not_take_lease() {
    let before = TerminalSession::lease_state();
    let sessions: Vec<TerminalSession> =
        (0..64).map(|_| TerminalSession::headless(80, 24)).collect();
    let during = TerminalSession::lease_state();
    drop(sessions);
    let after = TerminalSession::lease_state();

    assert_eq!(
        before, during,
        "headless construction perturbed the terminal lease"
    );
    assert_eq!(during, after, "headless drop perturbed the terminal lease");
}
