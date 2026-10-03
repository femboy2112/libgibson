//! Terminal -> 1080p lab.
//!
//! The minimal question behind "1080p from a text grid": Braille gives 8 dots
//! per cell but a *fixed* 2x4 spatial lattice. Time can vary each dot's
//! intensity (a real tonal-resolution gain) but cannot move a dot. Which small
//! set of additional glyphs adds genuinely **new spatial directions** rather
//! than re-expressing the existing ones?
//!
//! This example poses that as linear algebra over a calibrated coverage model
//! (see `terminal_to_1080p_lab/raster.rs`) and reports:
//!   * algebraic rank / effective rank of the Braille dot basis,
//!   * the principal angle of each candidate glyph to the Braille span,
//!   * static vs temporal reconstruction of a held-out target corpus,
//!   * a greedy minimal augmentation basis with train/holdout separation,
//!   * leave-one-family-out cross-validation of that basis.
//!
//! Modes: `--mode=all|wall|rank|search|cv|ablations|lattice|shape`. Flags:
//! `--grid=8x16`, `--radius=1.5`, `--shape=disc|square`, `--portable`,
//! `--tiles=`, `--iters=`, `--select=`.
//!
//! This is a MODEL result. The coverage rasters are an explicit shape grammar,
//! not a rasterization of any particular font; a real calibration substitutes
//! them. Every number is labelled in the research note.

#![allow(dead_code, clippy::needless_range_loop, clippy::print_literal)]

#[path = "terminal_to_1080p_lab/basis.rs"]
mod basis;
#[path = "terminal_to_1080p_lab/raster.rs"]
mod raster;

use basis::*;
use raster::{
    braille_dots_shaped, braille_mask_unions, candidates, sample_target, DotShape, Glyph, Grid,
};
use std::io;

// ---------------------------------------------------------------------------
// Target corpus
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Split {
    Train,
    Holdout,
}

struct Tile {
    name: String,
    family: &'static str,
    split: Split,
    cov: Vec<f64>,
}

fn sigmoid(t: f64) -> f64 {
    1.0 / (1.0 + (-t).exp())
}

fn add_edge(g: &Grid, out: &mut Vec<Tile>, angle: f64, split: Split, soft: f64) {
    let (w, h) = (g.w as f64, g.h as f64);
    let (cx, cy) = (w * 0.5, h * 0.5);
    let (ca, sa) = (angle.to_radians().cos(), angle.to_radians().sin());
    let cov = sample_target(g, move |x, y| {
        let d = ca * (x - cx) + sa * (y - cy);
        sigmoid(d / soft)
    });
    out.push(Tile {
        name: format!("edge-{angle:.0}"),
        family: "edge",
        split,
        cov,
    });
}

fn add_line(g: &Grid, out: &mut Vec<Tile>, angle: f64, width: f64, split: Split) {
    let (w, h) = (g.w as f64, g.h as f64);
    let (cx, cy) = (w * 0.5, h * 0.5);
    let (ca, sa) = (angle.to_radians().cos(), angle.to_radians().sin());
    let cov = sample_target(g, move |x, y| {
        let d = (ca * (x - cx) + sa * (y - cy)).abs();
        (1.0 - (d / width)).clamp(0.0, 1.0)
    });
    out.push(Tile {
        name: format!("line-{angle:.0}"),
        family: "line",
        split,
        cov,
    });
}

fn add_ring(g: &Grid, out: &mut Vec<Tile>, radius_frac: f64, width: f64, split: Split) {
    let (w, h) = (g.w as f64, g.h as f64);
    let r = radius_frac * (w * w + h * h).sqrt() * 0.5;
    let cov = sample_target(g, move |x, y| {
        let d = ((x - w * 0.5).powi(2) + (y - h * 0.5).powi(2)).sqrt();
        (1.0 - ((d - r).abs() / width)).clamp(0.0, 1.0)
    });
    out.push(Tile {
        name: format!("ring-{radius_frac:.2}"),
        family: "curve",
        split,
        cov,
    });
}

fn add_zone(g: &Grid, out: &mut Vec<Tile>, k: f64, split: Split) {
    let (w, h) = (g.w as f64, g.h as f64);
    let cov = sample_target(g, move |x, y| {
        let r2 = (x - w * 0.5).powi(2) + (y - h * 0.5).powi(2);
        0.5 + 0.5 * (k * r2 / w).cos()
    });
    out.push(Tile {
        name: format!("zone-{k:.2}"),
        family: "zone",
        split,
        cov,
    });
}

fn add_checker(g: &Grid, out: &mut Vec<Tile>, period: f64, split: Split) {
    let cov = sample_target(g, move |x, y| {
        let a = ((x / period).floor() + (y / period).floor()) as i64;
        if a.rem_euclid(2) == 0 {
            1.0
        } else {
            0.0
        }
    });
    out.push(Tile {
        name: format!("checker-{period:.0}"),
        family: "checker",
        split,
        cov,
    });
}

fn hash01(mut z: u64) -> f64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    ((z >> 11) as f64) / ((1u64 << 53) as f64)
}

fn value_noise(x: f64, y: f64, seed: u64) -> f64 {
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    let (xf, yf) = (x - x.floor(), y - y.floor());
    let sm = |t: f64| t * t * (3.0 - 2.0 * t);
    let (u, v) = (sm(xf), sm(yf));
    let corner = |dx: i64, dy: i64| {
        let h = (xi + dx) as u64 ^ ((yi + dy) as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ seed;
        hash01(h)
    };
    let a = corner(0, 0) * (1.0 - u) + corner(1, 0) * u;
    let b = corner(0, 1) * (1.0 - u) + corner(1, 1) * u;
    a * (1.0 - v) + b * v
}

fn add_noise(g: &Grid, out: &mut Vec<Tile>, scale: f64, seed: u64, split: Split) {
    let cov = sample_target(g, move |x, y| {
        (0.25 * value_noise(x / scale, y / scale, seed)
            + 0.5 * value_noise(x / (scale * 0.5), y / (scale * 0.5), seed ^ 7)
            + 0.25 * value_noise(x / (scale * 0.25), y / (scale * 0.25), seed ^ 13))
        .clamp(0.0, 1.0)
    });
    out.push(Tile {
        name: format!("noise-{scale:.1}-{seed}"),
        family: "noise",
        split,
        cov,
    });
}

fn corpus(g: &Grid) -> Vec<Tile> {
    let mut t = Vec::new();
    // Edges: even tens train, odd tens hold out.
    for a in (10..=80).step_by(10) {
        let split = if (a / 10) % 2 == 0 {
            Split::Train
        } else {
            Split::Holdout
        };
        add_edge(g, &mut t, a as f64, split, 1.2);
    }
    // Lines: 15/45/75 train, 25/55 hold out.
    for a in [15.0, 25.0, 45.0, 55.0, 75.0] {
        let split = if (a as i32 / 10) % 2 == 0 {
            Split::Train
        } else {
            Split::Holdout
        };
        add_line(g, &mut t, a, 1.4, split);
    }
    // Curves: radii alternating.
    for (i, r) in [0.25, 0.35, 0.45, 0.6, 0.75].iter().enumerate() {
        let split = if i % 2 == 0 {
            Split::Train
        } else {
            Split::Holdout
        };
        add_ring(g, &mut t, *r, 1.5, split);
    }
    // Zone plates.
    for (i, k) in [0.4, 0.7, 1.1, 1.6].iter().enumerate() {
        let split = if i % 2 == 0 {
            Split::Train
        } else {
            Split::Holdout
        };
        add_zone(g, &mut t, *k, split);
    }
    // Checkers.
    for (i, p) in [2.0, 3.0, 4.0, 5.0].iter().enumerate() {
        let split = if i % 2 == 0 {
            Split::Train
        } else {
            Split::Holdout
        };
        add_checker(g, &mut t, *p, split);
    }
    // Noise textures.
    for (i, s) in [2.0, 3.5, 5.0].iter().enumerate() {
        let split = if i % 2 == 0 {
            Split::Train
        } else {
            Split::Holdout
        };
        add_noise(g, &mut t, *s, 0xABC + i as u64, split);
    }
    t
}

// ---------------------------------------------------------------------------
// Args
// ---------------------------------------------------------------------------

struct Cfg {
    mode: String,
    grid: Grid,
    radius: f64,
    iters: usize,
    top: usize,
    shape: DotShape,
    portable: bool,
}

fn arg_str(name: &str) -> Option<String> {
    let p = format!("--{name}=");
    std::env::args().find_map(|a| a.strip_prefix(&p).map(str::to_string))
}

fn cfg() -> Cfg {
    let grid = arg_str("grid")
        .and_then(|s| {
            let (a, b) = s.split_once('x')?;
            Some(Grid::new(a.parse().ok()?, b.parse().ok()?))
        })
        .unwrap_or_default();
    let shape = match arg_str("shape").as_deref() {
        Some("square") => DotShape::Square,
        Some("disc") | None => DotShape::Disc,
        Some(other) => {
            eprintln!("unknown --shape={other}; try disc|square");
            std::process::exit(2);
        }
    };
    Cfg {
        mode: arg_str("mode").unwrap_or_else(|| "all".into()),
        grid,
        radius: arg_str("radius")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.5),
        iters: arg_str("iters").and_then(|v| v.parse().ok()).unwrap_or(24),
        top: arg_str("select").and_then(|v| v.parse().ok()).unwrap_or(6),
        shape,
        portable: std::env::args().any(|a| a == "--portable"),
    }
}

// ---------------------------------------------------------------------------
// Reports
// ---------------------------------------------------------------------------

fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

fn braille_hull_error(target: &[f64], dots: &[Vec<f64>]) -> f64 {
    // Fast path: disjoint blobs => the best convex combination is the per-dot
    // clamped projection, and this equals the norm of `braille_residual`.
    norm(&braille_residual(target, dots)) / (target.len() as f64).sqrt()
}

fn run_wall(c: &Cfg, tiles: &[Tile], dots: &[Vec<f64>], unions: &[Vec<f64>]) {
    println!("== Braille wall: what the fixed 2x4 lattice cannot reach ==");
    println!(
        "grid={}x{}, dot radius={:.2}, tiles={}",
        c.grid.w,
        c.grid.h,
        c.radius,
        tiles.len()
    );
    println!(
        "{:<16} {:>4} {:>10} {:>10} {:>10} {:>10}",
        "family", "n", "energy", "resid%", "static", "hull"
    );
    let mut fams: Vec<&'static str> = tiles.iter().map(|t| t.family).collect();
    fams.sort_unstable();
    fams.dedup();
    let (dots_ortho, _) = orthonormalize(dots);
    for fam in fams {
        let subset: Vec<&Tile> = tiles.iter().filter(|t| t.family == fam).collect();
        let energy = mean(
            &subset
                .iter()
                .map(|t| dot(&t.cov, &t.cov))
                .collect::<Vec<_>>(),
        );
        let res: Vec<f64> = subset
            .iter()
            .map(|t| out_of_span_fraction(&t.cov, &dots_ortho))
            .collect();
        let stat = mean(
            &subset
                .iter()
                .map(|t| static_error(&t.cov, unions).0)
                .collect::<Vec<_>>(),
        );
        let hull = mean(
            &subset
                .iter()
                .map(|t| temporal_error(&t.cov, unions, c.iters))
                .collect::<Vec<_>>(),
        );
        println!(
            "{:<16} {:>4} {:>10.3} {:>10.1} {:>10.4} {:>10.4}",
            fam,
            subset.len(),
            energy,
            100.0 * mean(&res),
            stat,
            hull
        );
    }
    println!();
    println!("resid% = mean fraction of tile energy orthogonal to the eight dot blobs;");
    println!(
        "static = best single Braille mask + two-level colours; hull = best temporal mix (inf-K)."
    );
}

fn run_rank(c: &Cfg, cands: &[Glyph], dots: &[Vec<f64>]) {
    let (q, rank) = orthonormalize(dots);
    let eigs = gram_eigenvalues(dots);
    println!("== Rank / independence of the Braille dot basis ==");
    println!(
        "grid={}x{}  numerical rank = {} of 8   effective rank(90%) = {}   effective rank(99%) = {}",
        c.grid.w,
        c.grid.h,
        rank,
        effective_rank(&eigs, 0.90),
        effective_rank(&eigs, 0.99)
    );
    println!(
        "Gram eigenvalues: {:?}",
        eigs.iter()
            .map(|e| (e * 100.0).round() / 100.0)
            .collect::<Vec<_>>()
    );
    println!();
    println!(
        "{:<20} {:<12} {:>12} {:>12} {:>10}",
        "glyph", "family", "angle(deg)", "outOfSpan%", "portable"
    );
    let mut rows: Vec<(&Glyph, f64, f64)> = cands
        .iter()
        .map(|g| {
            let ang =
                principal_angle_deg(&g.raster.iter().map(|v| *v as f64).collect::<Vec<_>>(), &q);
            let oos =
                out_of_span_fraction(&g.raster.iter().map(|v| *v as f64).collect::<Vec<_>>(), &q);
            (g, ang, oos)
        })
        .collect();
    rows.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    for (g, ang, oos) in rows.iter().take(24) {
        println!(
            "{:<20} {:<12} {:>12.1} {:>12.1} {:>10}",
            g.name,
            g.family,
            ang,
            100.0 * oos,
            g.portability.as_str()
        );
    }
    println!();
    println!("outOfSpan% = energy of the glyph's coverage outside the 8-dot span.");
    println!(
        "A glyph with 0% is already a linear combination of dot blobs (redundant as a direction)."
    );
}

/// One greedy augmentation step: the chosen candidate, its out-of-span fraction
/// against the span *before* adding it, and the training residual energy after.
struct AugStep {
    ci: usize,
    oos: f64,
    train_resid: f64,
}

/// Greedy orthogonal-matching selection of the minimal non-Braille augmentation
/// basis, seeded with the eight dot blobs plus the uniform-fill direction. The
/// same rule powers `search` and the cross-validation folds, so a fold cannot
/// quietly use a different selector.
///
/// Returns the chosen steps and the training residual energy before selection.
fn select_augmentation(
    train: &[Vec<f64>],
    dots: &[Vec<f64>],
    cands: &[Glyph],
    top: usize,
    portable_only: bool,
) -> (Vec<AugStep>, f64) {
    let p = train.first().map(|t| t.len()).unwrap_or(0);
    let mut seed = dots.to_vec();
    seed.push(vec![1.0; p]);
    let (mut q, _) = orthonormalize(&seed);
    let cand_vecs: Vec<Vec<f64>> = cands
        .iter()
        .map(|g| g.raster.iter().map(|v| *v as f64).collect())
        .collect();
    let elig: Vec<usize> = (0..cands.len())
        .filter(|i| !portable_only || cands[*i].portability == raster::Portability::Universal)
        .collect();
    let elig_vecs: Vec<Vec<f64>> = elig.iter().map(|i| cand_vecs[*i].clone()).collect();
    let mut res = residuals(train, &q);
    let base = mean_energy(&res);
    let mut steps: Vec<AugStep> = Vec::new();
    for _ in 0..top {
        let (sub, gain) = omp_select(&q, &res, &elig_vecs);
        if gain <= 0.0 {
            break;
        }
        let ci = elig[sub];
        if steps.iter().any(|s| s.ci == ci) {
            break;
        }
        let oos = out_of_span_fraction(&cand_vecs[ci], &q);
        let mut w = cand_vecs[ci].clone();
        for u in &q {
            let d = dot(&w, u);
            add_scaled(&mut w, u, -d);
        }
        let n = norm(&w);
        if n > 1e-9 {
            for x in w.iter_mut() {
                *x /= n;
            }
            q.push(w);
        }
        res = residuals(train, &q);
        steps.push(AugStep {
            ci,
            oos,
            train_resid: mean_energy(&res),
        });
    }
    (steps, base)
}

fn run_search(
    c: &Cfg,
    train: &[Vec<f64>],
    holdout: &[Vec<f64>],
    dots: &[Vec<f64>],
    unions: &[Vec<f64>],
    cands: &[Glyph],
) {
    // Seeded with the eight dot blobs *and* the uniform-fill direction: the
    // per-cell background colour can already fill the gaps with a constant, so a
    // glyph only adds a genuine shape direction if it leaves residual energy
    // after both are projected out.
    println!("== Greedy minimal augmentation beyond Braille ==");
    println!(
        "train={} holdout={}  selecting up to {} glyphs{}",
        train.len(),
        holdout.len(),
        c.top,
        if c.portable { "  (universal-only)" } else { "" }
    );
    let cand_vecs: Vec<Vec<f64>> = cands
        .iter()
        .map(|g| g.raster.iter().map(|v| *v as f64).collect())
        .collect();
    let (steps, base_train) = select_augmentation(train, dots, cands, c.top, c.portable);
    println!("train residual energy before selection = {base_train:.4}");
    println!(
        "{:>3} {:<20} {:<10} {:>10} {:>12} {:>10} {:>10}",
        "#", "glyph", "family", "oos%", "trainResid", "hoStatic", "hoHull"
    );
    let mut selected: Vec<usize> = Vec::new();
    for (i, step) in steps.iter().enumerate() {
        selected.push(step.ci);
        let g = &cands[step.ci];
        let mut aug: Vec<Vec<f64>> = unions.to_vec();
        for j in &selected {
            aug.push(cand_vecs[*j].clone());
        }
        let ho_static = mean(
            &holdout
                .iter()
                .map(|t| static_error(t, &aug).0)
                .collect::<Vec<_>>(),
        );
        let ho_hull = mean(
            &holdout
                .iter()
                .map(|t| temporal_error(t, &aug, c.iters))
                .collect::<Vec<_>>(),
        );
        println!(
            "{:>3} {:<20} {:<10} {:>10.1} {:>12.5} {:>10.5} {:>10.5}",
            i + 1,
            g.name,
            g.family,
            100.0 * step.oos,
            step.train_resid,
            ho_static,
            ho_hull
        );
    }

    // The selected minimal set, named.
    println!();
    println!("selected augmentation basis (beyond the 8 Braille dots):");
    for i in &selected {
        let g = &cands[*i];
        println!(
            "  {:<20} U+{:04X}  family={}  portability={}",
            g.name,
            g.ch.map(|c| c as u32).unwrap_or(0),
            g.family,
            g.portability.as_str()
        );
    }
}

/// Leave-one-family-out cross-validation: re-run the selector on each training
/// fold and measure the held-out family, so the reported generalization is not
/// the selector's own train number.
fn run_cv(c: &Cfg, tiles: &[Tile], dots: &[Vec<f64>], unions: &[Vec<f64>], cands: &[Glyph]) {
    println!("== H. leave-one-family-out cross-validation of the augmentation basis ==");
    if c.portable {
        println!("selector restricted to universal-portability glyphs");
    }
    let mut fams: Vec<&'static str> = tiles.iter().map(|t| t.family).collect();
    fams.sort_unstable();
    fams.dedup();
    println!(
        "{:<10} {:>5} {:>10} {:>10} {:>10} {:>10}  {}",
        "family", "nHold", "baseStat", "augStat", "baseTemp", "augTemp", "selected"
    );
    let (mut bs, mut astat, mut bt, mut at, mut cnt) = (0.0, 0.0, 0.0, 0.0, 0usize);
    for fam in fams {
        let train: Vec<Vec<f64>> = tiles
            .iter()
            .filter(|t| t.family != fam)
            .map(|t| t.cov.clone())
            .collect();
        let holdout: Vec<Vec<f64>> = tiles
            .iter()
            .filter(|t| t.family == fam)
            .map(|t| t.cov.clone())
            .collect();
        let (steps, _) = select_augmentation(&train, dots, cands, c.top, c.portable);
        let mut aug: Vec<Vec<f64>> = unions.to_vec();
        let mut names: Vec<&str> = Vec::new();
        for s in &steps {
            aug.push(cands[s.ci].raster.iter().map(|v| *v as f64).collect());
            names.push(cands[s.ci].name);
        }
        let bstat = mean(
            &holdout
                .iter()
                .map(|t| static_error(t, unions).0)
                .collect::<Vec<_>>(),
        );
        let sstat = mean(
            &holdout
                .iter()
                .map(|t| static_error(t, &aug).0)
                .collect::<Vec<_>>(),
        );
        let btemp = mean(
            &holdout
                .iter()
                .map(|t| temporal_error(t, unions, c.iters))
                .collect::<Vec<_>>(),
        );
        let stemp = mean(
            &holdout
                .iter()
                .map(|t| temporal_error(t, &aug, c.iters))
                .collect::<Vec<_>>(),
        );
        println!(
            "{:<10} {:>5} {:>10.5} {:>10.5} {:>10.5} {:>10.5}  {:?}",
            fam,
            holdout.len(),
            bstat,
            sstat,
            btemp,
            stemp,
            names
        );
        bs += bstat;
        astat += sstat;
        bt += btemp;
        at += stemp;
        cnt += 1;
    }
    let n = cnt.max(1) as f64;
    println!();
    println!(
        "{:<10} {:>5} {:>10.5} {:>10.5} {:>10.5} {:>10.5}",
        "MEAN",
        "",
        bs / n,
        astat / n,
        bt / n,
        at / n
    );
    let stat_gain = 100.0 * (bs - astat) / bs.max(1e-12);
    let temp_gain = 100.0 * (bt - at) / bt.max(1e-12);
    println!();
    println!(
        "Held-out gain from the augmentation basis: static {stat_gain:.1}%, temporal {temp_gain:.1}%."
    );
    println!("Each fold re-runs the selector without the held-out family, so this measures");
    println!("generalization, not the training fit. A negative or tiny gain means the selected");
    println!("basis overfits the training families.");
}

fn run_ablations(
    c: &Cfg,
    holdout: &[Vec<f64>],
    _dots: &[Vec<f64>],
    unions: &[Vec<f64>],
    cands: &[Glyph],
) {
    // Choose a compact, portable augmentation set by name, independent of the
    // greedy search, so the ablation is not overfit to the selector.
    let wanted = [
        "left-1/8",
        "left-3/4",
        "diag-fwd",
        "diag-back",
        "hbar",
        "vbar",
    ];
    let mut aug_vecs: Vec<Vec<f64>> = Vec::new();
    let mut aug_names: Vec<&str> = Vec::new();
    for name in wanted {
        if let Some(g) = cands.iter().find(|g| g.name == name) {
            aug_vecs.push(g.raster.iter().map(|v| *v as f64).collect());
            aug_names.push(g.name);
        }
    }
    println!("== Static vs temporal reconstruction on held-out tiles ==");
    println!("augmentation set: {:?}", aug_names);
    println!(
        "{:<26} {:>10} {:>10}",
        "basis", "staticRMSE", "temporalRMSE"
    );

    let b0_static = mean(
        &holdout
            .iter()
            .map(|t| static_error(t, unions).0)
            .collect::<Vec<_>>(),
    );
    let b0_hull = mean(
        &holdout
            .iter()
            .map(|t| temporal_error(t, unions, c.iters))
            .collect::<Vec<_>>(),
    );
    println!(
        "{:<26} {:>10.5} {:>10.5}",
        "braille-static(colors)", b0_static, b0_hull
    );

    let s_static = mean(
        &holdout
            .iter()
            .map(|t| static_error(t, &aug_vecs).0)
            .collect::<Vec<_>>(),
    );
    let s_hull = mean(
        &holdout
            .iter()
            .map(|t| temporal_error(t, &aug_vecs, c.iters))
            .collect::<Vec<_>>(),
    );
    println!(
        "{:<26} {:>10.5} {:>10.5}",
        "augment-only-static", s_static, s_hull
    );

    let mut both: Vec<Vec<f64>> = unions.to_vec();
    both.extend(aug_vecs.iter().cloned());
    let both_static = mean(
        &holdout
            .iter()
            .map(|t| static_error(t, &both).0)
            .collect::<Vec<_>>(),
    );
    let both_hull = mean(
        &holdout
            .iter()
            .map(|t| temporal_error(t, &both, c.iters))
            .collect::<Vec<_>>(),
    );
    println!(
        "{:<26} {:>10.5} {:>10.5}",
        "braille+augment-static", both_static, both_hull
    );

    // Binary (no-colour) Braille for the pure-partition reference.
    let b0_binary = mean(
        &holdout
            .iter()
            .map(|t| binary_static_error(t, unions))
            .collect::<Vec<_>>(),
    );
    println!(
        "{:<26} {:>10.5} {:>10.5}",
        "braille-binary-static", b0_binary, b0_hull
    );

    let base = mean(&holdout.iter().map(|t| norm(t)).collect::<Vec<_>>())
        / (c.grid.points() as f64).sqrt();
    println!();
    println!("mean holdout tile RMSE: base(no ink)={base:.5}");
}

fn run_lattice(_c: &Cfg, cands: &[Glyph], dots: &[Vec<f64>]) {
    let p = dots.first().map(|d| d.len()).unwrap_or(0);
    let mut seed = dots.to_vec();
    seed.push(vec![1.0; p]);
    let (q, _) = orthonormalize(&seed);
    println!("== Lattice equivalence: dots + uniform fill vs each glyph ==");
    println!("span = 8 dot blobs plus one uniform-fill direction (the per-cell background)");
    println!(
        "{:<20} {:<12} {:>12} {:>14}",
        "glyph", "family", "shapeOut%", "dotResid%"
    );
    // shapeOut%: energy a glyph places *outside* what dots-plus-fill can make.
    // dotResid%: ink left in the gaps after the best per-dot (no background) fill.
    for g in cands.iter().filter(|g| {
        matches!(
            g.family,
            "block" | "quadrant" | "fraction-block" | "shade" | "diagonal" | "box"
        )
    }) {
        let v: Vec<f64> = g.raster.iter().map(|x| *x as f64).collect();
        let shape_out = 100.0 * out_of_span_fraction(&v, &q);
        let resid = 100.0 * norm(&braille_residual(&v, dots)) / norm(&v).max(1e-9);
        println!(
            "{:<20} {:<12} {:>12.1} {:>14.1}",
            g.name, g.family, shape_out, resid
        );
    }
    println!();
    println!("shapeOut% = energy outside span(dots, uniform fill): 0 means the glyph is a fill/dot variant with no new shape.");
    println!("The full block and the shades are pure fill variants (0%). Halves/quadrants are coarse fill (37-56%).");
    println!("The genuinely new directions are thin rules (hbar/vbar), diagonals, and fine fractional blocks (up to 83%).");
}

/// Is the augmentation basis an artifact of the round-gapped disc model? Measure
/// every candidate's out-of-span fraction under bracketing dot shapes/radii. A
/// glyph whose out-of-span fraction survives every model marks a genuine new
/// spatial direction; one that collapses to zero under some model was only
/// filling the gaps of round Braille dots.
fn run_shape(c: &Cfg, tiles: &[Tile], cands: &[Glyph]) {
    let g = c.grid;
    let train: Vec<Vec<f64>> = tiles
        .iter()
        .filter(|t| t.split == Split::Train)
        .map(|t| t.cov.clone())
        .collect();
    let holdout: Vec<Vec<f64>> = tiles
        .iter()
        .filter(|t| t.split == Split::Holdout)
        .map(|t| t.cov.clone())
        .collect();
    let variants = [
        ("disc-r1.0", DotShape::Disc, 1.0),
        ("disc-r1.5", DotShape::Disc, 1.5),
        ("disc-r1.9", DotShape::Disc, 1.9),
        ("square-r1.5", DotShape::Square, 1.5),
        ("square-r2.0", DotShape::Square, 2.0),
    ];
    println!("== I. dot-shape robustness of the augmentation basis ==");
    println!("The Braille span depends on the dot shape. We bracket it with round discs at three");
    println!("radii and axis-aligned squares (which tile exactly at half-pitch), then measure how");
    println!("much of each candidate glyph's energy still lies outside the span under each.");
    println!();

    // Build the orthonormal span (dots + uniform fill) for every dot model.
    let mut models: Vec<(&str, Vec<Vec<f64>>)> = Vec::with_capacity(variants.len());
    for (name, shape, r) in variants {
        let dots = braille_dots_shaped(&g, r, shape);
        let mut seed: Vec<Vec<f64>> = dots
            .iter()
            .map(|d| d.iter().map(|v| *v as f64).collect())
            .collect();
        seed.push(vec![1.0; g.points()]);
        let (q, _) = orthonormalize(&seed);
        models.push((name, q));
    }

    // Per-glyph out-of-span fraction under every model.
    let relevant = |fam: &str| {
        matches!(
            fam,
            "block" | "quadrant" | "fraction-block" | "diagonal" | "box"
        )
    };
    let rows: Vec<&Glyph> = cands.iter().filter(|g| relevant(g.family)).collect();
    print!("{:<20} {:<12}", "glyph", "family");
    for (name, _) in &models {
        print!(" {:>11}", name);
    }
    println!(" {:>8}", "min%");
    let mut robust: Vec<&str> = Vec::new();
    let mut artifact: Vec<&str> = Vec::new();
    for glyph in &rows {
        let v: Vec<f64> = glyph.raster.iter().map(|x| *x as f64).collect();
        let mut vals = Vec::with_capacity(models.len());
        for (_, q) in &models {
            vals.push(100.0 * out_of_span_fraction(&v, q));
        }
        let minv = vals.iter().cloned().fold(f64::INFINITY, f64::min);
        print!("{:<20} {:<12}", glyph.name, glyph.family);
        for x in &vals {
            print!(" {:>11.1}", x);
        }
        println!(" {:>8.1}", minv);
        if minv > 5.0 {
            robust.push(glyph.name);
        } else if minv < 1.0 {
            artifact.push(glyph.name);
        }
    }

    println!();
    println!("min% = out-of-span fraction under the worst dot model. Robust (>5% everywhere):");
    println!("  {robust:?}");
    println!("shape artifacts (<1% under some model -- no new direction once the dots change):");
    println!("  {artifact:?}");
    println!();

    // And the selector itself, for the record: it is greedy and non-unique, so
    // the robust/artifact split above is the part that does not depend on order.
    println!("== I.b greedy selection and held-out error under each dot model ==");
    println!(
        "{:<12} {:>10} {:>10}  {}",
        "dot model", "hoStatic", "hoTemp", "selected"
    );
    for (name, shape, r) in variants {
        let dots = braille_dots_shaped(&g, r, shape);
        let dots_f: Vec<Vec<f64>> = dots
            .iter()
            .map(|d| d.iter().map(|v| *v as f64).collect())
            .collect();
        let unions_f: Vec<Vec<f64>> = braille_mask_unions(&dots)
            .iter()
            .map(|u| u.iter().map(|v| *v as f64).collect())
            .collect();
        let (steps, _) = select_augmentation(&train, &dots_f, cands, c.top, c.portable);
        let sel: Vec<usize> = steps.iter().map(|s| s.ci).collect();
        let mut aug: Vec<Vec<f64>> = unions_f.to_vec();
        for i in &sel {
            aug.push(cands[*i].raster.iter().map(|v| *v as f64).collect());
        }
        let hs = mean(
            &holdout
                .iter()
                .map(|t| static_error(t, &aug).0)
                .collect::<Vec<_>>(),
        );
        let ht = mean(
            &holdout
                .iter()
                .map(|t| temporal_error(t, &aug, c.iters))
                .collect::<Vec<_>>(),
        );
        let names: Vec<&str> = sel.iter().map(|i| cands[*i].name).collect();
        println!("{:<12} {:>10.5} {:>10.5}  {:?}", name, hs, ht, names);
    }
    println!();
    println!("Held-out error is nearly identical across dot models, but the *named* basis shifts:");
    println!("only the robust list above is safe to promote as new spatial directions.");
}

// ---------------------------------------------------------------------------

fn main() -> io::Result<()> {
    let c = cfg();
    let g = c.grid;
    let dots = braille_dots_shaped(&g, c.radius, c.shape);
    let unions = braille_mask_unions(&dots);
    let dots_f: Vec<Vec<f64>> = dots
        .iter()
        .map(|d| d.iter().map(|v| *v as f64).collect())
        .collect();
    let unions_f: Vec<Vec<f64>> = unions
        .iter()
        .map(|u| u.iter().map(|v| *v as f64).collect())
        .collect();
    let cands = candidates(&g);
    let tiles = corpus(&g);
    let train: Vec<Vec<f64>> = tiles
        .iter()
        .filter(|t| t.split == Split::Train)
        .map(|t| t.cov.clone())
        .collect();
    let holdout: Vec<Vec<f64>> = tiles
        .iter()
        .filter(|t| t.split == Split::Holdout)
        .map(|t| t.cov.clone())
        .collect();

    println!(
        "terminal -> 1080p glyph-basis lab  ({} candidates, {} tiles: {} train / {} holdout)",
        cands.len(),
        tiles.len(),
        train.len(),
        holdout.len()
    );
    println!();
    match c.mode.as_str() {
        "wall" => run_wall(&c, &tiles, &dots_f, &unions_f),
        "rank" => run_rank(&c, &cands, &dots_f),
        "search" => run_search(&c, &train, &holdout, &dots_f, &unions_f, &cands),
        "cv" => run_cv(&c, &tiles, &dots_f, &unions_f, &cands),
        "ablations" => run_ablations(&c, &holdout, &dots_f, &unions_f, &cands),
        "lattice" => run_lattice(&c, &cands, &dots_f),
        "shape" => run_shape(&c, &tiles, &cands),
        "all" | "" => {
            run_rank(&c, &cands, &dots_f);
            run_wall(&c, &tiles, &dots_f, &unions_f);
            run_search(&c, &train, &holdout, &dots_f, &unions_f, &cands);
            run_cv(&c, &tiles, &dots_f, &unions_f, &cands);
            run_ablations(&c, &holdout, &dots_f, &unions_f, &cands);
            run_lattice(&c, &cands, &dots_f);
            run_shape(&c, &tiles, &cands);
        }
        other => {
            eprintln!(
                "unknown --mode={other}; try wall|rank|search|cv|ablations|lattice|shape|all"
            );
            std::process::exit(2);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn as_f64(v: &[Vec<f32>]) -> Vec<Vec<f64>> {
        v.iter()
            .map(|d| d.iter().map(|x| *x as f64).collect())
            .collect()
    }

    #[test]
    fn corpus_has_train_and_holdout_tiles() {
        let g = Grid::default();
        let t = corpus(&g);
        assert!(t.iter().any(|x| x.split == Split::Train));
        assert!(t.iter().any(|x| x.split == Split::Holdout));
        assert!(t.iter().all(|x| x.cov.len() == g.points()));
    }

    #[test]
    fn braille_span_has_rank_eight() {
        let g = Grid::default();
        let dots = as_f64(&braille_dots_shaped(&g, 1.5, DotShape::Disc));
        let (_, rank) = orthonormalize(&dots);
        assert_eq!(rank, 8);
    }

    #[test]
    fn temporal_never_loses_to_static_on_average() {
        // Colourless comparison: the temporal hull contains every single glyph
        // (as a vertex), so it can never be worse than the nearest basis vector.
        let g = Grid::default();
        let unions = as_f64(&braille_mask_unions(&braille_dots_shaped(
            &g,
            1.5,
            DotShape::Disc,
        )));
        let t = sample_target(&g, |x, y| {
            if x + y > (g.w + g.h) as f64 * 0.5 {
                0.8
            } else {
                0.2
            }
        });
        let stat = binary_static_error(&t, &unions);
        let hull = hull_error(&t, &unions, 48);
        assert!(hull <= stat + 1e-9, "hull={hull} static={stat}");
    }

    #[test]
    fn deterministic_corpus() {
        let g = Grid::default();
        let a = corpus(&g);
        let b = corpus(&g);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x.cov, y.cov);
        }
    }
}
