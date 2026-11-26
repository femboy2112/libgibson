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
     x86_64 only**. Cross-architecture identity is not claimed. A golden `rev_id` test catches
     any drift in float evaluation order.
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
  the v0 control semantics as settled.

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
    code path, not the same routine run at a wider float type.
  - The tolerance is declared before the comparison and justified by an error bound, not tuned
    to the result.
- **Mutation controls**, each of which must fail the suite:
  1. An ignored skeleton update: a control changes a bone length, but the inverse binds or the
     posed mesh do not change.
  2. A stale bind: inverse binds left over from an earlier revision.
  3. A no-op morph: a morph weight that has no effect on any vertex.
- Each control must also visibly change `rev_id` or the posed output, so that the tests are
  shown to discriminate.

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

## Placement and release

- Everything is new types in a new feature-gated experimental module, so it is additive under
  Release Contract §1 and patch-eligible.
- No existing all-public struct or exhaustive enum gains a field or variant.
- No C ABI change.
- Release timing is the coordinator's decision.
