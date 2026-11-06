//! #259 — a headless `Context` has no terminal input source.
//!
//! `TerminalSession::headless` presents as a TTY so rendering behaves as on a terminal, and
//! before #259 `Context::poll_event` / `run_once` / `gibson_poll_event` gated only on that
//! flag: a headless context read the process's real terminal. With no terminal it failed
//! (`"Failed to initialize input reader"`); under a real PTY it **consumed the user's
//! keystrokes**. A headless context now takes input only through an injected source
//! (`Context::poll_from`), never from the terminal.
#![cfg(unix)]

use gibson::context::{Context, RenderMode};
use gibson::ffi::*;
use std::io::{Read, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const CHILD_ENV: &str = "GIBSON_259_PTY_CHILD";

#[test]
fn headless_poll_event_reports_no_input_at_once() {
    let mut ctx = Context::headless(RenderMode::Fullscreen, 20, 4);
    let t0 = Instant::now();
    assert_eq!(ctx.poll_event(Duration::from_millis(600)).unwrap(), None);
    // Before #259 this either failed (no terminal) or waited on the real terminal.
    assert!(
        t0.elapsed() < Duration::from_millis(400),
        "{:?}",
        t0.elapsed()
    );
}

#[test]
fn headless_run_once_takes_the_off_terminal_branch() {
    let mut ctx = Context::headless(RenderMode::Fullscreen, 20, 4);
    assert_eq!(ctx.run_once(Duration::from_millis(30)).unwrap(), None);
}

#[test]
fn the_c_abi_reports_no_event_for_a_headless_handle() {
    unsafe {
        let mut ctx: *mut GibsonContextOpaque = std::ptr::null_mut();
        assert_eq!(
            gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, 20, 4, &mut ctx),
            GibsonStatus::Ok
        );
        let mut ev = GibsonEvent {
            event_type: -1,
            key_code: 7,
            modifiers: 0,
            _pad: [0; 3],
            cols: 0,
            rows: 0,
            paste_str: std::ptr::null(),
        };
        let t0 = Instant::now();
        assert_eq!(gibson_poll_event(ctx, 600, &mut ev), GibsonStatus::Ok);
        assert!(t0.elapsed() < Duration::from_millis(400));
        assert_eq!(ev.event_type, GIBSON_EVENT_NONE);
        gibson_destroy_context(ctx);
    }
}

/// The decisive case: run a headless context inside a real PTY, type on that PTY, and require
/// that nothing typed reaches the headless context. This test re-runs its own binary as the
/// PTY child (`headless_pty_child`), selected by an environment variable.
#[test]
fn a_headless_context_never_consumes_real_terminal_keystrokes() {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("openpty");
    let mut cmd = CommandBuilder::new(std::env::current_exe().expect("test binary"));
    cmd.args([
        "--exact",
        "headless_pty_child",
        "--nocapture",
        "--test-threads=1",
    ]);
    cmd.env(CHILD_ENV, "1");
    let mut child = pair.slave.spawn_command(cmd).expect("spawn PTY child");
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().expect("reader");
    let mut writer = pair.master.take_writer().expect("writer");
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let mut out = String::new();
    let wait_for = |marker: &str, out: &mut String| {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !out.contains(marker) {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(
                !left.is_zero(),
                "PTY child never printed {marker:?}; output: {out:?}"
            );
            if let Ok(chunk) = rx.recv_timeout(left.min(Duration::from_millis(200))) {
                out.push_str(&String::from_utf8_lossy(&chunk));
            }
        }
    };
    wait_for("PROBE_READY", &mut out);
    writer.write_all(b"x\r").expect("type on the PTY");
    writer.flush().expect("flush");
    wait_for("PROBE_RESULT:", &mut out);
    wait_for("PROBE_END", &mut out);
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("PTY child did not exit; output: {out:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(status.success(), "child failed: {out:?}");
    let result = out
        .split("PROBE_RESULT:")
        .nth(1)
        .and_then(|r| r.split("PROBE_END").next())
        .unwrap_or_default()
        .trim()
        .to_string();
    assert_eq!(
        result, "[]",
        "keys typed on the real terminal reached a headless context"
    );
}

/// PTY child for the test above; a no-op in a normal run.
#[test]
fn headless_pty_child() {
    if std::env::var_os(CHILD_ENV).is_none() {
        return;
    }
    let mut ctx = Context::headless(RenderMode::Fullscreen, 20, 4);
    println!("PROBE_READY");
    std::io::stdout().flush().unwrap();
    let mut got = Vec::new();
    let until = Instant::now() + Duration::from_millis(1500);
    while Instant::now() < until {
        if let Some(e) = ctx.poll_event(Duration::from_millis(100)).unwrap() {
            got.push(e);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    println!("PROBE_RESULT: {got:?} PROBE_END");
    std::io::stdout().flush().unwrap();
}
