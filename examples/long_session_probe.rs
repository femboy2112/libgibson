//! Long-session trace/resource retention probe for issue #10 — ooh yeah,
//! MEASURE FIRST! No API design lives in this file, just cold hard numbers.
//!
//! `StoryDirector::update` unconditionally pushes a `TraceStep` onto
//! `StoryTrace.steps` on *every* call — even when `events` is empty — and it
//! does this before the trace ever gets a chance to gate on anything
//! (src/story.rs, the `self.trace.steps.push(..)` line runs ahead of the
//! empty-events check). There is no cap, no window, no opt-out. This probe
//! hammers that exact path for N ticks and reports exactly how big it gets.
//!
//! Run it:
//!   cargo +1.98.1 run --release --example long_session_probe -- --ticks 1000000
//!   cargo +1.98.1 run --release --example long_session_probe -- --ticks 1000000 --events

use gibson::{Beat, Story, StoryEvent, TraceRetention, TraceStep};
use std::time::{Duration, Instant};

/// Two numbers straight from `/proc/self/status`, in kB, exactly as the
/// kernel reports them. Linux-specific by construction — ooh, this whole
/// snapshot only exists because `/proc` exists! Nothing else in this probe
/// depends on that, just these two fields.
#[derive(Debug, Clone, Copy)]
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

/// `/proc/self/status` fields look like "    12345 kB"; grab the first token.
fn parse_kb(field: &str) -> Option<u64> {
    field.split_whitespace().next()?.parse::<u64>().ok()
}

/// A minimal self-looping story: one beat, zero transitions. With no arrow to
/// follow, `find_auto_transition` always returns `None`, so the director just
/// sits in `loop` forever and every `update(dt, &[])` is the worst case for
/// issue #10 — a session that never finishes, never replays early, never caps
/// its trace. Existence is pain, and so is an unbounded `Vec<TraceStep>`.
fn build_looping_story() -> Story {
    Story::new("loop").beat(Beat::new("loop"))
}

/// A ping-pong story: two beats, each auto-transitioning to the other on every
/// update (`After(0)`). Every `update` fires exactly one transition, so the
/// `StoryTrace.beats` log grows one entry per tick — the worst case for the beat
/// log, the counterpart to the looping story's worst case for the step log
/// (issue #10, A2). Left unbounded, `beats` is just as linear as `steps`.
fn build_ping_pong_story() -> Story {
    Story::new("ping")
        .beat(Beat::new("ping").after(Duration::ZERO, "pong"))
        .beat(Beat::new("pong").after(Duration::ZERO, "ping"))
}

/// Estimate of heap bytes owned by the per-step `events` vectors: the
/// `Vec<StoryEvent>` buffer itself (by *capacity*, since that's what's
/// actually allocated, not just `len`), plus any `String` payload carried by
/// `UserSelected`/`Command`/`Custom` variants (also by capacity). This is an
/// estimate, not an instrumented allocator count — it does not see allocator
/// bucket rounding or fragmentation.
fn events_heap_bytes(steps: &[TraceStep]) -> usize {
    steps
        .iter()
        .map(|step| {
            let vec_bytes = step.events.capacity() * std::mem::size_of::<StoryEvent>();
            let string_bytes: usize = step
                .events
                .iter()
                .map(|e| match e {
                    StoryEvent::UserSelected(s)
                    | StoryEvent::Command(s)
                    | StoryEvent::Custom(s) => s.capacity(),
                    _ => 0,
                })
                .sum();
            vec_bytes + string_bytes
        })
        .sum()
}

#[derive(Clone, Copy)]
enum StoryKind {
    /// One beat, no transitions: worst case for the step log.
    Looping,
    /// Two beats, one transition per update: worst case for the beat log.
    PingPong,
}

struct Args {
    ticks: u64,
    use_events: bool,
    story: StoryKind,
    retention: TraceRetention,
}

fn parse_args() -> Args {
    let mut ticks = 100_000u64;
    let mut use_events = false;
    let mut story = StoryKind::Looping;
    let mut retention = TraceRetention::All;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--ticks" => {
                if let Some(v) = args.next() {
                    ticks = match v.parse() {
                        Ok(n) => n,
                        Err(_) => panic!("--ticks expects an integer, got `{v}`"),
                    };
                }
            }
            "--events" => use_events = true,
            "--story" => {
                story = match args.next().as_deref() {
                    Some("loop") | Some("looping") => StoryKind::Looping,
                    Some("pingpong") | Some("ping-pong") => StoryKind::PingPong,
                    Some(other) => panic!("--story expects loop|pingpong, got `{other}`"),
                    None => panic!("--story expects a value"),
                };
            }
            "--retention" => {
                retention = match args.next().as_deref() {
                    Some("all") => TraceRetention::All,
                    Some("disabled") => TraceRetention::Disabled,
                    Some(spec) if spec.starts_with("bounded:") => {
                        let cap = spec["bounded:".len()..]
                            .parse::<usize>()
                            .expect("--retention bounded:CAP expects an integer cap");
                        TraceRetention::Bounded(cap)
                    }
                    Some(other) => {
                        panic!("--retention expects all|disabled|bounded:CAP, got `{other}`")
                    }
                    None => panic!("--retention expects a value"),
                };
            }
            other => eprintln!("(ignoring unknown arg `{other}`)"),
        }
    }
    Args {
        ticks,
        use_events,
        story,
        retention,
    }
}

fn fmt_kb(v: Option<u64>) -> String {
    match v {
        Some(kb) => format!("{kb} kB"),
        None => "unavailable".to_string(),
    }
}

fn fmt_kb_delta(before: Option<u64>, after: Option<u64>) -> String {
    match (before, after) {
        (Some(b), Some(a)) => format!("{} kB", a as i64 - b as i64),
        _ => "unavailable".to_string(),
    }
}

fn main() {
    let args = parse_args();
    let dt = Duration::from_millis(16); // one fixed, small "frame" tick

    println!("============================================================");
    println!(" LIBGIBSON LONG-SESSION TRACE/RESOURCE RETENTION PROBE");
    println!(" (issue #10 — measurement only, no retention API here)");
    println!("============================================================\n");
    println!("host note: VmRSS/VmHWM are read from /proc/self/status and are");
    println!("therefore Linux-specific; every other number here is portable.\n");

    let story = match args.story {
        StoryKind::Looping => build_looping_story(),
        StoryKind::PingPong => build_ping_pong_story(),
    };
    story.validate().expect("probe story must validate");
    let mut director = story.start();
    director.set_trace_retention(args.retention);

    // Baseline captured right here: the director already exists (fixed, O(1)
    // cost — one beat, one empty trace) but the trace has not yet been asked
    // to grow. Everything measured after this point is the N-tick cost.
    let baseline = RssSnapshot::read();

    let events: Vec<StoryEvent> = if args.use_events {
        vec![StoryEvent::custom("tick")]
    } else {
        vec![]
    };

    let start = Instant::now();
    for _ in 0..args.ticks {
        director.update(dt, &events);
    }
    let loop_elapsed = start.elapsed();

    let after = RssSnapshot::read();

    let trace = director.trace();
    let steps_len = trace.steps.len();
    let steps_cap = trace.steps.capacity();
    let step_size = std::mem::size_of::<TraceStep>();
    let len_basis_bytes = steps_len * step_size + events_heap_bytes(&trace.steps);
    let cap_basis_bytes = steps_cap * step_size + events_heap_bytes(&trace.steps);

    // Beat log: the second retained history #10/A2 must also bound. A ping-pong
    // story pushes one beat per tick, so this grows linearly under `All`.
    let beats_len = trace.beats.len();
    let beats_cap = trace.beats.capacity();
    let beat_pair_size = std::mem::size_of::<(Duration, String)>();
    let beats_string_bytes: usize = trace.beats.iter().map(|(_, s)| s.capacity()).sum();
    let beats_bytes = beats_cap * beat_pair_size + beats_string_bytes;
    let dropped_steps = trace.dropped_steps();
    let dropped_beats = trace.dropped_beats();
    let is_complete = trace.is_complete();

    let replay_start = Instant::now();
    let replayed = story.replay(trace);
    let replay_elapsed = replay_start.elapsed();
    assert_eq!(
        replayed.trace().steps.len(),
        steps_len,
        "replay must reproduce every recorded step"
    );

    let avg_update_ns = loop_elapsed.as_nanos() as f64 / args.ticks as f64;

    println!(
        "mode:                          {}",
        if args.use_events {
            "--events (1 StoryEvent::Custom per tick)"
        } else {
            "empty events (worst case: update(dt, &[]))"
        }
    );
    println!("ticks (N):                     {}", args.ticks);
    println!("dt per tick:                   {dt:?}");
    println!(
        "story:                         {}",
        match args.story {
            StoryKind::Looping => "looping (1 beat, 0 transitions: step-log worst case)",
            StoryKind::PingPong => "ping-pong (2 beats, 1 transition/tick: beat-log worst case)",
        }
    );
    println!("retention:                     {:?}", args.retention);
    println!();
    println!("trace().steps.len():           {steps_len}");
    println!("trace().steps.capacity():      {steps_cap}");
    println!("size_of::<TraceStep>():        {step_size} bytes");
    println!(
        "retained step bytes (cap):     {cap_basis_bytes} bytes  ({:.3} MiB)",
        cap_basis_bytes as f64 / (1024.0 * 1024.0)
    );
    println!("  (len basis:                  {len_basis_bytes} bytes)",);
    println!("trace().beats.len():           {beats_len}");
    println!("trace().beats.capacity():      {beats_cap}");
    println!(
        "retained beat bytes (cap):     {beats_bytes} bytes  ({:.3} MiB)",
        beats_bytes as f64 / (1024.0 * 1024.0)
    );
    println!("dropped_steps():               {dropped_steps}");
    println!("dropped_beats():               {dropped_beats}");
    println!("is_complete():                 {is_complete}");
    println!();
    println!(
        "VmRSS  before (baseline):      {}",
        fmt_kb(baseline.vm_rss_kb)
    );
    println!(
        "VmHWM  before (baseline):      {}",
        fmt_kb(baseline.vm_hwm_kb)
    );
    println!("VmRSS  after:                  {}", fmt_kb(after.vm_rss_kb));
    println!("VmHWM  after:                  {}", fmt_kb(after.vm_hwm_kb));
    println!(
        "VmRSS  delta:                  {}",
        fmt_kb_delta(baseline.vm_rss_kb, after.vm_rss_kb)
    );
    println!();
    println!("loop wall time:                {loop_elapsed:?}");
    println!("avg per-update cost:           {avg_update_ns:.2} ns");
    println!("replay wall time (full trace): {replay_elapsed:?}");
    println!("============================================================\n");
}
