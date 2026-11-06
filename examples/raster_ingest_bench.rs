//! raster_ingest_bench — v0.7 node B measurement (#129 / #128).
//!
//! A deterministic moving 320×200 software framebuffer is fitted onto terminal grids
//! (80×25, 120×40, 200×60 by default), lowered to cells, rendered by the one LibGibson
//! renderer into a headless context, and written to an OS pipe drained by another thread.
//! Each stage is timed separately:
//!
//! | stage   | what it measures |
//! |---------|------------------|
//! | gen     | producing the 320×200 source frame (the "game") |
//! | fit     | consumer-side box-filter fit onto the grid's sample raster (`lower.rs`) |
//! | lower   | `RgbRaster` → cells (`to_surface` / `to_mono_surface` + glyph transcode) |
//! | render  | `Context::render`: layout, paint, diff and ANSI compilation, headless |
//! | write   | `write_all` + `flush` of that frame's bytes into a pipe — **a pipe, not a terminal**: |
//! |         | it says nothing about a terminal's parse/display time or physical FPS |
//!
//! Numbers are for the host and SHA they were measured on; nothing here is a universal FPS
//! claim. Build with `--release` for timings (debug numbers are not meaningful).
//!
//! ```text
//! cargo run --release --example raster_ingest_bench -- [--frames N] [--grids 80x25,120x40]
//! cargo run --example raster_ingest_bench -- --selfcheck
//! ```
#[path = "raster_ingest_bench/lower.rs"]
mod lower;

use gibson::capability::ColorDepth;
use gibson::context::{Context, RenderMode};
use gibson::glyph::SubcellGlyphMode;
use gibson::node::Node;
use lower::{fit_box, fitted_size, fnv, lower, source_frame, Lowering};
use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Counts allocations made by this bench binary (the library is unchanged).
struct Counting;
static ALLOCS: AtomicU64 = AtomicU64::new(0);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: forwards the caller's layout to the system allocator unchanged.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `System.alloc` with this `layout`.
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static GLOBAL: Counting = Counting;

#[derive(Clone, Copy)]
struct Config {
    cols: u16,
    rows: u16,
    lowering: Lowering,
    depth: ColorDepth,
}

fn depth_name(d: ColorDepth) -> &'static str {
    match d {
        ColorDepth::Mono => "mono",
        ColorDepth::Ansi16 => "16",
        ColorDepth::Ansi256 => "256",
        _ => "truecolor",
    }
}

#[derive(Default)]
struct Samples {
    gen: Vec<u64>,
    fit: Vec<u64>,
    lower: Vec<u64>,
    render: Vec<u64>,
    write: Vec<u64>,
    bytes: Vec<u64>,
    dirty: Vec<u64>,
    allocs: Vec<u64>,
    digest: u64,
}

fn pct(v: &[u64], p: f64) -> u64 {
    if v.is_empty() {
        return 0;
    }
    let mut s = v.to_vec();
    s.sort_unstable();
    let i = ((s.len() - 1) as f64 * p).round() as usize;
    s[i]
}

fn mean(v: &[u64]) -> u64 {
    if v.is_empty() {
        0
    } else {
        v.iter().sum::<u64>() / v.len() as u64
    }
}

/// An OS pipe whose read end is drained by a thread, so writes measure the write path only.
fn drained_pipe() -> (std::fs::File, std::thread::JoinHandle<u64>) {
    let mut fds = [0; 2];
    // SAFETY: `fds` is a valid 2-element array for pipe(2).
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0, "pipe(2)");
    // SAFETY: both fds were just created by pipe(2) and are owned exactly once here.
    let (r, w) = unsafe {
        (
            std::fs::File::from_raw_fd(fds[0]),
            std::fs::File::from_raw_fd(fds[1]),
        )
    };
    let drain = std::thread::spawn(move || {
        let mut r = r;
        let mut buf = vec![0u8; 1 << 16];
        let mut total = 0u64;
        while let Ok(n) = r.read(&mut buf) {
            if n == 0 {
                break;
            }
            total += n as u64;
        }
        total
    });
    (w, drain)
}

fn run(cfg: Config, frames: u32, pipe: &mut std::fs::File) -> Samples {
    let mut ctx = Context::headless(RenderMode::Fullscreen, cfg.cols, cfg.rows);
    ctx.set_color_depth(cfg.depth);
    let (fw, fh) = fitted_size(cfg.cols, cfg.rows);
    let mut s = Samples::default();
    let mut digest = 0xcbf2_9ce4_8422_2325u64;
    for n in 0..frames {
        let a0 = ALLOCS.load(Ordering::Relaxed);
        let t0 = Instant::now();
        let src = source_frame(n);
        let t1 = Instant::now();
        let fitted = fit_box(&src, fw, fh);
        let t2 = Instant::now();
        let surface = lower(&fitted, cfg.lowering);
        let t3 = Instant::now();
        ctx.set_root(Node::raster(surface));
        ctx.render().expect("headless render");
        let out = ctx.take_output();
        let t4 = Instant::now();
        pipe.write_all(out.as_bytes()).expect("pipe write");
        pipe.flush().expect("pipe flush");
        let t5 = Instant::now();
        let us = |a: Instant, b: Instant| (b - a).as_micros() as u64;
        s.gen.push(us(t0, t1));
        s.fit.push(us(t1, t2));
        s.lower.push(us(t2, t3));
        s.render.push(us(t3, t4));
        s.write.push(us(t4, t5));
        s.bytes.push(out.len() as u64);
        s.allocs.push(ALLOCS.load(Ordering::Relaxed) - a0);
        digest = (digest ^ fnv(out.as_bytes())).wrapping_mul(0x0100_0000_01b3);
    }
    let stats = ctx.stats();
    s.dirty.push(stats.dirty_cells / u64::from(frames.max(1)));
    s.digest = digest;
    s
}

fn configs(grids: &[(u16, u16)]) -> Vec<Config> {
    let mut v = Vec::new();
    for &(cols, rows) in grids {
        for depth in [
            ColorDepth::TrueColor,
            ColorDepth::Ansi256,
            ColorDepth::Ansi16,
        ] {
            v.push(Config {
                cols,
                rows,
                lowering: Lowering::HalfBlockColor,
                depth,
            });
        }
        for mode in [
            SubcellGlyphMode::Braille2x4,
            SubcellGlyphMode::HalfBlock1x2,
            SubcellGlyphMode::Block,
            SubcellGlyphMode::Ascii,
        ] {
            v.push(Config {
                cols,
                rows,
                lowering: Lowering::Mono(mode),
                depth: ColorDepth::Mono,
            });
        }
    }
    v
}

fn parse_grids(s: &str) -> Vec<(u16, u16)> {
    s.split(',')
        .filter_map(|g| {
            let (c, r) = g.split_once('x')?;
            Some((c.trim().parse().ok()?, r.trim().parse().ok()?))
        })
        .collect()
}

fn selfcheck() -> Result<(), String> {
    // Determinism: two runs, identical frame digests.
    let cfg = Config {
        cols: 80,
        rows: 25,
        lowering: Lowering::HalfBlockColor,
        depth: ColorDepth::TrueColor,
    };
    let (mut p1, d1) = drained_pipe();
    let a = run(cfg, 12, &mut p1);
    drop(p1);
    let (mut p2, d2) = drained_pipe();
    let b = run(cfg, 12, &mut p2);
    drop(p2);
    let (n1, n2) = (d1.join().unwrap_or(0), d2.join().unwrap_or(0));
    if a.digest != b.digest {
        return Err(format!(
            "frame digests differ across runs: {:x} vs {:x}",
            a.digest, b.digest
        ));
    }
    if n1 != a.bytes.iter().sum::<u64>() || n1 != n2 {
        return Err(format!(
            "pipe conservation: wrote {} read {n1}/{n2}",
            a.bytes.iter().sum::<u64>()
        ));
    }
    // Every lowering produces a non-empty first frame at every default grid.
    for cfg in configs(&[(80, 25), (120, 40), (200, 60)]) {
        let (mut p, d) = drained_pipe();
        let s = run(cfg, 1, &mut p);
        drop(p);
        let _ = d.join();
        if s.bytes[0] == 0 {
            return Err(format!(
                "{}x{} {} produced no bytes",
                cfg.cols,
                cfg.rows,
                cfg.lowering.name()
            ));
        }
    }
    println!(
        "raster_ingest_bench selfcheck: OK digest={:016x} frames=12 bytes={n1}",
        a.digest
    );
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--selfcheck") {
        if let Err(e) = selfcheck() {
            eprintln!("raster_ingest_bench selfcheck: FAIL {e}");
            std::process::exit(1);
        }
        return;
    }
    let mut frames = 120u32;
    let mut grids = vec![(80u16, 25u16), (120, 40), (200, 60)];
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--frames" => frames = it.next().and_then(|v| v.parse().ok()).unwrap_or(frames),
            "--grids" => grids = it.next().map(|v| parse_grids(v)).unwrap_or(grids),
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    let profile = if cfg!(debug_assertions) {
        "debug (timings not meaningful)"
    } else {
        "release"
    };
    println!("# raster_ingest_bench frames={frames} profile={profile} source=320x200 write=pipe-not-terminal");
    println!("grid\tlowering\tdepth\tgen_p50\tgen_p95\tfit_p50\tfit_p95\tlower_p50\tlower_p95\trender_p50\trender_p95\twrite_p50\twrite_p95\ttotal_p50\tbytes_mean\tdirty_mean\tallocs_mean\tdigest");
    let (mut pipe, drain) = drained_pipe();
    for cfg in configs(&grids) {
        let s = run(cfg, frames, &mut pipe);
        let total: Vec<u64> = (0..s.gen.len())
            .map(|i| s.gen[i] + s.fit[i] + s.lower[i] + s.render[i] + s.write[i])
            .collect();
        println!(
            "{}x{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:016x}",
            cfg.cols,
            cfg.rows,
            cfg.lowering.name(),
            depth_name(cfg.depth),
            pct(&s.gen, 0.5),
            pct(&s.gen, 0.95),
            pct(&s.fit, 0.5),
            pct(&s.fit, 0.95),
            pct(&s.lower, 0.5),
            pct(&s.lower, 0.95),
            pct(&s.render, 0.5),
            pct(&s.render, 0.95),
            pct(&s.write, 0.5),
            pct(&s.write, 0.95),
            pct(&total, 0.5),
            mean(&s.bytes),
            mean(&s.dirty),
            mean(&s.allocs),
            s.digest
        );
    }
    drop(pipe);
    let drained = drain.join().unwrap_or(0);
    println!(
        "# pipe drained {drained} bytes; times in microseconds; alloc bytes total {}",
        ALLOC_BYTES.load(Ordering::Relaxed)
    );
}
