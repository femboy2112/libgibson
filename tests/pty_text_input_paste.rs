//! RXV Gate A — issue #252 on a REAL pseudo-terminal.
//!
//! `resize_test_app` is a real inline editor: `Context::run_once` enters raw mode and bracketed
//! paste, crossterm parses the PTY byte stream into `Event::Paste`, and `TextInputState` edits the
//! buffer the painter draws. Here a hostile bracketed paste — TAB, an OSC title set, an erase-display
//! CSI and a C1 DSR — goes through that whole path at 40×15 with `NO_COLOR` (monochrome), and is
//! compared against a CONTROL session that pastes the payload's already-sanitized form. The two
//! sessions must leave the same visible input row and the same terminal side-effect profile, and the
//! hostile session's complete output stream must contain no OSC introducer, no BEL and no C1 code.
//! Process exit is observed through the OS wait status, not inferred from bytes.

#![cfg(unix)]

#[path = "common/pty_capture.rs"]
mod pty_capture;

use portable_pty::CommandBuilder;
use std::time::{Duration, Instant};

const READY: &str = "GIBSON_RESIZE_PROBE_READY";
const DONE: &str = "GIBSON_RESIZE_PROBE_DONE";
const COLS: u16 = 40;
const ROWS: u16 = 15;

const HOSTILE: &str = "a\tb\x1b]0;PWNED\x07\x1b[2J\u{9b}6nz";
const SANITIZED: &str = "a b]0;PWNED[2J6nz";

#[derive(Default, Debug, Clone, PartialEq)]
struct Effects {
    bells: usize,
    titles: Vec<Vec<u8>>,
    clipboard: usize,
    unhandled_osc: usize,
    unhandled_char: Vec<char>,
    unhandled_control: Vec<u8>,
    unhandled_csi: Vec<String>,
}

impl vt100::Callbacks for Effects {
    fn audible_bell(&mut self, _: &mut vt100::Screen) {
        self.bells += 1;
    }
    fn set_window_icon_name(&mut self, _: &mut vt100::Screen, t: &[u8]) {
        self.titles.push(t.to_vec());
    }
    fn set_window_title(&mut self, _: &mut vt100::Screen, t: &[u8]) {
        self.titles.push(t.to_vec());
    }
    fn copy_to_clipboard(&mut self, _: &mut vt100::Screen, _: &[u8], _: &[u8]) {
        self.clipboard += 1;
    }
    fn unhandled_char(&mut self, _: &mut vt100::Screen, c: char) {
        self.unhandled_char.push(c);
    }
    fn unhandled_control(&mut self, _: &mut vt100::Screen, b: u8) {
        self.unhandled_control.push(b);
    }
    fn unhandled_csi(
        &mut self,
        _: &mut vt100::Screen,
        i1: Option<u8>,
        _: Option<u8>,
        params: &[&[u16]],
        c: char,
    ) {
        let s = format!("{:?}{:?}{c}", i1.map(char::from), params);
        // The SET of distinct unmodelled sequences is the profile; how many frames carried a
        // sync-update marker is timing, not content.
        if !self.unhandled_csi.contains(&s) {
            self.unhandled_csi.push(s);
        }
    }
    fn unhandled_osc(&mut self, _: &mut vt100::Screen, _: &[&[u8]]) {
        self.unhandled_osc += 1;
    }
}

struct Session {
    capture: pty_capture::Capture,
    parser: vt100::Parser<Effects>,
    processed: usize,
}

impl Session {
    fn spawn() -> Self {
        let mut cmd = CommandBuilder::new(pty_capture::example_path("resize_test_app"));
        cmd.env("TERM", "xterm-256color");
        cmd.env("NO_COLOR", "1");
        let capture = pty_capture::Capture::spawn(cmd, COLS, ROWS, Duration::from_secs(90));
        Self {
            capture,
            parser: vt100::Parser::new_with_callbacks(
                core::num::NonZeroU16::new(ROWS).expect("vt100 size is non-zero"),
                core::num::NonZeroU16::new(COLS).expect("vt100 size is non-zero"),
                200,
                Effects::default(),
            ),
            processed: 0,
        }
    }

    fn pump(&mut self) {
        self.capture
            .collect_for(Duration::from_millis(2))
            .expect("drain PTY");
        let raw = self.capture.raw();
        if raw.len() > self.processed {
            self.parser.process(&raw[self.processed..]);
            self.processed = raw.len();
        }
    }

    fn screen(&mut self) -> String {
        self.pump();
        self.parser.screen().contents()
    }

    fn wait(&mut self, what: &str, timeout: Duration, pred: impl Fn(&mut Self) -> bool) {
        let start = Instant::now();
        while !pred(self) {
            if start.elapsed() > timeout {
                let screen = self.screen();
                self.capture.terminate();
                panic!("timed out waiting for {what}; screen:\n{screen}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn send(&mut self, s: &str) {
        self.capture.write(s.as_bytes()).expect("write PTY input");
    }

    /// The live input row (`❯ …`).
    fn input_row(&mut self) -> String {
        self.screen()
            .lines()
            .find(|l| l.starts_with('❯'))
            .unwrap_or_default()
            .trim_end()
            .to_string()
    }

    /// Runs the whole session: wait for the first frame, type a prefix, paste `payload`, wait
    /// until `expect` is visible, quit, and return (input row, effects, full raw stream).
    fn run(payload: &str, expect: &str) -> (String, Effects, Vec<u8>) {
        let mut s = Self::spawn();
        s.wait("READY", Duration::from_secs(20), |s| {
            s.pump();
            String::from_utf8_lossy(s.capture.raw()).contains(READY)
        });
        s.wait("first frame", Duration::from_secs(10), |s| {
            s.screen().contains("type here...")
        });
        s.send("go:");
        s.wait("typed prefix", Duration::from_secs(10), |s| {
            s.screen().contains("go:")
        });
        s.send(&format!("\x1b[200~{payload}\x1b[201~"));
        let want = format!("❯ go:{expect}");
        s.wait("pasted text", Duration::from_secs(10), |s| {
            s.input_row() == want
        });
        let row = s.input_row();
        s.send("q");
        let start = Instant::now();
        let status = loop {
            if let Some(status) = s.capture.try_wait().expect("wait status") {
                break status;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "editor did not exit after q"
            );
            s.pump();
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(status.success(), "editor exit status: {status:?}");
        s.capture
            .collect_for(Duration::from_millis(200))
            .expect("drain after exit");
        s.pump();
        let raw = s.capture.raw().to_vec();
        assert!(
            String::from_utf8_lossy(&raw).contains(DONE),
            "DONE sentinel missing"
        );
        let screen = s.screen();
        assert!(
            screen.contains(&format!("❯ go:{expect}")),
            "the committed line must carry the same text; screen:\n{screen}"
        );
        (row, s.parser.callbacks().clone(), raw)
    }
}

#[test]
fn hostile_bracketed_paste_on_a_real_pty_matches_its_sanitized_control() {
    let (control_row, control_fx, control_raw) = Session::run(SANITIZED, SANITIZED);
    let (hostile_row, hostile_fx, hostile_raw) = Session::run(HOSTILE, SANITIZED);

    // Same visible input row: the controls were neither drawn nor interpreted.
    assert_eq!(hostile_row, control_row);
    assert_eq!(hostile_row, format!("❯ go:{SANITIZED}"));

    // Same terminal side-effect profile; and the control itself has none of the dangerous kinds.
    assert_eq!(
        hostile_fx, control_fx,
        "a hostile paste must not change what the terminal is asked to do"
    );
    assert!(control_fx.titles.is_empty() && control_fx.clipboard == 0 && control_fx.bells == 0);

    // Byte oracle over the hostile session's complete output stream.
    for (label, raw) in [("control", &control_raw), ("hostile", &hostile_raw)] {
        let text = String::from_utf8_lossy(raw);
        assert!(!text.contains("\x1b]"), "{label}: OSC introducer emitted");
        assert!(!raw.contains(&0x07), "{label}: BEL emitted");
        assert!(!raw.contains(&b'\t'), "{label}: TAB emitted");
        assert!(
            !text.chars().any(|c| ('\u{80}'..='\u{9f}').contains(&c)),
            "{label}: C1 control emitted"
        );
        assert!(
            !text.contains("\x1b[2J"),
            "{label}: an inline editor never erases the display"
        );
        // Monochrome: no foreground/background colour SGR.
        assert!(
            !text.contains("38;") && !text.contains("48;"),
            "{label}: NO_COLOR session emitted colour"
        );
    }
}
