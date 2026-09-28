//! The semantic category **𝒮**: objects are LibGibson presentation/story states, morphisms
//! are *meaningful* transitions between them (never frame ticks). Identity is
//! **prolongation** — a settled UI must not generate endless musical novelty just because
//! time passed.
//!
//! These enums mirror LibGibson's visual semantic axes (`gibson::ui::Tone`/`Emphasis`/…);
//! they are defined locally so the audio axis stays self-contained and independently
//! testable. A host maps its real `Style`/story state onto a [`SemanticState`] and feeds a
//! [`SemanticTrace`] to the HumanMusic functor.

/// Semantic tone — the qualitative "color" of a state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Accent,
    Info,
    Success,
    Warning,
    Danger,
}

/// Dynamic/articulation weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Emphasis {
    Faint,
    Muted,
    Normal,
    Strong,
}

/// Event/harmonic-rhythm density.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Density {
    Compact,
    Normal,
    Spacious,
}

/// Register/space/depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elevation {
    Flat,
    Raised,
    Overlay,
}

/// An object of 𝒮: a full semantic state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticState {
    pub tone: Tone,
    pub emphasis: Emphasis,
    pub density: Density,
    pub elevation: Elevation,
}

impl Default for SemanticState {
    fn default() -> Self {
        SemanticState {
            tone: Tone::Neutral,
            emphasis: Emphasis::Normal,
            density: Density::Normal,
            elevation: Elevation::Flat,
        }
    }
}

impl SemanticState {
    /// A state with just the tone changed from neutral defaults.
    pub fn toned(tone: Tone) -> SemanticState {
        SemanticState {
            tone,
            ..SemanticState::default()
        }
    }

    /// Semantic *pressure* in `[0,1]` — how much this state wants harmonic/rhythmic
    /// instability. Danger pulls hardest, Success releases, Neutral is at rest. Emphasis
    /// and a Compact density push it up; Spacious relaxes it.
    pub fn pressure(&self) -> f32 {
        let tone: f32 = match self.tone {
            Tone::Neutral => 0.20,
            Tone::Success => 0.10,
            Tone::Info => 0.35,
            Tone::Accent => 0.45,
            Tone::Warning => 0.70,
            Tone::Danger => 0.92,
        };
        let emph: f32 = match self.emphasis {
            Emphasis::Faint => -0.10,
            Emphasis::Muted => -0.05,
            Emphasis::Normal => 0.0,
            Emphasis::Strong => 0.12,
        };
        let dens: f32 = match self.density {
            Density::Compact => 0.08,
            Density::Normal => 0.0,
            Density::Spacious => -0.08,
        };
        (tone + emph + dens).clamp(0.0, 1.0)
    }

    /// Dynamic weight in `[0,1]` from emphasis (drives overall loudness/orchestration).
    pub fn dynamic(&self) -> f32 {
        match self.emphasis {
            Emphasis::Faint => 0.35,
            Emphasis::Muted => 0.55,
            Emphasis::Normal => 0.75,
            Emphasis::Strong => 1.0,
        }
    }

    /// Register bias in `[0,1]` from elevation (higher = brighter/higher/more spatial).
    pub fn register_bias(&self) -> f32 {
        match self.elevation {
            Elevation::Flat => 0.4,
            Elevation::Raised => 0.6,
            Elevation::Overlay => 0.85,
        }
    }

    /// Whether `other` differs enough from `self` to warrant a musical event (the
    /// significance threshold that keeps every focus wiggle from becoming a ding).
    pub fn is_significant_change(&self, other: &SemanticState) -> bool {
        self.tone != other.tone
            || (self.pressure() - other.pressure()).abs() > 0.15
            || self.elevation != other.elevation
    }
}

/// The *kind* of a semantic transition — the morphism label in 𝒮.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// Identity morphism: nothing new; prolong the current music.
    Prolong,
    /// The tone shifted (general recoloring).
    ToneShift,
    /// A focus/selection was acquired (local foregrounding).
    FocusAcquired,
    /// A section/task resolved (cadential opportunity).
    SectionResolved,
    /// A cinematic act / major structural boundary changed.
    ActChanged,
    /// A modal/overlay was entered (suspension).
    ModalEntered,
    /// An impact / danger spike (rhythmic urgency).
    Impact,
    /// A success confirmation (release).
    Confirmation,
}

impl EventKind {
    /// Whether this event requires a musical response at all (identity does not).
    pub fn requires_event(self) -> bool {
        self != EventKind::Prolong
    }
}

/// A morphism instance in 𝒮: at `at_beat` the presentation becomes `state` via `kind`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SemanticEvent {
    pub at_beat: f64,
    pub state: SemanticState,
    pub kind: EventKind,
}

/// A time-ordered trace of semantic events over a span — the input to the functor.
#[derive(Debug, Clone)]
pub struct SemanticTrace {
    pub events: Vec<SemanticEvent>,
    pub total_beats: f64,
}

impl SemanticTrace {
    /// A trace, sorted by beat and genuinely bounded by `total_beats`.
    ///
    /// Events at or beyond `total_beats` (or before beat 0) are dropped, so a fixed fixture
    /// like [`demo_trace`] rendered at a *short* requested length can never silently push the
    /// generated form past the request. Round I only sorted here, which is why
    /// `demo_trace(48.0)` produced a 27-bar form instead of a 12-bar one.
    pub fn new(mut events: Vec<SemanticEvent>, total_beats: f64) -> SemanticTrace {
        events.retain(|e| e.at_beat >= 0.0 && e.at_beat < total_beats);
        events.sort_by(|a, b| a.at_beat.partial_cmp(&b.at_beat).unwrap());
        SemanticTrace {
            events,
            total_beats,
        }
    }

    /// The active state at `beat` (the most recent event's state, or default).
    pub fn state_at(&self, beat: f64) -> SemanticState {
        self.events
            .iter()
            .rev()
            .find(|e| e.at_beat <= beat)
            .map(|e| e.state)
            .unwrap_or_default()
    }
}

/// The canonical frozen fixture the lab renders through every world: a complete story arc
/// — settle, inform, foreground, tension, climax, resolution, coda. The *same* trace under
/// three worlds must stay recognizably one piece of music.
pub fn demo_trace(total_beats: f64) -> SemanticTrace {
    // Fully qualified: `Normal` exists in both `Emphasis` and `Density`, so a glob import
    // would be ambiguous.
    let st = |tone, emphasis, density, elevation| SemanticState {
        tone,
        emphasis,
        density,
        elevation,
    };
    SemanticTrace::new(
        vec![
            SemanticEvent {
                at_beat: 0.0,
                state: st(
                    Tone::Neutral,
                    Emphasis::Muted,
                    Density::Spacious,
                    Elevation::Flat,
                ),
                kind: EventKind::ActChanged,
            },
            SemanticEvent {
                at_beat: 16.0,
                state: st(
                    Tone::Info,
                    Emphasis::Normal,
                    Density::Normal,
                    Elevation::Raised,
                ),
                kind: EventKind::FocusAcquired,
            },
            SemanticEvent {
                at_beat: 32.0,
                state: st(
                    Tone::Accent,
                    Emphasis::Normal,
                    Density::Normal,
                    Elevation::Raised,
                ),
                kind: EventKind::ToneShift,
            },
            SemanticEvent {
                at_beat: 48.0,
                state: st(
                    Tone::Warning,
                    Emphasis::Strong,
                    Density::Compact,
                    Elevation::Raised,
                ),
                kind: EventKind::ModalEntered,
            },
            SemanticEvent {
                at_beat: 64.0,
                state: st(
                    Tone::Danger,
                    Emphasis::Strong,
                    Density::Compact,
                    Elevation::Overlay,
                ),
                kind: EventKind::Impact,
            },
            SemanticEvent {
                at_beat: 88.0,
                state: st(
                    Tone::Success,
                    Emphasis::Normal,
                    Density::Normal,
                    Elevation::Raised,
                ),
                kind: EventKind::Confirmation,
            },
            SemanticEvent {
                at_beat: 104.0,
                state: st(
                    Tone::Neutral,
                    Emphasis::Muted,
                    Density::Spacious,
                    Elevation::Flat,
                ),
                kind: EventKind::SectionResolved,
            },
        ],
        total_beats,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn danger_has_more_pressure_than_success() {
        let d = SemanticState::toned(Tone::Danger);
        let s = SemanticState::toned(Tone::Success);
        assert!(d.pressure() > s.pressure());
        assert!(d.pressure() > 0.8);
        assert!(s.pressure() < 0.2);
    }

    #[test]
    fn identity_prolong_requires_no_event() {
        assert!(!EventKind::Prolong.requires_event());
        assert!(EventKind::Impact.requires_event());
    }

    #[test]
    fn insignificant_change_is_filtered() {
        let a = SemanticState::default();
        let b = SemanticState {
            emphasis: Emphasis::Normal, // same tone, same pressure-ish
            ..a
        };
        assert!(!a.is_significant_change(&b));
        let c = SemanticState::toned(Tone::Danger);
        assert!(a.is_significant_change(&c));
    }

    #[test]
    fn trace_state_lookup_is_last_event_wins() {
        let t = demo_trace(120.0);
        assert_eq!(t.state_at(0.0).tone, Tone::Neutral);
        assert_eq!(t.state_at(70.0).tone, Tone::Danger);
        assert_eq!(t.state_at(1000.0).tone, Tone::Neutral); // coda holds
    }

    #[test]
    fn emphasis_and_elevation_map_monotonically() {
        assert!(
            SemanticState {
                emphasis: Emphasis::Strong,
                ..Default::default()
            }
            .dynamic()
                > SemanticState {
                    emphasis: Emphasis::Faint,
                    ..Default::default()
                }
                .dynamic()
        );
    }

    #[test]
    fn trace_is_genuinely_bounded_by_total_beats() {
        // A short budget must drop the fixed events that lie beyond it (the Round-I
        // form-overrun bug: demo_trace(48.0) used to keep events at 64/88/104).
        let short = demo_trace(48.0);
        assert!(
            short.events.iter().all(|e| e.at_beat < 48.0),
            "events past the requested budget were not dropped"
        );
        assert!(!short.events.is_empty(), "clamp removed everything");
        // The generous default keeps the whole arc intact.
        let full = demo_trace(120.0);
        assert_eq!(
            full.events.len(),
            7,
            "120-beat arc should keep all 7 events"
        );
    }
}
