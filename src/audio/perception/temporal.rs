//! Temporal witnesses — **envelope** and **temporal fine structure (TFS)**, kept strictly
//! separate (the design note forbids collapsing them into one scalar).
//!
//! A band-limited signal splits into a slow amplitude **envelope** and a faster temporal
//! **fine structure** (the carrier detail). Both are perceptually load-bearing — TFS
//! carries pitch and masking cues — so the observer preserves them as independent
//! frame-rate series rather than throwing the fine structure away.
//!
//! These are pure functions over a single (already band-filtered) signal, so they compose
//! with [`super::filterbank::ErbBank::filter_band`] but know nothing about the bank.

/// Number of analysis frames for `n` samples with window `frame` and step `hop`.
pub fn frame_count(n: usize, frame: usize, hop: usize) -> usize {
    let frame = frame.max(1);
    let hop = hop.max(1);
    if n < frame {
        return if n == 0 { 0 } else { 1 };
    }
    (n - frame) / hop + 1
}

/// Per-frame RMS amplitude of `signal` — the **envelope** witness (slow magnitude).
pub fn envelope_rms(signal: &[f32], frame: usize, hop: usize) -> Vec<f32> {
    let frame = frame.max(1);
    let hop = hop.max(1);
    let nf = frame_count(signal.len(), frame, hop);
    (0..nf)
        .map(|f| {
            let start = f * hop;
            let end = (start + frame).min(signal.len());
            let win = &signal[start..end];
            if win.is_empty() {
                0.0
            } else {
                let ss: f64 = win.iter().map(|&v| (v as f64) * (v as f64)).sum();
                (ss / win.len() as f64).sqrt() as f32
            }
        })
        .collect()
}

/// Per-frame **zero-crossing rate** of `signal` in `[0,1]` — a TFS witness: fraction of
/// adjacent sample pairs that change sign. Independent of the envelope's magnitude, it
/// reflects the band's instantaneous fine structure (higher for faster carriers). It is a
/// deliberately minimal, deterministic, inspectable TFS proxy, not a full instantaneous-
/// frequency estimate.
pub fn fine_structure_zcr(signal: &[f32], frame: usize, hop: usize) -> Vec<f32> {
    let frame = frame.max(2);
    let hop = hop.max(1);
    let nf = frame_count(signal.len(), frame, hop);
    (0..nf)
        .map(|f| {
            let start = f * hop;
            let end = (start + frame).min(signal.len());
            if end - start < 2 {
                return 0.0;
            }
            let win = &signal[start..end];
            let crossings = win
                .windows(2)
                .filter(|p| (p[0] >= 0.0) != (p[1] >= 0.0))
                .count();
            crossings as f32 / (win.len() - 1) as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::perception::filterbank::ErbBank;

    fn sine(freq: f64, sr: u32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (2.0 * std::f64::consts::PI * freq * i as f64 / sr as f64).sin() as f32)
            .collect()
    }

    #[test]
    fn frame_count_basics() {
        assert_eq!(frame_count(0, 512, 256), 0);
        assert_eq!(frame_count(100, 512, 256), 1);
        assert_eq!(frame_count(512, 512, 256), 1);
        assert_eq!(frame_count(1024, 512, 256), 3);
    }

    #[test]
    fn envelope_of_silence_is_zero() {
        let e = envelope_rms(&vec![0.0f32; 4096], 512, 256);
        assert!(!e.is_empty() && e.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn envelope_of_steady_tone_is_steady() {
        let sr = 48_000;
        let bank = ErbBank::new(sr, 32, 50.0, 16_000.0);
        // nearest band to 1 kHz
        let b = bank
            .center_freqs()
            .iter()
            .enumerate()
            .min_by(|a, c| {
                (a.1 - 1000.0)
                    .abs()
                    .partial_cmp(&(c.1 - 1000.0).abs())
                    .unwrap()
            })
            .unwrap()
            .0;
        let sig = bank.filter_band(b, &sine(1000.0, sr, 24_000));
        let e = envelope_rms(&sig, 1024, 512);
        // skip the first few frames (filter/attack settling), then require low variation
        let tail = &e[4..];
        let mean = tail.iter().sum::<f32>() / tail.len() as f32;
        let var = tail.iter().map(|&v| (v - mean).powi(2)).sum::<f32>() / tail.len() as f32;
        assert!(mean > 1e-3, "tone should have energy");
        assert!(
            var.sqrt() / mean < 0.1,
            "steady tone envelope should be steady"
        );
    }

    #[test]
    fn envelope_tracks_a_ramp() {
        let sr = 48_000;
        let n = 24_000;
        let ramp: Vec<f32> = (0..n)
            .map(|i| {
                let a = i as f32 / n as f32;
                a * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr as f32).sin()
            })
            .collect();
        let e = envelope_rms(&ramp, 1024, 512);
        assert!(
            e[e.len() - 1] > e[2] * 3.0,
            "envelope should grow with the ramp"
        );
    }

    #[test]
    fn tfs_higher_for_faster_carrier() {
        let sr = 48_000;
        let bank = ErbBank::new(sr, 32, 50.0, 16_000.0);
        let near = |f: f32| {
            bank.center_freqs()
                .iter()
                .enumerate()
                .min_by(|a, c| (a.1 - f).abs().partial_cmp(&(c.1 - f).abs()).unwrap())
                .unwrap()
                .0
        };
        let lo = bank.filter_band(near(200.0), &sine(200.0, sr, 24_000));
        let hi = bank.filter_band(near(4000.0), &sine(4000.0, sr, 24_000));
        let z_lo = fine_structure_zcr(&lo, 1024, 512);
        let z_hi = fine_structure_zcr(&hi, 1024, 512);
        let m = |v: &[f32]| v[4..].iter().sum::<f32>() / (v.len() - 4) as f32;
        assert!(
            m(&z_hi) > m(&z_lo),
            "4 kHz carrier should cross zero more than 200 Hz"
        );
    }

    #[test]
    fn deterministic() {
        let sr = 48_000;
        let x = sine(880.0, sr, 8192);
        assert_eq!(envelope_rms(&x, 512, 256), envelope_rms(&x, 512, 256));
        assert_eq!(
            fine_structure_zcr(&x, 512, 256),
            fine_structure_zcr(&x, 512, 256)
        );
    }
}
