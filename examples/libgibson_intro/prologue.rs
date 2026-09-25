//! First-contact prologue: the movie's origin story.
//!
//! The intent (see the round mandate): a viewer starts in an ORDINARY terminal,
//! watches a credible — and explicitly SIMULATED — agent-harness boot sequence,
//! then sees that same information *acquire structure*: a LibGibson live region
//! assembles below the still-streaming boot log, section by section, until it
//! reaches the exact agent-operations pose the cinematic's Harness act opens on.
//! Ownership is then handed cleanly to the fullscreen film at its existing
//! internal `t = 0`. The 72-second film's timeline is NOT shifted; this is a
//! prelude with its own bounded clock.
//!
//! The transformation is deliberately *continuous*, not a snap:
//!   * PHASE A — a boot log committed to IMMUTABLE SCROLLBACK via `commit_*`
//!     (fg-only styling, no forced background, so it blends with the user's real
//!     terminal). This text is the "leave useful session text behind" payoff: it
//!     remains in the primary buffer's scrollback after the film's alternate
//!     screen is torn down.
//!   * PHASE B — a mutable LIVE REGION (`set_root` + `render`) that *attaches
//!     while the boot log is still emitting* (the two coexist for a few seconds)
//!     and then assembles progressively: a header rail, the objective, the
//!     orchestration frame, then the four worker slots arriving ONE AT A TIME in
//!     correspondence with their boot receipts, each resolving from a skeletal
//!     placeholder into a full agent record. ARCHITECT ignites to "active" only
//!     after its slot has cohered and the plan begins executing — and its
//!     progress bar settles at exactly `progress(0)` (an ignited head, no work
//!     yet done), which is precisely the film's `t = 0` state. The rest stay
//!     queued. A short readable hold lets the eye register "this is the same
//!     object" before the cut.
//!   * HANDOFF — `clear_live_region()` then `restore()` releases the process
//!     terminal lease (see `session.rs`). The caller then constructs a fullscreen
//!     `Context`, whose `TerminalSession::new()` re-acquires the lease. Two
//!     contexts never own the terminal at once (issue #11); the engine's lease
//!     enforces it — a leaked lease would make the film's `fullscreen()` fail.
//!
//! A note on the live region's height: the engine grows an inline live region by
//! allocating rows at its bottom (see `renderer.rs`), but has no shrink path
//! between frames. The assembly is therefore MONOTONIC — sections and rows are
//! only ever added, never removed — so the region never shrinks mid-prelude.
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
const MINT: Color = Color::Rgb(166, 246, 215); // VERIFY accent; the "sealed" counter

// ---- Prelude choreography (seconds on the prelude's own bounded clock) -------
//
// These thresholds are the single source of truth for both the run loop and the
// pure `live_ui` builder, so the boot receipts and the live assembly stay in
// lock-step. They were tuned by visual review at 80x24 / 120x32 / 160x40.

/// The live region attaches here — the moment the boot log reports the
/// coordinator online. Boot receipts keep streaming above it after this.
const SHELL_START: f32 = 2.66;
/// The objective row (and its sealed counter) resolves in.
const OBJECTIVE_AT: f32 = 3.00;
/// The rule wipes in and the orchestration frame appears.
const ORCH_AT: f32 = 3.30;
/// When each worker's live slot appears — ~0.10s after its boot receipt, so the
/// receipt in scrollback and the slot below it read as one event.
const AGENT_AT: [f32; 4] = [3.55, 4.15, 4.75, 5.35];
/// How long a worker slot takes to resolve from skeletal to full detail.
const SLOT_COHERE: f32 = 0.55;
/// ARCHITECT flips queued → active (just after the "plan executing" receipt).
const ARCH_IGNITE_AT: f32 = 6.05;
/// The legend and orchestrator footer settle in.
const OPERATIONAL_AT: f32 = 6.60;
/// Everything has cohered into the film's `t = 0` pose; the hold begins.
const MATCH_READY_AT: f32 = 7.15;
/// End of the prelude (the tail after MATCH_READY is a stable, readable hold).
const PRELUDE_END: f32 = 8.30;

/// Back-compat alias: the instant the live region first attaches.
const LIVE_START: f32 = SHELL_START;

/// What the caller should do after the prologue returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Proceed into the fullscreen film (normal completion or Space-to-skip).
    Proceed,
    /// The viewer asked to quit (Esc / Ctrl-C); do not run the film.
    Quit,
}

/// Coarse assembly stage at a given prelude time. The live region reveals
/// continuously; this is a readable label over the thresholds above, used for
/// choreography tests and to document the intended progression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Pure terminal receipts; no live region yet.
    Terminal,
    /// The header rail exists; nothing else.
    Shell,
    /// The objective row has resolved in.
    Objective,
    /// The rule + orchestration frame exist; no worker slots yet.
    Orchestrator,
    /// Worker slots are arriving / cohering one at a time.
    AgentSlots,
    /// Legend + footer present; ARCHITECT active; the pose is nearly complete.
    Operational,
    /// The exact film `t = 0` pose, held readable before the cut.
    MatchReady,
}

/// The coarse [`Stage`] at prelude time `t`.
pub fn stage(t: f32) -> Stage {
    if t < SHELL_START {
        Stage::Terminal
    } else if t < OBJECTIVE_AT {
        Stage::Shell
    } else if t < ORCH_AT {
        Stage::Objective
    } else if t < AGENT_AT[0] {
        Stage::Orchestrator
    } else if t < OPERATIONAL_AT {
        Stage::AgentSlots
    } else if t < MATCH_READY_AT {
        Stage::Operational
    } else {
        Stage::MatchReady
    }
}

/// One scheduled boot-log line: the prelude time it appears and its styled spans.
type BootLine = (f32, Vec<(&'static str, Color)>);

/// The credible — and explicitly simulated — agent-harness boot sequence.
/// Deterministic and ordered; each line is committed to scrollback when the
/// prelude clock passes its time. The four workers are the film's canonical
/// agents (same names/roles), brought online with their identity glyphs and
/// colors so the handoff is a match cut.
///
/// The content is a *demo project* ("~/agents/transit-planner", fictional revision,
/// the film's own transit dataset), and the second line marks the whole harness
/// as simulated — the ordinary-terminal framing must never read as a probe of the
/// viewer's real host.
fn boot_schedule() -> Vec<BootLine> {
    let mut s: Vec<BootLine> = vec![
        (
            0.15,
            vec![
                ("libgibson-harness", CYAN),
                ("   0.1.0 · engineering alpha", DIM),
            ],
        ),
        (
            0.42,
            vec![
                ("· session       ", DIM),
                (
                    "simulated harness · deterministic demo (no live tools run)",
                    DIM,
                ),
            ],
        ),
        (
            0.72,
            vec![("· workspace     ", DIM), ("~/agents/transit-planner", INK)],
        ),
        (
            1.00,
            vec![
                ("· repository    ", DIM),
                ("git ", INK),
                ("@ 5f3a9c1", CYAN),
                ("  ·  clean tree", DIM),
            ],
        ),
        (
            1.30,
            vec![
                ("· runtime       ", DIM),
                ("local · seed 0xC0FFEE · no wall-clock drift", INK),
            ],
        ),
        (
            1.60,
            vec![
                ("· tool registry ", DIM),
                ("read · write · search · shell · render", INK),
                ("  (5)", DIM),
            ],
        ),
        (
            1.92,
            vec![
                ("· dataset       ", DIM),
                ("fixtures/transit.graph · 128 stops · 384 links", INK),
            ],
        ),
        (
            2.22,
            vec![
                ("· acceptance    ", DIM),
                ("Build a resilient transit planner", INK),
            ],
        ),
        (
            2.44,
            vec![
                ("                ", DIM),
                ("4 ordered contracts loaded from PLAN.md", DIM),
            ],
        ),
        (
            2.66,
            vec![
                ("· coordinator   ", DIM),
                ("online", OK),
                ("  ·  queue depth 0", DIM),
            ],
        ),
        (2.98, vec![("· allocating workers", DIM), (" ...", DIM)]),
    ];
    // The four workers coming online — canonical identities + roles. Their times
    // sit ~0.10s ahead of each live slot in AGENT_AT, so each receipt in
    // scrollback and its materializing slot below read as a single beat.
    let receipts = [3.45, 4.05, 4.65, 5.25];
    for (i, a) in AGENTS.iter().enumerate() {
        let id = IDENTITIES[i];
        s.push((
            receipts[i],
            vec![
                ("   ", DIM),
                (id.signature, id.color()),
                (leak(format!(" {:<9}", a.name)), id.color()),
                (leak(format!(" {:<22}", a.role)), DIM),
                ("online", OK),
            ],
        ));
    }
    s.push((
        5.95,
        vec![
            ("· plan          ", DIM),
            ("executing", OK),
            ("  ·  task 1/4 → ", DIM),
            ("ARCHITECT", IDENTITIES[0].color()),
        ],
    ));
    s.push((6.25, vec![("", DIM)]));
    s.push((
        6.50,
        vec![
            ("libgibson", CYAN),
            (
                " › live region attached — telemetry is now a structured surface ↓",
                DIM,
            ),
        ],
    ));
    s
}

/// Leaks a `String` to `&'static str`. The boot schedule / live UI are built a
/// bounded number of times per run; a handful of small permanent allocations is
/// an acceptable price for uniform `&'static str` spans, and this is example
/// (not library) code.
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

/// Prelude time at which worker slot `i` first appears in the live region
/// (exposed for choreography tests; robust to retuning the constants).
pub fn agent_slot_time(i: usize) -> f32 {
    AGENT_AT[i]
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

/// A monotonic 0→1 entrance ramp for an element that begins at `at` and resolves
/// over `dur` seconds. Pure; clamps outside the window.
fn reveal(t: f32, at: f32, dur: f32) -> f32 {
    ((t - at) / dur).clamp(0.0, 1.0)
}

/// Builds one worker's live rows at prelude time `t`. Each slot occupies a fixed
/// number of rows from the instant it appears (so the region only ever grows),
/// and its CONTENT resolves in place from skeletal → full: a "linking"
/// placeholder becomes the queued (or, for ARCHITECT after ignition, active)
/// record, and the progress track wipes in. ARCHITECT's bar settles at
/// `progress(0)` — an ignited head with no fill — matching the film's `t = 0`.
fn agent_rows(i: usize, t: f32, bar_n: usize, full_detail: bool) -> Vec<Node> {
    let id = IDENTITIES[i];
    let a = AGENTS[i];
    let cohere = reveal(t, AGENT_AT[i], SLOT_COHERE);
    let cohered = cohere >= 1.0;
    let active = i == 0 && t >= ARCH_IGNITE_AT;

    let (rail, col) = if active {
        ("┃", id.color())
    } else {
        ("│", DIM)
    };
    let state = if active {
        "active"
    } else if cohered {
        "queued"
    } else {
        "· linking"
    };

    let mut rows = Vec::new();
    if full_detail {
        rows.push(styled_line(&[
            (rail, col),
            (leak(format!(" {} {:<10}", id.signature, a.name)), col),
            (leak(format!("{state:<9}")), DIM),
        ]));
        // The role resolves in a beat after the name (fades from placeholder).
        let role = if cohere > 0.35 { a.role } else { "·" };
        rows.push(styled_line(&[
            (rail, col),
            (leak(format!("   {role}")), DIM),
        ]));
    } else {
        // Compact: name, state and role share one line.
        let tail = if cohere > 0.35 {
            format!("{state:<8} · {}", a.role)
        } else {
            state.to_string()
        };
        rows.push(styled_line(&[
            (rail, col),
            (leak(format!(" {} {:<10}", id.signature, a.name)), col),
            (leak(tail), DIM),
        ]));
    }

    // Progress track wipes in as the slot coheres; the head lights on ignition.
    let lit = ((cohere * bar_n as f32) as usize).min(bar_n);
    let bar = if active {
        // Ignited, no work done yet: a single head glyph at position 0.
        let head = "╸";
        let rest = "─".repeat(bar_n.saturating_sub(1));
        vec![(rail, col), ("   ", DIM), (head, INK), (leak(rest), col)]
    } else {
        vec![(rail, col), ("   ", DIM), (leak("─".repeat(lit)), RULE)]
    };
    rows.push(styled_line(&bar));
    rows.push(text("", DIM));
    rows
}

/// The live-region UI at prelude time `t`, laid out for a `width`×`height`
/// terminal. Assembles the harness framing progressively and lands on the film's
/// `t = 0` pose (ARCHITECT active at progress 0, the rest queued). Pure function
/// of `t`, `width` and `height`; row count is monotonic non-decreasing in `t`.
pub fn live_ui(t: f32, width: u16, height: u16) -> Node {
    // On short terminals the inline region must fit within `height - 1` rows, so
    // fold each worker onto two lines and drop the wider spacers.
    let full_detail = height >= 29;
    let w = width as usize;
    let bar_n = w.saturating_sub(10).clamp(8, 44);

    let mut rows: Vec<Node> = Vec::new();

    // --- Shell: the header rail (first structured element, deliberately modest).
    if t >= SHELL_START {
        // The right-hand label locks in a beat after the wordmark.
        // Byte-identical to the film's header (harness.rs) for a pristine cut.
        let ops: &[(&str, Color)] = if t >= SHELL_START + 0.25 {
            &[("AGENT OPERATIONS    /    01", DIM)]
        } else {
            &[("", DIM)]
        };
        rows.push(Node::row().children(vec![
            styled_line(&[("G / ", CYAN), ("LIBGIBSON", INK)]).flex_grow(1.0),
            styled_line(ops),
        ]));
        if full_detail {
            rows.push(text("", DIM));
        }
    }

    // --- Objective: promoted from the "acceptance:" boot receipt.
    if t >= OBJECTIVE_AT {
        rows.push(Node::row().children(vec![
            styled_line(&[("Build a resilient transit planner.", INK)]).flex_grow(1.0),
            styled_line(&[("0/4 SEALED", MINT)]),
        ]));
        if full_detail {
            rows.push(text(
                "A finite multi-agent simulation. One shared objective.",
                DIM,
            ));
        }
    }

    // --- Orchestrator: the rule wipes in, then the orchestration frame.
    if t >= ORCH_AT {
        let rule_w = w.min(bar_n + 14).max(12);
        let lit = ((reveal(t, ORCH_AT, 0.45) * rule_w as f32) as usize).min(rule_w);
        rows.push(styled_line(&[(leak("─".repeat(lit)), RULE)]));
        if full_detail {
            rows.push(text("", DIM));
        }
        rows.push(styled_line(&[
            ("01  ", CYAN),
            ("ORCHESTRATION", INK),
            ("     PLAN → MAP → BUILD → PROVE", DIM),
        ]));
        if full_detail {
            rows.push(text("", DIM));
        }
    }

    // --- Agent slots: arrive one at a time, each resolving skeletal → full.
    for (i, &at) in AGENT_AT.iter().enumerate() {
        if t >= at {
            rows.extend(agent_rows(i, t, bar_n, full_detail));
        }
    }

    // --- Operational: the legend and the orchestrator footer settle in last.
    if t >= OPERATIONAL_AT {
        if full_detail {
            rows.push(text("", DIM));
        }
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
    }

    Node::col().width(width as f32).children(rows)
}

/// Runs the prologue on an inline context, then restores it (releasing the
/// terminal lease). `deterministic` advances a fixed synthetic clock (for
/// non-interactive / test runs); otherwise real time paces playback and
/// Space/Enter skips to the film, Esc / Ctrl-C quits.
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
        // Each commit finalizes the current live frame; the next render below
        // re-establishes the live region beneath the freshly committed line, so
        // receipts stream above a live region that is already assembling.
        while emitted < schedule.len() && schedule[emitted].0 <= t {
            ctx.commit_rich_text(&boot_rich(&schedule[emitted].1))?;
            emitted += 1;
        }

        // PHASE B: once the coordinator is online, the live region attaches and
        // assembles — overlapping the remaining worker receipts above it.
        if t >= LIVE_START {
            let (w, h) = ctx.session.terminal_size();
            ctx.set_root(live_ui(t, w.clamp(40, 200), h.max(10)));
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
        // The whole boot log finishes before the live phase ends (so the last
        // receipts land while the dashboard is still cohering, not after).
        let last = s.last().unwrap().0;
        assert!(
            last < PRELUDE_END,
            "boot log ({last}) must fit before PRELUDE_END"
        );
        const { assert!(LIVE_START < PRELUDE_END) };
        const { assert!(MATCH_READY_AT < PRELUDE_END) }; // there is a hold at the end
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
        // The framing is explicitly a simulation, not a probe of the real host.
        assert!(flat.contains("simulated harness"));
    }

    #[test]
    fn live_ui_builds_across_the_prelude_and_widths() {
        // Pure builder must not panic across the phase and a range of geometries.
        for &(w, h) in &[(40u16, 12u16), (80, 24), (120, 32), (160, 40), (200, 50)] {
            let mut t = LIVE_START;
            while t <= PRELUDE_END {
                let _ = live_ui(t, w, h);
                t += 0.1;
            }
        }
    }

    #[test]
    fn assembly_is_monotonic_in_row_count() {
        // The engine grows an inline live region but cannot shrink it between
        // frames, so the assembled row count must never decrease with time.
        for &(w, h) in &[(80u16, 24u16), (120, 32), (160, 40)] {
            let mut prev = 0usize;
            let mut t = LIVE_START;
            while t <= PRELUDE_END + 0.5 {
                let n = count_rows(&live_ui(t, w, h));
                assert!(
                    n >= prev,
                    "row count shrank at t={t:.2} ({prev} -> {n}) on {w}x{h}"
                );
                prev = n;
                t += 0.05;
            }
        }
    }

    // Approximate rendered height of a live_ui tree: the root is a column of
    // one-row leaves and one-row `Node::row()`s, so its child count is its row
    // count.
    fn count_rows(node: &Node) -> usize {
        node.children.len()
    }

    #[test]
    fn stage_progression_is_ordered_and_total() {
        use Stage::*;
        // The coarse stage advances monotonically through the whole prelude.
        let order = [
            Terminal,
            Shell,
            Objective,
            Orchestrator,
            AgentSlots,
            Operational,
            MatchReady,
        ];
        let mut idx = 0usize;
        let mut t = 0.0f32;
        while t <= PRELUDE_END {
            let s = stage(t);
            if s != order[idx] {
                idx += 1;
                assert!(idx < order.len(), "unexpected stage {s:?} at t={t:.2}");
                assert_eq!(s, order[idx], "stage jumped out of order at t={t:.2}");
            }
            t += 0.02;
        }
        assert_eq!(stage(PRELUDE_END), Stage::MatchReady);
        assert_eq!(stage(0.0), Stage::Terminal);
    }
}
