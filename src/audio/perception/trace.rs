//! The **`AuditoryTrace`** — the MVP's multi-witness summary, and an inspectable
//! per-witness **distance** between two traces.
//!
//! The design note's hard rule: the witnesses stay **distinct**. There is deliberately no
//! `human_quality: 0.87` scalar anywhere in this type. A trace carries five separate kinds
//! of evidence, each with its own units and its own distance, because "how two renders
//! differ" is a *vector*, not a number:
//!
//! | witness      | what it is                                   | lives in            |
//! | ------------ | -------------------------------------------- | ------------------- |
//! | `physical`   | RMS / peak (and modeled dB SPL *iff* calibrated) | pre-auditory, physical |
//! | `excitation` | per-band energy → **spectral shape**         | [`ErbBank`] channels |
//! | `envelope`   | per-band envelope **modulation depth**       | slow amplitude      |
//! | `tfs`        | per-band mean zero-crossing rate             | temporal fine structure |
//! | `onset`      | half-wave spectral-flux series               | transient salience  |
//!
//! Level is kept strictly in `physical`: `excitation` is compared as a *scale-invariant
//! shape*, so a pure gain change moves `physical` and leaves `excitation` alone. That
//! factoring is the whole point — it lets the ablation device (see [`super::ablation`]) say
//! *"energy barely moved but the auditory scene changed a lot"*, or the reverse.
//!
//! Everything here is pure and deterministic: one [`ErbBank::filter_band`] pass per channel,
//! no RNG, no global state. The onset series is accumulated in that same pass and is
//! bit-identical to [`super::onset::onset_salience`] on the same input.

use super::filterbank::ErbBank;
use super::listener::{LevelCalibration, ListeningContext};
use super::temporal::{envelope_rms, frame_count};

/// Pre-auditory, physical magnitude of the analyzed signal. Absolute level is **only**
/// defined under [`LevelCalibration::Calibrated`]; under `Relative` it is `None` (the
/// observer never fabricates an SPL from dimensionless PCM).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Physical {
    /// Root-mean-square amplitude of the analyzed mono signal (dimensionless, full scale 1.0).
    pub rms: f32,
    /// Peak absolute amplitude of the analyzed mono signal.
    pub peak: f32,
    /// Modeled sound-pressure level (dB SPL) under the declared calibration, RMS-referenced
    /// to full scale. `None` under [`LevelCalibration::Relative`] or for a silent signal.
    pub abs_db_spl: Option<f32>,
}

/// The multi-witness auditory summary of one signal under one [`ListeningContext`].
#[derive(Debug, Clone, PartialEq)]
pub struct AuditoryTrace {
    /// Physical level witness (never auditory).
    pub physical: Physical,
    /// Per-band energy — the spectral-shape witness (channel order = ascending centre freq).
    pub excitation: Vec<f32>,
    /// Per-band envelope **modulation index** `std(env)/mean(env)` — how much each band's
    /// loudness fluctuates over time (0 = perfectly steady).
    pub envelope: Vec<f32>,
    /// Per-band mean zero-crossing rate in `[0,1]` — the temporal-fine-structure witness.
    pub tfs: Vec<f32>,
    /// Per-frame half-wave spectral flux — the onset/transient-salience witness.
    pub onset: Vec<f32>,
    /// Number of ERB channels.
    pub n_bands: usize,
    /// Number of analysis frames.
    pub n_frames: usize,
}

impl AuditoryTrace {
    /// Observe `mono` under `ctx` through `bank`, with window `frame` / step `hop`.
    ///
    /// `bank` must have been built for `ctx.sample_rate_hz`; the trace cannot check this
    /// (the bank keeps its rate private) so it is a declared caller contract.
    pub fn observe(
        ctx: &ListeningContext,
        bank: &ErbBank,
        mono: &[f32],
        frame: usize,
        hop: usize,
    ) -> AuditoryTrace {
        let nb = bank.len();
        let nf = frame_count(mono.len(), frame, hop);

        // ---- physical (pre-auditory) ----
        let (ss, peak) = mono.iter().fold((0.0f64, 0.0f32), |(s, p), &v| {
            (s + (v as f64) * (v as f64), p.max(v.abs()))
        });
        let rms = if mono.is_empty() {
            0.0
        } else {
            (ss / mono.len() as f64).sqrt() as f32
        };
        let abs_db_spl = match ctx.level {
            LevelCalibration::Calibrated { db_spl_at_dbfs } if rms > 0.0 => {
                Some(db_spl_at_dbfs + 20.0 * rms.log10())
            }
            // Relative, or a silent signal: undefined rather than fabricated.
            _ => None,
        };

        // ---- per-band witnesses + onset, in a single filter pass per channel ----
        let mut excitation = vec![0.0f32; nb];
        let mut envelope = vec![0.0f32; nb];
        let mut tfs = vec![0.0f32; nb];
        let mut onset = vec![0.0f32; nf];
        for b in 0..nb {
            let y = bank.filter_band(b, mono);
            // excitation: total band energy (spectral shape; level factored out at compare time)
            excitation[b] = y.iter().map(|&v| v * v).sum();
            // per-band framed envelope (reused for both the modulation index and the onset flux)
            let env = envelope_rms(&y, frame, hop);
            // envelope modulation index: std/mean (coefficient of variation), 0 if silent band
            let (em, ev) = mean_var(&env);
            envelope[b] = if em > 1e-9 { ev.sqrt() / em } else { 0.0 };
            // onset: accumulate half-wave flux of this band's envelope (== onset_salience def)
            for t in 1..env.len().min(nf) {
                let rise = env[t] - env[t - 1];
                if rise > 0.0 {
                    onset[t] += rise;
                }
            }
            // tfs: mean zero-crossing rate across the band signal (frame-free, whole-band)
            tfs[b] = zcr(&y);
        }

        AuditoryTrace {
            physical: Physical {
                rms,
                peak,
                abs_db_spl,
            },
            excitation,
            envelope,
            tfs,
            onset,
            n_bands: nb,
            n_frames: nf,
        }
    }

    /// The per-witness distance from `self` to `other` (see [`TraceDistance`]).
    pub fn distance(&self, other: &AuditoryTrace) -> TraceDistance {
        TraceDistance {
            physical: rel(self.physical.rms, other.physical.rms),
            excitation: cosine_shape(&self.excitation, &other.excitation),
            envelope: norm_l2(&self.envelope, &other.envelope),
            tfs: norm_l2(&self.tfs, &other.tfs),
            onset: norm_l2(&self.onset, &other.onset),
        }
    }
}

/// An inspectable, **per-witness** distance between two [`AuditoryTrace`]s. Every field is a
/// separate change on `~[0,1]`; there is intentionally no combined "quality" or "similarity"
/// scalar. Removing highs vs smearing transients vs swapping in stationary noise light up
/// *different* fields — that separation is the diagnostic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraceDistance {
    /// Relative RMS-level change `|a-b|/(a+b)` — the physical witness.
    pub physical: f32,
    /// Spectral-**shape** change (scale-invariant cosine distance over band energies), `[0,1]`.
    pub excitation: f32,
    /// Envelope-modulation change (normalized L2 over per-band modulation indices), `[0,1]`.
    pub envelope: f32,
    /// Fine-structure change (normalized L2 over per-band ZCR), `[0,1]`.
    pub tfs: f32,
    /// Transient-salience change (normalized L2 over the onset-flux series), `[0,1]`.
    pub onset: f32,
}

impl TraceDistance {
    /// The five distances, each labeled — for receipts and tables.
    pub fn witnesses(&self) -> [(&'static str, f32); 5] {
        [
            ("physical", self.physical),
            ("excitation", self.excitation),
            ("envelope", self.envelope),
            ("tfs", self.tfs),
            ("onset", self.onset),
        ]
    }

    /// The single largest witness distance — a **ranking** convenience only. This is *not* a
    /// quality, importance, or "how bad" score: it says which witness moved most, nothing about
    /// whether the change is good or bad. The ear decides that.
    pub fn max_witness(&self) -> (&'static str, f32) {
        self.witnesses()
            .into_iter()
            .fold(("physical", 0.0), |m, w| if w.1 > m.1 { w } else { m })
    }
}

fn mean_var(v: &[f32]) -> (f32, f32) {
    if v.is_empty() {
        return (0.0, 0.0);
    }
    let n = v.len() as f64;
    let mean = v.iter().map(|&x| x as f64).sum::<f64>() / n;
    let var = v.iter().map(|&x| (x as f64 - mean).powi(2)).sum::<f64>() / n;
    (mean as f32, var as f32)
}

fn zcr(signal: &[f32]) -> f32 {
    if signal.len() < 2 {
        return 0.0;
    }
    let crossings = signal
        .windows(2)
        .filter(|p| (p[0] >= 0.0) != (p[1] >= 0.0))
        .count();
    crossings as f32 / (signal.len() - 1) as f32
}

/// Relative difference `|a-b| / (|a|+|b|)` in `[0,1]` (0 when both are 0).
fn rel(a: f32, b: f32) -> f32 {
    let s = a.abs() + b.abs();
    if s <= 1e-12 {
        0.0
    } else {
        (a - b).abs() / s
    }
}

/// Normalized L2 distance `||a-b|| / (||a||+||b||)` in `[0,1]` (0 when both are ~0).
fn norm_l2(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    let (mut num, mut na, mut nb) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        let (x, y) = (a[i] as f64, b[i] as f64);
        num += (x - y) * (x - y);
        na += x * x;
        nb += y * y;
    }
    let den = na.sqrt() + nb.sqrt();
    if den <= 1e-12 {
        0.0
    } else {
        (num.sqrt() / den) as f32
    }
}

/// Scale-invariant **shape** distance `(1 - cos θ)/2` in `[0,1]` between two vectors (0 when
/// identical in direction, i.e. identical up to a positive scale). Used for spectral shape so
/// overall level stays in the physical witness.
fn cosine_shape(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    let (mut dot, mut na, mut nb) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..n {
        let (x, y) = (a[i] as f64, b[i] as f64);
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    let den = na.sqrt() * nb.sqrt();
    if den <= 1e-12 {
        0.0
    } else {
        let cos = (dot / den).clamp(-1.0, 1.0);
        ((1.0 - cos) * 0.5) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;
    const FRAME: usize = 1024;
    const HOP: usize = 512;

    fn sine(freq: f64, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * freq * i as f64 / SR as f64).sin() as f32)
            .collect()
    }

    #[test]
    fn relative_context_has_no_absolute_spl() {
        let ctx = ListeningContext::relative(SR);
        let bank = ErbBank::default_for(SR);
        let t = AuditoryTrace::observe(&ctx, &bank, &sine(1000.0, 24_000), FRAME, HOP);
        assert!(
            t.physical.abs_db_spl.is_none(),
            "Relative must not emit SPL"
        );
        assert!(t.physical.rms > 0.0);
    }

    #[test]
    fn calibrated_context_emits_modeled_spl() {
        let ctx = ListeningContext {
            level: LevelCalibration::Calibrated {
                db_spl_at_dbfs: 100.0,
            },
            ..ListeningContext::relative(SR)
        };
        let bank = ErbBank::default_for(SR);
        let t = AuditoryTrace::observe(&ctx, &bank, &sine(1000.0, 24_000), FRAME, HOP);
        let spl = t.physical.abs_db_spl.expect("Calibrated should emit SPL");
        // a ~0.707 RMS sine is ~3 dB below full scale → just under 100 dB SPL
        assert!(spl < 100.0 && spl > 90.0, "modeled SPL {spl} implausible");
    }

    #[test]
    fn silent_calibrated_signal_is_still_undefined() {
        let ctx = ListeningContext {
            level: LevelCalibration::Calibrated {
                db_spl_at_dbfs: 90.0,
            },
            ..ListeningContext::relative(SR)
        };
        let bank = ErbBank::default_for(SR);
        let t = AuditoryTrace::observe(&ctx, &bank, &vec![0.0f32; 8192], FRAME, HOP);
        assert!(
            t.physical.abs_db_spl.is_none(),
            "silence has no defined SPL even when calibrated"
        );
    }

    #[test]
    fn identical_signals_are_zero_distance_everywhere() {
        let ctx = ListeningContext::relative(SR);
        let bank = ErbBank::default_for(SR);
        let x = sine(880.0, 24_000);
        let a = AuditoryTrace::observe(&ctx, &bank, &x, FRAME, HOP);
        let b = AuditoryTrace::observe(&ctx, &bank, &x, FRAME, HOP);
        let d = a.distance(&b);
        for (name, v) in d.witnesses() {
            assert!(
                v < 1e-6,
                "witness {name} should be ~0 for identical input, got {v}"
            );
        }
    }

    #[test]
    fn pure_gain_moves_only_physical_not_excitation_shape() {
        let ctx = ListeningContext::relative(SR);
        let bank = ErbBank::default_for(SR);
        let x = sine(1000.0, 24_000);
        let louder: Vec<f32> = x.iter().map(|&v| v * 0.5).collect();
        let a = AuditoryTrace::observe(&ctx, &bank, &x, FRAME, HOP);
        let b = AuditoryTrace::observe(&ctx, &bank, &louder, FRAME, HOP);
        let d = a.distance(&b);
        assert!(
            d.physical > 0.2,
            "halving level must move physical, got {}",
            d.physical
        );
        assert!(
            d.excitation < 1e-3,
            "a pure gain change must NOT move spectral shape, got {}",
            d.excitation
        );
    }

    #[test]
    fn different_pitch_moves_excitation_and_tfs() {
        let ctx = ListeningContext::relative(SR);
        let bank = ErbBank::default_for(SR);
        let lo = AuditoryTrace::observe(&ctx, &bank, &sine(300.0, 24_000), FRAME, HOP);
        let hi = AuditoryTrace::observe(&ctx, &bank, &sine(3000.0, 24_000), FRAME, HOP);
        let d = lo.distance(&hi);
        assert!(
            d.excitation > 0.2,
            "pitch change must move spectral shape, got {}",
            d.excitation
        );
        assert!(
            d.tfs > 0.01,
            "pitch change should move fine structure, got {}",
            d.tfs
        );
    }

    #[test]
    fn deterministic() {
        let ctx = ListeningContext::relative(SR);
        let bank = ErbBank::default_for(SR);
        let x = sine(523.0, 12_000);
        assert_eq!(
            AuditoryTrace::observe(&ctx, &bank, &x, FRAME, HOP),
            AuditoryTrace::observe(&ctx, &bank, &x, FRAME, HOP)
        );
    }
}
