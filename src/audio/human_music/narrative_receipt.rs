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
use super::performance::PerformancePlan;
use super::score::{Role, Score};
use super::song::SongMap;
use super::theory::pitch_class;

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

/// A named theme carrier that did not add its OWN material in a carrying carriage (C137-A R5). The
/// count-summing [`NarrativeReceipt::measure`] credits a multi-carrier obligation on the SUM across
/// carriers, so one carrier can cover for a silent other; this names the silent one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarrierShortfall {
    pub phrase: u32,
    pub meaning: MeaningKind,
    pub carrier: Role,
    /// story-minus-control note count for this carrier's role in the phrase (`<= 0` ⇒ carried nothing).
    pub added: i64,
}

/// A realized bass note, in a phrase where the narrative names the Bass a germ carrier, whose pitch
/// is NOT a germ-selected chord tone (C137-A R1/R4). The count-summing [`NarrativeReceipt::measure`]
/// credits a carry by note count and never reads pitch, so it cannot tell the germ from a scramble;
/// this is the source-aware check that can.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GermShortfall {
    pub phrase: u32,
    pub beat: f64,
    pub pitch: i32,
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

    /// Per-carrier shortfalls (C137-A R5): for every carriage with named theme carriers, each
    /// carrier whose OWN added material is `<= 0`. A multi-carrier obligation (a Payoff names Keys
    /// AND Bass) is fully realized only when every named carrier carries — this strengthens the
    /// count-SUMMING [`Self::measure`] (kept as the comparative control) so one carrier can no
    /// longer cover for a silent other. Comparative in the same sense as `measure`: story vs the
    /// BAND control, never the plan's claim. The harmonic dark-carrier has no separate per-carrier
    /// obligation here (it is covered by `measure`'s combined check).
    pub fn carrier_shortfalls(
        song: &SongMap,
        control: &Score,
        story: &Score,
        plan: &NarrativePlan,
    ) -> Vec<CarrierShortfall> {
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
                .count() as i64
        };
        let mut out = Vec::new();
        for c in &plan.carriages {
            if c.carriers.is_empty() {
                continue;
            }
            let Some((lo, hi)) = window(c.phrase) else {
                continue;
            };
            for role in c.carriers.iter().filter_map(|a| role_of(*a)) {
                let added = count(story, role, lo, hi) - count(control, role, lo, hi);
                if added <= 0 {
                    out.push(CarrierShortfall {
                        phrase: c.phrase,
                        meaning: c.meaning,
                        carrier: role,
                        added,
                    });
                }
            }
        }
        out
    }

    /// Membership carry witness (C137-A R1/R4 — **weak; superseded by the ordered
    /// [`Self::germ_contour_shortfalls`]**): for each carriage naming the Bass germ carrier, every
    /// realized bass note whose pitch class is NOT among the germ-selected chord tones
    /// `{ tones[germ_tone_index(d, n)] : d ∈ degrees }`. Reads the germ (`perf.bank.identity`) and
    /// chart (`perf.context_at`), never the plan's claim.
    ///
    /// **KNOWN HOLLOWNESS (C137-B).** This is pure set MEMBERSHIP, not ordered germ identity. When
    /// the germ's degrees span every residue mod `n_tones` — true on the shipped fixture, where
    /// `[0,1,1,2,7,6,4] mod 4 = {0,1,2,3}` over tetrads — the allowed set IS the whole chord and
    /// this degenerates to "is it a chord tone", carrying zero germ information: a flattened or
    /// reordered "germ" that still lands on chord tones passes clean. Retained only as a documented
    /// control beside the ordered witness, which is the real source-aware check. Keys carriers have
    /// no germ voicing yet (deferred), so they are unchecked here — a documented limitation.
    pub fn germ_carry_shortfalls(
        song: &SongMap,
        perf: &PerformancePlan,
        story: &Score,
        plan: &NarrativePlan,
    ) -> Vec<GermShortfall> {
        let germ = &perf.bank.identity;
        let mut out = Vec::new();
        if germ.degrees.is_empty() {
            return out;
        }
        let window = |phrase: u32| {
            song.plan
                .form
                .phrases
                .iter()
                .find(|p| p.ix == phrase)
                .map(|p| (p.start_beat(), p.end_beat()))
        };
        for c in plan
            .carriages
            .iter()
            .filter(|c| c.carriers.contains(&Agent::Bass))
        {
            let Some((lo, hi)) = window(c.phrase) else {
                continue;
            };
            for n in story.notes.iter().filter(|n| {
                n.role == Role::Bass && n.start_beat >= lo - 1e-6 && n.start_beat < hi - 1e-6
            }) {
                let Some(ctx) = perf.context_at(n.start_beat) else {
                    continue;
                };
                let tones = ctx.chord.pitch_classes();
                if tones.is_empty() {
                    continue;
                }
                let germ_selected = germ.degrees.iter().any(|&d| {
                    tones[super::bass::germ_tone_index(d, tones.len())] == pitch_class(n.pitch)
                });
                if !germ_selected {
                    out.push(GermShortfall {
                        phrase: c.phrase,
                        beat: n.start_beat,
                        pitch: n.pitch,
                    });
                }
            }
        }
        out
    }

    /// Ordered germ-contour witness (C137-B): the real source-aware check the membership
    /// [`Self::germ_carry_shortfalls`] could not deliver. For each carriage naming the Bass germ
    /// carrier the realized bass notes are taken in onset order, and note `i` must match the germ's
    /// `i`-th degree voiced into the chord at that beat — `pitch_class(n) ==
    /// tones[germ_tone_index(degrees[i % len], n_tones)]`. A note that does not is a shortfall.
    /// Unlike membership this pins each note to its ORDINAL germ degree, so it rejects a flattened or
    /// reordered "germ" that still lands on chord tones (C137-B KILL 1/2). On the ear-accepted §4
    /// render it returns empty — §4 lays the germ in exact pitch-class order (verified 15/15 at 96
    /// beats, 27/27 at 160), so the witness is sound-preserving. Reads germ + chart as the source of
    /// truth, never the plan. Keys carriers remain unwitnessed (no germ voicing yet).
    pub fn germ_contour_shortfalls(
        song: &SongMap,
        perf: &PerformancePlan,
        story: &Score,
        plan: &NarrativePlan,
    ) -> Vec<GermShortfall> {
        let germ = &perf.bank.identity;
        let mut out = Vec::new();
        if germ.degrees.is_empty() {
            return out;
        }
        let window = |phrase: u32| {
            song.plan
                .form
                .phrases
                .iter()
                .find(|p| p.ix == phrase)
                .map(|p| (p.start_beat(), p.end_beat()))
        };
        for c in plan
            .carriages
            .iter()
            .filter(|c| c.carriers.contains(&Agent::Bass))
        {
            let Some((lo, hi)) = window(c.phrase) else {
                continue;
            };
            let mut bass: Vec<_> = story
                .notes
                .iter()
                .filter(|n| {
                    n.role == Role::Bass && n.start_beat >= lo - 1e-6 && n.start_beat < hi - 1e-6
                })
                .collect();
            bass.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
            for (i, n) in bass.iter().enumerate() {
                let Some(ctx) = perf.context_at(n.start_beat) else {
                    continue;
                };
                let tones = ctx.chord.pitch_classes();
                if tones.is_empty() {
                    continue;
                }
                let d = germ.degrees[i % germ.degrees.len()];
                let expected = tones[super::bass::germ_tone_index(d, tones.len())];
                if expected != pitch_class(n.pitch) {
                    out.push(GermShortfall {
                        phrase: c.phrase,
                        beat: n.start_beat,
                        pitch: n.pitch,
                    });
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::human_music::composer::Composer;
    use crate::audio::human_music::contract::CompositionGrammar;
    use crate::audio::human_music::functor::perform_with_profile;
    use crate::audio::human_music::meaning::MeaningPlan;
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

    /// §13 anti-overfit — the band-story is not a trick of the one fixture. Across genuinely
    /// different deflecting stories, each realizes at least one real band obligation that the
    /// control did not, and nothing on the lead-only judged voice (grammar=None) does.
    #[test]
    fn the_band_story_generalizes_across_stories() {
        use crate::audio::human_music::semantic::{deflected_lift_trace, false_climax};
        let world = MusicWorld::black_ice();
        let perform = |song: &SongMap, profile| {
            perform_with_profile(song, &world, PerformanceOptions::default(), profile)
                .unwrap()
                .score
        };
        // Several different deflecting generated stories.
        let songs: Vec<(&str, SongMap)> = vec![
            (
                "deflected_lift@96",
                SongMap::compose(
                    &deflected_lift_trace(96.0),
                    SEED,
                    Some(CompositionGrammar::DeflectedLift),
                    Composer::MeaningDirected,
                ),
            ),
            (
                "false_climax@96",
                SongMap::compose(
                    &false_climax(96.0),
                    SEED,
                    Some(CompositionGrammar::DeflectedLift),
                    Composer::MeaningDirected,
                ),
            ),
            (
                "rise_unresolved@160",
                SongMap::compose(
                    &rise_unresolved(160.0),
                    SEED,
                    Some(CompositionGrammar::DeflectedLift),
                    Composer::MeaningDirected,
                ),
            ),
        ];
        for (name, song) in &songs {
            let control = perform(song, PerformanceProfile::BAND);
            let story = perform(
                song,
                PerformanceProfile::BAND.with_narrative(NarrativePolicy::Ensemble),
            );
            let plan = NarrativePlan::from_observation(&MeaningPlan::observe(song));
            let r = NarrativeReceipt::measure(song, &control, &story, &plan);
            assert!(
                r.band_obligations_realized() >= 1,
                "{name}: the band-story must realize a real obligation, not overfit the fixture\n{}",
                r.report()
            );
        }

        // The judged voice (grammar=None) has no harmony lane, so its narrative carries only the
        // theme; the comparative witness still never fabricates an obligation it did not realize.
        let none = SongMap::compose(
            &deflected_lift_trace(96.0),
            SEED,
            None,
            Composer::MeaningDirected,
        );
        let plan = NarrativePlan::from_observation(&MeaningPlan::observe(&none));
        assert!(
            plan.carriages.iter().all(|c| c.harmonic.is_none()),
            "the judged voice has no harmonic deflection to carry"
        );
    }

    // ─── C137-A red-first audit · characterizations of the receipt's known weaknesses ───
    // These PASS today: each asserts the CURRENT (count-based) behaviour and so documents the gap
    // the audit names. The predeclared DESIRED behaviour is the opposite; when the source-aware /
    // per-carrier witness lands, these assertions flip. See docs/HUMAN_MUSIC_C137_HARDENING_LEDGER.md.

    /// C137-A · R1/R4 — the receipt credits a CARRY by note-count delta and never reads pitch, so
    /// scrambling the carrier's material off the germ (count + timing preserved) does not change the
    /// verdict. A source-aware witness must additionally check the added material QUOTES the germ.
    #[test]
    fn c137a_r1_carry_is_credited_by_count_not_germ_identity() {
        let (song, control, story, plan) = fixture();
        let base = NarrativeReceipt::measure(&song, &control, &story, &plan);
        let realized = base
            .outcomes
            .iter()
            .find(|o| o.carried == Carried::Realized)
            .expect("the fixture realizes at least one carry");
        let phrase = realized.phrase;
        let c = plan.carriages.iter().find(|c| c.phrase == phrase).unwrap();
        let (lo, hi) = song
            .plan
            .form
            .phrases
            .iter()
            .find(|p| p.ix == phrase)
            .map(|p| (p.start_beat(), p.end_beat()))
            .unwrap();
        let carrier_roles: Vec<Role> = c
            .carriers
            .iter()
            .copied()
            .chain(c.harmonic.map(|h| h.carrier))
            .filter_map(role_of)
            .collect();
        // Scramble every carrier note in the phrase to an absurd, constant, non-germ pitch — count
        // and timing untouched. Nothing the ear would call "the germ" survives.
        let mut scrambled = story.clone();
        for n in scrambled.notes.iter_mut() {
            let in_win = n.start_beat >= lo - 1e-6 && n.start_beat < hi - 1e-6;
            if in_win && carrier_roles.contains(&n.role) {
                n.pitch = 1;
            }
        }
        let r = NarrativeReceipt::measure(&song, &control, &scrambled, &plan);
        let still = r.outcomes.iter().find(|o| o.phrase == phrase).unwrap();
        assert_eq!(
            still.carried,
            Carried::Realized,
            "C137-A R1: the receipt never reads pitch; scrambling the carrier off the germ leaves \
             the verdict Realized — it credits by count, not identity"
        );
    }

    /// C137-A · R5 — a multi-carrier obligation (a Payoff names Keys AND Bass) is credited on the SUM
    /// of added material across carriers, so it reads Realized when only ONE named carrier carries
    /// and the other adds nothing. A per-carrier witness must require EACH named carrier to add its
    /// own material. Demonstrated by zeroing the weaker carrier's contribution.
    #[test]
    fn c137a_r5_multicarrier_payoff_credited_without_each_carrier() {
        let (song, control, story, plan) = fixture();
        let in_win =
            |n: &Note, lo: f64, hi: f64| n.start_beat >= lo - 1e-6 && n.start_beat < hi - 1e-6;
        let base = NarrativeReceipt::measure(&song, &control, &story, &plan);
        let multi = plan
            .carriages
            .iter()
            .find(|c| {
                let roles: Vec<Role> = c.carriers.iter().copied().filter_map(role_of).collect();
                roles.len() >= 2
                    && base
                        .outcomes
                        .iter()
                        .any(|o| o.phrase == c.phrase && o.carried == Carried::Realized)
            })
            .expect("the fixture realizes a multi-carrier Payoff");
        let phrase = multi.phrase;
        let roles: Vec<Role> = multi.carriers.iter().copied().filter_map(role_of).collect();
        let (lo, hi) = song
            .plan
            .form
            .phrases
            .iter()
            .find(|p| p.ix == phrase)
            .map(|p| (p.start_beat(), p.end_beat()))
            .unwrap();
        let count = |sc: &Score, r: Role| {
            sc.notes
                .iter()
                .filter(|n| n.role == r && in_win(n, lo, hi))
                .count() as i64
        };
        // The carrier that added the LEAST — zero its contribution (story count down to control
        // count), leaving the other carrier to drive the sum. One named carrier now carries nothing.
        let weakest = *roles
            .iter()
            .min_by_key(|&&r| count(&story, r) - count(&control, r))
            .unwrap();
        let surplus = (count(&story, weakest) - count(&control, weakest)).max(0);
        let mut story2 = story.clone();
        let mut dropped = 0;
        story2.notes.retain(|n| {
            if n.role == weakest && in_win(n, lo, hi) && dropped < surplus {
                dropped += 1;
                false
            } else {
                true
            }
        });
        let r = NarrativeReceipt::measure(&song, &control, &story2, &plan);
        let still = r.outcomes.iter().find(|o| o.phrase == phrase).unwrap();
        assert_eq!(
            still.carried,
            Carried::Realized,
            "C137-A R5: a named carrier ({weakest:?}) now adds nothing, yet the multi-carrier \
             carriage reads Realized on the other carrier alone — the receipt sums across carriers"
        );
    }

    /// C137-A · R2b — the §4 germ substitution (bass.rs) overwrites `function` to `ChordTone` but
    /// leaves `prov.role_note` as it was, so a germ-driven chord tone can wear a stale structural
    /// tag. `role_note` is gate-read and fingerprinted, but the §4 pass is byte-exact-OFF the
    /// default path, so historical fingerprints are intact; the inconsistency is internal to the
    /// Ensemble story path. See docs/HUMAN_MUSIC_C137_HARDENING_LEDGER.md.
    #[test]
    fn c137a_r2b_germ_substituted_bass_keeps_a_stale_role_note() {
        use crate::audio::human_music::score::PitchFunction;
        let (song, _control, story, plan) = fixture();
        let bass_phrases: Vec<u32> = plan
            .carriages
            .iter()
            .filter(|c| c.carriers.contains(&Agent::Bass))
            .map(|c| c.phrase)
            .collect();
        let mut germ_notes = 0usize;
        let mut tags: Vec<(u32, &'static str)> = Vec::new();
        for ph in &bass_phrases {
            let Some(p) = song.plan.form.phrases.iter().find(|p| p.ix == *ph) else {
                continue;
            };
            let (lo, hi) = (p.start_beat(), p.end_beat());
            for n in story.notes.iter().filter(|n| {
                n.role == Role::Bass && n.start_beat >= lo - 1e-6 && n.start_beat < hi - 1e-6
            }) {
                if n.function == Some(PitchFunction::ChordTone) {
                    germ_notes += 1;
                    tags.push((*ph, n.prov.role_note));
                }
            }
        }
        println!(
            "C137-A R2b: {germ_notes} ChordTone bass notes in carrier phrases; role_notes: {tags:?}"
        );
        assert!(
            germ_notes > 0,
            "the fixture must germ-substitute some bass notes"
        );
        // The sharp, load-bearing finding: ≥1 germ-substituted note wears a GATE-READ tag —
        // "approach" (expression.rs:139), "answer" (temporal.rs:247, material.rs:890), "pedal"
        // (fingerprinted). Those gates fired on these stale tags IN THE EAR-ACCEPTED render, so
        // reconciling role_note would flip a gate branch and change the accepted sound: it is
        // ear-gated, not a free fix (correcting the ledger's earlier "inert fix" speculation).
        let gate_read_stale: Vec<_> = tags
            .iter()
            .filter(|(_, t)| matches!(*t, "approach" | "answer" | "pedal"))
            .collect();
        assert!(
            !gate_read_stale.is_empty(),
            "C137-A R2b: a germ-substituted bass note wears a gate-read stale tag; reconciling \
             role_note flips a gate branch and changes the accepted sound (ear-gated): {tags:?}"
        );
    }

    /// C137-A · R5 REPAIR — the per-carrier witness `carrier_shortfalls` NAMES a silent named
    /// carrier that the count-summing `measure` (the control) credits anyway. On the same
    /// one-carrier mutation as `c137a_r5_…`, `measure` still reports Realized, but
    /// `carrier_shortfalls` reports the silenced carrier. Also records the fixture's NATURAL
    /// shortfalls — the audit's "do all named payoff carriers actually fulfil the obligation?"
    #[test]
    fn c137a_r5_repair_per_carrier_names_the_silent_carrier() {
        let (song, control, story, plan) = fixture();
        let natural = NarrativeReceipt::carrier_shortfalls(&song, &control, &story, &plan);
        println!("C137-A R5 repair: natural fixture shortfalls = {natural:?}");

        let in_win =
            |n: &Note, lo: f64, hi: f64| n.start_beat >= lo - 1e-6 && n.start_beat < hi - 1e-6;
        let base = NarrativeReceipt::measure(&song, &control, &story, &plan);
        let multi = plan
            .carriages
            .iter()
            .find(|c| {
                let roles: Vec<Role> = c.carriers.iter().copied().filter_map(role_of).collect();
                roles.len() >= 2
                    && base
                        .outcomes
                        .iter()
                        .any(|o| o.phrase == c.phrase && o.carried == Carried::Realized)
            })
            .expect("the fixture realizes a multi-carrier Payoff");
        let phrase = multi.phrase;
        let roles: Vec<Role> = multi.carriers.iter().copied().filter_map(role_of).collect();
        let (lo, hi) = song
            .plan
            .form
            .phrases
            .iter()
            .find(|p| p.ix == phrase)
            .map(|p| (p.start_beat(), p.end_beat()))
            .unwrap();
        let count = |sc: &Score, r: Role| {
            sc.notes
                .iter()
                .filter(|n| n.role == r && in_win(n, lo, hi))
                .count() as i64
        };
        let weakest = *roles
            .iter()
            .min_by_key(|&&r| count(&story, r) - count(&control, r))
            .unwrap();
        let surplus = (count(&story, weakest) - count(&control, weakest)).max(0);
        let mut story2 = story.clone();
        let mut dropped = 0;
        story2.notes.retain(|n| {
            if n.role == weakest && in_win(n, lo, hi) && dropped < surplus {
                dropped += 1;
                false
            } else {
                true
            }
        });

        // Control: measure() still credits the carriage (it sums across carriers).
        let m = NarrativeReceipt::measure(&song, &control, &story2, &plan);
        assert_eq!(
            m.outcomes
                .iter()
                .find(|o| o.phrase == phrase)
                .unwrap()
                .carried,
            Carried::Realized
        );
        // Strengthening: the per-carrier witness names the silenced carrier.
        let shortfalls = NarrativeReceipt::carrier_shortfalls(&song, &control, &story2, &plan);
        assert!(
            shortfalls
                .iter()
                .any(|s| s.phrase == phrase && s.carrier == weakest && s.added <= 0),
            "C137-A R5 repair: carrier_shortfalls must name the silenced carrier {weakest:?} at \
             phrase {phrase}, though measure() still credits the carriage: {shortfalls:?}"
        );
    }

    /// C137-B — the ordered germ-contour witness catches a flattened "germ" that the membership
    /// witness (C137-A R1/R4) is blind to. A bass-carrier phrase re-voiced to its chord ROOT on every
    /// onset stays entirely on chord tones, so `germ_carry_shortfalls` (membership) finds nothing —
    /// its documented hollowness. The ordered `germ_contour_shortfalls` flags every onset whose
    /// ordinal germ degree is not the root, because it pins each note to its position in the germ. On
    /// the genuine §4 render BOTH are empty, so the new witness is sound-preserving.
    #[test]
    fn c137b_contour_witness_catches_flattened_germ_membership_misses() {
        let song = SongMap::compose(
            &rise_unresolved(160.0),
            SEED,
            Some(CompositionGrammar::DeflectedLift),
            Composer::MeaningDirected,
        );
        let world = MusicWorld::black_ice();
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

        // Genuine §4 render: the ordered witness is sound-preserving (membership is clean too).
        let genuine_contour =
            NarrativeReceipt::germ_contour_shortfalls(&song, &story.perf, &story.score, &plan);
        let genuine_member =
            NarrativeReceipt::germ_carry_shortfalls(&song, &story.perf, &story.score, &plan);
        assert!(
            genuine_contour.is_empty(),
            "ordered contour witness must be empty on the ear-accepted §4 render: {genuine_contour:?}"
        );
        assert!(
            genuine_member.is_empty(),
            "membership witness is also clean on the genuine render: {genuine_member:?}"
        );

        // Flatten one bass-carrier phrase to the chord ROOT on every onset: still chord tones (so
        // membership is blind), but the wrong ORDINAL germ tone (so the contour witness catches it).
        let bass_phrase = plan
            .carriages
            .iter()
            .find(|c| c.carriers.contains(&Agent::Bass))
            .map(|c| c.phrase)
            .expect("a bass-carrier phrase");
        let (lo, hi) = song
            .plan
            .form
            .phrases
            .iter()
            .find(|p| p.ix == bass_phrase)
            .map(|p| (p.start_beat(), p.end_beat()))
            .unwrap();
        let mut flat = story.score.clone();
        let mut hit = 0;
        for n in flat.notes.iter_mut() {
            if n.role == Role::Bass && n.start_beat >= lo - 1e-6 && n.start_beat < hi - 1e-6 {
                if let Some(ctx) = story.perf.context_at(n.start_beat) {
                    let tones = ctx.chord.pitch_classes();
                    if !tones.is_empty() {
                        n.pitch = 48 + tones[0];
                        hit += 1;
                    }
                }
            }
        }
        assert!(hit > 0, "the phrase must have bass notes to flatten");

        let member = NarrativeReceipt::germ_carry_shortfalls(&song, &story.perf, &flat, &plan);
        let contour = NarrativeReceipt::germ_contour_shortfalls(&song, &story.perf, &flat, &plan);
        println!(
            "C137-B contour: flattened {hit} bass notes in p{bass_phrase} to chord root — \
             membership shortfalls={}, contour shortfalls={}",
            member.len(),
            contour.len()
        );
        assert!(
            member.iter().all(|s| s.phrase != bass_phrase),
            "membership witness is HOLLOW: a root-flattened germ passes it (C137-B KILL 1/2): {member:?}"
        );
        assert!(
            contour.iter().filter(|s| s.phrase == bass_phrase).count() > 0,
            "ordered contour witness must flag the flattened germ: {contour:?}"
        );
    }

    /// C137-A · R1/R4 REPAIR — the source-aware `germ_carry_shortfalls` tells the germ from a
    /// scramble, which the pitch-blind `measure` (the control) cannot. On the genuine §4 story it
    /// finds few/no bass shortfalls (the bass notes in a bass-carrier phrase ARE germ-selected chord
    /// tones); scrambling those notes off the germ makes the source-aware witness flag them, strictly
    /// more than the genuine story — the discrimination the count-based receipt lacks.
    #[test]
    fn c137a_r1_repair_source_aware_witness_tells_germ_from_scramble() {
        let song = SongMap::compose(
            &rise_unresolved(160.0),
            SEED,
            Some(CompositionGrammar::DeflectedLift),
            Composer::MeaningDirected,
        );
        let world = MusicWorld::black_ice();
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

        let real = NarrativeReceipt::germ_carry_shortfalls(&song, &story.perf, &story.score, &plan);

        let bass_phrase = plan
            .carriages
            .iter()
            .find(|c| c.carriers.contains(&Agent::Bass))
            .map(|c| c.phrase)
            .expect("a bass-carrier phrase");
        let (lo, hi) = song
            .plan
            .form
            .phrases
            .iter()
            .find(|p| p.ix == bass_phrase)
            .map(|p| (p.start_beat(), p.end_beat()))
            .unwrap();
        let mut scrambled = story.score.clone();
        let mut hit = 0;
        for n in scrambled.notes.iter_mut() {
            if n.role == Role::Bass && n.start_beat >= lo - 1e-6 && n.start_beat < hi - 1e-6 {
                n.pitch = 1;
                hit += 1;
            }
        }
        assert!(hit > 0, "the phrase must have bass notes to scramble");
        let flagged =
            NarrativeReceipt::germ_carry_shortfalls(&song, &story.perf, &scrambled, &plan);
        println!(
            "C137-A R1 repair: real-story shortfalls={}, scrambled shortfalls={} (scrambled {hit} notes in p{bass_phrase})",
            real.len(),
            flagged.len()
        );
        assert!(
            flagged.iter().any(|s| s.phrase == bass_phrase),
            "the source-aware witness must flag bass scrambled off the germ: {flagged:?}"
        );
        assert!(
            flagged.len() > real.len(),
            "the source-aware witness must discriminate the scramble from the genuine germ voicing \
             (real={}, scrambled={})",
            real.len(),
            flagged.len()
        );
    }
}
