//! #285 — a key *release* is not a key press.
//!
//! The library-owned Unix input source decodes the kitty keyboard protocol, whose `CSI … u`
//! sequences carry an event type: `CSI 97u` presses `a`, `CSI 97;1:2u` repeats it and
//! `CSI 97;1:3u` releases it. LibGibson's public input model is press-only (a held key on an
//! ordinary terminal arrives as repeated presses), so the decided contract is:
//!
//! * **Press** → delivered as `Event::Key` (unchanged);
//! * **Repeat** → delivered as `Event::Key` (unchanged: the same observation a legacy
//!   terminal's autorepeat produces);
//! * **Release** → not delivered. The poll carries on for the rest of its timeout, so a
//!   dropped release neither ends the wait early (a false timeout) nor strands the events
//!   queued behind it.
//!
//! Before the fix every release reached the application as a second press.
//!
//! (0.6 release line: the 0.7-only `gibson_poll_event_kind` and DEEP SIGNAL legs are omitted.)
//!
//! Every case runs this test binary as the child of a real PTY (`portable-pty`), selected by
//! `GIBSON_285_CHILD`, and writes raw bytes to the PTY master as a terminal would.
#![cfg(unix)]
#[path = "common/pty_capture.rs"]
mod pty_capture;

use gibson::context::{Context, RenderMode};
use gibson::ffi::*;
use gibson::{Event, KeyCode};
use portable_pty::CommandBuilder;
use pty_capture::Capture;
use std::io::Write;
use std::time::{Duration, Instant};

const CHILD_ENV: &str = "GIBSON_285_CHILD";
const OUT_ENV: &str = "GIBSON_285_OUT";
/// Base path of the sync files the parent creates after writing, so a child measures a poll only
/// once the bytes are known to be queued on the terminal.
const SYNC_ENV: &str = "GIBSON_285_SYNC";

fn child(role: &str) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(std::env::current_exe().expect("test binary"));
    cmd.args([
        "--exact",
        "key_release_child",
        "--nocapture",
        "--test-threads=1",
    ]);
    cmd.env(CHILD_ENV, role);
    cmd
}

fn text(cap: &Capture) -> String {
    String::from_utf8_lossy(cap.raw()).into_owned()
}

fn wait_marker(cap: &mut Capture, marker: &str) {
    let m = marker.to_string();
    cap.collect_until(move |o| String::from_utf8_lossy(o).contains(&m))
        .unwrap_or_else(|e| panic!("child never printed {marker:?}: {e}"));
}

fn send(cap: &mut Capture, bytes: &[u8]) {
    cap.write(bytes).expect("write to the PTY");
    cap.collect_for(Duration::from_millis(60)).unwrap();
}

/// Waits until the child has printed an `EV …` line ending in `suffix`: an acknowledgement that
/// everything written before it has been consumed.
fn wait_ev(cap: &mut Capture, suffix: &str) {
    let want = suffix.to_string();
    cap.collect_until(move |o| {
        String::from_utf8_lossy(o)
            .split(['\r', '\n'])
            .any(|l| l.trim().starts_with("EV ") && l.trim().ends_with(want.as_str()))
    })
    .unwrap_or_else(|e| panic!("child never acknowledged {suffix:?}: {e}"));
}

/// The `EV …` lines the child printed, in order.
fn events(out: &str) -> Vec<String> {
    out.split(['\r', '\n'])
        .filter_map(|l| l.trim().strip_prefix("EV ").map(str::to_string))
        .collect()
}

/// Drives the full mixed sequence and returns the events the child reported.
fn run_sequence(role: &str) -> Vec<String> {
    let mut cap = Capture::spawn(child(role), 80, 24, Duration::from_secs(40));
    wait_marker(&mut cap, "READY");
    send(&mut cap, b"\x1b[97u"); // press a
    send(&mut cap, b"\x1b[97;1:2u"); // repeat a
    send(&mut cap, b"\x1b[97;1:3u"); // release a
    send(&mut cap, b"b"); // legacy press b, right after a release
    for _ in 0..3 {
        send(&mut cap, b"\x1b[99u"); // press c
        send(&mut cap, b"\x1b[99;1:3u"); // release c
    }
    // Fragmented across reads: a release of d, then a press of d.
    send(&mut cap, b"\x1b[100;1");
    send(&mut cap, b":3u");
    send(&mut cap, b"\x1b[10");
    send(&mut cap, b"0u");
    // U+1001 (MYANMAR LETTER KHA), the #274 collision range: press then release.
    send(&mut cap, b"\x1b[4097u");
    send(&mut cap, b"\x1b[4097;1:3u");
    // Focus changes around a release.
    send(&mut cap, b"\x1b[I");
    send(&mut cap, b"\x1b[97;1:3u");
    send(&mut cap, b"\x1b[O");
    // A resize between releases. The child first acknowledges everything written so far (the
    // press of `r` comes back as an EV line); otherwise a SIGWINCH could be decoded ahead of
    // bytes still queued, and the order would depend on scheduling.
    send(&mut cap, b"\x1b[98;1:3ur");
    wait_ev(&mut cap, ":114");
    cap.resize(100, 30);
    wait_ev(&mut cap, "resize:100x30");
    send(&mut cap, b"\x1b[98;1:3u");
    send(&mut cap, b"q");
    wait_marker(&mut cap, "DONE");
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        cap.collect_for(Duration::from_millis(30)).unwrap();
        if let Some(s) = cap.try_wait().unwrap() {
            break s;
        }
        assert!(Instant::now() < deadline, "[{role}] child did not exit");
    };
    let out = text(&cap);
    assert!(status.success(), "[{role}] child failed: {out:?}");
    events(&out)
}

#[test]
fn rust_poll_delivers_presses_and_repeats_but_never_a_release() {
    let got = run_sequence("seq:rust");
    let want = [
        "key:97",
        "key:97",
        "key:98",
        "key:99",
        "key:99",
        "key:99",
        "key:100",
        "key:4097",
        "focus:Gained",
        "focus:Lost",
        "key:114",
        "resize:100x30",
        "key:113",
    ];
    assert_eq!(got, want, "Rust Context::poll_event");
}

#[test]
fn c_abi_poll_delivers_presses_and_repeats_but_never_a_release() {
    // The legacy C poll reports focus changes as GIBSON_EVENT_NONE (unmodelled there), so they
    // do not appear; 4097 is U+1001 here (the documented #274 legacy ambiguity).
    let got = run_sequence("seq:c");
    let want = [
        "key:97",
        "key:97",
        "key:98",
        "key:99",
        "key:99",
        "key:99",
        "key:100",
        "key:4097",
        "key:114",
        "resize:100x30",
        "key:113",
    ];
    assert_eq!(got, want, "gibson_poll_event");
}

/// A release that is dropped must not end the poll: with nothing else to deliver, the poll
/// still waits out its timeout. And a key queued behind a release is delivered at once, with
/// nothing left over.
#[test]
fn a_dropped_release_neither_times_out_early_nor_strands_the_next_key() {
    let sync = std::env::temp_dir().join(format!("gibson_285_sync_{}", std::process::id()));
    let flag = |n: u8| sync.with_extension(n.to_string());
    let _ = std::fs::remove_file(flag(1));
    let _ = std::fs::remove_file(flag(2));
    let mut cmd = child("timeout");
    cmd.env(SYNC_ENV, sync.to_str().unwrap());
    let mut cap = Capture::spawn(cmd, 80, 24, Duration::from_secs(40));
    wait_marker(&mut cap, "READY");
    // A release, then F5 (a key code LibGibson does not model): neither is delivered.
    send(&mut cap, b"\x1b[97;1:3u\x1b[15~");
    std::fs::write(flag(1), b"").unwrap();
    wait_marker(&mut cap, "PHASE2");
    // One write: the release and the press arrive in the same read.
    send(&mut cap, b"\x1b[97;1:3ub");
    std::fs::write(flag(2), b"").unwrap();
    wait_marker(&mut cap, "DONE");
    let _ = std::fs::remove_file(flag(1));
    let _ = std::fs::remove_file(flag(2));
    let out = text(&cap);
    let line = |tag: &str| -> (String, u64) {
        let l = out
            .split(['\r', '\n'])
            .find_map(|l| {
                l.trim()
                    .strip_prefix(&format!("{tag} "))
                    .map(str::to_string)
            })
            .unwrap_or_else(|| panic!("no {tag} line in {out:?}"));
        let mut it = l.split_whitespace();
        let ev = it.next().unwrap().to_string();
        let ms = it.next().unwrap().parse().unwrap();
        (ev, ms)
    };
    let (t1, t1_ms) = line("T1");
    assert_eq!(t1, "none", "a lone release was delivered: {out:?}");
    assert!(
        t1_ms >= 700,
        "poll(800 ms) returned after {t1_ms} ms on a dropped release: a false timeout"
    );
    assert!(
        t1_ms < 1500,
        "poll(800 ms) took {t1_ms} ms: the dropped events restarted its timeout"
    );
    let (t2, t2_ms) = line("T2");
    assert_eq!(t2, "key:98", "the press queued behind a release: {out:?}");
    // The bytes were queued before the poll began (sync file), so the press comes out at once;
    // waiting for the timeout would take 800 ms.
    assert!(t2_ms < 700, "the queued press took {t2_ms} ms");
    let (t3, _) = line("T3");
    assert_eq!(
        t3, "none",
        "something was left over after the press: {out:?}"
    );
}

/// Key input immediately before a hangup: the press is delivered, the release is not turned
/// into a second press, and the poll then reports the hangup (BrokenPipe) instead of spinning.
#[test]
fn a_release_queued_before_a_hangup_is_not_a_press() {
    let out_path =
        std::env::temp_dir().join(format!("gibson_285_hangup_{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&out_path);
    let mut cmd = child("hangup");
    cmd.env(OUT_ENV, out_path.to_str().unwrap());
    let mut cap = Capture::spawn(cmd, 80, 24, Duration::from_secs(30));
    wait_marker(&mut cap, "READY");
    send(&mut cap, b"\x1b[120u\x1b[120;1:3u");
    wait_marker(&mut cap, "GOT1");
    cap.hang_up();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let _ = cap.collect_for(Duration::from_millis(30));
        if cap.try_wait().unwrap().is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "hangup child did not exit");
    }
    let got = std::fs::read_to_string(&out_path).expect("child result");
    let inq_path = format!("{}.inq", out_path.to_str().unwrap());
    let inq = std::fs::read_to_string(&inq_path).expect("child precondition");
    let _ = std::fs::remove_file(&out_path);
    let _ = std::fs::remove_file(&inq_path);
    assert_eq!(
        inq.trim(),
        "0 0",
        "precondition: the release bytes were still unread at the hangup (ioctl rc, unread)"
    );
    assert_eq!(got.trim(), "key:120 err:BrokenPipe");
}

// ---------------------------------------------------------------------------------------------
// Child side
// ---------------------------------------------------------------------------------------------

fn desc(e: &Event) -> String {
    match e {
        Event::Key(k) => match k.code {
            KeyCode::Char(c) => format!("key:{}", u32::from(c)),
            other => format!("key:{other:?}"),
        },
        Event::Focus(f) => format!("focus:{f:?}"),
        Event::Resize(c, r) => format!("resize:{c}x{r}"),
        other => format!("other:{other:?}"),
    }
}

fn say(s: &str) {
    // Raw mode: no newline translation, so end lines explicitly.
    print!("{s}\r\n");
    std::io::stdout().flush().unwrap();
}

fn blank_event() -> GibsonEvent {
    GibsonEvent {
        event_type: -1,
        key_code: 0,
        modifiers: 0,
        _pad: [0; 3],
        cols: 0,
        rows: 0,
        paste_str: std::ptr::null(),
    }
}

/// Child for the tests above; a no-op in a normal run.
#[test]
fn key_release_child() {
    let Some(role) = std::env::var_os(CHILD_ENV) else {
        return;
    };
    match role.to_string_lossy().as_ref() {
        "seq:rust" => seq_rust(),
        "seq:c" => seq_c(),
        "timeout" => timeout_child(),
        "hangup" => hangup_child(),
        other => panic!("unknown role {other}"),
    }
}

fn seq_rust() {
    let mut ctx = Context::new(RenderMode::Inline).expect("terminal context");
    // Arm raw mode before the parent types, so nothing is held by canonical input.
    let _ = ctx.poll_event(Duration::ZERO).unwrap();
    say("READY");
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if let Some(e) = ctx.poll_event(Duration::from_millis(100)).unwrap() {
            say(&format!("EV {}", desc(&e)));
            if matches!(&e, Event::Key(k) if k.code == KeyCode::Char('q')) {
                break;
            }
        }
    }
    ctx.restore().unwrap();
    say("DONE");
}

fn seq_c() {
    unsafe {
        let mut h: *mut GibsonContextOpaque = std::ptr::null_mut();
        assert_eq!(
            gibson_create_context(GIBSON_MODE_INLINE, &mut h),
            GibsonStatus::Ok
        );
        let mut ev = blank_event();
        let _ = gibson_poll_event(h, 0, &mut ev);
        say("READY");
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            let mut ev = blank_event();
            let st = gibson_poll_event(h, 100, &mut ev);
            assert_eq!(st, GibsonStatus::Ok);
            let line = match ev.event_type {
                GIBSON_EVENT_KEY => format!("key:{}", ev.key_code),
                GIBSON_EVENT_RESIZE => format!("resize:{}x{}", ev.cols, ev.rows),
                GIBSON_EVENT_NONE => continue,
                other => format!("type:{other}"),
            };
            say(&format!("EV {line}"));
            if ev.event_type == GIBSON_EVENT_KEY && ev.key_code == u32::from('q') {
                break;
            }
        }
        gibson_destroy_context(h);
    }
    say("DONE");
}

fn timeout_child() {
    let mut ctx = Context::new(RenderMode::Inline).expect("terminal context");
    let _ = ctx.poll_event(Duration::ZERO).unwrap();
    let timed = |ctx: &mut Context, ms: u64| {
        let t0 = Instant::now();
        let e = ctx.poll_event(Duration::from_millis(ms)).unwrap();
        let took = t0.elapsed().as_millis();
        (e.map(|e| desc(&e)).unwrap_or_else(|| "none".into()), took)
    };
    let sync = std::path::PathBuf::from(std::env::var(SYNC_ENV).expect("sync path"));
    let wait_flag = |n: u8| {
        let f = sync.with_extension(n.to_string());
        let cap = Instant::now() + Duration::from_secs(20);
        while !f.exists() {
            assert!(Instant::now() < cap, "parent never wrote sync flag {n}");
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    say("READY");
    // The parent writes a release and F5, then raises flag 1.
    wait_flag(1);
    let (e, ms) = timed(&mut ctx, 800);
    say(&format!("T1 {e} {ms}"));
    say("PHASE2");
    // The parent writes release + `b` in one write, then raises flag 2.
    wait_flag(2);
    let (e, ms) = timed(&mut ctx, 800);
    say(&format!("T2 {e} {ms}"));
    let (e, ms) = timed(&mut ctx, 300);
    say(&format!("T3 {e} {ms}"));
    ctx.restore().unwrap();
    say("DONE");
}

fn hangup_child() {
    // Survive the hangup (as an application that ignores SIGHUP does) and observe the poll.
    unsafe {
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
    }
    let out = std::env::var(OUT_ENV).expect("result path");
    let mut ctx = Context::new(RenderMode::Inline).expect("terminal context");
    let _ = ctx.poll_event(Duration::ZERO).unwrap();
    say("READY");
    let mut got: Vec<String> = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match ctx.poll_event(Duration::from_millis(200)) {
            Ok(Some(e)) => {
                got.push(desc(&e));
                if got.len() == 1 {
                    // Precondition: the release's bytes have been read off the terminal (into the
                    // source's decoder) before the hangup, which would otherwise discard them
                    // unread and let a release-as-press bug go unseen.
                    let mut unread: libc::c_int = 0;
                    // SAFETY: FIONREAD writes one c_int for a valid descriptor.
                    let rc = unsafe { libc::ioctl(0, libc::FIONREAD, &mut unread) };
                    std::fs::write(format!("{out}.inq"), format!("{rc} {unread}")).unwrap();
                    say("GOT1");
                    // The parent hangs up the terminal in this window.
                    std::thread::sleep(Duration::from_millis(400));
                }
            }
            Ok(None) => {}
            Err(e) => {
                got.push(format!("err:{:?}", e.kind()));
                break;
            }
        }
    }
    std::fs::write(out, got.join(" ")).unwrap();
}
