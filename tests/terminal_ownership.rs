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

/// Byte-exact substring search over the raw wire capture. We search bytes, not a
/// lossy UTF-8 view, so escape sequences and marker offsets stay exact.
fn find_sub(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

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

/// Issue #11 / B3: a recoverable panic on a non-owner worker thread must NOT
/// tear down the owner thread's live terminal. Proven on the wire — the
/// restoration sequence must not appear between the owner going active and the
/// owner restoring explicitly.
#[test]
fn worker_panic_leaves_owner_terminal_intact() {
    let mut capture = spawn_probe("worker-panic");
    capture
        .collect_until(|b| String::from_utf8_lossy(b).contains("OWNER_RESTORED"))
        .expect("capture worker-panic probe");
    let status = wait_exit(&mut capture);
    let output = capture.finish();

    let owner_active = find_sub(&output, b"OWNER_ACTIVE").expect("no OWNER_ACTIVE marker");
    let recovered =
        find_sub(&output, b"WORKER_PANIC_RECOVERED").expect("process did not survive worker panic");
    let restoring = find_sub(&output, b"OWNER_RESTORING").expect("no OWNER_RESTORING marker");
    assert!(
        owner_active < recovered && recovered < restoring,
        "probe markers out of order"
    );

    // The invariant: no terminal restoration on the wire until the owner asks
    // for it. Its presence in [OWNER_ACTIVE, OWNER_RESTORING) means a non-owner
    // worker panic dismantled the owner's live terminal — the B3 bug.
    let window = &output[owner_active..restoring];
    assert!(
        find_sub(window, SHOW_CURSOR).is_none(),
        "a non-owner worker-thread panic emitted terminal restoration (\\x1b[?25h) \
         before the owner restored: it tore down the owner's live terminal"
    );

    // The process recovered (did not abort) and the owner's own restore ran.
    assert!(
        status.success(),
        "worker-panic probe should exit 0, got {status:?}"
    );
    assert!(
        find_sub(&output, SHOW_CURSOR).is_some(),
        "owner never restored the terminal at all"
    );
}

/// Issue #11 / B1: an explicit restore whose stdout writes fail must report the
/// first error, yet still attempt every remaining cleanup step (raw mode, via a
/// termios path independent of the broken fd) and release the lease.
#[test]
fn restore_reports_output_failure_but_still_tears_down() {
    let mut capture = spawn_probe("restore-output-failure");
    capture
        .collect_until(|b| String::from_utf8_lossy(b).contains("RESTORE_FAILURE_DONE"))
        .expect("capture restore-output-failure probe");
    let status = wait_exit(&mut capture);
    let output = capture.finish();
    let s = String::from_utf8_lossy(&output);

    assert!(
        s.contains("RAW_BEFORE=true"),
        "raw mode was not actually enabled before the failure was induced"
    );
    assert!(
        s.contains("RESTORE_RESULT=ERR"),
        "restore() swallowed the output failure instead of reporting it: {s:?}"
    );
    assert!(
        s.contains("RAW_AFTER=false"),
        "restore() abandoned raw-mode teardown after the write failed"
    );
    assert!(
        s.contains("LEASE_AFTER=Available"),
        "restore() left the lease dangling after a failed restore"
    );
    assert!(
        status.success(),
        "restore-output-failure probe should exit 0, got {status:?}"
    );
}

/// Issue #11 / B5: LibGibson chains a host panic hook installed before the first
/// session. On an owner panic, both the host hook and LibGibson's restoration run.
#[test]
fn panic_hook_chains_a_preexisting_host_hook() {
    let mut capture = spawn_probe("host-hook-chain");
    capture
        .collect_until(|b| String::from_utf8_lossy(b).contains("HOOK_CHAIN_OWNED"))
        .expect("capture host-hook-chain probe");
    let status = wait_exit(&mut capture);
    let output = capture.finish();
    let s = String::from_utf8_lossy(&output);

    assert!(
        find_sub(&output, SHOW_CURSOR).is_some(),
        "LibGibson terminal restoration did not run on an owner panic"
    );
    assert!(
        s.contains("HOST_HOOK_RAN"),
        "LibGibson clobbered the host's pre-existing panic hook instead of chaining it"
    );
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
