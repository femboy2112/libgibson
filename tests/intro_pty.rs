//! Actual Unix PTY controls and lifecycle checks for the introductory film.
//! These assert semantic screens and restoration, not image goldens or timing FPS.
#![cfg(unix)]

#[path = "common/pty_capture.rs"]
mod pty_capture;

use portable_pty::CommandBuilder;
use std::time::{Duration, Instant};

struct Film {
    capture: pty_capture::Capture,
    parser: vt100::Parser,
    processed: usize,
}

impl Film {
    fn spawn(args: &[&str]) -> Self {
        let mut cmd = CommandBuilder::new(pty_capture::example_path("libgibson_intro"));
        cmd.args(args);
        cmd.env("TERM", "xterm-256color");
        Self {
            capture: pty_capture::Capture::spawn(cmd, 120, 32, Duration::from_secs(30)),
            parser: vt100::Parser::new(32, 120, 0),
            processed: 0,
        }
    }

    fn pump(&mut self, duration: Duration) {
        self.capture.collect_for(duration).expect("drain intro PTY");
        self.parser.process(&self.capture.raw()[self.processed..]);
        self.processed = self.capture.raw().len();
    }

    fn wait_text(&mut self, needle: &str) {
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            self.pump(Duration::from_millis(10));
            if self.parser.screen().contents().contains(needle) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "missing {needle:?} on intro screen:\n{}",
                self.parser.screen().contents()
            );
        }
    }

    fn key(&mut self, bytes: &[u8]) {
        self.capture.write(bytes).expect("send intro input");
    }

    fn assert_frozen(&mut self) -> Vec<u8> {
        // Drain a possible final frame already in flight when the hint arrived.
        self.pump(Duration::from_millis(60));
        let screen = self.parser.screen().contents_formatted();
        let bytes = self.capture.raw().len();
        self.pump(Duration::from_millis(160));
        assert_eq!(self.parser.screen().contents_formatted(), screen);
        assert_eq!(self.capture.raw().len(), bytes, "paused film emitted bytes");
        screen
    }

    fn resize(&mut self, cols: u16, rows: u16) {
        self.pump(Duration::from_millis(10));
        let bytes = self.capture.raw().len();
        self.parser.screen_mut().set_size(rows, cols);
        self.capture.resize(cols, rows);
        let deadline = Instant::now() + Duration::from_secs(4);
        while self.capture.raw().len() == bytes {
            self.pump(Duration::from_millis(10));
            assert!(Instant::now() < deadline, "resize produced no repaint");
        }
        self.wait_text("PAUSED");
    }

    fn assert_restored(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            self.pump(Duration::from_millis(10));
            if let Some(status) = self.capture.try_wait().expect("poll intro") {
                assert!(status.success(), "intro exited unsuccessfully: {status:?}");
                break;
            }
            assert!(Instant::now() < deadline, "intro did not exit voluntarily");
        }
        self.pump(Duration::from_millis(20));
        let raw = String::from_utf8_lossy(self.capture.raw());
        assert!(raw.contains("\x1b[?1049h"), "fullscreen not entered");
        assert!(raw.contains("\x1b[?1049l"), "fullscreen not restored");
        assert!(raw.contains("\x1b[?2004l"), "paste mode not restored");
        assert!(raw.contains("\x1b[0m\x1b[?2026l\x1b[?7h\x1b[?25h"));
        assert!(!self.parser.screen().alternate_screen());
        assert!(!self.parser.screen().hide_cursor());
        assert_eq!(self.capture.termios(), self.capture.initial_termios);
    }
}

#[test]
fn intro_space_pauses_exact_output_then_resumes_and_escape_restores() {
    let mut film = Film::spawn(&["--at=5", "--deterministic", "--color=truecolor"]);
    film.wait_text("AGENT OPERATIONS");
    assert!(!film.parser.screen().contents().contains("space pause"));
    film.key(b" ");
    film.wait_text("PAUSED");
    film.assert_frozen();
    let bytes = film.capture.raw().len();
    film.key(b" ");
    film.pump(Duration::from_millis(200));
    assert!(!film.parser.screen().contents().contains("PAUSED"));
    assert!(film.parser.screen().contents().contains("space pause"));
    assert!(film.capture.raw().len() > bytes);
    film.key(b"\x1b");
    film.assert_restored();
}

#[test]
fn intro_arrow_cues_and_restart_change_semantics_then_ctrl_c_restores() {
    let mut film = Film::spawn(&["--stage=city", "--freeze", "--color=mono"]);
    film.wait_text("PAUSED");
    assert!(!film.parser.screen().contents().contains("AGENT OPERATIONS"));
    film.key(b"\x1b[D"); // Membrane starts with the completed harness as its source.
    film.wait_text("4/4 SEALED");
    film.key(b"\x1b[D"); // Beginning of the harness: its tasks are not complete.
    film.wait_text("0/4 SEALED");
    film.key(b"\x1b[C");
    film.wait_text("4/4 SEALED");
    film.key(b"R");
    film.wait_text("0/4 SEALED");
    assert!(!film.parser.screen().contents().contains("PAUSED"));
    film.key(b"\x03");
    film.assert_restored();
}

#[test]
fn intro_resize_reprojects_frozen_membrane_city_and_earth_without_advancing() {
    for at in ["--at=24", "--at=32", "--at=68"] {
        let mut film = Film::spawn(&[at, "--freeze", "--color=truecolor"]);
        film.wait_text("PAUSED");
        let original = film.assert_frozen();
        for (cols, rows) in [(56, 24), (160, 40), (120, 32)] {
            film.resize(cols, rows);
            film.assert_frozen();
        }
        assert_eq!(
            film.parser.screen().contents_formatted(),
            original,
            "resize changed paused film time/state at {at}"
        );
        film.key(b"\x1b");
        film.assert_restored();
    }
}

#[test]
fn intro_auto_runs_from_harness_through_finale_and_exits_cleanly() {
    let mut film = Film::spawn(&[
        "--auto",
        "--deterministic",
        "--speed=30",
        "--color=ansi256",
        "--seconds=15",
    ]);
    film.wait_text("AGENT OPERATIONS");
    let deadline = Instant::now() + Duration::from_secs(14);
    let mut finale = false;
    loop {
        film.pump(Duration::from_millis(10));
        finale |= film.parser.screen().contents().contains("replay / R");
        if film.capture.try_wait().expect("poll auto film").is_some() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "auto film did not finish its timeline"
        );
    }
    assert!(
        finale,
        "auto film exited without reaching its final semantic label"
    );
    film.assert_restored();
}

#[test]
fn intro_smoke_limit_exits_even_when_visual_time_is_frozen() {
    let mut film = Film::spawn(&["--at=68", "--freeze", "--color=ansi16", "--seconds=0.5"]);
    film.wait_text("PAUSED");
    film.assert_restored();
}

#[test]
fn intro_opening_hints_disappear_and_navigation_restores_them() {
    let mut film = Film::spawn(&["--at=3.8", "--deterministic", "--color=truecolor"]);
    film.wait_text("space pause");
    let deadline = Instant::now() + Duration::from_secs(3);
    while film.parser.screen().contents().contains("space pause") {
        film.pump(Duration::from_millis(20));
        assert!(Instant::now() < deadline, "opening hints never retreated");
    }
    film.key(b"\x1b[C");
    film.wait_text("space pause");
    film.key(b" ");
    film.wait_text("PAUSED");
    film.assert_frozen();
    film.key(b"\x1b");
    film.assert_restored();
}
