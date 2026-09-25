//! First-contact prologue: the movie's origin story.
//!
//! The intent (see the round mandate): a viewer starts in an ORDINARY terminal,
//! watches a credible agent-harness boot sequence, sees simple terminal output
//! progressively give way to a structured LibGibson live-region UI, and arrives
//! at the same agent-operations state the cinematic's Harness act opens on — at
//! which point ownership is handed cleanly to the fullscreen film at its
//! existing internal `t = 0`. The 72-second film's timeline is NOT shifted; this
//! is a prelude with its own bounded clock.
//!
//! Mechanics:
//!   * PHASE A — a boot log committed to IMMUTABLE SCROLLBACK via `commit_*`
//!     (fg-only styling, no forced background, so it blends with the user's real
//!     terminal). This text is the "leave useful session text behind" payoff: it
//!     remains in the primary buffer's scrollback after the film's alternate
//!     screen is torn down.
//!   * PHASE B — a mutable LIVE REGION (`set_root` + `render`) that assembles the
//!     harness framing (title, objective, the four agents coming online) and
//!     animates until ARCHITECT is active and the rest are queued — exactly the
//!     film's `t = 0` state.
//!   * HANDOFF — `clear_live_region()` then `restore()` releases the process
//!     terminal lease (see `session.rs`). The caller then constructs a fullscreen
//!     `Context`, whose `TerminalSession::new()` re-acquires the lease. Two
//!     contexts never own the terminal at once (issue #11); the engine's lease
//!     enforces it — a leaked lease would make the film's `fullscreen()` fail.
//!
//! Every scripted beat is a pure function of the prelude clock: no wall-clock
//! branching, no randomness. `--deterministic` advances a fixed synthetic clock
//! so the sequence is reproducible for PTY/restoration tests.

use super::identity::IDENTITIES;
use super::model::AGENTS;
use gibson::cell::{Color, Line, RichText, Span, Style};
use gibson::input::{Event, KeyCode, KeyModifiers};
use gibson::node::Node;
use gibson::{ColorDepth, Context};
use std::io;
use std::time::{Duration, Instant};

// Palette shared with harness.rs so the live UI is visually continuous with the
// film's Harness act. Foreground-only (no forced background) keeps the boot log
// native to the user's terminal.
const INK: Color = Color::Rgb(207, 220, 237);
const DIM: Color = Color::Rgb(101, 124, 150);
const CYAN: Color = Color::Rgb(76, 218, 244);
const OK: Color = Color::Rgb(120, 224, 168);
const RULE: Color = Color::Rgb(32, 50, 69);

/// When PHASE A finishes and the live region attaches (prelude seconds).
const LIVE_START: f32 = 4.35;
/// When the prelude ends and control hands off to the film.
const PRELUDE_END: f32 = 8.0;

/// What the caller should do after the prologue returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Proceed into the fullscreen film (normal completion or Space-to-skip).
    Proceed,
    /// The viewer asked to quit (Esc / Ctrl-C); do not run the film.
    Quit,
}

/// One scheduled boot-log line: the prelude time it appears and its styled spans.
type BootLine = (f32, Vec<(&'static str, Color)>);

/// The credible agent-harness boot sequence. Deterministic and ordered; each
/// line is committed to scrollback when the prelude clock passes its time.
/// The four workers are the film's canonical agents (same names/roles), brought
/// online with their identity glyphs and colors so the handoff is a match cut.
fn boot_schedule() -> Vec<BootLine> {
    let mut s: Vec<BootLine> = vec![
        (
            0.15,
            vec![
                ("libgibson-harness", CYAN),
                ("  0.1.0  (engineering alpha)", DIM),
            ],
        ),
        (
            0.45,
            vec![("· workspace     ", DIM), ("/home/leah/LibGibson", INK)],
        ),
        (
            0.72,
            vec![
                ("· repository    ", DIM),
                ("git ", INK),
                ("d2d9146", CYAN),
                ("  ·  clean worktree", DIM),
            ],
        ),
        (
            0.98,
            vec![
                ("· worktree      ", DIM),
                ("1 root · 0 submodules · 247 tracked units", INK),
            ],
        ),
        (
            1.24,
            vec![
                ("· runtime       ", DIM),
                (
                    "deterministic clock · seed 0xC0FFEE · no wall-clock drift",
                    INK,
                ),
            ],
        ),
        (
            1.52,
            vec![
                ("· tool registry ", DIM),
                ("read · write · search · shell · render", INK),
                ("  (5)", DIM),
            ],
        ),
        (
            1.82,
            vec![
                ("· acceptance    ", DIM),
                ("Build a resilient transit planner", INK),
            ],
        ),
        (
            2.04,
            vec![
                ("                ", DIM),
                ("4 ordered contracts loaded from PLAN.md", DIM),
            ],
        ),
        (
            2.34,
            vec![
                ("· coordinator   ", DIM),
                ("online", OK),
                ("  ·  queue depth 0", DIM),
            ],
        ),
        (2.62, vec![("· allocating workers", DIM), (" ...", DIM)]),
    ];
    // The four workers coming online — canonical identities + roles.
    let mut t = 2.92;
    for (i, a) in AGENTS.iter().enumerate() {
        let id = IDENTITIES[i];
        s.push((
            t,
            vec![
                ("   ", DIM),
                (id.signature, id.color()),
                (leak(format!(" {:<9}", a.name)), id.color()),
                (leak(format!(" {:<22}", a.role)), DIM),
                ("online", OK),
            ],
        ));
        t += 0.26;
    }
    s.push((
        t + 0.10,
        vec![
            ("· plan          ", DIM),
            ("executing", OK),
            ("  ·  task 1/4  ", DIM),
            ("ARCHITECT", IDENTITIES[0].color()),
        ],
    ));
    s.push((t + 0.34, vec![("", DIM)]));
    s.push((
        t + 0.50,
        vec![
            ("libgibson", CYAN),
            (
                " › live region attached — telemetry moving to structured UI ↓",
                DIM,
            ),
        ],
    ));
    s
}

/// Leaks a `String` to `&'static str`. The boot schedule is built once per run;
/// a handful of small permanent allocations is an acceptable price for uniform
/// `&'static str` spans, and this is example (not library) code.
fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// Plain-text preview of the scheduled boot log (for inspection and tests).
pub fn boot_lines_plain() -> Vec<String> {
    boot_schedule()
        .into_iter()
        .map(|(_, parts)| parts.iter().map(|(s, _)| *s).collect::<String>())
        .collect()
}

/// The prelude length in seconds (its own bounded clock; does not affect the
/// film's internal 72-second timeline).
pub fn prelude_seconds() -> f32 {
    PRELUDE_END
}

fn boot_rich(parts: &[(&str, Color)]) -> RichText {
    RichText::new().line(Line::from_spans(
        parts
            .iter()
            .map(|(s, c)| Span::styled(*s, Style::new().fg(*c)))
            .collect(),
    ))
}

fn styled_line(parts: &[(&str, Color)]) -> Node {
    Node::line(Line::from_spans(
        parts
            .iter()
            .map(|(s, c)| Span::styled(*s, Style::new().fg(*c)))
            .collect(),
    ))
    .height(1.0)
}

fn text(s: impl Into<String>, color: Color) -> Node {
    Node::text(s, Style::new().fg(color)).height(1.0)
}

/// The live-region UI at prelude time `t`. Assembles the harness framing and
/// animates ARCHITECT to "active" while the rest stay "queued" — the film's
/// `t = 0` state. Pure function of `t` and `width`.
pub fn live_ui(t: f32, width: u16) -> Node {
    // Local phase in [0, 1] across PHASE B.
    let q = ((t - LIVE_START) / (PRELUDE_END - LIVE_START)).clamp(0.0, 1.0);
    let bar_w = usize::from(width.saturating_sub(34)).clamp(6, 24);

    let mut rows = vec![
        Node::row().children(vec![
            styled_line(&[("G / ", CYAN), ("LIBGIBSON", INK)]).flex_grow(1.0),
            text("AGENT OPERATIONS   /   01", DIM),
        ]),
        styled_line(&[("Build a resilient transit planner.", INK)]),
        Node::rule(None::<String>, Style::new().fg(RULE)),
        text("", DIM),
        styled_line(&[
            ("01  ", CYAN),
            ("ORCHESTRATION", INK),
            ("     PLAN → MAP → BUILD → PROVE", DIM),
        ]),
        text("", DIM),
    ];

    for (i, a) in AGENTS.iter().enumerate() {
        let id = IDENTITIES[i];
        // Only ARCHITECT (index 0) is active during the prelude; matches t=0.
        let active = i == 0;
        let (rail, col, state) = if active {
            ("┃", id.color(), "active")
        } else {
            ("│", DIM, "queued")
        };
        // ARCHITECT's bar creeps forward with the prelude phase; others empty.
        let filled = if active {
            (q * bar_w as f32) as usize
        } else {
            0
        };
        let bar_on = "━".repeat(filled);
        let bar_off = "─".repeat(bar_w.saturating_sub(filled));
        rows.push(styled_line(&[
            (rail, col),
            (leak(format!(" {} {:<10}", id.signature, a.name)), col),
            (leak(format!("{state:<8}")), DIM),
            (leak(bar_on), col),
            (leak(bar_off), RULE),
        ]));
        rows.push(styled_line(&[
            (rail, col),
            (leak(format!("   {}", a.role)), DIM),
        ]));
    }

    rows.push(text("", DIM));
    rows.push(styled_line(&[
        ("⌂ PLAN  ", IDENTITIES[0].color()),
        ("◇ MAP  ", IDENTITIES[1].color()),
        ("▥ BUILD  ", IDENTITIES[2].color()),
        ("⊞ PROVE", IDENTITIES[3].color()),
    ]));
    rows.push(styled_line(&[
        ("◈  ", CYAN),
        ("orchestrator › contracts in flight", INK),
    ]));

    Node::col().width(width as f32).children(rows)
}

/// Runs the prologue on an inline context, then restores it (releasing the
/// terminal lease). `deterministic` advances a fixed synthetic clock (for
/// non-interactive / test runs); otherwise real time paces playback and
/// Space skips to the film, Esc / Ctrl-C quits.
pub fn run(depth: Option<ColorDepth>, deterministic: bool) -> io::Result<Outcome> {
    let mut ctx = Context::inline()?;
    if let Some(d) = depth {
        ctx.set_color_depth(d);
    }
    ctx.set_max_fps(60);

    let schedule = boot_schedule();
    let start = Instant::now();
    let step_dt = 1.0 / 60.0_f32;
    let mut synth = 0.0_f32;
    let mut emitted = 0usize;
    let mut outcome = Outcome::Proceed;

    loop {
        let t = if deterministic {
            synth
        } else {
            start.elapsed().as_secs_f32()
        };

        // PHASE A: commit any boot lines whose time has arrived (scrollback).
        while emitted < schedule.len() && schedule[emitted].0 <= t {
            ctx.commit_rich_text(&boot_rich(&schedule[emitted].1))?;
            emitted += 1;
        }

        // PHASE B: once the boot log is complete, animate the live region.
        if emitted >= schedule.len() {
            let (w, _h) = ctx.session.terminal_size();
            ctx.set_root(live_ui(t, w.clamp(40, 200)));
            ctx.render()?;
        }

        if t >= PRELUDE_END {
            break;
        }

        // Input + pacing. On a real TTY we block briefly for events; otherwise
        // (deterministic or redirected) we advance the synthetic clock.
        if deterministic || !ctx.session.is_tty {
            synth += step_dt;
        } else if let Some(Event::Key(k)) = ctx.poll_event(Duration::from_millis(16))? {
            match k.code {
                KeyCode::Esc => {
                    outcome = Outcome::Quit;
                    break;
                }
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                    outcome = Outcome::Quit;
                    break;
                }
                KeyCode::Char(' ') | KeyCode::Enter => break, // skip to the film
                _ => {}
            }
        }
    }

    // Clean handoff: remove the live UI (boot scrollback stays), then release
    // the terminal lease so the film's fullscreen context can acquire it.
    ctx.clear_live_region()?;
    ctx.restore()?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_schedule_is_ordered_and_bounded() {
        let s = boot_schedule();
        assert!(!s.is_empty());
        // Times are non-decreasing.
        for w in s.windows(2) {
            assert!(w[1].0 >= w[0].0, "boot times must be non-decreasing");
        }
        // The whole boot log finishes before the live phase would end.
        let last = s.last().unwrap().0;
        assert!(
            last < PRELUDE_END,
            "boot log ({last}) must fit before PRELUDE_END"
        );
        const { assert!(LIVE_START < PRELUDE_END) };
    }

    #[test]
    fn boot_brings_the_four_canonical_agents_online() {
        let s = boot_schedule();
        let flat: String = s
            .iter()
            .flat_map(|(_, parts)| parts.iter().map(|(t, _)| *t))
            .collect();
        for a in AGENTS.iter() {
            assert!(flat.contains(a.name), "boot log missing agent {}", a.name);
        }
        assert!(flat.contains("online"));
        assert!(flat.contains("Build a resilient transit planner"));
    }

    #[test]
    fn live_ui_builds_at_start_and_end_widths() {
        // Pure builder must not panic across the phase and a range of widths.
        for &w in &[40u16, 80, 120, 200] {
            let _ = live_ui(LIVE_START, w);
            let _ = live_ui((LIVE_START + PRELUDE_END) / 2.0, w);
            let _ = live_ui(PRELUDE_END, w);
        }
    }
}
