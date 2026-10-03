//! Linear-algebra and geometric instruments for the glyph-basis experiment.
//!
//! All routines work on coverage vectors in `R^P` (`P = W*H` calibrated
//! samples). They answer four questions the addendum must make concrete:
//!
//! 1. **Algebraic independence** — the numerical rank and effective rank of a
//!    basis, and the principal angle of a candidate glyph to the Braille span
//!    (`= 0` means the candidate is already a linear combination of the eight
//!    dot blobs, even if its *static* raster differs).
//! 2. **Static realization** — the best single glyph plus a two-level
//!    foreground/background fit (the per-cell colour model), i.e. one cell, one
//!    instant.
//! 3. **Temporal realization** — the best point in the convex hull of the glyph
//!    rasters (free time-multiplexing weights, the infinite-`K` limit).
//! 4. **Minimal augmentation** — greedy orthogonal-matching selection of the
//!    smallest set of non-Braille glyphs that adds the missing directions.
//!
//! The static and temporal fits use coverage-space colours (a background level
//! `b` plus a foreground gain `a`) rather than full RGB; this isolates the
//! **spatial** basis question, which is what the addendum asks, and is stated as
//! the model's boundary in the research note.

pub type VecN = Vec<f64>;

pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

pub fn norm(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

pub fn sub(a: &[f64], b: &[f64]) -> VecN {
    a.iter().zip(b).map(|(x, y)| x - y).collect()
}

pub fn add_scaled(a: &mut [f64], b: &[f64], s: f64) {
    for (x, y) in a.iter_mut().zip(b) {
        *x += s * y;
    }
}

/// Orthonormal basis (Gram-Schmidt with a relative tolerance) and its rank.
pub fn orthonormalize(basis: &[VecN]) -> (Vec<VecN>, usize) {
    let mut q: Vec<VecN> = Vec::new();
    for v in basis {
        let mut w = v.clone();
        let n0 = norm(&w);
        if n0 <= 1e-12 {
            continue;
        }
        for u in &q {
            let d = dot(&w, u);
            add_scaled(&mut w, u, -d);
        }
        let n = norm(&w);
        if n > 1e-9 * n0.max(1.0) {
            for x in w.iter_mut() {
                *x /= n;
            }
            q.push(w);
        }
    }
    let rank = q.len();
    (q, rank)
}

/// Projects `v` onto the orthonormal basis `q`, returning the projection
/// coefficient norm and the residual norm.
pub fn projection_stats(v: &[f64], q: &[VecN]) -> (f64, f64) {
    let nv = norm(v);
    if nv <= 1e-12 {
        return (0.0, 0.0);
    }
    let mut w = v.to_vec();
    for u in q {
        let d = dot(v, u);
        add_scaled(&mut w, u, -d);
    }
    let res = norm(&w);
    let proj = (nv * nv - res * res).max(0.0).sqrt();
    (proj, res)
}

/// Principal angle, in degrees, of `v` to the span of `q`.
pub fn principal_angle_deg(v: &[f64], q: &[VecN]) -> f64 {
    let (proj, _) = projection_stats(v, q);
    let nv = norm(v);
    if nv <= 1e-12 {
        return 90.0;
    }
    let c = (proj / nv).clamp(0.0, 1.0);
    c.acos().to_degrees()
}

/// Fraction of `v`'s energy lying outside the span of `q`.
pub fn out_of_span_fraction(v: &[f64], q: &[VecN]) -> f64 {
    let nv = norm(v);
    if nv <= 1e-12 {
        return 0.0;
    }
    let (_, res) = projection_stats(v, q);
    (res / nv).powi(2)
}

/// Eigenvalues of the Gram matrix `B B^T` (descending), via cyclic Jacobi.
/// These are the squared singular values of the basis.
pub fn gram_eigenvalues(basis: &[VecN]) -> Vec<f64> {
    let n = basis.len();
    if n == 0 {
        return Vec::new();
    }
    let mut a = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        for j in i..n {
            let v = dot(&basis[i], &basis[j]);
            a[i][j] = v;
            a[j][i] = v;
        }
    }
    jacobi_eigen(&mut a)
}

fn jacobi_eigen(a: &mut [Vec<f64>]) -> Vec<f64> {
    let n = a.len();
    for _ in 0..100 {
        let mut off = 0.0f64;
        for i in 0..n {
            for j in (i + 1)..n {
                off += a[i][j] * a[i][j];
            }
        }
        if off < 1e-18 {
            break;
        }
        for p in 0..n {
            for q in (p + 1)..n {
                if a[p][q].abs() < 1e-15 {
                    continue;
                }
                let theta = 0.5 * (a[q][q] - a[p][p]) / a[p][q];
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..n {
                    let akp = a[k][p];
                    let akq = a[k][q];
                    a[k][p] = c * akp - s * akq;
                    a[k][q] = s * akp + c * akq;
                }
                for k in 0..n {
                    let apk = a[p][k];
                    let aqk = a[q][k];
                    a[p][k] = c * apk - s * aqk;
                    a[q][k] = s * apk + c * aqk;
                }
            }
        }
    }
    let mut eigs: Vec<f64> = (0..n).map(|i| a[i][i]).collect();
    eigs.sort_by(|x, y| y.partial_cmp(x).unwrap_or(std::cmp::Ordering::Equal));
    eigs
}

/// Smallest `k` such that the top `k` eigenvalues capture `frac` of the energy.
pub fn effective_rank(eigs: &[f64], frac: f64) -> usize {
    let total: f64 = eigs.iter().sum();
    if total <= 1e-18 {
        return 0;
    }
    let mut acc = 0.0;
    for (i, e) in eigs.iter().enumerate() {
        acc += e;
        if acc >= frac * total {
            return i + 1;
        }
    }
    eigs.len()
}

/// Best single-glyph two-level fit: `recon = b + a * g`, `a,b` in `[0,1]`.
/// Returns `(rmse, glyph_index, background, gain)`.
pub fn static_error(target: &[f64], glyphs: &[VecN]) -> (f64, usize, f64, f64) {
    let p = target.len() as f64;
    let t1: f64 = target.iter().sum();
    let mut best = (f64::INFINITY, 0usize, 0.0, 0.0);
    for (idx, g) in glyphs.iter().enumerate() {
        let gg = dot(g, g);
        let g1: f64 = g.iter().sum();
        let tg = dot(target, g);
        let det = p * gg - g1 * g1;
        let (mut a, mut b) = if det.abs() > 1e-9 {
            let a0 = (p * tg - g1 * t1) / det;
            let b0 = (gg * t1 - g1 * tg) / det;
            (a0, b0)
        } else {
            (0.0, (t1 / p).clamp(0.0, 1.0))
        };
        // Physical constraint: background in [0,1], foreground = b+a in [0,1].
        a = a.clamp(0.0, 1.0);
        b = b.clamp(0.0, 1.0 - a);
        let mut sse = 0.0f64;
        for (t, gv) in target.iter().zip(g) {
            let r = (b + a * gv).clamp(0.0, 1.0);
            sse += (r - t).powi(2);
        }
        let rmse = (sse / p).sqrt();
        if rmse < best.0 {
            best = (rmse, idx, b, a);
        }
    }
    best
}

/// Best point of the convex hull of `glyphs` approximating `target`, via
/// Frank-Wolfe on `0.5 ||t - q||^2`. Returned as a coverage vector.
pub fn hull_point(target: &[f64], glyphs: &[VecN], iters: usize) -> VecN {
    if glyphs.is_empty() {
        return vec![target.iter().sum::<f64>() / target.len() as f64; target.len()];
    }
    let mut best = f64::INFINITY;
    let mut q = glyphs[0].clone();
    for g in glyphs {
        let d: f64 = target.iter().zip(g).map(|(t, x)| (t - x).powi(2)).sum();
        if d < best {
            best = d;
            q = g.clone();
        }
    }
    for _ in 0..iters {
        // Gradient of 0.5||target-q||^2 is (q - target); the FW vertex is the
        // minimizer of <grad, s>.
        let mut arg = 0usize;
        let mut argval = f64::INFINITY;
        for (i, g) in glyphs.iter().enumerate() {
            let v: f64 = q
                .iter()
                .zip(target)
                .zip(g)
                .map(|((qi, ti), gi)| (qi - ti) * gi)
                .sum();
            if v < argval {
                argval = v;
                arg = i;
            }
        }
        // Exact line search along q -> glyphs[arg].
        let s = &glyphs[arg];
        let num: f64 = q
            .iter()
            .zip(target)
            .zip(s)
            .map(|((qi, ti), si)| (qi - ti) * (si - qi))
            .sum();
        let den: f64 = q.iter().zip(s).map(|(qi, si)| (si - qi).powi(2)).sum();
        if den < 1e-18 {
            break;
        }
        let gamma = (-num / den).clamp(0.0, 1.0);
        for (qi, si) in q.iter_mut().zip(s) {
            *qi += gamma * (si - *qi);
        }
    }
    q
}

/// Distance-to-convex-hull via Frank-Wolfe (colourless), as per-sample RMSE.
pub fn hull_error(target: &[f64], glyphs: &[VecN], iters: usize) -> f64 {
    let q = hull_point(target, glyphs, iters);
    (target
        .iter()
        .zip(&q)
        .map(|(t, x)| (t - x).powi(2))
        .sum::<f64>()
        / target.len() as f64)
        .sqrt()
}

/// Frank-Wolfe duality gap for `0.5 ||target - q||^2` at `q`, for the convex
/// hull of `glyphs`. It is a nonnegative bound on `f(q) - min f`; zero means `q`
/// is optimal up to floating point. Used to certify that [`hull_point`] has
/// actually converged rather than stopped on the iteration budget.
pub fn hull_gap(target: &[f64], q: &[f64], glyphs: &[VecN]) -> f64 {
    if glyphs.is_empty() {
        return 0.0;
    }
    // s* = argmin_s <q - target, s>.
    let mut arg = 0usize;
    let mut argval = f64::INFINITY;
    for (i, g) in glyphs.iter().enumerate() {
        let v: f64 = q
            .iter()
            .zip(target)
            .zip(g)
            .map(|((qi, ti), gi)| (qi - ti) * gi)
            .sum();
        if v < argval {
            argval = v;
            arg = i;
        }
    }
    let s = &glyphs[arg];
    q.iter()
        .zip(target)
        .zip(s)
        .map(|((qi, ti), si)| (qi - ti) * (qi - si))
        .sum()
}

/// Temporal reconstruction error **with the same two-level colour model as
/// [`static_error`]**: minimize over `q` in the glyph convex hull and scalars
/// `b, c` the quantity `|| target - (b + c q) ||`, with the reconstruction
/// clamped to `[0,1]`.
///
/// A single glyph is a hull vertex, so this is at least as expressive as the
/// static fit; it is the fair infinite-`K` temporal counterpart.
pub fn temporal_error(target: &[f64], glyphs: &[VecN], iters: usize) -> f64 {
    let n = target.len();
    if n == 0 {
        return 0.0;
    }
    let nf = n as f64;
    let t1: f64 = target.iter().sum();
    let tmean = t1 / nf;
    let tmin = target.iter().cloned().fold(f64::INFINITY, f64::min);
    let tmax = target.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let starts = [(0.0, 1.0), (tmin, (tmax - tmin).max(1e-6)), (tmean, 1.0)];
    let mut best = f64::INFINITY;
    for (b0, c0) in starts {
        let mut b = b0;
        let mut c = c0;
        for _ in 0..6 {
            if c.abs() < 1e-9 {
                break;
            }
            let adj: Vec<f64> = target.iter().map(|t| (t - b) / c).collect();
            let q = hull_point(&adj, glyphs, iters);
            let q1: f64 = q.iter().sum();
            let qq = dot(&q, &q);
            let tq = dot(target, &q);
            let det = nf * qq - q1 * q1;
            if det.abs() < 1e-12 {
                b = tmean;
                c = 0.0;
            } else {
                c = (nf * tq - q1 * t1) / det;
                b = (qq * t1 - q1 * tq) / det;
            }
        }
        let q = if c.abs() < 1e-9 {
            vec![0.0; n]
        } else {
            let adj: Vec<f64> = target.iter().map(|t| (t - b) / c).collect();
            hull_point(&adj, glyphs, iters)
        };
        let sse: f64 = target
            .iter()
            .zip(&q)
            .map(|(t, qv)| ((b + c * qv).clamp(0.0, 1.0) - t).powi(2))
            .sum();
        best = best.min(sse / nf);
    }
    // A single glyph is a hull vertex, so the static fit is always feasible for
    // the temporal problem. Taking the min makes `temporal <= static` a hard
    // guarantee rather than a hope that the hull_point search found the vertex.
    let stat = static_error(target, glyphs).0;
    best.max(0.0).sqrt().min(stat)
}

/// Static-only reconstruction error for a pure *binary* basis (no colours),
/// i.e. nearest basis vector. Used for the "coverage partition" ablation.
pub fn binary_static_error(target: &[f64], glyphs: &[VecN]) -> f64 {
    let mut best = f64::INFINITY;
    for g in glyphs {
        let d: f64 = target
            .iter()
            .zip(g)
            .map(|(t, x)| (t - x).powi(2))
            .sum::<f64>();
        best = best.min(d);
    }
    (best / target.len() as f64).sqrt()
}

/// Residual of `target` after its best per-dot scaling on the Braille blobs.
/// Because the modelled blobs are disjoint, this is the per-dot least-squares
/// intensity, which is exactly the best static Braille *coverage* fit with free
/// per-dot intensity (the temporal infinite-K limit on the dot lattice).
pub fn braille_residual(target: &[f64], dots: &[VecN]) -> VecN {
    let mut recon = vec![0.0f64; target.len()];
    for d in dots {
        let dd = dot(d, d);
        if dd <= 1e-12 {
            continue;
        }
        let c = (dot(target, d) / dd).clamp(0.0, 1.0);
        add_scaled(&mut recon, d, c);
    }
    sub(target, &recon)
}

/// Mean squared norm of a corpus of vectors.
pub fn mean_energy(corpus: &[VecN]) -> f64 {
    if corpus.is_empty() {
        return 0.0;
    }
    corpus.iter().map(|v| dot(v, v)).sum::<f64>() / corpus.len() as f64
}

/// Orthogonal matching pursuit step: given the current orthonormal basis `q` and
/// the target residuals `res`, return the candidate index maximizing the
/// explained residual energy, plus the fraction of remaining residual energy it
/// explains.
pub fn omp_select(q: &[VecN], res: &[VecN], candidates: &[VecN]) -> (usize, f64) {
    let mut best = (0usize, -1.0f64);
    for (ci, c) in candidates.iter().enumerate() {
        // Candidate direction orthogonal to the current span.
        let mut orth = c.clone();
        let nc = norm(&orth);
        if nc <= 1e-12 {
            continue;
        }
        for u in q {
            let d = dot(&orth, u);
            add_scaled(&mut orth, u, -d);
        }
        let no = norm(&orth);
        if no <= 1e-9 * nc.max(1.0) {
            continue;
        }
        for x in orth.iter_mut() {
            *x /= no;
        }
        // Explained energy of the residuals along this new direction.
        let mut gain = 0.0f64;
        for r in res {
            let d = dot(r, &orth);
            gain += d * d;
        }
        if gain > best.1 {
            best = (ci, gain);
        }
    }
    best
}

/// Projects a corpus onto `q` and returns the residual vectors.
pub fn residuals(corpus: &[VecN], q: &[VecN]) -> Vec<VecN> {
    corpus
        .iter()
        .map(|t| {
            let mut w = t.clone();
            for u in q {
                let d = dot(t, u);
                add_scaled(&mut w, u, -d);
            }
            w
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_of_independent_vectors_is_exact() {
        let basis = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![1.0, 1.0, 0.0],
        ];
        let (_, rank) = orthonormalize(&basis);
        assert_eq!(rank, 2);
    }

    #[test]
    fn principal_angle_of_in_span_vector_is_zero() {
        let q = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        assert!(principal_angle_deg(&[0.5, 0.5], &q) < 1e-6);
        assert!((principal_angle_deg(&[1.0, 1.0], &q)).abs() < 1e-6);
        assert!(principal_angle_deg(&[1.0, 0.0], &vec![vec![0.0, 1.0]]) > 89.0);
    }

    #[test]
    fn gram_eigenvalues_match_known_spectrum() {
        let basis = vec![vec![1.0, 0.0], vec![0.0, 2.0]];
        let eigs = gram_eigenvalues(&basis);
        assert!((eigs[0] - 4.0).abs() < 1e-9);
        assert!((eigs[1] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn effective_rank_counts_energy() {
        let eigs = vec![9.0, 0.5, 0.3, 0.2];
        assert_eq!(effective_rank(&eigs, 0.9), 1);
        assert_eq!(effective_rank(&eigs, 0.999), 4);
    }

    #[test]
    fn static_fit_recovers_a_scaled_glyph_exactly() {
        // Target is 0.3 + 0.6*g -> static fit should be near zero error.
        let g = vec![1.0, 0.0, 1.0, 0.0];
        let target: Vec<f64> = g.iter().map(|v| 0.3 + 0.6 * v).collect();
        let (rmse, _, b, a) = static_error(&target, std::slice::from_ref(&g));
        assert!(rmse < 1e-9, "rmse={rmse} b={b} a={a}");
        assert!((b - 0.3).abs() < 1e-9 && (a - 0.6).abs() < 1e-9);
    }

    #[test]
    fn temporal_error_never_exceeds_static_error() {
        // A single glyph is feasible for the temporal fit, so the guarantee must
        // hold on every input, including ones the FW search finds no better.
        let mut z = 0x1234_5678_9abc_def0u64;
        let mut rnd = || {
            z = z.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((z >> 40) as f64) / ((1u64 << 24) as f64)
        };
        for _ in 0..40 {
            let p = 12;
            let glyphs: Vec<Vec<f64>> = (0..5).map(|_| (0..p).map(|_| rnd()).collect()).collect();
            let target: Vec<f64> = (0..p).map(|_| rnd()).collect();
            let stat = static_error(&target, &glyphs).0;
            let temp = temporal_error(&target, &glyphs, 32);
            assert!(temp <= stat + 1e-9, "temp {temp} > stat {stat}");
        }
    }

    #[test]
    fn hull_gap_certifies_convergence() {
        let glyphs = vec![
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0],
            vec![0.0, 0.0, 1.0, 0.0],
            vec![0.0, 0.0, 0.0, 1.0],
        ];
        // Reachable target: the duality gap at the found point is zero.
        let target = vec![0.6, 0.4, 0.0, 0.0];
        let q = hull_point(&target, &glyphs, 64);
        assert!(hull_gap(&target, &q, &glyphs) < 1e-9);
        // Unreachable direction: the gap is nonnegative.
        let t2 = vec![0.2, 0.2, 0.2, 0.9];
        let q2 = hull_point(&t2, &glyphs, 8);
        assert!(hull_gap(&t2, &q2, &glyphs) >= -1e-12);
    }

    #[test]
    fn temporal_error_beats_static_on_a_non_trivial_mixture() {
        // Unit glyphs: their convex hull is the probability simplex. A target
        // inside the simplex but off every single-glyph two-level pattern is
        // exactly representable temporally and not statically.
        let glyphs = vec![
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0],
            vec![0.0, 0.0, 1.0, 0.0],
            vec![0.0, 0.0, 0.0, 1.0],
        ];
        let target = vec![0.6, 0.4, 0.0, 0.0];
        let stat = static_error(&target, &glyphs).0;
        let temporal = temporal_error(&target, &glyphs, 64);
        assert!(
            temporal < 1e-4,
            "temporal should reach the mixture: {temporal}"
        );
        assert!(temporal < stat, "temporal={temporal} static={stat}");
    }

    #[test]
    fn braille_residual_is_zero_on_a_dot_union() {
        let dots = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let target = vec![1.0, 1.0]; // union of both dots
        let r = braille_residual(&target, &dots);
        assert!(norm(&r) < 1e-9, "residual {r:?}");
    }

    #[test]
    fn braille_residual_is_isolated_to_the_gap() {
        let dots = vec![vec![1.0, 0.0, 0.0], vec![0.0, 0.0, 1.0]];
        let target = vec![0.0, 1.0, 0.0]; // a gap between the dots
        let r = braille_residual(&target, &dots);
        assert!((norm(&r) - 1.0).abs() < 1e-9, "residual {r:?}");
    }

    #[test]
    fn omp_picks_the_direction_with_the_largest_residual_alignment() {
        let res = vec![vec![10.0, 0.0]];
        let cands = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let (idx, gain) = omp_select(&[], &res, &cands);
        assert_eq!(idx, 0);
        assert!(gain > 99.0);
    }
}
