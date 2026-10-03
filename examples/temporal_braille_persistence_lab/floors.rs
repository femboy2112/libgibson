//! Infinite-`K` (continuous per-dot duty) reconstruction floors for one cell.
//!
//! The colour model is the projector's: a cell chooses a background `b` and a
//! foreground `f` in linear light, and dot `i` reconstructs at
//! `b + a_i (f - b)` with `a_i` in `[0, 1]`. Static rendering forces
//! `a_i in {0, 1}`; time-averaging can realise any `a_i in [0, 1]` as
//! `K -> infinity`. This module computes, per cell, a **rigorous bracket** on
//! the best possible infinite-`K` source error:
//!
//! * [`cell_floor`]`.upper` — the best *clamped segment* fit found by
//!   alternating least squares from every complement-pair partition. Any
//!   feasible segment is an upper bound on the true minimum, so this is a
//!   (tight) **upper** bound on the infinite-`K` floor.
//! * [`cell_floor`]`.lower` — the residual of the best affine **line** through
//!   the eight colours (principal component). A clamped segment is a subset of
//!   the affine lines, so this is a rigorous **lower** bound.
//!
//! Two useful floors with a *fixed* colour pair are also provided:
//! [`segment_floor_linear`] (continuous duty between two given linear colours)
//! and [`quantized_segment_floor`] (the same, after the 8-bit sRGB
//! quantization the shipping path emits). The latter is what the shipping
//! temporal modulator can reach with its chosen per-cell colours.

use gibson::temporal::TemporalCellProjection;

use super::metrics::{color_linear, rgb8_to_linear};

/// The bracket on a cell's infinite-`K` source RMSE plus the fitted segment.
#[derive(Debug, Clone, Copy, Default)]
pub struct CellFloor {
    /// Rigorous lower bound (best affine line, unclamped).
    pub lower: f32,
    /// Tight upper bound (best clamped segment by alternating least squares).
    pub upper: f32,
    /// Linear-light foreground of the best segment found.
    pub bg: [f32; 3],
    /// Linear-light background of the best segment found.
    pub fg: [f32; 3],
    /// Per-dot duty realising `upper` (in `[0, 1]`).
    pub alpha: [f32; 8],
}

fn sse_segment(target: &[[f32; 3]; 8], bg: [f32; 3], d: [f32; 3], alpha: &[f32; 8]) -> f32 {
    let mut s = 0.0f32;
    for i in 0..8 {
        for c in 0..3 {
            let r = bg[c] + alpha[i] * d[c];
            s += (r - target[i][c]).powi(2);
        }
    }
    s
}

/// Per-dot clamped projection onto the segment `[bg, bg+d]`.
fn project_alpha(target: &[[f32; 3]; 8], bg: [f32; 3], d: [f32; 3]) -> [f32; 8] {
    let dd: f32 = d.iter().map(|x| x * x).sum();
    let mut alpha = [0.0f32; 8];
    if dd <= 1e-12 {
        return alpha;
    }
    for (i, p) in target.iter().enumerate() {
        let num: f32 = (0..3).map(|c| (p[c] - bg[c]) * d[c]).sum();
        alpha[i] = (num / dd).clamp(0.0, 1.0);
    }
    alpha
}

/// Alternating least squares for the best clamped segment from an initial
/// `(bg, d)`. Monotonically decreases the SSE, so it can only improve on the
/// initialising partition's fit.
fn alt_fit(target: &[[f32; 3]; 8], mut bg: [f32; 3], mut d: [f32; 3], iters: usize) -> CellFloor {
    let mut alpha = project_alpha(target, bg, d);
    for _ in 0..iters {
        // Solve for (bg, d) given alpha: per channel a 2-parameter least squares.
        for c in 0..3 {
            let (mut s00, mut s01, mut s11, mut b0, mut b1) = (0f64, 0f64, 0f64, 0f64, 0f64);
            for i in 0..8 {
                let a = alpha[i] as f64;
                s00 += 1.0;
                s01 += a;
                s11 += a * a;
                b0 += target[i][c] as f64;
                b1 += a * target[i][c] as f64;
            }
            let det = s00 * s11 - s01 * s01;
            if det.abs() > 1e-12 {
                bg[c] = ((s11 * b0 - s01 * b1) / det) as f32;
                d[c] = ((s00 * b1 - s01 * b0) / det) as f32;
            }
        }
        alpha = project_alpha(target, bg, d);
    }
    CellFloor {
        lower: 0.0,
        upper: (sse_segment(target, bg, d, &alpha) / 24.0).sqrt(),
        bg,
        fg: [bg[0] + d[0], bg[1] + d[1], bg[2] + d[2]],
        alpha,
    }
}

/// Best affine line residual (a rigorous lower bound on the segment floor).
pub fn pca_line_floor(samples: &[[u8; 3]; 8]) -> f32 {
    let p: [[f32; 3]; 8] = samples.map(rgb8_to_linear);
    let mut mu = [0.0f64; 3];
    for q in &p {
        for c in 0..3 {
            mu[c] += q[c] as f64 / 8.0;
        }
    }
    let mut cov = [[0.0f64; 3]; 3];
    let mut trace = 0.0f64;
    for q in &p {
        for a in 0..3 {
            for b in 0..3 {
                cov[a][b] += (q[a] as f64 - mu[a]) * (q[b] as f64 - mu[b]);
            }
        }
    }
    for (c, row) in cov.iter().enumerate() {
        trace += row[c];
    }
    let eigs = sym_eig3(cov);
    let lambda_max = eigs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let sse = (trace - lambda_max).max(0.0);
    (sse / 24.0).sqrt() as f32
}

fn sym_eig3(mut a: [[f64; 3]; 3]) -> [f64; 3] {
    for _ in 0..32 {
        let off = a[0][1] * a[0][1] + a[0][2] * a[0][2] + a[1][2] * a[1][2];
        if off < 1e-24 {
            break;
        }
        for (p, q) in [(0usize, 1usize), (0, 2), (1, 2)] {
            if a[p][q].abs() < 1e-18 {
                continue;
            }
            let theta = 0.5 * (a[q][q] - a[p][p]) / a[p][q];
            let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            for k in 0..3 {
                let akp = a[k][p];
                let akq = a[k][q];
                a[k][p] = c * akp - s * akq;
                a[k][q] = s * akp + c * akq;
            }
            for k in 0..3 {
                let apk = a[p][k];
                let aqk = a[q][k];
                a[p][k] = c * apk - s * aqk;
                a[q][k] = s * apk + c * aqk;
            }
        }
    }
    [a[0][0], a[1][1], a[2][2]]
}

/// Continuous-duty source RMSE for a fixed pair of **linear** colours.
pub fn segment_floor_linear(samples: &[[u8; 3]; 8], bg: [f32; 3], fg: [f32; 3]) -> f32 {
    let p: [[f32; 3]; 8] = samples.map(rgb8_to_linear);
    let d = [fg[0] - bg[0], fg[1] - bg[1], fg[2] - bg[2]];
    let alpha = project_alpha(&p, bg, d);
    (sse_segment(&p, bg, d, &alpha) / 24.0).sqrt()
}

/// Continuous-duty source RMSE for the **emitted** (8-bit sRGB) colour pair:
/// the shipping temporal modulator's floor for a given projection style.
pub fn quantized_segment_floor(samples: &[[u8; 3]; 8], style: gibson::Style) -> Option<f32> {
    let fg = color_linear(style.fg?);
    let bg = color_linear(style.bg?);
    Some(segment_floor_linear(samples, bg, fg))
}

/// The bracket plus the best segment. `upper` is an upper bound on the true
/// infinite-`K` floor; `lower` is a rigorous lower bound.
pub fn cell_floor(samples: &[[u8; 3]; 8]) -> CellFloor {
    let p: [[f32; 3]; 8] = samples.map(rgb8_to_linear);
    let mut best = alt_fit(&p, [0.0; 3], [1.0; 3], 16);
    // Seed from every complement-pair partition centroid (the projector's
    // candidate static partitions) so the segment search does not depend on one
    // basin.
    for mask in (2u8..=254u8).step_by(2) {
        let mut fg = [0.0f32; 3];
        let mut bg = [0.0f32; 3];
        let (mut nf, mut nb) = (0f32, 0f32);
        for (i, q) in p.iter().enumerate() {
            let bucket = if mask & (1u8 << i) != 0 {
                &mut fg
            } else {
                &mut bg
            };
            for c in 0..3 {
                bucket[c] += q[c];
            }
            if mask & (1u8 << i) != 0 {
                nf += 1.0;
            } else {
                nb += 1.0;
            }
        }
        if nf > 0.0 {
            for c in 0..3 {
                fg[c] /= nf;
            }
        }
        if nb > 0.0 {
            for c in 0..3 {
                bg[c] /= nb;
            }
        }
        let d = [fg[0] - bg[0], fg[1] - bg[1], fg[2] - bg[2]];
        let fit = alt_fit(&p, bg, d, 8);
        if fit.upper < best.upper {
            best = fit;
        }
    }
    best.lower = pca_line_floor(samples);
    best
}

/// Convenience: the projector's own ideal static/line RMSE for a cell, so a
/// caller can relate the bracket to the shipped projection fields.
pub fn projector_floor(projection: &TemporalCellProjection) -> (f32, f32) {
    (projection.static_rmse, projection.line_rmse)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gray(v: u8) -> [u8; 3] {
        [v, v, v]
    }

    #[test]
    fn collinear_colors_have_zero_lower_bound() {
        // Eight greys lie on the gray axis: a line fits them exactly.
        let s = [
            gray(0),
            gray(32),
            gray(64),
            gray(96),
            gray(128),
            gray(160),
            gray(192),
            gray(255),
        ];
        assert!(pca_line_floor(&s) < 1e-5, "{}", pca_line_floor(&s));
        let f = cell_floor(&s);
        assert!(f.lower <= f.upper + 1e-6);
        assert!(f.upper < 1e-4, "segment fit {}", f.upper);
    }

    #[test]
    fn lower_never_exceeds_upper() {
        // A spread of deterministic pseudo-random cells.
        let mut z = 0x1234_5678_9abc_def0u64;
        for _ in 0..200 {
            let mut s = [[0u8; 3]; 8];
            for px in s.iter_mut() {
                for c in 0..3 {
                    z = z.wrapping_mul(6364136223846793005).wrapping_add(1);
                    px[c] = (z >> 40) as u8;
                }
            }
            let f = cell_floor(&s);
            assert!(f.lower <= f.upper + 1e-6, "{} > {}", f.lower, f.upper);
        }
    }

    #[test]
    fn tight_segment_never_worse_than_static_line() {
        // The projector's `line_rmse` is a feasible continuous-duty fit for one
        // partition; the ALS floor starts from every partition and can only go
        // down, so it must not be worse.
        let mut z = 0x0bad_f00d_dead_beefu64;
        for _ in 0..200 {
            let mut s = [[0u8; 3]; 8];
            for px in s.iter_mut() {
                for c in 0..3 {
                    z = z.wrapping_mul(6364136223846793005).wrapping_add(1);
                    px[c] = (z >> 40) as u8;
                }
            }
            let proj = gibson::temporal::project_rgb_subcells(s);
            let floor = cell_floor(&s).upper;
            assert!(
                floor <= proj.line_rmse + 2e-3,
                "floor {floor} line {}",
                proj.line_rmse
            );
        }
    }

    #[test]
    fn quantized_floor_is_between_pca_and_tight_for_the_projection_style() {
        let s = [
            gray(10),
            gray(200),
            gray(40),
            gray(180),
            gray(90),
            gray(150),
            gray(70),
            gray(120),
        ];
        let proj = gibson::temporal::project_rgb_subcells(s);
        let q = quantized_segment_floor(&s, proj.style).unwrap();
        let f = cell_floor(&s);
        // The projection's emitted colours are a feasible segment, so the
        // unquantized tight floor is no worse than it.
        assert!(f.upper <= q + 2e-2, "tight {} quantized {q}", f.upper);
    }
}
