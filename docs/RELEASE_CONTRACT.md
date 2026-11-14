# LibGibson Release Contract

**Status: engineering alpha · pre-1.0 (0.x) · Linux x86_64.**

This document is the authoritative statement of what LibGibson promises to the
people who consume it. It defines the versioning model, the Rust API stability
policy, the C ABI policy, the wrapper compatibility policy, the supported Rust
compiler floor (MSRV), the supported platforms, and what a release artifact
contains. The step-by-step release procedure lives in
[`RELEASING.md`](RELEASING.md); the human-readable change history lives in
[`../CHANGELOG.md`](../CHANGELOG.md).

"Engineering alpha" means: **a reproducible, installable artifact with documented
contracts.** It does *not* mean production-ready or forever-stable. 0.1.0 is a
coherent thing to release under that classification; it is not a 1.0.

---

## 1. Versioning model

There is **one** LibGibson version, canonical in `Cargo.toml` (`[package] version`).
While this policy is in force, the Rust crate, the native SDK, the Python wrapper,
and the Go module all release in lockstep at that single version — no independent
wrapper versioning until a real need forces it.

The **C ABI version** (`GIBSON_ABI_VERSION`) is a *separate* integer and is **not**
tied to the package version (see §5). A package release can advance without
touching the ABI version; a breaking ABI change bumps `GIBSON_ABI_VERSION` even
while the package stays 0.x.

**Release names** ([decision 0004](decisions/0004-release-naming.md)): each release also has a
name, `<utility> <signal> <patch>`, for example 0.7.11 = "clear SIGBUS 11". The name appears in
the GitHub release title and the tag message only. Tag names and version strings stay plain
SemVer.

### 0.x SemVer semantics

LibGibson is pre-1.0, so Cargo's 0.x compatibility rules apply. A consumer who
writes `libgibson = "0.1"` accepts `>=0.1.0, <0.2.0`. Therefore:

| Change | Version bump | Notes |
|---|---|---|
| Bug fix, internal change, additive API (new fn/type/module), doc | `0.1.z → 0.1.(z+1)` (patch) | Source-compatible for CORE consumers. |
| Breaking change to a CORE Rust API; removing/renaming public items; adding an enum variant (see §3); raising MSRV; changing an experimental API in a way that breaks a patch consumer | `0.1.z → 0.2.0` (the 0.x "breaking" slot) | Recorded in CHANGELOG. |
| C ABI break | independent `GIBSON_ABI_VERSION` bump (§5) **plus** the package bump the source change warrants | The two version numbers move independently. |

There is no 1.0 yet. 1.0 is when the CORE surface (§3) is frozen; that is out of
scope for this round.

**Deliberate minor milestones.** The table above sets the *minimum* bump a change
requires. The project may also **elect** a `0.y` minor bump to mark a substantial new
public subsystem or project milestone, even when the change is purely additive and
would otherwise qualify as a patch. Pre-1.0 minor lines are the natural place for the
public Rust API to evolve, so a milestone that introduces a large new (experimental)
surface is versioned as a minor to signal its scope — not because anything broke.

- **0.2.0** is such a milestone: it adds the experimental `gibson::ui` semantic
  composition layer (skins, typed interaction, a keyed presentation runtime) on top of
  the 0.1.x substrate. The change is additive and the **C ABI is unchanged
  (`GIBSON_ABI_VERSION` stays 1)** — per §5 the package and ABI versions move
  independently; the minor bump reflects the size of the new public Rust surface and
  the milestone, not an ABI break. `gibson::ui` is EXPERIMENTAL (§2) and Rust-only: it
  is not part of the C ABI or any language wrapper, and moving to 0.2.0 does not
  promote it to CORE.
- **0.4.0** is the next such milestone: it adds the experimental `gibson::audio`
  realization axis — a deterministic, device-free offline audio engine
  (sample-accurate transport, DSP synthesis, an FDN
  reverb / bus compressor / limiter) plus **HumanMusic**, a fully self-synthesized
  procedural music + SFX system driven by semantic style/story state (see
  [`HUMAN_MUSIC_ARCHITECTURE.md`](HUMAN_MUSIC_ARCHITECTURE.md)). A new realization axis of
  this size earns a deliberate minor. It is additive and Rust-only; **the C ABI is
  unchanged (`GIBSON_ABI_VERSION` stays 1)** and audio is not exposed through it this
  milestone. A real-audio-device backend is the optional `audio-cpal` feature, so the
  mandatory build, `cargo test` and CI never need system audio libraries.

- **0.5.0** is a prior milestone and is **tagged and released on GitHub** (state 2 below):
  tag `v0.5.0` → `d8c7b99`, GitHub prerelease of 2026-10-24 with the native SDK archive,
  its checksum and the Python wheel/sdist; not published to any registry. It adds the
  experimental, Rust-only interactive-instrument surface (spatial input and hit-map,
  reversible authoring, `gibson::plot` / field view, bounded transport). It is additive
  apart from one documented Rust CORE source-break (`Node` gained a public
  `tag: Option<u32>` field); **the C ABI is unchanged (`GIBSON_ABI_VERSION` stays 1)**,
  none of the new surface is exposed through C, C++, Python or Go, and MSRV stays 1.85.
  It remains an engineering alpha (Linux x86_64, GitHub-only prerelease). *(Until
  [#257](https://github.com/femboy2112/libgibson/issues/257) this entry still called 0.5.0
  "a prepared release candidate, not yet tagged" — the wording of its version-bump commit,
  not reconciled after the tag.)*
- **0.6.0** is **tagged and released on GitHub** (state 2):
  tag `v0.6.0` → `492d0fe` (and `bindings/go/v0.6.0` on the same commit), a GitHub prerelease of
  2026-11-08 ([release](https://github.com/femboy2112/libgibson/releases/tag/v0.6.0)) with the
  native SDK archive, its checksum and the Python wheel/sdist; not published to any registry.
  `492d0fe` is on the maintenance branch `release/v0.6.0-rc`, not in `main`'s history: it is the
  first release under the maintenance-line exception below
  ([decision 0001](decisions/0001-v0.6.0-release-line.md)). Its source-version baseline is
  `9a899e1` (0.6.0 prepared in source on `main`; `main` has since opened `0.7.0-dev`), which carries
  #264 and #269 and is never tagged. It integrates the v0.6
  "Instrumental Renaissance": a live Python↔Rust wire instrument and instrument-driven
  cinema (examples), renderer recovery from partial terminal writes (§9), the text-input
  glyph-boundary repair, and **seven additive C ABI functions (52 → 59; headless contexts,
  rendered-output and cell-grid readback, color-depth selection) with
  `GIBSON_ABI_VERSION` staying 1**. Experimental Rust surfaces changed shape where the
  work required it (for example the instrument-telemetry `Stage` set gained a stage), which
  a `0.y` minor permits (§2). MSRV stays 1.85; Linux x86_64 only.

- **0.7.1** "clear SIGBUS 1" ([decision 0004](decisions/0004-release-naming.md)) is the latest
  such milestone and is **tagged and released on GitHub** (state 2):
  - tag `v0.7.1` → `835796e` on `main`, with `bindings/go/v0.7.1` on the same commit;
  - a regular GitHub release of 2026-11-13
    ([release](https://github.com/femboy2112/libgibson/releases/tag/v0.7.1)) with the native SDK
    archive, its checksum and the Python wheel/sdist, not published to any registry.

  It promotes the `raster` RGB subset to CORE
  (decision [0003](decisions/0003-raster-rgb-core-promotion.md), §2) and changes no library code
  since 0.7.0. ABI v1 (67 symbols), MSRV 1.85 and Linux x86_64 are unchanged.
- **0.7.0** is **tagged and released on GitHub** (state 2):
  tag `v0.7.0` → `1df9690` on `main` (and `bindings/go/v0.7.0` on the same commit), a regular
  GitHub release (not a prerelease) of 2026-11-13
  ([release](https://github.com/femboy2112/libgibson/releases/tag/v0.7.0)) with the native SDK
  archive, its checksum and the Python wheel/sdist; not published to any registry. It is the first
  qualified 0.7 release (decision [0002](decisions/0002-v0.7.0-qualification.md)) and follows six
  consumer prereleases from `main` (`v0.7.0-alpha.1` through `v0.7.0-alpha.6`). The C ABI grows
  additively from 59 to 67 symbols with `GIBSON_ABI_VERSION` staying 1. MSRV stays 1.85; Linux
  x86_64 only.

**Release states.** A version passes through three separate states, and every document
that names a version says which one it has reached:

1. **Prepared in source** — `Cargo.toml`, `Cargo.lock` and the lockstep wrapper metadata
   carry the version on `main`. Nothing is tagged and nothing is installable as that
   release; README install instructions keep pointing at the latest *released* version.
2. **Tagged and released on GitHub** — a `v<version>` tag (plus the Go module tag
   `bindings/go/v<version>`) names a `main` commit, and a GitHub prerelease attaches the
   artifacts of §7. Install instructions move to this version. (`main` is the default; the
   maintenance-line exception below is the only other route.)
3. **Published through registries** — crates.io, PyPI or the Go module proxy. No
   LibGibson version has reached this state.

Each transition is a separate, explicitly approved step ([`RELEASING.md`](RELEASING.md)); a
version bump alone never implies a tag, a release or a registry upload.

**Maintenance-line exception — *Approved 2026-11-08* (owner decision
[#287](https://github.com/femboy2112/libgibson/issues/287), conditional on the gates below; recorded in
[decision 0001](decisions/0001-v0.6.0-release-line.md)).** A `v<version>` tag may name a qualified
commit on a maintenance branch instead of a `main` commit, under **all** of these conditions:

1. **Authorized branch, exact SHA.** The branch is named `release/v<x.y.z>-rc` or
   `release/<x.y>`. It is protected by immutable exact-SHA qualification: what is qualified,
   approved and tagged is one commit, never a branch name, and any further commit is a new
   candidate that is qualified again from the start.
2. **Declared baseline.** The release names its source-version baseline — the commit at which the
   version was prepared in source (0.6.0: `9a899e1`), or, for a later patch on the line, the line's
   previous release tag — and the candidate descends from it
   (`git merge-base --is-ancestor <baseline> <candidate>`).
3. **No imported development.** Against the baseline, the exported C ABI symbol set (the
   committed `abi/` baseline and what the built library exports) and its Rust definitions
   (signatures, `#[repr]` layouts, `GIBSON_*` constants), the declarations of the public
   headers (`include/`; a comment-only change, such as a disclosed limitation, is release
   engineering), the MSRV (`rust-version`), the frozen PCM golden (`src/audio/`,
   `tests/audio_pocket_voice.rs`) and the C ABI layout oracle (`tests/ffi_abi_layout_freeze.rs`) are
   unchanged, and every later commit is a listed backport of a
   fix (citing its `main` counterpart) or a release-engineering commit (version, changelog,
   documentation, test or CI adaptation). `scripts/release/check-maintenance-candidate.sh` checks
   these structurally, calibrated by `scripts/release/tests/test-maintenance-checker.sh`; it does
   not judge whether a listed commit is a legitimate backport, nor see a behaviour change behind an
   unchanged signature; that is the independent review's and CI's job (condition 5).
4. **Exact tree.** The version tag names the exact commit whose tree the §7 artifacts were built
   from, and `bindings/go/v<x.y.z>` resolves to that same commit.
5. **Owner approval per release.** Each maintenance-line release needs the owner's explicit
   approval, naming the version and the baseline, of one exact candidate SHA that has passed every
   gate of [`RELEASING.md`](RELEASING.md) §7, including an independent review of every backport,
   before any tag or publication. The owner may give that approval in advance, conditional on those
   gates (as for 0.6.0); a failed gate then voids it for that candidate. Approving this exception
   approves no release.
6. **`main` is never downgraded.** The release does not change `main`'s version. Merging the
   maintenance branch back into `main` is optional and accepted separately: the merge keeps
   `main`'s version strings, C ABI symbol set and green CI, and is a merge commit, so the tagged
   commit becomes reachable from `main` without rewriting either line.
7. **Reconstructible provenance.** The annotated tag records the baseline, the branch and the
   qualification record; with git ancestry (baseline → tag) and the release records (CHANGELOG,
   the milestone entry above, the decision record) the release's origin stays reconstructible
   whether or not it is merged back.

A maintenance version other than the baseline's own (e.g. a later `0.6.1`) is prepared in source on
its authorized branch by a listed release-engineering commit; for that version state 1 means "on
that branch". The exception permits a release; it does not widen the security support of §8.
Procedure: [`RELEASING.md`](RELEASING.md) §7. Decision record:
[`decisions/0001-v0.6.0-release-line.md`](decisions/0001-v0.6.0-release-line.md).

The conceptual progression: `0.1.x` established the framebuffer / renderer / terminal
substrate; `0.2.x` adds the semantic UI / design-system layer; `0.3.x` adds the
experimental temporal display axis; `0.4.x` adds the experimental audio /
HumanMusic axis; `0.5.x` adds the experimental interactive-instrument surface; `0.6.x`
connects that surface to live producers and grows the C ABI additively for headless,
inspectable consumers; later `0.x` milestones continue resolving architecture, portability and
stability debt; `1.0` waits until the CORE public contract (§3) is genuinely stable and
frozen.

---

## 2. Rust API stability (pre-1.0)

**Import note.** The package is `libgibson` but the library target is `gibson`
(`Cargo.toml` `[lib] name = "gibson"`). When you depend on the `libgibson` package
(via the git dependency shown in the README — it is not on crates.io), Rust code
imports as **`use gibson::…`**, not `use libgibson::…`. Examples in docs must use
`gibson::`.

The crate is already self-classified as "Engineering alpha" in its crate-level
docs (`src/lib.rs`). This contract makes the tiers explicit.

### CORE (best-effort stability at 0.x)

These modules are the load-bearing, C-ABI-adjacent surface. Within a given `0.y`
patch line they receive no breaking changes; a breaking CORE change requires the
next `0.y` minor and is recorded in the CHANGELOG:

- `cell` — `Cell`, `Color`, `Style`, `Span`, `Line`, `RichText`, `Theme`, `TextAlign`
- `surface` — `Surface`, `Rect`, `BorderType`
- `node` — `Node`, `NodeKind`, `LayoutStyle`, and the layout-style enums
- `layout` — `compute_layout`, `wrap_text`, `wrap_rich_text`
- `painter` — `paint`, `PaintContext`
- `renderer` — `Renderer`, `RenderMode`, `InsertStrategy`, `AnchorState`, and (0.3.0) `FrameReport`
- `context` — `Context`
- `session` — `TerminalSession`, `TerminalLease` (process-global terminal-ownership lease)
- `input` — `Event`, `KeyCode`, `KeyEvent`, `poll_event`
- `capability` — `TerminalCapabilities`, `Capability`, `ColorDepth`, quantization
- the **C-ABI-backed subset** (see §5)
- **From 0.7.1** ([decision 0003](decisions/0003-raster-rgb-core-promotion.md)): the `raster` RGB
  subset. That is `Rgb` (the `(u8, u8, u8)` alias), `MAX_RASTER_DIMENSION` (2048), `FrameError`
  and `RgbRaster` with all of its methods except `to_mono_surface`. The promise covers signatures,
  traits and the documented edge behaviour, pinned by `tests/raster_core_surface.rs`. In 0.7.0
  this subset was EXPERIMENTAL.
- **From 0.7.2** ([decision 0005](decisions/0005-raster-rgba-core-promotion.md)): the `raster`
  RGBA compositing subset.
  - **Items:** `Rgba` (the `[u8; 4]` straight-alpha alias); `RgbaRaster` with `new`,
    `from_rgba_bytes`, `width`, `height`, `get` and `pixels`; `composite_over`; and `flatten`.
  - **Promise:** signatures, traits and the documented behaviour, including `composite_over`'s
    exact formula `(src·a + dst·(255 − a) + 127) / 255`, its `a = 0` / `a = 255` fast paths and
    its overlap-only clipping. Pinned by `tests/raster_rgba_core_surface.rs`.
  - **Still EXPERIMENTAL:** `RgbaRaster::{set, pixels_mut, from_pixels, from_rgb}` and
    `downscale_area`.

A lower-risk **utility tier** — `ansi`, `canvas`, `clock`, `diff`, `field`,
`focus`, `geom`, `glitch`, `glyph`, `particles`, `replication`, `scheduler`, `show`,
`text` (`display_width`, `char_width`; 0.7.0),
`transition`, `viewport` — is public and treated as CORE (same 0.x promise) unless
a specific item is marked experimental. Two known specifics: `geom` is core vector
math *except* `CubicPath3`, which is marked experimental; `show` (composition
helpers such as sparklines/gradients/progress) is core-intent but currently
reachable only via its full path `gibson::show::…` (it is not re-exported at the
crate root).

New in 0.3.0 (CORE, `#[non_exhaustive]`): `scheduler::FramePacing` (opt-in
phase-locked frame pacing) and `renderer::FrameReport` (split generation/write
per-frame timing/accounting). These are stable-intent high-cadence rendering
infrastructure; the ABI-v1 `RenderStats` layout is deliberately unchanged and
`FrameReport` is a Rust-side addition only.

### EXPERIMENTAL (may change or be removed between minor releases)

These are already marked "Experimental" in their own module docs and in the
crate-level docs (`src/lib.rs`). They exist and are useful, but they carry **no**
`0.y`-level stability promise: they may change signature, behavior, or be removed
in any `0.2.0`-style release, and we will avoid — but do not guarantee against —
breaking them in a pure patch:

- `scene` — entity/effect composition graph (`Scene`, `SceneEntity`, `Effect`, …)
- `story` — narrative beat/reaction director (`Story`, `StoryDirector`, `Beat`, `TraceRetention`, …)
- `surface_fx` — ordered endomorphisms over a realized surface
- `raster`, `raster3d`, `raster_fx` — the software RGB/3D rasterizer stack (`raster` except the CORE subsets above: RGB from 0.7.1, RGBA compositing from 0.7.2)
- `raycast` — the grid raycaster rendering into an `RgbRaster` (0.7.0)
- `raster::RgbaRaster::{set, pixels_mut, from_pixels, from_rgb}`, `raster::downscale_area` and `ui::image_area` — RGBA editing, alternative constructors and area filtering (0.7.0; the rest of the RGBA subset is CORE from 0.7.2, decision 0005)
- `rig`, `city` — camera rigs (`Dolly`, `Orbit`, `Transition`, `Ease`) and wireframe-city geometry helpers for `raster3d`; also `raster3d::Camera::basis` / `CameraBasis` (0.7.0)
- `ui::experience` — semantic experience grammars realizing one `Experience` through
  several presentation grammars (Rust only; see [EXPERIENCE_GRAMMARS.md](EXPERIENCE_GRAMMARS.md))
  (0.7.0); `BladesStyle`, `Blades::with_style` and `Blades::with_skin` (0.7.0)
- `temporal` — experimental temporal cell realization (`TemporalDisplayProcessor`,
  `TemporalBrailleField`, the exact RGB subcell projector, `PresentationProfile`,
  `TemporalSafetyPolicy`, `ResetPolicy`). Rust-only, introduced in the 0.3.0
  milestone; default static, opt-in temporal, one renderer (emits ordinary
  `Surface`). See [TEMPORAL_DISPLAY_PROCESSOR.md](TEMPORAL_DISPLAY_PROCESSOR.md) and
  [TEMPORAL_DISPLAY_VALIDATION.md](TEMPORAL_DISPLAY_VALIDATION.md).
- `ui` — Rust-only semantic components, skins above `Theme`, typed interaction,
  stable-key presentation and finite motion, lowered to ordinary `Node` trees.
  It is introduced experimentally in the 0.2.0 milestone. `Element`, `Skin`,
  `UiRuntime`, `App`, token types and lowering sidecars all belong to this tier,
  not CORE. See [UI_LAYER.md](UI_LAYER.md).
- `audio` — the deterministic offline audio engine and HumanMusic scoring (0.4.0; the
  real-device backend is the optional `audio-cpal` feature). Not exposed through the C ABI.
- `plot` and the v0.5.0 interactive-instrument surface — `shape`, `selection`, `hit_test`,
  `world_view`, `gesture`, `edit`, `numeric`, `field_view`, `frame`, `snapshot`, `wire`,
  `instrument_telemetry` (0.5.0; Rust-only, see §1).
- `event_source` — injectable, recordable and replayable input sources (Rust-only).
- `hold` — held-key inference on a simulation timebase (`HoldTracker`; 0.7.0, execution-graph
  node D). An explicit model; it never synthesizes a key release.

**Unlisted modules.** A public module that this section does not list has no stability
promise and counts as EXPERIMENTAL until it is listed. `tests/doc_release_contract_tiers.rs`
fails while any `pub mod` in `src/lib.rs` is missing from this section.

The experimental UI layer does not extend the C ABI or C++/Python/Go wrappers,
change `GIBSON_ABI_VERSION`, raise the MSRV, or constitute a package release.
Existing `Theme`, `Node`, `Context` and terminal-ownership contracts continue to
apply. Its tests are scoped evidence, not a production-stability or human
usability guarantee.

### `ffi` module

`gibson::ffi` is `pub` because the `cdylib`/`staticlib` build targets require it,
but it is **not a supported Rust API**. Rust consumers should use the safe modules
above. The stability promise attached to `ffi` is the **C ABI promise** (§5), not a
Rust-level one.

`transaction::TerminalTransaction` is internal renderer plumbing (the batched wire
transaction — not atomic I/O, see §9). As of the 0.1.0 candidate the `transaction` module is `pub(crate)` —
it is **not** part of the public Rust API. (It was briefly `pub`; privatizing it
before the first tag avoids a later `0.y` break, and it had no external consumers.)

### Enum extensibility (forward-compatibility note)

The **extension-point enums are `#[non_exhaustive]`**, decided deliberately before
the first release (the cheapest moment — adding `#[non_exhaustive]` *after*
publication is itself source-breaking). For these, **adding a variant is a
non-breaking `0.1.z` change**; downstream `match`es must therefore include a `_`
wildcard arm:

- `node::NodeKind` — the primary extension point for new component kinds.
- `input::Event` — new event kinds are expected (mouse routing is planned; focus).
- `input::KeyCode` — the terminal key repertoire grows (function keys, Insert, …).
- `glyph::SubcellGlyphMode` — further sub-cell families are plausible (2×3 sextants,
  2×2 quadrants).
- `scheduler::FramePacing` — further pacing policies are plausible (0.3.0, CORE).
- `temporal::TemporalGate` — further static-fallback reasons are plausible (0.3.0,
  experimental).

Several new 0.3.0 **structs** are also `#[non_exhaustive]` so future fields are
additive, not breaking: CORE `renderer::FrameReport`, and experimental
`temporal::{PresentationProfile, TemporalSafetyPolicy, TemporalCellProjection,
TemporalDiagnostics}`. `temporal::ResetPolicy` is left exhaustive (a closed
`Keep`/`Reset` pair).

The remaining public enums are **deliberately left exhaustive** because their value
set is closed, so downstream exhaustive `match`es on them stay sound and adding a
variant would (correctly) be a breaking `0.y` change:

- `capability::Capability` — a complete `Supported`/`Unsupported`/`Unknown` tri-state.
- `capability::ColorDepth` — the closed set of color-realization tiers
  (`Mono`/`Ansi16`/`Ansi256`/`TrueColor`).
- `glyph::GlyphChoice` — a closed `Auto`-or-explicit choice.

`#[non_exhaustive]` is a Rust source-compatibility marker only: it changes no
`#[repr]`, crosses no C ABI (no audited enum is `#[repr(C)]`; input events are
*translated* at the FFI boundary, not shared by layout), and leaves
`GIBSON_ABI_VERSION` unchanged. Construction of known variants from downstream is
unaffected — only exhaustive matching requires the `_` arm. General guidance stands:
**downstream code that matches LibGibson enums should include a `_` wildcard arm** to
stay forward-compatible.

### Deprecations

Pre-1.0, deprecated items are marked with `#[deprecated]` where practical and may
be removed in the next `0.y` release. We do not promise a long deprecation window
before 1.0.

---

## 3. C ABI policy

The C ABI is the foundation the C, C++, Python, and Go bindings all stand on.

- `GIBSON_ABI_VERSION` is currently **1** (`include/gibson.h`). It is preserved.
- `gibson_abi_version()` returns it at runtime.
- `gibson_stats_t` carries `struct_size` and `abi_version` as its first two fields
  specifically so the struct can evolve under version control. The current
  exact-version discipline — the caller sets `struct_size = sizeof(gibson_stats_t)`
  and the engine validates `abi_version` against `GIBSON_ABI_VERSION` — **is
  preserved and must not be quietly changed.**
- The ABI v1 surface is the set of **67 exported `gibson_*` symbols** (59 in v0.6.0; v0.7
  development adds `gibson_push_event`, then `gibson_poll_event_kind` and
  `gibson_push_event_kind` for issue #274, then `gibson_take_rendered_output` and
  `gibson_clear_rendered_output` for issue #297, then `gibson_set_key_release_reporting` for
  key-release reporting (#275), then `gibson_get_terminal_size` and `gibson_set_focus_reporting`
  for the terminal-size query and focus events (#298), all additively), captured as a
  baseline in [`../abi/gibson-abi-v1.symbols`](../abi/gibson-abi-v1.symbols) and
  guarded in CI. Symbols in that set must not disappear or change signature without
  an ABI-version bump.

### ABI-compatible changes (no `GIBSON_ABI_VERSION` bump)

- adding new functions
- adding new opaque handle types
- internal implementation changes with unchanged observable behavior
- appending new fields to the **end** of `gibson_stats_t` under the `struct_size`
  discipline (a caller built against an older, smaller struct remains correct)
- adding new enum constants **where the output-direction semantics permit** and
  callers are expected to treat unknown constants defensively

### ABI-breaking changes (**must** bump `GIBSON_ABI_VERSION`)

- changing the signature of an existing exported function
- changing calling convention
- changing ownership/lifetime semantics incompatibly
- removing an exported symbol
- an incompatible public struct layout change (reordering or removing existing
  `gibson_stats_t` fields, or changing a field's meaning)

### Relationship to the package version

Crate/package semantic version **≠** C ABI version. The package can move from
`0.1.z` to `0.2.0` with the ABI still at 1. A breaking C ABI change **must** bump
`GIBSON_ABI_VERSION` (to 2, …) **even while LibGibson remains 0.x.**

---

## 4. Wrapper compatibility policy

The C++, Python, and Go wrappers all target native ABI **v1**. Each wrapper knows
the ABI version it was built for (`EXPECTED_ABI = 1`) and checks the loaded native
library's `gibson_abi_version()` **before** relying on it, failing with a clear,
actionable message on mismatch (never "segfault first, explain later"):

- **Python** — an `EXPECTED_ABI` constant and a runtime check on library load that
  raises a precise exception (e.g. "expected LibGibson ABI 1, loaded ABI 2; install
  a compatible libgibson").
- **Go** — an `EXPECTED_ABI` constant and a runtime check that returns an error on
  mismatch.
- **C++** — the header compiles against `gibson.h` and exposes
  `gibson::Context::abi_version()` and `gibson::Context::abi_compatible()`. The
  `Context` constructor verifies `gibson_abi_version() == GIBSON_ABI_VERSION`
  **before any other ABI call** and throws `std::runtime_error` on mismatch — the
  C++ analogue of the Python/Go load-time checks, not merely an opt-in helper.

There is no negotiation protocol; incompatible native ABI is a hard, clear failure.

---

## 5. Minimum Supported Rust Version (MSRV)

**MSRV = Rust 1.85** (declared as `rust-version = "1.85"` in `Cargo.toml`).

This is an empirically established floor, not the compiler CI happens to run. Two
distinct notions of "MSRV" both land on 1.85, and CI enforces both:

**Resolved-release MSRV** — the floor for our **committed** dependency graph
(`Cargo.lock`):

- **Positive control:** `cargo +1.85.1 check --locked --lib` and
  `cargo +1.85.1 test --locked --lib` both pass.
- **Negative control:** `cargo +1.84.1 check --locked --lib` is *refused* by Cargo
  with: `unicode-segmentation@1.13.3 requires rustc 1.85.0`.

**Declared-range consumer MSRV** — the floor a fresh crates.io consumer actually
faces. A library does not hand consumers its lockfile, so this is tested with a
fresh, **unlocked** resolution of our declared dependency *ranges* on the MSRV
toolchain (`scripts/release/msrv-consumer.sh`):

- Both the MSRV-aware resolver (`resolver.incompatible-rust-versions = "fallback"`,
  stable since Rust 1.84) **and** the default (`allow`) resolver resolve our
  declared ranges to a graph that builds on 1.85. The maximum `rust-version` in the
  freshly resolved graph is `unicode-segmentation 1.13.3` (rustc 1.85.0), so the
  declared-range floor is **also 1.85** — a naive fresh consumer does not even need
  the MSRV-aware resolver to land on a buildable graph.

The floor is therefore a **dependency requirement**, not a LibGibson language/API
requirement — LibGibson's own source may compile on older compilers, but the
supported dependency set does not, so we do not promise below 1.85.

**Maintenance rule:** a dependency upgrade must not silently raise the declared
MSRV. If a bump makes the fresh-resolution consumer require a newer compiler, either
constrain the offending dependency deliberately or raise (and document) the MSRV —
never let it drift unnoticed.

Scope: the MSRV covers **building the supported library** (`cargo check`/`test
--lib`). It does not promise that every example, dev tool, or the full test harness
builds under the MSRV toolchain. Raising the MSRV is a `0.y` (minor) change and is
recorded in the CHANGELOG. CI enforces the floor with a scoped
`cargo +1.85 check --locked --lib` job **plus** the declared-range fresh-resolution
consumer (`scripts/release/msrv-consumer.sh`), so a dependency-driven MSRV creep
fails CI early.

---

## 6. Supported platforms

- **Linux x86_64 (glibc)** is the only platform built and tested this round. The
  release artifact is labeled `linux-x86_64` engineering alpha accordingly.
- Negotiated terminal capability axes: color depth (TrueColor / 256 / 16 / mono)
  and insert-line support. **Synchronized updates (CSI ?2026) are *requested*, not
  negotiated:** LibGibson emits the wrappers by default on a TTY (opt out via
  `Context::set_sync_updates(false)`) and does not gate them on capability
  detection, because unknown DEC private modes are ignored by conforming terminals.
  The `TerminalCapabilities::synchronized_updates` field is currently
  detected-but-unused; the cadence beacon treats sync on/off as a measured variable.
  `?2026` is a presentation request only: it guarantees neither atomic I/O (a write can
  still tear — see §9) nor atomic panel presentation.
- **Glyph realization is an explicit policy/override axis — it is not font
  detection.** A terminal accepting UTF-8/Unicode does **not** imply its font can
  realize every glyph: the Linux kernel **virtual console** (TTY), for example, has a
  bounded console-font repertoire and typically cannot draw the Braille block
  (U+2800–U+28FF), so sub-cell Braille degrades there even though layout, text, and
  structure render correctly. LibGibson does **not** — and cannot, over any terminal
  protocol — probe a font's actual repertoire. Instead it exposes a realization
  ladder, `glyph::SubcellGlyphMode` (`Braille2x4` → `HalfBlock1x2` → `Block` →
  `Ascii`), and a surface-level transcode (`glyph::transcode_surface_glyphs`) that
  re-realizes any sub-cell surface into a chosen family. Selection
  (`glyph::detect_glyph_mode`) is by explicit policy, precedence **`--glyphs=` (CLI) >
  `LIBGIBSON_GLYPHS` (env) > `Auto`**; `Auto` keeps Braille on ordinary graphical
  terminals and chooses a **conservative, console-safe** family when `TERM=linux`.
  That is a heuristic, **not** a claim that Braille is unavailable there — a custom
  console font may provide it, and an explicit override can always request it. The
  axis is orthogonal to color depth, and `Braille2x4` is the identity realization, so
  the existing Braille hero path is byte-for-byte unchanged. Applications must still
  not *assume* any particular sub-cell family renders on an arbitrary terminal; the
  ladder is how they degrade deterministically instead of guessing. See
  `docs/GLYPHS.md`.
- **Not covered this round:** Windows, macOS, tmux/screen/SSH certification, Kitty
  graphics protocol. These may follow; they are not promised here.
- **Windows is UNVERIFIED and unsupported this round.** There is no Windows CI job and no
  Windows host run. One defect is named from reading the source, not from a run
  ([#292](https://github.com/femboy2112/libgibson/issues/292)): crossterm 0.29 reports a Windows
  console key-up as `KeyEventKind::Release`, LibGibson passes that kind through on Windows, and
  the key is delivered a second time as a press, so **every key is doubled**. A blanket release
  filter is not the fix, because an Alt+numpad character reaches the console only as a release.
  The fix is a LibGibson-owned Windows console source (future work, needs a native Windows host).

---

## 7. What a release contains

- **Rust crate** (crates.io-ready `libgibson`): the Rust library, the C header
  (`include/gibson.h`, for native-library builders), and license/readme/design
  docs. The engineering-demo corpus (examples, integration tests + goldens,
  internal `docs/`, dev scripts, and the language bindings) stays in the repository
  but is excluded from the crate (`Cargo.toml` `exclude`).
- **Native SDK** (Linux x86_64): `libgibson.so.1`, a `libgibson.so → libgibson.so.1`
  development symlink, `libgibson.a`, `gibson.h`, `gibson.hpp`, a `libgibson.pc`
  pkg-config file, and the license/notice files. The shared object carries an ELF
  **SONAME `libgibson.so.1`** (stamped by `build.rs` via
  `-Wl,-soname`) whose major tracks `GIBSON_ABI_VERSION` (§3), so a dynamic consumer
  linked through `-lgibson` records `NEEDED = libgibson.so.1` and an ABI-incompatible
  future major can coexist. The static `libgibson.a` and the pkg-config `-lgibson`
  are unaffected. (macOS/Windows use different shared-object versioning and are out
  of scope this round.)
- **Python wrapper** package (sdist + wheel) that depends on a separately installed,
  ABI-compatible native LibGibson. Both artifacts carry the project's dual-license
  text (`LICENSE`, `LICENSE-MIT`, `LICENSE-APACHE` in the sdist and in the wheel's
  `.dist-info/`), with `License-File` metadata — not classifiers alone.
- **Go module** that consumes the installed native SDK via `pkg-config`. The module
  root carries the dual-license text (`LICENSE`, `LICENSE-MIT`, `LICENSE-APACHE`) so
  the tagged module zip ships it — a subdirectory module does not inherit the
  repository-root license.
- **Release bundle**: `libgibson-<version>-linux-x86_64.tar.gz` (the staged native SDK)
  plus a checksums file.

Artifact contents are made deterministic where practical (stable file manifest +
checksums). Byte-for-byte reproducibility is **not** promised this round.

---

## 8. Security support

See [`../SECURITY.md`](../SECURITY.md). Pre-1.0: only the latest 0.x line receives
fixes; there is no long-term stable security-support window; the private reporting
path is as stated in `SECURITY.md`.

---

## 9. What we explicitly do NOT promise (pre-1.0)

- No Rust API is frozen. 1.0 will define the frozen surface.
- Experimental modules (§2) may change or be removed.
- No cross-platform binary wheels or non-Linux artifacts yet.
- No byte-for-byte build reproducibility (deterministic manifest + checksums instead).
- No package-manager ecosystem distribution (Homebrew/AUR/deb/rpm) yet.
- **Input delivery under a simultaneous resize/input collision depends on the reader**
  ([issue #15](https://github.com/femboy2112/libgibson/issues/15)). crossterm 0.29.0's Unix
  event reader can withhold a key that arrives in the same readiness batch as a resize until
  a later key arrives (upstream
  [crossterm#1128](https://github.com/crossterm-rs/crossterm/pull/1128), not merged). Since
  0.7.0-dev, LibGibson no longer reads a Unix controlling terminal through that reader: its
  own terminal source (`src/tty_source.rs`, issue #269) takes the key and the resize in the
  same pass, and the [Event Pressure Lab](EVENT_PRESSURE_LAB.md)'s delivery acceptance for
  the `Context` path is enabled and passing. The v0.6.0 release (tag `v0.6.0` → `492d0fe`, on
  `release/v0.6.0-rc`) carries the same fix (#269 backported). Released versions up to 0.5.0, the 0.6.0 source baseline
  `9a899e1`, and an application that reads crossterm's event reader directly still have the stall.
  RXIV (2026-11-05) re-measured it on a real PTY isolated from LibGibson: stock 0.29.0
  delivered 0/30 resize-then-key collisions, the #1128 fork 30/30
  ([`research/issue15_crossterm_ab`](research/issue15_crossterm_ab/README.md)); it remains
  an accepted limitation for v0.6 until a crossterm release contains the fix.

### v0.6 Engineering Alpha limitations (stated, not hidden)

The v0.6 integration — merged to `main` through #253 as `f6cd8c7`, and released as v0.6.0 on
2026-11-08 (state 2 of §1, from `release/v0.6.0-rc` @ `492d0fe`) — is qualified in
[`V06_QUALIFICATION_LEDGER.md`](V06_QUALIFICATION_LEDGER.md). These are its declared limits:

- **Terminal writes are not atomic.** A failed write may leave a torn prefix of a frame on the
  terminal; synchronized-update framing is a display hint, not I/O rollback. The renderer reports it
  (`Renderer::physical_state_uncertain()`) and re-establishes the screen on the next frame —
  exactly in fullscreen mode; in inline mode the torn region is abandoned to scrollback, not
  repaired. A failure that delivered no bytes is certain and retried in place.
- **Keyboard stack transitions are best effort after I/O failure.** A failed pop write or flush
  does not establish whether the terminal applied the pop; retrying can remove a foreign stack
  entry. A failed re-push may have applied no push or a partial one. No atomic restoration is
  claimed for these ambiguous cases, even though a successful pop followed by a failed screen
  switch is accounted for without a second pop (#305).
- **The C ABI's headless context is output-only.** In the 0.6.0 release (`release/v0.6.0-rc`, #259
  backported) `gibson_poll_event` on a headless context reports no input and never reads the
  process terminal; there is no event-injection entry point and no sub-cell pixel raster.
  Interactive parity across C++/Go/Python is not claimed beyond the existing parity harnesses.
- **A headless C context's ANSI capture grows for the context's lifetime**
  ([#297](https://github.com/femboy2112/libgibson/issues/297)). Every rendered frame's bytes are
  appended; `gibson_get_rendered_output` (and the Python wrapper's `rendered_output()`) copies them
  and never drains, and 0.6.0 has no C drain (Rust has `Context::take_output`). Measured on the
  release candidate: a frame whose every cell changes adds about 657 bytes at 40×15, 1,995 at 80×24
  and 19,387 at 240×80; an unchanged frame adds nothing. A C program that renders indefinitely must
  bound the context's lifetime: query the capture size (a size query never drains) and, past a byte
  budget, destroy the context and create a new one. That procedure is tested against the installed
  SDK by the clean-room consumer `bindings/sdk_forcing/headless_capture_lifetime.c` (capture never
  above budget plus one frame, rendering continues after each replacement, resident memory flat
  across replacements); unbounded rendering on one context is not a supported 0.6.0 use. The
  additive drain is v0.7 work.
- **The cinematic effect contract is single-quantity.** `field_cinema` judges the peak sample
  against the requested amplitude with a declared tolerance; it claims consistency with a request,
  never mechanism, and causal attribution only where a matched counterfactual exists.
- **Capture / trace checksums are FNV-1a receipts**, not authentication; replay reproduces
  decisions from recorded observations and does not re-execute a producer. These formats are
  example-level, not a stable public file format.
- **The performance envelope is one host's measurement** (`V06_PERFORMANCE_ENVELOPE.md`): no
  universal frame rate, no display latency, no streaming observation age.
- **Text-input content is control-safe; the low-level cell API trusts its caller.** Since RXV
  (#252) no C0, DEL or C1 control in text-input content — from `TextInputState`,
  `Node::text_input` or `gibson_node_text_input` — can reach the terminal: insertion drops controls
  and the painter draws any control-bearing or standalone zero-width grapheme as one `U+FFFD` cell.
  `Node::text` / `Surface::print_str` skip control characters (unchanged) but attach zero-width
  *format* characters (general category Cf, e.g. bidi overrides) to the preceding cell and emit
  them; these are not terminal controls, but a terminal that implements bidirectional text may
  reorder the line — this includes an unfocused `ui::text_input`, which is lowered to `Node::text`.
  `Surface::set_cell`, `Glyph::new` and the `*_raw_ansi_unchecked` entry points place exactly what
  they are given, by design; content handed to them must already be safe.
- **Terminal widths follow one model, not every terminal.** The cell model measures with
  `unicode-width`: East-Asian-ambiguous characters (box drawing, `U+FFFD`) are narrow, and an emoji
  ZWJ sequence or a VS16 emoji-presentation sequence is one 2-cell cluster. A terminal configured
  for ambiguous-wide characters, or one that draws ZWJ/VS16 components per codepoint (as the `vt100`
  model does), can place later glyphs differently. The text editor lays a grapheme wider than two
  cells codepoint by codepoint; elsewhere (`Node::text`) such a grapheme is still capped at two
  cells and can drift by the excess on per-codepoint terminals. There is no runtime width probing
  (e.g. mode 2027).
- **Issue #15** (above): fixed for LibGibson's own input path; crossterm's reader still has it.
