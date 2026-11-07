//! #259 on the 0.6 line: a headless `Context` has no terminal input source.
//!
//! `TerminalSession::headless` presents as a TTY so rendering behaves as on a terminal. Before
//! this fix `Context::poll_event`, `Context::run_once` and `gibson_poll_event` gated only on that
//! flag, so a headless context read the process's real terminal: with no terminal it failed, and
//! under a real PTY it **consumed the user's keystrokes**. The release contract calls the
//! headless C ABI output-only, so a headless context now reports no input, at once, and leaves
//! the terminal alone.
//!
//! The PTY cases prove both halves on a real terminal. A headless context polls through one
//! entry point while a key is typed on the PTY: it must report nothing. Then a real
//! (terminal-backed) context in the same process polls: it must receive that same key, which
//! shows the headless context neither read nor consumed it, and that real contexts still get
//! input.
#![cfg(unix)]

use gibson::context::{Context, RenderMode};
use gibson::ffi::*;
use gibson::{Event, KeyCode};
use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Selects the child role when this test binary re-runs itself (`pty:<leg>` or `notty`).
const CHILD_ENV: &str = "GIBSON_259_CHILD";

fn new_event() -> GibsonEvent {
    GibsonEvent {
        event_type: -1,
        key_code: 7,
        modifiers: 0,
        _pad: [0; 3],
        cols: 0,
        rows: 0,
        paste_str: std::ptr::null(),
    }
}

#[test]
fn headless_poll_event_reports_no_input_at_once() {
    let mut ctx = Context::headless(RenderMode::Fullscreen, 20, 4);
    let t0 = Instant::now();
    assert_eq!(ctx.poll_event(Duration::from_millis(600)).unwrap(), None);
    // Unfixed, this either failed (no terminal) or waited on the real terminal.
    assert!(
        t0.elapsed() < Duration::from_millis(400),
        "{:?}",
        t0.elapsed()
    );
}

#[test]
fn headless_run_once_takes_the_off_terminal_branch() {
    // The off-terminal branch sleeps for the wait, then renders if due: a due frame must be
    // rendered (observable bytes), which an implementation that merely returned Ok(None) does
    // not do.
    let mut ctx = Context::headless(RenderMode::Fullscreen, 20, 4);
    ctx.set_root(gibson::node::Node::text(
        "run_once",
        gibson::cell::Style::default(),
    ));
    let mut rendered = false;
    for _ in 0..10 {
        assert_eq!(ctx.run_once(Duration::from_millis(30)).unwrap(), None);
        if ctx.take_output().contains("run_once") {
            rendered = true;
            break;
        }
    }
    assert!(
        rendered,
        "a due frame is rendered by run_once on a headless context"
    );
}

/// Once its frame is presented and nothing is pending, a headless `run_once` waits out its
/// `max_wait` (#264) on the off-terminal branch. A `run_once` that instead delegated to the
/// guarded `poll_event` would return at once and spin an idle loop.
#[test]
fn a_headless_idle_run_once_waits_instead_of_spinning() {
    let mut ctx = Context::headless(RenderMode::Fullscreen, 20, 4);
    ctx.set_root(gibson::node::Node::text(
        "idle",
        gibson::cell::Style::default(),
    ));
    let mut rendered = false;
    for _ in 0..10 {
        assert_eq!(ctx.run_once(Duration::from_millis(30)).unwrap(), None);
        if ctx.take_output().contains("idle") {
            rendered = true;
            break;
        }
    }
    assert!(rendered, "the first frame renders");
    let t0 = Instant::now();
    let mut calls = 0u32;
    while t0.elapsed() < Duration::from_millis(600) {
        assert_eq!(ctx.run_once(Duration::from_millis(200)).unwrap(), None);
        calls += 1;
    }
    // About 3 calls of 200 ms each; a spinning loop makes thousands.
    assert!(
        calls <= 6,
        "{calls} idle run_once calls in 600 ms: it spins"
    );
}

#[test]
fn the_c_abi_reports_no_event_for_a_headless_handle() {
    unsafe {
        let mut ctx: *mut GibsonContextOpaque = std::ptr::null_mut();
        assert_eq!(
            gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, 20, 4, &mut ctx),
            GibsonStatus::Ok
        );
        let mut ev = new_event();
        let t0 = Instant::now();
        assert_eq!(gibson_poll_event(ctx, 600, &mut ev), GibsonStatus::Ok);
        assert!(t0.elapsed() < Duration::from_millis(400));
        assert_eq!(ev.event_type, GIBSON_EVENT_NONE);
        gibson_destroy_context(ctx);
    }
}

/// With no terminal at all (stdin `/dev/null`, stdout a pipe, no controlling terminal), every
/// headless entry point reports no input, promptly and without an error. Unfixed, the poll
/// tried to open a terminal and failed.
#[test]
fn a_headless_context_without_any_terminal_reports_no_input() {
    let mut cmd = Command::new(std::env::current_exe().expect("test binary"));
    cmd.args([
        "--exact",
        "headless_child",
        "--nocapture",
        "--test-threads=1",
    ])
    .env(CHILD_ENV, "notty")
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    // SAFETY: setsid is async-signal-safe; it detaches the child from any controlling terminal,
    // so `/dev/tty` cannot be opened.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let out = cmd.output().expect("run the no-terminal child");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "child failed: {stdout} {}",
        String::from_utf8_lossy(&out.stderr)
    );
    for leg in ["rust_poll", "run_once", "c_abi"] {
        assert!(
            stdout.contains(&format!("NOTTY {leg} OK")),
            "{leg} did not report Ok(None) promptly: {stdout}"
        );
    }
}

fn run_pty_leg(leg: &str) {
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
        "headless_child",
        "--nocapture",
        "--test-threads=1",
    ]);
    cmd.env(CHILD_ENV, format!("pty:{leg}"));
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
                "[{leg}] PTY child never printed {marker:?}; output: {out:?}"
            );
            if let Ok(chunk) = rx.recv_timeout(left.min(Duration::from_millis(200))) {
                out.push_str(&String::from_utf8_lossy(&chunk));
            }
        }
    };
    wait_for("PROBE_READY", &mut out);
    writer.write_all(b"x\r").expect("type on the PTY");
    writer.flush().expect("flush");
    wait_for("PROBE_END", &mut out);
    wait_for("POSITIVE_END", &mut out);
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("[{leg}] PTY child did not exit; output: {out:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let between = |a: &str, b: &str| {
        out.split(a)
            .nth(1)
            .and_then(|r| r.split(b).next())
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    let headless = between("PROBE_RESULT:", "PROBE_END");
    let positive = between("POSITIVE:", "POSITIVE_END");
    assert!(status.success(), "[{leg}] child failed: {out:?}");
    assert_eq!(
        headless, "[]",
        "[{leg}] a key typed on the real terminal reached a headless context"
    );
    assert_eq!(
        positive, "x",
        "[{leg}] the real context did not receive the key typed during the headless poll \
         (the headless context consumed it, or real input is broken); output: {out:?}"
    );
}

#[test]
fn a_headless_rust_poll_never_consumes_real_terminal_keystrokes() {
    run_pty_leg("rust_poll");
}

#[test]
fn a_headless_run_once_never_consumes_real_terminal_keystrokes() {
    run_pty_leg("run_once");
}

#[test]
fn a_headless_c_abi_poll_never_consumes_real_terminal_keystrokes() {
    run_pty_leg("c_abi");
}

fn key_char(e: &Event) -> Option<char> {
    match e {
        Event::Key(k) => match k.code {
            KeyCode::Char(c) => Some(c),
            _ => None,
        },
        _ => None,
    }
}

/// Child for the tests above; a no-op in a normal run.
#[test]
fn headless_child() {
    let Some(role) = std::env::var_os(CHILD_ENV) else {
        return;
    };
    let role = role.to_string_lossy().into_owned();
    if role == "notty" {
        notty_child();
    } else if let Some(leg) = role.strip_prefix("pty:") {
        pty_child(leg);
    } else {
        panic!("unknown child role {role}");
    }
}

fn notty_child() {
    let prompt = |t0: Instant| t0.elapsed() < Duration::from_millis(400);
    let mut ctx = Context::headless(RenderMode::Fullscreen, 20, 4);
    let t0 = Instant::now();
    let r = ctx.poll_event(Duration::from_millis(600));
    if matches!(r, Ok(None)) && prompt(t0) {
        println!("NOTTY rust_poll OK");
    } else {
        println!("NOTTY rust_poll BAD {r:?} {:?}", t0.elapsed());
    }
    let r = ctx.run_once(Duration::from_millis(50));
    if matches!(r, Ok(None)) {
        println!("NOTTY run_once OK");
    } else {
        println!("NOTTY run_once BAD {r:?}");
    }
    unsafe {
        let mut h: *mut GibsonContextOpaque = std::ptr::null_mut();
        assert_eq!(
            gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, 20, 4, &mut h),
            GibsonStatus::Ok
        );
        let mut ev = new_event();
        let t0 = Instant::now();
        let st = gibson_poll_event(h, 600, &mut ev);
        if st == GibsonStatus::Ok && ev.event_type == GIBSON_EVENT_NONE && prompt(t0) {
            println!("NOTTY c_abi OK");
        } else {
            println!(
                "NOTTY c_abi BAD {st:?} type={} {:?}",
                ev.event_type,
                t0.elapsed()
            );
        }
        gibson_destroy_context(h);
    }
    std::io::stdout().flush().unwrap();
}

fn pty_child(leg: &str) {
    // Phase 1: the headless context polls for 1.5 s through the selected entry point while the
    // parent types `x` + Enter on the PTY.
    let mut got: Vec<String> = Vec::new();
    let until = Instant::now() + Duration::from_millis(1500);
    match leg {
        "rust_poll" | "run_once" => {
            let mut ctx = Context::headless(RenderMode::Fullscreen, 20, 4);
            println!("PROBE_READY");
            std::io::stdout().flush().unwrap();
            while Instant::now() < until {
                let e = if leg == "rust_poll" {
                    ctx.poll_event(Duration::from_millis(100)).unwrap()
                } else {
                    ctx.run_once(Duration::from_millis(100)).unwrap()
                };
                if let Some(e) = e {
                    got.push(format!("{e:?}"));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        "c_abi" => unsafe {
            let mut h: *mut GibsonContextOpaque = std::ptr::null_mut();
            assert_eq!(
                gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, 20, 4, &mut h),
                GibsonStatus::Ok
            );
            println!("PROBE_READY");
            std::io::stdout().flush().unwrap();
            while Instant::now() < until {
                let mut ev = new_event();
                let st = gibson_poll_event(h, 100, &mut ev);
                if st != GibsonStatus::Ok || ev.event_type != GIBSON_EVENT_NONE {
                    got.push(format!("{st:?}/{}/{}", ev.event_type, ev.key_code));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            gibson_destroy_context(h);
        },
        other => panic!("unknown leg {other}"),
    }
    println!("PROBE_RESULT: [{}] PROBE_END", got.join(", "));
    std::io::stdout().flush().unwrap();

    // Phase 2 (positive control): a real terminal-backed context, through the same family of
    // entry point, must now receive the `x` typed during phase 1.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut first: Option<char> = None;
    if leg == "c_abi" {
        unsafe {
            let mut h: *mut GibsonContextOpaque = std::ptr::null_mut();
            assert_eq!(
                gibson_create_context(GIBSON_MODE_INLINE, &mut h),
                GibsonStatus::Ok
            );
            while Instant::now() < deadline && first.is_none() {
                let mut ev = new_event();
                if gibson_poll_event(h, 100, &mut ev) == GibsonStatus::Ok
                    && ev.event_type == GIBSON_EVENT_KEY
                {
                    first = char::from_u32(ev.key_code);
                }
            }
            gibson_destroy_context(h);
        }
    } else {
        let mut real = Context::new(RenderMode::Inline).expect("real context");
        assert!(real.is_interactive(), "the PTY child has a real terminal");
        while Instant::now() < deadline && first.is_none() {
            let e = if leg == "rust_poll" {
                real.poll_event(Duration::from_millis(100)).unwrap()
            } else {
                real.run_once(Duration::from_millis(100)).unwrap()
            };
            first = e.as_ref().and_then(key_char);
        }
        real.restore().unwrap();
    }
    let shown = first.map(String::from).unwrap_or_else(|| "NONE".into());
    println!("POSITIVE: {shown} POSITIVE_END");
    std::io::stdout().flush().unwrap();
}
