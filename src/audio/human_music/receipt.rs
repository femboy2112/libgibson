//! **PerformanceReceipt** — the general laws every checked performance holds, whether or not it
//! is a cover. A cover is admitted only when this receipt AND its [`super::cover::CoverConformance`]
//! pass: `CoverAdmission = PerformanceReceipt + CoverConformance`.
//!
//! Each field is an exact law with its failures named (no weighted score): the song the
//! performance claims is the one it performs (theme sites, landmarks, truthful settlements); every
//! score domain is valid; every pitched note carries a function and no temporal function claim is
//! false; held pitch identities do not flip; every hearing is causal; every declared continuation
//! is lawful; the occupancy ledger is valid; every planned verb is performed; nobody sounds off
//! stage; every planned window inhabits the piece; every declared anchor is established; and —
//! when the profile makes it a source law — every chord is inside the world's harmonic vocabulary.

use super::functor::Composition;
use super::policy::{HarmonyPolicy, PerformanceProfile};
use super::world::MusicWorld;

/// The general performance laws, measured on one composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceReceipt {
    /// [`super::song::SongMapConformance`] passes.
    pub song: bool,
    pub score_validation: Option<String>,
    /// Pitched notes with no pitch function.
    pub unclassified: usize,
    pub temporal_false: usize,
    pub held_identity_flips: usize,
    pub stale_hearings: usize,
    pub invalid_continuations: usize,
    pub occupancy_violations: Vec<String>,
    /// Planned verbs no realized event witnesses.
    pub unwitnessed_actions: usize,
    /// Events sounding where the stage has their player out.
    pub stage_violations: usize,
    /// Planned windows outside `[0, total_beats]` ([`super::performance::PerformancePlan::domain_violations`]).
    pub domain_violations: Vec<String>,
    /// Declared anchors missing where the form had room, or stated with another identity
    /// ([`super::song::AnchorReport::violations`]).
    pub anchor_violations: Vec<String>,
    /// Chords outside the world's harmonic vocabulary — judged only when the profile makes the
    /// vocabulary a source law ([`HarmonyPolicy::Vocabulary`]); archived colours are characterized.
    pub vocabulary_violations: Vec<String>,
}

impl PerformanceReceipt {
    /// The laws every performance holds (no profile-declared law).
    pub fn measure(c: &Composition, world: &MusicWorld) -> Self {
        Self::measure_with(c, world, None)
    }

    /// The general laws plus those `profile` declares (the harmonic vocabulary under
    /// [`HarmonyPolicy::Vocabulary`]).
    pub fn measure_under(c: &Composition, world: &MusicWorld, profile: PerformanceProfile) -> Self {
        Self::measure_with(c, world, Some(profile))
    }

    fn measure_with(
        c: &Composition,
        world: &MusicWorld,
        profile: Option<PerformanceProfile>,
    ) -> Self {
        let actions = super::witness::audit(&c.perf, &c.score);
        let vocabulary_violations = match profile.map(|p| p.harmony) {
            Some(HarmonyPolicy::Vocabulary) => {
                let v = super::vocabulary::HarmonicVocabulary::of(world, &c.perf.language);
                c.score
                    .chords
                    .iter()
                    .filter(|s| !v.admits(s.chord))
                    .map(|s| format!("{:?} at {}", s.chord, s.start_beat))
                    .collect()
            }
            _ => Vec::new(),
        };
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
                }) || profile
                    .is_some_and(|p| p.occupancy == super::policy::OccupancyPolicy::AuthoredIntent),
            ),
            invalid_continuations: super::voice::continuity_violations(
                &c.score.notes,
                &c.score.voice_continuity,
            )
            .len(),
            unwitnessed_actions: actions.rows.iter().filter(|r| !r.witnessed).count(),
            stage_violations: super::functor::orchestration_violations(&c.perf, &c.score).len(),
            domain_violations: c.perf.domain_violations(),
            anchor_violations: super::song::AnchorReport::check(&c.song, &c.perf, &c.score)
                .violations()
                .into_iter()
                .map(|(a, why)| format!("{a:?}: {why}"))
                .collect(),
            vocabulary_violations,
        }
    }

    /// Whether every law holds.
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
            && self.domain_violations.is_empty()
            && self.anchor_violations.is_empty()
            && self.vocabulary_violations.is_empty()
    }

    /// The failing laws, named (empty when every law holds).
    pub fn failures(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut law = |ok: bool, what: String| {
            if !ok {
                out.push(what);
            }
        };
        law(self.song, "song conformance".into());
        law(
            self.score_validation.is_none(),
            format!("score domain: {:?}", self.score_validation),
        );
        law(
            self.unclassified == 0,
            format!("{} unclassified notes", self.unclassified),
        );
        law(
            self.temporal_false == 0,
            format!("{} false temporal function claims", self.temporal_false),
        );
        law(
            self.held_identity_flips == 0,
            format!("{} held-identity flips", self.held_identity_flips),
        );
        law(
            self.stale_hearings == 0,
            format!("{} stale hearings", self.stale_hearings),
        );
        law(
            self.invalid_continuations == 0,
            format!("{} invalid continuations", self.invalid_continuations),
        );
        law(
            self.occupancy_violations.is_empty(),
            format!("occupancy: {:?}", self.occupancy_violations),
        );
        law(
            self.unwitnessed_actions == 0,
            format!("{} unwitnessed verbs", self.unwitnessed_actions),
        );
        law(
            self.stage_violations == 0,
            format!("{} events off stage", self.stage_violations),
        );
        law(
            self.domain_violations.is_empty(),
            format!("domain: {:?}", self.domain_violations),
        );
        law(
            self.anchor_violations.is_empty(),
            format!("anchors: {:?}", self.anchor_violations),
        );
        law(
            self.vocabulary_violations.is_empty(),
            format!("vocabulary: {:?}", self.vocabulary_violations),
        );
        out
    }
}

/// Why a checked performance was not returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PerformanceRejection {
    /// The laws could not be combined, or the planner refused the song (a lawful, typed refusal:
    /// e.g. a chart chord outside the world's harmonic vocabulary).
    Refused(super::policy::PolicyError),
    /// The performance was made and violates a general law; the receipt names every failure.
    Rejected(Box<PerformanceReceipt>),
}

impl std::fmt::Display for PerformanceRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(e) => write!(f, "refused: {e}"),
            Self::Rejected(r) => write!(f, "rejected: {}", r.failures().join("; ")),
        }
    }
}

impl std::error::Error for PerformanceRejection {}
