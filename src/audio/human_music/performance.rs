//! **PerformancePlan** — one shared performance that every instrument realizes a projection of.
//!
//! Through Round VI each part was realized blind from common static inputs: keys saw only the
//! chords, bass saw chords plus kick times, the lead saw chords plus the plan, and nobody saw
//! anybody else's notes. Call and response was a *slot relation* — `ThematicTrajectory::next_for`
//! handed the call to one discourse role and the answer to another, so "who answers" and "when"
//! were fixed by the form. Five composers shared a skeleton; none of them listened.
//!
//! The pipeline is now
//!
//! ```text
//! SongMap (contract, form, discourse, arrangement envelope, backbone
//!          timeline, thematic map, harmonic map)          — the SONG (Round IX: super::song)
//!        │  + MusicWorld (timbre)  + MusicalLanguage (idiom)
//!        ▼
//! PerformancePlan                                         — the shared PERFORMANCE
//!   harmony + HarmonicContext timeline (local pitch palettes, relations, expectations)
//!   ActionPlan (path-lifted morphisms + the spine's own verbs + interaction actions)
//!   AccentGrid (one rhythmic field every player draws different subsets from)
//!   lead statement plan + calls → ResponseWindows → Interactions (with InteractionMemory)
//!   EnsembleBar per bar (who is foreground, each player's mode, the complexity budget,
//!                        the kinetic target)
//!        ▼
//! Score realization — lead, keys, bass, drums, pad each read THE SAME plan, and later players
//!                     read the earlier players' notes (they listen).
//! ```

use super::action::{ActionCause, ActionKind, ActionPlan, Agent, Deferral};
use super::backbone::{DeflectWitness, HarmonicGesture};
use super::context::{analyze_regions, HarmonicContext};
use super::ensemble::{plan_ensemble, Stage};
use super::form::BEATS_PER_BAR;
use super::harmony::{ChordSpan, HarmonyEngine};
use super::ids::{ActionId, InteractionId, MaterialId};
use super::interaction::plan_interactions;
use super::language::MusicalLanguage;
use super::material::{InteractionMaterial, MaterialSource};
use super::motif::MotifBank;
use super::plan::CompositionPlan;
use super::region::{apply_harmonic_actions, HarmonicFrame, RegionTimeline};
use super::rng::Rng;
use super::theory::Scale;
use super::timeline::IntentTimeline;
use super::world::MusicWorld;

pub use super::ensemble::{BassMode, DrumsMode, EnsembleBar, KeysMode, PadMode};
pub use super::interaction::{
    Call, CallPolicy, Interaction, InteractionMemory, InteractionOpportunity, LeadStatement,
    OpportunitySource, Response, ResponseMode, Transform, Verdict,
};
pub use super::region::HarmonicEdit;

/// Sixteenth-note steps per 4/4 bar.
pub const STEPS: usize = 16;
/// Beats per step.
pub const STEP_BEATS: f64 = BEATS_PER_BAR / STEPS as f64;

/// The accent weights of one grid step, each in `[0, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StepWeight {
    /// Metric structure (the downbeat, beat 3).
    pub structural: f32,
    /// The backbeat (beats 2 and 4).
    pub backbeat: f32,
    /// An opportunity to syncopate (off-beats, the piece's gesture rhythm cells).
    pub syncopation: f32,
    /// A lead-in onto the next structural event.
    pub pickup: f32,
    /// An anticipation push onto a target.
    pub push: f32,
    /// A planned hole: players should leave it empty.
    pub hole: f32,
    /// An ensemble hit.
    pub hit: f32,
}

/// The shared rhythmic field: one [`StepWeight`] per sixteenth of every bar.
#[derive(Debug, Clone, PartialEq)]
pub struct AccentGrid {
    pub bars: Vec<[StepWeight; STEPS]>,
    /// The rhythm cell (off-beat steps) each gesture uses in this piece — its rhythmic identity.
    pub cells: [Vec<usize>; 4],
}

impl AccentGrid {
    /// Exact metric identity of an indexed sixteenth; never a performed swing coordinate.
    /// Oversized legacy step indices are rejected rather than overflowing the identity.
    pub fn metric_of(bar: u32, step: usize) -> Option<super::rhythm::MetricPosition> {
        let index = i64::from(bar)
            .checked_mul(STEPS as i64)?
            .checked_add(i64::try_from(step).ok()?)?;
        super::rhythm::MetricPosition::new(index, 4)
    }

    /// The beat of `(bar, step)`.
    pub fn beat_of(bar: u32, step: usize) -> f64 {
        bar as f64 * BEATS_PER_BAR + step as f64 * STEP_BEATS
    }

    /// Project `beat` to the nearest sixteenth step (half-step ties round upward).
    /// A rounded bar-end step becomes the next downbeat; this is not a floor projection.
    /// Negative input clamps to the first step, preserving the legacy API's domain boundary.
    pub fn step_of(beat: f64) -> (u32, usize) {
        let b = (beat / BEATS_PER_BAR).floor().max(0.0);
        let s = ((beat - b * BEATS_PER_BAR) / STEP_BEATS).round() as usize;
        if s >= STEPS {
            (b as u32 + 1, 0)
        } else {
            (b as u32, s)
        }
    }

    /// The weights at `(bar, step)` (zero outside the piece).
    pub fn at(&self, bar: u32, step: usize) -> StepWeight {
        self.bars
            .get(bar as usize)
            .map(|b| b[step.min(STEPS - 1)])
            .unwrap_or_default()
    }

    /// The weights at `beat`.
    pub fn at_beat(&self, beat: f64) -> StepWeight {
        let (b, s) = Self::step_of(beat);
        self.at(b, s)
    }

    /// Whether `beat` falls in a planned hole.
    pub fn is_hole(&self, beat: f64) -> bool {
        self.at_beat(beat).hole >= 0.5
    }
}

fn gesture_ix(g: HarmonicGesture) -> usize {
    match g {
        HarmonicGesture::Lift => 0,
        HarmonicGesture::Deflect => 1,
        HarmonicGesture::Open => 2,
        HarmonicGesture::Reset => 3,
    }
}

/// Options that select the calibration probes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerformanceOptions {
    pub language: MusicalLanguage,
    /// How the pitched players' simultaneous notes are chosen: `Independent` (the R7b band, the
    /// default the listen prefers), `CoupledR8` (the Round VIII joint bed — the rejected negative
    /// control) or `Surgical` (R7b, then only its real hard vertical defects repaired locally).
    pub coupling: EnsembleCoupling,
    /// Build the action plan (false = the mood-without-action probe).
    pub actions: bool,
    pub responses: ResponseMode,
    /// When a lead statement becomes a call (`EveryStatement` = the saturation probe).
    pub calls: CallPolicy,
    /// How gesture manifestations vary across cycles (`Fixed` = the gesture-rigidity probe).
    pub manifestations: super::action::ManifestationPolicy,
}

impl Default for PerformanceOptions {
    fn default() -> Self {
        PerformanceOptions {
            language: MusicalLanguage::default(),
            coupling: EnsembleCoupling::Independent,
            actions: true,
            responses: ResponseMode::Free,
            calls: CallPolicy::Selective,
            manifestations: super::action::ManifestationPolicy::Varied,
        }
    }
}

/// How the pitched players' simultaneous notes are realized.
///
/// The Round VIII listen rejected the coupled bed ("trash": it revoiced three quarters of the pad to
/// satisfy the collision ruler — pad motion 4.7 -> 7.6 semitones, common tones 1.3 -> 0.9), so the
/// default is the R7b band again until the ear approves another path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EnsembleCoupling {
    /// The R7b band, byte-identical (the default): lead, keys, pad and bass each choose their
    /// pitches alone (the keys hear the lead softly; the pad and the bass hear nobody's pitches).
    #[default]
    Independent,
    /// Round VIII, kept as the NEGATIVE CONTROL the listen rejected: the band realizes one harmonic
    /// state in rigidity order (lead, bass, the keys' material lines, then the pad and keys bed
    /// solved JOINTLY against everything already sounding). Pinned byte-for-byte at 5644c96.
    CoupledR8,
    /// Round VIIIb: the R7b realization, then [`super::surgical::repair`] — only the REAL hard
    /// vertical defects (measured over actual audible overlap) repaired, each by the smallest local
    /// edit of the offending support note, every edit recorded in `Score::vertical_repairs`.
    Surgical,
}

/// What the stage did with an action whose initiator the arrangement envelope had off stage —
/// decided BEFORE realization, so no realized action is ever deleted afterwards.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Admission {
    /// The player is brought on stage for the action's window (a pickup or fill from somebody
    /// about to enter is idiomatic).
    Admitted { agent: Agent },
    /// The verb is carried by somebody who is there, or as the subtraction the arrangement
    /// already plans.
    Recast {
        from_kind: ActionKind,
        from: Agent,
        to_kind: ActionKind,
        to: Agent,
    },
    /// Nobody can perform it: removed before realization and deferred with the reason.
    Rejected { reason: &'static str },
}

/// One admission decision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdmissionRecord {
    /// The action's id after admission (`None` when it was rejected and removed).
    pub action: Option<ActionId>,
    pub kind: ActionKind,
    pub start_beat: f64,
    pub outcome: Admission,
}

/// The performance's time domain `[0, total_beats]`: every planned window (action, stage,
/// ownership, material, statement, chord, region) and every realized event inhabits it — an
/// onset strictly before the end, an end no later than it. The one clipping authority for the
/// windows the planner creates and derives, so a partial final bar is never outlived by a window
/// computed from an earlier bar (holdout v2 H12).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerformanceDomain {
    total_beats: f64,
}

impl PerformanceDomain {
    /// Onsets closer than this to the end have no time left to be performed.
    pub const EPS: f64 = 1e-6;

    /// The domain of a piece `total_beats` long.
    pub fn new(total_beats: f64) -> Self {
        Self { total_beats }
    }

    /// The piece's length.
    pub fn total_beats(self) -> f64 {
        self.total_beats
    }

    /// The part of a window starting at `start` for `dur` beats that the piece performs: its
    /// duration, unchanged bit for bit when it already fits, cut at the end otherwise; `None`
    /// when it starts outside the domain or nothing of it remains.
    pub fn fit(self, start: f64, dur: f64) -> Option<f64> {
        if !start.is_finite() || !dur.is_finite() || start < 0.0 {
            return None;
        }
        let left = self.total_beats - start;
        let dur = if dur > left { left } else { dur };
        (dur > Self::EPS).then_some(dur)
    }

    /// `[start, end)` intersected with the domain — returned exactly as given when it already
    /// fits; `None` when nothing of it remains.
    pub fn clip(self, start: f64, end: f64) -> Option<(f64, f64)> {
        if !start.is_finite() || !end.is_finite() {
            return None;
        }
        let start = start.max(0.0);
        let end = end.min(self.total_beats);
        (end - start > Self::EPS).then_some((start, end))
    }

    /// Whether `[start, end)` lies inside the domain as given.
    pub fn contains(self, start: f64, end: f64) -> bool {
        start.is_finite()
            && end.is_finite()
            && start >= 0.0
            && start < self.total_beats - Self::EPS
            && end >= start
            && end <= self.total_beats + Self::EPS
    }
}

/// The whole shared performance.
#[derive(Clone)]
pub struct PerformancePlan {
    /// The fingerprint of the song this performs ([`super::song::SongMap::fingerprint`]) — the
    /// performance's CLAIM; [`super::song::SongMapConformance`] checks the content.
    pub song_fingerprint: u64,
    pub language: MusicalLanguage,
    /// Coupled or independent ensemble realization (see [`EnsembleCoupling`]).
    pub coupling: EnsembleCoupling,
    /// The HOME region (the world's tonic + mode). The region in force at a beat is
    /// [`Self::region_at`]: a Modulate moves it for a planned span.
    pub region: Scale,
    /// The tonal-region timeline (home, and every pivot / modulated span / return).
    pub regions: RegionTimeline,
    pub chords: Vec<ChordSpan>,
    pub contexts: Vec<HarmonicContext>,
    pub deflects: Vec<DeflectWitness>,
    pub edits: Vec<HarmonicEdit>,
    pub actions: ActionPlan,
    pub accent: AccentGrid,
    pub statements: Vec<LeadStatement>,
    pub interactions: Vec<Interaction>,
    pub ensemble: Vec<EnsembleBar>,
    pub bank: MotifBank,
    pub response_mode: ResponseMode,
    pub call_policy: CallPolicy,
    pub total_beats: f64,
    /// Every piece of interaction material (index = [`MaterialId`]): statements, figures, and
    /// the derived material of every response.
    pub materials: Vec<InteractionMaterial>,
    /// Every chance for a conversation the planner weighed, with its verdict.
    pub opportunities: Vec<InteractionOpportunity>,
    /// The single orchestration authority: who plays, where, how loud.
    pub stage: Stage,
    /// How actions were reconciled with the arrangement envelope before realization.
    pub admissions: Vec<AdmissionRecord>,
    /// The plan's discourse obligations with each settlement bound to the concrete action that
    /// discharges it in THIS performance (a paid suspended cadence cites its resolution).
    pub obligations: super::discourse::ObligationLedger,
    /// The shared complexity allocation per bar (planned before anybody plays; every realizer
    /// consumes its share).
    pub budget: Vec<super::budget::ComplexityAllocation>,
    /// Optional canonical cover constraints. Absent on every historical path.
    pub cover_constraints: Option<super::cover::CoverConstraints>,
    /// Rehearsed admission's record: every planned verb's outcome and every debt left open.
    /// Absent on every historical (`Planned`) path.
    pub rehearsal: Option<super::rehearsal::RehearsalTrace>,
    /// The law the bass and keys realize their pitch functions under. Archived on every
    /// historical path.
    pub functions: super::policy::FunctionPolicy,
    /// Opt-in life for the lead voice. Every axis off (the default) is the byte-exact v0.4 lead.
    pub lead_life: super::policy::LeadLifePolicy,
    /// The ensemble narrative this performance enacts — who carries which meaning, derived from
    /// μ(song) — or `None` on every historical path and whenever μ never leaves the lead. Opt-in via
    /// [`super::policy::NarrativePolicy::Ensemble`].
    pub narrative: Option<super::narrative::NarrativePlan>,
}

/// The ordinary twin of the cover path's settlement planner: a song obligation the discourse
/// settles in a phrase gets the event that musically discharges it there, generated by the
/// planner — never a stamp on something already played.
///
/// - GrooveDestabilization: the kit's re-entry at the start of the settling phrase.
/// - SuspendedCadence / HarmonicDeparture: a Resolve on the first home chord inside the settling
///   phrase (who arrives is decided later by who is on stage there).
///
/// A phrase that already holds such a verb is left alone. Other kinds are discharged by the
/// interaction planner (answers) or are unwitnessable by design (register ascent).
fn plan_settlements(
    plan: &CompositionPlan,
    chords: &[ChordSpan],
    home: Scale,
    actions: &mut ActionPlan,
) {
    use super::action::{EffectVector, MusicalAction};
    use super::discourse::ObligationKind;
    for debt in &plan.discourse.ledger.obligations {
        let Some(settlement) = debt.settlement else {
            continue;
        };
        let Some(phrase) = plan.form.phrases.get(settlement.by_phrase as usize) else {
            continue;
        };
        let (start, end) = (phrase.start_beat(), phrase.end_beat());
        let (kind, initiator, at) = match debt.kind {
            // Admission brings the drummer on for the re-entry window even where the arrangement
            // seats the kit Silent: the return is a real, brief event, exactly as on the cover path.
            ObligationKind::GrooveDestabilization => (ActionKind::ReEntry, Agent::Drums, start),
            ObligationKind::SuspendedCadence | ObligationKind::HarmonicDeparture => {
                let Some(home_chord) = chords.iter().find(|c| {
                    c.start_beat >= start - 1e-9
                        && c.start_beat < end - 1e-9
                        && c.chord.root_pc.rem_euclid(12) == home.tonic_pc.rem_euclid(12)
                }) else {
                    continue;
                };
                (ActionKind::Resolve, Agent::Bass, home_chord.start_beat)
            }
            _ => continue,
        };
        if actions
            .of_kind(kind)
            .any(|a| a.start_beat >= start - 1e-9 && a.start_beat < end - 1e-9)
        {
            continue;
        }
        actions.push(MusicalAction {
            id: ActionId(0),
            cause: ActionCause::Discourse {
                obligation: debt.id,
                phrase: phrase.ix,
            },
            initiator,
            start_beat: at,
            dur_beats: (end - at).min(1.0),
            kind,
            target_beat: Some(at),
            responders: Vec::new(),
            binding: None,
            pays: None,
            effect: EffectVector::NEUTRAL,
        });
    }
}

pub use super::rehearsal::ActionKey;

/// Plan-time admission inputs chosen by the realization profile. Only the function law is
/// stored in the plan (its realizers read it), and only when it is not the archived one, so a
/// historical (`Planned`) build is the exact historical plan.
#[derive(Debug, Clone, Default)]
pub(crate) struct AdmissionInputs {
    /// Rehearsed admission: each settled song obligation gets its discharging verb from the source
    /// planner, and a settlement stands only while a performed verb discharges it (otherwise the
    /// debt is left open, named — [`super::rehearsal::OpenDebt`]).
    pub rehearsed: bool,
    /// Verbs a rehearsal of this same plan found no player performing, struck wherever a planner
    /// makes them.
    pub vetoed: Vec<ActionKey>,
    /// Settlement verbs a rehearsal moved onto the arrival a player actually made.
    pub recast: Vec<super::rehearsal::Recast>,
    /// The archived colours, or the world/language harmonic vocabulary as a source law.
    pub harmony: super::policy::HarmonyPolicy,
    /// The archived support tolerances, or support functions earned where they sound.
    pub functions: super::policy::FunctionPolicy,
    /// Opt-in life for the lead voice. Every axis off (the default) is the byte-exact v0.4 lead.
    pub lead_life: super::policy::LeadLifePolicy,
    /// Opt-in ensemble narrative. [`super::policy::NarrativePolicy::Archived`] (the default) is the
    /// byte-exact v0.4 arrangement; `Ensemble` derives a per-phrase carrier assignment from μ(song).
    pub narrative: super::policy::NarrativePolicy,
}

/// The reason a rehearsed verb is struck from the chart before the take.
pub const REHEARSAL_REJECTION: &str = "rehearsed: no player performed this verb here";

impl PerformancePlan {
    /// The one place a performance is generated: `song` played in `world`'s room, spoken in
    /// `opts.language`. Everything the song fixes is read from it; everything decided here is the
    /// fiber (see [`super::song`]).
    pub fn from_song(
        song: &super::song::SongMap,
        world: &MusicWorld,
        opts: PerformanceOptions,
    ) -> PerformancePlan {
        Self::build(song, world, opts, None, &AdmissionInputs::default())
            .expect("unconstrained planner has no cover domain to reject")
    }

    pub(crate) fn from_song_admitted(
        song: &super::song::SongMap,
        world: &MusicWorld,
        opts: PerformanceOptions,
        constraints: Option<super::cover::CoverConstraints>,
        admission: &AdmissionInputs,
    ) -> Result<PerformancePlan, super::cover::CoverError> {
        match constraints {
            Some(c) => Self::constrained(song, world, opts, c, admission),
            None => Self::build(song, world, opts, None, admission),
        }
    }

    /// Apply invariant constraints before any dependent planner makes a choice.
    pub fn from_song_constrained(
        song: &super::song::SongMap,
        world: &MusicWorld,
        opts: PerformanceOptions,
        constraints: super::cover::CoverConstraints,
    ) -> Result<PerformancePlan, super::cover::CoverError> {
        Self::constrained(song, world, opts, constraints, &AdmissionInputs::default())
    }

    fn constrained(
        song: &super::song::SongMap,
        world: &MusicWorld,
        opts: PerformanceOptions,
        constraints: super::cover::CoverConstraints,
        admission: &AdmissionInputs,
    ) -> Result<PerformancePlan, super::cover::CoverError> {
        constraints.identity.validate()?;
        if constraints.identity.length.is_none() {
            return Err(super::cover::CoverError::Invalid(
                "metric constraints require a declared extent",
            ));
        }
        constraints.validate_target()?;
        // A pinned kit plays its canonical pattern whatever the phrase role, so it can perform no
        // groove return: a song owing one must be planned with a pinned kit
        // (`KitMotion::Pinned`, as `cover_candidate` does). Refused here, not asserted later.
        if constraints.identity.groove.is_some()
            && song
                .plan
                .discourse
                .ledger
                .obligations
                .iter()
                .any(|o| o.kind == super::discourse::ObligationKind::GrooveDestabilization)
        {
            return Err(super::cover::CoverError::Invalid(
                "a pinned kit cannot carry a groove debt: plan the song with a pinned kit",
            ));
        }
        Self::build(song, world, opts, Some(constraints), admission)
    }

    fn build(
        song: &super::song::SongMap,
        world: &MusicWorld,
        opts: PerformanceOptions,
        cover_constraints: Option<super::cover::CoverConstraints>,
        admission: &AdmissionInputs,
    ) -> Result<PerformancePlan, super::cover::CoverError> {
        let (timeline, plan, seed) = (&song.timeline, &song.plan, song.seed);
        let lang = opts.language;
        let region = Scale::new(world.tonic_pc, world.mode);
        // The one harmonic-vocabulary law, when the profile makes it a source law: every chord
        // producer below chooses inside it, and a chart chord it cannot admit refuses the plan.
        let vocabulary = (admission.harmony == super::policy::HarmonyPolicy::Vocabulary)
            .then(|| super::vocabulary::HarmonicVocabulary::of(world, &lang));
        // The exact requested length — a final partial bar ends here, not on the next bar line.
        let total_beats = plan.form.total_beats;
        let targets = plan.targets();
        // Opt-in ensemble narrative (band-story round): derive who carries which meaning from
        // μ(song) — what the song ACTUALLY means (`observe`), never F(trace) (plan §4/§20). Kept only
        // when it hands the germ off the lead; a μ that never leaves the lead is the historical
        // arrangement, so the plan stores None and the performance is byte-exact v0.4. Carried on the
        // plan here; the realizer enacts it in a later wave.
        let narrative = (admission.narrative == super::policy::NarrativePolicy::Ensemble)
            .then(|| {
                super::narrative::NarrativePlan::from_observation(
                    &super::meaning::MeaningPlan::observe(song),
                )
            })
            .filter(super::narrative::NarrativePlan::hands_off);

        // 1. Harmony: the song's chart realized in this room and language (Round IX: the room
        //    re-modes and colours the chart; it no longer searches its own), or the phrase engine.
        let (mut chords, mut deflects, home_chord) = if let Some(chords) = cover_constraints
            .as_ref()
            .map(|c| c.harmony(world, &lang))
            .transpose()?
            .flatten()
        {
            (chords, Vec::new(), None)
        } else {
            match (&plan.backbone, &song.harmonic) {
                (Some(tl), Some(hm)) => {
                    let r = super::backbone::realize_in(tl, hm, world, &lang, vocabulary.as_ref());
                    (r.spans, r.deflects, Some(r.cell.reset))
                }
                _ => (
                    HarmonyEngine::new(world, seed)
                        .with_vocabulary(vocabulary)
                        .generate(&targets, plan.contract.resolution),
                    Vec::new(),
                    None,
                ),
            }
        };
        // The backbone tiles whole bars; a partial final bar's harmony ends with the piece.
        chords.retain(|c| c.start_beat < total_beats - 1e-9);
        for c in &mut chords {
            if c.start_beat + c.dur_beats as f64 > total_beats + 1e-9 {
                c.dur_beats = (total_beats - c.start_beat) as f32;
            }
        }
        for d in &mut deflects {
            d.rejoin_beats = d.rejoin_beats.min(total_beats - d.at_beat);
        }

        // 2. Actions.
        let mut actions = if opts.actions {
            ActionPlan::build_with(
                timeline,
                plan.backbone.as_ref(),
                &lang,
                total_beats,
                seed,
                opts.manifestations,
            )
        } else {
            ActionPlan::none()
        };

        // 2b. ONE orchestration authority: the stage is seeded from the arrangement envelope and
        //     every action is reconciled with it NOW — admitted, recast or rejected — instead of
        //     being realized and then deleted by a stale phrase role.
        let mut constrained_rejections = Vec::new();
        if cover_constraints.as_ref().is_some_and(|c| {
            c.identity.harmony.is_some()
                || c.identity.line(super::score::Role::Lead).is_some()
                || c.identity.line(super::score::Role::Bass).is_some()
        }) {
            let rejected: Vec<_> = actions
                .actions
                .iter()
                .filter(|a| {
                    matches!(
                        a.kind,
                        ActionKind::Tonicize
                            | ActionKind::Reharmonize
                            | ActionKind::Recolor
                            | ActionKind::Modulate
                    )
                })
                .map(|a| {
                    constrained_rejections.push(AdmissionRecord {
                        action: None,
                        kind: a.kind,
                        start_beat: a.start_beat,
                        outcome: Admission::Rejected {
                            reason: "cover harmonic candidate domain",
                        },
                    });
                    a.id
                })
                .collect();
            // No stage, material or dependent action IDs exist yet.
            let _remap = actions.remove(&rejected);
        }
        // A rehearsal of this same plan found nobody performing these verbs: they are struck from
        // the chart now — before the settlement planner looks for a discharge (so it never counts
        // on a struck verb) and again after it — before any stage, material or dependent id
        // exists, and recorded.
        let strike = |actions: &mut ActionPlan, records: &mut Vec<AdmissionRecord>| {
            if admission.vetoed.is_empty() {
                return;
            }
            let vetoed: Vec<ActionId> = actions
                .actions
                .iter()
                .filter(|a| admission.vetoed.contains(&ActionKey::of(a, actions)))
                .map(|a| a.id)
                .collect();
            for &id in &vetoed {
                let a = actions.get(id).expect("vetoed action exists");
                records.push(AdmissionRecord {
                    action: None,
                    kind: a.kind,
                    start_beat: a.start_beat,
                    outcome: Admission::Rejected {
                        reason: REHEARSAL_REJECTION,
                    },
                });
                if let ActionCause::Morphism {
                    transition,
                    morphism,
                } = a.cause
                {
                    actions.deferred.push(Deferral {
                        transition,
                        morphism,
                        reason: REHEARSAL_REJECTION,
                    });
                }
            }
            let _remap = actions.remove(&vetoed);
        };
        if admission.rehearsed {
            strike(&mut actions, &mut constrained_rejections);
        }
        if let Some(c) = cover_constraints.as_ref().filter(|_| opts.actions) {
            c.plan_settlements(plan, &chords, &mut actions);
        } else if admission.rehearsed && opts.actions {
            plan_settlements(plan, &chords, region, &mut actions);
        }
        // A settlement a rehearsal found unplayed where it was planned moves onto the arrival a
        // player made in its phrase (performed there by that player).
        for r in &admission.recast {
            let domain = PerformanceDomain::new(total_beats);
            if let Some(a) = actions.actions.iter_mut().find(|a| {
                matches!(a.cause, ActionCause::Discourse { obligation, .. } if obligation == r.obligation)
            }) {
                let dur = domain.fit(r.start_beat, a.dur_beats).unwrap_or(a.dur_beats);
                a.start_beat = r.start_beat;
                a.target_beat = Some(r.start_beat);
                a.initiator = r.by;
                a.dur_beats = dur;
            }
        }
        strike(&mut actions, &mut constrained_rejections);
        let mut stage = Stage::from_arrangement(plan);
        if let Some(c) = &cover_constraints {
            c.apply_stage(&mut stage);
            c.pinned_strokes_seated(&stage)?;
        }
        let mut admissions = admit_actions(&mut actions, &mut stage);
        admissions.extend(constrained_rejections);

        // 3. Harmonic actions edit the harmony (so they are heard, not merely labelled) and a
        //    Modulate moves the tonal region itself; every context is analysed in its own region,
        //    and the misses are re-measured on the harmony that actually sounds.
        let frame = HarmonicFrame::from_plan(plan, timeline, region, total_beats)
            .with_vocabulary(vocabulary);
        let (mut edits, mut regions) = apply_harmonic_actions(&mut chords, &mut actions, &frame);
        // A harmonic verb the harmony could not perform anywhere in its window (no lawful
        // substitute, no recolouring, no applied dominant — e.g. a Reharmonize over a m7b5) is
        // not left in the plan as an unwitnessable promise: it is removed before anybody plays
        // and deferred with the reason.
        let harmonic = [
            ActionKind::Tonicize,
            ActionKind::Reharmonize,
            ActionKind::Recolor,
            ActionKind::Modulate,
        ];
        let unperformed: Vec<ActionId> = actions
            .actions
            .iter()
            .filter(|a| harmonic.contains(&a.kind))
            .filter(|a| {
                !edits.iter().any(|e| e.action == a.id)
                    && !regions.spans.iter().any(|sp| sp.cause == Some(a.id))
            })
            .map(|a| a.id)
            .collect();
        if !unperformed.is_empty() {
            let reason = "the harmony offers no lawful edit anywhere in its window";
            for &id in &unperformed {
                if let Some(a) = actions.get(id) {
                    admissions.push(AdmissionRecord {
                        action: None,
                        kind: a.kind,
                        start_beat: a.start_beat,
                        outcome: Admission::Rejected { reason },
                    });
                    if let ActionCause::Morphism {
                        transition,
                        morphism,
                    } = a.cause
                    {
                        actions.deferred.push(Deferral {
                            transition,
                            morphism,
                            reason,
                        });
                    }
                }
            }
            let remap = actions.remove(&unperformed);
            for e in &mut edits {
                e.action = remap(e.action).expect("an edited action is performed");
            }
            for sp in &mut regions.spans {
                sp.cause = sp.cause.and_then(&remap);
            }
            for r in &mut regions.relabels {
                if let Some(n) = remap(r.action) {
                    r.action = n;
                }
            }
            for w in &mut stage.windows {
                if let Some(n) = remap(w.action) {
                    w.action = n;
                }
            }
            for r in &mut admissions {
                r.action = r.action.and_then(&remap);
            }
        }
        // Every producer above chose inside the vocabulary; what remains outside it is a chord the
        // song itself names (a borrowed chart root) that this room cannot sound. Refuse, typed,
        // before anybody plays — never recolour a root.
        if let Some(v) = &vocabulary {
            if chords.iter().any(|c| !v.admits(c.chord)) {
                return Err(super::cover::CoverError::Invalid(
                    super::vocabulary::VOCABULARY_REFUSAL,
                ));
            }
        }
        let contexts = analyze_regions(&chords, &regions);
        if let (Some(tl), Some(home_chord)) = (&plan.backbone, home_chord) {
            deflects = super::backbone::deflect_witnesses(
                tl,
                &chords,
                &|b| regions.region_at(b),
                home_chord,
            );
        }

        // 4. The shared rhythmic field.
        let accent = build_accent_grid(plan, &actions, &lang, seed);

        // 5. Lead statements, materials, opportunities, calls, responses.
        // The song's own theme: the bank and every site come from the SongMap (Round IX), never
        // from this room's mode.
        let bank = song.thematic.bank.clone();
        let ip = plan_interactions(
            plan,
            &mut actions,
            &accent,
            &chords,
            &song.thematic,
            &lang,
            &opts,
            &stage,
            seed,
            cover_constraints.as_ref(),
            &admission.vetoed,
            admission.functions,
            admission.lead_life,
            narrative.as_ref(),
        );
        let (statements, interactions) = (ip.statements, ip.interactions);
        // 5b. A resolution is performed by whoever ARRIVES: the lead when a statement sounds at
        //     the target, otherwise the bass (or keys) who are there — never a silent lead.
        for a in actions.actions.iter_mut().filter(|a| {
            // A recast settlement is already performed by the player a rehearsal heard arrive.
            a.kind == ActionKind::Resolve
                && !admission.recast.iter().any(|r| {
                    matches!(a.cause, ActionCause::Discourse { obligation, .. } if obligation == r.obligation)
                })
        }) {
            let t = a.target_beat.unwrap_or(a.start_beat);
            let lead_sings = statements
                .iter()
                .any(|st| st.start_beat <= t + 0.5 && st.end_beat() > t + 0.25);
            let by = if lead_sings && stage.on_stage(Agent::Lead, t) {
                Some(Agent::Lead)
            } else {
                [Agent::Bass, Agent::Keys]
                    .into_iter()
                    .find(|&g| stage.on_stage(g, t))
            };
            if let Some(g) = by {
                a.initiator = g;
            }
        }

        // 5c. Intended stillness is declared (after every action, calls and answers included,
        //     exists), so it is not mistaken for accidental inactivity.
        if opts.actions {
            actions.stasis = declare_stasis(timeline, plan, &actions, total_beats);
        }

        // 6. The ensemble per bar.
        let ensemble = plan_ensemble(plan, &actions, &statements, &interactions, &lang, &stage);

        // 7. Bind every settled discourse debt to the action that discharges it here.
        let mut obligations = plan.discourse.ledger.clone();
        super::discourse::bind_settlement_witnesses(&mut obligations, &actions, |ix| {
            plan.form
                .phrases
                .get(ix as usize)
                .map(|p| (p.start_beat(), p.end_beat()))
                .unwrap_or((0.0, 0.0))
        });
        // Rehearsed: every verb left on this chart is performed (the rehearsal converged), so a
        // bound witness is a performed discharge. A debt the discourse settles here with no such
        // verb is not settled by this performance: it stays open, named. The phrase role alone
        // never makes a settlement true.
        let rehearsal = admission.rehearsed.then(|| {
            let mut open_debts = Vec::new();
            for o in &mut obligations.obligations {
                let Some(s) = o.settlement.filter(|s| s.witness.is_none()) else {
                    continue;
                };
                let rejected = admission.vetoed.iter().any(|k| k.settles() == Some(o.id));
                open_debts.push(super::rehearsal::OpenDebt {
                    obligation: o.id,
                    kind: o.kind,
                    by_phrase: s.by_phrase,
                    reason: super::rehearsal::open_reason(o.kind, rejected),
                });
                o.settlement = None;
            }
            super::rehearsal::RehearsalTrace {
                open_debts,
                ..Default::default()
            }
        });

        let mut perf = PerformancePlan {
            song_fingerprint: song.fingerprint(),
            language: lang,
            coupling: opts.coupling,
            region,
            regions,
            chords,
            contexts,
            deflects,
            edits,
            actions,
            accent,
            statements,
            interactions,
            ensemble,
            bank,
            response_mode: opts.responses,
            call_policy: opts.calls,
            total_beats,
            materials: ip.materials,
            opportunities: ip.opportunities,
            stage,
            admissions,
            obligations,
            budget: Vec::new(),
            cover_constraints,
            rehearsal,
            functions: admission.functions,
            lead_life: admission.lead_life,
            narrative,
        };
        // 8. The shared complexity budget: the lead's statements and the planned answers and
        //    figures are reserved, the rest is shared out to the accompanists.
        perf.budget = super::budget::allocate(&perf);
        for (eb, a) in perf.ensemble.iter_mut().zip(&perf.budget) {
            eb.budget = a.total;
        }
        // 9. A cover's pinned events are emitted from the plan alone; each must be justified in
        //    the planned harmony, or the lift is refused before anybody plays.
        if let Some(c) = &perf.cover_constraints {
            c.pinned_functions_hold(&perf)?;
        }
        Ok(perf)
    }

    /// The performance's time domain `[0, total_beats]`.
    pub fn domain(&self) -> PerformanceDomain {
        PerformanceDomain::new(self.total_beats)
    }

    /// Every planned window outside [`Self::domain`] (actions, stage windows, materials,
    /// statements, chords, regions), each named. Empty for a lawful plan.
    pub fn domain_violations(&self) -> Vec<String> {
        let d = self.domain();
        let mut out = Vec::new();
        let mut check = |what: String, start: f64, end: f64| {
            if !d.contains(start, end) {
                out.push(format!(
                    "{what} {start}..{end} outside 0..{}",
                    d.total_beats()
                ));
            }
        };
        for a in &self.actions.actions {
            check(
                format!("action {:?} {:?}", a.id, a.kind),
                a.start_beat,
                a.end_beat(),
            );
        }
        for w in &self.stage.windows {
            check(
                format!("stage window {:?}", w.agent),
                w.start_beat,
                w.end_beat,
            );
        }
        for m in &self.materials {
            check(
                format!("material {:?}", m.id),
                m.start_beat,
                m.start_beat + m.length(),
            );
        }
        for st in &self.statements {
            check(
                format!("statement {}", st.phrase),
                st.start_beat,
                st.end_beat(),
            );
        }
        for c in &self.chords {
            check(
                "chord".into(),
                c.start_beat,
                c.start_beat + f64::from(c.dur_beats),
            );
        }
        for r in &self.regions.spans {
            check("region".into(), r.start_beat, r.end_beat);
        }
        out
    }

    /// The performance's fingerprint: everything this performance decided (FNV-1a over its full
    /// debug form). Two performances of one song differ here; their songs do not.
    pub fn fingerprint(&self) -> u64 {
        super::song::fnv1a(&format!("{self:?}"))
    }

    /// Explicit name for the frozen Debug-based v1 receipt formula.
    pub fn legacy_fingerprint(&self) -> u64 {
        self.fingerprint()
    }

    /// `agent`'s complexity allowance in `bar` minus what the plan already reserved for it (its
    /// answers, its figures): what its free playing may still spend.
    pub fn free_allowance(&self, agent: Agent, bar: u32) -> f32 {
        let Some(a) = self.budget.get(bar as usize) else {
            return f32::INFINITY;
        };
        let i = super::ensemble::seat_ix(agent).unwrap_or(0);
        let burst = if a.burst.is_some() { 1.25 } else { 1.0 };
        (a.allowance[i] * burst - a.reserved[i]).max(0.0)
    }

    /// The material with id `id`.
    pub fn material(&self, id: MaterialId) -> &InteractionMaterial {
        &self.materials[id.index()]
    }

    /// The figures `agent` states (its call-or-not figure material), in time order.
    pub fn figures_for(&self, agent: Agent) -> impl Iterator<Item = &InteractionMaterial> {
        self.materials
            .iter()
            .filter(move |m| m.owner == agent && matches!(m.source, MaterialSource::Figure { .. }))
    }

    /// The figure material stated for action `id`, if any.
    pub fn figure_of(&self, id: ActionId) -> Option<&InteractionMaterial> {
        self.materials
            .iter()
            .find(|m| matches!(m.source, MaterialSource::Figure { action, .. } if action == id))
    }

    /// The actions of `kinds` that start within `tol` beats of `beat` (optionally only those
    /// initiated by `by`) — the planned accents a realizer's event at `beat` realizes.
    pub fn actions_starting(
        &self,
        kinds: &[ActionKind],
        beat: f64,
        tol: f64,
        by: Option<Agent>,
    ) -> impl Iterator<Item = ActionId> + '_ {
        let kinds = kinds.to_vec();
        self.actions
            .actions
            .iter()
            .filter(move |a| {
                kinds.contains(&a.kind)
                    && (a.start_beat - beat).abs() <= tol + 1e-9
                    && by.is_none_or(|g| a.initiator == g)
            })
            .map(|a| a.id)
    }

    /// The actions of `kinds` whose window covers `beat` (optionally only those initiated by `by`).
    pub fn actions_covering(
        &self,
        kinds: &[ActionKind],
        beat: f64,
        by: Option<Agent>,
    ) -> impl Iterator<Item = ActionId> + '_ {
        let kinds = kinds.to_vec();
        self.actions
            .actions
            .iter()
            .filter(move |a| {
                kinds.contains(&a.kind) && a.covers(beat) && by.is_none_or(|g| a.initiator == g)
            })
            .map(|a| a.id)
    }

    /// The force of the actions in `stamp` (their largest semantic strength), or `None` when the
    /// stamp names no action.
    pub fn force_of(&self, stamp: super::ids::ActionStamp) -> Option<f32> {
        stamp
            .iter()
            .filter_map(|id| self.actions.get(id))
            .map(|a| a.effect.strength)
            .reduce(f32::max)
    }

    /// The interaction in which action `id` is the call or the response.
    pub fn interaction_of(&self, id: ActionId) -> Option<InteractionId> {
        self.interactions
            .iter()
            .find(|i| i.call.action == id || i.response.is_some_and(|r| r.action == Some(id)))
            .map(|i| i.id)
    }

    /// Whether `agent` plays at `beat` (the stage is the single authority).
    pub fn on_stage(&self, agent: Agent, beat: f64) -> bool {
        self.stage.on_stage(agent, beat)
    }

    /// The level `agent` plays at `beat`.
    pub fn level(&self, agent: Agent, beat: f64) -> f32 {
        self.stage.level(agent, beat)
    }

    /// The ensemble plan for `bar`.
    pub fn bar(&self, bar: u32) -> Option<&EnsembleBar> {
        self.ensemble.get(bar as usize)
    }

    /// The ensemble plan for the bar holding `beat`.
    pub fn bar_at(&self, beat: f64) -> Option<&EnsembleBar> {
        self.bar((beat / BEATS_PER_BAR).floor().max(0.0) as u32)
    }

    /// The harmonic context sounding at `beat`.
    pub fn context_at(&self, beat: f64) -> Option<&HarmonicContext> {
        super::context::context_at(&self.contexts, beat)
    }

    /// The tonal region in force at `beat` (home outside every modulated span).
    pub fn region_at(&self, beat: f64) -> Scale {
        self.regions.region_at(beat)
    }

    /// The responses assigned to `agent`.
    pub fn responses_for(&self, agent: Agent) -> impl Iterator<Item = (&Call, &Response)> {
        self.interactions.iter().filter_map(move |i| {
            i.response
                .as_ref()
                .filter(|r| r.responder == agent && r.transform != Transform::Silence)
                .map(|r| (&i.call, r))
        })
    }

    /// The calls initiated by `agent` other than lead statements (figures a player states).
    pub fn figure_calls_for(&self, agent: Agent) -> impl Iterator<Item = &Call> {
        self.interactions
            .iter()
            .map(|i| &i.call)
            .filter(move |c| c.initiator == agent && c.statement.is_none())
    }

    /// A compact dump of the performance: statements, interactions, ensemble.
    pub fn dump(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let _ = writeln!(
            s,
            "performance: language={} responses={:?} statements={} interactions={} edits={}",
            self.language.id.label(),
            self.response_mode,
            self.statements.len(),
            self.interactions.len(),
            self.edits.len()
        );
        for i in &self.interactions {
            let c = &i.call;
            match &i.response {
                Some(r) => {
                    let _ = writeln!(
                        s,
                        "  call {} {:<5} {:>6.2}..{:<6.2} -> {:<5} {:+.2}b {:<8}{}{}",
                        c.action,
                        c.initiator.label(),
                        c.start_beat,
                        c.end_beat,
                        r.responder.label(),
                        r.latency,
                        r.transform.label(),
                        if r.overlap { " overlap" } else { "" },
                        if r.crosses_chord {
                            " crosses-chord"
                        } else {
                            ""
                        }
                    );
                }
                None => {
                    let _ = writeln!(
                        s,
                        "  call {} {:<5} {:>6.2}..{:<6.2} -> (no room)",
                        c.action,
                        c.initiator.label(),
                        c.start_beat,
                        c.end_beat
                    );
                }
            }
        }
        let _ = writeln!(
            s,
            "  ensemble (bar: gesture fg pad/keys/bass/drums kinetic):"
        );
        for e in &self.ensemble {
            let _ = writeln!(
                s,
                "    {:>2} {:<8} {:<6} {:?}/{:?}/{:?}/{:?} k={:.2}",
                e.bar,
                e.gesture.map(|g| g.label()).unwrap_or("-"),
                e.foreground.label(),
                e.pad,
                e.keys,
                e.bass,
                e.drums,
                e.kinetic
            );
        }
        s
    }
}

/// Declare the piece's deliberate still points (Round VIIb; `StasisSpan` existed and nothing
/// populated it). Two sources, both conservative:
/// - a semantic event that applies only `Prolong` (nothing new happened) holds the music still
///   until the next event, if no action starts in that span;
/// - a low-energy `Dissolve` phrase comes to rest after its last action.
///
/// Never inside a Culminate phrase and never across a salient (non-Prolong) event — the
/// diagnostics count any such span as `illegal_stasis`.
pub fn declare_stasis(
    timeline: &IntentTimeline,
    plan: &CompositionPlan,
    actions: &ActionPlan,
    total_beats: f64,
) -> Vec<super::action::StasisSpan> {
    use super::discourse::DiscourseRole;
    use super::intent::IntentMorphism;
    let starts_in = |a: f64, b: f64| {
        actions
            .actions
            .iter()
            .any(|x| x.start_beat >= a - 1e-6 && x.start_beat < b - 1e-6)
    };
    let culminates = |a: f64, b: f64| {
        plan.form.phrases.iter().any(|p| {
            p.start_beat() < b - 1e-6
                && p.end_beat() > a + 1e-6
                && plan.discourse.goal(p.ix as usize).role == DiscourseRole::Culminate
        })
    };
    let mut out = Vec::new();
    for (i, t) in timeline.transitions.iter().enumerate() {
        let quiet = t.applied.iter().all(|&m| m == IntentMorphism::Prolong);
        if !quiet || t.effect.strength > 0.45 {
            continue;
        }
        let end = timeline
            .transitions
            .get(i + 1)
            .map(|n| n.at_beat)
            .unwrap_or(total_beats)
            .min(t.at_beat + 16.0)
            .min(total_beats);
        // The stillness lasts until the next thing anybody does (the next event's lead-in may
        // start a little before that event).
        let end = actions
            .actions
            .iter()
            .map(|x| x.start_beat)
            .filter(|&b| b > t.at_beat + 0.25)
            .fold(end, f64::min);
        if end > t.at_beat + 1.0 && !starts_in(t.at_beat + 0.25, end) && !culminates(t.at_beat, end)
        {
            out.push(super::action::StasisSpan {
                start_beat: t.at_beat,
                end_beat: end,
                reason: "a Prolong event: nothing new happened, the music holds still",
            });
        }
    }
    // A relaxation (a Pullback after a resolution) is itself a declared stillness beyond its
    // onset: the band settles into it on purpose, as long as nothing else starts meanwhile.
    for a in actions
        .actions
        .iter()
        .filter(|a| a.kind == ActionKind::Pullback)
    {
        let (s0, e0) = (a.start_beat + 2.0, a.end_beat().min(total_beats));
        let next = actions
            .actions
            .iter()
            .filter(|x| x.id != a.id && x.start_beat > s0 - 1e-6 && x.start_beat < e0)
            .map(|x| x.start_beat)
            .fold(e0, f64::min);
        if next - s0 >= 1.0 && !culminates(s0, next) {
            out.push(super::action::StasisSpan {
                start_beat: s0,
                end_beat: next,
                reason: "the relaxation after a resolution holds",
            });
        }
    }
    for p in &plan.form.phrases {
        let g = plan.discourse.goal(p.ix as usize);
        if g.role != DiscourseRole::Dissolve || g.energy_target > 0.45 {
            continue;
        }
        let (ps, pe) = (p.start_beat(), p.end_beat().min(total_beats));
        let last = actions
            .actions
            .iter()
            .filter(|x| x.start_beat >= ps - 1e-6 && x.start_beat < pe - 1e-6)
            .map(|x| x.start_beat + x.dur_beats.min(2.0))
            .fold(ps, f64::max);
        if pe - last >= 1.0 && !culminates(last, pe) {
            out.push(super::action::StasisSpan {
                start_beat: last,
                end_beat: pe,
                reason: "the dissolve lets the piece come to rest",
            });
        }
    }
    out.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
    out
}

/// Reconcile every action with the stage before realization (Round VIIb). The arrangement is a
/// coarse envelope; an action is the authority for its own short window:
/// - an ensemble accent with one player on stage becomes that player's pickup, with none it is
///   rejected;
/// - a short entrance verb (pickup, fill, re-entry, fragment, accent, hold) by an off-stage player
///   ADMITS that player for the window;
/// - a relaxation by a player the arrangement takes out becomes the subtraction it plans
///   (Pullback → Thin); a harmonic verb is carried by somebody present;
/// - anything else nobody can perform is removed and deferred with its reason.
fn admit_actions(actions: &mut ActionPlan, stage: &mut Stage) -> Vec<AdmissionRecord> {
    let hitters = [Agent::Bass, Agent::Keys, Agent::Drums, Agent::Lead];
    let mut records = Vec::new();
    let mut reject: Vec<(ActionId, &'static str)> = Vec::new();
    for a in actions.actions.iter_mut() {
        let (s, e) = (a.start_beat, a.end_beat().max(a.start_beat + 0.25));
        let mut record = |outcome: Admission| {
            records.push(AdmissionRecord {
                action: Some(a.id),
                kind: a.kind,
                start_beat: s,
                outcome,
            })
        };
        if a.initiator == Agent::Ensemble {
            match a.kind {
                ActionKind::Push | ActionKind::Hit => {
                    let present = stage.present(&hitters, s, s + 0.25);
                    if present.len() == 1 {
                        // One player on stage: the accent is that player's own (Round VII turned
                        // it into a "pickup" that began ON its target).
                        let solo = present[0];
                        record(Admission::Recast {
                            from_kind: a.kind,
                            from: Agent::Ensemble,
                            to_kind: a.kind,
                            to: solo,
                        });
                        a.initiator = solo;
                    } else if present.is_empty() {
                        reject.push((a.id, "nobody is on stage for an ensemble accent"));
                    }
                }
                ActionKind::Deflect => {
                    // The miss is made concrete by the bass arriving on the ACTUAL root. With the
                    // bass out, it enters for the miss's first beat (if anybody pitched is there
                    // to deflect at all); with nobody pitched on stage the miss cannot sound.
                    if !stage.on_stage(Agent::Bass, s) {
                        let pitched =
                            stage.present(&[Agent::Pad, Agent::Keys, Agent::Lead], s, s + 0.25);
                        if pitched.is_empty() {
                            reject.push((a.id, "nobody pitched is on stage to sound the miss"));
                        } else {
                            stage.admit(Agent::Bass, s, s + 1.0, a.id);
                            record(Admission::Admitted { agent: Agent::Bass });
                        }
                    }
                }
                ActionKind::Unison => {
                    let pair = [Agent::Keys, Agent::Bass];
                    let present = stage.present(&pair, s, e);
                    if present.is_empty() {
                        reject.push((a.id, "neither keys nor bass is on stage for the unison"));
                    } else {
                        for g in pair.into_iter().filter(|g| !present.contains(g)) {
                            stage.admit(g, s, e, a.id);
                            record(Admission::Admitted { agent: g });
                        }
                    }
                }
                _ => {}
            }
            continue;
        }
        let agent = a.initiator;
        if stage.on_stage_span(agent, s, e) {
            continue;
        }
        match a.kind {
            ActionKind::Pickup
            | ActionKind::Fill
            | ActionKind::ReEntry
            | ActionKind::Fragment
            | ActionKind::Hit
            | ActionKind::Push
            | ActionKind::Hold
                if e - s <= BEATS_PER_BAR + 1e-6 =>
            {
                stage.admit(agent, s, e, a.id);
                record(Admission::Admitted { agent });
            }
            ActionKind::Resolve
            | ActionKind::Tonicize
            | ActionKind::Modulate
            | ActionKind::Reharmonize
            | ActionKind::Recolor => {
                // The harmony moves for everybody; the verb needs a pitched player who is there.
                match stage
                    .present(
                        &[Agent::Bass, Agent::Keys, Agent::Pad, Agent::Lead],
                        s,
                        s + 0.5,
                    )
                    .first()
                {
                    Some(&to) => {
                        record(Admission::Recast {
                            from_kind: a.kind,
                            from: agent,
                            to_kind: a.kind,
                            to,
                        });
                        a.initiator = to;
                    }
                    None => reject.push((a.id, "no pitched player is on stage to carry it")),
                }
            }
            ActionKind::Pullback | ActionKind::Thin
                if stage.on_stage(agent, s - 0.25) && !stage.on_stage(agent, s + 0.25) =>
            {
                // The arrangement takes this player out right here: that exit IS the relaxation.
                record(Admission::Recast {
                    from_kind: a.kind,
                    from: agent,
                    to_kind: ActionKind::Thin,
                    to: agent,
                });
                a.kind = ActionKind::Thin;
                a.dur_beats = a.dur_beats.min(BEATS_PER_BAR);
            }
            _ if stage.on_stage(agent, s) || stage.on_stage(agent, e - 0.25) => {
                // Partly on stage: the verb happens where the player is.
            }
            _ => reject.push((a.id, "its player is off stage for the whole window")),
        }
    }
    if !reject.is_empty() {
        for &(id, reason) in &reject {
            if let Some(a) = actions.get(id) {
                records.push(AdmissionRecord {
                    action: None,
                    kind: a.kind,
                    start_beat: a.start_beat,
                    outcome: Admission::Rejected { reason },
                });
                if let ActionCause::Morphism {
                    transition,
                    morphism,
                } = a.cause
                {
                    actions.deferred.push(Deferral {
                        transition,
                        morphism,
                        reason,
                    });
                }
            }
        }
        let gone: Vec<ActionId> = reject.iter().map(|r| r.0).collect();
        let remap = actions.remove(&gone);
        for r in &mut records {
            r.action = r.action.and_then(&remap);
        }
        stage.windows.iter_mut().for_each(|w| {
            if let Some(n) = remap(w.action) {
                w.action = n;
            }
        });
    }
    records
}

/// Build the accent grid: meter, the piece's per-gesture rhythm cells (varied per bar by
/// rotation and omission), and the actions' pushes, hits, holes, pickups and displacements.
fn build_accent_grid(
    plan: &CompositionPlan,
    actions: &ActionPlan,
    lang: &MusicalLanguage,
    seed: u64,
) -> AccentGrid {
    let total_bars = plan.form.total_bars as usize;
    let mut rng = Rng::new(seed ^ 0x6A1D_0C3E);
    let sync = lang.syncopation;
    let sixteenths = lang.surface_subdivision >= 4;

    // Per-gesture rhythm cells: a small set of off-beat steps chosen once per piece — the
    // gesture's rhythmic identity (Lift pushes most, Open least).
    let offbeats: Vec<usize> = (0..STEPS)
        .filter(|s| s % 4 != 0 && (sixteenths || s % 2 == 0))
        .collect();
    let sizes = [4usize, 3, 2, 3];
    let cells: [Vec<usize>; 4] = std::array::from_fn(|g| {
        let mut pool = offbeats.clone();
        let mut cell = Vec::new();
        for _ in 0..sizes[g].min(pool.len()) {
            // Prefer eighth off-beats (stronger syncopation), sixteenths as colour.
            let weights: Vec<f32> = pool
                .iter()
                .map(|s| if s % 2 == 0 { 1.0 } else { 0.55 })
                .collect();
            let total: f32 = weights.iter().sum();
            let mut r = rng.range_f32(0.0, total);
            let mut k = 0;
            for (i, w) in weights.iter().enumerate() {
                if r < *w {
                    k = i;
                    break;
                }
                r -= w;
            }
            cell.push(pool.remove(k));
        }
        cell.sort_unstable();
        cell
    });

    let mut bars = vec![[StepWeight::default(); STEPS]; total_bars];
    for (bar, w) in bars.iter_mut().enumerate() {
        for (s, sw) in w.iter_mut().enumerate() {
            sw.structural = match s {
                0 => 1.0,
                8 => 0.8,
                4 | 12 => 0.35,
                _ => 0.0,
            };
            sw.backbeat = if s == 4 || s == 12 { 1.0 } else { 0.0 };
            sw.syncopation = if s % 4 == 2 {
                0.4 * sync
            } else if s % 2 == 1 && sixteenths {
                0.2 * sync
            } else {
                0.0
            };
            if s >= 14 {
                sw.pickup = 0.3;
            }
        }
        // The gesture's cell, varied per bar: rotated by an eighth on odd bars of the slot,
        // thinned on the slot's last bar (a breath), with a pickup into the next slot.
        if let Some(bb) = &plan.backbone {
            if let Some(slot) = bb.slot_at_bar(bar as u32) {
                let in_slot = bar as u32 - slot.start_bar;
                let last = bar as u32 + 1 == slot.end_bar();
                let cell = &cells[gesture_ix(slot.gesture)];
                let rot = if in_slot % 2 == 1 { 2 } else { 0 };
                for (k, &c) in cell.iter().enumerate() {
                    if last && k + 1 == cell.len() && cell.len() > 1 {
                        continue;
                    }
                    let st = (c + rot) % STEPS;
                    if st % 4 == 0 {
                        continue;
                    }
                    w[st].syncopation = w[st].syncopation.max(0.35 + 0.6 * sync);
                }
                if last {
                    w[14].pickup = 0.8;
                    w[15].pickup = 0.7;
                }
            }
        }
    }

    // Actions mutate the grid.
    for a in &actions.actions {
        let (b0, s0) = AccentGrid::step_of(a.start_beat);
        let steps = ((a.dur_beats / STEP_BEATS).round() as usize).max(1);
        let mut each = |f: &mut dyn FnMut(&mut StepWeight, usize)| {
            for k in 0..steps {
                let abs = b0 as usize * STEPS + s0 + k;
                let (bi, si) = (abs / STEPS, abs % STEPS);
                if let Some(bw) = bars.get_mut(bi) {
                    f(&mut bw[si], si);
                }
            }
        };
        match a.kind {
            ActionKind::Push => each(&mut |w, _| {
                w.push = 1.0;
                w.hit = w.hit.max(0.7);
            }),
            ActionKind::Hit => {
                let (b, s) = AccentGrid::step_of(a.start_beat);
                if let Some(bw) = bars.get_mut(b as usize) {
                    bw[s].hit = 1.0;
                }
            }
            ActionKind::Break => each(&mut |w, _| w.hole = 1.0),
            ActionKind::Displace => each(&mut |w, s| {
                if s % 4 != 0 {
                    w.syncopation = (w.syncopation * 1.5 + 0.1).min(1.0);
                } else if s == 8 {
                    w.structural *= 0.5;
                }
            }),
            ActionKind::Pullback => each(&mut |w, s| {
                w.syncopation *= 0.5;
                if s % 2 == 1 {
                    w.hole = w.hole.max(0.5);
                }
            }),
            ActionKind::Accelerate => each(&mut |w, s| {
                if s % 2 == 1 {
                    w.syncopation = (w.syncopation + 0.3).min(1.0);
                }
            }),
            ActionKind::Fill | ActionKind::Pickup => each(&mut |w, _| {
                w.pickup = w.pickup.max(0.9);
            }),
            _ => {}
        }
    }
    // A partial final bar: every step at or after the piece's exact end is a planned hole.
    let end = plan.form.total_beats;
    for (bar, w) in bars.iter_mut().enumerate() {
        for (s, sw) in w.iter_mut().enumerate() {
            if AccentGrid::beat_of(bar as u32, s) >= end - 1e-9 {
                *sw = StepWeight {
                    hole: 1.0,
                    ..StepWeight::default()
                };
            }
        }
    }
    AccentGrid { bars, cells }
}

// Preserve the exact historical Debug stream when no cover constraints exist.
impl std::fmt::Debug for PerformancePlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("PerformancePlan");
        d.field("song_fingerprint", &self.song_fingerprint);
        d.field("language", &self.language);
        d.field("coupling", &self.coupling);
        d.field("region", &self.region);
        d.field("regions", &self.regions);
        d.field("chords", &self.chords);
        d.field("contexts", &self.contexts);
        d.field("deflects", &self.deflects);
        d.field("edits", &self.edits);
        d.field("actions", &self.actions);
        d.field("accent", &self.accent);
        d.field("statements", &self.statements);
        d.field("interactions", &self.interactions);
        d.field("ensemble", &self.ensemble);
        d.field("bank", &self.bank);
        d.field("response_mode", &self.response_mode);
        d.field("call_policy", &self.call_policy);
        d.field("total_beats", &self.total_beats);
        d.field("materials", &self.materials);
        d.field("opportunities", &self.opportunities);
        d.field("stage", &self.stage);
        d.field("admissions", &self.admissions);
        d.field("obligations", &self.obligations);
        d.field("budget", &self.budget);
        if let Some(c) = &self.cover_constraints {
            d.field("cover_constraints", c);
        }
        if let Some(r) = &self.rehearsal {
            d.field("rehearsal", r);
        }
        if self.functions != super::policy::FunctionPolicy::Archived {
            d.field("functions", &self.functions);
        }
        if self.lead_life != super::policy::LeadLifePolicy::default() {
            d.field("lead_life", &self.lead_life);
        }
        if let Some(n) = &self.narrative {
            d.field("narrative", n);
        }
        d.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::super::semantic::deflected_lift_trace;
    use super::*;

    /// A verb's identity survives the renumbering a veto causes: striking an earlier verb must
    /// not rename a later one (a response names its call; the call's number shifts).
    #[test]
    fn a_verbs_identity_survives_the_renumbering_a_veto_causes() {
        let p = flagship(PerformanceOptions::default());
        let answer = p
            .actions
            .actions
            .iter()
            .find(|a| matches!(a.cause, ActionCause::Interaction { .. }))
            .expect("the flagship answers a call");
        let before = ActionKey::of(answer, &p.actions);
        let start = answer.start_beat;
        let mut struck = p.actions.clone();
        let first = struck.actions[0].id;
        assert!(first != answer.id);
        let _ = struck.remove(&[first]);
        let same = struck
            .actions
            .iter()
            .find(|a| a.start_beat == start && matches!(a.cause, ActionCause::Interaction { .. }))
            .expect("still planned");
        assert_eq!(ActionKey::of(same, &struck), before);
    }

    /// A verb the rehearsal found nobody performing is struck wherever the planner makes it —
    /// including the interaction planner's calls and answers, which exist only after the base
    /// plan is admitted.
    #[test]
    fn a_vetoed_answer_is_struck_from_the_chart() {
        let song = super::super::song::SongMap::build(
            &deflected_lift_trace(120.0),
            2112,
            Some(super::super::contract::CompositionGrammar::DeflectedLift),
        );
        let world = MusicWorld::black_ice();
        let opts = PerformanceOptions::default();
        let base = PerformancePlan::from_song(&song, &world, opts);
        let answer = base
            .actions
            .actions
            .iter()
            .find(|a| {
                a.kind == ActionKind::Answer && matches!(a.cause, ActionCause::Interaction { .. })
            })
            .expect("an answer");
        let key = ActionKey::of(answer, &base.actions);
        let inputs = AdmissionInputs {
            vetoed: vec![key.clone()],
            ..AdmissionInputs::default()
        };
        let struck =
            PerformancePlan::from_song_admitted(&song, &world, opts, None, &inputs).unwrap();
        assert!(
            !struck
                .actions
                .actions
                .iter()
                .any(|a| ActionKey::of(a, &struck.actions) == key),
            "a vetoed answer is still on the chart"
        );
    }

    fn flagship(opts: PerformanceOptions) -> PerformancePlan {
        let song = super::super::song::SongMap::build(
            &deflected_lift_trace(120.0),
            2112,
            Some(super::super::contract::CompositionGrammar::DeflectedLift),
        );
        PerformancePlan::from_song(&song, &MusicWorld::black_ice(), opts)
    }

    #[test]
    fn free_responses_vary_responder_and_latency() {
        let p = flagship(PerformanceOptions::default());
        let rs: Vec<&Response> = p
            .interactions
            .iter()
            .filter_map(|i| i.response.as_ref())
            .filter(|r| r.transform != Transform::Silence)
            .collect();
        assert!(rs.len() >= 3, "too few responses: {}", rs.len());
        let responders: std::collections::BTreeSet<Agent> =
            rs.iter().map(|r| r.responder).collect();
        assert!(responders.len() >= 2, "one responder only: {responders:?}");
        let lat: std::collections::BTreeSet<i64> = rs
            .iter()
            .map(|r| (r.latency * 2.0).round() as i64)
            .collect();
        assert!(lat.len() >= 3, "latencies too uniform: {lat:?}");
    }

    #[test]
    fn clockwork_responses_are_rigid_by_construction() {
        let p = flagship(PerformanceOptions {
            responses: ResponseMode::Clockwork,
            ..PerformanceOptions::default()
        });
        for r in p.interactions.iter().filter_map(|i| i.response.as_ref()) {
            assert_eq!(r.responder, Agent::Keys);
            assert_eq!(AccentGrid::step_of(r.start_beat).1, 10);
        }
    }

    #[test]
    fn the_accent_grid_varies_across_bars_of_a_slot() {
        let p = flagship(PerformanceOptions::default());
        let sig = |bar: u32| -> Vec<usize> {
            (0..STEPS)
                .filter(|&s| p.accent.at(bar, s).syncopation > 0.5)
                .collect()
        };
        // Bars 4..8 are one Lift slot: the rotation makes consecutive bars differ.
        assert_ne!(sig(4), sig(5));
    }

    #[test]
    fn disabling_actions_keeps_the_harmony_and_removes_the_verbs() {
        let on = flagship(PerformanceOptions::default());
        let off = flagship(PerformanceOptions {
            actions: false,
            ..PerformanceOptions::default()
        });
        assert!(off.actions.actions.is_empty());
        assert!(off.interactions.is_empty());
        assert_eq!(on.statements.len(), off.statements.len());
        assert!(on.actions.actions.len() > off.actions.actions.len());
        assert_eq!(on.chords.len(), off.chords.len());
    }
}
