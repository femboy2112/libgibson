//! Issue #403 on a real PTY: a terminal-backed inline `Context` (geometry read from the PTY,
//! `Resize` delivered by SIGWINCH) under the opt-in `InlineResizePolicy::ReclaimLiveRows`
//! re-anchors a full-height live region across resizes.
//!
//! `examples/inline_resize_reanchor_probe.rs` commits one history line, then renders frame `n`
//! (rows `f<n>-r0`..) at full inline height, and frame `n + 1` after each delivered resize.
//!
//! * The PTY test uses `vt100` 0.18 (xterm resize semantics) as the terminal, resized at the
//!   exact byte offset where the PTY was resized, as a real terminal is. It asserts the same
//!   contract as `tests/inline_resize_reanchor.rs`.
//! * The `#[ignore]`d tmux test uses tmux as the terminal, which reflows on width changes.
//!   Run it explicitly: `cargo test --test pty_inline_resize_reanchor -- --ignored`.
#![cfg(unix)]

#[path = "common/pty_capture.rs"]
mod pty_capture;

use core::num::NonZeroU16;
use portable_pty::CommandBuilder;
use std::collections::BTreeSet;
use std::process::Command;
use std::time::{Duration, Instant};

const HISTORY: &str = "SYNTHETIC-HISTORY";
const SEQUENCE: [(u16, u16); 5] = [(80, 24), (40, 15), (120, 40), (79, 23), (80, 24)];

fn nz(v: u16) -> NonZeroU16 {
    NonZeroU16::new(v).expect("non-zero terminal size")
}

fn frame_step(row: &str) -> Option<usize> {
    let row = row.trim_end();
    let rest = row.strip_prefix('f')?;
    let (step, line) = rest.split_once("-r")?;
    line.parse::<u32>().ok()?;
    step.parse().ok()
}

/// From the first row of `current` to the bottom: its rows in order, blanks allowed, nothing else.
fn check_live_region(screen: &[String], current: usize, label: &str) -> Result<usize, String> {
    let first = screen
        .iter()
        .position(|r| frame_step(r) == Some(current))
        .ok_or_else(|| format!("{label}: frame {current} not visible"))?;
    let mut next = 0;
    for row in &screen[first..] {
        let row = row.trim_end();
        if row.is_empty() {
            continue;
        }
        if row != format!("f{current}-r{next}") {
            return Err(format!(
                "{label}: unexpected row {row:?} in or below the live region\n{}",
                screen.join("\n")
            ));
        }
        next += 1;
    }
    Ok(next)
}

struct VtPty {
    cap: pty_capture::Capture,
    vt: vt100::Parser,
    processed: usize,
    cols: u16,
    stranded: BTreeSet<String>,
}

impl VtPty {
    fn spawn(clear: bool) -> Self {
        let mut cmd =
            CommandBuilder::new(pty_capture::example_path("inline_resize_reanchor_probe"));
        cmd.arg("--reclaim");
        if clear {
            cmd.arg("--clear");
        }
        cmd.env("TERM", "xterm-256color");
        Self {
            cap: pty_capture::Capture::spawn(cmd, 40, 15, Duration::from_secs(60)),
            vt: vt100::Parser::new(nz(15), nz(40), 512),
            processed: 0,
            cols: 40,
            stranded: BTreeSet::new(),
        }
    }

    fn pump(&mut self) {
        self.cap
            .collect_for(Duration::from_millis(5))
            .expect("drain PTY");
        let raw = self.cap.raw();
        if raw.len() > self.processed {
            self.vt.process(&raw[self.processed..]);
            self.processed = raw.len();
        }
    }

    fn scrollback(&mut self) -> Vec<String> {
        let cols = self.cols;
        let screen = self.vt.screen_mut();
        screen.set_scrollback(usize::MAX);
        let len = screen.scrollback();
        let mut out = Vec::with_capacity(len);
        for offset in (1..=len).rev() {
            screen.set_scrollback(offset);
            out.push(screen.rows(0, cols).next().unwrap_or_default());
        }
        screen.set_scrollback(0);
        out
    }

    fn visible(&self) -> Vec<String> {
        self.vt.screen().rows(0, self.cols).collect()
    }

    fn resize(&mut self, cols: u16, rows: u16) {
        self.pump();
        let before = self.scrollback().len();
        self.cap.resize(cols, rows);
        self.vt.set_size(nz(rows), nz(cols));
        self.cols = cols;
        let pushed: Vec<String> = self
            .scrollback()
            .into_iter()
            .skip(before)
            .map(|r| r.trim_end().to_owned())
            .filter(|r| frame_step(r).is_some())
            .collect();
        self.stranded.extend(pushed);
    }

    fn wait_frame(&mut self, step: usize, rows: u16) {
        let last = format!("f{step}-r{}", rows - 2);
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            self.pump();
            if self.visible().iter().any(|r| r.trim_end() == last) {
                return;
            }
            assert!(
                Instant::now() < end,
                "frame {step} never completed; screen:\n{}",
                self.visible().join("\n")
            );
        }
    }

    fn check(&mut self, step: usize, label: &str) {
        let vis = self.visible();
        check_live_region(&vis, step, label).unwrap_or_else(|e| panic!("{e}"));
        let mut all = self.scrollback();
        all.extend(vis);
        assert_eq!(
            all.iter().filter(|r| r.trim_end() == HISTORY).count(),
            1,
            "{label}: committed history lost or duplicated\n{}",
            all.join("\n")
        );
        let mut seen = BTreeSet::new();
        for row in &all {
            let row = row.trim_end();
            if frame_step(row).is_some_and(|s| s != step) {
                assert!(
                    self.stranded.contains(row),
                    "{label}: stale row {row:?} was reachable by the renderer but survived\n{}",
                    all.join("\n")
                );
                assert!(
                    seen.insert(row.to_owned()),
                    "{label}: stale row {row:?} duplicated"
                );
            }
        }
    }
}

fn run_pty(clear: bool) {
    let mut t = VtPty::spawn(clear);
    t.wait_frame(0, 15);
    t.check(0, "initial");
    for (i, (cols, rows)) in SEQUENCE.into_iter().enumerate() {
        let step = i + 1;
        t.resize(cols, rows);
        t.wait_frame(step, rows);
        t.check(step, &format!("clear={clear} {cols}x{rows} (frame {step})"));
    }
    t.cap.write(b"q").expect("quit");
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        t.pump();
        if let Ok(Some(status)) = t.cap.try_wait() {
            assert!(status.success(), "probe exited with {status:?}");
            break;
        }
        assert!(Instant::now() < end, "probe did not exit");
    }
}

#[test]
fn pty_inline_resize_keeps_history_and_a_clean_live_region() {
    run_pty(false);
}

#[test]
fn pty_inline_resize_with_clear_live_region_keeps_history() {
    run_pty(true);
}

// ---------------------------------------------------------------------------------------------
// tmux as the terminal (reflows on width changes). Ignored by default: run explicitly.
// ---------------------------------------------------------------------------------------------

struct Tmux {
    socket: String,
}

impl Tmux {
    fn cmd(&self, args: &[&str]) -> std::process::Output {
        Command::new("tmux")
            .args(["-L", &self.socket, "-f", "/dev/null"])
            .args(args)
            .output()
            .expect("run tmux")
    }

    /// Scrollback plus visible screen, oldest first.
    fn capture(&self) -> Vec<String> {
        let out = self.cmd(&["capture-pane", "-p", "-t", "probe", "-S", "-"]);
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn visible(&self) -> Vec<String> {
        let out = self.cmd(&["capture-pane", "-p", "-t", "probe"]);
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn wait_frame(&self, step: usize, rows: u16) {
        let last = format!("f{step}-r{}", rows - 2);
        let end = Instant::now() + Duration::from_secs(10);
        while !self.visible().iter().any(|r| r.trim_end() == last) {
            assert!(
                Instant::now() < end,
                "tmux: frame {step} never completed; screen:\n{}",
                self.visible().join("\n")
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Tmux {
    fn drop(&mut self) {
        let _ = self.cmd(&["kill-server"]);
    }
}

fn run_tmux(clear: bool) {
    if Command::new("tmux").arg("-V").output().is_err() {
        eprintln!("tmux not installed: skipped");
        return;
    }
    let tmux = Tmux {
        socket: format!("gibson403-{}-{clear}", std::process::id()),
    };
    let probe = pty_capture::example_path("inline_resize_reanchor_probe");
    let mut command = format!("{} --reclaim", probe.display());
    if clear {
        command.push_str(" --clear");
    }
    let started = tmux.cmd(&[
        "new-session",
        "-d",
        "-s",
        "probe",
        "-x",
        "40",
        "-y",
        "15",
        &command,
    ]);
    assert!(started.status.success(), "tmux new-session: {started:?}");
    let _ = tmux.cmd(&["set-option", "-t", "probe", "history-limit", "2000"]);
    tmux.wait_frame(0, 15);
    for (i, (cols, rows)) in SEQUENCE.into_iter().enumerate() {
        let step = i + 1;
        let (c, r) = (cols.to_string(), rows.to_string());
        let resized = tmux.cmd(&["resize-window", "-t", "probe", "-x", &c, "-y", &r]);
        assert!(resized.status.success(), "tmux resize-window: {resized:?}");
        tmux.wait_frame(step, rows);
        let label = format!("tmux clear={clear} {cols}x{rows} (frame {step})");
        let vis = tmux.visible();
        check_live_region(&vis, step, &label).unwrap_or_else(|e| panic!("{e}"));
        let all = tmux.capture();
        assert_eq!(
            all.iter().filter(|r| r.trim_end() == HISTORY).count(),
            1,
            "{label}: committed history lost or duplicated\n{}",
            all.join("\n")
        );
        // tmux applies the resize before the probe hears of it, so the rows it pushed into its
        // history can't be told apart from outside: scrollback totals are a receipt only.
        let stale = all
            .iter()
            .filter(|r| frame_step(r).is_some_and(|s| s != step))
            .count();
        let stale_visible = vis
            .iter()
            .filter(|r| frame_step(r).is_some_and(|s| s != step))
            .count();
        eprintln!("{label}: stale rows total={stale} visible={stale_visible}");
        // The discriminator: v0.7.2 leaves one stale live row visible above the region at every
        // step of this sequence in tmux; the bounded re-anchor leaves none. (tmux pushes a
        // cleared screen into its history, so `total` counts erased copies, not visible rows.)
        assert_eq!(
            stale_visible,
            0,
            "{label}: stale live rows visible\n{}",
            vis.join("\n")
        );
    }
    let _ = tmux.cmd(&["send-keys", "-t", "probe", "q"]);
}

#[test]
#[ignore = "drives a real tmux; run explicitly with --ignored"]
fn tmux_inline_resize_keeps_history_and_a_clean_live_region() {
    run_tmux(false);
}

#[test]
#[ignore = "drives a real tmux; run explicitly with --ignored"]
fn tmux_inline_resize_with_clear_live_region_keeps_history() {
    run_tmux(true);
}
