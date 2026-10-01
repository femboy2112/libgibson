//! Musical realization laws, independent of the historical experiment names.
//!
//! A profile selects source generation, semantic ownership, support and lifetime separately.
//! [`PerformanceProfile::validate`] is the public boundary for combinations with final hearings.
//! Historical entry points retain their exact configurations through private adapters.

pub use super::percussion::{DrumRestraint, PercussionPolicy};
use super::performance::EnsembleCoupling;
use super::pocket::PocketOptions;
use super::voice::ObservedLifetimePolicy;

/// How the melodic search justifies pitch paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PitchPolicy {
    /// Frozen written/local classifier.
    Written,
    /// Time-aware source pitch paths.
    Temporal,
}

/// Whether candidate connective gestures are admitted using explicit continuation physics.
/// This is a source choice: changing it may change source notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuationAdmission {
    ReleaseEnvelope,
    ExplicitContinuation,
}

/// Source pulse selection factors. There is no support-top-voice switch: that treatment has
/// never been implemented. Archived factorial cells retain it only in [`PocketOptions`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PulsePolicy {
    pub lattice_positions: bool,
    pub legato_connectives: bool,
    pub continuation_admission: ContinuationAdmission,
    pub stable_precursors: bool,
}

impl PulsePolicy {
    /// The source law of the accepted pocket. Acceptance is specific to the BLACK_ICE flagship.
    pub const POCKET: Self = Self {
        lattice_positions: true,
        legato_connectives: true,
        continuation_admission: ContinuationAdmission::ExplicitContinuation,
        stable_precursors: false,
    };

    /// Whether this source policy differs from the phrase-expression control. Admission physics
    /// is included honestly because it can select different source candidates.
    pub fn changes_source(self) -> bool {
        self.lattice_positions
            || self.legato_connectives
            || self.continuation_admission == ContinuationAdmission::ExplicitContinuation
            || self.stable_precursors
    }

    pub(crate) fn from_legacy(options: PocketOptions) -> Self {
        Self {
            lattice_positions: options.lattice_positions,
            legato_connectives: options.legato_connectives,
            continuation_admission: if options.mono_voice {
                ContinuationAdmission::ExplicitContinuation
            } else {
                ContinuationAdmission::ReleaseEnvelope
            },
            stable_precursors: options.stable_precursors,
        }
    }

    /// The existing source planner's compatibility input. The renderer receives its own policy;
    /// it does not read this historical `mono_voice` spelling.
    pub(crate) fn source_options(self) -> PocketOptions {
        PocketOptions {
            lattice_positions: self.lattice_positions,
            legato_connectives: self.legato_connectives,
            mono_voice: self.continuation_admission == ContinuationAdmission::ExplicitContinuation,
            support_top_voice: false,
            stable_precursors: self.stable_precursors,
        }
    }
}

/// Source-expression strategy, before any dependent player hears the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpressionPolicy {
    Unchanged,
    LocalConnectives,
    Phrase,
    Pulse(PulsePolicy),
}

/// What downstream players interpret as an available place to speak.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OccupancyPolicy {
    Acoustic,
    AuthoredIntent,
}

/// How support chooses its own notes before committing them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportPolicy {
    Independent,
    HeardHarmony,
    SourceVoicePath,
}

/// Direct-voice rendering law. Changing this does not change source phrase admission.
/// Heard support may respond to the different lifetime, but lead/bass source plans do not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceLifetimePolicy {
    ReleaseEnvelope,
    ExplicitContinuations,
}

/// Evidence emitted by the source planner. Kept explicit for byte-exact historical adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceEvidencePolicy {
    Events,
    AuthoredSources,
}

/// How the performance plan admits its musical verbs before the take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionAdmission {
    /// Historical: every verb the planner and the stage admit goes into the take; a verb nobody
    /// performs stays in the plan as an unwitnessed promise.
    Planned,
    /// The band runs the chart once before the take. Each settled song obligation receives its
    /// discharging event from the source planner (the ordinary twin of the cover path's settlement
    /// planner), and a verb no player performed in the rehearsal is rejected — recorded, with its
    /// reason — before the take. Nothing is stamped that was not played.
    Rehearsed,
}

/// Which law governs the chords a performance sounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HarmonyPolicy {
    /// Historical: the backbone's colours and the harmonic edits as archived. They can sound a
    /// colour the world or language does not declare (a SWISS_SIGNAL `Maj6` Reset, a Simple-language
    /// `Min6` pedal); characterized, byte-exact (the accepted R17 arm).
    #[default]
    Archived,
    /// Every chord is admitted by the [`HarmonicVocabulary`](super::vocabulary::HarmonicVocabulary)
    /// its world and language declare. A room's own colour choice (a backbone colour, a recolour,
    /// an applied dominant, a modulation) is made inside the vocabulary or not at all; a chart chord
    /// the room cannot admit refuses the performance before anybody plays
    /// ([`VOCABULARY_REFUSAL`](super::vocabulary::VOCABULARY_REFUSAL)).
    Vocabulary,
}

/// Whether a support player's declared pitch functions must hold in the harmony they actually
/// sound against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FunctionPolicy {
    /// Historical: the archived bass and keys. A note's tail may ring up to half a beat into a
    /// harmony that excludes it, and the bass's bar-end chromatic approach aims at the next
    /// chord's root whether or not its own line sounds that root on the downbeat. Characterized,
    /// byte-exact (the accepted R17 arm).
    #[default]
    Archived,
    /// Every function the bass and keys declare is earned where it sounds: a note lifts off at a
    /// harmony change it does not belong to (neither player declares suspensions), and a bass
    /// approach is written only where its destination — the bass's own next root — sounds on the
    /// downbeat it approaches. A relational pitch function anywhere (the lead's appoggiatura,
    /// passing tone, neighbour…) is claimed only where its destination sounds inside the finite
    /// performance and holds the relation ([`super::pitch::classify_earned`]).
    Earned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HistoricalRepair {
    None,
    SupportMass,
    SoundingTension,
}

/// Orthogonal laws for realization. The default uses written sources with canonical observation;
/// the historical `perform` entry point separately retains its archived observation model.
/// Use [`Self::POCKET`] explicitly for the accepted flagship source/lifetime combination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerformanceProfile {
    pub pitch: PitchPolicy,
    pub expression: ExpressionPolicy,
    pub occupancy: OccupancyPolicy,
    pub support: SupportPolicy,
    pub lifetime: VoiceLifetimePolicy,
    /// Canonical direct-voice observation, or an explicitly requested archived masking model.
    /// Independent of whether the source actually declares any continuation edges.
    pub observation: ObservedLifetimePolicy,
    pub evidence: SourceEvidencePolicy,
    /// Planned (every historical profile) or rehearsed verb admission.
    pub admission: ActionAdmission,
    /// The historical unarbitrated drummer, or one arbitrated percussion surface.
    pub percussion: PercussionPolicy,
    /// The archived chord colours, or the world/language harmonic vocabulary as a source law.
    pub harmony: HarmonyPolicy,
    /// The archived support tolerances, or support functions earned where they sound.
    pub functions: FunctionPolicy,
    pub(crate) repair: HistoricalRepair,
}

impl Default for PerformanceProfile {
    fn default() -> Self {
        Self::WRITTEN
    }
}

impl PerformanceProfile {
    pub const WRITTEN: Self = Self {
        pitch: PitchPolicy::Written,
        expression: ExpressionPolicy::Unchanged,
        occupancy: OccupancyPolicy::Acoustic,
        support: SupportPolicy::Independent,
        lifetime: VoiceLifetimePolicy::ReleaseEnvelope,
        observation: ObservedLifetimePolicy::ExplicitContinuity,
        evidence: SourceEvidencePolicy::Events,
        admission: ActionAdmission::Planned,
        percussion: PercussionPolicy::Unarbitrated,
        harmony: HarmonyPolicy::Archived,
        functions: FunctionPolicy::Archived,
        repair: HistoricalRepair::None,
    };
    pub const TEMPORAL: Self = Self {
        pitch: PitchPolicy::Temporal,
        ..Self::WRITTEN
    };
    pub const HEARD: Self = Self {
        support: SupportPolicy::HeardHarmony,
        ..Self::TEMPORAL
    };
    pub const EXPRESSIVE: Self = Self {
        expression: ExpressionPolicy::LocalConnectives,
        ..Self::HEARD
    };
    pub const PHRASED: Self = Self {
        expression: ExpressionPolicy::Phrase,
        occupancy: OccupancyPolicy::AuthoredIntent,
        support: SupportPolicy::SourceVoicePath,
        evidence: SourceEvidencePolicy::AuthoredSources,
        ..Self::EXPRESSIVE
    };
    pub const POCKET: Self = Self {
        expression: ExpressionPolicy::Pulse(PulsePolicy::POCKET),
        lifetime: VoiceLifetimePolicy::ExplicitContinuations,
        ..Self::PHRASED
    };
    /// The hardened general profile: the accepted pocket's source laws, every verb it keeps
    /// actually performed ([`ActionAdmission::Rehearsed`]), a drummer who serves the pocket
    /// ([`DrumRestraint::Balanced`] on one arbitrated percussion surface), only the chords its
    /// world and language declare ([`HarmonyPolicy::Vocabulary`]), and support functions earned
    /// where they sound ([`FunctionPolicy::Earned`]). New work should start here;
    /// [`Self::POCKET`] stays the byte-exact accepted R17 arm.
    pub const BAND: Self = Self::POCKET
        .with_admission(ActionAdmission::Rehearsed)
        .with_drum_restraint(DrumRestraint::Balanced)
        .with_harmony(HarmonyPolicy::Vocabulary)
        .with_functions(FunctionPolicy::Earned);

    /// The same laws with another verb-admission law.
    pub const fn with_admission(self, admission: ActionAdmission) -> Self {
        Self { admission, ..self }
    }

    /// The same laws with another harmonic-vocabulary law.
    pub const fn with_harmony(self, harmony: HarmonyPolicy) -> Self {
        Self { harmony, ..self }
    }

    /// The same laws with another support-function law.
    pub const fn with_functions(self, functions: FunctionPolicy) -> Self {
        Self { functions, ..self }
    }

    /// The same laws with another percussion law.
    pub const fn with_percussion(self, percussion: PercussionPolicy) -> Self {
        Self { percussion, ..self }
    }

    /// The same laws with the drummer arbitrated at `restraint`.
    pub const fn with_drum_restraint(self, restraint: DrumRestraint) -> Self {
        self.with_percussion(PercussionPolicy::Arbitrated(restraint))
    }

    /// Validate once before planning/realizing a public profile. Historical post-hoc repair
    /// configurations are available only through their compatibility entry points.
    pub fn validate(self, coupling: EnsembleCoupling) -> Result<(), PolicyError> {
        if self.observation == ObservedLifetimePolicy::LegacyRoleMasking
            && self.lifetime == VoiceLifetimePolicy::ExplicitContinuations
        {
            return Err(PolicyError(
                "explicit continuation rendering requires explicit continuity observation",
            ));
        }
        if self.observation == ObservedLifetimePolicy::ExplicitContinuity
            && coupling != EnsembleCoupling::Independent
        {
            return Err(PolicyError(
                "canonical observation requires final-source independent coupling",
            ));
        }
        let expressed = self.expression != ExpressionPolicy::Unchanged;
        let phrase = matches!(
            self.expression,
            ExpressionPolicy::Phrase | ExpressionPolicy::Pulse(_)
        );
        if expressed && self.pitch != PitchPolicy::Temporal {
            return Err(PolicyError(
                "source expression requires temporal pitch paths",
            ));
        }
        if self.support != SupportPolicy::Independent && self.pitch != PitchPolicy::Temporal {
            return Err(PolicyError("heard support requires temporal pitch paths"));
        }
        if self.evidence == SourceEvidencePolicy::AuthoredSources && !expressed {
            return Err(PolicyError(
                "authored evidence requires a source-expression planner",
            ));
        }
        if expressed && self.support == SupportPolicy::Independent {
            return Err(PolicyError(
                "source expression requires support to hear final sources",
            ));
        }
        if (expressed || self.support != SupportPolicy::Independent)
            && coupling != EnsembleCoupling::Independent
        {
            return Err(PolicyError(
                "final-source hearings require independent coupling",
            ));
        }
        if self.occupancy == OccupancyPolicy::AuthoredIntent
            && (!expressed || self.evidence != SourceEvidencePolicy::AuthoredSources)
        {
            return Err(PolicyError(
                "authored occupancy requires authored source evidence",
            ));
        }
        if self.lifetime == VoiceLifetimePolicy::ExplicitContinuations && !phrase {
            return Err(PolicyError(
                "explicit voice continuation requires source phrase ownership",
            ));
        }
        if self.lifetime == VoiceLifetimePolicy::ExplicitContinuations
            && self.support != SupportPolicy::SourceVoicePath
        {
            return Err(PolicyError(
                "linked lifetime requires the source support observer",
            ));
        }
        if phrase && self.evidence != SourceEvidencePolicy::AuthoredSources {
            return Err(PolicyError(
                "phrase expression requires authored source evidence",
            ));
        }
        if self.support == SupportPolicy::SourceVoicePath
            && self.evidence != SourceEvidencePolicy::AuthoredSources
        {
            return Err(PolicyError(
                "source support paths require authored source evidence",
            ));
        }
        Ok(())
    }

    pub(crate) fn mass(self) -> bool {
        self.repair != HistoricalRepair::None
    }

    pub(crate) fn tension(self) -> bool {
        self.repair == HistoricalRepair::SoundingTension
    }

    pub(crate) fn legacy_pocket(options: PocketOptions) -> Self {
        Self {
            expression: ExpressionPolicy::Pulse(PulsePolicy::from_legacy(options)),
            // The archived factor intentionally coupled source admission and rendering.
            lifetime: if options.mono_voice {
                VoiceLifetimePolicy::ExplicitContinuations
            } else {
                VoiceLifetimePolicy::ReleaseEnvelope
            },
            ..Self::PHRASED
        }
    }
}

/// A rejected combination of musical laws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyError(pub &'static str);

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for PolicyError {}
