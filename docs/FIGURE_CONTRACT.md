# Figure contract: CharacterRevision, expression requests, asset packs (PARTIALLY FROZEN)

**Status: PARTIALLY FROZEN (GOD FREEZE #63, 2026-11-26).**
- FROZEN: the I-1 input-mapping table, I-2 Law 5 (activation and fit), and the I-3
  `ExpressionRequest` shape and rules. Changes to these need a GOD RULING.
- STILL DRAFT: the `rev.v0` canonical serialization and the body-controls normative sections,
  until the PASS2 audit, the hand-derived bytes test and gate B. Everything else is also DRAFT.

This is a shared interface. Freezing happened only after the consumer's producer-side drafts (I-1 and I-3) have
been cross-read against it and the coordinator issued FREEZE. Rulings #38 Q1 and Q2 and
ruling #48 (the cross-read of the consumer's AvatarRecipe draft, D1–D9) are folded in. It covers three interfaces:
- I-2, `CharacterRevision`, which libgibson owns;
- the libgibson (consumer) side of I-3, `ExpressionRequest`;
- I-4, the `AssetPack` manifest.

It builds on the puppet-runtime RFC (#445) and the Gibson World 2.0 draft (#439). Neither is
merged; this document cites them as design input, not as shipped behavior.

**Public-repo boundary:** synthetic or lawfully distributable assets only. No consumer art,
body measurements, reference images, private paths or approval records appear in libgibson code,
tests or docs.

**Code home:** a new experimental `figure` module (`src/figure/**`), behind the off-by-default
`figure` cargo feature. The default build, the terminal renderer contract, the C ABI and the
MSRV are unchanged. Slice 1 adds **no runtime dependency** (ruling #39; the evaluation is in
`docs/design/figure-dep-eval.md`). `figure-gltf` (gltf-json, default features off, never
`import`) and `figure-vrm` (serde_vrm) are reserved names, and each needs its own ruling.

Claim labels: **SHIPPED** means in a release; **SLICE 1** means planned for the first
implementation slice; **LATER** means reserved shape only, not implemented.

---

## I-2. `CharacterRevision`

### Invariant

> A body edit produces one complete, coherent character revision. That revision holds the
> neutral geometry, the shaped skeleton and landmarks, the inverse binds, the skin weights, the
> collision geometry, the garment fits and the motion calibration, all derived from the same
> inputs. Nothing is ever partially updated.

### Shape (proposed)

```rust
// Slice 1: the body family is in-code synthetic data. The AssetPack (I-4) loader produces
// a BodyFamily in a later slice, without changing compile's laws.
pub fn compile(recipe: &BodyRecipe, family: &BodyFamily) -> Result<CharacterRevision, CompileError>;

#[non_exhaustive]
pub struct CharacterRevision {
    pub rev_id: RevisionId,              // content hash, see below
    pub family: FamilyRef,               // body family id@version
    pub neutral: NeutralMesh,            // positions, normals, indices (stable topology per family)
    pub skeleton: Skeleton,              // joints (parent-before-child), bind transforms, landmarks
    pub inverse_binds: Vec<Mat4>,        // one per joint, derived from the shaped skeleton
    pub skin: SkinWeights,               // ≤ 4 influences per vertex, normalized
    pub morphs: Vec<MorphTarget>,        // named sparse deltas; face and expression targets
    pub colliders: Vec<Collider>,        // LATER: empty in Slice 1
    pub garment_fits: Vec<GarmentFit>,   // LATER: empty in Slice 1; each has a FitStatus
    pub calibration: MotionCalibration,  // LATER: defaulted in Slice 1; derived from the body only
    pub support: SupportClass,           // quality label only, never an activation input
}

#[non_exhaustive]
pub enum FitStatus {                     // per garment, the consumer's names (ruling #48 D1)
    Fit,
    NeedsAdaptation { reason: FitReason },
    Unfit { reason: FitReason },         // carries no fit data
}
```

`BodyRecipe` is generic: a family reference plus `{control_id → value}`, already resolved
from any locks and symmetry links. The consumer's richer `AvatarRecipe` (I-1) is mapped to it on
the consumer side. libgibson never stores identity, personality, memory or approval state.

**Input mapping from `AvatarRecipe` (ruling #48):**

| AvatarRecipe | libgibson | notes |
|---|---|---|
| `body.body_family` (`id@ver`) | `BodyRecipe.family` | `FamilyMismatch` if it differs from the supplied `BodyFamily` |
| `body.controls` (i32 thousandths, -1000..=1000, ≤256) | `BodyRecipe.controls` | converted as `value / 1000` to `f32`; range is checked against the family's declared bounds (`ControlOutOfRange`) |
| `body.locks` | not mapped | a Studio editing constraint only; never reaches `compile` |
| `body.symmetry` | not mapped | resolved by the host before `compile` |
| `owner`, `revision`, `RecipeRef` | not mapped | stripped. Two owners with identical bodies get the same `rev_id`: it is content, not lineage |
| `appearance` (skin, eyes, hair, makeup) | not in `rev_id` | rides beside the revision; materials are LATER. An appearance change is not a new body revision |
| `motion_style` | not a compile input | a runtime posing or realization parameter; not in `rev_id` (D3) |
| `asset_pins` (sha256, lowercase hex) | I-4 per-asset `sha256` | same encoding |
| `wardrobe[]` | `garment_fits` (LATER) | one `FitStatus` per wardrobe entry |

`MotionCalibration` is derived from the body only (bone lengths, limits). It is never derived from
`motion_style`, so a tempo or sway change never mints a new `rev_id`.

### Laws

1. **Atomic.**
   - `compile` returns either a complete revision whose parts are mutually consistent, or a
     typed `CompileError`.
   - No partially built revision is ever observable, and there is no in-place mutation API.
   - An edit is a new `compile`; the previous revision stays valid until the host drops it.
2. **Deterministic.**
   - The same recipe and the same family data give the same `rev_id` and the same derived data,
     bit for bit, on one build.
   - There is no clock, no randomness without an explicit seed, no environment read and no I/O.
3. **Content-hashed.**
   - `rev_id` is a 32-byte digest over a declared canonical serialization: format version,
     family ref, ordered vertices and indices, the skeleton, inverse binds, skin weights, morph
     deltas and the other derived fields.
   - Floats are serialized as little-endian IEEE-754 bit patterns.
   - It identifies bytes only. It does **not** certify likeness, correctness or approval.
   - The serialization is versioned. Changing it changes `rev_id` for unchanged inputs, which
     needs a minor release and a changelog note.
   - The digest is SHA-256, hand-written in `figure::sha256` with no dependency (ruling #39).
     It is checked against the NIST FIPS 180-4 vectors and differentially against the `sha2`
     crate, which is a dev-dependency only. It is a **content identifier, not a security
     primitive**: no constant-time or side-channel claims are made.
   - **Bit-stability scope.** Evaluation is scalar `f32` with no SIMD, no `unsafe` and a fixed
     evaluation order. `rev_id` is bit-stable **on the platforms we test, which today is
     x86_64 only**. Cross-architecture identity is not claimed. A golden `rev_id` test is a
     regression pin: it detects drift in the serialized bytes, and in the float evaluation
     order only to the extent the fixture makes the order visible (see "Verification
     obligations"). It is not an independent check of correctness.
4. **Posing never touches the revision.**
   - Evaluation is `rev.evaluate(&PoseInput) -> PosedMesh`. It takes joint locals or a sampled
     clip, plus morph weights.
   - It borrows the revision immutably, so `rev_id` cannot change.
   - Clothing changes and animation create no new body revision.
5. **Never rescue a fit by reshaping the body.**
   - Each garment is reported in the revision as a typed per-garment outcome, `FitStatus`:
     `Fit`, `NeedsAdaptation { reason }` or `Unfit { reason }`.
   - An unfit garment carries **no** fit data, and is never drawn with a degenerate fit.
   - The neutral body is not altered to make it fit.
   - `compile` still succeeds when a garment is unfit, even a *required* one.
   - A geometry, transform or topology corruption, or invalid controls, are still a
     `CompileError`, never a fit outcome.
   - **Host policy may be STRICTER than this mechanism** (FREEZE #63). For example, the
     consumer refuses any garment that has no fit verdict (its rule R5). libgibson stays
     mechanism-only and never enforces host policy.
   - **Activation is the host's decision, derived ONLY from the per-garment `FitStatus`**
     (ruling #38 Q1, ruling #48 D1). This rule lives in the consumer's avatar-recipe contract:
     - a required `Unfit` garment: the host refuses activation;
     - a required `NeedsAdaptation` garment: activatable, with a visible warning;
     - an optional `Unfit` garment: omitted and listed, never dropped silently.
   - `SupportClass` is a libgibson quality label. It is **never** an activation input.
   - **Slice 1 has no garment fitting** (`garment_fits` is empty). A recipe with a required
     garment therefore gets no fit verdict, and the host refuses it (`NoFitReport`). That is the
     correct fail-closed behaviour, not a bug. A garment-free recipe activates. *(Ruling #48 D2.)*
6. **Typed errors.** `CompileError` is `#[non_exhaustive]` and names the failing part. Examples:
   - `UnknownControl`, `ControlOutOfRange`, `FamilyMismatch`;
   - `SkeletonCycle`, `NonFinite`, `DegenerateBind`, `SkinWeightInvalid`, `MorphTopologyMismatch`;
   - `LimitExceeded`.
   It never panics on bad input.
7. **Bounded.** Hard limits are declared constants and are checked before allocation: joints,
   vertices, morph targets, influences per vertex, clip keys and clip duration.

### Canonical serialization `gibson.figure.rev.v0` (NORMATIVE)

*From the Slice-1 implementation. Ratified by the coordinator only after the independent
oracle/golden audit (PASS1/PASS2) and gate A. Until then this is DRAFT normative text. Once it
is ratified, it is the spec: the implementation and every audit are checked against it.*

`rev_id = SHA-256(bytes)`.

**Encoding.**
- Every integer and every float is **little-endian**.
- `u8`: 1 byte.
- `u32`: 4 bytes, unsigned.
- `i32`: 4 bytes, two's complement.
- `f32`: the IEEE-754 binary32 bit pattern, written as a `u32`, after `-0.0` becomes `+0.0`.
- `str`: a `u32` byte length, then the UTF-8 bytes, with no terminator and no normalization.
- Every count prefix below is a `u32`, except the per-vertex skin influence count, which is a
  `u8`.

**Byte content, not geometry.**
- **`-0.0` is canonicalized to `+0.0`** in every serialized `f32` field (ruling #59; it reverses
  #58 point 3). Ordinary negation, such as `-(0.0)` in an inverse-bind translation, produces
  `-0.0` incidentally. Without this rule, semantically neutral refactors would change `rev_id`.
  A test pins it: `negative_zero_canonicalized_same_rev_id`.
- NaN and infinity cannot occur, because they are a `CompileError`.
- Order matters wherever the family declares an order: reordering influences, deltas, morphs or
  landmarks changes `rev_id`.
- `rev_id` identifies bytes. It does not identify geometric equivalence.

**Layout, in order.**
1. The prefix: the 21 bytes `gibson.figure.rev.v0\0`.
2. The family: `str id`, then `u32 version`.
3. The vertices: `u32 n`, then `n ×` (`f32 x, f32 y, f32 z`), in vertex-index order. These are
   the **shaped** neutral positions, with the compile-time morph weights applied.
4. The indices: `u32 n` (the number of indices, i.e. 3 × triangles), then `n × u32`, in the
   family's order.
5. The joints: `u32 n`, then for each joint in **family order**, which validation requires to be
   parent-before-child:
   - `str name`;
   - `i32 parent` (`-1` for a root);
   - the **shaped** rest translation (`3 × f32`);
   - the rest rotation as `f32 x, y, z, w`;
   - the rest scale (`3 × f32`).
6. The landmarks: `u32 n`, then for each landmark in **family order**: `str name`, `u32 joint`,
   then the offset (`3 × f32`).
7. The inverse binds: `u32 n`, which equals the joint count, then for each joint in joint order
   16 `f32`s, column-major (`cols[0][0..4]`, then `cols[1][0..4]`, and so on).
8. The skin: `u32 vertex_count`. Then for each vertex in vertex order:
   - a `u8 count` (1..=4);
   - `count ×` (`u32 joint`, `f32 weight`), in the family's **declared influence order**. This
     order is NOT sorted by joint. Unused slots are not written.
   - Weights are **not renormalized**. They are serialized exactly as declared, after validation
     has checked that each weight is ≥ 0, that the weights sum to 1 within
     `WEIGHT_SUM_TOLERANCE` (1e-4, with the sum taken in f64), and that no joint repeats.
9. The morphs: `u32 n`. Then for each target in **family order**, which is not sorted by name:
   - `str name`;
   - `u32` delta count;
   - per delta, in the family's **declared order** (NOT sorted by vertex): `u32 vertex`, then
     `3 × f32`.
   - **Zero deltas are kept**, not omitted. A vertex can appear at most once, and validation
     refuses a duplicate.

Colliders, garment fits, calibration and `support` are not serialized in v0. Any change to this
layout, or to what it hashes, is a new prefix version (`…rev.v1\0`). That changes `rev_id` for
unchanged inputs, which means a minor release and a changelog note.

### Body controls (NORMATIVE, Slice 1)

*From the Slice-1 implementation. Ratification follows the same rule as above.*

- A `BodyFamily` declares its controls in order. Each has an `id`, a closed range `[min, max]`, a
  default, and a list of effects.
- A `BodyRecipe` sets values by id. Errors:
  - an unknown id: `UnknownControl`;
  - the same id twice: `DuplicateControl`;
  - a non-finite value: `NonFinite`;
  - a value outside the range: `ControlOutOfRange`, never a clamp.
- A control that is not set takes its default.
- **`rev_id` hashes the resolved, derived content, never the recipe.** The recipe values are not
  serialized at all; only their effect on the shaped geometry is. So an explicit default and an
  omitted control give equal `rev_id`s, and so does any reordering of recipe entries. A test pins
  this: `deterministic_rev_id`.
- Effects are applied in **family-declared control order**, then in each control's effect order.
  - `BoneLengthScale { joint }`: every joint whose parent is `joint` has its rest translation
    multiplied by the control value `v`. `v = 1` leaves the bone unchanged.
  - `MorphWeight { target }`: adds `v` to target `target`'s compile-time weight. Several controls
    on one target accumulate.
- The shaped neutral is `G0 + Σ wᵢ·Δᵢ` over the accumulated compile-time weights. The inverse
  binds are derived from the shaped skeleton: `IB_j = (W_j at bind)⁻¹`.
- A pose-time morph weight (`PoseInput`) adds on top of the shaped neutral. It never changes
  `rev_id`.
- **KNOWN v0 LIMIT.** `BoneLengthScale` moves **no vertices**. That is accepted for the Slice-1
  synthetic skeleton only. The parametric body (M5-C) needs bone-driven vertex displacement, and
  that will be a semantics or layout change under a new prefix version (`v1`). M5-C must not treat
  the v0 control semantics as settled. See "`rev.v1` (DRAFT)" below: the limit is superseded only there.

### Resolved spec ambiguities A1–A8 (from the independent hand-bytes test; RATIFIED by ruling #70, A4 amended)

Each resolution states what the Slice-1 implementation does (src/figure/revision.rs @956ae1e) and
what tests/figure_canonical_bytes.rs assumed; they agree (gate A: 3/3).

- **A1, translation placement.** `Mat4` is column-major, `cols[c][r]`. A transform's translation is
  `cols[3][0..3]`, and `cols[3][3] = 1`. Inverse binds are written `cols[0][0..4]`, then `cols[1]`,
  `cols[2]`, `cols[3]`.
- **A2, landmark offsets.** Written verbatim as declared by the family. They are not shaped:
  `BoneLengthScale` changes joint rest translations only, never landmark offsets. A landmark's
  world position still moves with its joint.
- **A3, morph deltas.** The raw declared delta is written, unscaled by any compile-time or pose-time
  weight. Weights affect only the shaped vertex positions (layout item 3).
- **A4, strings (AMENDED, coordinator ruling #70).** Family, joint, landmark, control and
  morph-target ids use the v0 ASCII alphabet: 1..=64 (`MAX_ID_LEN`) bytes, each in
  `[a-z0-9._-]`. `compile` enforces this with `CompileError::InvalidId { kind, id }`. This
  closes content-address aliasing: NFC and NFD spellings of one visible id would otherwise give
  different `rev_id`s. Morph-target names are included because they are serialized the same
  way. Ids are written as their bytes, which are ASCII. Free-text display strings, if any are
  added later, stay verbatim UTF-8. Test: `non_ascii_and_out_of_alphabet_ids_are_rejected`.
- **A5, unset controls.** An unset control takes its default, and the default flows through its
  effects exactly like a set value. A non-zero default therefore changes the shaped content.
- **A6, accumulation order.** Effects are applied in family control order, then each control's
  effect order. `MorphWeight` contributions are summed in f32, left to right, starting from
  `0.0`. Repeated `BoneLengthScale` effects on one joint multiply the children's translations in
  that order, in f32. The order is part of the bytes for non-dyadic values.
- **A7, indices.** Written verbatim in family order. There is no winding or rotation normalization.
- **A8, unreferenced vertices.** Allowed. Validation requires `indices.len() % 3 == 0` and every
  index `< vertex count`. It does not require every vertex to be referenced, and unreferenced
  vertices are serialized like any other.

### Evaluation order (SLICE 1 subset)

The order is: neutral, then morph targets (`G0 + Σ wᵢ·Δᵢ`), then a sample of the finite clips at
an explicit time, then joint world transforms in parent-before-child order, then linear-blend
skinning `v' = Σ wⱼ · (Wⱼ · IBⱼ) · v`.

Corrective shapes and the pre- or post-skin order of correctives are **LATER**, and are
declared per asset when they land (RFC #445 §3). IK, contacts and secondary motion are also
**LATER**.

### Finite clips (SLICE 1)

- Clips are declarative data: typed channels, keys and finite durations.
- They are sampled with `clip.sample(t)` at an explicit `t`, with no implicit clock and no
  implicit loop. Past the end, a clip holds its last key and reports that it has settled.
- An unknown target fails when the clip is bound to a revision, not silently at sample time.
- Clips contain no scripts.

### Verification obligations (SLICE 1)

- **Independent geometry oracle.**
  - The f32 implementation is checked against a separately written f64 reference: a different
    code path, not the same routine run at a wider float type. The oracle is row-major, uses a
    different quaternion-to-matrix formula and a general (Gauss-Jordan) inverse, and shares no
    code with `figure::math`.
  - The tolerance is declared before the comparison and justified by an error bound, not tuned
    to the result. The bound is `N * u * S`, with `u = 2^-24` (the f32 unit roundoff, not
    epsilon), 7 flops per matrix-entry product (4 multiplications and 3 additions), a dependent
    chain of `N = 64` operations and a magnitude sum `S <= 12`: `64 * 5.96e-8 * 12 = 4.6e-5`,
    against `TOL = 1e-4`. The derivation is written out at `TOL` in `tests/figure_mechanics.rs`.
  - **Fixture strength.** The oracle fixture must have non-identity rest rotations on every
    chain, a non-uniformly scaled joint, non-dyadic lengths, weights and control values, a
    4-influence vertex, and skin weight sums that are inside `WEIGHT_SUM_TOLERANCE` but not
    exact. Landmarks are evaluated and compared. The world bind matrices and the inverse binds
    are compared with the oracle directly, as well as through the posed mesh.
  - `TOL` cannot detect a renormalized weight sum within `1 +- 5e-5`. "No renormalization" is
    checked by an absolute-tolerance test on a large coordinate, and by the stored weights.
- **Mutation controls**, each of which must fail the suite through at least one test that is
  not the golden:
  1. An ignored skeleton update: a control changes a bone length, but the skeleton does not.
     The detectors are: the rest translations, the world bind matrices and the inverse binds
     compared directly with the oracle; the landmark positions; and `rev_id`. The posed mesh
     detects it **only** when the pose replaces the local transform of a joint whose bone
     length the control scaled. Otherwise the scaled local cancels in `W' * IB`, and the mesh
     is identical with or without the update. A suite that relies on the posed mesh alone does
     not detect this fault.
  2. A stale bind: inverse binds left over from an earlier revision. Detected by the inverse
     binds compared with the oracle, by `W' * IB` at the bind pose (the posed mesh is not the
     shaped neutral), and by the posed mesh in any pose.
  3. A no-op morph: a morph weight that has no effect on any vertex. Detected at compile time
     by the shaped neutral positions, and at pose time by a test that supplies a pose-time
     morph weight on a body whose compile-time weights are zero.
- Each control must also visibly change `rev_id` or the posed output, so that the tests are
  shown to discriminate.
- **Golden `rev_id`: REGRESSION PIN (not independent).**
  - `docs/evidence/m5b/golden_rev_id.py` is a numpy float32 replication of the implementation.
    It mirrors the implementation's float evaluation order on purpose, and shares its
    formulas. It pins the documented byte layout and the value content of one fixture and
    recipe. It is not an independent bearing on correctness.
  - What it can detect: a change to the serialized layout or content, and a change in float
    evaluation order that alters an f32 result on that fixture. Measured on the rich fixture
    with the script: reversing the summation order of the 4-term matrix product changes the
    hash, and so does reversing the summation order of the inverse-bind translation. Reversing
    the order of the 3-term determinant does NOT change it. So the claim is limited to those
    orderings; it is not a general "any drift in evaluation order" guarantee, and no
    cross-architecture claim is made.
  - Independent bearings are the f64 oracle comparisons and the hand-derived canonical bytes
    test (`tests/figure_canonical_bytes.rs`), which is written by a separate author from the
    normative text only.

---

## I-3. `ExpressionRequest`, the libgibson (consumer) side

The consumer owns policy: what to express, when, and what takes precedence over what.
libgibson owns realization: how one admitted request becomes finite, settling presentation on
either backend.

The request type is **one** type for both the sprite backend (M5-A: `FrameSequence`,
`LayerSlots`, `CueSchedule`) and the skeletal backend (M5-B and later).

### Shape (proposed, LATER; for agreement now)

```rust
#[non_exhaustive]
pub enum Expression {
    Attend,                       // attentional posture; no payload (ruling #48 D4)
    Look(LookTarget),             // separate from Attend
    Acknowledge,
    Express { affect: AffectId, intensity: u16 },  // thousandths 0..=1000; the affect id is opaque
    Gesture(GestureId),
    Posture(PostureId),
    Speak(SpeakTiming),           // AudioTiming { lead_in_ms, duration_ms } | TextOnly
    Rest,
}
#[non_exhaustive]
pub enum LookTarget {             // symbolic, mapped through the host data table
    Operator,
    Screen,
    Away,
    // LATER: a scene-local point. Never OS or camera input.
}
#[non_exhaustive]
pub struct ExpressionRequest {
    pub expression: Expression,
    pub source: RequestSource,    // Host | Operator; for tracing only, never used for precedence
    pub priority: u8,             // higher wins
    pub deadline: Option<Duration>,  // ABSOLUTE on the presentation clock; late is dropped, never replayed
    pub settle: Settle,
}
pub struct Settle {
    pub ease: Duration,           // a BLEND duration, not a delay (ruling 17); 0 under reduced motion
    pub end: SettleEnd,           // ReturnToRest (default) | Hold
}
```

- `intensity` outside `0..=1000` is a typed error, never a clamp that hides it (D5).
- `deadline` is `Option` for generic hosts. The consumer described in I-1 always sends one (D7).

### Laws for libgibson

- **No inference and no policy.**
  - libgibson does not interpret `AffectId`, choose gestures, or decide admission.
  - The backend maps ids to realizations through a host-supplied, data-only table: id to
    sequence or clip.
  - An unmapped id is a typed error. It does not fall back to a guess.
- **One presentation clock.** This is the Stage clock and the host's explicit time, as used by
  `PresentationDriver` and `CueSchedule`. Neither backend runs its own clock.
- **No catch-up replay.** After a hidden or detached interval, realization resumes from current
  host time. Missed requests and cues are never replayed (M5-A G1 and G4).
- **Modes.**
  - `Still`: rest only, with no cues.
  - `Responsive`: admitted requests only.
  - `Ambient`: requests plus seeded cues. There is **one ambient source** (ruling #48 D9):
    `CueSchedule` only *proposes* times, and the host's producer admits a proposal by emitting an
    ordinary `Posture` or `Gesture` request. libgibson never realizes a cue the host did not turn
    into a request. While the figure is withheld (hidden or detached), nothing is admitted and
    nothing is replayed.
  - Each mode composes with `MotionPreference`. `Reduced` and `None` realize every request as
    its designed static rest state.
- **Hidden quiescence.** A hidden figure schedules no deadline and writes no bytes. This is
  pinned by M5-A G4.
- **Arbitration among already-admitted requests** is mechanical and lives in libgibson: highest
  `priority` first, then the latest in **submission order into the consumer**. There is no
  timestamp comparison, and the ordering is deterministic. *(Coordinator ruling #38 Q2, ruling #48
  D8.)*
  - Admission, source authority and operator-over-host precedence are consumer policy. The
    consumer expresses them **only** through the `priority` it assigns.
  - libgibson never inspects `source` for precedence. `source` is carried for tracing only.

---

## I-4. `AssetPack` manifest

### Shape (proposed)

The manifest is declarative data only, with no scripts, shaders or executable content of any
kind:

```text
schema: gibson.figure.pack.v0
pack: <id>@<semver>
assets:
  - id, kind (mesh|skeleton|morphs|clip|material|garment|texture)
    sha256            # over the exact asset bytes
    license (SPDX)    # per asset; separate from the licence of any authoring tool
    source            # URL, or a reproducible recipe ref (generator + version + seed)
    family_compat     # body family id + version range
    limits            # declared sizes, checked against the hard caps
```

### Laws

- **Fail closed.** Validation rejects:
  - a digest mismatch, a missing licence, an unknown kind, or an unknown or disallowed licence;
  - out-of-range sizes;
  - external URIs and path traversal;
  - NaN or Inf values, invalid indices, cycles and inconsistent inverse binds;
  - an excessive morph or key count.
  A rejection is a typed error before any allocation that depends on asset size.
- **GLB and VRM import (LATER)** goes through a validating loader, and an import produces
  candidate assets only.
- **Import is not fit qualification.** A loaded garment or body still has to pass `compile` and
  per-garment fit before it is `Supported`.
- **Public fixtures** in libgibson are synthetic (generated in tests) or carry a recorded
  permissive per-asset licence. Private consumer assets never enter this repository.

---

## Continuous clips and transitions (C-1) (DRAFT, EXPERIMENTAL)

**Status: DRAFT.** Nothing in this section is frozen or normative, and it changes none of the
sections above: finite clips (`Clip::sample`) and their tests are untouched, and the I-3 shapes
are not edited. The code was drafted without a compiler: it is **not compiled, not run and not
measured**. The predicted results are in `docs/evidence/m5b/c1/KILL_MATRIX_PREDICTED.md` and the
numeric model is `docs/evidence/m5b/c1/model_c1.py`. Claim label: **C-1** (this packet), not
SLICE 1.

**Why.** Owner ruling 17: "make sure we model continuous motion and not just jarring stop
motion". A finite clip that interpolates linearly has a velocity jump at every key, and a
transition that snaps or freezes the old pose has one at its start. This section adds, in the
new module `figure::motion`:
- smooth (C1) key interpolation and loops without a seam;
- transitions between motions that are functions of time and blend over an explicit `ease`;
- preemption that preserves the old velocity (inertialization);
- a generic aim layer whose retargets are inertialized too.

**Laws.**
1. **Pure function of explicit time.** `Motion::sample(t: Duration) -> MotionPose`. There is no
   clock and no state that a call advances: the same inputs give bit-identical output, in any
   order of calls. All state is in the objects: the clip, the start times, the preemption
   instant, the `ease`, the retarget schedule.
2. **C1 at least.** Velocity is continuous across keys, across a loop seam, through a crossfade
   and across a preemption, within the f32 and finite-difference bounds the tests derive.
3. **Blend, never snap.** A transition blends over `ease` (the `Settle.ease` of I-3, a BLEND
   duration, not a delay). `ease == 0` is the one exception and is the designed static
   realization (reduced motion): it switches at once.
4. **No knowledge of the consumer.** The module sees named joints and morphs, durations and
   vectors. It knows nothing of Beings, personalities or the consumer's types, and it does not
   interpret `LookTarget` or any affect: the host maps those to directions and clips.

**Surface** (`gibson::figure::motion`, feature `figure`; all EXPERIMENTAL):
```rust
pub enum Interpolation { Linear, Smooth }
pub enum Repeat { Once, Loop }
pub enum Sample {
    Vec3 { value: Vec3, rate: Vec3 },
    Rotation { value: Quat, omega: Vec3 },   // omega: rad/s, parent frame, q' = 1/2 omega q
    Scalar { value: f32, rate: f32 },
}
pub struct MotionPose { /* tracks: (ChannelTarget, Sample) */ }
impl MotionPose { pub fn new(..); pub fn tracks(&self); pub fn get(&self, &ChannelTarget); pub fn overlay(&self, &MotionPose) -> MotionPose; }
#[non_exhaustive] pub enum MotionError { UnknownChannel { channel }, BadDirection, RetargetNotIncreasing, TooManyRetargets }
pub trait Motion { fn sample(&self, t: Duration) -> MotionPose; }

pub struct MotionClip;   // a Clip + per-channel Interpolation + Repeat
impl MotionClip {
    pub fn new(clip: Clip) -> MotionClip;                                  // Smooth, Once
    pub fn with_interpolation(self, channel: usize, mode: Interpolation) -> Result<MotionClip, MotionError>;
    pub fn with_repeat(self, repeat: Repeat) -> MotionClip;
    pub fn sample_at(&self, t: Duration) -> MotionPose;                    // local time
}
pub struct Playing;      // Playing::new(clip: MotionClip, start: Duration)
pub struct Crossfade<A, B>;  // new(from, to, start, ease); with_rest(MotionPose)
pub struct Preempt<A, B>;    // new(old, new, at, ease);    with_rest(MotionPose)
pub struct Look;         // new(joint, forward, initial, ease) -> Result; retarget(self, at, direction) -> Result
```
`Channel` and `Keys` are unchanged (the interpolation mode lives in `MotionClip`, not in
`Channel`, so no existing struct literal breaks).

**Smooth keys.** Catmull-Rom in the **time-parameterized, uniform-tension** form: a cubic Hermite
per segment with tangent `m_i = (p_{i+1} - p_{i-1}) / (t_{i+1} - t_{i-1})`, one value per key
shared by both segments. The centripetal form is **not** used: it picks the parameter by chord
length to avoid cusps of a *spatial* curve, and here the parameter is explicit time and each
channel is an independent scalar function of it. The tangent is a weighted mean of the two chord
slopes, which gives the bound `|p'| <= 3.5 S` and `|p''| <= 14 S / h_min` the tests use.
- One-shot clips clamp the end tangents to zero, which is the velocity of the hold, so the hold
  joins C1 (before the first key and after the last, value held, rate 0).
- Translations, scales and morph weights are interpolated per component.
- Rotations are interpolated **component-wise on hemisphere-aligned quaternions, then
  renormalized**. This is **not exactly constant angular speed**. Alignment is relative to the
  segment's start key, so authored neighbours more than 180 degrees apart flip the output sign
  between segments (the rotation stays continuous). Values at keys are returned as authored.
- Loops wrap time exactly (integer nanoseconds) with period `Clip::duration` and use wrap
  tangents. A first key at 0 and a last key at the duration are one instant; the last is dropped
  from the cycle and the first key's value wins.

**Crossfade.** `out = (1 - w) A + w B`, `w = 6x^5 - 15x^4 + 10x^3`, `x = (t - start) / ease`
(C2 at both ends). Rates include `w' (B - A)`. Rotations: nlerp with the hemisphere fix and an
analytic angular velocity. Before `start` it is `A`, from `start + ease` exactly `B`.

**Preempt (inertialization).** At the instant `t0` the offset `old(t0) - new(t0)` (rotations: the
shortest-arc delta `q_old q_new^-1`, as a rotation vector `r0`) and its velocity
`old'(t0) - new'(t0)` are captured. The offset then follows the quintic
`offset(x) = x0 h0(x) + v0 T h1(x)`, `x = (t - t0) / T`, with
`h0 = 1 - 10x^3 + 15x^4 - 6x^5` and `h1 = x - 6x^3 + 8x^4 - 3x^5`: it starts at `(x0, v0)` and
ends at zero value, velocity and acceleration. The old pose is **not frozen**: that makes the
velocity drop to zero at `t0`, a visible hitch (coordinator correction). The rotation offset's
rate is `J^-1(r0) (omega_old - R(q_off) omega_new)`, the SO(3) left Jacobian inverse, so angular
velocity is preserved too. Before `t0` the pose is `old`; from `t0 + ease` it is `new`, bit for
bit. A preemption may itself be preempted (the type nests).
- The quintic does not clamp: a large `v0` against `x0` can overshoot. It is a profile, not a
  limit.
- A target only one side names blends to or from the `rest` pose supplied with `with_rest`
  (default: translation 0, rotation identity, scale 1, weight 0). When the blend ends, targets
  the new motion does not name leave the pose; they equal `rest` at that instant, so there is no
  snap provided `rest` is what the consumer applies to an absent target.

**Look.** `Look::new(joint, forward, initial, ease)` aims the joint's `forward` axis (in the
joint's parent space) along a direction by the shortest-arc rotation; `retarget(at, direction)`
adds a command. Each retarget is a preemption between constant rotations, so a retarget in the
middle of an earlier blend preserves angular velocity as well. An antipodal command uses a fixed
perpendicular axis (deterministic). It owns the joint's rotation track: apply it with
`MotionPose::overlay`. Mapping the symbolic `LookTarget` to a direction is the host's data table
(see I-3); a scene-local point stays LATER. At most `MAX_RETARGETS` (4096) retargets.

**Verification obligations (C-1).**
- Tests: `tests/figure_motion.rs`. Every continuity assertion is a finite-difference velocity
  jump on a 1 ms grid, against a bound derived from the fixture's key data (largest chord slope
  `S`, shortest chord `h_min`, the quintic's fixed extrema) plus the roundoff term `4 u P / dt`.
  The derivation is the module comment of that file. Tolerances are declared before any
  comparison and never tuned. Where a mutant would only be detected if the motion is fast, the
  test asserts that premise (the old speed is above four times the bound).
- Mutation controls (`RUSTFLAGS='--cfg figure_mutant="..."'`, listed in `Cargo.toml`):
  `snap_transition` (no blend), `freeze_on_preempt` (old pose held, velocity lost),
  `linear_keys` (Smooth falls back to linear). Each is killed by at least one assertion.
  Predicted matrix: `docs/evidence/m5b/c1/KILL_MATRIX_PREDICTED.md`.
- **Not yet verified:** nothing here has been compiled or run. The model is f64 and shares the
  design with the Rust; it is not an independent bearing on the Rust.

**Known limits and LATER.**
- No `MotionPose` to `PoseInput` conversion (joint index binding and composing T, R and S over
  the rest locals) yet.
- Rotation offsets assume unit rotations; a raw key whose length is within the 1e-3 tolerance
  but not 1 gives a value step of that size when preempted exactly at the key.
- `Look` does not know the joint hierarchy or limits and does not constrain the neck; it is an
  aim, not an IK solver.
- Corrective shapes, IK, contacts and secondary motion remain LATER (see Evaluation order).

---

## `rev.v1` (DRAFT): coupled deformation, skinning mode and three identities

**Status: DRAFT, DESIGN ONLY.** Nothing in this section is frozen. Slice 1 of the M5-C deform
packet writes it down BEFORE any v1 code exists, so that serialization and cache identity are
decided first and the code is built to them. The names below are proposals. The only code that
accompanies this section is the DQS skinning path (see "Skinning mode"), which is additive to v0
and lands in Slice 2. Nothing here was compiled.

### What does not change (v0 is untouched)

- `gibson.figure.rev.v0\0`, the layout above, `compile`, `BodyFamily`, `ControlEffect`, the
  ratified resolutions A1 to A8 and the golden `rev_id` stay **bit-identical**. No v0 test, byte
  or fixture is edited by anything in this section.
- `BoneLengthScale` keeps its v0 meaning, including the KNOWN v0 LIMIT (it moves no vertices). That
  limit is superseded **only in v1**, by the coupled derivation below. v0 revisions stay valid and
  evaluable. There is no automatic upgrade, and no v0 revision is ever re-labelled as v1.
- A v1 `rev_id` is never equal to a v0 `rev_id` of "the same body": the prefixes differ, so the
  digests are unrelated by construction. Migration is an explicit `compile_v1` of a v1 family.

### Why a new version and not an extension

Coupled deformation changes what the skeleton and the inverse binds *are* (derived from the
surface, not declared), and the skinning mode changes what the same pose means. Both change the
bytes a revision hashes and the geometry a pose produces. Law 3 requires that to be a new
explicitly versioned layout. Appending optional fields to v0 would either leave the new inputs
out of `rev_id` (two different bodies with one id) or change v0 ids for unchanged inputs.

### Inputs (proposed `BodyFamilyV1`)

All of it is data in the family, never in the recipe:

- the **neutral mesh**: positions and a triangle index buffer, as v0;
- **per-control target deltas**: sparse morph targets as v0 (`MorphTarget`), driven by controls
  whose only v1 effect is `MorphWeight`. `BoneLengthScale` is **not accepted in v1**
  (`CompileErrorV1::LegacyEffect`): the skeleton changes only through the surface, so a control
  can no longer change a bone without moving a vertex;
- **joint-landmark vertex groups**: each joint may carry an anchor group, an ordered list of vertex
  indices (non-empty, in range, no repeats within a group). Surface landmarks (a name plus a
  group, used to measure a control's effect) use the same mechanism;
- the joint list, parents, rest rotations, rest scales and v0-style landmark offsets, as v0;
- the skin weights, as v0 (at most 4 influences, never renormalized);
- the **skinning mode**.

### Coupled derivation (compile order)

Validation precedes derived data, and nothing is observable until all of it succeeds (law 1).
After the v0 validation steps for mesh, morphs, skin and controls:

1. Resolve control values (defaults, range check, no clamp) and accumulate morph weights in
   family-declared control order, exactly as v0 (A5, A6).
2. Shape the surface: `G = G0 + sum(w_i * delta_i)`, as v0.
3. **Re-derive the joints from the shaped surface.** Walking joints in parent-before-child order:
   - an anchored joint `j` has bind world position `c_j`, the centroid of its group's SHAPED
     positions: the three sums run over the group in its declared order, in `f32`, then one
     division by the count converted to `f32`;
   - its derived rest local translation is `W_parent_bind^-1` applied to `c_j`, where
     `W_parent_bind` is the already derived parent world bind matrix (a root has none);
   - an unanchored joint keeps its declared local translation, so it rides with its parent;
   - the rest rotation and rest scale are the declared values. **Orientation and scale are not
     re-derived** (see Limits).
   *Implemented in Slice 3 (UNCOMPILED until a heavy lease) as `figure::refit`:
   `refit_skeleton(&Skeleton, &[JointAnchor], &[Vec3]) -> Result<Refit, RefitError>`, where the
   positions are the SHAPED vertices and `Refit { skeleton, inverse_binds }`; it is not yet called
   by `compile`. Limits `MAX_GROUP_VERTICES` 4096 and `MAX_GROUP_TOTAL` 65536 are enforced.
   Evidence: `tests/figure_refit.rs`, mutant `ignored_refit`.*
4. **Bone lengths follow the surface**: the length of a bone is the length of its child's derived
   local translation. It is an output, never an input.
5. Recompute the world bind matrices and the **inverse binds** from the derived skeleton
   (`IB_j = (W_j at bind)^-1`, as v0). A stale or ignored derivation is the v1 form of the v0
   mutants `ignored_skeleton_update` and `stale_bind`; each gets an equivalent v1 control.
6. Weights, colliders and motion calibration are derived from the same shaped content in the
   same compile. Colliders and calibration stay LATER; the contract keeps their hooks.
7. Serialize and hash (below).

The evaluation order of the `f32` code stays the v0 law: scalar, no SIMD, no `unsafe`, no fused
multiply-add, no reassociation, sums in ascending index order. The centroid is a sum in group
declaration order, which is part of the bytes for non-dyadic data. Bit-stability remains claimed
on the tested platform only (x86_64).

### Skinning mode

```rust
#[non_exhaustive]
pub enum SkinningMode { Linear /* default */, DualQuat }
```

- **Linear** is the v0 behaviour: `v' = sum(w_j * (W_j * IB_j) * v)`.
- **DualQuat** blends the rigid part of each skin matrix `M_j = W_j * IB_j` as a unit dual
  quaternion: convert `M_j` to a unit dual quaternion, align the sign of every influence to the
  FIRST influence in the vertex's declared order (negate when the 4-D dot product of the real
  parts is below zero; exactly zero is not flipped), sum `w_j * dq_j` in declared order,
  normalize by the dual-number norm, and apply to the point (rotation, then the translation read
  from the dual part) or to the normal (rotation only). The result does not depend on the weight
  sum, so DQS is the one place where the weights are effectively renormalized, by the blend.
- **Scale and shear in skin matrices (decision: reject).** A dual quaternion cannot represent
  scale or shear. When any joint that a vertex references has a skin matrix whose 3x3 block is
  not a rotation within `RIGID_TOLERANCE` (1e-3 on `|R^T R - I|` and on `|det R - 1|`), DualQuat
  skinning returns the typed `SkinError::NonRigid { joint }`. It never drops the scale silently
  and never applies it "separately". Reason: a separate scale pass has to choose an order (before
  or after the blend) and a pivot, and no choice is right for non-uniform scale in a hierarchy; a
  silent drop would make the mode's own acceptance check pass on a body it deforms wrongly. A
  Rigid skin matrices are the norm: every joint gives `M_j = I` at the bind pose, whatever its
  rest scale. A pose that keeps each local scale at its rest value and rotates only joints whose
  ancestors have uniform scale stays rigid. A non-uniformly scaled ancestor whose rotation changes
  gives its descendants a sheared skin matrix, and that is exactly the rejected case. Squash and
  stretch is not a v1 feature and would be a new mode.
- **Where the mode lives.** In v1 it is a field of the revision and part of the neutral-body
  `rev_id`, because it decides which deformation the body was validated against. For a v0
  revision, which cannot gain a hashed field, it is an evaluation argument:
  `evaluate_with(&PoseInput, SkinningMode)`. `evaluate` is `evaluate_with(.., Linear)` and is
  unchanged, bit for bit.
- **Implemented in Slice 2 (UNCOMPILED until a heavy lease).** `figure::skin::SkinningMode`
  (`#[non_exhaustive]`, `Linear` default, `DualQuat`); `SkinWeights::skin_with(mode, ..)` and
  `SkinWeights::skin_normals(mode, ..)`; `CharacterRevision::evaluate_with(&PoseInput, mode)`;
  `figure::math::DualQuat` (`from_rigid`, `from_mat4`, `aligned_to`, `blend`, `try_normalize`,
  `translation`, `transform_point`, `transform_vector`) with `RIGID_TOLERANCE` and
  `DEGENERATE_BLEND_NORM2`; `SkinError::{NonRigid, DegenerateBlend}`. Evidence:
  `tests/figure_dqs.rs`, `docs/evidence/m5c/` (predicted kill matrix, model, `gate_dqs.sh`).
- A blend whose real part cancels to (near) zero is the typed `SkinError::DegenerateBlend`.
  DQS is genuinely discontinuous where two influences are exactly 180 degrees apart (the shortest
  path is ambiguous); alignment removes the sign ambiguity of the representation, not that one.

### Canonical serialization `gibson.figure.rev.v1` (DRAFT layout)

Encoding (little-endian, `f32` bit patterns with `-0.0` written as `+0.0`, `str` as `u32` length
plus bytes, the v0 id alphabet) is exactly the v0 encoding. `body_rev_id = SHA-256(bytes)`.

1. The prefix: the 21 bytes `gibson.figure.rev.v1\0`.
2. `u8` skinning mode: `0` Linear, `1` DualQuat. Any other value is invalid.
3. The family: `str id`, `u32 version`.
4. The vertices (SHAPED positions) and the indices, as v0 items 3 and 4.
5. The joints: `u32 n`; per joint, in family order: `str name`, `i32 parent`, `u8 anchored`;
   if anchored, `u32 count` then `count x u32` vertex indices in declared order; then the DERIVED
   rest translation (3 x `f32`), the rest rotation (xyzw) and the rest scale (3 x `f32`).
6. The v0-style landmarks, as v0 item 6, then the surface landmarks: `u32 n`; per landmark
   `str name`, `u32 count`, `count x u32` vertex indices. The measured points are derived from
   hashed content and are not serialized.
7. The inverse binds, the skin and the morphs, as v0 items 7, 8 and 9 (declared influence order,
   declared delta order, zero deltas kept, nothing renormalized).

Control values and the recipe are not serialized (as v0). Two recipes that resolve to the same
shaped content, such as an explicit default and an omitted control, give one id.

### Three identities (decided here, before extension)

| Id | Meaning | Digest input | Prefix |
|---|---|---|---|
| neutral-body `rev_id` (`BodyRevId`) | the body and its controls | the bytes above | `gibson.figure.rev.v1\0` |
| fit id (`FitId`) | one garment-fit computation | `BodyRevId` (32 bytes), the garment set, the fit params | `gibson.figure.fit.v1\0` |
| presentation-cache id (`PresentationId`) | one cached look | `FitId` (32 bytes), the material and style params | `gibson.figure.presentation.v1\0` |

- **Fit id input, in order.** `BodyRevId`; `u32` garment count; per garment, in the HOST-DECLARED
  order (layering is semantic, so a reorder is a different fit): `str garment id`, then the
  32-byte SHA-256 of the garment asset's own canonical bytes (garment content, not its name); then
  the fit params as a versioned block: `str algorithm id`, `u32` parameter count and each
  parameter as `str name` plus `f32` bits. The result of fitting (each `FitStatus` and the fitted
  geometry) is a deterministic function of these inputs and is NOT hashed into the id; a golden
  and an independent oracle pin the results (obligations below). A garment id repeated in a set is
  an error.
- **Presentation id input, in order.** `FitId`; `u32` material-binding count and per binding, in
  declared order: `str slot`, `str material id`, the 32-byte SHA-256 of the material or texture
  bytes, a `u32` count of `f32` parameters (colours, roughness, and so on) and the parameters;
  then the style params block (hair, skin tone and other appearance values, as `str name` plus
  `f32`). `motion_style` and any pose or performance parameter are in **none** of the three ids
  (ruling D3 stays): the performance tape carries its own digest.
- **Domain separation.** The prefixes differ and each begins with its own version, so a body id, a
  fit id and a presentation id can never be confused for one another, whatever the content.

**Invariants** (each becomes a test with a control that must fail; they are the acceptance rows
"determinism + revision isolation" of the incarnate plan):
1. `BodyRevId` depends only on the family content, the resolved controls and the skinning mode. It
   does not depend on any garment, material, style, motion style or recipe lineage.
2. **An outfit edit leaves the neutral `rev_id` bit-identical.** Adding, removing or reordering a
   garment, or changing the fit params, changes `FitId` and `PresentationId`, and not `BodyRevId`.
3. **A material or style edit leaves `BodyRevId` and `FitId` bit-identical** and changes only
   `PresentationId`.
4. A body edit (any control, any family content, the mode) changes `BodyRevId`, and through it
   `FitId` and `PresentationId`. A fit result is never carried across body ids. After a body edit
   a garment has no verdict until it is refitted, and **a garment with no verdict is not drawn**
   (law 5, unchanged). `compile_v1` therefore returns the body only: it holds no `garment_fits`.
   Fitting is a separate step that takes the body and returns a `FitReport { fit_id, entries }`.
5. Caches are keyed by id equality alone. There is no time stamp, no counter and no "latest".

### Limits (proposed constants; each checked before allocation; any raise needs a ruling)

`MAX_GROUP_VERTICES` 4096 per group, `MAX_GROUP_TOTAL` 65536 indices across all groups,
`MAX_GARMENTS` 64 per set, `MAX_FIT_PARAMS` 64, `MAX_MATERIAL_BINDINGS` 256, `MAX_STYLE_PARAMS`
256. The v0 caps (`MAX_JOINTS` 256, `MAX_SKIN_VERTICES` 262144, `MAX_MORPH_TARGETS`, the morph
delta cap, the id alphabet and length) carry over unchanged.

### Out of scope for v1 and still v0-only or LATER

- **v0-only:** the `BoneLengthScale` effect and its limit; `CharacterRevision` with a
  `garment_fits` field; `rev.v0` ids; `evaluate` as the Linear default.
- **Not re-derived in v1:** joint orientation and scale (only positions follow the surface), so a
  large shape change can leave a bone's axis a little off its limb. Pose-space correctives,
  colliders and motion calibration are LATER and are not hashed.
- **Not defined here:** the garment asset layout and the fit algorithm (Lane B), the glTF or VRM
  interchange, and any cross-architecture identity claim.
- **Verification obligations when v1 lands** (not met by this draft): a golden `body_rev_id` by an
  independent numpy replication, a hand-derived bytes test written from this text by a separate
  author, an f64 oracle for the derived joints and inverse binds, and mutation controls
  `ignored_refit` (the derivation is skipped) and `stale_fit` (a verdict reused across body ids),
  each killed by a non-golden assertion.

### rev.v1 asset input format (`gibson.figure.asset.v1`) and the studio compile (SLICE 4, DRAFT)

*Implemented in Slice 4 (UNCOMPILED until a heavy lease) as `figure::studio` (`src/figure/studio.rs`):
`RecipeV1`, `AssetSet`, `compile(&RecipeV1, &AssetSet) -> Result<Compiled, CompileError>`,
`Compiled::pose_vertices(&TapeFrame)`. Evidence: `tests/figure_studio.rs` (synthetic fixture
`tests/fixtures/figure_studio`, written by `tools/figure_assets/convert_to_gibson.py fixture`),
mutants `ignore_skeleton_update_v1`, `ignore_surface_update`, `stale_fit`,
`docs/evidence/m5c/KILL_MATRIX_STUDIO_PREDICTED.md`, `gate_studio.sh`, and the independent f64
`docs/evidence/m5c/oracle_shape.py`. It follows the sections above where they are decided and
records here every place where the data forced a choice. v0 is untouched.*

**Files.** `convert_to_gibson.py convert` turns the `figure-asset/1` export into `index.txt`
(`format`, `body <file> <sha256>`, `part <id> <kind> <file> <sha256>`), `body.gfa` and
`part.<id>.gfa`. All integers little-endian, `str` = `u32` length + UTF-8, `f32` = IEEE bits. The
file sha256s are verified on load. The converter is a pure function of the export bytes.

`body.gfa`: prefix `gibson.figure.asset.body.v1\0`; `str` asset id; `u32` version; `u8` grounded;
`u32` n_vertices (the full index space, helpers included); `u32` n_body (indices below it are the
body, skinned and in triangles; the rest are helper vertices); the neutral positions; the triangle
indices (body vertices only); the joints (`str` name, `i32` parent, declared rest translation in
the parent frame, rest rotation xyzw in the parent frame, rest scale, the anchor group as `u32`
count + indices); the skin of the n_body vertices (`u8` count 1..4, then `u32` joint + `f32` weight,
descending, zero weights dropped, as exported: top 4); the controls (`str` name, `f32` min, max,
default, then a negative and a positive sparse delta list `u32` vertex + 3 `f32`); the surface
landmark groups (`str` name, `u32` count, indices).

`part.<id>.gfa` (a garment, hair, eyes, brows, lashes, teeth, skirt): prefix
`gibson.figure.asset.part.v1\0`; `str` id and kind; the neutral fit (reference); the triangles;
per vertex a binding `u32 v0, v1, v2`, `f32 w0, w1, w2`, `f32 ox, oy, oz`; the offset spec (0 or 3
entries: a constant, or a pair `a, b, axis, ref`); per vertex a skin entry (transferred from the
bound body vertices by the export); the body vertices hidden under the part.

**Decisions the data forced (each is a deviation from, or an addition to, the text above).**

1. **Bipolar controls.** A control value `c` lies in `[min, max]` (the export: `[-1, 1]`, default
   0) and applies `|c| * delta[side(c)]`, the positive side for `c > 0`, the negative for `c < 0`,
   neither for 0. The two sides are two morph targets (`<name>.neg`, `<name>.pos`) per control,
   both hashed (v0 item 9), so the effect is still `MorphWeight`; the weights are `|c|` and 0. A
   value outside the domain is NOT clamped: it scales the side linearly and compile reports
   `Diagnostic::ControlOutOfDomain`. The geometry check is a finite surface and a successful refit.
2. **Garment binding is the MakeHuman `.mhclo` rule, not an offset along an interpolated normal.**
   Most garments of the real asset bind to HELPER vertices (no triangles, hence no normals), and
   the offset is scaled per axis by the distance between two body vertices. Fit:
   `w0 P[v0] + w1 P[v1] + w2 P[v2] + (ox sx, oy sy, oz sz)`, `s_k` a constant or
   `|P[a][axis] - P[b][axis]| / ref`, `P` the SHAPED positions in the full index space, `f32` left
   to right. This is exactly what the export's Blender-side fit does, so the two can be compared.
   Garments are skinned by the export's own per-vertex weights (copied from the bound body
   vertices), so they follow the pose; hair is fitted the same way but has no verdict.
3. **Ground lift.** The export's controls are translation-free and the shaped body is re-grounded
   (`oracle_shaped.json`). When the asset says `grounded`, compile adds `-min z (body vertices)` to
   every vertex (helpers included) AFTER shaping and BEFORE the refit, so the lowest body vertex is at
   `z = 0` and the skeleton is derived from the lifted surface. On the real asset this reproduces
   Blender's `oracle.shaped_positions` to 1.2e-7 m (`oracle_shape.py --blender-oracle`).
4. **Body id additions.** `BodyRevId` hashes, in the contract's layout (the v1 prefix, the mode byte,
   the family `str id` + `u32 version`), also the 32-byte sha256 of the body file straight after the
   version ("neutral asset sha", docs/design/m5c-studio-api.md section 2), then the SHAPED positions
   of all vertices (helpers included), the indices, the joints (with the anchor group and the DERIVED
   rest translation), zero v0-style landmarks, the surface landmark groups, the inverse binds, the
   skin of the body vertices, the morph targets (both sides of every control) and, last, the two
   footwear floats (heel height, toe pitch). The footwear is hashed because the studio API puts it
   in the body id; it has no geometric effect in v1.
5. **Fit id.** As specified: the body id, `u32` garment count, per garment `str id` + the sha256 of
   the garment FILE, then `str "gibson.fit.vertex-normal-penetration.v1"`, `u32 2`, the parameters
   `tolerance_m` and `adapt_depth_m` (`str` name + `f32`). The verdict and the geometry are not
   hashed.
6. **Presentation id.** The fit id, then in order: the material bindings (`str` slot, `str` material
   id, the 32-byte texture sha, `u32 10`, albedo x3, roughness, metalness, then an emissive block of
   a 1.0/0.0 flag, colour x3, intensity), the garments' slot bindings (`u32` garment count; per
   garment `str` id, `u32` count, pairs of `str`), the hair (`u8` present, `str` style id and the
   32-byte sha of the hair file if present, then the colour x3 always), the attachments (`u32` count;
   per attachment `str` id, `u8` anchor kind 0 landmark / 1 bone, `str` anchor, `u8` emissive flag and
   colour x3), then the style parameters (`u32` count; `str` name + `f32`), the last of which is always
   `lower_face_covered` (1.0 or 0.0). `motion_style` and the provenance passthrough are in none of the
   three ids (the studio API lists "style" under the presentation id; this text and ruling D3 say
   the motion style is in none, and the code follows this text).
7. **FitStatus.** `figure::studio::FitStatus` is `Fitted`, `NeedsAdaptation(FitReason)` or
   `Rejected(FitReason)` (a different type from `figure::revision::FitStatus`, whose v0 enum stays
   as it is). The measure: the body's area-weighted vertex normals of the SHAPED triangles; for
   each garment vertex the nearest body vertex (ties to the lowest index) and the signed distance
   along that normal; negative is inside. `penetrating` counts the vertices deeper than
   `tolerance_m` (default 0.002), `max_depth_m` is the deepest. None penetrating: `Fitted`. The
   deepest at most `adapt_depth_m` (default 0.01): `NeedsAdaptation`. Deeper: `Rejected`. Only
   `Fitted` garments are in `Compiled::drawable_garments()` and in `pose_vertices`; every verdict is
   in `Compiled::fit_status()`, and each undrawn garment has a `Diagnostic::GarmentNotDrawn`. The
   thresholds are NOT calibrated: on the real asset at the neutral body the oracle reports 157 of
   2197 `female_casualsuit01` vertices deeper than 2 mm (deepest 9.7 mm). They are fit parameters and
   change the fit id.
8. **Pose input.** `TapeFrame { root_t: Vec3, locals: Vec<Transform> }`: one local transform per
   joint in skeleton order, REPLACING the rest locals (the tape's `local_t3`, `local_r4`, `local_s3`),
   plus a translation of the whole figure after skinning (`root_t3`). It is a studio-local type; the
   tape module owns the file format.
9. **Orientation is carried.** The asset's per-bone world rotations come from the export's
   `rest_matrix`; the converter stores the local rotation `conj(q_parent_world) * q_world` and
   the declared local translation `R_parent^T (head - head_parent)`. Compile re-derives only the
   translations (the Limits section above).
10. **Body and fit in one call.** Invariant 4 above says `compile_v1` returns the body only and
    fitting is a separate step. `studio::compile` is the studio's one entry point and does both, in
    that order, but the two never share state: the body id is computed before any garment is read, the
    fit is a pure function of the shaped body and the garment set, and the three ids are returned
    separately. A host that wants the split can call `compile` with an empty garment list.

---

## Placement and release

- Everything is new types in a new feature-gated experimental module, so it is additive under
  Release Contract §1 and patch-eligible.
- No existing all-public struct or exhaustive enum gains a field or variant.
- No C ABI change.
- Release timing is the coordinator's decision.
