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
use super::meaning::{Lane, Level, MeaningKind, Observation};

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

/// A band carrier making a harmonic deflection intelligible. When the chart lifts a dominant
/// ([`MeaningKind::Prepare`]) and then misses the expected arrival ([`MeaningKind::Miss`]) while a
/// lead phrase sounds, the lead may fall silent and this carrier voices the dark, so the deflection
/// reads as meant rather than as a mistake (the ear-proved "keys carry the dark"). This is the
/// HARMONY-lane obligation; it lives beside — not inside — the phrase's THEME obligation, so lane
/// provenance is never lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarmonicCarry {
    /// The harmony-lane slot index of the missed arrival (its `at` in μ).
    pub slot: u32,
    /// How hard the miss lands — the [`MeaningKind::Miss`] surprise level.
    pub level: Level,
    /// The band agent that voices the deflection: [`Agent::Keys`], the ear-proved dark-carrier.
    pub carrier: Agent,
}

/// Who carries one lead-seated phrase's meaning, and what the lead does there.
#[derive(Debug, Clone, PartialEq)]
pub struct PhraseCarriage {
    /// The lead-seated phrase index this is for (the `at` of its theme-lane event in μ).
    pub phrase: u32,
    /// The meaning the phrase gives, read from μ(song) — the THEME obligation.
    pub meaning: MeaningKind,
    /// Band agents BESIDES the lead that also carry the germ here — [`Agent::Keys`] on a Reinforce,
    /// [`Agent::Bass`] on a Develop, both on a Payoff tutti. Empty when the lead carries alone.
    pub carriers: Vec<Agent>,
    /// What the lead does at this phrase.
    pub lead_role: LeadRole,
    /// A harmonic deflection that lands while this lead phrase sounds — a [`MeaningKind::Miss`] with
    /// an actual expected arrival before it, joined from the harmony lane. `None` when no prepared
    /// miss meets the phrase. This is the HARMONY obligation; `meaning`/`carriers` stay the THEME
    /// obligation, so the two lanes never collapse into one.
    pub harmonic: Option<HarmonicCarry>,
}

impl PhraseCarriage {
    /// Whether this phrase hands the germ off the lead at all: a band carrier takes the theme, the
    /// lead withholds, or a band carrier voices a harmonic deflection here.
    fn hands_off(&self) -> bool {
        !self.carriers.is_empty() || self.lead_role != LeadRole::Stating || self.harmonic.is_some()
    }

    /// Whether this phrase states something the listener must hear FROM THE LEAD — a statement the
    /// lead may not withhold without erasing it (the thesis, its learning, a payoff, a settling
    /// answer, a recognized return). A transformation or repetition (Develop, Reinforce) is not
    /// protected: the lead may step aside there and let a carrier hold it.
    fn states_protected_theme(&self) -> bool {
        use MeaningKind as K;
        matches!(
            self.meaning,
            K::Thesis(_) | K::Learn | K::Payoff | K::Answer(_) | K::Recognize
        )
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
        // --- Pass 1: theme carrier obligations ---
        // One carriage per lead-seated theme event, in μ order. The thesis label co-locates with
        // Learn on the first site and is not its own obligation, so it is skipped. `Prepare`/`Miss`
        // are HARMONY-lane kinds and never appear here — the harmony join (pass 2) owns them.
        let mut carriages: Vec<PhraseCarriage> = obs
            .events
            .iter()
            .filter(|w| w.event.lane == Lane::Theme && !matches!(w.event.kind, K::Thesis(_)))
            .map(|w| {
                let carriers = match w.event.kind {
                    // The germ enters the keys — no longer the lead's alone (and building back).
                    K::Reinforce => vec![Agent::Keys],
                    // The bass quotes/develops the contour — same DNA, a new function.
                    K::Develop => vec![Agent::Bass],
                    // The return the band already knows carries it alongside the lead.
                    K::Recognize => vec![Agent::Keys],
                    // The earned ensemble octave-tutti.
                    K::Payoff => vec![Agent::Keys, Agent::Bass],
                    // Learn, the lead's consequent, home — the lead states it alone.
                    _ => Vec::new(),
                };
                PhraseCarriage {
                    phrase: w.event.at,
                    meaning: w.event.kind,
                    carriers,
                    lead_role: LeadRole::Stating,
                    harmonic: None,
                }
            })
            .collect();

        // --- Pass 2: the harmony join ---
        // Lead-phrase onsets, from the same observation (every theme event of a phrase shares its
        // onset beat). A deflection is attributed to the phrase sounding when it lands.
        let mut onsets: Vec<(u32, f64)> = obs
            .events
            .iter()
            .filter(|w| w.event.lane == Lane::Theme)
            .map(|w| (w.event.at, w.event.beat))
            .collect();
        onsets.sort_by(|a, b| a.1.total_cmp(&b.1));
        onsets.dedup_by_key(|&mut (p, _)| p);
        // An "actual expected arrival": the chart lifted a dominant somewhere before the miss.
        let first_prepare = obs
            .events
            .iter()
            .filter(|w| matches!(w.event.kind, K::Prepare(_)))
            .map(|w| w.event.beat)
            .fold(f64::INFINITY, f64::min);
        for w in obs.events.iter().filter(|w| w.event.lane == Lane::Harmony) {
            let K::Miss(level) = w.event.kind else {
                continue;
            };
            // No prepared arrival before this miss → it is not a narrative deflection to carry.
            let prepared = first_prepare < w.event.beat - 1e-9;
            if !prepared {
                continue;
            }
            // The lead phrase sounding at the miss: the latest onset at or before its beat.
            let Some(&(phrase, _)) = onsets
                .iter()
                .rev()
                .find(|&&(_, beat)| beat <= w.event.beat + 1e-9)
            else {
                continue; // the miss lands before the lead ever enters — the chart's own business
            };
            let Some(c) = carriages.iter_mut().find(|c| c.phrase == phrase) else {
                continue;
            };
            if c.harmonic.is_some() {
                continue; // one deflection per phrase; keep the first in μ order
            }
            c.harmonic = Some(HarmonicCarry {
                slot: w.event.at,
                level,
                carrier: Agent::Keys,
            });
            // The lead withholds ONLY where it would not erase a statement the listener must hear
            // from it. At a protected statement the keys still voice the dark, but the lead stays.
            if !c.states_protected_theme() {
                c.lead_role = LeadRole::Withheld;
            }
        }
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
    use crate::audio::human_music::meaning::{Close, MeaningEvent, Witness, Witnessed};

    /// A synthetic event at (`lane`, `at`, `beat`) carrying `kind`. `from_observation` reads only
    /// the lane, kind, phrase and beat, so the witness is an arbitrary stand-in. These stay FAITHFUL
    /// to μ's real laning: `Prepare`/`Miss` are only ever made on [`Lane::Harmony`], never Theme
    /// (the integration witness is `narrative_join_probes`, on real generated songs).
    fn ev(lane: Lane, at: u32, beat: f64, kind: MeaningKind) -> Witnessed {
        Witnessed {
            event: MeaningEvent {
                lane,
                at,
                beat,
                kind,
            },
            witness: Witness::Thesis { widest: 0 },
        }
    }

    fn theme(at: u32, kind: MeaningKind) -> Witnessed {
        ev(Lane::Theme, at, at as f64 * 8.0, kind)
    }

    #[test]
    fn maps_each_theme_meaning_to_its_ear_proved_carriers() {
        use MeaningKind as K;
        let obs = Observation {
            events: vec![
                theme(0, K::Learn),
                theme(1, K::Reinforce),
                theme(2, K::Develop),
                theme(3, K::Recognize),
                theme(4, K::Payoff),
            ],
        };
        let np = NarrativePlan::from_observation(&obs);
        assert_eq!(np.carriages.len(), 5, "one carriage per theme event");
        let at = |p: u32| np.carriages.iter().find(|c| c.phrase == p).unwrap();

        // Learn: the lead teaches it alone.
        assert_eq!(at(0).carriers, Vec::<Agent>::new());
        // Reinforce: the germ enters the keys.
        assert_eq!(at(1).carriers, vec![Agent::Keys]);
        // Develop: the bass quotes the contour.
        assert_eq!(at(2).carriers, vec![Agent::Bass]);
        // Recognize: the keys carry the known return alongside the lead.
        assert_eq!(at(3).carriers, vec![Agent::Keys]);
        // Payoff: the earned ensemble tutti.
        assert_eq!(at(4).carriers, vec![Agent::Keys, Agent::Bass]);

        // No harmony lane here, so nobody withholds and no deflection is carried.
        assert!(np
            .carriages
            .iter()
            .all(|c| c.lead_role == LeadRole::Stating));
        assert!(np.carriages.iter().all(|c| c.harmonic.is_none()));
        assert!(
            np.hands_off(),
            "this story leaves the lead (carriers present)"
        );
    }

    #[test]
    fn a_lead_only_story_hands_nothing_off() {
        use MeaningKind as K;
        // Learn then Answer: the lead states both; nobody else carries, the lead never withholds.
        let obs = Observation {
            events: vec![theme(0, K::Learn), theme(1, K::Answer(Close::Home))],
        };
        let np = NarrativePlan::from_observation(&obs);
        assert!(
            !np.hands_off(),
            "a lead-only μ is the historical arrangement and need not be enacted"
        );
    }

    #[test]
    fn a_prepared_miss_over_a_transformation_withholds_the_lead() {
        use MeaningKind as K;
        // The lead develops the germ at phrase 2 (beat 16); the chart lifts (Prepare @12) and then
        // misses the arrival (@18), while phrase 2 sounds. Develop is not protected → the lead
        // withholds and the keys voice the dark.
        let obs = Observation {
            events: vec![
                theme(1, K::Learn),
                theme(2, K::Develop),
                ev(Lane::Harmony, 3, 12.0, K::Prepare(Level::High)),
                ev(Lane::Harmony, 4, 18.0, K::Miss(Level::Mid)),
            ],
        };
        let np = NarrativePlan::from_observation(&obs);
        let p2 = np.at(2).unwrap();
        assert_eq!(p2.lead_role, LeadRole::Withheld, "the lead steps aside");
        assert_eq!(
            p2.harmonic,
            Some(HarmonicCarry {
                slot: 4,
                level: Level::Mid,
                carrier: Agent::Keys
            })
        );
        // The theme obligation is untouched: Develop still hands the contour to the bass.
        assert_eq!(p2.carriers, vec![Agent::Bass]);
        assert!(np.hands_off());
    }

    #[test]
    fn a_prepared_miss_over_a_protected_statement_keeps_the_lead() {
        use MeaningKind as K;
        // Same deflection, but it lands on a Payoff: the keys still voice the dark, yet the lead
        // must stay — withholding would erase the payoff the listener is owed.
        let obs = Observation {
            events: vec![
                theme(1, K::Learn),
                theme(2, K::Payoff),
                ev(Lane::Harmony, 3, 12.0, K::Prepare(Level::High)),
                ev(Lane::Harmony, 4, 18.0, K::Miss(Level::Mid)),
            ],
        };
        let np = NarrativePlan::from_observation(&obs);
        let p2 = np.at(2).unwrap();
        assert_eq!(p2.lead_role, LeadRole::Stating, "a payoff is protected");
        assert!(p2.harmonic.is_some(), "but the deflection is still carried");
    }

    #[test]
    fn an_unprepared_miss_is_not_a_narrative_deflection() {
        use MeaningKind as K;
        // A miss with no Prepare before it is not an expected-arrival-then-deflection; there is
        // nothing for the band to make intelligible, so no carry and no withhold.
        let obs = Observation {
            events: vec![
                theme(1, K::Learn),
                theme(2, K::Develop),
                ev(Lane::Harmony, 4, 18.0, K::Miss(Level::Mid)),
            ],
        };
        let np = NarrativePlan::from_observation(&obs);
        assert!(np.carriages.iter().all(|c| c.harmonic.is_none()));
        assert_eq!(np.at(2).unwrap().lead_role, LeadRole::Stating);
    }

    #[test]
    fn a_miss_before_the_lead_enters_is_the_charts_business() {
        use MeaningKind as K;
        // The chart deflects in the intro, before the lead's first phrase (beat 16). No lead phrase
        // sounds there, so it is not attached to any carriage — exactly the real deflected_lift
        // Miss @4 case.
        let obs = Observation {
            events: vec![
                ev(Lane::Harmony, 0, 2.0, K::Prepare(Level::High)),
                ev(Lane::Harmony, 1, 4.0, K::Miss(Level::Mid)),
                theme(1, K::Learn),
            ],
        };
        let np = NarrativePlan::from_observation(&obs);
        assert!(np.carriages.iter().all(|c| c.harmonic.is_none()));
    }

    /// C137-A · R3 — the harmony join licenses a Miss by the GLOBAL earliest Prepare, not a causally
    /// relevant predecessor in the Miss's own domain. An early, unrelated Prepare wrongly "prepares"
    /// a much later, locally-unprepared Miss. This PASSES today, characterizing the
    /// stale-global-predecessor weakness; a domain-local join must pair a Miss with the Prepare that
    /// set up ITS arrival. See docs/HUMAN_MUSIC_C137_HARDENING_LEDGER.md.
    #[test]
    fn c137a_r3_a_global_prepare_licenses_an_unrelated_later_miss() {
        use MeaningKind as K;
        // One early, unrelated Prepare in the intro (beat 6, before the lead enters at beat 8).
        // Much later, phrase 5 (beat 40) develops the germ, and a Miss lands at beat 42 with NO
        // Prepare anywhere near it. The only Prepare in the whole song is the early, unrelated one.
        let obs = Observation {
            events: vec![
                theme(1, K::Learn),
                ev(Lane::Harmony, 2, 6.0, K::Prepare(Level::High)),
                theme(5, K::Develop),
                ev(Lane::Harmony, 9, 42.0, K::Miss(Level::Mid)),
            ],
        };
        let np = NarrativePlan::from_observation(&obs);
        let p5 = np.at(5).expect("phrase 5 is a carriage");
        assert!(
            p5.harmonic.is_some(),
            "C137-A R3: the late Miss is joined to phrase 5 purely because a global-earliest Prepare \
             (beat 6) precedes it — the join does not require a Prepare in the Miss's own domain"
        );
    }
}
