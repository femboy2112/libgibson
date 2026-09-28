//! Projector micro-benchmark: measure-first instrumentation for the temporal
//! subcell projector hot path.
//!
//! Times [`gibson::temporal::project_braille_image`] across the cell counts a real
//! interactive application hits — a single cell up to a full 160x50 field — and
//! reports milliseconds per full-image projection and throughput in Mcell/s. Run
//! in release; debug is ~15-20x slower and not representative:
//!
//!   cargo run --release --example temporal_projector_bench
//!   cargo run --release --example temporal_projector_bench -- --iters=200
//!
//! This is a research instrument. It makes no perceptual or presentation claim; it
//! only measures CPU projection cost so optimization decisions are evidence-based.

use gibson::temporal::project_braille_image;
use gibson::{PresentationProfile, ResetPolicy, SubcellGlyphMode, TemporalDisplayProcessor};
use std::time::Instant;

/// A deterministic, non-degenerate per-subpixel sample: varied enough that the
/// projector does real partition work rather than hitting the uniform-cell fast
/// path, and reproducible across runs so before/after numbers are comparable.
fn sample(lx: u16, ly: u16) -> [u8; 3] {
    let r = (lx.wrapping_mul(7) ^ ly.wrapping_mul(13)) as u8;
    let g = (lx as u8).wrapping_add((ly as u8).wrapping_mul(5));
    let b = (ly as u8).wrapping_mul(3).wrapping_add(40);
    [r, g, b]
}

fn bench(w: u16, h: u16, iters: u32) {
    // Warm up (touch caches, force codegen paths).
    std::hint::black_box(&project_braille_image(w, h, sample));

    let start = Instant::now();
    for _ in 0..iters {
        std::hint::black_box(&project_braille_image(w, h, sample));
    }
    let elapsed = start.elapsed();

    let cells = (w as f64) * (h as f64);
    let per_ms = elapsed.as_secs_f64() * 1000.0 / iters as f64;
    let mcell_s = cells * iters as f64 / elapsed.as_secs_f64() / 1.0e6;
    println!("{w:>3}x{h:<3} ({cells:>6.0} cells, {iters:>7} iters):  {per_ms:8.4} ms/projection   {mcell_s:8.2} Mcell/s");
}

/// A smooth low-swing gradient: cells are safety-eligible, so the processor
/// actually modulates and this measures the residual (temporal-active) frame cost.
fn smooth(lx: u16, ly: u16) -> [u8; 3] {
    let v = 60u8.wrapping_add(((lx as u32 + ly as u32) & 0x1f) as u8);
    [v, v, v]
}

/// The per-frame cost of the temporal-active path: `advance` builds one ordinary
/// Surface via the residual sigma-delta. This is what runs every frame while
/// content is settled — the projector does NOT run per frame in that regime.
fn bench_advance(w: u16, h: u16, iters: u32) {
    let mut p = TemporalDisplayProcessor::new(w, h, SubcellGlyphMode::Braille2x4, 0xB0A);
    p.set_profile(PresentationProfile::measured(120.0, 0.99, 0.5));
    p.set_target_image(smooth, ResetPolicy::Reset);
    std::hint::black_box(&p.advance(0));

    let start = Instant::now();
    for _ in 0..iters {
        std::hint::black_box(&p.advance(0));
    }
    let elapsed = start.elapsed();
    let per_us = elapsed.as_secs_f64() * 1e6 / iters as f64;
    let modulating = p.diagnostics().modulating;
    println!(
        "advance  {w:>3}x{h:<3} (modulating={modulating}):  {per_us:8.3} us/frame   ({:>7.0} fps ceiling)",
        1e6 / per_us
    );
}

/// The interactive-edit cost: reprojecting only a moving sub-rect of a full field,
/// the Phase 3 incremental path. This is what a moving reticle/scan line costs.
fn bench_region(w: u16, h: u16, rw: u16, rh: u16, iters: u32) {
    let mut p = TemporalDisplayProcessor::new(w, h, SubcellGlyphMode::Braille2x4, 0xB0A);
    p.set_target_image(sample, ResetPolicy::Reset);
    std::hint::black_box(p.set_target_region(0, 0, rw, rh, sample, ResetPolicy::Keep));

    let span = w.saturating_sub(rw).saturating_add(1);
    let start = Instant::now();
    for i in 0..iters {
        let ox = (i as u16) % span;
        std::hint::black_box(p.set_target_region(ox, 0, rw, rh, sample, ResetPolicy::Keep));
    }
    let elapsed = start.elapsed();
    let per_ms = elapsed.as_secs_f64() * 1000.0 / iters as f64;
    println!("region {rw:>2}x{rh:<2} reproj in {w}x{h}:  {per_ms:8.4} ms/update");
}

fn main() {
    let base_iters = std::env::args()
        .find_map(|a| {
            a.strip_prefix("--iters=")
                .and_then(|v| v.parse::<u32>().ok())
        })
        .unwrap_or(100);

    println!("temporal projector benchmark (release recommended); base --iters={base_iters}\n");
    println!("full-image projection (settle-time / scene-change cost):");
    for (w, h) in [
        (1u16, 1u16),
        (4, 2),
        (8, 4),
        (32, 16),
        (80, 24),
        (120, 40),
        (160, 50),
    ] {
        // Give tiny fields more repetitions so timing resolution is comparable.
        let scaled = ((base_iters as u64 * 8000) / ((w as u64) * (h as u64)).max(1))
            .clamp(base_iters as u64, 5_000_000) as u32;
        bench(w, h, scaled);
    }

    println!("\nper-frame runtime cost (the temporal-active hot path — no reprojection):");
    bench_advance(80, 24, 20_000);
    bench_advance(160, 50, 8_000);

    println!("\nincremental region reprojection (Phase 3 interactive-edit cost):");
    bench_region(160, 50, 32, 16, 2_000);
    bench_region(160, 50, 8, 4, 20_000);
}
