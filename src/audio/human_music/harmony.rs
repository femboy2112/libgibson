//! The harmonic engine: generate a functional chord progression over a [`Form`],
//! constrained by the world's vocabulary and steered by the form's tension curve.
//!
//! Diatonic chords are derived from the scale (quality classified from the actual stacked
//! scale thirds, so it is correct in any mode). Function (tonic / pre-dominant / dominant)
//! follows the tension target; phrase ends cadence to tonic. Richer worlds may colour a
//! chord with a secondary dominant, modal mixture, or a chromatic mediant.

use super::form::{Form, BEATS_PER_BAR};
use super::rng::Rng;
use super::theory::{Chord, Function, Quality, Scale};
use super::world::MusicWorld;

/// A chord placed in time with its analysis.
#[derive(Debug, Clone, Copy)]
pub struct ChordSpan {
    pub start_beat: f64,
    pub dur_beats: f32,
    pub chord: Chord,
    pub function: Function,
    /// The scale degree (0-based) the chord is built on, or -1 for a borrowed/applied chord.
    pub degree: i32,
    /// A short provenance note (e.g. "V/ii", "bVI mix").
    pub note: &'static str,
}

/// Generates progressions for a world.
pub struct HarmonyEngine {
    scale: Scale,
    world_use_sevenths: bool,
    allow_secondary: bool,
    allow_mixture: bool,
    allow_chromatic_mediant: bool,
    rng: Rng,
}

impl HarmonyEngine {
    /// A harmony engine for `world`, seeded by `seed`.
    pub fn new(world: &MusicWorld, seed: u64) -> HarmonyEngine {
        HarmonyEngine {
            scale: Scale::new(world.tonic_pc, world.mode),
            world_use_sevenths: world.use_sevenths,
            allow_secondary: world.allow_secondary_dominant,
            allow_mixture: world.allow_modal_mixture,
            allow_chromatic_mediant: world.allow_chromatic_mediant,
            rng: Rng::new(seed ^ 0xC0FF_EE00),
        }
    }

    /// The scale this engine works in.
    pub fn scale(&self) -> Scale {
        self.scale
    }

    /// Build a diatonic chord on scale `degree` (0-based), a seventh if requested.
    pub fn diatonic_chord(&self, degree: i32, seventh: bool) -> Chord {
        let root = self.scale.degree_pitch(degree, 4);
        let third = self.scale.degree_pitch(degree + 2, 4);
        let fifth = self.scale.degree_pitch(degree + 4, 4);
        let seventh_p = self.scale.degree_pitch(degree + 6, 4);
        let t = third - root;
        let f = fifth - root;
        let s = seventh_p - root;
        let quality = classify(t, f, s, seventh);
        Chord::new(root.rem_euclid(12), quality)
    }

    /// Generate the full progression over `form`.
    pub fn generate(&mut self, form: &Form) -> Vec<ChordSpan> {
        let total_beats = form.total_bars as f64 * BEATS_PER_BAR;
        let mut spans = Vec::new();
        let mut beat = 0.0f64;
        let mut prev_degree: i32 = 0;

        while beat < total_beats - 1e-6 {
            let bar_f = beat / BEATS_PER_BAR;
            let tension = form.tension_at(bar_f);
            let density = form.density_at(bar_f);
            // Never let the last chord spill past the end of the piece.
            let dur = chord_dur(density).min((total_beats - beat) as f32);

            // Is this the last chord of a section? -> cadence to tonic.
            let sec = form.section_at_bar(bar_f as u32);
            let sec_end_beat = sec.end_bar() as f64 * BEATS_PER_BAR;
            let cadence = beat + dur as f64 >= sec_end_beat - 1e-6;

            let (degree, function) = if cadence {
                (0, Function::Tonic)
            } else {
                let func = function_for_tension(tension);
                (self.pick_degree(func, prev_degree), func)
            };

            let seventh =
                self.world_use_sevenths && (tension > 0.4 || function == Function::Dominant);
            let mut chord = self.diatonic_chord(degree, seventh);
            let mut note = "";

            // Embellishments (world-gated), never at a cadence.
            if !cadence {
                if self.allow_secondary && function == Function::Dominant && self.rng.chance(0.25) {
                    // Secondary dominant of the next tonic-ish target (V/target).
                    let target = self.pick_degree(Function::Tonic, degree);
                    let target_root = self.scale.degree_pitch(target, 4);
                    let dom_root = (target_root + 7).rem_euclid(12);
                    chord = Chord::new(dom_root, Quality::Dom7);
                    note = "V/of";
                } else if self.allow_mixture && tension > 0.6 && self.rng.chance(0.2) {
                    // Borrowed bVI (modal mixture) for a dark lift.
                    let bvi = (self.scale.tonic_pc + 8).rem_euclid(12);
                    chord = Chord::new(
                        bvi,
                        if self.world_use_sevenths {
                            Quality::Maj7
                        } else {
                            Quality::Maj
                        },
                    );
                    note = "bVI mix";
                } else if self.allow_chromatic_mediant && self.rng.chance(0.12) {
                    // Chromatic mediant: major third above tonic.
                    let cm = (self.scale.tonic_pc + 4).rem_euclid(12);
                    chord = Chord::new(cm, Quality::Maj);
                    note = "chr med";
                }
            }

            spans.push(ChordSpan {
                start_beat: beat,
                dur_beats: dur,
                chord,
                function,
                degree: if note.is_empty() { degree } else { -1 },
                note,
            });
            prev_degree = degree;
            beat += dur as f64;
        }
        spans
    }

    fn pick_degree(&mut self, func: Function, prev: i32) -> i32 {
        let choices: &[i32] = match func {
            Function::Tonic => &[0, 5, 2],
            Function::Predominant => &[3, 1],
            Function::Dominant => &[4, 6],
        };
        // Prefer not to repeat the previous degree.
        let filtered: Vec<i32> = choices.iter().copied().filter(|&d| d != prev).collect();
        let pool = if filtered.is_empty() {
            choices
        } else {
            &filtered
        };
        *self.rng.pick(pool).unwrap_or(&0)
    }
}

/// Chord duration in beats from a density target.
fn chord_dur(density: f32) -> f32 {
    if density < 0.4 {
        (BEATS_PER_BAR * 2.0) as f32 // one chord every two bars
    } else if density < 0.72 {
        BEATS_PER_BAR as f32 // one per bar
    } else {
        (BEATS_PER_BAR / 2.0) as f32 // two per bar
    }
}

/// Harmonic function targeted by a tension level.
fn function_for_tension(t: f32) -> Function {
    if t < 0.35 {
        Function::Tonic
    } else if t < 0.62 {
        Function::Predominant
    } else {
        Function::Dominant
    }
}

/// Classify a chord quality from stacked-third semitone intervals.
fn classify(third: i32, fifth: i32, seventh: i32, use_seventh: bool) -> Quality {
    let t = third.rem_euclid(12);
    let f = fifth.rem_euclid(12);
    let s = seventh.rem_euclid(12);
    if !use_seventh {
        return match (t, f) {
            (4, 7) => Quality::Maj,
            (3, 7) => Quality::Min,
            (3, 6) => Quality::Dim,
            (4, 8) => Quality::Aug,
            _ => Quality::Maj,
        };
    }
    match (t, f, s) {
        (4, 7, 11) => Quality::Maj7,
        (4, 7, 10) => Quality::Dom7,
        (3, 7, 10) => Quality::Min7,
        (3, 7, 11) => Quality::MinMaj7,
        (3, 6, 10) => Quality::Min7b5,
        (3, 6, 9) => Quality::Dim7,
        (4, 8, _) => Quality::Aug,
        _ => Quality::Maj7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::human_music::semantic::demo_trace;

    #[test]
    fn c_major_diatonic_chords_are_correct() {
        let w = MusicWorld::swiss_signal(); // C Ionian, triads
        let h = HarmonyEngine::new(&w, 1);
        // I = C major, ii = D minor, V = G major, vii° = B diminished.
        assert_eq!(h.diatonic_chord(0, false), Chord::new(0, Quality::Maj));
        assert_eq!(h.diatonic_chord(1, false), Chord::new(2, Quality::Min));
        assert_eq!(h.diatonic_chord(4, false), Chord::new(7, Quality::Maj));
        assert_eq!(h.diatonic_chord(6, false), Chord::new(11, Quality::Dim));
    }

    #[test]
    fn c_major_sevenths() {
        let mut w = MusicWorld::swiss_signal();
        w.use_sevenths = true;
        let h = HarmonyEngine::new(&w, 1);
        // Imaj7, ii m7, V7.
        assert_eq!(h.diatonic_chord(0, true), Chord::new(0, Quality::Maj7));
        assert_eq!(h.diatonic_chord(1, true), Chord::new(2, Quality::Min7));
        assert_eq!(h.diatonic_chord(4, true), Chord::new(7, Quality::Dom7));
    }

    #[test]
    fn progression_covers_form_and_ends_on_tonic() {
        let form = Form::from_trace(&demo_trace(120.0));
        let mut h = HarmonyEngine::new(&MusicWorld::black_ice(), 42);
        let prog = h.generate(&form);
        assert!(!prog.is_empty());
        // Contiguous in time.
        for w in prog.windows(2) {
            assert!((w[1].start_beat - (w[0].start_beat + w[0].dur_beats as f64)).abs() < 1e-3);
        }
        // The final chord is a tonic cadence.
        let last = prog.last().unwrap();
        assert_eq!(last.function, Function::Tonic);
        assert_eq!(last.chord.root_pc, 9); // A minor tonic
    }

    #[test]
    fn deterministic_progression_for_seed() {
        let form = Form::from_trace(&demo_trace(120.0));
        let mut a = HarmonyEngine::new(&MusicWorld::vapor95(), 7);
        let mut b = HarmonyEngine::new(&MusicWorld::vapor95(), 7);
        let pa = a.generate(&form);
        let pb = b.generate(&form);
        assert_eq!(pa.len(), pb.len());
        for (x, y) in pa.iter().zip(pb.iter()) {
            assert_eq!(x.chord, y.chord);
            assert_eq!(x.start_beat, y.start_beat);
        }
    }
}
