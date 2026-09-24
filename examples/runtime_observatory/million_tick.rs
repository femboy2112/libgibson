//! Million-Tick mode: drive a real, self-looping `StoryDirector` for N ticks
//! under a chosen trace-retention policy, and watch the trace/RSS curves live
//! or in one deterministic `--dump` frame. This is issue #10's fix, on stage:
//! flip `--retention` and the sparkline visibly stops climbing.
//!
//! Mirrors `examples/long_session_probe.rs`'s story setup (one beat, zero
//! transitions — `update(dt, &[])` never finds an auto-transition, so the
//! director just sits there accumulating trace steps forever unless told
//! not to) but this is a rendered instrument, not a measurement printout.

use std::time::{Duration, Instant};

use gibson::{Beat, ColorDepth, Story, TraceRetention};

use crate::diag::{Category, DiagLog};
use crate::visual::{self, PanelRow, Spark, Tone, View};

pub const DIAG_LOG_CAP: usize = 256;
const SAMPLE_TARGET: u64 = 300;

/// Two fields straight from `/proc/self/status`, in kB, exactly as the kernel
/// reports them. Linux-specific by construction, same as
/// `long_session_probe.rs`'s `RssSnapshot` — nothing else in this mode
/// depends on `/proc` existing.
struct RssSnapshot {
    vm_rss_kb: Option<u64>,
    vm_hwm_kb: Option<u64>,
}
impl RssSnapshot {
    fn read() -> Self {
        let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
        let mut vm_rss_kb = None;
        let mut vm_hwm_kb = None;
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                vm_rss_kb = parse_kb(rest);
            } else if let Some(rest) = line.strip_prefix("VmHWM:") {
                vm_hwm_kb = parse_kb(rest);
            }
        }
        Self {
            vm_rss_kb,
            vm_hwm_kb,
        }
    }
}
fn parse_kb(field: &str) -> Option<u64> {
    field.split_whitespace().next()?.parse::<u64>().ok()
}

pub fn parse_retention(s: &str) -> Result<TraceRetention, String> {
    if s == "all" {
        Ok(TraceRetention::All)
    } else if s == "disabled" {
        Ok(TraceRetention::Disabled)
    } else if let Some(cap) = s.strip_prefix("bounded:") {
        cap.parse::<usize>()
            .map(TraceRetention::Bounded)
            .map_err(|_| format!("invalid --retention bounded cap `{cap}`"))
    } else {
        Err(format!(
            "invalid --retention `{s}` (want all|bounded:CAP|disabled)"
        ))
    }
}

fn retention_label(r: TraceRetention) -> String {
    match r {
        TraceRetention::All => "all".to_string(),
        TraceRetention::Bounded(cap) => format!("bounded:{cap}"),
        TraceRetention::Disabled => "disabled".to_string(),
    }
}

pub struct RunResult {
    pub log: DiagLog,
    pub ticks_run: u64,
    pub ticks_target: u64,
    pub elapsed: Duration,
    pub retention: TraceRetention,
    pub final_retained_steps: usize,
    pub final_dropped_steps: usize,
    pub final_complete: bool,
    pub final_rss_kb: Option<u64>,
    pub final_peak_rss_kb: Option<u64>,
}

/// Runs the driven story to completion and returns the full bounded log. Used
/// by both `--dump` (run then render one last frame) and any future live loop
/// for this mode.
pub fn run(ticks: u64, retention: TraceRetention) -> RunResult {
    let story = Story::new("loop").beat(Beat::new("loop"));
    story.validate().expect("looping story must validate");
    let mut director = story.start();
    director.set_trace_retention(retention);

    let mut log = DiagLog::new(DIAG_LOG_CAP);
    let dt = Duration::from_millis(16);
    let sample_every = (ticks / SAMPLE_TARGET).max(1);
    let epoch = Instant::now();

    let sample = |log: &mut DiagLog, director: &gibson::story::StoryDirector, tick: u64| {
        let monotonic_us = epoch.elapsed().as_micros() as u64;
        let trace = director.trace();
        log.push(
            monotonic_us,
            Category::Resource,
            "TraceLength",
            "StoryDirector::trace().steps.len()",
            trace.steps.len(),
            Some(trace.steps.len() as f64),
        );
        log.push(
            monotonic_us,
            Category::Resource,
            "DroppedSteps",
            "StoryDirector::trace().dropped_steps()",
            trace.dropped_steps(),
            Some(trace.dropped_steps() as f64),
        );
        log.push(
            monotonic_us,
            Category::Resource,
            "Complete",
            "StoryDirector::trace().is_complete()",
            if trace.is_complete() { "yes" } else { "no" },
            None,
        );
        let rss = RssSnapshot::read();
        if let Some(kb) = rss.vm_rss_kb {
            log.push(
                monotonic_us,
                Category::Resource,
                "RssKb",
                "/proc/self/status:VmRSS",
                kb,
                Some(kb as f64),
            );
        }
        if let Some(kb) = rss.vm_hwm_kb {
            log.push(
                monotonic_us,
                Category::Resource,
                "PeakRssKb",
                "/proc/self/status:VmHWM",
                kb,
                Some(kb as f64),
            );
        }
        let _ = tick;
    };

    for tick in 1..=ticks {
        director.update(dt, &[]);
        if tick % sample_every == 0 || tick == ticks {
            sample(&mut log, &director, tick);
        }
    }

    let elapsed = epoch.elapsed();
    let trace = director.trace();
    let final_retained_steps = trace.steps.len();
    let final_dropped_steps = trace.dropped_steps();
    let final_complete = trace.is_complete();
    let final_rss = RssSnapshot::read();

    RunResult {
        log,
        ticks_run: ticks,
        ticks_target: ticks,
        elapsed,
        retention,
        final_retained_steps,
        final_dropped_steps,
        final_complete,
        final_rss_kb: final_rss.vm_rss_kb,
        final_peak_rss_kb: final_rss.vm_hwm_kb,
    }
}

fn fmt_kb(v: Option<u64>) -> String {
    v.map_or_else(|| "?".to_string(), |kb| format!("{kb} kB"))
}

/// Builds the render-ready `View` from a finished (or in-progress) run. Pure:
/// reads `result`, produces a `View` borrowing its log — no mutation, no
/// re-sampling.
pub fn build_view(result: &RunResult) -> View<'_> {
    let (hero_state, hero_tone) = match result.retention {
        TraceRetention::All => ("TRACE GROWTH".to_string(), Tone::Warn),
        TraceRetention::Bounded(cap) => (format!("BOUNDED RETENTION (cap {cap})"), Tone::Nominal),
        TraceRetention::Disabled => ("RETENTION DISABLED".to_string(), Tone::Nominal),
    };

    let panel_rows = vec![
        PanelRow::new(
            "Ticks",
            format!("{}/{}", result.ticks_run, result.ticks_target),
            Tone::Nominal,
        ),
        PanelRow::new(
            "Retention",
            retention_label(result.retention),
            Tone::Nominal,
        ),
        PanelRow::new(
            "Retained",
            result.final_retained_steps.to_string(),
            Tone::Nominal,
        ),
        PanelRow::new(
            "Dropped",
            result.final_dropped_steps.to_string(),
            if result.final_dropped_steps > 0 && matches!(result.retention, TraceRetention::All) {
                // Should be structurally impossible under All — if the API
                // ever lies about that, don't call it a mere warning.
                Tone::Fault
            } else if result.final_dropped_steps > 0 {
                Tone::Warn
            } else {
                Tone::Nominal
            },
        ),
        PanelRow::new(
            "Complete?",
            if result.final_complete { "yes" } else { "no" },
            if result.final_complete {
                Tone::Nominal
            } else {
                Tone::Warn
            },
        ),
        PanelRow::new("VmRSS", fmt_kb(result.final_rss_kb), Tone::Nominal),
        PanelRow::new("VmHWM pk", fmt_kb(result.final_peak_rss_kb), Tone::Nominal),
        PanelRow::new(
            "Wall(s)",
            format!("{:.3}", result.elapsed.as_secs_f64()),
            Tone::Nominal,
        ),
    ];

    let sparks = vec![
        Spark {
            title: "RETAINED STEPS".to_string(),
            values: result.log.series_of_kind("TraceLength"),
            unit: String::new(),
        },
        Spark {
            title: "VmRSS kB (Linux /proc)".to_string(),
            values: result.log.series_of_kind("RssKb"),
            unit: "k".to_string(),
        },
    ];

    View {
        monotonic_us: result.elapsed.as_micros() as u64,
        mode_label: "MILLION-TICK".to_string(),
        hero_state,
        hero_tone,
        subtitle: format!(
            "mode=million-tick retention={} ticks={}/{}",
            retention_label(result.retention),
            result.ticks_run,
            result.ticks_target
        ),
        panel_title: "RESOURCE TELEMETRY".to_string(),
        panel_rows,
        sparks,
        log: &result.log,
        footer:
            "RENDER/OUTPUT/INPUT/SESSION unobserved in this mode \u{2014} see SUBSYSTEM STATUS."
                .to_string(),
        controls: "--dump only in this chunk \u{7c} Esc exit (future live loop)".to_string(),
    }
}

pub fn dump(
    ticks: u64,
    retention: TraceRetention,
    width: u16,
    height: u16,
    depth: ColorDepth,
) -> String {
    let result = run(ticks, retention);
    let view = build_view(&result);
    let surface = visual::frame(&view, width, height, depth);
    render_to_text(&surface, width, height)
}

fn render_to_text(surface: &gibson::Surface, width: u16, height: u16) -> String {
    let mut out = String::new();
    for y in 0..height {
        for x in 0..width {
            out.push_str(surface.get(x, y).unwrap().glyph.grapheme.as_str());
        }
        out.push('\n');
    }
    out
}
