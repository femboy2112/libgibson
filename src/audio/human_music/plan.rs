//! The **composition plan** — the planning boundary between semantic meaning and note
//! generation.
//!
//! Round I let low-level generators invent high-level structure independently: form was a
//! flat list of semantic regions, every voice played in every section, and "silence" was
//! just a lower velocity. This module inserts the commitments a real piece makes *before*
//! any note is chosen:
//!
//! - [`FormGraph`] — a hierarchy of [`Phrase`]s in musical units (bars), with stable section
//!   *families* where recurrence means something (an `A` recurs; an `A'` is an explicit
//!   bounded transform of `A`, not a new random area that happens to share a label), each
//!   carrying its full intent trajectory. Its rhetorical job is the discourse layer's
//!   [`super::discourse::DiscourseRole`], not a positional label.
//! - [`ArrangementPlan`] — a per-phrase assignment of every voice to an [`ArrangementRole`]
//!   (foreground / support / foundation / pulse / texture / silent), with an enforced
//!   foreground budget. This is what finally lets the band *shut up*: a pad can disappear,
//!   drums can drop out, the lead can breathe, and silence is a planned event.
//!
//! [`CompositionPlan`] bundles these (and, as later rounds land them, the harmonic plan,
//! motif bank and groove identity) and [`CompositionPlan::dump`]s an inspectable structural
//! summary.

use super::contract::CoherenceContract;
use super::discourse::{DiscoursePlan, DiscourseRole, PhraseGoal};
use super::form::{SectionKind, BEATS_PER_BAR};
use super::intent::MusicIntent;
use super::score::Role;
use super::timeline::{IntentSpan, IntentTimeline};

/// A section family. Recurrence of a family is meaningful: two `A` phrases are the *same*
/// idea returning; `APrime` is a deliberately bounded transformation of an earlier `A`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionFamily {
    Intro,
    /// The primary recurring section.
    A,
    /// A bounded transformation of an earlier `A` (carries the index of its base phrase).
    APrime {
        base: u32,
    },
    /// A contrasting section.
    B,
    /// A breakdown / negative-space section.
    Break,
    /// The dynamic peak.
    Climax,
    Coda,
}

impl SectionFamily {
    /// A short provenance/dump label.
    pub fn label(self) -> &'static str {
        match self {
            SectionFamily::Intro => "intro",
            SectionFamily::A => "A",
            SectionFamily::APrime { .. } => "A'",
            SectionFamily::B => "B",
            SectionFamily::Break => "break",
            SectionFamily::Climax => "climax",
            SectionFamily::Coda => "coda",
        }
    }

    /// The base phrase this family is a transformation of, if any (only `A'`).
    pub fn base(self) -> Option<u32> {
        match self {
            SectionFamily::APrime { base } => Some(base),
            _ => None,
        }
    }

    /// Map to the legacy [`SectionKind`] used by `Provenance::section` and the old `Form`.
    pub fn to_section_kind(self) -> SectionKind {
        match self {
            SectionFamily::Intro => SectionKind::Intro,
            SectionFamily::A | SectionFamily::APrime { .. } => SectionKind::A,
            SectionFamily::B => SectionKind::Contrast,
            SectionFamily::Break => SectionKind::Development,
            SectionFamily::Climax => SectionKind::Climax,
            SectionFamily::Coda => SectionKind::Coda,
        }
    }

    /// True when two phrases belong to the same family root (`A` and its `A'`s match).
    pub fn same_family_as(self, other: SectionFamily) -> bool {
        use SectionFamily::*;
        matches!(
            (self, other),
            (A, A) | (A, APrime { .. }) | (APrime { .. }, A) | (APrime { .. }, APrime { .. })
        ) || core::mem::discriminant(&self) == core::mem::discriminant(&other)
    }
}

/// One phrase: a bar span with a family identity and the intent trajectory in force. Its
/// rhetorical job lives in the discourse layer ([`super::discourse::PhraseGoal`]), not here.
#[derive(Debug, Clone, Copy)]
pub struct Phrase {
    /// Index within the [`FormGraph`].
    pub ix: u32,
    /// First bar (0-based).
    pub start_bar: u32,
    /// Length in bars.
    pub bars: u32,
    /// Section-family identity.
    pub family: SectionFamily,
    /// Representative running intent (the span's start; kept for dumps and back-compat).
    pub intent: MusicIntent,
    /// The intent *trajectory* across the phrase — start/end/peaks and whether a salient semantic
    /// event fires inside it. Round III reads this instead of the single start snapshot, so a
    /// phrase can react to a mid-piece Confirmation/resolution instead of carrying stale intent.
    pub span: IntentSpan,
    /// Whether this phrase is a licensed rupture (climax / world-switch) where extra
    /// simultaneous novelty is permitted.
    pub is_rupture: bool,
    /// The exact end of the whole piece (its requested `total_beats`). [`Phrase::end_beat`] never
    /// runs past it, so a final phrase whose last bar is partial ends where the piece does.
    pub piece_end: f64,
}

impl Phrase {
    /// One past the last bar (the partial final bar counts as a bar).
    pub fn end_bar(&self) -> u32 {
        self.start_bar + self.bars
    }
    /// First beat of the phrase.
    pub fn start_beat(&self) -> f64 {
        self.start_bar as f64 * BEATS_PER_BAR
    }
    /// One past the last beat: the bar line after the phrase, or the piece's exact end when the
    /// phrase's last bar is partial.
    pub fn end_beat(&self) -> f64 {
        (self.end_bar() as f64 * BEATS_PER_BAR).min(self.piece_end)
    }
}

/// A hierarchy of phrases in musical units, with recurring families.
///
/// `total_bars` counts every bar the piece touches under the partial-bar rule
/// ([`super::form::bars_spanning`]); `total_beats` is the exact requested length, which may end
/// inside the last bar (9 beats = 3 bars, the third one beat long).
#[derive(Debug, Clone)]
pub struct FormGraph {
    pub phrases: Vec<Phrase>,
    pub total_bars: u32,
    /// The exact length of the piece in beats (`<= total_bars * BEATS_PER_BAR`).
    pub total_beats: f64,
}

impl FormGraph {
    /// Build a phrase graph over `total_bars` on the contract's phrase grid, **with phrase
    /// boundaries snapped to salient semantic events**.
    ///
    /// Boundaries are the base grid (`phrase_bars`, `2·phrase_bars`, …) plus every salient event
    /// ([`super::semantic::EventKind::is_salient`]) floored onto the 2-bar sub-grid. Because we
    /// only ever *subdivide* the base grid, no phrase exceeds `phrase_bars`, the grid stays
    /// musically legible, and a structural event (Impact / Confirmation / SectionResolved / …)
    /// starts its own phrase instead of being swallowed mid-phrase — the Round III fix for the
    /// one-snapshot defect. Each phrase then carries its full [`IntentSpan`], not just a start
    /// sample.
    ///
    /// Families are still assigned positionally here (intro first, coda last, the peak-energy
    /// interior phrase is the `Climax`, odd interior phrases form the recurring `A`-family, even
    /// ones are `B`); the discourse layer replaces that with rhetorical roles derived from the
    /// trajectory.
    pub fn build(
        timeline: &IntentTimeline,
        total_bars: u32,
        contract: &CoherenceContract,
    ) -> FormGraph {
        Self::build_for_beats(timeline, total_bars.max(1) as f64 * BEATS_PER_BAR, contract)
    }

    /// Like [`FormGraph::build`], but over an exact `total_beats` that may end inside a bar: the
    /// graph spans [`super::form::bars_spanning`] bars and its final phrase ends at `total_beats`.
    /// A degenerate request (not finite, or not positive) is one bar long.
    pub fn build_for_beats(
        timeline: &IntentTimeline,
        total_beats: f64,
        contract: &CoherenceContract,
    ) -> FormGraph {
        let total_beats = if total_beats.is_finite() && total_beats > 0.0 {
            total_beats
        } else {
            BEATS_PER_BAR
        };
        let total_bars = super::form::bars_spanning(total_beats, BEATS_PER_BAR);
        let phrase_bars = contract.phrase_bars.max(1);

        // Phrase boundaries: the base grid ∪ salient events snapped onto the 2-bar sub-grid.
        let mut bounds: Vec<u32> = (0..total_bars).step_by(phrase_bars as usize).collect();
        bounds.push(total_bars);
        for t in &timeline.transitions {
            if !t.event_kind.is_salient() {
                continue;
            }
            let bar = (t.at_beat / BEATS_PER_BAR).round() as u32;
            let snapped = (bar / 2) * 2; // floor onto the 2-bar grid
            if snapped > 0 && snapped < total_bars {
                bounds.push(snapped);
            }
        }
        bounds.sort_unstable();
        bounds.dedup();
        let n = bounds.len() - 1; // number of phrases (bounds always has >= 2 entries)

        // Pass 1: materialize each phrase's bar span and intent trajectory.
        let mut raw: Vec<(u32, u32, IntentSpan)> = Vec::with_capacity(n);
        for i in 0..n {
            let start_bar = bounds[i];
            let bars = bounds[i + 1] - bounds[i];
            let start_beat = start_bar as f64 * BEATS_PER_BAR;
            let end_beat = ((start_bar + bars) as f64 * BEATS_PER_BAR).min(total_beats);
            raw.push((start_bar, bars, timeline.span(start_beat, end_beat)));
        }

        // The climax lands on the peak phrase via the SINGLE shared definition of "the peak"
        // ([`super::discourse::culmination_index`], peak energy+tension) that the discourse layer's
        // Culminate role also uses — so the arrangement (keyed on the Climax family) and the melody
        // (keyed on Culminate) never disagree about which phrase is the peak (the Round IV
        // split-brain fix). The interior clamp inside `culmination_index` keeps it off the intro/coda
        // for pieces of three or more phrases.
        let spans: Vec<IntentSpan> = raw.iter().map(|(_, _, s)| *s).collect();
        let climax_ix = super::discourse::culmination_index(&spans) as u32;

        // Pass 2: assign a positional family and build each phrase.
        let mut phrases = Vec::with_capacity(n);
        let mut first_a: Option<u32> = None;
        for (i, (start_bar, bars, span)) in raw.into_iter().enumerate() {
            let ix = i as u32;
            let family = if i == 0 {
                SectionFamily::Intro
            } else if i == n - 1 {
                SectionFamily::Coda
            } else if ix == climax_ix {
                SectionFamily::Climax
            } else if i % 2 == 1 {
                match first_a {
                    None => {
                        first_a = Some(ix);
                        SectionFamily::A
                    }
                    Some(base) => SectionFamily::APrime { base },
                }
            } else {
                SectionFamily::B
            };
            phrases.push(Phrase {
                ix,
                start_bar,
                bars,
                family,
                intent: span.start,
                span,
                is_rupture: matches!(family, SectionFamily::Climax),
                piece_end: total_beats,
            });
        }

        FormGraph {
            phrases,
            total_bars,
            total_beats,
        }
    }

    /// The phrase containing `beat` (clamped to the last phrase).
    pub fn phrase_at(&self, beat: f64) -> &Phrase {
        self.phrases
            .iter()
            .find(|p| beat >= p.start_beat() - 1e-9 && beat < p.end_beat() - 1e-9)
            .unwrap_or_else(|| self.phrases.last().expect("form graph has no phrases"))
    }

    /// All phrases sharing a family root with `family` (an `A` and all its `A'`s).
    pub fn family_members(&self, family: SectionFamily) -> Vec<&Phrase> {
        self.phrases
            .iter()
            .filter(|p| p.family.same_family_as(family))
            .collect()
    }
}

/// The job a voice is doing in a phrase. Only [`ArrangementRole::Foreground`] counts against
/// the foreground budget; [`ArrangementRole::Silent`] drops the voice entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrangementRole {
    Foreground,
    Support,
    Foundation,
    Pulse,
    Texture,
    Punctuation,
    Silent,
}

impl ArrangementRole {
    /// A short provenance/dump label.
    pub fn label(self) -> &'static str {
        match self {
            ArrangementRole::Foreground => "foreground",
            ArrangementRole::Support => "support",
            ArrangementRole::Foundation => "foundation",
            ArrangementRole::Pulse => "pulse",
            ArrangementRole::Texture => "texture",
            ArrangementRole::Punctuation => "punctuation",
            ArrangementRole::Silent => "silent",
        }
    }

    /// The velocity multiplier this role applies to a voice — the dynamic hierarchy that
    /// puts one voice forward and the rest behind it (silence is literal zero).
    ///
    /// Round VI narrows this spread deliberately. It used to run Foreground 1.0 down to
    /// Texture 0.42 — a ~7.5 dB cliff that, stacked on top of the world's per-voice patch gain
    /// and bus mix (both of which also favor the lead), let the melody sit ~11 dB over the
    /// harmony bed: "pasted 3 dB in front of everybody." Semantic role decides *who leads*, not
    /// *by how much* — that's the mix engineer's job downstream. So the non-foreground tiers are
    /// pulled up close: Foreground↔Texture is now ~2.9 dB, not a canyon. Silence stays a literal
    /// zero (that's meaning, not level), and Punctuation stays near the top — a stab should land.
    pub fn gain(self) -> f32 {
        match self {
            ArrangementRole::Foreground => 1.0,
            ArrangementRole::Support => 0.85,
            ArrangementRole::Foundation => 0.82,
            ArrangementRole::Pulse => 0.88,
            ArrangementRole::Texture => 0.72,
            ArrangementRole::Punctuation => 0.9,
            ArrangementRole::Silent => 0.0,
        }
    }

    /// Whether the voice sounds at all in this role.
    pub fn is_audible(self) -> bool {
        !matches!(self, ArrangementRole::Silent)
    }
}

/// One phrase's arrangement: what every voice is doing.
#[derive(Debug, Clone, Copy)]
pub struct PhraseArrangement {
    pub pad: ArrangementRole,
    pub keys: ArrangementRole,
    pub bass: ArrangementRole,
    pub lead: ArrangementRole,
    pub drums: ArrangementRole,
}

impl PhraseArrangement {
    /// The role for a melodic [`Role`].
    pub fn role_for(&self, role: Role) -> ArrangementRole {
        match role {
            Role::Pad => self.pad,
            Role::Keys => self.keys,
            Role::Bass => self.bass,
            Role::Lead => self.lead,
        }
    }

    /// The number of voices in the foreground (what the budget caps).
    pub fn foreground_count(&self) -> u8 {
        [self.pad, self.keys, self.bass, self.lead, self.drums]
            .iter()
            .filter(|r| matches!(r, ArrangementRole::Foreground))
            .count() as u8
    }
}

/// A generic intro archetype: a designed subtraction that TEASES the song rather than hanging a
/// lone pad. Each exposes at least two identity axes over the backbone's already-moving harmonic
/// cell, and the choice foreshadows the phrase that follows the intro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntroArchetype {
    /// Bass states the cell's root motion under a texture pad — harmonic + bass identity.
    BassPickup,
    /// The lead exposes the theme over a texture pad — harmonic + thematic identity.
    ThemeFragment,
    /// Pad plus a sparse keys comp — harmonic identity with a rhythmic pulse.
    HarmonicTease,
    /// A held pad with upper keys motion and a bass anchor.
    PedalWithUpperMotion,
}

impl IntroArchetype {
    /// Choose an intro that foreshadows what the song does next (the phrase after the intro).
    fn foreshadowing(next_role: DiscourseRole) -> IntroArchetype {
        match next_role {
            DiscourseRole::Depart | DiscourseRole::Intensify => IntroArchetype::BassPickup,
            DiscourseRole::Question => IntroArchetype::ThemeFragment,
            _ => IntroArchetype::HarmonicTease,
        }
    }

    /// The intro phrase's arrangement.
    fn arrangement(self) -> PhraseArrangement {
        use ArrangementRole::*;
        match self {
            IntroArchetype::BassPickup => PhraseArrangement {
                pad: Texture,
                keys: Silent,
                bass: Foundation,
                lead: Silent,
                drums: Silent,
            },
            IntroArchetype::ThemeFragment => PhraseArrangement {
                pad: Texture,
                keys: Silent,
                bass: Silent,
                lead: Support,
                drums: Silent,
            },
            IntroArchetype::HarmonicTease => PhraseArrangement {
                pad: Texture,
                keys: Support,
                bass: Silent,
                lead: Silent,
                drums: Silent,
            },
            IntroArchetype::PedalWithUpperMotion => PhraseArrangement {
                pad: Texture,
                keys: Support,
                bass: Foundation,
                lead: Silent,
                drums: Silent,
            },
        }
    }
}

/// A per-phrase orchestration plan with an enforced foreground budget and first-class silence.
#[derive(Debug, Clone)]
pub struct ArrangementPlan {
    pub phrases: Vec<PhraseArrangement>,
    pub foreground_budget: u8,
}

impl ArrangementPlan {
    /// Derive an arrangement from the form and contract. The foreground rotates so each voice
    /// gets airtime (lead states the hook in A-family phrases, keys carry B contrast), the
    /// intro and coda subtract, and the climax licenses a wider texture.
    pub fn build(
        form: &FormGraph,
        discourse: &DiscoursePlan,
        contract: &CoherenceContract,
    ) -> ArrangementPlan {
        use ArrangementRole::*;
        let budget = contract.foreground_budget.max(1);

        let mut phrases: Vec<PhraseArrangement> = form
            .phrases
            .iter()
            .map(|p| match p.family {
                SectionFamily::Intro => PhraseArrangement {
                    pad: Texture,
                    keys: Silent,
                    bass: Silent,
                    lead: Silent,
                    drums: Silent,
                },
                SectionFamily::A | SectionFamily::APrime { .. } => PhraseArrangement {
                    pad: Texture,
                    keys: Support,
                    bass: Foundation,
                    lead: Foreground,
                    drums: Pulse,
                },
                SectionFamily::B => PhraseArrangement {
                    pad: Texture,
                    keys: Foreground,
                    bass: Foundation,
                    lead: Silent, // the melody breathes; keys carry the contrast
                    drums: Pulse,
                },
                SectionFamily::Break => PhraseArrangement {
                    pad: Texture,
                    keys: Silent,
                    bass: Silent,
                    lead: Silent,
                    drums: Silent, // negative space
                },
                SectionFamily::Climax => PhraseArrangement {
                    pad: Support,
                    keys: if budget >= 2 { Foreground } else { Support },
                    bass: Foundation,
                    lead: Foreground,
                    drums: Pulse, // everyone in — the form licenses the density
                },
                SectionFamily::Coda => PhraseArrangement {
                    pad: Texture,
                    keys: Support,
                    bass: Foundation,
                    lead: Silent,
                    drums: Silent, // wind down
                },
            })
            .collect();

        // Backbone-aware hook routing: every recurring Culminate hook SINGS, even where its
        // positional family would silence the lead (the R5 bug — a hook that landed in a B-family
        // phrase went mute). The budget pass below keeps the lead's priority over other voices.
        for (i, a) in phrases.iter_mut().enumerate() {
            if matches!(discourse.goal(i).role, DiscourseRole::Culminate) {
                a.lead = Foreground;
            }
        }

        // The intro TEASES the song instead of hanging a lone pad: an archetype chosen to
        // foreshadow the phrase that follows, exposing >=2 identity axes over the backbone's
        // already-moving harmonic cell.
        if form
            .phrases
            .first()
            .is_some_and(|p| matches!(p.family, SectionFamily::Intro))
            && !phrases.is_empty()
        {
            let next = 1.min(form.phrases.len().saturating_sub(1));
            phrases[0] = IntroArchetype::foreshadowing(discourse.goal(next).role).arrangement();
        }

        // Enforce the foreground budget: if a phrase over-spends, demote extra foregrounds to
        // support (lead keeps priority as the melodic identity).
        for a in &mut phrases {
            let mut allowed = budget;
            for slot in [
                &mut a.lead,
                &mut a.keys,
                &mut a.pad,
                &mut a.bass,
                &mut a.drums,
            ] {
                if matches!(slot, Foreground) {
                    if allowed == 0 {
                        *slot = Support;
                    } else {
                        allowed -= 1;
                    }
                }
            }
        }

        // Coverage guard: every melodic voice must sound somewhere, or the "all four roles
        // present" invariant breaks and, worse, an instrument the world defined is simply
        // never heard. Promote any fully-silent voice in its best (highest-energy) phrase.
        let mut plan = ArrangementPlan {
            phrases,
            foreground_budget: budget,
        };
        for role in [Role::Pad, Role::Keys, Role::Bass, Role::Lead] {
            plan.ensure_audible_somewhere(form, role);
        }
        plan
    }

    fn ensure_audible_somewhere(&mut self, form: &FormGraph, role: Role) {
        let audible = self.phrases.iter().any(|a| a.role_for(role).is_audible());
        if audible {
            return;
        }
        // Find the highest-energy phrase and give the role a supporting seat there.
        let best = form
            .phrases
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.intent.energy.partial_cmp(&b.1.intent.energy).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);
        let a = &mut self.phrases[best];
        match role {
            Role::Pad => a.pad = ArrangementRole::Texture,
            Role::Keys => a.keys = ArrangementRole::Support,
            Role::Bass => a.bass = ArrangementRole::Foundation,
            Role::Lead => a.lead = ArrangementRole::Support,
        }
    }

    /// Give the lead a seat wherever the discourse poses a Question or delivers an Answer, so the
    /// thematic question/answer gesture is actually heard even in a section family whose base
    /// arrangement would silence the lead. Support (not Foreground) — it does not spend the budget.
    pub fn voice_lead_for_discourse(&mut self, discourse: &DiscoursePlan) {
        for (i, a) in self.phrases.iter_mut().enumerate() {
            let role = discourse.goal(i).role;
            if matches!(role, DiscourseRole::Question | DiscourseRole::Answer)
                && !a.lead.is_audible()
            {
                a.lead = ArrangementRole::Support;
            }
        }
    }

    /// The number of melodic voices audible in the intro phrase (phrase 0). A live intro teases
    /// with >=2 identity axes; a dead intro is a lone pad (1).
    pub fn intro_axes(&self) -> u8 {
        self.phrases
            .first()
            .map(|a| {
                [a.pad, a.keys, a.bass, a.lead]
                    .iter()
                    .filter(|r| r.is_audible())
                    .count() as u8
            })
            .unwrap_or(0)
    }

    /// The arrangement for phrase `ix` (clamped to the last).
    pub fn at(&self, ix: usize) -> PhraseArrangement {
        *self.phrases.get(ix).unwrap_or_else(|| {
            self.phrases
                .last()
                .expect("arrangement plan has no phrases")
        })
    }

    /// Verify no phrase exceeds the foreground budget (a self-check for the diagnostics/tests).
    pub fn validate(&self) -> Result<(), String> {
        for (i, a) in self.phrases.iter().enumerate() {
            let fg = a.foreground_count();
            if fg > self.foreground_budget {
                return Err(format!(
                    "phrase {i} has {fg} foreground voices, budget is {}",
                    self.foreground_budget
                ));
            }
        }
        Ok(())
    }
}

/// The full composition plan — the commitments the score is realized *from*.
///
/// It carries the coherence contract (what stays recognizable), the form graph, the arrangement
/// plan, and — Round III — the [`DiscoursePlan`] (where the piece is going and what each phrase
/// owes the future). The harmonic/motif/groove realizers consume these, not a parallel form.
#[derive(Debug, Clone)]
pub struct CompositionPlan {
    pub contract: CoherenceContract,
    pub form: FormGraph,
    pub arrangement: ArrangementPlan,
    pub discourse: DiscoursePlan,
    /// The world-independent Lift/Deflect/Open/Reset spine (Round VII), when the grammar has one.
    /// Every downstream planner and realizer reads the active gesture from here — the backbone is
    /// no longer invented privately inside realization.
    pub backbone: Option<super::backbone::BackboneTimeline>,
}

impl CompositionPlan {
    /// Build the plan from a semantic trace's timeline and a bar budget, inferring the grammar
    /// from the trace's shape.
    pub fn build(timeline: &IntentTimeline, total_bars: u32) -> CompositionPlan {
        Self::build_with_contract(timeline, total_bars, CoherenceContract::infer(timeline))
    }

    /// Build the plan under an explicitly chosen `contract` — the calibration path, where a piece
    /// is constructed to exercise one grammar (its `ResolutionPolicy`, budgets and anchors) rather
    /// than whatever the trace shape would infer.
    pub fn build_with_contract(
        timeline: &IntentTimeline,
        total_bars: u32,
        contract: CoherenceContract,
    ) -> CompositionPlan {
        Self::build_with_contract_for_beats(
            timeline,
            total_bars.max(1) as f64 * BEATS_PER_BAR,
            contract,
        )
    }

    /// Build the plan for an exact length in beats (which may end inside a bar), inferring the
    /// grammar from the trace's shape — the path [`super::functor::compose_full`] takes, so a
    /// request of 9 beats is a 9-beat piece, not a rounded 8 or 12.
    pub fn build_for_beats(timeline: &IntentTimeline, total_beats: f64) -> CompositionPlan {
        Self::build_with_contract_for_beats(
            timeline,
            total_beats,
            CoherenceContract::infer(timeline),
        )
    }

    /// [`CompositionPlan::build_with_contract`] over an exact `total_beats`: the form spans every bar
    /// the piece touches (the last may be partial) and carries the exact end; the backbone tiles
    /// the same bars (its last slot's end is clamped to the piece by every reader).
    pub fn build_with_contract_for_beats(
        timeline: &IntentTimeline,
        total_beats: f64,
        contract: CoherenceContract,
    ) -> CompositionPlan {
        let form = FormGraph::build_for_beats(timeline, total_beats, &contract);
        let discourse = DiscoursePlan::build(timeline, &form, &contract);
        let mut arrangement = ArrangementPlan::build(&form, &discourse, &contract);
        // The thematic question/answer must be audible: give the lead a seat on Question/Answer
        // phrases even where the family-based arrangement would otherwise silence it.
        arrangement.voice_lead_for_discourse(&discourse);
        let mut backbone = matches!(
            contract.grammar,
            super::contract::CompositionGrammar::DeflectedLift
        )
        .then(|| {
            let starts: Vec<u32> = form.phrases.iter().map(|p| p.start_bar).collect();
            super::backbone::BackboneTimeline::build(
                timeline,
                form.total_bars,
                &starts,
                contract.phrase_bars,
                contract.recurrence_bars,
            )
        });
        if contract.grammar == super::contract::CompositionGrammar::PropulsiveReturn {
            backbone = Some(super::backbone::BackboneTimeline::propulsive_return(
                form.total_bars,
                contract.phrase_bars,
            ));
        }
        CompositionPlan {
            contract,
            form,
            arrangement,
            discourse,
            backbone,
        }
    }

    /// The per-phrase [`PhraseTarget`]s the realizers consume — one per phrase, bundling its
    /// timing/family with its discourse goal. This is the single authority: harmony, groove, bass
    /// and the lead all steer from these, not from a parallel form.
    pub fn targets(&self) -> Vec<PhraseTarget> {
        self.form
            .phrases
            .iter()
            .map(|p| PhraseTarget {
                phrase: *p,
                goal: *self.discourse.goal(p.ix as usize),
            })
            .collect()
    }

    /// A structural dump a cold reader can use to answer *what recurs, what changed, why is
    /// this instrument playing, what obligation is in force*.
    pub fn dump(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let c = &self.contract;
        let anchors: Vec<&str> = c
            .anchors
            .iter()
            .map(|a| match a {
                super::contract::CoherenceAnchor::Motif => "motif",
                super::contract::CoherenceAnchor::Riff => "riff",
                super::contract::CoherenceAnchor::Groove => "groove",
                super::contract::CoherenceAnchor::HarmonicContour => "harmonic-contour",
                super::contract::CoherenceAnchor::HarmonicLoop => "harmonic-loop",
                super::contract::CoherenceAnchor::Form => "form",
                super::contract::CoherenceAnchor::Orchestration => "orchestration",
                super::contract::CoherenceAnchor::BassFigure => "bass-figure",
            })
            .collect();
        let _ = writeln!(
            s,
            "contract: {:?}  anchors=[{}]  recurrence={}bar  phrase={}bar  fg_budget={}  novelty_budget={:.2}  resolution={:?}",
            c.grammar,
            anchors.join(", "),
            c.recurrence_bars,
            c.phrase_bars,
            c.foreground_budget,
            c.novelty_budget,
            c.resolution
        );
        let d = &self.discourse;
        let _ = writeln!(
            s,
            "discourse: thesis@A{} home(E={:.2} T={:.2} R={:.2} D={:.2})  culmination=phrase{}  answer={}",
            d.thesis.established_by,
            d.thesis.home_energy,
            d.thesis.home_tension,
            d.thesis.home_register,
            d.thesis.home_density,
            d.culmination,
            d.answer
                .map(|a| format!("phrase{a}"))
                .unwrap_or_else(|| "none (unresolved)".into()),
        );
        let _ = writeln!(s, "phrases ({}):", self.form.phrases.len());
        for (p, a) in self
            .form
            .phrases
            .iter()
            .zip(self.arrangement.phrases.iter())
        {
            let g = self.discourse.goal(p.ix as usize);
            // Every debt this phrase opens (+) and settles (-): the relation is many-to-many.
            let led = &self.discourse.ledger;
            let debt: String = led
                .created_by(p.ix)
                .map(|o| format!(" +{}", o.id))
                .chain(led.settled_by(p.ix).map(|o| format!(" -{}", o.id)))
                .collect();
            let _ = writeln!(
                s,
                "  [{:>2}] bars {:>2}..{:<2} {:>9}/{:<8} {:>6} T→{:.2} R→{:.2}{}  | pad:{} keys:{} bass:{} lead:{} drums:{}",
                p.ix,
                p.start_bar,
                p.end_bar(),
                g.role.label(),
                g.closure.label(),
                p.family.label(),
                g.tension_target,
                g.register_target,
                debt,
                a.pad.label(),
                a.keys.label(),
                a.bass.label(),
                a.lead.label(),
                a.drums.label(),
            );
        }
        if let Some(bb) = &self.backbone {
            s.push_str(&bb.dump());
        }
        // Obligation ledger: what was owed, when it fell due, and how (and whether on time) it was
        // settled — paid, deflected, late, deferred or abandoned.
        let led = &self.discourse.ledger;
        let _ = writeln!(
            s,
            "obligations ({}): {} resolved, {} late, {} abandoned, {} not yet bound to an action (witnesses bind per performance: PerformancePlan::obligations)",
            led.obligations.len(),
            led.resolved_count(),
            led.late_count(),
            led.abandoned_count(),
            led.unwitnessed_settlements().count(),
        );
        for o in &led.obligations {
            use super::discourse::ObligationStatus;
            let due = o
                .deadline
                .map(|d| format!("phrase{d}"))
                .unwrap_or_else(|| "none".into());
            let status = match (led.final_status(o.id), o.settlement) {
                (Some(ObligationStatus::Settled(how)), Some(st)) => {
                    format!("{} by phrase{}", how.label(), st.by_phrase)
                }
                (Some(ObligationStatus::Late), Some(st)) => {
                    format!("LATE: {} by phrase{}", st.how.label(), st.by_phrase)
                }
                (Some(ObligationStatus::Abandoned), _) => "ABANDONED".into(),
                _ => "left open (deferred)".into(),
            };
            let witness = o
                .settlement
                .and_then(|st| st.witness)
                .map(|w| format!("  witness {w}"))
                .unwrap_or_default();
            let _ = writeln!(
                s,
                "  {:<4} {:<24} opened@phrase{} due@{} → {}{}",
                o.id.to_string(),
                o.kind.label(),
                o.source_phrase,
                due,
                status,
                witness,
            );
        }
        s
    }
}

/// The single per-phrase target every realizer consumes — the phrase (timing, family, span) plus
/// its discourse [`PhraseGoal`] (role, closure, and the energy/tension/density/register targets).
/// This is what makes [`CompositionPlan`] the sole compositional authority: instead of harmony
/// reading one form's tension curve while the lead reads a different form's phrase intent, every
/// realizer projects *this* shared object into its own domain.
#[derive(Debug, Clone, Copy)]
pub struct PhraseTarget {
    pub phrase: Phrase,
    pub goal: PhraseGoal,
}

impl PhraseTarget {
    /// The phrase index.
    pub fn ix(&self) -> u32 {
        self.phrase.ix
    }
    /// First beat of the phrase.
    pub fn start_beat(&self) -> f64 {
        self.phrase.start_beat()
    }
    /// One past the last beat of the phrase.
    pub fn end_beat(&self) -> f64 {
        self.phrase.end_beat()
    }

    /// A minimal flat target for unit tests: a single phrase spanning `bars` from `start_bar`
    /// with constant tension/density targets (used to exercise harmony deterministically without
    /// standing up a whole trace).
    #[cfg(test)]
    pub(crate) fn test_flat(start_bar: u32, bars: u32, tension: f32, density: f32) -> PhraseTarget {
        use super::discourse::{Closure, DiscourseRole};
        let mi = MusicIntent {
            energy: tension,
            tension,
            density,
            ..MusicIntent::default()
        };
        let span = IntentSpan {
            start: mi,
            end: mi,
            peak_energy: mi,
            peak_tension: mi,
            events_inside: 0,
            salient_inside: 0,
            next_salient_beat: None,
        };
        let phrase = Phrase {
            ix: 0,
            start_bar,
            bars,
            family: SectionFamily::A,
            intent: mi,
            span,
            is_rupture: false,
            piece_end: (start_bar + bars) as f64 * BEATS_PER_BAR,
        };
        let goal = PhraseGoal {
            phrase_ix: 0,
            role: DiscourseRole::Intensify,
            closure: Closure::Strong,
            refers_to: None,
            next_goal: None,
            energy_target: tension,
            tension_target: tension,
            density_target: density,
            register_target: 0.5,
            thematic_distance: 0.5,
            harmonic_distance: 0.5,
            novelty_budget: 0.5,
        };
        PhraseTarget { phrase, goal }
    }
}

#[cfg(test)]
mod tests {
    use super::super::semantic::demo_trace;
    use super::*;

    fn deflected_lift_plan() -> CompositionPlan {
        use super::super::contract::{CoherenceContract, CompositionGrammar};
        use super::super::semantic::deflected_lift_trace;
        let tl = IntentTimeline::walk(&deflected_lift_trace(120.0));
        CompositionPlan::build_with_contract(
            &tl,
            30,
            CoherenceContract::for_grammar(CompositionGrammar::DeflectedLift),
        )
    }

    #[test]
    fn the_intro_teases_with_more_than_a_lonely_pad() {
        let plan = deflected_lift_plan();
        assert!(
            plan.arrangement.intro_axes() >= 2,
            "the intro is a lonely pad: {:?}",
            plan.arrangement.phrases.first()
        );
        // Mutation control: a pad-only intro counts one axis — the dead-intro shape.
        let mut dead = plan.arrangement.clone();
        dead.phrases[0] = PhraseArrangement {
            pad: ArrangementRole::Texture,
            keys: ArrangementRole::Silent,
            bass: ArrangementRole::Silent,
            lead: ArrangementRole::Silent,
            drums: ArrangementRole::Silent,
        };
        assert_eq!(
            dead.intro_axes(),
            1,
            "a lone pad should count exactly one axis"
        );
    }

    #[test]
    fn every_recurring_hook_gets_a_foreground_lead() {
        let plan = deflected_lift_plan();
        let culm: Vec<usize> = (0..plan.form.phrases.len())
            .filter(|&i| plan.discourse.goal(i).role == DiscourseRole::Culminate)
            .collect();
        assert!(
            culm.len() >= 2,
            "expected the hook to recur, found {} culminations",
            culm.len()
        );
        for i in culm {
            assert_eq!(
                plan.arrangement.at(i).lead,
                ArrangementRole::Foreground,
                "hook phrase {i} did not put the lead in the foreground"
            );
        }
    }

    fn demo_plan() -> CompositionPlan {
        // The demo arc is 30 bars (120 beats / 4).
        let tl = IntentTimeline::walk(&demo_trace(120.0));
        CompositionPlan::build(&tl, 30)
    }

    #[test]
    fn a_partial_final_bar_is_a_bar_and_the_piece_ends_where_it_was_asked_to() {
        use super::super::semantic::deflected_lift_trace;
        for beats in [9.0, 10.5, 17.0] {
            let tl = IntentTimeline::walk(&deflected_lift_trace(beats));
            let plan = CompositionPlan::build_for_beats(&tl, beats);
            let form = &plan.form;
            assert_eq!(form.total_beats, beats);
            assert_eq!(
                form.total_bars,
                super::super::form::bars_spanning(beats, BEATS_PER_BAR)
            );
            // Phrases tile every bar, and the last one ends at the exact request, not a bar line.
            let mut expect = 0u32;
            for p in &form.phrases {
                assert_eq!(p.start_bar, expect, "{beats}: phrase {} has a gap", p.ix);
                assert!(
                    p.end_beat() <= beats + 1e-9,
                    "{beats}: phrase runs past the end"
                );
                expect = p.end_bar();
            }
            assert_eq!(expect, form.total_bars);
            assert_eq!(form.phrases.last().unwrap().end_beat(), beats);
        }
        // An exact multiple of the bar is unchanged by the rule: 30 bars, ending on bar 30's line.
        let tl = IntentTimeline::walk(&demo_trace(120.0));
        let exact = CompositionPlan::build_for_beats(&tl, 120.0);
        assert_eq!(exact.form.total_bars, 30);
        assert_eq!(exact.form.phrases.last().unwrap().end_beat(), 120.0);
    }

    #[test]
    fn phrases_tile_the_form_on_the_grid() {
        let plan = demo_plan();
        assert!(!plan.form.phrases.is_empty());
        // Phrases tile without gaps and cover exactly total_bars.
        let mut expect = 0u32;
        for p in &plan.form.phrases {
            assert_eq!(p.start_bar, expect, "phrase {} has a gap", p.ix);
            assert!(
                p.bars <= plan.contract.phrase_bars,
                "phrase longer than grid"
            );
            expect = p.end_bar();
        }
        assert_eq!(expect, plan.form.total_bars);
        // First is intro, last is coda, and there is exactly one climax.
        assert_eq!(plan.form.phrases[0].family, SectionFamily::Intro);
        assert_eq!(
            plan.form.phrases.last().unwrap().family,
            SectionFamily::Coda
        );
        assert_eq!(
            plan.form
                .phrases
                .iter()
                .filter(|p| matches!(p.family, SectionFamily::Climax))
                .count(),
            1
        );
    }

    #[test]
    fn a_recurs_and_a_prime_points_at_its_base() {
        let plan = demo_plan();
        let a_ix: Vec<u32> = plan
            .form
            .phrases
            .iter()
            .filter(|p| matches!(p.family, SectionFamily::A))
            .map(|p| p.ix)
            .collect();
        assert_eq!(a_ix.len(), 1, "there should be exactly one base A");
        let base = a_ix[0];
        // Every A' must point back at that base A (an explicit relationship, not a coincidence).
        for p in &plan.form.phrases {
            if let SectionFamily::APrime { base: b } = p.family {
                assert_eq!(b, base, "A' at {} does not reference the base A", p.ix);
                assert!(p.ix > base, "A' precedes its base");
            }
        }
        // A-family recurs (base A plus at least one A').
        assert!(
            plan.form.family_members(SectionFamily::A).len() >= 2,
            "the recurring anchor never recurs"
        );
    }

    #[test]
    fn salient_semantic_events_align_to_phrase_boundaries() {
        // Round III: the Confirmation at beat 88 (bar 22) and SectionResolved at beat 104 (bar 26)
        // used to land mid-phrase on the 4-bar grid (…,20,24,…) and be swallowed by the phrase's
        // start snapshot. They must now each *start* a phrase, and no phrase may cross a salient
        // event strictly inside it (all demo salient bars are even, so snapping is exact).
        let plan = demo_plan();
        let starts: Vec<u32> = plan.form.phrases.iter().map(|p| p.start_bar).collect();
        assert!(
            starts.contains(&22),
            "Confirmation (bar 22) does not start a phrase: {starts:?}"
        );
        assert!(
            starts.contains(&26),
            "SectionResolved (bar 26) does not start a phrase: {starts:?}"
        );
        for p in &plan.form.phrases {
            assert!(
                !p.span.crosses_salient(),
                "phrase {} (bars {}..{}) swallows a salient event mid-phrase",
                p.ix,
                p.start_bar,
                p.end_bar()
            );
        }
    }

    #[test]
    fn arrangement_respects_the_foreground_budget() {
        let plan = demo_plan();
        plan.arrangement.validate().expect("budget violated");
        // The base contract for a hook arc budgets one foreground voice.
        assert_eq!(plan.arrangement.foreground_budget, 1);
    }

    #[test]
    fn silence_is_a_first_class_event_and_every_voice_is_heard_somewhere() {
        let plan = demo_plan();
        // At least one phrase silences at least one voice (the band shuts up).
        let has_silence = plan.arrangement.phrases.iter().any(|a| {
            [a.pad, a.keys, a.bass, a.lead, a.drums]
                .iter()
                .any(|r| matches!(r, ArrangementRole::Silent))
        });
        assert!(has_silence, "nobody ever shuts up — no silence anywhere");
        // ...but every melodic voice is audible in at least one phrase (coverage guard).
        for role in [Role::Pad, Role::Keys, Role::Bass, Role::Lead] {
            assert!(
                plan.arrangement
                    .phrases
                    .iter()
                    .any(|a| a.role_for(role).is_audible()),
                "{role:?} is silent in every phrase"
            );
        }
    }

    #[test]
    fn foreground_is_not_a_loudness_cliff_over_texture() {
        use super::super::world::MusicWorld;
        // Round VI thesis: semantic role decides WHO leads, not BY HOW MUCH. Loudness is a mix
        // decision downstream, not a synonym for "foreground." Three multiplicative gain stages —
        // the role table (this file), the world's per-voice patch gain, and its bus mix — must not
        // compound into a canyon. Baseline BLACK_ICE ran the lead ~11.5 dB over the pad ("pasted
        // 3 dB in front of everybody"); this budget keeps every world well under that, and trips if
        // a future world re-widens any stage.
        const BUDGET_DB: f32 = 6.0;

        // The role stage in isolation (world-independent): the Foreground↔Texture tier Task 1
        // narrowed from a ~7.5 dB cliff. Pins the table in this file directly.
        let role_db =
            20.0 * (ArrangementRole::Foreground.gain() / ArrangementRole::Texture.gain()).log10();
        assert!(
            role_db <= 3.5,
            "the role-gain Foreground↔Texture spread re-widened to {role_db:.2} dB"
        );

        // The full three-stage spread, per world: the lead at Foreground vs the pad at Texture —
        // the exact pairing the A-family arrangement uses, and the exact comparison that measured
        // the baseline cliff.
        for w in MusicWorld::all() {
            let fg = ArrangementRole::Foreground.gain() * w.lead.gain * w.lead_mix;
            let tex = ArrangementRole::Texture.gain() * w.pad.gain * w.pad_mix;
            let db = 20.0 * (fg / tex).log10();
            assert!(
                db <= BUDGET_DB,
                "{}: Foreground lead sits {db:.2} dB over the Texture pad (budget {BUDGET_DB} dB)",
                w.name
            );
        }
    }

    #[test]
    fn dump_reports_families_and_roles() {
        let plan = demo_plan();
        let d = plan.dump();
        assert!(d.contains("contract:"));
        assert!(d.contains("phrases ("));
        assert!(d.contains("foreground") || d.contains("silent"));
    }
}
