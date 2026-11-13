//! Lease authority after restore and across owners (#330). Each case runs in its own PTY and
//! process.
//!
//! * The lease, not the public `is_tty` field, decides whether `restore()` and `Drop` have a
//!   terminal to give back. An owner that clears `is_tty` must still leave the terminal cooked
//!   and the lease free.
//! * A restored `Context` cannot render or commit into the terminal of the next owner. The
//!   restored session's mode setters are covered by `mouse_lease_pty.rs`,
//!   `key_release_reporting_pty.rs` and `enter_interactive_pty.rs`.
#![cfg(target_os = "linux")]

#[path = "common/pty_capture.rs"]
mod pty_capture;

use gibson::cell::Style;
use gibson::context::{Context, RenderMode};
use gibson::node::Node;
use gibson::TerminalSession;
use portable_pty::CommandBuilder;
use std::time::{Duration, Instant};

const CHILD: &str = "GIBSON_LEASE_AUTHORITY_CHILD";
const STALE_BEGIN: &[u8] = b"STALE_CALLS_BEGIN";
const STALE_END: &[u8] = b"STALE_CALLS_END";
const PAYLOAD: &str = "ZZPAYLOAD";

fn find(buf: &[u8], needle: &[u8]) -> Option<usize> {
    buf.windows(needle.len()).position(|w| w == needle)
}

fn run(role: &str) -> String {
    let mut cmd = CommandBuilder::new(std::env::current_exe().unwrap());
    cmd.args([
        "--exact",
        "lease_authority_child",
        "--nocapture",
        "--test-threads=1",
    ]);
    cmd.env(CHILD, role);
    let mut cap = pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_secs(10));
    cap.collect_until(|b| find(b, b"AUTH_DONE").is_some())
        .unwrap_or_else(|e| panic!("{role}: no completion marker: {e}"));
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        cap.collect_for(Duration::from_millis(20)).unwrap();
        if let Some(status) = cap.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "{role}: child did not exit");
    };
    cap.collect_all().unwrap();
    let raw = String::from_utf8_lossy(cap.raw()).into_owned();
    assert!(status.success(), "{role}: {raw}");
    raw
}

/// The owner was raw before restore, and after it the PTY is cooked and the lease is free.
fn assert_given_back(role: &str) {
    let raw = run(role);
    assert!(raw.contains("BEFORE RAW=1"), "{role}: never raw: {raw}");
    assert!(
        raw.contains("AFTER LEASE=Available COOKED=1"),
        "{role}: terminal or lease not given back: {raw}"
    );
}

#[test]
fn restore_gives_the_terminal_back_control() {
    assert_given_back("control");
}

#[test]
fn restore_ignores_a_cleared_is_tty_field_while_owning_the_lease() {
    assert_given_back("is-tty-false-restore");
}

#[test]
fn drop_ignores_a_cleared_is_tty_field_while_owning_the_lease() {
    assert_given_back("is-tty-false-drop");
}

#[test]
fn restored_context_cannot_render_or_commit_into_the_next_owners_terminal() {
    let raw = run("restored-context");
    let bytes = raw.as_bytes();
    let begin = find(bytes, STALE_BEGIN).expect("begin marker") + STALE_BEGIN.len();
    let end = find(bytes, STALE_END).expect("end marker");
    let stale = &bytes[begin..end];
    assert!(
        !stale.contains(&0x1b) && find(stale, PAYLOAD.as_bytes()).is_none(),
        "restored Context wrote into the next owner's terminal: {:?}",
        String::from_utf8_lossy(stale)
    );
    assert!(
        raw.contains("RENDER_ERR=1 COMMIT_ERR=1"),
        "stale calls must report failure: {raw}"
    );
}

fn cooked() -> bool {
    let mut t = std::mem::MaybeUninit::<libc::termios>::uninit();
    // SAFETY: stdin is this child's PTY; tcgetattr initializes `t` on success.
    assert_eq!(
        unsafe { libc::tcgetattr(libc::STDIN_FILENO, t.as_mut_ptr()) },
        0
    );
    let t = unsafe { t.assume_init() };
    t.c_lflag & (libc::ICANON | libc::ECHO) == (libc::ICANON | libc::ECHO)
}

#[test]
fn lease_authority_child() {
    let Ok(role) = std::env::var(CHILD) else {
        return;
    };
    if role == "restored-context" {
        let mut a = Context::new(RenderMode::Inline).unwrap();
        a.set_root(Node::text("OWNER A", Style::default()));
        a.render().unwrap();
        a.restore().unwrap();
        let mut b = TerminalSession::new().unwrap();
        a.set_root(Node::text(format!("{PAYLOAD} RENDER"), Style::default()));
        eprintln!("STALE_CALLS_BEGIN");
        let render_err = a.render_now().is_err();
        let commit_err = a.commit_text(&format!("{PAYLOAD} COMMIT")).is_err();
        eprintln!("STALE_CALLS_END");
        eprintln!(
            "RENDER_ERR={} COMMIT_ERR={}",
            u8::from(render_err),
            u8::from(commit_err)
        );
        b.restore().unwrap();
        eprintln!("AUTH_DONE");
        return;
    }
    let mut s = TerminalSession::new().unwrap();
    s.enter_interactive().unwrap();
    eprintln!("BEFORE RAW={}", u8::from(!cooked()));
    match role.as_str() {
        "control" => s.restore().unwrap(),
        "is-tty-false-restore" => {
            s.is_tty = false;
            s.restore().unwrap();
        }
        "is-tty-false-drop" => {
            s.is_tty = false;
            drop(s);
        }
        other => panic!("unknown role {other}"),
    }
    eprintln!(
        "AFTER LEASE={:?} COOKED={}",
        TerminalSession::lease_state(),
        u8::from(cooked())
    );
    eprintln!("AUTH_DONE");
}
