//! v0.7 nodes D/H — a focus loss on a real terminal releases DEEP SIGNAL's held keys.
//!
//! Terminals report key presses, not releases, so a held key is inferred from autorepeat
//! (`HoldTracker`). When the window loses focus the repeats stop while the key may still be down;
//! the tracker must release it at once (`ReleaseCause::FocusLost`) instead of waiting out its
//! grace window. That needs focus reporting on (`CSI ?1004h`), enabled through the session so that
//! restore turns it off again (`CSI ?1004l`).
#![cfg(unix)]
#[path = "common/pty_capture.rs"]
mod pty_capture;

use portable_pty::CommandBuilder;
use pty_capture::{example_path, Capture};
use std::time::{Duration, Instant};

fn field<'a>(text: &'a str, key: &str) -> &'a str {
    text.split(&format!("{key}="))
        .nth(1)
        .and_then(|r| r.split_whitespace().next())
        .unwrap_or_else(|| panic!("no {key}= in {text:?}"))
}

fn replay(trace: &std::path::Path) -> String {
    let out = std::process::Command::new(example_path("deep_signal"))
        .arg("--replay")
        .arg(trace)
        .output()
        .expect("run --replay");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    text
}

#[test]
fn a_focus_loss_is_reported_recorded_and_releases_the_held_turn() {
    let trace = std::env::temp_dir().join(format!("deep_signal_focus_{}.tsv", std::process::id()));
    let _ = std::fs::remove_file(&trace);
    let mut cmd = CommandBuilder::new(example_path("deep_signal"));
    cmd.args(["--seed", "7", "--max-seconds", "30", "--record"]);
    cmd.arg(&trace);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    let mut cap = Capture::spawn(cmd, 80, 24, Duration::from_secs(45));
    cap.collect_until(|o| {
        let s = String::from_utf8_lossy(o);
        s.find("SHARDS")
            .is_some_and(|i| s[i..].contains("\x1b[?2026l"))
    })
    .expect("first complete frame");
    let has = |raw: &[u8], seq: &[u8]| raw.windows(seq.len()).any(|w| w == seq);
    assert!(
        has(cap.raw(), b"\x1b[?1004h"),
        "focus reporting enabled while playing"
    );
    // Hold "turn right" (a turn always changes the trajectory; a wall can stop a walk), then
    // lose focus right after a repeat, inside the hold's grace window.
    for _ in 0..20 {
        cap.write(b"d").unwrap();
        cap.collect_for(Duration::from_millis(30)).unwrap();
    }
    cap.write(b"d\x1b[O").unwrap();
    cap.collect_for(Duration::from_millis(400)).unwrap();
    cap.write(b"\x1b[I").unwrap();
    cap.collect_for(Duration::from_millis(100)).unwrap();
    cap.write(b"q").unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        cap.collect_for(Duration::from_millis(50)).unwrap();
        if let Some(status) = cap.try_wait().unwrap() {
            assert!(status.success(), "{status:?}");
            break;
        }
        assert!(Instant::now() < deadline, "deep_signal did not exit on q");
    }
    cap.collect_for(Duration::from_millis(100)).unwrap();
    let raw = cap.finish();
    // Restore turned reporting off after the last time it was turned on.
    let on = raw
        .windows(8)
        .rposition(|w| w == b"\x1b[?1004h")
        .expect("enabled");
    assert!(
        has(&raw[on..], b"\x1b[?1004l"),
        "focus reporting left on after exit"
    );

    let recorded = std::fs::read_to_string(&trace).expect("trace written");
    assert!(
        recorded.lines().any(|l| l.ends_with("\tfocus\tlost")),
        "the focus loss was decoded and recorded:\n{recorded}"
    );
    // The focus loss changed the simulation: without it the turn runs on to the end of its
    // grace window, so the same trace minus that line replays to a different trajectory.
    let with = replay(&trace);
    let without_body: String = recorded
        .lines()
        .filter(|l| !l.ends_with("\tfocus\tlost"))
        .map(|l| format!("{l}\n"))
        .collect();
    let trace2 = trace.with_extension("nofocus.tsv");
    std::fs::write(&trace2, without_body).unwrap();
    let without = replay(&trace2);
    let _ = std::fs::remove_file(&trace2);
    let _ = std::fs::remove_file(&trace);
    assert_ne!(
        field(&with, "path"),
        field(&without, "path"),
        "the recorded focus loss did not release the held turn\nwith:    {with}without: {without}"
    );
}
