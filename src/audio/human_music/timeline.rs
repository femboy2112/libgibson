//! `IntentTimeline` — the **causal spine** of HumanMusic Round II.
//!
//! Round I computed a running [`MusicIntent`] only to throw it away: `walk_intent`
//! folded the whole trace down to one endpoint intent used by a couple of tests, and the
//! four voice generators each made *local* decisions with private RNGs. The intent path
//! was decorative, not causal.
//!
//! This module makes it causal. [`IntentTimeline::walk`] walks the semantic trace **once**
//! and materializes the running intent over time as a sequence of inspectable
//! [`IntentTransition`]s — previous intent, the semantic event, the morphisms it applied,
//! the resulting intent, the step cost and the accumulated (holonomy-bearing) cost. Every
//! downstream planning stage reads its intent from here via [`IntentTimeline::intent_at`],
//! so the concrete score is *derived from* this spine rather than inventing structure in
//! parallel.
//!
//! It also fixes a concrete Round-I defect: the semantic **elevation** axis was dropped
//! entirely (`SemanticState::register_bias` had zero callers). Here each transition carries
//! elevation into `MusicIntent::register`, so the register axis is finally live and can
//! drive planned register downstream.

use super::functor::event_to_morphisms;
use super::intent::{IntentMorphism, MorphismCost, MusicIntent};
use super::semantic::{EventKind, SemanticTrace};

/// One inspectable step of the intent walk: everything needed to explain *why* the running
/// intent is what it is at a given beat.
#[derive(Debug, Clone)]
pub struct IntentTransition {
    /// Beat at which the semantic event fired.
    pub at_beat: f64,
    /// The semantic event kind that drove this transition.
    pub event_kind: EventKind,
    /// The running intent *before* this event.
    pub prev: MusicIntent,
    /// The intent morphisms this event applied, in order.
    pub applied: Vec<IntentMorphism>,
    /// The running intent *after* this event (including the elevation→register carry).
    pub next: MusicIntent,
    /// The vector cost of *this* event's morphism sequence alone.
    pub step_cost: MorphismCost,
    /// The accumulated vector cost since the start of the trace — because
    /// [`MorphismCost::combine`] sums, this is where path dependence (holonomy) is visible.
    pub acc_cost: MorphismCost,
}

/// The causal materialization of musical intent over an entire semantic trace.
#[derive(Debug, Clone)]
pub struct IntentTimeline {
    /// The intent before any event (the default settled state).
    pub initial: MusicIntent,
    /// Every transition, in beat order.
    pub transitions: Vec<IntentTransition>,
    /// The intent after the last event (`initial` if the trace is empty).
    pub final_intent: MusicIntent,
}

impl IntentTimeline {
    /// Walk `trace` once, materializing the running intent at every semantic event.
    ///
    /// This is the single source of truth for intent evolution — `functor::walk_intent`
    /// delegates to it, and the planning layer reads from it. Deterministic: it applies the
    /// same `event_to_morphisms` mapping the functor already declared, so it cannot drift
    /// from the category-level action the tests certify.
    pub fn walk(trace: &SemanticTrace) -> IntentTimeline {
        let initial = MusicIntent::default();
        let mut intent = initial;
        let mut acc = MorphismCost::default();
        let mut transitions = Vec::with_capacity(trace.events.len());

        for ev in &trace.events {
            let prev = intent;
            let applied = event_to_morphisms(ev.kind, ev.state.tone);

            let mut step = MorphismCost::default();
            for &m in &applied {
                let (ni, c) = m.apply(intent);
                intent = ni;
                step = step.combine(c);
            }

            // Carry the semantic ELEVATION axis into `register`. Round I dropped this
            // entirely; no morphism touches `register`, so elevation is the honest source.
            intent.register = ev.state.register_bias();

            acc = acc.combine(step);
            transitions.push(IntentTransition {
                at_beat: ev.at_beat,
                event_kind: ev.kind,
                prev,
                applied,
                next: intent,
                step_cost: step,
                acc_cost: acc,
            });
        }

        IntentTimeline {
            initial,
            transitions,
            final_intent: intent,
        }
    }

    /// The intent in force at `beat` — the most recent transition at or before it, or the
    /// initial settled intent before the first event.
    pub fn intent_at(&self, beat: f64) -> MusicIntent {
        self.transitions
            .iter()
            .rev()
            .find(|t| t.at_beat <= beat + 1e-9)
            .map(|t| t.next)
            .unwrap_or(self.initial)
    }

    /// The transition produced by the event at `index`, if any.
    pub fn step(&self, index: usize) -> Option<&IntentTransition> {
        self.transitions.get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::super::semantic::{
        demo_trace, Density, Elevation, Emphasis, SemanticEvent, SemanticState, Tone,
    };
    use super::*;

    #[test]
    fn walk_has_one_transition_per_event_and_a_consistent_endpoint() {
        let trace = demo_trace(120.0);
        let tl = IntentTimeline::walk(&trace);
        assert_eq!(tl.transitions.len(), trace.events.len());
        // final_intent is exactly the last transition's result.
        assert_eq!(tl.final_intent, tl.transitions.last().unwrap().next);
    }

    #[test]
    fn walk_endpoint_matches_the_old_fold_semantics() {
        // The demo arc must still end resolved and with a developed motif — the invariant
        // the old `walk_intent` test pinned, now proven on the timeline it delegates to.
        let tl = IntentTimeline::walk(&demo_trace(120.0));
        assert!(tl.final_intent.tension < 0.5, "arc should end resolved");
        assert!(
            tl.final_intent.motif.development > 0,
            "motif should develop"
        );
    }

    #[test]
    fn elevation_axis_is_live_in_register() {
        // The Round-I bug: elevation never reached composition. Now register tracks it.
        let tl = IntentTimeline::walk(&demo_trace(120.0));
        // The Impact at beat 64 carries Elevation::Overlay (register_bias 0.85).
        let hi = tl.intent_at(70.0).register;
        // The coda at beat 104 is Elevation::Flat (register_bias 0.40).
        let lo = tl.intent_at(110.0).register;
        assert!(
            hi > lo,
            "overlay register {hi} should exceed flat register {lo}"
        );
        assert!((hi - 0.85).abs() < 1e-6, "overlay maps to 0.85, got {hi}");
        assert!((lo - 0.40).abs() < 1e-6, "flat maps to 0.40, got {lo}");
    }

    #[test]
    fn prolong_transition_preserves_structural_identity() {
        // A Prolong event must not manufacture structural novelty: energy/tension/function
        // are untouched (only expectation decays, and register follows elevation).
        let st = SemanticState {
            tone: Tone::Neutral,
            emphasis: Emphasis::Normal,
            density: Density::Normal,
            elevation: Elevation::Flat,
        };
        let trace = SemanticTrace::new(
            vec![SemanticEvent {
                at_beat: 0.0,
                state: st,
                kind: EventKind::Prolong,
            }],
            16.0,
        );
        let tl = IntentTimeline::walk(&trace);
        let t = &tl.transitions[0];
        assert_eq!(t.prev.energy, t.next.energy);
        assert_eq!(t.prev.tension, t.next.tension);
        assert_eq!(t.prev.function, t.next.function);
    }

    #[test]
    fn accumulated_cost_is_monotone_nondecreasing() {
        // combine() sums non-negative costs, so every accumulated component only grows.
        let tl = IntentTimeline::walk(&demo_trace(120.0));
        let mut prev = MorphismCost::default();
        for tr in &tl.transitions {
            assert!(tr.acc_cost.novelty + 1e-6 >= prev.novelty);
            assert!(tr.acc_cost.tension_error + 1e-6 >= prev.tension_error);
            prev = tr.acc_cost;
        }
    }
}
