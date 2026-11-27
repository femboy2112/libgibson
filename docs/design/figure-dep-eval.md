# Figure module: dependency evaluation (2026-11-26)

**Status:** input for the coordinator ruling, not a decision.
- The method was read-only crates.io API lookups, with no builds.
- Dependency counts are approximate (an API walk).
- MSRV values are the declared fields only.
- Reviewer spot-check: the sha2 and glam MSRV and licence rows were re-verified against the crates.io versions API.


Target: licence `MIT OR Apache-2.0`, `rust-version = "1.85"`, edition 2021, GitHub-only releases.
Method: crates.io API only (`/crates`, `/versions`, `/<ver>/dependencies`). Script: `tools/walk.py`; raw cache `cache/`.
"approx (API walk)" = recursive walk of normal, non-optional deps (newest semver-compatible, optional deps only if enabled by the feature closure; build-deps and dev-deps NOT counted; duplicates by version counted separately). MSRV = `rust_version` field of the crates.io version record; "undeclared" = absent. Behaviour/API claims marked (K) are from prior knowledge, not verified this session.

## Table A: math / import / hash

| crate | latest (date) | licence | declared MSRV vs 1.85 | transitive deps off / on (approx, API walk) | build cost | unsafe/SIMD/build.rs |
|---|---|---|---|---|---|---|
| glam | 0.34.3 (2026-11-08) | MIT OR Apache-2.0 | 1.85.0 (== ours; 0.33.x was 1.68.2) | 0 / 0 (default = `std`,`all-types`; all deps optional) | small (all-types = lots of generic-free but large type surface; no proc-macro) | unsafe + SSE2/NEON/wasm SIMD on by default; `scalar-math` disables SIMD; no build.rs |
| mint | 0.5.9 (2022-02-02) | MIT | undeclared | 0 / 0 | tiny | no unsafe concerns known (K); interop types only, no math |
| gltf | 1.4.1 (2024-05-10) | MIT OR Apache-2.0 | 1.61 declared (but newest serde/serde_json/syn in walk likely need more; unverified) | 17 / 37 | medium (proc-macro `gltf-derive`+serde_derive+syn) ; default adds image stack: heavy | no build.rs known; image/zune/png bring unsafe/SIMD |
| gltf-json | 1.4.1 (2024-05-10) | MIT OR Apache-2.0 | 1.61 | 14 / 14 | medium (serde_derive + gltf-derive proc-macros) | none known |
| nalgebra | 0.35.0 (2026-05-24) | Apache-2.0 only (permissive, subset of ours; fine to depend on) | **1.89.0 > 1.85: FAILS MSRV** | 7 / 17 (default `std`,`macros`) | heavy (typenum generics, simba, proc-macro `macros`; slow compile, big monomorphisation) | `wide`/`safe_arch` SIMD only via optional features; unsafe present |
| ultraviolet | 0.10.0 (2025-04-26) | MIT OR Apache-2.0 OR Zlib | undeclared | 3 / 3 (wide, safe_arch, bytemuck: always on) | small-medium | SIMD wide types (via `wide`); f64 only via `f64` feature (K) |
| vek | 0.17.2 (2025-09-23) | MIT OR Apache-2.0 | undeclared | 3 / 10 (default `rgba`,`rgb`,`std`; on pulls serde via defaults in walk) | medium (very large generic surface) | no build.rs known |
| sha2 | 0.11.2 (2026-11-25) | MIT OR Apache-2.0 | **1.88 (0.11.1+); 0.11.0 declares 1.85; 0.10.9 undeclared** | 8 / 9 (digest, block-buffer, crypto-common, hybrid-array, typenum, cpufeatures, cfg-if, libc; `oid` adds const-oid) | small-medium (typenum generics) | `unsafe` + runtime-detected SHA-NI/ARM intrinsics via `cpufeatures`; no build.rs; no `asm` feature in 0.11 (backend chosen by cfg) |
| blake3 | 1.8.8 (2026-11-26) | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception (permissive) | undeclared | 5 / 5 (+ `cc` as BUILD dep, not counted) | small-medium, but build.rs compiles C/asm | **build.rs + `cc`**; C/asm SIMD (SSE2/4.1/AVX2/AVX-512/NEON) with runtime detect; unsafe. **Pure option: feature `pure`** (portable Rust only, no C/asm; `cc` crate is still a build-dep and compiled, build.rs just skips C). Also `no_sse2`... per-ISA disables, `rayon`, `mmap` off by default |

## Table B: skinning / IK / VRM crates found (maturity honest)

| crate | ver (date) | licence | MSRV | deps off / on | verdict |
|---|---|---|---|---|---|
| ozz-animation-rs | 0.11.0 (2025-10-14) | **MPL-2.0 (weak copyleft, file-level; flag)** | 1.88 | 13 / 32 (glam 0.30, rkyv, serde) | most mature runtime (port of ozz-animation, 17k dl); licence + MSRV block it. Useful only as reference |
| tinyik | 0.1.0 (2026-11-10) | MIT | 1.85 | 0 / 0 | dependency-free two-bone + FABRIK; brand new (13 dl). Read, do not depend |
| rebirth-ik | 0.1.0 (2026-10-27) | MIT | undeclared | 0 / 0 | "safe Rust" FABRIK + analytic; 24 dl. Same |
| caliper-ik | 0.2.1 (2026-07-31) | Apache-2.0 | 1.89 | 40 / 40 (nalgebra, regex, xml) | too heavy, MSRV fail |
| ik-geo | 0.1.2 (2024-07-09) | non-standard | undeclared | 23 (nalgebra 0.32, nlopt native) | reject (licence, native lib) |
| k | 0.32.0 (2024-09-10) | (not walked) | (not walked) | nalgebra/urdf stack | robotics kinematics, 57k recent dl; heavy, not walked |
| fabrik | 0.1.0 (2021-01-06) | (not walked) | | | abandoned |
| bevy_mod_inverse_kinematics, bevy_fabrik(_solver), bevy_vrm(1) | various | | | | Bevy-coupled, unusable |
| vrm-spec | 0.1.1 (2026-06-15) | Apache-2.0 | 1.68 | 11 / 39 (default `gltf_index` pulls gltf with default features) | VRM extension serde types; Apache-only |
| serde_vrm | 0.5.0 (2026-10-13) | MIT OR Apache-2.0 | undeclared | 7 / 7 | VRM serde types only; most relevant for the VRM slice; ~8k dl |
| vrm-runtime | 0.1.0 (2026-08-18) | MIT OR Apache-2.0 | undeclared | 45 (gltf, glam 0.15 AND 0.29) | young, duplicate glam versions; reject |
| polysmelt 0.19.0, gltf-part-validator 0.6.1, character-retargeting 0.1.0 | 2026-10/11 | not checked | | | new, <1k dl; not evaluated |

No skinning/IK crate has the maturity or MSRV/licence profile to adopt; all are 0.x with tiny usage except `k` and ozz.

## Notes per candidate

- **glam**: has `DVec2/3/4`, `DQuat`, `DMat3/4`, `DAffine3` under feature `f64` (included in `float-types`, hence in default `all-types`). To shrink: `default-features = false, features = ["std", "f64"]` gives f32 + f64 floats only (the f32 types are unconditional). Provides Quat (slerp/nlerp, from_rotation_*), Mat4 inverse, Affine3A. Has no skeleton, skinning, IK. `scalar-math` feature gives bit-reproducible non-SIMD f32 paths (useful for determinism; SIMD and scalar results can differ in last bits). Very active (releases weekly in Oct-Nov 2026); 201M downloads.
- **mint**: interop-only struct types (no operations). Last release 2022. Only worth it if the public API must accept foreign vector types; a figure module can expose plain `[f32; N]` instead. glam `mint-05` feature exists but the glam-side `mint` is 0.5.
- **gltf**: (K) exposes `Document::skins()` -> `Skin::joints()`, `Skin::reader().read_inverse_bind_matrices()`, `Skin::skeleton()`; `Primitive::morph_targets()` (positions/normals/tangents deltas) plus mesh `weights`; `Document::animations()` -> channels, samplers, interpolation (LINEAR/STEP/CUBICSPLINE), property targets incl. weights; `Gltf::from_slice`/`import` validate JSON against spec (the `validation` module lives in gltf-json: `Validate` trait, `Checked<T>`, `Error` paths) but does NOT enforce skinning semantic rules like matrix invertibility, joint-count limits or NaN. Extensions: VRM is not covered (use serde_vrm / own serde structs over `extensions`, needs feature `extensions`). **Last release 2024-05-10, 2.5 years stale; declared MSRV 1.61 is probably optimistic against current serde ecosystem.** Not published since the glTF 2.0 extension churn, so treat as stable-but-quiet. Recommend `default-features = false` and only `names`/`extras` as needed; use `Gltf::from_slice_without_validation` only if we do our own checks (K). `gltf-json` alone (14 deps, all serde) is the leaner choice if we parse the buffers ourselves with explicit bounds checks (safe import: GLB container/chunk parsing is ~100 lines and keeps us in control of limits).
- **nalgebra**: most capable (`UnitQuaternion<f64>`, `Isometry3`, SVD for IK/Jacobians, `DMatrix`), but MSRV 1.89 > 1.85 (an `rust-version` bump would be needed, or pin to an older 0.33/0.34, not walked) and heavy compile. Contrast only; reject for Slice 1. Note it is the right tool for later damped-least-squares IK, but FABRIK/CCD/two-bone IK need no linear solver.
- **ultraviolet**: SIMD-wide (4 lanes AoSoA) types, Rotor3 not quaternions (affects glTF quaternion interop); last release 2025-04; always pulls `wide`+`safe_arch` (old 0.7.x). Reject: wrong shape for a skeleton of < few hundred joints.
- **vek**: broad generic types incl. Quaternion, Mat4; 3 deps off; stale-ish (2025-09), huge generic surface, MSRV undeclared. Reject.
- **sha2**: pure Rust, NIST SHA-256 (also 224/384/512). Latest 0.11.2 needs rustc 1.88 so it violates our 1.85; **0.11.0 (2026-03-25) declares 1.85** and 0.10.9 (undeclared, digest 0.10 stack with generic-array; not walked) is the old line. Active (Cargo releases monthly).
- **blake3**: reference implementation, very fast, 32-byte default output natively (`blake3::hash(&bytes).as_bytes()`), streaming `Hasher`. Default build runs build.rs with `cc` and compiles C/asm for x86 AVX2/AVX-512 (unless `pure`); this adds a C compiler requirement to every consumer build (libgibson is currently pure Rust), unsafe SIMD, and hits the shared compile budget. With `features = ["pure"]` it is portable Rust, ~5 deps. MSRV undeclared (do not guess).

## RECOMMENDATION

### Slice 1 (skeleton / LBS / morph / clips / revision hash)
1. **Zero required third-party math**: hand-write small `Vec3/Quat/Mat4` (f32) in the module (about 300 lines: quat mul/normalise/slerp, affine compose, rigid inverse, Mat4 mul). Skinning with inverse-bind matrices and morph deltas is only mul-add; clips at explicit time are sampling (lerp/slerp/step/cubic-spline Hermite). This keeps MSRV 1.85, zero compile cost, no SIMD nondeterminism, and keeps `#![forbid(unsafe_code)]` trivially true.
2. If a math crate is preferred: **glam 0.34.x** with `default-features = false, features = ["std"]` (add `scalar-math` for deterministic non-SIMD f32). It is the only candidate that matches MSRV 1.85 exactly, licence-identical, zero transitive deps. Caveat: MSRV equals ours exactly, so any glam patch bump that raises MSRV is a break (cap `<0.35`/pin in the lockfile; 0.33 line is 1.68.2).
3. **Hash (CharacterRevision, 32-byte digest over declared canonical bytes)**: recommend **sha2 pinned to `=0.11.0`** (declared MSRV 1.85, pure Rust, no build.rs, no C, 8 small deps, standard SHA-256 so any external tool can verify golden vectors). Reject blake3 for a TUI library: build.rs + cc + C/asm by default and undeclared MSRV; `pure` fixes the C part but you keep the build-dep and gain nothing needed (we hash KB-sized records, speed irrelevant). A third zero-dependency option is a ~100-line hand-written SHA-256 checked against NIST vectors, worth considering if sha2's 1.88 drift on the 0.11 line or the exact pin is unacceptable; note a hand-rolled hash is only as independent as its vectors.
   - Do not float `sha2 = "0.11"`: 0.11.1+ requires 1.88 and breaks a 1.85 build unless the resolver honours rust-version (resolver "3"/`CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback`).

### Defer to the GLB import slice
`gltf-json` (preferred, `default-features = false`) or `gltf` (`default-features = false`, features `names` and, for VRM, `extensions`/`extras`); never the `import` feature (pulls `image`, `png`, `zune-jpeg`, `moxcms`, `base64`, `urlencoding`: +20 crates). VRM: `serde_vrm` (7 deps, 2 licences OK) or own serde structs over `extensions`; reject vrm-runtime/bevy_vrm. Own explicit bounds/size limits and NaN/non-invertible checks on top (the validator checks schema, not geometry). IK/contact: write FABRIK + two-bone analytically in module; consider nothing off the shelf (tinyik/rebirth-ik are weeks old). If a solver is later required, nalgebra only with an MSRV bump decision.

### Cargo features (all optional, default-off, `figure` itself experimental)
```
figure          = []                     # math-only core, no deps
figure-glam     = ["dep:glam"]           # optional math backend / interop
figure-hash     = ["dep:sha2"]           # CharacterRevision digest (sha2 =0.11.0)
figure-gltf     = ["dep:gltf-json"]      # GLB/glTF import slice
figure-vrm      = ["figure-gltf", "dep:serde_vrm"]
figure-oracle   = []                     # dev-only f64 reference (see below), no deps
```
### Independent f64 oracle: use a hand-written f64 reference, NOT glam's f64 types
Reasons: (1) glam `DQuat/DMat4` and glam `Quat/Mat4` share the same source templates (macro-generated from one `.rs.tera` template), so an algorithmic bug (e.g. slerp shortest-path sign, quaternion-to-matrix convention, mat4 multiply order) is common-mode and would cancel in comparison; only precision differs. (2) If the production f32 path is itself hand-written (option 1), then a glam-f64 oracle is a genuine different-code-path check, which is also acceptable. (3) Rule: the oracle must differ from production in at least implementation and precision, ideally also formulation: e.g. oracle composes skinning as explicit per-vertex sum over `W_j * (G_j * IBM_j) * v` in f64 with row-major `[[f64;4];4]` and quaternion rotation via Rodrigues/matrix, while production uses fused affine/quat paths. Compare with a tolerance derived from the f32 epsilon times the condition number, plus golden hand-computed fixtures (identity pose = bind pose, single-joint 90 deg rotation) that need no library at all. glam f64 is then a third cross-check in dev-dependencies only, not the oracle of record.

Boundaries: counts are an API walk, not `cargo tree`; no Cargo.lock/feature unification, build-deps excluded; no code or docs read; MSRV values are the declared fields only (true buildability on 1.85 not tested, by rule). k, fabrik, polysmelt, gltf-part-validator not walked.
