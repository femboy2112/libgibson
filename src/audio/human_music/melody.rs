//! The **lead** — one projection of the [`PerformancePlan`]: it realizes the planned
//! [`super::performance::LeadStatement`]s (their motif, handoff and lawfully varied entry points,
//! including entries that answer another player's figure) against the performance's harmony.

use super::discourse::DiscourseRole;
use super::performance::PerformancePlan;
use super::plan::CompositionPlan;
use super::score::{Note, Provenance, Role};
use super::theory::Midi;

/// The realized lead line plus how many notes the search had to repair.
pub struct LeadRealization {
    pub notes: Vec<Note>,
    pub repairs: usize,
}

/// Realize every planned lead statement, connecting each to the previous statement's exit pitch.
pub fn realize_lead(perf: &PerformancePlan, _plan: &CompositionPlan) -> LeadRealization {
    let scale = perf.region;
    let mut notes = Vec::new();
    let mut repairs = 0usize;
    let mut prev_exit: Option<Midi> = None;
    for st in &perf.statements {
        let motif = &st.motif;
        // Register realizes the role: the culmination and intensification climb, the dissolve
        // settles low, everything else follows the phrase's elevation target — then, for ordinary
        // roles, the octave nudges to CONNECT to the previous statement's exit.
        let base_octave = match st.role {
            DiscourseRole::Culminate | DiscourseRole::Intensify => 5,
            DiscourseRole::Dissolve => 3,
            _ => 4 + (st.register > 0.65) as i32,
        };
        let octave = match prev_exit {
            Some(pe) if !st.is_rupture && st.role != DiscourseRole::Culminate => {
                let first_deg = motif.degrees.first().copied().unwrap_or(0);
                [base_octave - 1, base_octave, base_octave + 1]
                    .into_iter()
                    .min_by_key(|&o| (scale.degree_pitch(first_deg, o) - pe).abs())
                    .unwrap_or(base_octave)
            }
            _ => base_octave,
        };
        let (realized, reps) = super::motif::realize_phrase_reporting(
            motif,
            &perf.chords,
            &scale,
            0,
            octave,
            st.start_beat,
            prev_exit,
            4,
        );
        repairs += reps;
        if let Some(&(_, _, last_pitch, _)) = realized.last() {
            prev_exit = Some(last_pitch);
        }
        for (nb, dur, pitch, function) in realized {
            let mut note = Note::new(
                nb,
                (dur * 0.9).max(0.1),
                pitch,
                (0.55 + 0.4 * st.energy).clamp(0.1, 1.0),
                Role::Lead,
                Provenance {
                    motif_id: Some(motif.id),
                    motif_xform: Some(st.handoff.label()),
                    anchor: Some("motif"),
                    role_note: "melody",
                    ..Provenance::new(super::form::SectionKind::A)
                },
            );
            note.function = function;
            notes.push(note);
        }
    }
    // A held note lifts off when the harmony moves under it (the realizer's classification assumed
    // it would; this makes the sounding body agree).
    super::comp::release_at_harmony_change(&mut notes, &perf.chords);
    // A note the classifier rejected ONLY because its sustain crossed into foreign harmony is a
    // chord tone once released: re-judge it against its onset harmony.
    for n in notes.iter_mut().filter(|n| n.function.is_none()) {
        let Some(ctx) = perf.context_at(n.start_beat) else {
            continue;
        };
        let pc = super::theory::pitch_class(n.pitch);
        let end = n.start_beat + n.dur_beats as f64;
        let boundary = ctx.start_beat + ctx.dur_beats as f64;
        if ctx.chord.contains_pc(pc) {
            if end > boundary + 1e-6 {
                n.dur_beats = ((boundary - n.start_beat) as f32 * 0.97).max(0.1);
            }
            n.function = Some(super::score::PitchFunction::ChordTone);
        }
    }
    LeadRealization { notes, repairs }
}
