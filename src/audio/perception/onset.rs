//! Onset / transient salience — the third MVP witness.
//!
//! Attacks and transients carry disproportionate perceptual weight (instrument identity,
//! rhythm, intelligibility). We measure onset salience as **half-wave-rectified spectral
//! flux across the ERB bank**: the summed per-band *increase* in short-time energy from
//! one frame to the next. Energy that rises (an attack) contributes; energy that decays
//! does not. Deterministic and inspectable; peaks mark onsets.

use super::filterbank::ErbBank;
use super::temporal::{envelope_rms, frame_count};

/// Per-frame onset salience for `mono` under `bank`: for each analysis frame, the sum over
/// channels of `max(0, rms[band][t] - rms[band][t-1])`. The first frame is `0.0`. Larger =
/// more simultaneous band-energy *increase* = a stronger attack.
pub fn onset_salience(bank: &ErbBank, mono: &[f32], frame: usize, hop: usize) -> Vec<f32> {
    let nf = frame_count(mono.len(), frame, hop);
    if nf == 0 {
        return Vec::new();
    }
    // Per-band framed envelope, then accumulate half-wave flux across bands.
    let mut flux = vec![0.0f32; nf];
    for b in 0..bank.len() {
        let env = envelope_rms(&bank.filter_band(b, mono), frame, hop);
        for t in 1..env.len() {
            let rise = env[t] - env[t - 1];
            if rise > 0.0 {
                flux[t] += rise;
            }
        }
    }
    flux
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;
    const FRAME: usize = 1024;
    const HOP: usize = 512;

    /// A train of short 1 kHz tone bursts (attack onsets) separated by silence.
    fn burst_train(period: usize, on: usize, reps: usize) -> Vec<f32> {
        let mut x = vec![0.0f32; period * reps];
        for r in 0..reps {
            let base = r * period;
            for i in 0..on {
                x[base + i] =
                    (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / SR as f64).sin() as f32;
            }
        }
        x
    }

    #[test]
    fn silence_has_no_onsets() {
        let bank = ErbBank::default_for(SR);
        let s = onset_salience(&bank, &vec![0.0f32; 8192], FRAME, HOP);
        assert!(s.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn bursts_peak_at_onsets() {
        let bank = ErbBank::new(SR, 24, 50.0, 16_000.0);
        let period = 4800; // 100 ms
        let on = 1200; // 25 ms tone, then silence
        let x = burst_train(period, on, 6);
        let s = onset_salience(&bank, &x, FRAME, HOP);
        // The single largest energy-rise in the whole signal must be an attack: its frame
        // sits at a burst onset (allowing a couple of frames of window/filter lag).
        let argmax = s
            .iter()
            .enumerate()
            .skip(1)
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(t, _)| t)
            .unwrap();
        // The detecting frame can lead the true onset by up to one window (the RMS window
        // reaches FRAME samples forward), so accept a phase within ±FRAME of a burst start.
        let phase = (argmax * HOP) % period;
        assert!(
            phase < FRAME || phase > period - FRAME,
            "largest onset salience (frame {argmax}) should land at a burst onset, phase={phase}",
        );
        assert!(
            s.iter().sum::<f32>() > 0.0,
            "burst train must produce some onset salience"
        );
    }

    #[test]
    fn steady_tone_has_low_sustained_salience() {
        let bank = ErbBank::new(SR, 24, 50.0, 16_000.0);
        let tone: Vec<f32> = (0..24_000)
            .map(|i| (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / SR as f64).sin() as f32)
            .collect();
        let s = onset_salience(&bank, &tone, FRAME, HOP);
        // after the initial attack settles, salience should be near zero
        let tail = &s[6..];
        let m = tail.iter().sum::<f32>() / tail.len() as f32;
        assert!(m < 0.05, "sustained tone salience should be low, got {m}");
    }

    #[test]
    fn deterministic() {
        let bank = ErbBank::new(SR, 16, 50.0, 12_000.0);
        let x = burst_train(4000, 1000, 4);
        assert_eq!(
            onset_salience(&bank, &x, FRAME, HOP),
            onset_salience(&bank, &x, FRAME, HOP)
        );
    }
}
