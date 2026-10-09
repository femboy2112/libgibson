//! **Perceptual ablation** — `observer(x)` vs `observer(x − component)`.
//!
//! Given a signal and a named component of it, this asks two *separate* questions and
//! refuses to merge them:
//!
//! 1. **Physical:** what fraction of the signal's energy was the component? (`physical_energy_share`)
//! 2. **Auditory:** how much did the modeled auditory scene change when it was removed?
//!    (`observer_distance`, the per-witness [`TraceDistance`])
//!
//! Keeping these apart is the entire diagnostic value. A component can be a large share of the
//! energy yet barely move the auditory scene (e.g. sub-audible rumble), or a tiny share yet
//! dominate it (e.g. a cymbal transient). Reporting a single blended "importance" would erase
//! exactly the thing worth seeing.
//!
//! **This is not an importance or quality claim.** It is a ratio and a vector under a declared
//! [`ListeningContext`] and a *declared* decomposition `x = component + residual`. The choice
//! of what counts as "the component" is the caller's modeling decision, made explicit.

use super::filterbank::ErbBank;
use super::listener::ListeningContext;
use super::trace::{AuditoryTrace, TraceDistance};

/// The result of one ablation: a physical energy ratio and an auditory-scene distance, kept
/// strictly separate (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ablation {
    /// `energy(component) / energy(full)` — a physical ratio in `[0, ∞)` (usually `[0,1]`,
    /// but can exceed 1 if the declared component is anti-correlated with the rest).
    pub physical_energy_share: f32,
    /// `observer(full).distance(observer(full − component))` — the per-witness auditory change.
    pub observer_distance: TraceDistance,
}

/// Ablate `component` from `full` and report the physical share and auditory distance
/// separately. `full` and `component` are aligned sample-for-sample; the residual is their
/// elementwise difference over the shorter length. `bank` must match `ctx.sample_rate_hz`.
pub fn ablate(
    ctx: &ListeningContext,
    bank: &ErbBank,
    full: &[f32],
    component: &[f32],
    frame: usize,
    hop: usize,
) -> Ablation {
    let n = full.len().min(component.len());
    let residual: Vec<f32> = (0..n).map(|i| full[i] - component[i]).collect();

    let e_full: f64 = full[..n].iter().map(|&v| (v as f64) * (v as f64)).sum();
    let e_comp: f64 = component[..n]
        .iter()
        .map(|&v| (v as f64) * (v as f64))
        .sum();
    let physical_energy_share = if e_full <= 1e-12 {
        0.0
    } else {
        (e_comp / e_full) as f32
    };

    let t_full = AuditoryTrace::observe(ctx, bank, &full[..n], frame, hop);
    let t_res = AuditoryTrace::observe(ctx, bank, &residual, frame, hop);

    Ablation {
        physical_energy_share,
        observer_distance: t_full.distance(&t_res),
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
    fn removing_nothing_changes_nothing() {
        let ctx = ListeningContext::relative(SR);
        let bank = ErbBank::default_for(SR);
        let x = sine(1000.0, 24_000);
        let zero = vec![0.0f32; x.len()];
        let ab = ablate(&ctx, &bank, &x, &zero, FRAME, HOP);
        assert!(ab.physical_energy_share < 1e-6);
        for (name, v) in ab.observer_distance.witnesses() {
            assert!(v < 1e-6, "removing silence moved {name} by {v}");
        }
    }

    #[test]
    fn removing_everything_is_full_energy_share() {
        let ctx = ListeningContext::relative(SR);
        let bank = ErbBank::default_for(SR);
        let x = sine(1000.0, 24_000);
        let ab = ablate(&ctx, &bank, &x, &x, FRAME, HOP);
        assert!(
            (ab.physical_energy_share - 1.0).abs() < 1e-4,
            "removing the whole signal should be share ~1.0, got {}",
            ab.physical_energy_share
        );
    }

    #[test]
    fn energy_share_matches_a_known_mixture() {
        // full = strong 300 Hz + weak 3 kHz; ablate the 3 kHz part. Its energy share should
        // match the analytic ratio of the summed powers within a loose tolerance.
        let ctx = ListeningContext::relative(SR);
        let bank = ErbBank::default_for(SR);
        let n = 24_000;
        let lo = sine(300.0, n);
        let hi: Vec<f32> = sine(3000.0, n).iter().map(|&v| v * 0.25).collect();
        let full: Vec<f32> = (0..n).map(|i| lo[i] + hi[i]).collect();
        let e_hi: f64 = hi.iter().map(|&v| (v as f64) * (v as f64)).sum();
        let e_full: f64 = full.iter().map(|&v| (v as f64) * (v as f64)).sum();
        let expect = (e_hi / e_full) as f32;
        let ab = ablate(&ctx, &bank, &full, &hi, FRAME, HOP);
        assert!(
            (ab.physical_energy_share - expect).abs() < 1e-3,
            "share {} vs expected {expect}",
            ab.physical_energy_share
        );
        // and removing it must move the auditory scene at least a little
        assert!(ab.observer_distance.max_witness().1 > 1e-3);
    }

    #[test]
    fn deterministic() {
        let ctx = ListeningContext::relative(SR);
        let bank = ErbBank::default_for(SR);
        let x = sine(1000.0, 12_000);
        let c: Vec<f32> = sine(5000.0, 12_000).iter().map(|&v| v * 0.3).collect();
        assert_eq!(
            ablate(&ctx, &bank, &x, &c, FRAME, HOP),
            ablate(&ctx, &bank, &x, &c, FRAME, HOP)
        );
    }
}
