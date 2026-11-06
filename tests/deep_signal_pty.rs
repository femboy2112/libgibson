//! Deep Signal (v0.7 node E) — a real-PTY playthrough at 80×24 colour and 40×15 ASCII.
#![cfg(unix)]
#[path = "common/pty_capture.rs"]
mod pty_capture;

use portable_pty::CommandBuilder;
use pty_capture::{example_path, Capture};
use std::time::{Duration, Instant};

fn parse(raw: &[u8], cols: u16, rows: u16) -> vt100::Parser {
    let mut p = vt100::Parser::new(rows, cols, 0);
    p.process(raw);
    p
}

fn spawn(cols: u16, rows: u16, extra: &[&str]) -> Capture {
    let mut cmd = CommandBuilder::new(example_path("deep_signal"));
    cmd.args(["--seed", "7", "--max-seconds", "30"]);
    cmd.args(extra);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    Capture::spawn(cmd, cols, rows, Duration::from_secs(45))
}

/// Presses `key` and then "autorepeats" it every 30 ms for `repeats` times.
fn hold(cap: &mut Capture, key: &[u8], repeats: u32) {
    for _ in 0..repeats {
        cap.write(key).unwrap();
        cap.collect_for(Duration::from_millis(30)).unwrap();
    }
}

fn quit_and_reap(cap: &mut Capture) -> portable_pty::ExitStatus {
    cap.write(b"q").unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        cap.collect_for(Duration::from_millis(50)).unwrap();
        if let Some(status) = cap.try_wait().unwrap() {
            cap.collect_for(Duration::from_millis(100)).unwrap();
            return status;
        }
        assert!(Instant::now() < deadline, "deep_signal did not exit on q");
    }
}

/// The view rows (everything above the 2-line HUD), formatted (glyphs + SGR).
fn view_rows(p: &vt100::Parser, cols: u16, rows: u16) -> Vec<Vec<u8>> {
    p.screen()
        .rows_formatted(0, cols)
        .take(usize::from(rows - 2))
        .collect()
}

/// Non-blank cells in the view rows.
fn view_ink(p: &vt100::Parser, cols: u16, rows: u16) -> usize {
    p.screen()
        .rows(0, cols)
        .take(usize::from(rows - 2))
        .map(|r| r.chars().filter(|c| !c.is_whitespace()).count())
        .sum()
}

fn summary_tics(raw: &[u8]) -> u64 {
    let text = String::from_utf8_lossy(raw);
    let line = text
        .lines()
        .find(|l| l.contains("deep_signal: tics="))
        .expect("exit summary");
    line.split("tics=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn deep_signal_plays_and_restores_the_terminal_at_80x24() {
    let mut cap = spawn(80, 24, &[]);
    cap.collect_until(|o| parse(o, 80, 24).screen().contents().contains("SHARDS"))
        .expect("HUD appears");
    let first = parse(cap.raw(), 80, 24);
    assert!(
        view_ink(&first, 80, 24) > 80 * 22 * 9 / 10,
        "the 3D view is painted"
    );
    let before = view_rows(&first, 80, 24);
    hold(&mut cap, b"w", 45);
    hold(&mut cap, b"d", 15);
    cap.collect_for(Duration::from_millis(1200)).unwrap();
    let screen = parse(cap.raw(), 80, 24);
    assert!(
        screen.screen().alternate_screen(),
        "fullscreen play uses the alternate screen"
    );
    assert_ne!(
        view_rows(&screen, 80, 24),
        before,
        "moving changed the 3D view itself"
    );
    let hud = screen.screen().contents();
    assert!(hud.contains("SECTOR 1/3"), "{hud}");
    assert!(
        !hud.contains("00:00"),
        "the simulation clock advanced: {hud}"
    );
    let status = quit_and_reap(&mut cap);
    assert!(status.success(), "{status:?}");
    let raw = cap.finish();
    let end = parse(&raw, 80, 24);
    assert!(
        !end.screen().alternate_screen(),
        "alternate screen left on exit"
    );
    assert!(summary_tics(&raw) > 35, "at least one second of simulation");
}

#[test]
fn deep_signal_ascii_fallback_fits_40x15_without_colour() {
    let mut cap = spawn(40, 15, &["--ascii"]);
    cap.collect_until(|o| parse(o, 40, 15).screen().contents().contains("S1/3"))
        .expect("compact HUD appears");
    let first = parse(cap.raw(), 40, 15);
    assert!(
        view_ink(&first, 40, 15) > 40 * 13 / 2,
        "the ASCII view is painted"
    );
    let before = view_rows(&first, 40, 15);
    hold(&mut cap, b"w", 20);
    cap.collect_for(Duration::from_millis(600)).unwrap();
    let screen = parse(cap.raw(), 40, 15);
    assert_ne!(
        view_rows(&screen, 40, 15),
        before,
        "moving changed the ASCII view"
    );
    let text = screen.screen().contents();
    assert!(text.contains("S1/3"), "{text}");
    assert!(
        text.lines().all(|l| l.chars().count() <= 40),
        "nothing wraps past 40 columns"
    );
    let status = quit_and_reap(&mut cap);
    assert!(status.success());
    let raw = cap.finish();
    let s = String::from_utf8_lossy(&raw);
    assert!(
        !s.contains("38;2;") && !s.contains("48;2;"),
        "monochrome emits no RGB colour"
    );
    assert!(!parse(&raw, 40, 15).screen().alternate_screen());
}
