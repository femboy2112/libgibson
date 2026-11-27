//! `figure::studio`: the `rev.v1` coupled-deformation compiler (M5-C slice 4). DRAFT, EXPERIMENTAL.
//!
//! `compile(&RecipeV1, &AssetSet) -> Result<Compiled, CompileError>` shapes a body from per-control
//! sparse deltas, re-derives the skeleton from the shaped surface (`figure::refit`), derives the
//! inverse binds, fits every garment to the shaped body from its binding, gates drawing on a
//! typed per-garment [`FitStatus`] and derives three domain-separated identities (body, fit,
//! presentation). Nothing here is wired into the v0 `compile`: v0 bytes and the golden stay
//! untouched. The asset input format is `gibson.figure.asset.v1` (`docs/FIGURE_CONTRACT.md`,
//! "rev.v1 asset input format"); `tools/figure_assets/convert_to_gibson.py` writes it.
//!
//! # Compile order (validation precedes any derived data; all or nothing)
//!
//! 1. limits, 2. control values (unknown, duplicate, non-finite are errors; a value outside the
//!    declared domain is applied as given and reported as [`Diagnostic::ControlOutOfDomain`], never
//!    clamped), 3. garment, hair, material, attachment and fit-parameter checks, 4. shaping
//!    `G = G0 + sum(|c_i| * delta_i[side(c_i)])` in family control order (negative values use the
//!    negative side, positive the positive side, zero neither), 5. ground lift (the whole figure is
//!    translated so the lowest body vertex has `z = 0`, when the asset says it is grounded), 6. joints
//!    re-derived from the anchor groups of the SHAPED vertices, 7. inverse binds, 8. canonical body
//!    bytes and `BodyRevId`, 9. garment fits and verdicts, `FitId`, 10. `PresentationId`.
//!
//! Scalar `f32`, no SIMD, no `unsafe`, no clock, no I/O except the explicit [`AssetSet::load_dir`].
//!
//! # Mutation controls (`RUSTFLAGS='--cfg figure_mutant="..."'`)
//!
//! `ignore_skeleton_update_v1` skips the refit (the declared skeleton is kept), `ignore_surface_update`
//! skips the control deltas, `stale_fit` fits and gates garments against the NEUTRAL body instead of
//! the shaped one (a fit carried across a body edit). Each is killed by a non-golden assertion in
//! `tests/figure_studio.rs`.

use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use super::math::{Mat4, Quat, Transform, Vec3};
use super::morph::{self, MorphError, MorphTarget};
use super::refit::{refit_skeleton, JointAnchor, RefitError};
use super::revision::canonical_f32_bits;
use super::sha256;
use super::skeleton::{Joint, Skeleton, SkeletonError, MAX_JOINTS};
use super::skin::{SkinError, SkinWeights, SkinningMode, MAX_INFLUENCES};

/// Prefix of a body asset file.
pub const BODY_MAGIC: &[u8] = b"gibson.figure.asset.body.v1\0";
/// Prefix of a part (garment, hair, ...) asset file.
pub const PART_MAGIC: &[u8] = b"gibson.figure.asset.part.v1\0";
/// The first line of `index.txt`, after `format`.
pub const ASSET_FORMAT: &str = "gibson.figure.asset.v1";
/// Prefix of the canonical body bytes (the 21 bytes `gibson.figure.rev.v1\0`).
pub const BODY_REV_PREFIX: &[u8] = b"gibson.figure.rev.v1\0";
/// Prefix of the fit-id digest input.
pub const FIT_PREFIX: &[u8] = b"gibson.figure.fit.v1\0";
/// Prefix of the presentation-id digest input.
pub const PRESENT_PREFIX: &[u8] = b"gibson.figure.presentation.v1\0";
/// The garment-fit algorithm id hashed into the fit id.
pub const FIT_ALGORITHM_ID: &str = "gibson.fit.vertex-normal-penetration.v2";

/// The closed set of material slot names.
pub const MATERIAL_SLOTS: [&str; 8] = [
    "base",
    "panel",
    "trim",
    "ornament",
    "gem_emissive",
    "mask",
    "iris",
    "accessory_emissive",
];

/// Hard cap on vertices (body plus helpers) in an asset, and on vertices of a part.
pub const MAX_ASSET_VERTICES: usize = 1 << 18;
/// Hard cap on controls in an asset (two morph targets each, under `MAX_MORPH_TARGETS`).
pub const MAX_ASSET_CONTROLS: usize = 128;
/// Hard cap on landmark groups in an asset.
pub const MAX_ASSET_LANDMARKS: usize = 1024;
/// Hard cap on parts in an asset set.
pub const MAX_PARTS: usize = 256;
/// Hard cap on garments in a recipe.
pub const MAX_GARMENTS: usize = 64;
/// Hard cap on fit parameters hashed into a fit id.
pub const MAX_FIT_PARAMS: usize = 64;
/// Hard cap on material bindings in a recipe.
pub const MAX_MATERIAL_BINDINGS: usize = 256;
/// Hard cap on style parameters in a recipe.
pub const MAX_STYLE_PARAMS: usize = 256;
/// Hard cap on attachments in a recipe.
pub const MAX_ATTACHMENTS: usize = 64;
/// Hard cap on control values in a recipe.
pub const MAX_RECIPE_CONTROLS: usize = 256;

// --------------------------------------------------------------------------------------------
// Identities.

fn hex32(b: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for &x in b.iter() {
        s.push(char::from(b"0123456789abcdef"[usize::from(x >> 4)]));
        s.push(char::from(b"0123456789abcdef"[usize::from(x & 15)]));
    }
    s
}

macro_rules! digest_id {
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
                write!(f, "{}({})", stringify!($name), self.to_hex())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.to_hex())
            }
        }
    };
}

digest_id! {
    /// The neutral-body id: SHA-256 of the `gibson.figure.rev.v1` canonical body bytes. Depends on
    /// the asset, the resolved body controls, the skinning mode and the footwear, and on nothing
    /// else (no garment, material, hair, style, provenance or motion style).
    BodyRevId
}
digest_id! {
    /// One garment-fit computation: SHA-256 of the body id, the ordered garment set with each
    /// garment asset's content hash, and the fit parameters.
    FitId
}
digest_id! {
    /// One cached look: SHA-256 of the fit id, the materials, the garment slot bindings, the hair,
    /// the attachments and the style parameters. A cache key only.
    PresentationId
}

// --------------------------------------------------------------------------------------------
// Canonical byte helpers (the v0 encoding).

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_f32(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&canonical_f32_bits(v).to_le_bytes());
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

// --------------------------------------------------------------------------------------------
// Asset loading.

/// Why an asset file or set was refused.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum AssetError {
    /// The bytes end before a field does (`at` is the byte offset).
    Truncated { at: usize },
    /// A file does not start with the expected prefix.
    BadMagic { what: &'static str },
    /// Bytes remain after the last field.
    TrailingBytes { extra: usize },
    /// A field has an impossible value.
    Invalid(&'static str),
    /// A field of entry `index` has an impossible value.
    InvalidAt { what: &'static str, index: usize },
    /// A hard limit was exceeded.
    LimitExceeded {
        what: &'static str,
        count: usize,
        max: usize,
    },
    /// Two parts share an id.
    DuplicatePart { id: String },
    /// `index.txt` is malformed.
    Index(String),
    /// A file could not be read.
    Io(String),
    /// A file's SHA-256 does not match `index.txt`.
    ShaMismatch { file: String },
    /// The skin weights are invalid.
    Skin(SkinError),
    /// A morph target is invalid.
    Morph(MorphError),
    /// The declared skeleton is invalid.
    Skeleton(SkeletonError),
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetError::Truncated { at } => write!(f, "asset bytes end early at offset {at}"),
            AssetError::BadMagic { what } => write!(f, "{what} asset has the wrong prefix"),
            AssetError::TrailingBytes { extra } => {
                write!(f, "{extra} trailing bytes after the asset")
            }
            AssetError::Invalid(what) => write!(f, "invalid asset: {what}"),
            AssetError::InvalidAt { what, index } => {
                write!(f, "invalid asset: {what} (entry {index})")
            }
            AssetError::LimitExceeded { what, count, max } => {
                write!(f, "too many {what}: {count} (max {max})")
            }
            AssetError::DuplicatePart { id } => write!(f, "part {id:?} is listed twice"),
            AssetError::Index(m) => write!(f, "index.txt: {m}"),
            AssetError::Io(m) => write!(f, "io: {m}"),
            AssetError::ShaMismatch { file } => {
                write!(f, "sha256 of {file} does not match index.txt")
            }
            AssetError::Skin(e) => write!(f, "skin: {e}"),
            AssetError::Morph(e) => write!(f, "morph: {e}"),
            AssetError::Skeleton(e) => write!(f, "skeleton: {e}"),
        }
    }
}

impl std::error::Error for AssetError {}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], AssetError> {
        if n > self.remaining() {
            return Err(AssetError::Truncated { at: self.pos });
        }
        let data: &'a [u8] = self.data;
        let s = &data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, AssetError> {
        let b = self.take(1)?;
        Ok(b[0])
    }

    fn u32(&mut self) -> Result<u32, AssetError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn i32(&mut self) -> Result<i32, AssetError> {
        let b = self.take(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn f32(&mut self) -> Result<f32, AssetError> {
        let v = self.u32()?;
        Ok(f32::from_bits(v))
    }

    fn vec3(&mut self) -> Result<Vec3, AssetError> {
        let x = self.f32()?;
        let y = self.f32()?;
        let z = self.f32()?;
        Ok(Vec3::new(x, y, z))
    }

    fn string(&mut self) -> Result<String, AssetError> {
        let n = self.u32()? as usize;
        let b = self.take(n)?;
        match String::from_utf8(b.to_vec()) {
            Ok(s) => Ok(s),
            Err(_) => Err(AssetError::Invalid("a string is not UTF-8")),
        }
    }

    /// A `u32` count whose entries each take at least `min_entry` bytes. A count that cannot fit
    /// in the remaining bytes is refused here, so a hostile count never reaches an allocation.
    fn count(&mut self, min_entry: usize) -> Result<usize, AssetError> {
        let n = self.u32()? as usize;
        if min_entry > 0 && n > self.remaining() / min_entry {
            return Err(AssetError::Truncated { at: self.pos });
        }
        Ok(n)
    }
}

fn read_skin_entry(r: &mut Reader<'_>) -> Result<Vec<(usize, f32)>, AssetError> {
    let n = usize::from(r.u8()?);
    if n == 0 || n > MAX_INFLUENCES {
        return Err(AssetError::Invalid(
            "a skin entry has 0 or more than 4 influences",
        ));
    }
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let j = r.u32()? as usize;
        let w = r.f32()?;
        v.push((j, w));
    }
    Ok(v)
}

#[derive(Debug, Clone)]
struct JointAsset {
    anchor: Vec<u32>,
}

#[derive(Debug, Clone)]
struct ControlAsset {
    name: String,
    min: f32,
    max: f32,
    default: f32,
}

#[derive(Debug, Clone)]
struct BodyAsset {
    id: String,
    version: u32,
    grounded: bool,
    n_body: usize,
    positions: Vec<Vec3>,
    indices: Vec<u32>,
    joints: Vec<JointAsset>,
    declared: Skeleton,
    skin: SkinWeights,
    controls: Vec<ControlAsset>,
    /// `morphs[2 * c]` is control `c`'s negative side, `morphs[2 * c + 1]` its positive side.
    morphs: Vec<MorphTarget>,
    landmarks: Vec<(String, Vec<u32>)>,
}

fn parse_body(bytes: &[u8]) -> Result<BodyAsset, AssetError> {
    let mut r = Reader::new(bytes);
    let magic = r.take(BODY_MAGIC.len())?;
    if magic != BODY_MAGIC {
        return Err(AssetError::BadMagic { what: "body" });
    }
    let id = r.string()?;
    let version = r.u32()?;
    let grounded = match r.u8()? {
        0 => false,
        1 => true,
        _ => return Err(AssetError::Invalid("grounded flag is not 0 or 1")),
    };
    let n_v = r.u32()? as usize;
    let n_body = r.u32()? as usize;
    if n_v > MAX_ASSET_VERTICES {
        return Err(AssetError::LimitExceeded {
            what: "asset vertices",
            count: n_v,
            max: MAX_ASSET_VERTICES,
        });
    }
    if n_body == 0 || n_body > n_v {
        return Err(AssetError::Invalid(
            "body vertex count is zero or above the vertex count",
        ));
    }
    if n_v > r.remaining() / 12 {
        return Err(AssetError::Truncated { at: r.pos });
    }
    let mut positions = Vec::with_capacity(n_v);
    for _ in 0..n_v {
        positions.push(r.vec3()?);
    }
    if !positions.iter().all(|p| p.is_finite()) {
        return Err(AssetError::Invalid("a neutral position is not finite"));
    }
    let n_idx = r.count(4)?;
    if n_idx % 3 != 0 {
        return Err(AssetError::Invalid("index count is not a multiple of 3"));
    }
    let mut indices = Vec::with_capacity(n_idx);
    for _ in 0..n_idx {
        let i = r.u32()?;
        if (i as usize) >= n_body {
            return Err(AssetError::Invalid("a triangle index is outside the body"));
        }
        indices.push(i);
    }

    let nj = r.count(52)?;
    if nj > MAX_JOINTS {
        return Err(AssetError::LimitExceeded {
            what: "joints",
            count: nj,
            max: MAX_JOINTS,
        });
    }
    let mut joints = Vec::with_capacity(nj);
    let mut decl: Vec<Joint> = Vec::with_capacity(nj);
    for i in 0..nj {
        let name = r.string()?;
        let parent_raw = r.i32()?;
        let t = r.vec3()?;
        let qx = r.f32()?;
        let qy = r.f32()?;
        let qz = r.f32()?;
        let qw = r.f32()?;
        let s = r.vec3()?;
        let na = r.count(4)?;
        let mut anchor = Vec::with_capacity(na);
        for _ in 0..na {
            let a = r.u32()?;
            if (a as usize) >= n_v {
                return Err(AssetError::InvalidAt {
                    what: "anchor vertex out of range",
                    index: i,
                });
            }
            anchor.push(a);
        }
        let parent = if parent_raw == -1 {
            None
        } else if parent_raw >= 0 && (parent_raw as usize) < i {
            Some(parent_raw as usize)
        } else {
            return Err(AssetError::InvalidAt {
                what: "parent is not -1 or an earlier joint",
                index: i,
            });
        };
        decl.push(Joint::new(
            name,
            parent,
            Transform::new(t, Quat::new(qx, qy, qz, qw), s),
        ));
        joints.push(JointAsset { anchor });
    }
    let declared = Skeleton::new(decl, Vec::new()).map_err(AssetError::Skeleton)?;

    let mut influences: Vec<Vec<(usize, f32)>> = Vec::with_capacity(n_body);
    for _ in 0..n_body {
        influences.push(read_skin_entry(&mut r)?);
    }
    let skin = SkinWeights::new(&influences, nj).map_err(AssetError::Skin)?;

    let nc = r.count(24)?;
    if nc > MAX_ASSET_CONTROLS {
        return Err(AssetError::LimitExceeded {
            what: "controls",
            count: nc,
            max: MAX_ASSET_CONTROLS,
        });
    }
    let mut controls: Vec<ControlAsset> = Vec::with_capacity(nc);
    let mut morphs: Vec<MorphTarget> = Vec::with_capacity(2 * nc);
    for ci in 0..nc {
        let name = r.string()?;
        let min = r.f32()?;
        let max = r.f32()?;
        let default = r.f32()?;
        if !(min.is_finite() && max.is_finite() && default.is_finite())
            || min > max
            || default < min
            || default > max
        {
            return Err(AssetError::InvalidAt {
                what: "control domain is not finite, empty, or excludes its default",
                index: ci,
            });
        }
        if controls.iter().any(|c| c.name == name) {
            return Err(AssetError::InvalidAt {
                what: "duplicate control name",
                index: ci,
            });
        }
        for side in 0..2 {
            let nd = r.count(16)?;
            let mut deltas = Vec::with_capacity(nd);
            for _ in 0..nd {
                let v = r.u32()?;
                let d = r.vec3()?;
                deltas.push((v, d));
            }
            let suffix = if side == 0 { "neg" } else { "pos" };
            morphs.push(MorphTarget::new(format!("{name}.{suffix}"), deltas));
        }
        controls.push(ControlAsset {
            name,
            min,
            max,
            default,
        });
    }
    morph::validate_targets(&morphs, n_v).map_err(AssetError::Morph)?;

    let nl = r.count(8)?;
    if nl > MAX_ASSET_LANDMARKS {
        return Err(AssetError::LimitExceeded {
            what: "landmarks",
            count: nl,
            max: MAX_ASSET_LANDMARKS,
        });
    }
    let mut landmarks: Vec<(String, Vec<u32>)> = Vec::with_capacity(nl);
    for li in 0..nl {
        let name = r.string()?;
        let n = r.count(4)?;
        if n == 0 {
            return Err(AssetError::InvalidAt {
                what: "empty landmark group",
                index: li,
            });
        }
        let mut idx = Vec::with_capacity(n);
        for _ in 0..n {
            let v = r.u32()?;
            if (v as usize) >= n_v {
                return Err(AssetError::InvalidAt {
                    what: "landmark vertex out of range",
                    index: li,
                });
            }
            idx.push(v);
        }
        let mut sorted = idx.clone();
        sorted.sort_unstable();
        if sorted.windows(2).any(|w| w[0] == w[1]) {
            return Err(AssetError::InvalidAt {
                what: "landmark repeats a vertex",
                index: li,
            });
        }
        if landmarks.iter().any(|(n2, _)| *n2 == name) {
            return Err(AssetError::InvalidAt {
                what: "duplicate landmark name",
                index: li,
            });
        }
        landmarks.push((name, idx));
    }
    if r.remaining() != 0 {
        return Err(AssetError::TrailingBytes {
            extra: r.remaining(),
        });
    }
    Ok(BodyAsset {
        id,
        version,
        grounded,
        n_body,
        positions,
        indices,
        joints,
        declared,
        skin,
        controls,
        morphs,
        landmarks,
    })
}

#[derive(Debug, Clone, PartialEq)]
enum ScaleSpec {
    Const(f32),
    Pair {
        a: u32,
        b: u32,
        axis: u8,
        reference: f32,
    },
}

#[derive(Debug, Clone, PartialEq)]
struct Binding {
    v: [u32; 3],
    w: [f32; 3],
    off: Vec3,
}

#[derive(Debug, Clone)]
struct PartAsset {
    id: String,
    kind: String,
    indices: Vec<u32>,
    bindings: Vec<Binding>,
    spec: Vec<ScaleSpec>,
    skin: SkinWeights,
    hidden: Vec<u32>,
    sha: [u8; 32],
}

fn parse_part(
    bytes: &[u8],
    n_v: usize,
    n_body: usize,
    joint_count: usize,
) -> Result<PartAsset, AssetError> {
    let mut r = Reader::new(bytes);
    let magic = r.take(PART_MAGIC.len())?;
    if magic != PART_MAGIC {
        return Err(AssetError::BadMagic { what: "part" });
    }
    let id = r.string()?;
    let kind = r.string()?;
    let n = r.count(12)?;
    if n == 0 || n > MAX_ASSET_VERTICES {
        return Err(AssetError::Invalid(
            "part vertex count is zero or above the limit",
        ));
    }
    for _ in 0..n {
        let p = r.vec3()?;
        if !p.is_finite() {
            return Err(AssetError::Invalid(
                "a part reference position is not finite",
            ));
        }
    }
    let n_idx = r.count(4)?;
    if n_idx % 3 != 0 {
        return Err(AssetError::Invalid(
            "part index count is not a multiple of 3",
        ));
    }
    let mut indices = Vec::with_capacity(n_idx);
    for _ in 0..n_idx {
        let i = r.u32()?;
        if (i as usize) >= n {
            return Err(AssetError::Invalid("a part triangle index is out of range"));
        }
        indices.push(i);
    }
    if n > r.remaining() / 36 {
        return Err(AssetError::Truncated { at: r.pos });
    }
    let mut bindings = Vec::with_capacity(n);
    for vi in 0..n {
        let v0 = r.u32()?;
        let v1 = r.u32()?;
        let v2 = r.u32()?;
        let w0 = r.f32()?;
        let w1 = r.f32()?;
        let w2 = r.f32()?;
        let off = r.vec3()?;
        if (v0 as usize) >= n_v || (v1 as usize) >= n_v || (v2 as usize) >= n_v {
            return Err(AssetError::InvalidAt {
                what: "binding vertex out of range",
                index: vi,
            });
        }
        if !(w0.is_finite() && w1.is_finite() && w2.is_finite() && off.is_finite()) {
            return Err(AssetError::InvalidAt {
                what: "binding weight or offset is not finite",
                index: vi,
            });
        }
        bindings.push(Binding {
            v: [v0, v1, v2],
            w: [w0, w1, w2],
            off,
        });
    }
    let ns = r.count(5)?;
    if ns != 0 && ns != 3 {
        return Err(AssetError::Invalid("offset spec must have 0 or 3 entries"));
    }
    let mut spec = Vec::with_capacity(ns);
    for si in 0..ns {
        match r.u8()? {
            0 => {
                let c = r.f32()?;
                if !c.is_finite() {
                    return Err(AssetError::InvalidAt {
                        what: "constant scale is not finite",
                        index: si,
                    });
                }
                spec.push(ScaleSpec::Const(c));
            }
            1 => {
                let a = r.u32()?;
                let b = r.u32()?;
                let axis = r.u8()?;
                let reference = r.f32()?;
                if (a as usize) >= n_v || (b as usize) >= n_v || axis > 2 {
                    return Err(AssetError::InvalidAt {
                        what: "scale pair vertex or axis out of range",
                        index: si,
                    });
                }
                if !reference.is_finite() || reference.abs() < 1.0e-12 {
                    return Err(AssetError::InvalidAt {
                        what: "scale reference is not finite or is zero",
                        index: si,
                    });
                }
                spec.push(ScaleSpec::Pair {
                    a,
                    b,
                    axis,
                    reference,
                });
            }
            _ => {
                return Err(AssetError::InvalidAt {
                    what: "unknown scale kind",
                    index: si,
                })
            }
        }
    }
    let mut influences: Vec<Vec<(usize, f32)>> = Vec::with_capacity(n);
    for _ in 0..n {
        influences.push(read_skin_entry(&mut r)?);
    }
    let skin = SkinWeights::new(&influences, joint_count).map_err(AssetError::Skin)?;
    let nh = r.count(4)?;
    let mut hidden = Vec::with_capacity(nh);
    for _ in 0..nh {
        let h = r.u32()?;
        if (h as usize) >= n_body {
            return Err(AssetError::Invalid("a hidden vertex is outside the body"));
        }
        hidden.push(h);
    }
    if r.remaining() != 0 {
        return Err(AssetError::TrailingBytes {
            extra: r.remaining(),
        });
    }
    Ok(PartAsset {
        id,
        kind,
        indices,
        bindings,
        spec,
        skin,
        hidden,
        sha: sha256::digest(bytes),
    })
}

/// A loaded, validated body asset and its parts. Cheap to share by reference across compiles.
#[derive(Debug, Clone)]
pub struct AssetSet {
    body: BodyAsset,
    body_sha: [u8; 32],
    parts: Vec<PartAsset>,
}

fn safe_file_name(s: &str) -> bool {
    !s.is_empty() && !s.starts_with('.') && !s.contains('/') && !s.contains('\\')
}

impl AssetSet {
    /// Validate and build from in-memory file bytes: one body file and any number of part files.
    /// The core of loading: no I/O.
    pub fn from_bytes(body: &[u8], parts: &[&[u8]]) -> Result<AssetSet, AssetError> {
        if parts.len() > MAX_PARTS {
            return Err(AssetError::LimitExceeded {
                what: "parts",
                count: parts.len(),
                max: MAX_PARTS,
            });
        }
        let b = parse_body(body)?;
        let mut out: Vec<PartAsset> = Vec::with_capacity(parts.len());
        for bytes in parts {
            let p = parse_part(bytes, b.positions.len(), b.n_body, b.joints.len())?;
            if out.iter().any(|o| o.id == p.id) {
                return Err(AssetError::DuplicatePart { id: p.id });
            }
            out.push(p);
        }
        Ok(AssetSet {
            body_sha: sha256::digest(body),
            body: b,
            parts: out,
        })
    }

    /// Read `index.txt` and the files it lists from `dir`, verify every SHA-256, then
    /// [`AssetSet::from_bytes`]. File names in the index must be plain names (no separators).
    pub fn load_dir(dir: &Path) -> Result<AssetSet, AssetError> {
        let index = fs::read_to_string(dir.join("index.txt"))
            .map_err(|e| AssetError::Io(format!("index.txt: {e}")))?;
        let mut saw_format = false;
        let mut body_entry: Option<(String, String)> = None;
        let mut part_entries: Vec<(String, String, String, String)> = Vec::new();
        for line in index.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let f: Vec<&str> = line.split_whitespace().collect();
            match f[0] {
                "format" => {
                    if f.len() != 2 || f[1] != ASSET_FORMAT {
                        return Err(AssetError::Index("unknown format".to_string()));
                    }
                    saw_format = true;
                }
                "source_manifest_sha256" => {}
                "body" => {
                    if f.len() != 3 || body_entry.is_some() {
                        return Err(AssetError::Index("bad or repeated body line".to_string()));
                    }
                    body_entry = Some((f[1].to_string(), f[2].to_string()));
                }
                "part" => {
                    if f.len() != 5 {
                        return Err(AssetError::Index("bad part line".to_string()));
                    }
                    part_entries.push((
                        f[1].to_string(),
                        f[2].to_string(),
                        f[3].to_string(),
                        f[4].to_string(),
                    ));
                }
                _ => return Err(AssetError::Index("unknown directive".to_string())),
            }
        }
        if !saw_format {
            return Err(AssetError::Index("missing format line".to_string()));
        }
        let Some((body_file, body_hex)) = body_entry else {
            return Err(AssetError::Index("missing body line".to_string()));
        };
        if !safe_file_name(&body_file) {
            return Err(AssetError::Index("unsafe file name".to_string()));
        }
        let body_bytes = fs::read(dir.join(&body_file))
            .map_err(|e| AssetError::Io(format!("{body_file}: {e}")))?;
        if hex32(&sha256::digest(&body_bytes)) != body_hex {
            return Err(AssetError::ShaMismatch { file: body_file });
        }
        let mut part_bytes: Vec<Vec<u8>> = Vec::with_capacity(part_entries.len());
        for (_, _, file, hex) in &part_entries {
            if !safe_file_name(file) {
                return Err(AssetError::Index("unsafe file name".to_string()));
            }
            let b = fs::read(dir.join(file)).map_err(|e| AssetError::Io(format!("{file}: {e}")))?;
            if hex32(&sha256::digest(&b)) != *hex {
                return Err(AssetError::ShaMismatch { file: file.clone() });
            }
            part_bytes.push(b);
        }
        let refs: Vec<&[u8]> = part_bytes.iter().map(|b| b.as_slice()).collect();
        let set = AssetSet::from_bytes(&body_bytes, &refs)?;
        for (i, (pid, kind, _, _)) in part_entries.iter().enumerate() {
            if set.parts[i].id != *pid || set.parts[i].kind != *kind {
                return Err(AssetError::Index(
                    "a part's id or kind differs from its file".to_string(),
                ));
            }
        }
        Ok(set)
    }

    /// The asset id (hashed into the body id).
    pub fn asset_id(&self) -> &str {
        &self.body.id
    }

    /// SHA-256 of the body file's bytes (hashed into the body id).
    pub fn body_sha256(&self) -> [u8; 32] {
        self.body_sha
    }

    /// All vertices, helpers included.
    pub fn vertex_count(&self) -> usize {
        self.body.positions.len()
    }

    /// Body vertices: indices `0..body_vertex_count()` are skinned; the rest are helpers.
    pub fn body_vertex_count(&self) -> usize {
        self.body.n_body
    }

    /// The neutral positions (full index space).
    pub fn neutral_positions(&self) -> &[Vec3] {
        &self.body.positions
    }

    /// Control names in declared order.
    pub fn control_names(&self) -> Vec<&str> {
        self.body.controls.iter().map(|c| c.name.as_str()).collect()
    }

    /// `(min, max, default)` of a control.
    pub fn control_domain(&self, name: &str) -> Option<(f32, f32, f32)> {
        self.body
            .controls
            .iter()
            .find(|c| c.name == name)
            .map(|c| (c.min, c.max, c.default))
    }

    /// Number of joints.
    pub fn joint_count(&self) -> usize {
        self.body.joints.len()
    }

    /// The name of joint `i`.
    pub fn joint_name(&self, i: usize) -> Option<&str> {
        self.body.declared.joints().get(i).map(|j| j.name.as_str())
    }

    /// The anchor group of joint `i` (empty: unanchored).
    pub fn joint_anchor(&self, i: usize) -> Option<&[u32]> {
        self.body.joints.get(i).map(|j| j.anchor.as_slice())
    }

    /// A surface landmark group by name.
    pub fn landmark(&self, name: &str) -> Option<&[u32]> {
        self.body
            .landmarks
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_slice())
    }

    /// Part ids in file order.
    pub fn part_ids(&self) -> Vec<&str> {
        self.parts.iter().map(|p| p.id.as_str()).collect()
    }

    /// The kind of a part (`clothes`, `hair`, ...).
    pub fn part_kind(&self, id: &str) -> Option<&str> {
        self.parts
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.kind.as_str())
    }

    /// SHA-256 of a part file's bytes (hashed into the fit id for garments).
    pub fn part_sha256(&self, id: &str) -> Option<[u8; 32]> {
        self.parts.iter().find(|p| p.id == id).map(|p| p.sha)
    }
}

// --------------------------------------------------------------------------------------------
// Recipe.

/// Footwear: feeds the stance and foot-contact calibration (LATER); hashed into the body id.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct FootwearV1 {
    pub heel_height_m: f32,
    pub toe_pitch_rad: f32,
}

impl Default for FootwearV1 {
    fn default() -> FootwearV1 {
        FootwearV1 {
            heel_height_m: 0.0,
            toe_pitch_rad: 0.0,
        }
    }
}

/// A garment from the asset set plus its material slot bindings (part name -> slot name).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct GarmentV1 {
    pub id: String,
    pub slot_bindings: Vec<(String, String)>,
}

impl GarmentV1 {
    /// A garment with no slot bindings.
    pub fn new(id: impl Into<String>) -> GarmentV1 {
        GarmentV1 {
            id: id.into(),
            slot_bindings: Vec::new(),
        }
    }
}

/// Hair: an optional style (a part of kind `hair`) and a colour.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct HairV1 {
    pub style_id: Option<String>,
    pub color: [f32; 3],
}

impl Default for HairV1 {
    fn default() -> HairV1 {
        HairV1 {
            style_id: None,
            color: [0.0, 0.0, 0.0],
        }
    }
}

/// Where an attachment is anchored.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum AnchorV1 {
    /// A surface landmark group of the asset.
    Landmark(String),
    /// A bone of the rig.
    Bone(String),
}

/// An accessory: an id, an anchor and an optional emissive colour. Hashed into the presentation
/// id; v1 draws no accessory geometry.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct AttachmentV1 {
    pub id: String,
    pub anchor: AnchorV1,
    pub emissive: Option<[f32; 3]>,
}

impl AttachmentV1 {
    /// An attachment with no emissive colour.
    pub fn new(id: impl Into<String>, anchor: AnchorV1) -> AttachmentV1 {
        AttachmentV1 {
            id: id.into(),
            anchor,
            emissive: None,
        }
    }
}

/// One named material slot (see [`MATERIAL_SLOTS`]).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct MaterialSlotV1 {
    pub slot: String,
    pub material_id: String,
    pub texture_sha256: [u8; 32],
    pub albedo: [f32; 3],
    pub roughness: f32,
    pub metalness: f32,
    pub emissive: Option<([f32; 3], f32)>,
}

impl MaterialSlotV1 {
    /// A mid-grey, rough, non-metal slot with no texture and no emission.
    pub fn new(slot: impl Into<String>) -> MaterialSlotV1 {
        MaterialSlotV1 {
            slot: slot.into(),
            material_id: String::new(),
            texture_sha256: [0u8; 32],
            albedo: [0.5, 0.5, 0.5],
            roughness: 0.5,
            metalness: 0.0,
            emissive: None,
        }
    }
}

/// Motion style: validated and carried, but in NONE of the three ids (the tape has its own digest).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct MotionStyleV1 {
    pub preset: String,
    pub amplitude: f32,
    pub tempo: f32,
    pub settle_ease_ms: f32,
}

impl Default for MotionStyleV1 {
    fn default() -> MotionStyleV1 {
        MotionStyleV1 {
            preset: String::new(),
            amplitude: 1.0,
            tempo: 1.0,
            settle_ease_ms: 0.0,
        }
    }
}

/// Opaque consumer provenance: echoed in [`Compiled`], never hashed.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ProvenanceV1 {
    pub key: String,
    pub sha256: [u8; 32],
}

impl ProvenanceV1 {
    /// Provenance with the given key and sha.
    pub fn new(key: impl Into<String>, sha256: [u8; 32]) -> ProvenanceV1 {
        ProvenanceV1 {
            key: key.into(),
            sha256,
        }
    }
}

/// Garment-fit parameters (hashed into the fit id).
///
/// A garment vertex whose signed distance to the nearest body surface point, along that point's
/// normal, is below `-tolerance_m` counts as penetrating. No penetrating vertex: `Fitted`. The
/// deepest one no deeper than `adapt_depth_m`: `NeedsAdaptation`. Deeper: `Rejected`.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct FitParamsV1 {
    pub tolerance_m: f32,
    pub adapt_depth_m: f32,
}

impl Default for FitParamsV1 {
    fn default() -> FitParamsV1 {
        FitParamsV1 {
            tolerance_m: 0.002,
            adapt_depth_m: 0.01,
        }
    }
}

/// The recipe of `figure::studio` (docs/design/m5c-studio-api.md section 1). All fields explicit.
///
/// Build it with [`RecipeV1::new`] (or `Default`) and assign fields; the struct is
/// `#[non_exhaustive]` so it can grow.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RecipeV1 {
    /// Body control values by name. An omitted control takes its default. Never clamped.
    pub controls: Vec<(String, f32)>,
    pub footwear: FootwearV1,
    /// The garment set, in host-declared (layering) order.
    pub garments: Vec<GarmentV1>,
    pub hair: HairV1,
    pub attachments: Vec<AttachmentV1>,
    pub materials: Vec<MaterialSlotV1>,
    /// When set, jaw and mouth expression channels are inert (see
    /// [`Compiled::face_channel_diagnostics`]).
    pub lower_face_covered: bool,
    pub motion_style: MotionStyleV1,
    pub provenance: Option<ProvenanceV1>,
    pub fit_params: FitParamsV1,
    /// Appearance parameters (skin tone, ...) hashed into the presentation id only.
    pub style_params: Vec<(String, f32)>,
    /// The skinning mode of the body (hashed into the body id). `DualQuat` for v1.
    pub skinning: SkinningMode,
}

impl RecipeV1 {
    /// An empty recipe: default controls, no garments, `DualQuat` skinning.
    pub fn new() -> RecipeV1 {
        RecipeV1 {
            controls: Vec::new(),
            footwear: FootwearV1::default(),
            garments: Vec::new(),
            hair: HairV1::default(),
            attachments: Vec::new(),
            materials: Vec::new(),
            lower_face_covered: false,
            motion_style: MotionStyleV1::default(),
            provenance: None,
            fit_params: FitParamsV1::default(),
            style_params: Vec::new(),
            skinning: SkinningMode::DualQuat,
        }
    }

    /// Set (or replace) the control `id`.
    pub fn set_control(&mut self, id: &str, value: f32) {
        for c in self.controls.iter_mut() {
            if c.0 == id {
                c.1 = value;
                return;
            }
        }
        self.controls.push((id.to_string(), value));
    }
}

impl Default for RecipeV1 {
    fn default() -> RecipeV1 {
        RecipeV1::new()
    }
}

// --------------------------------------------------------------------------------------------
// Diagnostics, fit status, errors.

/// A non-fatal finding of a compile.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Diagnostic {
    /// A control value lies outside its declared domain. It was applied as given.
    ControlOutOfDomain {
        id: String,
        value: f32,
        min: f32,
        max: f32,
    },
    /// A footwear value lies outside its documented domain. It was hashed and carried as given.
    FootwearOutOfDomain { field: &'static str, value: f32 },
    /// A garment has no `Fitted` verdict and is not drawn.
    GarmentNotDrawn { id: String, status: FitStatus },
    /// A jaw or mouth channel was driven while the lower face is covered.
    FaceChannelInert { channel: String },
}

/// Why a garment is not `Fitted`.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum FitReason {
    /// `count` vertices lie below the body surface by more than the tolerance; the deepest by
    /// `max_depth_m`.
    Penetration { count: u32, max_depth_m: f32 },
    /// The fit produced a non-finite vertex (a degenerate body, say).
    NonFiniteFit,
}

/// The typed outcome of fitting one garment. Only `Fitted` is drawn.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum FitStatus {
    Fitted,
    NeedsAdaptation(FitReason),
    Rejected(FitReason),
}

/// The measured penetration of a garment against the shaped body.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct FitMetrics {
    /// Garment vertices.
    pub vertices: u32,
    /// Vertices deeper than the tolerance.
    pub penetrating: u32,
    /// The deepest penetration over all vertices (metres, 0 if none).
    pub max_depth_m: f32,
}

/// One garment's verdict and metrics.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct GarmentFitReport {
    pub id: String,
    pub status: FitStatus,
    pub metrics: FitMetrics,
}

/// Why [`compile`] refused a recipe.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum CompileError {
    /// The recipe sets a control the asset does not define.
    UnknownControl {
        id: String,
    },
    /// The recipe sets one control twice.
    DuplicateControl {
        id: String,
    },
    /// A value in `part` is NaN or infinite.
    NonFinite {
        part: &'static str,
    },
    /// A garment id is not a part of the asset set.
    UnknownGarment {
        id: String,
    },
    /// A garment id appears twice in the set.
    DuplicateGarment {
        id: String,
    },
    /// The part named as a garment is hair.
    NotAGarment {
        id: String,
    },
    /// The hair style is not a part of kind `hair`.
    UnknownHair {
        id: String,
    },
    /// A material slot name is not one of [`MATERIAL_SLOTS`].
    UnknownMaterialSlot {
        slot: String,
    },
    /// A material slot is bound twice.
    DuplicateMaterialSlot {
        slot: String,
    },
    /// An attachment names a landmark or bone the asset does not have.
    UnknownAnchor {
        attachment: String,
        anchor: String,
    },
    /// An attachment id appears twice.
    DuplicateAttachment {
        id: String,
    },
    /// The fit parameters are negative, non-finite or out of order.
    BadFitParams,
    /// A hard limit was exceeded.
    LimitExceeded {
        what: &'static str,
        count: usize,
        max: usize,
    },
    /// The shaped geometry failed its check (non-finite surface, say).
    Geometry {
        detail: &'static str,
    },
    Morph(MorphError),
    Refit(RefitError),
    Skeleton(SkeletonError),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::UnknownControl { id } => write!(f, "unknown control {id:?}"),
            CompileError::DuplicateControl { id } => write!(f, "control {id:?} is set twice"),
            CompileError::NonFinite { part } => write!(f, "{part} is not finite"),
            CompileError::UnknownGarment { id } => write!(f, "unknown garment {id:?}"),
            CompileError::DuplicateGarment { id } => write!(f, "garment {id:?} is listed twice"),
            CompileError::NotAGarment { id } => write!(f, "part {id:?} is hair, not a garment"),
            CompileError::UnknownHair { id } => write!(f, "unknown hair style {id:?}"),
            CompileError::UnknownMaterialSlot { slot } => {
                write!(f, "unknown material slot {slot:?}")
            }
            CompileError::DuplicateMaterialSlot { slot } => {
                write!(f, "material slot {slot:?} is bound twice")
            }
            CompileError::UnknownAnchor { attachment, anchor } => {
                write!(
                    f,
                    "attachment {attachment:?} names unknown anchor {anchor:?}"
                )
            }
            CompileError::DuplicateAttachment { id } => {
                write!(f, "attachment {id:?} is listed twice")
            }
            CompileError::BadFitParams => write!(f, "fit parameters are invalid"),
            CompileError::LimitExceeded { what, count, max } => {
                write!(f, "too many {what}: {count} (max {max})")
            }
            CompileError::Geometry { detail } => write!(f, "geometry check failed: {detail}"),
            CompileError::Morph(e) => write!(f, "morph: {e}"),
            CompileError::Refit(e) => write!(f, "refit: {e}"),
            CompileError::Skeleton(e) => write!(f, "skeleton: {e}"),
        }
    }
}

impl std::error::Error for CompileError {}

// --------------------------------------------------------------------------------------------
// Geometry helpers.

fn component(v: Vec3, axis: u8) -> f32 {
    match axis {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

/// The three offset scales of a part for body positions `p`.
fn part_scales(spec: &[ScaleSpec], p: &[Vec3]) -> [f32; 3] {
    let mut s = [1.0f32; 3];
    if spec.len() == 3 {
        for (k, sp) in spec.iter().enumerate() {
            s[k] = match sp {
                ScaleSpec::Const(c) => *c,
                ScaleSpec::Pair {
                    a,
                    b,
                    axis,
                    reference,
                } => {
                    let d = component(p[*a as usize], *axis) - component(p[*b as usize], *axis);
                    d.abs() / *reference
                }
            };
        }
    }
    s
}

/// The MakeHuman `.mhclo` fit rule: `w0 P[v0] + w1 P[v1] + w2 P[v2] + (ox sx, oy sy, oz sz)` for
/// every vertex of `part`, against body positions `p` (full index space). Bindings were validated
/// against `p.len()` at load.
fn fit_part(part: &PartAsset, p: &[Vec3]) -> Vec<Vec3> {
    let s = part_scales(&part.spec, p);
    let mut out = Vec::with_capacity(part.bindings.len());
    for b in &part.bindings {
        let a0 = p[b.v[0] as usize];
        let a1 = p[b.v[1] as usize];
        let a2 = p[b.v[2] as usize];
        out.push(Vec3::new(
            b.w[0] * a0.x + b.w[1] * a1.x + b.w[2] * a2.x + b.off.x * s[0],
            b.w[0] * a0.y + b.w[1] * a1.y + b.w[2] * a2.y + b.off.y * s[1],
            b.w[0] * a0.z + b.w[1] * a1.z + b.w[2] * a2.z + b.off.z * s[2],
        ));
    }
    out
}

/// Area-weighted vertex normals of the body triangles (unit; a vertex with no triangle gets zero).
fn body_normals(p: &[Vec3], indices: &[u32], n_body: usize) -> Vec<Vec3> {
    let mut acc = vec![Vec3::ZERO; n_body];
    for t in indices.chunks_exact(3) {
        let a = t[0] as usize;
        let b = t[1] as usize;
        let c = t[2] as usize;
        let e1 = p[b].sub(p[a]);
        let e2 = p[c].sub(p[a]);
        let n = e1.cross(e2);
        acc[a] = acc[a].add(n);
        acc[b] = acc[b].add(n);
        acc[c] = acc[c].add(n);
    }
    let mut out = Vec::with_capacity(n_body);
    for v in acc {
        match v.try_normalize() {
            Some(u) => out.push(u),
            None => out.push(Vec3::ZERO),
        }
    }
    out
}

/// Unit vertex normals of a mesh of `n` vertices; a vertex in no triangle gets +z (it is never
/// drawn, and a zero normal would make linear skinning refuse it).
fn unit_normals(p: &[Vec3], indices: &[u32], n: usize) -> Vec<Vec3> {
    let mut v = body_normals(p, indices, n);
    for x in v.iter_mut() {
        if x.dot(*x) == 0.0 {
            *x = Vec3::new(0.0, 0.0, 1.0);
        }
    }
    v
}

/// Penetration of `garment` into the body: for each garment vertex the nearest body vertex (ties:
/// lowest index) among `candidates`, and the signed distance along that vertex's normal. Below
/// zero is inside. A garment vertex whose nearest body vertex is `masked` (hidden under the
/// garment) is skipped. Returns `(count deeper than tolerance, deepest depth)`.
fn penetration(
    garment: &[Vec3],
    body: &[Vec3],
    normals: &[Vec3],
    candidates: &[usize],
    masked: &[bool],
    tolerance: f32,
) -> (u32, f32) {
    let mut count = 0u32;
    let mut max_depth = 0.0f32;
    for &g in garment {
        let mut best = usize::MAX;
        let mut best_d2 = f32::INFINITY;
        for &b in candidates {
            let d = g.sub(body[b]);
            let d2 = d.dot(d);
            if d2 < best_d2 {
                best_d2 = d2;
                best = b;
            }
        }
        if best == usize::MAX || masked[best] {
            continue;
        }
        let s = g.sub(body[best]).dot(normals[best]);
        if s < 0.0 {
            let depth = -s;
            if depth > tolerance {
                count += 1;
            }
            if depth > max_depth {
                max_depth = depth;
            }
        }
    }
    (count, max_depth)
}

fn verdict(metrics: &FitMetrics, p: &FitParamsV1) -> FitStatus {
    if metrics.penetrating == 0 {
        FitStatus::Fitted
    } else if metrics.max_depth_m <= p.adapt_depth_m {
        FitStatus::NeedsAdaptation(FitReason::Penetration {
            count: metrics.penetrating,
            max_depth_m: metrics.max_depth_m,
        })
    } else {
        FitStatus::Rejected(FitReason::Penetration {
            count: metrics.penetrating,
            max_depth_m: metrics.max_depth_m,
        })
    }
}

// --------------------------------------------------------------------------------------------
// The compiled figure.

/// A garment that has a `Fitted` verdict: the only garments a renderer may draw.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DrawableGarment {
    pub id: String,
    pub kind: String,
    /// Fitted vertex positions at rest (the shaped body's frame).
    pub positions: Vec<Vec3>,
    pub indices: Vec<u32>,
    /// Rest vertex normals (area-weighted, unit; a vertex in no triangle gets +z).
    pub normals: Vec<Vec3>,
    /// Body vertex indices hidden under this garment (MakeHuman `delete_verts`). Body triangles
    /// that use one of them are not drawn, and these vertices are not part of the penetration
    /// measure of this garment.
    pub hidden_body_vertices: Vec<u32>,
    skin: SkinWeights,
}

/// The hair style, fitted to the body (no verdict: hair is presentation, not part of the fit id).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct FittedHair {
    pub id: String,
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub indices: Vec<u32>,
    skin: SkinWeights,
}

/// One frame of a performance tape, as the skinner needs it: a world translation of the whole
/// figure plus one local transform per joint, in skeleton order (the tape's `root_t3`,
/// `local_t3`, `local_r4`, `local_s3` channels). Locals REPLACE the joints' rest locals.
#[derive(Debug, Clone, PartialEq)]
pub struct TapeFrame {
    pub root_t: Vec3,
    pub locals: Vec<Transform>,
}

impl TapeFrame {
    /// A frame from its parts.
    pub fn new(root_t: Vec3, locals: Vec<Transform>) -> TapeFrame {
        TapeFrame { root_t, locals }
    }
}

/// Why a pose could not be evaluated.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PoseError {
    /// The frame has the wrong number of joint locals.
    JointCount {
        expected: usize,
        got: usize,
    },
    /// A local transform or the root translation is not finite (or has a zero-length rotation).
    InvalidTransform {
        joint: Option<usize>,
    },
    Skeleton(SkeletonError),
    Skin(SkinError),
}

impl fmt::Display for PoseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PoseError::JointCount { expected, got } => {
                write!(f, "frame has {got} joint locals, expected {expected}")
            }
            PoseError::InvalidTransform { joint } => match joint {
                Some(j) => write!(f, "joint {j} has an invalid local transform"),
                None => write!(f, "root translation is not finite"),
            },
            PoseError::Skeleton(e) => write!(f, "skeleton: {e}"),
            PoseError::Skin(e) => write!(f, "skin: {e}"),
        }
    }
}

impl std::error::Error for PoseError {}

/// Posed vertices of one fitted part.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PosedPart {
    pub id: String,
    pub positions: Vec<Vec3>,
    /// Posed unit normals (the rotation of the skin transform; DQS: the blended rotation).
    pub normals: Vec<Vec3>,
}

/// The skinned figure for one tape frame.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PosedFigure {
    /// Skinned body vertices (`0..body_vertex_count`), root translation applied.
    pub body: Vec<Vec3>,
    /// Posed body normals, one per body vertex.
    pub body_normals: Vec<Vec3>,
    /// Drawable garments only, in set order.
    pub garments: Vec<PosedPart>,
    pub hair: Option<PosedPart>,
    /// Joint world matrices (without the root translation).
    pub joint_world: Vec<Mat4>,
}

/// What a mesh is, for a renderer that styles bodies, garments and hair differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MeshRole {
    Body,
    Garment,
    Hair,
}

/// One drawable mesh, ready for a renderer: positions and normals per vertex, triangles, and a
/// material slot per triangle (an index into [`MATERIAL_SLOTS`]; resolve it with
/// [`Compiled::material`]). Only drawable content appears: garments without a `Fitted` verdict
/// are absent, and body triangles that use a vertex hidden under a drawn garment (its
/// `delete_verts`) are dropped, so the body never shows through its clothes.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RenderMesh {
    pub id: String,
    pub role: MeshRole,
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
    /// One per triangle: index into [`MATERIAL_SLOTS`]. The body is `base` (0); a garment takes
    /// the slot of its first slot binding, else `base`; hair is `base` (its colour is
    /// `recipe().hair.color`).
    pub slot_per_triangle: Vec<u8>,
}

fn slot_index(name: &str) -> u8 {
    match MATERIAL_SLOTS.iter().position(|s| *s == name) {
        Some(i) => i as u8,
        None => 0,
    }
}

fn mesh_of(
    id: &str,
    role: MeshRole,
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    indices: &[u32],
    slot: u8,
    hidden: Option<&[bool]>,
) -> RenderMesh {
    let mut triangles = Vec::with_capacity(indices.len() / 3);
    for t in indices.chunks_exact(3) {
        if let Some(mask) = hidden {
            if mask[t[0] as usize] || mask[t[1] as usize] || mask[t[2] as usize] {
                continue;
            }
        }
        triangles.push([t[0], t[1], t[2]]);
    }
    let slot_per_triangle = vec![slot; triangles.len()];
    RenderMesh {
        id: id.to_string(),
        role,
        positions,
        normals,
        triangles,
        slot_per_triangle,
    }
}

/// The result of [`compile`]: the shaped body, the derived skeleton, the garment verdicts and the
/// three identities.
#[derive(Debug, Clone)]
pub struct Compiled {
    body_rev_id: BodyRevId,
    fit_id: FitId,
    present_id: PresentationId,
    mode: SkinningMode,
    n_body: usize,
    positions: Vec<Vec3>,
    indices: Vec<u32>,
    skeleton: Skeleton,
    inverse_binds: Vec<Mat4>,
    skin: SkinWeights,
    body_normals: Vec<Vec3>,
    landmarks: Vec<(String, Vec<u32>)>,
    ground_lift_m: f32,
    body_bytes: Vec<u8>,
    fit_reports: Vec<GarmentFitReport>,
    garments: Vec<DrawableGarment>,
    hair: Option<FittedHair>,
    diagnostics: Vec<Diagnostic>,
    provenance: Option<ProvenanceV1>,
    lower_face_covered: bool,
    motion_style: MotionStyleV1,
    recipe: RecipeV1,
    assets: Arc<AssetSet>,
}

impl Compiled {
    /// The neutral-body id.
    pub fn body_rev_id(&self) -> BodyRevId {
        self.body_rev_id
    }

    /// The fit id.
    pub fn fit_id(&self) -> FitId {
        self.fit_id
    }

    /// The presentation id.
    pub fn present_id(&self) -> PresentationId {
        self.present_id
    }

    /// A verdict per garment of the recipe, in set order (drawn or not).
    pub fn fit_status(&self) -> &[GarmentFitReport] {
        &self.fit_reports
    }

    /// Non-fatal findings, in a fixed order: control domains, footwear, garments.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// The consumer's provenance, echoed. It is in no id.
    pub fn provenance(&self) -> Option<&ProvenanceV1> {
        self.provenance.as_ref()
    }

    /// The motion style, echoed. It is in no id.
    pub fn motion_style(&self) -> &MotionStyleV1 {
        &self.motion_style
    }

    /// Whether the lower face is covered (jaw and mouth channels inert).
    pub fn lower_face_covered(&self) -> bool {
        self.lower_face_covered
    }

    /// The garments with a `Fitted` verdict: the only ones a renderer may draw.
    pub fn drawable_garments(&self) -> &[DrawableGarment] {
        &self.garments
    }

    /// The fitted hair, if the recipe named a style.
    pub fn hair(&self) -> Option<&FittedHair> {
        self.hair.as_ref()
    }

    /// The canonical `gibson.figure.rev.v1` bytes the body id is the SHA-256 of.
    pub fn body_bytes(&self) -> &[u8] {
        &self.body_bytes
    }

    /// The skinning mode (hashed into the body id).
    pub fn skinning_mode(&self) -> SkinningMode {
        self.mode
    }

    /// The re-derived skeleton.
    pub fn skeleton(&self) -> &Skeleton {
        &self.skeleton
    }

    /// The inverse binds of the re-derived skeleton.
    pub fn inverse_binds(&self) -> &[Mat4] {
        &self.inverse_binds
    }

    /// Shaped (and grounded) vertex positions, full index space (helpers included).
    pub fn shaped_positions(&self) -> &[Vec3] {
        &self.positions
    }

    /// Body vertices (the skinned prefix of [`Compiled::shaped_positions`]).
    pub fn body_vertex_count(&self) -> usize {
        self.n_body
    }

    /// Rest unit vertex normals of the shaped body, one per body vertex (area-weighted; a vertex
    /// in no triangle gets +z).
    pub fn body_normals(&self) -> &[Vec3] {
        &self.body_normals
    }

    /// The body triangle index buffer.
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    /// The translation applied so the lowest body vertex sits at `z = 0` (0 if not grounded).
    pub fn ground_lift_m(&self) -> f32 {
        self.ground_lift_m
    }

    /// The centroid of a surface landmark group of the SHAPED body: component sums in the group's
    /// declared order, then one division by the count as `f32`.
    pub fn landmark_centroid(&self, name: &str) -> Option<Vec3> {
        let (_, idx) = self.landmarks.iter().find(|(n, _)| n == name)?;
        let mut s = Vec3::ZERO;
        for &v in idx {
            s = s.add(self.positions[v as usize]);
        }
        let n = idx.len() as f32;
        Some(Vec3::new(s.x / n, s.y / n, s.z / n))
    }

    /// The distance between the centroids of two landmark groups.
    pub fn landmark_distance(&self, a: &str, b: &str) -> Option<f32> {
        let pa = self.landmark_centroid(a)?;
        let pb = self.landmark_centroid(b)?;
        Some(pa.sub(pb).length())
    }

    /// The rest frame: no root translation, the derived rest locals.
    pub fn rest_frame(&self) -> TapeFrame {
        TapeFrame {
            root_t: Vec3::ZERO,
            locals: self.skeleton.rest_locals(),
        }
    }

    /// Diagnostics for jaw or mouth channels driven while the lower face is covered. A channel
    /// whose name starts with `jaw` or `mouth` is such a channel.
    pub fn face_channel_diagnostics(&self, driven: &[&str]) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        if self.lower_face_covered {
            for c in driven {
                if c.starts_with("jaw") || c.starts_with("mouth") {
                    out.push(Diagnostic::FaceChannelInert {
                        channel: (*c).to_string(),
                    });
                }
            }
        }
        out
    }

    /// The recipe this was compiled from.
    pub fn recipe(&self) -> &RecipeV1 {
        &self.recipe
    }

    /// The shared asset set this was compiled against.
    pub fn assets(&self) -> &Arc<AssetSet> {
        &self.assets
    }

    /// The recipe's material for slot index `slot` (an index into [`MATERIAL_SLOTS`]), if the
    /// recipe binds one.
    pub fn material(&self, slot: u8) -> Option<&MaterialSlotV1> {
        let name = MATERIAL_SLOTS.get(usize::from(slot))?;
        self.recipe.materials.iter().find(|m| m.slot == *name)
    }

    /// Re-evaluate with ONE control changed. The result is bit-identical to
    /// `compile(&edited_recipe, assets)`: the edit is applied to this compile's recipe and the
    /// same deterministic pipeline runs on the shared assets (no intermediate is reused yet, so the
    /// equality is by construction; `tests/figure_studio.rs` asserts it). An unknown name or a
    /// non-finite value is the same typed error a full compile gives; an out-of-domain value is
    /// applied and reported, as in `compile`.
    pub fn with_control(&self, name: &str, value: f32) -> Result<Compiled, CompileError> {
        // Fault injection (mutation control `stale_control`): the old shape is returned.
        if cfg!(figure_mutant = "stale_control") {
            return Ok(self.clone());
        }
        let mut r = self.recipe.clone();
        r.set_control(name, value);
        compile_shared(&r, &self.assets)
    }

    fn hidden_mask(&self) -> Vec<bool> {
        let mut mask = vec![false; self.n_body];
        for g in &self.garments {
            for &h in &g.hidden_body_vertices {
                mask[h as usize] = true;
            }
        }
        mask
    }

    fn garment_slot(&self, id: &str) -> u8 {
        match self.recipe.garments.iter().find(|g| g.id == id) {
            Some(g) => match g.slot_bindings.first() {
                Some((_, slot)) => slot_index(slot),
                None => 0,
            },
            None => 0,
        }
    }

    /// The drawable meshes at rest (the shaped body): the body, then every `Fitted` garment in
    /// set order, then the hair.
    pub fn rest_render_meshes(&self) -> Vec<RenderMesh> {
        let mask = self.hidden_mask();
        let mut out = Vec::with_capacity(self.garments.len() + 2);
        out.push(mesh_of(
            "body",
            MeshRole::Body,
            self.positions[..self.n_body].to_vec(),
            self.body_normals.clone(),
            &self.indices,
            0,
            Some(mask.as_slice()),
        ));
        for g in &self.garments {
            out.push(mesh_of(
                &g.id,
                MeshRole::Garment,
                g.positions.clone(),
                g.normals.clone(),
                &g.indices,
                self.garment_slot(&g.id),
                None,
            ));
        }
        if let Some(h) = &self.hair {
            out.push(mesh_of(
                &h.id,
                MeshRole::Hair,
                h.positions.clone(),
                h.normals.clone(),
                &h.indices,
                0,
                None,
            ));
        }
        out
    }

    /// The drawable meshes for one tape frame (same set and order as
    /// [`Compiled::rest_render_meshes`]), skinned, with posed normals.
    pub fn render_meshes(&self, frame: &TapeFrame) -> Result<Vec<RenderMesh>, PoseError> {
        let posed = self.pose_vertices(frame)?;
        let mask = self.hidden_mask();
        let mut out = Vec::with_capacity(self.garments.len() + 2);
        out.push(mesh_of(
            "body",
            MeshRole::Body,
            posed.body,
            posed.body_normals,
            &self.indices,
            0,
            Some(mask.as_slice()),
        ));
        for (g, p) in self.garments.iter().zip(posed.garments) {
            out.push(mesh_of(
                &g.id,
                MeshRole::Garment,
                p.positions,
                p.normals,
                &g.indices,
                self.garment_slot(&g.id),
                None,
            ));
        }
        if let (Some(h), Some(p)) = (&self.hair, posed.hair) {
            out.push(mesh_of(
                &h.id,
                MeshRole::Hair,
                p.positions,
                p.normals,
                &h.indices,
                0,
                None,
            ));
        }
        Ok(out)
    }

    /// Skin the body, every drawable garment and the hair for one tape frame, with the body's
    /// skinning mode. Garments are skinned by their own (body-transferred) weights, so they follow
    /// the pose. A garment without a `Fitted` verdict is not in the result.
    pub fn pose_vertices(&self, frame: &TapeFrame) -> Result<PosedFigure, PoseError> {
        let nj = self.skeleton.joint_count();
        if frame.locals.len() != nj {
            return Err(PoseError::JointCount {
                expected: nj,
                got: frame.locals.len(),
            });
        }
        if !frame.root_t.is_finite() {
            return Err(PoseError::InvalidTransform { joint: None });
        }
        for (i, t) in frame.locals.iter().enumerate() {
            if !t.is_valid() {
                return Err(PoseError::InvalidTransform { joint: Some(i) });
            }
        }
        let world = self
            .skeleton
            .world_from_locals(&frame.locals)
            .map_err(PoseError::Skeleton)?;
        let shift =
            |v: Vec<Vec3>| -> Vec<Vec3> { v.into_iter().map(|p| p.add(frame.root_t)).collect() };
        let body = self
            .skin
            .skin_with(
                self.mode,
                &self.positions[..self.n_body],
                &world,
                &self.inverse_binds,
            )
            .map_err(PoseError::Skin)?;
        let body_normals = self
            .skin
            .skin_normals(self.mode, &self.body_normals, &world, &self.inverse_binds)
            .map_err(PoseError::Skin)?;
        let mut garments = Vec::with_capacity(self.garments.len());
        for g in &self.garments {
            let p = g
                .skin
                .skin_with(self.mode, &g.positions, &world, &self.inverse_binds)
                .map_err(PoseError::Skin)?;
            let n = g
                .skin
                .skin_normals(self.mode, &g.normals, &world, &self.inverse_binds)
                .map_err(PoseError::Skin)?;
            garments.push(PosedPart {
                id: g.id.clone(),
                positions: shift(p),
                normals: n,
            });
        }
        let hair = match &self.hair {
            Some(h) => {
                let p = h
                    .skin
                    .skin_with(self.mode, &h.positions, &world, &self.inverse_binds)
                    .map_err(PoseError::Skin)?;
                let n = h
                    .skin
                    .skin_normals(self.mode, &h.normals, &world, &self.inverse_binds)
                    .map_err(PoseError::Skin)?;
                Some(PosedPart {
                    id: h.id.clone(),
                    positions: shift(p),
                    normals: n,
                })
            }
            None => None,
        };
        Ok(PosedFigure {
            body: shift(body),
            body_normals,
            garments,
            hair,
            joint_world: world,
        })
    }
}

// --------------------------------------------------------------------------------------------
// compile.

fn limit(what: &'static str, count: usize, max: usize) -> Result<(), CompileError> {
    if count > max {
        Err(CompileError::LimitExceeded { what, count, max })
    } else {
        Ok(())
    }
}

fn finite3(v: &[f32; 3]) -> bool {
    v[0].is_finite() && v[1].is_finite() && v[2].is_finite()
}

fn mode_byte(m: SkinningMode) -> u8 {
    match m {
        SkinningMode::Linear => 0,
        SkinningMode::DualQuat => 1,
    }
}

/// Compile `recipe` against `assets`. All or nothing; deterministic.
pub fn compile(recipe: &RecipeV1, assets: &AssetSet) -> Result<Compiled, CompileError> {
    compile_shared(recipe, &Arc::new(assets.clone()))
}

/// [`compile`] against a shared asset set: the result keeps the `Arc` (for
/// [`Compiled::with_control`]) instead of copying the assets.
pub fn compile_shared(recipe: &RecipeV1, assets: &Arc<AssetSet>) -> Result<Compiled, CompileError> {
    let body = &assets.body;

    // 1. Limits.
    limit(
        "recipe controls",
        recipe.controls.len(),
        MAX_RECIPE_CONTROLS,
    )?;
    limit("garments", recipe.garments.len(), MAX_GARMENTS)?;
    limit(
        "material bindings",
        recipe.materials.len(),
        MAX_MATERIAL_BINDINGS,
    )?;
    limit(
        "style parameters",
        recipe.style_params.len() + 1,
        MAX_STYLE_PARAMS,
    )?;
    limit("attachments", recipe.attachments.len(), MAX_ATTACHMENTS)?;

    // 2. Control values: errors for unknown, duplicate, non-finite; out of domain is a diagnostic.
    let nc = body.controls.len();
    let mut values: Vec<f32> = body.controls.iter().map(|c| c.default).collect();
    let mut set = vec![false; nc];
    for (id, v) in &recipe.controls {
        let Some(ci) = body.controls.iter().position(|c| &c.name == id) else {
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
        set[ci] = true;
        values[ci] = *v;
    }
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    for (c, &v) in body.controls.iter().zip(values.iter()) {
        if v < c.min || v > c.max {
            diagnostics.push(Diagnostic::ControlOutOfDomain {
                id: c.name.clone(),
                value: v,
                min: c.min,
                max: c.max,
            });
        }
    }
    let fw = &recipe.footwear;
    if !(fw.heel_height_m.is_finite() && fw.toe_pitch_rad.is_finite()) {
        return Err(CompileError::NonFinite { part: "footwear" });
    }
    if fw.heel_height_m < 0.0 || fw.heel_height_m > 0.2 {
        diagnostics.push(Diagnostic::FootwearOutOfDomain {
            field: "heel_height_m",
            value: fw.heel_height_m,
        });
    }
    if fw.toe_pitch_rad < -0.6 || fw.toe_pitch_rad > 0.6 {
        diagnostics.push(Diagnostic::FootwearOutOfDomain {
            field: "toe_pitch_rad",
            value: fw.toe_pitch_rad,
        });
    }

    // 3. Garments, hair, materials, attachments, fit and style parameters.
    let mut garment_parts: Vec<&PartAsset> = Vec::with_capacity(recipe.garments.len());
    for (i, g) in recipe.garments.iter().enumerate() {
        if recipe.garments[..i].iter().any(|o| o.id == g.id) {
            return Err(CompileError::DuplicateGarment { id: g.id.clone() });
        }
        let Some(part) = assets.parts.iter().find(|p| p.id == g.id) else {
            return Err(CompileError::UnknownGarment { id: g.id.clone() });
        };
        if part.kind == "hair" {
            return Err(CompileError::NotAGarment { id: g.id.clone() });
        }
        for (_, slot) in &g.slot_bindings {
            if !MATERIAL_SLOTS.contains(&slot.as_str()) {
                return Err(CompileError::UnknownMaterialSlot { slot: slot.clone() });
            }
        }
        garment_parts.push(part);
    }
    let hair_part: Option<&PartAsset> = match &recipe.hair.style_id {
        Some(id) => match assets
            .parts
            .iter()
            .find(|p| &p.id == id && p.kind == "hair")
        {
            Some(p) => Some(p),
            None => return Err(CompileError::UnknownHair { id: id.clone() }),
        },
        None => None,
    };
    if !finite3(&recipe.hair.color) {
        return Err(CompileError::NonFinite { part: "hair color" });
    }
    for (i, m) in recipe.materials.iter().enumerate() {
        if !MATERIAL_SLOTS.contains(&m.slot.as_str()) {
            return Err(CompileError::UnknownMaterialSlot {
                slot: m.slot.clone(),
            });
        }
        if recipe.materials[..i].iter().any(|o| o.slot == m.slot) {
            return Err(CompileError::DuplicateMaterialSlot {
                slot: m.slot.clone(),
            });
        }
        let emissive_ok = match &m.emissive {
            Some((c, k)) => finite3(c) && k.is_finite(),
            None => true,
        };
        if !(finite3(&m.albedo)
            && m.roughness.is_finite()
            && m.metalness.is_finite()
            && emissive_ok)
        {
            return Err(CompileError::NonFinite {
                part: "material parameter",
            });
        }
    }
    for (i, a) in recipe.attachments.iter().enumerate() {
        if recipe.attachments[..i].iter().any(|o| o.id == a.id) {
            return Err(CompileError::DuplicateAttachment { id: a.id.clone() });
        }
        let ok = match &a.anchor {
            AnchorV1::Landmark(n) => body.landmarks.iter().any(|(ln, _)| ln == n),
            AnchorV1::Bone(n) => body.declared.joint_index(n).is_some(),
        };
        if !ok {
            let name = match &a.anchor {
                AnchorV1::Landmark(n) => n.clone(),
                AnchorV1::Bone(n) => n.clone(),
            };
            return Err(CompileError::UnknownAnchor {
                attachment: a.id.clone(),
                anchor: name,
            });
        }
        if let Some(e) = &a.emissive {
            if !finite3(e) {
                return Err(CompileError::NonFinite {
                    part: "attachment emissive",
                });
            }
        }
    }
    let fp = &recipe.fit_params;
    if !(fp.tolerance_m.is_finite() && fp.adapt_depth_m.is_finite())
        || fp.tolerance_m < 0.0
        || fp.adapt_depth_m < fp.tolerance_m
    {
        return Err(CompileError::BadFitParams);
    }
    if !recipe.style_params.iter().all(|(_, v)| v.is_finite()) {
        return Err(CompileError::NonFinite {
            part: "style parameter",
        });
    }
    let ms = &recipe.motion_style;
    if !(ms.amplitude.is_finite() && ms.tempo.is_finite() && ms.settle_ease_ms.is_finite()) {
        return Err(CompileError::NonFinite {
            part: "motion style",
        });
    }

    // 4. Shape the surface: positive values weight the positive side, negative values the
    //    negative side (by magnitude), in declared control order. No clamp, no compensation.
    let mut weights = vec![0.0f32; body.morphs.len()];
    // Fault injection (mutation control `ignore_surface_update`): the control deltas are skipped.
    if !cfg!(figure_mutant = "ignore_surface_update") {
        for (ci, &v) in values.iter().enumerate() {
            if v < 0.0 {
                weights[2 * ci] = -v;
            } else if v > 0.0 {
                weights[2 * ci + 1] = v;
            }
        }
    }
    let mut shaped = morph::apply_morphs(&body.positions, &body.morphs, &weights)
        .map_err(CompileError::Morph)?;

    // 5. Ground lift: the lowest body vertex goes to z = 0; the whole figure moves with it.
    let mut lift = 0.0f32;
    if body.grounded {
        let mut min_z = shaped[0].z;
        for p in &shaped[..body.n_body] {
            if p.z < min_z {
                min_z = p.z;
            }
        }
        lift = -min_z;
        for p in shaped.iter_mut() {
            p.z += lift;
        }
    }
    if !shaped.iter().all(|p| p.is_finite()) {
        return Err(CompileError::Geometry {
            detail: "shaped surface is not finite",
        });
    }

    // 6 and 7. Joints re-derived from the shaped anchor groups, then the inverse binds.
    let mut anchors: Vec<JointAnchor> = Vec::new();
    for (j, ja) in body.joints.iter().enumerate() {
        if !ja.anchor.is_empty() {
            anchors.push(JointAnchor::new(j, ja.anchor.clone()));
        }
    }
    let (skeleton, inverse_binds) = if cfg!(figure_mutant = "ignore_skeleton_update_v1") {
        // Fault injection (mutation control `ignore_skeleton_update_v1`): the refit is skipped and
        // the declared skeleton is kept.
        let ib = body
            .declared
            .inverse_binds()
            .map_err(CompileError::Skeleton)?;
        (body.declared.clone(), ib)
    } else {
        let r = refit_skeleton(&body.declared, &anchors, &shaped).map_err(CompileError::Refit)?;
        (r.skeleton, r.inverse_binds)
    };

    // 8. Canonical body bytes and the body id.
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(BODY_REV_PREFIX);
    out.push(mode_byte(recipe.skinning));
    put_str(&mut out, &body.id);
    put_u32(&mut out, body.version);
    out.extend_from_slice(&assets.body_sha);
    put_u32(&mut out, shaped.len() as u32);
    for &p in &shaped {
        put_vec3(&mut out, p);
    }
    put_u32(&mut out, body.indices.len() as u32);
    for &i in &body.indices {
        put_u32(&mut out, i);
    }
    put_u32(&mut out, skeleton.joint_count() as u32);
    for (i, j) in skeleton.joints().iter().enumerate() {
        put_str(&mut out, &j.name);
        let parent: i32 = match j.parent {
            Some(p) => p as i32,
            None => -1,
        };
        out.extend_from_slice(&parent.to_le_bytes());
        let anchor = &body.joints[i].anchor;
        if anchor.is_empty() {
            out.push(0);
        } else {
            out.push(1);
            put_u32(&mut out, anchor.len() as u32);
            for &a in anchor {
                put_u32(&mut out, a);
            }
        }
        put_vec3(&mut out, j.rest.translation);
        let q = j.rest.rotation;
        put_f32(&mut out, q.x);
        put_f32(&mut out, q.y);
        put_f32(&mut out, q.z);
        put_f32(&mut out, q.w);
        put_vec3(&mut out, j.rest.scale);
    }
    put_u32(&mut out, 0); // v0-style landmarks: none in v1 assets
    put_u32(&mut out, body.landmarks.len() as u32);
    for (name, idx) in &body.landmarks {
        put_str(&mut out, name);
        put_u32(&mut out, idx.len() as u32);
        for &v in idx {
            put_u32(&mut out, v);
        }
    }
    put_u32(&mut out, inverse_binds.len() as u32);
    for m in &inverse_binds {
        for col in &m.cols {
            for &v in col {
                put_f32(&mut out, v);
            }
        }
    }
    put_u32(&mut out, body.n_body as u32);
    body.skin.write_canonical(&mut out);
    put_u32(&mut out, body.morphs.len() as u32);
    for m in &body.morphs {
        m.write_canonical(&mut out);
    }
    put_f32(&mut out, fw.heel_height_m);
    put_f32(&mut out, fw.toe_pitch_rad);
    let body_rev_id = BodyRevId(sha256::digest(&out));
    let body_bytes = out;

    // 9. Garment fits, verdicts and the fit id. The fit is a function of the shaped body.
    // Fault injection (mutation control `stale_fit`): garments are fitted and gated against the
    // NEUTRAL body, as if a fit computed before this body edit were carried over to it.
    let fit_src: &[Vec3] = if cfg!(figure_mutant = "stale_fit") {
        &body.positions[..]
    } else {
        &shaped[..]
    };
    let normals = body_normals(fit_src, &body.indices, body.n_body);
    let mut candidates: Vec<usize> = Vec::new();
    for (i, n) in normals.iter().enumerate() {
        if n.dot(*n) > 0.0 {
            candidates.push(i);
        }
    }
    let mut fit_reports: Vec<GarmentFitReport> = Vec::with_capacity(garment_parts.len());
    let mut drawable: Vec<DrawableGarment> = Vec::new();
    for part in &garment_parts {
        let pos = fit_part(part, fit_src);
        let finite = pos.iter().all(|p| p.is_finite());
        // The body vertices this garment hides (`delete_verts`) are masked out of its measure: a
        // garment vertex whose nearest body vertex is hidden is not measured.
        let mut masked = vec![false; body.n_body];
        for &h in &part.hidden {
            masked[h as usize] = true;
        }
        let metrics;
        let status;
        if finite {
            let (count, max_depth) = penetration(
                &pos,
                fit_src,
                &normals,
                &candidates,
                &masked,
                fp.tolerance_m,
            );
            metrics = FitMetrics {
                vertices: pos.len() as u32,
                penetrating: count,
                max_depth_m: max_depth,
            };
            status = verdict(&metrics, fp);
        } else {
            metrics = FitMetrics {
                vertices: pos.len() as u32,
                penetrating: 0,
                max_depth_m: 0.0,
            };
            status = FitStatus::Rejected(FitReason::NonFiniteFit);
        }
        if status == FitStatus::Fitted {
            drawable.push(DrawableGarment {
                id: part.id.clone(),
                kind: part.kind.clone(),
                normals: unit_normals(&pos, &part.indices, pos.len()),
                positions: pos,
                indices: part.indices.clone(),
                hidden_body_vertices: part.hidden.clone(),
                skin: part.skin.clone(),
            });
        } else {
            diagnostics.push(Diagnostic::GarmentNotDrawn {
                id: part.id.clone(),
                status: status.clone(),
            });
        }
        fit_reports.push(GarmentFitReport {
            id: part.id.clone(),
            status,
            metrics,
        });
    }
    let hair = match hair_part {
        Some(p) => {
            let positions = fit_part(p, fit_src);
            Some(FittedHair {
                id: p.id.clone(),
                normals: unit_normals(&positions, &p.indices, positions.len()),
                positions,
                indices: p.indices.clone(),
                skin: p.skin.clone(),
            })
        }
        None => None,
    };

    let mut fit_bytes: Vec<u8> = Vec::new();
    fit_bytes.extend_from_slice(FIT_PREFIX);
    fit_bytes.extend_from_slice(&body_rev_id.0);
    put_u32(&mut fit_bytes, garment_parts.len() as u32);
    for part in &garment_parts {
        put_str(&mut fit_bytes, &part.id);
        fit_bytes.extend_from_slice(&part.sha);
    }
    put_str(&mut fit_bytes, FIT_ALGORITHM_ID);
    put_u32(&mut fit_bytes, 2);
    put_str(&mut fit_bytes, "tolerance_m");
    put_f32(&mut fit_bytes, fp.tolerance_m);
    put_str(&mut fit_bytes, "adapt_depth_m");
    put_f32(&mut fit_bytes, fp.adapt_depth_m);
    let fit_id = FitId(sha256::digest(&fit_bytes));

    // 10. The presentation id.
    let mut pb: Vec<u8> = Vec::new();
    pb.extend_from_slice(PRESENT_PREFIX);
    pb.extend_from_slice(&fit_id.0);
    put_u32(&mut pb, recipe.materials.len() as u32);
    for m in &recipe.materials {
        put_str(&mut pb, &m.slot);
        put_str(&mut pb, &m.material_id);
        pb.extend_from_slice(&m.texture_sha256);
        put_u32(&mut pb, 10);
        for c in m.albedo {
            put_f32(&mut pb, c);
        }
        put_f32(&mut pb, m.roughness);
        put_f32(&mut pb, m.metalness);
        match &m.emissive {
            Some((c, k)) => {
                put_f32(&mut pb, 1.0);
                for x in c {
                    put_f32(&mut pb, *x);
                }
                put_f32(&mut pb, *k);
            }
            None => {
                for _ in 0..5 {
                    put_f32(&mut pb, 0.0);
                }
            }
        }
    }
    put_u32(&mut pb, recipe.garments.len() as u32);
    for g in &recipe.garments {
        put_str(&mut pb, &g.id);
        put_u32(&mut pb, g.slot_bindings.len() as u32);
        for (part_name, slot) in &g.slot_bindings {
            put_str(&mut pb, part_name);
            put_str(&mut pb, slot);
        }
    }
    match (&recipe.hair.style_id, hair_part) {
        (Some(id), Some(p)) => {
            pb.push(1);
            put_str(&mut pb, id);
            pb.extend_from_slice(&p.sha);
        }
        _ => pb.push(0),
    }
    for c in recipe.hair.color {
        put_f32(&mut pb, c);
    }
    put_u32(&mut pb, recipe.attachments.len() as u32);
    for a in &recipe.attachments {
        put_str(&mut pb, &a.id);
        match &a.anchor {
            AnchorV1::Landmark(n) => {
                pb.push(0);
                put_str(&mut pb, n);
            }
            AnchorV1::Bone(n) => {
                pb.push(1);
                put_str(&mut pb, n);
            }
        }
        match &a.emissive {
            Some(e) => {
                pb.push(1);
                for x in e {
                    put_f32(&mut pb, *x);
                }
            }
            None => {
                pb.push(0);
                for _ in 0..3 {
                    put_f32(&mut pb, 0.0);
                }
            }
        }
    }
    put_u32(&mut pb, (recipe.style_params.len() + 1) as u32);
    for (name, v) in &recipe.style_params {
        put_str(&mut pb, name);
        put_f32(&mut pb, *v);
    }
    put_str(&mut pb, "lower_face_covered");
    put_f32(&mut pb, if recipe.lower_face_covered { 1.0 } else { 0.0 });
    let present_id = PresentationId(sha256::digest(&pb));

    let rest_normals = unit_normals(&shaped, &body.indices, body.n_body);
    Ok(Compiled {
        body_rev_id,
        fit_id,
        present_id,
        mode: recipe.skinning,
        n_body: body.n_body,
        positions: shaped,
        indices: body.indices.clone(),
        skeleton,
        inverse_binds,
        skin: body.skin.clone(),
        body_normals: rest_normals,
        landmarks: body.landmarks.clone(),
        ground_lift_m: lift,
        body_bytes,
        fit_reports,
        garments: drawable,
        hair,
        diagnostics,
        provenance: recipe.provenance.clone(),
        lower_face_covered: recipe.lower_face_covered,
        motion_style: recipe.motion_style.clone(),
        recipe: recipe.clone(),
        assets: Arc::clone(assets),
    })
}
