//! Round XI: a vector of musical witnesses, independent of the Round X event grammar.
//! F is explicit about the intended regime; μ reads the written song, never its target label.
//! These are compositional affordances, not measured emotions or a model of enjoyment.

use super::meaning::Level;
use super::semantic::SemanticTrace;

/// An explicit composition request. Neither regime is a quality judgment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhenomenalRegime {
    /// Sustained preparation, recurring denial, and withheld closure: a negative control.
    SuspendedDeflection,
    /// A recurring rhythmic identity over tonic prolongation and confirmed returns.
    StablePropulsion,
}

/// Independent coordinates. No weighted total is defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhenomenalState {
    pub stability: Level,
    pub propulsion: Level,
    pub expectation: Level,
    pub surprise: Level,
    /// Written melodic register span; performance voicing/texture is outside this observer.
    pub openness: Level,
    pub familiarity: Level,
}

/// F(trace, regime), before grammar or notes exist. The explicit request selects the meaning;
/// the trace supplies duration. The legacy pressure-to-deflection interpretation remains R10.
#[derive(Debug, Clone, PartialEq)]
pub struct PhenomenalTarget {
    pub regime: PhenomenalRegime,
    pub total_beats: f64,
    pub state: PhenomenalState,
    /// Whether recurring expected home arrivals are requested as positive payoffs.
    pub confirm: bool,
}

impl PhenomenalTarget {
    /// One factorization per regime; a deflection slot never has to mean confirmation.
    pub fn grammar(&self) -> super::contract::CompositionGrammar {
        match self.regime {
            PhenomenalRegime::StablePropulsion => {
                super::contract::CompositionGrammar::PropulsiveReturn
            }
            PhenomenalRegime::SuspendedDeflection => {
                super::contract::CompositionGrammar::DeflectedLift
            }
        }
    }
    pub fn from_trace(trace: &SemanticTrace, regime: PhenomenalRegime) -> Self {
        use Level::*;
        let state = match regime {
            PhenomenalRegime::StablePropulsion => PhenomenalState {
                stability: High,
                propulsion: High,
                expectation: Low,
                surprise: Low,
                openness: High,
                familiarity: High,
            },
            PhenomenalRegime::SuspendedDeflection => PhenomenalState {
                stability: Low,
                propulsion: High,
                expectation: High,
                surprise: High,
                openness: High,
                familiarity: High,
            },
        };
        Self {
            regime,
            total_beats: if trace.total_beats.is_finite() && trace.total_beats > 0.0 {
                trace.total_beats
            } else {
                4.0
            },
            state,
            confirm: regime == PhenomenalRegime::StablePropulsion,
        }
    }

    /// Preparation derives from the requested expectation coordinate, never from a slot name.
    /// Low expectation permits no forced dominant; high expectation calls for a strong pointer.
    pub fn preparation(&self) -> Level {
        self.state.expectation
    }
}

/// Fulfilled expectation is an event with positive content, including learned plagal returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrival {
    /// No predicted arrival occurs at this point.
    None,
    /// A dominant or a previously learned neighboring motion arrives at home.
    Confirm,
    /// A dominant points home, but the next root withholds it.
    Withheld,
}
