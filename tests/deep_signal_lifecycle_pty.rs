//! v0.7 node H — how each way of ending a DEEP SIGNAL session leaves the terminal, on a real PTY.
//!
//! While playing, the game holds the terminal in raw mode on the alternate screen with the cursor
//! hidden (each test first checks that, so a "restored" verdict is not vacuous). Every ending that
//! leaves the terminal alive must hand it back cooked (`ICANON|ECHO`), on the main screen, with
//! the cursor shown: a delivered SIGTERM or SIGINT (sent with `kill`; in raw mode a typed Ctrl-C is
//! a key, tested elsewhere), and a panic in the game loop. The process must still report the
//! signal as its cause of death, so a shell or supervisor sees what happened.
//!
//! A hangup leaves no terminal to restore. The game deliberately keeps SIGHUP's default action, so
//! the kernel ends it within a bound: handling SIGHUP would route the shutdown through
//! `poll_event`, which can busy-spin forever on a terminal that hangs up mid-call (#269). The
//! `--record` trace is lost on a hangup; that is the stated cost.
//!
//! Finally, diagnostics that cannot be written (stderr on a full device; the same failure as a
//! dead terminal under `nohup`) must be dropped, not turned into a panic.
#![cfg(unix)]
#[path = "common/pty_capture.rs"]
mod pty_capture;

use portable_pty::{CommandBuilder, ExitStatus};
use pty_capture::{example_path, Capture};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const COOKED: u64 = (libc::ICANON | libc::ECHO) as u64;

fn trace_path(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "deep_signal_lifecycle_{}_{tag}.tsv",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&p);
    p
}

/// Starts a recorded session and waits for its first complete frame. `stderr_to` redirects the
/// game's stderr (through `sh -c 'exec ...'`, so the game is still the direct child).
fn launch(trace: &Path, extra: &[&str], stderr_to: Option<&str>) -> Capture {
    let game = example_path("deep_signal");
    let mut cmd = if let Some(target) = stderr_to {
        let mut c = CommandBuilder::new("/bin/sh");
        c.args(["-c", &format!("exec \"$0\" \"$@\" 2>{target}")]);
        c.arg(&game);
        c
    } else {
        CommandBuilder::new(&game)
    };
    cmd.args(["--seed", "7", "--max-seconds", "30", "--record"]);
    cmd.arg(trace);
    cmd.args(extra);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    let mut cap = Capture::spawn(cmd, 80, 24, Duration::from_secs(45));
    // The first frame, complete: the renderer hides the cursor at the end of the synchronized
    // update, so a read that stops mid-frame (after the HUD text) has not seen it yet.
    cap.collect_until(|o| {
        let s = String::from_utf8_lossy(o);
        s.find("SHARDS")
            .is_some_and(|i| s[i..].contains("\x1b[?2026l"))
    })
    .expect("first complete frame");
    // Positive control: the session really is raw, on the alternate screen, cursor hidden.
    let lflag = cap.local_flags().expect("tcgetattr on the master");
    assert_eq!(
        lflag & COOKED,
        0,
        "raw mode not in effect: c_lflag={lflag:#x}"
    );
    let mut term = vt100::Parser::new(24, 80, 0);
    term.process(cap.raw());
    assert!(
        term.screen().alternate_screen(),
        "not on the alternate screen"
    );
    assert!(
        term.screen().hide_cursor(),
        "cursor not hidden while playing; mode toggles: {:?}",
        toggles(cap.raw())
    );
    // Hold a key so the shutdown happens with input state live.
    for _ in 0..10 {
        cap.write(b"w").unwrap();
        cap.collect_for(Duration::from_millis(30)).unwrap();
    }
    cap
}

/// The private-mode toggles (`ESC [ ? n h|l`) in a byte stream, in order, with byte offsets.
fn toggles(raw: &[u8]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 < raw.len() {
        if raw[i] == 0x1b && raw[i + 1] == b'[' && raw[i + 2] == b'?' {
            let mut j = i + 3;
            while j < raw.len() && raw[j].is_ascii_digit() {
                j += 1;
            }
            if j < raw.len() && (raw[j] == b'h' || raw[j] == b'l') {
                out.push((i, String::from_utf8_lossy(&raw[i + 2..=j]).into_owned()));
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out.push((raw.len(), "<end>".into()));
    out
}

/// Waits for the child to exit, collecting output while the terminal is alive.
fn wait_exit(cap: &mut Capture, bound: Duration, collect: bool) -> ExitStatus {
    let deadline = Instant::now() + bound;
    loop {
        if collect {
            cap.collect_for(Duration::from_millis(50)).unwrap();
        } else {
            std::thread::sleep(Duration::from_millis(20));
        }
        if let Some(status) = cap.try_wait().unwrap() {
            if collect {
                cap.collect_for(Duration::from_millis(100)).unwrap();
            }
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "deep_signal did not exit within {bound:?}"
        );
    }
}

fn assert_restored(cap: &Capture, what: &str) {
    let lflag = cap.local_flags().expect("tcgetattr on the master");
    let mut term = vt100::Parser::new(24, 80, 0);
    term.process(cap.raw());
    let s = term.screen();
    assert_eq!(
        lflag & COOKED,
        COOKED,
        "{what}: terminal left raw (c_lflag={lflag:#x})"
    );
    assert!(
        !s.alternate_screen(),
        "{what}: left on the alternate screen"
    );
    assert!(!s.hide_cursor(), "{what}: cursor left hidden");
}

/// The shutdown path ran to completion: the trace was written through its final `end` line.
fn assert_trace_written(trace: &Path, what: &str) {
    let body = std::fs::read_to_string(trace)
        .unwrap_or_else(|e| panic!("{what}: no trace written ({e}): shutdown path skipped"));
    let _ = std::fs::remove_file(trace);
    assert!(
        body.starts_with("deep_signal-trace v1\t") && body.lines().any(|l| l.starts_with("end\t")),
        "{what}: trace incomplete:\n{body}"
    );
}

fn signal_name(sig: libc::c_int) -> String {
    // SAFETY: strsignal returns a pointer to a static or thread-local NUL-terminated string.
    unsafe { std::ffi::CStr::from_ptr(libc::strsignal(sig)) }
        .to_string_lossy()
        .into_owned()
}

#[test]
fn delivered_sigterm_and_sigint_restore_the_terminal() {
    for (sig, tag) in [(libc::SIGTERM, "term"), (libc::SIGINT, "int")] {
        let trace = trace_path(tag);
        let mut cap = launch(&trace, &[], None);
        let pid = libc::pid_t::try_from(cap.process_id()).unwrap();
        // SAFETY: plain kill(2) to our own direct child, which is still running (not reaped).
        assert_eq!(unsafe { libc::kill(pid, sig) }, 0);
        let status = wait_exit(&mut cap, Duration::from_secs(10), true);
        let what = format!("SIG{}", tag.to_uppercase());
        assert_restored(&cap, &what);
        // The cause of death is still the signal (cleanup, then re-raise), not a clean exit.
        assert_eq!(
            status.signal(),
            Some(signal_name(sig).as_str()),
            "{what}: {status:?}"
        );
        assert_trace_written(&trace, &what);
        let _ = cap.finish();
    }
}

#[test]
fn a_panic_in_the_game_loop_restores_the_terminal() {
    let trace = trace_path("panic");
    let mut cap = launch(&trace, &["--fault-panic-key"], None);
    cap.write(b"!").unwrap();
    let status = wait_exit(&mut cap, Duration::from_secs(10), true);
    assert_restored(&cap, "panic");
    assert_eq!(
        (status.exit_code(), status.signal()),
        (101, None),
        "a Rust panic exits 101: {status:?}"
    );
    let out = String::from_utf8_lossy(cap.raw()).into_owned();
    assert!(
        out.contains("injected fault at tic"),
        "the panic message reached the terminal: {:?}",
        &out[out.len().saturating_sub(400)..]
    );
    let _ = std::fs::remove_file(&trace);
    let _ = cap.finish();
}

#[test]
fn a_hangup_ends_the_session_within_a_bound() {
    let trace = trace_path("hup");
    let mut cap = launch(&trace, &[], None);
    // The contract, checked structurally (a SIGHUP handler would usually still pass the timing
    // check below, and spin only when the hangup races `poll_event`, #269): SIGHUP is not caught.
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string(format!("/proc/{}/status", cap.process_id()))
            .expect("/proc status");
        let caught = status
            .lines()
            .find_map(|l| l.strip_prefix("SigCgt:"))
            .and_then(|v| u64::from_str_radix(v.trim(), 16).ok())
            .expect("SigCgt");
        let bit = |sig: libc::c_int| 1u64 << (sig - 1);
        assert_eq!(
            caught & bit(libc::SIGHUP),
            0,
            "SIGHUP is caught: SigCgt={caught:#x}"
        );
        assert_ne!(
            caught & bit(libc::SIGTERM),
            0,
            "SIGTERM not caught: SigCgt={caught:#x}"
        );
    }
    cap.hang_up();
    let status = wait_exit(&mut cap, Duration::from_secs(15), false);
    assert_eq!(
        status.signal(),
        Some(signal_name(libc::SIGHUP).as_str()),
        "{status:?}"
    );
    let _ = std::fs::remove_file(&trace);
    let _ = cap.finish();
}

#[test]
fn diagnostics_that_cannot_be_written_do_not_panic() {
    // Every write to /dev/full fails (ENOSPC), as writes to a hung-up terminal fail (EIO).
    let trace = trace_path("full");
    let mut cap = launch(&trace, &[], Some("/dev/full"));
    cap.write(b"q").unwrap();
    let status = wait_exit(&mut cap, Duration::from_secs(10), true);
    assert_restored(&cap, "quit with stderr unwritable");
    assert_eq!(
        (status.exit_code(), status.signal()),
        (0, None),
        "the exit summary could not be written; that must not become a panic (101): {status:?}"
    );
    assert_trace_written(&trace, "quit with stderr unwritable");
    let _ = cap.finish();
}
