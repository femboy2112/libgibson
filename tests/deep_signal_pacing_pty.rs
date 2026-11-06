//! v0.7 node F — pacing under a slow terminal, measured on a real PTY.
//!
//! DEEP SIGNAL writes each frame synchronously. A reader that drains the PTY slowly makes those
//! writes block, so presentation must drop frames — but the simulation must keep wall-clock pace
//! (35 tics per second), catching up after each slow frame, never slowing down with the
//! terminal. A full-speed reader is the control. This measures a PTY's buffer and a throttled
//! reader, not a real terminal's display rate.
#![cfg(unix)]
#[path = "common/pty_capture.rs"]
mod pty_capture;

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::Read;
use std::time::{Duration, Instant};

#[derive(Debug)]
struct Summary {
    tics: u64,
    presented: u64,
    behind: u64,
    wall_ms: u64,
    bytes: usize,
}

fn field(line: &str, key: &str) -> u64 {
    line.split(&format!("{key}="))
        .nth(1)
        .and_then(|r| r.split_whitespace().next())
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("summary lacks {key}: {line}"))
}

/// Runs the game for `seconds` at 80x24 while a reader drains the PTY in `chunk`-byte reads,
/// sleeping `pause` after each one.
fn run(seconds: u64, chunk: usize, pause: Duration) -> Summary {
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("openpty");
    let mut cmd = CommandBuilder::new(pty_capture::example_path("deep_signal"));
    cmd.args(["--seed", "7", "--max-seconds", &seconds.to_string()]);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    let mut child = pair.slave.spawn_command(cmd).expect("spawn");
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().expect("reader");
    // Keep the view changing: hold "turn right" with terminal-like autorepeat, so most cells
    // differ every frame and frames are large enough to fill the PTY buffer.
    let mut writer = pair.master.take_writer().expect("writer");
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_keys = std::sync::Arc::clone(&stop);
    let keys = std::thread::spawn(move || {
        use std::io::Write;
        while !stop_keys.load(std::sync::atomic::Ordering::Relaxed) {
            if writer
                .write_all(b"d")
                .and_then(|()| writer.flush())
                .is_err()
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    });
    let drain = std::thread::spawn(move || {
        let mut out = Vec::new();
        let mut buf = vec![0u8; chunk];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
            if !pause.is_zero() {
                std::thread::sleep(pause);
            }
        }
        out
    });
    let deadline = Instant::now() + Duration::from_secs(seconds + 30);
    loop {
        if child.try_wait().expect("try_wait").is_some() {
            break;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("deep_signal did not exit");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = keys.join();
    drop(pair.master);
    let out = drain.join().expect("drain thread");
    let text = String::from_utf8_lossy(&out);
    let tail: String = text
        .chars()
        .rev()
        .take(400)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let at = text
        .rfind("deep_signal: tics=")
        .unwrap_or_else(|| panic!("no exit summary in {} bytes; tail: {tail:?}", out.len()));
    let line = text[at..].split(['\r', '\n']).next().unwrap_or_default();
    for key in ["tics", "presented", "frames_behind", "wall_ms"] {
        assert!(
            line.contains(&format!("{key}=")),
            "summary lacks {key}: {line:?}; tail: {tail:?}"
        );
    }
    Summary {
        tics: field(line, "tics"),
        presented: field(line, "presented"),
        behind: field(line, "frames_behind"),
        wall_ms: field(line, "wall_ms"),
        bytes: out.len(),
    }
}

/// Tics a 35 Hz simulation owes over `wall_ms`.
fn owed(wall_ms: u64) -> u64 {
    wall_ms * 35 / 1000
}

#[test]
fn a_slow_reader_drops_frames_but_never_slows_the_simulation() {
    let fast = run(5, 64 * 1024, Duration::ZERO);
    // ~50 KB/s: a changing 80x24 truecolor frame (~15 KB) then takes ~0.3 s (~10 tics) to drain.
    let slow = run(5, 2048, Duration::from_millis(40));
    eprintln!("fast: {fast:?}\nslow: {slow:?}");
    // The scenario must actually pressure the writer: frames large and changing.
    assert!(
        fast.bytes > 200_000,
        "control produced too little output to pressure a PTY: {fast:?}"
    );
    for (name, s) in [("fast", &fast), ("slow", &slow)] {
        let due = owed(s.wall_ms);
        // Never ahead of wall time (fixed timestep), and behind by at most the last frame's
        // catch-up window plus margin (the loop exits at --max-seconds between frames).
        assert!(
            s.tics <= due + 2,
            "{name}: simulation ran ahead of wall time: {s:?}"
        );
        assert!(
            s.tics + 75 >= due * 3 / 4,
            "{name}: simulation fell behind wall time: {} of {due} tics: {s:?}",
            s.tics
        );
    }
    // The control presents most tics; the throttled reader forces dropped presentations.
    assert!(
        fast.presented * 2 >= fast.tics,
        "control should present most tics: {fast:?}"
    );
    assert!(
        slow.presented * 3 < slow.tics,
        "slow reader should drop frames: {slow:?}"
    );
    assert!(
        slow.behind > 0,
        "frames after a blocked write owed several tics: {slow:?}"
    );
    assert!(slow.presented < fast.presented, "{slow:?} vs {fast:?}");
}
