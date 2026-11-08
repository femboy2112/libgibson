//! #296: `Context::enter_interactive` makes "the screen says READY" imply "the terminal is
//! interactive".
//!
//! `Context::inline()` used to leave raw mode and bracketed paste to the first `poll_event`. A
//! consumer that printed a READY line before that poll showed it on a cooked tty, so the kernel
//! echoed type-ahead and ICRNL turned Enter (`\r`) into `\n`, which raw-mode decoding reads as
//! Ctrl+J: the line was never submitted.
//!
//! The probe: a child (this test binary, selected by `GIBSON_296_CHILD`) runs on a real PTY,
//! creates `Context::inline()`, optionally calls `enter_interactive()`, reports its termios and
//! prints READY. The parent waits for READY, writes `hello\r` in one write, then raises a flag
//! file. The child holds its first poll until the flag is up and the bytes have been through the
//! line discipline (`FIONREAD` reports them), so the old behaviour is a deterministic failure,
//! not a race; no step sleeps for correctness. The child then polls until Enter (or a deadline
//! that only bounds a failure), prints each event and the completed line, and restores.
#![cfg(unix)]
#[path = "common/pty_capture.rs"]
mod pty_capture;

use gibson::context::{Context, RenderMode};
use gibson::{Event, KeyCode, KeyModifiers};
use portable_pty::CommandBuilder;
use pty_capture::Capture;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const CHILD_ENV: &str = "GIBSON_296_CHILD";
/// Flag file the parent creates once its write to the master has returned.
const FLAG_ENV: &str = "GIBSON_296_FLAG";
/// Result file for the disconnect child (its terminal is gone, so it cannot print).
const OUT_ENV: &str = "GIBSON_296_OUT";

/// Bracketed paste on (`CSI ? 2004 h`), emitted by the interactive entry.
const PASTE_ON: &[u8] = b"\x1b[?2004h";

fn child(role: &str) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(std::env::current_exe().expect("test binary"));
    cmd.args([
        "--exact",
        "enter_interactive_child",
        "--nocapture",
        "--test-threads=1",
    ]);
    cmd.env(CHILD_ENV, role);
    cmd.env("TERM", "xterm-256color");
    cmd
}

fn temp(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("gibson_296_{tag}_{}", std::process::id()))
}

fn wait_marker(cap: &mut Capture, marker: &str) {
    let m = marker.to_string();
    cap.collect_until(move |o| String::from_utf8_lossy(o).contains(&m))
        .unwrap_or_else(|e| panic!("child never printed {marker:?}: {e}"));
}

fn wait_exit(cap: &mut Capture, what: &str) -> portable_pty::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let _ = cap.collect_for(Duration::from_millis(20));
        if let Some(s) = cap.try_wait().unwrap() {
            // The child can write its last bytes (RESTORED, panic cleanup) after the collection
            // above stopped and before it exited. Read on until EOF, which follows at once now
            // that it has exited, or for at most 2 s.
            let _ = cap.collect_for(Duration::from_secs(2));
            return s;
        }
        assert!(Instant::now() < deadline, "[{what}] child did not exit");
    }
}

/// What one probe run showed.
#[derive(Debug)]
struct Probe {
    /// Whole PTY output, as text.
    out: String,
    /// `TERMIOS` line: (ICANON, ECHO) after the child's setup, before READY.
    cooked_at_ready: (bool, bool),
    /// The `EV` lines, in order. A marker can share a line with libtest's `test … ` prefix or
    /// an escape sequence, so markers are found anywhere in a line.
    events: Vec<String>,
    /// The child's own comparison of its final termios with the one it started with.
    restored: bool,
    /// Bracketed-paste enables written over the whole run.
    paste_enables: usize,
}

fn count(hay: &[u8], needle: &[u8]) -> usize {
    hay.windows(needle.len()).filter(|w| *w == needle).count()
}

fn run_probe(role: &str) -> Probe {
    let flag = temp(&format!("{role}_flag"));
    let _ = std::fs::remove_file(&flag);
    let mut cmd = child(role);
    cmd.env(FLAG_ENV, flag.to_str().unwrap());
    let mut cap = Capture::spawn(cmd, 80, 24, Duration::from_secs(30));
    wait_marker(&mut cap, "READY");
    // One write, only after READY, and no sleep: the flag tells the child the write returned.
    cap.write(b"hello\r").expect("write type-ahead");
    std::fs::write(&flag, b"1").expect("raise flag");
    let status = wait_exit(&mut cap, role);
    let _ = std::fs::remove_file(&flag);
    let raw = cap.raw().to_vec();
    let out = String::from_utf8_lossy(&raw).into_owned();
    assert!(status.success(), "[{role}] child failed: {out:?}");
    let line = |p: &str| {
        out.split(['\r', '\n'])
            .find_map(|l| l.split_once(p).map(|(_, rest)| rest.trim().to_string()))
            .unwrap_or_else(|| panic!("[{role}] no {p:?} line: {out:?}"))
    };
    let termios = line("TERMIOS ");
    let flag_on = |k: &str| termios.split(' ').any(|f| f == format!("{k}=1"));
    Probe {
        cooked_at_ready: (flag_on("icanon"), flag_on("echo")),
        events: out
            .split(['\r', '\n'])
            .filter_map(|l| l.split_once("EV ").map(|(_, rest)| rest.trim().to_string()))
            .collect(),
        restored: line("RESTORED ") == "same",
        paste_enables: count(&raw, PASTE_ON),
        out,
    }
}

const WANT_EVENTS: [&str; 6] = ["h", "e", "l", "l", "o", "Enter"];

/// The #296 probe's verdict: every way the type-ahead can go wrong, or `Ok`.
fn verdict(p: &Probe) -> Result<(), Vec<String>> {
    let mut why = Vec::new();
    if p.cooked_at_ready.0 || p.cooked_at_ready.1 {
        why.push(format!(
            "tty not raw at READY (icanon, echo) = {:?}",
            p.cooked_at_ready
        ));
    }
    // The child renders the typed line only as `LINE hello`; any `hello` before it is the
    // kernel's echo.
    let rendered = p.out.find("LINE hello").map(|i| i + "LINE ".len());
    match (p.out.find("hello"), rendered) {
        (Some(first), Some(r)) if first == r => {}
        (Some(first), r) => why.push(format!(
            "`hello` appeared at byte {first} before the child rendered it (render at {r:?}): \
             kernel echo"
        )),
        (None, _) => why.push("`hello` never appeared".into()),
    }
    if p.events != WANT_EVENTS {
        why.push(format!("events {:?}, want {WANT_EVENTS:?}", p.events));
    }
    if why.is_empty() {
        Ok(())
    } else {
        Err(why)
    }
}

/// GREEN: with `enter_interactive()` before READY, type-ahead at READY is raw input: no echo,
/// five characters and exactly one Enter, the termios is restored at a normal quit, and
/// bracketed paste was enabled exactly once across the call and every later poll.
#[test]
fn type_ahead_at_ready_is_raw_after_enter_interactive() {
    let p = run_probe("eager");
    eprintln!(
        "GREEN probe: events={:?} cooked_at_ready={:?}",
        p.events, p.cooked_at_ready
    );
    assert_eq!(verdict(&p), Ok(()), "probe output: {:?}", p.out);
    assert!(
        p.restored,
        "a normal quit must restore the starting termios: {:?}",
        p.out
    );
    assert_eq!(
        p.paste_enables, 1,
        "bracketed paste enabled more than once: {:?}",
        p.out
    );
}

/// Calling it twice changes nothing: same result, one bracketed-paste enable.
#[test]
fn enter_interactive_twice_is_harmless() {
    let p = run_probe("eager-twice");
    assert_eq!(verdict(&p), Ok(()), "probe output: {:?}", p.out);
    assert!(p.restored, "termios not restored: {:?}", p.out);
    assert_eq!(
        p.paste_enables, 1,
        "bracketed paste enabled twice: {:?}",
        p.out
    );
}

/// RED control: the same probe without `enter_interactive()` (READY printed before the first
/// poll, the behaviour before #296) fails, deterministically and for the reported reasons.
#[test]
fn the_probe_fails_without_enter_interactive() {
    let p = run_probe("lazy");
    let why = verdict(&p).expect_err("the probe must fail when raw mode waits for the first poll");
    eprintln!("RED probe: events={:?} why={why:#?}", p.events);
    assert_eq!(
        p.cooked_at_ready,
        (true, true),
        "cooked at READY: {:?}",
        p.out
    );
    assert!(
        why.iter().any(|w| w.contains("kernel echo")),
        "no kernel echo seen: {why:?}"
    );
    assert_eq!(
        p.events,
        ["h", "e", "l", "l", "o", "Ctrl+j"],
        "ICRNL should have turned Enter into Ctrl+J"
    );
    assert!(p.restored, "termios not restored: {:?}", p.out);
}

/// A disconnect (the master closed) after `enter_interactive` makes the next poll fail instead
/// of spinning on a dead descriptor.
#[test]
fn a_disconnect_after_enter_interactive_errors_without_spinning() {
    let out = temp("hangup_out");
    let _ = std::fs::remove_file(&out);
    let mut cmd = child("hangup");
    cmd.env(OUT_ENV, out.to_str().unwrap());
    let mut cap = Capture::spawn(cmd, 80, 24, Duration::from_secs(30));
    wait_marker(&mut cap, "READY");
    cap.hang_up();
    let _ = wait_exit(&mut cap, "hangup");
    let got = std::fs::read_to_string(&out).expect("child result");
    let _ = std::fs::remove_file(&out);
    let fields: std::collections::BTreeMap<&str, &str> = got
        .split_whitespace()
        .filter_map(|kv| kv.split_once('='))
        .collect();
    assert_eq!(fields.get("result"), Some(&"err"), "{got}");
    // Counted only from the first poll that starts after the child itself sees the hangup on
    // its terminal, so healthy polls before the master closed (however late the parent closes
    // it) don't count. A spinning poll would return Ok(None) at once, many times, after it.
    let after: u32 = fields["after_hangup"].parse().unwrap();
    assert!(
        after <= 10,
        "{after} polls after the hangup before the disconnect surfaced: {got}"
    );
}

/// After an explicit `restore()` the context has handed the terminal back (and released the
/// lease): `enter_interactive` refuses instead of re-arming raw mode that nothing would undo.
#[test]
fn enter_interactive_after_restore_refuses_and_leaves_the_tty_cooked() {
    let mut cap = Capture::spawn(child("restored"), 80, 24, Duration::from_secs(30));
    let status = wait_exit(&mut cap, "restored");
    let out = String::from_utf8_lossy(cap.raw()).into_owned();
    assert!(status.success(), "child failed: {out:?}");
    assert!(
        out.contains("AFTER_RESTORE err icanon=1 echo=1"),
        "enter_interactive after restore must refuse and leave the tty cooked: {out:?}"
    );
}

/// A panic after `enter_interactive` still restores the terminal (the panic hook and Drop own
/// what it enabled): cooked again, bracketed paste disabled.
#[test]
fn a_panic_after_enter_interactive_restores_the_terminal() {
    let mut cap = Capture::spawn(child("panic"), 80, 24, Duration::from_secs(30));
    wait_marker(&mut cap, "ARMED");
    let status = wait_exit(&mut cap, "panic");
    let raw = cap.raw().to_vec();
    let out = String::from_utf8_lossy(&raw).into_owned();
    assert!(!status.success(), "the child was meant to panic: {out:?}");
    let lflag = cap.local_flags().expect("tcgetattr on the master") as libc::tcflag_t;
    assert!(
        lflag & libc::ICANON != 0 && lflag & libc::ECHO != 0,
        "termios still raw after the panic (c_lflag {lflag:#o}): {out:?}"
    );
    let on = raw.windows(PASTE_ON.len()).rposition(|w| w == PASTE_ON);
    let off = raw.windows(8).rposition(|w| w == b"\x1b[?2004l");
    assert!(
        matches!((on, off), (Some(a), Some(b)) if b > a),
        "bracketed paste not disabled after the panic: {out:?}"
    );
}

/// Headless and non-TTY contexts: a no-op that returns Ok and writes nothing.
#[test]
fn headless_enter_interactive_is_a_noop() {
    let mut ctx = Context::headless(RenderMode::Inline, 20, 4);
    ctx.enter_interactive().expect("headless: Ok");
    ctx.enter_interactive().expect("headless twice: Ok");
    assert!(ctx.take_output().is_empty(), "headless wrote bytes");
    assert_eq!(ctx.poll_event(Duration::ZERO).unwrap(), None);
}

#[test]
fn non_tty_enter_interactive_is_a_noop() {
    // Stdout is a pipe in this child, so the context is not a TTY.
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "enter_interactive_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ENV, "nontty")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("run non-tty child");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "non-tty child failed: {out:?}");
    assert!(text.contains("NONTTY ok"), "{text:?}");
    assert_eq!(
        count(&out.stdout, PASTE_ON),
        0,
        "non-TTY wrote paste enable: {text:?}"
    );
}

// ------------------------------------------------------------------------------- child side

fn say(s: &str) {
    // Raw mode may be on: end lines explicitly.
    print!("{s}\r\n");
    std::io::stdout().flush().unwrap();
}

fn tcgetattr0() -> libc::termios {
    // SAFETY: a zeroed termios is a valid out-buffer; fd 0 is the PTY slave in the child.
    unsafe {
        let mut t: libc::termios = std::mem::zeroed();
        assert_eq!(libc::tcgetattr(0, &mut t), 0, "tcgetattr");
        t
    }
}

fn same_termios(a: &libc::termios, b: &libc::termios) -> bool {
    a.c_iflag == b.c_iflag
        && a.c_oflag == b.c_oflag
        && a.c_cflag == b.c_cflag
        && a.c_lflag == b.c_lflag
        && a.c_cc == b.c_cc
}

fn desc(e: &Event) -> String {
    match e {
        Event::Key(k) if k.code == KeyCode::Enter => "Enter".into(),
        Event::Key(k) => match k.code {
            KeyCode::Char(c) if k.modifiers.contains(KeyModifiers::CONTROL) => format!("Ctrl+{c}"),
            KeyCode::Char(c) => c.to_string(),
            other => format!("{other:?}"),
        },
        other => format!("{other:?}"),
    }
}

/// Bytes queued on stdin (the PTY slave), past the line discipline.
fn stdin_queued() -> i32 {
    let mut n: libc::c_int = 0;
    // SAFETY: FIONREAD writes one c_int for a valid descriptor.
    let rc = unsafe { libc::ioctl(0, libc::FIONREAD, &mut n) };
    assert_eq!(rc, 0, "FIONREAD");
    n
}

/// Child for the tests above; a no-op in a normal run.
#[test]
fn enter_interactive_child() {
    let Some(role) = std::env::var_os(CHILD_ENV) else {
        return;
    };
    match role.to_string_lossy().as_ref() {
        "eager" => probe_child(1),
        "eager-twice" => probe_child(2),
        "lazy" => probe_child(0),
        "hangup" => hangup_child(),
        "restored" => {
            let mut ctx = Context::inline().expect("terminal context");
            ctx.enter_interactive().expect("enter_interactive");
            ctx.restore().expect("restore");
            let r = ctx.enter_interactive();
            let t = tcgetattr0();
            say(&format!(
                "AFTER_RESTORE {} icanon={} echo={}",
                if r.is_err() { "err" } else { "ok" },
                u8::from(t.c_lflag & libc::ICANON != 0),
                u8::from(t.c_lflag & libc::ECHO != 0),
            ));
        }
        "panic" => {
            let mut ctx = Context::inline().expect("terminal context");
            ctx.enter_interactive().expect("enter_interactive");
            say("ARMED");
            panic!("deliberate panic with the terminal interactive");
        }
        "nontty" => {
            let mut ctx = Context::inline().expect("context");
            ctx.enter_interactive().expect("non-tty: Ok");
            ctx.enter_interactive().expect("non-tty twice: Ok");
            assert_eq!(ctx.poll_event(Duration::ZERO).unwrap(), None);
            drop(ctx);
            println!("NONTTY ok");
        }
        other => panic!("unknown role {other}"),
    }
}

fn probe_child(enter_calls: usize) {
    let before = tcgetattr0();
    let mut ctx = Context::inline().expect("terminal context");
    for _ in 0..enter_calls {
        ctx.enter_interactive().expect("enter_interactive");
    }
    let t = tcgetattr0();
    let bit = |f: libc::tcflag_t, m: libc::tcflag_t| u8::from(f & m != 0);
    say(&format!(
        "TERMIOS icanon={} echo={} icrnl={}",
        bit(t.c_lflag, libc::ICANON),
        bit(t.c_lflag, libc::ECHO),
        bit(t.c_iflag, libc::ICRNL),
    ));
    say("READY");

    // Hold the first poll until the parent's write has returned (the flag) and the line
    // discipline has processed the bytes (FIONREAD sees them). Waiting on a condition, bounded.
    let flag = PathBuf::from(std::env::var(FLAG_ENV).expect("flag path"));
    let cap = Instant::now() + Duration::from_secs(10);
    while !flag.exists() || stdin_queued() < 6 {
        assert!(Instant::now() < cap, "type-ahead never arrived");
        std::thread::sleep(Duration::from_millis(5));
    }

    let mut line = String::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        let Some(e) = ctx.poll_event(Duration::from_millis(100)).unwrap() else {
            continue;
        };
        let d = desc(&e);
        say(&format!("EV {d}"));
        match e {
            Event::Key(k) if k.code == KeyCode::Enter => {
                // Anything still queued behind the Enter would be a second event.
                if let Some(extra) = ctx.poll_event(Duration::from_millis(100)).unwrap() {
                    say(&format!("EV {}", desc(&extra)));
                }
                break;
            }
            Event::Key(k) if k.modifiers.is_empty() => {
                if let KeyCode::Char(c) = k.code {
                    line.push(c);
                }
            }
            _ => {}
        }
    }
    say(&format!("LINE {line}"));
    drop(ctx);
    let after = tcgetattr0();
    say(&format!(
        "RESTORED {}",
        if same_termios(&before, &after) {
            "same"
        } else {
            "differs"
        }
    ));
}

fn hangup_child() {
    // Survive the hangup (as an application that ignores SIGHUP does) and observe the poll.
    // SAFETY: setting a signal disposition to SIG_IGN is always sound.
    unsafe {
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
    }
    let out = std::env::var(OUT_ENV).expect("result path");
    let mut ctx = Context::inline().expect("terminal context");
    ctx.enter_interactive().expect("enter_interactive");
    say("READY");
    let mut polls = 0u32;
    let mut after_hangup = 0u32;
    let deadline = Instant::now() + Duration::from_secs(5);
    let result = loop {
        if Instant::now() >= deadline {
            break "timeout".to_string();
        }
        polls += 1;
        if terminal_hung_up() {
            after_hangup += 1;
        }
        match ctx.poll_event(Duration::from_millis(200)) {
            Ok(_) => {}
            Err(e) => break format!("err kind={:?}", e.kind()),
        }
    };
    let (result, kind) = result.split_once(' ').unwrap_or((&result, ""));
    std::fs::write(
        out,
        format!("result={result} {kind} polls={polls} after_hangup={after_hangup}"),
    )
    .unwrap();
}

/// Whether the child's terminal (fd 0, the PTY slave) reports a hangup: the master has closed.
/// A zero-timeout `poll` reads nothing, so it cannot change what `poll_event` sees.
fn terminal_hung_up() -> bool {
    let mut p = libc::pollfd {
        fd: 0,
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: `p` is one initialized entry; fd 0 stays open for the process's lifetime.
    let ready = unsafe { libc::poll(&mut p, 1, 0) };
    ready > 0 && p.revents & libc::POLLHUP != 0
}
