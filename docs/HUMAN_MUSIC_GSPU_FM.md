# HumanMusic / GSPU-FM — Genesis + Sound Blaster Synthesis Architecture

> **STATUS: RESEARCH DESIGN ONLY — FUTURE INTEGRATION / HOLD.**
> No source code, runtime behavior, release metadata, sample assets, or active C137/Meatsack branch changes are authorized by this document.
> Design branch: **research/humanmusic-gspu-fm-2026-10-09**, created independently from main at **80706c91b88e59495b131bfd8ab23aa3ed013414** (v0.4.0).
> Snapshot: **2026-10-09**. At snapshot, C137 work: **feat/v0.5-humanmusic-beefup @ 530f400b4bb43fdbd61db4cef1da6374e90d0c56** (draft PR #87), and Meatsack research: **research/humanmusic-meatsack-projection @ b9cccb5f892719d8592438ac5e9780a944fe6e1e**. All are moving/independent; re-read actual heads before implementing.
>
> **Owner's scope decision:** prioritize **Sega Mega Drive / Genesis OPN2 (YM2612/YM3438)** and **Sound Blaster OPL2/OPL3 (YM3812/YMF262)** FM synthesis. No available/licensed sample collection is assumed. SNES-inspired sampled sound is preserved as a possible **unscheduled future adapter**, not a dependency, release criterion, or excuse to divert the FM project.

## 0. Mission and non-mission

Build an optional, self-contained, fully programmable FM sound engine within the existing Rust-only gibson::audio substrate. Its concrete north stars are:

1. **Genesis-style 4-operator FM**: expressive bass, EP, brass, leads, bells, organs, percussion and timbral movement, drawing from the OPN2 algorithm/control family.
2. **Sound Blaster-style FM**: OPL2 2-operator and OPL3 2/4-operator program semantics, rhythm synthesis and recognizable DOS-era textures.
3. **High-definition rendering**: modern sample-rate support, deliberate anti-alias options, deterministic offline behavior, bounded real-time performance, optional clean/retro output character.
4. **HumanMusic interoperability**: C137 retains composition and ensemble narrative; Meatsack retains physical feasibility and gesture execution; the GSPU owns sound-program execution. Identical song/gesture source IDs across sound-palette swaps.

**Not** a second composer, physical instrument model, universal "humanity" score, chip-ROM extractor, game soundtrack copier, immediate bit/cycle-exact emulator, or mandatory sample player. FM does not require samples; OPN2 DAC playback and SNES sample engines are explicitly deferred.

**Definition of done is a vector**, not one "sounds retro" score: musical-source invariance; graph/control correctness; measured spectral accuracy; distinct chip-dialect constraints; stable rendering/performance; ear-accepted original presets; and historical OFF compatibility. A favorable sound does not prove chip fidelity, and chip fidelity does not prove musical excellence.

## 1. Verified local baseline: already an FM foothold

Observed in the public repo at the snapshot:

- src/audio/dsp/osc.rs has Osc waveforms (sine, polyBLEP saw/pulse, triangle) and FmOsc, a **2-operator phase-modulation pair** with a carrier/modulator ratio and index.
- src/audio/human_music/instrument.rs has a compact Copy Patch with OscKind::Fm {ratio, index}, a shared amplitude ADSR, filter envelope, sub/unison, filter, gain and pan. **No operator graph, per-operator envelopes, multi-operator routing or chip personality.**
- src/audio/human_music/synth.rs has sample-scheduled note events, voice pools, role buses, production effects, stem/ablation controls, and an FmOsc incorporated into SynthVoice.
- In the historical SynthVoice FM path, the FM signal is summed with a separate unmodulated sine from the ordinary oscillator array. **This is a layered tone, not a pure 2-operator pair**; it may sound good, so preserve it exactly under historical selection instead of silently "fixing" it.
- The current FmOsc::alias_free_index bounds modulation with a Carson-like bandwidth heuristic. It is useful protection, but FM sidebands have no strict finite support: **do not claim mathematical alias freedom** merely from that bound.
- src/audio/dsp/env.rs already supplies deterministic ADSR and exponential-decay primitives; src/audio/time.rs supplies integer SampleTime/TempoMap; src/audio/render.rs and src/audio/perception/ provide offline rendering and observer instruments.
- gibson::audio is experimental Rust-only, and its core historically uses no mandatory audio-device dependencies. Preserve that unless explicitly approved.
- Main's historical v0.4 and all future accepted C137 controls must remain **byte-exact with GSPU disabled**. This includes score/performance/canonical fingerprints, voice continuity, R17, cover controls, patches and PCM.

**Existing known engineering constraints**: upstream issues #74 (offline rendering materializes the entire PCM) and #78 (non-contiguous RenderCtx::start behaves as forward-only; a fresh synth may fire skipped events). Audit these when designing longer renders or interactive seeking. Do not declare streaming/seek solved as part of FM merely because one FM oscillator works.

## 2. Hardware reference families: do not collapse different chips into one vague "retro" mode

| Target | Useful characteristic | First-stage emulation boundary |
|---|---|---|
| Yamaha **YM2612 / OPN2** (Genesis/Mega Drive) | 6 FM channels; 4 operators/channel; 8 operator algorithms; per-operator envelopes and feedback; shared LFO; per-channel pan | Implement **OPN2-inspired musical program** and audible controls first. Exact registers, timer behavior, channel-3 special mode, SSG-EG, DAC channel, clocking and analog/DAC artifacts require later explicit work. |
| Yamaha **YM3438 / OPN2C** | CMOS hardware relative of YM2612, with materially different output-stage character | Cross-reference and a separate authenticity fixture; do not mislabel YM3438 output as an exact YM2612 DAC clone. |
| Yamaha **YM3812 / OPL2** (AdLib/early Sound Blaster) | 9 2-operator FM channels; selectable waveforms and rhythm mode | Derive a constrained OPL2 personality from the validated FM core; distinguish chip restrictions from HD mode. |
| Yamaha **YMF262 / OPL3** (later Sound Blaster families) | Up to 18 2-operator channels; six supported paired 4-operator combinations; additional waveforms and output routing; rhythm mode | Validate real OPL3 paired-channel/algorithm semantics, waveforms and resource budget. Not interchangeable with the 8 OPN2 algorithms. |
| **HD-GSPU** (original project mode) | Optional greater polyphony, richer envelopes/automation, selectable sample-rate/oversampling, modern effects | **A new programmable instrument**, not a historical Yamaha chip or a claim of bit-exact compatibility. |

"Genesis/Sound Blaster compatibility" must be split into **inspired**, **behavioral**, and **hardware-accurate** claims:

- **Inspired**: recognizable program topology and acoustic character; modern precision and simplified controls; no register-compatibility claim.
- **Behavioral**: specified chip-personality algorithms, parameters, key scaling, envelope transitions, voice limits and routing validated against independent reference fixtures for selected regimes; claim the exact tested subset.
- **Hardware-accurate**: requires register-level/timing behavior, table quantization, clock and envelope stepping, feedback semantics, panning/DAC/output path, variant differences and hostile test vectors. This is an optional, separately gated research milestone, not a result of merely having four operators.

First target = **OPN2 four-operator inspired + testable subset of behavioral behavior**; second = **OPL3 two/four-operator inspired + tested subset**; exact-emulator route must justify its cost. OPL2 follows by its constrained subset, with actual hardware distinctions respected.

## 3. Mathematical core: typed, compiled phase-modulation graphs

For each note and operator i, choose phase in **radians** and define, for sample n:

    theta_i[n+1] = wrap_2pi(theta_i[n] + 2*pi*f_i[n]/sr)
    u_i[n] = a_i[n] * w_i(theta_i[n] + sum_(j -> i) k_ji * u_j[n - d_ji])

Here a_i[n] is the operator envelope and level; w_i is the operator's chosen waveform; k_ji is a **declared radians-per-input-unit phase-modulation coefficient**. The modulation graph describes how sound is made, not what the song means.

- Zero-delay modulation edges must form a DAG with a validated topological execution order. Cyclic/feedback edges need **declared delay/state** or a separate solved/quantized feedback model, never accidental recursion with undefined evaluation order.
- Carrier outputs are explicitly designated and mixed with a documented normalization/headroom law; a modulator does not reach the audio output unless it is also assigned a carrier role.
- Every operator has independently configurable ratio vs fixed frequency (where supported), detune, waveform, level/envelope, key scaling, velocity response and optional pitch/amplitude modulation. Add controls in response to reference/program needs, not as knobs without evidence.
- Graph compile performs resource-budget and parameter validation, resolves chip algorithms, allocates all operator/voice state, fixes summation order and emits an immutable runtime program. The audio callback only advances prepared, bounded state; no heap allocation, locks, graph search or procedural song planning.
- **Stable original VoiceEventId/source provenance** flows alongside the executed voice. Never use retimed physical onset to re-key song/source identity.
- Keep a **legacy path** for the historical Patch/FmOsc exact bits. Prefer an optional SoundPalette/compiled SoundProgram selected at the **render boundary** over expanding existing SongMap, historical Patch literals or serializations. A new program gets a separately versioned program/realization fingerprint, not a retroactive song fingerprint.

### The irreducible controls

A minimum useful OPN2-like 4-op program has: one of eight validated topology maps; operator envelopes and levels; ratios/detuning; feedback; key-on/reset and key-off behavior; key/velocity scaling; pan/channel mix; optional shared LFO. Operator algorithm labels are chip-specific **edge maps**, not generic UI ordering labels.

For OPL3, two-operator and legal channel-paired four-operator algorithms, waveforms, key-scale levels/rates, envelope modes, tremolo/vibrato and rhythm/resource mode must be **OPL-specific**. Do not implement it as "OPN2 algorithm plus different preset".

Illustrative 4-op graph (program topology, not a fixed chip algorithm):

    op4 -> op3 -> op1 ----\
                            +--> output
                     op2 --/

The four/six/eight operator ambition is **capability-driven**: 4 operators are the first hardware-aligned implementation; add 2-op OPL, 4-op OPL3, and only later higher-operator HD programs if tests and listening justify them. Six- or eight-operator generalization is optional; it must not displace the Genesis/Sound Blaster priority.

### Spectral identity and its limits

For one sinusoidal phase-modulated carrier of constant index beta (radians),

    sin(omega_c*t + beta*sin(omega_m*t))

has spectral lines at omega_c + k*omega_m with amplitudes related to Bessel J_k(beta), k in Z. This is an analytical calibration **only for that controlled regime**; nonlinear feedback, waveshaping, changing envelopes, multiple modulators, clamping and chip quantization need different references.

The current alias bound's Carson-style cutoff is a heuristic. FM produces infinitely many mathematical sidebands, and feedback/non-sinusoidal operators can widen the spectrum. Define a measured alias-energy budget on reproducible signals relative to a trustworthy oversampled/decimated reference, include high pitches and large modulation/feedback, and label any limit **bounded under the declared measurement**, never absolutely alias-free.

High-definition control modes might be Native, Oversampled(2x), Oversampled(4x), and OfflineReference with explicit decimation filters and measured CPU/latency; these are **candidate designs**, not confirmed defaults. Start with a cheap correct native path, compare against controlled oversampling, then ship only modes justified by actual numbers and listening.

## 4. Authority boundaries and categorical discipline

The source-owned chain is:

    SemanticTrace --> SongMap / observed meaning mu(song)
                  --> PerformancePlan / NarrativePlan --> accepted Score
                  --> [optional Meatsack gesture/rig execution]
                  --> [GSPU SoundProgram + synthesis] --> PCM
                  --> auditory observer + ear

These are different authority layers:

- **Song** owns form, themes, harmony, discourse, obligations and identity.
- **Performance** owns who carries what meaning, when, coordinated pocket and phrase/dynamic intent.
- **Meatsack** owns declared instrument/player resources, gesture feasibility, contacts/excitations, physical continuous controls and honest refusals; its output does not silently rewrite Score.
- **GSPU** owns oscillators/operators, envelopes, timbral mapping, rendering time, buses and PCM. An FM patch is *not* an instrument body; Role::Bass does not mean "six-string bass rig".
- **Observer/ear** measures exposure/listening outcome; a program fingerprint or valid Score cannot itself prove audibility.

Proposed non-destructive diagram: selecting a different well-formed SoundProgram for the **same accepted performance and licensed execution** changes PCM and realization receipt but not SongMap, motif lineage, carrier assignment, chord identity, metric source, or intentional silence. This is an exact **source-invariance** claim checked with identifiers, not a claim that different PCM is perceptually identical. There is no automatic commutation law from waveform equality to story recognition.

GSPU parameter changes that affect upstream **acoustic assumptions** must be explicit. C137 currently reads Patch ADSR/attack/lifetime details before final synthesis (voice, sonority, pocket, expression). New FM release tails, delayed attack, or physical controls can invalidate those symbolic checks. Supply a minimal fixed acoustic-assumption snapshot, re-check affected judgments if assumptions differ, or refuse the substitution. **No two-way invisible repair.**

A source intent may map to several excitations and one excitation to a compiled multi-operator timbre; this does not create independent new musical events. Single source onset vs several string contacts remains Meatsack's decision, never GSPU's invented strum.

## 5. Runtime and controls

- Keep the audio implementation fully optional and compile the new program ahead of device callbacks. Existing software renderer, world production, stem masking and gain staging stay authoritative unless a justified new route is introduced.
- Store operator parameters as immutable/compiled patch state; operator phases/envelopes/LFO/feedback are per-voice or appropriately shared chip state. Define reset, retrigger, legato, voice steal, key-off/tail and polyphony behavior.
- Use sample-accurate integer event scheduling. For control trajectories, use timestamped bounded control events or block-safe smoothing with a declared interpolation rule. No independent note-on jitter and no timer/thread-based audio clock.
- Support honest chip **resource constraints** (e.g., OPN2 six channels vs HD free polyphony) and typed refusal/steal outcomes. Do not label 64 HD voices "authentic Genesis six-channel".
- Preset/program authoring: original, small, versioned text/typed Rust definitions, no ROM dumps, presets from copied games, VGM packs or proprietary soundbanks. One seed/song can be rendered under OPN2-style, OPL3-style, HD, and historical-world palettes as controlled A/B.
- Sound design goals: Genesis-style moving bass, brass, organ, chip drums and FM keys; OPL-era DOS-style brass, metallic percussion, plucks and leads; at least one complex evolving timbre that the existing fixed 2-op Patch cannot express. **Ear gate** decides success after structural and DSP checks.
- Explore diagnostic UI later using libgibson's scientific/temporal display machinery: operator topology, envelope traces, spectrum, high-frequency alias estimate, CPU/voice occupancy, FM/clean/retro A/B. Audio timing is never driven by terminal redraw.

## 6. Research/implementation ladder (after C137 + Meatsack acceptance)

**G0 — Reconcile + freeze.** Refresh accepted C137 and Meatsack heads; record main, instructions, tests, active PRs, source API, and relevant historical PCM/fingerprint goldens. Create a new implementation branch **from the explicitly accepted integration base**, not from this v0.4-era research commit. Leave this branch as design provenance. Compare existing FmOsc output with a separate pure 2-op reference; characterize intentional layered sine, index clamp and note/tail behavior before touching them.

**G1 — Pure compiled FM core, only.** Implement tested pure 2-op and 4-op program graphs behind an OFF-by-default route. Distinguish carrier vs modulator; independent envelopes and explicit modulation routing; feedback with defined delay. Analytical and negative tests first. No new default patches, score changes or sample playback.

**G2 — Genesis OPN2 personality.** Implement the eight actual 4-operator algorithm maps and measured compatible envelope/key/feedback/pan subset. Offer exactly named fidelity levels (Inspired, VerifiedSubset). Add original demonstration presets and source-equal A/B WAVs. For strict OPN2 features (special ch3, SSG-EG, CSM, timer/DAC/register/analog path) create a coverage checklist; implement only if evidence and project use earn the complexity. Report excluded behaviors explicitly.

**G3 — Sound Blaster OPL personality.** Implement OPL2 two-operator constraints, OPL3 two/four-operator pairings, waveform/operator properties, rhythm resource model and routing as actually supported, rather than naively cloning OPN2. Use independent OPL reference fixtures and original musical presets. Hardware-accuracy claim separately gated.

**G4 — HD render and program ergonomics.** Conditional oversampling/band limiting, calibrated interpolation, bounded CPU, stable program serialization/fingerprints, sensible APIs for consumers, stereo/production integration and rich diagnostics. Keep chip-compatible behaviors reproducible under declared conditions. Fix/contain #74/#78 only in separately proven changes when necessary.

**G5 — C137 + Meatsack vertical integration.** Same generated accepted SongMap and realizer performance under old synth, OPN2, OPL3 and optional HD palettes; no source mutation. Map a limited set of Meatsack effort/attack/contact events to **documented** FM parameters (e.g. modulation index envelope, operator level, noise/transient mix). Test whether one coordinated gesture produces correlated timbre/attack changes, instead of adding random changes. Physical realism not claimed from the FM engine alone.

**G6 — Only if earned:** precise chip-emulation backend(s), HD 6/8-op generalization, optional sample-voice/SNES-style adapter, more sophisticated operator editor. No SNES-HD/sample milestone is scheduled; no samples are needed or expected. OPN2 DAC support is out of the first FM milestone and must not sneak a sample requirement in.

Each gated round should produce a standalone reversible commit, exact-head hosted CI/test receipts, frozen render hashes (with platform/toolchain scope), controlled WAV/stems and a human listening verdict when sound changes. Never merge, release or change historical defaults without explicit maintainer authorization.

## 7. Acceptance and falsification contract

A future implementation **must** defeat negative/mutation controls, not merely sound exciting.

**Mathematical/DSP probes:** zero index = clean carrier; no ghost sine/output from a modulator-only route; pure 2-op sinusoidal sidebands track the Bessel reference in a controlled low-alias regime; graph topological ordering is stable; a cycle without legal feedback is refused; feedback is finite/bounded/seeded; independent envelopes really change timbre under a fixed musical note; key-off/legato/voice stealing do not fabricate extra attacks. High-frequency aliasing and clipping must be measured, not asserted absent.

**Chip dialect probes:** OPN2's 8 algorithms produce distinguishable, correct topologies; OPL2 restrictions and OPL3 pairings/resource modes can reject wrong configurations; strict six-channel OPN2 cannot silently produce a seventh voice; an OPL pairing consumes the correct resources. Compare against two independently implemented/reference routes when feasible; shared library provenance is not independent testimony. Disclose any table/clock/DAC mismatches.

**Musical probes:** swap only the palette for identical generated score/gesture objects; canonical song/performance/identity/cover provenance does not move; narrative carrier withhold remains silent; exposed carrier may change acoustic audibility and gets its **own** downstream witness; release-tail changes must not invalidate upstream acoustic assumptions silently. Mutating program data cannot mutate SongMap.

**Engineering probes:** historical GSPU-OFF path bit-identical; deterministic same-head/same-platform/same-settings and across block sizes; no callbacks allocate or lock; sample-rate changes do not create unstable pitches, stuck notes or NaNs; FM modulation/feedback bounded in specified finite operating domains; long renders have explicit memory budgets; no unsupported seeks masquerading as correct random access. Test malformed graphs, extremes, quick retriggers, simultaneous MIDI-equivalent events, quiet/fast notes and silence.

**Listening probes:** original program A/B (no reference soundtrack copying), level-matched, same form/pocket/notes/gain topology; inspect stems and matched dry controls. Accept "better", "worse", "stylistic alternative", and "ambiguous" as separate verdicts. A Sound Blaster timbre can be intentionally gritty while still being numerically correct. Machine metrics may reject defects but never declare aesthetic success.

**No-go on first contact** if any GSPU-OFF PCM/fingerprint drifts; if chip claims exceed tested features; if excessive aliasing is hidden under new presets; if Meatsack or a new FM envelope silently rewrites song/physical contracts; or if it turns the terminal/audio callback into a graph-planning engine.

## 8. Source, licensing and provenance

### Verified repo paths at the design snapshot
- https://github.com/femboy2112/libgibson/blob/80706c91b88e59495b131bfd8ab23aa3ed013414/src/audio/dsp/osc.rs
- https://github.com/femboy2112/libgibson/blob/80706c91b88e59495b131bfd8ab23aa3ed013414/src/audio/human_music/instrument.rs
- https://github.com/femboy2112/libgibson/blob/80706c91b88e59495b131bfd8ab23aa3ed013414/src/audio/human_music/synth.rs
- https://github.com/femboy2112/libgibson/blob/80706c91b88e59495b131bfd8ab23aa3ed013414/src/audio/time.rs
- https://github.com/femboy2112/libgibson/issues/74 and https://github.com/femboy2112/libgibson/issues/78
- C137 boundary (moving): https://github.com/femboy2112/libgibson/blob/feat/v0.5-humanmusic-beefup/docs/HUMAN_MUSIC_MEATSACK_PROJECTION_BOUNDARY.md
- Independent physical-research design: https://github.com/femboy2112/libgibson/blob/research/humanmusic-meatsack-projection/docs/HUMAN_MUSIC_MEATSACK_PROJECTION.md

### Hardware references to verify during implementation
- YM2612/OPN2 technical notes (third-party compiled reference, **not** a primary Yamaha datasheet): https://github.com/user-none/emmd/blob/main/docs/ym2612_reference.md
- Nuked OPN2/YM3438, a highly accurate external implementation/reference: https://github.com/nukeykt/Nuked-OPN2
- Nuked OPL3/YMF262, an external implementation/reference: https://github.com/nukeykt/Nuked-OPL3
- YMF262 driver/reference comments describing OPL3 capabilities: https://github.com/gtaylormb/opl3_fpga/blob/master/docs/ymf262.c

**Licensing caution:** libgibson is MIT OR Apache-2.0; the above Nuked chip emulation projects use LGPL-family licensing. Reading behavior/benchmarking is not permission to copy their source into libgibson. If any library integration or code adaptation is considered, separately audit license compatibility, distribution obligations, attribution and build linkage, and obtain approval. Prefer original Rust implementation plus lawful behavioral test vectors; do not treat a copied emulator as independent confirmation of itself. No proprietary patches, ROM, copyrighted soundtrack or sample pack should be committed.

**Claim ledger at creation:** existing 2-op FM, patch and DSP substrate = OBSERVED source. GSPU compiled FM graph, Genesis/OPL dialects, HD anti-alias modes, and Meatsack audio mappings = DESIGN / UNVERIFIED. Exact chip fidelity, superior listener preference and physical instrument realism = NOT CLAIMED.

## 9. The future handoff

Read [HUMAN_MUSIC_GSPU_FM_HANDOFF.md](HUMAN_MUSIC_GSPU_FM_HANDOFF.md) before integrating. The decisive point is to **wait until C137/Meatsack boundaries are accepted**, then adapt this design to the actual merged code — not forcibly transplant a v0.4 research branch.

**Motto:** the song determines **what**; the band determines **who/when**; the body determines **what is physically executable**; the FM program determines **how the excitation sounds**. Every cut has a separate witness. The instruments can change without making the band forget its song.
