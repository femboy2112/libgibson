//! Real-TTY ABI construction-path coverage (#142, v0.6 Gate J).
//!
//! Gate I's headless ABI pair forces `is_tty = true` + truecolor + a Buffer sink, so the REAL-TTY
//! path through the C ABI — `gibson_create_context` → `Context::new` → `TerminalSession::new`
//! (lease acquire) → crossterm → alternate screen → the stdout sink → restore — stayed uncovered
//! by any ABI test (only the Gate H non-TTY leg and native example PTY tests touched that area).
//! This drives it on a real pseudo-terminal, with NO new FFI symbol, NO ABI change, and NO cdylib:
//! an in-process `dup2` of a PTY slave onto fd 1 is unsafe (crossterm's size/raw-mode ioctls target
//! `/dev/tty`, the real controlling terminal, not fd 1), so the only honest shape is a child whose
//! controlling terminal IS the PTY — exactly the `terminal_ownership_probe` precedent.
//!
//! Oracles (the wire + the exit code, never a color): on a real TTY the ABI create succeeds and
//! owns the terminal lease (asserted inside the probe, surfaced via exit 0); the wire carries the
//! alternate-screen enter, the painted text, and the restore tail; the termios is restored to its
//! initial value on exit; and — the real-TTY analogue of Gate I's headless parity — the ABI wire
//! equals the native wire byte-for-byte for the same op-script at the same PTY geometry.
//!
//! Scope boundary (L10): the construction/paint/restore path only. Real input events through the
//! PTY, resize, and hangup are covered natively elsewhere (`event_pressure_pty`, `pty_hangup*`),
//! and the cdylib's own C/Python bindings on a PTY would be a separate, Bindings-job-only harness.

#![cfg(unix)]

#[path = "common/pty_capture.rs"]
mod pty_capture;

use portable_pty::CommandBuilder;
use std::time::{Duration, Instant};

const COLS: u16 = 40;
const ROWS: u16 = 12;

/// Run one probe scenario to completion on a fresh PTY of fixed geometry and pinned capabilities,
/// returning the full captured wire plus the initial and final termios snapshots. Pins `TERM` /
/// `COLORTERM` so capability detection (and thus the emitted SGR) is identical across scenarios.
fn run(scenario: &str) -> (Vec<u8>, Option<String>, Option<String>) {
    let mut cmd = CommandBuilder::new(pty_capture::example_path("ffi_realtty_probe"));
    cmd.cwd(env!("CARGO_MANIFEST_DIR"));
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.arg(scenario);

    let mut cap = pty_capture::Capture::spawn(cmd, COLS, ROWS, Duration::from_secs(60));
    let initial = cap.initial_termios.clone();

    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        cap.collect_for(Duration::from_millis(50))
            .expect("collect PTY output");
        if let Some(status) = cap.try_wait().expect("poll probe") {
            assert!(
                status.success(),
                "{scenario} probe exited non-zero ({status:?}) — a lease/create assertion failed\n--- wire ---\n{}",
                String::from_utf8_lossy(cap.raw())
            );
            break;
        }
        if Instant::now() >= deadline {
            panic!(
                "{scenario} probe did not exit within 30s\n--- wire ---\n{}",
                String::from_utf8_lossy(cap.raw())
            );
        }
    }
    // Drain any bytes emitted between the last poll and exit (restore tail).
    for _ in 0..5 {
        let _ = cap.collect_for(Duration::from_millis(20));
    }
    let after = cap.termios();
    (cap.raw().to_vec(), initial, after)
}

fn assert_real_tty_wire(
    scenario: &str,
    wire: &[u8],
    initial: &Option<String>,
    after: &Option<String>,
) {
    let has = |needle: &[u8]| wire.windows(needle.len()).any(|w| w == needle);
    assert!(
        has(b"\x1b[?1049h"),
        "{scenario}: wire must enter the alternate screen (real-TTY fullscreen path)"
    );
    assert!(
        has(b"J3-RealTTY"),
        "{scenario}: wire must carry the painted text\n{}",
        String::from_utf8_lossy(wire)
    );
    assert!(
        has(b"\x1b[?25h"),
        "{scenario}: cursor must be shown on restore"
    );
    assert!(
        has(b"\x1b[?1049l"),
        "{scenario}: must leave the alternate screen on restore"
    );
    // Raw mode is a termios ioctl, not a wire byte; the PTY's termios must return to its initial
    // value once the child exits (restore ran).
    assert_eq!(
        after, initial,
        "{scenario}: termios must be restored to its initial value on exit"
    );
}

/// The C-ABI create on a real TTY paints and restores: `gibson_create_context` reaches the live
/// `Context::new` path, enters the alternate screen, paints, and `gibson_destroy_context` restores.
#[test]
fn abi_real_tty_create_renders_and_restores() {
    let (wire, initial, after) = run("abi");
    assert_real_tty_wire("abi", &wire, &initial, &after);
}

/// The native real-TTY path, as a control and the parity reference.
#[test]
fn native_real_tty_create_renders_and_restores() {
    let (wire, initial, after) = run("native");
    assert_real_tty_wire("native", &wire, &initial, &after);
}

/// The real-TTY analogue of Gate I's headless parity: the C-ABI construction/paint/restore wire is
/// byte-identical to the native wire for the same op-script at the same PTY geometry — proving
/// `gibson_create_context`/`gibson_destroy_context` do not alter the live terminal behaviour of
/// `Context::new`/restore.
///
/// Red-first (by construction): were `gibson_create_context` to build a `Context::headless` instead
/// of `Context::new` (the Gate I path), the ABI wire would lose the alternate-screen enter and the
/// live frame entirely, and this equality — and the alt-screen assertion above — would fail.
#[test]
fn abi_wire_equals_native_wire() {
    let (abi, _, _) = run("abi");
    let (native, _, _) = run("native");
    assert_eq!(
        abi,
        native,
        "real-TTY ABI wire diverged from native\n--- abi ---\n{}\n--- native ---\n{}",
        String::from_utf8_lossy(&abi),
        String::from_utf8_lossy(&native)
    );
}
