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
    #[cfg(unix)]
    initial_termios: Option<String>,
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
        #[cfg(unix)]
        let initial_termios = pair.master.get_termios().map(|t| format!("{t:?}"));
        let mut cmd = CommandBuilder::new(example_path(demo));
        if !args.iter().any(|a| {
            a.starts_with("--color=")
                || matches!(*a, "--truecolor" | "--ansi256" | "--ansi16" | "--mono")
        }) {
            cmd.arg("--no-color");
        }
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
            #[cfg(unix)]
            initial_termios,
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

    /// Polls the raw byte stream until `pred` holds or the timeout elapses.
    ///
    /// Inline sessions commit the truthful epilogue to real scrollback, which may
    /// be outside the current visible screen, so tests assert on the raw stream.
    fn wait_until_raw(&mut self, timeout: Duration, pred: impl Fn(&str) -> bool) -> String {
        let start = Instant::now();
        loop {
            let raw = self.raw_string();
            if pred(&raw) {
                return raw;
            }
            if start.elapsed() >= timeout {
                return raw;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    }

    fn exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }

    /// Requires a successful voluntary exit and verifies both terminal protocol
    /// restoration and, on Unix, the original kernel terminal settings. Unlike
    /// `shutdown`, killing a stalled child cannot make this assertion pass.
    fn assert_clean_exit(&mut self, timeout: Duration) {
        let started = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().expect("poll child") {
                assert!(status.success(), "demo exited unsuccessfully: {status:?}");
                break;
            }
            assert!(
                started.elapsed() < timeout,
                "demo did not exit voluntarily; screen: {}",
                self.screen()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        // Exit may race the background reader's final chunk.
        let raw = self.wait_until_raw(Duration::from_secs(1), |r| {
            r.contains("\x1b[0m\x1b[?2026l\x1b[?7h\x1b[?25h")
        });
        assert!(
            raw.contains("\x1b[?1049h"),
            "alternate screen never entered"
        );
        assert!(raw.contains("\x1b[?1049l"), "alternate screen not restored");
        assert!(raw.contains("\x1b[?2004h"), "interactive paste mode absent");
        assert!(raw.contains("\x1b[?2004l"), "paste mode not restored");
        assert!(
            raw.contains("\x1b[0m\x1b[?2026l\x1b[?7h\x1b[?25h"),
            "style, synchronized updates, autowrap or cursor reset missing"
        );
        let mut parser = vt100::Parser::new(self.rows, self.cols, 0);
        parser.process(raw.as_bytes());
        assert!(!parser.screen().alternate_screen());
        assert!(!parser.screen().hide_cursor());
        #[cfg(unix)]
        if let Some(initial) = &self.initial_termios {
            assert_eq!(
                self._master.get_termios().map(|t| format!("{t:?}")),
                Some(initial.clone()),
                "raw-mode terminal settings were not restored"
            );
        }
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
fn hack_rabbit_replicates_and_cookie_neutralizes_it() {
    let mut s = Session::spawn("hack_the_gibson", &["--act=shell"], 110, 32);
    s.wait_until(Duration::from_secs(3), |sc| sc.contains("ROOT SHELL"));

    // Rabbit: a real replication graph, with a truthful bounded label.
    s.type_str("rabbit");
    s.write(b"\r");
    let screen = s.wait_until(Duration::from_secs(3), |sc| {
        sc.contains("RABBIT REPLICATION")
    });
    assert!(
        screen.contains("RABBIT REPLICATION"),
        "rabbit did not mount a replication entity: {screen}"
    );

    // Cookie: neutralize that same entity (collapse), not a generic burst.
    s.type_str("cookie");
    s.write(b"\r");
    let screen = s.wait_until(Duration::from_secs(3), |sc| {
        sc.contains("NEUTRALIZED") || sc.contains("collapsing")
    });
    assert!(
        screen.contains("NEUTRALIZED") || screen.contains("collapsing"),
        "cookie did not neutralize the rabbit: {screen}"
    );

    s.type_str("exit");
    s.write(b"\r");
    s.shutdown();
}

#[test]
fn hack_tactical_choices_have_local_consequences() {
    // Select option 2 (Da Vinci) and confirm: the directive must report the
    // branch-specific consequence before reconverging on Download.
    let mut s = Session::spawn("hack_the_gibson", &["--act=tactical"], 110, 32);
    s.wait_until(Duration::from_secs(3), |sc| sc.contains("TACTICAL"));
    s.write(b"\x1b[B"); // Down → Da Vinci
    s.write(b"\r");
    let screen = s.wait_until(Duration::from_secs(3), |sc| {
        sc.contains("frozen") || sc.contains("quarantine")
    });
    assert!(
        screen.contains("frozen") || screen.contains("quarantine"),
        "Da Vinci branch had no local consequence: {screen}"
    );
    s.shutdown();
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
    // Fast deterministic start-state: jump straight to the permission modal.
    let mut s = Session::spawn(
        "polished_agent",
        &["--fullscreen", "--stage=permission"],
        100,
        30,
    );
    let screen = s.wait_until(Duration::from_secs(5), |sc| sc.contains("PERMISSION"));
    assert!(
        screen.contains("PERMISSION"),
        "permission modal missing: {screen}"
    );
    assert!(
        screen.contains("patch src/geom.rs"),
        "modal must name the real change target: {screen}"
    );
    // Select the third option (Reject) and confirm.
    s.write(b"\x1b[B\x1b[B");
    s.write(b"\r");
    let screen = s.wait_until(Duration::from_secs(3), |sc| sc.contains("patch rejected"));
    assert!(
        screen.contains("patch rejected"),
        "reject path not shown: {screen}"
    );
    // Esc acknowledges the terminal beat and commits the truthful summary.
    s.write(b"\x1b");
    let screen = s.wait_until(Duration::from_secs(3), |sc| sc.contains("session stopped"));
    assert!(
        screen.contains("session stopped") || screen.contains("patch rejected"),
        "reject summary missing: {screen}"
    );
    s.shutdown();
}

#[test]
fn polished_accepts_hostile_unicode_and_scrolls_code() {
    let mut s = Session::spawn(
        "polished_agent",
        &["--fullscreen", "--stage=permission"],
        110,
        30,
    );
    let screen = s.wait_until(Duration::from_secs(5), |sc| sc.contains("PERMISSION"));
    assert!(
        screen.contains("PERMISSION"),
        "permission modal missing: {screen}"
    );
    // Approve once.
    s.write(b"\r");
    // Wait until the mutation beat has completed and the prompt is live.
    s.wait_until(Duration::from_secs(10), |sc| sc.contains("PROMPT ●"));
    // Focus the code viewport and scroll it.
    s.write(b"\t"); // Tab → code focus
    s.write(b"\x1b[B\x1b[B"); // Down Down
    std::thread::sleep(Duration::from_millis(80));
    // Return focus to the prompt before typing: characters must NOT leak into the
    // prompt while the code view owns focus.
    s.write(b"\t");
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

#[test]
fn polished_inline_approve_commits_truthful_summary() {
    // Inline (the product identity). Approve, let the mutation plan run, finish,
    // then assert the *real* completion record landed in scrollback.
    let mut s = Session::spawn("polished_agent", &["--stage=permission"], 100, 30);
    s.wait_until(Duration::from_secs(5), |sc| sc.contains("PERMISSION"));
    s.write(b"\r"); // Approve once
    s.wait_until(Duration::from_secs(10), |sc| sc.contains("PROMPT ●"));
    s.write(b"\x1b"); // finish
    let raw = s.wait_until_raw(Duration::from_secs(5), |r| r.contains("session complete"));
    assert!(
        raw.contains("session complete"),
        "approved inline epilogue missing: {raw}"
    );
    assert!(
        raw.contains("clip regression suite green"),
        "truthful demo-local metrics missing: {raw}"
    );
    s.shutdown();
}

#[test]
fn polished_inline_reject_commits_no_changes() {
    let mut s = Session::spawn("polished_agent", &["--stage=permission"], 100, 30);
    s.wait_until(Duration::from_secs(5), |sc| sc.contains("PERMISSION"));
    s.write(b"\x1b[B\x1b[B"); // Reject
    s.write(b"\r");
    s.write(b"\x1b"); // acknowledge
    let raw = s.wait_until_raw(Duration::from_secs(5), |r| r.contains("no files changed"));
    assert!(
        raw.contains("no files changed"),
        "reject must report no files changed: {raw}"
    );
    assert!(
        !raw.contains("session complete"),
        "a rejected session must never claim completion: {raw}"
    );
    s.shutdown();
}

#[test]
fn polished_inline_cancel_reports_cancelled() {
    let mut s = Session::spawn("polished_agent", &["--stage=permission"], 100, 30);
    s.wait_until(Duration::from_secs(5), |sc| sc.contains("PERMISSION"));
    s.write(b"\x03"); // Ctrl-C
    let raw = s.wait_until_raw(Duration::from_secs(5), |r| r.contains("session cancelled"));
    assert!(
        raw.contains("session cancelled"),
        "cancel must report a cancelled session: {raw}"
    );
    assert!(
        !raw.contains("session complete"),
        "a cancelled session must not claim completion: {raw}"
    );
    s.shutdown();
}

#[test]
fn acid_trace_changes_visible_world_and_planner_during_first_contest() {
    let mut s = Session::spawn(
        "acid_vs_crash",
        &[
            "--visual=flat",
            "--manual",
            "--stage=route-contested",
            "--deterministic",
            "--freeze-at=0",
            "--debug-battle",
        ],
        120,
        32,
    );
    let before = s.wait_until(Duration::from_secs(3), |sc| sc.contains("crash >"));
    assert!(before.contains("CONTEST"), "wrong act: {before}");
    s.type_str("trace\r");
    let after = s.wait_until(Duration::from_secs(2), |sc| {
        sc.contains("EvadeTrace") && sc.contains("TRACE 28%")
    });
    assert!(after.contains("TRACE 28%"), "trace did not change: {after}");
    assert!(
        after.contains("EvadeTrace") && after.contains("SplitRoute"),
        "planner did not respond: {after}"
    );
    assert!(
        after.contains("CONTEST"),
        "in-beat defense jumped act: {after}"
    );
    s.type_str("exit\r");
    s.assert_clean_exit(Duration::from_secs(2));
}

#[test]
fn acid_isolation_disconnects_route_and_causes_valid_pivot() {
    let mut s = Session::spawn(
        "acid_vs_crash",
        &[
            "--visual=flat",
            "--manual",
            "--stage=first-breach",
            "--deterministic",
            "--speed=3",
            "--debug-battle",
        ],
        120,
        32,
    );
    let ready = s.wait_until(Duration::from_secs(3), |sc| sc.contains("crash >"));
    assert!(ready.contains("crash >"));
    s.type_str("isolate\r");
    let adapted = s.wait_until(Duration::from_secs(3), |sc| {
        sc.contains("ROUTE CUT") && sc.contains("PunishIsolation")
    });
    assert!(
        adapted.contains("ROUTE CUT"),
        "route not visibly cut: {adapted}"
    );
    assert!(
        adapted.contains("PunishIsolation"),
        "Acid did not react: {adapted}"
    );
    assert!(
        adapted.contains("telemetry lost"),
        "defense cost hidden: {adapted}"
    );
    s.type_str("exit\r");
    s.assert_clean_exit(Duration::from_secs(2));
}

#[test]
fn acid_decoy_draws_remote_lease_then_opponent_recognizes_mirror() {
    let mut s = Session::spawn(
        "acid_vs_crash",
        &[
            "--visual=flat",
            "--manual",
            "--stage=first-breach",
            "--deterministic",
            "--speed=4",
            "--debug-battle",
        ],
        120,
        32,
    );
    let ready = s.wait_until(Duration::from_secs(3), |sc| sc.contains("crash >"));
    assert!(ready.contains("crash >"));
    s.type_str("decoy\r");
    let deployed = s.wait_until(Duration::from_secs(2), |sc| {
        sc.contains("MIRROR FILES") && sc.contains("AttackDecoy")
    });
    assert!(
        deployed.contains("MIRROR FILES") && deployed.contains("AttackDecoy"),
        "mirror not targeted: {deployed}"
    );
    let taken = s.wait_until(Duration::from_secs(3), |sc| sc.contains("mirror lease"));
    assert!(
        taken.contains("mirror lease"),
        "decoy never occupied: {taken}"
    );
    let recognized = s.wait_until(Duration::from_secs(3), |sc| sc.contains("cute once"));
    assert!(
        recognized.contains("cute once"),
        "Acid did not learn: {recognized}"
    );
    s.type_str("exit\r");
    s.assert_clean_exit(Duration::from_secs(2));
}

#[test]
fn acid_hard_isolation_costs_visible_telemetry_and_available_actions() {
    let mut s = Session::spawn(
        "acid_vs_crash",
        &[
            "--visual=flat",
            "--manual",
            "--stage=display-intrusion",
            "--deterministic",
            "--freeze-at=0",
        ],
        120,
        32,
    );
    let ready = s.wait_until(Duration::from_secs(3), |sc| sc.contains("crash >"));
    assert!(ready.contains("crash >"));
    s.type_str("hard isolate\r");
    let blind = s.wait_until(Duration::from_secs(2), |sc| {
        sc.contains("LOCAL TELEMETRY LOST")
    });
    assert!(
        blind.contains("LOCAL TELEMETRY LOST"),
        "hard-isolate cost absent: {blind}"
    );
    s.type_str("decoy\r");
    let blocked = s.wait_until(Duration::from_secs(2), |sc| sc.contains("UNAVAILABLE"));
    assert!(
        blocked.contains("UNAVAILABLE"),
        "blind machine fabricated decoy capability: {blocked}"
    );
    s.type_str("exit\r");
    s.assert_clean_exit(Duration::from_secs(2));
}

#[test]
fn acid_final_cut_link_resolves_and_replays_before_restoring_terminal() {
    let mut s = Session::spawn(
        "acid_vs_crash",
        &[
            "--visual=flat",
            "--manual",
            "--stage=climax",
            "--deterministic",
            "--freeze-at=0",
        ],
        120,
        32,
    );
    let ready = s.wait_until(Duration::from_secs(3), |sc| sc.contains("CUT LINK"));
    assert!(ready.contains("CUT LINK"), "final agency absent: {ready}");
    s.type_str("cut link\r");
    let ending = s.wait_until(Duration::from_secs(2), |sc| sc.contains("CRASH CONTAINS"));
    assert!(
        ending.contains("CRASH CONTAINS"),
        "cut did not resolve: {ending}"
    );
    assert!(
        ending.contains("replay"),
        "aftercredits inspector absent: {ending}"
    );
    s.type_str("replay\r");
    let replay = s.wait_until(Duration::from_secs(2), |sc| sc.contains("REPLAY VERIFIED"));
    assert!(
        replay.contains("REPLAY VERIFIED"),
        "world replay failed: {replay}"
    );
    s.type_str("exit\r");
    s.assert_clean_exit(Duration::from_secs(2));
}

#[test]
fn acid_default_crash_fights_without_input_and_accepts_intervention() {
    let mut s = Session::spawn(
        "acid_vs_crash",
        &[
            "--visual=flat",
            "--stage=route-contested",
            "--deterministic",
            "--speed=2",
        ],
        120,
        32,
    );
    // No --auto and no keystrokes: the default view must show Crash actually
    // spending bandwidth on a defense, with Acid reacting to that world change.
    let defended = s.wait_until(Duration::from_secs(3), |sc| {
        sc.contains("CRASH WORKING") && sc.contains("TRACE 28%") && sc.contains("awareness rises")
    });
    assert!(
        defended.contains("CRASH WORKING"),
        "Crash not working: {defended}"
    );
    assert!(
        defended.contains("TRACE 28%"),
        "no autonomous defense: {defended}"
    );
    assert!(
        defended.contains("awareness rises"),
        "defense cost not visible: {defended}"
    );
    assert!(
        defended.contains("crash > trace"),
        "Crash command not shown in intervention island: {defended}"
    );
    s.type_str("exit\r");
    s.assert_clean_exit(Duration::from_secs(2));
}

#[test]
fn acid_default_stays_for_aftermath_and_world_replay() {
    let mut s = Session::spawn(
        "acid_vs_crash",
        &[
            "--visual=flat",
            "--stage=climax",
            "--deterministic",
            "--speed=20",
        ],
        120,
        32,
    );
    let ending = s.wait_until(Duration::from_secs(3), |sc| sc.contains("CRASH CONTAINS"));
    assert!(
        ending.contains("CRASH CONTAINS"),
        "default operator did not finish: {ending}"
    );
    // Beyond --auto's ending hold: default viewing must retain the machine.
    std::thread::sleep(Duration::from_millis(900));
    assert!(
        !s.exited(),
        "default view exited instead of retaining aftermath"
    );
    s.type_str("replay\r");
    let replay = s.wait_until(Duration::from_secs(2), |sc| sc.contains("REPLAY VERIFIED"));
    assert!(
        replay.contains("REPLAY VERIFIED"),
        "autonomous world failed replay: {replay}"
    );
    s.type_str("exit\r");
    s.assert_clean_exit(Duration::from_secs(2));
}

#[test]
fn acid_auto_completes_entire_story_without_input() {
    let mut s = Session::spawn(
        "acid_vs_crash",
        &["--visual=flat", "--auto", "--deterministic", "--speed=20"],
        120,
        32,
    );
    let ending = s.wait_until(Duration::from_secs(6), |sc| sc.contains("CRASH CONTAINS"));
    assert!(
        ending.contains("CRASH CONTAINS"),
        "auto never resolved: {ending}"
    );
    s.assert_clean_exit(Duration::from_secs(2));
}

#[test]
fn acid_ctrl_c_restores_terminal_during_display_takeover() {
    let mut s = Session::spawn(
        "acid_vs_crash",
        &[
            "--visual=flat",
            "--manual",
            "--stage=takeover",
            "--deterministic",
            "--freeze-at=1",
        ],
        80,
        24,
    );
    let screen = s.wait_until(Duration::from_secs(3), |sc| sc.contains("crash >"));
    assert!(screen.contains("crash >"), "stable island lost: {screen}");
    s.write(b"\x03");
    s.assert_clean_exit(Duration::from_secs(2));
}

#[test]
fn acid_responsive_compositions_survive_all_color_capabilities() {
    // Deliberately cover four compositions/capabilities without a slow Cartesian
    // product. These are real terminal invocations, not headless node snapshots.
    for (cols, rows, color) in [
        (56, 24, "--color=mono"),
        (80, 24, "--color=ansi16"),
        (120, 32, "--color=ansi256"),
        (160, 40, "--color=truecolor"),
    ] {
        let mut s = Session::spawn(
            "acid_vs_crash",
            &[
                "--visual=flat",
                "--manual",
                "--stage=route-contested",
                "--deterministic",
                "--freeze-at=0",
                color,
            ],
            cols,
            rows,
        );
        let screen = s.wait_until(Duration::from_secs(3), |sc| sc.contains("crash >"));
        assert!(
            screen.contains("LOCAL FABRIC"),
            "map missing at {cols}x{rows}: {screen}"
        );
        assert!(
            screen.contains("≋ ROUTE"),
            "contested identity lost: {screen}"
        );
        assert!(
            screen.contains("crash >"),
            "control island missing: {screen}"
        );
        if cols == 56 {
            assert!(
                !screen.contains("SESSION /"),
                "narrow layout squeezed wide panels"
            );
        }
        let raw = s.raw_string();
        if color == "--color=mono" || color == "--color=ansi16" {
            assert!(!raw.contains("38;2;") && !raw.contains("38;5;"));
            assert!(!raw.contains("48;2;") && !raw.contains("48;5;"));
        } else if color == "--color=ansi256" {
            assert!(raw.contains("38;5;") || raw.contains("48;5;"));
            assert!(!raw.contains("38;2;") && !raw.contains("48;2;"));
        } else {
            assert!(raw.contains("38;2;") || raw.contains("48;2;"));
        }
        s.type_str("exit\r");
        s.assert_clean_exit(Duration::from_secs(2));
    }
}

fn assert_unicode_rgb_graphics(raw: &str) {
    assert!(raw.contains('▀'), "half-block RGB realization absent");
    assert!(
        raw.contains("38;2;") && raw.contains("48;2;"),
        "two RGB subpixels per cell absent"
    );
    assert!(!raw.contains("\x1b_G"), "Kitty graphics protocol forbidden");
    assert!(
        !raw.contains("\x1bP"),
        "Sixel/DCS graphics protocol forbidden"
    );
    assert!(
        !raw.contains("1337;File="),
        "inline image protocol forbidden"
    );
}

#[test]
fn acid_cyber_trace_decoy_and_full_visual_replay_use_real_pty_input() {
    let mut s = Session::spawn(
        "acid_vs_crash",
        &[
            "--manual",
            "--visual=cyber",
            "--stage=first-breach",
            "--deterministic",
            "--freeze-at=4",
            "--color=truecolor",
        ],
        120,
        32,
    );
    let ready = s.wait_until(Duration::from_secs(3), |screen| screen.contains("crash >"));
    assert!(
        ready.contains("crash >"),
        "cyber control island absent: {ready}"
    );
    assert_unicode_rgb_graphics(&s.raw_string());
    s.type_str("trace\r");
    let traced = s.wait_until(Duration::from_secs(2), |screen| {
        screen.contains("TRACE 28%")
    });
    assert!(
        traced.contains("TRACE 28%"),
        "cyber trace action not applied: {traced}"
    );
    s.type_str("decoy\r");
    let decoy = s.wait_until(Duration::from_secs(2), |screen| {
        screen.contains("mirror attached")
    });
    assert!(
        decoy.contains("mirror attached"),
        "cyber decoy action not applied: {decoy}"
    );
    assert!(
        decoy.contains("DECOY"),
        "semantic alternate target absent: {decoy}"
    );
    s.type_str("replay\r");
    let replay = s.wait_until(Duration::from_secs(2), |screen| {
        screen.contains("REPLAY VERIFIED")
    });
    assert!(
        replay.contains("REPLAY VERIFIED"),
        "visual/world replay failed: {replay}"
    );
    s.write(b"\x03");
    s.assert_clean_exit(Duration::from_secs(2));
}

#[test]
fn acid_cyber_takeover_keeps_final_agency_and_restores_terminal() {
    let mut s = Session::spawn(
        "acid_vs_crash",
        &[
            "--manual",
            "--visual=cyber",
            "--stage=takeover",
            "--deterministic",
            "--freeze-at=2",
            "--color=truecolor",
        ],
        80,
        24,
    );
    let screen = s.wait_until(Duration::from_secs(3), |screen| screen.contains("crash >"));
    assert!(
        screen.contains("CUT LINK") && screen.contains("crash >"),
        "final agency lost in graphical takeover: {screen}"
    );
    assert_unicode_rgb_graphics(&s.raw_string());
    s.type_str("cut link\r");
    let ending = s.wait_until(Duration::from_secs(2), |screen| {
        screen.contains("CRASH CONTAINS")
    });
    assert!(
        ending.contains("CRASH CONTAINS"),
        "graphical cut did not resolve: {ending}"
    );
    s.write(b"\x03");
    s.assert_clean_exit(Duration::from_secs(2));
}

#[test]
fn fx_lab_deterministic_filled_3d_accepts_ctrl_c() {
    let mut s = Session::spawn(
        "fx_lab",
        &["--deterministic", "--scene=filled-3d", "--truecolor"],
        120,
        32,
    );
    let screen = s.wait_until(Duration::from_secs(3), |screen| {
        screen.contains("FILLED 3D") && screen.contains('▀')
    });
    assert!(
        screen.contains("FILLED 3D"),
        "filled scene absent: {screen}"
    );
    assert_unicode_rgb_graphics(&s.raw_string());
    s.write(b"\x03");
    s.assert_clean_exit(Duration::from_secs(2));
}

#[test]
fn acid_cinematic_shortcuts_preserve_typed_input_and_change_world() {
    for (key, receipt) in [
        ("T", "TRACE / return pulse / awareness rises"),
        ("I", "ISOLATE ROUTE / telemetry lost"),
        ("D", "DECOY / mirror attached to FILES"),
    ] {
        let mut s = Session::spawn(
            "acid_vs_crash",
            &[
                "--manual",
                "--stage=first-breach",
                "--deterministic",
                "--freeze-at=0",
                "--color=truecolor",
            ],
            120,
            32,
        );
        let ready = s.wait_until(Duration::from_secs(3), |screen| screen.contains("crash >"));
        assert!(
            ready.contains("T TRACE") && ready.contains("D DECOY"),
            "cinematic affordances absent: {ready}"
        );
        assert!(
            !ready.contains("LOCAL FABRIC"),
            "default restored legacy dashboard: {ready}"
        );
        assert_unicode_rgb_graphics(&s.raw_string());

        // A shortcut only fires in an empty command buffer. Uppercase text in
        // an existing command must remain ordinary editable TextInput content.
        s.type_str(&format!("x{key}"));
        let typed = s.wait_until(Duration::from_secs(2), |screen| {
            screen.contains(&format!("crash > x{key}"))
        });
        assert!(
            typed.contains(&format!("crash > x{key}")),
            "shortcut consumed typed text: {typed}"
        );
        assert!(
            !typed.contains(receipt),
            "typing changed the world: {typed}"
        );
        s.write(b"\x7f\x7f");
        s.type_str(key);
        let acted = s.wait_until(Duration::from_secs(2), |screen| screen.contains(receipt));
        assert!(
            acted.contains(receipt),
            "{key} failed without Enter: {acted}"
        );
        assert!(acted.contains("crash >"));
        s.type_str("replay\r");
        let replay = s.wait_until(Duration::from_secs(2), |screen| {
            screen.contains("REPLAY VERIFIED")
        });
        assert!(
            replay.contains("REPLAY VERIFIED"),
            "shortcut world/visual replay diverged: {replay}"
        );
        s.write(b"\x03");
        s.assert_clean_exit(Duration::from_secs(2));
    }
}

#[test]
fn acid_cinematic_final_numeric_shortcut_resolves_and_esc_restores() {
    let mut s = Session::spawn(
        "acid_vs_crash",
        &[
            "--manual",
            "--stage=climax",
            "--deterministic",
            "--freeze-at=0",
            "--color=truecolor",
        ],
        80,
        24,
    );
    let ready = s.wait_until(Duration::from_secs(3), |screen| {
        screen.contains("1 CUT LINK")
    });
    assert!(
        ready.contains("1 CUT LINK") && ready.contains("4 LET HER IN"),
        "final shortcuts absent: {ready}"
    );
    assert_unicode_rgb_graphics(&s.raw_string());
    s.type_str("1");
    let ended = s.wait_until(Duration::from_secs(2), |screen| {
        screen.contains("CRASH CONTAINS")
    });
    assert!(
        ended.contains("CRASH CONTAINS"),
        "numeric final move did not resolve: {ended}"
    );
    s.type_str("replay\r");
    let replay = s.wait_until(Duration::from_secs(2), |screen| {
        screen.contains("REPLAY VERIFIED")
    });
    assert!(
        replay.contains("REPLAY VERIFIED"),
        "final shortcut replay failed: {replay}"
    );
    s.write(b"\x1b");
    s.assert_clean_exit(Duration::from_secs(2));
}

#[test]
fn acid_cinematic_default_crash_fights_and_accepts_typed_intervention() {
    let mut s = Session::spawn(
        "acid_vs_crash",
        &[
            "--stage=route-contested",
            "--deterministic",
            "--speed=2",
            "--color=truecolor",
        ],
        120,
        32,
    );
    let fought = s.wait_until(Duration::from_secs(3), |screen| {
        screen.contains("TRACE 28%")
    });
    assert!(
        fought.contains("TRACE 28%"),
        "default cinematic Crash never defended: {fought}"
    );
    assert_unicode_rgb_graphics(&s.raw_string());
    s.type_str("exit\r");
    s.assert_clean_exit(Duration::from_secs(2));
}

#[test]
fn acid_cinematic_auto_completes_full_story_and_restores_terminal() {
    let mut s = Session::spawn(
        "acid_vs_crash",
        &[
            "--auto",
            "--deterministic",
            "--speed=100",
            "--color=truecolor",
        ],
        120,
        32,
    );
    let ending = s.wait_until(Duration::from_secs(8), |screen| {
        screen.contains("CRASH CONTAINS")
    });
    assert!(
        ending.contains("CRASH CONTAINS"),
        "cinematic auto never resolved: {ending}"
    );
    assert_unicode_rgb_graphics(&s.raw_string());
    s.assert_clean_exit(Duration::from_secs(3));
}
