//! `CharacterRevision`: an atomic, deterministic, content-addressed compile of a body recipe
//! (contract I-2). Slice 1 subset: neutral geometry, shaped skeleton and landmarks, inverse
//! binds, skin weights, morph targets and a content id. Colliders, garment fits and motion
//! calibration are typed placeholders, empty or defaulted.
//!
//! # Compile order (validation precedes any derived data; all-or-nothing)
//!
//! 1. hard limits (counts), 2. family reference match, 3. mesh (finite positions, triangle
//!    index buffer in range), 4. skeleton, 5. skin weights, 6. morph targets, 7. control
//!    definitions, 8. recipe values (unknown, duplicate, non-finite, out of range), then
//!    9. effects in FAMILY-DECLARED control order (so recipe order never matters), 10. the
//!    shaped skeleton, 11. inverse binds recomputed FROM THE SHAPED skeleton, 12. content id.
//!
//! # Canonical serialization (`gibson.figure.rev.v0`)
//!
//! Little-endian; floats as their IEEE-754 bits with `-0.0` canonicalized to `+0.0`; counts are `u32`; strings are a `u32`
//! byte length followed by UTF-8. In order: the 21-byte prefix `b"gibson.figure.rev.v0\0"`,
//! family ref (id string, `u32` version), vertex count then positions (x, y, z), index count
//! then indices (`u32`), joint count then joints (name, parent as `i32` with -1 for the root,
//! translation xyz, rotation xyzw, scale xyz), landmark count then landmarks (name, joint
//! `u32`, offset xyz), inverse-bind count then matrices (16 floats, column-major), skin
//! (vertex count, then per vertex the influence count as one byte and `(joint u32, weight)`
//! pairs), morph count then morphs (name, delta count, `(vertex u32, dx, dy, dz)`).
//! The placeholders (colliders, garment fits, calibration, support class) are not hashed in
//! Slice 1; adding them later changes the format version.

use std::fmt;

use super::math::{Mat4, Transform, Vec3};
use super::morph::{self, MorphError, MorphTarget, MAX_MORPH_TARGETS};
use super::sha256;
use super::skeleton::{Joint, Landmark, Skeleton, SkeletonError, MAX_JOINTS};
use super::skin::{SkinError, SkinWeights, SkinningMode, MAX_SKIN_VERTICES};

/// Hard cap on vertices in a body family.
pub const MAX_VERTICES: usize = MAX_SKIN_VERTICES;
/// Hard cap on entries in the triangle index buffer.
pub const MAX_INDICES: usize = 3 * MAX_VERTICES;
/// Hard cap on controls in a family and on values in a recipe.
pub const MAX_CONTROLS: usize = 256;
/// The versioned prefix of the canonical serialization.
pub const CANONICAL_PREFIX: &[u8] = b"gibson.figure.rev.v0\0";

/// A body family reference: `id@version`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct FamilyRef {
    pub id: String,
    pub version: u32,
}

impl FamilyRef {
    /// Construct a family reference.
    pub fn new(id: impl Into<String>, version: u32) -> FamilyRef {
        FamilyRef {
            id: id.into(),
            version,
        }
    }
}

impl fmt::Display for FamilyRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.id, self.version)
    }
}

/// A 32-byte SHA-256 content identifier of a revision's canonical bytes. It identifies bytes
/// only: it does not certify likeness, correctness or approval.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RevisionId(pub [u8; 32]);

impl RevisionId {
    /// Lower-case hex form.
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(64);
        for b in self.0 {
            s.push(char::from(b"0123456789abcdef"[usize::from(b >> 4)]));
            s.push(char::from(b"0123456789abcdef"[usize::from(b & 15)]));
        }
        s
    }
}

impl fmt::Debug for RevisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RevisionId({})", self.to_hex())
    }
}

impl fmt::Display for RevisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// Neutral geometry: positions and a triangle index buffer (3 indices per triangle).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct NeutralMesh {
    pub positions: Vec<Vec3>,
    pub indices: Vec<u32>,
}

impl NeutralMesh {
    /// Construct a mesh. Validation happens in [`compile`].
    pub fn new(positions: Vec<Vec3>, indices: Vec<u32>) -> NeutralMesh {
        NeutralMesh { positions, indices }
    }
}

/// What a control does to the body.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ControlEffect {
    /// Scale the bone from `joint` to each of its children: every direct child's rest
    /// translation (expressed in `joint`'s frame, so along the bone) is multiplied by the
    /// control value. A value of 1 leaves the bone unchanged. The mesh is not moved: pair the
    /// control with a morph if the geometry should follow.
    BoneLengthScale { joint: usize },
    /// Add the control value to the weight of morph target `target` (weights from several
    /// controls on the same target accumulate in control-declaration order).
    MorphWeight { target: usize },
}

/// A named control: a closed range, a default and its effects.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ControlDef {
    pub id: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub effects: Vec<ControlEffect>,
}

impl ControlDef {
    /// Construct a control definition. Validation happens in [`compile`].
    pub fn new(
        id: impl Into<String>,
        min: f32,
        max: f32,
        default: f32,
        effects: Vec<ControlEffect>,
    ) -> ControlDef {
        ControlDef {
            id: id.into(),
            min,
            max,
            default,
            effects,
        }
    }
}

/// Synthetic in-code body family data. Nothing here is validated until [`compile`], which is
/// the single validator (so hostile data can be handed to it and gets a typed error).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BodyFamily {
    pub family: FamilyRef,
    pub neutral: NeutralMesh,
    pub joints: Vec<Joint>,
    pub landmarks: Vec<Landmark>,
    /// Per-vertex `(joint, weight)` influences (raw; validated by `compile`).
    pub skin: Vec<Vec<(usize, f32)>>,
    pub morphs: Vec<MorphTarget>,
    pub controls: Vec<ControlDef>,
}

impl BodyFamily {
    /// A family with no morphs and no controls; add them with [`BodyFamily::with_morphs`] and
    /// [`BodyFamily::with_controls`].
    pub fn new(
        family: FamilyRef,
        neutral: NeutralMesh,
        joints: Vec<Joint>,
        landmarks: Vec<Landmark>,
        skin: Vec<Vec<(usize, f32)>>,
    ) -> BodyFamily {
        BodyFamily {
            family,
            neutral,
            joints,
            landmarks,
            skin,
            morphs: Vec::new(),
            controls: Vec::new(),
        }
    }

    /// Set the morph targets.
    pub fn with_morphs(mut self, morphs: Vec<MorphTarget>) -> BodyFamily {
        self.morphs = morphs;
        self
    }

    /// Set the controls.
    pub fn with_controls(mut self, controls: Vec<ControlDef>) -> BodyFamily {
        self.controls = controls;
        self
    }
}

/// A family reference plus `{control id -> value}`. Omitted controls take their default.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BodyRecipe {
    pub family: FamilyRef,
    pub values: Vec<(String, f32)>,
}

impl BodyRecipe {
    /// Construct a recipe.
    pub fn new(family: FamilyRef, values: Vec<(String, f32)>) -> BodyRecipe {
        BodyRecipe { family, values }
    }
}

/// How a support class was reached. Slice 1 only produces `Supported`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SupportClass {
    Supported,
    NeedsAdaptation,
    Experimental,
}

/// LATER: collision geometry. Empty in Slice 1.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Collider {}

/// Why a garment needs adaptation or does not fit: exactly one variant per check the code
/// performs (coordinator ruling #68; no catch-all). Slice 1 performs NO garment fit checks
/// (`garment_fits` is always empty), so this enum has no variants yet and
/// `FitStatus::NeedsAdaptation` / `FitStatus::Unfit` cannot be constructed. Each future fit
/// check adds its own variant, with a typed payload where one is needed.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum FitReason {}

/// The typed outcome of fitting one garment (contract law 5).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum FitStatus {
    Fit,
    /// The garment fits only after adaptation. A required garment in this state is still
    /// activatable by the host, with a visible warning (contract law 5, ruling #48 D1).
    NeedsAdaptation {
        reason: FitReason,
    },
    /// The garment does not fit; it carries no fit data.
    Unfit {
        reason: FitReason,
    },
}

/// LATER: a per-garment fit outcome. Empty in Slice 1.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct GarmentFit {
    pub garment_id: String,
    pub status: FitStatus,
}

impl GarmentFit {
    /// Builds a fit outcome from exactly its two fields. There is deliberately no `Default`:
    /// a fit status is never implied.
    pub fn new(garment_id: impl Into<String>, status: FitStatus) -> GarmentFit {
        GarmentFit {
            garment_id: garment_id.into(),
            status,
        }
    }
}

/// LATER: motion calibration. Defaulted in Slice 1.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct MotionCalibration {}

/// Why [`compile`] refused a recipe or family.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum CompileError {
    /// The recipe names a different family id or version.
    FamilyMismatch {
        recipe: FamilyRef,
        family: FamilyRef,
    },
    /// The recipe sets a control the family does not define.
    UnknownControl { id: String },
    /// The recipe sets one control twice.
    DuplicateControl { id: String },
    /// A recipe value lies outside the control's range.
    ControlOutOfRange {
        id: String,
        value: f32,
        min: f32,
        max: f32,
    },
    /// A joint's parent is not earlier in the list (a cycle or child-before-parent order).
    SkeletonCycle { joint: usize, parent: usize },
    /// A value in `part` is NaN or infinite (or a zero-length rotation).
    NonFinite { part: &'static str },
    /// The world bind matrix of `joint` cannot be inverted.
    DegenerateBind { joint: usize },
    /// The skin weights are invalid (see the inner error).
    SkinWeightInvalid(SkinError),
    /// A morph target does not fit the mesh (see the inner error).
    MorphTopologyMismatch(MorphError),
    /// A hard limit was exceeded.
    LimitExceeded {
        what: &'static str,
        count: usize,
        max: usize,
    },
    /// Two joints, landmarks, controls or morph targets share a name.
    DuplicateName { kind: &'static str, name: String },
    /// An id is outside the v0 alphabet: 1..=[`MAX_ID_LEN`] bytes of `[a-z0-9._-]`. `kind` is
    /// "family", "joint", "landmark", "control" or "morph target". Ids are content-addressed
    /// into `rev_id`, so non-ASCII ids are refused rather than allowed to alias (NFC vs NFD).
    InvalidId { kind: &'static str, id: String },
    /// The family is internally inconsistent in a way no other variant names (empty skeleton,
    /// bad index buffer, a control effect that points at nothing, an invalid control range).
    InvalidFamily { detail: &'static str },
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::FamilyMismatch { recipe, family } => {
                write!(f, "recipe is for family {recipe}, got family {family}")
            }
            CompileError::UnknownControl { id } => write!(f, "unknown control {id:?}"),
            CompileError::DuplicateControl { id } => {
                write!(f, "control {id:?} is set more than once")
            }
            CompileError::ControlOutOfRange {
                id,
                value,
                min,
                max,
            } => write!(f, "control {id:?} value {value} outside [{min}, {max}]"),
            CompileError::SkeletonCycle { joint, parent } => write!(
                f,
                "joint {joint} has parent {parent}, which is not earlier in the joint list"
            ),
            CompileError::NonFinite { part } => write!(f, "non-finite value in {part}"),
            CompileError::DegenerateBind { joint } => {
                write!(f, "world bind matrix of joint {joint} is not invertible")
            }
            CompileError::SkinWeightInvalid(e) => write!(f, "invalid skin weights: {e}"),
            CompileError::MorphTopologyMismatch(e) => write!(f, "invalid morph target: {e}"),
            CompileError::LimitExceeded { what, count, max } => {
                write!(f, "too many {what}: {count} (max {max})")
            }
            CompileError::DuplicateName { kind, name } => {
                write!(f, "duplicate {kind} name {name:?}")
            }
            CompileError::InvalidFamily { detail } => write!(f, "invalid family: {detail}"),
            CompileError::InvalidId { kind, id } => write!(
                f,
                "{kind} id {id:?} is not 1..={MAX_ID_LEN} bytes of [a-z0-9._-]"
            ),
        }
    }
}

impl std::error::Error for CompileError {}

fn map_skeleton_error(e: SkeletonError) -> CompileError {
    match e {
        SkeletonError::Empty => CompileError::InvalidFamily {
            detail: "skeleton has no joints",
        },
        SkeletonError::LimitExceeded { what, count, max } => {
            CompileError::LimitExceeded { what, count, max }
        }
        SkeletonError::ParentOrder { joint, parent } => {
            CompileError::SkeletonCycle { joint, parent }
        }
        SkeletonError::NonFinite { .. } => CompileError::NonFinite {
            part: "joint transform",
        },
        SkeletonError::DuplicateName { kind, name } => CompileError::DuplicateName { kind, name },
        SkeletonError::LandmarkJoint { .. } => CompileError::InvalidFamily {
            detail: "landmark refers to a missing joint",
        },
        SkeletonError::LandmarkNonFinite { .. } => CompileError::NonFinite {
            part: "landmark offset",
        },
        SkeletonError::DegenerateBind { joint } => CompileError::DegenerateBind { joint },
        SkeletonError::LengthMismatch { .. } => CompileError::InvalidFamily {
            detail: "skeleton length mismatch",
        },
    }
}

fn map_skin_error(e: SkinError) -> CompileError {
    match e {
        SkinError::LimitExceeded { count, max } => CompileError::LimitExceeded {
            what: "skinned vertices",
            count,
            max,
        },
        other => CompileError::SkinWeightInvalid(other),
    }
}

fn map_morph_error(e: MorphError) -> CompileError {
    match e {
        MorphError::LimitExceeded { what, count, max } => {
            CompileError::LimitExceeded { what, count, max }
        }
        MorphError::NonFinite { .. } => CompileError::NonFinite { part: "morph" },
        other => CompileError::MorphTopologyMismatch(other),
    }
}

/// A complete, coherent character revision. Immutable once built: there is no in-place
/// mutation API, and [`CharacterRevision::evaluate`] borrows it immutably.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CharacterRevision {
    pub rev_id: RevisionId,
    pub family: FamilyRef,
    /// The neutral geometry AFTER the recipe's morph controls (shaped neutral).
    pub neutral: NeutralMesh,
    /// The shaped skeleton (recipe bone-length controls applied), with landmarks.
    pub skeleton: Skeleton,
    /// One per joint, derived from the SHAPED skeleton's world bind matrices.
    pub inverse_binds: Vec<Mat4>,
    pub skin: SkinWeights,
    /// The family's declared morph targets, available to [`PoseInput`] by name.
    pub morphs: Vec<MorphTarget>,
    /// LATER: empty in Slice 1.
    pub colliders: Vec<Collider>,
    /// LATER: empty in Slice 1.
    pub garment_fits: Vec<GarmentFit>,
    /// LATER: defaulted in Slice 1.
    pub calibration: MotionCalibration,
    pub support: SupportClass,
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_f32(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&canonical_f32_bits(v).to_le_bytes());
}

/// The canonical bit pattern of a serialized `f32`: `-0.0` is written as `+0.0`, so an
/// ordinary negation such as `-(0.0)` cannot change `rev_id` (ruling #59). NaN never reaches
/// serialization, because it is a `CompileError`.
pub(crate) fn canonical_f32_bits(v: f32) -> u32 {
    if v == 0.0 {
        0
    } else {
        v.to_bits()
    }
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    put_u32(out, s.len() as u32);
    out.extend_from_slice(s.as_bytes());
}

fn put_vec3(out: &mut Vec<u8>, v: Vec3) {
    put_f32(out, v.x);
    put_f32(out, v.y);
    put_f32(out, v.z);
}

#[allow(clippy::too_many_arguments)]
fn canonical_bytes_of(
    family: &FamilyRef,
    neutral: &NeutralMesh,
    skeleton: &Skeleton,
    inverse_binds: &[Mat4],
    skin: &SkinWeights,
    morphs: &[MorphTarget],
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(CANONICAL_PREFIX);
    put_str(&mut out, &family.id);
    put_u32(&mut out, family.version);
    put_u32(&mut out, neutral.positions.len() as u32);
    for &p in &neutral.positions {
        put_vec3(&mut out, p);
    }
    put_u32(&mut out, neutral.indices.len() as u32);
    for &i in &neutral.indices {
        put_u32(&mut out, i);
    }
    put_u32(&mut out, skeleton.joints().len() as u32);
    for j in skeleton.joints() {
        put_str(&mut out, &j.name);
        let parent: i32 = match j.parent {
            Some(p) => p as i32,
            None => -1,
        };
        out.extend_from_slice(&parent.to_le_bytes());
        put_vec3(&mut out, j.rest.translation);
        let q = j.rest.rotation;
        for c in [q.x, q.y, q.z, q.w] {
            put_f32(&mut out, c);
        }
        put_vec3(&mut out, j.rest.scale);
    }
    put_u32(&mut out, skeleton.landmarks().len() as u32);
    for l in skeleton.landmarks() {
        put_str(&mut out, &l.name);
        put_u32(&mut out, l.joint as u32);
        put_vec3(&mut out, l.offset);
    }
    put_u32(&mut out, inverse_binds.len() as u32);
    for m in inverse_binds {
        for col in &m.cols {
            for &v in col {
                put_f32(&mut out, v);
            }
        }
    }
    put_u32(&mut out, skin.vertex_count() as u32);
    skin.write_canonical(&mut out);
    put_u32(&mut out, morphs.len() as u32);
    for m in morphs {
        m.write_canonical(&mut out);
    }
    out
}

impl CharacterRevision {
    /// The canonical serialization this revision's content id is computed over.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        canonical_bytes_of(
            &self.family,
            &self.neutral,
            &self.skeleton,
            &self.inverse_binds,
            &self.skin,
            &self.morphs,
        )
    }

    /// Recompute the content id from the current content (equals `rev_id` for any revision
    /// returned by [`compile`]).
    pub fn content_id(&self) -> RevisionId {
        RevisionId(sha256::digest(&self.canonical_bytes()))
    }

    /// Pose this revision: morph weights by name on the shaped neutral (in declared target
    /// order), then joint world transforms in parent-before-child order, then linear-blend
    /// skinning with this revision's inverse binds. Borrows `&self`, so `rev_id` cannot change.
    pub fn evaluate(&self, pose: &PoseInput) -> Result<PosedMesh, EvalError> {
        self.evaluate_with(pose, SkinningMode::Linear)
    }

    /// [`CharacterRevision::evaluate`] with an explicit [`SkinningMode`]. `Linear` is exactly
    /// `evaluate`. The mode is an evaluation argument for a v0 revision, so it never enters
    /// `rev_id`; a `rev.v1` revision carries it as hashed content (contract, "rev.v1 (DRAFT)").
    /// `DualQuat` can fail with `EvalError::Skin(SkinError::NonRigid { .. })` when the pose or
    /// the skeleton makes a referenced joint's skin matrix non-rigid.
    pub fn evaluate_with(
        &self,
        pose: &PoseInput,
        mode: SkinningMode,
    ) -> Result<PosedMesh, EvalError> {
        let mut weights = vec![0.0f32; self.morphs.len()];
        let mut set = vec![false; self.morphs.len()];
        for (name, w) in &pose.morph_weights {
            let Some(idx) = self.morphs.iter().position(|m| &m.name == name) else {
                return Err(EvalError::UnknownMorph { name: name.clone() });
            };
            if set[idx] {
                return Err(EvalError::DuplicateMorph { name: name.clone() });
            }
            if !w.is_finite() {
                return Err(EvalError::NonFinite {
                    what: "morph weight",
                });
            }
            set[idx] = true;
            weights[idx] = *w;
        }
        let positions = morph::apply_morphs(&self.neutral.positions, &self.morphs, &weights)
            .map_err(EvalError::Morph)?;

        let mut locals = self.skeleton.rest_locals();
        let mut overridden = vec![false; locals.len()];
        for &(j, t) in &pose.joint_locals {
            if j >= locals.len() {
                return Err(EvalError::JointOutOfRange { joint: j });
            }
            if overridden[j] {
                return Err(EvalError::DuplicateJoint { joint: j });
            }
            if !t.is_valid() {
                return Err(EvalError::InvalidTransform { joint: j });
            }
            overridden[j] = true;
            locals[j] = t;
        }
        let joint_world = self
            .skeleton
            .world_from_locals(&locals)
            .map_err(EvalError::Skeleton)?;
        let positions = self
            .skin
            .skin_with(mode, &positions, &joint_world, &self.inverse_binds)
            .map_err(EvalError::Skin)?;
        Ok(PosedMesh {
            positions,
            indices: self.neutral.indices.clone(),
            joint_world,
        })
    }
}

/// A pose request: sparse per-joint local-transform overrides and morph weights by name.
/// An override REPLACES the joint's shaped rest local transform entirely.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct PoseInput {
    pub joint_locals: Vec<(usize, Transform)>,
    pub morph_weights: Vec<(String, f32)>,
}

impl PoseInput {
    /// An empty pose (the rest pose, no morph weights).
    pub fn new() -> PoseInput {
        PoseInput::default()
    }

    /// Override joint `joint`'s local transform.
    pub fn with_joint(mut self, joint: usize, local: Transform) -> PoseInput {
        self.joint_locals.push((joint, local));
        self
    }

    /// Set the weight of the morph target called `name`.
    pub fn with_morph(mut self, name: impl Into<String>, weight: f32) -> PoseInput {
        self.morph_weights.push((name.into(), weight));
        self
    }
}

/// The result of posing a revision.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PosedMesh {
    /// Skinned vertex positions.
    pub positions: Vec<Vec3>,
    /// Triangle index buffer (3 per triangle), copied from the revision's neutral mesh so a
    /// renderer needs nothing but the posed mesh.
    pub indices: Vec<u32>,
    /// Posed joint world matrices, one per joint.
    pub joint_world: Vec<Mat4>,
}

/// Why [`CharacterRevision::evaluate`] refused a pose.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum EvalError {
    UnknownMorph {
        name: String,
    },
    DuplicateMorph {
        name: String,
    },
    JointOutOfRange {
        joint: usize,
    },
    DuplicateJoint {
        joint: usize,
    },
    /// An override transform is not finite or has a zero-length rotation.
    InvalidTransform {
        joint: usize,
    },
    NonFinite {
        what: &'static str,
    },
    Morph(MorphError),
    Skeleton(SkeletonError),
    Skin(SkinError),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::UnknownMorph { name } => write!(f, "unknown morph target {name:?}"),
            EvalError::DuplicateMorph { name } => {
                write!(f, "morph target {name:?} weighted more than once")
            }
            EvalError::JointOutOfRange { joint } => write!(f, "joint {joint} out of range"),
            EvalError::DuplicateJoint { joint } => {
                write!(f, "joint {joint} overridden more than once")
            }
            EvalError::InvalidTransform { joint } => {
                write!(f, "override for joint {joint} is not a valid transform")
            }
            EvalError::NonFinite { what } => write!(f, "{what} is not finite"),
            EvalError::Morph(e) => write!(f, "morph: {e}"),
            EvalError::Skeleton(e) => write!(f, "skeleton: {e}"),
            EvalError::Skin(e) => write!(f, "skin: {e}"),
        }
    }
}

impl std::error::Error for EvalError {}

/// Longest id, in bytes, accepted by [`compile`] (v0 alphabet, resolution A4).
pub const MAX_ID_LEN: usize = 64;

/// Allowlist byte check, never a regex: 1..=MAX_ID_LEN bytes, each in `[a-z0-9._-]`.
fn check_id(kind: &'static str, id: &str) -> Result<(), CompileError> {
    let ok = !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-'));
    if ok {
        Ok(())
    } else {
        Err(CompileError::InvalidId {
            kind,
            id: id.to_string(),
        })
    }
}

/// Compile `recipe` against `family` into a [`CharacterRevision`], or a typed error. All or
/// nothing: nothing partially built is observable. See the module docs for the exact order.
pub fn compile(
    recipe: &BodyRecipe,
    family: &BodyFamily,
) -> Result<CharacterRevision, CompileError> {
    // 1. Hard limits, before any derived allocation.
    let limit = |what: &'static str, count: usize, max: usize| {
        if count > max {
            Err(CompileError::LimitExceeded { what, count, max })
        } else {
            Ok(())
        }
    };
    limit("vertices", family.neutral.positions.len(), MAX_VERTICES)?;
    limit("indices", family.neutral.indices.len(), MAX_INDICES)?;
    limit("joints", family.joints.len(), MAX_JOINTS)?;
    limit("morph targets", family.morphs.len(), MAX_MORPH_TARGETS)?;
    limit("controls", family.controls.len(), MAX_CONTROLS)?;
    limit("recipe values", recipe.values.len(), MAX_CONTROLS)?;

    // 2. Family reference.
    if recipe.family != family.family {
        return Err(CompileError::FamilyMismatch {
            recipe: recipe.family.clone(),
            family: family.family.clone(),
        });
    }

    // 2b. Ids: the v0 ASCII alphabet (coordinator ruling #70, resolution A4).
    check_id("family", &family.family.id)?;
    for j in &family.joints {
        check_id("joint", &j.name)?;
    }
    for l in &family.landmarks {
        check_id("landmark", &l.name)?;
    }
    for c in &family.controls {
        check_id("control", &c.id)?;
    }
    for m in &family.morphs {
        check_id("morph target", &m.name)?;
    }

    // 3. Mesh.
    let vcount = family.neutral.positions.len();
    if !family.neutral.positions.iter().all(|p| p.is_finite()) {
        return Err(CompileError::NonFinite { part: "positions" });
    }
    if family.neutral.indices.len() % 3 != 0 {
        return Err(CompileError::InvalidFamily {
            detail: "index count is not a multiple of 3",
        });
    }
    if family
        .neutral
        .indices
        .iter()
        .any(|&i| (i as usize) >= vcount)
    {
        return Err(CompileError::InvalidFamily {
            detail: "triangle index out of range",
        });
    }

    // 4. Skeleton (unshaped).
    let base_skeleton = Skeleton::new(family.joints.clone(), family.landmarks.clone())
        .map_err(map_skeleton_error)?;

    // 5. Skin weights.
    if family.skin.len() != vcount {
        return Err(CompileError::SkinWeightInvalid(SkinError::LengthMismatch {
            what: "skin vertices",
            expected: vcount,
            got: family.skin.len(),
        }));
    }
    let skin =
        SkinWeights::new(&family.skin, base_skeleton.joint_count()).map_err(map_skin_error)?;

    // 6. Morph targets.
    morph::validate_targets(&family.morphs, vcount).map_err(map_morph_error)?;
    for (i, m) in family.morphs.iter().enumerate() {
        if family.morphs[..i].iter().any(|o| o.name == m.name) {
            return Err(CompileError::DuplicateName {
                kind: "morph target",
                name: m.name.clone(),
            });
        }
    }

    // 7. Control definitions.
    for (i, c) in family.controls.iter().enumerate() {
        if !(c.min.is_finite() && c.max.is_finite() && c.default.is_finite()) {
            return Err(CompileError::NonFinite {
                part: "control range",
            });
        }
        if c.min > c.max || c.default < c.min || c.default > c.max {
            return Err(CompileError::InvalidFamily {
                detail: "control range is empty or its default is outside it",
            });
        }
        if family.controls[..i].iter().any(|o| o.id == c.id) {
            return Err(CompileError::DuplicateName {
                kind: "control",
                name: c.id.clone(),
            });
        }
        for e in &c.effects {
            match *e {
                ControlEffect::BoneLengthScale { joint } if joint >= family.joints.len() => {
                    return Err(CompileError::InvalidFamily {
                        detail: "control bone-length effect names a missing joint",
                    });
                }
                ControlEffect::MorphWeight { target } if target >= family.morphs.len() => {
                    return Err(CompileError::InvalidFamily {
                        detail: "control morph effect names a missing target",
                    });
                }
                _ => {}
            }
        }
    }

    // 8. Recipe values.
    let mut values: Vec<f32> = family.controls.iter().map(|c| c.default).collect();
    let mut set = vec![false; family.controls.len()];
    for (id, v) in &recipe.values {
        let Some(ci) = family.controls.iter().position(|c| &c.id == id) else {
            return Err(CompileError::UnknownControl { id: id.clone() });
        };
        if set[ci] {
            return Err(CompileError::DuplicateControl { id: id.clone() });
        }
        if !v.is_finite() {
            return Err(CompileError::NonFinite {
                part: "control value",
            });
        }
        let c = &family.controls[ci];
        if *v < c.min || *v > c.max {
            return Err(CompileError::ControlOutOfRange {
                id: id.clone(),
                value: *v,
                min: c.min,
                max: c.max,
            });
        }
        set[ci] = true;
        values[ci] = *v;
    }

    // 9. Effects, in family-declared control order.
    let mut joints = family.joints.clone();
    let mut morph_weights = vec![0.0f32; family.morphs.len()];
    for (def, &v) in family.controls.iter().zip(values.iter()) {
        for e in &def.effects {
            match *e {
                ControlEffect::BoneLengthScale { joint } => {
                    // Fault injection (mutation control `ignored_skeleton_update`): the
                    // bone-length effect is ignored.
                    if cfg!(figure_mutant = "ignored_skeleton_update") {
                        continue;
                    }
                    for child in joints.iter_mut() {
                        if child.parent == Some(joint) {
                            child.rest.translation = child.rest.translation.scale(v);
                        }
                    }
                }
                ControlEffect::MorphWeight { target } => {
                    morph_weights[target] += v;
                }
            }
        }
    }
    let positions = morph::apply_morphs(&family.neutral.positions, &family.morphs, &morph_weights)
        .map_err(map_morph_error)?;

    // 10. Shaped skeleton (re-validated: scaling can overflow to non-finite).
    let shaped = Skeleton::new(joints, family.landmarks.clone()).map_err(map_skeleton_error)?;

    // 11. Inverse binds from the SHAPED skeleton.
    // Fault injection (mutation control `stale_bind`): binds come from the unshaped rest.
    let bind_source = if cfg!(figure_mutant = "stale_bind") {
        &base_skeleton
    } else {
        &shaped
    };
    let inverse_binds = bind_source.inverse_binds().map_err(map_skeleton_error)?;

    // 12. Assemble and content-address.
    let neutral = NeutralMesh::new(positions, family.neutral.indices.clone());
    let bytes = canonical_bytes_of(
        &family.family,
        &neutral,
        &shaped,
        &inverse_binds,
        &skin,
        &family.morphs,
    );
    let rev_id = RevisionId(sha256::digest(&bytes));
    Ok(CharacterRevision {
        rev_id,
        family: family.family.clone(),
        neutral,
        skeleton: shaped,
        inverse_binds,
        skin,
        morphs: family.morphs.clone(),
        colliders: Vec::new(),
        garment_fits: Vec::new(),
        calibration: MotionCalibration::default(),
        support: SupportClass::Supported,
    })
}
