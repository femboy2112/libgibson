#![cfg(feature = "figure")]
//! Independent byte-layout audit of `gibson.figure.rev.v0` (feature `figure`).
//!
//! Authored from the NORMATIVE spec text ("Canonical serialization
//! `gibson.figure.rev.v0`" and "Body controls (Slice 1)") and the public
//! signature sheet ONLY. No implementation source was read. Every expected byte
//! below is derived by hand from the spec and written with this file's own
//! helpers; nothing is produced by the library under test except the two values
//! being checked (`canonical_bytes()` and `rev_id`).
//!
//! ## Fixture (all values exactly representable in f32)
//!
//! Family `test.fam` version 3.
//!
//! Neutral G0 (3 vertices, 1 triangle, indices deliberately `[0, 2, 1]`):
//!   v0 = (0.5, 0.0, 0.0)
//!   v1 = (1.0, 0.5, 0.25)
//!   v2 = (0.0, 1.0, -0.5)
//!
//! Joints (parent-before-child), identity rotation (0,0,0,1), unit scale:
//!   j0 "root"  parent -1  rest translation (0, 0, 0)
//!   j1 "child" parent  0  rest translation (0, 2, 0)
//!
//! Landmark: "tip" on joint 1, offset (0.25, 0.5, 0.0).
//!
//! Skin (declared influence order, NOT sorted by joint):
//!   v0: [(joint 1, 0.25), (joint 0, 0.75)]
//!   v1: [(joint 0, 1.0)]
//!   v2: [(joint 1, 1.0)]
//!
//! Morph target 0 "smile", deltas in declared (unsorted) vertex order:
//!   (vertex 2, ( 0.0, 0.0,  0.0))   <- zero delta, must be kept
//!   (vertex 0, ( 0.5, 0.0, -1.0))
//!
//! Controls, in family-declared order:
//!   c0 "bone_len": [0.5, 2.0] default 1.0, BoneLengthScale { joint: 0 }
//!   c1 "morph_w" : [0.0, 1.0] default 0.0, MorphWeight { target: 0 }
//!   c2 "spare"   : [-1.0, 1.0] default 0.25, MorphWeight { target: 0 }
//!
//! Main recipe sets bone_len = 1.5 and morph_w = 0.5 and OMITS "spare", so
//! "spare" takes its default 0.25.
//!
//! ## Hand derivation (spec: "Body controls")
//!
//! Accumulated compile-time weight on morph target 0 (MorphWeight adds v):
//!   w0 = 0 + morph_w (0.5) + spare (0.25) = 0.75
//!
//! Shaped neutral = G0 + sum(w_i * delta_i) = G0 + 0.75 * smile:
//!   v0 = (0.5 + 0.75*0.5, 0.0 + 0.75*0.0, 0.0 + 0.75*(-1.0))
//!      = (0.5 + 0.375,    0.0,            -0.75)
//!      = (0.875, 0.0, -0.75)
//!   v1 = (1.0, 0.5, 0.25)          (no delta)
//!   v2 = (0.0, 1.0, -0.5)          (zero delta: unchanged)
//!
//! Shaped skeleton. BoneLengthScale { joint: 0 } with v = 1.5 multiplies the rest
//! translation of every joint whose parent is joint 0, i.e. j1:
//!   j1 translation = (0, 2, 0) * 1.5 = (0, 3, 0)
//!   j0 is a root (no parent): unchanged (0, 0, 0)
//!
//! World bind (identity rotation, unit scale, so worlds are pure translations):
//!   W0 = T(0, 0, 0)
//!   W1 = W0 * T(0, 3, 0) = T(0, 3, 0)
//!
//! Inverse binds, IB_j = (W_j at bind)^-1. With identity rotation and unit
//! scale, IB_j is the identity with the translation NEGATED:
//!   IB0 translation = -(0, 0, 0) = (-0.0, -0.0, -0.0)
//!   IB1 translation = -(0, 3, 0) = (-0.0, -3.0, -0.0)
//! The negations of 0.0 are -0.0; the spec canonicalizes -0.0 to +0.0 in every
//! serialized f32, so the bytes written are (0, 0, 0) and (0, -3, 0).
//! Column-major, 16 values each (cols[0][0..4], cols[1][0..4], ...), translation
//! in cols[3][0..3] (see SPEC-AMBIGUITY A1):
//!   IB0 = [ 1, 0, 0, 0,   0, 1, 0, 0,   0, 0, 1, 0,   -0.0, -0.0, -0.0, 1 ]
//!   IB1 = [ 1, 0, 0, 0,   0, 1, 0, 0,   0, 0, 1, 0,   -0.0, -3.0, -0.0, 1 ]
//!
//! Hand-computed section offsets for the main fixture (python3 -I checked):
//!   prefix 0 | family 21 | vertices 37 | indices 77 | joints 93 |
//!   landmarks 202 | inverse binds 229 | skin 361 | morphs 400 | total 449.
//!
//! ## SPEC-AMBIGUITY register (mirrored in the hand-back)
//!
//! A1. Column-major placement of translation in `Mat4`: the spec only says
//!     "column-major (cols[0][0..4], then cols[1][0..4], ...)" and never says the
//!     translation lives in column 3. Chosen (most literal standard reading, and
//!     the only one under which "IB is the translation negated" can be written
//!     into 16 floats): translation in cols[3][0..3], w = 1.
//! A2. Landmark offsets: the layout says "the offset (3 x f32)" and says the
//!     joint rest translation is the SHAPED one, but does not say whether the
//!     landmark offset is shaped or whether BoneLengthScale touches it. Chosen
//!     reading: the family-declared offset, written verbatim.
//! A3. Morph deltas: the layout says "3 x f32" per delta without saying
//!     whether the family's raw delta or a weight-scaled delta is written, though
//!     "Zero deltas are kept" and "family's declared order" imply the raw family
//!     data. Chosen reading: raw declared delta, unscaled by any weight.
//! A4. The `str` encoding of non-ASCII text: spec says UTF-8, u32 byte length,
//!     "no normalization". RESOLVED by coordinator ruling #70: ids use the v0
//!     ASCII alphabet [a-z0-9._-], 1..=64 bytes, so test 3 now uses "fam.7" and
//!     test 4 checks that non-ASCII ids are rejected.
//! A5. Whether a control left at its default is "applied" when unset: spec says
//!     "A control that is not set takes its default"; chosen reading is that the
//!     default value then flows through its effects exactly like a set value
//!     (so an omitted "spare" adds 0.25 to the morph weight). This is why the
//!     spare control's default is deliberately non-zero.
//! A6. Accumulation order of MorphWeight contributions is unspecified in float
//!     terms; every value in the fixture is dyadic, so every order gives the same
//!     exact result and the question cannot affect these bytes.
//! A7. Index order: the spec says indices are written "in the family's order".
//!     Chosen reading: no winding or canonical-rotation normalization, so the
//!     fixture's `[0, 2, 1]` appears verbatim.
//! A8. Spec says validation "requires" parent-before-child and the skin count
//!     in 1..=4; it does not list whether every vertex must be referenced by a
//!     triangle. The fixture references all three vertices to stay clear of that
//!     silence.

use gibson::figure::math::{Quat, Transform, Vec3};
use gibson::figure::morph::MorphTarget;
use gibson::figure::revision::{
    compile, BodyFamily, BodyRecipe, CharacterRevision, CompileError, ControlDef, ControlEffect,
    FamilyRef, NeutralMesh, MAX_ID_LEN,
};
use gibson::figure::skeleton::{Joint, Landmark};
use sha2::{Digest as _, Sha256};

// ---------------------------------------------------------------------------
// Fixture construction (inputs only; no expected values are derived from it).
// ---------------------------------------------------------------------------

/// What varies between the fixtures used by the three tests.
struct Fixture {
    family_id: &'static str,
    family_version: u32,
    /// (name, joint, offset) in declared order.
    landmarks: Vec<(&'static str, u32, [f32; 3])>,
}

fn main_fixture() -> Fixture {
    Fixture {
        family_id: "test.fam",
        family_version: 3,
        landmarks: vec![("tip", 1, [0.25, 0.5, 0.0])],
    }
}

fn build_family(fx: &Fixture) -> BodyFamily {
    let unit = Vec3::new(1.0, 1.0, 1.0);
    let neutral = NeutralMesh::new(
        vec![
            Vec3::new(0.5, 0.0, 0.0),
            Vec3::new(1.0, 0.5, 0.25),
            Vec3::new(0.0, 1.0, -0.5),
        ],
        vec![0, 2, 1],
    );
    let joints = vec![
        Joint::new(
            "root",
            None,
            Transform::new(Vec3::new(0.0, 0.0, 0.0), Quat::IDENTITY, unit),
        ),
        Joint::new(
            "child",
            Some(0),
            Transform::new(Vec3::new(0.0, 2.0, 0.0), Quat::IDENTITY, unit),
        ),
    ];
    let landmarks: Vec<Landmark> = fx
        .landmarks
        .iter()
        .map(|(n, j, o)| Landmark::new(*n, *j as usize, Vec3::new(o[0], o[1], o[2])))
        .collect();
    let skin = vec![vec![(1, 0.25), (0, 0.75)], vec![(0, 1.0)], vec![(1, 1.0)]];
    let morphs = vec![MorphTarget::new(
        "smile",
        vec![
            (2, Vec3::new(0.0, 0.0, 0.0)),
            (0, Vec3::new(0.5, 0.0, -1.0)),
        ],
    )];
    let controls = vec![
        ControlDef::new(
            "bone_len",
            0.5,
            2.0,
            1.0,
            vec![ControlEffect::BoneLengthScale { joint: 0 }],
        ),
        ControlDef::new(
            "morph_w",
            0.0,
            1.0,
            0.0,
            vec![ControlEffect::MorphWeight { target: 0 }],
        ),
        ControlDef::new(
            "spare",
            -1.0,
            1.0,
            0.25,
            vec![ControlEffect::MorphWeight { target: 0 }],
        ),
    ];
    BodyFamily::new(
        FamilyRef::new(fx.family_id, fx.family_version),
        neutral,
        joints,
        landmarks,
        skin,
    )
    .with_morphs(morphs)
    .with_controls(controls)
}

fn compile_fx(fx: &Fixture, values: &[(&str, f32)]) -> CharacterRevision {
    let family = build_family(fx);
    let recipe = BodyRecipe::new(
        FamilyRef::new(fx.family_id, fx.family_version),
        values.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
    );
    compile(&recipe, &family).expect("fixture must compile")
}

/// The canonical recipe: bone_len = 1.5, morph_w = 0.5, spare omitted.
const MAIN_VALUES: [(&str, f32); 2] = [("bone_len", 1.5), ("morph_w", 0.5)];

// ---------------------------------------------------------------------------
// Independent expected-bytes builder with (offset, label) recording.
// ---------------------------------------------------------------------------

struct Rec {
    bytes: Vec<u8>,
    labels: Vec<(usize, String)>,
}

impl Rec {
    fn new() -> Rec {
        Rec {
            bytes: Vec::new(),
            labels: Vec::new(),
        }
    }

    fn mark(&mut self, label: &str) {
        self.labels.push((self.bytes.len(), label.to_string()));
    }

    fn offset_of(&self, label: &str) -> usize {
        self.labels
            .iter()
            .find(|(_, l)| l == label)
            .unwrap_or_else(|| panic!("no label {label}"))
            .0
    }

    fn push_raw(&mut self, label: &str, raw: &[u8]) {
        self.mark(label);
        self.bytes.extend_from_slice(raw);
    }

    fn push_u8(&mut self, label: &str, v: u8) {
        self.mark(label);
        self.bytes.push(v);
    }

    fn push_u32_le(&mut self, label: &str, v: u32) {
        self.mark(label);
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }

    fn push_i32_le(&mut self, label: &str, v: i32) {
        self.mark(label);
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }

    /// f32 as its IEEE-754 bit pattern, little-endian, with -0.0 written as +0.0.
    fn push_f32_le(&mut self, label: &str, v: f32) {
        self.mark(label);
        let bits = if v.to_bits() == 0x8000_0000 {
            0u32
        } else {
            v.to_bits()
        };
        self.bytes.extend_from_slice(&bits.to_le_bytes());
    }

    /// str: u32 byte length, then the UTF-8 bytes, no terminator.
    fn push_str(&mut self, label: &str, s: &str) {
        self.mark(&format!("{label}.len"));
        self.bytes
            .extend_from_slice(&(s.len() as u32).to_le_bytes());
        self.mark(&format!("{label}.utf8"));
        self.bytes.extend_from_slice(s.as_bytes());
    }

    fn push_f32x3(&mut self, label: &str, v: [f32; 3]) {
        self.push_f32_le(&format!("{label}.x"), v[0]);
        self.push_f32_le(&format!("{label}.y"), v[1]);
        self.push_f32_le(&format!("{label}.z"), v[2]);
    }
}

/// Builds the expected bytes for the shaped fixture (bone_len = 1.5, accumulated
/// morph weight 0.75) under the given family id/version and landmark list. All
/// numeric literals are the hand-derived values from the module comment.
fn expected_bytes(fx: &Fixture) -> Rec {
    let mut r = Rec::new();

    // 1. Prefix: the 21 bytes `gibson.figure.rev.v0\0` (spec text).
    let prefix: &[u8] = b"gibson.figure.rev.v0\0";
    assert_eq!(prefix.len(), 21, "spec says the prefix is 21 bytes");
    r.push_raw("prefix", prefix);

    // 2. Family: str id, u32 version.
    r.push_str("family.id", fx.family_id);
    r.push_u32_le("family.version", fx.family_version);

    // 3. Vertices: SHAPED neutral positions (hand-derived above).
    let shaped: [[f32; 3]; 3] = [[0.875, 0.0, -0.75], [1.0, 0.5, 0.25], [0.0, 1.0, -0.5]];
    r.push_u32_le("vertices.count", 3);
    for (i, p) in shaped.iter().enumerate() {
        r.push_f32x3(&format!("vertex[{i}]"), *p);
    }

    // 4. Indices: u32 n (= 3 x triangles), then n x u32, family order.
    r.push_u32_le("indices.count", 3);
    for (i, ix) in [0u32, 2, 1].iter().enumerate() {
        r.push_u32_le(&format!("index[{i}]"), *ix);
    }

    // 5. Joints, family order: str name, i32 parent, shaped translation,
    //    rotation x,y,z,w, scale.
    r.push_u32_le("joints.count", 2);
    // j0 root
    r.push_str("joint[0].name", "root");
    r.push_i32_le("joint[0].parent", -1);
    r.push_f32x3("joint[0].translation", [0.0, 0.0, 0.0]);
    r.push_f32_le("joint[0].rot.x", 0.0);
    r.push_f32_le("joint[0].rot.y", 0.0);
    r.push_f32_le("joint[0].rot.z", 0.0);
    r.push_f32_le("joint[0].rot.w", 1.0);
    r.push_f32x3("joint[0].scale", [1.0, 1.0, 1.0]);
    // j1 child, shaped translation (0, 3, 0)
    r.push_str("joint[1].name", "child");
    r.push_i32_le("joint[1].parent", 0);
    r.push_f32x3("joint[1].translation", [0.0, 3.0, 0.0]);
    r.push_f32_le("joint[1].rot.x", 0.0);
    r.push_f32_le("joint[1].rot.y", 0.0);
    r.push_f32_le("joint[1].rot.z", 0.0);
    r.push_f32_le("joint[1].rot.w", 1.0);
    r.push_f32x3("joint[1].scale", [1.0, 1.0, 1.0]);

    // 6. Landmarks, family order: str name, u32 joint, offset.
    r.push_u32_le("landmarks.count", fx.landmarks.len() as u32);
    for (i, (name, joint, off)) in fx.landmarks.iter().enumerate() {
        r.push_str(&format!("landmark[{i}].name"), name);
        r.push_u32_le(&format!("landmark[{i}].joint"), *joint);
        r.push_f32x3(&format!("landmark[{i}].offset"), *off);
    }

    // 7. Inverse binds: u32 n (= joint count), 16 f32 each, column-major.
    //    -0.0 literals are the raw negations; push_f32_le canonicalizes them.
    let ib0: [f32; 16] = [
        1.0, 0.0, 0.0, 0.0, // cols[0]
        0.0, 1.0, 0.0, 0.0, // cols[1]
        0.0, 0.0, 1.0, 0.0, // cols[2]
        -0.0, -0.0, -0.0, 1.0, // cols[3]: translation negated
    ];
    let ib1: [f32; 16] = [
        1.0, 0.0, 0.0, 0.0, // cols[0]
        0.0, 1.0, 0.0, 0.0, // cols[1]
        0.0, 0.0, 1.0, 0.0, // cols[2]
        -0.0, -3.0, -0.0, 1.0, // cols[3]: translation negated
    ];
    r.push_u32_le("inverse_binds.count", 2);
    for (j, ib) in [ib0, ib1].iter().enumerate() {
        for (k, v) in ib.iter().enumerate() {
            r.push_f32_le(&format!("ib[{j}].cols[{}][{}]", k / 4, k % 4), *v);
        }
    }

    // 8. Skin: u32 vertex_count; per vertex u8 count then (u32 joint, f32 weight)
    //    in declared influence order, weights verbatim.
    let skin: [&[(u32, f32)]; 3] = [&[(1, 0.25), (0, 0.75)], &[(0, 1.0)], &[(1, 1.0)]];
    r.push_u32_le("skin.vertex_count", 3);
    for (v, infl) in skin.iter().enumerate() {
        r.push_u8(&format!("skin[{v}].count"), infl.len() as u8);
        for (k, (joint, weight)) in infl.iter().enumerate() {
            r.push_u32_le(&format!("skin[{v}].infl[{k}].joint"), *joint);
            r.push_f32_le(&format!("skin[{v}].infl[{k}].weight"), *weight);
        }
    }

    // 9. Morphs, family order: str name, u32 delta count, then per delta in
    //    declared order u32 vertex + 3 x f32. Zero delta is kept.
    r.push_u32_le("morphs.count", 1);
    r.push_str("morph[0].name", "smile");
    r.push_u32_le("morph[0].delta_count", 2);
    r.push_u32_le("morph[0].delta[0].vertex", 2);
    r.push_f32x3("morph[0].delta[0].d", [0.0, 0.0, 0.0]);
    r.push_u32_le("morph[0].delta[1].vertex", 0);
    r.push_f32x3("morph[0].delta[1].d", [0.5, 0.0, -1.0]);

    r
}

/// Compare actual bytes to the recorded expectation; on mismatch report the first
/// differing offset together with the field it falls inside.
fn assert_bytes_match(actual: &[u8], rec: &Rec, what: &str) {
    let expected = &rec.bytes;
    let common = actual.len().min(expected.len());
    let first_diff = (0..common).find(|&i| actual[i] != expected[i]).or({
        if actual.len() != expected.len() {
            Some(common)
        } else {
            None
        }
    });
    if let Some(off) = first_diff {
        let label = rec
            .labels
            .iter()
            .rev()
            .find(|(o, _)| *o <= off)
            .map(|(o, l)| format!("{l} (field starts at offset {o})"))
            .unwrap_or_else(|| "<before first field>".to_string());
        let a = actual.get(off).map(|b| format!("0x{b:02x}"));
        let e = expected.get(off).map(|b| format!("0x{b:02x}"));
        panic!(
            "{what}: first byte difference at offset {off} in field {label}\n  actual   byte = {a:?}\n  expected byte = {e:?}\n  actual len = {}, expected len = {}",
            actual.len(),
            expected.len()
        );
    }
}

fn assert_rev_id_is_sha256_of(rev: &CharacterRevision, expected: &[u8]) {
    let digest = Sha256::digest(expected);
    assert_eq!(
        rev.rev_id.0.as_slice(),
        digest.as_slice(),
        "rev_id must equal SHA-256 of the spec-derived canonical bytes"
    );
}

// ---------------------------------------------------------------------------
// Test 1: byte-exact layout of the main fixture.
// ---------------------------------------------------------------------------

#[test]
fn canonical_bytes_match_spec_hand_derivation() {
    let fx = main_fixture();
    let rec = expected_bytes(&fx);

    // The builder agrees with the independently hand-computed section offsets.
    assert_eq!(rec.offset_of("prefix"), 0);
    assert_eq!(rec.offset_of("family.id.len"), 21);
    assert_eq!(rec.offset_of("vertices.count"), 37);
    assert_eq!(rec.offset_of("indices.count"), 77);
    assert_eq!(rec.offset_of("joints.count"), 93);
    assert_eq!(rec.offset_of("landmarks.count"), 202);
    assert_eq!(rec.offset_of("inverse_binds.count"), 229);
    assert_eq!(rec.offset_of("skin.vertex_count"), 361);
    assert_eq!(rec.offset_of("morphs.count"), 400);
    assert_eq!(rec.bytes.len(), 449);

    // The fixture really exercises -0.0: the raw (non-canonical) bit pattern of
    // the hand-derived negations differs from +0.0, and the expected buffer
    // holds +0.0 at those positions (IB0 translation x, at least).
    assert_ne!((-0.0f32).to_bits(), 0.0f32.to_bits());
    let ib0_tx = rec
        .labels
        .iter()
        .find(|(_, l)| l == "ib[0].cols[3][0]")
        .unwrap()
        .0;
    assert_eq!(&rec.bytes[ib0_tx..ib0_tx + 4], &[0, 0, 0, 0]);

    let rev = compile_fx(&fx, &MAIN_VALUES);
    let actual = rev.canonical_bytes();
    assert_bytes_match(&actual, &rec, "canonical_bytes");
    assert_eq!(actual, rec.bytes);
    assert_rev_id_is_sha256_of(&rev, &rec.bytes);
}

// ---------------------------------------------------------------------------
// Test 2: rev_id laws from "Body controls".
// ---------------------------------------------------------------------------

#[test]
fn rev_id_ignores_recipe_order_and_explicit_defaults() {
    let fx = main_fixture();
    let base = compile_fx(&fx, &MAIN_VALUES);
    let base_bytes = base.canonical_bytes();

    // Recipe entries in reverse order.
    let reversed = compile_fx(&fx, &[("morph_w", 0.5), ("bone_len", 1.5)]);
    assert_eq!(
        reversed.rev_id.0, base.rev_id.0,
        "reordering recipe entries"
    );
    assert_eq!(reversed.canonical_bytes(), base_bytes);

    // The omitted control "spare" set explicitly to its default 0.25.
    let explicit_default = compile_fx(&fx, &[("bone_len", 1.5), ("morph_w", 0.5), ("spare", 0.25)]);
    assert_eq!(
        explicit_default.rev_id.0, base.rev_id.0,
        "explicit default equals omitted control"
    );
    assert_eq!(explicit_default.canonical_bytes(), base_bytes);

    // Both at once, in a third order.
    let both = compile_fx(&fx, &[("spare", 0.25), ("morph_w", 0.5), ("bone_len", 1.5)]);
    assert_eq!(
        both.rev_id.0, base.rev_id.0,
        "reordered AND explicit default"
    );
    assert_eq!(both.canonical_bytes(), base_bytes);

    // rev_id hashes resolved content, never the recipe: a different recipe that
    // resolves to the same shaped content (morph weights 0.25 + 0.5 = 0.75, same
    // bone length) yields the same rev_id. All values are dyadic, so the
    // accumulation order cannot matter (A6).
    let same_content = compile_fx(&fx, &[("bone_len", 1.5), ("morph_w", 0.25), ("spare", 0.5)]);
    assert_eq!(
        same_content.rev_id.0, base.rev_id.0,
        "recipe is not hashed; only resolved content is"
    );
    assert_eq!(same_content.canonical_bytes(), base_bytes);

    // Discrimination controls: a control that changes shaped geometry changes the
    // bytes and rev_id (so the equalities above are not vacuous).
    let bone_default = compile_fx(&fx, &[("bone_len", 1.0), ("morph_w", 0.5)]);
    assert_ne!(
        bone_default.rev_id.0, base.rev_id.0,
        "bone length changes rev_id"
    );
    assert_ne!(bone_default.canonical_bytes(), base_bytes);

    let more_morph = compile_fx(&fx, &[("bone_len", 1.5), ("morph_w", 0.5), ("spare", 0.5)]);
    assert_ne!(
        more_morph.rev_id.0, base.rev_id.0,
        "morph weight changes rev_id"
    );
    assert_ne!(more_morph.canonical_bytes(), base_bytes);

    // Every variant that is meant to be equal also matches the spec-derived hash.
    let rec = expected_bytes(&fx);
    for rev in [&base, &reversed, &explicit_default, &both, &same_content] {
        assert_rev_id_is_sha256_of(rev, &rec.bytes);
    }
}

// ---------------------------------------------------------------------------
// Test 3: landmark order matters ("reordering ... landmarks changes rev_id").
// ---------------------------------------------------------------------------

#[test]
fn landmark_order_changes_bytes_and_rev_id() {
    // A different family from the main fixture: a 5-byte ASCII id (A4 as amended by
    // ruling #70: ids are [a-z0-9._-]), version 7, two landmarks. Only the landmark list
    // order differs.
    let a: (&'static str, u32, [f32; 3]) = ("a", 0, [0.0, 0.0, 0.5]);
    let b: (&'static str, u32, [f32; 3]) = ("b", 1, [0.25, 0.0, 0.0]);
    let fx_ab = Fixture {
        family_id: "fam.7",
        family_version: 7,
        landmarks: vec![a, b],
    };
    let fx_ba = Fixture {
        family_id: "fam.7",
        family_version: 7,
        landmarks: vec![b, a],
    };

    let rev_ab = compile_fx(&fx_ab, &MAIN_VALUES);
    let rev_ba = compile_fx(&fx_ba, &MAIN_VALUES);

    // Each order matches its own spec-derived bytes and hash.
    let rec_ab = expected_bytes(&fx_ab);
    let rec_ba = expected_bytes(&fx_ba);
    assert_bytes_match(&rev_ab.canonical_bytes(), &rec_ab, "landmarks [a, b]");
    assert_bytes_match(&rev_ba.canonical_bytes(), &rec_ba, "landmarks [b, a]");
    assert_rev_id_is_sha256_of(&rev_ab, &rec_ab.bytes);
    assert_rev_id_is_sha256_of(&rev_ba, &rec_ba.bytes);

    // Swapping the order must change both the bytes and the rev_id, while the
    // length stays the same (same content, permuted).
    assert_ne!(rev_ab.canonical_bytes(), rev_ba.canonical_bytes());
    assert_ne!(rev_ab.rev_id.0, rev_ba.rev_id.0);
    assert_eq!(
        rev_ab.canonical_bytes().len(),
        rev_ba.canonical_bytes().len()
    );
    assert_ne!(rec_ab.bytes, rec_ba.bytes);
}

// ---------------------------------------------------------------------------
// Test 4: the v0 id alphabet (A4 as amended by coordinator ruling #70).
// ---------------------------------------------------------------------------

fn compile_id(
    family_id: &'static str,
    landmark: &'static str,
) -> Result<CharacterRevision, CompileError> {
    let fx = Fixture {
        family_id,
        family_version: 3,
        landmarks: vec![(landmark, 1, [0.25, 0.5, 0.0])],
    };
    let family = build_family(&fx);
    let recipe = BodyRecipe::new(
        FamilyRef::new(fx.family_id, fx.family_version),
        MAIN_VALUES
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect(),
    );
    compile(&recipe, &family)
}

#[test]
fn non_ascii_and_out_of_alphabet_ids_are_rejected() {
    // NFC and NFD spellings of the same visible id would otherwise hash to different rev_ids.
    let nfc: &'static str = "fam\u{e9}";
    let nfd: &'static str = "fame\u{301}";
    const LONG_OK: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const LONG_BAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    assert_eq!(LONG_OK.len(), MAX_ID_LEN);
    assert_eq!(LONG_BAD.len(), MAX_ID_LEN + 1);
    for bad in [nfc, nfd, "Fam", "fam id", "", LONG_BAD, "fam\n"] {
        match compile_id(bad, "tip") {
            Err(CompileError::InvalidId { kind, id }) => {
                assert_eq!(kind, "family");
                assert_eq!(id, bad);
            }
            other => panic!("family id {bad:?} must be InvalidId, got {other:?}"),
        }
    }
    match compile_id("test.fam", "t\u{ee}p") {
        Err(CompileError::InvalidId { kind, .. }) => assert_eq!(kind, "landmark"),
        other => panic!("non-ASCII landmark must be InvalidId, got {other:?}"),
    }
    // Positive controls: the whole alphabet and the length limit are accepted.
    assert!(compile_id("a-z.0_9", "tip").is_ok());
    assert!(compile_id(LONG_OK, "tip").is_ok());
}
