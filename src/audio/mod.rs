//! Experimental Rust-only **audio realization axis** (`gibson::audio`).
//!
//! Audio is orthogonal to the visual pipeline. It is *not* owned by [`crate::Node`],
//! [`crate::Surface`], [`crate::Renderer`] or [`crate::TerminalSession`]; the terminal
//! renderer has no audio callbacks and the audio graph has no terminal buffers. The two
//! axes share only **semantic state** and **clocks**, never ownership:
//!
//! ```text
//!   VISUAL:  semantic state -> Node/Surface -> terminal renderer
//!   AUDIO:   semantic state -> Score/AudioEvents -> AudioGraph -> PCM -> device
//! ```
//!
//! # Layers
//!
//! - [`time`] — sample-accurate clock/transport and beat<->sample tempo mapping. Audio
//!   time is an integer [`SampleTime`] counter; we never accumulate elapsed-seconds
//!   floats, so there is no timing drift over a long render.
//! - [`buffer`] — the canonical internal representation: planar `f32` stereo blocks.
//! - [`dsp`] — self-synthesized synthesis primitives (oscillators, envelopes, filters,
//!   drums) and production effects. No sampled/prerecorded assets.
//! - [`graph`] — a bounded, inspectable processing graph with `Dialogue`/`Music`/`Sfx`/
//!   `Master` buses, prepared outside any realtime callback.
//! - [`human_music`] — HumanMusic: a fully self-synthesized procedural music + SFX
//!   system driven by LibGibson's semantic style/story state.
//! - [`render`] — the deterministic **offline renderer**: the same score/DSP path that a
//!   real device would drive, rendered to PCM with no audio hardware. Load-bearing for
//!   CI, tests, profiling and listening review.
//! - [`wav`] — a minimal pure-Rust RIFF/WAVE writer for offline listening artifacts.
//! - [`report`] — [`report::AudioReport`], the audio analogue of [`crate::FrameReport`].
//!
//! # Backends
//!
//! The core (DSP, scoring, offline render) is always-compiled pure Rust with **no new
//! mandatory dependencies**. A physical audio-device backend is an *optional* Cargo
//! feature (`audio-cpal`) so that `cargo test`, headless builds and server environments
//! never require ALSA/system audio libraries. See [`docs/HUMAN_MUSIC_ARCHITECTURE.md`].
//!
//! # Stability
//!
//! This module is **experimental and Rust-only**. It is intentionally *not* exposed
//! through the C ABI in this milestone; `GIBSON_ABI_VERSION` stays `1`. The model is
//! expected to evolve between releases.

pub mod buffer;
pub mod dsp;
pub mod graph;
pub mod human_music;
pub mod media;
pub mod render;
pub mod report;
pub mod time;
pub mod wav;

#[cfg(feature = "audio-cpal")]
pub mod device;

pub use buffer::StereoBlock;
pub use render::{OfflineRenderer, RenderCtx};
pub use report::AudioReport;
pub use time::{AudioClock, AudioTransport, SampleRate, SampleTime, TempoMap};
