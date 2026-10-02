//! `CoherenceContract` — the piece's declared identity.
//!
//! Round I assumed exactly one source of musical coherence (functional harmony over a
//! world-independent section skeleton) and then failed to actually enforce even that. The
//! deeper problem: not every coherent piece is coherent *for the same reason*. A riff-driven
//! track is held together by a rhythmic riff and a bass/drum lock over near-static harmony;
//! a loop-evolution track by groove + a short harmonic loop + timbral change; a hook song by
//! a recurring motif + a directed harmonic contour.
//!
//! A [`CoherenceContract`] states, up front, **what the listener is expected to recognize**
//! (the load-bearing [`CoherenceAnchor`]s), how phrases are sized, how the piece resolves and
//! how many voices may be in the foreground at once — the planners hold themselves to these, and
//! [`super::song::AnchorReport`] judges whether the realized score established each anchor. The
//! contract also records calibration nobody enforces (a transformation bound, a novelty budget);
//! each field's documentation states which it is, and a perturbation audit proves it.
//!
//! This is deliberately separate from [`super::world::MusicWorld`], which stays a *sonic
//! dialect* (timbre, mix, harmonic vocabulary constraints). The same contract under three
//! worlds is the same piece in three dialects; the same world under two contracts is two
//! different pieces sharing a sound.

use super::timeline::IntentTimeline;

/// A load-bearing identity axis: something the listener is meant to recognize and track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoherenceAnchor {
    /// A melodic motif (interval contour + rhythm) developed across the piece.
    Motif,
    /// A rhythmic riff figure repeated with insistence.
    Riff,
    /// A groove cell (kick/snare/hat pattern) with bounded variation.
    Groove,
    /// A directed harmonic contour (a trajectory with departure and arrival).
    HarmonicContour,
    /// A short, cyclically repeated harmonic loop.
    HarmonicLoop,
    /// Large-scale sectional form (recurrence of section families).
    Form,
    /// A timbral / orchestration fingerprint.
    Orchestration,
    /// A characteristic bass figure.
    BassFigure,
}

/// A compositional *strategy* family. These are ways a piece can cohere — not artist
/// presets and not sonic worlds. A world realizes any of them in its own dialect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionGrammar {
    /// Pop/rock-like directed song: a recurring hook over a directed harmonic contour and a
    /// steady groove, built toward a chorus-like arrival.
    HookArc,
    /// Production-driven track: a small harmonic/riff loop whose interest comes from groove,
    /// orchestration, density and timbre evolving under controlled mutation.
    LoopEvolution,
    /// Rhythmically forceful recurring riff locked to bass/drums; harmony may be pedal/modal
    /// and comparatively static, with breakdown / re-entry and clear negative space.
    RiffDrive,
    /// Two locally coherent regimes joined by an explicit transport that preserves at least
    /// one declared identity axis across the switch.
    WorldSwitch,
    /// A bittersweet **bounce**: a small cyclic harmonic identity the ear learns, a hook that
    /// keeps returning, and a lift → soft deflection → warm opening → rounded reset contour that
    /// recurs instead of building to one cinematic climax. Failure/deflection becomes part of the
    /// bounce; the song keeps reinterpreting a little material rather than inventing new.
    DeflectedLift,
    /// Tonic prolongation, literal recurrence and a plagal neighbor that returns as expected.
    /// Complexity is performed over a stable center; no denied cadence is required.
    PropulsiveReturn,
}

/// How phrases are expected to resolve — the coarse policy the harmonic planner refines into
/// concrete cadences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionPolicy {
    /// Functional cadential motion (predominant → dominant → tonic arrivals).
    Functional,
    /// Cyclic return to the loop's home chord; no cadential pull required.
    Loop,
    /// Modal/pedal centring; stability by drone/centre rather than cadence.
    ModalPedal,
}

/// The declared identity and budgets of one piece.
///
/// Every field states its standing: **Load-bearing** (a planner, realizer or law reads it) or
/// **Descriptive** (recorded in the song's fingerprints and the plan dump, read by nothing). The
/// standing is audited by perturbation (`contract_standing.rs`): a Descriptive field that starts
/// being read, or a Load-bearing one nothing reads any more, fails that audit.
#[derive(Debug, Clone)]
pub struct CoherenceContract {
    /// The strategy family governing planning. **Load-bearing**: it selects the backbone
    /// (DeflectedLift, PropulsiveReturn), the stable thesis and chart, and the harmonic map.
    pub grammar: CompositionGrammar,
    /// The axes the listener is expected to recognize and track (most-load-bearing first).
    /// **Load-bearing**: the arrangement seats the groove a Groove anchor declares, a
    /// Motif-declaring song restates its thesis, [`super::song::AnchorReport`] judges each
    /// anchor's presence and conformance (a [`super::receipt::PerformanceReceipt`] law), and a
    /// cover of a generated source pins only the established ones.
    pub anchors: Vec<CoherenceAnchor>,
    /// The period, in bars, over which the primary anchor should recur. **Load-bearing** only as
    /// DeflectedLift's fixed-tiling rate: a trace too sparse to bind gestures tiles the cell at
    /// `recurrence_bars / 4` bars per gesture. Under every other grammar, and for a DeflectedLift
    /// whose gestures bind, nothing reads it.
    pub recurrence_bars: u32,
    /// The transformation magnitude `[0,1]` the grammar was calibrated to allow an anchor off a
    /// rupture. **Descriptive**: no planner bounds a transformation by it and no law measures one
    /// against it.
    pub max_transform: f32,
    /// The preferred phrase length in bars (a musically legible grid: 2/4/8). **Load-bearing**:
    /// the form's phrase grid.
    pub phrase_bars: u32,
    /// How phrases are expected to resolve. **Load-bearing**: the harmony engine's cadential
    /// policy, and the harmonic-trajectory conformance of a song without a chart.
    pub resolution: ResolutionPolicy,
    /// The maximum number of simultaneous foreground voices (usually 1, sometimes 2).
    /// **Load-bearing**: the arrangement seats at most this many foreground voices per phrase.
    pub foreground_budget: u8,
    /// The novelty a single phrase may spend before it counts as a rupture `[0,1]`, as
    /// calibrated. **Descriptive**: it is scaled per role into each phrase goal's
    /// `novelty_budget`, which nothing reads; no phrase's novelty is measured against it.
    pub novelty_budget: f32,
}

impl CoherenceContract {
    /// The canonical contract for a grammar family. Calibration probes construct pieces by
    /// picking a grammar directly; [`CoherenceContract::infer`] picks one from a trace.
    pub fn for_grammar(grammar: CompositionGrammar) -> CoherenceContract {
        match grammar {
            CompositionGrammar::HookArc => CoherenceContract {
                grammar,
                anchors: vec![
                    CoherenceAnchor::Motif,
                    CoherenceAnchor::HarmonicContour,
                    CoherenceAnchor::Groove,
                ],
                recurrence_bars: 8,
                max_transform: 0.40,
                phrase_bars: 4,
                resolution: ResolutionPolicy::Functional,
                foreground_budget: 1,
                novelty_budget: 0.50,
            },
            CompositionGrammar::LoopEvolution => CoherenceContract {
                grammar,
                anchors: vec![
                    CoherenceAnchor::Groove,
                    CoherenceAnchor::HarmonicLoop,
                    CoherenceAnchor::Orchestration,
                ],
                recurrence_bars: 4,
                max_transform: 0.30,
                phrase_bars: 4,
                resolution: ResolutionPolicy::Loop,
                foreground_budget: 1,
                novelty_budget: 0.40,
            },
            CompositionGrammar::RiffDrive => CoherenceContract {
                grammar,
                anchors: vec![
                    CoherenceAnchor::Riff,
                    CoherenceAnchor::Groove,
                    CoherenceAnchor::BassFigure,
                ],
                recurrence_bars: 2,
                max_transform: 0.25,
                phrase_bars: 4,
                resolution: ResolutionPolicy::ModalPedal,
                foreground_budget: 1,
                novelty_budget: 0.35,
            },
            CompositionGrammar::WorldSwitch => CoherenceContract {
                grammar,
                // The transported identity: what survives the switch to keep it one piece.
                anchors: vec![CoherenceAnchor::Motif, CoherenceAnchor::BassFigure],
                recurrence_bars: 8,
                max_transform: 0.60,
                phrase_bars: 4,
                resolution: ResolutionPolicy::Functional,
                foreground_budget: 2,
                novelty_budget: 0.70,
            },
            CompositionGrammar::DeflectedLift => CoherenceContract {
                grammar,
                // Motif hook + a small cyclic harmonic loop the ear learns, over a steady groove.
                anchors: vec![
                    CoherenceAnchor::Motif,
                    CoherenceAnchor::HarmonicLoop,
                    CoherenceAnchor::Groove,
                ],
                // The hook recurs often; the harmonic cell repeats every few bars, deliberately
                // small so prediction can build. Cyclic resolution, not cinematic cadence.
                recurrence_bars: 4,
                max_transform: 0.35,
                phrase_bars: 4,
                resolution: ResolutionPolicy::Loop,
                foreground_budget: 1,
                novelty_budget: 0.40,
            },
            CompositionGrammar::PropulsiveReturn => CoherenceContract {
                grammar,
                anchors: vec![
                    CoherenceAnchor::Motif,
                    CoherenceAnchor::HarmonicLoop,
                    CoherenceAnchor::Groove,
                ],
                recurrence_bars: 8,
                max_transform: 0.35,
                phrase_bars: 4,
                resolution: ResolutionPolicy::Loop,
                foreground_budget: 1,
                novelty_budget: 0.40,
            },
        }
    }

    /// Infer a contract from the *shape* of a semantic trace's [`IntentTimeline`].
    ///
    /// The classifier keys on shape, not absolute magnitude (a fixed tension threshold would
    /// just be fit to one fixture): a trace whose intent **rises** meaningfully above its
    /// settled start and then **falls back** by the end is a directed arc (→ `HookArc`); one
    /// that rises and *stays* elevated is riff-driven (→ `RiffDrive`); one that never builds
    /// is loop-evolution material (→ `LoopEvolution`). This is a *starting* classification,
    /// not a claim about quality.
    pub fn infer(timeline: &IntentTimeline) -> CoherenceContract {
        let start = timeline.initial;
        let peak_tension = timeline
            .transitions
            .iter()
            .map(|t| t.next.tension)
            .fold(start.tension, f32::max);
        let peak_energy = timeline
            .transitions
            .iter()
            .map(|t| t.next.energy)
            .fold(start.energy, f32::max);
        let final_tension = timeline.final_intent.tension;

        // Built up = drive rose clearly above the settled opening on either axis.
        let built_up = (peak_tension - start.tension) > 0.15 || (peak_energy - start.energy) > 0.2;
        // Resolved = tension came back down, both absolutely and relative to its own peak.
        let resolves = final_tension < 0.5 && final_tension < peak_tension - 0.2;

        let grammar = if built_up && resolves {
            CompositionGrammar::HookArc
        } else if built_up {
            CompositionGrammar::RiffDrive
        } else {
            CompositionGrammar::LoopEvolution
        };
        Self::for_grammar(grammar)
    }
}

#[cfg(test)]
mod tests {
    use super::super::semantic::demo_trace;
    use super::*;

    #[test]
    fn demo_arc_infers_hook_arc() {
        // The demo trace rises to Danger/Impact then resolves — a directed arc.
        let tl = IntentTimeline::walk(&demo_trace(120.0));
        let c = CoherenceContract::infer(&tl);
        assert_eq!(c.grammar, CompositionGrammar::HookArc);
        assert!(c.anchors.contains(&CoherenceAnchor::Motif));
        assert_eq!(c.resolution, ResolutionPolicy::Functional);
    }

    #[test]
    fn every_grammar_has_a_primary_anchor_and_a_sane_budget() {
        for g in [
            CompositionGrammar::HookArc,
            CompositionGrammar::LoopEvolution,
            CompositionGrammar::RiffDrive,
            CompositionGrammar::WorldSwitch,
            CompositionGrammar::DeflectedLift,
        ] {
            let c = CoherenceContract::for_grammar(g);
            assert!(!c.anchors.is_empty(), "{g:?} has no anchor");
            assert!(c.foreground_budget >= 1, "{g:?} silences the whole band");
            assert!((0.0..=1.0).contains(&c.max_transform));
            assert!((0.0..=1.0).contains(&c.novelty_budget));
            assert!(c.phrase_bars >= 1);
        }
    }

    #[test]
    fn world_switch_transports_at_least_one_anchor() {
        let c = CoherenceContract::for_grammar(CompositionGrammar::WorldSwitch);
        assert!(
            !c.anchors.is_empty(),
            "a world switch with no transported identity is two songs, not one"
        );
        assert!(c.foreground_budget >= 1);
    }
}

#[cfg(test)]
#[path = "contract_standing.rs"]
mod standing;
