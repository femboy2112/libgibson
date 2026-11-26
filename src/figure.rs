//! Generic character mechanics. EXPERIMENTAL (feature `figure`).
//!
//! Slice 1 of the M5-B runtime: a synthetic skeleton with semantic landmarks, linear-blend
//! skinning with inverse binds, sparse morph targets, finite declarative clips sampled at an
//! explicit time, and [`CharacterRevision`](revision::CharacterRevision): an atomic,
//! deterministic, content-addressed compile of a body recipe. Interface contract:
//! `docs/FIGURE_CONTRACT.md`.
//!
//! Pure Rust, scalar `f32`, no `unsafe`, no SIMD, no dependency, no clock, no I/O.
#![forbid(unsafe_code)]

pub mod clip;
pub mod math;
pub mod morph;
pub mod revision;
pub mod sha256;
pub mod skeleton;
pub mod skin;
