//! Live Runtime Observatory loop (workstream D1) — the interactive instrument.
//!
//! The `--dump` modes render one deterministic frame and exit; this module is
//! the opposite half: a real terminal loop that advances a mode's REAL state
//! and repaints it. It reuses LibGibson's own machinery — a [`TerminalSession`]
//! for lifecycle, [`compute_diff`] + [`AnsiCompiler`] for painting the bespoke
//! mission-control [`Surface`], and [`poll_event`] for input. No second
//! renderer, and — the whole point — no faked telemetry: every number a live
//! mode shows is measured this frame or replayed from a real recording. What we
//! did not observe stays `?`.
//!
//! Controls: `1`-`6` switch mode · `Space` pause · `R` restart · `[` / `]`
//! adjust intensity · `Esc`/`q` exit.

use gibson::ansi::AnsiCompiler;
use gibson::diff::compute_diff;
use gibson::input::{poll_event, Event, KeyCode};
use gibson::{Beat, ColorDepth, Story, StoryDirector, Surface, TerminalSession};
use std::io::{self, Write};
use std::time::{Duration, Instant};

use crate::diag::{Category, DiagLog};
use crate::visual::{frame, PanelRow, Spark, Tone, View};

/// A live Observatory mode: it owns real state, advances it on `tick`, and
/// projects it to a [`View`] for the shared renderer. It never fabricates —
/// `build_view` only reports what the mode actually measured or replayed.
pub trait LiveMode {
    /// The digit that selects this mode (`'1'`..=`'6'`).
    fn key(&self) -> char;
    /// Advance real state. `intensity` is the `[`/`]` knob (mode-interpreted).
    fn tick(&mut self, now_us: u64, intensity: i32);
    /// Project current state to a frame. Borrows the mode's own diagnostic log.
    fn build_view(&self, now_us: u64, paused: bool, intensity: i32) -> View<'_>;
    /// Reset to the mode's initial state (the `R` control).
    fn restart(&mut self);
}

/// Reads `VmRSS` from `/proc/self/status` in kB. Linux-specific by construction;
/// `None` anywhere `/proc` is absent, rendered honestly as an absent sample.
fn vm_rss_kb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest.split_whitespace().next()?.parse::<u64>().ok();
        }
    }
    None
}

/// Push a sample onto a fixed-width series, evicting the oldest — the sparkline
/// buffers are bounded too, because an unbounded history Vec is the exact bug
/// this instrument exists to watch for.
fn push_sample(series: &mut Vec<f64>, value: f64, width: usize) {
    series.push(value);
    if series.len() > width {
        let overflow = series.len() - width;
        series.drain(0..overflow);
    }
}

const SERIES_WIDTH: usize = 240;

// ---------------------------------------------------------------------------
// Mode 1 — Million-Tick live (D2): All vs Bounded, identical workload.
// ---------------------------------------------------------------------------

/// A ping-pong story transitions on every update, so it exercises BOTH the step
/// log and the beat log — the exact pair issue #10 must bound. Driving one under
/// `All` and one under `Bounded` on identical input makes the contrast the panel
/// exists to show: All climbs, Bounded flattens.
fn ping_pong_story() -> Story {
    Story::new("ping")
        .beat(Beat::new("ping").after(Duration::ZERO, "pong"))
        .beat(Beat::new("pong").after(Duration::ZERO, "ping"))
}

/// Honest byte estimate for a director's retained trace: both Vec buffers by
/// capacity, plus the beat-id string payloads. The same basis the probe uses;
/// an estimate, not an allocator count, and labelled as such.
fn retained_bytes(d: &StoryDirector) -> u64 {
    let step_sz = std::mem::size_of::<gibson::TraceStep>();
    let beat_sz = std::mem::size_of::<(Duration, String)>();
    let steps = d.trace().steps.capacity() * step_sz;
    let beats = d.trace().beats.capacity() * beat_sz;
    let strings: usize = d.trace().beats.iter().map(|(_, s)| s.capacity()).sum();
    (steps + beats + strings) as u64
}

pub struct MillionTick {
    all: StoryDirector,
    bounded: StoryDirector,
    cap: usize,
    ticks: u64,
    log: DiagLog,
    all_series: Vec<f64>,
    bounded_series: Vec<f64>,
}

impl MillionTick {
    pub fn new(cap: usize) -> Self {
        let story = ping_pong_story();
        let all = story.start();
        let mut bounded = story.start();
        bounded.set_trace_retention(gibson::TraceRetention::Bounded(cap));
        Self {
            all,
            bounded,
            cap,
            ticks: 0,
            log: DiagLog::new(256),
            all_series: Vec::new(),
            bounded_series: Vec::new(),
        }
    }

    /// Updates per frame, scaled by the intensity knob. Accelerated so the trace
    /// climbs visibly without waiting for a literal million real frames.
    fn batch(intensity: i32) -> u64 {
        let steps = 6 + intensity.clamp(-5, 8);
        1u64 << steps.clamp(1, 20) as u32
    }
}

impl LiveMode for MillionTick {
    fn key(&self) -> char {
        '1'
    }

    fn tick(&mut self, now_us: u64, intensity: i32) {
        let batch = Self::batch(intensity);
        let dt = Duration::from_micros(16);
        for _ in 0..batch {
            self.all.update(dt, &[]);
            self.bounded.update(dt, &[]);
        }
        self.ticks += batch;

        let all_bytes = retained_bytes(&self.all) as f64;
        let bounded_bytes = retained_bytes(&self.bounded) as f64;
        push_sample(&mut self.all_series, all_bytes, SERIES_WIDTH);
        push_sample(&mut self.bounded_series, bounded_bytes, SERIES_WIDTH);

        self.log.push(
            now_us,
            Category::Resource,
            "all.retained_bytes",
            "StoryDirector{All}::trace()",
            format!("{all_bytes:.0}"),
            Some(all_bytes),
        );
        self.log.push(
            now_us,
            Category::Resource,
            "bounded.dropped",
            "StoryDirector{Bounded}::trace().dropped_steps()",
            self.bounded.trace().dropped_steps(),
            Some(self.bounded.trace().dropped_steps() as f64),
        );
        if let Some(rss) = vm_rss_kb() {
            self.log.push(
                now_us,
                Category::Resource,
                "proc.vm_rss_kb",
                "/proc/self/status",
                format!("{rss} kB"),
                Some(rss as f64),
            );
        }
    }

    fn build_view(&self, now_us: u64, paused: bool, intensity: i32) -> View<'_> {
        let all_t = self.all.trace();
        let bnd_t = self.bounded.trace();
        let rss = vm_rss_kb();
        let hero = if paused { "PAUSED" } else { "STREAMING" };
        let hero_tone = if paused { Tone::Warn } else { Tone::Nominal };

        let panel_rows = vec![
            PanelRow::new("ticks", format!("{}", self.ticks), Tone::Nominal),
            PanelRow::new(
                "batch/frame",
                format!("{}", Self::batch(intensity)),
                Tone::Nominal,
            ),
            PanelRow::new("All steps", format!("{}", all_t.steps.len()), Tone::Warn),
            PanelRow::new("All beats", format!("{}", all_t.beats.len()), Tone::Warn),
            PanelRow::new(
                "Bnd steps",
                format!("{} (cap {})", bnd_t.steps.len(), self.cap),
                Tone::Nominal,
            ),
            PanelRow::new("Bnd beats", format!("{}", bnd_t.beats.len()), Tone::Nominal),
            PanelRow::new(
                "Bnd dropped",
                format!("{}s / {}b", bnd_t.dropped_steps(), bnd_t.dropped_beats()),
                Tone::Nominal,
            ),
            PanelRow::new(
                "Bnd complete",
                if bnd_t.is_complete() { "yes" } else { "no" },
                if bnd_t.is_complete() {
                    Tone::Nominal
                } else {
                    Tone::Warn
                },
            ),
            PanelRow::new(
                "proc VmRSS",
                rss.map(|k| format!("{k} kB")).unwrap_or_else(|| "?".into()),
                Tone::Warn,
            ),
        ];

        let sparks = vec![
            Spark {
                title: "All retained (unbounded)".into(),
                values: self.all_series.clone(),
                unit: "B".into(),
            },
            Spark {
                title: "Bounded retained (flat)".into(),
                values: self.bounded_series.clone(),
                unit: "B".into(),
            },
        ];

        View {
            monotonic_us: now_us,
            mode_label: "MILLION-TICK".into(),
            hero_state: hero.into(),
            hero_tone,
            subtitle: format!(
                "identical ping-pong workload, two policies · cap={} · issue #10",
                self.cap
            ),
            panel_title: "STORY TRACE RETENTION (measured, live)".into(),
            panel_rows,
            stage_rows: Vec::new(),
            sparks,
            log: &self.log,
            footer: "All climbs (steps+beats unbounded); Bounded holds a window. VmRSS is process-total, both directors + example.".into(),
            controls: "1-6 mode · Space pause · R restart · [ ] batch · Esc exit".into(),
        }
    }

    fn restart(&mut self) {
        *self = MillionTick::new(self.cap);
    }
}

// ---------------------------------------------------------------------------
// Mode 6 — Endurance (D7): a sustained bounded soak that stays healthy.
// ---------------------------------------------------------------------------

/// The counterpart to Million-Tick's leak demo: one director under a *bounded*
/// policy driven hard over a long accelerated soak, to show the runtime stays
/// flat — RSS bounded, per-update cost steady, history windowed — instead of
/// degrading. Real measurements, sustained interaction, not a benchmark score.
pub struct Endurance {
    dir: StoryDirector,
    cap: usize,
    iterations: u64,
    frames: u64,
    rss_series: Vec<f64>,
    cost_series: Vec<f64>,
    log: DiagLog,
}

impl Endurance {
    pub fn new(cap: usize) -> Self {
        let mut dir = ping_pong_story().start();
        dir.set_trace_retention(gibson::TraceRetention::Bounded(cap));
        Self {
            dir,
            cap,
            iterations: 0,
            frames: 0,
            rss_series: Vec::new(),
            cost_series: Vec::new(),
            log: DiagLog::new(256),
        }
    }
}

impl LiveMode for Endurance {
    fn key(&self) -> char {
        '6'
    }

    fn tick(&mut self, now_us: u64, intensity: i32) {
        let batch = 1u64 << (7 + intensity.clamp(-6, 6)).clamp(1, 20) as u32;
        let dt = Duration::from_micros(16);
        let t0 = Instant::now();
        for _ in 0..batch {
            self.dir.update(dt, &[]);
        }
        let ns_per = t0.elapsed().as_nanos() as f64 / batch as f64;
        self.iterations += batch;
        self.frames += 1;
        push_sample(&mut self.cost_series, ns_per, SERIES_WIDTH);
        if let Some(rss) = vm_rss_kb() {
            push_sample(&mut self.rss_series, rss as f64, SERIES_WIDTH);
            self.log.push(
                now_us,
                Category::Resource,
                "proc.vm_rss_kb",
                "/proc/self/status",
                format!("{rss} kB"),
                Some(rss as f64),
            );
        }
        self.log.push(
            now_us,
            Category::Resource,
            "update.ns_per",
            "Instant timing over the batch",
            format!("{ns_per:.1} ns"),
            Some(ns_per),
        );
    }

    fn build_view(&self, now_us: u64, paused: bool, _intensity: i32) -> View<'_> {
        let t = self.dir.trace();
        let rss = vm_rss_kb();
        let (hero, tone) = if paused {
            ("PAUSED".to_string(), Tone::Warn)
        } else {
            (format!("SOAK · {} iters", self.iterations), Tone::Nominal)
        };
        let panel_rows = vec![
            PanelRow::new("iterations", format!("{}", self.iterations), Tone::Nominal),
            PanelRow::new("frames", format!("{}", self.frames), Tone::Nominal),
            PanelRow::new(
                "retained",
                format!("{}s / {}b (cap {})", t.steps.len(), t.beats.len(), self.cap),
                Tone::Nominal,
            ),
            PanelRow::new(
                "dropped",
                format!("{}s / {}b", t.dropped_steps(), t.dropped_beats()),
                Tone::Nominal,
            ),
            PanelRow::new(
                "proc VmRSS",
                rss.map(|k| format!("{k} kB")).unwrap_or_else(|| "?".into()),
                Tone::Nominal,
            ),
            PanelRow::new(
                "cost",
                self.cost_series
                    .last()
                    .map(|c| format!("{c:.0} ns/upd"))
                    .unwrap_or_else(|| "?".into()),
                Tone::Nominal,
            ),
        ];
        let sparks = vec![
            Spark {
                title: "VmRSS (should stay flat)".into(),
                values: self.rss_series.clone(),
                unit: "kB".into(),
            },
            Spark {
                title: "update cost (steady)".into(),
                values: self.cost_series.clone(),
                unit: "ns".into(),
            },
        ];
        View {
            monotonic_us: now_us,
            mode_label: "ENDURANCE".into(),
            hero_state: hero,
            hero_tone: tone,
            subtitle: format!("bounded soak · cap={} · sustained interaction, not a benchmark", self.cap),
            panel_title: "SUSTAINED SOAK (bounded retention, measured)".into(),
            panel_rows,
            stage_rows: Vec::new(),
            sparks,
            log: &self.log,
            footer: "Bounded retention holds RSS and per-update cost flat over a long accelerated run: no leak, no slowdown.".into(),
            controls: "1-6 mode · Space pause · R restart · [ ] batch · Esc exit".into(),
        }
    }

    fn restart(&mut self) {
        *self = Endurance::new(self.cap);
    }
}

// ---------------------------------------------------------------------------
// The live loop.
// ---------------------------------------------------------------------------

/// Run the interactive Observatory until `Esc`/`q` (or `max_frames`, for the
/// headless PTY smoke test). Restores the terminal on every exit path.
pub fn run(
    depth: ColorDepth,
    initial: char,
    max_frames: Option<u64>,
    glyphs: gibson::SubcellGlyphMode,
) -> io::Result<()> {
    let mut session = TerminalSession::new()?;
    if !session.is_tty {
        return Err(io::Error::other(
            "the live Observatory needs a real terminal; use --dump for non-tty/deterministic output",
        ));
    }

    let mut modes: Vec<Box<dyn LiveMode>> = vec![
        Box::new(MillionTick::new(1024)),
        Box::new(crate::supervised::SupervisedReplay::ownership_duel()),
        Box::new(crate::supervised::SupervisedReplay::restore_failure()),
        Box::new(Endurance::new(1024)),
    ];
    let mut current = modes.iter().position(|m| m.key() == initial).unwrap_or(0);
    let mut paused = false;
    let mut intensity: i32 = 0;

    session.enter_interactive()?;
    session.enter_alternate_screen()?;
    let mut out = io::stdout();
    if let Some(bytes) = session.hide_cursor() {
        out.write_all(bytes)?;
    }
    out.flush()?;

    let mut compiler = AnsiCompiler::new();
    let mut prev: Option<Surface> = None;
    let epoch = Instant::now();
    let mut frames: u64 = 0;

    // Isolate the loop so we always run restore() below, even on an I/O error.
    let loop_result = (|| -> io::Result<()> {
        loop {
            let now_us = epoch.elapsed().as_micros() as u64;
            if !paused {
                modes[current].tick(now_us, intensity);
            }
            let (cols, rows) = session.terminal_size();
            let surface = {
                let view = modes[current].build_view(now_us, paused, intensity);
                frame(&view, cols, rows, depth, glyphs)
            };
            let diff = compute_diff(prev.as_ref(), &surface);
            let bytes = compiler.compile(&diff);
            out.write_all(&bytes)?;
            out.flush()?;
            prev = Some(surface);

            frames += 1;
            if max_frames.is_some_and(|m| frames >= m) {
                break;
            }

            if let Some(event) = poll_event(std::time::Duration::from_millis(33))? {
                match event {
                    Event::Key(k) => match k.code {
                        KeyCode::Esc => break,
                        KeyCode::Char('q') => break,
                        KeyCode::Char(' ') => paused = !paused,
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            modes[current].restart();
                            prev = None;
                        }
                        KeyCode::Char('[') => intensity = (intensity - 1).max(-5),
                        KeyCode::Char(']') => intensity = (intensity + 1).min(8),
                        KeyCode::Char(c @ '1'..='9') => {
                            if let Some(i) = modes.iter().position(|m| m.key() == c) {
                                if i != current {
                                    current = i;
                                    prev = None; // full repaint for the new mode
                                }
                            }
                        }
                        _ => {}
                    },
                    // Reflow: force a full repaint at the new geometry.
                    Event::Resize(_, _) => prev = None,
                    _ => {}
                }
            }
        }
        Ok(())
    })();

    let restore_result = session.restore();
    loop_result.and(restore_result)
}
