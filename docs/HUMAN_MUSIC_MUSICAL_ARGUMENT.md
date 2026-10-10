# Optional musical arguments: implemented boundary

This is a source-bound experimental vertical slice, not a completed replacement composer or a new listening acceptance. Historical entry points, public structs, canonical encodings, cover mode and release metadata are unchanged. The exact historical base is `fbdbffcee3f77dab3cef0234666192cadc7da370`; PRs #93 and #94 supply provenance, not runtime dependencies.

## What the old model loses

For fixed composition plan P, the inspected implementation has `F_P(trace) = g_P(story_arc(trace))`. The target also stores that arc, so equal targets imply equal arcs. Its finite trace quotient has at most five attainable classes: Low/Home, Mid/Home, Mid/Open, High/Home and High/Open. This is a finite quotient, not a claimed linear rank. Opposed intermediate traces collide even though their richer IntentTimelines differ. The executable baseline tests also show an unrelated generic Answer paying a MotifQuestion, and a pitch-unit mutation retaining the entire old symbolic observation and commutation while changing independently realized intervals. None implies identical whole-pipeline PCM.

The old laws and observers remain intact as historical contracts. A source-related argument is additional information the old quotient cannot certify. See `docs/research/HUMANMUSIC_SEMANTIC_BASELINE.md` for exact controls and their scope.

## Source objects and finite laws

`MusicalReferent` owns ordered `MaterialEvent`s: independent beat onset, sounding gate, accent and declared pitch unit. This slice accepts explicit semitone sources. A `ReferentId` names a source; an `ArgumentStepId` names an occurrence. `ArgumentStep` binds one source, one concrete transformation, mandatory carriers, a relation to an earlier occurrence and explicit dependencies. `MusicalArgument` binds the steps and a declared resolved or intentionally unresolved ending.

`ArgumentTransform` licenses an integer octave displacement and positive rational time scaling. On the completion-free domain these operations have identity and tested associative composition, subject to representable finite bounds. Completion appends a specified terminal pitch and gate; it is a source-bound operation with a restricted domain, not an unrestricted invertible transformation. The API refuses to pretend completion-bearing transforms form the same closed algebra. No categorical claim exceeds these implemented domains and laws.

A Question declares a particular expected completion. An Answer must retain the whole question's source head, permit only the declared time/octave changes, append the expected completion and depend on the actual question. A Denial must follow an expectation taught by an earlier completed statement and substitute a different completion. A Return must preserve the established source and respect predecessor order. Ending with open debt is accepted only under the explicit unresolved-ending contract.

`CompiledArgument` wraps an existing `SongMap` instead of adding fields to established public structs. Compilation uses its existing phrase grid and arrangement, assigns source-related thematic pages and a joint functional chart, updates discourse roles/references, and places mandatory timed sources. `Motif` remains an auxiliary inter-onset planning page; it is not falsely described as a lossless encoding of note-off times. The timed placements remain authoritative.

## Actual causal pipeline

`MusicalArgument::calibration(family, seed)` generates original source pitches, pickup timing and gates. `compile(&base_song)` chooses phrase-aligned source occurrences and harmonic consequences. `functor::perform_argument` uses the production PerformancePlan and existing players. Before any player chooses notes, `prepare_performance` pins required functional chords, checks the actual world/language vocabulary, and rebuilds contexts and deflection evidence. This closes a falsified source transfer: ordinary Lift coloration could turn declared V7 into a major seventh and destroy its tritone.

Required sources replace the relevant player's full reserved phrase at the player's source seam, before dependent hearings. Lead/authored occupancy, keys before bass, and each bass take before pad/drums use the replaced sources. Replaced phrase-expression plans are discarded rather than advertised as evidence for the new material. Exact onsets, gates, accents and all named carriers survive; earlier crossing backing gates are explicitly clipped. A same-count replacement succeeds. No source is reduced to an optional final-four-note reply, and bass is not remapped modulo chord-tone count.

Source graph IDs live in the wrapper and transport receipts. They are not forged into the existing action/material/obligation ID spaces. This choice preserves existing APIs but leaves an explicit integration debt for the old generic receipts.

## Independent contract and calibration domain

The raw Score witness independently reconstructs expectations from original referents and declared licenses. It does not call the compiler's transformation or trust provenance as content proof. It checks every required carrier, source intervals in a common tonic frame, timing/gates/accents/rests, ordered dependencies, source-specific answer gluing and the written Score chart's root and pitch content at the destination. Legal constant octave moves and permitted carrier/timbre changes preserve structural identity; arbitrary semitone displacement does not. The observation lattice is one microbeat and one millionth of accent. Raw matching allows 0.00001 beat float tolerance; this is a bounded observer, not exact real-number equality.

The finite contrast domain is A/B across predeclared fresh seeds and supported comparison worlds, plus explicitly malformed counterfactuals. The tests require distinct signatures for different generated trajectories under the same affect source. This is restricted distinguishability, not universal injectivity, a literal-language claim or a psychophysical theorem.

| Family | Implemented relation | Boundary |
|---|---|---|
| A: call and earned answer | Establish completed head; reopen it; augment/develop via another carrier; source-bound delayed completion; plural return | Machine source/chord evidence only; listening open |
| B: promise, denial, transformed return | Teach completion; reopen; deny with different completion; develop; answer; augmented return | Machine source/chord evidence only; listening open |
| C: conflict and reconciliation | Reserved name only | Factory/validator explicitly refuse; not implemented |

The calibration grammar is intentionally narrow. A distinct related hook exists in the bank but is not separately scheduled as an audibly established hook. Broad harmonic/thematic search, richer premises and hook development remain future work. This slice establishes infrastructure and falsifiers; it does not certify C137 musicality.

## Separate evidence layers and unfinished bridges

1. Source validation checks intention and dependency obligations.
2. The independent Score witness checks actual source-bound note realization and the written functional chart. `Score.chords` is a chart: the synth does not read it directly. Required source notes are observed independently, but backing-note harmony and acoustic harmonic preparation are not reconstructed by this witness. Planning the players after pinned harmony is causal evidence, not an independent sounding-harmony certificate.
3. Existing coarse commutation and general PerformanceReceipt are measured and reported separately. Their historical projections do not yet recognize the new timed-source graph. Passing the new witness does not mean `perform_checked` accepted the take, and coarse failures are not concealed by setting the target equal to the observation.
4. PCM ablation verifies a finite contribution actually reaches output. It can reject absent/nonfinite contribution. A nonzero residual cannot prove pitch recognition, masking adequacy or comprehension. Bus meters are unsuitable because they stay active under stem muting.
5. Blind listening records perceived relationship and artistic merit separately. No ACCEPT verdict is inferred.

First-slice auditions explicitly disable discretionary actions so unrelated harmonic edits/modulations cannot violate the common source frame. Generic ensemble narrative and post-hoc repair are refused. Stock Swiss Signal excludes sevenths and is a valid negative control; the audition's shared A-Aeolian, 108 BPM, sevenths/mixture-enabled timbre comparisons are explicit variants, not historical default changes. Harmonic incompatibility is an invalid comparison, not a bad-music verdict.

The next smallest integration step is an honest optional read-side projection of the timed source/obligation graph into the old SongMap/performance receipts, respecting pitch units and functional quality, without fabricating historical IDs. Then broaden the generated source grammar only where blind listening identifies the responsible source, salience or masking limitation.

## Future C ABI representation

No C ABI changes occur here. Future v1.0 parity needs explicit versioned representations for source/occurrence IDs, event units and timed events, rational licenses, source-linked completion/dependency records, mandatory carrier sets, unresolved-ending policy, compilation refusals and separate compositional/Score/acoustic evidence. The Rust wrappers contain owned vectors and enums and must not be exported by memory-layout assumptions. This is a parity inventory, not an ABI implementation or version bump.
