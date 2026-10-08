//! **Auditory observer** (`gibson::audio::perception`) — Phase 0, observer-only.
//!
//! This layer answers one question: *given a stream of acoustic samples and an
//! explicitly declared listening context, what does a deterministic auditory model
//! expose?* It is the second of the three distinctions the design note draws:
//!
//! ```text
//!   Score / performance   !=   rendered PCM   !=   auditory percept
//! ```
//!
//! The first distinction already lives upstream (in `human_music`). This module adds
//! the second → third step: PCM in, **multiscale auditory evidence** out.
//!
//! # Boundary law (do not violate)
//!
//! `audio::perception` knows **nothing** about chords, `SongMap`, instrument roles,
//! motifs, musical grammar, "good" music, or HumanMusic worlds. It consumes acoustic
//! samples plus a declared [`listener::ListeningContext`] and produces inspectable
//! auditory evidence. HumanMusic-specific interpretation belongs in a separate
//! `human_music::audition` layer (not here), so the observer can never "pass" by
//! reading the answer key. See `docs/HUMAN_MUSIC_AUDITORY_OBSERVER.md`.
//!
//! # One-directional
//!
//! Analysis only. Nothing here feeds back into synthesis or rewrites upstream history.
//! Phases 1–4 (mix/timbre/gesture/planning feedback) are explicitly out of scope until
//! the observer survives discriminating controls.
//!
//! # Determinism
//!
//! For a fixed PCM input and a fixed [`listener::ListeningContext`], every result here
//! is bit-reproducible: pure `f32`/`f64` arithmetic, no RNG, no global state.

pub mod filterbank;
pub mod listener;

pub use filterbank::ErbBank;
pub use listener::{ChannelModel, LevelCalibration, ListenerProfile, ListeningContext};
