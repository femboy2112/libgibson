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

// --- Gate K · K3: real-PTY INPUT + RESIZE through the C ABI ----------------
//
// J3 (above) ceded input/resize. ABI v1 already ships `gibson_poll_event` and
// `GIBSON_EVENT_{KEY,PASTE,RESIZE}`, but no test ever called them — the whole
// event-decode path was unverified on a real terminal. These tests inject real
// keystrokes / a real SIGWINCH on the PTY master and assert the ABI's event
// RECEIPT (emitted by the `abi-input` / `abi-resize` probe scenarios). Keys and
// resize live in separate scenarios so a key never coincides with a resize (the
// crossterm #15 queue-until-next-key collision). Still test-only: no `src/`
// change, no ABI change, no new symbol.

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    needle.len() <= hay.len() && hay.windows(needle.len()).any(|w| w == needle)
}

/// Spawn an interactive probe scenario, wait until it has entered raw mode
/// (READY, printed only after the first poll), run `after_ready` to inject input
/// on the PTY master, then collect the receipt until the probe prints DONE.
fn drive(scenario: &str, after_ready: impl FnOnce(&mut pty_capture::Capture)) -> String {
    let mut cmd = CommandBuilder::new(pty_capture::example_path("ffi_realtty_probe"));
    cmd.cwd(env!("CARGO_MANIFEST_DIR"));
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.arg(scenario);
    let mut cap = pty_capture::Capture::spawn(cmd, COLS, ROWS, Duration::from_secs(60));

    cap.collect_until(|b| contains(b, b"READY"))
        .unwrap_or_else(|e| panic!("{scenario}: never reached READY: {e}"));
    after_ready(&mut cap);

    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let _ = cap.collect_for(Duration::from_millis(50));
        if contains(cap.raw(), b"DONE") {
            break;
        }
        if cap.try_wait().expect("poll probe").is_some() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "{scenario}: probe did not finish within 30s\n--- wire ---\n{}",
            String::from_utf8_lossy(cap.raw())
        );
    }
    for _ in 0..5 {
        let _ = cap.collect_for(Duration::from_millis(20));
    }
    String::from_utf8_lossy(cap.raw()).into_owned()
}

/// Real keystrokes injected on the PTY master are decoded by `gibson_poll_event`
/// into the right ABI events: a plain char, a CSI special key, a control-key
/// mapping, a modified key, and a bracketed paste — exercising every branch of
/// the key/paste decode that ABI v1 exposes.
#[test]
fn abi_poll_event_surfaces_keys_and_paste_on_a_real_tty() {
    let receipt = drive("abi-input", |cap| {
        cap.write(b"a").expect("write a"); // Char('a')
        cap.write(b"\x1b[A").expect("write Up"); // GIBSON_KEY_UP
        cap.write(b"\r").expect("write Enter"); // GIBSON_KEY_ENTER
        cap.write(b"\x03").expect("write Ctrl-C"); // Char('c') + CONTROL (raw mode: not a signal)
        cap.write(b"\x1b[200~hi\x1b[201~").expect("write paste"); // bracketed paste "hi"
        cap.write(b"q").expect("write sentinel"); // quit
    });

    assert!(
        contains(receipt.as_bytes(), b"EVKEY code=97 mod=0"),
        "plain 'a' must decode as Char(97) with no modifiers\n{receipt}"
    );
    assert!(
        contains(
            receipt.as_bytes(),
            format!("EVKEY code={} mod=0", 0x1004u32).as_bytes()
        ),
        "Up arrow (CSI A) must decode as GIBSON_KEY_UP (0x1004)\n{receipt}"
    );
    assert!(
        contains(
            receipt.as_bytes(),
            format!("EVKEY code={}", 0x1001u32).as_bytes()
        ),
        "Enter must decode as GIBSON_KEY_ENTER (0x1001)\n{receipt}"
    );
    // Ctrl-C: Char('c') = 99, with crossterm's CONTROL bit = 2 (pinned 0.29.0).
    assert!(
        contains(receipt.as_bytes(), b"EVKEY code=99 mod=2"),
        "Ctrl-C must decode as Char(99) carrying the CONTROL modifier bit\n{receipt}"
    );
    assert!(
        contains(receipt.as_bytes(), b"EVPASTE text=hi"),
        "a bracketed paste must decode as a Paste event with the pasted text\n{receipt}"
    );
    assert!(
        contains(receipt.as_bytes(), b"DONE"),
        "probe must reach its sentinel\n{receipt}"
    );
}

/// A real SIGWINCH on the PTY master surfaces through the ABI as one
/// `GIBSON_EVENT_RESIZE` carrying the new geometry, and a render afterward still
/// succeeds at the new size.
#[test]
fn abi_poll_event_surfaces_resize_on_a_real_tty() {
    let receipt = drive("abi-resize", |cap| {
        cap.resize(100, 40); // deliver SIGWINCH with a geometry clearly != the 40x12 spawn size
    });

    assert!(
        contains(receipt.as_bytes(), b"EVRESIZE cols=100 rows=40"),
        "a PTY resize must surface as GIBSON_EVENT_RESIZE with the new geometry\n{receipt}"
    );
    assert!(
        contains(receipt.as_bytes(), b"RENDERED_AFTER_RESIZE status=0"),
        "a render after the resize must still succeed\n{receipt}"
    );
    assert!(
        contains(receipt.as_bytes(), b"DONE"),
        "probe must reach completion\n{receipt}"
    );
}

/// RXIV Gate G1 — a controlled real-TTY input -> state -> render loop entirely through ABI v1:
/// three Right arrows injected on the PTY master are decoded by `gibson_poll_event`, each one
/// advances application state that is rebuilt and painted with `gibson_render`. The PAINTED frame
/// is verified on a vt100 parse of the alternate screen (not only the receipts), so this proves the
/// closed loop a terminal game needs from the public ABI — on a real TTY (a headless context has
/// no input source; see docs/SDK_DOOM_HORIZON.md).
#[test]
fn abi_real_tty_input_drives_state_that_is_rendered() {
    let wire = drive("abi-loop", |cap| {
        for _ in 0..3 {
            cap.write(b"\x1b[C").expect("write Right");
            let _ = cap.collect_for(Duration::from_millis(60));
        }
        cap.write(b"q").expect("write sentinel");
    });
    let bytes = wire.as_bytes();
    for n in 1..=3 {
        assert!(
            contains(bytes, format!("STATE count={n}").as_bytes()),
            "state step {n} missing\n{wire}"
        );
    }
    // Parse the stream up to the alternate-screen exit: the painted frame, not the receipt.
    let end = bytes
        .windows(8)
        .rposition(|w| w == b"\x1b[?1049l")
        .unwrap_or(bytes.len());
    let mut p = vt100::Parser::new(ROWS, COLS, 0);
    p.process(&bytes[..end]);
    let screen = p.screen().contents();
    assert!(
        screen.contains("abi-loop count 3"),
        "the ABI-rendered frame must show the state the input drove:\n{screen}"
    );
}
