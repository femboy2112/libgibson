//! Gate K · K4 — the `ffi_lifecycle` terminal-lease race, proven on a real PTY.
//!
//! FOUND DEBT (flagged during Gate J): `tests/ffi_lifecycle.rs` has nine
//! `#[test]` fns that each take the process-global terminal lease via
//! `gibson_create_context`. libtest runs them concurrently, so on a real TTY
//! they contend the lease and the loser fails with `AlreadyExists` (surfaced as
//! `ErrIo`). CI never sees it: CI's stdout is a pipe, `is_terminal()` is false,
//! and no test ever takes the lease.
//!
//! The library is correct — a terminal genuinely cannot have two owners, a
//! contract locked by `tests/terminal_ownership.rs::second_owner_errors`. So the
//! fix is TEST-ONLY: serialize the lease-taking tests behind a file-local gate
//! (see `tests/ffi_lifecycle.rs`). These two tests spawn a child onto a PTY
//! slave (so `is_tty` is true) and prove, deterministically, both halves:
//!
//!   * the hazard is real on a TTY (concurrent lease-takers DO collide), and
//!   * the serialization gate removes the collision entirely.
//!
//! Neither this file nor the fix touches `src/` or the ABI.
#![cfg(unix)]

use portable_pty::{CommandBuilder, ExitStatus};
use std::time::{Duration, Instant};

#[path = "common/pty_capture.rs"]
mod pty_capture;

fn spawn(scenario: &str) -> pty_capture::Capture {
    let mut cmd = CommandBuilder::new(pty_capture::example_path("ffi_lifecycle_lease_probe"));
    cmd.arg(scenario);
    pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_secs(20))
}

/// Drain and reap the child, bounded, so a full PTY can't wedge it and a hang
/// can't outlive the deadline.
fn wait_exit(capture: &mut pty_capture::Capture) -> ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let _ = capture.collect_for(Duration::from_millis(20));
        if let Some(status) = capture.try_wait().expect("poll lease probe child") {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "lease probe did not exit before deadline"
        );
    }
}

/// CONTROL (the hazard, as CI's pipe hides it): two threads take the lease with
/// no serialization; while one holds it, the other's create MUST be refused.
#[test]
fn ungated_lease_takers_collide_on_a_real_tty() {
    let mut capture = spawn("race");
    capture
        .collect_until(|b| String::from_utf8_lossy(b).contains("RACE_SCENARIO_DONE"))
        .expect("capture race probe");
    let status = wait_exit(&mut capture);
    let out = String::from_utf8_lossy(&capture.finish()).to_string();

    assert!(
        out.contains("SECOND_REJECTED"),
        "a second concurrent create on a real TTY must be refused with AlreadyExists; got:\n{out}"
    );
    assert!(
        !out.contains("SECOND_UNEXPECTEDLY_OK"),
        "two TerminalSessions must never coexist; got:\n{out}"
    );
    assert!(status.success(), "race probe should exit 0, got {status:?}");
}

/// FIX MECHANISM: four threads take the lease behind a shared gate. Serialization
/// means whoever runs second finds the lease free, so zero collisions occur —
/// the same gate `tests/ffi_lifecycle.rs` now holds across its lease-takers.
#[test]
fn a_serialized_gate_removes_the_collision() {
    let mut capture = spawn("gated");
    capture
        .collect_until(|b| String::from_utf8_lossy(b).contains("GATED_SCENARIO_DONE"))
        .expect("capture gated probe");
    let status = wait_exit(&mut capture);
    let out = String::from_utf8_lossy(&capture.finish()).to_string();

    assert!(
        out.contains("GATED_COLLISIONS 0"),
        "a serialized gate must prevent every lease collision; got:\n{out}"
    );
    assert!(
        status.success(),
        "gated probe should exit 0, got {status:?}"
    );
}
