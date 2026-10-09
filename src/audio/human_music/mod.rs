//! **HumanMusic** — a fully self-synthesized procedural music + SFX system driven by
//! LibGibson's semantic style/story state.
//!
//! Named for the Rick and Morty joke, built to be the opposite of joke-quality: the goal
//! is complex harmony, coherent voice leading, genuine groove, motif development and
//! production-quality dynamics — all generated programmatically with **no prerecorded
//! musical assets**.
//!
//! The pipeline is a planning pipeline, not a proof: a semantic trace becomes ONE
//! [`song::SongMap`] (the song — the causal [`timeline::IntentTimeline`], the
//! [`plan::CompositionPlan`], and the thematic and harmonic maps, all before any room or idiom),
//! the song plus a [`world::MusicWorld`] (timbre, home mode) and a [`language::MusicalLanguage`]
//! (idiom) one [`performance::PerformancePlan`] (the shared performance: actions, the stage,
//! harmony, interaction material, the complexity budget), and every player realizes a projection
//! of that performance into the [`score::Score`] IR, stamped with the exact actions it performs
//! ([`witness`] audits them; [`song::SongMapConformance`] checks the performance preserves the
//! song; Round X: [`composer`] can choose a song's content toward the listener plan
//! [`meaning::MeaningPlan`] its story asks for, and [`meaning::Commutation`] checks that it means
//! it). Modern policy and exact cover APIs are opt-in; historical entry points remain controls.
//! See `docs/HUMAN_MUSIC_CONSOLIDATION.md` for current authorities and
//! `docs/HUMAN_MUSIC_COVER.md` for source ingestion, invariant projection and checked lifts.
//! `docs/HUMAN_MUSIC_ARCHITECTURE.md` preserves the experimental lineage.

pub mod action;
pub mod backbone;
pub mod bass;
pub mod budget;
pub mod comp;
pub mod composer;
pub mod context;
pub mod contract;
pub mod cover;
pub mod diagnostics;
pub mod discourse;
pub mod ensemble;
#[cfg(test)]
mod ensemble_probes;
pub mod expression;
pub mod fingerprint;
pub mod form;
pub mod functor;
pub mod gesture;
pub mod groove;
pub mod harmonic_state;
pub mod harmony;
#[cfg(test)]
mod heard_witnesses;
pub mod identity;
pub mod ids;
pub mod instrument;
pub mod intent;
pub mod interaction;
pub mod language;
pub mod mass;
#[cfg(test)]
mod mass_witnesses;
pub mod material;
pub mod meaning;
#[cfg(test)]
mod meaning_probes;
pub mod melody;
pub mod motif;
pub mod narrative;
#[cfg(test)]
mod narrative_join_probes;
pub mod occupancy;
pub mod percussion;
pub mod performance;
pub mod phenomenal;
#[cfg(test)]
mod phenomenal_probes;
pub mod phrase_diagnostics;
pub mod phrase_expression;
pub mod pitch;
pub mod plan;
pub mod pocket;
pub mod pocket_diagnostics;
pub mod policy;
#[cfg(test)]
mod probes;
pub mod projection;
pub mod receipt;
pub mod reference_song;
pub mod region;
pub mod rehearsal;
pub mod rhythm;
pub mod rng;
pub mod score;
pub mod semantic;
pub mod sfx;
pub mod song;
#[cfg(test)]
mod song_probes;
pub mod sonority;
pub mod support;
pub mod support_voicing;
pub mod surgical;
pub mod synth;
pub mod temporal;
#[cfg(test)]
mod temporal_probes;
#[cfg(test)]
mod temporal_witnesses;
pub mod tension;
#[cfg(test)]
mod tension_witnesses;
pub mod theory;
pub mod timeline;
pub mod vocabulary;
pub mod voice;
pub mod voicing;
pub mod voicing_diagnostics;
pub mod witness;
pub mod world;

pub use functor::{compose, compose_with_options, perform, CompositionOptions};
pub use score::Score;
pub use semantic::{demo_trace, SemanticTrace};
pub use song::{MotifRepetition, SongMap};
pub use synth::HumanMusicSynth;
pub use world::{MusicWorld, WorldId};

use super::render::{OfflineRenderer, RenderResult};
use super::time::SampleRate;

/// Compose a score for `trace` under `world` (seed `seed`) and render it offline to PCM.
/// The one call the lab and tests use to go from semantic meaning to a finished stereo
/// buffer, deterministically.
pub fn render(
    trace: &SemanticTrace,
    world: &MusicWorld,
    seed: u64,
    sr: SampleRate,
    block: usize,
) -> (Score, RenderResult) {
    let score = compose(trace, world, seed);
    let mut synth = HumanMusicSynth::new(&score, world, sr);
    let frames = synth.total_samples();
    let out = OfflineRenderer::new(sr, block).render(&mut synth, frames);
    (score, out)
}

/// Like [`render`], but composed under explicit [`CompositionOptions`] (the recurrence dial, an
/// optional forced grammar). `CompositionOptions::default()` renders exactly as [`render`] does.
pub fn render_with_options(
    trace: &SemanticTrace,
    world: &MusicWorld,
    seed: u64,
    sr: SampleRate,
    block: usize,
    opts: CompositionOptions,
) -> (Score, RenderResult) {
    let score = compose_with_options(trace, world, seed, opts);
    let mut synth = HumanMusicSynth::new(&score, world, sr);
    let frames = synth.total_samples();
    let out = OfflineRenderer::new(sr, block).render(&mut synth, frames);
    (score, out)
}
