//! **HarmonicEnsembleState** — the band's one harmonic state, as a ledger the players realize
//! against (Round VIII).
//!
//! R7b's players shared material identity but each chose its pitches alone: the lead against the
//! chord, the keys against the chord (hearing the lead softly), the pad and the bass against the
//! chord and nobody. The union was never evaluated, so four lawful interpretations of one harmony
//! sounded at once. Here every pitched event is committed to one ledger in RIGIDITY order — the lead
//! (the first mover; target-first, never re-pitched), the bass (the floor), the stamped material lines
//! (answers, figures, unisons: pitch free within their contour), then the pad-and-keys bed solved
//! jointly — and each later choice asks the ledger what is already sounding. Every question is
//! answered by the ONE vertical theory in [`super::sonority`], so the planner and the audit cannot
//! mean different things by "unowned collision".
//!
//! Nothing is deleted after the fact (R7b's single Stage authority): a candidate that would create a
//! hard vertical problem is re-voiced or refused AT EMISSION, only when nothing pins it, and every
//! such decision is logged with its reason.

use super::context::HarmonicContext;
use super::score::{Note, PitchFunction, Role};
use super::sonority::{
    classify_clash, core_pcs, identity_pcs, is_suspension, low_interval_limit, settle_resolutions,
    BassFunction, Clash, ColorPolicy, SonorityPlan, VerticalClass, Voice, LINEAR_MAX_BEATS,
    MIN_OVERLAP_BEATS,
};
use super::theory::{note_name, pitch_class, Midi};

/// One explained decision the ledger made (a re-voicing, a refusal, a substitution).
#[derive(Debug, Clone, PartialEq)]
pub struct VerticalDecision {
    pub beat: f64,
    pub role: Role,
    /// What happened: `"revoiced"`, `"refused"`, `"substituted"`, `"octave"`, `"held"` (a note
    /// sustained through a moving line instead of re-attacking), `"owned"` (a non-root floor an
    /// action owns), `"kept"` (a hazard with no clean alternative on a note an action pins).
    pub what: &'static str,
    /// Why, naming the notes.
    pub reason: String,
}

/// A hard vertical problem a candidate would create against what already sounds.
#[derive(Debug, Clone, PartialEq)]
pub enum Hazard {
    /// An unowned minor 2nd / minor 9th against a committed voice.
    Clash {
        clash: Clash,
        with: Role,
        pitch: Midi,
    },
    /// The candidate is the floor and names an extension or non-chord tone nobody planned.
    Floor { function: BassFunction },
    /// Below the low-interval limit against a committed voice.
    Mud {
        interval: i32,
        with: Role,
        pitch: Midi,
    },
}

impl Hazard {
    /// A short human reason.
    pub fn describe(&self, cand: Midi) -> String {
        match self {
            Hazard::Clash { clash, with, pitch } => format!(
                "{} against {} {} = unowned {}",
                note_name(cand),
                with.label(),
                note_name(*pitch),
                clash.label()
            ),
            Hazard::Floor { function } => {
                format!("{} as the floor = {}", note_name(cand), function.label())
            }
            Hazard::Mud {
                interval,
                with,
                pitch,
            } => format!(
                "{} against {} {} = {interval} semitones under the low-interval limit",
                note_name(cand),
                with.label(),
                note_name(*pitch)
            ),
        }
    }

    /// Whether this hazard is a hard rejection (clashes and floors) rather than a soft cost (mud).
    pub fn is_hard(&self) -> bool {
        !matches!(self, Hazard::Mud { .. })
    }
}

/// The ledger: every committed pitched voice, the harmonic contexts, the per-harmony allocations.
#[derive(Debug, Clone)]
pub struct HarmonicEnsembleState<'a> {
    contexts: &'a [HarmonicContext],
    plans: Vec<SonorityPlan>,
    policy: ColorPolicy,
    /// Committed voices, kept sorted by (start, role, pitch).
    voices: Vec<Voice>,
    log: Vec<VerticalDecision>,
}

impl<'a> HarmonicEnsembleState<'a> {
    /// An empty ledger over `contexts` with the per-harmony `plans` and the world/language `policy`.
    pub fn new(
        contexts: &'a [HarmonicContext],
        plans: Vec<SonorityPlan>,
        policy: ColorPolicy,
    ) -> HarmonicEnsembleState<'a> {
        HarmonicEnsembleState {
            contexts,
            plans,
            policy,
            voices: Vec::new(),
            log: Vec::new(),
        }
    }

    /// The harmonic contexts the ledger judges against.
    pub fn contexts(&self) -> &'a [HarmonicContext] {
        self.contexts
    }

    /// The per-harmony allocations.
    pub fn plans(&self) -> &[SonorityPlan] {
        &self.plans
    }

    /// The colour/density policy.
    pub fn policy(&self) -> &ColorPolicy {
        &self.policy
    }

    /// Every committed voice.
    pub fn voices(&self) -> &[Voice] {
        &self.voices
    }

    /// The explained decisions so far.
    pub fn log(&self) -> &[VerticalDecision] {
        &self.log
    }

    /// Record an explained decision.
    pub fn record(&mut self, beat: f64, role: Role, what: &'static str, reason: String) {
        self.log.push(VerticalDecision {
            beat,
            role,
            what,
            reason,
        });
    }

    /// The index of the harmony sounding at `beat`.
    pub fn context_index(&self, beat: f64) -> Option<usize> {
        self.contexts
            .iter()
            .rposition(|c| c.start_beat <= beat + 1e-6)
    }

    /// The harmony sounding at `beat`.
    pub fn context_at(&self, beat: f64) -> Option<&'a HarmonicContext> {
        self.context_index(beat).map(|i| &self.contexts[i])
    }

    /// The allocation of the harmony sounding at `beat`.
    pub fn plan_at(&self, beat: f64) -> Option<&SonorityPlan> {
        let ci = self.context_index(beat)?;
        self.plans.iter().find(|p| p.context == ci)
    }

    /// Commit a finished line of notes (one player's, or several). Linear and suspended notes get
    /// their resolution flag from what the same player plays next INSIDE the committed notes.
    pub fn commit(&mut self, notes: &[Note]) {
        let mut add: Vec<Voice> = notes.iter().map(voice_of).collect();
        add.sort_by(order);
        settle_resolutions(&mut add, self.contexts);
        self.voices.extend(add);
        self.voices.sort_by(order);
    }

    /// Committed voices sounding (non-trivially) inside `[a, b)`.
    pub fn sounding(&self, a: f64, b: f64) -> impl Iterator<Item = &Voice> + '_ {
        self.voices
            .iter()
            .filter(move |v| v.start < b - 1e-6 && v.end > a + 1e-6)
    }

    /// Committed voices of `role` sounding inside `[a, b)`.
    pub fn sounding_role(&self, role: Role, a: f64, b: f64) -> impl Iterator<Item = &Voice> + '_ {
        self.sounding(a, b).filter(move |v| v.role == role)
    }

    /// Every hazard `cand` would create against the committed voices it overlaps by at least
    /// [`MIN_OVERLAP_BEATS`], judged by [`classify_clash`] over the harmony at each overlap's start
    /// (so a note crossing a chord change is judged in both), plus — for a bass candidate — its floor
    /// function. The SAME rules the audit applies to the finished Score.
    pub fn hazards(&self, cand: &Voice) -> Vec<Hazard> {
        let mut out = Vec::new();
        for v in self.sounding(cand.start, cand.end) {
            let a = cand.start.max(v.start);
            let b = cand.end.min(v.end);
            if b - a < MIN_OVERLAP_BEATS - 1e-9 || v.pitch == cand.pitch {
                continue;
            }
            let Some(ci) = self.context_index(a) else {
                continue;
            };
            let ctx = &self.contexts[ci];
            let plan = self.plans.iter().find(|p| p.context == ci);
            if let Some(clash) = Clash::of(cand.pitch, v.pitch) {
                if classify_clash(ctx, cand, v, plan) == VerticalClass::UnownedCollision {
                    out.push(Hazard::Clash {
                        clash,
                        with: v.role,
                        pitch: v.pitch,
                    });
                }
            }
            let (lo, hi) = if cand.pitch < v.pitch {
                (cand.pitch, v.pitch)
            } else {
                (v.pitch, cand.pitch)
            };
            if let Some(limit) = low_interval_limit(hi - lo) {
                if lo < limit && !cand.is_linear() && !v.is_linear() {
                    out.push(Hazard::Mud {
                        interval: hi - lo,
                        with: v.role,
                        pitch: v.pitch,
                    });
                }
            }
        }
        if cand.role == Role::Bass {
            if let Some(ctx) = self.context_at(cand.start) {
                let f = BassFunction::of(&ctx.chord, cand.pitch);
                // The audit's rule exactly: a linear floor owns its function only by being short
                // AND resolving (`candidate` assumes it resolves; a caller that knows better says so).
                let owned =
                    (cand.is_linear() && cand.dur() <= LINEAR_MAX_BEATS + 1e-9 && cand.resolves)
                        || cand.function == Some(PitchFunction::PedalTone)
                        || self
                            .plan_at(cand.start)
                            .is_some_and(|p| p.bass_pc == pitch_class(cand.pitch));
                if matches!(f, BassFunction::Tension(_) | BassFunction::NonChord) && !owned {
                    out.push(Hazard::Floor { function: f });
                }
            }
        }
        out
    }

    /// Only the hard hazards of `cand`.
    pub fn hard_hazards(&self, cand: &Voice) -> Vec<Hazard> {
        self.hazards(cand)
            .into_iter()
            .filter(|h| h.is_hard())
            .collect()
    }

    /// The colour pitch classes already sounding (resting, non-core) in `[a, b)` over the harmony
    /// at `a`, with the roles that own each.
    pub fn colors_in(&self, a: f64, b: f64) -> Vec<(i32, Vec<Role>)> {
        let Some(ctx) = self.context_at(a) else {
            return Vec::new();
        };
        let core = core_pcs(&ctx.chord);
        let mut out: Vec<(i32, Vec<Role>)> = Vec::new();
        for v in self.sounding(a, b) {
            if v.is_linear() || v.function.is_some_and(is_suspension) {
                continue;
            }
            let pc = pitch_class(v.pitch);
            if core.contains(&pc) {
                continue;
            }
            match out.iter_mut().find(|x| x.0 == pc) {
                Some(x) => {
                    if !x.1.contains(&v.role) {
                        x.1.push(v.role);
                    }
                }
                None => out.push((pc, vec![v.role])),
            }
        }
        out.sort_by_key(|x| x.0);
        out
    }

    /// Whether the committed voices in `[a, b)` already contain every identity tone of the harmony
    /// at `a` (so a support player may complement rather than respell the chord).
    pub fn identity_covered(&self, a: f64, b: f64) -> bool {
        let Some(ctx) = self.context_at(a) else {
            return false;
        };
        identity_pcs(&ctx.chord).iter().all(|&pc| {
            self.sounding(a, b)
                .any(|v| pitch_class(v.pitch) == pc && !v.is_linear())
        })
    }
}

/// A Score note as a [`Voice`] (resolution to be computed by the caller).
pub fn voice_of(n: &Note) -> Voice {
    Voice {
        role: n.role,
        pitch: n.pitch,
        start: n.start_beat,
        end: n.start_beat + n.dur_beats as f64,
        function: n.function,
        tag: n.prov.role_note,
        resolves: true,
        resolves_to: None,
        unison: n.prov.motif_xform == Some("unison") || n.prov.role_note == "unison",
        written_end: n.start_beat + n.dur_beats as f64,
        sfx: false,
        owned: false,
    }
}

/// A candidate voice: `role` sounding `pitch` over `[start, end)` with `function` and `tag`.
pub fn candidate(
    role: Role,
    pitch: Midi,
    start: f64,
    end: f64,
    function: Option<PitchFunction>,
    tag: &'static str,
) -> Voice {
    Voice {
        role,
        pitch,
        start,
        end,
        function,
        tag,
        resolves: true,
        resolves_to: None,
        unison: false,
        written_end: end,
        sfx: false,
        owned: false,
    }
}

fn order(a: &Voice, b: &Voice) -> std::cmp::Ordering {
    a.start
        .total_cmp(&b.start)
        .then(a.role.label().cmp(b.role.label()))
        .then(a.pitch.cmp(&b.pitch))
}

#[cfg(test)]
mod tests {
    use super::super::context::analyze;
    use super::super::harmony::ChordSpan;
    use super::super::score::Provenance;
    use super::super::theory::{Chord, Mode, Quality, Scale};
    use super::*;

    fn ctxs() -> Vec<HarmonicContext> {
        let spans = vec![ChordSpan::test(0.0, 4.0, Chord::new(0, Quality::Maj7))];
        analyze(&spans, &Scale::new(0, Mode::Ionian))
    }

    fn note(role: Role, pitch: Midi, start: f64, dur: f32, f: PitchFunction) -> Note {
        let mut n = Note::new(
            start,
            dur,
            pitch,
            0.8,
            role,
            Provenance::new(super::super::form::SectionKind::A),
        );
        n.function = Some(f);
        n
    }

    #[test]
    fn the_ledger_refuses_what_the_audit_would_call_garbage() {
        let c = ctxs();
        let mut st = HarmonicEnsembleState::new(&c, Vec::new(), ColorPolicy::lenient());
        st.commit(&[note(Role::Lead, 72, 0.0, 2.0, PitchFunction::ChordTone)]);
        // A pad B4 under the lead's C5: an unowned minor 2nd.
        let b4 = candidate(
            Role::Pad,
            71,
            0.0,
            4.0,
            Some(PitchFunction::ChordTone),
            "pad",
        );
        assert!(matches!(
            st.hard_hazards(&b4).as_slice(),
            [Hazard::Clash {
                clash: Clash::MinorSecond,
                with: Role::Lead,
                ..
            }]
        ));
        // The B an octave lower is a minor 9th under the C (13 semitones) — still unowned: with the
        // root in the melody, Cmaj7's 7th must sit ABOVE it or leave; the 6th (A4) is clean.
        let b3 = candidate(
            Role::Pad,
            59,
            0.0,
            4.0,
            Some(PitchFunction::ChordTone),
            "pad",
        );
        assert!(matches!(
            st.hard_hazards(&b3).as_slice(),
            [Hazard::Clash {
                clash: Clash::MinorNinth,
                ..
            }]
        ));
        let a4 = candidate(
            Role::Pad,
            69,
            0.0,
            4.0,
            Some(PitchFunction::LicensedExtension),
            "pad",
        );
        assert!(st.hard_hazards(&a4).is_empty());
        // A bass D (a 9th) as the floor: refused.
        let d2 = candidate(
            Role::Bass,
            38,
            0.0,
            1.0,
            Some(PitchFunction::ChordTone),
            "unison",
        );
        assert!(st
            .hard_hazards(&d2)
            .iter()
            .any(|h| matches!(h, Hazard::Floor { .. })));
    }

    #[test]
    fn colours_and_identity_are_read_from_what_sounds() {
        let c = ctxs();
        let mut st = HarmonicEnsembleState::new(&c, Vec::new(), ColorPolicy::lenient());
        st.commit(&[
            note(Role::Keys, 64, 0.0, 4.0, PitchFunction::ChordTone),
            note(Role::Keys, 71, 0.0, 4.0, PitchFunction::ChordTone),
            note(Role::Lead, 74, 0.0, 4.0, PitchFunction::LicensedExtension),
        ]);
        assert!(
            st.identity_covered(0.0, 4.0),
            "E and B are Cmaj7's identity"
        );
        assert_eq!(st.colors_in(0.0, 4.0), vec![(2, vec![Role::Lead])]);
    }
}
