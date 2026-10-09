//! Ensemble narrative — who in the band carries which meaning, derived from μ(song).
//!
//! The band-story round's load-bearing law (plan §4/§20): a performance narrates **μ(song)** — what
//! the song ACTUALLY means, read by [`super::meaning::MeaningPlan::observe`] — never F(trace), the
//! listener *target*. So the carrier assignment here keys off an [`Observation`], never
//! `song.meaning` (which is a `MeaningPlan` = F(trace)). Wiring it to the target would let a
//! performance repair a weak composition, which the law forbids.
//!
//! Proved by ear (Leah, 2026-10-09; see `docs/HUMAN_MUSIC_BAND_STORY.md`): the germ handed from the
//! lead to the keys (Reinforce) and the bass (Develop), withheld by the lead at the Miss while the
//! keys carry the dark, and returned in an earned tutti (Payoff) — the handoff, not density, is what
//! makes the song breathe, and the story survives a lead-mute. This module only *computes* the
//! assignment; a later wave enacts it in the performance planner behind
//! [`super::policy::NarrativePolicy::Ensemble`].

use super::action::Agent;
use super::meaning::{Lane, MeaningKind, Observation};

/// The lead's role at a meaning site under an ensemble narrative. The band-story round proved only
/// these two by ear; a richer countervoice demotion is deliberately not built until an ear asks for
/// it (plan §15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeadRole {
    /// The lead states the site itself — the historical behaviour, and most sites.
    Stating,
    /// The lead WITHHOLDS at the expected arrival; a band carrier takes the site instead (the Miss).
    Withheld,
}

/// Who carries one lead-seated phrase's meaning, and what the lead does there.
#[derive(Debug, Clone, PartialEq)]
pub struct PhraseCarriage {
    /// The lead-seated phrase index this is for (the `at` of its theme-lane event in μ).
    pub phrase: u32,
    /// The meaning the phrase gives, read from μ(song).
    pub meaning: MeaningKind,
    /// Band agents BESIDES the lead that also carry the germ here — [`Agent::Keys`] on a Reinforce,
    /// [`Agent::Bass`] on a Develop, both on a Payoff tutti. Empty when the lead carries alone.
    pub carriers: Vec<Agent>,
    /// What the lead does at this phrase.
    pub lead_role: LeadRole,
}

impl PhraseCarriage {
    /// Whether this phrase hands the germ off the lead at all (a band carrier, or a withheld lead).
    fn hands_off(&self) -> bool {
        !self.carriers.is_empty() || self.lead_role != LeadRole::Stating
    }
}

/// The per-phrase carrier assignment for a song, derived from μ(song). Empty when the song gives no
/// observed theme events (nothing thematic to hand around).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NarrativePlan {
    pub carriages: Vec<PhraseCarriage>,
}

impl NarrativePlan {
    /// Assign carriers from μ: one carriage per observed theme-lane event, mapping its
    /// [`MeaningKind`] to the band agents that carry it and the lead's role. This mirrors the
    /// ear-proved carriage (see the module docs); it reads only what the song actually gives, in the
    /// order μ gives it. Harmony-lane events are the chart's business, not the theme's, and are
    /// ignored here.
    pub fn from_observation(obs: &Observation) -> NarrativePlan {
        use MeaningKind as K;
        let carriages = obs
            .events
            .iter()
            .filter(|w| w.event.lane == Lane::Theme)
            .map(|w| {
                let (carriers, lead_role) = match w.event.kind {
                    // The germ enters the keys — no longer the lead's alone (and building back).
                    K::Reinforce | K::Prepare(_) => (vec![Agent::Keys], LeadRole::Stating),
                    // The bass quotes/develops the contour — same DNA, a new function.
                    K::Develop => (vec![Agent::Bass], LeadRole::Stating),
                    // The return the band already knows carries it alongside the lead.
                    K::Recognize => (vec![Agent::Keys], LeadRole::Stating),
                    // The lead withholds at the expected arrival; the keys carry the dark.
                    K::Miss(_) => (vec![Agent::Keys], LeadRole::Withheld),
                    // The earned ensemble octave-tutti.
                    K::Payoff => (vec![Agent::Keys, Agent::Bass], LeadRole::Stating),
                    // Learn, the thesis, the lead's consequent, home — the lead states it alone.
                    _ => (Vec::new(), LeadRole::Stating),
                };
                PhraseCarriage {
                    phrase: w.event.at,
                    meaning: w.event.kind,
                    carriers,
                    lead_role,
                }
            })
            .collect();
        NarrativePlan { carriages }
    }

    /// The carriage for a lead-seated phrase, if the narrative assigns one.
    pub fn at(&self, phrase: u32) -> Option<&PhraseCarriage> {
        self.carriages.iter().find(|c| c.phrase == phrase)
    }

    /// Whether any phrase hands the germ off the lead. A plan with none is the historical
    /// lead-carries-everything arrangement and need not be enacted — the band-story layer is a no-op
    /// for a song whose μ never leaves the lead.
    pub fn hands_off(&self) -> bool {
        self.carriages.iter().any(PhraseCarriage::hands_off)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::human_music::meaning::{Level, MeaningEvent, Witness, Witnessed};

    /// A synthetic theme-lane event carrying `kind` at phrase `at`. `from_observation` reads only the
    /// lane and kind, so the witness is an arbitrary stand-in.
    fn theme(at: u32, kind: MeaningKind) -> Witnessed {
        Witnessed {
            event: MeaningEvent {
                lane: Lane::Theme,
                at,
                beat: at as f64 * 8.0,
                kind,
            },
            witness: Witness::Thesis { widest: 0 },
        }
    }

    #[test]
    fn maps_each_meaning_to_its_ear_proved_carriers() {
        use MeaningKind as K;
        let obs = Observation {
            events: vec![
                theme(0, K::Learn),
                theme(1, K::Reinforce),
                theme(2, K::Develop),
                theme(3, K::Miss(Level::Mid)),
                theme(4, K::Payoff),
            ],
        };
        let np = NarrativePlan::from_observation(&obs);
        assert_eq!(np.carriages.len(), 5, "one carriage per theme event");
        let at = |p: u32| np.carriages.iter().find(|c| c.phrase == p).unwrap();

        // Learn: the lead teaches it alone.
        assert_eq!(at(0).carriers, Vec::<Agent>::new());
        assert_eq!(at(0).lead_role, LeadRole::Stating);

        // Reinforce: the germ enters the keys.
        assert_eq!(at(1).carriers, vec![Agent::Keys]);
        assert_eq!(at(1).lead_role, LeadRole::Stating);

        // Develop: the bass quotes the contour.
        assert_eq!(at(2).carriers, vec![Agent::Bass]);

        // Miss: the lead WITHHOLDS, the keys carry the dark.
        assert_eq!(at(3).lead_role, LeadRole::Withheld);
        assert_eq!(at(3).carriers, vec![Agent::Keys]);

        // Payoff: the earned ensemble tutti.
        assert_eq!(at(4).carriers, vec![Agent::Keys, Agent::Bass]);
        assert_eq!(at(4).lead_role, LeadRole::Stating);

        assert!(np.hands_off(), "this story leaves the lead");
    }

    #[test]
    fn a_lead_only_story_hands_nothing_off() {
        use MeaningKind as K;
        // Learn then Answer: the lead states both; nobody else carries, the lead never withholds.
        let obs = Observation {
            events: vec![
                theme(0, K::Learn),
                theme(1, K::Answer(super::super::meaning::Close::Home)),
            ],
        };
        let np = NarrativePlan::from_observation(&obs);
        assert!(
            !np.hands_off(),
            "a lead-only μ is the historical arrangement and need not be enacted"
        );
    }

    #[test]
    fn harmony_lane_events_are_ignored() {
        use MeaningKind as K;
        let obs = Observation {
            events: vec![
                theme(0, K::Learn),
                Witnessed {
                    event: MeaningEvent {
                        lane: Lane::Harmony,
                        at: 0,
                        beat: 0.0,
                        kind: K::Establish,
                    },
                    witness: Witness::Thesis { widest: 0 },
                },
            ],
        };
        let np = NarrativePlan::from_observation(&obs);
        assert_eq!(
            np.carriages.len(),
            1,
            "only the theme-lane event becomes a carriage"
        );
        assert_eq!(np.carriages[0].phrase, 0);
    }
}
