# MeatsackProjection — deferred implementation handoff to Claude

> **Research branch only:** `research/humanmusic-meatsack-projection`. This branch intentionally forks from **main @ 80706c91b88e59495b131bfd8ab23aa3ed013414**, not C137's active branch. It contains architecture, validation and this handoff **with no implementation**. Do **not** merge it into main or C137 without the maintainer's explicit approval.
>
> Deferred launch condition: the maintainer and Claude have completed/accepted the C137 semantic band-story round. There is no background task or automatic merger attached to this document.

## 0. Fast cold start

1. Read [HUMAN_MUSIC_MEATSACK_PROJECTION.md](HUMAN_MUSIC_MEATSACK_PROJECTION.md) **in full**.
2. Read [MEATSACK_PROJECTION_VALIDATION.md](MEATSACK_PROJECTION_VALIDATION.md) **in full**.
3. Inspect the **current** repository head, C137 final accepted SHA and its tests; do not assume the source layout stayed at the v0.4 design-parent. At design time Claude was actively landing GEN-STORY-2 (branch `feat/v0.5-humanmusic-beefup`; latest observed `0c7314fc176f5338336c0a2c2e82051f6c1d23d7` — refresh before use).
4. Examine latest C137 narrative/meaning/score/feel/voice-continuity/PCM authority boundaries. The source notes below are **historical anchors**, not immutable line numbers.
5. Freeze a new, independent development branch **from the accepted C137 commit**, for example `feat/v0.5-meatsack-projection`; keep this original design branch unmodified as the design provenance. Copy or cherry-pick reviewed docs if desired, not the entire history blindly.
6. Work toward first-wave falsifiers (drummer + fretted strings) before planning keyboards/winds or rewriting acoustic synthesis.

**C137 work owns meaning and musical agency; Meatsack owns constrained embodiment of the result.** Do not let either secretly rewrite the other.

## 1. Existing source interfaces to inspect when restarting

| Source area at the design parent | Actual purpose | Meatsack integration hazard |
| --- | --- | --- |
| `src/audio/human_music/semantic.rs`, `song.rs`, `meaning.rs`, `discourse.rs` | Source/story, actual song meaning, phrase goals | Physical realization may not synthesize missing song meaning |
| `src/audio/human_music/policy.rs` | Opt-in realization policies; C137 adds narrative/lead controls | Body policy must default OFF and not contaminate archived fingerprints |
| `src/audio/human_music/performance.rs`, `interaction.rs`, `action.rs`, `material.rs` | Planned obligations, carrier actions/material handoffs | Use actual planned gesture ownership; do not invent second action/call ontology |
| `src/audio/human_music/score.rs` | `Note`, `DrumHit`, provenance, `StrokeOrigin` | Notes and hits currently do not identify guitar strings/limbs; moving onset in place changes source identity |
| `src/audio/human_music/rhythm.rs` | `MetricPosition -> GrooveTransport -> FeelTransport -> PerformedPosition` | Avoid second phrase lean and extra swing in body layer |
| `src/audio/human_music/voice.rs` | Explicit source-owned voice continuation; role not string identity | Preserve continuation through physical timing/voice allocation |
| `src/audio/human_music/instrument.rs` / `world.rs` | Synth patch/sonic world | Role/patch is not a physical performer; don't infer instrument rig from timbre |
| `src/audio/human_music/synth.rs` | Sample-accurate event scheduling, voice pools, acoustic patch realization | End-of-chain only; not a feasibility planner. Optional new event/control intake may be needed |
| `src/audio/time.rs` / `render.rs` | Integer sample clock, offline/device rendering | Physical time in seconds must translate to sample indices at one controlled boundary |
| `src/audio/perception/` (C137) | Downstream auditory diagnostics | Don't use observer as source authority or quality optimizer |

Review historical stability notes: `docs/HUMAN_MUSIC_ARCHITECTURE.md`, `docs/HUMAN_MUSIC_POCKET.md`, `docs/HUMAN_MUSIC_COVER.md` and the *actual current* C137 research/acceptance documents.

## 2. Proposed implementation strategy (not yet API commitment)

**Preferred first seam:** a physical-gesture projection operating from **explicitly grouped intended gestures** with a source-event mapping, before final synth event scheduling. Keep the authored Score separate and immutable for checks/history. Let the first experimental execution be an optional sidecar.

An initial public-facing mode might be **none / sequencer / declared rig** with a strong OFF default. Whether its switch lives beside the synth constructor or a dedicated physical execution entrypoint must be decided by current code reality. Do **not** automatically put everything into `PerformanceProfile`: a physical performer configuration is distinct from musical intention and `MusicWorld`.

Possible private module layout after audit (not prescribed):

```text
src/audio/human_music/physical/
    mod.rs          boundary + docs
    intent.rs       explicit grouped gestures / source IDs
    resource.rs     finite effectors / targets / recoveries
    timeline.rs     time conversion + source-to-excitation schedule
    drums.rs        drumkit rig / sticking/pedals
    fretted.rs      tunable strings / fret hand / picking hand
    receipt.rs      independent witness / refusals
```

Do not create all these files until their contracts are earned. A smaller module/experiment directory is preferred if sufficient.

### A five-stage vertical slice

**(i) Identity baseline:** add a no-op sequencer passthrough with stable gesture-source mapping. Legacy `HumanMusicSynth::new` and default composition/performance paths unchanged and byte-identical. Verify early that physically shifted events have stable original IDs and cannot break `VoiceContinuation`.

**(ii) One gesture through a sample-accurate output:** a synthetic original chord with an explicit `Strum` intent. Realize down and up sweeps as ordered contact events from one gesture, output via the existing patch as a timing demonstration. Repeated runs and different offline block sizes must agree.

**(iii) Resource-constrained drummer:** configurable two hands/two feet default, target transition times, foot pedals, normal groove, flam and explicitly impossible multi-hand hit. Return **typed infeasible/unsupported** rather than an illegal success or secretly dropping required notes.

**(iv) Guitar fingering/strum constraints:** available fingers, barre, optional thumb-over-neck, string/fret choices, pick vs fingers, travel across strings and transitions. Model a **low-dimensional constrained mapping**, not an entire hand. Compare same chord under pick down/up/fingerstyle.

**(v) Then evaluate the abstraction:** only generalize the finite effector/resource formalism after (iii) and (iv) both work; only add continuous acoustic control support once the output path can actually render the control effect rather than merely record an inert event.

After each stage run tests, controlled WAV/stem A/B and a specific failure audit; pause for maintainer listening.

## 3. High-level ergonomic goal for final users

A program author should not need to specify the angle of each finger or limb coordinate. It should feel roughly like:

```rust
// PSEUDOCODE ONLY — no such API exists yet.
let song = existing_song_and_performance;
let physical = PerformerRig::drumkit(default_human_kit)
    .with_handedness(...)
    .with_techniques(...);

let execution = project_gestures(&song.intent, &physical, &tempo, seed)?;
let audio = render_with_physical_execution(&song.score, &execution, &world)?;
```

Realism or convenience should come from **selecting a performer rig and technique**, not manually dragging every note off-grid. Expert callers may specify their own target graph, finger configurations, breath model or gesture allocation: **no walled garden**.

Expose the *logical* distinction among (a) physical rig, (b) sonic patch and (c) musical role. A style switch among human guitarist, human pianist and electronic sequencer should not require rewriting SongMap or C137 narrative.

## 4. Error/unknown semantics

The result must distinguish:

- **Realized:** all required physical actions feasible, source identity preserved.
- **RealizedWithDeclaredVariation:** within licensed beat/attack/physical-control tolerances.
- **Infeasible:** provable resource/motion/technique contradiction *under declared rig*.
- **Unsupported:** requested technique or physical control outside implemented model.
- **Unknown:** insufficient source grouping/rig data to decide; NOT evidence that the gesture is free, and NOT a license to invent missing hands.
- **OptionalOmitted:** a specific authored optional ornament removed by an explicitly licensed policy with a reason.

Prefer small typed variants only as implementation evidence demands. A coherent failure is better than silently repairing composition by changing the chord.

No plan-to-itself confirming receipt. An adversarial independent observer should inspect actual emitted contacts, controls and source mappings.

## 5. Concurrency and CPU discipline

Use one persistent principal Claude agent, retaining causal/debug state across rounds. Delegate only bounded, nonoverlapping work:

- **Drum adversary:** independent feasibility cases/rebound/pedal controls, ideally test code against a frozen primitive interface.
- **Guitar adversary:** string/fretting/picking constraints with negative controls.
- **Independent receipt adversary:** verify source identity and physical resource claims without trusting planner-provided success labels.

Freeze the shared intent/receipt contract before subagents edit; don't allow three separate `GestureId` definitions. Prefer forking warm context; fresh agents only for independent tests. Keep candidate set/workspace bounded. Simple deterministic greedy plus bounded backtracking/DP is enough if proved sufficient; NO general-purpose anatomical solver.

Physical state can be causal/persistent without using live audio feedback. A deterministic finite lookahead of **already authored** upcoming gestures is allowed for planning; never retroactively change already emitted execution because a later quality heuristic disliked it. Rejected configuration/technique stays explicit.

## 6. C137 coordination gate

**Wait until the current C137 story round is frozen.** The just-landed `NarrativePolicy`/meaning-carrier handoff is a *reason* for Meatsack, not a subsystem this research branch should race to edit. Future physical projection must satisfy a flagship:

```text
one SongMap / one motif / one narrative identity
    first carried by lead
    later carried by bass or keys
    same thematic provenance
    multiple physical rigs and gestures
    independent physical receipt for each
```

Mutating performer changes articulation/physical feasibility; it does **not** turn Learn into Payoff or undo a withheld gesture. Instrumentally impossible carrier assignments must be surfaced to the planner before accepting final execution; no after-the-fact silent note erasure.

If current C137 implementation exposes no grouped gestures, **that is the first seam to derive/fix**. Do not heuristically infer every simultaneous set as guitar chord.

## 7. First commits and checkpoints for future Claude

Suggested causal work (after explicit start authorization):

1. `docs(audio): reconcile MeatsackProjection blueprint against accepted C137 head`
2. `test(audio): red identity/gesture grouping and immutable song boundary`
3. `feat(audio): opt-in source-grouped physical execution passthrough`
4. `test(audio): red drum resources and infeasible event witnesses`
5. `feat(audio): deterministic drum limb allocator and controls`
6. `test(audio): red fretted-string fingering/strum variants`
7. `feat(audio): first fretted-string Meatsack realization`
8. `test(audio): independent physical execution receipt and hostile mutations`
9. `docs(audio): evidence, A/B listening, rejected constraints, outstanding models`

Use exact-head hosted CI and human listening after each important rung. Preserve the v0.4 and C137 control paths. Do not merge/tag/release until explicit maintainer approval.

## 8. Future acceptance bundle

At minimum Claude must report:

- START: accepted C137 SHA, design branch SHA, new experiment branch SHA.
- INTENDED: source gesture/score identities, role, carrier and timing.
- EXECUTED: assigned physical rig/effectors, sorted attacks/controls, offsets in seconds/samples, resource feasibility.
- CONTROLS: electronic, matched independent jitter, physical time-only, physical full, zero physical variation, historical no-op.
- HARD GATES: no phantom limbs, no chord/identity loss, correct source IDs, deterministic bounded scheduling, no double feel transport, no silent optional omission, R17/covers/fingerprints exact with OFF.
- AUDIO: full mix, relevant stems, anonymous A/B, listener result and limitations.
- COST: CPU/memory bound and realtime/offline consistency.
- HONEST RESULT: accepted physical mechanism, merely valid model, rejected idea, or unknown — never "human by construction".

## 9. Text to paste into a later Claude session

> **Do not execute this automatically when discovered. Use only after the maintainer tells you C137 is finished and authorizes Meatsack implementation.**
>
> You are continuing HumanMusic after the accepted Rick-C137 narrative round. Your next assignment is an independent, opt-in **MeatsackProjection** physical-gesture vertical slice. Fetch `femboy2112/libgibson` and read the three documents on `research/humanmusic-meatsack-projection`; verify the docs-only design branch and final C137 head. Make a new feature branch FROM final C137, never from the design branch by default. Do a source audit first, repair stale architecture assumptions, preserve ALL historical archived fingerprints/PCM, and freeze the minimal intent/excitation/receipt contracts before coding instrument models. Red-first drumkit (finite hands/feet/pedals/targets/recovery, flams, impossible hits) and guitar (strings, fretting grip/barre/optional thumb, down/up pick sweeps and fingerpicking). Never model an entire human skeleton. Use existing sound patches as a *timing* control, not proof of realistic guitar acoustics. Separate shared musical FeelTransport from local physical contact offsets and continuous controls. Variations must be correlated, bounded, attributable, and allowed to be zero; never independent jitter or a feedback loop that rewrites the song. Keep song, carrier and source identities fixed; report infeasible/unknown instead of conjuring limbs or silently dropping required strikes. Produce offline deterministic controls, independent receipts, negative mutants, stems, and blind listening material. Only generalize the second-wave keyboard/wind model if drum/guitar controls earn a shared interface. Commit in causal chunks, push a draft PR, check exact-head CI, do NOT merge/tag/release. Conclude with what is **actually** physically modeled, what is **only** an abstraction, what the listener heard, and what is still WIP.

**Standing law:** The musical plan says *why*; the performer rig determines *how*. Physical variation breathes **inside** the song's constraints; it never becomes a second composer.
