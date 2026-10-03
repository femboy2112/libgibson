//! Candidate temporal schedules.
//!
//! Each schedule turns one cell's eight duty targets into `K` binary Braille
//! masks. A schedule is **not** a renderer: the masks are realized as ordinary
//! `Surface` cells by the caller, exactly as `temporal::TemporalBrailleField`
//! does. These are pure deterministic functions of `(kind, duty, static_mask,
//! K, seed, cell)` so a schedule and a seed fully determine the output.
//!
//! The library-backed schedules (`Library*`) are not here; they are obtained by
//! advancing a real `TemporalBrailleField` so the experiment measures the
//! shipping algorithm, not a reimplementation.

/// Phase counts the experiment sweeps. `K` phases give `K+1` representable
/// duty levels per dot (including the residual).
pub const K_CHOICES: [usize; 7] = [2, 3, 4, 5, 6, 7, 8];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleKind {
    /// All dots share one phase; toggles in lockstep. The naive baseline.
    NaiveAligned,
    /// First-order error feedback with a per-dot deterministic start phase,
    /// reset every window. The frame-local analogue of the shipping
    /// sigma-delta.
    WindowedErrorFeedback,
    /// Like `WindowedErrorFeedback` but operating on the residual over the
    /// stored static mask, so most phases are exact static (low wire churn).
    ResidualWindowedEF,
    /// Independent per-phase Bernoulli rounding from a deterministic hash.
    /// White temporal error; zero expected DC but per-window variance.
    StochasticRound,
    /// Balanced low-discrepancy phase sets (van der Corput order, per-dot
    /// rotation) with deterministic count rounding.
    VdcBalanced,
}

impl ScheduleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ScheduleKind::NaiveAligned => "naive-aligned",
            ScheduleKind::WindowedErrorFeedback => "windowed-ef",
            ScheduleKind::ResidualWindowedEF => "residual-windowed-ef",
            ScheduleKind::StochasticRound => "stochastic-round",
            ScheduleKind::VdcBalanced => "vdc-balanced",
        }
    }
}

/// SplitMix64 mapped to `[0,1)`.
fn hash01(seed: u64, a: u64, b: u64, c: u64) -> f32 {
    let mut z = seed
        ^ a.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ b.wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ c.wrapping_mul(0x94D0_49BB_1331_11EB);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    ((z >> 40) as f32) / ((1u32 << 24) as f32)
}

/// Radical inverse (van der Corput) in base 2.
fn vdc2(mut n: u32) -> f64 {
    let mut r = 0.0f64;
    let mut f = 0.5f64;
    while n > 0 {
        r += f * (n & 1) as f64;
        n >>= 1;
        f *= 0.5;
    }
    r
}

fn residual_sequence(d: f32, baseline_on: bool, k: usize, phi: f32, threshold: f32) -> u8 {
    let mut acc = phi;
    let residual = d - if baseline_on { 1.0 } else { 0.0 };
    let mut mask = 0u8;
    for t in 0..k {
        acc += residual;
        let on = if baseline_on {
            if acc <= -threshold {
                acc += 1.0;
                false
            } else {
                true
            }
        } else if acc >= threshold {
            acc -= 1.0;
            true
        } else {
            false
        };
        if on {
            mask |= 1 << t;
        }
    }
    mask
}

#[inline]
fn bit(mask: u8, t: usize) -> bool {
    mask & (1u8 << t) != 0
}

/// Builds `K` masks for one cell under `kind`.
///
/// Returns a `Vec<u8>` of length `K`; element `t` is the mask emitted at phase
/// `t`. `static_mask` is the authoritative static fallback mask (may differ
/// from `duty` thresholded at 0.5 at exact ties).
pub fn pure_masks(
    kind: ScheduleKind,
    duty: &[f32; 8],
    static_mask: u8,
    k: usize,
    seed: u64,
    cell: u64,
) -> Vec<u8> {
    let k = k.max(1);
    match kind {
        ScheduleKind::NaiveAligned => {
            let mut out = vec![0u8; k];
            for t in 0..k {
                let level = (t as f32 + 0.5) / k as f32;
                let mut m = 0u8;
                for (i, d) in duty.iter().enumerate() {
                    if *d >= level {
                        m |= 1 << i;
                    }
                }
                out[t] = m;
            }
            out
        }
        ScheduleKind::WindowedErrorFeedback => {
            let mut acc = [0f32; 8];
            for i in 0..8 {
                acc[i] = hash01(seed, cell, i as u64, 0xE1);
            }
            let mut out = vec![0u8; k];
            for t in 0..k {
                let mut m = 0u8;
                for i in 0..8 {
                    acc[i] += duty[i];
                    if acc[i] >= 1.0 {
                        acc[i] -= 1.0;
                        m |= 1 << i;
                    }
                }
                out[t] = m;
            }
            out
        }
        ScheduleKind::ResidualWindowedEF => {
            // Per-dot phase-offset residual error feedback, the frame-local form
            // of the library's `advance_residual_masks`.
            let seqs: Vec<u8> = (0..8)
                .map(|i| {
                    let phi = hash01(seed, cell, i as u64, 0xF00D);
                    residual_sequence(duty[i], bit(static_mask, i), k, phi, 1.0)
                })
                .collect();
            (0..k)
                .map(|t| {
                    let mut m = 0u8;
                    for (i, s) in seqs.iter().enumerate() {
                        if bit(*s, t) {
                            m |= 1 << i;
                        }
                    }
                    m
                })
                .collect()
        }
        ScheduleKind::StochasticRound => {
            let mut out = vec![0u8; k];
            for t in 0..k {
                let mut m = 0u8;
                for i in 0..8 {
                    if hash01(seed, cell, (i as u64) * 4096 + t as u64, 0xABCD) < duty[i] {
                        m |= 1 << i;
                    }
                }
                out[t] = m;
            }
            out
        }
        ScheduleKind::VdcBalanced => {
            // Order phases by a per-dot rotated van der Corput key, then fire the
            // n_i earliest, where n_i is the deterministic (phase-jittered) rounded
            // duty*K.
            let mut seqs = vec![[false; 8]; k];
            for i in 0..8 {
                let phi = hash01(seed, cell, i as u64, 0x1234) as f64;
                let mut keys: Vec<(f64, usize)> = (0..k)
                    .map(|t| ((phi + vdc2(t as u32 + 1)).fract(), t))
                    .collect();
                keys.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                let n = (duty[i] * k as f32 + hash01(seed, cell, i as u64, 0x5678)).floor();
                let n = (n.max(0.0) as usize).min(k);
                for &(_, t) in keys.iter().take(n) {
                    seqs[t][i] = true;
                }
            }
            (0..k)
                .map(|t| {
                    let mut m = 0u8;
                    for i in 0..8 {
                        if seqs[t][i] {
                            m |= 1 << i;
                        }
                    }
                    m
                })
                .collect()
        }
    }
}

/// Number of dots that differ between two masks.
#[inline]
pub fn mask_hamming(a: u8, b: u8) -> u32 {
    (a ^ b).count_ones()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn duties() -> [f32; 8] {
        [0.0, 0.2, 0.5, 0.75, 1.0, 0.33, 0.66, 0.9]
    }

    #[test]
    fn every_schedule_is_deterministic_and_well_formed() {
        let kinds = [
            ScheduleKind::NaiveAligned,
            ScheduleKind::WindowedErrorFeedback,
            ScheduleKind::ResidualWindowedEF,
            ScheduleKind::StochasticRound,
            ScheduleKind::VdcBalanced,
        ];
        for kind in kinds {
            for k in K_CHOICES {
                let a = pure_masks(kind, &duties(), 0b1010_1010, k, 42, 9);
                let b = pure_masks(kind, &duties(), 0b1010_1010, k, 42, 9);
                assert_eq!(a, b, "{kind:?} k={k} not deterministic");
                assert_eq!(a.len(), k);
            }
        }
    }

    #[test]
    fn zero_and_full_duty_are_exact_for_error_feedback() {
        let mut d = [0.5f32; 8];
        d[0] = 0.0;
        d[1] = 1.0;
        for kind in [
            ScheduleKind::WindowedErrorFeedback,
            ScheduleKind::ResidualWindowedEF,
        ] {
            for k in K_CHOICES {
                let masks = pure_masks(kind, &d, 0, k, 1, 1);
                for m in &masks {
                    assert!(!bit(*m, 0), "{kind:?} k={k}: zero duty lit");
                    assert!(bit(*m, 1), "{kind:?} k={k}: full duty unlit");
                }
            }
        }
    }

    #[test]
    fn windowed_error_feedback_mean_tracks_duty() {
        // Over a full K-window the per-dot mean must be within one level (1/K)
        // of the target duty.
        for k in K_CHOICES {
            let d = duties();
            let masks = pure_masks(ScheduleKind::WindowedErrorFeedback, &d, 0, k, 7, 3);
            for i in 0..8 {
                let lit = masks.iter().filter(|m| bit(**m, i)).count() as f32 / k as f32;
                assert!(
                    (lit - d[i]).abs() <= 1.0 / k as f32 + 1e-6,
                    "k={k} dot={i} duty={} lit={lit}",
                    d[i]
                );
            }
        }
    }

    #[test]
    fn naive_aligned_is_coherent_and_vdc_is_decorrelated() {
        // A uniform 0.5 field at K=4: aligned toggles all dots together (phases of
        // 8 or 0 dots), while the balanced schedule never flashes the whole cell
        // to all-on or all-off.
        let d = [0.5f32; 8];
        let aligned = pure_masks(ScheduleKind::NaiveAligned, &d, 0, 4, 1, 1);
        let popcounts: Vec<u32> = aligned.iter().map(|m| m.count_ones()).collect();
        assert_eq!(popcounts, vec![8, 8, 0, 0]);

        let vdc = pure_masks(ScheduleKind::VdcBalanced, &d, 0, 4, 1, 1);
        for m in &vdc {
            let n = m.count_ones();
            assert!(n != 0 && n != 8, "vdc flashed the whole cell: popcount {n}");
        }
        for i in 0..8 {
            let lit = vdc.iter().filter(|m| bit(**m, i)).count();
            assert_eq!(lit, 2, "dot {i} lit {lit} times");
        }
    }

    #[test]
    fn stochastic_round_has_near_zero_bias_over_many_windows() {
        // Average over many independent windows: expectation equals duty. Uses
        // cell index as the fresh randomiser per window.
        let d = [0.15f32, 0.35, 0.5, 0.65, 0.85, 0.25, 0.75, 0.45];
        let k = 4;
        let mut sum = [0f32; 8];
        let windows = 4000u64;
        for c in 0..windows {
            let masks = pure_masks(ScheduleKind::StochasticRound, &d, 0, k, 99, c);
            for m in &masks {
                for i in 0..8 {
                    if bit(*m, i) {
                        sum[i] += 1.0;
                    }
                }
            }
        }
        for i in 0..8 {
            let mean = sum[i] / (windows * k as u64) as f32;
            assert!((mean - d[i]).abs() < 0.02, "dot {i}: {mean} vs {}", d[i]);
        }
    }
}
