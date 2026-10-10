# Claude integration handoff: HumanMusic semantic resurrection

This branch implements an optional, source-bound A/B musical-argument vertical slice. It is
not an accepted C137 composer, not a completed generic receipt integration, and not a release.
Human listening remains **UNVERIFIED**. Preserve the historical controls and the separation
between intended argument, actual source/Score, PCM exposure and artistic acceptance.

## Exact lineage and authority

| Item | Branch / artifact | Exact head |
| --- | --- | --- |
| Refreshed implementation base; PR #87 | `feat/v0.5-humanmusic-beefup` | `fbdbffcee3f77dab3cef0234666192cadc7da370` |
| v0.5 finale plan; PR #92 | `docs/v0.5-observable-instruments` | `7f89c6cdd6729929b87750b6306950c08b1b7775` |
| Quality-transfer audit; PR #93 | `aletheia/c137-quality-transfer-audit-2026-10-09` | `066520780f0aaaa3c9225135c10fae7dc16d1143` |
| Semantic-identifiability plan; PR #94 | `research/humanmusic-semantic-identifiability-2026-10-09` | `feb681cd96fc22eaf16bd5c86960e380f655fcbf` |
| New implementation branch | `impl/humanmusic-semantic-resurrection-2026-10-10` | Resolve current pushed head with `git rev-parse origin/impl/humanmusic-semantic-resurrection-2026-10-10` after fetching |
| Checkpoint 1: baseline falsifiers | legacy collision tests and source audit | `beeab460986c28f880aa932935b956b4c437cbae` |
| Checkpoint 2: validated implementation | optional compiler, source seams, witnesses, audition | `2b72f00e387d4f59abc36d8e01760fb735119136` |
| Checkpoint 3: acoustic controls | source-removal continuity isolation and full-song metadata/muted PCM tests | `1439077f29b46c2c6ea16b5231059bc00a39d49e` |
| Checkpoint 4: integration/evidence handoff | this handoff, final formatter and blind-packet wording | Resolve the final handoff commit from the branch log; its exact SHA is also in PR #95 and the delivered evidence manifest |
| Draft review PR | [PR #95](https://github.com/femboy2112/libgibson/pull/95) | review the exact pushed branch head, not the earlier source-audit head |

Ancestry is refreshed PR #87 base → checkpoint 1 → code checkpoint → acoustic-control checkpoint → handoff checkpoint.
PR #93 and #94 are provenance inputs, not prerequisites to merge into this branch. In
particular PR #93's experiment audited **`229b397b46a800342a58ac5c38c8bbad3d2cfb87`**,
not the later `fbdbff` head. Its isolated Python outputs are not retrospectively updated by
Claude's completed 2×2 narrative/lead-life audition. That audition was already present at
the refreshed base and is not presented as new implementation work.

The user authorized non-main commits/pushes and a draft PR. This work does not authorize a
main merge, tag, package publication or release. PR #92's epic upgraded meme-intro finale
remains the v0.5 release-finale requirement, after musical acceptance and visual integration.
MeatsackProjection, Genesis/GSPU-FM, terminal Doom and broad language parity remain separate.

## What changed and what it actually establishes

The unchanged legacy target satisfies
`F_P(trace) = g_P(story_arc(trace))` for a fixed `CompositionPlan`. Its target retains the
pair, so equality is exactly equality of that quotient, with at most five classes. This is
not a whole-pipeline factorization: richer intent paths still differ. Seven executed
baseline tests expose the fixed-plan collision, unrelated generic Answer discharge and
declared-pitch-unit collision. A unit mutation of the old flagship preserves its complete
legacy observation and successful commutation while changing realized intervals.

The new source object reuses ordered `MaterialEvent` data: independent onset, sounding
gate, accent and explicit semitone pitch coordinates. `ReferentId` identifies a source;
`ArgumentStepId` identifies an occurrence. A step has mandatory carriers, a declared
transformation, a source-specific relation and finite dependencies. A `CompiledArgument`
wraps an existing `SongMap` without adding fields to historical public structs.

The transformation license is integer octave displacement plus positive rational time
scaling. On its bounded completion-free domain, identity and associative composition are
tested. Appending a declared completion is a restricted source-bound operation, not an
invertible arrow or a falsely closed general transformation algebra.

The compiler assigns phrase-aligned timed placements, source-related thematic pages,
discourse roles/references and a joint functional chart. `functor::perform_argument`
enters the existing PerformancePlan/player route. Mandatory material replaces the relevant
player's reserved phrase **before dependent players hear it**, including written gaps and
same-count replacements. Bass receives the complete declared source rather than a
chord-tone modulo mapping. Prior backing sustains crossing a reservation are clipped.

The independent observer reads raw Score notes, reconstructs expected contents from the
original referents with separate arithmetic, checks every carrier and declared dependency,
and inspects actual chord pitch content at preparation/completion. It does not reuse the
compiler's transform or accept IDs/provenance as a content proof. Common-tonic checking
rejects arbitrary semitone shifts while permitting lawful octave movement. Observation
precision is one microbeat and one millionth of accent.

| Calibration family | Current implemented slice | Acceptance boundary |
| --- | --- | --- |
| A — call and earned answer | Teach completed source; reopen its question; develop through another carrier; delay and complete that same source; plural return | Source/Score witness observed passing; human comprehension and musical merit unverified |
| B — promise, denial and transformed return | Teach expected completion; reopen it; substitute a different completion; develop; answer; genuinely augment the returned source | Source/Score witness observed passing; human comprehension and musical merit unverified |
| C — conflict and reconciliation | Reserved family/relation only; factory and validator explicitly refuse it | **Unsupported**, not a completed third family |

A/B use the same fixed affect trace and form timing. Their source-defined relationships
produce distinct raw-note signatures across fresh seeds. The source grammar is modest;
it generates pitches, pickup timing and gates through the same operations, not a hardcoded
final Score. A related hook exists in the bank but is not independently scheduled as an
audibly taught hook. Broad joint theme/chart search remains unfinished.

## Fixed build-time falsifiers; do not regress them

The initial optional source chart could announce V7 while the ordinary Lift colorer
realized a major seventh or dropped the seventh. That removed the dominant tritone and
invalidated the prepared arrival. The repair is explicit: `prepare_performance` runs
before players choose notes, pins the actual functional chords, checks the world's real
vocabulary, rebuilds contexts and deflection evidence, and refuses incompatible harmonic
edits/modulation. The observer checks actual pitch content rather than a V label.

Question/development material is now scheduled over actual V7, answer/return over home,
and denial over the connected different arrival. The generated `{2, 7, 14}` source head
avoids the initial tonic-fourth clash against the dominant's major third. Development
stays on V7; a previously tried IV continuation was not disguised as an authentic cadence.
The final plural return is Lead+Keys, avoiding an unintended ninth-in-bass floor.

Stock Swiss Signal declares no sevenths and therefore correctly **refuses** this V7
calibration. The comparison example explicitly creates A-Aeolian, 108-BPM, sevenths and
mixture-enabled variants using each world's timbres. These are named comparison settings,
not modifications of `MusicWorld::swiss_signal()` or claims of native world equivalence.
An incompatible hybrid is an invalid comparison, not an ear rejection.

The optional path currently uses `actions: false`, Independent coupling, no post-hoc
repair and argument-owned narration. Generic ensemble narrative is refused on this path.
Do not casually enable discretionary edits or re-mode a chromatic source and retain the
old source-identity claim.

## Unfinished legacy receipt bridge is a real integration debt

The new source/occurrence IDs are not forged into historical ActionId/MaterialId/
ObligationId spaces. Consequently legacy coarse commutation and `PerformanceReceipt`
remain separately measured and are **not green merely because the source witness passes**.
This path is not advertised as an accepted `perform_checked` take.

Across the 12 rendered A/B positives reported by the acceptance run, independent source
witnesses passed. Existing coarse commutation reported **5 divergences for A**, **7 for B**,
and `stale = false` for both. General receipts failed the incomplete identity bridge.
All six B comparison cells also reported one unclassified written pitch; A reported zero.
These limitations are recorded, not hidden by invented chord-tone tags or a target copied
from the observation. The preserved initial render reports name their working source rather than pretending to have been built at the later commit. `BUILD_SOURCE.json` fingerprints the final replay binary's compiled files; the delivered provenance compares those files with the final committed tree. The general receipt failures remain actual measured outcomes, not green pending work.

The next smallest decisive engineering step is an honest optional **read-side projection**
of the timed source/dependency graph into existing SongMap/performance receipts. Preserve
pitch units and functional quality; separate any genuinely incompatible old law from a
missing projection; do not counterfeit IDs or silently relax historic defaults to get
green. Keep the independent source witness as a separate observer.

## Changed-file inventory and likely integration conflicts

| File | Purpose / integration note |
| --- | --- |
| `src/audio/human_music/argument.rs` | New typed source graph, bounded transformation laws, A/B generation/compiler and functional preparation |
| `src/audio/human_music/argument_transport.rs` | New mandatory timed-source transport, per-carrier receipts, atomic refusal and original Rick phrase controls |
| `src/audio/human_music/argument_witness.rs` | New independent raw-note/source/harmonic witness and separate reject-only PCM exposure gate |
| `src/audio/human_music/functor.rs` | Existing production realization seams; optional `perform_argument`; highest conflict risk with concurrent Claude pipeline edits |
| `src/audio/human_music/mod.rs` | Three new module exports; likely small conflict if Claude adds modules |
| `tests/audio_argument_collisions.rs` | Seven historical boundary falsifiers from checkpoint 1 |
| `tests/audio_musical_argument.rs` | Finite A/B contrasts, semantic/order/carrier/harmony/metadata mutations, world refusal and PCM controls |
| `examples/humanmusic_argument.rs` | Reproducible original A/B takes, old C137 control, stems, counterfactuals and raw evidence manifest |
| `scripts/humanmusic/blind_argument_packet.py` | Standard-library-only PCM16 RMS matching, opaque clip IDs, private key and blind public zip |
| `docs/HUMAN_MUSIC_MUSICAL_ARGUMENT.md` | New APIs, implemented contract, scoped mathematical laws, boundaries and future C ABI inventory |
| `docs/research/HUMANMUSIC_SEMANTIC_BASELINE.md` | Exact-base source quotient and observed legacy failures |
| `docs/research/HUMANMUSIC_ARGUMENT_MUSICAL_AUDIT.md` | Independent musical audit, hostile findings and their repairs |
| `docs/research/HUMANMUSIC_SEMANTIC_RESURRECTION_HANDOFF.md` | This integration/continuation handoff |

There are no new mandatory sample, device or cloud-model dependencies; no Cargo dependency
changes; no package version or ABI changes; and no silent default changes. Existing
SongMap/PerformanceOptions layouts remain intact. Future C ABI parity needs versioned owned
representations of referents/events, occurrence IDs, rational licenses, completion and
dependency records, carrier sets, unresolved endings and separate evidence/error types.
Do not export Rust vector/enum memory layouts or attempt the entire parity roadmap now.

For functor conflicts, preserve both source order and the historical `None`-argument path.
Do not replace the integrated source seams with an after-the-fact Score patch: lead, keys,
bass, pad and drums must hear the actual earlier material. Keep authored occupancy aligned
with replacement and clear obsolete phrase-expression evidence. Changes in reference
arrays, world vocabulary or identity observers need separate review and rerun controls.

## Reproduction and evidence ledger

Run from the repository root on the exact checked-out head. The minimum supported compiler
remains Rust 1.85.0; CI uses stable. All-feature clippy on Linux needs the project's existing
optional ALSA headers. This is not a new mandatory default-build dependency.

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --no-deps
cargo test --test audio_argument_collisions
cargo test audio::human_music::argument::
cargo test audio::human_music::argument_transport::
cargo test --test audio_musical_argument
cargo test --test audio_source_contracts --test audio_semantic_consolidation --test audio_cover_contracts
cargo build --examples
cargo test
cargo build --release
cargo +1.85.0 test --test audio_argument_collisions --test audio_musical_argument
```

The targeted transport controls independently recover the original 24-event Rick verse,
including chromatic pitch, onset, sounding gate, accent and gaps. They reject wrong pitch,
rhythm, gate, accent, tail reduction, infeasible sources and missing context; count-neutral
replacement and all required carriers are controls that pass. They establish transport of
that source, not the generation or ear acceptance of a new argument.

```sh
cargo run --release --example humanmusic_argument -- --out=target/humanmusic-argument --source-sha="$(git rev-parse HEAD)" --sample-rate=24000 --seeds=2112,770919 --worlds=black_ice,vapor95,swiss_signal
python3 scripts/humanmusic/blind_argument_packet.py target/humanmusic-argument/manifest.tsv target/humanmusic-blind --seed=20261009
cargo run --release --example rick_probe -- --world=black_ice --out=target/humanmusic-handcrafted
```

The unchanged `rick_probe` render is a handcrafted calibration reference, with a different
chart/length from the matched A/B cells. Do not treat it as a causal A/B with the generator,
or invent the hash/settings of a previously ear-accepted archival file. The full audition
manifest names source SHA, family, seed, world, tempo, condition and stem. Voice-continuation edges in source-removal controls are retained whenever both endpoints survive; unrelated backing lifetimes must not be changed by thematic ablation. Inactive `prov.family` metadata is PCM-inert in a full generated song. `role_note` and `motif_id` participate in VoiceEventId and are not universally inactive metadata. Full mix, Lead,
Keys, Bass and Drums stems, source-only exposure controls, wrong answers, scrambled sources
and missing-carrier counterfactuals are emitted. Nonlinear master processing means stems
need not sum exactly to the full mix.

The blind pack uses a single common PCM16 RMS target, globally reduced to prevent peak
clipping with linear gain (no nonlinear limiter), and randomized opaque filenames. This controls level advantages, not perceptual
loudness in all timbres. Keep the answer key outside the public listening zip and do not
read it before recording perceived relationship, supporting moments, confidence, musical
merit and ACCEPT/REJECT/AMBIGUOUS. Tempo/rate/duration mismatches inside comparison groups
are refused. The packet does not supply or infer a listener verdict.

| Evidence | Recorded status / final fill |
| --- | --- |
| Exact-base historical collision suite | **Observed: 7/7 passed**, Rust 1.85.0, isolated `fbdbff` worktree |
| Current optional source/transport/acceptance tests | **PASS:** compiler 6, transport 8, collision 7 and original acceptance 11; updated acceptance 12/12 (19 collision+acceptance tests) on stable and Rust 1.85.0 |
| Full Cargo suite and historical fingerprint/cover/R17 regression | **PASS:** captured C2 full stable suite 1,622 tests, 0 failed, 19 intentionally ignored; production code unchanged since C2. C3 adds one full-song PCM test, separately passed. FX/UI example tests 19/19. Exact final-tip hosted CI independently runs the complete updated suite. |
| fmt / strict clippy / denied-warning rustdoc | **PASS:** final fmt; stable strict clippy all targets/all features; warnings-denied rustdoc. C3 hosted CI run 38010066845 failed only a helper-format omission, corrected in this checkpoint; skipped downstream jobs are not counted as passed. |
| Release/examples/MSRV checks | **PASS:** all debug examples before the instrumentation-only change; final release library and humanmusic_argument/rick_probe/rick_story; Rust 1.85 library and argument example checks, plus 19 targeted tests. No new mandatory dependencies. |
| Rendered artifact inventory, sample hashes and reproducibility | `33 exact-base historical WAVs (9 independently replayed byte-identically); 114 generated comparison/stem/control WAVs; 66 anonymous blind mixes. Initial same-binary replay: 57/57 byte-identical. Corrected final replay: 114/114 WAVs byte-identical across both seeds and all three comparison worlds. The original render manifests/evidence are preserved; final compiled-file reconciliation is recorded in delivered provenance. New cells use 24 kHz stereo PCM16, 108 BPM, 128 beats; historic controls use their separate recorded conditions.` |
| Exact-head GitHub CI | CI is assessed at the final PR #95 head and captured in the delivered `ci.json`; no historical/base CI is substituted. The final run starts when this checkpoint is pushed |
| Maintainer listening | **UNVERIFIED**; no new ACCEPT verdict |

The local validation records have code inventories and command-specific before/after digests. Production source is unchanged after C2; C3 and this checkpoint change only the new example/tests, formatting and packet/document wording. A local pass and a hosted exact-head CI result are separate facts. If old MSRV lint failures are encountered,
compare against the identical baseline before assigning them to this change; do not
describe a command that failed as green. Shared-target link corruption and an incorrect provisional example invocation were kept as transient diagnostics; final checks used isolated targets with CARGO_INCREMENTAL=0.

## Recommended integration sequence

Refresh refs and confirm whether Claude's active C137 branch moved after `fbdbff`.
Use an isolated integration branch and inspect the ancestry before applying changes.

```sh
git fetch origin
git switch -c integrate/humanmusic-semantic-resurrection origin/feat/v0.5-humanmusic-beefup
git cherry-pick beeab460986c28f880aa932935b956b4c437cbae
git cherry-pick 2b72f00e387d4f59abc36d8e01760fb735119136
# Inspect post-code fixes/evidence, then apply their ancestry order:
git log --reverse --oneline 2b72f00e387d4f59abc36d8e01760fb735119136..origin/impl/humanmusic-semantic-resurrection-2026-10-10
git cherry-pick 2b72f00e387d4f59abc36d8e01760fb735119136..origin/impl/humanmusic-semantic-resurrection-2026-10-10
```

If integrating from the exact `fbdbff` base, merging the complete implementation branch
into an isolated review branch is equally coherent. If the C137 branch moved, use the
ordered audit → code → handoff checkpoints, resolve functor/module seams explicitly, then
run the commands above and new exact-head CI. This instruction does not authorize merging
the review branch into main.

Continue first with the honest legacy receipt bridge and the maintainer's blind listening
verdict. If the naked source is ambiguous, improve the small source grammar/discourse
primitive; if source-only is recognizable but the mix is ambiguous, inspect arrangement
salience/masking; if raw events are missing, repair the source seam. Do not automatically
add operators, note density or the unsupported C family to explain an unresolved verdict.
