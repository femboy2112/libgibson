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

// Re-entering raw input before synchronizing the ended Kitty request must not
// allow that later reconciliation to erase the new raw-mode cleanup obligation.
#[test]
fn key_reporting_reconciliation_after_reentry_preserves_raw_cleanup() {
    for role in ["direct", "disable", "poll-from", "reenable", "sync-before"] {
        let mut cmd = CommandBuilder::new(std::env::current_exe().unwrap());
        cmd.args(["--exact", "key_reconciliation_child", "--nocapture"]);
        cmd.env("GIBSON_KEY_RECONCILIATION_CHILD", role);
        let mut cap = Capture::spawn(cmd, 80, 24, Duration::from_secs(15));
        wait_for(&mut cap, b"KEY_RECONCILIATION_RESULT");
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            cap.collect_for(Duration::from_millis(20)).unwrap();
            if let Some(status) = cap.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "child did not exit");
        };
        cap.collect_all().unwrap();
        assert!(
            status.success(),
            "{role}: {}",
            String::from_utf8_lossy(cap.raw())
        );
        assert!(
            contains(cap.raw(), b"KEY_RECONCILIATION_RESULT initial_raw=1 post_panic_cooked=1 reentered_raw=1 after_sync_raw=1 restored=1"),
            "{role}: {}",
            String::from_utf8_lossy(cap.raw())
        );
    }
}

#[test]
fn key_reconciliation_child() {
    let Ok(role) = std::env::var("GIBSON_KEY_RECONCILIATION_CHILD") else {
        return;
    };
    fn flags() -> libc::tcflag_t {
        let mut state = std::mem::MaybeUninit::<libc::termios>::uninit();
        assert_eq!(
            unsafe { libc::tcgetattr(libc::STDIN_FILENO, state.as_mut_ptr()) },
            0
        );
        unsafe { state.assume_init() }.c_lflag & (libc::ICANON | libc::ECHO)
    }
    let cooked = libc::ICANON | libc::ECHO;
    let mut ctx = Context::new(RenderMode::Inline).unwrap();
    ctx.session.set_key_release_reporting(true).unwrap();
    let initial_raw = flags() == 0;
    assert!(std::panic::catch_unwind(|| panic!("caught owner panic")).is_err());
    let post_panic_cooked = flags() == cooked;
    if role == "sync-before" {
        ctx.session.set_key_release_reporting(false).unwrap();
    }
    ctx.enter_interactive().unwrap();
    let reentered_raw = flags() == 0;
    match role.as_str() {
        "direct" | "sync-before" => {}
        "disable" => ctx.session.set_key_release_reporting(false).unwrap(),
        "poll-from" => {
            let mut source = gibson::event_source::ScriptedSource::from_events(vec![]);
            assert!(matches!(
                ctx.poll_from(&mut source, Duration::ZERO).unwrap(),
                Some(Event::KeyReleaseReporting(
                    gibson::input::KeyReleaseReporting::Unsupported
                ))
            ));
            assert!(ctx
                .poll_from(&mut source, Duration::ZERO)
                .unwrap()
                .is_none());
        }
        "reenable" => ctx.session.set_key_release_reporting(true).unwrap(),
        _ => panic!("unknown role"),
    }
    let after_sync_raw = flags() == 0;
    ctx.restore().unwrap();
    let restored = flags() == cooked;
    eprintln!("KEY_RECONCILIATION_RESULT initial_raw={} post_panic_cooked={} reentered_raw={} after_sync_raw={} restored={}", u8::from(initial_raw), u8::from(post_panic_cooked), u8::from(reentered_raw), u8::from(after_sync_raw), u8::from(restored));
}

// A restored session must never regain the terminal behind a later owner's back.
// Each role has a separate PTY/process, so a failed assertion cannot contaminate another case.
#[test]
fn relinquished_context_poll_and_session_entry_cannot_touch_a_new_owner() {
    for role in ["poll", "session", "poll-pending-panic"] {
        let mut cmd = CommandBuilder::new(std::env::current_exe().unwrap());
        cmd.args(["--exact", "relinquished_session_child", "--nocapture"]);
        cmd.env("GIBSON_RELINQUISHED_INPUT_CHILD", role);
        let mut cap = Capture::spawn(cmd, 80, 24, Duration::from_secs(15));
        wait_for(&mut cap, b"RELINQUISHED_RESULT");
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            cap.collect_for(Duration::from_millis(20)).unwrap();
            if let Some(status) = cap.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "child did not exit");
        };
        cap.collect_all().unwrap();
        assert!(
            status.success(),
            "{role}: {}",
            String::from_utf8_lossy(cap.raw())
        );
        assert!(
            contains(
                cap.raw(),
                b"RELINQUISHED_RESULT rejected=1 canonical=1 owner_raw=1 restored=1"
            ),
            "{role}: {}",
            String::from_utf8_lossy(cap.raw())
        );
    }
}

#[test]
fn relinquished_session_child() {
    let Ok(role) = std::env::var("GIBSON_RELINQUISHED_INPUT_CHILD") else {
        return;
    };
    fn flags() -> libc::tcflag_t {
        let mut state = std::mem::MaybeUninit::<libc::termios>::uninit();
        assert_eq!(
            unsafe { libc::tcgetattr(libc::STDIN_FILENO, state.as_mut_ptr()) },
            0
        );
        let state = unsafe { state.assume_init() };
        state.c_lflag & (libc::ICANON | libc::ECHO)
    }
    let mut old = Context::new(RenderMode::Inline).unwrap();
    old.enter_interactive().unwrap();
    old.restore().unwrap();
    assert_eq!(
        flags(),
        libc::ICANON | libc::ECHO,
        "restore must leave canonical input and echo enabled"
    );
    let mut owner = Context::new(RenderMode::Inline).unwrap();
    if role == "poll-pending-panic" {
        owner.enter_interactive().unwrap();
        assert!(std::panic::catch_unwind(|| panic!("new owner panic (caught)")).is_err());
        assert_eq!(
            flags(),
            libc::ICANON | libc::ECHO,
            "panic hook must disable raw mode"
        );
    }
    let rejected = match role.as_str() {
        "poll" | "poll-pending-panic" => old.poll_event(Duration::ZERO).is_err(),
        "session" => old.session.enter_interactive().is_err(),
        _ => unreachable!(),
    };
    let still_canonical = flags() == libc::ICANON | libc::ECHO;
    // Polling the owner must still consume its own panic notification and recover raw mode.
    owner.poll_event(Duration::ZERO).unwrap();
    let owner_raw = flags() == 0;
    owner.restore().unwrap();
    let restored = flags() == libc::ICANON | libc::ECHO;
    eprintln!(
        "RELINQUISHED_RESULT rejected={} canonical={} owner_raw={} restored={}",
        u8::from(rejected),
        u8::from(still_canonical),
        u8::from(owner_raw),
        u8::from(restored)
    );
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
