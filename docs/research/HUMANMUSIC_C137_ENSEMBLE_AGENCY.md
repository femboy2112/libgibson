# C137 ENSEMBLE AGENCY — four musicians, one conversation, meaningful silence

> **Research + executed Score diagnostic only; NO runtime or source-default modification.** User-local 2026-10-11 20:40 ET. Pinned source: \`feat/humanmusic-c137-semantic-arc@aceed7e690acaf7a9fb00f01c4a6396bcce23e56\`, draft **PR #100** stacked on PR #98 (which inherited Sol #95 / Claude #96 and earlier C137). During inspection the latest CI job on \`aceed7e\` was still **in progress**; earlier \`8b619c0\` and \`008a60a\` succeeded. Refresh current heads/results before acting.
>
> **Maintainer ear verdict:** almost C137; similar density to the accepted \`teacher_golden7\`, but the musicians are **too cohesed**. Each individual should breathe and behave coherently with respect to the *same song and pocket*, while their energies interact via specific musical decisions. Existing output sounds like a statistically averaged band-energy envelope despite numerical dynamics. **Silence is information.** The seven actual accepted teacher WAVs, checksums and exact invocations were NOT found as verified GitHub WAVs; never invent them or claim to have heard them here.
>
> **This refines—not replaces—the macroform/semantic-density program in PR #99.** The current branch has corrected the 128-beat false ending through a higher-order narrative arc; do not redo it. The new target is *independent, responsive player agency and perceptually consequential complementary dynamics*, without reducing source/Score correctness, groove, or historical compatibility.

## I. Root diagnosis from the ACTUAL code

### F1 — One scalar for every backing musician is a shared gain, not a dialogue

\`argument.rs::backing_energy_for(relation)\` gives one value per semantic role, and \`CompiledArgument::backing_energy_at(beat)\` chooses the most recent placement. \`functor.rs::scale_backing_by_section_energy\` does:

\`\`\`rust
if n.prov.role_note != "argument" {
    n.velocity = (n.velocity * argument.backing_energy_at(n.start_beat))
        .clamp(0.02, 1.0);
}
\`\`\`

It applies that SAME \`g(t)\` to pad, keys and bass. \`scale_drums_by_section_energy\` applies the SAME scalar to the kit; thematic carrier events instead use the equally section-fixed \`section_gain_for(relation)\` through \`argument_transport\`.

For backing role \`i\`, written roughly as \`v_i(t)=g(t)u_i(t)\`, this generates shared macro-dynamic movement **regardless of any response between roles**. It can increase raw cross-role envelope correlation even if there are no causal musical transfers; it can also **reduce differentiated dynamic contrast** because every individual is pushed toward the same chorus rise or verse dip. The user's perceptual diagnosis fits this mechanism, but **the exact correlation/flatness has not yet been measured in the actual teacher/generator PCM**.

### F2 — The scalar is applied AFTER everyone has already performed

\`functor.rs::realize_policy_with_argument\` generates lead, keys, bass, pad and drums, then inserts new counterpoint and multiplies \`keys/bass/pad/drum\` velocities **after drums have already heard the earlier band**. \`Score.hearings\`, role plans, \`PerformancePlan\`, budgets and drum arbitration thus refer to a different dynamic performance than the final Score. This is a causal-order defect for any feature that purports to model **mutually responsive intensity**.

The existing \`Stage\` and \`EnsembleBar\` already know per-role on/off, modes, foreground and per-bar kinetic targets. They are the correct base for the next development; a second after-the-fact \`Score\` arranger is NOT.

### F3 — The recent countervoice is added after the drummer's hearing

In \`comp.rs::realize_countersubject\` the germ is inverted and placed in lead gaps or against held notes, with \`COUNTER_VEL=0.62\`, a fixed lead-in, and a fixed release/gate method. In \`functor.rs\`, that counter is appended to the keys voice AFTER \`realize_drums_*\`, so the drummer can neither decide to accent its answering entrance nor leave a planned space for it. Same-onset ordinary keys stabs are removed, but overlapping comp sustains and the bass remain as previously planned. The keys counter is source-related and the lead-hole calculus is legitimate progress; it's not yet **ensemble-level mutual listening**.

It's **optional** behind \`PerformanceProfile::with_counterpoint(true)\`; \`fusion_gates --counterpoint\` opts in. The harness's default is OFF. Do not assume which arm the maintainer last auditioned: pin exact CLI, score hash, world, seed, BPM and WAV before attributing a listening symptom to counterpoint.

### F4 — “The car” supplies propulsion in largely unvarying patterns

\`perf.drive=true\` is assigned inside \`functor::plan_and_realize_inner\` AFTER \`PerformancePlan::from_song_admitted\` builds its budget and ensemble. In \`bass.rs\`, \`Foundation/Counter/Quote\` with drive request the same eighth-note onsets \`0,2,4,...,14\` on the 16th grid. \`groove.rs\` gives the fusion kit fixed kick anchors \`0,6,8,11\`. \`comp.rs\` grants one additional keys stab per bar when drive is on. These are valuable drive/pocket controls, but can cause all bars to have similar underlying activity independent of a musician's actual local development. This is not proof the notes are bad; it gives precise intervention points for dynamic *subtraction* and *selective response*.

Move the optional drive/duty decision upstream of budget allocation before broadening it. Never change historical \`drive=false\` or its byte-exact controls.

### F5 — Existing gates don't measure the phenomenon the user hears

\`fusion_gates\` R1–R9 reject poor note density, weak lead coverage, insufficient harmonic variety, etc. Weave W1 tests lead/bass parallel perfect intervals; W2 tests keys countersubject motion relative to lead; W3 tests counter note consonance. **They do not measure whole-band role-energy dialogue or meaningful silences.** Moreover W2/W3 treat \`counter.len()<2\` as \`pass:true\` with \`n/a\`—a suitable optional-control semantics but NOT proof of counterpoint when absent.

Do not adjust old gate thresholds to make this verdict green. Add a separate observational + causal-intervention instrument.

### F6 — Production could exaggerate compression/averaging (unverified competing cause)

\`synth.rs\` applies shared saturation, memory/reverb/echo, a bus compressor (\`set(-14.0,2.5,12.0,140.0,1.5)\`), and a limiter. A common mastering stage can make legitimate role-specific energy contrast harder to hear. \`ProductionControl\` already provides \`no_bus_comp\`, \`dry\`, \`no_saturation\`, and \`HARMONIC_REFERENCE\` on the same Score. Perform a controlled producer A/B before redesigning the player dynamics purely from ear inference. **The score-level scalar remains a source fact whether or not production adds perceptual flattening.**

## II. Mathematical interpretation — global meaning, locally independent actors

The correct shared object is a **song context**, not a shared instantaneous energy value. Every musician observes projections of the same source-bound \`SongMap\`, final \`HarmonicRoute\`, \`AccentGrid\` and named semantic commitments, but has its own state and delayed causal hearing.

A *candidate* bounded discrete dynamics (design, not verified physics):

\`\`\`
S(t) = (source referents, form/section, final harmony, pocket, obligations)
x_i(t) = (role, motif-memory, breath/debt, energy, anticipated openings, last gesture)
m_(j→i)(t) = typed observed call/response/withhold/push/pickup, with causal time and source
x_i(t+Δ) = Φ_i(x_i(t), S(t), actionable messages arriving from j, role-specific constraints)
notes_i = Π_i(x_i, S)
\`\`\`

Shared source \`S\` constrains *meaning and pitch-time context*. Each musician's private state \`x_i\` makes its behavior distinct; **messages** and allocated response windows create causal correlation. Player energies can rise at different times or move in opposing directions when one hands the spotlight to another. We should observe complementary **phase/role relations**, not demand global positive correlation.

An optional role-specific intensity equation can be used for implementation only if grounded in named commitments:

\[
e_i(t+\Delta)
 = \operatorname{clip}\left(
    \underbrace{\rho_i e_i(t)}_{\text{memory / recovery}}
    + \underbrace{u_i(S(t),\mathrm{duty}_i)}_{\text{own mission}}
    + \underbrace{\sum_j K_{ji}(t)\,m_{j\to i}(t-\tau_{ji})}_{\text{heard gestures}}
    - \underbrace{r_i(t)}_{\text{breath / silence / release}}
 \right).
\]

**Do not call these physical energy conservation laws** or choose the weights to optimize a made-up intelligence score. Use typed musical events, bounded finite memory and compact explicit parameter contracts. A common envelope may still influence all roles **weakly**; it must not substitute for each musician's own dynamics. Avoid chaos, arbitrary independent random-jitter, non-convergent fixed points, and duplicate decision authorities.

Categorical law only where implemented: treat one role's musical realization as a typed projection from the shared \`SongContext\`; a response message is an arrow carrying **source ID + beat interval + role + consequence**. Two role traces compose when their local clocks, harmonic state, stage ownership, source IDs and silence/answer obligations match. A morphism that causes an entrance at the wrong source/time is not composable and must be refused, not corrected after rendering. Distinguish global coherence from an illegitimate same-energy requirement.

## III. Minimal implementation architecture (optional, additive, no second composer)

### A. Source-level \`EnsembleAgencyPlan\` before performance

Create one small opt-in planned object (name to fit code) derived from actual \`CompiledArgument\`, \`CompositionPlan\`, final harmony, groove and stage:

- Per-role \`AgencyLane\`: time-bounded \`Foundation / Foreground / Counter / Answer / Sustain / Anticipate / Recede / Listen / Withhold / HardSilence\` duties, with individual energy trajectory and carried/answered source IDs.
- \`Message\` / \`Cue\` with initiating agent, destination agent(s), source/material/debt ID, beat, allowed reaction window, handoff type, strength and verification witness.
- Onset/silence ownership: \`WrittenBreath\` can invite an answering voice while \`HardSilence\` forbids it; \`Listen\` causes a player to refrain; \`Withhold\` explicitly carries a musical debt; a band break can be a collective silence, not a missing instrument.
- A role-specific motion envelope \`e_i(t)\` with meaningful attack, recovery, look-ahead and lag, initialized from local role type and musical history. It is a *planned input* to note generation, not a post-render gain sweep.
- Source thematic carriers remain special: never modify a witnessed \`MaterialEvent\` without a declared licensed variation and independent witness; energy changes should be sourced and audited.

Re-use \`Stage\`, \`EnsembleBar\`, \`ActionPlan\`, \`Interaction\`, \`Occupancy\`, \`budget::allocate\`, \`Hearing\` and \`SongMap\`. No duplicate Rust public music engine; no broad ABI change now. Historic v0.4, R17, cover, \`perform()\` and previous opt-in sounds remain controls.

### B. Give different musicians different functions, not arbitrary offsets

- **Lead:** take a thematic breath on purpose; delegate source or answer to keys, then return at an intelligently changed dynamic; a held peak may demand *bass/drum restraint*, not a louder ensemble.
- **Keys:** anticipate and answer a particular lead gesture using its actual source/contour, not fill every rest. Sometimes choose \`Listen\` or a single chord instead of a counterline. Respond with a finite delay under common beat; avoid overwriting intentional silence.
- **Bass:** maintain harmonic floor when another role is active; approach the **next** harmonic event, selectively syncopate against kick, answer in lead holes when it carries the motif; its local energy can intensify while keys recede or drop for a critical lead entry. Avoid driving uniform eighths in every bar as an unconditional law.
- **Drums:** react to *final planned keys/bass/lead events* and the upcoming harmonic/semantic change, including controlled pullbacks, ghost dynamics, fill/withhold and re-entry; do not let static kick anchors dictate every dynamic motion.
- **Pad:** sit out or sustain with delayed dynamics when the counterline needs air. Long release/reverb tails must be treated separately from written Score silence.

### C. Causality/order

Build a shared \`EnsembleAgencyPlan\` and musical duties **before** any part is realized. Plan harmonic constraints before any dependent actions. For a stable finite production ordering, allow players to **respond to earlier known/authorized events** or source-level anticipated cues with explicit timelines. If bidirectional responses demand iteration, use bounded stage-by-stage proposals with deterministic tie-breaks and ensure each player's final hearing corresponds to the final notes. Do not add counterpoint after the drum pass and then claim the drummer heard it. No after-the-fact \`Score\` repair pretending to be agency.

Prefer a first minimal vertical slice: **one 4-bar passage with a lead breath that transfers to keys, drums open the window, bass supports the destination, then lead re-enters on a prepared chord**. At least one player must actively choose NOT to play where a naive density-maximizer would. The source witness and stage/absence receipts must pass.

## IV. Diagnostic instrument and falsification

A self-contained Python post-processor accompanies this plan:

\`research/humanmusic_ensemble_agency/energy_probe.py\`.

It parses the **current** \`fusion_gates --dump-tsv\` format (\`T/N/D/C\`) and reports:
- role-specific attacks/beat and sustained-activity proxies;
- raw pairwise correlations and short-scale correlations after removing each role's slow envelope;
- time-lagged lead/keys, lead/bass, bass/drums activity correlations (lags signed; *not* causal proof);
- lead attack-drop → keys follow-up opportunities;
- zero written-note/kit bins, carefully labelled **Score silence**, not PCM silence.

It has synthetic tests for **perfectly common energy**, **staggered complementary entries**, **true written silence**, malformed data and parser fidelity. The tests prove only metric discrimination on artificial inputs, not quality of the live band. Run on actual PR #100 TSV and handcrafted teacher TSV **if an honest exporter is available**. \`fusion_gates\` TSV currently omits source/role-note provenance and full actions; the analysis MUST NOT fabricate per-source transfer evidence from it.

**Strongest evidence: counterfactual causality, not correlations alone.** Given the same global song form/harmony/macro energy and seed, mutate a specific lead call and recompute. Only roles with declared dependencies should change in their authorized response windows. An unrelated section/song ID should remain stable. Mutate a keys answer and watch a later drum response change if the drummer explicitly heard it. A random-noise band or one multiplied master curve must fail these directional intervention tests.

Prescribe finite, label-free listening arms:

| Arm | Macro arc | Role-specific agency | Counterpoint/hearing | Interpretation |
|---|---|---|---|---|
| A: current | current common \`g(t)\` | none new | OFF | canonical existing control |
| B: current+counter | same | none new | optional keys counter | isolates whether independent line helps |
| C: individual-energy only | same source/harmony | distinct per-role trajectories | unchanged interactions | isolates gain shaping vs agency |
| D: cue-responsive | same source/harmony | source/duty/message driven | keys/bass/kit hear authorized response | candidate causal band |
| E: cue ablated | identical composition/density constraints | all timing but message transfer disabled | no source-linked response | falsifies decorative cue tags |
| F: no master compression / dry | same Score as A/D | same | same | isolates DSP flattening from composition |

Use *normal* vs *nocomp/dry* carefully: limiter and release/reverb memory still influence envelopes; alternate production is an audit, not a new default. Use 128/160 beats, fresh seeds, exact branch hashes and equivalent tempos/patches/loudness whenever possible. If \`teacher_golden7\` is not available as local source media, mark absent and make reconstructed \`rick_probe\` a separate comparison, never an accepted golden.

Adversaries:
1. All voices have same scaled macro envelope: false "coherence" should NOT be accepted as independent responsiveness.
2. Everyone varies independently/randomly: high diversity but no causal musical relation must also fail.
3. Keys enter on every lead gap: a **counterpoint saturation** negative control; authentic \`Listen\`/silence must sometimes win.
4. Drums always fill before Return: an automatic fill without actual musical setup must fail intentional response attribution.
5. One musician's cue is removed yet all role outputs remain bit-identical: purported cue was metadata.
6. Late inserted countervoice is absent from drummer's hearing yet drummer claims to answer it: fail.
7. Hard source silence filled by ornamental note: fail.
8. Cross-role exact onsets repeat every bar despite a directive to counter/syncopate: reject sameness where independent duties required.
9. Total onset density increased but listener comprehensibility worsens: **reject musical change by ear**, even with numeric gates green.
10. Underlying source/Score meaning correct but after mastering the audible contrasts flatten: isolate production/masking, do not overdesign agency.

## V. Commit and acceptance sequence for Claude

**R0: Freeze and observe.** Refresh PR #100, exact head CI, user-render command, \`teacher_golden7\` actual WAV/Score provenance. Run \`fusion_gates --counterpoint\` ON/OFF with TSV/stems and energy probe, and a same-Score \`ProductionControl\` ablation. Report whether common-gain dominance and average-envelope perception are *measured*, not merely guessed.

**R1: Red-first test for fake interaction.** In Rust prove late countervoice is not in drummer's hearing and post hoc backing gain differs from what they heard. Demonstrate current dynamic cue lacks an effect on individual choice. Record exact source/event hashes and positional witnesses. Preserve useful existing behavior; tests should express a proper new contract rather than assume current code is correct.

**R2: Smallest \`AgencyLane\` / \`Cue\` slice.** Build a four-bar lead-withhold/keys-answer/bass-floor/drum-restatement act with one authentic absence; ensure role-specific response windows are part of shared PerformancePlan BEFORE realization and respect source IDs/silences. Test per-role provenance and unchanged unrelated notes.

**R3: Generalize to a full song.** Let roles maintain memory, recovery, preparation and source-linked cues across sections; vary response lags under meter with finite rules. Not random jitter. Ensure the same global arc while individual energy streams differ and interact. Guard against unwanted phase drift, beat misalignment, endless rehearsal or a uniformly active band.

**R4: Controlled ablations and blind listening.** A/B/C/D/E/F above, score and PCM, real final voices including counterpoint in drums hearing, identical source and world as controls. Listen for intelligible musician identities, groove, dialogue, silence and payoffs. Record maintainer \`ACCEPT/REJECT/AMBIGUOUS\` separately from test outcomes.

**R5: Integration hardening.** fmt, strict clippy, rustdoc \`-D warnings\`, targeted/full tests, exact-HEAD GitHub Actions, v0.4/R17/covers/fingerprints and original opt-in fusion output hash, regression on \`--counterpoint\` ON/OFF and optional production. One non-main code branch, draft PR, versioned manifest, WAVs/stems and exact Claude handoff. No release, main merge, version bump, ABI rewrite or Meatsack/GSPU/Doom diversion.

### Safe agent fan-out

- Scout A: teacher/generator timbral + Score-level per-role energy audit, independently recover actual accepted golden files if present.
- Scout B: final hearing and causal pipeline auditor (\`functor\`, \`comp\`, \`drums\`), red tests.
- Scout C: typed \`AgencyLane/Cue\` architecture with minimal source/Stage/Budget seams.
- Scout D: independent Score/PCM energy, lag, silence, mutational/subtraction adversaries.
- Scout E: mastering \`ProductionControl\` vs acoustic listening; blind matched packets.

**One principal integrator** owns source architecture/implementation branch, synchronizes Rust interfaces, cherry-picks reviewed isolated commits, and does exact-head CI. Do not let Scouts B/C simultaneously rewrite \`functor.rs\`, \`performance.rs\` or \`argument.rs\`.

## VI. The actual musical objective

A genuine band is neither a single player with five volume knobs nor five independent random note generators. Its musicians share time, context and an evolving musical argument—but carry separate memory, anticipation, energy and obligations. Sometimes one player becomes intense because another yields. Sometimes the bass is preparing what the lead cannot yet say. Sometimes everyone stays out **because a silence itself answers the theme**.

The golden standard is **correlated INTENT, not identical instantaneous energy**. Musician A's increased tension may cause B to recede and C to prepare an accent two beats later. That negative or delayed correlation is musical communication. Do not force a positive Pearson statistic and call it coherence. Only actual responsive, source-preserving decisions and a positive human listening verdict warrant a C137 claim.

**Deliverable gate:** a source-correct, audibly better song that lets the listener hear *individual musicians breathing together*, with their silences and entrances telling the musical story. Every historical promise remains intact.
