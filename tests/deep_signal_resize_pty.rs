//! v0.7 node G (game level) — resize during play, on a real PTY.
//!
//! The terminal is resized while a turn is held. After each resize settles, the frame must
//! fill the NEW geometry: the 3D view paints every view row (no rows left blank or stale from
//! the previous size), and the HUD sits on the last two rows, in its wide or compact form.
#![cfg(unix)]
#[path = "common/pty_capture.rs"]
mod pty_capture;

use portable_pty::CommandBuilder;
use pty_capture::{example_path, Capture};
use std::time::Duration;

fn screen(raw: &[u8], cols: u16, rows: u16) -> vt100::Parser {
    let mut p = vt100::Parser::new(rows, cols, 0);
    p.process(raw);
    p
}

/// Feeds held-turn input for `ms` while collecting output.
fn turn_for(cap: &mut Capture, ms: u64) {
    for _ in 0..(ms / 40) {
        cap.write(b"d").unwrap();
        cap.collect_for(Duration::from_millis(40)).unwrap();
    }
}

#[test]
fn every_resize_repaints_the_whole_new_geometry() {
    let mut cmd = CommandBuilder::new(example_path("deep_signal"));
    cmd.args(["--seed", "7", "--max-seconds", "30"]);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    let mut cap = Capture::spawn(cmd, 80, 24, Duration::from_secs(45));
    cap.collect_until(|o| screen(o, 80, 24).screen().contents().contains("SHARDS"))
        .expect("first frame");
    // One terminal model, resized at the exact byte offset where the PTY was resized - as a
    // real terminal is - so each check sees what a user would.
    let mut term = vt100::Parser::new(24, 80, 0);
    let mut consumed = 0;
    for (cols, rows) in [(120u16, 40u16), (40, 15), (100, 30), (80, 24)] {
        term.process(&cap.raw()[consumed..]);
        consumed = cap.raw().len();
        cap.resize(cols, rows);
        term.screen_mut().set_size(rows, cols);
        turn_for(&mut cap, 800);
        term.process(&cap.raw()[consumed..]);
        consumed = cap.raw().len();
        let p = &term;
        let s = p.screen();
        let lines: Vec<String> = s.rows(0, cols).collect();
        let view_rows = usize::from(rows - 2);
        let blank_view_rows: Vec<usize> = (0..view_rows)
            .filter(|&r| lines[r].trim().is_empty())
            .collect();
        // Colour half-blocks are "▀" glyphs: every view row of a painted frame has ink.
        assert!(
            blank_view_rows.is_empty(),
            "{cols}x{rows}: blank view rows {blank_view_rows:?}\n{}",
            s.contents()
        );
        let hud = &lines[view_rows];
        let wide = cols >= 60;
        assert!(
            if wide {
                hud.contains("SHARDS") && hud.contains("SECTOR")
            } else {
                hud.contains("S1/3")
            },
            "{cols}x{rows}: HUD not on row {view_rows}: {hud:?}"
        );
        assert!(
            lines.iter().all(|l| l.chars().count() <= usize::from(cols)),
            "{cols}x{rows}: a line exceeds the width"
        );
    }
    cap.write(b"q").unwrap();
    // Wait for the game's own exit (finish() would SIGKILL a child still running).
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        cap.collect_for(Duration::from_millis(50)).unwrap();
        if let Some(status) = cap.try_wait().unwrap() {
            assert!(status.success(), "{status:?}");
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "deep_signal did not exit on q"
        );
    }
    cap.collect_for(Duration::from_millis(100)).unwrap();
    term.process(&cap.raw()[consumed..]);
    let _ = cap.finish();
    assert!(
        !term.screen().alternate_screen(),
        "alternate screen left on exit"
    );
}
