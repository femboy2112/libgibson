//! Defect F (issue #131): terminal-restore behaviour across a hangup.
//!
//! Proves the *graceful* consumer pattern — catch `BrokenPipe`, drive
//! [`gibson::Context::restore`], then exit — runs the restore path to
//! completion without panicking or hanging on a terminal that is already gone,
//! and that raw mode genuinely took effect on the real slave termios first (so
//! the restore claim is not vacuous).
//!
//! Deliberately NOT gated: latency, and the post-hangup settability of the dead
//! terminal. `tcsetattr`/`tcgetattr` on a severed slave may fail with `EIO`,
//! which is honest best-effort cleanup, not a defect — the probe records those
//! as informational `RESTORE_RESULT`/`C_LFLAG` lines, and we require only that
//! restore *completed*.
#![cfg(unix)]

use portable_pty::CommandBuilder;
use std::time::{Duration, Instant};

#[path = "common/pty_capture.rs"]
mod pty_capture;

/// Waits up to `bound` for the child to exit; returns its exit code, or `None`
/// if it was still running — here that means `restore()` hung on the dead tty.
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

/// The value of one `KEY=VALUE` line in the probe's evidence log.
fn field<'a>(log: &'a str, key: &str) -> Option<&'a str> {
    log.lines()
        .find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
}

fn has_marker(log: &str, marker: &str) -> bool {
    log.lines().any(|l| l == marker)
}

fn parse_hex(v: &str) -> Option<u64> {
    v.strip_prefix("0x")
        .and_then(|h| u64::from_str_radix(h, 16).ok())
}

#[test]
fn graceful_hangup_restore_runs_to_completion_and_raw_mode_took_effect() {
    // Unique evidence file on a regular filesystem: it outlives the PTY, which
    // the hangup severs (post-hangup child stdout goes to /dev/null).
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    let log_path = std::env::temp_dir().join(format!("gibson_hangup_restore_{unique}.log"));
    let _ = std::fs::remove_file(&log_path);

    let mut cmd = CommandBuilder::new(pty_capture::example_path("hangup_restore_probe"));
    cmd.env("GIBSON_HANGUP_LOG", &log_path);
    let mut cap = pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_secs(12));

    cap.collect_until(|b| String::from_utf8_lossy(b).contains("READY"))
        .expect("probe should reach READY");
    // Park the child inside poll_event, mid-wait — the real mid-poll hangup case.
    cap.collect_for(Duration::from_millis(150)).expect("settle");

    cap.hang_up();

    // Generous bound: the whole point is that restore() must not hang on a dead
    // terminal. `None` here means it hung (or the hangup never surfaced).
    let code = wait_exit_code(&mut cap, Duration::from_millis(3000));
    assert_eq!(
        code,
        Some(3),
        "child must handle BrokenPipe gracefully (restore + exit 3) within 3s of \
         the hangup; None means restore() hung or the hangup never surfaced"
    );

    let log = std::fs::read_to_string(&log_path).expect("probe evidence log");
    let _ = std::fs::remove_file(&log_path);

    // --- Positive control: raw mode genuinely mutated the real slave termios. ---
    let a = field(&log, "A_LFLAG")
        .and_then(parse_hex)
        .unwrap_or_else(|| panic!("A_LFLAG must be recorded; log:\n{log}"));
    let b = field(&log, "B_LFLAG")
        .and_then(parse_hex)
        .unwrap_or_else(|| panic!("B_LFLAG must be recorded; log:\n{log}"));
    let icanon = libc::ICANON as u64;
    let echo = libc::ECHO as u64;
    assert!(
        (a & icanon) != 0 && (a & echo) != 0,
        "before raw mode the slave must be canonical (ICANON|ECHO set); A_LFLAG={a:#x}"
    );
    assert!(
        (b & icanon) == 0 && (b & echo) == 0,
        "entering interactive mode must clear ICANON|ECHO on the real slave termios \
         (proves raw mode took effect, so the restore claim is non-vacuous); B_LFLAG={b:#x}"
    );

    // --- The graceful path reached BrokenPipe and ran restore to completion. ---
    assert!(
        has_marker(&log, "BROKENPIPE_SEEN"),
        "the hangup must surface as ErrorKind::BrokenPipe to the consumer; log:\n{log}"
    );
    assert!(
        has_marker(&log, "RESTORE_DONE"),
        "Context::restore() must return without panicking or hanging on a hung-up \
         terminal; log:\n{log}"
    );

    // RESTORE_RESULT / C_LFLAG are recorded but intentionally NOT asserted on:
    // a tcsetattr/tcgetattr on a severed slave may fail with EIO, which is honest
    // best-effort cleanup. We require only that restore was attempted and recorded.
    assert!(
        field(&log, "RESTORE_RESULT").is_some(),
        "restore result must be recorded (informational); log:\n{log}"
    );
}
