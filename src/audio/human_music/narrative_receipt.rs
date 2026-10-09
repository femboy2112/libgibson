//! **Rick-C137 GEN-STORY-6 — the independent narrative receipt.**
//!
//! A [`NarrativePlan`] DECLARES who carries which meaning and where the lead withholds. This
//! witness refuses to take that declaration as evidence (plan §9): it reads the realized Score and
//! checks the declared obligations are actually BORNE OUT by the notes. It is comparative — it
//! measures the STORY render against the historical BAND control of the same song — so a carriage
//! counts as realized only when the narrative demonstrably CHANGED the performance in the lawful
//! direction, never because the plan said so:
//!
//! * a WITHHOLD is real only if the lead sounds LESS in the phrase under the narrative than in the
//!   control, AND a band carrier still holds the phrase (a silent gap is not a withhold);
//! * a CARRY is real only if the designated band carrier adds material in the phrase it did not add
//!   in the control (the "one extra note" that proves nothing is caught here — §3).
//!
//! The receipt is deliberately able to FAIL, and the mutation controls below prove it does: restore
//! the withheld lead, or strip the band from the withheld phrase, and the witness reports the
//! obligation unrealized for the right reason. It certifies the SYMBOLIC enactment only; whether
//! the carrier is audible in the mix, and whether it sounds like a story, are separate questions
//! (the auditory observer, and the ear).

use super::action::Agent;
use super::meaning::MeaningKind;
use super::narrative::{LeadRole, NarrativePlan};
use super::score::{Role, Score};
use super::song::SongMap;

/// What the realized Score did with one carriage's band obligation — a typed outcome, never a bare
/// pass/fail. `Unrealized` carries the reason the Score did not bear the obligation out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carried {
    /// A band carrier adds material in this phrase that the control did not — the germ was taken up.
    Realized,
    /// The lead stepped aside here and a band carrier held the phrase — the withhold is real.
    WithheldAsPlanned,
    /// The carriage asks nothing of the band: the lead states it alone (not an obligation to meet).
    LeadAlone,
    /// The Score does not bear out the declared obligation, for this reason.
    Unrealized(&'static str),
}

impl Carried {
    /// Whether a declared band obligation was met (a withhold, a carry) — `LeadAlone` is no
    /// obligation and counts as met vacuously; `Unrealized` is the only failure.
    pub fn is_met(self) -> bool {
        !matches!(self, Carried::Unrealized(_))
    }
}

/// One carriage's realization verdict.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CarriageOutcome {
    pub phrase: u32,
    pub meaning: MeaningKind,
    pub carried: Carried,
}

/// The independent receipt for one STORY performance against its BAND control.
#[derive(Debug, Clone, PartialEq)]
pub struct NarrativeReceipt {
    pub outcomes: Vec<CarriageOutcome>,
}

/// The realizer role a narrative [`Agent`] drives, if it is a single pitched voice. `Drums` and
/// `Ensemble` are not single carrier voices and never name a theme carrier here.
fn role_of(a: Agent) -> Option<Role> {
    match a {
        Agent::Lead => Some(Role::Lead),
        Agent::Keys => Some(Role::Keys),
        Agent::Bass => Some(Role::Bass),
        Agent::Pad => Some(Role::Pad),
        Agent::Drums | Agent::Ensemble => None,
    }
}

impl NarrativeReceipt {
    /// Measure the STORY score against the BAND `control` score for the same `song`, checking each
    /// carriage of `plan` (the STORY performance's narrative) against the realized notes.
    pub fn measure(
        song: &SongMap,
        control: &Score,
        story: &Score,
        plan: &NarrativePlan,
    ) -> NarrativeReceipt {
        let window = |phrase: u32| {
            song.plan
                .form
                .phrases
                .iter()
                .find(|p| p.ix == phrase)
                .map(|p| (p.start_beat(), p.end_beat()))
        };
        let count = |score: &Score, role: Role, lo: f64, hi: f64| {
            score
                .notes
                .iter()
                .filter(|n| n.role == role && n.start_beat >= lo - 1e-6 && n.start_beat < hi - 1e-6)
                .count()
        };
        let outcomes = plan
            .carriages
            .iter()
            .map(|c| {
                let carried = match window(c.phrase) {
                    None => Carried::Unrealized("no phrase window for this carriage"),
                    Some((lo, hi)) if c.lead_role == LeadRole::Withheld => {
                        let lead_ctrl = count(control, Role::Lead, lo, hi);
                        let lead_story = count(story, Role::Lead, lo, hi);
                        let band_story =
                            count(story, Role::Keys, lo, hi) + count(story, Role::Bass, lo, hi);
                        if lead_story >= lead_ctrl {
                            Carried::Unrealized("the lead was not actually withheld")
                        } else if band_story == 0 {
                            Carried::Unrealized(
                                "the lead stepped aside but no band held the phrase",
                            )
                        } else {
                            Carried::WithheldAsPlanned
                        }
                    }
                    Some((lo, hi)) if !c.carriers.is_empty() || c.harmonic.is_some() => {
                        // The designated carriers (theme carriers + any harmonic dark-carrier) must
                        // add material here that the control did not — a carrier that plays no more
                        // than it always did has not taken up the germ.
                        let mut roles: Vec<Role> = Vec::new();
                        for a in c
                            .carriers
                            .iter()
                            .copied()
                            .chain(c.harmonic.map(|h| h.carrier))
                        {
                            if let Some(r) = role_of(a) {
                                if !roles.contains(&r) {
                                    roles.push(r);
                                }
                            }
                        }
                        let added: i64 = roles
                            .iter()
                            .map(|&r| {
                                count(story, r, lo, hi) as i64 - count(control, r, lo, hi) as i64
                            })
                            .sum();
                        if added > 0 {
                            Carried::Realized
                        } else {
                            Carried::Unrealized("the carrier added no material to take up the germ")
                        }
                    }
                    Some(_) => Carried::LeadAlone,
                };
                CarriageOutcome {
                    phrase: c.phrase,
                    meaning: c.meaning,
                    carried,
                }
            })
            .collect();
        NarrativeReceipt { outcomes }
    }

    /// Carriages whose declared band obligation the Score did not bear out.
    pub fn unrealized(&self) -> Vec<&CarriageOutcome> {
        self.outcomes
            .iter()
            .filter(|o| !o.carried.is_met())
            .collect()
    }

    /// Carriages that realized a genuine band obligation (a carry or a withhold) — the story the
    /// band actually tells, not the one the plan claims.
    pub fn band_obligations_realized(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|o| matches!(o.carried, Carried::Realized | Carried::WithheldAsPlanned))
            .count()
    }

    /// Whether every declared band obligation is borne out by the Score. A song whose narrative
    /// declares carries the realizer did not make returns `false` — honestly.
    pub fn all_obligations_met(&self) -> bool {
        self.outcomes.iter().all(|o| o.carried.is_met())
    }

    /// A human-readable receipt, per carriage.
    pub fn report(&self) -> String {
        let mut s = format!(
            "NarrativeReceipt: {} carriages, {} band obligations realized, {} unrealized\n",
            self.outcomes.len(),
            self.band_obligations_realized(),
            self.unrealized().len()
        );
        for o in &self.outcomes {
            s.push_str(&format!(
                "  p{:<2} {:<14} {:?}\n",
                o.phrase,
                format!("{:?}", o.meaning),
                o.carried
            ));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::human_music::composer::Composer;
    use crate::audio::human_music::contract::CompositionGrammar;
    use crate::audio::human_music::functor::perform_with_profile;
    use crate::audio::human_music::performance::PerformanceOptions;
    use crate::audio::human_music::policy::{NarrativePolicy, PerformanceProfile};
    use crate::audio::human_music::score::{Note, Score};
    use crate::audio::human_music::semantic::rise_unresolved;
    use crate::audio::human_music::world::MusicWorld;

    const SEED: u64 = 2112;

    /// The canonical withhold fixture + its BAND control and STORY render.
    fn fixture() -> (SongMap, Score, Score, NarrativePlan) {
        let song = SongMap::compose(
            &rise_unresolved(160.0),
            SEED,
            Some(CompositionGrammar::DeflectedLift),
            Composer::MeaningDirected,
        );
        let world = MusicWorld::black_ice();
        let band = perform_with_profile(
            &song,
            &world,
            PerformanceOptions::default(),
            PerformanceProfile::BAND,
        )
        .unwrap();
        let story = perform_with_profile(
            &song,
            &world,
            PerformanceOptions::default(),
            PerformanceProfile::BAND.with_narrative(NarrativePolicy::Ensemble),
        )
        .unwrap();
        let plan = story
            .perf
            .narrative
            .clone()
            .expect("ensemble plans a narrative");
        (song, band.score, story.score, plan)
    }

    /// The withheld phrase's beat window.
    fn withheld_window(song: &SongMap, plan: &NarrativePlan) -> (u32, f64, f64) {
        let c = plan
            .carriages
            .iter()
            .find(|c| c.lead_role == LeadRole::Withheld)
            .expect("fixture withholds");
        let p = song
            .plan
            .form
            .phrases
            .iter()
            .find(|p| p.ix == c.phrase)
            .unwrap();
        (c.phrase, p.start_beat(), p.end_beat())
    }

    #[test]
    fn the_withhold_is_realized_on_the_generated_song() {
        let (song, control, story, plan) = fixture();
        let r = NarrativeReceipt::measure(&song, &control, &story, &plan);
        println!("{}", r.report());
        // The withheld carriage is borne out by the Score.
        let (phrase, _, _) = withheld_window(&song, &plan);
        let held = r.outcomes.iter().find(|o| o.phrase == phrase).unwrap();
        assert_eq!(held.carried, Carried::WithheldAsPlanned);
        assert!(r.band_obligations_realized() >= 1);
    }

    #[test]
    fn restoring_the_withheld_lead_is_caught() {
        let (song, control, story, plan) = fixture();
        let (_, lo, hi) = withheld_window(&song, &plan);
        // MUTATION A: put the control's lead back into the withheld window — the withhold is undone.
        let mut mutated = story.clone();
        mutated.notes.retain(|n| {
            !(n.role == Role::Lead && n.start_beat >= lo - 1e-6 && n.start_beat < hi - 1e-6)
        });
        let restored: Vec<Note> = control
            .notes
            .iter()
            .filter(|n| {
                n.role == Role::Lead && n.start_beat >= lo - 1e-6 && n.start_beat < hi - 1e-6
            })
            .cloned()
            .collect();
        mutated.notes.extend(restored);
        let r = NarrativeReceipt::measure(&song, &control, &mutated, &plan);
        let bad = r.unrealized();
        assert!(
            bad.iter()
                .any(|o| o.carried == Carried::Unrealized("the lead was not actually withheld")),
            "the receipt must catch a restored lead: {}",
            r.report()
        );
    }

    #[test]
    fn stripping_the_band_from_the_withheld_phrase_is_caught() {
        let (song, control, story, plan) = fixture();
        let (_, lo, hi) = withheld_window(&song, &plan);
        // MUTATION B: remove the band from the withheld window — a silent gap is not a withhold.
        let mut mutated = story.clone();
        mutated.notes.retain(|n| {
            !(matches!(n.role, Role::Keys | Role::Bass)
                && n.start_beat >= lo - 1e-6
                && n.start_beat < hi - 1e-6)
        });
        let r = NarrativeReceipt::measure(&song, &control, &mutated, &plan);
        assert!(
            r.unrealized().iter().any(|o| o.carried
                == Carried::Unrealized("the lead stepped aside but no band held the phrase")),
            "the receipt must catch an empty withheld phrase: {}",
            r.report()
        );
    }

    #[test]
    fn an_unchanged_control_realizes_no_band_obligation() {
        // Measuring the BAND control AS the story (no narrative change) must realize nothing: the
        // witness never credits an obligation the performance did not actually carry.
        let (song, control, _story, plan) = fixture();
        let r = NarrativeReceipt::measure(&song, &control, &control, &plan);
        assert_eq!(
            r.band_obligations_realized(),
            0,
            "an unchanged performance carries nothing: {}",
            r.report()
        );
    }
}
