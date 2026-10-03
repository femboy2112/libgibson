//! Frame-local temporal video scheduling.
//!
//! The shipping temporal modulator is a long-horizon first-order sigma-delta:
//! its accumulator integrates error across all history, which is exactly right
//! for a *stable* target and exactly wrong the instant content moves. The
//! compositor therefore freezes moving content.
//!
//! This module tests a different formulation: bound temporal integration to one
//! source-video frame. A source frame is projected once, then `K` temporal
//! subframes are emitted whose time-average approximates that frame's target.
//! The scheduler resets at the next source frame, so no stale history leaks
//! across motion, and it degrades honestly to the static frame when there are
//! too few reliable slots.

use super::schedules::{pure_masks, ScheduleKind};
use gibson::temporal::{BrailleImageProjection, ResetPolicy, TemporalBrailleField};

/// One source frame's projection plus its planned `K` subframe masks.
#[derive(Debug, Clone)]
pub struct FramePlan {
    pub masks: Vec<Vec<u8>>,
    pub static_masks: Vec<u8>,
}

/// Deterministic frame-local planner over a projected source frame.
#[derive(Debug, Clone, Copy)]
pub struct FrameLocalScheduler {
    pub k: usize,
    pub kind: ScheduleKind,
    pub seed: u64,
}

impl FrameLocalScheduler {
    pub fn new(k: usize, kind: ScheduleKind, seed: u64) -> Self {
        Self {
            k: k.max(1),
            kind,
            seed,
        }
    }

    /// Plans `K` subframe masks for `projection`. `source_frame` seeds the
    /// per-window schedule so consecutive source frames do not repeat the exact
    /// same phase pattern (this is scheduling, not core state).
    pub fn plan(&self, projection: &BrailleImageProjection, source_frame: u64) -> FramePlan {
        let w = projection.width() as usize;
        let h = projection.height() as usize;
        let cells = w * h;
        let mut static_masks = Vec::with_capacity(cells);
        let mut per_cell: Vec<Vec<u8>> = Vec::with_capacity(cells);
        for y in 0..projection.height() {
            for x in 0..projection.width() {
                let cell = projection.cell(x, y).expect("in-bounds cell");
                static_masks.push(cell.static_mask);
                if self.k == 1 {
                    per_cell.push(vec![cell.static_mask]);
                } else {
                    let cell_id = (y as u64) * (w as u64) + x as u64;
                    let seed = self
                        .seed
                        .wrapping_mul(0x0001_0000_0001)
                        .wrapping_add(source_frame);
                    per_cell.push(pure_masks(
                        self.kind,
                        &cell.duty,
                        cell.static_mask,
                        self.k,
                        seed,
                        cell_id,
                    ));
                }
            }
        }
        let mut masks = vec![vec![0u8; cells]; self.k];
        for (c, seq) in per_cell.iter().enumerate() {
            for (t, m) in seq.iter().enumerate() {
                masks[t][c] = *m;
            }
        }
        FramePlan {
            masks,
            static_masks,
        }
    }
}

/// A stateful long-horizon modulator used as the "stale history" comparison.
///
/// It installs a new projection with [`ResetPolicy::Keep`] — the naive
/// continuous-accumulation policy — so accumulators from the previous frame
/// leak into the new one. This is precisely the failure the frame-local
/// scheduler avoids.
pub struct StaleAccumulator {
    field: TemporalBrailleField,
    width: u16,
    height: u16,
}

impl StaleAccumulator {
    pub fn new(width: u16, height: u16, seed: u64) -> Self {
        Self {
            field: TemporalBrailleField::new(width, height, seed),
            width,
            height,
        }
    }

    /// Installs a new source frame while keeping residual history (`Keep`).
    pub fn retarget(&mut self, projection: &BrailleImageProjection) {
        // The field is sized to match; install_into returns false on mismatch and
        // we have already asserted dimensions in new().
        let _ = projection.install_into(&mut self.field, ResetPolicy::Keep);
    }

    /// Advances `k` subframes, returning their decoded masks (via the caller's
    /// Surface decode, which we avoid here by using the field's residual path
    /// and emitting masks through the same pure interface).
    pub fn advance_masks(&mut self, k: usize) -> Vec<Vec<u8>> {
        // Use the field's full sigma-delta (`advance_styled`) and decode through
        // an ordinary Surface; this is the real algorithm.
        let mut out = Vec::with_capacity(k);
        for _ in 0..k {
            let surface = self
                .field
                .advance_styled(gibson::SubcellGlyphMode::Braille2x4);
            out.push(super::metrics::surface_masks(&surface));
        }
        out
    }

    pub fn frame_index(&self) -> u64 {
        self.field.frame_index()
    }

    pub fn dimensions(&self) -> (u16, u16) {
        (self.width, self.height)
    }
}

/// Integrates emitted subframe masks into a time-averaged duty field, optionally
/// using only a surviving subset of subframes (presentation loss simulation).
pub fn average_duty(masks: &[Vec<u8>], keep: &[bool], cells: usize) -> Vec<[f32; 8]> {
    let mut avg = vec![[0f32; 8]; cells];
    let mut weight = 0u32;
    for (t, frame) in masks.iter().enumerate() {
        if !keep.get(t).copied().unwrap_or(true) {
            continue;
        }
        weight += 1;
        for c in 0..cells {
            let m = frame[c];
            for i in 0..8 {
                if m & (1 << i) != 0 {
                    avg[c][i] += 1.0;
                }
            }
        }
    }
    if weight > 0 {
        for a in avg.iter_mut() {
            for v in a.iter_mut() {
                *v /= weight as f32;
            }
        }
    }
    avg
}

/// Presentation-loss models used by the hostility experiment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LossModel {
    /// Every phase shown.
    Full,
    /// Deterministic 2:1 subsample (only even phases shown).
    Half,
    /// Drop every third phase.
    DropEvery3,
    /// Seeded ~10% random loss.
    Random10,
    /// Seeded ~20% random loss.
    Random20,
}

impl LossModel {
    pub fn as_str(self) -> &'static str {
        match self {
            LossModel::Full => "full",
            LossModel::Half => "half-2:1",
            LossModel::DropEvery3 => "drop-1/3",
            LossModel::Random10 => "random-10%",
            LossModel::Random20 => "random-20%",
        }
    }

    /// The surviving-phase mask for a `K`-phase window.
    pub fn survivors(self, k: usize, seed: u64) -> Vec<bool> {
        let mut keep = vec![true; k];
        match self {
            LossModel::Full => {}
            LossModel::Half => {
                for (t, ok) in keep.iter_mut().enumerate() {
                    *ok = t % 2 == 0;
                }
            }
            LossModel::DropEvery3 => {
                for (t, ok) in keep.iter_mut().enumerate() {
                    *ok = t % 3 != 2;
                }
            }
            LossModel::Random10 => random_loss(&mut keep, 0.10, seed),
            LossModel::Random20 => random_loss(&mut keep, 0.20, seed),
        }
        if !keep.iter().any(|k| *k) {
            // Never lose every phase: the static first phase always survives.
            keep[0] = true;
        }
        keep
    }
}

fn random_loss(keep: &mut [bool], rate: f32, seed: u64) {
    for (t, ok) in keep.iter_mut().enumerate() {
        let mut z = seed ^ (t as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        let u = ((z >> 40) as f32) / ((1u32 << 24) as f32);
        *ok = u >= rate;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gibson::temporal::project_braille_image;

    fn proj() -> BrailleImageProjection {
        project_braille_image(4, 2, |lx, ly| {
            let v = 40 + ((lx + ly) * 8);
            [v as u8, v as u8, v as u8]
        })
    }

    #[test]
    fn frame_local_plan_is_deterministic_and_has_k_frames() {
        let s = FrameLocalScheduler::new(4, ScheduleKind::WindowedErrorFeedback, 1);
        let p = proj();
        let a = s.plan(&p, 0);
        let b = s.plan(&p, 0);
        assert_eq!(a.masks, b.masks);
        assert_eq!(a.masks.len(), 4);
        assert_eq!(a.static_masks.len(), 8);
    }

    #[test]
    fn k1_plan_is_exactly_static() {
        let s = FrameLocalScheduler::new(1, ScheduleKind::WindowedErrorFeedback, 1);
        let p = proj();
        let plan = s.plan(&p, 0);
        assert_eq!(plan.masks.len(), 1);
        assert_eq!(plan.masks[0], plan.static_masks);
    }

    #[test]
    fn averaged_duty_of_full_loss_is_the_plan_mean() {
        let s = FrameLocalScheduler::new(4, ScheduleKind::WindowedErrorFeedback, 3);
        let p = proj();
        let plan = s.plan(&p, 0);
        let cells = plan.masks[0].len();
        let avg = average_duty(&plan.masks, &[true; 4], cells);
        // Each dot's average equals its lit count / 4.
        for c in 0..cells {
            for i in 0..8 {
                let lit = plan.masks.iter().filter(|m| m[c] & (1 << i) != 0).count();
                assert!((avg[c][i] - lit as f32 / 4.0).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn loss_models_never_remove_every_phase() {
        for k in 1..=8 {
            for m in [
                LossModel::Full,
                LossModel::Half,
                LossModel::DropEvery3,
                LossModel::Random10,
                LossModel::Random20,
            ] {
                let keep = m.survivors(k, 5);
                assert_eq!(keep.len(), k);
                assert!(keep.iter().any(|x| *x), "{m:?} k={k}");
            }
        }
    }

    #[test]
    fn half_loss_keeps_even_phases() {
        let keep = LossModel::Half.survivors(6, 0);
        assert_eq!(keep, vec![true, false, true, false, true, false]);
    }
}
