//! Managed Context output must never report a frame that its physical target lacks.
//! Derived from the full-grid counterexample at bfafd766; a fail-closed API is allowed.
#![cfg(target_os = "linux")]
#[path = "common/pty_capture.rs"]
mod pty_capture;
use gibson::{Context, Node, Style};
use portable_pty::CommandBuilder;
use std::{
    io::Write,
    time::{Duration, Instant},
};
const CHILD: &str = "GIBSON_CONTEXT_OUTPUT_IDENTITY_CHILD";
const START: &str = "\x1b]0;TARGET_START\x07";
const AFTER_A: &str = "\x1b]0;TARGET_AFTER_A\x07";
const AFTER_ROUTE: &str = "\x1b]0;TARGET_AFTER_ROUTE\x07";
const END: &str = "\x1b]0;TARGET_END\x07";
fn capture(role: &str) -> String {
    let mut cmd = CommandBuilder::new(std::env::current_exe().unwrap());
    cmd.args([
        "--exact",
        "context_output_child",
        "--nocapture",
        "--test-threads=1",
    ]);
    cmd.env(CHILD, role);
    let mut cap = pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_secs(10));
    cap.collect_until(|b| b.windows(11).any(|w| w == b"TARGET_DONE"))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        cap.collect_for(Duration::from_millis(20)).unwrap();
        if let Some(s) = cap.try_wait().unwrap() {
            break s;
        }
        assert!(Instant::now() < deadline, "child did not exit");
    };
    cap.collect_all().unwrap();
    assert!(status.success(), "{}", String::from_utf8_lossy(cap.raw()));
    String::from_utf8_lossy(cap.raw()).into_owned()
}
fn screens(output: &str) -> (vt100::Screen, vt100::Screen, vt100::Screen) {
    let start = output.find(START).unwrap() + START.len();
    let middle = output.find(AFTER_A).unwrap();
    let routed = output.find(AFTER_ROUTE).unwrap();
    let end = output.find(END).unwrap();
    assert!(start < middle && middle < end);
    let mut parser = vt100::Parser::new(24, 80, 0);
    parser.process(&output.as_bytes()[start..middle]);
    let initial = parser.screen().clone();
    parser.process(&output.as_bytes()[middle + AFTER_A.len()..routed]);
    let intermediate = parser.screen().clone();
    parser.process(&output.as_bytes()[routed + AFTER_ROUTE.len()..end]);
    (initial, intermediate, parser.screen().clone())
}
fn assert_screen(actual: &vt100::Screen, expected: &vt100::Screen, label: &str) {
    for row in 0..24 {
        for col in 0..80 {
            assert_eq!(
                actual.cell(row, col),
                expected.cell(row, col),
                "{label}: cell {row},{col}"
            );
        }
    }
    assert_eq!(
        actual.cursor_position(),
        expected.cursor_position(),
        "{label}: cursor"
    );
    assert_eq!(
        actual.hide_cursor(),
        expected.hide_cursor(),
        "{label}: visibility"
    );
}
fn verify_route(role: &str) {
    let control = capture("physical");
    let changed = capture(role);
    let (initial, intermediate, actual) = screens(&changed);
    let (control_initial, _, expected) = screens(&control);
    assert_screen(&initial, &control_initial, "initial physical frame");
    assert!(initial.contents().contains("FRAME_A"));
    assert!(expected.contents().contains("FRAME_B"));
    let field = |name: &str| -> &str {
        changed
            .split(name)
            .nth(1)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
    };
    let first_error = match field("FIRST_ERROR=") {
        "0" => false,
        "1" => true,
        x => panic!("bad result {x}"),
    };
    let final_error = match field("FINAL_ERROR=") {
        "0" => false,
        "1" => true,
        x => panic!("bad result {x}"),
    };
    let hex = field("PIPE_HEX=");
    let hex = if hex == "-" { "" } else { hex };
    assert_eq!(hex.len() % 2, 0);
    let bytes: Vec<_> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    let after_a = changed.find(AFTER_A).unwrap() + AFTER_A.len();
    let after_route = changed.find(AFTER_ROUTE).unwrap();
    let end = changed.find(END).unwrap();
    assert!(after_a <= after_route && after_route + AFTER_ROUTE.len() <= end);
    if first_error {
        assert_eq!(
            after_a, after_route,
            "refused first call wrote to physical target"
        );
    }
    if final_error {
        assert_eq!(
            after_route + AFTER_ROUTE.len(),
            end,
            "refused final call wrote to physical target"
        );
    }
    if first_error {
        assert!(
            bytes.is_empty(),
            "fail-closed target refusal must precede output"
        );
    } else if bytes.is_empty() {
        assert_screen(
            &intermediate,
            &expected,
            "successful first physical B request",
        );
    } else {
        let mut replay = vt100::Parser::new(24, 80, 0);
        let start = changed.find(START).unwrap() + START.len();
        let middle = changed.find(AFTER_A).unwrap();
        replay.process(&changed.as_bytes()[start..middle]);
        replay.process(&bytes);
        // A successful redirected stream is real signal; its complete B frame
        // is independently compared with a physical no-redirection execution.
        assert_screen(replay.screen(), &expected, "pipe replay positive control");
    }
    if !final_error {
        assert_screen(&actual, &expected, "successful physical B request");
    } else if !first_error && bytes.is_empty() {
        // A pinned target may already have presented B during the first call.
        assert_screen(&actual, &expected, "last successful pinned frame");
    } else {
        assert_screen(&actual, &initial, "refused physical frame unchanged");
    }
}
#[test]
fn redirected_stdout_cannot_create_false_physical_frame_agreement() {
    verify_route("redirect");
}
#[test]
fn oracle_accepts_pinned_and_refused_output_and_rejects_silent_success() {
    // These synthetic child controls validate the oracle, not the engine repair.
    verify_route("refused-control");
    verify_route("pinned-control");
    assert!(std::panic::catch_unwind(|| verify_route("dropped-control")).is_err());
}
#[test]
fn context_output_child() {
    let Ok(role) = std::env::var(CHILD) else {
        return;
    };
    use std::{
        io::Read,
        os::fd::{AsRawFd, FromRawFd},
    };
    assert!(matches!(
        role.as_str(),
        "physical" | "redirect" | "refused-control" | "pinned-control" | "dropped-control"
    ));
    let mut ctx = Context::fullscreen().unwrap();
    ctx.set_color_depth(gibson::capability::ColorDepth::TrueColor);
    let mut out = std::io::stdout();
    write!(out, "\x1b[2J\x1b[H{START}").unwrap();
    out.flush().unwrap();
    ctx.set_root(Node::text("FRAME_A", Style::default()));
    ctx.render_now().unwrap();
    out.write_all(AFTER_A.as_bytes()).unwrap();
    out.flush().unwrap();
    let mut bytes = Vec::new();
    let mut first_error = false;
    if role != "physical" {
        let saved = unsafe { libc::dup(1) };
        assert!(saved >= 0);
        let mut pipe = [0; 2];
        assert_eq!(unsafe { libc::pipe(pipe.as_mut_ptr()) }, 0);
        assert_eq!(unsafe { libc::dup2(pipe[1], 1) }, 1);
        unsafe {
            libc::close(pipe[1]);
        }
        // Retain an extra writer until AFTER the read, so no EOF is available.
        // A destination-aware implementation may retain its own writer too.
        let held_fd = unsafe { libc::dup(1) };
        assert!(held_fd >= 0);
        let held_writer = unsafe { std::fs::File::from_raw_fd(held_fd) };
        ctx.set_root(Node::text("FRAME_B", Style::default()));
        if role == "pinned-control" {
            assert_eq!(unsafe { libc::dup2(saved, 1) }, 1);
        }
        let result = match role.as_str() {
            "refused-control" => Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "synthetic refusal",
            )),
            "dropped-control" => Ok(gibson::painter::PaintContext::default()),
            _ => ctx.render_now(),
        };
        if role == "pinned-control" {
            assert_eq!(unsafe { libc::dup2(held_writer.as_raw_fd(), 1) }, 1);
        }
        if let Err(e) = result {
            assert!(
                matches!(
                    e.kind(),
                    std::io::ErrorKind::NotConnected | std::io::ErrorKind::InvalidData
                ),
                "{e}"
            );
            first_error = true;
        }
        assert_eq!(unsafe { libc::dup2(saved, 1) }, 1);
        unsafe {
            libc::close(saved);
        }
        let mut reader = unsafe { std::fs::File::from_raw_fd(pipe[0]) };
        let flags = unsafe { libc::fcntl(reader.as_raw_fd(), libc::F_GETFL) };
        assert!(flags >= 0);
        assert_eq!(
            unsafe { libc::fcntl(reader.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) },
            0
        );
        let mut buffer = [0; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    bytes.extend_from_slice(&buffer[..n]);
                    assert!(bytes.len() <= 1_048_576);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => panic!("pipe read: {e}"),
            }
        }
        drop(held_writer);
    } else {
        assert_eq!(role, "physical");
    }
    out.write_all(AFTER_ROUTE.as_bytes()).unwrap();
    out.flush().unwrap();
    ctx.set_root(Node::text("FRAME_B", Style::default()));
    let final_result = if role == "refused-control" {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "synthetic refusal",
        ))
    } else {
        ctx.render_now()
    };
    let final_error = match final_result {
        Ok(_) => false,
        Err(e) => {
            assert_ne!(role, "physical", "control must remain usable");
            assert!(
                matches!(
                    e.kind(),
                    std::io::ErrorKind::NotConnected | std::io::ErrorKind::InvalidData
                ),
                "{e}"
            );
            true
        }
    };
    out.write_all(END.as_bytes()).unwrap();
    out.flush().unwrap();
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    // A sentinel keeps the empty pipe record parseable without swallowing the next field.
    eprintln!(
        "FIRST_ERROR={} FINAL_ERROR={} PIPE_HEX={}",
        u8::from(first_error),
        u8::from(final_error),
        if hex.is_empty() { "-" } else { &hex }
    );
    ctx.restore().unwrap();
    eprintln!("TARGET_DONE");
}
