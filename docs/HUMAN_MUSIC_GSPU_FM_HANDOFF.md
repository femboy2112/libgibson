# GSPU-FM — deferred implementation handoff and falsification matrix

> **HOLD: documentation only.** Read [the authoritative FM design](HUMAN_MUSIC_GSPU_FM.md) in full before acting. This research branch has **no FM engine implementation**, no chip-compatibility certification, and no permission to integrate automatically.
>
> Scope approved for design: **Genesis/OPN2 first, Sound Blaster OPL2/OPL3 second; high-definition FM as an opt-in extension**. Sample/SNES-HD work is unscheduled and must not delay the FM effort.
>
> Design source: research/humanmusic-gspu-fm-2026-10-09 (created 2026-10-09 from main @ 80706c91b88e59495b131bfd8ab23aa3ed013414).
> Historical C137 @ feat/v0.5-humanmusic-beefup / 530f400b4bb43fdbd61db4cef1da6374e90d0c56, Meatsack research @ b9cccb5f892719d8592438ac5e9780a944fe6e1e; **all versions must be refreshed** at actual launch.

## A. Launch gate / ordering

**Do not launch merely because you discover this file.** The maintainer's presently active work is C137 semantic narrative and its MeatsackProjection integration. This document is for **future** work after that project's acceptance/freeze and an explicit new FM go-ahead.

At FM go-ahead:

1. Check GitHub current main, branch refs, PR status, repo instructions, the accepted C137+Meatsack integration SHA, tools and test baseline. Record **START SHA** in a new log.
2. Read the live post-integration implementations of instrument.rs, synth.rs, dsp/osc.rs, dsp/env.rs, world.rs, time.rs, render.rs, voice.rs, audio graph, source receipts, existing GSPU/FM code (if any), and relevant tests. Read the latest C137/Meatsack acceptance and physical/acoustic assumption documents. The design's line numbers are **not** frozen.
3. Work on a **new non-main implementation branch forked from the accepted live integration SHA**. This original research branch remains unchanged as design provenance. Do not cherry-pick an old v0.4 source tree.
4. Prove old controls unchanged **before** adding a single new audible feature: v0.4 PCM/patch bytes, R17, covers, canonical + legacy fingerprints, C137 source/narrative receipts and Meatsack source/gesture IDs, insofar as present at launch. Report any archival fixture migration.
5. Execute the smallest zero-audio-change type/compiler scaffold first. Only then build a 2/4-operator signal path and chip dialects. No new default world tone/preset without an explicit ear gate.

## B. First bounded prototype (proof of useful architecture, not chip emulation)

Keep the legacy oscillator and historical Patch path untouched; expose an explicitly opt-in **CompiledFmProgram**, through a new sound-palette/render configuration rather than SongMap mutation.

Proposed minimum internal contract (pseudocode, NOT current API):

~~~text
FmProgramDescription {
  dialect: HD | OPN2_Inspired | OPL2_Inspired | OPL3_Inspired,
  operators: [frequency_mode, ratio_or_hz, level, envelope, waveform, key_scale],
  edges: [source_operator, destination_operator, coefficient_rad, delay],
  outputs: [carrier_operator, output_gain, pan],
  resource_budget: [voice_count, max_operators, max_feedback_paths]
}

compile(program) -> Result<CompiledFmProgram, ProgramError>
note_on(compiled, source_event_id, original_time, pitch, velocity, controls)
  -> RealizationEvent
render(compiled, prepared_voices, integer_sample_time) -> PCM + RenderReceipt
~~~

This is a *proposal*: first inspect current types, minimize duplication, and choose smaller structures if sufficient. Keep chip dialect compatibility separate from HD "bigger numbers" behavior. No allocator/lock or graph search in the real-time callback. Explicit feedback state must be well-defined (e.g. delayed edge), and zero-delay cycles rejected.

Crucial historical detail: the existing FmOsc modulation index is specified in **cycles**, whereas the proposed operator graph should use a declared unit such as **radians**. Do not silently equate them; for comparable pure pairs the phase-depth conversion is beta_rad = 2*pi*index_cycles. The existing historical FM voice also sums an unmodulated oscillator; tests must isolate that accidental/intentional layer rather than claiming exact equivalence to a pure FM pair.

Do not add 6/8-operator programs until four-operator Genesis behavior and two/four-operator OPL behaviors are correct enough to justify generalization.

## C. Predeclared falsifiers

Before fitting a new FM voice, define PASS / FAIL / AMBIGUOUS outcomes for each case. Freeze program parameters, source score, sample rate, test harness and expected diagnostics before first test contact. Keep raw numeric traces, WAVs, and reasoned refusals.

| ID | Input / mutation | Required witness | Failure interpretation |
|---|---|---|---|
| F01 | One pure carrier, every modulator coefficient zero | Only declared carrier; correct frequency/phase/gain | Ghost sine from legacy layering or routing bug |
| F02 | Modulator-only graph with no declared output carrier | Silence or typed invalid program, per contract | Undeclared audio output |
| F03 | Pure two-sine PM, fixed beta, known fc/fm | Sidebands near Bessel J_k(beta) in alias-controlled setting | Wrong modulation phase/units/topology |
| F04 | Scale beta, fixed song/note/ADSR | Predictable changed sideband family; source timing unchanged | Modulation mislabeled as amplitude or tempo |
| F05 | Independent per-operator envelope mutation | Sound/spectrum change with source note identity fixed | Envelope is no-op or shared by accident |
| F06 | Zero-delay cycle vs legal delayed feedback | Illegal cycle rejected; legal feedback bounded/deterministic | Undefined recursion or unstable feedback |
| F07 | Same program/event stream rendered with different block sizes | Equal PCM under same exact platform/run settings | Callback-size-dependent oscillator/control state |
| F08 | Long release, rapid key-off/retrigger, voice steal | No stuck tail, lost scheduled note or unplanned reattack | Invalid lifecycle / note provenance |
| F09 | High carrier, strong index/feedback across sample rates | Quantified alias energy against oversampled reference; no NaNs | "Alias-free" heuristic is an unsupported absolute claim |
| F10 | Disable new program path / GSPU off | Historical PCM, R17, covers, fingerprints unchanged | Regressive refactor: STOP |
| G01 | OPN2 eight algorithm topologies | All correct graph maps; suitable reference cases distinguish them | Naive universal chain labeled OPN2 |
| G02 | OPN2 strict six-channel profile with seven held notes | Refuse/steal under explicit deterministic priority | Unreported phantom chip voice |
| G03 | Optional OPN2 ch3/SSG-EG/DAC requests when unimplemented | Typed Unsupported; do not silently emulate incorrectly | Overclaimed Genesis compatibility |
| O01 | OPL2 2-op mode | Legal operator/waveform/channel restrictions | OPN2 defaults masquerading as OPL |
| O02 | OPL3 legal 4-op channel pairing + unpaired channel pressure | Paired resource accounting, topology and output routing | Two arbitrary 2-op voices labeled one 4-op |
| O03 | OPL rhythm mode | Synthesized/allocated allowed percussion paths; no sample bank | Phantom voice budget or hidden sample dependency |
| I01 | Same source SongMap/Score through old/FM variants | Song/perf fingerprint, chord/motif identities and source links unchanged | Timbre rewrite of musical meaning |
| I02 | Mutate bass/keys or lead-withhold sound program | Only sound changes; source carrier/intentional silence preserved | Extra pseudo-notes defeating narrative |
| I03 | FM envelope/tail changes hearing assumptions | Assumptions rechecked or typed refusal before acceptance | Invalid upstream occupancy/temporal claims |
| I04 | Meatsack one explicit strum/flam with multiple linked excitations | Excitations share original gesture cause; FM responds to declared controls | FM invented independent note jitter/gesture |
| A01 | Balanced A/B same score, gain, processing, dry/wet | Listener reports specific quality or ambiguity, no forced preferred take | Uncontrolled loudness/preset comparison |
| P01 | Long render and non-contiguous seek request | Bounded memory or documented limit; exact seek or explicit refusal | Regression to known #74/#78 walls |

**Positive controls:** pure sine; historical 2-op FM path as *legacy characterized* signal (including its extra oscillator); a simple known 4-op graph; supported OPL2 2-op voice; generated C137 positive narrative fixture.

**Negative controls:** disconnected carrier; intentionally reversed modulation edge; wrong beta units; wrong OPL pair; phantom seventh OPN2 voice; independent-onset jitter not owned by score/rig; identical timbre offered under two distinct algorithm labels.

**Null/nuisance controls:** index=0, identical preset across render calls, silence, no event, no modulation, gain matched dry/wet, sample-rate change with normalized pitch, identical seed with different render block boundaries.

**Independent triangulation:** analytic phase-modulation sideband derivation; a clean independent test implementation; optionally externally licensed chip emulator outputs as **behavioral reference without copying code**. A wrong shared interpretation can survive multiple libraries; source implementations and reference inputs must be provenance-labeled.

## D. Required instrumentation and exact meaning of proof

Maintain separate receipts:

1. **ProgramCompileReceipt**: parsed operator count/graph, edge units, topo order, feedback delays, dialect feature support/refusal, estimated resource budget; independently re-check graph validity.
2. **RealizationReceipt**: original source note/gesture IDs, chosen program, exact event/sample indices, voice allocation/steal, envelope/control program ID, finite samples and clipping counters. No recomputed source identity from modified onset.
3. **SpectralEvidence**: raw samples, reference spectrum, sideband/alias residual, sample rate, window length/taper, oversample factor, frequency regime; distinguish exact analytically expected components from pragmatic tolerance budgets.
4. **Auditory/ListeningEvidence**: levels/stems, render hashes, anonymized A/B order, human verdict with date and actual filenames. Observer measurements describe signal structure, never certify human musicality.

Do not sum those into one confidence/quality scalar. Claims inherit the weakest proven bridge. Different chip settings that produce similar sound are not evidence of register-level hardware equivalence.

## E. Engineering and release gates

Required at every causal checkpoint, adapted to repository state:

~~~bash
git status --short
git rev-parse HEAD
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --no-fail-fast
cargo build --examples
git diff --check
~~~

Use the repository's pinned toolchain as available. Run targeted release/optimized builds, deterministic render/FFT scripts, CPU and bounded-memory measurements, MSRV checks and exact-head hosted CI as appropriate; include actual commands, exit codes and platform/source SHAs. **This handoff itself runs none of those tests.**

Keep new engine feature OFF by default. No patch-world color shift, tempo/harmony change, released ABI change, new mandatory device/system dependencies, bundled sample assets or sample-driven path as a quiet prerequisite. Source patches/weights should be original. Test cross-OS determinism only with declared tolerance and toolchain; bit-exact equality is not promised across arbitrary floating-point implementations.

Accepted audio improvements require listener verdict. If OPN2-inspired presets sound good but strict chip control vectors fail, label the result **OPN2-inspired**, not "verified emulator". If a full FM program simply sounds worse than the existing VAPOR95 keys, preserve the original and treat FM as a dial.

## F. Implementation branch and commits (deferred)

Only after explicit future start, commit on a new non-main branch based on the **accepted C137+Meatsack integration head**. Suggested causal commit sequence, not a mandate:

1. docs(audio): reconcile GSPU-FM design to accepted source
2. test(audio): pin FM legacy layering, radians/cycles and spectral negative controls
3. feat(audio): compile typed two/four-operator FM programs (no audio default)
4. test(audio): validate graph topology, feedback, envelopes, lifecycle and deterministic render
5. feat(audio): OPN2 eight-algorithm inspired program and source-equal sound lab
6. test(audio): chip resource limits, algorithm mutations, reference spectral holdouts
7. feat(audio): OPL2/OPL3 dialect and paired four-operator resource model
8. feat(audio): measured HD render quality mode, if earned
9. test(audio): integration: same C137 source + optional Meatsack execution, multiple FM palettes
10. docs(audio): listening packets, evidence ledger, unsupported features, API and maintenance contract

Push a **draft PR**, observe exact-head CI, but do **not** merge, tag, publish, expose a new stable ABI, or enable a sound-changing default without the maintainer's explicit approval.

### What to hand back

- Repository/ref provenance: exact accepted base, implementation HEAD, design-branch SHA and review links.
- What changed and why: original cause, live mechanical obstruction, chosen correction, alternatives that failed.
- Math: operator graph semantics, units, feedback/timing law and explicit limitations.
- Chip-coverage matrix: Inspired vs VerifiedSubset vs HardwareAccurate, per feature, with reproducible tests.
- Files/commands: source paths, tests, logs, baseline hashes and new WAV/stem paths.
- Performance: voice/polyphony CPU and memory budget, block consistency, alias profiles.
- Listening: what human actually judged vs what is merely machine-valid.
- Next smallest decisive discriminator.

## G. Future-session cold-start prompt (paste only when the maintainer authorizes FM work)

> Continue libgibson GSPU-FM from its isolated research design. First read docs/HUMAN_MUSIC_GSPU_FM.md and docs/HUMAN_MUSIC_GSPU_FM_HANDOFF.md on research/humanmusic-gspu-fm-2026-10-09. Verify their parent commit and fetch the current accepted C137+Meatsack heads, repo instructions, tests and existing architecture. This is primarily a Genesis YM2612/OPN2 four-operator FM project, with Sound Blaster OPL2/OPL3 2/4-op sound as the second chip dialect and optional modern HD rendering; sample playback/SNES is deferred. On a fresh feature branch from the accepted implementation base, reproduce historical FM's two-operator-with-extra-sine behavior and byte-exact OFF controls, then red-first implement a compact compiled FM operator graph with explicit phase units, carrier selection, independent operator envelopes, delayed feedback, bounded prepared voice state and honest chip-resource modes. Build actual OPN2 eight-algorithm and OPL3 legal pairing semantics rather than a generic FM patch falsely labeled a chip. Calibrate spectral accuracy analytically and against independent reference routes; quantify aliasing without claiming perfect alias freedom; keep samples and third-party copyleft emulator source out of the core absent separate approval/licensing review. Preserve C137 SongMap/motif/narrative/source identities and Meatsack gesture/physical authority, keep acoustic assumptions valid, test controls/negative mutants/hostile regimes, and deliver original level-matched A/B WAVs and stems for human judgment. Make reversible scoped commits with exact-head CI on the feature branch; no merge, version bump, new default sound or release. End with an honest chip-coverage/claim ledger and the next falsifying probe.

**Standing invariant:** FM changes the *instrument's voice*, not the *song's statement*. Make the Genesis and Sound Blaster dialects worthy of the narrative band, but do not make the band dependent on a bank of samples.
