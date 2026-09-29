//! **MusicalLanguage** — *how the band speaks*, orthogonal to *what the song is*.
//!
//! Three axes stay separate:
//!
//! - the **composition** ([`super::plan::CompositionPlan`]: form, discourse, the DeflectedLift
//!   backbone timeline) is the song's identity;
//! - the **world** ([`super::world::MusicWorld`]) is timbre, production and local physics;
//! - the **language** (this module) is the performance idiom: a declared transform of the song's
//!   harmonic rhythm ([`HarmonicRhythm`]), colour depth, voicing and melodic-connective policy,
//!   rhythmic surface, how often and how freely the players interact, and how much simultaneous
//!   information the ensemble may spend.
//!
//! The same composition under [`MusicalLanguage::simple`] and [`MusicalLanguage::fusion_conversation`]
//! is the same song spoken two ways. No artist is encoded here — only generic mechanisms.

/// How an idiom treats the song's harmonic rhythm: a declared, typed transform of the chart's
/// canonical rhythm ([`super::song::HarmonicMap::bars_per_chord`]) — never a private number (Round
/// IX: `harmonic_rhythm_bars` used to move the chart's change points with no song coordinate
/// owning them). Both transforms preserve the chart's landmarks: every slot's anchor on its
/// downbeat and the pointer closing every Lift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarmonicRhythm {
    /// One chord per chart unit, as charted.
    AsCharted,
    /// Diminution: every chart unit split in two. The added changes are prolongations — an
    /// anchor's satellite or colour pedal, the reach's sequence step, the pointer's own applied
    /// dominant — and the pointer moves to the Lift's last (shorter) unit, still closing it.
    Diminished,
}

impl HarmonicRhythm {
    /// Bars per chord under this transform of a chart that changes every `chart_bars` bars.
    pub fn bars_per_chord(self, chart_bars: u32) -> u32 {
        match self {
            HarmonicRhythm::AsCharted => chart_bars.max(1),
            HarmonicRhythm::Diminished => (chart_bars / 2).max(1),
        }
    }

    /// A short lowercase label for dumps.
    pub fn label(self) -> &'static str {
        match self {
            HarmonicRhythm::AsCharted => "as-charted",
            HarmonicRhythm::Diminished => "diminished",
        }
    }
}

/// Which language profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageId {
    /// Plain statement: the chart's own harmonic rhythm, triads/7ths, on-beat support, rare
    /// interaction.
    Simple,
    /// High-sophistication conversational fusion: diminished harmonic rhythm, extended colour, shell
    /// voicings, syncopated surface, distributed initiative, unison figures, a tight complexity
    /// budget so density is coordinated rather than piled up.
    FusionConversation,
}

impl LanguageId {
    /// A short lowercase label for dumps.
    pub fn label(self) -> &'static str {
        match self {
            LanguageId::Simple => "simple",
            LanguageId::FusionConversation => "fusion",
        }
    }
}

/// A complete language profile. Every field has one concrete consumer in the performance planner
/// or a realizer; none is decorative.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MusicalLanguage {
    pub id: LanguageId,
    /// The declared transform of the song's harmonic rhythm inside a backbone gesture slot.
    pub harmonic_rhythm: HarmonicRhythm,
    /// Harmonic colour depth: 0 = triads/7ths only, 1 = add 9ths/6ths on open windows, 2 = full
    /// extended colour on every stable harmony.
    pub color_depth: u8,
    /// Keys/pad voice with 2–3 note upper-structure shells rather than full close voicings.
    pub shell_voicings: bool,
    /// Rhythmic surface subdivision the accent grid emphasizes (2 = eighths, 4 = sixteenths).
    pub surface_subdivision: u32,
    /// Weight on syncopation opportunities in the accent grid, `[0, 1]`.
    pub syncopation: f32,
    /// How readily a finished call opens a response window, `[0, 1]`.
    pub interaction: f32,
    /// Whether responders other than keys (bass, drums) and non-lead initiators are allowed.
    pub distributed_agency: bool,
    /// Whether planned ensemble unison figures/hits are part of the vocabulary.
    pub unison_figures: bool,
    /// Simultaneous-information budget per beat window (sum of capped per-role onset counts).
    pub complexity_budget: f32,
    /// How much the melodic connective line may use chromatic approach/enclosure, `[0, 1]`.
    pub chromatic_connectives: f32,
    /// Fraction of a statement the lead may leave as internal rest, `[0, 1]`.
    pub internal_rest: f32,
}

impl MusicalLanguage {
    /// The plain-statement language — the A/B control for "same song, simpler speech".
    pub fn simple() -> MusicalLanguage {
        MusicalLanguage {
            id: LanguageId::Simple,
            harmonic_rhythm: HarmonicRhythm::AsCharted,
            color_depth: 0,
            shell_voicings: false,
            surface_subdivision: 2,
            syncopation: 0.15,
            interaction: 0.25,
            distributed_agency: false,
            unison_figures: false,
            complexity_budget: 7.0,
            chromatic_connectives: 0.1,
            internal_rest: 0.1,
        }
    }

    /// The conversational-fusion language (the flagship idiom).
    pub fn fusion_conversation() -> MusicalLanguage {
        MusicalLanguage {
            id: LanguageId::FusionConversation,
            harmonic_rhythm: HarmonicRhythm::Diminished,
            color_depth: 2,
            shell_voicings: true,
            surface_subdivision: 4,
            syncopation: 0.6,
            interaction: 0.85,
            distributed_agency: true,
            unison_figures: true,
            complexity_budget: 9.0,
            chromatic_connectives: 0.55,
            internal_rest: 0.25,
        }
    }

    /// The profile for an id.
    pub fn from_id(id: LanguageId) -> MusicalLanguage {
        match id {
            LanguageId::Simple => MusicalLanguage::simple(),
            LanguageId::FusionConversation => MusicalLanguage::fusion_conversation(),
        }
    }
}

impl Default for MusicalLanguage {
    fn default() -> Self {
        MusicalLanguage::fusion_conversation()
    }
}
