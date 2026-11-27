//! Generic authored body families: an authored ORIGIN plus versioned, region-scoped controls,
//! and a REBIND step that re-derives everything that depends on the shaped surface.
//!
//! This layer carries no character data. A family is whatever its author put in it: neutral
//! positions with a fixed triangle topology, named vertex regions, controls whose sparse
//! per-side deltas live inside their declared region, optional combo correctives keyed on a
//! control pair, region locks and a mirror (symmetry) map. Two families with different authored
//! origins have different [`FamilyId`]s even when their controls are identical: a family is not
//! a recolour of another.
//!
//! # Evaluation (all-or-nothing, deterministic)
//!
//! `P = origin + Σᵢ |cᵢ|·Δᵢ[side(cᵢ)] + Σⱼ |ca|·|cb|·Cⱼ`, controls and correctives in DECLARED
//! order, per vertex, scalar `f32`, no clamp and no compensation. `side` is the negative side for
//! `c < 0` and the positive side for `c > 0`; `c = 0` contributes nothing. This is the same law
//! as the studio asset controls (`{name}.neg` / `{name}.pos` morphs weighted by `|c|`), so a
//! family can be loaded from such an asset without changing its numbers. Locked-region vertices
//! are copied from the origin and are bit-identical to it for every legal recipe.
//!
//! # Validation (the constructor is the only door)
//!
//! [`BodyFamily::new`] refuses, with a typed error naming the control: a control that touches a
//! locked vertex ([`FamilyError::LockedVertexTouched`]); a delta outside the control's declared
//! region ([`FamilyError::WrongRegion`], the "wrong-region morph"); a control with no effect at
//! all ([`FamilyError::IgnoredControl`], the "ignored parameter"); a control whose range reaches
//! a side that has no delta ([`FamilyError::IgnoredSide`]); out-of-range or duplicate vertices;
//! a non-involutive mirror map; mirror partners whose regions or deltas do not mirror.
//! Recipe values are refused when unknown, duplicate, non-finite or outside `[min, max]`.
//!
//! # Rebind
//!
//! [`BodyFamily::rebind`] takes the shaped body and a [`RigSpec`] (declared skeleton, joint
//! anchors, skin weights, compliant-region declarations) and derives, from the SHAPED surface:
//! joint bind positions (the mean of each anchor ring, in the parent's inverse-bind frame, as
//! the studio refit does), inverse binds, one collision capsule per bone from its dominantly
//! bound vertices, and compliant-region tags. Skin weights are carried over unchanged. The
//! result is a [`BoundRig`] stamped with the [`BodyRevId`] it was derived from; every accessor
//! that feeds simulation or skinning is guarded, so a rig derived from a different body
//! revision is refused with [`RebindError::StaleSkeleton`] rather than silently used.
//!
//! # Canonical bytes
//!
//! Little-endian; floats are IEEE-754 bits with `-0.0` canonicalised to `+0.0`; counts `u32`;
//! strings `u32` length plus UTF-8. Prefixes `gibson.figure.family.v0\0`,
//! `gibson.figure.family.body.v0\0` and `gibson.figure.family.rig.v0\0`. Identifiers identify
//! bytes only: they do not certify likeness, correctness or approval.

use std::collections::BTreeMap;
use std::fmt;

use super::math::{Mat4, Vec3};
use super::morph::{MorphError, MorphTarget};
use super::revision::{canonical_f32_bits, MAX_INDICES, MAX_VERTICES};
use super::sha256;
use super::skeleton::{Joint, Skeleton, SkeletonError};
use super::skin::{SkinError, SkinWeights};

/// Hard cap on controls in a family.
pub const MAX_FAMILY_CONTROLS: usize = 128;
/// Hard cap on regions in a family.
pub const MAX_FAMILY_REGIONS: usize = 256;
/// Hard cap on combo correctives in a family.
pub const MAX_CORRECTIVES: usize = 256;
/// Hard cap on vertices in one anchor group.
pub const MAX_ANCHOR_VERTICES: usize = 4096;
/// Tolerance for the mirror-delta check (absolute, per component).
pub const MIRROR_TOLERANCE: f32 = 1.0e-5;

/// Canonical prefix of a family id.
pub const FAMILY_PREFIX: &[u8] = b"gibson.figure.family.v0\0";
/// Canonical prefix of a shaped-body revision id.
pub const BODY_PREFIX: &[u8] = b"gibson.figure.family.body.v0\0";
/// Canonical prefix of a bound-rig id.
pub const RIG_PREFIX: &[u8] = b"gibson.figure.family.rig.v0\0";

/// The control ids every body family for the Apotheosis work must define. This is a SCHEMA
/// check (names and liveness), not a statement about how good the shapes are.
pub const REQUIRED_CONTROLS: &[&str] = &[
    "ribcage_width",
    "ribcage_depth",
    "bust_volume",
    "bust_projection",
    "bust_base",
    "bust_fullness",
    "waist_width",
    "waist_depth",
    "pelvis",
    "hip_flare",
    "glute_projection",
    "glute_fullness",
    "thigh_upper",
    "thigh_lower",
    "shoulder",
    "upper_arm",
    "calf",
    "posture",
];

fn hex32(b: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for &x in b {
        s.push(char::from(b"0123456789abcdef"[usize::from(x >> 4)]));
        s.push(char::from(b"0123456789abcdef"[usize::from(x & 15)]));
    }
    s
}

macro_rules! id32 {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(pub [u8; 32]);

        impl $name {
            /// Lower-case hex form.
            pub fn to_hex(&self) -> String {
                hex32(&self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.to_hex())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.to_hex())
            }
        }
    };
}

id32!(
    /// SHA-256 of a family's canonical bytes (origin, topology, regions, locks, mirror, controls,
    /// correctives, name and revision).
    FamilyId
);
id32!(
    /// SHA-256 of the origin alone (positions and topology): the "which body is this" part.
    OriginId
);
id32!(
    /// SHA-256 of a shaped body (family id plus shaped positions). A no-op edit has the same
    /// id as the unedited body.
    BodyRevId
);
id32!(
    /// SHA-256 of a bound rig (the body revision it was derived from plus everything derived).
    RigId
);

/// Which side of a control a delta list belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Applied for negative control values, weighted by `|c|`.
    Neg,
    /// Applied for positive control values, weighted by `c`.
    Pos,
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Side::Neg => "negative",
            Side::Pos => "positive",
        })
    }
}

/// A named set of vertices (sorted and unique after validation).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct FamilyRegion {
    pub name: String,
    pub vertices: Vec<u32>,
}

impl FamilyRegion {
    /// Construct a region. Validation is [`BodyFamily::new`].
    pub fn new(name: impl Into<String>, vertices: Vec<u32>) -> FamilyRegion {
        FamilyRegion {
            name: name.into(),
            vertices,
        }
    }
}

/// A control: a closed range, a default, a region and sparse per-side deltas.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct FamilyControl {
    pub id: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    /// The region this control may move. Every delta vertex must lie inside it.
    pub region: String,
    /// Deltas applied for negative values (weight `|c|`).
    pub neg: Vec<(u32, Vec3)>,
    /// Deltas applied for positive values (weight `c`).
    pub pos: Vec<(u32, Vec3)>,
    /// The control that mirrors this one across the family's mirror map, if any.
    pub mirror_partner: Option<String>,
}

impl FamilyControl {
    /// Construct a control with no mirror partner.
    pub fn new(
        id: impl Into<String>,
        min: f32,
        max: f32,
        default: f32,
        region: impl Into<String>,
        neg: Vec<(u32, Vec3)>,
        pos: Vec<(u32, Vec3)>,
    ) -> FamilyControl {
        FamilyControl {
            id: id.into(),
            min,
            max,
            default,
            region: region.into(),
            neg,
            pos,
            mirror_partner: None,
        }
    }

    /// Set the mirror partner.
    pub fn with_mirror_partner(mut self, partner: impl Into<String>) -> FamilyControl {
        self.mirror_partner = Some(partner.into());
        self
    }
}

/// A combo corrective keyed on a control pair: added with weight `|ca|·|cb|`.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Corrective {
    pub a: String,
    pub b: String,
    pub deltas: Vec<(u32, Vec3)>,
}

impl Corrective {
    /// Construct a corrective on the control pair `(a, b)`.
    pub fn new(a: impl Into<String>, b: impl Into<String>, deltas: Vec<(u32, Vec3)>) -> Corrective {
        Corrective {
            a: a.into(),
            b: b.into(),
            deltas,
        }
    }
}

/// Unvalidated authored family data. Hostile data is welcome: [`BodyFamily::new`] is the single
/// validator and answers with a typed error.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct FamilySpec {
    pub name: String,
    pub revision: u32,
    /// Neutral positions: the authored origin.
    pub origin: Vec<Vec3>,
    /// Triangle indices (fixed topology).
    pub indices: Vec<u32>,
    pub regions: Vec<FamilyRegion>,
    /// Names of locked regions. No control or corrective may list any of their vertices.
    pub locked: Vec<String>,
    /// Vertex mirror map (`mirror[v]` is the mirror image of `v`; an involution). Empty when the
    /// family declares no symmetry.
    pub mirror: Vec<u32>,
    pub controls: Vec<FamilyControl>,
    pub correctives: Vec<Corrective>,
}

impl FamilySpec {
    /// A spec with an origin and topology and nothing else; fill the public fields.
    pub fn new(
        name: impl Into<String>,
        revision: u32,
        origin: Vec<Vec3>,
        indices: Vec<u32>,
    ) -> FamilySpec {
        FamilySpec {
            name: name.into(),
            revision,
            origin,
            indices,
            regions: Vec::new(),
            locked: Vec::new(),
            mirror: Vec::new(),
            controls: Vec::new(),
            correctives: Vec::new(),
        }
    }
}

/// Why a family was refused.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum FamilyError {
    /// A hard limit was exceeded.
    LimitExceeded {
        what: &'static str,
        count: usize,
        max: usize,
    },
    /// The origin has no vertices.
    EmptyOrigin,
    /// An origin position is not finite.
    NonFiniteOrigin { vertex: usize },
    /// The index buffer length is not a multiple of 3 or an index is out of range.
    BadTopology { detail: &'static str },
    /// Two regions, controls or correctives share a name.
    DuplicateName { kind: &'static str, name: String },
    /// A control, lock or rig names a region that is not declared.
    UnknownRegion { by: String, region: String },
    /// A region lists no vertices.
    EmptyRegion { region: String },
    /// A region lists a vertex out of range or twice.
    BadRegion { region: String, vertex: u32 },
    /// A control's min/max/default is not finite, the range is empty, or the default is outside it.
    ControlDomain { control: String },
    /// A delta list is malformed (out-of-range vertex, duplicate vertex, non-finite delta).
    BadDeltas { owner: String, error: MorphError },
    /// IGNORED PARAMETER: the control has no non-zero delta on either side.
    IgnoredControl { control: String },
    /// The control's range reaches `side` but that side has no non-zero delta.
    IgnoredSide { control: String, side: Side },
    /// WRONG REGION: a delta addresses a vertex outside the control's declared region.
    WrongRegion { owner: String, vertex: u32 },
    /// A delta addresses a vertex of a locked region.
    LockedVertexTouched {
        owner: String,
        vertex: u32,
        region: String,
    },
    /// A corrective names a control that does not exist.
    UnknownControl { by: String, control: String },
    /// A corrective pairs a control with itself.
    CorrectiveSameControl { corrective: String },
    /// A corrective has no non-zero delta.
    IgnoredCorrective { corrective: String },
    /// The mirror map is the wrong length or not an involution on `0..n`.
    BadMirror { vertex: u32 },
    /// A mirror partner is declared without a mirror map, or is missing, or is not reciprocal.
    BadMirrorPair { control: String },
    /// Mirror partners' regions or deltas are not mirror images of each other.
    MirrorMismatch { control: String },
}

impl fmt::Display for FamilyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FamilyError::LimitExceeded { what, count, max } => {
                write!(f, "too many {what}: {count} (max {max})")
            }
            FamilyError::EmptyOrigin => write!(f, "origin has no vertices"),
            FamilyError::NonFiniteOrigin { vertex } => {
                write!(f, "origin vertex {vertex} is not finite")
            }
            FamilyError::BadTopology { detail } => write!(f, "bad topology: {detail}"),
            FamilyError::DuplicateName { kind, name } => write!(f, "duplicate {kind} {name:?}"),
            FamilyError::UnknownRegion { by, region } => {
                write!(f, "{by} names unknown region {region:?}")
            }
            FamilyError::EmptyRegion { region } => write!(f, "region {region:?} is empty"),
            FamilyError::BadRegion { region, vertex } => {
                write!(f, "region {region:?} has bad or duplicate vertex {vertex}")
            }
            FamilyError::ControlDomain { control } => {
                write!(f, "control {control:?} has an invalid min/max/default")
            }
            FamilyError::BadDeltas { owner, error } => write!(f, "{owner:?} deltas: {error}"),
            FamilyError::IgnoredControl { control } => {
                write!(
                    f,
                    "control {control:?} has no effect (all deltas are zero or absent)"
                )
            }
            FamilyError::IgnoredSide { control, side } => write!(
                f,
                "control {control:?} reaches its {side} side but that side has no effect"
            ),
            FamilyError::WrongRegion { owner, vertex } => write!(
                f,
                "{owner:?} moves vertex {vertex}, outside its declared region"
            ),
            FamilyError::LockedVertexTouched {
                owner,
                vertex,
                region,
            } => write!(
                f,
                "{owner:?} moves vertex {vertex}, which is in locked region {region:?}"
            ),
            FamilyError::UnknownControl { by, control } => {
                write!(f, "{by:?} names unknown control {control:?}")
            }
            FamilyError::CorrectiveSameControl { corrective } => {
                write!(f, "corrective {corrective:?} pairs a control with itself")
            }
            FamilyError::IgnoredCorrective { corrective } => {
                write!(f, "corrective {corrective:?} has no effect")
            }
            FamilyError::BadMirror { vertex } => {
                write!(f, "mirror map is not an involution at vertex {vertex}")
            }
            FamilyError::BadMirrorPair { control } => {
                write!(
                    f,
                    "control {control:?} has a missing or non-reciprocal mirror partner"
                )
            }
            FamilyError::MirrorMismatch { control } => {
                write!(
                    f,
                    "control {control:?} is not the mirror image of its partner"
                )
            }
        }
    }
}

impl std::error::Error for FamilyError {}

/// Why a recipe was refused at evaluation.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum EvalError {
    /// A value names a control the family does not define.
    UnknownControl { control: String },
    /// A control is given twice.
    DuplicateControl { control: String },
    /// A value is NaN or infinite.
    NonFinite { control: String },
    /// A value is outside `[min, max]`.
    OutOfRange {
        control: String,
        value: f32,
        min: f32,
        max: f32,
    },
    /// The shaped surface is not finite (overflowing deltas).
    NonFiniteResult { vertex: usize },
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::UnknownControl { control } => write!(f, "unknown control {control:?}"),
            EvalError::DuplicateControl { control } => {
                write!(f, "control {control:?} given twice")
            }
            EvalError::NonFinite { control } => write!(f, "control {control:?} is not finite"),
            EvalError::OutOfRange {
                control,
                value,
                min,
                max,
            } => write!(f, "control {control:?} = {value} is outside [{min}, {max}]"),
            EvalError::NonFiniteResult { vertex } => {
                write!(f, "shaped vertex {vertex} is not finite")
            }
        }
    }
}

impl std::error::Error for EvalError {}

/// Why a rebind, or use of a bound rig, was refused.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum RebindError {
    /// The shaped body belongs to a different family.
    ForeignBody { family: FamilyId, body: FamilyId },
    /// STALE SKELETON: the rig was derived from a different body revision than the one in hand.
    StaleSkeleton { bound_to: BodyRevId, got: BodyRevId },
    /// The rig's bytes differ from a fresh derivation for the same body revision.
    Tampered { expected: RigId, got: RigId },
    /// The skin weights do not match the family's vertex count (topology mismatch).
    TopologyMismatch {
        skin_vertices: usize,
        vertices: usize,
    },
    /// An anchor names a joint out of range, twice, or has no vertices.
    BadAnchor { joint: usize },
    /// An anchor names a vertex out of range or twice.
    BadAnchorVertex { joint: usize, vertex: u32 },
    /// A compliant declaration names a family region that does not exist, or repeats a name.
    BadCompliant { name: String },
    /// A limit was exceeded.
    LimitExceeded {
        what: &'static str,
        count: usize,
        max: usize,
    },
    /// The derived skeleton is invalid.
    Skeleton(SkeletonError),
    /// The skin weights or skinning call are invalid.
    Skin(SkinError),
    /// A derived value is not finite.
    NonFinite { what: &'static str },
}

impl fmt::Display for RebindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RebindError::ForeignBody { family, body } => {
                write!(f, "body belongs to family {body}, not {family}")
            }
            RebindError::StaleSkeleton { bound_to, got } => write!(
                f,
                "stale skeleton: rig was bound to body revision {bound_to}, but the body is {got}"
            ),
            RebindError::Tampered { expected, got } => {
                write!(f, "rig {got} differs from a fresh derivation {expected}")
            }
            RebindError::TopologyMismatch {
                skin_vertices,
                vertices,
            } => write!(
                f,
                "skin covers {skin_vertices} vertices, family has {vertices}"
            ),
            RebindError::BadAnchor { joint } => write!(f, "bad anchor for joint {joint}"),
            RebindError::BadAnchorVertex { joint, vertex } => {
                write!(f, "anchor of joint {joint} has bad vertex {vertex}")
            }
            RebindError::BadCompliant { name } => {
                write!(f, "bad compliant-region declaration {name:?}")
            }
            RebindError::LimitExceeded { what, count, max } => {
                write!(f, "too many {what}: {count} (max {max})")
            }
            RebindError::Skeleton(e) => write!(f, "skeleton: {e}"),
            RebindError::Skin(e) => write!(f, "skin: {e}"),
            RebindError::NonFinite { what } => write!(f, "derived {what} is not finite"),
        }
    }
}

impl std::error::Error for RebindError {}

// ---------------------------------------------------------------------------------------------
// Canonical byte helpers.
// ---------------------------------------------------------------------------------------------

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_f32(out: &mut Vec<u8>, v: f32) {
    put_u32(out, canonical_f32_bits(v));
}

fn put_vec3(out: &mut Vec<u8>, v: Vec3) {
    put_f32(out, v.x);
    put_f32(out, v.y);
    put_f32(out, v.z);
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    put_u32(out, s.len() as u32);
    out.extend_from_slice(s.as_bytes());
}

fn put_deltas(out: &mut Vec<u8>, d: &[(u32, Vec3)]) {
    put_u32(out, d.len() as u32);
    for &(v, p) in d {
        put_u32(out, v);
        put_vec3(out, p);
    }
}

fn put_mat(out: &mut Vec<u8>, m: &Mat4) {
    for col in &m.cols {
        for &v in col {
            put_f32(out, v);
        }
    }
}

fn origin_bytes(origin: &[Vec3], indices: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + origin.len() * 12 + indices.len() * 4);
    put_u32(&mut out, origin.len() as u32);
    for &p in origin {
        put_vec3(&mut out, p);
    }
    put_u32(&mut out, indices.len() as u32);
    for &i in indices {
        put_u32(&mut out, i);
    }
    out
}

fn has_effect(d: &[(u32, Vec3)]) -> bool {
    d.iter()
        .any(|&(_, p)| p.x != 0.0 || p.y != 0.0 || p.z != 0.0)
}

// ---------------------------------------------------------------------------------------------
// The family.
// ---------------------------------------------------------------------------------------------

/// A validated body family. Fields are private so the validated invariants (locks, regions,
/// liveness, mirror) cannot be broken afterwards.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyFamily {
    spec: FamilySpec,
    id: FamilyId,
    origin_id: OriginId,
    /// Per vertex: the locked region's index in `spec.regions`, or `u32::MAX`.
    locked_by: Vec<u32>,
}

impl BodyFamily {
    /// Validate and build. Order: limits, origin, topology, regions, locks, controls (domain,
    /// deltas, liveness, region membership, lock membership), correctives, mirror, mirror pairs;
    /// then the canonical bytes and ids. All-or-nothing.
    pub fn new(mut spec: FamilySpec) -> Result<BodyFamily, FamilyError> {
        let n = spec.origin.len();
        let lim = |what: &'static str, count: usize, max: usize| {
            if count > max {
                Err(FamilyError::LimitExceeded { what, count, max })
            } else {
                Ok(())
            }
        };
        lim("vertices", n, MAX_VERTICES)?;
        lim("indices", spec.indices.len(), MAX_INDICES)?;
        lim("controls", spec.controls.len(), MAX_FAMILY_CONTROLS)?;
        lim("regions", spec.regions.len(), MAX_FAMILY_REGIONS)?;
        lim("correctives", spec.correctives.len(), MAX_CORRECTIVES)?;
        if n == 0 {
            return Err(FamilyError::EmptyOrigin);
        }
        if let Some(vertex) = spec.origin.iter().position(|p| !p.is_finite()) {
            return Err(FamilyError::NonFiniteOrigin { vertex });
        }
        if spec.indices.len() % 3 != 0 {
            return Err(FamilyError::BadTopology {
                detail: "index count is not a multiple of 3",
            });
        }
        if spec.indices.iter().any(|&i| (i as usize) >= n) {
            return Err(FamilyError::BadTopology {
                detail: "index out of range",
            });
        }

        // Regions: unique names, in range, no duplicate vertices; stored sorted.
        for r in spec.regions.iter_mut() {
            if r.vertices.is_empty() {
                return Err(FamilyError::EmptyRegion {
                    region: r.name.clone(),
                });
            }
            let mut v = r.vertices.clone();
            v.sort_unstable();
            if let Some(&bad) = v.iter().find(|&&x| (x as usize) >= n) {
                return Err(FamilyError::BadRegion {
                    region: r.name.clone(),
                    vertex: bad,
                });
            }
            if let Some(w) = v.windows(2).find(|w| w[0] == w[1]) {
                return Err(FamilyError::BadRegion {
                    region: r.name.clone(),
                    vertex: w[0],
                });
            }
            r.vertices = v;
        }
        // Name uniqueness.
        for i in 0..spec.regions.len() {
            if spec.regions[..i]
                .iter()
                .any(|o| o.name == spec.regions[i].name)
            {
                return Err(FamilyError::DuplicateName {
                    kind: "region",
                    name: spec.regions[i].name.clone(),
                });
            }
        }
        let region_ix = |name: &str| spec.regions.iter().position(|r| r.name == name);

        // Locks.
        let mut locked_by = vec![u32::MAX; n];
        for name in &spec.locked {
            let Some(ri) = region_ix(name) else {
                return Err(FamilyError::UnknownRegion {
                    by: "lock".to_string(),
                    region: name.clone(),
                });
            };
            for &v in &spec.regions[ri].vertices {
                if locked_by[v as usize] == u32::MAX {
                    locked_by[v as usize] = ri as u32;
                }
            }
        }

        // Controls.
        for (ci, c) in spec.controls.iter().enumerate() {
            if spec.controls[..ci].iter().any(|o| o.id == c.id) {
                return Err(FamilyError::DuplicateName {
                    kind: "control",
                    name: c.id.clone(),
                });
            }
            if !(c.min.is_finite() && c.max.is_finite() && c.default.is_finite())
                || c.min > c.max
                || c.default < c.min
                || c.default > c.max
            {
                return Err(FamilyError::ControlDomain {
                    control: c.id.clone(),
                });
            }
            let Some(ri) = region_ix(&c.region) else {
                return Err(FamilyError::UnknownRegion {
                    by: c.id.clone(),
                    region: c.region.clone(),
                });
            };
            for (side, list) in [(Side::Neg, &c.neg), (Side::Pos, &c.pos)] {
                MorphTarget::new(format!("{}.{side}", c.id), list.clone())
                    .validate(ci, n)
                    .map_err(|error| FamilyError::BadDeltas {
                        owner: c.id.clone(),
                        error,
                    })?;
                check_membership(&c.id, list, &spec.regions[ri].vertices, &locked_by, &spec)?;
            }
            if !has_effect(&c.neg) && !has_effect(&c.pos) {
                return Err(FamilyError::IgnoredControl {
                    control: c.id.clone(),
                });
            }
            if c.min < 0.0 && !has_effect(&c.neg) {
                return Err(FamilyError::IgnoredSide {
                    control: c.id.clone(),
                    side: Side::Neg,
                });
            }
            if c.max > 0.0 && !has_effect(&c.pos) {
                return Err(FamilyError::IgnoredSide {
                    control: c.id.clone(),
                    side: Side::Pos,
                });
            }
        }

        // Correctives: confined to the union of the pair's regions, never in a lock.
        for (ki, k) in spec.correctives.iter().enumerate() {
            let label = format!("corrective[{ki}]({},{})", k.a, k.b);
            if spec.correctives[..ki]
                .iter()
                .any(|o| (o.a == k.a && o.b == k.b) || (o.a == k.b && o.b == k.a))
            {
                return Err(FamilyError::DuplicateName {
                    kind: "corrective",
                    name: label,
                });
            }
            if k.a == k.b {
                return Err(FamilyError::CorrectiveSameControl { corrective: label });
            }
            let mut union: Vec<u32> = Vec::new();
            for name in [&k.a, &k.b] {
                let Some(c) = spec.controls.iter().find(|c| &c.id == name) else {
                    return Err(FamilyError::UnknownControl {
                        by: label,
                        control: name.clone(),
                    });
                };
                if let Some(ri) = region_ix(&c.region) {
                    union.extend_from_slice(&spec.regions[ri].vertices);
                }
            }
            union.sort_unstable();
            union.dedup();
            MorphTarget::new(label.clone(), k.deltas.clone())
                .validate(ki, n)
                .map_err(|error| FamilyError::BadDeltas {
                    owner: label.clone(),
                    error,
                })?;
            check_membership(&label, &k.deltas, &union, &locked_by, &spec)?;
            if !has_effect(&k.deltas) {
                return Err(FamilyError::IgnoredCorrective { corrective: label });
            }
        }

        // Mirror map and pairs.
        if !spec.mirror.is_empty() {
            if spec.mirror.len() != n {
                return Err(FamilyError::BadMirror { vertex: 0 });
            }
            for (v, &m) in spec.mirror.iter().enumerate() {
                if (m as usize) >= n || spec.mirror[m as usize] as usize != v {
                    return Err(FamilyError::BadMirror { vertex: v as u32 });
                }
            }
        }
        for c in &spec.controls {
            let Some(pname) = &c.mirror_partner else {
                continue;
            };
            let bad = || FamilyError::BadMirrorPair {
                control: c.id.clone(),
            };
            let mismatch = || FamilyError::MirrorMismatch {
                control: c.id.clone(),
            };
            if spec.mirror.is_empty() {
                return Err(bad());
            }
            let Some(p) = spec.controls.iter().find(|o| &o.id == pname) else {
                return Err(bad());
            };
            if p.mirror_partner.as_deref() != Some(c.id.as_str()) {
                return Err(bad());
            }
            if c.min != p.min || c.max != p.max || c.default != p.default {
                return Err(mismatch());
            }
            let (Some(ra), Some(rb)) = (region_ix(&c.region), region_ix(&p.region)) else {
                return Err(bad());
            };
            let mut mapped: Vec<u32> = spec.regions[ra]
                .vertices
                .iter()
                .map(|&v| spec.mirror[v as usize])
                .collect();
            mapped.sort_unstable();
            if mapped != spec.regions[rb].vertices {
                return Err(mismatch());
            }
            for (mine, theirs) in [(&c.neg, &p.neg), (&c.pos, &p.pos)] {
                if !deltas_mirror(mine, theirs, &spec.mirror) {
                    return Err(mismatch());
                }
            }
        }

        // Canonical bytes and ids.
        let ob = origin_bytes(&spec.origin, &spec.indices);
        let origin_id = OriginId(sha256::digest(&ob));
        let mut out = Vec::new();
        out.extend_from_slice(FAMILY_PREFIX);
        put_str(&mut out, &spec.name);
        put_u32(&mut out, spec.revision);
        out.extend_from_slice(&ob);
        put_u32(&mut out, spec.regions.len() as u32);
        for r in &spec.regions {
            put_str(&mut out, &r.name);
            put_u32(&mut out, r.vertices.len() as u32);
            for &v in &r.vertices {
                put_u32(&mut out, v);
            }
        }
        put_u32(&mut out, spec.locked.len() as u32);
        for l in &spec.locked {
            put_str(&mut out, l);
        }
        put_u32(&mut out, spec.mirror.len() as u32);
        for &m in &spec.mirror {
            put_u32(&mut out, m);
        }
        put_u32(&mut out, spec.controls.len() as u32);
        for c in &spec.controls {
            put_str(&mut out, &c.id);
            put_f32(&mut out, c.min);
            put_f32(&mut out, c.max);
            put_f32(&mut out, c.default);
            put_str(&mut out, &c.region);
            put_deltas(&mut out, &c.neg);
            put_deltas(&mut out, &c.pos);
            match &c.mirror_partner {
                Some(p) => {
                    out.push(1);
                    put_str(&mut out, p);
                }
                None => out.push(0),
            }
        }
        put_u32(&mut out, spec.correctives.len() as u32);
        for k in &spec.correctives {
            put_str(&mut out, &k.a);
            put_str(&mut out, &k.b);
            put_deltas(&mut out, &k.deltas);
        }
        let id = FamilyId(sha256::digest(&out));
        Ok(BodyFamily {
            spec,
            id,
            origin_id,
            locked_by,
        })
    }

    /// The family id (hash of the whole authored definition).
    pub fn id(&self) -> FamilyId {
        self.id
    }

    /// The origin id (hash of the neutral positions and topology only).
    pub fn origin_id(&self) -> OriginId {
        self.origin_id
    }

    /// Family display name.
    pub fn name(&self) -> &str {
        &self.spec.name
    }

    /// Family revision number (part of the id).
    pub fn revision(&self) -> u32 {
        self.spec.revision
    }

    /// The validated definition (read only).
    pub fn spec(&self) -> &FamilySpec {
        &self.spec
    }

    /// The neutral positions.
    pub fn origin(&self) -> &[Vec3] {
        &self.spec.origin
    }

    /// The triangle indices.
    pub fn indices(&self) -> &[u32] {
        &self.spec.indices
    }

    /// Vertex count.
    pub fn vertex_count(&self) -> usize {
        self.spec.origin.len()
    }

    /// The control `id`.
    pub fn control(&self, id: &str) -> Option<&FamilyControl> {
        self.spec.controls.iter().find(|c| c.id == id)
    }

    /// The vertices of region `name` (sorted).
    pub fn region(&self, name: &str) -> Option<&[u32]> {
        self.spec
            .regions
            .iter()
            .find(|r| r.name == name)
            .map(|r| r.vertices.as_slice())
    }

    /// Whether vertex `v` is in a locked region.
    pub fn is_locked(&self, v: u32) -> bool {
        self.locked_by
            .get(v as usize)
            .is_some_and(|&r| r != u32::MAX)
    }

    /// The mirror partner of control `id`, when declared.
    pub fn mirror_partner(&self, id: &str) -> Option<&str> {
        self.control(id)?.mirror_partner.as_deref()
    }

    /// Check the required control schema ([`REQUIRED_CONTROLS`]). Returns the missing ids.
    /// Liveness (non-zero deltas, region membership) is already guaranteed by construction.
    pub fn check_required_controls(&self) -> Result<(), Vec<&'static str>> {
        let missing: Vec<&'static str> = REQUIRED_CONTROLS
            .iter()
            .copied()
            .filter(|id| self.control(id).is_none())
            .collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }

    /// Coupling of two controls: the cosine similarity of their displacement fields at the
    /// control's maximum (positive side), in `[-1, 1]`. A value near 1 means the two controls
    /// are the same shape under two names. `None` for an unknown id or a zero field.
    pub fn coupling(&self, a: &str, b: &str) -> Option<f32> {
        let field = |c: &FamilyControl| -> BTreeMap<u32, Vec3> {
            c.pos.iter().map(|&(v, d)| (v, d)).collect()
        };
        let (fa, fb) = (field(self.control(a)?), field(self.control(b)?));
        let (mut dot, mut na, mut nb) = (0.0f64, 0.0f64, 0.0f64);
        for (v, da) in &fa {
            let da = [f64::from(da.x), f64::from(da.y), f64::from(da.z)];
            na += da.iter().map(|x| x * x).sum::<f64>();
            if let Some(db) = fb.get(v) {
                dot += da[0] * f64::from(db.x) + da[1] * f64::from(db.y) + da[2] * f64::from(db.z);
            }
        }
        for db in fb.values() {
            nb += f64::from(db.x).powi(2) + f64::from(db.y).powi(2) + f64::from(db.z).powi(2);
        }
        if na == 0.0 || nb == 0.0 {
            return None;
        }
        Some((dot / (na.sqrt() * nb.sqrt())) as f32)
    }

    /// Resolve a recipe: omitted controls take their default; unknown, duplicate, non-finite or
    /// out-of-range values are errors. Returns one value per control in DECLARED order.
    pub fn resolve(&self, values: &[(String, f32)]) -> Result<Vec<f32>, EvalError> {
        let mut out: Vec<Option<f32>> = vec![None; self.spec.controls.len()];
        for (name, v) in values {
            let Some(ci) = self.spec.controls.iter().position(|c| &c.id == name) else {
                return Err(EvalError::UnknownControl {
                    control: name.clone(),
                });
            };
            if out[ci].is_some() {
                return Err(EvalError::DuplicateControl {
                    control: name.clone(),
                });
            }
            let c = &self.spec.controls[ci];
            if !v.is_finite() {
                return Err(EvalError::NonFinite {
                    control: name.clone(),
                });
            }
            if *v < c.min || *v > c.max {
                return Err(EvalError::OutOfRange {
                    control: name.clone(),
                    value: *v,
                    min: c.min,
                    max: c.max,
                });
            }
            out[ci] = Some(*v);
        }
        Ok(out
            .into_iter()
            .zip(self.spec.controls.iter())
            .map(|(v, c)| v.unwrap_or(c.default))
            .collect())
    }

    /// Evaluate a recipe into a [`ShapedBody`] (module docs give the law).
    pub fn evaluate(&self, values: &[(String, f32)]) -> Result<ShapedBody, EvalError> {
        let resolved = self.resolve(values)?;
        let mut p = self.spec.origin.clone();
        for (c, &v) in self.spec.controls.iter().zip(resolved.iter()) {
            let list = if v < 0.0 {
                &c.neg
            } else if v > 0.0 {
                &c.pos
            } else {
                continue;
            };
            let w = v.abs();
            for &(vi, d) in list {
                let q = p[vi as usize];
                p[vi as usize] = Vec3::new(q.x + w * d.x, q.y + w * d.y, q.z + w * d.z);
            }
        }
        for k in &self.spec.correctives {
            let wa = self.weight_of(&resolved, &k.a);
            let wb = self.weight_of(&resolved, &k.b);
            if wa == 0.0 || wb == 0.0 {
                continue;
            }
            let w = wa * wb;
            for &(vi, d) in &k.deltas {
                let q = p[vi as usize];
                p[vi as usize] = Vec3::new(q.x + w * d.x, q.y + w * d.y, q.z + w * d.z);
            }
        }
        // Belt and braces: validation already forbids any delta on a locked vertex, so this
        // copy changes nothing; it keeps the lock guarantee independent of the loops above.
        for (v, &l) in self.locked_by.iter().enumerate() {
            if l != u32::MAX {
                p[v] = self.spec.origin[v];
            }
        }
        if let Some(vertex) = p.iter().position(|q| !q.is_finite()) {
            return Err(EvalError::NonFiniteResult { vertex });
        }
        let mut out = Vec::with_capacity(BODY_PREFIX.len() + 32 + p.len() * 12);
        out.extend_from_slice(BODY_PREFIX);
        out.extend_from_slice(&self.id.0);
        put_u32(&mut out, p.len() as u32);
        for &q in &p {
            put_vec3(&mut out, q);
        }
        Ok(ShapedBody {
            family: self.id,
            rev: BodyRevId(sha256::digest(&out)),
            positions: p,
            values: resolved,
        })
    }

    fn weight_of(&self, resolved: &[f32], id: &str) -> f32 {
        self.spec
            .controls
            .iter()
            .position(|c| c.id == id)
            .map_or(0.0, |i| resolved[i].abs())
    }

    /// Rebind a rig to a shaped body (module docs, "Rebind"). Complete or an error.
    pub fn rebind(&self, body: &ShapedBody, spec: &RigSpec) -> Result<BoundRig, RebindError> {
        if body.family != self.id {
            return Err(RebindError::ForeignBody {
                family: self.id,
                body: body.family,
            });
        }
        let n = self.vertex_count();
        if spec.skin.vertex_count() != n {
            return Err(RebindError::TopologyMismatch {
                skin_vertices: spec.skin.vertex_count(),
                vertices: n,
            });
        }
        let joints = spec.skeleton.joints();
        if spec.skin.joint_count() != joints.len() {
            return Err(RebindError::Skin(SkinError::LengthMismatch {
                what: "skin joint count",
                expected: joints.len(),
                got: spec.skin.joint_count(),
            }));
        }

        // 1. Joint positions from the anchor rings of the SHAPED surface; inverse binds.
        let mut centroids: Vec<Option<Vec3>> = vec![None; joints.len()];
        let mut total = 0usize;
        for a in &spec.anchors {
            if a.joint >= joints.len()
                || centroids[a.joint].is_some()
                || a.vertices.is_empty()
                || a.vertices.len() > MAX_ANCHOR_VERTICES
            {
                return Err(RebindError::BadAnchor { joint: a.joint });
            }
            total += a.vertices.len();
            if total > 16 * MAX_ANCHOR_VERTICES {
                return Err(RebindError::LimitExceeded {
                    what: "anchor vertices",
                    count: total,
                    max: 16 * MAX_ANCHOR_VERTICES,
                });
            }
            let mut sorted = a.vertices.clone();
            sorted.sort_unstable();
            if let Some(&bad) = sorted.iter().find(|&&v| (v as usize) >= n) {
                return Err(RebindError::BadAnchorVertex {
                    joint: a.joint,
                    vertex: bad,
                });
            }
            if let Some(w) = sorted.windows(2).find(|w| w[0] == w[1]) {
                return Err(RebindError::BadAnchorVertex {
                    joint: a.joint,
                    vertex: w[0],
                });
            }
            let mut s = Vec3::ZERO;
            for &v in &a.vertices {
                s = s.add(body.positions[v as usize]);
            }
            let cnt = a.vertices.len() as f32;
            centroids[a.joint] = Some(Vec3::new(s.x / cnt, s.y / cnt, s.z / cnt));
        }
        let mut out_joints: Vec<Joint> = Vec::with_capacity(joints.len());
        let mut world: Vec<Mat4> = Vec::with_capacity(joints.len());
        let mut ibs: Vec<Mat4> = Vec::with_capacity(joints.len());
        for (i, j) in joints.iter().enumerate() {
            let mut rest = j.rest;
            if let Some(c) = centroids[i] {
                rest.translation = match j.parent {
                    Some(p) => ibs[p].transform_point(c),
                    None => c,
                };
            }
            let local = Mat4::from_transform(&rest);
            let w = match j.parent {
                Some(p) => world[p].mul(&local),
                None => local,
            };
            let ib = w
                .inverse()
                .ok_or(RebindError::Skeleton(SkeletonError::DegenerateBind {
                    joint: i,
                }))?;
            out_joints.push(Joint::new(j.name.clone(), j.parent, rest));
            world.push(w);
            ibs.push(ib);
        }
        let skeleton = Skeleton::new(out_joints, spec.skeleton.landmarks().to_vec())
            .map_err(RebindError::Skeleton)?;
        if !world.iter().chain(ibs.iter()).all(Mat4::is_finite) {
            return Err(RebindError::NonFinite { what: "bind" });
        }
        let joint_pos: Vec<Vec3> = world
            .iter()
            .map(|w| w.transform_point(Vec3::ZERO))
            .collect();

        // 2. Dominant joint per vertex (first maximum weight; skin order is declared order).
        let mut dominant: Vec<usize> = Vec::with_capacity(n);
        for v in 0..n {
            let inf = spec.skin.influences(v).unwrap_or_default();
            let mut best = (0usize, -1.0f32);
            for &(j, w) in &inf {
                if w > best.1 {
                    best = (j, w);
                }
            }
            dominant.push(best.0);
        }

        // 3. One capsule per bone that has bound vertices.
        let mut colliders: Vec<Capsule> = Vec::new();
        for (ji, j) in joints.iter().enumerate() {
            let bound: Vec<u32> = (0..n as u32)
                .filter(|&v| dominant[v as usize] == ji)
                .collect();
            if bound.is_empty() {
                continue;
            }
            let a = joint_pos[ji];
            let child = joints.iter().position(|c| c.parent == Some(ji));
            let b = match child {
                Some(c) => joint_pos[c],
                None => {
                    // Leaf: extend along parent -> joint to the farthest bound projection.
                    let dir = match j.parent {
                        Some(p) => a.sub(joint_pos[p]),
                        None => Vec3::new(0.0, 0.0, 1.0),
                    };
                    let len = dir.length();
                    if len > 0.0 {
                        let u = dir.scale(1.0 / len);
                        let mut far = 0.0f32;
                        for &v in &bound {
                            let t = body.positions[v as usize].sub(a).dot(u);
                            if t > far {
                                far = t;
                            }
                        }
                        a.add(u.scale(far))
                    } else {
                        a
                    }
                }
            };
            let mut radius = 0.0f32;
            for &v in &bound {
                let d = dist_point_segment(body.positions[v as usize], a, b);
                if d > radius {
                    radius = d;
                }
            }
            if !(a.is_finite() && b.is_finite() && radius.is_finite()) {
                return Err(RebindError::NonFinite { what: "capsule" });
            }
            colliders.push(Capsule {
                joint: ji,
                a,
                b,
                radius,
                bound_vertices: bound.len() as u32,
            });
        }

        // 4. Compliant-region tags from family regions, re-measured on the shaped surface.
        let mut compliant: Vec<CompliantTag> = Vec::with_capacity(spec.compliant.len());
        for (name, region) in &spec.compliant {
            if compliant.iter().any(|t| &t.name == name) {
                return Err(RebindError::BadCompliant { name: name.clone() });
            }
            let Some(verts) = self.region(region) else {
                return Err(RebindError::BadCompliant { name: name.clone() });
            };
            let mut counts: BTreeMap<usize, usize> = BTreeMap::new();
            let mut s = Vec3::ZERO;
            for &v in verts {
                *counts.entry(dominant[v as usize]).or_insert(0) += 1;
                s = s.add(body.positions[v as usize]);
            }
            let cnt = verts.len() as f32;
            let centroid = Vec3::new(s.x / cnt, s.y / cnt, s.z / cnt);
            let mut best = (0usize, 0usize);
            for (&j, &c) in &counts {
                if c > best.1 {
                    best = (j, c);
                }
            }
            let mut radius = 0.0f32;
            let mut half_extent = Vec3::ZERO;
            for &v in verts {
                let o = body.positions[v as usize].sub(centroid);
                let d = o.length();
                if d > radius {
                    radius = d;
                }
                half_extent = Vec3::new(
                    half_extent.x.max(o.x.abs()),
                    half_extent.y.max(o.y.abs()),
                    half_extent.z.max(o.z.abs()),
                );
            }
            compliant.push(CompliantTag {
                name: name.clone(),
                region: region.clone(),
                joint: best.0,
                vertices: verts.to_vec(),
                centroid,
                local_centroid: ibs[best.0].transform_point(centroid),
                radius,
                half_extent,
            });
        }

        let mut rig = BoundRig {
            body_rev: body.rev,
            skeleton,
            inverse_binds: ibs,
            skin: spec.skin.clone(),
            colliders,
            compliant,
            id: RigId([0; 32]),
        };
        rig.id = rig.compute_id();
        Ok(rig)
    }
}

fn check_membership(
    owner: &str,
    deltas: &[(u32, Vec3)],
    allowed: &[u32],
    locked_by: &[u32],
    spec: &FamilySpec,
) -> Result<(), FamilyError> {
    for &(v, _) in deltas {
        let l = locked_by[v as usize];
        if l != u32::MAX {
            return Err(FamilyError::LockedVertexTouched {
                owner: owner.to_string(),
                vertex: v,
                region: spec.regions[l as usize].name.clone(),
            });
        }
        if allowed.binary_search(&v).is_err() {
            return Err(FamilyError::WrongRegion {
                owner: owner.to_string(),
                vertex: v,
            });
        }
    }
    Ok(())
}

/// Do `theirs` equal `mine` reflected through the vertex mirror map (x negated)?
fn deltas_mirror(mine: &[(u32, Vec3)], theirs: &[(u32, Vec3)], mirror: &[u32]) -> bool {
    let tm: BTreeMap<u32, Vec3> = theirs.iter().copied().collect();
    let mm: BTreeMap<u32, Vec3> = mine.iter().copied().collect();
    let near = |a: f32, b: f32| (a - b).abs() <= MIRROR_TOLERANCE;
    for (&v, d) in &mm {
        let Some(t) = tm.get(&mirror[v as usize]) else {
            if d.x == 0.0 && d.y == 0.0 && d.z == 0.0 {
                continue;
            }
            return false;
        };
        if !(near(t.x, -d.x) && near(t.y, d.y) && near(t.z, d.z)) {
            return false;
        }
    }
    // Any vertex only the partner moves must be a zero delta.
    for (&v, t) in &tm {
        if !mm.contains_key(&mirror[v as usize]) && (t.x != 0.0 || t.y != 0.0 || t.z != 0.0) {
            return false;
        }
    }
    true
}

fn dist_point_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b.sub(a);
    let l2 = ab.dot(ab);
    let t = if l2 > 0.0 {
        (p.sub(a).dot(ab) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.sub(a.add(ab.scale(t))).length()
}

// ---------------------------------------------------------------------------------------------
// The shaped body.
// ---------------------------------------------------------------------------------------------

/// An evaluated body: positions, the resolved control values and the content id.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapedBody {
    family: FamilyId,
    rev: BodyRevId,
    positions: Vec<Vec3>,
    values: Vec<f32>,
}

impl ShapedBody {
    /// The family this body was evaluated from.
    pub fn family(&self) -> FamilyId {
        self.family
    }

    /// The body revision id (family id plus shaped positions).
    pub fn rev_id(&self) -> BodyRevId {
        self.rev
    }

    /// The shaped positions.
    pub fn positions(&self) -> &[Vec3] {
        &self.positions
    }

    /// Resolved control values, in the family's declared control order.
    pub fn values(&self) -> &[f32] {
        &self.values
    }
}

// ---------------------------------------------------------------------------------------------
// Rig input and output.
// ---------------------------------------------------------------------------------------------

/// A joint's anchor ring: the vertices whose SHAPED mean is the joint's bind position.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct JointAnchor {
    pub joint: usize,
    pub vertices: Vec<u32>,
}

impl JointAnchor {
    /// Construct an anchor; validation is [`BodyFamily::rebind`].
    pub fn new(joint: usize, vertices: Vec<u32>) -> JointAnchor {
        JointAnchor { joint, vertices }
    }
}

/// What a rebind needs besides the shaped body: the declared skeleton (rest rotations and
/// scales are kept; anchored joints' translations are re-derived), anchors, skin weights, and
/// compliant-region declarations `(tag name, family region)`.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RigSpec {
    pub skeleton: Skeleton,
    pub anchors: Vec<JointAnchor>,
    pub skin: SkinWeights,
    pub compliant: Vec<(String, String)>,
}

impl RigSpec {
    /// Construct a rig spec.
    pub fn new(
        skeleton: Skeleton,
        anchors: Vec<JointAnchor>,
        skin: SkinWeights,
        compliant: Vec<(String, String)>,
    ) -> RigSpec {
        RigSpec {
            skeleton,
            anchors,
            skin,
            compliant,
        }
    }
}

/// A collision capsule for one bone, in bind-world space: the segment `a..b` and a radius that
/// encloses every vertex dominantly bound to the joint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Capsule {
    pub joint: usize,
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
    pub bound_vertices: u32,
}

/// A compliant (soft) region re-measured on the shaped surface: its dominant joint, its
/// centroid in bind-world space and in that joint's bind frame, its radius and its half-extents.
#[derive(Debug, Clone, PartialEq)]
pub struct CompliantTag {
    pub name: String,
    pub region: String,
    pub joint: usize,
    pub vertices: Vec<u32>,
    pub centroid: Vec3,
    pub local_centroid: Vec3,
    pub radius: f32,
    /// Per-axis maximum `|p - centroid|` over the region (bind-world axes): sees width and
    /// depth changes that leave the centroid and the largest radius alone.
    pub half_extent: Vec3,
}

/// Everything derived from one shaped body: shaped skeleton, inverse binds, the carried-over
/// skin weights, collision capsules and compliant tags, stamped with the body revision.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundRig {
    body_rev: BodyRevId,
    skeleton: Skeleton,
    inverse_binds: Vec<Mat4>,
    skin: SkinWeights,
    colliders: Vec<Capsule>,
    compliant: Vec<CompliantTag>,
    id: RigId,
}

impl BoundRig {
    fn compute_id(&self) -> RigId {
        let mut out = Vec::new();
        out.extend_from_slice(RIG_PREFIX);
        out.extend_from_slice(&self.body_rev.0);
        put_u32(&mut out, self.skeleton.joint_count() as u32);
        for j in self.skeleton.joints() {
            put_str(&mut out, &j.name);
            let parent: i32 = j.parent.map_or(-1, |p| p as i32);
            out.extend_from_slice(&parent.to_le_bytes());
            put_vec3(&mut out, j.rest.translation);
            let q = j.rest.rotation;
            for c in [q.x, q.y, q.z, q.w] {
                put_f32(&mut out, c);
            }
            put_vec3(&mut out, j.rest.scale);
        }
        put_u32(&mut out, self.inverse_binds.len() as u32);
        for m in &self.inverse_binds {
            put_mat(&mut out, m);
        }
        self.skin.write_canonical(&mut out);
        put_u32(&mut out, self.colliders.len() as u32);
        for c in &self.colliders {
            put_u32(&mut out, c.joint as u32);
            put_vec3(&mut out, c.a);
            put_vec3(&mut out, c.b);
            put_f32(&mut out, c.radius);
            put_u32(&mut out, c.bound_vertices);
        }
        put_u32(&mut out, self.compliant.len() as u32);
        for t in &self.compliant {
            put_str(&mut out, &t.name);
            put_str(&mut out, &t.region);
            put_u32(&mut out, t.joint as u32);
            put_u32(&mut out, t.vertices.len() as u32);
            for &v in &t.vertices {
                put_u32(&mut out, v);
            }
            put_vec3(&mut out, t.centroid);
            put_vec3(&mut out, t.local_centroid);
            put_f32(&mut out, t.radius);
            put_vec3(&mut out, t.half_extent);
        }
        RigId(sha256::digest(&out))
    }

    /// The body revision this rig was derived from.
    pub fn body_rev(&self) -> BodyRevId {
        self.body_rev
    }

    /// The rig id (hash of everything derived).
    pub fn id(&self) -> RigId {
        self.id
    }

    /// STALE-SKELETON DETECTOR: refuse unless this rig was derived from exactly `body`'s revision.
    pub fn check_current(&self, body: &ShapedBody) -> Result<(), RebindError> {
        if self.body_rev == body.rev {
            Ok(())
        } else {
            Err(RebindError::StaleSkeleton {
                bound_to: self.body_rev,
                got: body.rev,
            })
        }
    }

    /// Stronger check: re-derive from `spec` for `body` and require the same rig id, so a rig
    /// whose stamp was forged or whose contents were edited is also refused.
    pub fn verify(
        &self,
        family: &BodyFamily,
        body: &ShapedBody,
        spec: &RigSpec,
    ) -> Result<(), RebindError> {
        self.check_current(body)?;
        let fresh = family.rebind(body, spec)?;
        if fresh.id == self.id {
            Ok(())
        } else {
            Err(RebindError::Tampered {
                expected: fresh.id,
                got: self.id,
            })
        }
    }

    /// The shaped skeleton, guarded against a stale body.
    pub fn skeleton_for(&self, body: &ShapedBody) -> Result<&Skeleton, RebindError> {
        self.check_current(body)?;
        Ok(&self.skeleton)
    }

    /// The inverse binds, guarded against a stale body.
    pub fn inverse_binds_for(&self, body: &ShapedBody) -> Result<&[Mat4], RebindError> {
        self.check_current(body)?;
        Ok(&self.inverse_binds)
    }

    /// The collision capsules (one per bone with bound vertices), guarded against a stale body.
    pub fn colliders_for(&self, body: &ShapedBody) -> Result<&[Capsule], RebindError> {
        self.check_current(body)?;
        Ok(&self.colliders)
    }

    /// The compliant-region tags, guarded against a stale body.
    pub fn compliant_for(&self, body: &ShapedBody) -> Result<&[CompliantTag], RebindError> {
        self.check_current(body)?;
        Ok(&self.compliant)
    }

    /// The skin weights (unchanged from the rig spec), guarded against a stale body.
    pub fn skin_for(&self, body: &ShapedBody) -> Result<&SkinWeights, RebindError> {
        self.check_current(body)?;
        Ok(&self.skin)
    }

    /// Linear-blend skin `body`'s shaped positions with joint world matrices `world`, using this
    /// rig's inverse binds. Refused for a stale body.
    pub fn skin_positions(
        &self,
        body: &ShapedBody,
        world: &[Mat4],
    ) -> Result<Vec<Vec3>, RebindError> {
        self.check_current(body)?;
        self.skin
            .skin(&body.positions, world, &self.inverse_binds)
            .map_err(RebindError::Skin)
    }
}
