// Part of the `field_cinema` example (`--envelope`). Includers must declare the same sibling
// modules `film.rs` needs.
#![allow(dead_code)]
//! RXIV Gate G2 — a bounded, MEASURED resource and latency envelope for the live film stack.
//!
//! Three real workloads against the reference Python producer, every number taken from a
//! monotonic clock or `/proc` on this host — nothing extrapolated:
//!
//! * **A. lockstep film** — `N` frames: advance → admitted field (observation round trip),
//!   `Film::on_field`, `Film::view` (scene build + effect evaluation + lowering), headless
//!   `Context::render` (renderer generation), and every 25th frame a keyboard edit: key →
//!   command written+flushed (input-to-dispatch) → correlated verdict (command-to-verdict).
//! * **B. terminal write path** — the renderer committing alternating film frames into an OS pipe
//!   drained concurrently: the blocking write + flush time. An OS pipe is not a physical
//!   terminal; no display frame rate is inferred from it.
//! * **C. streaming** — the free-running producer (`--hz 0`) consumed at ~60 Hz through the
//!   bounded coalescing reader: producer model cadence, consumer observation rate, lawful
//!   coalescing, drops, retained-byte high-water.
//!
//! Plus whole-process RSS (this process and the producer, `VmHWM`/`VmRSS`) and supervised
//! cleanup time. The report states its own scope; the observation age reported is the lockstep
//! round trip (an UPPER bound: the wire carries no producer clock, so streaming-mode age is not
//! measured).

use std::io::Read;
use std::time::{Duration, Instant};

use crate::film::{Film, FilmAction, FilmConfig};
use crate::live_link::{python, LiveError, LiveSession, ProducerSpec, LIVE_SOURCE, SHUTDOWN_GRACE};
use crate::trace::Admission;
use gibson::context::Context;
use gibson::input::KeyCode;
use gibson::renderer::{RenderMode, Renderer};
use gibson::session::TerminalSession;

/// A named latency sample set.
pub struct Samples {
    name: &'static str,
    v: Vec<Duration>,
}

impl Samples {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            v: Vec::new(),
        }
    }
    fn push(&mut self, d: Duration) {
        self.v.push(d);
    }
    fn line(&self) -> String {
        if self.v.is_empty() {
            return format!("{:<34} n=0", self.name);
        }
        let mut s = self.v.clone();
        s.sort();
        let q = |p: f64| s[((s.len() - 1) as f64 * p).round() as usize];
        let us = |d: Duration| d.as_secs_f64() * 1e6;
        format!(
            "{:<34} n={:<6} p50={:>9.1}us p99={:>9.1}us max={:>9.1}us",
            self.name,
            s.len(),
            us(q(0.5)),
            us(q(0.99)),
            us(*s.last().unwrap())
        )
    }
}

/// `(VmHWM, VmRSS)` in KiB for `pid` (or this process).
fn rss_kib(pid: Option<u32>) -> (u64, u64) {
    let path = pid.map_or("/proc/self/status".to_string(), |p| {
        format!("/proc/{p}/status")
    });
    let mut s = String::new();
    if std::fs::File::open(path)
        .and_then(|mut f| f.read_to_string(&mut s))
        .is_err()
    {
        return (0, 0);
    }
    let field = |k: &str| {
        s.lines()
            .find(|l| l.starts_with(k))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    };
    (field("VmHWM:"), field("VmRSS:"))
}

fn host() -> String {
    let read = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
    let cpu = read("/proc/cpuinfo")
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    let cores = std::thread::available_parallelism().map_or(0, |n| n.get());
    let mem = read("/proc/meminfo")
        .lines()
        .find(|l| l.starts_with("MemTotal"))
        .map(|l| l.split_whitespace().nth(1).unwrap_or("0").to_string())
        .unwrap_or_default();
    format!(
        "{cpu} · {cores} logical cpus · MemTotal {mem} KiB · kernel {}",
        read("/proc/sys/kernel/osrelease").trim()
    )
}

/// Run the three workloads and return the receipt text.
pub fn run(lockstep_frames: usize, stream_secs: f64) -> Result<String, LiveError> {
    let mut out = Vec::new();
    out.push(format!("host: {}", host()));
    out.push(format!(
        "build: {} profile",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    ));

    // ---- A. lockstep film ----------------------------------------------------------------------
    let t_spawn = Instant::now();
    let mut s = LiveSession::spawn(&ProducerSpec::reference(false), LIVE_SOURCE)?;
    let spawn_to_first = {
        let f = s.next_field()?;
        let d = t_spawn.elapsed();
        let mut film = Film::new(FilmConfig {
            reduced_motion: false,
            auto_advance: false,
        });
        let mut ctx = Context::headless(RenderMode::Fullscreen, 100, 32);
        let start = Instant::now();
        film.on_field(f, start.elapsed());
        let (mut obs_rtt, mut on_field, mut scene, mut gen) = (
            Samples::new("A observation round trip (lockstep)"),
            Samples::new("A Film::on_field"),
            Samples::new("A scene build+eval+lower (view)"),
            Samples::new("A renderer generation (headless)"),
        );
        let (mut dispatch, mut verdict) = (
            Samples::new("A input-to-dispatch (key->flushed)"),
            Samples::new("A command-to-verdict (round trip)"),
        );
        let mut want_low = false;
        for i in 0..lockstep_frames {
            let t = Instant::now();
            let f = s.step()?;
            obs_rtt.push(t.elapsed());
            let t = Instant::now();
            film.on_field(f, start.elapsed());
            on_field.push(t.elapsed());
            if i % 25 == 5 {
                // Alternate the target 2.0 <-> 1.0 so every edit is evidenceable.
                let key = if want_low { '[' } else { ']' };
                for _ in 0..2 {
                    film.on_key(KeyCode::Char(key), start.elapsed());
                }
                let t_key = Instant::now();
                if let FilmAction::Submit(v) = film.on_key(KeyCode::Enter, start.elapsed()) {
                    let id = s.submit_amplitude(v)?;
                    dispatch.push(t_key.elapsed());
                    film.on_submitted(id, Admission::Accepted, start.elapsed());
                    let t = Instant::now();
                    let st = s.await_verdict(id)?;
                    verdict.push(t.elapsed());
                    film.on_verdict(id, st, start.elapsed());
                    want_low = !want_low;
                }
            }
            let t = Instant::now();
            let node = film.view(start.elapsed(), 100, 32, false);
            scene.push(t.elapsed());
            ctx.set_root(node);
            let _ = ctx.render();
            gen.push(ctx.last_frame_report().generation_duration);
        }
        let (hwm, rss) = rss_kib(s.producer_pid());
        out.push(format!(
            "A lockstep: {lockstep_frames} frames · transforms {} · trace events {} (truncated {}) · captured frames {} (truncated {})",
            film.arc().transforms(),
            film.record().trace.len(),
            film.record().trace.is_truncated(),
            film.record().frames.len(),
            film.record().frames_truncated,
        ));
        for smp in [&obs_rtt, &on_field, &scene, &gen, &dispatch, &verdict] {
            out.push(smp.line());
        }
        out.push(format!("A producer RSS: VmHWM {hwm} KiB · VmRSS {rss} KiB"));
        out.push(format!("A counters: {:?}", s.counters()));
        let t = Instant::now();
        let end = s.finish(SHUTDOWN_GRACE);
        out.push(format!(
            "A cleanup (EOF -> reaped): {:.1} ms (graceful {})",
            t.elapsed().as_secs_f64() * 1e3,
            end.graceful
        ));
        d
    };
    out.push(format!(
        "A spawn -> first admitted field: {:.1} ms",
        spawn_to_first.as_secs_f64() * 1e3
    ));

    // ---- B. terminal write path into an OS pipe -------------------------------------------------
    {
        // An OS pipe drained concurrently by a real reader (`wc -c`, which also counts what it
        // drained). `std::io::pipe` would need Rust 1.87; the crate's MSRV is 1.85.
        let mut sink = std::process::Command::new("wc")
            .arg("-c")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| LiveError::Spawn {
                program: "wc".into(),
                kind: e.kind(),
            })?;
        let mut tx = sink.stdin.take().expect("piped stdin");
        let mut r = Renderer::new(RenderMode::Fullscreen);
        let mut session = TerminalSession::headless(100, 32);
        let mut write = Samples::new("B renderer write+flush (OS pipe)");
        let mut bytes: Vec<usize> = Vec::new();
        let mut s2 = LiveSession::spawn(&ProducerSpec::reference(false), LIVE_SOURCE)?;
        let mut film = Film::new(FilmConfig {
            reduced_motion: true,
            auto_advance: false,
        });
        film.on_field(s2.next_field()?, Duration::ZERO);
        for i in 0..400u64 {
            film.on_field(s2.step()?, Duration::from_millis(i));
            let mut node = film.view(Duration::from_millis(i), 100, 32, false);
            if r.render(&mut node, &mut session, &mut tx).is_ok() {
                let rep = r.last_frame_report();
                write.push(rep.write_duration);
                bytes.push(rep.bytes_emitted);
            }
        }
        s2.finish(SHUTDOWN_GRACE);
        drop(tx);
        let mut counted = String::new();
        if let Some(mut so) = sink.stdout.take() {
            let _ = so.read_to_string(&mut counted);
        }
        let _ = sink.wait();
        let total: usize = counted.trim().parse().unwrap_or(0);
        out.push(write.line());
        bytes.sort_unstable();
        let mean = bytes.iter().sum::<usize>() as f64 / bytes.len().max(1) as f64;
        out.push(format!(
            "B bytes/frame: n={} mean {mean:.0} · p50 {} · max {} · total drained {total} bytes",
            bytes.len(),
            bytes.get(bytes.len() / 2).copied().unwrap_or(0),
            bytes.last().copied().unwrap_or(0),
        ));
    }

    // ---- C. streaming -------------------------------------------------------------------------
    {
        let spec = ProducerSpec::custom(
            &python(),
            &[
                "bindings/python/producer.py",
                "--frames",
                "100000000",
                "--hz",
                "0",
                "--cols",
                "48",
                "--rows",
                "24",
                "--source",
                "1",
                "--epoch",
                "1",
            ],
        );
        let mut s = LiveSession::spawn(&spec, LIVE_SOURCE)?;
        let start = Instant::now();
        let (mut first, mut last, mut seen) = (None::<u64>, 0u64, 0u64);
        let mut gaps = 0u64;
        while start.elapsed().as_secs_f64() < stream_secs {
            if let Some(f) = s.latest_field()? {
                let t = f.field.model_tick;
                if let Some(prev) = first.map(|_| last) {
                    gaps += t.saturating_sub(prev).saturating_sub(1);
                }
                first.get_or_insert(t);
                last = t;
                seen += 1;
            }
            std::thread::sleep(Duration::from_millis(16));
        }
        let secs = start.elapsed().as_secs_f64();
        let ticks = last.saturating_sub(first.unwrap_or(last));
        let (hwm, rss) = rss_kib(s.producer_pid());
        out.push(format!(
            "C streaming {secs:.1}s: producer cadence {:.0} model ticks/s · consumer observed {:.1}/s ({seen} frames) · ticks skipped between observations {gaps}",
            ticks as f64 / secs,
            seen as f64 / secs
        ));
        out.push(format!(
            "C transport: {:?} · retained FIELD bytes high-water {} KiB",
            s.counters(),
            s.reader_bytes_high_water() / 1024
        ));
        out.push(format!("C producer RSS: VmHWM {hwm} KiB · VmRSS {rss} KiB"));
        let t = Instant::now();
        let end = s.finish(SHUTDOWN_GRACE);
        out.push(format!(
            "C cleanup (EOF -> reaped): {:.1} ms (graceful {}; a free-running producer ignores EOF while streaming, so the bounded kill backstop applies)",
            t.elapsed().as_secs_f64() * 1e3,
            end.graceful
        ));
    }

    let (hwm, rss) = rss_kib(None);
    out.push(format!(
        "presenter process RSS: VmHWM {hwm} KiB · VmRSS {rss} KiB"
    ));
    Ok(out.join("\n"))
}
