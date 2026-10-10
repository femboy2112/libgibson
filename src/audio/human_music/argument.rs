//! Optional source-bound musical arguments.
//!
//! This layer supplies concrete timed material and finite obligations to the existing composer.
//! It does not interpret prose or certify what a listener hears. The old coarse meaning map is
//! retained unchanged; this wrapper records the stronger source commitments separately.
//!
//! The implemented identity licenses are whole-octave translation and positive rational time
//! scaling. They compose by adding octaves and multiplying ratios. Appending a declared terminal
//! consequence is a partial operation: it is valid only for its particular earlier question.
//! No transformation is licensed to crop the question, change its internal intervals, or replace
//! its written rests, gates or accents. A complete source is lowered to [`SongMap`] theme sites
//! for ordinary planning, while [`ArgumentPlacement`] retains the independent onset/gate source
//! required by the performance path.

use super::action::Agent;
use super::backbone::{
    BackboneTimeline, ChartCell, ChartRoot, ClockBinding, CycleVariation, GestureSlot,
    HarmonicGesture, TimeScales,
};
use super::discourse::{resolve_obligations, Closure, DiscourseRole};
use super::material::MaterialEvent;
use super::meaning::MeaningPlan;
use super::motif::{Handoff, Motif, MotifBank};
use super::performance::PerformancePlan;
use super::plan::ArrangementRole;
use super::rng::Rng;
use super::song::{HarmonicMap, SongMap, ThemeSite};
use super::theory::{Chord, PitchBasis, Quality};
use super::vocabulary::HarmonicVocabulary;
use super::world::MusicWorld;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// A concrete phrase identity, rather than a mood or instrument label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReferentId(pub u8);

/// An occurrence in a finite argument; references point to occurrences, not generic verbs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ArgumentStepId(pub u8);

/// The two currently implemented calibration families, and the reserved third research case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgumentFamily {
    CallAndEarnedAnswer,
    PromiseDeniedReturn,
    /// Not yet implemented: two identities need a substantive compatibility operation.
    ConflictReconciliation,
}

/// An exact timed musical source. Pitches are offsets from one song tonic, in declared units.
#[derive(Debug, Clone, PartialEq)]
pub struct MusicalReferent {
    pub id: ReferentId,
    pub pitch_basis: PitchBasis,
    pub events: Vec<MaterialEvent>,
}

/// The terminal consequence specifically expected by a question.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Completion {
    /// Source-relative pitch coordinate before octave transport.
    pub step: i32,
    /// Sounding duration before rational time scaling.
    pub dur: f64,
}

/// A small, exact license over a source. No lossy contour repair or four-note cropping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArgumentTransform {
    pub transpose_octaves: i8,
    pub time_numerator: u8,
    pub time_denominator: u8,
    /// Appended at `ceil(max(source.onset + source.dur))`, before time scaling.
    pub completion: Option<Completion>,
}

impl Default for ArgumentTransform {
    fn default() -> Self {
        Self {
            transpose_octaves: 0,
            time_numerator: 1,
            time_denominator: 1,
            completion: None,
        }
    }
}

impl ArgumentTransform {
    /// Compose the identity-preserving operations in performance order. Terminal consequences
    /// are source-bound partial arrows and cannot silently become general identity operations.
    pub fn then(self, next: Self) -> Result<Self, ArgumentError> {
        if self.completion.is_some() || next.completion.is_some() {
            return Err(ArgumentError::NonComposableCompletion);
        }
        self.check()?;
        next.check()?;
        let mut numerator = u16::from(self.time_numerator) * u16::from(next.time_numerator);
        let mut denominator = u16::from(self.time_denominator) * u16::from(next.time_denominator);
        let mut a = numerator;
        let mut b = denominator;
        while b != 0 {
            (a, b) = (b, a % b);
        }
        numerator /= a;
        denominator /= a;
        let octaves = i16::from(self.transpose_octaves) + i16::from(next.transpose_octaves);
        Ok(Self {
            transpose_octaves: i8::try_from(octaves)
                .map_err(|_| ArgumentError::InvalidTransform)?,
            time_numerator: u8::try_from(numerator).map_err(|_| ArgumentError::InvalidTransform)?,
            time_denominator: u8::try_from(denominator)
                .map_err(|_| ArgumentError::InvalidTransform)?,
            completion: None,
        })
    }

    fn check(self) -> Result<(), ArgumentError> {
        if self.time_numerator == 0 || self.time_denominator == 0 {
            return Err(ArgumentError::InvalidTransform);
        }
        if self
            .completion
            .is_some_and(|c| !c.dur.is_finite() || c.dur <= 0.0 || !(-48..=48).contains(&c.step))
        {
            return Err(ArgumentError::InvalidTransform);
        }
        Ok(())
    }

    /// Apply the declared license to the complete timed source.
    pub fn apply(self, source: &MusicalReferent) -> Result<Vec<MaterialEvent>, ArgumentError> {
        self.check()?;
        validate_material(source)?;
        let mut events = source.events.clone();
        if let Some(c) = self.completion {
            events.push(MaterialEvent {
                onset: material_length(&events).ceil(),
                dur: c.dur,
                accent: 1.0,
                step: Some(c.step),
            });
        }
        let time = f64::from(self.time_numerator) / f64::from(self.time_denominator);
        let pitch = 12 * i32::from(self.transpose_octaves);
        for event in &mut events {
            event.onset *= time;
            event.dur *= time;
            event.step = event.step.map(|step| step + pitch);
        }
        Ok(events)
    }
}

/// Musical relationships between actual earlier occurrences.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArgumentRelation {
    Establish,
    Question {
        source: ArgumentStepId,
        expected: Completion,
    },
    Develop {
        source: ArgumentStepId,
    },
    Denial {
        question: ArgumentStepId,
    },
    Answer {
        question: ArgumentStepId,
    },
    Return {
        source: ArgumentStepId,
    },
    /// Reserved, rejected by the current validator instead of falsely certifying simultaneity.
    Reconcile {
        left: ArgumentStepId,
        right: ArgumentStepId,
    },
}

impl ArgumentRelation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Establish => "establish",
            Self::Question { .. } => "question",
            Self::Develop { .. } => "develop",
            Self::Denial { .. } => "denial",
            Self::Answer { .. } => "answer",
            Self::Return { .. } => "return",
            Self::Reconcile { .. } => "reconcile",
        }
    }

    fn predecessor(self) -> Option<ArgumentStepId> {
        match self {
            Self::Establish => None,
            Self::Question { source, .. } | Self::Develop { source } | Self::Return { source } => {
                Some(source)
            }
            Self::Denial { question } | Self::Answer { question } => Some(question),
            Self::Reconcile { .. } => None,
        }
    }

    fn role(self) -> DiscourseRole {
        match self {
            Self::Establish => DiscourseRole::Establish,
            Self::Question { .. } => DiscourseRole::Question,
            Self::Develop { .. } => DiscourseRole::Depart,
            Self::Denial { .. } => DiscourseRole::Withhold,
            Self::Answer { .. } => DiscourseRole::Answer,
            Self::Return { .. } | Self::Reconcile { .. } => DiscourseRole::Return,
        }
    }

    fn gesture(self) -> HarmonicGesture {
        match self {
            Self::Question { .. } => HarmonicGesture::Lift,
            Self::Denial { .. } => HarmonicGesture::Deflect,
            Self::Develop { .. } => HarmonicGesture::Open,
            _ => HarmonicGesture::Reset,
        }
    }
}

/// A finite causal commitment. Every named carrier must state the whole source.
#[derive(Debug, Clone, PartialEq)]
pub struct ArgumentStep {
    pub id: ArgumentStepId,
    pub referent: ReferentId,
    pub transform: ArgumentTransform,
    pub relation: ArgumentRelation,
    pub depends_on: Vec<ArgumentStepId>,
    pub carriers: Vec<Agent>,
}

/// An open question is legitimate only when the source explicitly chooses this ending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgumentEnding {
    Resolved,
    IntentionallyUnresolved,
}

/// Affect is supplied by the ordinary trace; this object supplies a concrete musical argument.
#[derive(Debug, Clone, PartialEq)]
pub struct MusicalArgument {
    pub family: ArgumentFamily,
    pub referents: Vec<MusicalReferent>,
    pub steps: Vec<ArgumentStep>,
    pub ending: ArgumentEnding,
}

/// A required source statement with an explicit reservation, including its written silence.
#[derive(Debug, Clone, PartialEq)]
pub struct ArgumentPlacement {
    pub step: ArgumentStepId,
    pub referent: ReferentId,
    pub relation: ArgumentRelation,
    pub phrase: u32,
    pub start_beat: f64,
    pub span_beats: f64,
    pub events: Vec<MaterialEvent>,
    pub pitch_basis: PitchBasis,
    pub carriers: Vec<Agent>,
    /// Semitones from the performance world's tonic, fixed for the entire argument.
    pub anchor_semitones: i32,
}

/// A wrapper instead of additional fields in the established public `SongMap` structure.
#[derive(Debug, Clone)]
pub struct CompiledArgument {
    pub song: SongMap,
    pub placements: Vec<ArgumentPlacement>,
    pub argument: MusicalArgument,
}

/// A precise failed source commitment, not a numerical meaning-quality score.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgumentError {
    UnsupportedFamily,
    InvalidMaterial(ReferentId),
    InvalidTransform,
    NonComposableCompletion,
    DuplicateReferent(ReferentId),
    DuplicateStep(ArgumentStepId),
    MissingReferent(ReferentId),
    MissingDependency(ArgumentStepId),
    WrongSource(ArgumentStepId),
    WrongAnswer(ArgumentStepId),
    UnpreparedDenial(ArgumentStepId),
    UnresolvedQuestion(ArgumentStepId),
    InvalidCarrier(ArgumentStepId),
    IncompatibleHarmonicVocabulary,
    IncompatibleHarmonicEdit,
    IncompatibleTonalRegion,
    InsufficientPhrases,
    SourceDoesNotFit(ArgumentStepId),
}

impl fmt::Display for ArgumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "musical argument failed: {self:?}")
    }
}

impl std::error::Error for ArgumentError {}

fn material_length(events: &[MaterialEvent]) -> f64 {
    events.iter().map(|e| e.onset + e.dur).fold(0.0, f64::max)
}

fn validate_material(source: &MusicalReferent) -> Result<(), ArgumentError> {
    if source.pitch_basis != PitchBasis::Semitones
        || source.events.len() < 3
        || source.events.len() > 64
        || source.events.iter().any(|e| {
            !e.onset.is_finite()
                || !e.dur.is_finite()
                || !e.accent.is_finite()
                || e.onset < 0.0
                || e.dur <= 0.0
                || !(0.0..=1.0).contains(&e.accent)
                || e.step.is_none_or(|step| !(-48..=48).contains(&step))
        })
        || source.events.windows(2).any(|w| w[0].onset >= w[1].onset)
    {
        return Err(ArgumentError::InvalidMaterial(source.id));
    }
    Ok(())
}

// Compare the entire head, independently of IDs. This is exact source identity modulo the two
// licenses, not the old motif-similarity average over the shorter sequence.
fn same_head(a: &[MaterialEvent], b: &[MaterialEvent], count: usize) -> bool {
    if count == 0 || a.len() < count || b.len() < count {
        return false;
    }
    let (Some(pa), Some(pb)) = (a[0].step, b[0].step) else {
        return false;
    };
    if (pb - pa).rem_euclid(12) != 0 {
        return false;
    }
    let time = b[0].dur / a[0].dur;
    a.iter().zip(b).take(count).all(|(x, y)| {
        x.step.map(|p| p - pa) == y.step.map(|p| p - pb)
            && (x.onset * time - y.onset).abs() < 1e-9
            && (x.dur * time - y.dur).abs() < 1e-9
            && (x.accent - y.accent).abs() < 1e-6
    })
}

impl MusicalArgument {
    /// Validate the finite dependency graph and source-specific completion contracts.
    pub fn validate(&self) -> Result<(), ArgumentError> {
        if self.family == ArgumentFamily::ConflictReconciliation {
            return Err(ArgumentError::UnsupportedFamily);
        }
        let mut referents = BTreeMap::new();
        for source in &self.referents {
            validate_material(source)?;
            if referents.insert(source.id, source).is_some() {
                return Err(ArgumentError::DuplicateReferent(source.id));
            }
        }
        let mut prior: BTreeMap<ArgumentStepId, (&ArgumentStep, Vec<MaterialEvent>)> =
            BTreeMap::new();
        let mut open = BTreeSet::new();
        for step in &self.steps {
            if prior.contains_key(&step.id) {
                return Err(ArgumentError::DuplicateStep(step.id));
            }
            if step.carriers.is_empty()
                || step
                    .carriers
                    .iter()
                    .any(|agent| !matches!(agent, Agent::Lead | Agent::Keys | Agent::Bass))
                || step.carriers.iter().copied().collect::<BTreeSet<_>>().len()
                    != step.carriers.len()
            {
                return Err(ArgumentError::InvalidCarrier(step.id));
            }
            for dependency in &step.depends_on {
                if !prior.contains_key(dependency) {
                    return Err(ArgumentError::MissingDependency(step.id));
                }
            }
            let source = referents
                .get(&step.referent)
                .ok_or(ArgumentError::MissingReferent(step.referent))?;
            let events = step.transform.apply(source)?;
            if let Some(predecessor) = step.relation.predecessor() {
                if !step.depends_on.contains(&predecessor) {
                    return Err(ArgumentError::MissingDependency(step.id));
                }
                let (earlier, earlier_events) = prior
                    .get(&predecessor)
                    .ok_or(ArgumentError::MissingDependency(step.id))?;
                let earlier_source = referents[&earlier.referent];
                if !same_head(&earlier_source.events, &source.events, source.events.len())
                    || earlier_source.events.len() != source.events.len()
                {
                    return Err(ArgumentError::WrongSource(step.id));
                }
                match step.relation {
                    ArgumentRelation::Question { expected, .. } => {
                        if step.transform.completion.is_some()
                            || expected.dur <= 0.0
                            || !expected.dur.is_finite()
                            || (source.events.last().and_then(|e| e.step).unwrap() - expected.step)
                                .abs()
                                > 2
                            || source.events.last().and_then(|e| e.step) == Some(expected.step)
                        {
                            return Err(ArgumentError::WrongAnswer(step.id));
                        }
                        open.insert(step.id);
                    }
                    ArgumentRelation::Answer { question } => {
                        let ArgumentRelation::Question { expected, .. } = earlier.relation else {
                            return Err(ArgumentError::WrongAnswer(step.id));
                        };
                        if !open.contains(&question)
                            || step.transform.completion != Some(expected)
                            || (source.events.last().and_then(|e| e.step).unwrap() - expected.step)
                                .abs()
                                > 2
                            || !same_head(earlier_events, &events, source.events.len())
                        {
                            return Err(ArgumentError::WrongAnswer(step.id));
                        }
                        open.remove(&question);
                    }
                    ArgumentRelation::Denial { question } => {
                        let ArgumentRelation::Question {
                            source: establishment,
                            expected,
                        } = earlier.relation
                        else {
                            return Err(ArgumentError::UnpreparedDenial(step.id));
                        };
                        // An expectation is taught by an earlier actual completion, then opened.
                        let taught = prior[&establishment].0.transform.completion;
                        if !open.contains(&question)
                            || taught != Some(expected)
                            || step
                                .transform
                                .completion
                                .is_none_or(|c| c.step == expected.step)
                            || !same_head(earlier_events, &events, source.events.len())
                        {
                            return Err(ArgumentError::UnpreparedDenial(step.id));
                        }
                    }
                    ArgumentRelation::Return { .. } | ArgumentRelation::Develop { .. }
                        if !same_head(earlier_events, &events, source.events.len()) =>
                    {
                        return Err(ArgumentError::WrongSource(step.id));
                    }
                    _ => {}
                }
            } else if step.relation != ArgumentRelation::Establish {
                return Err(ArgumentError::UnsupportedFamily);
            }
            prior.insert(step.id, (step, events));
        }
        if self.steps.is_empty() {
            return Err(ArgumentError::InsufficientPhrases);
        }
        if self.ending == ArgumentEnding::Resolved {
            if let Some(question) = open.iter().next() {
                return Err(ArgumentError::UnresolvedQuestion(*question));
            }
        }
        Ok(())
    }

    /// Compile on the existing production form, allocating distinct phrases jointly with source
    /// duration and the harmonic destination. This is opt-in; the input `SongMap` is untouched.
    pub fn compile(&self, base: &SongMap) -> Result<CompiledArgument, ArgumentError> {
        self.validate()?;
        let mut song = base.clone();
        let count = song.plan.form.phrases.len();
        if count < self.steps.len() {
            return Err(ArgumentError::InsufficientPhrases);
        }
        let mut placements = Vec::with_capacity(self.steps.len());
        for (i, step) in self.steps.iter().enumerate() {
            let phrase_ix = if self.steps.len() == 1 {
                0
            } else {
                i * (count - 1) / (self.steps.len() - 1)
            };
            let phrase = &song.plan.form.phrases[phrase_ix];
            let source = self
                .referents
                .iter()
                .find(|s| s.id == step.referent)
                .unwrap();
            let events = step.transform.apply(source)?;
            let span = phrase.end_beat() - phrase.start_beat();
            if material_length(&events) > span + 1e-9 {
                return Err(ArgumentError::SourceDoesNotFit(step.id));
            }
            let placement = ArgumentPlacement {
                step: step.id,
                referent: step.referent,
                relation: step.relation,
                phrase: phrase.ix,
                start_beat: phrase.start_beat(),
                span_beats: span,
                events,
                pitch_basis: source.pitch_basis,
                carriers: step.carriers.clone(),
                anchor_semitones: 0,
            };
            let arrangement = &mut song.plan.arrangement.phrases[phrase_ix];
            // A handed-off statement has one foreground speaker. The old lead must not keep
            // reciting an unrelated motif over a keys/bass question or its written silence.
            if !step.carriers.contains(&Agent::Lead) {
                arrangement.lead = ArrangementRole::Silent;
            }
            for carrier in &step.carriers {
                // Supporting seats preserve the foreground budget, while guaranteeing opportunity.
                match carrier {
                    Agent::Lead => arrangement.lead = ArrangementRole::Support,
                    Agent::Keys => arrangement.keys = ArrangementRole::Support,
                    Agent::Bass => arrangement.bass = ArrangementRole::Foundation,
                    _ => unreachable!("validated carrier"),
                }
            }
            placements.push(placement);
        }

        // One phrase-aligned harmonic commitment per phrase. Gaps prolong the previous gesture;
        // the final question pointer is never replaced by an unrelated chart picked beforehand.
        let mut relation = ArgumentRelation::Establish;
        let mut slots = Vec::with_capacity(count);
        for phrase in &song.plan.form.phrases {
            if let Some(p) = placements.iter().find(|p| p.phrase == phrase.ix) {
                relation = p.relation;
            }
            let goal = &mut song.plan.discourse.goals[phrase.ix as usize];
            goal.role = relation.role();
            goal.closure = match relation {
                ArgumentRelation::Question { .. } => Closure::Half,
                ArgumentRelation::Denial { .. } => Closure::Deceptive,
                ArgumentRelation::Develop { .. } => Closure::Open,
                ArgumentRelation::Answer { .. } | ArgumentRelation::Return { .. } => {
                    Closure::Strong
                }
                _ => Closure::Weak,
            };
            goal.refers_to = relation
                .predecessor()
                .and_then(|id| placements.iter().find(|p| p.step == id).map(|p| p.phrase));
            slots.push(GestureSlot {
                gesture: relation.gesture(),
                cycle: 0,
                start_bar: phrase.start_bar,
                bars: phrase.bars,
                binding: None,
                variation: if phrase.ix == 0 {
                    CycleVariation::Statement
                } else {
                    CycleVariation::Transformed
                },
            });
        }
        let roles: Vec<_> = song.plan.discourse.goals.iter().map(|g| g.role).collect();
        song.plan.discourse.ledger = resolve_obligations(&roles);
        song.plan.discourse.answer = placements
            .iter()
            .find(|p| matches!(p.relation, ArgumentRelation::Answer { .. }))
            .map(|p| p.phrase);
        song.plan.discourse.culmination = placements
            .iter()
            .rev()
            .find(|p| {
                matches!(
                    p.relation,
                    ArgumentRelation::Question { .. } | ArgumentRelation::Denial { .. }
                )
            })
            .map_or(0, |p| p.phrase);
        song.plan.backbone = Some(BackboneTimeline {
            scales: TimeScales {
                beats_per_bar: 4.0,
                phrase_bars: song.plan.contract.phrase_bars,
                binding: ClockBinding::FixedTiling {
                    bars_per_gesture: song.plan.contract.phrase_bars,
                },
            },
            slots,
            bindings: Vec::new(),
            total_bars: song.plan.form.total_bars,
        });
        let home = ChartRoot::Degree(0);
        let denied = ChartRoot::Degree(5);
        let prepared = ChartRoot::Chromatic {
            semitones: 7,
            quality: Quality::Dom7,
        };
        song.harmonic = Some(HarmonicMap {
            cell: ChartCell {
                // The source question occurs at the phrase's beginning. Merely ending the
                // accompaniment on a pointer later would not put that question over a prepared
                // dominant. The entire question slot therefore prolongs V7 in this calibration.
                lift: prepared,
                lift_alt: prepared,
                pointer: ChartRoot::Degree(4),
                expected: home,
                deflect: denied,
                // Development retains the open question's dominant debt. Its final pointer is
                // therefore actually adjacent to the answering tonic, rather than a chart label
                // pretending that an intervening IV-to-I plagal arrival was V7-to-I.
                open: prepared,
                reset: home,
                satellites: [denied, prepared, home],
            },
            bars_per_chord: 1,
        });
        let identity = motif_for(&placements[0]);
        let answer = placements
            .iter()
            .find(|p| matches!(p.relation, ArgumentRelation::Answer { .. }))
            .map(motif_for)
            .unwrap_or_else(|| identity.clone());
        song.thematic.bank = MotifBank {
            identity: identity.clone(),
            // A distinct rhythmic head retains source ancestry instead of always duplicating thesis.
            hook: identity.fragment(3).scale_rhythm(0.5),
            rhythmic_cell: identity.fragment(3).scale_rhythm(0.5),
            bass_cell: identity.fragment(3).transpose(-12),
            countermotif: Some(answer),
        };
        song.thematic.sites = placements
            .iter()
            .map(|p| ThemeSite {
                phrase: p.phrase,
                role: p.relation.role(),
                motif: motif_for(p),
                handoff: match p.relation {
                    ArgumentRelation::Establish | ArgumentRelation::Return { .. } => {
                        Handoff::Restatement
                    }
                    ArgumentRelation::Question { .. } | ArgumentRelation::Denial { .. } => {
                        Handoff::Call
                    }
                    ArgumentRelation::Answer { .. } => Handoff::Consequent,
                    _ => Handoff::Develop,
                },
            })
            .collect();
        // The old coarse target is still computed from its declared input and the actual new
        // plan. It is not replaced with observation to force a green commutation check, and
        // it is not the source-specific argument verifier.
        song.meaning = Some(MeaningPlan::target(&song.trace, &song.plan));
        Ok(CompiledArgument {
            song,
            placements,
            argument: self.clone(),
        })
    }

    /// Original, seeded source phrases generated through the same timed-source operations.
    /// This does not contain a final Score or a pre-rendered song.
    pub fn calibration(family: ArgumentFamily, seed: u64) -> Result<Self, ArgumentError> {
        if family == ArgumentFamily::ConflictReconciliation {
            return Err(ArgumentError::UnsupportedFamily);
        }
        let mut rng = Rng::new(seed ^ 0xA261_7E57_5EED);
        // C2.2 — a RICHER melodic theme (the lead crux). NOTE: the argument contract models the
        // antecedent/consequent of ONE theme (an answer must share the question's head+length, octave
        // and time-scale aside — `same_head`), so a distinct verse+hook PAIR of two tunes is NOT
        // expressible here without extending the contract. This is the contract-respecting step: one
        // reaching ~6.5-beat theme (11 notes, vs the old 6-note germ) that opens on the rootless
        // 2nd->5th germ leap, reaches the octave peak, and HANGS on the 2nd (step 2 = a question that
        // wants home). The Answer/Return steps resolve it to the tonic via their completion. Steps are
        // semitones from the tonic in the world's A-Aeolian; ~6.5 beats fits every phrase span.
        let peak = if rng.chance(0.5) { 10 } else { 12 };
        let theme = MusicalReferent {
            id: ReferentId(0),
            pitch_basis: PitchBasis::Semitones,
            events: [
                (0.0, 2, 0.5, 1.00),
                (0.5, 7, 0.5, 0.82),
                (1.0, 10, 0.5, 0.80),
                (1.5, peak, 0.75, 0.95),
                (2.25, 10, 0.5, 0.78),
                (2.75, 7, 0.5, 0.74),
                (3.25, 5, 0.5, 0.72),
                (3.75, 7, 0.5, 0.78),
                (4.25, 5, 0.25, 0.72),
                (4.5, 2, 0.5, 0.70),
            ]
            .into_iter()
            .map(|(onset, step, dur, accent)| MaterialEvent {
                onset,
                dur,
                accent,
                step: Some(step),
            })
            .collect(),
        };
        let terminal = Completion { step: 0, dur: 1.0 };
        let complete = ArgumentTransform {
            completion: Some(terminal),
            ..ArgumentTransform::default()
        };
        // Claude integration fix (pocket drift): the former 3/2 `augmented` time-scale mapped the
        // source's 0.25-grid onsets onto a 0.125 grid, so the carrier drifted out of phase against
        // the band's pocket (audible from ~beat 83 of the Develop, and again in the Answer/Return).
        // The only time ratio that is both pocket-coherent and fits the 8-beat answer/return spans is
        // 1/1, so the calibration now develops by carrier / relation / register (the hand-authored
        // teacher's device) and never by a grid-fighting tempo ratio. The general rational
        // time-scaling capability remains available in `ArgumentTransform` for other arguments.
        let make = |id, transform, relation, depends_on, carriers| ArgumentStep {
            id: ArgumentStepId(id),
            referent: ReferentId(0),
            transform,
            relation,
            depends_on,
            carriers,
        };
        let mut program = vec![
            make(
                0,
                complete,
                ArgumentRelation::Establish,
                vec![],
                vec![Agent::Lead],
            ),
            make(
                1,
                ArgumentTransform::default(),
                ArgumentRelation::Question {
                    source: ArgumentStepId(0),
                    expected: terminal,
                },
                vec![ArgumentStepId(0)],
                vec![Agent::Lead],
            ),
        ];
        if family == ArgumentFamily::PromiseDeniedReturn {
            program.push(make(
                2,
                ArgumentTransform {
                    completion: Some(Completion { step: 2, dur: 1.0 }),
                    ..ArgumentTransform::default()
                },
                ArgumentRelation::Denial {
                    question: ArgumentStepId(1),
                },
                vec![ArgumentStepId(1)],
                vec![Agent::Keys],
            ));
        } else {
            // The family contrast uses the same number of slots and carrier handoffs: where B
            // denies its prepared consequence, A keeps the original question open. Otherwise a
            // listener could identify a different story from shifted phrase times alone.
            program.push(make(
                2,
                ArgumentTransform::default(),
                ArgumentRelation::Develop {
                    source: ArgumentStepId(1),
                },
                vec![ArgumentStepId(1)],
                // Teacher-faithful: the lead STAYS singing the germ while keys join (a reinforce,
                // not a handoff). The tune is the crux; the band develops around it, it does not
                // replace it. Only family B's Denial drops the lead, as a single dramatic absence.
                vec![Agent::Lead, Agent::Keys],
            ));
        }
        let development = program.len() as u8;
        program.push(make(
            development,
            ArgumentTransform::default(),
            ArgumentRelation::Develop {
                source: ArgumentStepId(1),
            },
            vec![ArgumentStepId(1)],
            // Lead stays on the tune; bass joins underneath (develop, not handoff).
            vec![Agent::Lead, Agent::Bass],
        ));
        let answer = program.len() as u8;
        program.push(make(
            answer,
            complete,
            ArgumentRelation::Answer {
                question: ArgumentStepId(1),
            },
            vec![ArgumentStepId(1), ArgumentStepId(development)],
            // Lead answers its own question (resolves to the tonic); keys join for the arrival.
            vec![Agent::Lead, Agent::Keys],
        ));
        let returning = program.len() as u8;
        // Both families return via `complete` (1/1 + terminal): the A/B contrast is carried by
        // step 2 (B denies with a substituted completion; A keeps the question open), not by a
        // drifting time-scale on the return.
        let returned = complete;
        program.push(make(
            returning,
            returned,
            ArgumentRelation::Return {
                source: ArgumentStepId(0),
            },
            vec![ArgumentStepId(0), ArgumentStepId(answer)],
            vec![Agent::Lead, Agent::Keys],
        ));
        let argument = Self {
            family,
            referents: vec![theme],
            steps: program,
            ending: ArgumentEnding::Resolved,
        };
        argument.validate()?;
        Ok(argument)
    }
}

// Motif rhythm is an inter-onset page used by existing planning. It cannot represent sounding
// gates independently; the placement remains authoritative and the transport keeps those gates.
fn motif_for(p: &ArgumentPlacement) -> Motif {
    Motif {
        id: p.referent.0,
        pitch_basis: p.pitch_basis,
        degrees: p.events.iter().filter_map(|e| e.step).collect(),
        rhythm: p
            .events
            .iter()
            .enumerate()
            .map(|(i, e)| {
                p.events
                    .get(i + 1)
                    .map_or(e.dur, |next| next.onset - e.onset) as f32
            })
            .collect(),
    }
}

/// Preserve the argument's source-level functional chart before any player chooses notes.
///
/// The archived backbone colourer can turn a charted V7 into a major-seventh chord or remove
/// its seventh. That loses a declared prepared expectation. On this optional route, the source
/// chart's actual functional chords are therefore pinned at the existing harmonic change points.
/// This is not a note/PCM repair: all players, contexts and deflection witnesses receive the same
/// committed harmony. Incompatible vocabulary, modulation or discretionary harmonic edits are
/// refused instead of silently changing the source's target or the world's contract. In
/// particular, stock SwissSignal (which declares no sevenths) cannot realize this V7 calibration.
pub fn prepare_performance(
    argument: &CompiledArgument,
    performance: &mut PerformancePlan,
    world: &MusicWorld,
) -> Result<(), ArgumentError> {
    argument.argument.validate()?;
    if !performance.edits.is_empty() {
        return Err(ArgumentError::IncompatibleHarmonicEdit);
    }
    let vocabulary = HarmonicVocabulary::of(world, &performance.language);
    if performance.region != vocabulary.home {
        return Err(ArgumentError::IncompatibleTonalRegion);
    }
    let chart = argument
        .song
        .harmonic
        .as_ref()
        .ok_or(ArgumentError::IncompatibleTonalRegion)?;
    let mut chords = performance.chords.clone();
    for span in &mut chords {
        let region = performance.regions.region_at(span.start_beat);
        if region != performance.region {
            return Err(ArgumentError::IncompatibleTonalRegion);
        }
        let relation = argument
            .placements
            .iter()
            .rev()
            .find(|p| p.start_beat <= span.start_beat + 1e-9)
            .map_or(ArgumentRelation::Establish, |p| p.relation);
        let desired = match relation {
            ArgumentRelation::Question { .. } | ArgumentRelation::Develop { .. } => {
                Chord::new(chart.cell.pointer.root_pc(&region), Quality::Dom7)
            }
            ArgumentRelation::Denial { .. } => chart.cell.deflect.chord(&region),
            _ => chart.cell.reset.chord(&region),
        };
        if !vocabulary.admits(desired) {
            return Err(ArgumentError::IncompatibleHarmonicVocabulary);
        }
        span.chord = desired;
        span.function = super::context::contextual_function(&desired, &region);
        span.degree = ChartRoot::degree_of_pc(desired.root_pc, &region).unwrap_or(-1);
        span.note = "argument-chart";
    }
    // Atomic: no mutation happened until all source commitments were admitted.
    performance.contexts = super::context::analyze_regions(&chords, &performance.regions);
    if let Some(timeline) = &argument.song.plan.backbone {
        let pointer = Chord::new(
            chart.cell.pointer.root_pc(&performance.region),
            Quality::Dom7,
        );
        performance.deflects = super::backbone::deflect_witnesses(
            timeline,
            &chords,
            &|beat| performance.regions.region_at(beat),
            pointer,
        );
    }
    performance.chords = chords;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::contract::CompositionGrammar;
    use super::super::semantic::demo_trace;
    use super::*;

    #[test]
    fn original_seeded_sources_and_two_programs_compile() {
        let base = SongMap::build(
            &demo_trace(128.0),
            17,
            Some(CompositionGrammar::DeflectedLift),
        );
        for seed in [1, 2, 19, 701, 0xDEAD_BEEF] {
            for family in [
                ArgumentFamily::CallAndEarnedAnswer,
                ArgumentFamily::PromiseDeniedReturn,
            ] {
                let argument = MusicalArgument::calibration(family, seed).unwrap();
                let compiled = argument.compile(&base).unwrap();
                assert_eq!(compiled.placements.len(), argument.steps.len());
                assert!(compiled
                    .placements
                    .windows(2)
                    .all(|p| p[0].start_beat < p[1].start_beat));
                assert!(compiled
                    .placements
                    .iter()
                    .all(|p| material_length(&p.events) <= p.span_beats));
            }
        }
        assert_eq!(base.plan.discourse.goals[0].role, DiscourseRole::Establish);
    }

    #[test]
    fn unrelated_answer_generic_kind_cannot_pay_source_question() {
        let mut argument =
            MusicalArgument::calibration(ArgumentFamily::CallAndEarnedAnswer, 71).unwrap();
        let mut foreign = argument.referents[0].clone();
        foreign.id = ReferentId(1);
        foreign.events[2].step = Some(3);
        argument.referents.push(foreign);
        let answer = argument
            .steps
            .iter_mut()
            .find(|s| matches!(s.relation, ArgumentRelation::Answer { .. }))
            .unwrap();
        answer.referent = ReferentId(1);
        assert!(matches!(
            argument.validate(),
            Err(ArgumentError::WrongSource(_))
        ));
    }

    #[test]
    fn same_notes_wrong_order_and_untaught_denial_are_rejected() {
        let mut argument =
            MusicalArgument::calibration(ArgumentFamily::PromiseDeniedReturn, 81).unwrap();
        argument.steps.swap(1, 3);
        assert!(matches!(
            argument.validate(),
            Err(ArgumentError::MissingDependency(_))
        ));
        let mut argument =
            MusicalArgument::calibration(ArgumentFamily::PromiseDeniedReturn, 81).unwrap();
        argument.steps[0].transform.completion = None;
        assert!(matches!(
            argument.validate(),
            Err(ArgumentError::UnpreparedDenial(_))
        ));
    }

    #[test]
    fn unresolved_is_an_explicit_valid_ending_not_an_accidental_payment() {
        let mut argument =
            MusicalArgument::calibration(ArgumentFamily::CallAndEarnedAnswer, 7).unwrap();
        argument.steps.truncate(2);
        assert!(matches!(
            argument.validate(),
            Err(ArgumentError::UnresolvedQuestion(_))
        ));
        argument.ending = ArgumentEnding::IntentionallyUnresolved;
        assert_eq!(argument.validate(), Ok(()));
    }

    #[test]
    fn licensed_operations_obey_identity_and_associative_composition() {
        let a = ArgumentTransform {
            transpose_octaves: 1,
            time_numerator: 3,
            time_denominator: 2,
            completion: None,
        };
        let b = ArgumentTransform {
            transpose_octaves: -1,
            time_numerator: 2,
            time_denominator: 3,
            completion: None,
        };
        let c = ArgumentTransform {
            transpose_octaves: 1,
            time_numerator: 2,
            time_denominator: 1,
            completion: None,
        };
        assert_eq!(a.then(ArgumentTransform::default()).unwrap(), a);
        assert_eq!(a.then(b).unwrap(), ArgumentTransform::default());
        assert_eq!(
            a.then(b).unwrap().then(c).unwrap(),
            a.then(b.then(c).unwrap()).unwrap()
        );
        let source = &MusicalArgument::calibration(ArgumentFamily::CallAndEarnedAnswer, 19)
            .unwrap()
            .referents[0];
        let transformed = a.then(b).unwrap().apply(source).unwrap();
        assert_eq!(source.events, transformed);
    }

    #[test]
    fn source_preparation_retains_real_dominant_and_refuses_incompatible_world() {
        use super::super::context::PullEvidence;
        use super::super::performance::PerformanceOptions;
        let base = SongMap::build(
            &demo_trace(128.0),
            17,
            Some(CompositionGrammar::DeflectedLift),
        );
        let compiled = MusicalArgument::calibration(ArgumentFamily::CallAndEarnedAnswer, 91)
            .unwrap()
            .compile(&base)
            .unwrap();
        let options = PerformanceOptions {
            actions: false,
            ..PerformanceOptions::default()
        };
        for world in [MusicWorld::black_ice(), MusicWorld::vapor95()] {
            let mut performance = PerformancePlan::from_song(&compiled.song, &world, options);
            prepare_performance(&compiled, &mut performance, &world).unwrap();
            let question = &compiled.placements[1];
            let chord = performance
                .chords
                .iter()
                .find(|s| {
                    question.start_beat >= s.start_beat
                        && question.start_beat < s.start_beat + f64::from(s.dur_beats)
                })
                .unwrap()
                .chord;
            let evidence = PullEvidence::of(&chord, world.tonic_pc);
            assert!(evidence.leading_tone && evidence.resolving_tritone);
        }
        let world = MusicWorld::swiss_signal();
        let mut performance = PerformancePlan::from_song(&compiled.song, &world, options);
        let before = performance
            .chords
            .iter()
            .map(|s| s.chord)
            .collect::<Vec<_>>();
        assert_eq!(
            prepare_performance(&compiled, &mut performance, &world),
            Err(ArgumentError::IncompatibleHarmonicVocabulary)
        );
        assert_eq!(
            before,
            performance
                .chords
                .iter()
                .map(|s| s.chord)
                .collect::<Vec<_>>()
        );
    }
}
