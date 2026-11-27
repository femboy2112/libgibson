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
pub mod family;
pub mod math;
pub mod morph;
pub mod motion;
pub mod revision;
pub mod sha256;
pub mod skeleton;
pub mod skin;

pub use math::{Mat4, Quat, Transform, Vec3};
pub use morph::MorphTarget;
pub use revision::{
    compile, BodyFamily, BodyRecipe, CharacterRevision, CompileError, EvalError, FamilyRef,
    PoseInput, PosedMesh, RevisionId, SupportClass,
};
pub use skeleton::{Joint, Landmark, Skeleton};
pub use skin::SkinWeights;
