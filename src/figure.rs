//! Generic character mechanics. EXPERIMENTAL (feature `figure`).
//!
//! Slice 1 of the M5-B runtime: a synthetic skeleton with semantic landmarks, linear-blend
//! skinning (linear blend, and dual quaternion via `SkinningMode`) with inverse binds, sparse
//! morph targets, finite declarative clips sampled at an explicit time, and [`CharacterRevision`](revision::CharacterRevision): an atomic,
//! deterministic, content-addressed compile of a body recipe. Interface contract:
//! `docs/FIGURE_CONTRACT.md`.
//!
//! `studio` is the M5-C `rev.v1` compiler (`RecipeV1` -> coupled deformation, garment fits, three
//! identities); see its module docs.
//!
//! Pure Rust, scalar `f32`, no `unsafe`, no SIMD, no dependency, no clock, no I/O (the one
//! exception is `studio::AssetSet::load_dir`).
#![forbid(unsafe_code)]

pub mod clip;
pub mod family;
pub mod ik;
pub mod living_matter;
pub mod math;
pub mod morph;
pub mod motion;
pub mod perform;
pub mod preview;
pub mod refit;
pub mod revision;
pub mod sha256;
pub mod skeleton;
pub mod skin;
pub mod studio;

pub use math::{DualQuat, Mat4, Quat, Transform, Vec3};
pub use morph::MorphTarget;
pub use perform::{
    perform, perform_with, Beat, GestureKind, Intent, IntentKind, Live, PerformError,
    PerformOptions, Performance, PerformanceScript, Pose, Rig, RigGeometry, RigMap, RigRoles,
    Style, Tape, TapeFiles, TapeIdentity,
};
pub use preview::{
    frame_bounds, frame_bounds_view, render_preview, PreviewCamera, PreviewImage, PreviewLight,
    PreviewMaterial, PreviewMesh, PreviewOptions,
};
pub use refit::{refit_skeleton, JointAnchor, Refit, RefitError};
pub use revision::{
    compile, BodyFamily, BodyRecipe, CharacterRevision, CompileError, EvalError, FamilyRef,
    PoseInput, PosedMesh, RevisionId, SupportClass,
};
pub use skeleton::{Joint, Landmark, Skeleton};
pub use skin::{SkinWeights, SkinningMode};
