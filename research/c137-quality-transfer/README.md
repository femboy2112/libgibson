# C137 quality transfer: recover the music before generalizing the generator

**Audit snapshot:** `229b397b46a800342a58ac5c38c8bbad3d2cfb87`, `feat/v0.5-humanmusic-beefup`, 2026-10-09. **Audit branch:** `aletheia/c137-quality-transfer-audit-2026-10-09`. No runtime, default, dependency, or active-branch modifications. This is a source audit, executable isolated counterexamples, and an implementation handoff; NOT a claim that the sound is fixed.

**Question:** Why does the handwritten Rick/C137 reference sound substantially better than the infrastructure-generated rendition, and what should be repaired next?

**Working diagnosis:** there is one definite feature-wiring omission, followed by several information-losing or semantically different operations between the accepted reference and the generated route. Changing a short melodic seed cannot repair all of them. The next round should demonstrate faithful lowering of one known-good musical phrase through shared infrastructure, then generalize that construction. More diagnostic predicates alone do not produce better musical content.

## 1. Baseline and limits

The latest inspected commits already fixed the old rustdoc private-link failure (`e9e9e1f`), added an ordered bass-germ witness (`69673561`), introduced `ThemeSeed` / `compose_with_germ` and finale dials (`8d80f6be`), and altered the diagnostic seed's shape (`229b397b`). Do not redo those as new work. GitHub reported CI run **38004209581** successful for **229b397b** during this audit. The older hardening ledger still says C137-B is not started; code/commit evidence supersedes that stale status.

The last commit records the listening verdict **“better, but still not there”** after the seeded melody gained a different contour/rhythm. That is useful evidence, not proof that only melodic development is defective.

I inspected the repository through the GitHub connector. This environment has Python execution but no available Rust toolchain; a direct clone also failed because container DNS could not resolve GitHub. **No local Cargo tests, Rust execution, end-to-end audio rendering, or independent listening took place here.** Nine isolated Python counterexample tests were executed successfully; a missing-source negative control correctly refused to claim source verification. See `probe.py` and `PROBE_RESULTS.md`. Hosted CI is a separate evidence source, not a local run.

Reference sources are the actual handwritten `examples/rick_probe.rs` and its narrative extension `examples/band_story_probe.rs`; the generated entry is `examples/rick_story.rs`. Confirm the maintainer's exact accepted file/arguments/hash before choosing the final acoustic reference. Keep both handwritten references when their accepted properties differ.

## 2. Findings, in repair order

### F1 — The current STORY entry does not enable the earlier lead-life improvements

At this snapshot:

- `PerformanceProfile::WRITTEN` sets development, spacing, and dynamics false.
- `POCKET` and `BAND` inherit those values; the BAND builder chain does not enable lead life.
- `rick_story.rs` constructs BAND and `BAND.with_narrative(Ensemble)`. It never calls `with_lead_life`.
- `rick_gen3.rs` explicitly does call `with_lead_life`, including the `dev_dyn` arm.

Thus the present STORY audition is **not the accumulated combination of the accepted lead-development/dynamics and narrative work**. Narrative and lead life remain separate experiments. This is disclosed by source inspection; the acoustic effect of combining them is still unverified.

**First repair experiment:** preserve every default and current control. Add a named, opt-in audition arm equivalent to:

```rust
PerformanceProfile::BAND
    .with_lead_life(LeadLifePolicy {
        development: true,
        spacing: false,
        dynamics: true,
    })
    .with_narrative(NarrativePolicy::Ensemble)
```

Run all four cells of narrative OFF/ON × lead development+dynamics OFF/ON on the SAME song, chart, world, seed and render configuration. Spacing is a separate experiment, not automatically enabled. Print the *effective* full profile and assert it reaches `PerformancePlan`. A prior ear verdict on `StablePropulsion` does not prove that the combined profile works with the current MeaningDirected/DeflectedLift song.

### F2 — The selected diagnostic input is not the accepted handwritten composition

The handwritten controls have explicit timed phrases, distinct verse/hook/bridge material, section-specific harmonic progressions, accents, articulation, accompaniment, carriage, and a 56-bar form. The current `ThemeSeed` contains only `thesis: Motif` and `answer: Motif`. Its chart is searched before the seed is considered; its form comes from the existing trace-derived plan.

The seed is also a new short diagnostic tune, not the actual accepted VERSE/HOOK material. Consequently “seeded still sounds worse” does NOT isolate chooser versus developer. Several sources and policies differ simultaneously. The example's comments overstate this experimental identification.

**Correction:** retain native accepted teacher audio; then build a matched-context comparison with named source variables. Do not normalize away tempo/arrangement and claim to have preserved the original listening control. Each causal comparison must explicitly hold its unrelated factors fixed.

### F3 — `Motif.rhythm` cannot independently preserve authored onset spacing and note gates

`Motif` contains pitch coordinates and one rhythm vector, not separate source onsets, gates and accents. `Motif::render` advances the onset by that same duration. `melodic_events` derives gates/rests/accents from the motif plus `LineStyle`; it cannot receive the complete arbitrary authored event data through `ThemeSeed` alone.

The exact VERSE fixture in `band_story_probe.rs` has **24 notes, 2.5 beats of internal silence across three gaps, and 11 distinct source velocities**. Encoding its gate durations as `Motif.rhythm` shifts later onsets by up to 2.5 beats. Encoding inter-onset spacings preserves starts but gives those same 2.5 beats to sounding gates, before subsequent style heuristics. The phrase ends its last sound at beat 15.5 in a 16-beat phrase; trailing silence also needs a declared span.

This is a limitation of that input representation, not of all LibGibson. **`MaterialEvent` already has separate `onset`, `dur`, `accent`, `step`, and `InteractionMaterial` carries a pitch basis and provenance.** Reuse its event vocabulary; expose the smallest source-level timed-phrase representation/adapter with an explicit phrase span. Do not write a second score engine or replace all legacy Motifs.

A repaired timed representation must preserve independent onsets, note-off times, accents, pitch units, rests and span through the relevant lowering stages. It must validate lengths, finite values, positive durations, domains and pitch-basis conversions. Bypassing taste preferences is not permission to bypass malformed-input validation.

### F4 — Generated carriage is not the operation the handwritten reference uses

The handwritten `carry` copies a whole phrase to each declared carrier under an explicit octave transposition and gain. `bar_plan` suppresses the ordinary backing of a role while that role carries the phrase; pads supply support as required. All named final carriers can sound the same source gesture.

The generated mechanism is different:

- `interaction.rs` routes narrative through opportunities and a **single optional response per call**. A large preference for named carriers does not satisfy an AND obligation for two carriers.
- The response can still become probabilistic `Transform::Silence`; narrative demand does not disable that discretionary silence decision.
- `Transform::Quote` deliberately takes only the last **four events**, then clips to available room. This is legitimate short-response semantics, but NOT whole-phrase carriage. For the 24-note VERSE, at most 4/24 events survive that crop even before room clipping.
- A withheld lead site is skipped before its lead statement/material is instantiated. The thematic source must exist independently of whether the lead emits it.
- The bass narrative pass re-pitches its already-generated notes using `bank.identity.degrees[i % len]` and `germ_tone_index`; it does not read the germ's rhythm, does not replace the backing with that phrase, and retains pre-substitution role tags.

**Smallest repair:** compile explicitly scheduled narrative material for EVERY required carrier before discretionary response selection, ensemble reservation and budget allocation. Source the phrase from its thematic site even when no lead plays it. Derive per-role material views with declared octave/revoicing/rhythm licenses. Let carrier duties suppress or replace conflicting ordinary backing; retain legitimate harmonic support. Keep old opportunistic Quote/Answer and the old bass post-pass as historical controls, not as the new implementation of mandatory carriage.

Prefer a small opt-in placement/reservation sidecar using existing MaterialId, source events, stage and occupancy mechanisms. A new universal performance ontology is not justified. Do not merely change `Option<Response>` to a vector globally: mandatory source statements and discretionary conversational replies are different concepts.

### F5 — The newer ordered witness verifies a lossy mapping, not the intended musical identity

Claude correctly improved membership-only checking with `germ_contour_shortfalls`. That is progress. But both emitter and witness still refer to modular chord-tone selection. Matching the same lossy rule does not prove original interval/rhythm preservation.

An isolated tetrad example is decisive: the latest diagnostic seed begins degrees `[0,4,7,6,4,4]`; modulo four selects `[0,0,3,2,0,0]`. On chord tones `[0,4,7,11]`, the result is `[0,0,11,7,0,0]`. Its opening leap has become a repeated pitch. This is a counterexample to general contour preservation, not a claim that every actual generated bar has this exact chord.

Similarly, `[0,1,2,3]` and `[0,5,2,7]` have different interval sequences but identical ordered images modulo four. And a per-carrier positive note-count delta is NOT necessary for a successful carry: a role can replace four backing notes with four actual thematic notes.

**Repair the contract, not just the score:** preserve separate witnesses for required role coverage, source-event identity, ordered intervals in the DECLARED transported pitch basis, onsets/gates/accent relationships, licensed variations, and unsupported/missing evidence. No eligible notes or missing source must not return an indistinguishable clean pass. The checker should use the source contract and independent representation logic, not merely the generator's helper again. Intentional chord-tone adaptation may be useful, but name it adaptation and measure its loss; do not call it literal theme transfer.

### F6 — The current composer structurally differs from the reference's verse/chorus/bridge organization

`compose_meaning_seeded` retains the trace-derived form and independently searched chart. `schedule` builds `bank.identity = thesis`, **`bank.hook = thesis`**, uses the thesis at Payoff, uses the answer only at Answer, and drops sites that do not fit. Statement scheduling packs material against an eight-beat boundary. These can make lawful songs, but they do not reproduce the accepted reference's distinct four-bar VERSE and HOOK, recurring section-specific progressions, D-minor bridge, and return.

The `--chart=travel` experiment mutates the chart after composition; it is not a matched transfer of the reference's harmonic form and the previous selection/fit report no longer describes the final chart. The performer does re-observe the final song; the issue is experimental attribution and composition coherence, not necessarily a stale performance observation.

**Next composer work, after faithful lowering:** allow a bounded explicit source plan for form, distinct thematic materials/hook, chart and cast, preferably using existing SongMap/CompositionPlan structures. Compare that with the generated plan one layer at a time. A long source phrase that does not fit must be reported or split under a declared musical transformation, not silently deleted and later called successfully transferred. Do not change global grammar defaults to force one reference through.

### F7 — The audition harness is too weak to explain failures

`rick_story` uses `perform_with_profile`, not the full `perform_checked` gate, and its report mostly contains fingerprints/counts/peak/RMS. The function does have policy checks and rehearsal; it is not entirely unchecked, but it does not establish every general or narrative law. The output names omit most experimental settings. `--germ=seed` forces MeaningDirected while the summary can still print the separately requested composer. Seed 2112 is hard-coded.

Add a manifest: exact git SHA, effective composer/grammar/profile, trace, complete germ/event payload, chart before/after any edit, world and tempo, seed, rendering/observation/lifetime controls, file hashes and actual acceptance state. Refuse unknown CLI values instead of silently substituting defaults. Keep rejected candidate audio available with failure receipts; do not silently erase the failed arm or claim accepted output because it rendered finite PCM. Distinguish RAW TEACHER, SOURCE-CHECKED, NARRATIVE-CHECKED and HUMAN-ACCEPTED.

## 3. The load-bearing mathematical boundary

Let A be an explicitly authored musical source containing timed phrase events, harmonic/form context and carrier duties. Let L map that source into the shared infrastructure. Let R be the realization pipeline, and let Pi_Gamma recover the declared musical observables under a stated transformation license Gamma.

The finite acceptance law for the calibration class is:

`Pi_Gamma(R(L(A))) = A`.

For an identity transport, compare event pitches, metric starts, gates, velocities, role duties and phrase boundaries directly; for a licensed octave/rhythm transform, compare only after applying its declared inverse/normalization. Keep both raw and adjusted views. This is a source-preservation law, **not** a perceptual theorem. Same source observables do not alone prove identical PCM; render metadata, voice lifetime, order and DSP settings must also be pinned. Same PCM does not certify an unseen song's aesthetic quality.

A direct Score clone is a baseline, not proof of expressive infrastructure. A permanently pinned cover is a useful representational control, not a generative solution. The real checkpoint is to reconstruct one reference phrase through reusable source/placement APIs, then generate genuinely new phrases through the SAME APIs and validate them on fresh holdouts.

## 4. Concrete recovery ladder

### P0 — Wire and measure the present system before changing architecture

Add the explicit NARRATIVE x LEAD-LIFE four-cell audition described in F1; leave WRITTEN/BAND and all existing examples' defaults unchanged. Add a seed/config/receipt manifest and coherent output directories. Confirm effective policies at execution. Use the currently accepted source trace unchanged for this test. Deliver the actual four WAVs plus consistent lead/band stems. Do not assume this one fix closes the larger problem.

### P1 — Freeze and export a real teacher, including its missing source metadata

Pin the maintainer-accepted `rick_probe` and/or `band_story_probe` invocation. Extract one four-bar verse, one contrasting hook, and a multi-carrier/withhold excerpt from the source builders. Capture actual notes, independent onsets/gates/velocities, chart, phrase span, cast, backing suppression and render configuration.

The handwritten Score builders bypass much semantic provenance and pitch-function classification. Do not forge IDs or mark every note ChordTone to make a checker pass. Reconstruct genuine source context from the builder; report unsupported assumptions when needed. The teacher's fixed pitches and explicit 0.022-beat lean also differ from the generator's scale interpretation and pulse path. Preserve the original sound and declare any metric normalization so timing is never applied twice.

### P2 — Implement the minimal lossless timed-phrase lowering

Reuse MaterialEvent's timed vocabulary and current source IDs, with a phrase span and a validated source interface. Prove teacher timing/contour/accent preservation before allowing stylistic alteration. Instrument after source scheduling, material realization, harmony/pitch selection, support release, expression, rehearsal, and sample scheduling; report the FIRST divergence, not just the final WAV. Fix that layer only. Keep the reference/control code out of core source.

### P3 — Implement real mandatory carrier placements

Start with one phrase, fixed harmony and two required carriers. Instantiate source material independently of lead audibility. Reserve all required carriers before optional responses and backing generation. Source-linked copies must preserve the declared phrase (or report a licensed adaptation) while ordinary backing is replaced and other roles preserve support. Required duties cannot silently be removed by rehearsal to obtain a clean receipt; infeasibility is a typed outcome with inspectable candidate audio.

Add tests for lead withholding with keys still stating the source; true lead+keys+bass same-source payoff; same-count replacement; missing carrier; missing source; reordered notes; wrong rests; drifted gates; wrong pitch basis; copied provenance on unrelated notes; and double-applied pulse. Avoid random onset noise.

### P4 — Isolate composition from accompaniment using complete controlled contrasts

Within a COMPATIBLE fixed context, build all four cells of teacher/generated thematic material x teacher/generated accompaniment. If a hybrid is harmonically incompatible, label it invalid, not a negative musical verdict. Keep native teacher audio separate from these matched experiments. This distinguishes a weak chooser from a weak phrase realization or accompaniment architecture.

Then admit distinct verse/hook/bridge and section harmonic plans through the shared source model. Generalize the relationships, not the reference's absolute note arrays. At this point investigate joint phrase-chart choice: the existing chart-first/short-germ search is a limited prior, not a universal music-composition domain. Do not widen the search space before proving the compiler can retain what it chooses.

### P5 — Fresh generated holdouts, then the v0.5 film

After lossless source transfer and genuine carrier planning work, test fresh seeds, pitch regions, worlds, tempos, phrase lengths, grammars and cover constraints. Give each learned generative operation at least one unseen composition test and a mutation it must reject. Human listening remains the gate for musical improvement.

Only then feed cinematic dramatic intent through the coherent composition path for the mandatory v0.5 upgraded Filthy Frank meme intro (PR #92). This recovery round must not become an FM/Meatsack/DOOM/ABI redesign. Those programs remain on the roadmap; the present defect is musical-source/realization transfer, not evidence that a different soundcard is required.

## 5. Fan-out without a second orchestra of implementations

Use one principal integrator and at most four isolated workers:

1. **Recipe/harness:** effective profile, manifest, four-cell audition, complete receipts. Own examples/tests only initially.
2. **Teacher/representation:** export real reference excerpts, timing/span/pitch-basis roundtrip, minimal timed-material seam. Do not independently redesign orchestration.
3. **Carrier planning:** required-material reservations, all-carrier fulfillment, backing replacement, withheld-lead source, late-mutation removal. Agree interface with worker 2 before coding shared types.
4. **Adversarial acceptance:** independently check source/event/frame invariants and hostile mutations, chart/form contrasts, baseline regressions and fresh listening packet. This worker must not reuse emitter logic as its only oracle.

Separate worktrees, exclusive files, explicit base commits. Merge child changes only on the integration feature branch after review. Multiple assistants reading the same helper are not independent witnesses. No main merge, tag, release, default sound change or invented ear verdict.

## 6. Sources and next decisive result

All source paths below refer to audited commit 229b397b; relative links resolve correctly on this audit branch because runtime source is unchanged:

- [rick_story](../../examples/rick_story.rs), [rick_gen3](../../examples/rick_gen3.rs), [rick_probe](../../examples/rick_probe.rs), [band_story_probe](../../examples/band_story_probe.rs).
- [policy](../../src/audio/human_music/policy.rs): WRITTEN/POCKET/BAND inheritance and LeadLifePolicy.
- [composer](../../src/audio/human_music/composer.rs): ThemeSeed, chart-first selection, schedule, cloned hook, statement_starts.
- [motif](../../src/audio/human_music/motif.rs): Motif::render and melodic_events timing/derived articulation.
- [material](../../src/audio/human_music/material.rs): MaterialEvent, from_motif, Quote tail and room fitting.
- [interaction](../../src/audio/human_music/interaction.rs): withheld-site skip, single-response selection, narrative bias and optional silence.
- [performance](../../src/audio/human_music/performance.rs): source observation, accent grid, interactions, ensemble and budget order.
- [bass](../../src/audio/human_music/bass.rs): late pitch-only narrative post-pass and modular tone map.
- [functor](../../src/audio/human_music/functor.rs): candidate vs checked route and rehearsal.
- [song](../../src/audio/human_music/song.rs): ThemeSite::statable and symbolic identity boundaries.

**The next valuable deliverable is not another proof that existing notes obey an inadequate mapping. It is a controlled four-cell profile audition followed by one known-good four-bar phrase that survives normal source planning, truthful multi-player carriage, and the existing renderer.** Once that exists, generic music generation has a faithful target to learn into.
