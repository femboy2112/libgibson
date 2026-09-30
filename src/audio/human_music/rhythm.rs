//! Canonical metric positions and declared groove transports.
//!
//! Song identity uses exact rational beats. A transported position carries its metric
//! source: projection is exact by construction, rather than guessed from a nearby float.
//! Legacy float adapters preserve the historical rounding and arithmetic at their boundary.

use std::cmp::Ordering;
use std::num::NonZeroU32;

/// A reduced rational number of quarter-note beats, independent of tempo and groove.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MetricPosition {
    ticks: i64,
    subdivision: NonZeroU32,
}

impl MetricPosition {
    /// Construct an indexed metric position. A zero subdivision has no meaning.
    pub fn new(ticks: i64, subdivision: u32) -> Option<Self> {
        let mut denominator = u64::from(NonZeroU32::new(subdivision)?.get());
        let mut numerator = ticks.unsigned_abs();
        while denominator != 0 {
            (numerator, denominator) = (denominator, numerator % denominator);
        }
        let divisor = numerator as i64;
        Some(Self {
            ticks: ticks / divisor,
            subdivision: NonZeroU32::new(subdivision / numerator as u32)?,
        })
    }

    /// Import a finite legacy float only when its exact value fits this rational domain.
    /// No tolerance or quantization is applied; unsupported denominators are rejected.
    pub fn from_exact_beats(beats: f64) -> Option<Self> {
        if !beats.is_finite() {
            return None;
        }
        if beats == 0.0 {
            return Self::new(0, 1);
        }
        let bits = beats.to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as i32;
        let mut mantissa = bits & ((1_u64 << 52) - 1);
        let mut shift = if exponent == 0 {
            -1074
        } else {
            mantissa |= 1_u64 << 52;
            exponent - 1023 - 52
        };
        let trailing = mantissa.trailing_zeros();
        mantissa >>= trailing;
        shift += trailing as i32;
        if shift > 63 {
            return None;
        }
        let (numerator, denominator) = if shift >= 0 {
            (i128::from(mantissa).checked_shl(shift as u32)?, 1)
        } else {
            (i128::from(mantissa), 1_u32.checked_shl((-shift) as u32)?)
        };
        let signed = if beats.is_sign_negative() {
            -numerator
        } else {
            numerator
        };
        Self::new(i64::try_from(signed).ok()?, denominator)
    }

    pub fn ticks(self) -> i64 {
        self.ticks
    }

    pub fn subdivision(self) -> u32 {
        self.subdivision.get()
    }

    /// Explicit conversion at a realization/render boundary.
    pub fn beats(self) -> f64 {
        self.ticks as f64 / f64::from(self.subdivision.get())
    }

    fn odd_eighth(self) -> bool {
        let eighth = i128::from(self.ticks) * 2;
        let subdivision = i128::from(self.subdivision.get());
        eighth % subdivision == 0 && (eighth / subdivision).rem_euclid(2) == 1
    }
}

impl Ord for MetricPosition {
    fn cmp(&self, other: &Self) -> Ordering {
        (i128::from(self.ticks) * i128::from(other.subdivision.get()))
            .cmp(&(i128::from(other.ticks) * i128::from(self.subdivision.get())))
    }
}

impl PartialOrd for MetricPosition {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl super::fingerprint::CanonicalFingerprint for MetricPosition {
    fn encode(&self, writer: &mut super::fingerprint::FingerprintWriter) {
        writer.tag("MetricPosition/v2");
        writer.field("ticks", &self.ticks);
        writer.field("subdivision", &self.subdivision.get());
    }
}

/// The current eighth-offbeat transport. Zero is the identity feel; no timing noise is added.
/// This sparse historical law is not a bijection on arbitrary real-valued beats. Its target
/// retains the source coordinate; callers admitting arbitrary grids must check event order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrooveTransport {
    swing: f32,
}

impl GrooveTransport {
    pub const fn straight() -> Self {
        Self { swing: 0.0 }
    }

    /// A bounded, finite eighth-offbeat delay. Other swing conventions need their own law.
    pub fn eighth_swing(amount: f32) -> Option<Self> {
        (amount.is_finite() && (0.0..=1.0).contains(&amount)).then_some(Self { swing: amount })
    }

    pub fn swing(self) -> f32 {
        self.swing
    }

    pub fn transport(self, metric: MetricPosition) -> PerformedPosition {
        let straight = metric.beats();
        let beats = if self.swing > 0.0 && metric.odd_eighth() {
            straight + f64::from(self.swing) * 0.25
        } else {
            straight
        };
        PerformedPosition { metric, beats }
    }
}

impl super::fingerprint::CanonicalFingerprint for GrooveTransport {
    fn encode(&self, writer: &mut super::fingerprint::FingerprintWriter) {
        writer.tag("GrooveTransport/eighth-swing/v2");
        writer.field("swing", &self.swing);
    }
}

/// A performed coordinate with the exact metric source needed for lawful projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerformedPosition {
    metric: MetricPosition,
    beats: f64,
}

impl PerformedPosition {
    pub fn metric(self) -> MetricPosition {
        self.metric
    }

    pub fn beats(self) -> f64 {
        self.beats
    }
}

/// Historical float boundary shared by phrase and current drum realization.
/// The tolerance belongs to importing untyped coordinates, not to metric identity.
pub(crate) fn legacy_eighth_position(beat: f64, swing: f32) -> f64 {
    let eighth = (beat * 2.0).round() as i64;
    if swing > 0.0 && (beat * 2.0 - eighth as f64).abs() < 1e-6 && eighth.rem_euclid(2) == 1 {
        beat + f64::from(swing) * 0.25
    } else {
        beat
    }
}

/// Historical subdivision swing for the original GrooveBuilder experiment.
pub(crate) fn legacy_subdivision_position(beat: f64, swing: f32, subdivision: u32) -> f64 {
    if swing <= 0.0 {
        return beat;
    }
    let sub = 1.0 / subdivision as f64;
    let idx = (beat / sub).round() as i64;
    if idx % 2 != 0 {
        beat + swing as f64 * sub * 0.5
    } else {
        beat
    }
}

/// One indexed pulse slot. The historical fields and Debug receipt are preserved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatticeSlot {
    pub index: i64,
    pub subdivision: u32,
    pub position: f64,
    pub swing_phase: f64,
}

impl LatticeSlot {
    pub fn phase(self) -> u32 {
        self.index.rem_euclid(i64::from(self.subdivision)) as u32
    }

    /// The canonical source of a slot produced by [`lattice_slots`].
    pub fn metric(self) -> Option<MetricPosition> {
        MetricPosition::new(self.index, self.subdivision)
    }
}

/// Enumerate declared pulse grids from beat zero, transporting before corridor restriction.
/// Which subdivisions an experiment admits is a caller policy, not a world-wide identity law.
pub fn lattice_slots(divisions: &[NonZeroU32], swing: f32, from: f64, to: f64) -> Vec<LatticeSlot> {
    let mut positions = Vec::new();
    for division in divisions {
        let division = division.get();
        let first_bar = (from / 4.0).floor() as i64;
        let last_bar = (to / 4.0).floor() as i64;
        for bar in first_bar..=last_bar {
            for tick in 0..4 * i64::from(division) {
                let index = bar * 4 * i64::from(division) + tick;
                let straight = index as f64 / f64::from(division);
                let position = legacy_eighth_position(straight, swing);
                if position >= from - 1e-6 && position < to - 1e-6 {
                    positions.push(LatticeSlot {
                        index,
                        subdivision: division,
                        position,
                        swing_phase: position - straight,
                    });
                }
            }
        }
    }
    positions.sort_by(|a, b| a.position.total_cmp(&b.position));
    positions.dedup_by(|a, b| (a.position - b.position).abs() < 1e-6);
    positions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_identity_is_exact_and_float_import_never_quantizes() {
        assert_eq!(MetricPosition::new(2, 6), MetricPosition::new(1, 3));
        assert_eq!(MetricPosition::new(0, 7), MetricPosition::new(0, 1));
        assert!(MetricPosition::new(1, 0).is_none());
        assert_eq!(
            MetricPosition::from_exact_beats(-0.375),
            MetricPosition::new(-3, 8)
        );
        assert!(MetricPosition::from_exact_beats(1.0 / 3.0).is_none());
        assert!(MetricPosition::from_exact_beats(f64::NAN).is_none());
        assert!(MetricPosition::from_exact_beats(f64::INFINITY).is_none());
        assert!(MetricPosition::from_exact_beats(f64::MAX).is_none());
        assert_eq!(
            MetricPosition::from_exact_beats(i64::MIN as f64),
            MetricPosition::new(i64::MIN, 1)
        );
        assert!(MetricPosition::new(-1, 3) < MetricPosition::new(-1, 4));
    }

    #[test]
    fn metric_transport_projection_commutes_without_a_float_inverse() {
        for division in [1, 2, 3, 4, 8, 12] {
            for index in -96..=96 {
                let metric = MetricPosition::new(index, division).unwrap();
                for swing in [0.0, 0.16, 0.5, 1.0] {
                    let transported = GrooveTransport::eighth_swing(swing)
                        .unwrap()
                        .transport(metric);
                    assert_eq!(transported.metric(), metric);
                    assert_eq!(
                        transported.beats().to_bits(),
                        legacy_eighth_position(metric.beats(), swing).to_bits()
                    );
                }
                assert_eq!(
                    GrooveTransport::straight().transport(metric).beats(),
                    metric.beats()
                );
            }
        }
        assert!(GrooveTransport::eighth_swing(f32::NAN).is_none());
        assert!(GrooveTransport::eighth_swing(-0.1).is_none());
        assert!(GrooveTransport::eighth_swing(1.1).is_none());
    }

    #[test]
    fn authored_metric_grid_and_world_hat_grid_are_distinct_domains() {
        let eighths = lattice_slots(&[NonZeroU32::new(2).unwrap()], 0.16, 0.0, 1.0);
        let sixteenths = lattice_slots(&[NonZeroU32::new(4).unwrap()], 0.16, 0.0, 1.0);
        let quarter = MetricPosition::new(1, 4).unwrap();
        let half = MetricPosition::new(1, 2).unwrap();
        assert!(!eighths.iter().any(|slot| slot.metric() == Some(quarter)));
        assert!(sixteenths.iter().any(|slot| slot.metric() == Some(quarter)));
        let swung_half = sixteenths
            .iter()
            .find(|slot| slot.metric() == Some(half))
            .unwrap();
        assert_ne!(swung_half.position, half.beats());
        assert_eq!(
            swung_half.position,
            GrooveTransport::eighth_swing(0.16)
                .unwrap()
                .transport(half)
                .beats()
        );
        assert_eq!(
            half.beats(),
            0.5,
            "structural metric coordinate does not move"
        );
    }
}
