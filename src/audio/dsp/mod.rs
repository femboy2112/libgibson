//! Self-synthesized DSP: synthesis primitives, drum voices and production effects.
//!
//! Everything here generates sound from code — **no sampled or prerecorded assets**.
//! Submodules:
//!
//! - [`osc`] — band-limited oscillators (sine, polyBLEP saw/pulse, triangle) and FM.
//! - [`env`] — ADSR and exponential envelopes.
//! - [`filter`] — a stable TPT/SVF state-variable filter (LP/HP/BP).
//! - [`drums`] — synthesized kick / snare / hat / clap voices.
//! - [`fx`] — equal-power pan, saturation, delay, chorus, an FDN reverb, a bus
//!   compressor and a peak limiter.
//!
//! Conventions shared across the DSP layer: audio is `f32`; per-voice generators expose a
//! `next()` / `process(x)` idiom advancing one sample at the ambient sample rate; nothing
//! allocates on the audio path once constructed.

pub mod drums;
pub mod env;
pub mod filter;
pub mod fx;
pub mod osc;
