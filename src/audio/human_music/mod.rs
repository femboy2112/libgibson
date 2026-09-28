//! **HumanMusic** — a fully self-synthesized procedural music + SFX system driven by
//! LibGibson's semantic style/story state.
//!
//! Named for the Rick and Morty joke, built to be the opposite of joke-quality: the goal
//! is complex harmony, coherent voice leading, genuine groove, motif development and
//! production-quality dynamics — all generated programmatically with **no prerecorded
//! musical assets**.
//!
//! The central construction is a lax-monoidal functor `F_skin : 𝒮 -> 𝓜` from a semantic
//! style/story category to a musical-intent category, parameterized by a LibGibson
//! [`world::MusicWorld`]. See `docs/HUMAN_MUSIC_ARCHITECTURE.md`.
//!
//! Round I builds the full vertical slice: semantic category, intent category, the three
//! MusicWorlds, form/harmony/voicing/motif/groove engines, the Score IR, the synthesizer,
//! and the `F_skin` functor tying them together.

pub mod backbone;
pub mod context;
pub mod contract;
pub mod diagnostics;
pub mod discourse;
pub mod form;
pub mod functor;
pub mod groove;
pub mod harmony;
pub mod instrument;
pub mod intent;
pub mod motif;
pub mod pitch;
pub mod plan;
pub mod rng;
pub mod score;
pub mod semantic;
pub mod synth;
pub mod theory;
pub mod timeline;
pub mod voicing;
pub mod world;

pub use functor::compose;
pub use score::Score;
pub use semantic::{demo_trace, SemanticTrace};
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
