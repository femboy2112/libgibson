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

    /// Whether this event marks a large-scale **structural** boundary that phrase segmentation
    /// should try to align to, so it is never swallowed mid-phrase (Round III). Local recolorings
    /// (`ToneShift`, `FocusAcquired`) and the identity (`Prolong`) do not; the act/section/impact/
    /// resolution family does.
    pub fn is_salient(self) -> bool {
        matches!(
            self,
            EventKind::ActChanged
                | EventKind::ModalEntered
                | EventKind::Impact
                | EventKind::Confirmation
                | EventKind::SectionResolved
        )
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

/// Build a `SemanticEvent` compactly (used by the synthetic fixtures below).
fn ev(
    at_beat: f64,
    tone: Tone,
    emphasis: Emphasis,
    density: Density,
    elevation: Elevation,
    kind: EventKind,
) -> SemanticEvent {
    SemanticEvent {
        at_beat,
        state: SemanticState {
            tone,
            emphasis,
            density,
            elevation,
        },
        kind,
    }
}

/// Synthetic fixture — **rise then remain unresolved**: the trace builds to a danger impact and
/// never releases. A directed planner should represent this honestly (no answer; the culmination's
/// debt left open), not manufacture a resolution. Guards against overfitting the resolved demo.
pub fn rise_unresolved(total_beats: f64) -> SemanticTrace {
    let t = total_beats;
    SemanticTrace::new(
        vec![
            ev(
                0.0,
                Tone::Neutral,
                Emphasis::Muted,
                Density::Spacious,
                Elevation::Flat,
                EventKind::ActChanged,
            ),
            ev(
                t * 0.25,
                Tone::Info,
                Emphasis::Normal,
                Density::Normal,
                Elevation::Raised,
                EventKind::FocusAcquired,
            ),
            ev(
                t * 0.5,
                Tone::Warning,
                Emphasis::Strong,
                Density::Compact,
                Elevation::Raised,
                EventKind::ModalEntered,
            ),
            ev(
                t * 0.78,
                Tone::Danger,
                Emphasis::Strong,
                Density::Compact,
                Elevation::Overlay,
                EventKind::Impact,
            ),
        ],
        total_beats,
    )
}

/// Synthetic fixture — **false climax then true resolution**: an early impact that partly settles,
/// then a larger impact, then a real resolution. The culmination must land on the *true* (later,
/// bigger) peak, not the first one to arrive.
pub fn false_climax(total_beats: f64) -> SemanticTrace {
    let t = total_beats;
    SemanticTrace::new(
        vec![
            ev(
                0.0,
                Tone::Neutral,
                Emphasis::Muted,
                Density::Spacious,
                Elevation::Flat,
                EventKind::ActChanged,
            ),
            ev(
                t * 0.22,
                Tone::Warning,
                Emphasis::Strong,
                Density::Normal,
                Elevation::Raised,
                EventKind::Impact,
            ),
            ev(
                t * 0.42,
                Tone::Success,
                Emphasis::Normal,
                Density::Normal,
                Elevation::Raised,
                EventKind::Confirmation,
            ),
            ev(
                t * 0.64,
                Tone::Danger,
                Emphasis::Strong,
                Density::Compact,
                Elevation::Overlay,
                EventKind::Impact,
            ),
            ev(
                t * 0.9,
                Tone::Neutral,
                Emphasis::Muted,
                Density::Spacious,
                Elevation::Flat,
                EventKind::SectionResolved,
            ),
        ],
        total_beats,
    )
}

/// Synthetic fixture — **a calm loop with no dramatic arc**: low, steady tension throughout. There
/// is no meaningful build; a directed planner should not invent a climax with real pressure.
pub fn calm_loop(total_beats: f64) -> SemanticTrace {
    let t = total_beats;
    SemanticTrace::new(
        vec![
            ev(
                0.0,
                Tone::Neutral,
                Emphasis::Muted,
                Density::Normal,
                Elevation::Flat,
                EventKind::ActChanged,
            ),
            ev(
                t * 0.34,
                Tone::Info,
                Emphasis::Normal,
                Density::Normal,
                Elevation::Flat,
                EventKind::FocusAcquired,
            ),
            ev(
                t * 0.67,
                Tone::Info,
                Emphasis::Normal,
                Density::Normal,
                Elevation::Raised,
                EventKind::ToneShift,
            ),
        ],
        total_beats,
    )
}

/// Synthetic fixture — **the bittersweet bounce**: a recurring lift → deflect → open → reset
/// contour, stated twice. Unlike [`demo_trace`]'s single cinematic climb to a `Danger`/`Overlay`
/// peak, this trace *bounces*: each cycle reaches only a modest Accent/Warning crest and settles
/// back to warmth, so the story's tension rises into the grammar's Depart/Culminate phrases and
/// falls into its Answer/Return phrases instead of fighting a monotone arc. It deliberately never
/// reaches `Tone::Danger` or `Elevation::Overlay` — that catastrophic vocabulary is the cinematic
/// stress test's alone. This is the flagship story for the DeflectedLift grammar: a song that keeps
/// reaching, just missing, opening a warm window, and resetting to reach again.
pub fn deflected_lift_trace(total_beats: f64) -> SemanticTrace {
    let t = total_beats;
    SemanticTrace::new(
        vec![
            // Establish — settle at home, low pressure, ready to reach.
            ev(
                0.0,
                Tone::Neutral,
                Emphasis::Muted,
                Density::Normal,
                Elevation::Flat,
                EventKind::ActChanged,
            ),
            // Cycle 1 — lift (reach up), deflect (the crest that almost lands), open (warm
            // release), reset (settle home, ready to go again).
            ev(
                t * 0.133,
                Tone::Info,
                Emphasis::Normal,
                Density::Normal,
                Elevation::Raised,
                EventKind::FocusAcquired,
            ),
            ev(
                t * 0.267,
                Tone::Accent,
                Emphasis::Strong,
                Density::Compact,
                Elevation::Raised,
                EventKind::ModalEntered,
            ),
            ev(
                t * 0.40,
                Tone::Success,
                Emphasis::Normal,
                Density::Normal,
                Elevation::Raised,
                EventKind::Confirmation,
            ),
            ev(
                t * 0.533,
                Tone::Info,
                Emphasis::Muted,
                Density::Spacious,
                Elevation::Flat,
                EventKind::SectionResolved,
            ),
            // Cycle 2 — the same shape again, a touch higher at the crest (the second reach), then
            // a warm opening and a final rounded reset. Recurrence is the point.
            ev(
                t * 0.667,
                Tone::Accent,
                Emphasis::Normal,
                Density::Normal,
                Elevation::Raised,
                EventKind::FocusAcquired,
            ),
            ev(
                t * 0.733,
                Tone::Warning,
                Emphasis::Normal,
                Density::Normal,
                Elevation::Raised,
                EventKind::ModalEntered,
            ),
            ev(
                t * 0.833,
                Tone::Success,
                Emphasis::Normal,
                Density::Normal,
                Elevation::Raised,
                EventKind::Confirmation,
            ),
            ev(
                t * 0.933,
                Tone::Neutral,
                Emphasis::Muted,
                Density::Spacious,
                Elevation::Flat,
                EventKind::SectionResolved,
            ),
        ],
        total_beats,
    )
}

/// Synthetic fixture — **the impossible-demo flight**: a high-energy intro arc built to carry
/// "amazing, impossible-seeming demo" energy. It opens on an immediate HOOK (grab the ear at
/// once, no slow fade-in), pulls back for one BREATH (dynamic contrast), climbs through a rising
/// BUILD into a tension crest, slams a DROP (the impact payoff at peak pressure), then turns that
/// released energy upward through a REVEAL and a RE-LIFT into a sustained TRIUMPH — a clear global
/// shape (accumulate → peak → earned bright arrival) with the emphasis swinging the full
/// `Faint`↔`Strong` range, so the band answers a richer arc with more motion. Unlike
/// [`deflected_lift_trace`]'s bittersweet bounce it commits to an arrival; unlike [`demo_trace`]'s
/// single cinematic climb it hooks, breathes, drops and lands. The one stock trace that reaches
/// the catastrophic `Danger`/`Overlay` vocabulary on purpose — the drop is the point.
pub fn intro_demo(total_beats: f64) -> SemanticTrace {
    let t = total_beats;
    SemanticTrace::new(
        vec![
            // HOOK — grab the ear immediately: bright, loud, dense, up front.
            ev(
                0.0,
                Tone::Accent,
                Emphasis::Strong,
                Density::Compact,
                Elevation::Raised,
                EventKind::ActChanged,
            ),
            // BREATH — one quick pull-back for contrast (dynamic range, not a slow intro).
            ev(
                t * 0.12,
                Tone::Info,
                Emphasis::Faint,
                Density::Spacious,
                Elevation::Flat,
                EventKind::SectionResolved,
            ),
            // BUILD — start the climb, foreground sharpening.
            ev(
                t * 0.25,
                Tone::Accent,
                Emphasis::Normal,
                Density::Normal,
                Elevation::Raised,
                EventKind::FocusAcquired,
            ),
            // TENSION — the build tightens toward the edge.
            ev(
                t * 0.40,
                Tone::Warning,
                Emphasis::Strong,
                Density::Compact,
                Elevation::Raised,
                EventKind::ModalEntered,
            ),
            // DROP — the impact: the payoff hit at maximum pressure.
            ev(
                t * 0.52,
                Tone::Danger,
                Emphasis::Strong,
                Density::Compact,
                Elevation::Overlay,
                EventKind::Impact,
            ),
            // REVEAL — the drop opens into light; the world widens.
            ev(
                t * 0.64,
                Tone::Success,
                Emphasis::Strong,
                Density::Normal,
                Elevation::Overlay,
                EventKind::Confirmation,
            ),
            // RE-LIFT — gather again toward the summit (the second reach).
            ev(
                t * 0.78,
                Tone::Accent,
                Emphasis::Strong,
                Density::Compact,
                Elevation::Overlay,
                EventKind::FocusAcquired,
            ),
            // TRIUMPH — the earned, sustained arrival.
            ev(
                t * 0.90,
                Tone::Success,
                Emphasis::Strong,
                Density::Normal,
                Elevation::Raised,
                EventKind::Confirmation,
            ),
        ],
        total_beats,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Count strict interior local maxima of the event pressure sequence — a proxy for "how many
    /// times does this story reach and release". A single cinematic arc has exactly one; a bounce
    /// has two or more.
    fn local_pressure_maxima(tr: &SemanticTrace) -> usize {
        let p: Vec<f32> = tr.events.iter().map(|e| e.state.pressure()).collect();
        (1..p.len().saturating_sub(1))
            .filter(|&i| p[i] > p[i - 1] && p[i] > p[i + 1])
            .count()
    }

    #[test]
    fn deflected_lift_trace_bounces_where_the_cinematic_arc_climbs_once() {
        // The bounce fixture reaches and releases at least twice; the cinematic demo climbs to a
        // single peak. This is the discriminating property that makes the bounce its own story.
        let bounce = deflected_lift_trace(120.0);
        let cinematic = demo_trace(120.0);
        assert!(
            local_pressure_maxima(&bounce) >= 2,
            "bounce should have >=2 tension crests, got {}",
            local_pressure_maxima(&bounce)
        );
        assert_eq!(
            local_pressure_maxima(&cinematic),
            1,
            "the cinematic arc should climb to exactly one peak"
        );
    }

    #[test]
    fn deflected_lift_trace_never_reaches_catastrophe() {
        // The bounce is bittersweet, not cinematic: no Danger tone, no Overlay elevation.
        let bounce = deflected_lift_trace(120.0);
        assert!(bounce.events.iter().all(|e| e.state.tone != Tone::Danger));
        assert!(bounce
            .events
            .iter()
            .all(|e| e.state.elevation != Elevation::Overlay));
        // And it is genuinely bounded, like every fixture.
        assert!(bounce.events.iter().all(|e| e.at_beat < 120.0));
    }

    #[test]
    fn intro_demo_drops_to_catastrophe_then_arrives() {
        let tr = intro_demo(96.0);
        // The flight commits to its impact: it DOES reach the catastrophic vocabulary (the DROP),
        // unlike the bittersweet bounce, which never does.
        assert!(
            tr.events
                .iter()
                .any(|e| e.state.tone == Tone::Danger && e.state.elevation == Elevation::Overlay),
            "the intro flight must reach its drop"
        );
        // It ends arrived, not peaked: the final triumph releases the drop's pressure.
        let peak = tr
            .events
            .iter()
            .map(|e| e.state.pressure())
            .fold(0.0f32, f32::max);
        let last = tr.events.last().unwrap().state.pressure();
        assert!(
            last < peak,
            "triumph should release the drop's pressure (last {last} < peak {peak})"
        );
        // And it is genuinely bounded, like every fixture.
        assert!(tr.events.iter().all(|e| e.at_beat < 96.0));
    }

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
