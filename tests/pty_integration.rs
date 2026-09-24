//! Unix PTY probes use poll-based bounded capture; these are not ConPTY coverage.
#![cfg(unix)]

use portable_pty::{native_pty_system, CommandBuilder, PtySize};

#[path = "common/pty_capture.rs"]
mod pty_capture;
use std::time::Duration;

#[test]
fn test_pty_agent_chat_lifecycle() {
    let mut cmd = CommandBuilder::new(pty_capture::example_path("agent_chat"));
    cmd.arg("--auto");
    let mut capture = pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_secs(10));
    capture
        .collect_until(|bytes| String::from_utf8_lossy(bytes).contains("[metrics]"))
        .expect("capture agent chat");
    let output = String::from_utf8_lossy(&capture.finish()).into_owned();

    // Verify key invariants in output
    assert!(
        output.contains("LibGibson Engine"),
        "Output missing engine banner"
    );
    assert!(
        output.contains("Analyzing codebase architecture"),
        "Output missing assistant message"
    );
    assert!(
        output.contains("Allow command?"),
        "Output missing permission prompt"
    );
    assert!(
        output.contains("Permission granted: Yes, once"),
        "Output missing decision commit"
    );
    assert!(
        output.contains("[metrics]"),
        "Output missing performance metrics"
    );
}

#[test]
fn test_pty_window_resize_safety() {
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("Failed to create PTY pair");

    // Simulate resizing the terminal window during session
    pair.master
        .resize(PtySize {
            rows: 30,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("Failed to resize PTY");

    pair.master
        .resize(PtySize {
            rows: 15,
            cols: 40,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("Failed to shrink PTY");
}

#[test]
fn pty_capture_bounds_silent_and_partial_line_children_and_reaps() {
    for script in [
        "exec sleep 30",
        "printf partial; exec sleep 30",
        "printf early",
    ] {
        let mut cmd = CommandBuilder::new("sh");
        cmd.args(["-c", script]);
        let start = std::time::Instant::now();
        let mut capture = pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_millis(100));
        capture
            .collect_until(|_| false)
            .expect("controlled capture");
        let raw = capture.finish(); // Asserts direct child was reaped, no reader exists.
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "capture deadline was not enforced"
        );
        if script.contains("partial") {
            assert!(raw.windows(7).any(|s| s == b"partial"));
        }
        if script.contains("early") {
            assert!(raw.windows(5).any(|s| s == b"early"));
        }
    }
}

#[test]
fn pty_capture_reaps_on_assertion_unwind() {
    let child_pid = std::sync::atomic::AtomicU32::new(0);
    let result = std::panic::catch_unwind(|| {
        let mut cmd = CommandBuilder::new("sh");
        cmd.args(["-c", "exec sleep 30"]);
        let capture = pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_millis(100));
        child_pid.store(capture.process_id(), std::sync::atomic::Ordering::Relaxed);
        panic!("controlled assertion failure");
    });
    assert!(result.is_err());
    let pid = child_pid.load(std::sync::atomic::Ordering::Relaxed) as libc::pid_t;
    assert!(pid > 0);
    // SAFETY: waitpid probes only our recorded child; WNOHANG cannot block.
    let result = unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) };
    assert_eq!(result, -1, "child remained live or unreaped after unwind");
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
}
