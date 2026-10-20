//! Independent content-ownership witness for the riff-driven keys COMP.
//!
//! The carrier witness ([`super::argument_witness`]) deliberately EXCLUDES comp notes
//! (`role_note == "comp"`) so a Keys-carried phrase can coexist with polyphonic comp stabs. That
//! leaves the comp itself unwitnessed. This module is the independent, bounded witness for exactly
//! those notes, so the two never cover for each other.
//!
//! The ownership relation is `RiffOnset -> CompGesture -> ChordNoteEvents`: one riff onset may emit a
//! lean guide-tone chord (one to three notes), and that is lawful ONLY if every emitted note belongs
//! to a real riff onset's slot, sits at the pocket time the lock law demands, and is a GUIDE TONE of
//! the harmony sounding at that slot's bar. The witness reconstructs the legitimate slots from the
//! source riff (`comp_onsets`), the form, and the typed `comp_attack` lock — never from the kernel's
//! emission — and reconstructs the ownership set (3rd / 7th / 9th) from the Score's own chords. It is
//! a membership check on independent evidence, not a copy of the compiler's voicing branch, so a
//! ghost chord, an unauthorised displacement, a wrong chord, a forged `comp` label, a crossed barline
//! or a wholly omitted onset each break it. Inert unless `comp_onsets` is set (C137 never sets it).

use super::argument::CompiledArgument;
use super::score::{Role, Score};
use super::theme_family::pocket_offset;
use super::theory::Chord;

const EPS: f64 = 1e-4;
const EPISODE_BEATS: f64 = 16.0;

/// A comp-ownership failure located at a performed beat (or, for an omitted gesture, the riff onset).
#[derive(Debug, Clone, PartialEq)]
pub struct CompViolation {
    pub at: f64,
    pub reason: &'static str,
}

/// Comp content ownership, reported separately from the carrier witness and from any listening.
#[derive(Debug, Clone, PartialEq)]
pub struct CompWitness {
    violations: Vec<CompViolation>,
    gestures: usize,
    notes: usize,
}

impl CompWitness {
    pub fn valid(&self) -> bool {
        self.violations.is_empty()
    }
    pub fn violations(&self) -> &[CompViolation] {
        &self.violations
    }
    /// Distinct comp slots (chord attacks) observed.
    pub fn gesture_count(&self) -> usize {
        self.gestures
    }
    /// Total comp notes observed.
    pub fn note_count(&self) -> usize {
        self.notes
    }
}

/// A legitimate comp slot the source owns: the performed time, the grid bar whose harmony it reads,
/// and the riff onset it descends from.
struct Slot {
    at: f64,
    grid_bar: f64,
    onset: f64,
}

/// Observe the comp gesture's content ownership against the frozen Score. Empty (valid) when no comp
/// gesture is declared.
pub fn observe_comp(compiled: &CompiledArgument, score: &Score) -> CompWitness {
    let mut violations = Vec::new();
    let Some(onsets) = compiled.comp_onsets.as_ref() else {
        return CompWitness {
            violations,
            gestures: 0,
            notes: 0,
        };
    };
    let bpm = f64::from(score.tempo_bpm);
    let displacement = compiled.comp_attack.eval_beats(bpm);
    let episodes = (compiled.song.plan.form.total_beats / EPISODE_BEATS)
        .round()
        .max(0.0) as usize;

    // Reconstruct the legitimate slots from the riff + the lock law (never from the emission).
    let mut slots: Vec<Slot> = Vec::new();
    for e in 0..episodes {
        for rep in 0..2u32 {
            let rep_base = e as f64 * EPISODE_BEATS + f64::from(rep) * 8.0;
            for &o in onsets {
                if !(0.0..8.0).contains(&o) {
                    continue;
                }
                let posin = o.rem_euclid(4.0);
                let at_grid = rep_base + o;
                let at = (at_grid + pocket_offset(posin) + displacement).max(0.0);
                let grid_bar = (at_grid / 4.0).floor() * 4.0;
                slots.push(Slot {
                    at,
                    grid_bar,
                    onset: o,
                });
            }
        }
    }

    let chord_at = |bar: f64| -> Option<Chord> {
        score
            .chords
            .iter()
            .find(|c| {
                c.start_beat <= bar + EPS && c.start_beat + f64::from(c.dur_beats) > bar + EPS
            })
            .map(|c| c.chord)
    };
    // The comp's ownership set: the chord's guide tones — 3rd, 7th, optional 9th colour — exactly the
    // tones the lean voicing is allowed to name. The root (`tones[0]`) belongs to the bass, never here.
    let guide_pcs = |chord: &Chord| -> Vec<i32> {
        let tones = chord.pitch_classes();
        let mut g = Vec::new();
        if let Some(&third) = tones.get(1) {
            g.push(third.rem_euclid(12));
        }
        if let Some(&seventh) = tones.get(3).or_else(|| tones.get(2)) {
            g.push(seventh.rem_euclid(12));
        }
        if let Some(&ninth) = tones.get(4) {
            g.push(ninth.rem_euclid(12));
        }
        g
    };

    let comps: Vec<_> = score
        .role_notes(Role::Keys)
        .filter(|n| n.prov.role_note == "comp")
        .collect();
    let notes = comps.len();

    // Every comp note must sit at a source-owned slot and be a guide tone of that slot's harmony.
    for n in &comps {
        let Some(slot) = slots.iter().find(|s| (s.at - n.start_beat).abs() < EPS) else {
            violations.push(CompViolation {
                at: n.start_beat,
                reason: "comp note at a beat no riff onset owns (ghost / displacement / crossed barline)",
            });
            continue;
        };
        let pc = n.pitch.rem_euclid(12);
        match chord_at(slot.grid_bar) {
            Some(chord) if guide_pcs(&chord).contains(&pc) => {}
            Some(_) => violations.push(CompViolation {
                at: n.start_beat,
                reason: "comp pitch is not a guide tone of its bar's chord (wrong chord / forged label)",
            }),
            None => violations.push(CompViolation {
                at: n.start_beat,
                reason: "comp note over a bar with no harmony",
            }),
        }
    }

    // Multiplicity: a lean stab is 3rd (+7th (+9th)) — at most three notes. More is an added tone.
    let mut occupied: Vec<(f64, usize)> = Vec::new();
    for n in &comps {
        if let Some(entry) = occupied
            .iter_mut()
            .find(|(a, _)| (*a - n.start_beat).abs() < EPS)
        {
            entry.1 += 1;
        } else {
            occupied.push((n.start_beat, 1));
        }
    }
    for &(at, count) in &occupied {
        if count > 3 {
            violations.push(CompViolation {
                at,
                reason: "comp chord has more than three notes (an added / ghost tone)",
            });
        }
    }
    let gestures = occupied.len();

    // Omitted gesture: a riff onset with a harmonic home somewhere must own at least one comp chord.
    // (A single near-song-end instance may be legitimately dropped by the domain guard; total absence
    // of an onset across every episode is not.)
    for &o in onsets {
        if !(0.0..8.0).contains(&o) {
            continue;
        }
        let onset_slots: Vec<&Slot> = slots.iter().filter(|s| (s.onset - o).abs() < EPS).collect();
        let has_harmonic_home = onset_slots
            .iter()
            .any(|s| chord_at(s.grid_bar).is_some_and(|c| c.pitch_classes().len() >= 2));
        let covered = onset_slots
            .iter()
            .any(|s| comps.iter().any(|n| (n.start_beat - s.at).abs() < EPS));
        if has_harmonic_home && !covered {
            violations.push(CompViolation {
                at: o,
                reason: "a riff onset owns no comp gesture anywhere (omitted source gesture)",
            });
        }
    }

    CompWitness {
        violations,
        gestures,
        notes,
    }
}
