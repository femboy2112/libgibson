//! Music-theory fundamentals: pitch (MIDI), scales/modes, chords/qualities and harmonic
//! function. The vocabulary the harmony, voicing, motif and melody engines are built on.
//!
//! Pitch is a MIDI note number (`i32`, middle C = 60, A4 = 69 = 440 Hz). Pitch *class* is
//! `0..=11` (C = 0). Keeping pitch as a plain integer makes voice-leading distance an
//! honest semitone count.

/// The coordinate unit of a relative pitch contour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PitchBasis {
    /// Steps through a declared scale.
    ScaleSteps,
    /// Exact chromatic offsets, independent of the target scale.
    Semitones,
}

/// A MIDI note number.
pub type Midi = i32;

/// Convert a MIDI note to frequency in Hz (equal temperament, A4 = 440).
#[inline]
pub fn midi_to_hz(m: Midi) -> f32 {
    440.0 * 2f32.powf((m as f32 - 69.0) / 12.0)
}

/// A MIDI pitch as a note name with octave, sharps only, MIDI 60 = `C4` (`A#2`, `E4`, …) — for
/// dumps and diagnostics that must show the actual offending notes.
pub fn note_name(m: Midi) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!(
        "{}{}",
        NAMES[m.rem_euclid(12) as usize],
        m.div_euclid(12) - 1
    )
}

/// The pitch class `0..=11` of a MIDI note.
#[inline]
pub fn pitch_class(m: Midi) -> i32 {
    m.rem_euclid(12)
}

/// An octave-independent pitch-class set. Only the twelve musical bits can be represented.
/// Ordered chord tones remain a separate object: this set deliberately forgets their order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PitchClassSet(u16);

impl PitchClassSet {
    pub const EMPTY: Self = Self(0);

    /// Import a legacy mask; bits outside the twelve pitch classes have no musical meaning.
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits & 0x0fff)
    }

    pub fn from_pitches(pitches: &[Midi]) -> Self {
        pitches.iter().copied().collect()
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub fn contains(self, pitch: Midi) -> bool {
        self.0 & (1_u16 << pitch_class(pitch)) != 0
    }
}

impl FromIterator<Midi> for PitchClassSet {
    fn from_iter<T: IntoIterator<Item = Midi>>(iter: T) -> Self {
        Self(
            iter.into_iter()
                .fold(0, |mask, pitch| mask | (1_u16 << pitch_class(pitch))),
        )
    }
}

impl super::fingerprint::CanonicalFingerprint for PitchClassSet {
    fn encode(&self, writer: &mut super::fingerprint::FingerprintWriter) {
        writer.tag("PitchClassSet/v2");
        writer.field("bits", &self.0);
    }
}

/// Diatonic and common synthetic modes (interval pattern from the tonic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Ionian,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Aeolian,
    Locrian,
    HarmonicMinor,
}

impl Mode {
    /// Semitone offsets of the seven scale degrees from the tonic.
    pub fn intervals(self) -> [i32; 7] {
        match self {
            Mode::Ionian => [0, 2, 4, 5, 7, 9, 11],
            Mode::Dorian => [0, 2, 3, 5, 7, 9, 10],
            Mode::Phrygian => [0, 1, 3, 5, 7, 8, 10],
            Mode::Lydian => [0, 2, 4, 6, 7, 9, 11],
            Mode::Mixolydian => [0, 2, 4, 5, 7, 9, 10],
            Mode::Aeolian => [0, 2, 3, 5, 7, 8, 10],
            Mode::Locrian => [0, 1, 3, 5, 6, 8, 10],
            Mode::HarmonicMinor => [0, 2, 3, 5, 7, 8, 11],
        }
    }
}

/// A scale: a tonic pitch class plus a mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scale {
    /// Tonic pitch class `0..=11`.
    pub tonic_pc: i32,
    /// The mode.
    pub mode: Mode,
}

impl Scale {
    /// A scale on `tonic_pc` in `mode`.
    pub fn new(tonic_pc: i32, mode: Mode) -> Scale {
        Scale {
            tonic_pc: tonic_pc.rem_euclid(12),
            mode,
        }
    }

    /// The MIDI pitch of a (0-based) scale `degree` at `octave` (octave 4 ≈ middle).
    /// Degrees beyond 6 wrap to the next octave, so degree 7 is the tonic an octave up.
    pub fn degree_pitch(&self, degree: i32, octave: i32) -> Midi {
        let iv = self.mode.intervals();
        let d = degree.rem_euclid(7) as usize;
        let oct_shift = degree.div_euclid(7);
        (octave + oct_shift + 1) * 12 + self.tonic_pc + iv[d]
    }

    /// True if `pc` (a pitch class) is in the scale.
    pub fn contains_pc(&self, pc: i32) -> bool {
        let iv = self.mode.intervals();
        let rel = (pc - self.tonic_pc).rem_euclid(12);
        iv.contains(&rel)
    }

    /// The nearest scale pitch to `m` (ties resolve downward).
    pub fn nearest_scale_pitch(&self, m: Midi) -> Midi {
        if self.contains_pc(pitch_class(m)) {
            return m;
        }
        for d in 1..=6 {
            if self.contains_pc(pitch_class(m - d)) {
                return m - d;
            }
            if self.contains_pc(pitch_class(m + d)) {
                return m + d;
            }
        }
        m
    }
}

/// Chord qualities — enough vocabulary for functional harmony with extensions and the
/// borrowed colors the richer worlds use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Maj,
    Min,
    Dim,
    Aug,
    Maj7,
    Min7,
    Dom7,
    Min7b5,
    Dim7,
    MinMaj7,
    Sus4,
    Sus2,
    Maj9,
    Min9,
    Dom9,
    Add9,
    Maj6,
    Min6,
}

impl Quality {
    /// Semitone offsets from the root (root = 0 included).
    pub fn intervals(self) -> &'static [i32] {
        match self {
            Quality::Maj => &[0, 4, 7],
            Quality::Min => &[0, 3, 7],
            Quality::Dim => &[0, 3, 6],
            Quality::Aug => &[0, 4, 8],
            Quality::Maj7 => &[0, 4, 7, 11],
            Quality::Min7 => &[0, 3, 7, 10],
            Quality::Dom7 => &[0, 4, 7, 10],
            Quality::Min7b5 => &[0, 3, 6, 10],
            Quality::Dim7 => &[0, 3, 6, 9],
            Quality::MinMaj7 => &[0, 3, 7, 11],
            Quality::Sus4 => &[0, 5, 7],
            Quality::Sus2 => &[0, 2, 7],
            Quality::Maj9 => &[0, 4, 7, 11, 14],
            Quality::Min9 => &[0, 3, 7, 10, 14],
            Quality::Dom9 => &[0, 4, 7, 10, 14],
            Quality::Add9 => &[0, 4, 7, 14],
            Quality::Maj6 => &[0, 4, 7, 9],
            Quality::Min6 => &[0, 3, 7, 9],
        }
    }

    /// A short label for provenance / diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            Quality::Maj => "",
            Quality::Min => "m",
            Quality::Dim => "dim",
            Quality::Aug => "aug",
            Quality::Maj7 => "maj7",
            Quality::Min7 => "m7",
            Quality::Dom7 => "7",
            Quality::Min7b5 => "m7b5",
            Quality::Dim7 => "dim7",
            Quality::MinMaj7 => "mMaj7",
            Quality::Sus4 => "sus4",
            Quality::Sus2 => "sus2",
            Quality::Maj9 => "maj9",
            Quality::Min9 => "m9",
            Quality::Dom9 => "9",
            Quality::Add9 => "add9",
            Quality::Maj6 => "6",
            Quality::Min6 => "m6",
        }
    }
}

/// A chord's family by its triad: what a quality-family relation preserves, and the family a
/// [`super::vocabulary::HarmonicVocabulary`] retracts an undeclared colour into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QualityFamily {
    Major,
    Minor,
    Diminished,
    Augmented,
    Suspended,
}

impl QualityFamily {
    pub fn of(q: Quality) -> Self {
        match q {
            Quality::Maj
            | Quality::Maj7
            | Quality::Dom7
            | Quality::Maj9
            | Quality::Dom9
            | Quality::Add9
            | Quality::Maj6 => QualityFamily::Major,
            Quality::Min | Quality::Min7 | Quality::MinMaj7 | Quality::Min9 | Quality::Min6 => {
                QualityFamily::Minor
            }
            Quality::Dim | Quality::Min7b5 | Quality::Dim7 => QualityFamily::Diminished,
            Quality::Aug => QualityFamily::Augmented,
            Quality::Sus4 | Quality::Sus2 => QualityFamily::Suspended,
        }
    }
    /// The family's members, simplest first (the order a target vocabulary is searched).
    pub fn members(self) -> &'static [Quality] {
        match self {
            QualityFamily::Major => &[
                Quality::Maj,
                Quality::Dom7,
                Quality::Maj7,
                Quality::Maj6,
                Quality::Add9,
                Quality::Dom9,
                Quality::Maj9,
            ],
            QualityFamily::Minor => &[
                Quality::Min,
                Quality::Min7,
                Quality::Min6,
                Quality::Min9,
                Quality::MinMaj7,
            ],
            QualityFamily::Diminished => &[Quality::Dim, Quality::Min7b5, Quality::Dim7],
            QualityFamily::Augmented => &[Quality::Aug],
            QualityFamily::Suspended => &[Quality::Sus4, Quality::Sus2],
        }
    }
}

/// A chord as a root pitch class plus a quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    /// Root pitch class `0..=11`.
    pub root_pc: i32,
    /// The quality.
    pub quality: Quality,
}

impl Chord {
    /// A chord.
    pub fn new(root_pc: i32, quality: Quality) -> Chord {
        Chord {
            root_pc: root_pc.rem_euclid(12),
            quality,
        }
    }

    /// The chord's pitch classes.
    pub fn pitch_classes(&self) -> Vec<i32> {
        self.pitch_class_iter().collect()
    }

    fn pitch_class_iter(&self) -> impl Iterator<Item = i32> + '_ {
        self.quality
            .intervals()
            .iter()
            .map(|&i| (self.root_pc + i).rem_euclid(12))
    }

    /// The unordered chord membership relation, without allocating an ordered tone vector.
    pub fn pitch_class_set(&self) -> PitchClassSet {
        self.pitch_class_iter().collect()
    }

    /// The chord tones as MIDI pitches stacked from `base` (the root at or above `base`).
    pub fn pitches_from(&self, base: Midi) -> Vec<Midi> {
        let root = {
            let mut r = (base / 12) * 12 + self.root_pc;
            if r < base {
                r += 12;
            }
            r
        };
        self.quality.intervals().iter().map(|&i| root + i).collect()
    }

    /// True if `pc` is a chord tone.
    pub fn contains_pc(&self, pc: i32) -> bool {
        self.pitch_class_set().contains(pc)
    }

    /// The nearest chord tone to `m`.
    pub fn nearest_chord_tone(&self, m: Midi) -> Midi {
        let pcs = self.pitch_class_set();
        (0..=12)
            .flat_map(|d| [m - d, m + d])
            .find(|&c| pcs.contains(c))
            .unwrap_or(m)
    }

    /// A pitch-class-set root label, e.g. `Dm7`.
    pub fn label(&self) -> String {
        const NAMES: [&str; 12] = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        format!("{}{}", NAMES[self.root_pc as usize], self.quality.label())
    }
}

/// Harmonic function — the role a chord plays in a tonal region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Function {
    /// Stability / arrival.
    Tonic,
    /// Motion away — the pre-dominant.
    Predominant,
    /// Maximum instability — pulls to tonic.
    Dominant,
}

/// Total absolute semitone distance between two equal-length pitch lists (a voice-leading
/// cost primitive). Panics-free: the shorter list governs.
pub fn voice_motion(a: &[Midi], b: &[Midi]) -> i32 {
    a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_is_440() {
        assert!((midi_to_hz(69) - 440.0).abs() < 1e-3);
        assert!((midi_to_hz(60) - 261.6256).abs() < 0.01); // middle C
    }

    #[test]
    fn c_major_scale_degrees() {
        let s = Scale::new(0, Mode::Ionian);
        // C4=60, D4=62, E4=64, F4=65, G4=67, A4=69, B4=71, C5=72.
        assert_eq!(s.degree_pitch(0, 4), 60);
        assert_eq!(s.degree_pitch(1, 4), 62);
        assert_eq!(s.degree_pitch(6, 4), 71);
        assert_eq!(s.degree_pitch(7, 4), 72); // wraps to next octave tonic
        assert!(s.contains_pc(4)); // E
        assert!(!s.contains_pc(1)); // C#
    }

    #[test]
    fn chord_pitch_classes_and_stacking() {
        let cmaj7 = Chord::new(0, Quality::Maj7);
        assert_eq!(cmaj7.pitch_classes(), vec![0, 4, 7, 11]);
        let p = cmaj7.pitches_from(60);
        assert_eq!(p, vec![60, 64, 67, 71]);
        // Root below base gets lifted an octave.
        let g = Chord::new(7, Quality::Maj);
        assert_eq!(g.pitches_from(60), vec![67, 71, 74]);
        assert_eq!(cmaj7.label(), "Cmaj7");
    }

    #[test]
    fn nearest_scale_and_chord_snapping() {
        let s = Scale::new(0, Mode::Ionian);
        assert_eq!(s.nearest_scale_pitch(61), 60); // C# -> C
        let c = Chord::new(0, Quality::Maj);
        assert_eq!(c.nearest_chord_tone(62), 60); // D -> nearest of {C,E,G}
    }

    #[test]
    fn voice_motion_counts_semitones() {
        assert_eq!(voice_motion(&[60, 64, 67], &[60, 65, 67]), 1);
        assert_eq!(voice_motion(&[60, 64, 67], &[62, 65, 69]), 2 + 1 + 2);
    }
}
