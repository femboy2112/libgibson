//! The musical-intent category **𝓜**: objects are [`MusicIntent`] states (far richer than
//! "the current chord"), morphisms are the typed musical transformations, and the Hom-sets
//! are *enriched* over a [`MorphismCost`] vector so the engine can actually optimize.
//!
//! Two properties the design protects:
//!
//! - **Path dependence / holonomy.** `MusicIntent` carries `expectation` and motif
//!   development, so `A → B → C → A` is *not* `identity(A)`: returning to a tonal center
//!   after a journey leaves accumulated expectation and a more-developed motif behind.
//! - **Identity = prolong.** Applying [`IntentMorphism::Prolong`] changes essentially
//!   nothing (a tiny expectation decay), so a settled UI does not manufacture novelty.

use super::theory::Function;

/// How developed the running motif is (path memory, not just "which motif").
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MotifState {
    /// Stable motif identity index (which seed).
    pub id: u8,
    /// Development depth 0..=N — how far the motif has been transformed from its seed.
    pub development: u8,
    /// Current transposition in scale degrees (path-accumulated).
    pub transpose: i8,
}

/// An object of 𝓜.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MusicIntent {
    /// Overall drive `[0,1]`.
    pub energy: f32,
    /// Harmonic/melodic instability `[0,1]`.
    pub tension: f32,
    /// Event density `[0,1]`.
    pub density: f32,
    /// Register bias `[0,1]` (low..high).
    pub register: f32,
    /// Current harmonic function.
    pub function: Function,
    /// Motif development (path memory).
    pub motif: MotifState,
    /// Accumulated expectation / harmonic pull `[0,1]` — history, not just the chord.
    pub expectation: f32,
}

impl Default for MusicIntent {
    fn default() -> Self {
        MusicIntent {
            energy: 0.4,
            tension: 0.2,
            density: 0.4,
            register: 0.5,
            function: Function::Tonic,
            motif: MotifState::default(),
            expectation: 0.0,
        }
    }
}

/// The typed musical transformations (morphisms of 𝓜).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentMorphism {
    /// Identity: hold the current intent.
    Prolong,
    /// Build pre-dominant expectation.
    Prepare,
    /// Raise energy/density.
    Intensify,
    /// Relax energy/density.
    Relax,
    /// Suspend (raise tension, defer resolution).
    Suspend,
    /// Pivot toward a new region without committing.
    Pivot,
    /// Resolve to tonic (release expectation).
    Resolve,
    /// Modulate to a new tonal center.
    Modulate,
    /// Reharmonize the current material.
    Reharmonize,
    /// Fragment the motif (take a piece).
    FragmentMotif,
    /// Sequence the motif (repeat transposed).
    SequenceMotif,
    /// Rhythmic augmentation.
    Augment,
    /// Rhythmic diminution.
    Diminish,
    /// Add syncopation.
    Syncopate,
    /// Thin the texture.
    ThinTexture,
    /// Thicken the texture.
    ThickenTexture,
    /// A cadence (strong resolution, closes a phrase).
    Cadence,
}

/// The enriched cost of applying a morphism — a vector, not one opaque float, so different
/// stages can weight the dimensions they care about.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MorphismCost {
    pub voice_leading: f32,
    pub tension_error: f32,
    pub register_violation: f32,
    pub parallel_motion: f32,
    pub groove_disruption: f32,
    pub motif_loss: f32,
    pub novelty: f32,
    pub repetition: f32,
}

impl MorphismCost {
    /// Sum two costs (composition of morphisms adds their costs — the enriched structure).
    pub fn combine(self, o: MorphismCost) -> MorphismCost {
        MorphismCost {
            voice_leading: self.voice_leading + o.voice_leading,
            tension_error: self.tension_error + o.tension_error,
            register_violation: self.register_violation + o.register_violation,
            parallel_motion: self.parallel_motion + o.parallel_motion,
            groove_disruption: self.groove_disruption + o.groove_disruption,
            motif_loss: self.motif_loss + o.motif_loss,
            novelty: self.novelty + o.novelty,
            repetition: self.repetition + o.repetition,
        }
    }

    /// A weighted scalar for greedy selection. Default weights are sane; a planner can
    /// pass its own.
    pub fn weighted(&self, w: &CostWeights) -> f32 {
        self.voice_leading * w.voice_leading
            + self.tension_error * w.tension_error
            + self.register_violation * w.register_violation
            + self.parallel_motion * w.parallel_motion
            + self.groove_disruption * w.groove_disruption
            + self.motif_loss * w.motif_loss
            + self.novelty * w.novelty
            + self.repetition * w.repetition
    }
}

/// Weights for collapsing a [`MorphismCost`] to a scalar during search.
#[derive(Debug, Clone, Copy)]
pub struct CostWeights {
    pub voice_leading: f32,
    pub tension_error: f32,
    pub register_violation: f32,
    pub parallel_motion: f32,
    pub groove_disruption: f32,
    pub motif_loss: f32,
    pub novelty: f32,
    pub repetition: f32,
}

impl Default for CostWeights {
    fn default() -> Self {
        CostWeights {
            voice_leading: 1.0,
            tension_error: 2.0,
            register_violation: 1.5,
            parallel_motion: 1.2,
            groove_disruption: 1.0,
            motif_loss: 1.5,
            novelty: 0.6,
            repetition: 0.6,
        }
    }
}

impl IntentMorphism {
    /// Apply this morphism to `intent`, returning the new intent and the intrinsic cost of
    /// the move. (Concrete voice-leading cost is added later by the voicing stage; this is
    /// the category-level cost of the *transformation itself*.)
    pub fn apply(self, intent: MusicIntent) -> (MusicIntent, MorphismCost) {
        let mut m = intent;
        let mut cost = MorphismCost::default();
        match self {
            IntentMorphism::Prolong => {
                m.expectation = (m.expectation - 0.03).max(0.0);
                cost.repetition = 0.4;
            }
            IntentMorphism::Prepare => {
                m.function = Function::Predominant;
                m.expectation = (m.expectation + 0.25).min(1.0);
                cost.tension_error = 0.1;
            }
            IntentMorphism::Intensify => {
                m.energy = (m.energy + 0.2).min(1.0);
                m.density = (m.density + 0.15).min(1.0);
                cost.novelty = 0.3;
            }
            IntentMorphism::Relax => {
                m.energy = (m.energy - 0.2).max(0.0);
                m.density = (m.density - 0.15).max(0.0);
            }
            IntentMorphism::Suspend => {
                m.tension = (m.tension + 0.2).min(1.0);
                m.expectation = (m.expectation + 0.2).min(1.0);
                cost.tension_error = 0.15;
            }
            IntentMorphism::Pivot => {
                m.function = Function::Predominant;
                m.expectation = (m.expectation + 0.15).min(1.0);
                cost.novelty = 0.4;
            }
            IntentMorphism::Resolve => {
                m.function = Function::Tonic;
                m.tension = (m.tension - 0.3).max(0.0);
                m.expectation = (m.expectation - 0.5).max(0.0);
            }
            IntentMorphism::Modulate => {
                // A journey: register drifts, expectation and motif development persist —
                // this is where holonomy accrues.
                m.function = Function::Dominant;
                m.expectation = (m.expectation + 0.3).min(1.0);
                m.motif.development = m.motif.development.saturating_add(1);
                cost.novelty = 0.6;
            }
            IntentMorphism::Reharmonize => {
                m.tension = (m.tension + 0.1).min(1.0);
                cost.novelty = 0.5;
                cost.voice_leading = 0.2;
            }
            IntentMorphism::FragmentMotif => {
                m.motif.development = m.motif.development.saturating_add(1);
                cost.motif_loss = 0.3;
            }
            IntentMorphism::SequenceMotif => {
                m.motif.development = m.motif.development.saturating_add(1);
                m.motif.transpose = m.motif.transpose.saturating_add(2);
                cost.repetition = 0.2;
            }
            IntentMorphism::Augment => {
                m.density = (m.density - 0.2).max(0.0);
            }
            IntentMorphism::Diminish => {
                m.density = (m.density + 0.2).min(1.0);
                cost.groove_disruption = 0.1;
            }
            IntentMorphism::Syncopate => {
                cost.groove_disruption = 0.3;
                m.energy = (m.energy + 0.05).min(1.0);
            }
            IntentMorphism::ThinTexture => {
                m.density = (m.density - 0.25).max(0.0);
            }
            IntentMorphism::ThickenTexture => {
                m.density = (m.density + 0.25).min(1.0);
                cost.novelty = 0.2;
            }
            IntentMorphism::Cadence => {
                m.function = Function::Tonic;
                m.tension = (m.tension - 0.4).max(0.0);
                m.expectation = 0.0;
            }
        }
        (m, cost)
    }

    /// A provenance label.
    pub fn label(self) -> &'static str {
        match self {
            IntentMorphism::Prolong => "prolong",
            IntentMorphism::Prepare => "prepare",
            IntentMorphism::Intensify => "intensify",
            IntentMorphism::Relax => "relax",
            IntentMorphism::Suspend => "suspend",
            IntentMorphism::Pivot => "pivot",
            IntentMorphism::Resolve => "resolve",
            IntentMorphism::Modulate => "modulate",
            IntentMorphism::Reharmonize => "reharmonize",
            IntentMorphism::FragmentMotif => "fragment",
            IntentMorphism::SequenceMotif => "sequence",
            IntentMorphism::Augment => "augment",
            IntentMorphism::Diminish => "diminish",
            IntentMorphism::Syncopate => "syncopate",
            IntentMorphism::ThinTexture => "thin",
            IntentMorphism::ThickenTexture => "thicken",
            IntentMorphism::Cadence => "cadence",
        }
    }
}

/// Apply a sequence of morphisms, threading the intent and summing costs (composition in
/// the enriched category).
pub fn compose(intent: MusicIntent, seq: &[IntentMorphism]) -> (MusicIntent, MorphismCost) {
    let mut m = intent;
    let mut c = MorphismCost::default();
    for &morph in seq {
        let (nm, nc) = morph.apply(m);
        m = nm;
        c = c.combine(nc);
    }
    (m, c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_prolong_barely_changes_intent() {
        let a = MusicIntent::default();
        let (b, _) = IntentMorphism::Prolong.apply(a);
        assert_eq!(b.energy, a.energy);
        assert_eq!(b.tension, a.tension);
        assert_eq!(b.function, a.function);
    }

    #[test]
    fn composition_threads_state_and_sums_cost() {
        let a = MusicIntent::default();
        let (seq_state, seq_cost) =
            compose(a, &[IntentMorphism::Prepare, IntentMorphism::Resolve]);
        // Manually compose.
        let (m1, c1) = IntentMorphism::Prepare.apply(a);
        let (m2, c2) = IntentMorphism::Resolve.apply(m1);
        assert_eq!(seq_state, m2);
        assert_eq!(seq_cost, c1.combine(c2));
    }

    #[test]
    fn holonomy_returning_is_not_identity() {
        // A -> (modulate away) -> ... -> (modulate back to tonic) is NOT identity(A):
        // expectation and motif development persist.
        let a = MusicIntent::default();
        let (returned, _) = compose(
            a,
            &[
                IntentMorphism::Modulate,
                IntentMorphism::SequenceMotif,
                IntentMorphism::Modulate,
                IntentMorphism::Resolve,
            ],
        );
        assert_eq!(returned.function, Function::Tonic); // same "chord label"
        assert_ne!(returned, a); // but not the same phenomenological state
        assert!(returned.motif.development > a.motif.development);
    }

    #[test]
    fn resolve_releases_expectation_built_by_prepare() {
        let a = MusicIntent::default();
        let (prepared, _) = IntentMorphism::Prepare.apply(a);
        assert!(prepared.expectation > a.expectation);
        let (resolved, _) = IntentMorphism::Resolve.apply(prepared);
        assert!(resolved.expectation < prepared.expectation);
        assert_eq!(resolved.function, Function::Tonic);
    }

    #[test]
    fn cost_weighting_is_monotone() {
        let w = CostWeights::default();
        let cheap = MorphismCost {
            voice_leading: 1.0,
            ..Default::default()
        };
        let dear = MorphismCost {
            voice_leading: 5.0,
            ..Default::default()
        };
        assert!(dear.weighted(&w) > cheap.weighted(&w));
    }
}
