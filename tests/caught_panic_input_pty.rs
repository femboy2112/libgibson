//! Input-only recovery after a caught owner-thread panic. Each case uses its own PTY and process.
//! This does not claim recovery of alternate-screen, cursor, renderer, mouse, or focus state.
#![cfg(unix)]

#[path = "common/pty_capture.rs"]
mod pty_capture;

use gibson::context::{Context, RenderMode};
use gibson::input::{Event, KeyCode};
use portable_pty::CommandBuilder;
use pty_capture::Capture;
use std::time::{Duration, Instant};

const CHILD: &str = "GIBSON_CAUGHT_PANIC_INPUT_CHILD";
const PASTE_ON: &[u8] = b"\x1b[?2004h";
const PASTE_OFF: &[u8] = b"\x1b[?2004l";

fn contains(buf: &[u8], needle: &[u8]) -> bool {
    buf.windows(needle.len()).any(|part| part == needle)
}

fn count(buf: &[u8], needle: &[u8]) -> usize {
    buf.windows(needle.len())
        .filter(|part| *part == needle)
        .count()
}

fn wait_for(cap: &mut Capture, needle: &'static [u8]) {
    cap.collect_until(|output| contains(output, needle))
        .unwrap_or_else(|e| panic!("missing {:?}: {e}", String::from_utf8_lossy(needle)));
}

fn run_case(role: &str) -> Vec<u8> {
    let mut cmd = CommandBuilder::new(std::env::current_exe().unwrap());
    cmd.args([
        "--exact",
        "caught_panic_input_child",
        "--nocapture",
        "--test-threads=1",
    ]);
    cmd.env(CHILD, role);
    let mut cap = Capture::spawn(cmd, 80, 24, Duration::from_secs(15));
    wait_for(&mut cap, b"BEFORE_READY");
    let flags = cap.local_flags().expect("termios before first input");
    assert_eq!(
        flags & (libc::ICANON | libc::ECHO) as u64,
        0,
        "control must begin in raw mode"
    );
    cap.write(b"a").unwrap();
    wait_for(&mut cap, b"AFTER_READY");
    // The marker is emitted immediately before poll. Give poll an observable opportunity to
    // enter raw mode before sending a key; on the broken path send anyway and let its timeout
    // report the missed key. This avoids relying on a scheduler delay as evidence of re-entry.
    let readiness_deadline = Instant::now() + Duration::from_millis(500);
    loop {
        let flags = cap.local_flags().expect("termios during second poll");
        if flags & (libc::ICANON | libc::ECHO) as u64 == 0 || Instant::now() >= readiness_deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    cap.write(b"b").unwrap();
    wait_for(&mut cap, b"AFTER_RESULT");
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        cap.collect_for(Duration::from_millis(20)).unwrap();
        if let Some(status) = cap.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "child did not exit");
    };
    cap.collect_all().unwrap();
    let raw = cap.raw();
    assert!(status.success(), "{role}: {}", String::from_utf8_lossy(raw));
    assert!(
        contains(raw, b"AFTER_RESULT key=1 raw=1"),
        "{role}: {}",
        String::from_utf8_lossy(raw)
    );
    raw.to_vec()
}

#[test]
fn no_panic_input_control() {
    let raw = run_case("control");
    assert_eq!(count(&raw, PASTE_ON), 1);
    assert_eq!(count(&raw, PASTE_OFF), 1);
}

#[test]
fn non_owner_panic_does_not_reinitialize_input() {
    let raw = run_case("worker");
    assert_eq!(count(&raw, PASTE_ON), 1);
    assert_eq!(count(&raw, PASTE_OFF), 1);
}

#[test]
fn caught_owner_panic_reenters_raw_mode_and_bracketed_paste_on_poll() {
    let raw = run_case("owner");
    assert_eq!(
        count(&raw, PASTE_ON),
        2,
        "initial request and request after panic"
    );
    assert_eq!(count(&raw, PASTE_OFF), 2, "panic cleanup and restore");
}

#[test]
fn caught_owner_panic_with_focus_requested_still_recovers_input() {
    let raw = run_case("owner-focus");
    assert_eq!(count(&raw, PASTE_ON), 2);
    assert_eq!(count(&raw, PASTE_OFF), 2);
}

#[test]
fn caught_panic_input_child() {
    let Ok(role) = std::env::var(CHILD) else {
        return;
    };
    let mut ctx = Context::new(RenderMode::Inline).unwrap();
    ctx.session.enter_interactive().unwrap();
    if role == "owner-focus" {
        ctx.session.set_focus_reporting(true).unwrap();
    }
    eprintln!("BEFORE_READY");
    let first = ctx.poll_event(Duration::from_secs(4)).unwrap();
    assert!(matches!(first, Some(Event::Key(k)) if k.code == KeyCode::Char('a')));
    if role.starts_with("owner") {
        assert!(std::panic::catch_unwind(|| panic!("owner panic (caught)")).is_err());
    } else if role == "worker" {
        assert!(std::thread::spawn(|| panic!("worker panic (caught)"))
            .join()
            .is_err());
    }
    eprintln!("AFTER_READY");
    let second = ctx.poll_event(Duration::from_secs(2)).unwrap();
    let key = matches!(second, Some(Event::Key(k)) if k.code == KeyCode::Char('b'));
    let mut state = std::mem::MaybeUninit::<libc::termios>::uninit();
    // SAFETY: stdin is this child's live PTY; tcgetattr initializes state on success.
    assert_eq!(
        unsafe { libc::tcgetattr(libc::STDIN_FILENO, state.as_mut_ptr()) },
        0
    );
    let state = unsafe { state.assume_init() };
    let raw = state.c_lflag & (libc::ICANON | libc::ECHO) == 0;
    eprintln!("AFTER_RESULT key={} raw={}", u8::from(key), u8::from(raw));
    ctx.restore().unwrap();
}
