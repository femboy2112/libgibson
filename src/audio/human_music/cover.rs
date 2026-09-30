//! Covering as an exact invariant projection and a constrained lift.
//!
//! A `CoverMap` contains only selected identities. Generation takes this value and a
//! target context, never a reference `Composition`. Relative pitches and metric
//! positions survive key, tempo and declared groove transport. The remaining band
//! is generated through the ordinary planner and final-source hearing pipeline.
use super::action::Agent;
use super::contract::{CoherenceAnchor, CoherenceContract, CompositionGrammar};
use super::fingerprint::{CanonicalFingerprint, FingerprintWriter};
use super::functor::Composition;
use super::harmony::ChordSpan;
use super::performance::{PerformanceOptions, PerformancePlan};
use super::plan::{ArrangementRole, FormGraph, Phrase, SectionFamily};
use super::rhythm::{GrooveTransport, MetricPosition};
use super::score::{DrumHit, DrumVoice, Note, Provenance, Role};
use super::song::SongMap;
use super::theory::{Chord, Quality};
use super::world::MusicWorld;

/// A selected axis has an exact relation, never a similarity threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverAxis {
    Motif,
    Riff,
    Groove,
    HarmonicContour,
    HarmonicLoop,
    Form,
    Orchestration,
    BassFigure,
}
impl CoverAxis {
    pub const ALL: [Self; 8] = [
        Self::Motif,
        Self::Riff,
        Self::Groove,
        Self::HarmonicContour,
        Self::HarmonicLoop,
        Self::Form,
        Self::Orchestration,
        Self::BassFigure,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Motif => "motif",
            Self::Riff => "riff",
            Self::Groove => "groove",
            Self::HarmonicContour => "harmonic-contour",
            Self::HarmonicLoop => "harmonic-loop",
            Self::Form => "form",
            Self::Orchestration => "orchestration",
            Self::BassFigure => "bass-figure",
        }
    }
    pub fn relation(self) -> &'static str {
        match self { Self::Motif|Self::Riff|Self::BassFigure=>"relative chromatic contour, canonical onsets and observed rest boundaries; global octave and shorter gates free", Self::HarmonicContour|Self::HarmonicLoop=>"relative roots, chord quality and exact metric span boundaries", Self::Groove=>"canonical kick/snare pattern; hats, clap, dynamics and timbre free", Self::Form=>"phrase boundary and family topology", Self::Orchestration=>"per-bar seated role topology; gain and patch free" }
    }
}
impl From<CoherenceAnchor> for CoverAxis {
    fn from(a: CoherenceAnchor) -> Self {
        match a {
            CoherenceAnchor::Motif => Self::Motif,
            CoherenceAnchor::Riff => Self::Riff,
            CoherenceAnchor::Groove => Self::Groove,
            CoherenceAnchor::HarmonicContour => Self::HarmonicContour,
            CoherenceAnchor::HarmonicLoop => Self::HarmonicLoop,
            CoherenceAnchor::Form => Self::Form,
            CoherenceAnchor::Orchestration => Self::Orchestration,
            CoherenceAnchor::BassFigure => Self::BassFigure,
        }
    }
}
impl CanonicalFingerprint for CoverAxis {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag(self.label());
    }
}

/// Selection is explicit. Exact piece length is the common domain, even with no axes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverSpec {
    axes: Vec<CoverAxis>,
}
impl CoverSpec {
    /// A grammar's declared recognition anchors, plus the phrase/family scaffold.
    pub fn from_contract(contract: &CoherenceContract) -> Self {
        let mut axes: Vec<_> = contract.anchors.iter().copied().map(Into::into).collect();
        axes.push(CoverAxis::Form);
        Self::new(axes)
    }
    pub fn new(axes: impl IntoIterator<Item = CoverAxis>) -> Self {
        let input: Vec<_> = axes.into_iter().collect();
        Self {
            axes: CoverAxis::ALL
                .into_iter()
                .filter(|a| input.contains(a))
                .collect(),
        }
    }
    pub fn all() -> Self {
        Self::new(CoverAxis::ALL)
    }
    pub fn none() -> Self {
        Self::new([])
    }
    pub fn axes(&self) -> &[CoverAxis] {
        &self.axes
    }
    pub fn contains(&self, axis: CoverAxis) -> bool {
        self.axes.contains(&axis)
    }
    pub fn with(mut self, axis: CoverAxis, pinned: bool) -> Self {
        self.axes.retain(|a| *a != axis);
        if pinned {
            self.axes.push(axis);
        }
        Self::new(self.axes)
    }
    /// A machine description of the selection, not a listening-quality verdict.
    pub fn has_song_identity(&self) -> bool {
        self.axes.iter().any(|a| {
            matches!(
                a,
                CoverAxis::Motif
                    | CoverAxis::Riff
                    | CoverAxis::HarmonicContour
                    | CoverAxis::HarmonicLoop
                    | CoverAxis::BassFigure
            )
        })
    }
}
impl CanonicalFingerprint for CoverSpec {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag("CoverSpec/v1");
        w.field("axes", &self.axes);
    }
}

/// Sounding melody/riff identity. Duration, velocity, register and function are not copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverNote {
    pub at: MetricPosition,
    pub relative_pitch: i32,
    /// Known metric note/rest boundary; sounding gate may shorten, never fill the rest.
    pub reserved_until: Option<MetricPosition>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverLine {
    pub role: Role,
    pub notes: Vec<CoverNote>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverChord {
    pub at: MetricPosition,
    pub end: MetricPosition,
    pub relative_root: i32,
    pub quality: Quality,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverPhrase {
    pub start_bar: u32,
    pub bars: u32,
    pub family: SectionFamily,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrooveVoice {
    Kick,
    Snare,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverStroke {
    pub at: MetricPosition,
    pub voice: GrooveVoice,
}
/// Per bar, in Stage's Lead/Keys/Pad/Bass/Drums order. Gain is deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverSeats {
    pub roles: [ArrangementRole; 5],
}

/// Epistemic state of an axis. Unknown is not an instruction to copy or invent it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverKnowledge {
    Invariant,
    Free,
    Unknown,
}
/// An observed chord order, without a claim about meter or duration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedChord {
    pub relative_root: i32,
    pub quality: Quality,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedSection {
    pub family: String,
    pub chords: Option<Vec<OrderedChord>>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedChart {
    pub sections: Vec<OrderedSection>,
}
/// Explicit target composition decisions for an unmetered reference. These are
/// output scaffolding, never promoted to source observations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkeletonSchedule {
    pub bars_per_chord: u32,
}

/// The quotient value: no source seed, trace, complete plan, score, patch or lookup key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverMap {
    pub spec: CoverSpec,
    /// Unobserved axes remain distinct from observed coordinates deliberately freed.
    pub unknown_axes: Vec<CoverAxis>,
    /// An ordered chart has no inferred source durations. It is enriched into the
    /// same map when metric observations become available.
    pub ordered_chart: Option<OrderedChart>,
    pub length: Option<MetricPosition>,
    pub form: Option<Vec<CoverPhrase>>,
    pub motif: Option<CoverLine>,
    pub riff: Option<CoverLine>,
    pub harmony: Option<Vec<CoverChord>>,
    pub groove: Option<Vec<CoverStroke>>,
    pub bass: Option<CoverLine>,
    pub orchestration: Option<Vec<CoverSeats>>,
}

/// A missing/unsupported source is an error, never permission to fabricate notes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverError {
    Invalid(&'static str),
    MissingAxis(CoverAxis),
    UnprojectableTiming,
    ConflictingPins,
    Policy(String),
    Rejected(Box<CoverAdmission>),
}
impl std::fmt::Display for CoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for CoverError {}
fn metric(value: f64) -> Result<MetricPosition, CoverError> {
    MetricPosition::from_exact_beats(value).ok_or(CoverError::UnprojectableTiming)
}

impl CoverMap {
    /// Extract actual sounded pitches and actual chord spans. `world` declares the
    /// legacy percussion transport, whose bounded jitter is uniquely projected.
    pub fn extract(
        reference: &Composition,
        world: &MusicWorld,
        spec: CoverSpec,
    ) -> Result<Self, CoverError> {
        let score = &reference.score;
        let extract_line = |role: Role| -> Result<CoverLine, CoverError> {
            let mut sounding: Vec<_> = score.role_notes(role).collect();
            sounding.sort_by(|a, b| {
                a.start_beat
                    .total_cmp(&b.start_beat)
                    .then(a.pitch.cmp(&b.pitch))
            });
            let Some(first) = sounding.first() else {
                return Err(CoverError::MissingAxis(if role == Role::Bass {
                    CoverAxis::BassFigure
                } else {
                    CoverAxis::Motif
                }));
            };
            let origin =
                first.pitch - world.tonic_pc - (first.pitch - world.tonic_pc).rem_euclid(12);
            let mut notes = Vec::new();
            for n in sounding {
                let at = if let Some(c) = reference
                    .perf
                    .cover_constraints
                    .as_ref()
                    .filter(|c| c.identity.line(role).is_some())
                {
                    c.project_note(role, n.start_beat)?
                } else {
                    // Historical phrase evidence identifies the source coordinate. It does not
                    // supply the projected pitch: the sounding note above remains the observer.
                    let source = score
                        .expression_decisions
                        .iter()
                        .rev()
                        .find(|d| {
                            d.after.is_some_and(|a| {
                                a.role == n.role
                                    && a.start_beat == n.start_beat
                                    && a.pitch == n.pitch
                            })
                        })
                        .map_or(n.start_beat, |d| d.before.note.start_beat);
                    metric(source)?
                };
                let reserved_until = reference
                    .perf
                    .cover_constraints
                    .as_ref()
                    .and_then(|c| c.identity.line(role))
                    .and_then(|l| l.notes.iter().find(|p| p.at == at))
                    .and_then(|p| p.reserved_until);
                if let Some(end) = reserved_until {
                    let limit = reference
                        .perf
                        .cover_constraints
                        .as_ref()
                        .unwrap()
                        .transport
                        .transport(end)
                        .beats();
                    if n.start_beat + f64::from(n.dur_beats) > limit {
                        return Err(CoverError::Invalid(
                            "note sounds through a pinned rest boundary",
                        ));
                    }
                }
                notes.push(CoverNote {
                    at,
                    relative_pitch: n.pitch - world.tonic_pc - origin,
                    reserved_until,
                });
            }
            notes.sort_by_key(|n| n.at);
            Ok(CoverLine { role, notes })
        };
        let motif = spec
            .contains(CoverAxis::Motif)
            .then(|| extract_line(Role::Lead))
            .transpose()?;
        let riff = spec
            .contains(CoverAxis::Riff)
            .then(|| {
                extract_line(if score.role_notes(Role::Lead).next().is_some() {
                    Role::Lead
                } else {
                    Role::Bass
                })
            })
            .transpose()?;
        let bass = spec
            .contains(CoverAxis::BassFigure)
            .then(|| extract_line(Role::Bass))
            .transpose()?;
        let harmony = if spec.contains(CoverAxis::HarmonicContour)
            || spec.contains(CoverAxis::HarmonicLoop)
        {
            if score.chords.is_empty() {
                return Err(CoverError::MissingAxis(CoverAxis::HarmonicContour));
            }
            Some(
                score
                    .chords
                    .iter()
                    .map(|c| {
                        Ok(CoverChord {
                            at: metric(c.start_beat)?,
                            end: metric(c.start_beat + f64::from(c.dur_beats))?,
                            relative_root: (c.chord.root_pc - world.tonic_pc).rem_euclid(12),
                            quality: c.chord.quality,
                        })
                    })
                    .collect::<Result<Vec<_>, CoverError>>()?,
            )
        } else {
            None
        };
        let groove = if spec.contains(CoverAxis::Groove) {
            let transport =
                GrooveTransport::eighth_swing(world.swing).ok_or(CoverError::Invalid("swing"))?;
            let mut strokes = Vec::new();
            for hit in &score.drums {
                let voice = match hit.voice {
                    DrumVoice::Kick => GrooveVoice::Kick,
                    DrumVoice::Snare => GrooveVoice::Snare,
                    _ => continue,
                };
                let at = if let Some(c) = reference
                    .perf
                    .cover_constraints
                    .as_ref()
                    .filter(|c| c.identity.groove.is_some())
                {
                    c.project_stroke(hit.start_beat)?
                } else {
                    project_legacy_stroke(hit.start_beat, transport)?
                };
                strokes.push(CoverStroke { at, voice });
            }
            strokes.sort_by_key(|s| {
                (
                    s.at,
                    match s.voice {
                        GrooveVoice::Kick => 0,
                        GrooveVoice::Snare => 1,
                    },
                )
            });
            if strokes.is_empty() {
                return Err(CoverError::MissingAxis(CoverAxis::Groove));
            }
            Some(strokes)
        } else {
            None
        };
        if spec.contains(CoverAxis::Form) {
            let planned: Vec<_> = reference
                .song
                .plan
                .form
                .phrases
                .iter()
                .map(|p| (p.start_bar, p.bars, p.family.to_section_kind()))
                .collect();
            let sounded: Vec<_> = score
                .sections
                .iter()
                .map(|s| (s.start_bar, s.bars, s.kind))
                .collect();
            if planned != sounded {
                return Err(CoverError::Invalid("actual sections disagree with form"));
            }
        }
        if spec.contains(CoverAxis::Orchestration) {
            let offstage =
                !super::functor::orchestration_violations(&reference.perf, score).is_empty();
            if offstage {
                return Err(CoverError::Invalid(
                    "actual event outside admitted orchestration",
                ));
            }
        }
        let out = Self {
            unknown_axes: reference
                .perf
                .cover_constraints
                .as_ref()
                .map_or_else(Vec::new, |c| {
                    c.identity
                        .unknown_axes
                        .iter()
                        .copied()
                        .filter(|a| !spec.contains(*a))
                        .collect()
                }),
            ordered_chart: None,
            length: Some(metric(score.total_beats)?),
            form: spec.contains(CoverAxis::Form).then(|| {
                reference
                    .song
                    .plan
                    .form
                    .phrases
                    .iter()
                    .map(|p| CoverPhrase {
                        start_bar: p.start_bar,
                        bars: p.bars,
                        family: p.family,
                    })
                    .collect()
            }),
            orchestration: spec.contains(CoverAxis::Orchestration).then(|| {
                reference
                    .perf
                    .stage
                    .seats
                    .iter()
                    .map(|s| CoverSeats {
                        roles: s.map(|x| x.role),
                    })
                    .collect()
            }),
            spec,
            motif,
            riff,
            harmony,
            groove,
            bass,
        };
        out.validate()?;
        Ok(out)
    }
    pub fn knowledge(&self, axis: CoverAxis) -> CoverKnowledge {
        if self.unknown_axes.contains(&axis) {
            CoverKnowledge::Unknown
        } else if self.spec.contains(axis) {
            CoverKnowledge::Invariant
        } else {
            CoverKnowledge::Free
        }
    }
    fn metric_length(&self) -> MetricPosition {
        self.length
            .expect("metric lift validates observed or explicitly planned length")
    }
    pub fn validate(&self) -> Result<(), CoverError> {
        if self.unknown_axes
            != CoverAxis::ALL
                .into_iter()
                .filter(|a| self.unknown_axes.contains(a))
                .collect::<Vec<_>>()
        {
            return Err(CoverError::Invalid(
                "unknown axes must use canonical set order",
            ));
        }
        if self.unknown_axes.iter().any(|a| self.spec.contains(*a)) {
            return Err(CoverError::Invalid(
                "an axis cannot be both pinned and unknown",
            ));
        }
        if let Some(chart) = &self.ordered_chart {
            if self.length.is_some()
                || self.form.is_some()
                || self.motif.is_some()
                || self.riff.is_some()
                || self.harmony.is_some()
                || self.groove.is_some()
                || self.bass.is_some()
                || self.orchestration.is_some()
            {
                return Err(CoverError::Invalid(
                    "ordered chart and metric fields require explicit enrichment",
                ));
            }
            if self.spec != CoverSpec::new([CoverAxis::Form, CoverAxis::HarmonicContour]) {
                return Err(CoverError::Invalid("ordered chart pin shape"));
            }
            if chart.sections.is_empty()
                || chart.sections.iter().any(|s| {
                    s.family.is_empty()
                        || s.chords.as_ref().is_some_and(|cs| {
                            cs.is_empty() || cs.iter().any(|c| !(0..12).contains(&c.relative_root))
                        })
                })
            {
                return Err(CoverError::Invalid("invalid ordered chart"));
            }
            return Ok(());
        }
        let length = self
            .length
            .ok_or(CoverError::Invalid("unknown metric length"))?;
        if length.beats() <= 0.0 {
            return Err(CoverError::Invalid("nonpositive length"));
        }
        for (axis, present) in [
            (CoverAxis::Form, self.form.is_some()),
            (CoverAxis::Motif, self.motif.is_some()),
            (CoverAxis::Riff, self.riff.is_some()),
            (CoverAxis::Groove, self.groove.is_some()),
            (CoverAxis::BassFigure, self.bass.is_some()),
            (CoverAxis::Orchestration, self.orchestration.is_some()),
        ] {
            if self.spec.contains(axis) != present {
                return Err(CoverError::MissingAxis(axis));
            }
        }
        if (self.spec.contains(CoverAxis::HarmonicContour)
            || self.spec.contains(CoverAxis::HarmonicLoop))
            != self.harmony.is_some()
        {
            return Err(CoverError::MissingAxis(CoverAxis::HarmonicContour));
        }
        if self.motif.as_ref().is_some_and(|l| l.role != Role::Lead)
            || self.bass.as_ref().is_some_and(|l| l.role != Role::Bass)
            || self
                .riff
                .as_ref()
                .is_some_and(|l| !matches!(l.role, Role::Lead | Role::Bass))
        {
            return Err(CoverError::Invalid("axis role"));
        }
        for line in [&self.motif, &self.riff, &self.bass].into_iter().flatten() {
            if line.notes.is_empty() {
                return Err(CoverError::Invalid("empty pinned line"));
            }
            if line
                .notes
                .iter()
                .any(|n| !(-108..=108).contains(&n.relative_pitch))
            {
                return Err(CoverError::Invalid("relative pitch domain"));
            }
            if let Some(seats) = &self.orchestration {
                let ix = if line.role == Role::Lead { 0 } else { 3 };
                if line.notes.iter().any(|n| {
                    seats
                        .get((n.at.beats() / 4.0) as usize)
                        .is_none_or(|s| !s.roles[ix].is_audible())
                }) {
                    return Err(CoverError::ConflictingPins);
                }
            }
            let mut prev = None;
            for n in &line.notes {
                if n.at.beats() < 0.0
                    || n.at >= length
                    || prev.is_some_and(|p| p >= n.at)
                    || n.reserved_until
                        .is_some_and(|end| end <= n.at || end > length)
                {
                    return Err(CoverError::Invalid(
                        "line requires ordered monophonic metric events",
                    ));
                }
                prev = Some(n.at);
            }
        }
        for role in [Role::Lead, Role::Bass] {
            let lines: Vec<_> = [&self.motif, &self.riff, &self.bass]
                .into_iter()
                .flatten()
                .filter(|l| l.role == role)
                .collect();
            if lines.windows(2).any(|w| w[0] != w[1]) {
                return Err(CoverError::ConflictingPins);
            }
        }
        if let Some(form) = &self.form {
            let mut next = 0u32;
            for p in form {
                if p.start_bar != next || p.bars == 0 {
                    return Err(CoverError::Invalid("form must partition bars"));
                }
                next = next
                    .checked_add(p.bars)
                    .ok_or(CoverError::Invalid("form bar extent overflow"))?;
            }
            if next != super::form::bars_spanning(self.metric_length().beats(), 4.0) {
                return Err(CoverError::Invalid("form length"));
            }
        }
        if let Some(harmony) = &self.harmony {
            let mut end = metric(0.0)?;
            for h in harmony {
                if h.at != end
                    || h.end <= h.at
                    || h.end > length
                    || !(0..12).contains(&h.relative_root)
                {
                    return Err(CoverError::Invalid("harmony must partition metric time"));
                }
                end = h.end;
            }
            if end != length {
                return Err(CoverError::Invalid("incomplete harmony"));
            }
        }
        if let Some(groove) = &self.groove {
            let key = |s: &CoverStroke| {
                (
                    s.at,
                    match s.voice {
                        GrooveVoice::Kick => 0,
                        GrooveVoice::Snare => 1,
                    },
                )
            };
            if groove.windows(2).any(|w| key(&w[0]) > key(&w[1])) {
                return Err(CoverError::Invalid(
                    "groove strokes must use canonical order",
                ));
            }

            if groove.iter().any(|s| s.at.beats() < 0.0 || s.at >= length) {
                return Err(CoverError::Invalid("groove outside piece"));
            }
        }
        if self.orchestration.as_ref().is_some_and(|s| {
            s.len() != super::form::bars_spanning(self.metric_length().beats(), 4.0) as usize
        }) {
            return Err(CoverError::Invalid("orchestration length"));
        }
        Ok(())
    }
    pub(crate) fn line(&self, role: Role) -> Option<&CoverLine> {
        [&self.motif, &self.riff, &self.bass]
            .into_iter()
            .flatten()
            .find(|l| l.role == role)
    }
}

/// Historical drums add at most this exact f32 bound in beat coordinates. Only
/// uniquely identified quarter-grid sources are admitted; this is not a tolerance
/// used by modern transported events, whose source coordinates are exact.
fn project_legacy_stroke(
    at: f64,
    transport: GrooveTransport,
) -> Result<MetricPosition, CoverError> {
    let center = (at * 4.0).round() as i64;
    let mut candidates = Vec::new();
    for k in center - 2..=center + 2 {
        let m = MetricPosition::new(k, 4).ok_or(CoverError::UnprojectableTiming)?;
        for time in [m.beats(), transport.transport(m).beats()] {
            if (time - at).abs() <= f64::from(0.008_f32) && !candidates.contains(&m) {
                candidates.push(m);
            }
        }
    }
    if candidates.len() == 1 {
        Ok(candidates[0])
    } else {
        Err(CoverError::UnprojectableTiming)
    }
}

/// Immutable constraints consumed BEFORE harmony, stage, interaction and source choices.
#[derive(Debug, Clone, PartialEq)]
pub struct CoverConstraints {
    pub identity: CoverMap,
    pub transport: GrooveTransport,
    pub seed: u64,
    pub tonic: i32,
    pub occupancy_policy: super::policy::OccupancyPolicy,
}
impl CoverConstraints {
    pub(crate) fn validate_target(&self) -> Result<(), CoverError> {
        if !(0..12).contains(&self.tonic) {
            return Err(CoverError::Invalid("target tonic"));
        }
        for line in [
            &self.identity.motif,
            &self.identity.riff,
            &self.identity.bass,
        ]
        .into_iter()
        .flatten()
        {
            if !(0..=10).any(|octave| {
                line.notes
                    .iter()
                    .all(|n| (0..128).contains(&(self.tonic + octave * 12 + n.relative_pitch)))
            }) {
                return Err(CoverError::Invalid("pinned contour exceeds MIDI range"));
            }
            if line.notes.iter().any(|n| {
                self.transport.transport(n.at).beats() >= self.identity.metric_length().beats()
            }) {
                return Err(CoverError::Invalid(
                    "target groove moves attack outside extent",
                ));
            }
            if line.notes.windows(2).any(|p| {
                self.transport.transport(p[0].at).beats()
                    >= self.transport.transport(p[1].at).beats()
            }) {
                return Err(CoverError::Invalid(
                    "target groove is not strictly monotone on pinned attacks",
                ));
            }
            if line.notes.iter().any(|n| {
                n.reserved_until.is_some_and(|end| {
                    self.transport.transport(end).beats() <= self.transport.transport(n.at).beats()
                })
            }) {
                return Err(CoverError::Invalid(
                    "target groove reverses a pinned rest boundary",
                ));
            }
        }
        if let Some(strokes) = &self.identity.groove {
            let mut positions = strokes.iter().map(|s| s.at).collect::<Vec<_>>();
            positions.dedup();
            if positions.windows(2).any(|p| {
                self.transport.transport(p[0]).beats() >= self.transport.transport(p[1]).beats()
            }) {
                return Err(CoverError::Invalid(
                    "target groove is not strictly monotone on pinned strokes",
                ));
            }
            if positions.iter().any(|p| {
                self.transport.transport(*p).beats() >= self.identity.metric_length().beats()
            }) {
                return Err(CoverError::Invalid(
                    "target groove moves stroke outside extent",
                ));
            }
        }
        Ok(())
    }
    /// Pinned attacks are structural authored metric events. Instrument gates and
    /// transported acoustic positions never replace their semantic reservations.
    pub(crate) fn occupancy(
        &self,
        perf: &PerformancePlan,
        source: &[Note],
        role: Role,
    ) -> Option<super::occupancy::AuthoredOccupancy> {
        use super::occupancy::{
            AuthoredOccupancy, OwnershipKind, OwnershipSpan, RhythmReservation,
        };
        let line = self.identity.line(role)?;
        let mut rhythm = Vec::new();
        let mut spans = Vec::new();
        for (i, pin) in line.notes.iter().enumerate() {
            let note = source.iter().find(|n| {
                n.role == role && n.start_beat == self.transport.transport(pin.at).beats()
            })?;
            let next = line
                .notes
                .get(i + 1)
                .map_or(self.identity.metric_length(), |n| n.at)
                .beats();
            let end = pin.reserved_until.map_or(next, |e| e.beats().min(next));
            rhythm.push(RhythmReservation {
                beat: pin.at.beats(),
                end_beat: end,
                structural: true,
                material: note.prov.material,
                actions: note.prov.actions,
            });
            if end < next {
                spans.push(OwnershipSpan {
                    start: end,
                    end: next,
                    material: note.prov.material,
                    kind: OwnershipKind::InternalRest,
                });
            }
        }
        if role == Role::Lead {
            spans.extend(perf.statements.iter().map(|s| OwnershipSpan {
                start: s.start_beat,
                end: s.end_beat(),
                material: Some(s.material),
                kind: OwnershipKind::Phrase,
            }));
        }
        Some(AuthoredOccupancy {
            role,
            rhythm,
            spans,
        })
    }
    pub(crate) fn plan_statements(
        &self,
        plan: &super::plan::CompositionPlan,
        statements: &mut Vec<super::interaction::LeadStatement>,
        materials: &mut Vec<super::material::InteractionMaterial>,
    ) {
        let Some(line) = self.identity.line(Role::Lead) else {
            return;
        };
        for phrase in &plan.form.phrases {
            let notes: Vec<_> = line
                .notes
                .iter()
                .filter(|n| n.at.beats() >= phrase.start_beat() && n.at.beats() < phrase.end_beat())
                .collect();
            if notes.is_empty() {
                continue;
            }
            let first = notes[0].at.beats();
            let motif = super::motif::Motif {
                pitch_basis: super::theory::PitchBasis::Semitones,
                id: (phrase.ix % 256) as u8,
                degrees: notes.iter().map(|n| n.relative_pitch).collect(),
                rhythm: notes
                    .iter()
                    .enumerate()
                    .map(|(i, n)| {
                        (notes.get(i + 1).map_or(phrase.end_beat(), |x| x.at.beats())
                            - n.at.beats()) as f32
                    })
                    .collect(),
            };
            let id = super::ids::MaterialId(materials.len() as u32);
            let material = super::material::InteractionMaterial::from_motif(
                id,
                Agent::Lead,
                &motif,
                first,
                super::material::MaterialSource::Statement {
                    statement: statements.len(),
                    motif: motif.id,
                },
            );
            materials.push(material);
            let goal = plan.discourse.goal(phrase.ix as usize);
            statements.push(super::interaction::LeadStatement {
                phrase: phrase.ix,
                start_beat: first,
                motif,
                handoff: super::motif::Handoff::Restatement,
                role: goal.role,
                energy: goal.energy_target,
                register: goal.register_target,
                is_rupture: phrase.is_rupture,
                call: None,
                material: id,
                answers: None,
                fragment: None,
            });
        }
    }
    pub(crate) fn project_note(&self, role: Role, at: f64) -> Result<MetricPosition, CoverError> {
        let Some(line) = self.identity.line(role) else {
            return metric(at);
        };
        line.notes
            .iter()
            .find(|n| self.transport.transport(n.at).beats() == at)
            .map(|n| n.at)
            .ok_or(CoverError::UnprojectableTiming)
    }
    fn project_stroke(&self, at: f64) -> Result<MetricPosition, CoverError> {
        self.identity
            .groove
            .as_ref()
            .and_then(|s| {
                s.iter()
                    .find(|s| self.transport.transport(s.at).beats() == at)
            })
            .map(|s| s.at)
            .ok_or(CoverError::UnprojectableTiming)
    }
    pub(crate) fn harmony(
        &self,
        world: &MusicWorld,
        language: &super::language::MusicalLanguage,
    ) -> Result<Option<Vec<ChordSpan>>, CoverError> {
        let scale = super::theory::Scale::new(self.tonic, world.mode);
        if let Some(hs) = &self.identity.harmony {
            if hs.iter().any(|h| {
                !chord_admitted(
                    Chord::new(self.tonic + h.relative_root, h.quality),
                    world,
                    language,
                )
            }) {
                return Err(CoverError::Invalid(
                    "pinned harmony outside target vocabulary",
                ));
            }
            return Ok(Some(
                hs.iter()
                    .map(|h| ChordSpan {
                        start_beat: h.at.beats(),
                        dur_beats: (h.end.beats() - h.at.beats()) as f32,
                        chord: Chord::new(self.tonic + h.relative_root, h.quality),
                        function: super::context::contextual_function(
                            &Chord::new(self.tonic + h.relative_root, h.quality),
                            &scale,
                        ),
                        degree: -1,
                        note: "cover-chart",
                    })
                    .collect(),
            ));
        }
        let lines: Vec<_> = [
            self.identity.line(Role::Lead),
            self.identity.line(Role::Bass),
        ]
        .into_iter()
        .flatten()
        .collect();
        if lines.is_empty() {
            return Ok(None);
        }
        // A harmonic candidate is admitted only if it contains every pinned attack
        // in its window. Choose maximal windows up to one bar, then minimize root
        // motion and target vocabulary cost. Constraints precede contexts and notes.
        let mut starts: Vec<f64> = lines
            .iter()
            .flat_map(|l| l.notes.iter().map(|n| n.at.beats()))
            .collect();
        starts.push(0.0);
        starts.push(self.identity.metric_length().beats());
        starts.sort_by(f64::total_cmp);
        starts.dedup();
        let mut qualities = vec![
            Quality::Maj,
            Quality::Min,
            Quality::Dim,
            Quality::Sus2,
            Quality::Sus4,
        ];
        if world.use_sevenths {
            qualities.extend([Quality::Maj7, Quality::Min7, Quality::Dom7, Quality::Min7b5]);
        }
        let candidates: Vec<_> = (0..12)
            .filter(|pc| world.allow_modal_mixture || scale.contains_pc(*pc))
            .flat_map(|root| qualities.iter().map(move |q| Chord::new(root, *q)))
            .filter(|chord| chord_admitted(*chord, world, language))
            .collect();
        let mut out = Vec::new();
        let mut i = 0;
        let mut previous = self.tonic;
        while i + 1 < starts.len() {
            let begin = starts[i];
            let max_end = (begin / 4.0 + 1.0).floor() * 4.0;
            let mut chosen = None;
            for (end_index, &end) in starts.iter().enumerate().skip(i + 1) {
                if end > max_end && end_index > i + 1 {
                    break;
                }
                let pcs: Vec<_> = lines
                    .iter()
                    .flat_map(|l| l.notes.iter())
                    .filter(|n| n.at.beats() >= begin && n.at.beats() < end)
                    .map(|n| (self.tonic + n.relative_pitch).rem_euclid(12))
                    .collect();
                let best = candidates
                    .iter()
                    .copied()
                    .filter(|c| pcs.iter().all(|pc| c.contains_pc(*pc)))
                    .min_by_key(|c| {
                        let motion = (c.root_pc - previous)
                            .rem_euclid(12)
                            .min((previous - c.root_pc).rem_euclid(12));
                        let color = c.quality.intervals().len() as i32 - 3;
                        let home = i32::from(c.root_pc != self.tonic);
                        let tie = ((c.root_pc as u64 + self.seed) % 12) as i32;
                        (motion * 3 + color + home, tie)
                    });
                if let Some(chord) = best {
                    chosen = Some((end_index, chord));
                } else {
                    break;
                }
            }
            let (end_index, chord) = chosen.ok_or(CoverError::Invalid(
                "no lawful harmony contains the pinned simultaneous attacks",
            ))?;
            out.push(ChordSpan {
                start_beat: begin,
                dur_beats: (starts[end_index] - begin) as f32,
                chord,
                function: super::context::contextual_function(&chord, &scale),
                degree: -1,
                note: "cover-generated-harmony",
            });
            previous = chord.root_pc;
            i = end_index;
        }
        Ok(Some(out))
    }
    /// A target discourse settlement must become a real source arrival before
    /// notes are made. No settlement is certified merely from the phrase label.
    pub(crate) fn plan_settlements(
        &self,
        plan: &super::plan::CompositionPlan,
        chords: &[ChordSpan],
        actions: &mut super::action::ActionPlan,
    ) {
        use super::action::{ActionCause, ActionKind, EffectVector, MusicalAction};
        use super::discourse::ObligationKind;
        for debt in &plan.discourse.ledger.obligations {
            let Some(settlement) = debt.settlement else {
                continue;
            };
            let Some(phrase) = plan.form.phrases.get(settlement.by_phrase as usize) else {
                continue;
            };
            if debt.kind == ObligationKind::GrooveDestabilization {
                let at = phrase.start_beat();
                if self.identity.groove.is_none()
                    && !actions
                        .of_kind(ActionKind::ReEntry)
                        .any(|a| a.start_beat >= at && a.start_beat < phrase.end_beat())
                {
                    actions.push(MusicalAction {
                        id: super::ids::ActionId(0),
                        cause: ActionCause::Discourse {
                            obligation: debt.id,
                            phrase: phrase.ix,
                        },
                        initiator: Agent::Drums,
                        start_beat: at,
                        dur_beats: (phrase.end_beat() - at).min(1.0),
                        kind: ActionKind::ReEntry,
                        target_beat: Some(at),
                        responders: Vec::new(),
                        binding: None,
                        pays: None,
                        effect: EffectVector::NEUTRAL,
                    });
                }
                continue;
            }
            if !matches!(
                debt.kind,
                ObligationKind::HarmonicDeparture | ObligationKind::SuspendedCadence
            ) {
                continue;
            }
            if actions
                .of_kind(ActionKind::Resolve)
                .any(|a| a.start_beat >= phrase.start_beat() && a.start_beat < phrase.end_beat())
            {
                continue;
            }
            let Some(line) = self.identity.line(Role::Lead) else {
                continue;
            };
            let arrival = line.notes.iter().find(|n| {
                let at = self.transport.transport(n.at).beats();
                at >= phrase.start_beat()
                    && at < phrase.end_beat()
                    && chords.iter().any(|h| {
                        at >= h.start_beat
                            && at < h.start_beat + f64::from(h.dur_beats)
                            && h.chord.root_pc == self.tonic
                            && h.chord.contains_pc(self.tonic + n.relative_pitch)
                    })
            });
            if let Some(note) = arrival {
                let at = self.transport.transport(note.at).beats();
                actions.push(MusicalAction {
                    id: super::ids::ActionId(0),
                    cause: ActionCause::Discourse {
                        obligation: debt.id,
                        phrase: phrase.ix,
                    },
                    initiator: Agent::Lead,
                    start_beat: at,
                    dur_beats: (phrase.end_beat() - at).min(1.0),
                    kind: ActionKind::Resolve,
                    target_beat: Some(at),
                    responders: Vec::new(),
                    binding: None,
                    pays: None,
                    effect: EffectVector::NEUTRAL,
                });
            }
        }
    }
    pub(crate) fn apply_stage(&self, stage: &mut super::ensemble::Stage) {
        if let Some(seats) = &self.identity.orchestration {
            for (bar, pin) in stage.seats.iter_mut().zip(seats) {
                for (seat, role) in bar.iter_mut().zip(pin.roles) {
                    seat.role = role;
                    seat.on = role.is_audible();
                    seat.gain = role.gain();
                }
            }
        }
        for role in [Role::Lead, Role::Bass] {
            if let Some(line) = self.identity.line(role) {
                let agent = if role == Role::Lead {
                    Agent::Lead
                } else {
                    Agent::Bass
                };
                let ix = super::ensemble::seat_ix(agent).unwrap();
                for n in &line.notes {
                    if let Some(bar) = stage.seats.get_mut((n.at.beats() / 4.0) as usize) {
                        if self.identity.orchestration.is_none() {
                            bar[ix].on = true;
                            bar[ix].role = if role == Role::Lead {
                                ArrangementRole::Foreground
                            } else {
                                ArrangementRole::Foundation
                            };
                            bar[ix].gain = bar[ix].role.gain();
                        }
                    }
                }
            }
        }
        if self.identity.groove.is_some() && self.identity.orchestration.is_none() {
            for bar in &mut stage.seats {
                bar[4].on = true;
                bar[4].role = ArrangementRole::Pulse;
                bar[4].gain = ArrangementRole::Pulse.gain();
            }
        }
    }
    /// Exact pitches constrain the domain; register, gate and dynamics are target draws.
    pub(crate) fn source_notes(&self, perf: &PerformancePlan, role: Role) -> Option<Vec<Note>> {
        let line = self.identity.line(role)?;
        let mut rng = super::rng::Rng::new(
            self.seed
                ^ if role == Role::Lead {
                    0xC0FE_1EAD
                } else {
                    0xC0FE_BA55
                },
        );
        let preferred =
            if role == Role::Lead { 60 } else { 36 } + 12 * ((self.seed % 3) as i32 - 1);
        let base = (0..=10)
            .map(|o| o * 12)
            .filter(|base| {
                line.notes
                    .iter()
                    .all(|n| (0..128).contains(&(self.tonic + base + n.relative_pitch)))
            })
            .min_by_key(|base| (base - preferred).abs())
            .expect("cover validation checks realizable register");
        let mut notes = Vec::new();
        for (i, n) in line.notes.iter().enumerate() {
            let at = self.transport.transport(n.at).beats();
            let next = line
                .notes
                .get(i + 1)
                .map_or(self.identity.metric_length().beats(), |x| {
                    self.transport.transport(x.at).beats()
                });
            let end = n
                .reserved_until
                .map_or(next, |e| self.transport.transport(e).beats().min(next));
            let slot = end - at;
            let gate = (slot * f64::from(rng.range_f32(0.58, 0.9))).min(3.0) as f32;
            let statement = perf
                .statements
                .iter()
                .find(|s| n.at.beats() >= s.start_beat && n.at.beats() < s.end_beat());
            let mut prov = Provenance {
                role_note: "cover-identity",
                anchor: Some("cover"),
                ..Provenance::new(super::form::SectionKind::A)
            };
            if role == Role::Lead {
                if let Some(s) = statement {
                    prov.material = Some(s.material);
                    prov.motif_id = Some(s.motif.id);
                    prov = prov.realizing_opt(s.call);
                    prov.interaction = s.call.and_then(|id| perf.interaction_of(id));
                }
            }
            for action in perf
                .actions
                .of_kind(super::action::ActionKind::Resolve)
                .filter(|a| a.initiator == Agent::Lead && a.target_beat == Some(at))
            {
                if role == Role::Lead {
                    prov = prov.realizing(action.id);
                }
            }
            notes.push(Note::new(
                at,
                gate,
                self.tonic + base + n.relative_pitch,
                rng.range_f32(0.58, 0.86),
                role,
                prov,
            ));
        }
        super::comp::release_at_harmony_change(&mut notes, &perf.chords);
        bound_source_gates(perf, &mut notes);
        classify_source(perf, &mut notes);
        Some(notes)
    }
    pub(crate) fn drums(&self, perf: &PerformancePlan, world: &MusicWorld) -> Option<Vec<DrumHit>> {
        let strokes = self.identity.groove.as_ref()?;
        let mut rng = super::rng::Rng::new(self.seed ^ 0xC0FE_DA05);
        let mut hits = Vec::new();
        for s in strokes {
            let at = self.transport.transport(s.at).beats();
            if !perf.on_stage(Agent::Drums, at) {
                continue;
            }
            hits.push(DrumHit {
                start_beat: at,
                voice: match s.voice {
                    GrooveVoice::Kick => DrumVoice::Kick,
                    GrooveVoice::Snare => DrumVoice::Snare,
                },
                velocity: rng.range_f32(0.65, 0.95) * world.base_dynamic,
                prov: Provenance {
                    groove_variation: Some("cover-cell"),
                    ..Provenance::new(super::form::SectionKind::A)
                },
            });
        }
        let subdivision = perf
            .language
            .surface_subdivision
            .max(world.subdiv)
            .clamp(1, 4);
        for tick in
            0..(self.identity.metric_length().beats() * f64::from(subdivision)).ceil() as i64
        {
            let m = MetricPosition::new(tick, subdivision).unwrap();
            let at = self.transport.transport(m).beats();
            if at >= self.identity.metric_length().beats() || !perf.on_stage(Agent::Drums, at) {
                continue;
            }
            hits.push(DrumHit {
                start_beat: at,
                voice: if tick % 8 == 7 && self.seed % 2 == 1 {
                    DrumVoice::OpenHat
                } else {
                    DrumVoice::ClosedHat
                },
                velocity: rng.range_f32(0.25, 0.55) * world.base_dynamic,
                prov: Provenance::new(super::form::SectionKind::A),
            });
        }
        hits.sort_by(|a, b| a.start_beat.total_cmp(&b.start_beat));
        Some(hits)
    }
}

fn chord_admitted(
    chord: Chord,
    world: &MusicWorld,
    language: &super::language::MusicalLanguage,
) -> bool {
    let scale = super::theory::Scale::new(world.tonic_pc, world.mode);
    let extension = matches!(
        chord.quality,
        Quality::Maj9
            | Quality::Min9
            | Quality::Dom9
            | Quality::Add9
            | Quality::Maj6
            | Quality::Min6
    );
    (!extension || language.color_depth > 0)
        && (world.use_sevenths || chord.quality.intervals().len() <= 3)
        && (world.allow_modal_mixture
            || chord
                .quality
                .intervals()
                .iter()
                .all(|offset| scale.contains_pc(chord.root_pc + offset)))
}

fn classify_source(perf: &PerformancePlan, notes: &mut [Note]) {
    let snapshot = notes.to_vec();
    for (i, n) in notes.iter_mut().enumerate() {
        let ctx = perf.context_at(n.start_beat);
        let boundary = ctx.map(|c| c.start_beat + f64::from(c.dur_beats));
        let next = boundary.and_then(|b| perf.context_at(b));
        let previous = ctx.and_then(|c| {
            perf.contexts
                .iter()
                .rev()
                .find(|p| p.start_beat < c.start_beat)
        });
        n.function = super::pitch::classify(
            &super::pitch::PitchContext {
                pitch: n.pitch,
                onset: n.start_beat,
                duration: f64::from(n.dur_beats),
                prev: i.checked_sub(1).map(|j| snapshot[j].pitch),
                next: snapshot.get(i + 1).map(|x| x.pitch),
                next_onset: snapshot.get(i + 1).map(|x| x.start_beat),
                prev_chord: previous.map(|c| c.chord),
                cur: ctx.map(|c| c.chord),
                next_chord: next.map(|c| c.chord),
                next_boundary: boundary,
                is_strong: n.start_beat.fract() == 0.0,
                licensed: ctx.map_or(0, |c| super::pitch::pc_mask(&c.palette.tensions)),
            },
            &perf.region_at(n.start_beat),
        );
    }
}

/// All free generation inputs. The source composition cannot cross this boundary.
pub struct CoverTarget<'a> {
    pub world: &'a MusicWorld,
    pub seed: u64,
    pub grammar: CompositionGrammar,
    pub options: PerformanceOptions,
    pub profile: super::policy::PerformanceProfile,
}

/// Checked lift: the cover quotient and enumerated `CoverPipelineReceipt` laws must pass.
/// Use [`cover_candidate`] to retain a rejected candidate for diagnosis and listening.
pub fn cover(map: &CoverMap, target: CoverTarget<'_>) -> Result<Composition, CoverError> {
    let world = target.world;
    let candidate = cover_candidate(map, target)?;
    let conformance = CoverConformance::check(map, &candidate, world);
    let pipeline = CoverPipelineReceipt::measure(&candidate, world);
    if !conformance.passes() || !pipeline.passes() {
        return Err(CoverError::Rejected(Box::new(CoverAdmission {
            conformance,
            pipeline,
        })));
    }
    Ok(candidate)
}

/// Explicit admission evidence; quotient equality alone is not a lawful performance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverAdmission {
    pub conformance: CoverConformance,
    pub pipeline: CoverPipelineReceipt,
}

/// Lift selected invariants into a newly planned band. No completed Score is patched.
pub fn cover_candidate(map: &CoverMap, target: CoverTarget<'_>) -> Result<Composition, CoverError> {
    map.validate()?;
    if !target.world.tempo_bpm.is_finite() || target.world.tempo_bpm <= 0.0 {
        return Err(CoverError::Invalid("target tempo"));
    }
    if map.length.is_none() {
        return Err(CoverError::Invalid(
            "ordered chart needs an explicit target metric schedule; use cover_skeleton",
        ));
    }
    let trace = super::semantic::SemanticTrace::new(Vec::new(), map.metric_length().beats());
    let mut song = SongMap::build(&trace, target.seed, Some(target.grammar));
    if let Some(form) = &map.form {
        let phrases = form
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let span = song.timeline.span(
                    f64::from(p.start_bar) * 4.0,
                    (f64::from(p.start_bar + p.bars) * 4.0).min(map.metric_length().beats()),
                );
                Phrase {
                    ix: i as u32,
                    start_bar: p.start_bar,
                    bars: p.bars,
                    family: p.family,
                    intent: span.start,
                    span,
                    is_rupture: matches!(p.family, SectionFamily::Climax),
                    piece_end: map.metric_length().beats(),
                }
            })
            .collect();
        let graph = FormGraph {
            phrases,
            total_bars: super::form::bars_spanning(map.metric_length().beats(), 4.0),
            total_beats: map.metric_length().beats(),
        };
        song.plan = super::plan::CompositionPlan::from_form(
            &song.timeline,
            graph,
            CoherenceContract::for_grammar(target.grammar),
        );
    }
    // The new song has no retained source page. Its themes are target-generated when free.
    // With an external chart, the generic chart in a generated backbone is not a second authority.
    if map.harmony.is_some() {
        song.plan.backbone = None;
        song.harmonic = None;
    }
    let constraints = CoverConstraints {
        identity: map.clone(),
        transport: GrooveTransport::eighth_swing(target.world.swing)
            .ok_or(CoverError::Invalid("swing"))?,
        seed: target.seed,
        tonic: target.world.tonic_pc,
        occupancy_policy: target.profile.occupancy,
    };
    if map.line(Role::Lead).is_some() {
        let mut statements = Vec::new();
        constraints.plan_statements(&song.plan, &mut statements, &mut Vec::new());
        song.thematic.sites = statements
            .iter()
            .map(|s| super::song::ThemeSite {
                phrase: s.phrase,
                role: s.role,
                motif: s.motif.clone(),
                handoff: s.handoff,
            })
            .collect();
        if let Some(first) = statements.first() {
            song.thematic.bank.identity = first.motif.clone();
            song.thematic.bank.hook = first.motif.clone();
        }
    }
    constraints.validate_target()?;
    let perf =
        PerformancePlan::from_song_constrained(&song, target.world, target.options, constraints)?;
    let score = super::functor::realize_with_profile(&song, target.world, &perf, target.profile)
        .map_err(|e| CoverError::Policy(e.to_string()))?;
    Ok(Composition { score, song, perf })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverCheck {
    pub axis: CoverAxis,
    pub relation: &'static str,
    pub passed: bool,
    pub detail: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverConformance {
    pub length_passed: bool,
    pub checks: Vec<CoverCheck>,
}
impl CoverConformance {
    pub fn check(expected: &CoverMap, actual: &Composition, world: &MusicWorld) -> Self {
        if let Some(chart) = &expected.ordered_chart {
            return chart.check(actual, world);
        }
        let length_passed = expected
            .length
            .is_none_or(|length| metric(actual.score.total_beats).is_ok_and(|v| v == length));
        let checks = expected
            .spec
            .axes
            .iter()
            .copied()
            .map(|axis| {
                let projected = CoverMap::extract(actual, world, CoverSpec::new([axis]));
                let passed = projected.as_ref().is_ok_and(|p| match axis {
                    CoverAxis::Motif => p.motif == expected.motif,
                    CoverAxis::Riff => p.riff == expected.riff,
                    CoverAxis::BassFigure => p.bass == expected.bass,
                    CoverAxis::Groove => p.groove == expected.groove,
                    CoverAxis::HarmonicContour | CoverAxis::HarmonicLoop => {
                        p.harmony == expected.harmony
                    }
                    CoverAxis::Form => p.form == expected.form,
                    CoverAxis::Orchestration => p.orchestration == expected.orchestration,
                });
                CoverCheck {
                    axis,
                    relation: axis.relation(),
                    passed,
                    detail: if passed {
                        "exact projection agrees".into()
                    } else {
                        projected.as_ref().err().map_or_else(
                            || "projected coordinate differs".into(),
                            ToString::to_string,
                        )
                    },
                }
            })
            .collect();
        Self {
            length_passed,
            checks,
        }
    }
    pub fn passes(&self) -> bool {
        self.length_passed && self.checks.iter().all(|c| c.passed)
    }
    pub fn report(&self) -> String {
        let mut text = format!(
            "CoverConformance: {} (metric length {})",
            if self.passes() { "PASS" } else { "FAIL" },
            self.length_passed
        );
        for c in &self.checks {
            text.push_str(&format!(
                "\n  {}: {} — {} ({})",
                c.axis.label(),
                if c.passed { "PASS" } else { "FAIL" },
                c.relation,
                c.detail
            ));
        }
        text
    }
}

/// A decomposed freedom witness. A different PCM hash alone could be only a patch swap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverFreedom {
    pub support_voicing: bool,
    pub dynamics: bool,
    pub percussion_detail: bool,
    pub interactions: bool,
}
impl CoverFreedom {
    pub fn compare(a: &Composition, b: &Composition) -> Self {
        let support = |c: &Composition| {
            c.score
                .notes
                .iter()
                .filter(|n| matches!(n.role, Role::Keys | Role::Pad))
                .map(|n| (n.role, n.start_beat.to_bits(), n.pitch))
                .collect::<Vec<_>>()
        };
        let dynamics = |c: &Composition| {
            c.score
                .notes
                .iter()
                .map(|n| (n.velocity.to_bits(), n.dur_beats.to_bits()))
                .collect::<Vec<_>>()
        };
        let drums = |c: &Composition| {
            c.score
                .drums
                .iter()
                .filter(|d| {
                    matches!(
                        d.voice,
                        DrumVoice::ClosedHat | DrumVoice::OpenHat | DrumVoice::Clap
                    )
                })
                .map(|d| (d.start_beat.to_bits(), d.velocity.to_bits()))
                .collect::<Vec<_>>()
        };
        Self {
            support_voicing: support(a) != support(b),
            dynamics: dynamics(a) != dynamics(b),
            percussion_detail: drums(a) != drums(b),
            interactions: a.perf.interactions.canonical_fingerprint()
                != b.perf.interactions.canonical_fingerprint(),
        }
    }
    pub fn has_freedom(&self) -> bool {
        self.support_voicing || self.dynamics || self.percussion_detail || self.interactions
    }
}

macro_rules! encode_fields {($ty:ty,$tag:literal,$($field:ident),+)=>{impl CanonicalFingerprint for $ty {fn encode(&self,w:&mut FingerprintWriter){let Self { $($field,)+ } = self; w.tag($tag);$(w.field(stringify!($field),$field);)+}}};}
encode_fields!(
    CoverNote,
    "CoverNote/v1",
    at,
    relative_pitch,
    reserved_until
);
encode_fields!(CoverLine, "CoverLine/v1", role, notes);
encode_fields!(CoverChord, "CoverChord/v1", at, end, relative_root, quality);
encode_fields!(CoverPhrase, "CoverPhrase/v1", start_bar, bars, family);
impl CanonicalFingerprint for GrooveVoice {
    fn encode(&self, w: &mut FingerprintWriter) {
        w.tag(match self {
            Self::Kick => "kick",
            Self::Snare => "snare",
        });
    }
}
encode_fields!(CoverStroke, "CoverStroke/v1", at, voice);
encode_fields!(CoverSeats, "CoverSeats/v1", roles);
encode_fields!(
    CoverMap,
    "CoverMap/v1",
    spec,
    unknown_axes,
    ordered_chart,
    length,
    form,
    motif,
    riff,
    harmony,
    groove,
    bass,
    orchestration
);
impl CanonicalFingerprint for CoverConstraints {
    fn encode(&self, w: &mut FingerprintWriter) {
        let Self {
            identity,
            transport,
            seed,
            tonic,
            occupancy_policy,
        } = self;
        w.tag("CoverConstraints/v1");
        w.field("identity", identity);
        w.field("swing", &transport.swing());
        w.field("seed", seed);
        w.field("tonic", tonic);
        w.field("occupancy_policy", occupancy_policy);
    }
}

encode_fields!(OrderedChord, "OrderedChord/v1", relative_root, quality);
encode_fields!(OrderedSection, "OrderedSection/v1", family, chords);
encode_fields!(OrderedChart, "OrderedChart/v1", sections);
impl OrderedChart {
    fn check(&self, actual: &Composition, world: &MusicWorld) -> CoverConformance {
        let harmonic = self.sections.iter().enumerate().all(|(i, s)| {
            let Some(expected) = &s.chords else {
                return true;
            };
            let Some(phrase) = actual.song.plan.form.phrases.get(i) else {
                return false;
            };
            let heard: Vec<_> = actual
                .score
                .chords
                .iter()
                .filter(|c| c.start_beat >= phrase.start_beat() && c.start_beat < phrase.end_beat())
                .map(|c| OrderedChord {
                    relative_root: (c.chord.root_pc - world.tonic_pc).rem_euclid(12),
                    quality: c.chord.quality,
                })
                .collect();
            *expected == heard
        });
        let mut names = Vec::<&str>::new();
        let expected_families: Vec<_> = self
            .sections
            .iter()
            .map(|s| {
                let id = if let Some(i) = names.iter().position(|n| *n == s.family) {
                    i
                } else {
                    names.push(&s.family);
                    names.len() - 1
                };
                SectionFamily::Named {
                    identity: id as u32,
                }
            })
            .collect();
        let shape = actual
            .song
            .plan
            .form
            .phrases
            .iter()
            .map(|p| p.family)
            .collect::<Vec<_>>()
            == expected_families
            && actual
                .score
                .sections
                .iter()
                .map(|s| (s.start_bar, s.bars, s.kind))
                .eq(actual
                    .song
                    .plan
                    .form
                    .phrases
                    .iter()
                    .map(|p| (p.start_bar, p.bars, p.family.to_section_kind())));
        CoverConformance {
            length_passed: true,
            checks: vec![
                CoverCheck {
                    axis: CoverAxis::HarmonicContour,
                    relation: "ordered relative chord sequence; durations unobserved",
                    passed: harmonic,
                    detail: "actual score chord order; source metric timing remains unknown".into(),
                },
                CoverCheck {
                    axis: CoverAxis::Form,
                    relation: "ordered section topology; lengths unobserved",
                    passed: shape,
                    detail: "target-generated section durations are not source observations".into(),
                },
            ],
        }
    }
}
impl CoverMap {
    pub fn from_ordered_chart(chart: OrderedChart) -> Result<Self, CoverError> {
        let out = Self {
            spec: CoverSpec::new([CoverAxis::Form, CoverAxis::HarmonicContour]),
            unknown_axes: CoverAxis::ALL
                .into_iter()
                .filter(|a| !matches!(a, CoverAxis::Form | CoverAxis::HarmonicContour))
                .collect(),
            ordered_chart: Some(chart),
            length: None,
            form: None,
            motif: None,
            riff: None,
            harmony: None,
            groove: None,
            bass: None,
            orchestration: None,
        };
        out.validate()?;
        Ok(out)
    }
}
/// A partial-cover skeleton. Exact source melody, bass, groove and timing remain
/// UNKNOWN; the schedule is an explicit target assumption recorded in the receipt.
pub fn cover_skeleton(
    map: &CoverMap,
    target: CoverTarget<'_>,
    schedule: SkeletonSchedule,
) -> Result<Composition, CoverError> {
    let chart = map
        .ordered_chart
        .as_ref()
        .ok_or(CoverError::Invalid("not an ordered chart"))?;
    if schedule.bars_per_chord == 0 {
        return Err(CoverError::Invalid("zero target bars per chord"));
    }
    let mut expanded = map.clone();
    expanded.ordered_chart = None;
    let mut harmony = Vec::new();
    let mut phrases = Vec::new();
    let mut bar = 0u32;
    let mut seen = Vec::<String>::new();
    for section in &chart.sections {
        let identity = if let Some(i) = seen.iter().position(|s| s == &section.family) {
            i as u32
        } else {
            seen.push(section.family.clone());
            (seen.len() - 1) as u32
        };
        let family = SectionFamily::Named { identity };
        let start = bar;
        let free_home = [OrderedChord {
            relative_root: 0,
            quality: Quality::Maj,
        }];
        let chords = section.chords.as_deref().unwrap_or(&free_home);
        if chords.is_empty() {
            return Err(CoverError::Invalid(
                "known harmonic silence is not yet supported by this skeleton planner",
            ));
        }
        for chord in chords {
            let at = MetricPosition::new(i64::from(bar) * 4, 1).unwrap();
            bar = bar
                .checked_add(schedule.bars_per_chord)
                .ok_or(CoverError::Invalid("target bar extent overflow"))?;
            harmony.push(CoverChord {
                at,
                end: MetricPosition::new(i64::from(bar) * 4, 1).unwrap(),
                relative_root: chord.relative_root,
                quality: chord.quality,
            });
        }
        phrases.push(CoverPhrase {
            start_bar: start,
            bars: bar - start,
            family,
        });
    }
    expanded.length = MetricPosition::new(i64::from(bar) * 4, 1);
    expanded.form = Some(phrases);
    expanded.harmony = Some(harmony);
    cover_candidate(&expanded, target)
}

#[cfg(test)]
#[path = "cover_tests.rs"]
mod tests;

/// Ordinary pipeline laws are assessed independently of cover identity. A passed
/// quotient never conceals a red pitch, hearing, identity, stage or action receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverPipelineReceipt {
    pub song: bool,
    pub score_validation: Option<String>,
    pub unclassified: usize,
    pub temporal_false: usize,
    pub held_identity_flips: usize,
    pub stale_hearings: usize,
    pub invalid_continuations: usize,
    pub occupancy_violations: Vec<String>,
    pub unwitnessed_actions: usize,
    pub stage_violations: usize,
}
impl CoverPipelineReceipt {
    pub fn measure(c: &Composition, world: &MusicWorld) -> Self {
        let actions = super::witness::audit(&c.perf, &c.score);
        Self {
            song: super::song::SongMapConformance::check(&c.song, &c.perf, &c.score).passes(),
            score_validation: c.score.validate().err(),
            unclassified: c
                .score
                .notes
                .iter()
                .filter(|n| n.function.is_none())
                .count(),
            temporal_false: super::temporal::TemporalPitchDiagnostics::measure(&c.perf, &c.score)
                .false_function_claims,
            held_identity_flips: super::identity::IdentityDiagnostics::measure_score(
                &c.score,
                &c.perf.contexts,
                world,
            )
            .flips()
            .count(),
            stale_hearings: c.score.stale_hearings().len(),
            occupancy_violations: super::occupancy::violations(
                &c.perf,
                &c.score,
                c.perf.cover_constraints.as_ref().is_some_and(|c| {
                    c.occupancy_policy == super::policy::OccupancyPolicy::AuthoredIntent
                }),
            ),
            invalid_continuations: super::voice::continuity_violations(
                &c.score.notes,
                &c.score.voice_continuity,
            )
            .len(),
            unwitnessed_actions: actions.rows.iter().filter(|r| !r.witnessed).count(),
            stage_violations: super::functor::orchestration_violations(&c.perf, &c.score).len(),
        }
    }
    pub fn passes(&self) -> bool {
        self.song
            && self.score_validation.is_none()
            && self.unclassified == 0
            && self.temporal_false == 0
            && self.held_identity_flips == 0
            && self.stale_hearings == 0
            && self.invalid_continuations == 0
            && self.occupancy_violations.is_empty()
            && self.unwitnessed_actions == 0
            && self.stage_violations == 0
    }
}
impl OrderedChart {
    /// Generic ordered chart: `section LABEL ROOT:QUALITY ...`. ROOT is a
    /// chromatic offset above the declared reference tonic; `?` means unobserved.
    pub fn from_tsv(text: &str) -> Result<Self, CoverError> {
        let mut sections = Vec::new();
        for line in text
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        {
            let fields: Vec<_> = line.split_whitespace().collect();
            let ["section", family, rest @ ..] = fields.as_slice() else {
                return Err(CoverError::Invalid("ordered section record"));
            };
            let chords = if rest == ["?"] {
                None
            } else {
                let mut chords = Vec::new();
                for token in rest {
                    let (root, q) = token
                        .split_once(':')
                        .ok_or(CoverError::Invalid("ordered chord token"))?;
                    let relative_root: i32 = root
                        .parse()
                        .map_err(|_| CoverError::Invalid("ordered chord root"))?;
                    if !(0..12).contains(&relative_root) {
                        return Err(CoverError::Invalid("ordered root range"));
                    }
                    let quality = match q {
                        "maj" => Quality::Maj,
                        "min" => Quality::Min,
                        "dim" => Quality::Dim,
                        "7" => Quality::Dom7,
                        "maj7" => Quality::Maj7,
                        "min7" => Quality::Min7,
                        _ => return Err(CoverError::Invalid("unsupported ordered chord quality")),
                    };
                    chords.push(OrderedChord {
                        relative_root,
                        quality,
                    });
                }
                if chords.is_empty() {
                    return Err(CoverError::Invalid("empty observed chord order"));
                }
                Some(chords)
            };
            sections.push(OrderedSection {
                family: (*family).into(),
                chords,
            });
        }
        if sections.is_empty() {
            return Err(CoverError::Invalid("empty chart"));
        }
        Ok(Self { sections })
    }
}

/// A constrained source without an explicit carry obligation releases at the first
/// foreign harmony. This is a source law, before expression and downstream hearing;
/// the historical half-beat overhang is not an identity-preserving harmonic license.
pub(crate) fn bound_source_gates(perf: &PerformancePlan, notes: &mut [Note]) {
    for note in notes {
        if matches!(
            note.function,
            Some(
                super::score::PitchFunction::PedalTone
                    | super::score::PitchFunction::Suspension
                    | super::score::PitchFunction::Retardation
                    | super::score::PitchFunction::Anticipation
            )
        ) {
            continue;
        }
        if let Some(next) = perf.chords.iter().find(|c| {
            c.start_beat > note.start_beat
                && c.start_beat < note.start_beat + f64::from(note.dur_beats)
                && !c.chord.contains_pc(note.pitch.rem_euclid(12))
        }) {
            note.dur_beats = (next.start_beat - note.start_beat) as f32;
        }
    }
}
