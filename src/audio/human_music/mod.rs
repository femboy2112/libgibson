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
//! [`world::MusicWorld`]. See [`docs/HUMAN_MUSIC_ARCHITECTURE.md`].
//!
//! Stub root — the harmony/voicing/form/motif/groove/synthesis engines land across the
//! 0.4 audio round.
