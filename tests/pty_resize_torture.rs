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

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const READY: &str = "GIBSON_RESIZE_PROBE_READY";
const DONE: &str = "GIBSON_RESIZE_PROBE_DONE";

struct PtyHarness {
    child: Box<dyn portable_pty::Child + Send + Sync>,
    killer: Box<dyn portable_pty::ChildKiller + Send + Sync>,
    writer: Box<dyn Write + Send>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    buf: Arc<Mutex<Vec<u8>>>,
    parser: vt100::Parser,
    processed: usize,
}

impl PtyHarness {
    fn spawn(cols: u16, rows: u16) -> Self {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("openpty");

        let exe_path = std::env::current_exe()
            .expect("current_exe")
            .parent()
            .expect("deps dir")
            .parent()
            .expect("target dir")
            .join("examples")
            .join("resize_test_app");

        if !exe_path.exists() {
            let status = std::process::Command::new("cargo")
                .args(["build", "--example", "resize_test_app"])
                .status()
                .expect("build resize_test_app");
            assert!(status.success());
        }

        let mut cmd = CommandBuilder::new(&exe_path);
        cmd.env("TERM", "xterm-256color");
        let child = pair.slave.spawn_command(cmd).expect("spawn");
        let killer = child.clone_killer();
        let writer = pair.master.take_writer().expect("take_writer");
        let mut reader = pair.master.try_clone_reader().expect("reader");

        let buf = Arc::new(Mutex::new(Vec::new()));
        let buf2 = Arc::clone(&buf);
        std::thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf2.lock().unwrap().extend_from_slice(&chunk[..n]),
                }
            }
        });

        Self {
            child,
            killer,
            writer,
            master: pair.master,
            buf,
            parser: vt100::Parser::new(rows, cols, 8000),
            processed: 0,
        }
    }

    fn send(&mut self, s: &str) {
        self.writer.write_all(s.as_bytes()).expect("write");
        self.writer.flush().expect("flush");
    }

    fn resize(&mut self, cols: u16, rows: u16) {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("resize");
        self.parser.set_size(rows, cols);
    }

    /// Feeds any new PTY bytes into the virtual terminal model.
    fn pump(&mut self) {
        let snapshot = self.buf.lock().unwrap().clone();
        if snapshot.len() > self.processed {
            self.parser.process(&snapshot[self.processed..]);
            self.processed = snapshot.len();
        }
    }

    fn raw(&self) -> Vec<u8> {
        self.buf.lock().unwrap().clone()
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
                let _ = self.killer.kill();
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
                let _ = self.killer.kill();
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_exit(&mut self, timeout: Duration) -> bool {
        let start = Instant::now();
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return true,
                Ok(None) => {}
                Err(_) => return false,
            }
            if start.elapsed() > timeout {
                let _ = self.killer.kill();
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
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
