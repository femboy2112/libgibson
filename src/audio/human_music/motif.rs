//! Motif identities and their transformations. A motif is a scale-degree contour plus a
//! rhythm; the same motif heard at the climax must be recognizably the object seeded at the
//! start. Transformations preserve the identity (`id`) while developing the material, so
//! the melody engine can grow one idea across the whole piece instead of inventing a new
//! tune every four bars.

use super::theory::{Midi, Scale};

/// A motif: parallel scale-degree and rhythm vectors sharing an identity.
#[derive(Debug, Clone, PartialEq)]
pub struct Motif {
    /// Stable identity — survives every transformation.
    pub id: u8,
    /// Scale-degree offsets from a tonal root (the melodic contour).
    pub degrees: Vec<i32>,
    /// Note durations in beats, parallel to `degrees`.
    pub rhythm: Vec<f32>,
}

impl Motif {
    /// The signature seed: a rising-then-turning four-note call.
    pub fn seed_a() -> Motif {
        Motif {
            id: 0,
            degrees: vec![0, 2, 4, 3],
            rhythm: vec![1.0, 1.0, 1.0, 1.0],
        }
    }

    /// A contrasting, more active seed.
    pub fn seed_b() -> Motif {
        Motif {
            id: 1,
            degrees: vec![4, 3, 1, 0, 1],
            rhythm: vec![0.5, 0.5, 1.0, 0.5, 1.5],
        }
    }

    /// Number of notes.
    pub fn len(&self) -> usize {
        self.degrees.len()
    }

    /// True if empty.
    pub fn is_empty(&self) -> bool {
        self.degrees.is_empty()
    }

    /// Total duration in beats.
    pub fn total_beats(&self) -> f32 {
        self.rhythm.iter().sum()
    }

    /// Transpose the contour by `by` scale degrees (identity preserved).
    pub fn transpose(&self, by: i32) -> Motif {
        Motif {
            id: self.id,
            degrees: self.degrees.iter().map(|d| d + by).collect(),
            rhythm: self.rhythm.clone(),
        }
    }

    /// Invert the contour about its first degree (intervals negated).
    pub fn invert(&self) -> Motif {
        let pivot = self.degrees.first().copied().unwrap_or(0);
        Motif {
            id: self.id,
            degrees: self.degrees.iter().map(|d| pivot - (d - pivot)).collect(),
            rhythm: self.rhythm.clone(),
        }
    }

    /// Reverse both contour and rhythm.
    pub fn retrograde(&self) -> Motif {
        let mut degrees = self.degrees.clone();
        degrees.reverse();
        let mut rhythm = self.rhythm.clone();
        rhythm.reverse();
        Motif {
            id: self.id,
            degrees,
            rhythm,
        }
    }

    /// Scale the rhythm by `factor` (>1 augments/slows, <1 diminishes/quickens).
    pub fn scale_rhythm(&self, factor: f32) -> Motif {
        let f = factor.max(0.05);
        Motif {
            id: self.id,
            degrees: self.degrees.clone(),
            rhythm: self.rhythm.iter().map(|r| r * f).collect(),
        }
    }

    /// Take the first `take` notes (a fragment).
    pub fn fragment(&self, take: usize) -> Motif {
        let n = take.clamp(1, self.len().max(1));
        Motif {
            id: self.id,
            degrees: self.degrees.iter().take(n).copied().collect(),
            rhythm: self.rhythm.iter().take(n).copied().collect(),
        }
    }

    /// Build a sequence: `times` copies, each transposed a further `step` degrees.
    pub fn sequence(&self, step: i32, times: usize) -> Motif {
        let mut degrees = Vec::new();
        let mut rhythm = Vec::new();
        for k in 0..times.max(1) {
            for (i, d) in self.degrees.iter().enumerate() {
                degrees.push(d + step * k as i32);
                rhythm.push(self.rhythm[i]);
            }
        }
        Motif {
            id: self.id,
            degrees,
            rhythm,
        }
    }

    /// Realize the motif to `(start_beat, dur_beats, pitch)` notes on `scale`, with each
    /// degree offset from `root_degree` at `octave`, beginning at `start_beat`.
    pub fn render(
        &self,
        scale: &Scale,
        root_degree: i32,
        octave: i32,
        start_beat: f64,
    ) -> Vec<(f64, f32, Midi)> {
        let mut out = Vec::with_capacity(self.len());
        let mut t = start_beat;
        for (i, &deg) in self.degrees.iter().enumerate() {
            let dur = self.rhythm[i];
            let pitch = scale.degree_pitch(root_degree + deg, octave);
            out.push((t, dur, pitch));
            t += dur as f64;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::super::theory::Mode;
    use super::*;

    #[test]
    fn transformations_preserve_identity() {
        let m = Motif::seed_a();
        assert_eq!(m.transpose(3).id, m.id);
        assert_eq!(m.invert().id, m.id);
        assert_eq!(m.retrograde().id, m.id);
        assert_eq!(m.scale_rhythm(2.0).id, m.id);
        assert_eq!(m.fragment(2).id, m.id);
        assert_eq!(m.sequence(2, 3).id, m.id);
    }

    #[test]
    fn transpose_shifts_all_degrees() {
        let m = Motif::seed_a();
        let t = m.transpose(2);
        assert_eq!(t.degrees, vec![2, 4, 6, 5]);
        assert_eq!(t.rhythm, m.rhythm); // rhythm unchanged
    }

    #[test]
    fn invert_negates_intervals_about_first() {
        let m = Motif {
            id: 9,
            degrees: vec![0, 2, 4],
            rhythm: vec![1.0, 1.0, 1.0],
        };
        // pivot 0: 0-> 0, 2-> -2, 4-> -4.
        assert_eq!(m.invert().degrees, vec![0, -2, -4]);
    }

    #[test]
    fn augment_doubles_duration_diminish_halves() {
        let m = Motif::seed_a();
        assert!((m.scale_rhythm(2.0).total_beats() - m.total_beats() * 2.0).abs() < 1e-5);
        assert!((m.scale_rhythm(0.5).total_beats() - m.total_beats() * 0.5).abs() < 1e-5);
    }

    #[test]
    fn sequence_length_and_transposition() {
        let m = Motif::seed_a();
        let s = m.sequence(2, 3);
        assert_eq!(s.len(), m.len() * 3);
        // Third copy's first degree is original + 2*2.
        assert_eq!(s.degrees[m.len() * 2], m.degrees[0] + 4);
    }

    #[test]
    fn render_places_notes_in_time_and_pitch() {
        let m = Motif::seed_a();
        let s = Scale::new(0, Mode::Ionian);
        let notes = m.render(&s, 0, 4, 8.0);
        assert_eq!(notes.len(), 4);
        assert_eq!(notes[0].0, 8.0); // first at start
        assert_eq!(notes[0].2, 60); // degree 0 -> C4
        assert_eq!(notes[1].0, 9.0); // after 1 beat
        assert_eq!(notes[1].2, 64); // degree 2 -> E4
        assert_eq!(notes[2].2, 67); // degree 4 -> G4 (0-based scale degree 4 = 5th tone)
    }
}
