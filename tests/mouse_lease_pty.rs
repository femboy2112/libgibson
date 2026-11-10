//! A restored session must not change a later owner's mouse-reporting state (#330).
//! These Linux PTY tests inspect requested escape sequences, not terminal support.
#![cfg(target_os = "linux")]

#[path = "common/pty_capture.rs"]
mod pty_capture;

use gibson::TerminalSession;
use portable_pty::CommandBuilder;
use std::time::{Duration, Instant};

const CHILD: &str = "GIBSON_MOUSE_LEASE_CHILD";
const ON: &[u8] = b"\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1015h\x1b[?1006h";
const OFF: &[u8] = b"\x1b[?1006l\x1b[?1015l\x1b[?1003l\x1b[?1002l\x1b[?1000l";
const ALT_ON: &[u8] = b"\x1b[?1049h";
const ALT_OFF: &[u8] = b"\x1b[?1049l";

fn capture(role: &str) -> Vec<u8> {
    let mut cmd = CommandBuilder::new(std::env::current_exe().unwrap());
    cmd.args([
        "--exact",
        "mouse_lease_child",
        "--nocapture",
        "--test-threads=1",
    ]);
    cmd.env(CHILD, role);
    let mut cap = pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_secs(10));
    cap.collect_until(|b| b.windows(11).any(|w| w == b"MOUSE_DONE "))
        .expect("child completion marker");
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        cap.collect_for(Duration::from_millis(20)).unwrap();
        if let Some(status) = cap.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "child did not exit");
    };
    cap.collect_all().unwrap();
    assert!(status.success(), "{}", String::from_utf8_lossy(cap.raw()));
    cap.raw().to_vec()
}

fn mouse_requests(raw: &[u8]) -> Vec<&'static str> {
    let mut requests = Vec::new();
    for i in 0..raw.len() {
        if raw[i..].starts_with(ON) {
            requests.push("on");
        }
        if raw[i..].starts_with(OFF) {
            requests.push("off");
        }
    }
    requests
}

#[test]
fn restored_session_cannot_enable_or_disable_mouse_for_new_owner() {
    let raw = capture("stale");
    assert_eq!(mouse_requests(&raw), Vec::<&str>::new());
    assert!(String::from_utf8_lossy(&raw).contains("MOUSE_DONE rejected=true,true"));
}

#[test]
fn current_owner_mouse_requests_and_restore_still_work() {
    let raw = capture("owner");
    assert_eq!(mouse_requests(&raw), ["on", "off", "on", "off"]);
}

#[test]
fn stale_mouse_calls_cannot_hide_owner_capture_from_panic_cleanup() {
    let raw = capture("panic");
    // The hook disables the active request; ordinary restore also attempts cleanup.
    assert_eq!(mouse_requests(&raw), ["on", "off", "off"]);
    assert!(String::from_utf8_lossy(&raw).contains("MOUSE_DONE rejected=true,true"));
}

#[test]
fn caught_owner_panic_allows_a_fresh_mouse_request() {
    let raw = capture("panic-retry");
    assert_eq!(mouse_requests(&raw), ["on", "off", "on", "off"]);
    assert!(String::from_utf8_lossy(&raw).contains("MOUSE_DONE invalidated=true,confirmed=true"));
}

#[test]
fn non_owner_panic_does_not_invalidate_mouse_request() {
    let raw = capture("worker-retry");
    assert_eq!(mouse_requests(&raw), ["on", "off"]);
    assert!(String::from_utf8_lossy(&raw).contains("MOUSE_DONE confirmed=true,true"));
}

#[test]
fn owner_after_a_caught_panic_has_fresh_mouse_state() {
    let raw = capture("panic-sequential");
    assert_eq!(mouse_requests(&raw), ["on", "off", "off", "on", "off"]);
    assert!(String::from_utf8_lossy(&raw).contains("MOUSE_DONE confirmed=true,true"));
}

fn screen_requests(raw: &[u8]) -> Vec<&'static str> {
    let mut requests = Vec::new();
    for i in 0..raw.len() {
        if raw[i..].starts_with(ALT_ON) {
            requests.push("on");
        }
        if raw[i..].starts_with(ALT_OFF) {
            requests.push("off");
        }
    }
    requests
}

#[test]
fn restored_session_cannot_switch_a_new_owners_screen() {
    let raw = capture("screen-stale");
    assert_eq!(screen_requests(&raw), ["on", "off"]);
    assert!(String::from_utf8_lossy(&raw).contains("MOUSE_DONE rejected=true,true"));
}

#[test]
fn current_owner_screen_switches_and_restore_still_work() {
    let raw = capture("screen-owner");
    assert_eq!(screen_requests(&raw), ["on", "off", "on", "off"]);
}

#[test]
fn headless_and_redirected_sessions_remain_noops() {
    let mut headless = TerminalSession::headless(40, 15);
    for _ in 0..2 {
        headless.set_mouse_capture(true).unwrap();
        headless.set_mouse_capture(false).unwrap();
        headless.enter_alternate_screen().unwrap();
        headless.leave_alternate_screen().unwrap();
        headless.restore().unwrap();
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "mouse_lease_child", "--nocapture"])
        .env(CHILD, "non-tty")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("NON_TTY_OK"));
}

#[test]
fn mouse_lease_child() {
    let Ok(role) = std::env::var(CHILD) else {
        return;
    };
    if role == "non-tty" {
        let mut redirected = TerminalSession::new().unwrap();
        assert!(!redirected.is_tty);
        for _ in 0..2 {
            redirected.set_mouse_capture(true).unwrap();
            redirected.set_mouse_capture(false).unwrap();
            redirected.enter_alternate_screen().unwrap();
            redirected.leave_alternate_screen().unwrap();
            redirected.restore().unwrap();
        }
        eprintln!("NON_TTY_OK");
        return;
    }
    let mut old = TerminalSession::new().unwrap();
    assert!(old.is_tty);
    old.restore().unwrap();
    let mut owner = TerminalSession::new().unwrap();
    assert!(owner.is_tty);
    if role == "screen-owner" {
        owner.enter_alternate_screen().unwrap();
        owner.enter_alternate_screen().unwrap();
        owner.leave_alternate_screen().unwrap();
        owner.leave_alternate_screen().unwrap();
        owner.enter_alternate_screen().unwrap();
        owner.restore().unwrap();
        eprintln!("MOUSE_DONE owner");
        return;
    }
    if role == "screen-stale" {
        owner.enter_alternate_screen().unwrap();
        let enter_rejected = matches!(old.enter_alternate_screen(), Err(e) if e.kind() == std::io::ErrorKind::NotConnected);
        let leave_rejected = matches!(old.leave_alternate_screen(), Err(e) if e.kind() == std::io::ErrorKind::NotConnected);
        owner.restore().unwrap();
        eprintln!("MOUSE_DONE rejected={enter_rejected},{leave_rejected}");
        return;
    }
    if role == "owner" {
        owner.set_mouse_capture(true).unwrap();
        owner.set_mouse_capture(true).unwrap();
        owner.set_mouse_capture(false).unwrap();
        owner.set_mouse_capture(false).unwrap();
        owner.set_mouse_capture(true).unwrap();
        owner.restore().unwrap();
        eprintln!("MOUSE_DONE owner");
        return;
    }
    if role == "panic-retry" {
        owner.set_mouse_capture(true).unwrap();
        assert!(std::panic::catch_unwind(|| panic!("caught owner panic")).is_err());
        let invalidated = !owner.mouse_capture_enabled();
        owner.set_mouse_capture(true).unwrap();
        let confirmed = owner.mouse_capture_enabled();
        owner.restore().unwrap();
        eprintln!("MOUSE_DONE invalidated={invalidated},confirmed={confirmed}");
        return;
    }
    if role == "worker-retry" {
        owner.set_mouse_capture(true).unwrap();
        assert!(std::thread::spawn(|| panic!("caught worker panic"))
            .join()
            .is_err());
        let before_retry = owner.mouse_capture_enabled();
        owner.set_mouse_capture(true).unwrap();
        let after_retry = owner.mouse_capture_enabled();
        owner.restore().unwrap();
        eprintln!("MOUSE_DONE confirmed={before_retry},{after_retry}");
        return;
    }
    if role == "panic-sequential" {
        owner.set_mouse_capture(true).unwrap();
        assert!(std::panic::catch_unwind(|| panic!("caught owner panic")).is_err());
        owner.restore().unwrap();
        let mut next = TerminalSession::new().unwrap();
        next.set_mouse_capture(true).unwrap();
        let before_retry = next.mouse_capture_enabled();
        next.set_mouse_capture(true).unwrap();
        let after_retry = next.mouse_capture_enabled();
        next.restore().unwrap();
        eprintln!("MOUSE_DONE confirmed={before_retry},{after_retry}");
        return;
    }
    if role == "panic" {
        owner.set_mouse_capture(true).unwrap();
    }
    let enable_rejected = matches!(old.set_mouse_capture(true), Err(e) if e.kind() == std::io::ErrorKind::NotConnected);
    let disable_rejected = matches!(old.set_mouse_capture(false), Err(e) if e.kind() == std::io::ErrorKind::NotConnected);
    if role == "panic" {
        assert!(std::panic::catch_unwind(|| panic!("caught owner panic")).is_err());
    }
    owner.restore().unwrap();
    eprintln!("MOUSE_DONE rejected={enable_rejected},{disable_rejected}");
}
