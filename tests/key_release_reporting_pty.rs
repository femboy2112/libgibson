//! #275 — key-release reporting through a real PTY: what LibGibson writes to the terminal, and
//! what it delivers back.
//!
//! `TerminalSession::set_key_release_reporting(true)` pushes kitty keyboard flags 15 on the
//! current screen and asks the terminal whether it took them (`CSI ? u`, then `CSI c`). This test
//! plays the terminal: it reads the child's output from the PTY master, answers as kitty 0.32.2
//! answered under Xvfb (`CSI ? 15 u`, `CSI ? 62 ; c`) or as a terminal without the protocol
//! (`CSI ? 62 ; c` only), and types the bytes kitty sent for real key presses and releases. The
//! child prints every event it polls as an `EV …` line.
//!
//! Checked: the push lands on the screen the input is read on and is popped before that screen is
//! left, on a normal exit and from the panic hook; the answer arrives in input order; releases are
//! delivered named as their presses; keys held together are released independently; nothing is
//! written and no release is delivered when the application does not opt in (#285 unchanged).
#![cfg(unix)]
#[path = "common/pty_capture.rs"]
mod pty_capture;

use gibson::context::{Context, RenderMode};
use gibson::{Event, KeyCode, KeyReleaseReporting};
use portable_pty::CommandBuilder;
use pty_capture::Capture;
use std::io::Write;
use std::time::{Duration, Instant};

const CHILD_ENV: &str = "GIBSON_275_CHILD";

const PUSH_AND_ASK: &[u8] = b"\x1b[>15u\x1b[?u\x1b[c";
const POP: &[u8] = b"\x1b[<u";
const ENTER_ALT: &[u8] = b"\x1b[?1049h";
const LEAVE_ALT: &[u8] = b"\x1b[?1049l";
/// kitty 0.32.2's answers after a push of 15.
const KITTY_ANSWER: &[u8] = b"\x1b[?15u\x1b[?62;c";
/// A terminal without the protocol answers only the device attributes query.
const PLAIN_ANSWER: &[u8] = b"\x1b[?62;c";

fn child(role: &str) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(std::env::current_exe().expect("test binary"));
    cmd.args([
        "--exact",
        "key_release_reporting_child",
        "--nocapture",
        "--test-threads=1",
    ]);
    cmd.env(CHILD_ENV, role);
    cmd
}

fn wait_for(cap: &mut Capture, what: &str) {
    let w = what.to_string();
    cap.collect_until(move |o| String::from_utf8_lossy(o).contains(&w))
        .unwrap_or_else(|e| panic!("child never printed {what:?}: {e}"));
}

fn wait_bytes(cap: &mut Capture, what: &'static [u8]) {
    cap.collect_until(move |o| find(o, what, 0).is_some())
        .unwrap_or_else(|e| panic!("child never wrote {what:?}: {e}"));
}

fn send(cap: &mut Capture, bytes: &[u8]) {
    cap.write(bytes).expect("write to the PTY");
    cap.collect_for(Duration::from_millis(60)).unwrap();
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    hay.get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|i| i + from)
}

fn count(hay: &[u8], needle: &[u8]) -> usize {
    hay.windows(needle.len()).filter(|w| *w == needle).count()
}

fn events(cap: &Capture) -> Vec<String> {
    String::from_utf8_lossy(cap.raw())
        .split(['\r', '\n'])
        // An `Unsupported` answer is followed at once by the pop that withdraws the push, written
        // just before the child prints the event.
        .filter_map(|l| {
            l.trim()
                .trim_start_matches("\x1b[<u")
                .strip_prefix("EV ")
                .map(str::to_string)
        })
        .collect()
}

fn finish(mut cap: Capture, role: &str) -> (Capture, portable_pty::ExitStatus) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        cap.collect_for(Duration::from_millis(30)).unwrap();
        if let Some(s) = cap.try_wait().unwrap() {
            break s;
        }
        assert!(Instant::now() < deadline, "[{role}] child did not exit");
    };
    // An exited child may have written more than was read. The test harness's own summary is
    // the child's last output, after every terminal write; wait for it rather than for a deadline
    // (#275 re-review).
    cap.collect_until(|o| String::from_utf8_lossy(o).contains("test result:"))
        .unwrap_or_else(|e| panic!("[{role}] the child's output never ended: {e}"));
    (cap, status)
}

/// What kitty 0.32.2 sent (captured under Xvfb with xdotool, flags 15), in order:
/// shift+w with shift let go first; shift+1 likewise; Left pressed with shift, repeated and
/// released after it; the chord w held, d pressed, autorepeated and released, then w released;
/// Enter pressed and released.
const KITTY_KEYS: &[&[u8]] = &[
    b"\x1b[57441;2u\x1b[119:87;2u\x1b[57441;1:3u\x1b[119;1:3u",
    b"\x1b[57441;2u\x1b[49:33;2u\x1b[57441;1:3u\x1b[49;1:3u",
    b"\x1b[1;2D\x1b[1;2:2D\x1b[1;1:3D",
    b"\x1b[119u\x1b[100u\x1b[100;1:2u\x1b[100;1:2u\x1b[100;1:3u\x1b[119;1:3u",
    b"\x1b[13u\x1b[13;1:3u",
];

const KITTY_EVENTS: &[&str] = &[
    "key:W:0",
    "release:W:0",
    "key:!:0",
    "release:!:0",
    "key:Left:1",
    "key:Left:1",
    "release:Left:1",
    "key:w:0",
    "key:d:0",
    "key:d:0",
    "key:d:0",
    "release:d:0",
    "release:w:0",
    "key:Enter:0",
    "release:Enter:0",
];

#[test]
fn a_kitty_terminal_reports_releases_named_as_their_presses() {
    let mut cap = Capture::spawn(child("fullscreen"), 80, 24, Duration::from_secs(40));
    wait_for(&mut cap, "READY");
    wait_bytes(&mut cap, PUSH_AND_ASK);
    let raw = cap.raw().to_vec();
    let alt = find(&raw, ENTER_ALT, 0).expect("fullscreen enters the alternate screen");
    let push = find(&raw, PUSH_AND_ASK, 0).unwrap();
    assert!(
        alt < push,
        "the push must land on the alternate screen's stack"
    );
    assert_eq!(count(&raw, PUSH_AND_ASK), 1);

    // A key typed before the answer is an ordinary press (holds are still inferred).
    send(&mut cap, b"x");
    send(&mut cap, KITTY_ANSWER);
    wait_for(&mut cap, "EV reporting:Active");
    for keys in KITTY_KEYS {
        send(&mut cap, keys);
    }
    send(&mut cap, b"\x1b[113u"); // q, as kitty sends it with flags 15
    wait_for(&mut cap, "DONE");
    let (cap, status) = finish(cap, "fullscreen");
    assert!(status.success(), "{:?}", String::from_utf8_lossy(cap.raw()));

    let ev = events(&cap);
    let mut want = vec!["key:x:0".to_string(), "reporting:Active".to_string()];
    want.extend(KITTY_EVENTS.iter().map(|s| s.to_string()));
    want.push("key:q:0".into());
    assert_eq!(ev, want);

    let raw = cap.raw();
    let pop = find(raw, POP, push).expect("restore pops the flags");
    let leave = find(raw, LEAVE_ALT, push).expect("restore leaves the alternate screen");
    assert!(
        pop < leave,
        "popped on the alternate screen, before leaving it"
    );
    assert_eq!(count(raw, POP), 1, "popped exactly once");
}

#[test]
fn a_terminal_without_the_protocol_answers_unsupported() {
    let mut cap = Capture::spawn(child("fullscreen"), 80, 24, Duration::from_secs(40));
    wait_for(&mut cap, "READY");
    wait_bytes(&mut cap, PUSH_AND_ASK);
    send(&mut cap, PLAIN_ANSWER);
    wait_for(&mut cap, "EV reporting:Unsupported");
    send(&mut cap, b"wq");
    wait_for(&mut cap, "DONE");
    let (cap, status) = finish(cap, "plain");
    assert!(status.success());
    assert_eq!(
        events(&cap),
        ["reporting:Unsupported", "key:w:0", "key:q:0"]
    );
    // The flags were still pushed, so they are popped (a no-op on such a terminal), once, as the
    // answer is handed out rather than at restore.
    let raw = cap.raw();
    assert_eq!(count(raw, POP), 1);
    assert!(find(raw, POP, 0) < find(raw, b"EV reporting:Unsupported", 0));
}

#[test]
fn without_the_opt_in_nothing_is_written_and_no_release_is_delivered() {
    let mut cap = Capture::spawn(child("off"), 80, 24, Duration::from_secs(40));
    wait_for(&mut cap, "READY");
    // Even if something else left the protocol on, releases stay undelivered (#285), and a
    // stray answer is not mistaken for keystrokes or for an answer to a request never made.
    send(&mut cap, KITTY_ANSWER);
    send(&mut cap, b"\x1b[119u\x1b[119;1:3u");
    send(&mut cap, b"q");
    wait_for(&mut cap, "DONE");
    let (cap, status) = finish(cap, "off");
    assert!(status.success());
    assert_eq!(events(&cap), ["key:w:0", "key:q:0"]);
    let raw = cap.raw();
    assert_eq!(find(raw, b"\x1b[>", 0), None, "no push");
    assert_eq!(find(raw, b"\x1b[?u", 0), None, "no query");
    assert_eq!(find(raw, POP, 0), None, "no pop");
}

#[test]
fn the_panic_hook_pops_the_flags_before_leaving_the_alternate_screen() {
    let mut cap = Capture::spawn(child("panic"), 80, 24, Duration::from_secs(40));
    wait_for(&mut cap, "READY");
    wait_bytes(&mut cap, PUSH_AND_ASK);
    send(&mut cap, KITTY_ANSWER);
    wait_for(&mut cap, "EV reporting:Active");
    send(&mut cap, b"\x1b[33u"); // `!` panics the child
    let (cap, status) = finish(cap, "panic");
    assert!(!status.success(), "the child panicked");
    let raw = cap.raw();
    let push = find(raw, PUSH_AND_ASK, 0).unwrap();
    let pop = find(raw, POP, push).expect("the panic hook pops the flags");
    let leave = find(raw, LEAVE_ALT, push).expect("and leaves the alternate screen");
    assert!(pop < leave);
    // The restore that unwinding runs afterwards must not pop again: on the main screen, that
    // would remove another program's entry (#275 review).
    assert_eq!(count(raw, POP), 1, "popped exactly once");
}

#[test]
fn a_stray_device_attributes_reply_reads_as_unsupported_and_is_final() {
    // Somebody else's earlier `CSI c` is answered after the request was written (#275 review). It
    // cannot be told from a terminal without the protocol, and `Unsupported` is final: the push
    // is withdrawn as it is handed out, so kitty's own flags reply that follows is not an answer
    // any more, and no release is delivered.
    let mut cap = Capture::spawn(child("fullscreen"), 80, 24, Duration::from_secs(40));
    wait_for(&mut cap, "READY");
    wait_bytes(&mut cap, PUSH_AND_ASK);
    send(&mut cap, PLAIN_ANSWER); // the stray
    wait_for(&mut cap, "EV reporting:Unsupported");
    send(&mut cap, KITTY_ANSWER);
    send(&mut cap, b"\x1b[119u\x1b[119;1:3u\x1b[113u");
    wait_for(&mut cap, "DONE");
    let (cap, status) = finish(cap, "stray");
    assert!(status.success());
    assert_eq!(
        events(&cap),
        ["reporting:Unsupported", "key:w:0", "key:q:0"]
    );
    assert_eq!(count(cap.raw(), POP), 1);
}

#[test]
fn after_disabling_a_late_answer_and_releases_are_dropped() {
    // The child enables, then disables before reading anything; the answer arrives late
    // (#275 review: it must not switch a hold model back on).
    let mut cap = Capture::spawn(child("disable"), 80, 24, Duration::from_secs(40));
    wait_for(&mut cap, "DISABLED");
    send(&mut cap, KITTY_ANSWER);
    send(&mut cap, b"\x1b[119u\x1b[119;1:3uq");
    wait_for(&mut cap, "DONE");
    let (cap, status) = finish(cap, "disable");
    assert!(status.success());
    assert_eq!(events(&cap), ["key:w:0", "key:q:0"]);
    let raw = cap.raw();
    assert_eq!(count(raw, PUSH_AND_ASK), 1);
    assert_eq!(
        count(raw, POP),
        1,
        "popped by the disable, not again at restore"
    );
}

#[test]
fn after_a_caught_panic_reporting_is_off_and_can_be_enabled_again() {
    // #275 re-review: the panic hook pops the flags even when the panic is caught; the session
    // must then say it is off, and enabling again must push again.
    let mut cap = Capture::spawn(child("caught-panic"), 80, 24, Duration::from_secs(40));
    wait_for(&mut cap, "REENABLED");
    // The terminal answers; raw mode must be back on, or a cooked terminal echoes the answer and
    // holds it back (#275 re-review 3).
    send(&mut cap, KITTY_ANSWER);
    wait_for(&mut cap, "EV reporting:Active");
    send(&mut cap, b"\x1b[113u");
    wait_for(&mut cap, "DONE");
    let (cap, status) = finish(cap, "caught-panic");
    assert!(status.success(), "{:?}", String::from_utf8_lossy(cap.raw()));
    let out = String::from_utf8_lossy(cap.raw()).into_owned();
    assert!(out.contains("AFTER-PANIC enabled=false"), "{out:?}");
    assert!(out.contains("REENABLED enabled=true"), "{out:?}");
    let raw = cap.raw();
    assert_eq!(
        count(raw, PUSH_AND_ASK),
        2,
        "pushed again after the caught panic"
    );
    assert_eq!(count(raw, POP), 2, "the hook's pop and the final restore's");
}

#[test]
fn after_a_caught_panic_no_release_is_delivered() {
    // #275 review: the panic hook pops the flags; a client that catches the panic and keeps
    // polling, without calling into the session, must not receive releases from then on (it
    // would hold every key whose release it waits for). The release here is one kitty sent
    // before the pop took effect.
    let mut cap = Capture::spawn(child("caught-panic-poll"), 80, 24, Duration::from_secs(40));
    wait_for(&mut cap, "READY");
    wait_bytes(&mut cap, PUSH_AND_ASK);
    send(&mut cap, KITTY_ANSWER);
    wait_for(&mut cap, "EV reporting:Active");
    send(&mut cap, b"\x1b[33u"); // `!`: the child panics and catches it
    wait_for(&mut cap, "CAUGHT");
    send(&mut cap, b"\x1b[119u\x1b[119;1:3u\x1b[113u");
    wait_for(&mut cap, "DONE");
    let (cap, status) = finish(cap, "caught-panic-poll");
    assert!(status.success(), "{:?}", String::from_utf8_lossy(cap.raw()));
    assert_eq!(
        events(&cap),
        [
            "reporting:Active",
            "key:!:0",
            // The client learns that releases ended, once, on the next poll.
            "reporting:Unsupported",
            "key:w:0",
            "key:q:0"
        ]
    );
    assert_eq!(count(cap.raw(), POP), 1, "the hook's pop only");
}

#[test]
fn a_partial_flags_reply_is_unsupported_and_the_push_is_withdrawn() {
    // A terminal that took only flags 3 (no 4, no 8) answers `CSI ? 3 u`: Unsupported. LibGibson
    // pops its push as it hands that answer out, so the terminal goes back to legacy keys (one
    // `\r` per Enter, no release), and delivers no release from then on.
    let mut cap = Capture::spawn(child("fullscreen"), 80, 24, Duration::from_secs(40));
    wait_for(&mut cap, "READY");
    wait_bytes(&mut cap, PUSH_AND_ASK);
    let push = find(cap.raw(), PUSH_AND_ASK, 0).unwrap();
    send(&mut cap, b"\x1b[?3u\x1b[?62;c");
    wait_for(&mut cap, "EV reporting:Unsupported");
    let raw = cap.raw().to_vec();
    let answer = find(&raw, b"EV reporting:Unsupported", push).unwrap();
    let pop = find(&raw, POP, push);
    // Played as the terminal: a w release sent before the pop took effect, then Enter, which
    // with flags 3 still applied is `\r` twice (press, and release as the legacy byte).
    let enter: &[u8] = if pop.is_some() { b"\r" } else { b"\r\r" };
    send(&mut cap, b"w\x1b[119;1:3u");
    send(&mut cap, enter);
    send(&mut cap, b"q");
    wait_for(&mut cap, "DONE");
    let (cap, status) = finish(cap, "partial");
    assert!(status.success(), "{:?}", String::from_utf8_lossy(cap.raw()));
    assert_eq!(
        events(&cap),
        ["reporting:Unsupported", "key:w:0", "key:Enter:0", "key:q:0"]
    );
    assert!(
        pop.is_some_and(|p| p < answer),
        "popped as the answer was handed out"
    );
    assert_eq!(
        count(cap.raw(), POP),
        1,
        "withdrawn once, not again at restore"
    );
}

/// The owner enables; another reader hands the answer out (#275 review 8).
fn unsupported_through_another_reader(role: &str) {
    let mut cap = Capture::spawn(child(role), 80, 24, Duration::from_secs(40));
    wait_for(&mut cap, "READY");
    wait_bytes(&mut cap, PUSH_AND_ASK);
    send(&mut cap, b"\x1b[?3u\x1b[?62;c");
    wait_for(&mut cap, "EV reporting:Unsupported");
    send(&mut cap, b"w\x1b[119;1:3u");
    send(&mut cap, b"q");
    wait_for(&mut cap, "DONE");
    let (cap, status) = finish(cap, role);
    assert!(status.success(), "{:?}", String::from_utf8_lossy(cap.raw()));
    assert_eq!(
        events(&cap),
        ["reporting:Unsupported", "key:w:0", "key:q:0"],
        "[{role}] no release once Unsupported is handed out, whoever polls"
    );
    let out = String::from_utf8_lossy(cap.raw()).into_owned();
    assert!(out.contains("OWNER enabled=false"), "[{role}] {out:?}");
    // The owner never polled through a Context: its restore performs the pending pop, once.
    assert_eq!(count(cap.raw(), POP), 1, "[{role}]");
}

#[test]
fn unsupported_through_the_free_poll_function_stops_releases() {
    unsupported_through_another_reader("foreign-free");
}

#[test]
fn unsupported_through_a_headless_context_reading_the_terminal_stops_releases() {
    unsupported_through_another_reader("foreign-headless");
}

#[test]
fn reenabling_after_a_caught_panic_keeps_the_ended_notice() {
    // #275 review 8: Active, w held (an exact hold), a caught panic ends reporting, and the
    // client re-enables before polling again. It must still see Unsupported, before the new
    // answer, so the old hold ends.
    let mut cap = Capture::spawn(
        child("caught-panic-reenable"),
        80,
        24,
        Duration::from_secs(40),
    );
    wait_for(&mut cap, "READY");
    wait_bytes(&mut cap, PUSH_AND_ASK);
    send(&mut cap, KITTY_ANSWER);
    wait_for(&mut cap, "EV reporting:Active");
    send(&mut cap, b"\x1b[119u"); // w down
    wait_for(&mut cap, "EV key:w:0");
    send(&mut cap, b"\x1b[33u"); // `!`: the child panics, catches it and re-enables
    wait_for(&mut cap, "REENABLED");
    send(&mut cap, KITTY_ANSWER);
    send(&mut cap, b"\x1b[113u");
    wait_for(&mut cap, "DONE");
    let (cap, status) = finish(cap, "caught-panic-reenable");
    assert!(status.success(), "{:?}", String::from_utf8_lossy(cap.raw()));
    assert_eq!(
        events(&cap),
        [
            "reporting:Active",
            "key:w:0",
            "key:!:0",
            "reporting:Unsupported",
            "reporting:Active",
            "key:q:0"
        ]
    );
    let out = String::from_utf8_lossy(cap.raw()).into_owned();
    assert!(out.contains("HELD []"), "the old w hold ended: {out:?}");
}

/// Where the `caught-panic-hangup` child writes its findings: after the hangup its terminal is
/// gone, so it cannot print them.
const LOG_ENV: &str = "GIBSON_275_LOG";

#[test]
fn a_failed_raw_mode_reentry_keeps_the_ended_notice() {
    // #275 review 8: a caught panic ends reporting, then the terminal hangs up. The next
    // `Context` poll cannot re-enter raw mode and fails; the `Unsupported` notice it would have
    // delivered must wait for the next poll, not vanish with the error. (A notice that dies with
    // its messenger is how a hold stays down forever.)
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    let log = std::env::temp_dir().join(format!("gibson_275_hangup_{unique}.log"));
    let _ = std::fs::remove_file(&log);
    let mut cmd = child("caught-panic-hangup");
    cmd.env(LOG_ENV, &log);
    let mut cap = Capture::spawn(cmd, 80, 24, Duration::from_secs(40));
    wait_bytes(&mut cap, PUSH_AND_ASK);
    wait_for(&mut cap, "CAUGHT");
    cap.hang_up();
    let deadline = Instant::now() + Duration::from_secs(15);
    while cap.try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < deadline,
            "the child did not exit after the hangup"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let got = std::fs::read_to_string(&log).unwrap_or_else(|e| panic!("child log {log:?}: {e}"));
    let _ = std::fs::remove_file(&log);
    assert_eq!(got, "POLL error\nFROM reporting:Unsupported\n");
}

#[test]
fn a_restored_session_cannot_enable_reporting() {
    // #275 review: after restore the session no longer holds the terminal.
    let mut cap = Capture::spawn(child("restored"), 80, 24, Duration::from_secs(40));
    wait_for(&mut cap, "DONE");
    let (cap, status) = finish(cap, "restored");
    assert!(status.success(), "{:?}", String::from_utf8_lossy(cap.raw()));
    let raw = cap.raw();
    assert_eq!(find(raw, b"\x1b[>", 0), None, "no push after restore");
    assert_eq!(find(raw, POP, 0), None);
}

#[test]
fn reporting_follows_the_screen_inline_and_across_an_alternate_screen_switch() {
    let mut cap = Capture::spawn(child("inline"), 80, 24, Duration::from_secs(40));
    wait_for(&mut cap, "READY");
    wait_bytes(&mut cap, PUSH_AND_ASK);
    assert_eq!(
        find(cap.raw(), ENTER_ALT, 0),
        None,
        "inline: pushed on the main screen"
    );
    send(&mut cap, KITTY_ANSWER);
    wait_for(&mut cap, "EV reporting:Active");
    // The child now enters the alternate screen: pop on main, switch, push and ask again.
    send(&mut cap, b"\x1b[97u"); // a
    wait_for(&mut cap, "ALT");
    send(&mut cap, KITTY_ANSWER);
    wait_for(&mut cap, "EV reporting:Active\r\nEV");
    send(&mut cap, b"\x1b[113u"); // q
    wait_for(&mut cap, "DONE");
    let (cap, status) = finish(cap, "inline");
    assert!(status.success(), "{:?}", String::from_utf8_lossy(cap.raw()));
    let raw = cap.raw();
    let first_push = find(raw, PUSH_AND_ASK, 0).unwrap();
    let first_pop = find(raw, POP, first_push).expect("popped before switching screens");
    let enter = find(raw, ENTER_ALT, first_pop).expect("then the switch");
    let second_push = find(raw, PUSH_AND_ASK, enter).expect("pushed again on the new screen");
    let final_pop = find(raw, POP, second_push).expect("and popped at restore");
    let leave = find(raw, LEAVE_ALT, second_push).expect("before leaving the alternate screen");
    assert!(final_pop < leave);
    assert_eq!(count(raw, PUSH_AND_ASK), 2);
    assert_eq!(count(raw, POP), 2);
    assert_eq!(
        events(&cap),
        ["reporting:Active", "key:a:0", "reporting:Active", "key:q:0"]
    );
}

// ------------------------------------------------------------------------------- child side

fn desc(e: &Event) -> String {
    let name = |k: &gibson::KeyEvent| {
        let code = match k.code {
            KeyCode::Char(c) => c.to_string(),
            other => format!("{other:?}"),
        };
        format!("{code}:{}", k.modifiers.bits())
    };
    match e {
        Event::Key(k) => format!("key:{}", name(k)),
        Event::KeyRelease(k) => format!("release:{}", name(k)),
        Event::KeyReleaseReporting(KeyReleaseReporting::Active) => "reporting:Active".into(),
        Event::KeyReleaseReporting(_) => "reporting:Unsupported".into(),
        other => format!("other:{other:?}"),
    }
}

fn say(s: &str) {
    // Raw mode: no newline translation, so end lines explicitly.
    print!("{s}\r\n");
    std::io::stdout().flush().unwrap();
}

/// Child for the tests above; a no-op in a normal run.
#[test]
fn key_release_reporting_child() {
    let Some(role) = std::env::var_os(CHILD_ENV) else {
        return;
    };
    let role = role.to_string_lossy().into_owned();
    let mode = if role == "inline" {
        RenderMode::Inline
    } else {
        RenderMode::Fullscreen
    };
    let mut ctx = Context::new(mode).expect("terminal context");
    if role == "restored" {
        ctx.restore().unwrap();
        ctx.session.set_key_release_reporting(true).unwrap();
        assert!(!ctx.session.key_release_reporting_enabled());
        say("DONE");
        return;
    }
    ctx.session.enter_interactive().unwrap();
    if role == "foreign-free" || role == "foreign-headless" {
        ctx.session.set_key_release_reporting(true).unwrap();
        say("READY");
        let mut headless = Context::headless(RenderMode::Fullscreen, 80, 24);
        let mut terminal = gibson::TerminalSource;
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            let polled = if role == "foreign-free" {
                gibson::input::poll_event(Duration::from_millis(100))
            } else {
                headless.poll_from(&mut terminal, Duration::from_millis(100))
            };
            let Some(e) = polled.unwrap() else {
                continue;
            };
            if matches!(e, Event::Resize(..) | Event::Focus(_)) {
                continue;
            }
            say(&format!("EV {}", desc(&e)));
            if matches!(e, Event::KeyReleaseReporting(_)) {
                let on = ctx.session.key_release_reporting_enabled();
                say(&format!("OWNER enabled={on}"));
            }
            if matches!(&e, Event::Key(k) if k.code == KeyCode::Char('q')) {
                break;
            }
        }
        ctx.restore().unwrap();
        say("DONE");
        return;
    }
    if role == "caught-panic-reenable" {
        use gibson::hold::{HoldConfig, HoldTracker};
        let mut holds = HoldTracker::new(HoldConfig::new(1, 1));
        ctx.session.set_key_release_reporting(true).unwrap();
        say("READY");
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut tic = 0u64;
        while Instant::now() < deadline {
            let Some(e) = ctx.poll_event(Duration::from_millis(100)).unwrap() else {
                continue;
            };
            if matches!(e, Event::Resize(..) | Event::Focus(_)) {
                continue;
            }
            tic += 1;
            holds.observe_event(tic, &e, |k| Some(k.code)).unwrap();
            say(&format!("EV {}", desc(&e)));
            match &e {
                Event::Key(k) if k.code == KeyCode::Char('q') => {
                    let held: Vec<_> = holds
                        .held()
                        .map(|(k, _)| k)
                        .filter(|k| *k != KeyCode::Char('q'))
                        .collect();
                    say(&format!("HELD {held:?}"));
                    break;
                }
                Event::Key(k) if k.code == KeyCode::Char('!') => {
                    let caught = std::panic::catch_unwind(|| panic!("caught, on the owner thread"));
                    assert!(caught.is_err());
                    ctx.session.set_key_release_reporting(true).unwrap();
                    say("REENABLED");
                }
                _ => {}
            }
        }
        ctx.restore().unwrap();
        say("DONE");
        return;
    }
    if role == "caught-panic-hangup" {
        // The hangup must not end this child; it reports through a file instead.
        // SAFETY: SIG_IGN installs no handler code.
        unsafe { libc::signal(libc::SIGHUP, libc::SIG_IGN) };
        let log = std::env::var_os(LOG_ENV).expect("log path");
        ctx.session.set_key_release_reporting(true).unwrap();
        let caught = std::panic::catch_unwind(|| panic!("caught, on the owner thread"));
        assert!(caught.is_err());
        say("CAUGHT");
        // Wait for the parent to hang up; until then a poll would succeed and say too much.
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let mut p = libc::pollfd {
                fd: libc::STDIN_FILENO,
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: one valid pollfd.
            let rc = unsafe { libc::poll(&mut p, 1, 100) };
            if rc > 0 && p.revents & (libc::POLLHUP | libc::POLLERR) != 0 {
                break;
            }
            assert!(Instant::now() < deadline, "no hangup");
        }
        let first = match ctx.poll_event(Duration::from_millis(100)) {
            Err(_) => "error".to_string(),
            Ok(e) => format!("{:?}", e.as_ref().map(desc)),
        };
        let mut empty = gibson::event_source::ScriptedSource::from_events(Vec::<Event>::new());
        let second = match ctx.poll_from(&mut empty, Duration::ZERO) {
            Ok(Some(e)) => desc(&e),
            other => format!("{other:?}"),
        };
        std::fs::write(log, format!("POLL {first}\nFROM {second}\n")).unwrap();
        // Nothing left to restore onto, and nowhere to print the harness's summary.
        std::process::exit(0);
    }
    if role == "caught-panic" {
        ctx.session.set_key_release_reporting(true).unwrap();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            panic!("caught, on the owner thread");
        }));
        assert!(caught.is_err());
        let on = ctx.session.key_release_reporting_enabled();
        say(&format!("AFTER-PANIC enabled={on}"));
        ctx.session.set_key_release_reporting(true).unwrap();
        let on = ctx.session.key_release_reporting_enabled();
        say(&format!("REENABLED enabled={on}"));
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            let Some(e) = ctx.poll_event(Duration::from_millis(100)).unwrap() else {
                continue;
            };
            if matches!(e, Event::Resize(..) | Event::Focus(_)) {
                continue;
            }
            say(&format!("EV {}", desc(&e)));
            if matches!(&e, Event::Key(k) if k.code == KeyCode::Char('q')) {
                break;
            }
        }
        ctx.restore().unwrap();
        say("DONE");
        return;
    }
    if role == "disable" {
        ctx.session.set_key_release_reporting(true).unwrap();
        ctx.session.set_key_release_reporting(false).unwrap();
        say("DISABLED");
    } else if role != "off" {
        ctx.session.set_key_release_reporting(true).unwrap();
        assert!(ctx.session.key_release_reporting_enabled());
    }
    say("READY");
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        let Some(e) = ctx.poll_event(Duration::from_millis(100)).unwrap() else {
            continue;
        };
        if matches!(e, Event::Resize(..) | Event::Focus(_)) {
            continue;
        }
        say(&format!("EV {}", desc(&e)));
        match &e {
            Event::Key(k) if k.code == KeyCode::Char('q') => break,
            Event::Key(k) if k.code == KeyCode::Char('!') && role == "panic" => {
                panic!("injected panic with key-release reporting on")
            }
            Event::Key(k) if k.code == KeyCode::Char('!') && role == "caught-panic-poll" => {
                let caught = std::panic::catch_unwind(|| panic!("caught, on the owner thread"));
                assert!(caught.is_err());
                // The hook left raw mode; take it back without touching the session's reporting
                // state, so the bytes typed next arrive as they are sent.
                crossterm::terminal::enable_raw_mode().unwrap();
                say("CAUGHT");
            }
            Event::Key(k) if k.code == KeyCode::Char('a') && role == "inline" => {
                ctx.session.enter_alternate_screen().unwrap();
                say("ALT");
            }
            _ => {}
        }
    }
    ctx.restore().unwrap();
    say("DONE");
}
