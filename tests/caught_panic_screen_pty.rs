//! Screen, cursor and renderer recovery after a caught owner-thread panic (#328 residual).
//!
//! The owner-thread panic hook leaves the alternate screen, shows the cursor and resets SGR. If
//! the panic is caught and the owner keeps rendering, the next frame must be drawn on the screen
//! the session is meant to own, with the cursor hidden, and in full. A diff against the frame
//! from before the panic is not enough. Input recovery after the same panic is
//! `caught_panic_input_pty.rs` (#329). Each case runs in its own PTY and process.
//!
//! Frame F1 is drawn, the panic is caught (or not, for the controls), and frame F2 is drawn. Then
//! the child writes a marker. The byte stream up to that marker is parsed with `vt100`, so the
//! assertions see the terminal exactly as F2 left it.
#![cfg(unix)]

#[path = "common/pty_capture.rs"]
mod pty_capture;

use gibson::cell::Style;
use gibson::context::{Context, RenderMode};
use gibson::node::Node;
use portable_pty::CommandBuilder;
use pty_capture::Capture;
use std::time::{Duration, Instant};

const CHILD: &str = "GIBSON_CAUGHT_PANIC_SCREEN_CHILD";
const MARKER: &[u8] = b"F2_DONE";

fn find(buf: &[u8], needle: &[u8]) -> Option<usize> {
    buf.windows(needle.len()).position(|part| part == needle)
}

fn frame(n: u32) -> Node {
    Node::col()
        .child(Node::text("TOP STATIC LINE", Style::default()))
        .child(Node::text(format!("COUNTER {n}"), Style::default()))
}

/// Runs one child role. Returns the terminal as parsed up to the marker, the terminal at exit,
/// and the raw stream (for messages).
fn run_case(role: &str) -> (vt100::Parser, vt100::Parser, Vec<u8>) {
    let mut cmd = CommandBuilder::new(std::env::current_exe().unwrap());
    cmd.args([
        "--exact",
        "caught_panic_screen_child",
        "--nocapture",
        "--test-threads=1",
    ]);
    cmd.env(CHILD, role);
    let mut cap = Capture::spawn(cmd, 80, 24, Duration::from_secs(15));
    cap.collect_until(|output| find(output, MARKER).is_some())
        .unwrap_or_else(|e| panic!("{role}: missing marker: {e}"));
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        cap.collect_for(Duration::from_millis(20)).unwrap();
        if let Some(status) = cap.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "{role}: child did not exit");
    };
    cap.collect_all().unwrap();
    let raw = cap.raw().to_vec();
    assert!(
        status.success(),
        "{role}: {}",
        String::from_utf8_lossy(&raw)
    );
    let at = find(&raw, MARKER).expect("marker");
    let mut at_marker = vt100::Parser::new(24, 80, 0);
    at_marker.process(&raw[..at]);
    let mut at_exit = vt100::Parser::new(24, 80, 0);
    at_exit.process(&raw);
    (at_marker, at_exit, raw)
}

/// F2 is on the alternate screen, the cursor is hidden, and the whole of F2 is visible.
fn assert_fullscreen_f2(role: &str) {
    let (at_marker, at_exit, raw) = run_case(role);
    let screen = at_marker.screen();
    let lossy = String::from_utf8_lossy(&raw);
    assert!(
        screen.alternate_screen(),
        "{role}: F2 was not drawn on the alternate screen: {lossy:?}"
    );
    assert!(
        screen.hide_cursor(),
        "{role}: cursor visible after F2: {lossy:?}"
    );
    let contents = screen.contents();
    assert!(
        contents.contains("TOP STATIC LINE")
            && contents.contains("COUNTER 2")
            && !contents.contains("COUNTER 1"),
        "{role}: F2 not drawn in full; screen: {contents:?}"
    );
    // Restore still runs: back on the main screen, cursor shown.
    assert!(!at_exit.screen().alternate_screen(), "{role}: left on alt");
    assert!(
        !at_exit.screen().hide_cursor(),
        "{role}: cursor left hidden"
    );
}

/// Inline: F2 is visible in full and the cursor is hidden.
fn assert_inline_f2(role: &str) {
    let (at_marker, at_exit, raw) = run_case(role);
    let screen = at_marker.screen();
    let lossy = String::from_utf8_lossy(&raw);
    assert!(
        screen.hide_cursor(),
        "{role}: cursor visible after F2: {lossy:?}"
    );
    let contents = screen.contents();
    let lines: Vec<&str> = contents.lines().filter(|l| !l.trim().is_empty()).collect();
    // The live region starts on the harness's own "test ... " line and overwrites it from column
    // 0 without erasing the rest (autowrap off), so that row starts with F2's first line.
    let n = lines.len();
    assert!(
        n >= 2
            && lines[n - 2].starts_with("TOP STATIC LINE")
            && lines[n - 1].trim_end() == "COUNTER 2",
        "{role}: F2 not the live region; screen: {contents:?}"
    );
    assert!(
        !at_exit.screen().hide_cursor(),
        "{role}: cursor left hidden"
    );
}

#[test]
fn fullscreen_no_panic_control() {
    assert_fullscreen_f2("control");
}

#[test]
fn fullscreen_non_owner_panic_control() {
    assert_fullscreen_f2("worker");
}

#[test]
fn fullscreen_caught_owner_panic_redraws_f2_on_the_alternate_screen() {
    assert_fullscreen_f2("owner");
}

#[test]
fn inline_no_panic_control() {
    assert_inline_f2("inline-control");
}

#[test]
fn inline_caught_owner_panic_redraws_f2_with_the_cursor_hidden() {
    assert_inline_f2("inline-owner");
}

#[test]
fn caught_panic_screen_child() {
    let Ok(role) = std::env::var(CHILD) else {
        return;
    };
    // Silence the panic message: the library hook chains to the hook installed before the first
    // session, and a message on the main screen would only add noise to the captures.
    std::panic::set_hook(Box::new(|_| {}));
    let mode = if role.starts_with("inline") {
        RenderMode::Inline
    } else {
        RenderMode::Fullscreen
    };
    let mut ctx = Context::new(mode).unwrap();
    ctx.set_root(frame(1));
    ctx.render().unwrap();
    match role.trim_start_matches("inline-") {
        "owner" => assert!(std::panic::catch_unwind(|| panic!("owner panic (caught)")).is_err()),
        "worker" => assert!(std::thread::spawn(|| panic!("worker panic (caught)"))
            .join()
            .is_err()),
        _ => {}
    }
    ctx.set_root(frame(2));
    ctx.render().unwrap();
    eprintln!("F2_DONE");
    ctx.restore().unwrap();
}
