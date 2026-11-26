# Figure contract: CharacterRevision, expression requests, asset packs (DRAFT)

**Status: DRAFT, not frozen.** This is a shared interface. The coordinator reviews it before it
is frozen, and changes after freezing need a coordinator ruling. It covers three interfaces:
- I-2, `CharacterRevision`, which libgibson owns;
- the libgibson (consumer) side of I-3, `ExpressionRequest`;
- I-4, the `AssetPack` manifest.

It builds on the puppet-runtime RFC (#445) and the Gibson World 2.0 draft (#439). Neither is
merged; this document cites them as design input, not as shipped behavior.

**Public-repo boundary:** synthetic or lawfully distributable assets only. No consumer art,
body measurements, reference images, private paths or approval records appear in libgibson code,
tests or docs.

**Code home:** a new experimental `figure` module (`src/figure/**`), behind an off-by-default
cargo feature. The default build, the terminal renderer contract, the C ABI and the MSRV are
unchanged. New dependencies are optional and enabled only by that feature. The dependency
evaluation and ruling come before any of them is added.

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
pub fn compile(recipe: &BodyRecipe, pack: &AssetPack) -> Result<CharacterRevision, CompileError>;

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
    pub garment_fits: Vec<GarmentFit>,   // LATER: empty in Slice 1
    pub calibration: MotionCalibration,  // LATER: defaulted in Slice 1
    pub support: SupportClass,           // Supported | NeedsAdaptation | Experimental
}
```

`BodyRecipe` is generic: a family reference plus `{control_id → value}`, already resolved
from any locks and symmetry links. The consumer's richer `AvatarRecipe` (I-1) is mapped to it on
the consumer side. libgibson never stores identity, personality, memory or approval state.

### Laws

1. **Atomic.**
   - `compile` returns either a complete revision whose parts are mutually consistent, or a
     typed `CompileError`.
   - No partially built revision is ever observable, and there is no in-place mutation API.
   - An edit is a new `compile`; the previous revision stays valid until the host drops it.
2. **Deterministic.**
   - The same recipe and the same pack bytes give the same `rev_id` and the same derived data,
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
   - The digest crate is pending the dependency ruling.
4. **Posing never touches the revision.**
   - Evaluation is `rev.evaluate(&PoseInput) -> PosedMesh`. It takes joint locals or a sampled
     clip, plus morph weights.
   - It borrows the revision immutably, so `rev_id` cannot change.
   - Clothing changes and animation create no new body revision.
5. **Never rescue a fit by reshaping the body.**
   - A garment that does not fit is reported as a typed per-garment outcome
     (`GarmentFit::NeedsAdaptation { reason }`).
   - The neutral body is not altered to make it fit.
   - Body errors (invalid controls, a degenerate skeleton) fail the whole `compile`.
   - **Open question for the coordinator:** should an unfit *required* garment fail `compile`,
     or set `support = NeedsAdaptation`? The draft takes the second option.
6. **Typed errors.** `CompileError` is `#[non_exhaustive]` and names the failing part. Examples:
   - `UnknownControl`, `ControlOutOfRange`, `FamilyMismatch`;
   - `SkeletonCycle`, `NonFinite`, `DegenerateBind`, `SkinWeightInvalid`, `MorphTopologyMismatch`;
   - `LimitExceeded`.
   It never panics on bad input.
7. **Bounded.** Hard limits are declared constants and are checked before allocation: joints,
   vertices, morph targets, influences per vertex, clip keys and clip duration.

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
    Attend,
    Look(LookTarget),             // a scene-local target or a vector; never the OS or a camera
    Acknowledge,
    Express { affect: AffectId, intensity: f32 },  // the affect id is opaque to libgibson
    Gesture(GestureId),
    Posture(PostureId),
    Speak(SpeakTiming),           // AudioTiming(..) | TextOnly
    Rest,
}
#[non_exhaustive]
pub struct ExpressionRequest {
    pub expression: Expression,
    pub source: RequestSource,    // Host | Operator
    pub priority: u8,
    pub deadline: Option<Duration>,  // on the presentation clock; a late request is dropped, never replayed
    pub settle: Settle,           // how the realization ends (return to rest, hold)
}
```

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
  - `Ambient`: requests plus seeded cues.
  - Each mode composes with `MotionPreference`. `Reduced` and `None` realize every request as
    its designed static rest state.
- **Hidden quiescence.** A hidden figure schedules no deadline and writes no bytes. This is
  pinned by M5-A G4.
- **Arbitration among already-admitted requests** is mechanical: highest `priority` first, then
  latest-issued.
  - **Open question for the coordinator:** is even this host-side? The draft keeps it in
    libgibson because it is deterministic and policy-free.

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
