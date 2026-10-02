# Independent cover boundary development review

These are targeted development mutations of existing fixtures, not a fresh holdout or seed matrix. Source compilation used the evolving cover worktree; binary/library hashes and explicit custody limits are recorded in the JSON receipts. The root engineering gate separately binds final committed source.

Observed first probe: **2 pass / 3 fail**, exit101. `red.txt` preserves exact output. Two malformed public bar descriptions escaped their Result boundaries as integer-overflow panics; a generated Note mutated to MIDI128 did not fail structural Score validation.

The cover author replaced unchecked form/skeleton bar accumulation with checked addition and a typed error. The independent reviewer added the three-line Note MIDI-domain check to `Score::validate`, with positive endpoints0/127 and negative values-1/128. This check changes validation only, not generated events or PCM.

The source-record-order control and mutated-source-tonic control were already passing when the first executable probe ran: their earlier static findings had been repaired by the cover author. Source voices now retain canonical event order without a duplicate melody cache; extraction revalidates mutable reference metadata.

`multiplicity-control-red.txt` preserves a separate **refuted review assumption**: repeated simultaneous groove strokes were initially expected to be invalid. The declared groove relation is an ordered multiset; multiplicity is retained semantic data. The corrected control accepts canonical repeated strokes and proves one versus two strokes changes the canonical fingerprint. Unknown-axis sets reject duplicates/order variation, and unsorted groove strokes reject. No historical assertion or holdout result was altered.

`groove-transport-positive.txt` is a positive control, despite the initial concern: the author repaired target validation before that executable test built. Sparse eighth swing0.5 would collapse distinct metric Kick attacks at1/2 and5/8 into performed beat0.625. Target admission now rejects that noninjective projection before drum generation. Simultaneous strokes sharing one canonical position remain valid multiplicity.

Final independent suite: **8 pass / 0 fail**, exit0 (`green.txt`, `green.json`). It checks source-order identity, malformed reference metadata, typed arithmetic rejection, MIDI domain/admission, canonical set/stroke representation, and transported drum-event identity.

Other previously reported review gaps were resolved in the integrated code: actual section kinds are checked for partial ordered charts; noninterference compares complete generated evidence; strict admission includes score validity, continuation violations, shared stage authority and semantic occupancy; pinned source occupancy is reconstructed in canonical metric coordinates; harmonic candidate admission consults target world and language. Those integrated laws are additionally exercised by the cover author's tests; this eight-test receipt does not claim independent coverage of every one.

Read-only boundary review found no retained reference Composition or source lookup in cover generation, and no finished-Score pitch/rhythm repair to force cover conformance. Source constraints enter planner and source generation. The partial chart's generated timing/intro remain target assumptions, not asserted source observations. These facts and tests do not establish that the output sounds recognizable or good.

Reproduce:

```sh
CARGO_BUILD_JOBS=1 cargo +1.98.1 test --all-features --test audio_cover_review -- --test-threads=1 --nocapture
```

Log custody: `raw/*.txt.gz` retain exact original stdout/stderr bytes. `raw-log-sha256.json` hashes their decompressed bytes. Adjacent `.txt` displays only strip trailing whitespace/extra final blank lines for the repository whitespace gate; no result or failure text is removed.
