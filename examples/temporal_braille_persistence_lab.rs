//! Temporal Braille persistence lab.
//!
//! A research instrument for one hypothesis: can rapid sequences of ordinary
//! Braille-cell [`gibson::Surface`]s act as temporal pulse-density modulation so
//! a W x H terminal behaves perceptually more like a 2W x 4H logical field with
//! finer tonal resolution, with the quantization residual shaped into fine
//! analog grain rather than coherent flicker?
//!
//! It adds **no renderer**. Every emitted frame is an ordinary `Surface`; the
//! library's one renderer owns terminal state. The lab reuses
//! `temporal::project_braille_image`, `TemporalBrailleField` and
//! `TemporalDisplayProcessor` for the shipping algorithms and adds only
//! deterministic schedules + measurement code (research-local).
//!
//! Modes:
//!   --mode=matrix      static vs temporal reconstruction matrix (A)
//!   --mode=montecarlo  seed-to-seed spread of the fidelity claim (A2)
//!   --mode=spectrum    residual spectrum / coherent-flicker proxies (B)
//!   --mode=pareto      fidelity vs coherence Pareto frontier (B2)
//!   --mode=loss        dropped-phase presentation hostility (C)
//!   --mode=framelocal  frame-local temporal video (D)
//!   --mode=reach       H3 reachable-space / rank demonstration
//!   --mode=decompose   H1/H2 energy decomposition with a rigorous floor bracket
//!   --mode=color       temporal-aware colour-choice headroom
//!   --mode=live        interactive A/B/grain live demo (requires a TTY)
//!
//! Flags: `--cols=`, `--rows=`, `--k=`, `--n=`, `--seed=`, `--quick`.
//!
//! Live flags (`--mode=live`):
//!   --measured-hz=<hz>  real presentation cadence (honest profile)
//!   --assume-hz=<hz>    assumed cadence if unmeasured (default 120; labelled ASSUMED!)
//!   --no-temporal       static fallback only
//!   --force             visualize modulation even if the gate is not Enabled (demo only)
//!   --slowmo=<hz>       advance the PDM phase at `hz` so the eye can resolve it
//!   --dwell=<s>         seconds per A/B/grain arm (default 3)
//!   --seconds=<s>       total run length (default 24)

// Research lab: several helpers exercise the full measurement surface and are
// only used from opt-in modes or the unit tests, so dead-code analysis is off.
// The numeric kernels index fixed-size masks directly, where an enumerate
// rewrite would obscure the maths; that lint is off for this example only.
#![allow(dead_code, clippy::needless_range_loop, clippy::type_complexity)]

#[path = "temporal_braille_persistence_lab/floors.rs"]
mod floors;
#[path = "temporal_braille_persistence_lab/framelocal.rs"]
mod framelocal;
#[path = "temporal_braille_persistence_lab/metrics.rs"]
mod metrics;
#[path = "temporal_braille_persistence_lab/schedules.rs"]
mod schedules;
#[path = "temporal_braille_persistence_lab/targets.rs"]
mod targets;

use framelocal::{FrameLocalScheduler, LossModel, StaleAccumulator};
use gibson::temporal::{
    project_braille_image, BrailleImageProjection, ResetPolicy, TemporalBrailleField,
};
use gibson::{AnsiCompiler, Color, ColorDepth, Style, SubcellGlyphMode};
use metrics::{recon_stats, spectrum_stats, CellMetric, ReconStats};
use schedules::{ScheduleKind, K_CHOICES};
use std::io;
use std::time::Instant;
use targets::{classify_cell, named_targets, LogicalImage};

// ---------------------------------------------------------------------------
// Args
// ---------------------------------------------------------------------------

fn arg_str(name: &str) -> Option<String> {
    let prefix = format!("--{name}=");
    std::env::args().find_map(|a| a.strip_prefix(&prefix).map(str::to_string))
}

fn arg_u32(name: &str, default: u32) -> u32 {
    arg_str(name)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn has(flag: &str) -> bool {
    std::env::args().any(|a| a == flag)
}

struct Config {
    mode: String,
    cols: u16,
    rows: u16,
    k: usize,
    n: usize,
    seed: u64,
    seeds: usize,
    quick: bool,
}

impl Config {
    fn parse() -> Self {
        let quick = has("--quick");
        let cols = arg_u32("cols", if quick { 32 } else { 56 }) as u16;
        let rows = arg_u32("rows", if quick { 12 } else { 18 }) as u16;
        Self {
            mode: arg_str("mode").unwrap_or_else(|| "matrix".into()),
            cols: cols.clamp(4, 160),
            rows: rows.clamp(2, 60),
            k: arg_u32("k", 4) as usize,
            n: arg_u32("n", 256) as usize,
            seed: arg_u32("seed", 0x7E5) as u64,
            seeds: arg_u32("seeds", 48).max(2) as usize,
            quick,
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn project(img: &LogicalImage) -> BrailleImageProjection {
    project_braille_image(img.cols, img.rows, |lx, ly| img.sample(lx, ly))
}

type CellDatum = ([[u8; 3]; 8], Style, [f32; 8], targets::CellClass);

fn per_cell_data(img: &LogicalImage, proj: &BrailleImageProjection) -> Vec<CellDatum> {
    let mut out = Vec::with_capacity(proj.width() as usize * proj.height() as usize);
    for y in 0..proj.height() {
        for x in 0..proj.width() {
            let cell = proj.cell(x, y).expect("in-bounds");
            let source = img.cell_samples(x, y);
            out.push((source, cell.style, cell.duty, classify_cell(&source)));
        }
    }
    out
}

/// The masks of the stored static realization, in row-major order.
fn static_masks(proj: &BrailleImageProjection) -> Vec<u8> {
    let mut out = Vec::with_capacity(proj.width() as usize * proj.height() as usize);
    for y in 0..proj.height() {
        for x in 0..proj.width() {
            out.push(proj.cell(x, y).expect("in-bounds").static_mask);
        }
    }
    out
}

fn pure_plan(
    proj: &BrailleImageProjection,
    kind: ScheduleKind,
    k: usize,
    seed: u64,
    window: u64,
) -> Vec<Vec<u8>> {
    FrameLocalScheduler::new(k, kind, seed)
        .plan(proj, window)
        .masks
}

fn library_masks(
    proj: &BrailleImageProjection,
    k: usize,
    dither: bool,
    residual: bool,
    seed: u64,
) -> Vec<Vec<u8>> {
    let mut field = TemporalBrailleField::new(proj.width(), proj.height(), seed);
    assert!(proj.install_into(&mut field, ResetPolicy::Reset));
    field.set_dither(dither);
    let mut out = Vec::with_capacity(k);
    for _ in 0..k {
        let surface = if residual {
            field.advance_residual_styled(SubcellGlyphMode::Braille2x4)
        } else {
            field.advance_styled(SubcellGlyphMode::Braille2x4)
        };
        out.push(metrics::surface_masks(&surface));
    }
    out
}

// ---------------------------------------------------------------------------
// A. Reconstruction matrix
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Sched {
    Pure(ScheduleKind),
    LibraryFull,
    LibraryResidual,
    LibraryDithered,
}

impl Sched {
    fn as_str(self) -> &'static str {
        match self {
            Sched::Pure(k) => k.as_str(),
            Sched::LibraryFull => "lib-full-sigma-delta",
            Sched::LibraryResidual => "lib-residual-sigma-delta",
            Sched::LibraryDithered => "lib-dithered-residual",
        }
    }

    fn masks(
        self,
        proj: &BrailleImageProjection,
        k: usize,
        seed: u64,
        window: u64,
    ) -> Vec<Vec<u8>> {
        match self {
            Sched::Pure(kind) => pure_plan(proj, kind, k, seed, window),
            Sched::LibraryFull => library_masks(proj, k, false, false, seed),
            Sched::LibraryResidual => library_masks(proj, k, false, true, seed),
            Sched::LibraryDithered => library_masks(proj, k, true, true, seed),
        }
    }
}

const SCHEDS: [Sched; 8] = [
    Sched::Pure(ScheduleKind::NaiveAligned),
    Sched::Pure(ScheduleKind::WindowedErrorFeedback),
    Sched::Pure(ScheduleKind::ResidualWindowedEF),
    Sched::Pure(ScheduleKind::StochasticRound),
    Sched::Pure(ScheduleKind::VdcBalanced),
    Sched::LibraryFull,
    Sched::LibraryResidual,
    Sched::LibraryDithered,
];

fn mean_metric(stats: &[ReconStats], f: impl Fn(&ReconStats) -> f32) -> f32 {
    if stats.is_empty() {
        return 0.0;
    }
    stats.iter().map(f).sum::<f32>() / stats.len() as f32
}

/// A projected target plus its per-cell data and emitted static baseline.
struct Prepared {
    name: String,
    proj: BrailleImageProjection,
    data: Vec<CellDatum>,
    static_stats: ReconStats,
}

fn prepare_targets(cfg: &Config) -> Vec<Prepared> {
    named_targets(cfg.cols, cfg.rows, cfg.seed)
        .into_iter()
        .map(|(name, img)| {
            let proj = project(&img);
            let data = per_cell_data(&img, &proj);
            let static_stats = recon_stats(&data, &[static_masks(&proj)]);
            Prepared {
                name,
                proj,
                data,
                static_stats,
            }
        })
        .collect()
}

fn mean_and_ci(xs: &[f32]) -> (f32, f32, f32, f32, f32) {
    let n = xs.len().max(1) as f32;
    let mean = xs.iter().sum::<f32>() / n;
    let var = if xs.len() > 1 {
        xs.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / (n - 1.0)
    } else {
        0.0
    };
    let std = var.sqrt();
    let ci95 = 1.96 * std / n.sqrt();
    let lo = xs.iter().cloned().fold(f32::INFINITY, f32::min);
    let hi = xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    (mean, std, ci95, lo, hi)
}

fn run_matrix(cfg: &Config) {
    let prepared = prepare_targets(cfg);
    println!(
        "== A. static vs temporal reconstruction ({}x{} cells, {} targets) ==",
        cfg.cols,
        cfg.rows,
        prepared.len()
    );
    let base = mean_metric(
        &prepared.iter().map(|t| t.static_stats).collect::<Vec<_>>(),
        |s| s.rmse_source,
    );
    println!("static baseline: mean emitted static RMSE over targets = {base:.5}");
    println!();
    println!(
        "{:<26} {:>4} {:>9} {:>9} {:>9} {:>9} {:>8} {:>9} {:>8}",
        "schedule", "K", "rmse", "impr%", "edgeRMSE", "lineRMSE", "|bias|", "flips/ph", "cells/ph"
    );
    for sched in SCHEDS {
        for k in K_CHOICES {
            let stats: Vec<ReconStats> = prepared
                .iter()
                .map(|t| recon_stats(&t.data, &sched.masks(&t.proj, k, cfg.seed, 0)))
                .collect();
            let rmse = mean_metric(&stats, |s| s.rmse_source);
            let impr = if base > 0.0 {
                100.0 * (base - rmse) / base
            } else {
                0.0
            };
            println!(
                "{:<26} {:>4} {:>9.5} {:>9.2} {:>9.5} {:>9.5} {:>8.4} {:>9.1} {:>8.1}",
                sched.as_str(),
                k,
                rmse,
                impr,
                mean_metric(&stats, |s| s.rmse_source_edge),
                mean_metric(&stats, |s| s.rmse_line),
                mean_metric(&stats, |s| s.abs_bias),
                mean_metric(&stats, |s| s.flips_per_phase),
                mean_metric(&stats, |s| s.changed_cells_per_phase),
            );
        }
    }
    println!();
    println!("-- per-target K-sweep for the two reference schedules --");
    println!(
        "{:<18} {:>4} {:>9} {:>9} {:>9}",
        "target", "K", "windowed-ef", "res-windowed", "lib-dithered"
    );
    let shown = if cfg.quick { 4 } else { prepared.len() };
    for t in prepared.iter().take(shown) {
        for k in K_CHOICES {
            let wef = recon_stats(
                &t.data,
                &pure_plan(&t.proj, ScheduleKind::WindowedErrorFeedback, k, cfg.seed, 0),
            );
            let rwef = recon_stats(
                &t.data,
                &pure_plan(&t.proj, ScheduleKind::ResidualWindowedEF, k, cfg.seed, 0),
            );
            let lib = recon_stats(&t.data, &library_masks(&t.proj, k, true, true, cfg.seed));
            println!(
                "{:<18} {:>4} {:>9.5} {:>9.5} {:>9.5}",
                t.name, k, wef.rmse_source, rwef.rmse_source, lib.rmse_source
            );
        }
    }
}

// ---------------------------------------------------------------------------
// A2. Monte Carlo robustness of the fidelity claim
// ---------------------------------------------------------------------------

fn run_montecarlo(cfg: &Config) {
    let prepared = prepare_targets(cfg);
    let base = mean_metric(
        &prepared.iter().map(|t| t.static_stats).collect::<Vec<_>>(),
        |s| s.rmse_source,
    );
    let seeds = cfg.seeds;
    println!(
        "== A2. Monte Carlo over {seeds} seeds (K={}, {} targets, static baseline {base:.5}) ==",
        cfg.k,
        prepared.len()
    );
    println!("Every schedule but naive-aligned is a deterministic function of its seed, so the");
    println!("headline improvement must be reported with its seed-to-seed spread, not one draw.\n");
    println!(
        "{:<24} {:>9} {:>9} {:>9} {:>8} {:>9} {:>9} {:>7}",
        "schedule", "meanRMSE", "std", "ci95", "impr%", "imprLo", "imprHi", "win%"
    );
    let mut all_rmse: Vec<Vec<f32>> = Vec::with_capacity(SCHEDS.len());
    for sched in SCHEDS {
        let mut vals = Vec::with_capacity(seeds);
        for s in 0..seeds {
            let seed = cfg.seed ^ (s as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let stats: Vec<ReconStats> = prepared
                .iter()
                .map(|t| recon_stats(&t.data, &sched.masks(&t.proj, cfg.k, seed, 0)))
                .collect();
            vals.push(mean_metric(&stats, |s| s.rmse_source));
        }
        all_rmse.push(vals);
    }
    // Win rate: which schedule has the lowest mean RMSE on each seed.
    let mut wins = vec![0usize; SCHEDS.len()];
    for s in 0..seeds {
        let mut best = 0usize;
        for (i, v) in all_rmse.iter().enumerate() {
            if v[s] < all_rmse[best][s] {
                best = i;
            }
        }
        wins[best] += 1;
    }
    for (i, sched) in SCHEDS.iter().enumerate() {
        let (mean, std, ci, lo, hi) = mean_and_ci(&all_rmse[i]);
        let impr = if base > 0.0 {
            100.0 * (base - mean) / base
        } else {
            0.0
        };
        let lo_i = if base > 0.0 {
            100.0 * (base - hi) / base
        } else {
            0.0
        };
        let hi_i = if base > 0.0 {
            100.0 * (base - lo) / base
        } else {
            0.0
        };
        let _ = (ci, lo_i);
        println!(
            "{:<24} {:>9.5} {:>9.6} {:>9.6} {:>8.2} {:>9.2} {:>9.2} {:>7.1}",
            sched.as_str(),
            mean,
            std,
            ci,
            impr,
            lo_i,
            hi_i,
            100.0 * wins[i] as f32 / seeds as f32,
        );
    }
    println!();
    println!("imprLo/imprHi bracket the per-seed improvement range, not a confidence interval;");
    println!("ci95 is the standard error of the mean times 1.96. win% counts seeds where the");
    println!("schedule had the lowest mean RMSE. A schedule whose ci95 swamps its margin over");
    println!("the next is not distinguishable on this corpus.");
}

// ---------------------------------------------------------------------------
// B. Residual spectrum
// ---------------------------------------------------------------------------

struct SpectrumRow {
    name: &'static str,
    stats: metrics::SpectrumStats,
}

/// `n` frames of a library long-horizon field, from a fresh install.
fn library_frames(
    proj: &BrailleImageProjection,
    n: usize,
    dither: bool,
    residual: bool,
    seed: u64,
) -> Vec<Vec<u8>> {
    let mut field = TemporalBrailleField::new(proj.width(), proj.height(), seed);
    assert!(proj.install_into(&mut field, ResetPolicy::Reset));
    field.set_dither(dither);
    let mut frames = Vec::with_capacity(n);
    for _ in 0..n {
        let s = if residual {
            field.advance_residual_styled(SubcellGlyphMode::Braille2x4)
        } else {
            field.advance_styled(SubcellGlyphMode::Braille2x4)
        };
        frames.push(metrics::surface_masks(&s));
    }
    frames
}

/// `n` frames of any schedule: pure schedules are chained window over window,
/// library schedules advance a real `TemporalBrailleField`.
fn schedule_spectrum_frames(
    sched: Sched,
    proj: &BrailleImageProjection,
    k: usize,
    n: usize,
    seed: u64,
) -> Vec<Vec<u8>> {
    match sched {
        Sched::Pure(kind) => {
            let mut frames = Vec::with_capacity(n);
            let mut window = 0u64;
            while frames.len() < n {
                frames.extend(pure_plan(proj, kind, k, seed, window));
                window += 1;
            }
            frames.truncate(n);
            frames
        }
        Sched::LibraryFull => library_frames(proj, n, false, false, seed),
        Sched::LibraryResidual => library_frames(proj, n, false, true, seed),
        Sched::LibraryDithered => library_frames(proj, n, true, true, seed),
    }
}

fn run_spectrum(cfg: &Config) {
    let n = cfg.n.max(16);
    let k = cfg.k.max(2);
    let (name, img) = if cfg.quick {
        (
            "smooth-gradient",
            targets::smooth_gradient(cfg.cols, cfg.rows, cfg.seed),
        )
    } else {
        ("portrait", targets::portrait(cfg.cols, cfg.rows, cfg.seed))
    };
    let proj = project(&img);
    let cells = proj.width() as usize * proj.height() as usize;
    println!(
        "== B. residual spectrum / coherent-flicker proxies ({name}, {cells} cells, n={n}, frame-local K={k}) =="
    );
    println!(
        "{:<26} {:>8} {:>9} {:>9} {:>11} {:>9}",
        "schedule", "meanLit", "lfRatio", "acf1", "flashVar", "dotCorr"
    );

    let rows: Vec<SpectrumRow> = SCHEDS
        .iter()
        .map(|&sched| SpectrumRow {
            name: sched.as_str(),
            stats: spectrum_stats(&schedule_spectrum_frames(sched, &proj, k, n, cfg.seed)),
        })
        .collect();

    for r in &rows {
        println!(
            "{:<26} {:>8.4} {:>9.4} {:>9.4} {:>11.6} {:>9.4}",
            r.name,
            r.stats.mean_lit,
            r.stats.lf_ratio,
            r.stats.acf1,
            r.stats.flash_var,
            r.stats.dot_corr,
        );
    }
    println!();
    println!(
        "lfRatio = AC energy in the lowest 1/8 of temporal bins; acf1 = lag-1 autocorrelation;"
    );
    println!("flashVar = variance of the whole-region lit fraction (global breathing);");
    println!("dotCorr = mean within-cell pairwise dot correlation of the temporal residual.");
}

// ---------------------------------------------------------------------------
// B2. Fidelity / coherence Pareto frontier
// ---------------------------------------------------------------------------

fn run_pareto(cfg: &Config) {
    let prepared = prepare_targets(cfg);
    let base = mean_metric(
        &prepared.iter().map(|t| t.static_stats).collect::<Vec<_>>(),
        |s| s.rmse_source,
    );
    let n = cfg.n.max(64);
    let seeds = cfg.seeds.clamp(1, 16);
    let (name, img) = if cfg.quick {
        (
            "smooth-gradient",
            targets::smooth_gradient(cfg.cols, cfg.rows, cfg.seed),
        )
    } else {
        ("portrait", targets::portrait(cfg.cols, cfg.rows, cfg.seed))
    };
    let proj = project(&img);
    println!(
        "== B2. fidelity vs coherence Pareto frontier (K={}, {name}, n={n}, {seeds} fidelity seeds) ==",
        cfg.k
    );
    println!("Fidelity is the mean reconstruction improvement over the targets; coherence is the");
    println!("temporal residual's self-similarity. We want high improvement AND low dotCorr and");
    println!("flashVar. A schedule is *dominated* if another beats it on all three.\n");

    struct Pt {
        name: &'static str,
        impr: f32,
        dot_corr: f32,
        flash_var: f32,
        lf_ratio: f32,
    }
    let mut pts: Vec<Pt> = Vec::with_capacity(SCHEDS.len());
    for &sched in SCHEDS.iter() {
        let mut acc = 0f32;
        for s in 0..seeds {
            let seed = cfg.seed ^ (s as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let stats: Vec<ReconStats> = prepared
                .iter()
                .map(|t| recon_stats(&t.data, &sched.masks(&t.proj, cfg.k, seed, 0)))
                .collect();
            acc += mean_metric(&stats, |s| s.rmse_source);
        }
        let rmse = acc / seeds as f32;
        let impr = if base > 0.0 {
            100.0 * (base - rmse) / base
        } else {
            0.0
        };
        let st = spectrum_stats(&schedule_spectrum_frames(sched, &proj, cfg.k, n, cfg.seed));
        pts.push(Pt {
            name: sched.as_str(),
            impr,
            dot_corr: st.dot_corr,
            flash_var: st.flash_var,
            lf_ratio: st.lf_ratio,
        });
    }
    let dominated = |i: usize| -> bool {
        pts.iter().enumerate().any(|(j, q)| {
            j != i
                && q.impr >= pts[i].impr - 1e-4
                && q.dot_corr <= pts[i].dot_corr + 1e-6
                && q.flash_var <= pts[i].flash_var + 1e-9
                && (q.impr > pts[i].impr + 1e-4
                    || q.dot_corr < pts[i].dot_corr - 1e-6
                    || q.flash_var < pts[i].flash_var - 1e-9)
        })
    };
    println!(
        "{:<24} {:>9} {:>9} {:>11} {:>9} {:>6}",
        "schedule", "impr%", "dotCorr", "flashVar", "lfRatio", "Pareto"
    );
    for i in 0..pts.len() {
        println!(
            "{:<24} {:>9.2} {:>9.4} {:>11.6} {:>9.4} {:>6}",
            pts[i].name,
            pts[i].impr,
            pts[i].dot_corr,
            pts[i].flash_var,
            pts[i].lf_ratio,
            if dominated(i) { "no" } else { "YES" },
        );
    }
    println!();
    println!("The frontier is the honest headline: fidelity alone (naive-aligned) is not the");
    println!("recommendation, because it buys its margin with the most coherent residual.");
}

// ---------------------------------------------------------------------------
// C. Dropped-phase hostility
// ---------------------------------------------------------------------------

fn run_loss(cfg: &Config) {
    let (name, img) = (
        "smooth-gradient",
        targets::smooth_gradient(cfg.cols, cfg.rows, cfg.seed),
    );
    let proj = project(&img);
    let data = per_cell_data(&img, &proj);
    println!("== C. presentation-loss hostility ({name}) ==");
    println!(
        "{:<26} {:>4} {:>14} {:>9} {:>9} {:>9}",
        "schedule", "K", "loss", "rmse", "|bias|", "lineRMSE"
    );
    let losses = [
        LossModel::Full,
        LossModel::Half,
        LossModel::DropEvery3,
        LossModel::Random10,
        LossModel::Random20,
    ];
    for kind in [
        ScheduleKind::NaiveAligned,
        ScheduleKind::WindowedErrorFeedback,
        ScheduleKind::ResidualWindowedEF,
        ScheduleKind::StochasticRound,
        ScheduleKind::VdcBalanced,
    ] {
        for k in [4usize, 6, 8] {
            let plan = pure_plan(&proj, kind, k, cfg.seed, 0);
            for loss in losses {
                let keep = loss.survivors(k, cfg.seed);
                let kept: Vec<Vec<u8>> = plan
                    .iter()
                    .zip(&keep)
                    .filter(|(_, k)| **k)
                    .map(|(m, _)| m.clone())
                    .collect();
                let s = recon_stats(&data, &kept);
                println!(
                    "{:<26} {:>4} {:>14} {:>9.5} {:>9.4} {:>9.5}",
                    kind.as_str(),
                    k,
                    loss.as_str(),
                    s.rmse_source,
                    s.abs_bias,
                    s.rmse_line,
                );
            }
        }
    }
    // The shipping dithered residual for the same conditions.
    for k in [4usize, 6, 8] {
        let plan = library_masks(&proj, k, true, true, cfg.seed);
        for loss in losses {
            let keep = loss.survivors(k, cfg.seed);
            let kept: Vec<Vec<u8>> = plan
                .iter()
                .zip(&keep)
                .filter(|(_, k)| **k)
                .map(|(m, _)| m.clone())
                .collect();
            let s = recon_stats(&data, &kept);
            println!(
                "{:<26} {:>4} {:>14} {:>9.5} {:>9.4} {:>9.5}",
                "lib-dithered-residual",
                k,
                loss.as_str(),
                s.rmse_source,
                s.abs_bias,
                s.rmse_line,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// D. Frame-local temporal video
// ---------------------------------------------------------------------------

fn run_framelocal(cfg: &Config) {
    let sources = if cfg.quick { 8 } else { 24 };
    let cols = cfg.cols;
    let rows = cfg.rows;
    let cells = cols as usize * rows as usize;
    println!(
        "== D. frame-local temporal video (moving bar, {sources} source frames, {cols}x{rows}) =="
    );

    // Build every source frame's projection once.
    let mut projections = Vec::with_capacity(sources);
    let mut datas = Vec::with_capacity(sources);
    let mut project_us = 0f64;
    for f in 0..sources {
        let img = targets::moving_bar(cols, rows, f as f32 / sources as f32);
        let t0 = Instant::now();
        let proj = project(&img);
        project_us += t0.elapsed().as_secs_f64() * 1e6;
        datas.push(per_cell_data(&img, &proj));
        projections.push(proj);
    }
    project_us /= sources as f64;

    // Static baseline per frame.
    let static_stats: Vec<ReconStats> = projections
        .iter()
        .zip(&datas)
        .map(|(p, d)| recon_stats(d, &[static_masks(p)]))
        .collect();
    let static_rmse = mean_metric(&static_stats, |s| s.rmse_source);
    println!(
        "static baseline rmse = {static_rmse:.5}; projection = {project_us:.1} us/source-frame"
    );
    println!();
    println!(
        "{:<26} {:>4} {:>9} {:>8} {:>9} {:>9} {:>9}",
        "schedule", "K", "rmse", "impr%", "firstSub", "planUs/sf", "ansiB/sf"
    );

    for kind in [
        ScheduleKind::WindowedErrorFeedback,
        ScheduleKind::ResidualWindowedEF,
        ScheduleKind::VdcBalanced,
        ScheduleKind::NaiveAligned,
    ] {
        for k in K_CHOICES {
            let mut plan_us = 0f64;
            let mut stats = Vec::with_capacity(sources);
            let mut first_stats = Vec::with_capacity(sources);
            for (p, d) in projections.iter().zip(&datas) {
                let t0 = Instant::now();
                let plan = pure_plan(p, kind, k, cfg.seed, 0);
                plan_us += t0.elapsed().as_secs_f64() * 1e6;
                let avg = framelocal::average_duty(&plan, &vec![true; k], cells);
                stats.push(recon_from_avg(d, &avg));
                first_stats.push(recon_from_avg(
                    d,
                    &framelocal::average_duty(&plan[..1], &[true], cells),
                ));
            }
            plan_us /= sources as f64;
            let ansi = ansi_bytes_per_source_frame(&projections[0], kind, k, cfg.seed, cells);
            let rmse = mean_metric(&stats, |s| s.rmse_source);
            let impr = if static_rmse > 0.0 {
                100.0 * (static_rmse - rmse) / static_rmse
            } else {
                0.0
            };
            println!(
                "{:<26} {:>4} {:>9.5} {:>8.2} {:>9.5} {:>9.1} {:>9.1}",
                kind.as_str(),
                k,
                rmse,
                impr,
                mean_metric(&first_stats, |s| s.rmse_source),
                plan_us,
                ansi,
            );
        }
    }

    // Stale long-horizon accumulator across motion: the failure this avoids.
    {
        let mut stale = StaleAccumulator::new(cols, rows, cfg.seed);
        let k = cfg.k.max(2);
        let mut stats = Vec::with_capacity(sources);
        for (p, d) in projections.iter().zip(&datas) {
            stale.retarget(p);
            let masks = stale.advance_masks(k);
            let avg = framelocal::average_duty(&masks, &vec![true; k], cells);
            stats.push(recon_from_avg(d, &avg));
        }
        let rmse = mean_metric(&stats, |s| s.rmse_source);
        let impr = if static_rmse > 0.0 {
            100.0 * (static_rmse - rmse) / static_rmse
        } else {
            0.0
        };
        println!(
            "{:<26} {:>4} {:>9.5} {:>8.2}  (stale accumulator, no per-frame reset)",
            "lib-full-stateful", k, rmse, impr
        );
    }

    // Loss sensitivity at a representative K.
    println!();
    println!("-- frame-local loss sensitivity at K={} --", cfg.k.max(2));
    println!(
        "{:<26} {:>14} {:>9} {:>9}",
        "schedule", "loss", "rmse", "|bias|"
    );
    let k = cfg.k.max(2);
    for kind in [
        ScheduleKind::WindowedErrorFeedback,
        ScheduleKind::ResidualWindowedEF,
        ScheduleKind::VdcBalanced,
    ] {
        let mut stats: Vec<ReconStats> = Vec::new();
        for loss in [LossModel::Full, LossModel::Half, LossModel::Random20] {
            let keep = loss.survivors(k, cfg.seed);
            let mut per_frame = Vec::with_capacity(sources);
            for (p, d) in projections.iter().zip(&datas) {
                let plan = pure_plan(p, kind, k, cfg.seed, 0);
                let kept: Vec<Vec<u8>> = plan
                    .iter()
                    .zip(&keep)
                    .filter(|(_, k)| **k)
                    .map(|(m, _)| m.clone())
                    .collect();
                let avg = framelocal::average_duty(&kept, &vec![true; kept.len()], cells);
                per_frame.push(recon_from_avg(d, &avg));
            }
            let s = ReconStats {
                rmse_source: mean_metric(&per_frame, |s| s.rmse_source),
                abs_bias: mean_metric(&per_frame, |s| s.abs_bias),
                ..Default::default()
            };
            println!(
                "{:<26} {:>14} {:>9.5} {:>9.4}",
                kind.as_str(),
                loss.as_str(),
                s.rmse_source,
                s.abs_bias
            );
            stats.push(s);
        }
        let _ = stats;
    }
}

/// Reconstructs the aggregate source RMSE from a precomputed time-averaged duty
/// field, bypassing `recon_stats`' mask summation.
fn recon_from_avg(data: &[CellDatum], avg: &[[f32; 8]]) -> ReconStats {
    let mut acc = ReconStats {
        cells: data.len(),
        ..Default::default()
    };
    let mut edge = (0usize, 0f64);
    let mut detail = (0usize, 0f64);
    let mut flat = (0usize, 0f64);
    for (c, (source, style, duty, class)) in data.iter().enumerate() {
        let m: CellMetric = metrics::cell_metric(source, *style, duty, &avg[c]);
        acc.rmse_source += m.rmse_source;
        acc.rmse_line += m.rmse_line;
        acc.bias += m.bias;
        acc.abs_bias += m.abs_bias;
        match class {
            targets::CellClass::Flat => {
                flat.0 += 1;
                flat.1 += m.rmse_source as f64;
            }
            targets::CellClass::Edge => {
                edge.0 += 1;
                edge.1 += m.rmse_source as f64;
            }
            targets::CellClass::Detail => {
                detail.0 += 1;
                detail.1 += m.rmse_source as f64;
            }
        }
    }
    let n = data.len().max(1);
    acc.rmse_source /= n as f32;
    acc.rmse_line /= n as f32;
    acc.bias /= n as f32;
    acc.abs_bias /= n as f32;
    acc.rmse_source_flat = if flat.0 > 0 {
        (flat.1 / flat.0 as f64) as f32
    } else {
        0.0
    };
    acc.rmse_source_edge = if edge.0 > 0 {
        (edge.1 / edge.0 as f64) as f32
    } else {
        0.0
    };
    acc.rmse_source_detail = if detail.0 > 0 {
        (detail.1 / detail.0 as f64) as f32
    } else {
        0.0
    };
    acc
}

/// Real ANSI wire bytes for one source frame's temporal plan (glyph-changes
/// only; the projection style is constant across phases).
fn ansi_bytes_per_source_frame(
    proj: &BrailleImageProjection,
    kind: ScheduleKind,
    k: usize,
    seed: u64,
    _cells: usize,
) -> f64 {
    let plan = pure_plan(proj, kind, k, seed, 0);
    let w = proj.width();
    let h = proj.height();
    let style_at = |i: usize| {
        proj.cell((i % w as usize) as u16, (i / w as usize) as u16)
            .unwrap()
            .style
    };
    let mut compiler = AnsiCompiler::new();
    let mut prev: Option<gibson::Surface> = None;
    let mut bytes = 0usize;
    for masks in &plan {
        let s = metrics::surface_from_masks(w, h, masks, style_at);
        let diff = gibson::compute_diff(prev.as_ref(), &s);
        bytes += compiler.compile(&diff).len();
        prev = Some(s);
    }
    bytes as f64
}

// ---------------------------------------------------------------------------
// Reachable space (H3)
// ---------------------------------------------------------------------------

fn run_reach(cfg: &Config) {
    println!("== H3. reachable-space / rank demonstration ==");
    // The eight Braille dot rasters over a P=8x16 cell, each dot a 4x4 block.
    let p = 8usize * 16;
    let mut basis = vec![[0f64; 8]; p];
    for (px, row) in basis.iter_mut().enumerate() {
        let x = px % 8;
        let y = px / 8;
        let dx = x / 4; // 0..2
        let dy = y / 4; // 0..4
                        // Map (dy,dx) to the Braille dot index used by the projector.
        const IDX: [[usize; 2]; 4] = [[0, 3], [1, 4], [2, 5], [6, 7]];
        row[IDX[dy][dx]] = 1.0;
    }
    // Rank via Gram-Schmidt.
    let mut gs: Vec<Vec<f64>> = Vec::new();
    for col in 0..8 {
        let mut v: Vec<f64> = (0..p).map(|r| basis[r][col]).collect();
        for q in &gs {
            let d: f64 = v.iter().zip(q).map(|(a, b)| a * b).sum();
            for (vi, qi) in v.iter_mut().zip(q) {
                *vi -= d * qi;
            }
        }
        let n: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if n > 1e-9 {
            for x in v.iter_mut() {
                *x /= n;
            }
            gs.push(v);
        }
    }
    println!(
        "dimension of the 2x4 Braille dot-indicator span = {} (of 8)",
        gs.len()
    );
    println!(
        "temporal masks m_t in {{0,1}}^8; equal-weight K-phase average d = mean_t m_t lies in [0,1]^8"
    );
    println!("(the convex hull of {{0,1}}^8). No phase sequence leaves that hull.\n");

    // Practical spatial limit, now with a rigorous bracket on the continuous
    // duty floor:
    //   segSP   = projector's `line_rmse` (static-optimal partition, unquantized)
    //   segFree = tight ALS best-segment floor (upper bound on the true floor)
    //   pca     = best affine line (rigorous lower bound on the true floor)
    println!(
        "{:<18} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
        "target", "static", "K=8", "segSP", "segFree", "pcaLo", "static/seg"
    );
    let targets = named_targets(cfg.cols, cfg.rows, cfg.seed);
    for (name, img) in targets.iter().take(if cfg.quick { 5 } else { 12 }) {
        let proj = project(img);
        let data = per_cell_data(img, &proj);
        let static_s = recon_stats(&data, &[static_masks(&proj)]);
        let k8 = recon_stats(&data, &library_masks(&proj, 8, true, true, cfg.seed));
        let mut seg_sp = 0f32;
        let mut seg_free = 0f32;
        let mut pca = 0f32;
        let mut n = 0usize;
        for y in 0..proj.height() {
            for x in 0..proj.width() {
                let cell = proj.cell(x, y).unwrap();
                let samples = img.cell_samples(x, y);
                seg_sp += cell.line_rmse;
                let floor = floors::cell_floor(&samples);
                seg_free += floor.upper;
                pca += floor.lower;
                n += 1;
            }
        }
        let n = n.max(1) as f32;
        seg_sp /= n;
        seg_free /= n;
        pca /= n;
        println!(
            "{:<18} {:>9.5} {:>9.5} {:>9.5} {:>9.5} {:>9.5} {:>9.2}",
            name,
            static_s.rmse_source,
            k8.rmse_source,
            seg_sp,
            seg_free,
            pca,
            static_s.rmse_source / seg_free.max(1e-9),
        );
    }
    println!("\nH3 verdict: pure Braille cannot place samples between the 8 fixed dot rasters;");
    println!("time changes each dot's intensity, not its position. Spatial rank stays 8.");
    println!("segFree/pca bracket the continuous-duty floor; static/seg is the most time can buy.");
}

// ---------------------------------------------------------------------------
// E. Energy decomposition: what time can and cannot remove
// ---------------------------------------------------------------------------

/// Sums, over every cell of a target, the source energy and the four floors.
#[derive(Default, Clone, Copy)]
struct Decomp {
    /// Cells counted.
    n: usize,
    /// (sum of squares) emitted static reconstruction error.
    static_sse: f64,
    /// (sum of squares) K=8 windowed-EF reconstruction error.
    k8_sse: f64,
    /// (sum of squares) quantized-colour continuous-duty floor.
    quant_sse: f64,
    /// (sum of squares) free-colour tight segment floor (upper bound).
    segfree_sse: f64,
    /// (sum of squares) free-colour PCA line floor (lower bound).
    pca_sse: f64,
    /// (sum of squares) projector's static-partition continuous floor.
    segsp_sse: f64,
    /// (sum of squares) source samples' own energy (zero-reconstruction error).
    src_sse: f64,
}

impl Decomp {
    fn add(&mut self, other: &Decomp) {
        self.n += other.n;
        self.static_sse += other.static_sse;
        self.k8_sse += other.k8_sse;
        self.quant_sse += other.quant_sse;
        self.segfree_sse += other.segfree_sse;
        self.pca_sse += other.pca_sse;
        self.segsp_sse += other.segsp_sse;
        self.src_sse += other.src_sse;
    }
    fn rms(&self, sse: f64) -> f32 {
        if self.n == 0 {
            0.0
        } else {
            (sse / (self.n as f64 * 24.0)).sqrt() as f32
        }
    }
}

fn decompose_target(img: &targets::LogicalImage, k: usize, seed: u64) -> Decomp {
    let proj = project(img);
    let static_masks_v = static_masks(&proj);
    let k8_masks = pure_plan(&proj, ScheduleKind::WindowedErrorFeedback, k, seed, 0);
    let mut d = Decomp::default();
    for y in 0..proj.height() {
        for x in 0..proj.width() {
            d.n += 1;
            let cell = proj.cell(x, y).unwrap();
            let samples = img.cell_samples(x, y);
            let src = rgb8_to_linear_vec(&samples);
            let sty = cell.style;
            let f = metrics::color_linear(sty.fg.unwrap_or(Color::Reset));
            let b = metrics::color_linear(sty.bg.unwrap_or(Color::Reset));
            // Static emitted reconstruction.
            let idx = y as usize * proj.width() as usize + x as usize;
            for i in 0..8 {
                let on = static_masks_v[idx] & (1 << i) != 0;
                for c in 0..3 {
                    let r = if on { f[c] } else { b[c] };
                    d.static_sse += (r - src[i][c]).powi(2) as f64;
                }
            }
            // K=8 schedule reconstruction.
            let mut avg = [0f32; 8];
            for m in &k8_masks {
                let mv = m[idx];
                for i in 0..8 {
                    if mv & (1 << i) != 0 {
                        avg[i] += 1.0;
                    }
                }
            }
            for a in avg.iter_mut() {
                *a /= k as f32;
            }
            for i in 0..8 {
                for c in 0..3 {
                    let r = b[c] + avg[i] * (f[c] - b[c]);
                    d.k8_sse += (r - src[i][c]).powi(2) as f64;
                }
            }
            // Floors. If the emitted colours are not a complete pair, claim no
            // headroom for this cell (fall back to the static error).
            match floors::quantized_segment_floor(&samples, sty) {
                Some(q) => d.quant_sse += (q as f64).powi(2) * 24.0,
                None => {
                    for i in 0..8 {
                        let on = static_masks_v[idx] & (1 << i) != 0;
                        for c in 0..3 {
                            let r = if on { f[c] } else { b[c] };
                            d.quant_sse += (r - src[i][c]).powi(2) as f64;
                        }
                    }
                }
            }
            d.segsp_sse += (cell.line_rmse as f64).powi(2) * 24.0;
            let floor = floors::cell_floor(&samples);
            d.segfree_sse += (floor.upper as f64).powi(2) * 24.0;
            d.pca_sse += (floor.lower as f64).powi(2) * 24.0;
            for i in 0..8 {
                for c in 0..3 {
                    d.src_sse += (src[i][c] as f64).powi(2);
                }
            }
        }
    }
    d
}

fn rgb8_to_linear_vec(samples: &[[u8; 3]; 8]) -> [[f32; 3]; 8] {
    samples.map(metrics::rgb8_to_linear)
}

fn print_decomp_row(name: &str, d: &Decomp) {
    let denom = d.static_sse.max(1e-12);
    println!(
        "{:<18} {:>8.5} {:>8.5} {:>8.5} {:>8.5} {:>8.5} {:>7.1} {:>7.1} {:>7.1}",
        name,
        d.rms(d.static_sse),
        d.rms(d.k8_sse),
        d.rms(d.quant_sse),
        d.rms(d.segfree_sse),
        d.rms(d.pca_sse),
        100.0 * (d.static_sse - d.quant_sse) / denom,
        100.0 * (d.quant_sse - d.segfree_sse) / denom,
        100.0 * d.segfree_sse / denom,
    );
}

fn run_decompose(cfg: &Config) {
    let k = cfg.k.max(2);
    println!("== E. what time can remove vs what is permanently out of reach (K={k}) ==");
    println!(
        "{:<18} {:>8} {:>8} {:>8} {:>8} {:>8} {:>7} {:>7} {:>7}",
        "target",
        "static",
        format!("K={k}"),
        "quantSeg",
        "segFree",
        "pcaLo",
        "tonal%",
        "colr%",
        "floor%"
    );
    let mut gray_total = Decomp::default();
    let mut chroma_total = Decomp::default();
    let mut total = Decomp::default();
    for (name, img) in named_targets(cfg.cols, cfg.rows, cfg.seed) {
        let d = decompose_target(&img, k, cfg.seed);
        print_decomp_row(&name, &d);
        gray_total.add(&d);
        total.add(&d);
    }
    println!();
    println!("-- chromatic targets: per-cell colours non-collinear, so no two-colour");
    println!("   line fits them and a genuine spatial/colour floor appears --");
    for (name, img) in targets::chromatic_targets(cfg.cols, cfg.rows, cfg.seed) {
        let d = decompose_target(&img, k, cfg.seed);
        print_decomp_row(&name, &d);
        chroma_total.add(&d);
        total.add(&d);
    }
    println!();
    let mean_row = |label: &str, d: &Decomp| {
        let denom = d.static_sse.max(1e-12);
        println!(
            "{:<18} {:>8.5} {:>8.5} {:>8.5} {:>8.5} {:>8.5} {:>7.1} {:>7.1} {:>7.1}",
            label,
            d.rms(d.static_sse),
            d.rms(d.k8_sse),
            d.rms(d.quant_sse),
            d.rms(d.segfree_sse),
            d.rms(d.pca_sse),
            100.0 * (d.static_sse - d.quant_sse) / denom,
            100.0 * (d.quant_sse - d.segfree_sse) / denom,
            100.0 * d.segfree_sse / denom,
        );
    };
    mean_row("MEAN gray", &gray_total);
    mean_row("MEAN chroma", &chroma_total);
    mean_row("MEAN all", &total);
    println!();
    println!("All columns are energy fractions of the emitted static error squared:");
    println!(
        "  tonal% = (static^2 - quantSeg^2)/static^2 ... removed by time with the emitted colours;"
    );
    println!(
        "  colr%  = (quantSeg^2 - segFree^2)/static^2 ... colour quantization + two-colour model;"
    );
    println!("  floor% = segFree^2/static^2                ... out of reach of any duty sequence.");
    println!(
        "Unreachable fraction of pure source energy: gray {:.2}%, chroma {:.2}%.",
        100.0 * gray_total.segfree_sse / gray_total.src_sse.max(1e-12),
        100.0 * chroma_total.segfree_sse / chroma_total.src_sse.max(1e-12),
    );
    println!();
    println!("Reading: for GRAYSCALE targets the eight per-cell samples are collinear, so");
    println!("free per-dot duty reaches them exactly (floor 0): the entire static error is");
    println!("tonal (H1) plus finite-K / 8-bit colour quantization. For CHROMATIC targets a");
    println!("two-colour cell has a genuine floor (H2): no duty sequence removes it. Time is");
    println!("a per-dot tonal instrument, not a way to add spatial or chromatic samples.");
}

// ---------------------------------------------------------------------------
// F. Temporal-aware colour choice: headroom left by static-optimal colours
// ---------------------------------------------------------------------------

const MU_CHOICES: [f64; 9] = [1.0, 1.1, 1.25, 1.5, 2.0, 3.0, 5.0, 10.0, f64::INFINITY];

#[derive(Clone, Copy)]
struct FrontierRow {
    n: usize,
    static_sse: f64,
    k8_sse: f64,
    /// Temporal floor for each allowed static-inflation factor `mu`.
    floor_sse: [f64; MU_CHOICES.len()],
    /// Static error of the pair actually chosen at each `mu`.
    chosen_static_sse: [f64; MU_CHOICES.len()],
}

impl Default for FrontierRow {
    fn default() -> Self {
        FrontierRow {
            n: 0,
            static_sse: 0.0,
            k8_sse: 0.0,
            floor_sse: [0.0; MU_CHOICES.len()],
            chosen_static_sse: [0.0; MU_CHOICES.len()],
        }
    }
}

impl FrontierRow {
    fn rms(&self, sse: f64) -> f32 {
        if self.n == 0 {
            0.0
        } else {
            (sse / (self.n as f64 * 24.0)).sqrt() as f32
        }
    }
    fn add(&mut self, o: &FrontierRow) {
        self.n += o.n;
        self.static_sse += o.static_sse;
        self.k8_sse += o.k8_sse;
        for m in 0..MU_CHOICES.len() {
            self.floor_sse[m] += o.floor_sse[m];
            self.chosen_static_sse[m] += o.chosen_static_sse[m];
        }
    }
}

/// Best binary static fit (over the 256 masks) for a fixed colour pair.
fn static_sse_fixed(src: &[[f32; 3]; 8], bg: [f32; 3], fg: [f32; 3]) -> f64 {
    // Per-sample squared distance to bg and to fg, then the cheapest mask.
    let mut dbg = [0.0f64; 8];
    let mut dfg = [0.0f64; 8];
    for (i, p) in src.iter().enumerate() {
        for c in 0..3 {
            dbg[i] += (p[c] - bg[c]).powi(2) as f64;
            dfg[i] += (p[c] - fg[c]).powi(2) as f64;
        }
    }
    let mut best = f64::INFINITY;
    for mask in 0u16..256 {
        let mut sse = 0.0f64;
        for i in 0..8 {
            sse += if mask & (1 << i) != 0 { dfg[i] } else { dbg[i] };
        }
        best = best.min(sse);
    }
    best
}

fn frontier_target(
    img: &targets::LogicalImage,
    palette: &[[u8; 3]],
    k: usize,
    seed: u64,
) -> FrontierRow {
    let proj = project(img);
    let static_masks_v = static_masks(&proj);
    let k8_masks = pure_plan(&proj, ScheduleKind::WindowedErrorFeedback, k, seed, 0);
    let mut row = FrontierRow::default();
    for y in 0..proj.height() {
        for x in 0..proj.width() {
            row.n += 1;
            let cell = proj.cell(x, y).unwrap();
            let samples = img.cell_samples(x, y);
            let src = rgb8_to_linear_vec(&samples);
            let f = metrics::color_linear(cell.style.fg.unwrap_or(Color::Reset));
            let b = metrics::color_linear(cell.style.bg.unwrap_or(Color::Reset));
            let idx = y as usize * proj.width() as usize + x as usize;
            // Projector static error for this cell.
            let mut static0 = 0.0f64;
            for i in 0..8 {
                let on = static_masks_v[idx] & (1 << i) != 0;
                for c in 0..3 {
                    let r = if on { f[c] } else { b[c] };
                    static0 += (r - src[i][c]).powi(2) as f64;
                }
            }
            row.static_sse += static0;
            // K-phase schedule error.
            let mut avg = [0f32; 8];
            for m in &k8_masks {
                let mv = m[idx];
                for i in 0..8 {
                    if mv & (1 << i) != 0 {
                        avg[i] += 1.0;
                    }
                }
            }
            for a in avg.iter_mut() {
                *a /= k as f32;
            }
            for i in 0..8 {
                for c in 0..3 {
                    let r = b[c] + avg[i] * (f[c] - b[c]);
                    row.k8_sse += (r - src[i][c]).powi(2) as f64;
                }
            }
            // Candidate colour pairs: the palette plus the projector's own pair.
            let mut cand: Vec<(f64, f64)> = Vec::with_capacity(palette.len() * palette.len() + 1);
            for (ci, c0) in palette.iter().enumerate() {
                for (cj, c1) in palette.iter().enumerate() {
                    if ci == cj {
                        continue;
                    }
                    let pbg = metrics::rgb8_to_linear(*c0);
                    let pfg = metrics::rgb8_to_linear(*c1);
                    let s = static_sse_fixed(&src, pbg, pfg);
                    let t = floors::segment_floor_linear(&samples, pbg, pfg);
                    cand.push((s, (t as f64).powi(2) * 24.0));
                }
            }
            let q = floors::quantized_segment_floor(&samples, cell.style).unwrap_or(0.0);
            cand.push((static0, (q as f64).powi(2) * 24.0));
            // Constrained minima, one per allowed static inflation.
            for (m, mu) in MU_CHOICES.iter().enumerate() {
                let limit = mu * static0 + 1e-12;
                let mut best_t = f64::INFINITY;
                let mut chosen_s = static0;
                for &(s, t) in &cand {
                    if s <= limit && t < best_t {
                        best_t = t;
                        chosen_s = s;
                    }
                }
                if !best_t.is_finite() {
                    best_t = (q as f64).powi(2) * 24.0;
                }
                row.floor_sse[m] += best_t;
                row.chosen_static_sse[m] += chosen_s;
            }
        }
    }
    row
}

fn run_color(cfg: &Config) {
    let palette = floors::frontier_palette();
    let k = cfg.k.max(2);
    println!("== F. static/temporal colour-pair tradeoff frontier (K={k}) ==");
    println!("A cell's bg/fg pair fixes the line per-dot duty moves along. The projector");
    println!("picks that pair to minimise the STATIC error, which need not minimise the");
    println!("continuous-duty temporal floor. flr@1 keeps the static fallback exactly as");
    println!("good (mu=1); flr@inf optimises purely for time. st@ is the static error the");
    println!("chosen pair actually commits to.\n");
    println!(
        "{:<18} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>7}",
        "target",
        "static",
        format!("K={k}"),
        "flr@1",
        "st@1",
        "flr@inf",
        "st@inf",
        "recov%"
    );
    let mut gray = FrontierRow::default();
    let mut chroma = FrontierRow::default();
    let mut all = FrontierRow::default();
    let print_row = |name: &str, r: &FrontierRow| {
        let last = MU_CHOICES.len() - 1;
        let recov = 100.0 * (r.floor_sse[0] - r.floor_sse[last]) / r.static_sse.max(1e-12);
        println!(
            "{:<18} {:>8.5} {:>8.5} {:>8.5} {:>8.5} {:>8.5} {:>8.5} {:>7.1}",
            name,
            r.rms(r.static_sse),
            r.rms(r.k8_sse),
            r.rms(r.floor_sse[0]),
            r.rms(r.chosen_static_sse[0]),
            r.rms(r.floor_sse[last]),
            r.rms(r.chosen_static_sse[last]),
            recov,
        );
    };
    for (name, img) in named_targets(cfg.cols, cfg.rows, cfg.seed) {
        let r = frontier_target(&img, &palette, k, cfg.seed);
        print_row(&name, &r);
        gray.add(&r);
        all.add(&r);
    }
    for (name, img) in targets::chromatic_targets(cfg.cols, cfg.rows, cfg.seed) {
        let r = frontier_target(&img, &palette, k, cfg.seed);
        print_row(&name, &r);
        chroma.add(&r);
        all.add(&r);
    }
    println!();
    print_row("MEAN gray", &gray);
    print_row("MEAN chroma", &chroma);
    print_row("MEAN all", &all);
    println!();
    println!("Frontier, mean over all targets: as the allowed static inflation mu grows, how");
    println!("far the continuous-duty temporal floor falls, and what static error it costs.\n");
    println!(
        "{:<8} {:>10} {:>11} {:>12} {:>10}",
        "mu", "floorRMS", "staticRMS", "floor/emit%", "recov%"
    );
    for (m, mu) in MU_CHOICES.iter().enumerate() {
        let rel = 100.0 * all.floor_sse[m] / all.floor_sse[0].max(1e-12);
        let recov = 100.0 * (all.floor_sse[0] - all.floor_sse[m]) / all.static_sse.max(1e-12);
        let label = if mu.is_infinite() {
            "inf".to_string()
        } else {
            format!("{mu:.2}")
        };
        println!(
            "{:<8} {:>10.5} {:>11.5} {:>12.1} {:>10.1}",
            label,
            all.rms(all.floor_sse[m]),
            all.rms(all.chosen_static_sse[m]),
            rel,
            recov,
        );
    }
    println!();
    println!("recov% = (emitFloor^2 - floor^2)/static^2: the temporal error energy recovered");
    println!("that the static-optimal colour choice leaves on the table. The frontier shows the");
    println!("price in static fallback error. A temporal-first renderer should sit at the knee,");
    println!("not at either extreme.");
}

// ---------------------------------------------------------------------------
// Live A/B demo
// ---------------------------------------------------------------------------

fn color_linear_opt(c: Color) -> Option<[f32; 3]> {
    match c {
        Color::Reset => None,
        other => {
            let (r, g, b) = other.to_rgb();
            Some([
                metrics::srgb8_to_linear(r),
                metrics::srgb8_to_linear(g),
                metrics::srgb8_to_linear(b),
            ])
        }
    }
}

fn lum(lin: [f32; 3]) -> f32 {
    0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2]
}

/// Mirrors `temporal`'s emitted-swing safety evaluation: quantize at the wire
/// depth, then measure the linear-light luminance gap. `None` = freeze.
fn resolved_swing(style: Style, depth: ColorDepth) -> Option<f32> {
    if depth == ColorDepth::Mono {
        return None;
    }
    let fg = gibson::capability::quantize_color(style.fg?, depth)?;
    let bg = gibson::capability::quantize_color(style.bg?, depth)?;
    Some((lum(color_linear_opt(fg)?) - lum(color_linear_opt(bg)?)).abs())
}

/// Short, stable label for a gate state, for the live header.
fn gate_tag(g: gibson::TemporalGate) -> &'static str {
    match g {
        gibson::TemporalGate::Enabled => "gate:on",
        gibson::TemporalGate::ReducedMotion => "gate:reduced",
        gibson::TemporalGate::Unmeasured => "gate:unmeasured",
        gibson::TemporalGate::CadenceTooLow => "gate:slow",
        gibson::TemporalGate::SurvivalTooLow => "gate:lossy",
        gibson::TemporalGate::JitterTooHigh => "gate:jitter",
        gibson::TemporalGate::DepthTooHigh => "gate:deep",
        _ => "gate:?",
    }
}

/// The Braille dot mask a cell encodes, or 0 for a space/non-Braille glyph.
fn glyph_mask(c: &gibson::Cell) -> u8 {
    c.glyph
        .grapheme
        .chars()
        .next()
        .and_then(|ch| {
            let u = ch as u32;
            if (0x2800..=0x28FF).contains(&u) {
                Some((u - 0x2800) as u8)
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn cells_differ(a: &gibson::Cell, b: &gibson::Cell) -> bool {
    a.glyph.grapheme != b.glyph.grapheme || a.style != b.style
}

/// Number of cells whose glyph or style differs — the live "is it moving?" probe.
fn changed_cells(a: &gibson::Surface, b: &gibson::Surface) -> usize {
    a.cells
        .iter()
        .zip(&b.cells)
        .filter(|(x, y)| cells_differ(x, y))
        .count()
}

/// Renders the XOR of two Braille frames, hot-on-dark. The actual modulation is
/// designed to vanish after eye integration; this makes the pulse structure
/// visible on an ordinary display so a human can confirm it is really changing.
fn amplified_diff(a: &gibson::Surface, b: &gibson::Surface, hot: Style) -> gibson::Surface {
    let mut out = gibson::Surface::new(a.width, a.height);
    for y in 0..a.height {
        for x in 0..a.width {
            let (Some(ca), Some(cb)) = (a.get(x, y), b.get(x, y)) else {
                continue;
            };
            let d = glyph_mask(ca) ^ glyph_mask(cb);
            let (glyph, style) = if d == 0 {
                (gibson::Glyph::space(), Style::default())
            } else {
                (
                    gibson::Glyph::from_char(char::from_u32(0x2800 + d as u32).unwrap_or(' ')),
                    hot,
                )
            };
            out.set_cell(x, y, gibson::Cell::new(glyph, style));
        }
    }
    out
}

fn run_live(cfg: &Config) -> io::Result<()> {
    use gibson::{
        BorderType, Context, FramePacing, Node, PresentationProfile, TemporalDisplayProcessor,
    };
    use std::sync::Arc;
    use std::time::Duration;

    // Temporal is ON by default: without it the "live" demo is static by
    // construction and looks broken. An honest caller passes `--measured-hz`;
    // otherwise we ASSUME a profile and say so, loudly, everywhere.
    let temporal = !has("--no-temporal");
    let measured_hz = arg_str("measured-hz").and_then(|v| v.parse::<f32>().ok());
    let assume_hz = arg_str("assume-hz")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(120.0);
    let survival = arg_str("survival")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.95);
    let jitter = arg_str("jitter-p95")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.5);
    let depth_cap = arg_str("depth-cap").and_then(|v| v.parse::<f32>().ok());
    let reduced = has("--reduced-motion");
    let force = has("--force");
    let slowmo = arg_str("slowmo").and_then(|v| v.parse::<f32>().ok());
    let sync = !has("--no-sync");

    let (cols, rows) = (cfg.cols, cfg.rows);
    let mut processor =
        TemporalDisplayProcessor::new(cols, rows, SubcellGlyphMode::Braille2x4, cfg.seed);
    if let Some(cap) = depth_cap {
        let mut policy = gibson::TemporalSafetyPolicy::default();
        policy.max_luminance_depth = cap;
        processor.set_policy(policy);
    }
    processor.set_reduced_motion(reduced);

    let mut profile_label = "UNMEASURED";
    if temporal {
        if let Some(hz) = measured_hz {
            processor.set_profile(PresentationProfile::measured(hz, survival, jitter));
            profile_label = "MEASURED";
        } else {
            processor.set_profile(PresentationProfile::measured(assume_hz, survival, jitter));
            profile_label = "ASSUMED!";
            eprintln!(
                "WARNING: assuming a {assume_hz:.0} Hz / {survival:.2} survival / {jitter:.2} ms-jitter"
            );
            eprintln!(
                "         presentation profile. This is NOT a measurement. Pass --measured-hz=<hz>"
            );
            eprintln!("         for an honest profile, or --no-temporal to render static only.");
        }
    } else {
        eprintln!("--no-temporal: rendering the static fallback only.");
    }

    // A designed "world": a vaporwave-ish gradient with a radial sun, a horizon
    // rule and a few foreground structures — one dominant image, minimal chrome.
    let world = move |lx: u16, ly: u16| -> [u8; 3] {
        let w = 2.0 * cols as f32;
        let h = 4.0 * rows as f32;
        let x = lx as f32 / w;
        let y = ly as f32 / h;
        let horizon = 0.58 + 0.05 * (x * 6.0).sin();
        if y < horizon {
            // sun disc
            let dx = lx as f32 - w * 0.5;
            let dy = ly as f32 - h * 0.42;
            let r = ((dx * dx + dy * dy) / (w * 0.22).powi(2)).sqrt();
            if r < 1.0 {
                let t = (1.0 - r).clamp(0.0, 1.0);
                let v = 180.0 + t * 75.0;
                let g = 90.0 + t * 120.0;
                let b = 40.0 + t * 90.0;
                return [v as u8, g as u8, b as u8];
            }
            // sky gradient
            let t = y / horizon;
            [
                (26.0 + t * 60.0) as u8,
                (22.0 + t * 40.0) as u8,
                (60.0 + t * 90.0) as u8,
            ]
        } else {
            // ground with a receding neon grid
            let gy = (y - horizon) / (1.0 - horizon);
            let gx = (x - 0.5).abs();
            let grid = (gy * 18.0).fract().min(1.0 - (gy * 18.0).fract());
            let vline = ((gx * 22.0).fract()).min(1.0 - (gx * 22.0).fract());
            let line = grid.min(vline).min(0.12);
            if line < 0.05 {
                [90, 30, 110]
            } else {
                [
                    (14.0 + (1.0 - gy) * 30.0) as u8,
                    (10.0 + (1.0 - gy) * 12.0) as u8,
                    (34.0 + (1.0 - gy) * 50.0) as u8,
                ]
            }
        }
    };
    processor.set_target_image(world, ResetPolicy::Reset);

    let mut ctx = Context::fullscreen()?;
    if !ctx.is_interactive() {
        return Err(io::Error::other("--mode=live requires an interactive TTY"));
    }
    ctx.set_max_fps(120);
    ctx.set_frame_pacing(FramePacing::PhaseLocked);
    ctx.set_sync_updates(sync);
    processor.set_color_depth(ctx.capabilities().color_depth);

    let enabled = matches!(processor.gate(), gibson::TemporalGate::Enabled);
    let k = ((measured_hz.unwrap_or(assume_hz) / 30.0).round() as usize).clamp(2, 8);
    let sched = FrameLocalScheduler::new(k, ScheduleKind::WindowedErrorFeedback, cfg.seed);
    let proj = project(&LogicalImage::new(
        cols,
        rows,
        targets::TargetKind::Portrait,
        world,
    ));
    let depth = ctx.capabilities().color_depth;
    let cap = depth_cap.unwrap_or(0.10);
    let stat = processor.static_fallback();
    let hot = Style::default()
        .fg(Color::BrightWhite)
        .bg(Color::Black)
        .bold();

    eprintln!(
        "temporal braille persistence lab (live): {cols}x{rows}; profile={profile_label}; gate={:?} ({}); temporal={enabled}; K={k}.",
        processor.gate(),
        gate_tag(processor.gate())
    );
    if !enabled {
        eprintln!("gate is NOT Enabled -> the library arm falls back to static.");
        if force {
            eprintln!(
                "--force: the visualization arms (GRAIN/DIFF) modulate anyway. UNSAFE, demo only."
            );
        } else {
            eprintln!("pass --force to visualize the modulation anyway, or fix the profile gate.");
        }
    }
    if let Some(hz) = slowmo {
        eprintln!(
            "--slowmo={hz}: PDM phase advanced at {hz} Hz so the eye can resolve it (visualization only)."
        );
    }
    eprintln!(
        "A/B/grain: STATIC / TEMPORAL-LIB / TEMPORAL-GRAIN / DIFF(x8). The header ticks every frame."
    );

    let modes = ["STATIC", "TEMPORAL-LIB", "TEMPORAL-GRAIN", "DIFF(x8)"];
    let per_mode = arg_u32("dwell", 3).max(1) as u64 * 120;
    let total = arg_u32("seconds", 24) as u64 * 120;
    let mut frame: u64 = 0;
    let mut prev: Option<Arc<gibson::Surface>> = None;
    while frame < total {
        let mode = ((frame / per_mode) % modes.len() as u64) as usize;
        let missed = ctx.missed_periods_last_frame();
        // Phase clock for the explicit schedule. At the shipping cadence this is
        // `frame` exactly; `--slowmo=<hz>` stretches each of the K phases so a
        // human can resolve the pulse pattern instead of integrating it away.
        let phase = match slowmo {
            Some(hz) if hz > 0.0 => (frame as f64 * hz as f64 / 120.0).round() as u64,
            _ => frame,
        };
        let temporal_surface =
            |phase: u64| grain_surface(&proj, &sched, &stat, depth, cap, force, enabled, phase);
        let surface = match mode {
            0 => stat.clone(),
            1 => {
                if enabled {
                    processor.advance(missed)
                } else {
                    stat.clone()
                }
            }
            2 => temporal_surface(phase),
            _ => {
                let t = if enabled || force {
                    temporal_surface(phase)
                } else {
                    stat.clone()
                };
                amplified_diff(&t, &stat, hot)
            }
        };
        let vs_prev = prev
            .as_ref()
            .map_or(0, |p| changed_cells(p.as_ref(), &surface));
        let vs_static = changed_cells(&stat, &surface);
        let gate = processor.gate();

        let header = format!(
            "[{:<14}] {:>5.1}s f={:<6} {} {} K={} d={:<4} vs={:<4}",
            modes[mode],
            frame as f64 / 120.0,
            frame,
            gate_tag(gate),
            profile_label,
            k,
            vs_prev,
            vs_static,
        );
        let arc = Arc::new(surface);
        prev = Some(arc.clone());
        let root = Node::col()
            .child(Node::text(header, Style::default()))
            .child(
                Node::panel("", BorderType::Rounded, Style::default()).child(Node::surface(arc)),
            );
        ctx.set_root(root);
        while !ctx.render_if_due()? {
            let wait = ctx.time_until_next_frame();
            if wait.is_zero() {
                std::thread::yield_now();
            } else {
                std::thread::sleep(wait.min(Duration::from_millis(2)));
            }
        }
        frame += 1;
    }
    ctx.restore()?;
    Ok(())
}

/// Builds the experimental grain frame: the frame-local schedule, but each cell
/// whose emitted swing exceeds the cap (or is unknown) is held to its exact
/// static mask — the same freeze the safety controller would apply.
///
/// `force` bypasses the *profile* gate for the visualization arms only (the
/// per-cell safety mirror still applies); `enabled` is the real library gate.
/// `phase` is the scheduler clock, which `--slowmo` stretches.
#[allow(clippy::too_many_arguments)]
fn grain_surface(
    proj: &BrailleImageProjection,
    sched: &FrameLocalScheduler,
    stat: &gibson::Surface,
    depth: ColorDepth,
    cap: f32,
    force: bool,
    enabled: bool,
    phase: u64,
) -> gibson::Surface {
    if !enabled && !force {
        return stat.clone();
    }
    let k = sched.k.max(1);
    let window = phase / k as u64;
    let sub = (phase % k as u64) as usize;
    let plan = sched.plan(proj, window);
    let w = proj.width() as usize;
    let mut masks = plan.masks[sub.min(plan.masks.len() - 1)].clone();
    for i in 0..w * proj.height() as usize {
        let x = (i % w) as u16;
        let y = (i / w) as u16;
        let style = proj.cell(x, y).unwrap().style;
        let ok = matches!(resolved_swing(style, depth), Some(s) if s <= cap);
        if !ok {
            masks[i] = proj.cell(x, y).unwrap().static_mask;
        }
    }
    let style_at = |i: usize| proj.cell((i % w) as u16, (i / w) as u16).unwrap().style;
    metrics::surface_from_masks(proj.width(), proj.height(), &masks, style_at)
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() -> io::Result<()> {
    let cfg = Config::parse();
    if has("--help") || cfg.mode == "help" {
        println!(
            "modes: matrix montecarlo spectrum pareto loss framelocal reach decompose color live"
        );
        println!(
            "live flags: --measured-hz= --assume-hz= --no-temporal --force --slowmo=<hz> --dwell=<s> --seconds=<s>"
        );
        return Ok(());
    }
    match cfg.mode.as_str() {
        "matrix" => run_matrix(&cfg),
        "montecarlo" => run_montecarlo(&cfg),
        "spectrum" => run_spectrum(&cfg),
        "pareto" => run_pareto(&cfg),
        "loss" => run_loss(&cfg),
        "framelocal" => run_framelocal(&cfg),
        "reach" => run_reach(&cfg),
        "decompose" => run_decompose(&cfg),
        "color" => run_color(&cfg),
        "live" => run_live(&cfg)?,
        other => {
            eprintln!("unknown --mode={other}; try --help");
            std::process::exit(2);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_schedule_mean_converges_to_duty_for_stationary_target() {
        // Over a long chained window the frame-local error feedback mean must
        // track the projector duty within a tight tolerance.
        let img = targets::smooth_gradient(16, 8, 1);
        let proj = project(&img);
        let data = per_cell_data(&img, &proj);
        let k = 4;
        let mut frames = Vec::new();
        for w in 0..64 {
            frames.extend(pure_plan(
                &proj,
                ScheduleKind::WindowedErrorFeedback,
                k,
                3,
                w,
            ));
        }
        let avg = framelocal::average_duty(&frames, &vec![true; frames.len()], data.len());
        let mut worst = 0f32;
        let mut signed_sum = 0f64;
        let mut signed_n = 0f64;
        for (c, (_, _, duty, _)) in data.iter().enumerate() {
            for i in 0..8 {
                let e = avg[c][i] - duty[i];
                worst = worst.max(e.abs());
                signed_sum += e as f64;
                signed_n += 1.0;
            }
        }
        // Frame-local error feedback resets its phase every window, so its
        // per-window error is bounded by one level (1/K) but not zero. The
        // meaningful claims are: no systematic DC bias over many windows, and a
        // worst-case drift well under one level.
        let bias = (signed_sum / signed_n) as f32;
        assert!(bias.abs() < 0.01, "systematic duty bias {bias}");
        assert!(worst < 0.6 / k as f32, "worst duty drift {worst}");
    }

    #[test]
    fn static_matrix_row_is_exact_static_rmse() {
        let img = targets::portrait(12, 6, 2);
        let proj = project(&img);
        let data = per_cell_data(&img, &proj);
        let s = recon_stats(&data, &[static_masks(&proj)]);
        let expected = proj.mean_emitted_static_rmse();
        assert!(
            (s.rmse_source - expected).abs() < 1e-4,
            "{} vs {}",
            s.rmse_source,
            expected
        );
    }

    #[test]
    fn frame_local_beats_static_on_smooth_gradient() {
        let img = targets::smooth_gradient(24, 10, 3);
        let proj = project(&img);
        let data = per_cell_data(&img, &proj);
        let stat = recon_stats(&data, &[static_masks(&proj)]);
        let k8 = recon_stats(
            &data,
            &pure_plan(&proj, ScheduleKind::WindowedErrorFeedback, 8, 3, 0),
        );
        assert!(
            k8.rmse_source < stat.rmse_source,
            "temporal {} should beat static {}",
            k8.rmse_source,
            stat.rmse_source
        );
    }

    #[test]
    fn mean_and_ci_matches_known_values() {
        let xs = [1.0f32, 2.0, 3.0, 4.0, 5.0];
        let (m, s, ci, lo, hi) = mean_and_ci(&xs);
        assert!((m - 3.0).abs() < 1e-6);
        assert!((s - 1.581_138_8).abs() < 1e-5, "std {s}");
        assert!((ci - 1.96 * s / 5f32.sqrt()).abs() < 1e-6);
        assert_eq!(lo, 1.0);
        assert_eq!(hi, 5.0);
    }

    #[test]
    fn colour_frontier_floor_is_monotone_in_mu() {
        let img = targets::smooth_gradient(4, 3, 1);
        let palette = floors::frontier_palette();
        let r = frontier_target(&img, &palette, 4, 1);
        // A looser static constraint cannot raise the temporal floor.
        for m in 1..MU_CHOICES.len() {
            assert!(
                r.floor_sse[m] <= r.floor_sse[m - 1] + 1e-9,
                "floor rose at mu={}",
                MU_CHOICES[m]
            );
        }
        // The projector's own pair is always a candidate, so mu=1 never commits
        // to a pair with more static error than the projector already has.
        assert!(r.chosen_static_sse[0] <= r.static_sse + 1e-6);
    }

    #[test]
    fn decomposition_floors_are_ordered() {
        let img = targets::smooth_gradient(16, 8, 5);
        let d = decompose_target(&img, 8, 5);
        // Energy ordering: pca <= segFree <= quantSeg <= static. Each step is a
        // relaxation of the previous, so the sums of squares must be ordered.
        assert!(d.pca_sse <= d.segfree_sse + 1e-2, "pca>segfree");
        assert!(d.segfree_sse <= d.quant_sse + 1e-2, "segfree>quant");
        assert!(d.quant_sse <= d.static_sse + 1e-2, "quant>static");
    }

    #[test]
    fn reach_line_floor_is_invariant_to_k() {
        // The infinite-K line floor from the projector must not be beaten by any
        // finite temporal schedule beyond quantization noise; compare K=8.
        let img = targets::slanted_edge(16, 8, 45.0);
        let proj = project(&img);
        let data = per_cell_data(&img, &proj);
        let mut line = 0f32;
        for y in 0..proj.height() {
            for x in 0..proj.width() {
                line += proj.cell(x, y).unwrap().line_rmse;
            }
        }
        line /= data.len() as f32;
        let k8 = recon_stats(
            &data,
            &pure_plan(&proj, ScheduleKind::WindowedErrorFeedback, 8, 3, 0),
        );
        // K=8 is at most the emitted quantization over the line floor.
        assert!(
            k8.rmse_source >= line * 0.9,
            "k8={} line={line}",
            k8.rmse_source
        );
    }

    #[test]
    fn live_arms_actually_animate_and_diff_is_nonempty() {
        // Headless proxy for the live demo: the demo "looked static" because it
        // was run without a profile, so every arm fell back to static. Prove the
        // pieces the demo relies on do move once the gate is satisfied, and that
        // the XOR view exposes structure the integrated image hides.
        use gibson::{SubcellGlyphMode, TemporalDisplayProcessor};
        let (cols, rows) = (16u16, 8u16);
        let world = |lx: u16, ly: u16| -> [u8; 3] {
            let v = ((lx as f32 / (2.0 * cols as f32)) * 255.0) as u8;
            let w = ((ly as f32 / (4.0 * rows as f32)) * 255.0) as u8;
            [v, w, 128]
        };
        let mut processor =
            TemporalDisplayProcessor::new(cols, rows, SubcellGlyphMode::Braille2x4, 3);
        processor.set_target_image(world, ResetPolicy::Reset);
        let stat = processor.static_fallback();
        let proj = project(&LogicalImage::new(
            cols,
            rows,
            targets::TargetKind::Portrait,
            world,
        ));
        let k = 4usize;
        let sched = FrameLocalScheduler::new(k, ScheduleKind::WindowedErrorFeedback, 3);

        // Gate off, no force: the arm is exactly static — the failure mode the
        // live demo hit when no measured profile was supplied.
        let off = grain_surface(
            &proj,
            &sched,
            &stat,
            ColorDepth::Ansi256,
            0.10,
            false,
            false,
            0,
        );
        assert_eq!(changed_cells(&off, &stat), 0, "gated arm must be static");

        // Forced, it must change between consecutive phases.
        let frames: Vec<_> = (0..(k as u64 * 4))
            .map(|p| {
                grain_surface(
                    &proj,
                    &sched,
                    &stat,
                    ColorDepth::Ansi256,
                    1.0,
                    true,
                    false,
                    p,
                )
            })
            .collect();
        let moving = frames
            .windows(2)
            .filter(|w| changed_cells(&w[0], &w[1]) > 0)
            .count();
        assert!(moving > 0, "forced temporal arm never changes");

        let hot = Style::default()
            .fg(Color::BrightWhite)
            .bg(Color::Black)
            .bold();
        let diff = amplified_diff(&frames[0], &stat, hot);
        let lit = diff.cells.iter().filter(|c| glyph_mask(c) != 0).count();
        assert!(lit > 0, "amplified diff is empty");
    }
}
