# LibGibson v1.0 — Rust-equivalent Language ABI Parity

> **Status: OWNER-REQUESTED FUTURE RELEASE GATE, DESIGN ONLY / NOT IMPLEMENTED.**
> Owner direction (2026-10-09): **bringing supported language APIs up to Rust-equivalent expressive capability is part of the v1.0 roadmap**, not an optional polish task. The milestone skeleton may evolve with feature additions; the parity standard does not silently disappear when scope grows.
>
> Research-only companion to [Terminal Doom v0.6–v0.7](DOOM_TERMINAL_0_6_0_7_PLAN.md) and the [integration/validation handoff](DOOM_TERMINAL_PLATFORM_HANDOFF.md). Branch forked from \`main@80706c91b88e59495b131bfd8ab23aa3ed013414\` (v0.4.0). This document does **not** modify the existing release contract or claim API parity exists today.
>
> Existing \`docs/RELEASE_CONTRACT.md\` says 1.0 freezes an actually stable CORE API; the owner's requirement **adds** a parity gate. Reconcile both explicitly before publishing 1.0. Do not overwrite the release contract from an isolated research branch while ongoing feature branches are moving.

## 0. Operational definition: parity of capability, not syntax

For each capability \`c\` officially included in the supported **1.0 stable feature surface**, there must be a usable implementation route through Rust, C, C++, Python and Go (unless the maintainer explicitly changes the declared supported-language set *before* release). The routes must share one underlying engine and demonstrate equivalent **observable contracts** on the declared supported platform/configurations.

This does **not** require Rust traits, closures, lifetimes, generic types or syntax to be duplicated in C. It requires that a C caller, for example, can **create a drawable raster, send new frames, compose it into a scene, receive input, control lifetime/resize and obtain the same output semantics** as a Rust caller. C++ may provide RAII; Go and Python may use safe owned wrappers over the identical C ABI.

**No second engines in bindings.** Foreign users should not need to reimplement Rust rendering, scene progression, animation, audio or terminal diff algorithms.

### Stable-core classification: prevent fake success

A project can have internal Rust-only implementation details and experimental features during 0.x. Before declaring 1.0, **every publicly promoted stable feature** must carry a parity row. If an experiment remains Rust-only, its status must be conspicuously EXPERIMENTAL/NOT-STABLE and it cannot be advertised as part of fully supported 1.0 capability. The maintainer must review any large or mature Rust-only subsystem deferred from stable scope rather than "passing" by hiding almost every feature from the matrix.

Track two independent obligations:

1. **Parity closure:** the agreed 1.0 stable semantic feature family is actually reachable and behaves equivalently from all supported languages.
2. **Stable compatibility:** version/ABI/ownership/error contracts are supportable long-term and do not force clients to rely on an unstable private escape hatch.

A binding that compiles, or a wrapper that exposes a similarly named method, is **not parity**.

## 1. What exists and what is missing at the 2026-10-09 baseline

Observed in repository main @ \`80706c91...\`:

- \`include/gibson.h\` exposes a versioned C ABI (v1), opaque \`gibson_context_t\`, \`gibson_node_t\`, \`gibson_line_t\`, \`gibson_rich_text_t\`, basic terminal events/render calls and \`gibson_stats_t\`.
- \`include/gibson.hpp\` is a C++ RAII wrapper for a subset of the C surface; \`bindings/cpp/\` has a consumer example. Python \`ctypes\` and Go \`cgo\` bindings and consumer examples exist.
- \`abi/gibson-abi-v1.symbols\`, \`scripts/release/check-abi.sh\`, \`scripts/dev/bindings_smoke.sh\`, release preflight, C/C++/Python/Go CI are already part of the project. These are a **starting floor**, not evidence of full Rust equivalence.
- The Rust engine has \`RgbRaster\`, \`HalfBlockCanvas\`, \`BrailleCanvas\`, raster nodes, Scene/Story, temporal display, advanced UI and an experimental HumanMusic/audio axis not fully available to C/C++/Python/Go.
- The existing \`gibson_event_t\` loses key press/release phase in its Rust source adapter, which an external Doom input consumer exposes as a real behavior gap.
- Doomgeneric needs a **bulk pixel-frame interface** absent from the C ABI at the snapshot. The \`0.6\` route must build that general capability first.

These statements are **source observations**; no new external parity CI or game run was performed while writing this roadmap.

## 2. Binding stack and invariant

~~~text
                ONE Rust implementation
  terminal, raster, diff, Scene, Story, UI, temporal,
  audio/music/DSP, scheduling, interaction contracts
                           |
             versioned *common C ABI*
    tagged opaque handles / buffers / events / status
          resource ownership + error semantics
          /             |            |            \
        C             C++          Python          Go
 (direct calls)   (RAII layer)     (ctypes)       (cgo)
~~~

**Do not export raw internal Rust structs or trait objects** merely to make signatures resemble Rust. Define narrow, versioned C data types with checked discriminants, \`struct_size\`/version headers where appropriate, explicit status codes and handle lifecycle. Preserve the existing ABI v1 behavior while adding functions. Existing exported symbol and data layout changes require the documented breaking-ABI procedure; package version and ABI version are separate. ABI v2 is allowed **when an actual incompatibility is justified**, never as a ritual for 1.0.

All foreign clients use the same reference model:

- **Memory:** the caller knows whether a pointer is borrowed for the call, copied synchronously or retained by an opaque engine handle; zero-copy requires specific lifetime and aliasing rules. No Rust \`Vec\`/\`String\` layout crosses directly.
- **Identity:** stable resource handles, ownership/transfer/retain/release, parent-child and borrowed-view lifetimes are explicit and tested; a destroyed owner invalidates descendants as documented.
- **Error behavior:** bad pointer, dimensions, unsupported capability, busy terminal, violated thread ownership, invalid UTF-8/encoding, and version mismatch yield documented errors; no unwinding across FFI.
- **Concurrency:** callback thread, reentrancy, signal/terminal lease constraints, synchronization and allowed calls from audio thread are specified. Avoid allocating or locking in real-time audio callbacks.
- **Event fidelity:** press/repeat/release/unknown keyboard kinds, modifier meanings, size/focus/control events and capability origin stay representable even if a platform cannot supply every phase. A degraded terminal mode is reported, not disguised as full keyboard semantics.
- **Observability:** rendered frames, exact changed cells, bytes, timing, audio sample scheduling, deterministic trace and diagnostic receipts stay inspectable where the Rust stable contract exposes them.
- **Feature negotiation:** unsupported backend/capability is an explicit result; do not simulate unsupported graphics modes invisibly in one wrapper and claim semantic parity.

## 3. Capability inventory and gap matrix

This is a **starting skeleton**, not a fabricated completed test matrix. Rows should be split into auditable API families when a feature becomes stable. Each cell needs the actual source symbol/wrapper function, version/commit, external consumer test, and evidence. Status vocabulary: \`Verified\`, \`Partial\`, \`Absent\`, \`Experimental\`, \`Blocked\`, \`Unknown\`. Unknown is not success.

| Family | Rust baseline | C | C++ | Python | Go | Potential graduation evidence |
|---|---|---|---|---|---|---|
| Terminal session, render lifecycle, resize, stats | Exists | Partial | Partial | Partial | Partial | ownership/resize/restore comparison |
| Basic tree/text nodes, rich text, interactive input | Exists | Partial | Partial | Partial | Partial | visually equivalent output and events |
| Bulk RGB frames / raster nodes | Exists | **Absent** | Absent | Absent | Absent | Doom-style external RGB producer |
| Subcell fallback, color and glyph capability | Exists | Partial/Absent | Partial/Absent | Partial/Absent | Partial/Absent | same frame across color/glyph modes |
| Scene/Story and interactive cinematic composition | Experimental | Absent | Absent | Absent | Absent | matching semantic event sequences |
| Temporal image/display and observable diagnostics | Experimental | Absent | Absent | Absent | Absent | same static fallback, timing contracts |
| High-level UI/focus/control state | Experimental | Partial/Absent | Partial/Absent | Partial/Absent | Partial/Absent | same state/action result through wrappers |
| HumanMusic/audio event/render control | Experimental | Absent | Absent | Absent | Absent | accepted source identity, same PCM where claimed |
| GSPU-FM/Meatsack (future) | Research/experimental | Not built | Not built | Not built | Not built | defined only after implementation/stabilization |
| Resource handles/callbacks/errors/ABI negotiation | Core partial | Core partial | RAII partial | ctypes partial | cgo partial | hostile lifecycle and version mismatch |

**This table records interface availability impressions from source, not a fresh implementation-by-implementation audit.** Do not promote any cell to Verified without running its specific external test on the actual current head.

### Living changes policy

When a new requested feature is added to LibGibson:

1. Add it to this matrix as \`Research\` or \`Experimental\` with an owner and intended maturity.
2. Identify its semantic contract and whether it introduces new binary resources/events.
3. Track the minimal cross-language route early, but do not force premature stable ABI commitments.
4. Before promotion to stable, require C ABI representation, ownership/error tests, and wrappers across the supported language set.
5. If timing makes the feature too large for the current milestone, **move the feature or its stability promotion**, not the integrity/parity gate.

This keeps the skeleton expandable without allowing technical debt to vanish in release marketing.

## 4. External parity tests: the real proof instrument

For each stable capability \`c\`, define a finite contract test set \`T_c\`. Implement a minimal independent consumer per supported language using **only its public installed SDK**. The test compares outputs through an independent semantic oracle:

~~~text
Rust reference consumer  \
C consumer               \
C++ consumer               ---> normalize observable trace ---> equality/invariants + residuals
Python consumer           /
Go consumer              /

output trace: source inputs, screen cells/colors, emitted bytes where promised,
             events, errors, resource lifecycle, PCM hash within specified scope,
             deterministic clocks/receipts as applicable
~~~

No language's wrapper implementation gets to be its own oracle. Tests should compare to an independent expected model or fixture wherever possible; if all wrappers share a buggy ABI, their mutual agreement alone is not validation. Preserve raw outputs, parser/projection versions and exact head SHAs. Use hostile and mutation controls to show witnesses can reject broken cases.

**Required adversaries by category:**

- Buffer memory: wrong stride, truncated pixel buffer, overflowed dimensions, null or freed handle, retained borrowed pointer, invalid alignment/format, CPU/memory budget exceeded, resize mid-frame.
- Ownership: double-free, use-after-destroy, leaking refcounts, transferring a node twice, terminal lease conflicts, unhandled exceptions during C++ RAII, Python GC lifecycle, Go cgo pointer rules.
- Events: real press/release, protocol-incapable terminal, pasted escape sequences, repeated input, simultaneous controls, resize/focus, stuck-key recovery, terminal restore on interrupted game.
- Renderer: same pixel import and scene overlay across language clients, different color depths, wide glyphs, offscreen clipping, dirty region/full repaint, headless PTY/vt100 reconstruction and real display limitations.
- Audio (when promoted): identical source and scheduling, silence and release, patch ID/version, per-voice limits, callback safety and PCM equality only on declared platform/toolchain configuration.
- Errors/compatibility: wrong ABI version, undersized/oversized versioned struct, unknown discriminants, old v1 consumer on new library, new consumer with old library, unsupported feature/refusal.

Every parity claim names its platforms. Linux x86_64 is the historical Engineering Alpha support floor; macOS, Windows, tmux, screen, SSH, WSL and mobile require **actual tested support** to be advertised. Headless VT100 coverage is not a guarantee for every terminal emulator.

## 5. Milestone allocation (editable skeleton)

| Candidate version | Language/API work | Gate |
|---|---|---|
| **0.6** | Additive reusable pixel/raster C interface + expanded input events; C++ RAII wrapper; clean-room C/C++ consumers | [Doom platform contract](DOOM_TERMINAL_0_6_0_7_PLAN.md), exact old ABI retained |
| **0.7** | Doomgeneric C consumer and real-world stress; add missing general graphics/input capabilities rather than hacks | game runs through documented C SDK and terminal is restored correctly |
| **0.8** | Scene/Story/temporal/UI/audio public ABI coverage in reviewable clusters | per-family capability+ownership definitions and minimum two-language consumers |
| **0.9** | Complete Python/Go/C++ ergonomics and cross-language external test matrix; ABI and compatibility hardening | all planned stable 1.0 rows reach Verified; no unexplained failures |
| **1.0** | Freeze the stable semantic capability contract and supported SDK distribution | Rust-equivalent stable capability access through all declared languages, reproducible independent tests |

The version labels and sequence can change. **No v1.0 is released with a hidden unresolved stable-feature parity hole**. The user's intent is a *substantively usable* common engine, not a C header that exposes only text while Rust users get the entire visual/audio universe.

## 6. 1.0 release readiness criteria

The release is eligible to claim language parity only if all hold:

1. A human-reviewed, source-mapped **stable capability manifest** is frozen at a named commit and names which Rust features are in/out, with justifications for experimental deferrals.
2. For every included capability, public C, C++, Python and Go routes exist with checked lifetime/callback/errors; at least one independent external consumer per language demonstrates the capability (batch related assertions where justified).
3. Behavioral equivalence is verified with positive, negative, null and mutation controls, including representative hostile resources and actual terminal sessions. Claims that require live sound or terminal visual fidelity also get real-device/ear evidence.
4. The original ABI v1 baseline is preserved or a deliberately breaking, documented version migration is validated against old/new clients. Package semantic version, C ABI version and wrapper package versions are coherent.
5. Distribution smoke tests load the installed SDK without relying on repository-relative paths or Rust internals. Multi-platform support is limited to **actually proven platforms**, not implied universal compatibility.
6. No Rust-only shadow implementation of a stable feature is silently smuggled behind a "foreign parity" facade. Source drift, freeze conditions and future feature growth have a declared maintenance path.
7. The maintainer explicitly approves the release; automated tests, documentation or a demo cannot grant that approval.

## 7. Parity-focused engineering strategy

Prioritize **one authoritative C ABI**, not four independent technology stacks. Use narrow opaque handles and high-level batch operations. Prefer mechanical metadata/code-generation for repetitive declarations only after validating the generated output and independently testing at least one target; avoid blindly autogenerating ABI unsoundness.

The Doom platform is an early canary because it exercises frame bandwidth, resize, input truthfulness, long session lifetime and terminal corruption—all things a text-only demo misses. But don't let Doom alone determine the abstractions; prove the same C primitives work for at least one non-game raster/temporal client.

Next pressure tests after Doom should include a C++ cinematic app, a Python scientific raster program, a Go concurrent console and—once stabilized—an audio/GSPU consumer that accepts events from external code. Test that all call the SAME engine and produce comparable output under controlled inputs. Move mature Rust features to stable with a matching parity implementation rather than leaving all of them experimental indefinitely.

## 8. Provenance, open claims and handoff

**Observed:** v0.4.0 Rust graphic/temporal/audio capabilities exist, foreign C API exposes a smaller subset, existing release contract defines ABI version rules. **Owner-defined release requirement:** parity by v1.0. **UNVERIFIED:** any newly proposed raster/event API, Doom gameplay on public C SDK, arbitrary-platform compatibility, full Scene/Story/audio binding parity. **Not claimed:** current 1.0 readiness.

See \`ROADMAP.md\`, \`docs/STATE_OF_LIBGIBSON.md\`, \`docs/RELEASE_CONTRACT.md\`, \`include/gibson.h\`, \`include/gibson.hpp\`, \`src/ffi.rs\`, \`bindings/{c,cpp,python,go}\`, \`scripts/dev/bindings_smoke.sh\`, \`scripts/release/check-abi.sh\`, and \`abi/gibson-abi-v1.symbols\` at current implementation time.

**Future integration is explicit:** do not edit \`ROADMAP.md\` or replace the release contract from a stale docs-only base without reconciling active branches. When C137/Meatsack and the v0.5 milestone settle, transplant the reviewed plan or link it from the live roadmap, refresh feature matrix and maintain the real parity table with commit-level evidence.

**Guiding sentence:** if Rust can use a **supported stable** LibGibson capability, C, C++, Python and Go should be able to use the same capability, with the same meaningful guarantees, through the same underlying engine.
