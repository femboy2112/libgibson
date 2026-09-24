//! Deterministic PTY resize torture probe.
//!
//! Sequence: start at 120x30, render history + live input, type, resize to
//! 40x12, continue typing, insert an async event above the live region, resize
//! to 100x24, continue typing, exit cleanly. We continuously drain the PTY,
//! synchronize on line sentinels (no fixed sleeps as the only synchronization),
//! and feed the *complete* byte stream through a `vt100` terminal model before
//! asserting on the resulting screen.
//!
//! Readiness for typed text is checked against the virtual screen, not raw
//! bytes: the engine emits a minimal diff, so words can legitimately be written
//! as separate runs separated by cursor-motion sequences.

#![cfg(unix)]

#[path = "common/pty_capture.rs"]
mod pty_capture;

use portable_pty::CommandBuilder;
use std::time::{Duration, Instant};

const READY: &str = "GIBSON_RESIZE_PROBE_READY";
const DONE: &str = "GIBSON_RESIZE_PROBE_DONE";

struct PtyHarness {
    capture: pty_capture::Capture,
    parser: vt100::Parser,
    processed: usize,
    #[cfg(unix)]
    initial_termios: Option<String>,
}

impl PtyHarness {
    fn spawn(cols: u16, rows: u16) -> Self {
        Self::spawn_demo("resize_test_app", &[], cols, rows)
    }

    fn spawn_demo(name: &str, args: &[&str], cols: u16, rows: u16) -> Self {
        let mut cmd = CommandBuilder::new(pty_capture::example_path(name));
        for arg in args {
            cmd.arg(arg);
        }
        cmd.env("TERM", "xterm-256color");
        let capture = pty_capture::Capture::spawn(cmd, cols, rows, Duration::from_secs(90));
        let initial_termios = capture.initial_termios.clone();
        Self {
            capture,
            parser: vt100::Parser::new(rows, cols, 8000),
            processed: 0,
            #[cfg(unix)]
            initial_termios,
        }
    }

    fn send(&mut self, s: &str) {
        self.capture.write(s.as_bytes()).expect("write PTY input");
    }

    fn resize(&mut self, cols: u16, rows: u16) {
        self.pump();
        self.capture.resize(cols, rows);
        self.parser.set_size(rows, cols);
    }

    /// Feeds any new PTY bytes into the virtual terminal model.
    fn pump(&mut self) {
        self.capture
            .collect_for(Duration::from_millis(2))
            .expect("drain PTY");
        let snapshot = self.capture.raw();
        if snapshot.len() > self.processed {
            self.parser.process(&snapshot[self.processed..]);
            self.processed = snapshot.len();
        }
    }

    fn raw(&mut self) -> Vec<u8> {
        self.pump();
        self.capture.raw().to_vec()
    }

    fn screen(&mut self) -> String {
        self.pump();
        self.parser.screen().contents()
    }

    fn wait_raw(&mut self, needle: &str, timeout: Duration) -> bool {
        let start = Instant::now();
        loop {
            if String::from_utf8_lossy(&self.raw()).contains(needle) {
                return true;
            }
            if start.elapsed() > timeout {
                self.capture.terminate();
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_screen(&mut self, needle: &str, timeout: Duration) -> bool {
        let start = Instant::now();
        loop {
            if self.screen().contains(needle) {
                return true;
            }
            if start.elapsed() > timeout {
                self.capture.terminate();
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_exit(&mut self, timeout: Duration) -> bool {
        let start = Instant::now();
        loop {
            match self.capture.try_wait() {
                Ok(Some(status)) => return status.success(),
                Ok(None) => {}
                Err(_) => return false,
            }
            if start.elapsed() > timeout {
                self.capture.terminate();
                return false;
            }
            self.capture
                .collect_for(Duration::from_millis(10))
                .expect("drain on exit");
        }
    }
}

#[test]
fn test_pty_resize_torture_with_assertions() {
    let mut h = PtyHarness::spawn(120, 30);

    assert!(
        h.wait_raw(READY, Duration::from_secs(15)),
        "child never reached READY sentinel; output: {:?}",
        String::from_utf8_lossy(&h.raw())
    );

    // Type initial text; readiness is judged from the virtual screen.
    h.send("Hello world!");
    assert!(
        h.wait_screen("Hello world!", Duration::from_secs(6)),
        "typed text did not render"
    );

    // Resize narrow.
    h.resize(40, 12);
    std::thread::sleep(Duration::from_millis(80));
    h.send(" How are you?");
    assert!(
        h.wait_screen("How are you?", Duration::from_secs(6)),
        "text after narrow resize did not render; screen: {:?}",
        h.screen()
    );

    // Async event inserted above the live region (Ctrl-G).
    h.send("\x07");
    assert!(
        h.wait_screen("ASYNC INSERTION", Duration::from_secs(6)),
        "async insertion did not appear"
    );

    // Resize wide again.
    h.resize(100, 24);
    std::thread::sleep(Duration::from_millis(80));
    h.send(" PTY is resizing.");
    assert!(
        h.wait_screen("PTY is resizing.", Duration::from_secs(6)),
        "text after wide resize did not render; screen: {:?}",
        h.screen()
    );

    // Clean exit.
    h.send("q");
    assert!(
        h.wait_exit(Duration::from_secs(10)),
        "child did not exit after 'q'"
    );
    assert!(
        h.wait_raw(DONE, Duration::from_secs(6)),
        "DONE sentinel missing; terminal may not have been restored"
    );

    let raw = h.raw();
    let text = String::from_utf8_lossy(&raw);

    // --- terminal restoration proof ---
    assert!(
        !text.contains("\u{1b}[?1049h"),
        "inline app must never enter the alternate screen"
    );
    assert!(text.contains("\u{1b}[?25h"), "cursor must be shown on exit");
    assert!(
        text.contains("\u{1b}[?7h"),
        "autowrap (DECAWM) must be restored"
    );
    assert!(
        text.contains("\u{1b}[?2026l"),
        "synchronized update must be closed"
    );
    let opens = text.matches("\u{1b}[?2026h").count();
    let closes = text.matches("\u{1b}[?2026l").count();
    assert!(opens > 0, "expected synchronized update usage");
    assert!(
        closes >= opens,
        "every sync update open must be closed ({opens} vs {closes})"
    );
    assert!(
        text.rfind("\u{1b}[?2026l") > text.rfind("\u{1b}[?2026h"),
        "synchronized update must be closed at exit"
    );

    // --- resize/anchor assertion ---
    let done_line = text
        .lines()
        .find(|l| l.contains(DONE))
        .expect("DONE line")
        .to_string();
    let resyncs: u64 = done_line
        .split("anchor_resyncs=")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .and_then(|s| s.parse().ok())
        .expect("anchor_resyncs metric");
    assert!(
        resyncs > 0,
        "at least one resize must have invalidated/re-established the anchor: {done_line}"
    );

    // --- final virtual screen state ---
    let contents = h.screen();
    assert!(
        contents.contains("Hello world!"),
        "final screen lost typed text"
    );
    assert!(
        contents.contains("How are you?"),
        "final screen lost resized text"
    );
    assert!(
        contents.contains("PTY is resizing."),
        "final screen lost final text"
    );
    assert!(
        contents.contains("ASYNC INSERTION"),
        "final screen lost async insertion"
    );

    let live_header_count = contents.matches("resize probe :: live").count();
    assert!(
        live_header_count <= 4,
        "suspicious duplicated live region ({live_header_count} occurrences)"
    );
}

#[test]
fn acid_graphical_and_feedback_resize_torture_restores_terminal() {
    // Actual moving rasters, not only frozen snapshots. The ending uses the
    // default shot director and returns to its compact postmortem, while the
    // explicit cyber cases retain coverage of the historical RGB projection.
    let cases: [(&str, &[&str], &str); 4] = [
        (
            "acid_vs_crash",
            &[
                "--manual",
                "--visual=cyber",
                "--stage=first-breach",
                "--deterministic",
                "--color=truecolor",
            ],
            "crash >",
        ),
        (
            "acid_vs_crash",
            &[
                "--manual",
                "--visual=cyber",
                "--stage=takeover",
                "--deterministic",
                "--color=truecolor",
            ],
            "crash >",
        ),
        (
            "acid_vs_crash",
            &[
                "--manual",
                "--visual=auto",
                "--stage=crash-win",
                "--deterministic",
                "--speed=20",
                "--color=truecolor",
            ],
            "crash >",
        ),
        (
            "fx_lab",
            &["--deterministic", "--scene=feedback", "--truecolor"],
            "FEEDBACK TRAILS",
        ),
    ];
    for (name, args, marker) in cases {
        let mut h = PtyHarness::spawn_demo(name, args, 120, 32);
        assert!(
            h.wait_screen(marker, Duration::from_secs(3)),
            "initial graphical frame missing: {}",
            h.screen()
        );
        let mut typed = String::new();
        for (cols, rows) in [(56, 24), (80, 24), (160, 40), (120, 32)] {
            let before = h.raw().len();
            h.resize(cols, rows);
            // Let SIGWINCH reach the event reader before sending the next key;
            // readiness below still requires actual reconstructed input.
            std::thread::sleep(Duration::from_millis(60));
            if name == "acid_vs_crash" {
                h.send("r");
                typed.push('r');
                let input = format!("crash > {typed}");
                assert!(
                    h.wait_screen(&input, Duration::from_secs(2)),
                    "control input lost after {cols}x{rows}: {}",
                    h.screen()
                );
            } else {
                // Pump until bytes from a post-resize frame arrive. The scene's
                // readable header and RGB body must survive each actual resize.
                let start = Instant::now();
                while h.raw().len() <= before && start.elapsed() < Duration::from_secs(2) {
                    std::thread::sleep(Duration::from_millis(10));
                }
                assert!(
                    h.wait_screen(marker, Duration::from_secs(2)),
                    "feedback header lost at {cols}x{rows}: {}",
                    h.screen()
                );
            }
            assert!(h.raw().len() > before, "resize emitted no new frame");
            let screen = h.screen();
            assert!(
                screen.contains(marker),
                "semantic island lost after resize: {screen}"
            );
            assert!(screen.lines().count() <= usize::from(rows));
        }
        if args.contains(&"--stage=crash-win") {
            assert!(
                h.wait_screen("SYSTEM SCARS", Duration::from_secs(2)),
                "ending never revealed the cinematic postmortem: {}",
                h.screen()
            );
        }
        let raw = String::from_utf8_lossy(&h.raw()).into_owned();
        assert!(
            raw.contains('▀') && raw.contains("38;2;") && raw.contains("48;2;"),
            "RGB raster path not exercised for {name} {args:?}"
        );
        assert!(!raw.contains("\x1b_G") && !raw.contains("\x1bP") && !raw.contains("1337;File="));
        h.send("\x03");
        assert!(
            h.wait_exit(Duration::from_secs(2)),
            "Ctrl-C failed after graphical resizing"
        );
        assert!(
            h.wait_raw("\x1b[?1049l", Duration::from_secs(1)),
            "alternate screen not restored"
        );
        assert!(
            h.wait_raw(
                "\x1b[0m\x1b[?2026l\x1b[?7h\x1b[?25h",
                Duration::from_secs(1)
            ),
            "terminal cleanup missing"
        );
        h.pump();
        assert!(!h.parser.screen().alternate_screen());
        assert!(!h.parser.screen().hide_cursor());
        #[cfg(unix)]
        if let Some(initial) = &h.initial_termios {
            assert_eq!(
                h.capture.termios(),
                Some(initial.clone()),
                "raw terminal state not restored"
            );
        }
    }
}

#[test]
fn acid_cinematic_shots_resize_without_losing_input_or_terminal_state() {
    for stage in [
        "first-breach",
        "trace",
        "display-intrusion",
        "climax",
        "crash-win",
        "acid-win",
        "stalemate",
    ] {
        let stage_arg = format!("--stage={stage}");
        let mut h = PtyHarness::spawn_demo(
            "acid_vs_crash",
            &[
                "--manual",
                "--deterministic",
                "--color=truecolor",
                &stage_arg,
            ],
            120,
            32,
        );
        assert!(
            h.wait_screen("crash >", Duration::from_secs(3)),
            "{stage} never rendered: {}",
            h.screen()
        );
        let mut typed = String::new();
        for (cols, rows) in [(56, 24), (80, 24), (160, 40), (120, 32)] {
            h.resize(cols, rows);
            std::thread::sleep(Duration::from_millis(60));
            // Lowercase input is ordinary text even in final/ending shots.
            typed.push('r');
            h.send("r");
            assert!(
                h.wait_screen(&format!("crash > {typed}"), Duration::from_secs(2)),
                "{stage} input lost at {cols}x{rows}: {}",
                h.screen()
            );
            let screen = h.screen();
            assert!(
                !screen.contains("LOCAL FABRIC"),
                "{stage} reverted to dashboard at {cols}x{rows}"
            );
            assert!(screen.lines().count() <= usize::from(rows));
        }
        let raw = String::from_utf8_lossy(&h.raw()).into_owned();
        assert!(
            raw.contains('▀') && raw.contains("38;2;") && raw.contains("48;2;"),
            "{stage} did not exercise RGB cells"
        );
        assert!(!raw.contains("\x1b_G") && !raw.contains("\x1bP") && !raw.contains("1337;File="));
        h.send("\x03");
        assert!(
            h.wait_exit(Duration::from_secs(2)),
            "{stage} did not exit cleanly"
        );
        assert!(h.wait_raw("\x1b[?1049l", Duration::from_secs(1)));
        assert!(h.wait_raw(
            "\x1b[0m\x1b[?2026l\x1b[?7h\x1b[?25h",
            Duration::from_secs(1)
        ));
        h.pump();
        assert!(!h.parser.screen().alternate_screen());
        assert!(!h.parser.screen().hide_cursor());
        #[cfg(unix)]
        if let Some(initial) = &h.initial_termios {
            assert_eq!(
                h.capture.termios(),
                Some(initial.clone()),
                "{stage} raw mode not restored"
            );
        }
    }
}
