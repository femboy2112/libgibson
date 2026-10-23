//! Wave 6 — the instrument workbench FLAGSHIP (#144).
//!
//! A domain-neutral, explicitly synthetic interactive instrument built entirely from
//! public `gibson` primitives: a geometric drawing surface over a scalar field, a
//! validated numeric inspector, pan/zoom, a cursor probe with a linked time-series, a
//! transport for a deterministic app-owned evolving field, undo/redo, capture, and
//! graceful degradation to mono / keyboard-only / narrow terminals — with the renderer,
//! input protocol and edit machinery all supplied by the library, none reinvented here.
//!
//! The model + composition live in `workbench.rs`; this file is the two consumers of it:
//!   * `run_interactive` — the real-terminal loop (alt-screen, raw mode, opt-in mouse +
//!     focus reporting leased through the `TerminalSession`), and
//!   * `selfcheck` — a deterministic scripted replay that prints digests, the field /
//!     probe conservation receipts, and a captured still (used by CI/goldens and as a
//!     headless smoke test). It also proves the same trace replays to the same digest.
//!
//! Keys: `n` draw rectangle wall · `l` draw line · `[` / `]` source − / + ·
//! `space` play/pause · `.` / `,` step ± · `0` rewind · `m` mono toggle · `c` capture ·
//! arrows move selection (Shift+arrows pan) · Ctrl-Z / Ctrl-Y undo / redo · `q` quit.
//! Click the canvas to select + probe a cell; drag to move the selection.

use std::io::{self, Write};
use std::time::{Duration, Instant};

use gibson::shape::Point;
use gibson::{input, Context, RenderMode};

mod workbench;
use workbench::{Intent, TransportCmd, Workbench};

fn main() -> io::Result<()> {
    if std::env::args().any(|a| a == "--selfcheck") {
        selfcheck()
    } else {
        run_interactive()
    }
}

/// The real-terminal flagship loop. Acquires the terminal lease + alternate screen via
/// `Context`, opts into mouse + focus reporting (Finding F), and always restores — even
/// on error — through the one-shot `TerminalSession::restore`.
fn run_interactive() -> io::Result<()> {
    let mut ctx = Context::new(RenderMode::Fullscreen)?;
    input::set_mouse_capture(true)?;
    input::set_focus_reporting(true)?;

    let mut wb = Workbench::new();
    let res = run_loop(&mut ctx, &mut wb);

    let _ = input::set_mouse_capture(false);
    let _ = input::set_focus_reporting(false);
    io::stdout().flush().ok();
    ctx.session.restore()?;
    res
}

fn run_loop(ctx: &mut Context, wb: &mut Workbench) -> io::Result<()> {
    let mut last = Instant::now();
    loop {
        let now = Instant::now();
        let dt = now.duration_since(last);
        last = now;
        // The app-owned field advances on its OWN clock, only while playing (L6).
        wb.tick_wall(dt);

        let (w, h) = ctx.session.terminal_size();
        let node = wb.refresh_layout(w, h);
        ctx.set_root(node);
        ctx.render()?;
        wb.note_write();

        if wb.should_quit {
            break;
        }
        if let Some(event) = ctx.poll_event(Duration::from_millis(50))? {
            wb.process(&event);
        }
    }
    Ok(())
}

/// A scripted, deterministic session: prints digests + receipts and proves the same
/// intent trace replays to the same model/command digest (#144 replay determinism).
fn selfcheck() -> io::Result<()> {
    let script = [
        Intent::DrawRect {
            a: Point::new(2.0, 2.0),
            b: Point::new(6.0, 5.0),
        },
        Intent::SetSource(8.0),
        Intent::Transport(TransportCmd::Step(5)),
        Intent::ProbeAt(Point::new(3.0, 3.0)),
        Intent::SelectAt(Point::new(3.0, 3.0)),
        Intent::MoveSelected { dx: 2.0, dy: 1.0 },
        Intent::SetSource(99.0), // out of range -> refused, model unchanged
        Intent::Undo,
        Intent::Redo,
        Intent::Capture,
    ];

    let run_once = || {
        let mut wb = Workbench::new();
        let _ = wb.refresh_layout(80, 24);
        for it in script.iter().copied() {
            wb.apply_intent(it);
        }
        wb
    };

    let wb = run_once();
    let replay = run_once();

    let out = io::stdout();
    let mut o = out.lock();
    writeln!(o, "workbench selfcheck")?;
    writeln!(o, "model_digest={:016x}", wb.digest())?;
    writeln!(o, "command_digest={:016x}", wb.command_digest)?;
    writeln!(
        o,
        "replay_matches={}",
        wb.digest() == replay.digest() && wb.command_digest == replay.command_digest
    )?;

    let frame = wb.render_field(wb.canvas_rect());
    let r = &frame.report;
    writeln!(
        o,
        "field receipt: cells={} value={} masked={} nonfinite={} off_field={} clipped={} conserves={}",
        r.cells_in_area,
        r.value_cells,
        r.masked_cells,
        r.nonfinite_cells,
        r.off_field_cells,
        r.clipped_cells,
        r.conserves(),
    )?;

    let series = wb.probe_series(Point::new(3.0, 3.0), 8);
    writeln!(o, "linked series points={}", series.len())?;

    writeln!(o, "--- captured still ---")?;
    write!(o, "{}", wb.capture_still())?;
    Ok(())
}
