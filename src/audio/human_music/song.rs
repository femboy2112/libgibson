//! **SongMap** — the song, defined once, upstream of every performance (Round IX).
//!
//! The chart every musician already knows before anybody plays: what the piece IS, independent of
//! the room it is played in ([`super::world::MusicWorld`]: timbre, production, the home mode) and
//! the idiom it is spoken in ([`super::language::MusicalLanguage`]: colour, surface, conversation).
//!
//! There is exactly one place a song is generated — [`SongMap::build`] — and exactly one place a
//! performance of it is generated — [`super::performance::PerformancePlan::from_song`], realized
//! by [`super::functor::perform`]. The law this module exists for:
//!
//! ```text
//!   Performance ──π──▶ SongMap        π(P) = S for every lawful performance P of S;
//!                                      the fiber π⁻¹(S) is the band.
//! ```
//!
//! **Song identity** (this object): the coherence contract, the form graph (phrases, families,
//! exact length), the discourse (roles, closures, culmination, the obligation ledger), the
//! arrangement envelope (who is seated per phrase), and the DeflectedLift backbone timeline
//! (gestures, cycles, slot grid).
//!
//! **Performance freedom** (the fiber, NOT here): register, voicing, chord colour and extensions,
//! articulation, dynamics, timbre, pan, swing, microtiming, passing and approach tones, fills,
//! discretionary calls and answers, who answers, response latency, density, ornamentation.

use super::contract::{CoherenceContract, CompositionGrammar};
use super::plan::CompositionPlan;
use super::semantic::SemanticTrace;
use super::timeline::IntentTimeline;

/// The song: every coordinate a performance must preserve, built before any world or language is
/// known.
#[derive(Debug, Clone)]
pub struct SongMap {
    /// The semantic trace the song was composed from (its events also pitch the reaction SFX).
    pub trace: SemanticTrace,
    /// The causal intent timeline walked from the trace.
    pub timeline: IntentTimeline,
    /// Contract, form, discourse, arrangement envelope and (DeflectedLift) the backbone timeline.
    pub plan: CompositionPlan,
    /// The composition seed. Every song-level choice draws from it; performances reuse it for
    /// their own (fiber) randomness.
    pub seed: u64,
}

impl SongMap {
    /// Compose the song for `trace`, deterministic in `seed`. `grammar` forces a grammar (the
    /// calibration path); `None` infers one from the trace's shape.
    pub fn build(trace: &SemanticTrace, seed: u64, grammar: Option<CompositionGrammar>) -> SongMap {
        // The piece is exactly as long as the request: a partial final bar is represented, not
        // rounded away (9 beats used to render 8, 10.5 → 12, 17 → 16).
        let timeline = IntentTimeline::walk(trace);
        let plan = match grammar {
            Some(g) => CompositionPlan::build_with_contract_for_beats(
                &timeline,
                trace.total_beats,
                CoherenceContract::for_grammar(g),
            ),
            None => CompositionPlan::build_for_beats(&timeline, trace.total_beats),
        };
        SongMap {
            trace: trace.clone(),
            timeline,
            plan,
            seed,
        }
    }
}
