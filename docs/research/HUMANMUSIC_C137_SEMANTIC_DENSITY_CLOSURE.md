# C137 — SEMANTIC DENSITY, TRUE CLOSURE, AND THE NON-PRETENTIOUS PATH

> **Source audit + falsification design, 2026-10-10 (user local). Not an implemented musical repair.**
> Pinned base: \`feat/humanmusic-c137-fusion-ascension@2465ad06a5424d77ca92f14fffbdab568000c540\`, **draft PR #98**. That head has passing hosted CI [38102356247](https://github.com/femboy2112/libgibson/actions/runs/38102356247). Historical v0.4 main remains separate. Research branch \`research/humanmusic-c137-semantic-density-2026-10-10\` changes ONLY research files, not any runtime, output defaults, 1.0 contract or active feature branch.
>
> **Maintainer's current ear classification:** "Morty and the Ricks" — generically interesting HumanMusic with vaguely but observably coherent semantics. The seven handcrafted \`teacher_golden\` renders remain the artistic bar. Added directive: **dynamic, coherent SEMANTIC INFORMATION DENSITY, through an interesting nontrivial path, without being pretentious.** Mathematical receipts cannot replace that ear verdict.
>
> This updates—not overwrites—[the prior fusion audit](HUMANMUSIC_C137_FUSION_RECOVERY.md) (PR #97), and the source-bound argument architecture in PRs #95/#96. Refresh refs, inputs, handwritten teacher files and listening notes first.

## 0. Short diagnosis — a demonstrated causal graph defect underneath improved music

Source reviewed:
- \`src/audio/human_music/semantic.rs::deflected_lift_trace\` and \`EventKind::is_salient\`;
- \`timeline.rs::quantize_event_beat\`;
- \`plan.rs::FormGraph::build_for_beats\`;
- \`argument.rs::MusicalArgument::fusion\`, \`validate\`, \`section_progression\`, \`compile\`;
- \`argument_witness.rs::verify\`;
- \`theme_family.rs::generate_pair/generate_bridge/Kinship\`;
- \`examples/fusion_gates.rs\` (the **actual default** audition);
- current \`performance.rs\` and \`functor.rs\` for the action/harmony/groove authority split.

### F0 — DEFAULT FUSION SONG RETURNS BEFORE ITS BRIDGE, THEN ENDS DEPARTED

For the **actual \`fusion_gates\` default** \`deflected_lift_trace(128)\`, the form builder's 4-bar grid plus quantized salient events produces these 10 phrase intervals (in beats):

| Span | Relation chosen by \`MusicalArgument::fusion\` | Source |
|---|---|---|
| 0–16 | Establish | Verse |
| 16–32 | Consequent | Hook |
| 32–48 | Establish | Verse |
| 48–64 | Consequent | Hook |
| 64–80 | Establish | Verse |
| **80–96** | **Return** | **Hook** |
| 96–104 | Depart | Bridge |
| 104–112 | Depart | Bridge |
| 112–120 | Depart | Bridge |
| **120–128** | **Depart** | **Bridge** |

**The “Return” happens BEFORE all four subsequent “Depart” segments.** The final phrase is a bridge departure, **not** a return/coda. This is not a matter of taste or harmonic embellishment: it violates the ordinary meaning of the program's claimed narrative sequence.

Why:
1. \`fusion\` classifies phrases by **length**, not by their declared formal/discourse role: every short phrase becomes \`Depart\`; the **last full-length** phrase with a previous hook becomes \`Return\`.
2. \`FormGraph\` legitimately subdivides the final part of the 128-beat trace into four **8-beat** phrases at beats 96, 104, 112, 120.
3. The program has no higher-level rule enforcing \`Depart < Return\` or a terminal \`Return\`.
4. \`validate\`'s \`ArgumentEnding::Resolved\` check only examines outstanding \`Question\` debts. This \`fusion\` sequence includes **no Question and no Answer**; the "resolved" debt check passes **vacuously**. The independent Score observer checks the defined source/content relationships, but does not enforce a concrete final departure-return contract.

A distinct **160-beat test fixture** used by \`argument.rs\` creates a different phrase partition and can end in a Return, masking the default problem. Source-level reconstruction also finds terminal \`Depart\` at 96,112,120,144 beats; verify directly in Rust and on fresh traces, not only from this transcription.

**Executed independent limited probe:** \`research/c137_semantic_density/form_order_probe.py\` transcribes precisely these **pinned source control-flow rules**. 5 isolated Python tests PASS, including the 128 negative and 160 positive control. It is NOT the Rust runtime, no WAV was heard, and \`--repo\` is required for pinned Git blob verification on a local checkout. A deliberate missing-checkout run exits 2 as a negative source-verification control. The Rust test is the next necessary independent proof.

### F1 — Bridge “travel” also resets at each short-phrase seam

\`section_progression(Depart)\` returns an **eight-bar** \`BRIDGE_PROG\`. But \`compile\` loops **per phrase**, restarting \`prog[b % prog.len()]\` at \`b=0\`. Each late 2-bar \`Depart\` therefore receives **the FIRST TWO bridge chords again**, rather than the intended eight-bar sequence. The trace splits the global bridge into short phrases; the harmonic planner forgets the section's **cumulative phase**.

This is a clean compositional boundary-gluing failure. Test the actual chord sequence at 96–128, with an intentional late Return. Do not merely rename the same loop's last chord as a cadence.

### F2 — Kinship and high rejection-gate counts are necessary but not sufficient

\`Kinship::altered_consequent\` currently tests common rhythm skeleton/head/sign contour, substantial pitch change, scaled note distribution and Open→Closed final pitch. Useful. It does **not** require that the consequent answer an actual *earlier unresolved musical obligation*, nor that a bridge causes a new final interpretation.

\`theme_family::is_departure\` checks sign-contour cell, register lift, scale membership and Open arrival. A hook fragment can share a common three-sign cell without having undergone a distinctive narrative transformation. This proves finite schematic kinship, not an experienced journey.

\`examples/fusion_gates.rs\` states **a deliberately scrambled arrangement passes all nine calibrated R gates**. Those gates reject insufficient lead onsets, poor coverage, lack of chord motion, low bass variation and dynamics, but can be Goodharted without a meaningful song. R-ROOT and R-GRID diagnostic backstops also do not replace semantic correctness. Preserve every calibrated test; add an **orthogonal** causal/long-horizon witness.

### F3 — The musical source has increased but its *semantic decisions* remain few

The newest \`theme_family.rs\` adds real generated four-bar verse/hook phrases and a bridge. It uses a limited set of phrase shapes/grids, global tonic-relative minor scale pool, shared first two intervals, and a common arch/descent pattern. The formal language is more interesting and teacher-calibrated, but the global generator still alternates the same verse and hook, with a brief bridge repeated by length. Its meaningful **referential events per 16-beat span** remain substantially fewer than the apparent note-count increase.

\`fusion\`'s RETURN is a literal hook source transport, not a source-dependent change in consequence, orchestration, register, expected cadence or relationship that only becomes possible because the bridge happened. To earn closure, its **content and function must depend on the path**; not necessarily every note should change.

### F4 — One authority is still only partially established

\`functor::plan_and_realize_inner\` first calls \`PerformancePlan::from_song_admitted\`, which constructs harmonic contexts, actions, accents, interactions and budgets; only afterward \`prepare_performance\` replaces chords/contexts and re-establishes deflection evidence. Then \`perf.drive=true\` gives argument songs a functional "car" groove. That can make an energetic song, but the code still needs to prove that all previously planned dependent fields read the **final** committed harmonic/semantic route.

Changing the audio engine is not the first hypothesis. The architecture should compile a final, typed source-level Section/Obligation graph and harmonic route **before** the existing performance derivations, then allow role-specific projection, all while preserving the historical OFF path byte-identically.

## 1. INFORMATION DENSITY — DEFINE THE THING WE WANT, NOT A NEW GOODHART SCORE

**Not:** note count, chords/minute, exotic modulations, speed, entropy of pitch histogram, maximum dissonance, maximal Shannon surprisal, or mandatory new “semantic verb” on every hi-hat.

**Desired:** a high rate of **recognizable, causally relevant musical developments**, dynamically distributed across macroform, four-bar phrases, individual instruments and immediate groove, without destroying listener memory or overwriting contrast.

Operationally retain FIVE SEPARATE OBSERVATION AXES, not an absolute \`human_quality\` number:

1. **Thematic referents and recognizable novelty** — How many identifiable statements are introduced, recalled, contradicted, developed, answered, or recontextualized? Which \`ReferentId\`, real event relationship, transformation license and Score witness supports each? Does a foreign unrelated note fail while a licensed variation passes?
2. **Harmonic information** — Do chords create real expectations through guide tones, bass motion, chromatic approaches, pivots and resolutions, or only display more chord labels? Is the final destination prepared by actual notes that can be heard?
3. **Ensemble relational information** — Do bass, keys, kit and lead *respond to one another* at the correct shared pulse/holes? Is a busy bass an independent meaningful counterline with foundation, or just a second restless stream? Separate complementary interlock from duplicated unisons.
4. **Temporal hierarchy and dramatic economy** — How do activity, suspense, apparent complexity and purposeful silence vary over individual bars, four-bar phrases and entire sections? High activity everywhere means no contrast and poor salience. A simple hook in a carefully changed context may carry more meaning than a dense run.
5. **Listener-level intelligibility** — On blind label-free audition, can a listener distinguish a correct earned return from a reordered/foreign/false one? Can they recall the hook and notice an intentional relation at the end? Report \`CLEAR / VAGUE / UNRECOGNIZABLE\` with separate \`COMPELLING / GENERIC / SHOWY\` judgment; no universal decoding claim.

Do not invent a numerical information theory without a well-defined probability model and genuine listening observations. Optional future \`I(\text{intended argument};\text{listener classification})\` is empirically definable from **real blinded responses** and a confusion matrix, but one ear verdict cannot estimate a robust distribution. Code-only \`SemanticMoveLedger\` may count independently verified events and missing debts, but MUST NOT pretend to measure human information bits.

### What “without being pretentious” means in implementation

Every conspicuous source-level flourish must be justified by at least one of:
- clarifying or transforming a recognizable motif;
- changing a specific harmonic destination or expectation;
- deepening a particular player-to-player call or answer;
- improving groove/contrast/breathing;
- creating a meaningful absence or preparing a payoff.

Ordinary groove strokes and texture need not carry a unique literary theme. They can legitimately sustain an established musical context.

**The subtraction test:** remove an ornament, chromatic approach, added chord, counterline or fill while keeping unrelated sources fixed. If the main theme, groove, dramatic consequence and perceived value are unchanged, prefer the simpler arrangement. An ornament can justify itself musically by feel or groove, not just by a written plot marker, but demand the owner identify and test that effect.

**Path dependence:** a nontrivial journey has consequence. With the *same final chord, apparent hook and loudness*, permuting the bridge/departure or replacing its development with unrelated busy writing should meaningfully change whether the final return is experienced as earned. Predeclare score-level failure predicates, then conduct a blind matched listening contrast. Complexity that can be shuffled without changing the argument is decorative complexity.

High-level musical intent should schedule *where* novelty and density are useful. Local player improvisation chooses *how* to express the space it is given. There is no requirement that the user be able to verbalize literal prose from instrumental music; the desired outcome is perceptible thematic and harmonic causality.

## 2. MINIMAL ARCHITECTURAL FIX — SOURCE-BOUND ARC OVER A FORM GRAPH

Make a small optional \`ArgumentArc\` (names illustrative) as a **higher-order span over one or more existing \`FormGraph::Phrase\` nodes**. It is the authoritative causal sequence; phrase length is a capacity constraint, NOT a story-type classifier.

Conceptually:
- \`ArcRole::Teach | Question | Consequent | Develop | Depart | Reframe | Return | Coda | DeliberatelyOpen\`.
- A typed \`ArcSpan\` has absolute beat interval, underlying phrase IDs, actual source referent, dependency edges, harmonic destination and required carrier duties; the source may span multiple short phrases only through explicit source/timing/room checking.
- The \`Arc\` carries typed open **obligations**: \`Departure(id)\` must be closed by a **later** \`Return(id)\`; an \`UnansweredQuestion\` by its actual source-linked answer; an open-ending exception is an explicitly different target.
- A return is distinct from a casual copy: relate the final theme, source memory, context-changing bridge, prepared cadence and band arrival. Record what the bridge changed, how the return is different in function, and what evidence would be absent without the intervening music.
- \`ArgumentEnding::Resolved\` must be **non-vacuous** for arcs promising closure; never pass with no open debt because no debt was created. A simple intentionally open/freeform vignette is valid only if its declared ending contract says so.

The **strong source identity**/independent \`argument_witness\` still checks the actual notes, timing, carriers and rests. The higher-order witness independently reads realized timed parts/chords and checks arc order, harmonic preparation and source transformations. Discourse roles cannot be inferred from mere length and then used as evidence that a source-resolved obligation was satisfied.

### 128-beat default first repair — make the final 32 beats causally meaningful

At 128 beats the last four 8-beat phrases span **96–128**. A candidate edit (one of several, to be tested) is:
- **96–112**: a 16-beat bridge/departure made from two adjacent 8-beat phrases, with a continuous harmonic route rather than \`BRIDGE_PROG[0:2]\` reset twice.
- **112–128**: a 16-beat source-linked hook return/coda made from the final two adjacent 8-beat phrases. Do not silently squeeze a 16-beat hook into one 8-beat subphrase.
- If a genuinely shorter final reprise works better, implement and witness a proper shortened-source adaptation. Changing \`time_numerator/time_denominator\` to force a fit is not a substitute for pocket-safe transformation.

This is *only* a first valid scheduling **candidate**, not an automatically good composition. Test another candidate with a longer bridge and shorter developed return; which feels more earned is an ear question. Preserve any intentional silence.

The 160-beat example where the Return happens last is a necessary positive control, not proof of the default; test both 128 and 160 and lengths 96,112,120,144, with multiple traces/semantics. Avoid a single-case patch.

### Harmonic and semantic clocks MUST align

A \`BRIDGE_PROG\` of eight bars is one continuous dramatic sequence. If the form fragments its section into 2-bar phrase nodes, index the chord route by **global section-local bar** (absolute bar minus section start), not by each little phrase's local bar offset. Ensure final harmonic preparations occur immediately before the declared return/coda.

Make final chord/source commitments BEFORE the Plan derives mutable actions/interactions/budgets, or prove by explicit differential tests that downstream derived fields are rebuilt consistently. No retroactive stamp.

## 3. HOW TO ADD SEMANTIC DENSITY WITHOUT ADDING NOISE

Use a 3-level composition plan:

**Macro (section):** small number of source-backed central questions, fresh departure/pivot, timely recontextualization, earned return with recognizable altered consequences. A deliberately unresolved ending is allowed when requested; do not confuse accident with intention.

**Meso (4-bar phrase):** one **salient, listener-trackable** musical move per expressive phrase (or a deliberate meaningful held context), plus clear relationships to the prior phrase. Vary transformations and positions according to a planned dramatic trajectory, not independent re-rolls.

**Micro (band):** existing \`EnsembleBar\`, \`ActionPlan\`, \`AccentGrid\`, stages, voice hearings, phrase/groove realization and \`budget::allocate\` generate dense, idiomatic instrumentation around that move. Bass can anticipate the next root while supporting harmony, keys can answer a specific lead hole, drums can announce a pivot, and a pad can sit out. **No obligation to crowd every interval.**

Introduce an opt-in, **read-only** \`SemanticMoveLedger\` with per-window:
- \`source_id, relation, introduced/developed/deferred/paid, actual event witness\`;
- harmonic pivot and functional voice-leading evidence;
- complementary interlock, actual beat/groove alignment, meaningful silence;
- unsettled commitments and legitimate open endings;
- section age, time since motif heard, arrival context;
- note-onset density by role, unison share, phrase coverage, not as substitutes for those source-linked events.

Use **equivalence-class diversity with memory**: a hook remains recognizably itself after lawful change. New novelty matters only if the earlier object stays trackable. Abundant random novelty erases reference; simple repetition without changed context is stagnant. Make the composer decide where surprise is earned.

## 4. RED-FIRST TESTS AND ACTUAL LISTENING LAB

Before implementing, record predicted PASS/FAIL for independent controls:

1. **Default form regression**: \`fusion(deflected_lift_trace(128))\` must fail the **new** \`Resolved/DepartureReturn\` witness with \`return_precedes_departure\` or \`ends_in_departure\`. The current Rust source otherwise permits it; characterize without breaking historic defaults.
2. **Fixed positive**: \`fusion(deflected_lift_trace(160))\` has a late Return and should pass a narrow order predicate (NOT automatically all the new obligations).
3. **Many lengths, semantic event shocks**: 96,112,120,128,144,160; include events that subdivide a bridge or finale. Do not let short phrase count decide arc role.
4. **Bridge continuity**: given a bridge spanning four 2-bar phrases, the chosen global bridge harmonic route must advance rather than restart at bar 1 four times. A mutation that resets the local index must fail.
5. **Vacuous closure**: \`Resolved\` plus no bound causal debt must fail *where the selected argument archetype promises resolution*. Legacy \`calibration\` and legitimately open/freeform programs must retain their own declared contracts.
6. **False return**: move a correct hook return BEFORE the departure, retain the same notes, instrument labels, tempo and final chord: new source-aware witness MUST reject; old nine musical-surface gates can plausibly still pass.
7. **Unrelated reprise / wrong source / decorative pivot**: maintain event count and harmonic diversity while substituting a different referent or merely extra jazzy chords; reject causal falsehood.
8. **Distinct real development vs literal restatement**: both may be musically valid, but only actual transformed content+context can be called developed; literal repeat with no changed function cannot claim an earned transformed return.
9. **Subtraction tests**: remove a claimed essential harmonic pivot, counterline, fill, pause, or altered consequent; if the source/Score receipts do not change, classify that ornament as **optional**, and compare by ear for genuine musical value.
10. **Goodhart controls**: run \`band_scrambled\`/same-note-count/over-decorated high-chord-count examples that pass R1–R9; the new relational witness should still refuse missing causal order. Do not change calibrated rejection thresholds to call it solved.
11. **PCM salience**: with the same source/Score semantics, compare stem, source-only, full mix, source-muted and loudness-controlled versions. Score correctness and raw PCM residual cannot alone establish intelligibility.
12. **Historical compatibility**: old v0.4/R17/cover/fingerprint outputs and unswitched generic \`perform()\` stay byte-identical; new arc behavior is confined to opt-in argument/fusion.

Suggested shell probes (report exactly what was run):

\`\`\`bash
python3 research/c137_semantic_density/form_order_probe.py --repo . \
  --out /tmp/c137-form-order.json

cargo test --lib audio::human_music
cargo test --test audio_musical_argument
cargo run --release --example fusion_gates -- \
  --seeds=2112,7,99,701 --beats=128 --table-only
cargo run --release --example fusion_gates -- \
  --seeds=2112,7,99,701 --beats=160 --table-only
cargo run --release --example fusion_gates -- \
  --seeds=2112,7,99 --beats=128 --dump-tsv=/tmp/c137-tsv --wav=/tmp/c137-audio
\`\`\`

Check the real CLI for any changes and create output directories before \`--dump-tsv\` (the current harness writes directly). All \`fusion_gates\` R gates are **reject-only** and its program exit code does not signal R1–R9 rejection; parse the report, not merely exit status.

**\`teacher_golden\` limitation:** the seven actual accepted media files, exact hashes and listening notes were NOT present as trackable GitHub WAV files at the inspected PR #98 tree. Prior source examples \`rick_probe.rs\` / \`band_story_probe.rs\` are available and are calibration controls, but their freshly reconstructed audio MUST NOT be called the original accepted seven. If the maintainer's local \`teacher_golden\` directory is available in Claude's environment, pin filenames, exact configurations/source sha, hashes, listener labels and native/matched-context distinctions.

## 5. EXECUTION PLAN FOR CLAUDE

**Round 0 — make the missing causal obligation demonstrably red.** Verify the isolated probe with the pinned files. Reproduce 128/160 scheduling and actual \`Score.chords\`/source event traces with a NEW opt-in Rust test/harness. Pin available teacher artifacts and the latest live CI, record the user ear verdict. Do not start by adding melody operators or tuning density thresholds.

**Round 1 — repair macroform and coherent bridge clock.** Add a typed, source-bound arc over existing FormGraph phrases. Ensure meaningful \`Depart→Return\` order and finale, even when form subdivides into 8-beat phrases. Prove bridge harmonic progression carries state through phrase boundaries; test false-vs-real return. Keep legitimate open endings distinct and preserve old generator modes.

**Round 2 — real semantic payload and dynamic density.** Use the actual ThemeFamily verse/hook/bridge, extend the return with a declared transformation that *depends on what happened* in the middle. Track useful motif developments/harmonic changes per segment, and give stage/ensemble a planned arc of complexity, calls, answers and meaningful holes. No forced "new idea" on every bar. Don't add density unless its *compositional reason* can be observed and tested.

**Round 3 — shared source plan and ensemble cooperation.** Make \`HarmonicRoute\`, source obligations, \`ActionPlan\` and player-specific bass/keys/drums response plans agree at compilation time. Preserve drive groove improvements and section energy but prove actors saw final chord context. Test ablated accents/groove/counterpoint; no random-note density padding.

**Round 4 — hard listening triangulation.** Blind matched output packets: original accepted golden (if locally present), reconstructed teacher, previous \`Morty and the Ricks\` run, actual repaired C137, counterfactually reordered/decorated controls and complementary role stems. Hold loudness/tempo/patch stable in matched comparisons; native teacher context separately. Ask: “Which hook is being answered?”, “Does the bridge cause the return to mean something different?”, “Does the rhythm section add coherent information, or merely events?”, “Which moments are showy rather than useful?” Results \`CLEAR/VAGUE/UNRECOGNIZABLE\` and \`COMPELLING/GENERIC/SHOWY\`; leave unperformed listening \`PENDING\`.

**Round 5 — mergeable checkpoint.** Separate worktree from the latest accepted #98 head, exact checks, adversarial mutants, archival regressions, honest PR and sound artifacts. No merge/release/main edit. The v0.5 finale remains the epic Filthy Frank keyed cinematic demo on the accepted integrated head; C137 quality gates precede it. Later Meatsack, GSPU-FM, Doom, language-parity projects are outside this round.

**Safe fan-out:** (A) macroform/obligation correctness, (B) continuous harmonic route, (C) independent density/masking/Goldens audit, (D) ensemble dialogue, (E) blind listening/Goodhart adversaries. One integrator owns \`argument.rs\`, \`functor.rs\` and the production branch; children own separate tests/fixtures/worktrees. Begin by agreeing the smallest \`ArcSpan\` contract; never build two competing music engines.

## 6. THE C137 DIFFERENCE, STATED WITHOUT HYPE

“Morty and the Ricks” can play an interesting musical vocabulary. **C137 must remember exactly what the first phrase promised, travel somewhere that changes the significance of that promise, and come back with a response that couldn't be swapped with an arbitrary earlier chorus.**

A high-information piece is **not** one with many events. It is one where many different events make the **same evolving musical argument easier to follow, more surprising in justified ways, and more rewarding to hear**. Simpler moments are load-bearing: the pause makes the next response intelligible. Strip any grand gesture that cannot demonstrate a musical function or an audible aesthetic benefit. Sophistication without self-importance; energy without clutter; an interesting path with an earned destination.

**Research status: verified-by-inspection hypotheses plus isolated test, not a working upgrade or accepted audio.** Next action = implement a direct Rust falsifier for the 128-beat default and close the actual terminal bridge→return dependency before tuning anything else.
