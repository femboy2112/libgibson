//! The canonical internal audio representation: planar `f32` stereo blocks.
//!
//! Processing is planar (separate `left`/`right`) because per-channel DSP is cleaner and
//! branch-free; output to WAV/device interleaves on the way out. Samples are nominally in
//! `[-1.0, 1.0]` but intermediate sums may exceed that until the master limiter runs — the
//! type does not clamp, it only *measures* (peak/RMS/DC), so mastering stays explicit.

/// A block of stereo audio, `left`/`right` planar, both the same length in frames.
#[derive(Debug, Clone, PartialEq)]
pub struct StereoBlock {
    /// Left-channel samples.
    pub left: Vec<f32>,
    /// Right-channel samples.
    pub right: Vec<f32>,
}

impl StereoBlock {
    /// A zeroed block of `frames` frames.
    pub fn new(frames: usize) -> StereoBlock {
        StereoBlock {
            left: vec![0.0; frames],
            right: vec![0.0; frames],
        }
    }

    /// Frame count (per channel). Panics-free; both channels are kept equal by every
    /// mutator in this module.
    #[inline]
    pub fn frames(&self) -> usize {
        self.left.len()
    }

    /// True if there are no frames.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.left.is_empty()
    }

    /// Zero every sample in place (reused across render blocks without reallocating).
    #[inline]
    pub fn clear(&mut self) {
        self.left.iter_mut().for_each(|s| *s = 0.0);
        self.right.iter_mut().for_each(|s| *s = 0.0);
    }

    /// Add another block sample-for-sample. The shorter length governs (defensive: a
    /// mismatched block adds only its overlapping prefix rather than panicking).
    #[inline]
    pub fn add(&mut self, other: &StereoBlock) {
        let n = self.frames().min(other.frames());
        for i in 0..n {
            self.left[i] += other.left[i];
            self.right[i] += other.right[i];
        }
    }

    /// Add another block scaled by `gain`.
    #[inline]
    pub fn add_scaled(&mut self, other: &StereoBlock, gain: f32) {
        let n = self.frames().min(other.frames());
        for i in 0..n {
            self.left[i] += other.left[i] * gain;
            self.right[i] += other.right[i] * gain;
        }
    }

    /// Multiply every sample by `gain`.
    #[inline]
    pub fn scale(&mut self, gain: f32) {
        self.left.iter_mut().for_each(|s| *s *= gain);
        self.right.iter_mut().for_each(|s| *s *= gain);
    }

    /// Peak absolute sample across both channels.
    #[inline]
    pub fn peak(&self) -> f32 {
        let mut p = 0.0f32;
        for &s in self.left.iter().chain(self.right.iter()) {
            let a = s.abs();
            if a > p {
                p = a;
            }
        }
        p
    }

    /// RMS across both channels.
    pub fn rms(&self) -> f32 {
        let n = (self.left.len() + self.right.len()).max(1) as f64;
        let mut sum = 0.0f64;
        for &s in self.left.iter().chain(self.right.iter()) {
            sum += (s as f64) * (s as f64);
        }
        (sum / n).sqrt() as f32
    }

    /// Mean sample value per channel — a DC-offset probe. `(left_dc, right_dc)`.
    pub fn dc_offset(&self) -> (f32, f32) {
        let mean = |v: &[f32]| {
            if v.is_empty() {
                0.0
            } else {
                (v.iter().map(|&s| s as f64).sum::<f64>() / v.len() as f64) as f32
            }
        };
        (mean(&self.left), mean(&self.right))
    }

    /// True if any sample is NaN or infinite — a hard correctness failure for DSP.
    pub fn has_nonfinite(&self) -> bool {
        self.left
            .iter()
            .chain(self.right.iter())
            .any(|s| !s.is_finite())
    }

    /// Append interleaved `L,R,L,R,…` `f32` frames to `out` (device/WAV order).
    pub fn interleave_into(&self, out: &mut Vec<f32>) {
        out.reserve(self.frames() * 2);
        for i in 0..self.frames() {
            out.push(self.left[i]);
            out.push(self.right[i]);
        }
    }

    /// Append this block onto the tail of `dst` (accumulating a full offline render).
    pub fn append_to(&self, dst: &mut StereoBlock) {
        dst.left.extend_from_slice(&self.left);
        dst.right.extend_from_slice(&self.right);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_block_is_zeroed_and_finite() {
        let b = StereoBlock::new(64);
        assert_eq!(b.frames(), 64);
        assert_eq!(b.peak(), 0.0);
        assert!(!b.has_nonfinite());
    }

    #[test]
    fn add_and_scale_compose() {
        let mut a = StereoBlock::new(4);
        let mut b = StereoBlock::new(4);
        b.left.iter_mut().for_each(|s| *s = 0.5);
        b.right.iter_mut().for_each(|s| *s = -0.5);
        a.add(&b);
        a.add_scaled(&b, 2.0); // total +1.5 / -1.5
        assert!((a.left[0] - 1.5).abs() < 1e-6);
        assert!((a.right[0] + 1.5).abs() < 1e-6);
        a.scale(0.5);
        assert!((a.left[0] - 0.75).abs() < 1e-6);
    }

    #[test]
    fn peak_rms_dc_probes() {
        let mut b = StereoBlock::new(2);
        b.left = vec![1.0, -1.0];
        b.right = vec![0.5, 0.5];
        assert_eq!(b.peak(), 1.0);
        // rms over 4 samples: sqrt((1+1+0.25+0.25)/4) = sqrt(0.625)
        assert!((b.rms() - 0.625f32.sqrt()).abs() < 1e-6);
        let (ldc, rdc) = b.dc_offset();
        assert!(ldc.abs() < 1e-6); // 1 and -1 cancel
        assert!((rdc - 0.5).abs() < 1e-6);
    }

    #[test]
    fn nonfinite_is_detected() {
        let mut b = StereoBlock::new(2);
        b.left[1] = f32::NAN;
        assert!(b.has_nonfinite());
        b.left[1] = f32::INFINITY;
        assert!(b.has_nonfinite());
    }

    #[test]
    fn interleave_is_lrlr() {
        let mut b = StereoBlock::new(2);
        b.left = vec![1.0, 3.0];
        b.right = vec![2.0, 4.0];
        let mut out = Vec::new();
        b.interleave_into(&mut out);
        assert_eq!(out, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn mismatched_add_uses_overlap_not_panic() {
        let mut a = StereoBlock::new(2);
        let b = StereoBlock::new(4);
        a.add(&b); // must not panic
        assert_eq!(a.frames(), 2);
    }
}
