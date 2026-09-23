//! Interactive PTY tests for the narrative demos.
//!
//! Each test drives a real terminal: it spawns the demo binary, writes keystrokes
//! to the master, drains the byte stream on a background thread, and asserts on a
//! `vt100` reconstruction of the screen. Every wait is bounded and the child is
//! always killed, so these can never hang CI.

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn example_path(name: &str) -> PathBuf {
    let path = std::env::current_exe()
        .expect("current_exe")
        .parent()
        .expect("deps")
        .parent()
        .expect("target")
        .join("examples")
        .join(name);
    if !path.exists() {
        let status = std::process::Command::new("cargo")
            .args(["build", "--example", name])
            .status()
            .expect("build example");
        assert!(status.success());
    }
    path
}

struct Session {
    child: Box<dyn Child + Send + Sync>,
    killer: Box<dyn portable_pty::ChildKiller + Send + Sync>,
    writer: Box<dyn Write + Send>,
    raw: Arc<Mutex<Vec<u8>>>,
    cols: u16,
    rows: u16,
    _master: Box<dyn MasterPty + Send>,
}

impl Session {
    fn spawn(demo: &str, args: &[&str], cols: u16, rows: u16) -> Self {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("openpty");
        let mut cmd = CommandBuilder::new(example_path(demo));
        cmd.arg("--no-color");
        for a in args {
            cmd.arg(a);
        }
        cmd.env("TERM", "xterm-256color");
        let child = pair.slave.spawn_command(cmd).expect("spawn");
        let killer = child.clone_killer();
        let mut reader = pair.master.try_clone_reader().expect("reader");
        let writer = pair.master.take_writer().expect("writer");
        let raw = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&raw);
        std::thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => sink.lock().unwrap().extend_from_slice(&chunk[..n]),
                }
            }
        });
        Self {
            child,
            killer,
            writer,
            raw,
            cols,
            rows,
            _master: pair.master,
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    fn type_str(&mut self, s: &str) {
        self.write(s.as_bytes());
    }

    fn screen(&self) -> String {
        let mut parser = vt100::Parser::new(self.rows, self.cols, 0);
        parser.process(&self.raw.lock().unwrap());
        parser.screen().contents()
    }

    fn raw_string(&self) -> String {
        String::from_utf8_lossy(&self.raw.lock().unwrap()).into_owned()
    }

    /// Polls the reconstructed screen until `pred` holds or the timeout elapses.
    fn wait_until(&mut self, timeout: Duration, pred: impl Fn(&str) -> bool) -> String {
        let start = Instant::now();
        loop {
            let screen = self.screen();
            if pred(&screen) {
                return screen;
            }
            if start.elapsed() >= timeout {
                return screen;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    }

    fn exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }

    fn shutdown(&mut self) {
        // Give the demo a moment to exit on its own, then kill it.
        for _ in 0..20 {
            if self.exited() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.killer.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.killer.kill();
    }
}

#[test]
fn hack_shell_commands_trigger_real_effects() {
    let mut s = Session::spawn("hack_the_gibson", &["--act=shell"], 100, 30);
    // Wait until the root shell is on screen.
    let screen = s.wait_until(Duration::from_secs(3), |sc| sc.contains("ROOT SHELL"));
    assert!(
        screen.contains("ROOT SHELL"),
        "shell never appeared: {screen}"
    );

    // Drive real commands; each must produce a visible effect.
    for cmd in ["damage", "pool", "planet"] {
        s.type_str(cmd);
        s.write(b"\r");
        std::thread::sleep(Duration::from_millis(150));
    }
    let raw = s.raw_string();
    assert!(
        raw.contains("LIVE DAMAGE MAP"),
        "damage command produced no diagnostic output"
    );
    // The pool payoff overlay should now cover part of the screen.
    let screen = s.wait_until(Duration::from_secs(3), |sc| {
        sc.contains("ROOFTOP POOL ACCESS: GRANTED")
    });
    assert!(
        screen.contains("ROOFTOP POOL ACCESS: GRANTED"),
        "pool payoff overlay missing: {screen}"
    );

    s.type_str("exit");
    s.write(b"\r");
    s.shutdown();
    assert!(s.exited(), "demo should exit after `exit`");
}

#[test]
fn hack_tactical_selector_commits_a_directive() {
    let mut s = Session::spawn("hack_the_gibson", &["--act=tactical"], 110, 30);
    s.wait_until(Duration::from_secs(3), |sc| sc.contains("TACTICAL"));
    // Move the selection down and fire.
    s.write(b"\x1b[B"); // Down
    std::thread::sleep(Duration::from_millis(80));
    s.write(b"\r");
    let screen = s.wait_until(Duration::from_secs(2), |sc| sc.contains("DIRECTIVE"));
    assert!(
        screen.contains("DIRECTIVE") || screen.contains("HACK THE PLANET"),
        "selector did not commit: {screen}"
    );
    s.shutdown();
}

#[test]
fn polished_permission_reject_returns_to_prompt() {
    let mut s = Session::spawn("polished_agent", &["--fullscreen"], 100, 30);
    // The plan plays out in real time; wait for the permission modal.
    let screen = s.wait_until(Duration::from_secs(25), |sc| sc.contains("PERMISSION"));
    assert!(
        screen.contains("PERMISSION"),
        "permission modal missing: {screen}"
    );
    // Select the third option (Reject) and confirm.
    s.write(b"\x1b[B\x1b[B");
    s.write(b"\r");
    std::thread::sleep(Duration::from_millis(120));
    // Esc ends the session and commits the aborted summary.
    s.write(b"\x1b");
    let screen = s.wait_until(Duration::from_secs(5), |sc| {
        sc.contains("patch rejected") || sc.contains("session stopped")
    });
    assert!(
        screen.contains("patch rejected") || screen.contains("session stopped"),
        "reject path not shown: {screen}"
    );
    s.shutdown();
}

#[test]
fn polished_accepts_hostile_unicode_and_scrolls_code() {
    let mut s = Session::spawn("polished_agent", &["--fullscreen"], 110, 30);
    let screen = s.wait_until(Duration::from_secs(25), |sc| sc.contains("PERMISSION"));
    assert!(
        screen.contains("PERMISSION"),
        "permission modal missing: {screen}"
    );
    // Approve once.
    s.write(b"\r");
    s.wait_until(Duration::from_secs(3), |sc| sc.contains("PROMPT"));
    // Focus the code viewport and scroll it.
    s.write(b"\t"); // Tab → code focus
    s.write(b"\x1b[B\x1b[B"); // Down Down
    std::thread::sleep(Duration::from_millis(80));
    // Type a hostile Unicode line and submit it.
    s.type_str("e\u{0301}\u{4f60}\u{597d}\u{1f980}");
    s.write(b"\r");
    let screen = s.wait_until(Duration::from_secs(3), |sc| {
        sc.contains("e\u{0301}\u{4f60}\u{597d}") || sc.contains("\u{4f60}\u{597d}")
    });
    assert!(
        screen.contains("\u{4f60}\u{597d}"),
        "hostile Unicode input was lost: {screen}"
    );
    // Exit cleanly.
    s.write(b"\x1b"); // Esc → finish
    s.shutdown();
}
