//! The **listening context** — mandatory, explicit, never hidden global state.
//!
//! HumanMusic PCM is nominally dimensionless `f32`: a sample value of `0.2` does not
//! imply any sound-pressure level. Therefore absolute-loudness claims cannot be made
//! honestly without a declared calibration. This module makes the context that the
//! observer `A_c` is indexed by a first-class, inspectable value — `c` is data the
//! caller supplies, not a listener state the observer hides.
//!
//! Any diagnostic that would require absolute SPL must return "undefined under this
//! context" (e.g. `None`) when the calibration is [`LevelCalibration::Relative`],
//! rather than fabricate a number.

/// How digital full scale maps (or does not map) to an absolute acoustic level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LevelCalibration {
    /// Relative comparisons only. No claim about absolute SPL or sones is permitted;
    /// any absolute-loudness diagnostic returns "undefined under this context".
    Relative,

    /// A declared mapping from digital full scale (0 dBFS) to acoustic level, in
    /// dB SPL. Only with this may absolute-level diagnostics emit a number, and even
    /// then as a modeled value under the declared mapping, not a measurement.
    Calibrated {
        /// dB SPL produced by a full-scale (0 dBFS) signal under the declared chain.
        db_spl_at_dbfs: f32,
    },
}

impl LevelCalibration {
    /// Whether absolute-level diagnostics are defined under this calibration.
    pub fn absolute_defined(self) -> bool {
        matches!(self, LevelCalibration::Calibrated { .. })
    }
}

/// How a stereo PCM pair is reduced to the single signal the (monaural, v1) observer
/// analyzes. Binaural models are explicitly out of scope for the first implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelModel {
    /// Equal-power-ish mean of the two channels: `(l + r) * 0.5`.
    MonoDownmix,
    /// Analyze the left channel only.
    LeftOnly,
    /// Analyze the right channel only.
    RightOnly,
}

impl ChannelModel {
    /// Reduce a stereo frame to the monaural sample the observer analyzes.
    pub fn reduce(self, l: f32, r: f32) -> f32 {
        match self {
            ChannelModel::MonoDownmix => (l + r) * 0.5,
            ChannelModel::LeftOnly => l,
            ChannelModel::RightOnly => r,
        }
    }
}

/// A minimal, declared frequency response for an optional playback/device transform.
/// v1 carries only a flat (identity) response; the type exists so later contexts can
/// add a real device/room transform without changing the observer's signature.
#[derive(Debug, Clone, PartialEq)]
pub struct FrequencyResponse {
    /// Piecewise gain breakpoints as `(hz, linear_gain)`, ascending by `hz`. Empty = flat.
    pub breakpoints: Vec<(f32, f32)>,
}

impl FrequencyResponse {
    /// A flat (unity) response.
    pub fn flat() -> Self {
        FrequencyResponse {
            breakpoints: Vec::new(),
        }
    }

    /// Linear gain at `hz` (linear interpolation between breakpoints; unity if empty).
    pub fn gain_at(&self, hz: f32) -> f32 {
        let bp = &self.breakpoints;
        if bp.is_empty() {
            return 1.0;
        }
        if hz <= bp[0].0 {
            return bp[0].1;
        }
        if hz >= bp[bp.len() - 1].0 {
            return bp[bp.len() - 1].1;
        }
        for w in bp.windows(2) {
            let (f0, g0) = w[0];
            let (f1, g1) = w[1];
            if hz >= f0 && hz <= f1 {
                let t = if (f1 - f0).abs() < 1e-9 {
                    0.0
                } else {
                    (hz - f0) / (f1 - f0)
                };
                return g0 + t * (g1 - g0);
            }
        }
        1.0
    }
}

impl Default for FrequencyResponse {
    fn default() -> Self {
        Self::flat()
    }
}

/// A minimal listener profile. v1 is intentionally near-empty: it is a named placeholder
/// so that per-listener parameters (hearing profile, etc.) can be added later without a
/// signature change. It carries no learned or personalized hearing model yet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListenerProfile {
    /// A label for provenance in receipts (e.g. "reference"). Not interpreted by the model.
    pub label: &'static str,
}

/// The explicit context `c` that the observer `A_c` is indexed by. Supplied by the
/// caller; never hidden or global.
#[derive(Debug, Clone, PartialEq)]
pub struct ListeningContext {
    /// Absolute-level calibration (or its explicit absence).
    pub level: LevelCalibration,
    /// The sample rate of the PCM being analyzed, in Hz.
    pub sample_rate_hz: u32,
    /// How stereo is reduced to the analyzed monaural signal.
    pub channel_model: ChannelModel,
    /// An optional declared device/playback response. `None` = flat/none.
    pub device_response: Option<FrequencyResponse>,
    /// The listener profile.
    pub listener: ListenerProfile,
}

impl ListeningContext {
    /// The honest default: relative-only calibration (no absolute SPL claims), mono
    /// downmix, flat/none device response, a reference listener — at `sample_rate_hz`.
    pub fn relative(sample_rate_hz: u32) -> Self {
        ListeningContext {
            level: LevelCalibration::Relative,
            sample_rate_hz,
            channel_model: ChannelModel::MonoDownmix,
            device_response: None,
            listener: ListenerProfile { label: "reference" },
        }
    }

    /// Whether absolute-level diagnostics may emit numbers under this context.
    pub fn absolute_defined(&self) -> bool {
        self.level.absolute_defined()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_forbids_absolute_level() {
        let c = ListeningContext::relative(48_000);
        assert!(!c.absolute_defined());
        assert_eq!(c.sample_rate_hz, 48_000);
        assert_eq!(c.channel_model, ChannelModel::MonoDownmix);
    }

    #[test]
    fn calibrated_allows_absolute_level() {
        let c = ListeningContext {
            level: LevelCalibration::Calibrated {
                db_spl_at_dbfs: 83.0,
            },
            ..ListeningContext::relative(44_100)
        };
        assert!(c.absolute_defined());
    }

    #[test]
    fn channel_reduce() {
        assert_eq!(ChannelModel::MonoDownmix.reduce(1.0, -1.0), 0.0);
        assert_eq!(ChannelModel::LeftOnly.reduce(0.3, 0.9), 0.3);
        assert_eq!(ChannelModel::RightOnly.reduce(0.3, 0.9), 0.9);
    }

    #[test]
    fn flat_response_is_unity() {
        let fr = FrequencyResponse::flat();
        assert_eq!(fr.gain_at(100.0), 1.0);
        assert_eq!(fr.gain_at(10_000.0), 1.0);
    }

    #[test]
    fn response_interpolates_and_clamps() {
        let fr = FrequencyResponse {
            breakpoints: vec![(100.0, 1.0), (1000.0, 2.0)],
        };
        assert_eq!(fr.gain_at(50.0), 1.0); // clamp low
        assert_eq!(fr.gain_at(2000.0), 2.0); // clamp high
        assert!((fr.gain_at(550.0) - 1.5).abs() < 1e-5); // midpoint
    }
}
