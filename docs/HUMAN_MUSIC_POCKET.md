# HumanMusic Round XVII — the pocket

The mission began at `c9242ff154fc60d9321995126af12f18574dd88e`, verified clean and equal
to fetched `origin/feat/v0.4-humanmusic-audio`. Main/base remains
`7b4efdc4c7fc6523d890a28f2aa8e27a6ba59286`. PR #70 stays draft. Package 0.3.1,
ABI 1, MSRV 1.85, Rust-only synthesis, release gates and default arm are unchanged.

**Round verdict: experimental candidate, general acceptance NO-GO.** The seed-2112
BLACK flagship passes the frozen lattice/blip and source-consumer checks. The fresh sweep
at production source `f886e16008b8b87315270a7d4ca7bc51f6efd52c` is **red: 61/120
complete passes, 59 failed cases**. The failed holdout is preserved without fitting or
threshold changes. Engineering green does not erase these failures. SWISS support remains
the maintainer-requested no-go; BLACK's flagship pocket is now human-accepted, with residual grid-like/robotic feel.

## Human evidence and the support stop boundary

**Observed, human, final audition:** after hearing `black_ice_r17.full.wav`, the maintainer
reported: “in the pocket, much better, but still a little grid/robotic. this is a good place
to call it for this round, but make note. good work”. This accepts the flagship pocket and
closes Round XVII's listening work. It does not accept every fresh-sweep case or erase
its counterexamples. The residual mechanical feel is recorded for a future round; no
feel-offset fitting follows this verdict.


**Observed, human:** R16 BLACK's pitches were coherent, but its timing sounded like a
musician stopping to locate a fret before rejoining the band. SWISS was nearly accepted,
with a recurring in-key horn-like intrusion at 28/36/44. These are independent acceptance
bearings; diagnostics cannot override them.

The maintainer compared the R16 beat 28 single-note knockouts and reported:
“shit sound in 0031_m79, 0028_m60 sounds in tune.” The first removes pad G5; the second
removes pad C4. **Observed:** removing G5 did not remove the unwanted sound; removing C4
produced the locally accepted result. **Conjectured:** low-root involvement is the leading
source explanation. C4/B2 contact, masking and release/timbre interaction remain distinct
mechanisms. This is not new listening evidence for beats 36/44.

The suggested `C4 G4 B4 E5` voicing retains the implicated C4. It was not promoted from
within-song resemblance. The source-exact [finite search](fixtures/humanmusic-r17/support-no-go/README.md)
checks 58 existing octave alternatives per site, including 32 that remove C4. Zero pass all
unchanged guards at any of the three sites. Eight pass the band/tail guards at 28, but each
raises the top to B5 or higher; none passes those guards at 36/44. The old high-seventh
excursion witness has a blind spot when an alternative puts C5 at the bottom: it can stop
flagging B5 without demonstrating acceptance. No threshold was weakened.

**Boundary, explicitly requested by the maintainer:** retain this support no-go and finish
BLACK and the receipts. `support_top_voice` remains a declared but unimplemented treatment;
its duplicated factorial cells are not evidence that a real support change would have no
effect. SWISS's proposed listening arm stays exactly R16. No three-note pad was promoted.

## Frozen falsifiers before production

Commit `8974dfc` precedes every production edit. The [baseline](fixtures/humanmusic-r17/baseline/witnesses.txt)
and [contract](fixtures/humanmusic-r17/experiment-contract.txt) are reproduced by
`scripts/dev/humanmusic_r17_witnesses.py`, independently parsing the frozen text receipts.

| Frozen score | Off-lattice pitched onsets | Roles |
|---|---:|---|
| BLACK R14 |0|none|
| BLACK R15 |34|17 lead,16 bass,1 keys|
| BLACK R16 |15|7 lead,7 bass,1 keys; all at x.625|
| SWISS R14/R15/R16 |0|none|

**Verified, finite score measurement:** straight BLACK/SWISS use subdivision 4 and swing 0;
no pitched jitter allowance is introduced. Drums retain seeded ±0.008-beat timing. The
lattice observer calibrates straight, swing-transported, conditionally licensed triplet,
and small illegal pitched-offset cases. All 84 historical WAV hashes were recomputed.

The complete 35-row lead/bass retiming table records onset, gate milliseconds, written
silence to the next same-role target and before/after legato ratio. Thirty connective
rows fail the frozen 40-ms written-gap witness; five stable precursor rows are reported
separately. R14's authored rests are controls, not retimed-connective failures. Written
gate silence is an articulation measurement, not a claim of master-PCM silence.

## Source causes and changed coordinate system

**Verified, source:** R16's
[`PhraseGrid`](https://github.com/femboy2112/libgibson/blob/c9242ff/src/audio/human_music/phrase_expression.rs#L26)
enumerated durations, including 0.125 and 0.375 beat, then subtracted
multiples from a destination
([preference at lines 371–380, construction at 403–414](https://github.com/femboy2112/libgibson/blob/c9242ff/src/audio/human_music/phrase_expression.rs#L371)). A dotted-sixteenth duration generated a 32nd-note position
in a straight 16th world. Its 0.40×step connector gate reproduced short-blip morphology.
The old
[expression observer, lines 271–316](https://github.com/femboy2112/libgibson/blob/c9242ff/src/audio/human_music/expression.rs#L271)
included a release tail the synth did not choke; R16's
[polyphonic allocator, lines 708–723](https://github.com/femboy2112/libgibson/blob/c9242ff/src/audio/human_music/synth.rs#L708)
allowed that tail to overlap the target. These historical paths
remain available unchanged for byte-exact controls.

Round XVII introduces indexed `LatticeSlot` values from a shared beat-zero/bar origin.
It enumerates the groove first, transports through the declared swing rule, restricts
the source corridor, then orders candidates lexicographically by decomposed source phase,
IOI contour/displacement, shared AccentGrid/pickup evidence, kinetic context and previous
gesture information. Structural/local targets, action anchors, stage boundaries and
source-to-destination order remain hard guards. The physical ruler only admits/rejects;
it cannot manufacture a position. Tuplet indices are normalized by their own denominator:
index1 of a triplet is not index1 of a sixteenth lattice.

The two-viable-slots falsifier uses the same local pitch path on a short SWISS patch:
24.5 and 25.0 beats both belong to the lattice and pass physical validation before a fixed
25.5 target. The selector preserves the authored 24.5 phase/IOI, instead of picking the
nearest permissible latency. A second hostile diagnostic demonstrates zero lattice
residual with a one-sixteenth phase error. This distinguishes the laws mechanically;
it does not prove that every new phrase has a good pocket.

Optional transformed connectives use 0.97 of the actual next-event IOI. Velocity contours
remain R16's. Explicit Push/Hit articulation remains anchored. Stable precursor gathering
has its own `stable_precursors` switch, off by default, with adjacency and movement each
bounded to one subdivision. The old full-beat relocation of a stable E is not silently
carried into this arm. Unplaceable optional events yield source-owned space; R15's
seconds-based fallback never generates an onset in the lattice arm. An ablation that
cannot express required unison content retains a visibly unresolved physical obligation,
rather than deleting it and triggering an invented replacement figure.

## One authoritative explicit voice relation

Role is an instrument family, not proof of one logical voice. Source phrase plans declare
`VoiceContinuation` edges between specific optional connectives and destinations. Stable
event identities survive arrangement-description stamps and final gate clipping; missing,
ambiguous or conflicting endpoints are rejected. A bass unison receives its own linked
endpoints. Independently authored simultaneous same-role voices are not killed.

The shared direct-voice law is
`effective_end = min(envelope_end, linked_destination + 15 ms)`.
Only explicit edges authorize the deterministic output ramp; there is no pitch glide.
The PCM scheduler, expression/contact observer, score-aware temporal mass, heard identity,
sounding tension, sonority and source support trials consume that law. Historical entry
points retain their original masking/envelope semantics. The score's old event fingerprint
stays frozen; it does not encode this new realization contract, so receipts separately
record continuity edges, the physics flag and WAV hashes.

Calibration checks cover a long release, exactly 720 choke samples at 48 kHz, repeated choke
requests, block sizes 1/128/257/511/4096, missing/ambiguous endpoints and two simultaneous
logical voices of one role. One voice advances while the unrelated one remains alive.
A reused choked voice starts a fresh envelope rather than resuming old sustain. Consumer
checks compare expression/mass/identity/sonority windows against the shared endpoint.
**Boundary:** shared downstream reverb can outlive the direct voice; the envelope model
and generated tests share implementation provenance and are not listening evidence.

## Opt-in source architecture and consumer falsifiers

```text
SongMap → PerformancePlan
  → authored lead reservations
  → indexed groove slots → phrase selection → physical admission → final lead
  → keys hear final lead and read original lead ownership
  → bass hears final lead/keys; authored bass intent reads original lead reservations
  → final bass plus explicit source continuity edges
  → pad trials hear final band using the same effective-lifetime law
  → drums hear final lead/bass and retain authored pulse reservations
  → Score + explicit continuity → linked direct-voice PCM → unchanged production bus
```

`perform_pocketed` opts into the new source arm. `perform_pocket_experiment` independently
toggles `lattice_positions`, `legato_connectives`, `mono_voice`, `support_top_voice` and the
separate precursor A/B. All primary factors off reproduce R16. Historical/default arms
never read the new options. No finished Score is repaired.

Two integration falsifiers found dependency defects before arm admission. R16's bass
reservation before the unison at 84 inherited the final lead timing: its end was 84.59375,
where the original authored source ends 84.475. The new arm snapshots the complete authored
bass reservation stream from original lead intent, while acoustic hearings still contain
only final notes. In the lattice+legato/no-mono ablation, omitting a pickup reduced the lead
unison to one member and activated the consumer's canned replacement cell, changing a keys
pitch. The source now protects that shared figure and records `RetainedObligation` with its
unchanged failing physical observation. Neither correction manufactures a passing exposure
receipt. The full arm and 32 primary cells passed the resulting source/consumer checks.

## Diagnostics and factorial interpretation

Each transformed phrase has separate lattice residual, source displacement, source/performed/
destination indices and phase, before/after IOI sequences with structural neighbors, raw
AccentGrid weights, nearest actual drum onset including jitter, shared-action lead–bass
relative starts, repeated phase classes and consecutive transformed-phase runs. Missing or
ambiguous actual notes stay unresolved; source ledgers cannot stand in for a heard event.

The four-bit design is recorded in full for BLACK and SWISS; bit order is support/mono/
legato/lattice from high to low. Support is intentionally inactive under the accepted no-go,
so only three treatments are implemented. Retained physical failures in ablated cells are
reported independently of lattice, structural and action results. Mixed differences show
non-additivity of these measured fields, not a musical mechanism or a quality probability.
Stable precursor on/off is a separate comparison.

Implementation references at the frozen production revision:
[pulse lattice and selector](../src/audio/human_music/pocket.rs),
[source phrase realization](../src/audio/human_music/phrase_expression.rs),
[shared lifetime](../src/audio/human_music/voice.rs),
[PCM scheduling](../src/audio/human_music/synth.rs),
[opt-in entry points](../src/audio/human_music/functor.rs), and
[decomposed diagnostics](../src/audio/human_music/pocket_diagnostics.rs). **Observed:** the maintainer subsequently judged this BLACK full mix in the pocket and
much better. The remaining robotic feel is explicit; the mechanism and generalization
remain unestablished.


## Fresh coverage and preserved counterexamples

**Verified, failed finite experiment:** five seeds (3, 7, 19, 43, 101), two composers
(StablePropulsion, MeaningDirected), two stories (demo, DeflectedLift), three worlds
(SWISS_SIGNAL, BLACK_ICE, VAPOR95), and nominal/−12 BPM: 120 constructed performances.
VAPOR95 was withheld until the production source was frozen. Two earlier optimized
compilations were stopped before runtime; their logs do not count as executions.
The executed binary's source hashes and HEAD were verified immediately before contact.

| World | Complete invariant passes | First-assertion failures |
|---|---:|---:|
| SWISS_SIGNAL | 30 | 10 |
| BLACK_ICE | 31 | 9 |
| VAPOR95 | 0 | 40 |
| Total | 61 | 59 |

| First failure | Cases | Consequence |
|---|---:|---|
| Accepted pitch differs from R16 | 14 | General pitch preservation is not established |
| Off transported lattice | 38 | VAPOR's inherited unswung onsets conflict with the all-onset witness |
| New temporal function claim | 5 | All five BLACK demo/MeaningDirected cases at 76 BPM fail 0→1 |
| New held identity flip | 2 | VAPOR seed 43/DeflectedLift/StablePropulsion fails at both tempos |

The two identity counterexamples begin at beat 114.25: chart Fmaj6 is heard as rival Am
for 0.985 s at 71 BPM and 1.140 s at 59 BPM. The swing mismatch was noticed before first
holdout execution; the frozen observer was still executed unchanged. A structural onset
cannot be repaired to make that observer pass. These facts do not excuse the identity or
pitch failures, and do not turn the VAPOR holdout green.

Each failed case stops checking at its first assertion. Later invariants are **Not
Established** for that case. The family counts are not an exhaustive count of defects.
The complete [raw sweep](fixtures/humanmusic-r17/verification/fresh-sweep.txt),
[parsed first failures](fixtures/humanmusic-r17/verification/fresh-failures.json), and
[pre-contact source manifest](fixtures/humanmusic-r17/verification/fresh-source.json)
are retained. No source fitting followed holdout contact.


A read-only [three-case classification](fixtures/humanmusic-r17/verification/fresh-classification.md)
separates source edits from accepted-output differences. At BLACK seed 3/demo/MeaningDirected,
76 BPM, R14/R17 retain bass C2 at 63.5 after structural D2 at 62; R15/R16 substitute C♯2,
which supplies the held D2 resolution across the F♯7 change. Restoring the authored pitch
therefore restores a false temporal claim. At SWISS seed 3/DeflectedLift/StablePropulsion,
106 BPM, R17 retains authored F4 while R15/R16 had substituted G4. The accepted-output
pitch constraint still fails despite zero literal R17 source pitch edits.

At SWISS seed 3/demo/StablePropulsion, 106 BPM, swapping only the final lead in the keys
source reproduces an added E5 at 67.25. The changed lead occupancy alters the chosen stab
sequence; that onset changes from odd to even stab index, selecting three voices instead
of two. The candidate voicing path itself is identical. This explains a consumer pitch
set change without relabeling the failed preservation test as success. No production
repairs or parameter choices followed these probes.

## Exact flagship perturbations

**Verified, score comparison:** BLACK R14 has 449 notes, R16 has 446, and R17 has 449.
The following counts use the source ledger to match identities across retiming; categories
can overlap. Every row is expanded to before/after events in the
[final receipts](fixtures/humanmusic-r17/final/). “Added” means absent from that comparison
arm: the three bass additions relative to R16 restore R14 source events.

| BLACK comparison | Role | Retimed | Gate | Velocity | Pitch | Omitted | Added |
|---|---|---:|---:|---:|---:|---:|---:|
| R14→R17 | Pad | 0 | 0 | 0 | 0 | 0 | 0 |
| R14→R17 | Keys | 1 | 1 | 0 | 0 | 0 | 0 |
| R14→R17 | Bass | 16 | 19 | 15 | 0 | 0 | 0 |
| R14→R17 | Lead | 17 | 17 | 17 | 0 | 0 | 0 |
| R16→R17 | Pad | 0 | 0 | 0 | 0 | 0 | 0 |
| R16→R17 | Keys | 1 | 1 | 0 | 0 | 0 | 0 |
| R16→R17 | Bass | 7 | 14 | 2 | 0 | 0 | 3 |
| R16→R17 | Lead | 12 | 22 | 5 | 0 | 0 | 0 |

The SWISS listening R17 arm is the support-only no-op: all roles, drum events and PCM
remain R16 controls. Full pocket treatment on SWISS is nevertheless measured separately
in its factorial cells, and must not be mistaken for the proposed listening arm.

## Factorial results and audio hashes

**Verified:** all 16 cells per world are rendered. Bits read high to low as support, mono,
legato, lattice. The support-on counterpart of every row below is exactly identical in
these fields and in full-mix PCM; both halves remain in the receipts.

| Active bits mono/legato/lattice | BLACK off-lattice | BLACK blips | BLACK unresolved obligation | SWISS off-lattice/blips/unresolved |
|---|---:|---:|---:|---|
| 000 | 15 | 30 | 0 | 0/0/0 |
| 001 | 0 | 33 | 0 | 0/0/0 |
| 010 | 26 | 0 | 0 | 0/0/0 |
| 011 | 0 | 0 | 1 | 0/0/0 |
| 100 | 19 | 33 | 0 | 0/0/0 |
| 101 | 0 | 33 | 0 | 0/0/0 |
| 110 | 9 | 0 | 0 | 0/0/0 |
| 111 | 0 | 0 | 0 | 0/0/0 |

The complete [per-role factorial perturbations](fixtures/humanmusic-r17/final/perturbation-summary.md)
report retiming, gate, velocity, pitch, omission and addition separately relative to both
R14 and R16. The [mixed differences](fixtures/humanmusic-r17/final/factorial-interactions.json)
use Δ_S f(0) = Σ_{T⊆S} (−1)^(|S|−|T|) f(T), with exact inverse reconstruction checked.
For example, BLACK lattice×legato has blip mixed difference −3, while the three-active-factor
off-lattice mixed difference is +21. These are count non-additivities, not causal mechanism
identification or listening scores. Lattice alone leaves 33 blips; lattice+legato without
mono leaves one explicit unresolved obligation and omits 16 R16 lead notes (while restoring three
bass notes). Its zero blips
therefore is not positive acceptance. All three active factors clear these
flagship witnesses.

The stable-precursor switch changes no flagship score or full-mix bytes relative to the
otherwise full treatment. Its bounded gathering path therefore has no positive flagship
effect to claim; the separate A/B WAVs preserve this null result.

BLACK R17 has 0 held identity flips, 0 false temporal function claims, 0 stale hearings,
and 45/45 witnessed actions. The same values hold for SWISS’s unchanged listening arm.
These are flagship receipts, not the fresh-sweep verdict.

The [final SHA256 manifest](fixtures/humanmusic-r17/final/SHA256SUMS) covers all 82 WAVs:
48 main/stem renders (two worlds × R14/R16/R17 × eight stems), 32 factorial full mixes,
and two stable-precursor A/B mixes. All 32 rendered historical controls match R16’s
committed hashes. [Control comparisons](fixtures/humanmusic-r17/final/historical-controls.txt)
also verify both all-off full mixes and all eight unchanged SWISS listening stems.

| Full mix | SHA256 |
|---|---|
| `black_ice_r14.full.wav` | `cd056dbe7fb8fa0c4fe82c8f8463003ea9d48cc787f15c66a8519034d1a43548` |
| `black_ice_r16.full.wav` | `5a50584074e48374495624566969bfcb3f8c884795c83d4f2fb6182f6eee8c3a` |
| `black_ice_r17.full.wav` | `6fe3abf55cee22b69e6d3c06684574770eea40445de4183588cfd12707382f4d` |
| `swiss_r14.full.wav` | `5b56aea38500c2471e71b45aa438510a47c8081e87ba54909c6f967959cfc792` |
| `swiss_r16.full.wav` | `833fb91981b6afed7b596c6dec2ce6cc0096e17dc0f8f5f646c3d92b2f90e17e` |
| `swiss_r17.full.wav` | `833fb91981b6afed7b596c6dec2ce6cc0096e17dc0f8f5f646c3d92b2f90e17e` |

The [final human audition receipt](fixtures/humanmusic-r17/verification/human-final-audition.json)
binds the BLACK acceptance and residual robotic-feel note to the R17 full-mix hash above.

## Claim boundaries

| Status | Claim | Boundary |
|---|---|---|
| Observed | Maintainer accepted C4-muted b028 and rejected G5-muted b028 | Human contact at one window; does not identify the physical mechanism |
| Verified | Frozen BLACK lattice counts 0/34/15 and 30 R16 connective gap failures | Parsed score/written-gate evidence, not perceived silence |
| Verified | Pulse-indexed generation distinguishes two legal viable phases | Synthetic falsifier shares implementation environment |
| Verified | Linked predecessor dies under the shared 15 ms direct-voice contract | Downstream reverb persists; unrelated voices survive |
| Verified | Flagship/factorial source and interaction invariants pass | Seed 2112 only; global validity is refuted by the sweep |
| Verified | Fresh sweep fails 59 of 120 cases | First failure per case; no fitting after contact |
| Observed | Maintainer calls the BLACK R17 full mix in the pocket and much better | Residual grid-like/robotic feel; one flagship audition |
| Conjectured | The three active treatments explain the improvement | Human acceptance does not isolate their causal contributions |
| Not Established | R17 preserves accepted output pitches and held identity for all songs/worlds | Fresh counterexamples prevent this claim |
| Boundary | SWISS support treatment is not implemented | Retained no-go explicitly chosen by maintainer |
| Boundary | Four-bit support contrasts are identically zero by implementation | They do not measure a real voicing intervention |
| Boundary | Default arm, package, ABI, release gates, main and PR draft status remain unchanged | No merge, tag, release or promotion |

The scouts, envelope models, source probes and generated tests share repository provenance.
Their agreement is not independent listening evidence. The human BLACK timing judgment
and human SWISS intrusion judgment remain separate acceptance bearings. BLACK is locally
accepted; SWISS support remains a no-go. No one quality
score aggregates them. A per-player pocket-feel field, pitch glide, pad redesign and new
emotion model remain outside this round; feel offsets are parked as a possible Phase 2
after this accepted flagship; none is fitted in Round XVII.

## Engineering verification

**Verified:** production source remains byte-identical to `f886e16` after holdout contact.
The [source manifest](fixtures/humanmusic-r17/verification/fresh-source.json) covers all
Rust source plus Cargo/build inputs. The final reporting/probe changes do not alter it.

| Gate | Result |
|---|---|
| Full `--all-features` suite | 1,344 passed, 0 failed, 10 ignored |
| `fmt --check` | Pass |
| Strict Clippy, all targets/features | Pass, including final diagnostic example |
| Rustdoc with `-D warnings`, all features | Pass |
| Rust 1.85 library, locked/all features | Pass |
| Frozen fresh sweep, separately executed | **FAIL**, 61 passes / 59 failed cases |
| CI at `616a037`, identical production source | [Green, all six jobs](https://github.com/femboy2112/libgibson/actions/runs/36678510724) |
| Release Preflight at `f886e16` | [Green, publication-free](https://github.com/femboy2112/libgibson/actions/runs/36678220521) |

[Engineering commands and results](fixtures/humanmusic-r17/verification/engineering.json)
link the captured logs. [CI](fixtures/humanmusic-r17/verification/ci.json) ran at
`616a037`, which adds only frozen failure receipts to `f886e16`; all production source
hashes are identical. [Release Preflight](fixtures/humanmusic-r17/verification/release-preflight.json)
ran at `f886e16` itself. The normal suite intentionally excludes expensive ignored sweeps;
its green result cannot stand in for the separately executed red sweep. No test, release
threshold, workflow or acceptance comparator was weakened. Earlier intermediate CI runs
canceled by later branch pushes are not reported as passes.

## Reproduction

Use Rust 1.98.1 for matched audio. Production source authority is
`f886e16008b8b87315270a7d4ca7bc51f6efd52c`; later commits add evidence/reporting only.
From a checkout of the final receipt commit, these commands reproduce the artifacts:

```sh
python3 scripts/dev/humanmusic_r17_witnesses.py
cargo +1.98.1 test --test audio_pocket_integration
cargo +1.98.1 test --test audio_pocket_phase --test audio_pocket_voice
cargo +1.98.1 test --release --test audio_pocket_integration r17_fresh_world_seed_tempo_sweep -- --ignored --nocapture
cargo +1.98.1 run --release --example pocket_music_lab -- --out=target/humanmusic-r17/final --render --render-factorial
python3 scripts/dev/humanmusic_r17_receipts.py target/humanmusic-r17/final --require-audio --source-head f886e16008b8b87315270a7d4ca7bc51f6efd52c --copy-receipts docs/fixtures/humanmusic-r17/final
cargo +1.98.1 run --release --example pocket_knockout_lab -- --out=target/humanmusic-r17/knockouts
python3 scripts/dev/humanmusic_r17_knockouts.py target/humanmusic-r17/knockouts
cargo +1.98.1 run --release --example pocket_failure_probe
cargo +1.98.1 fmt --check
cargo +1.98.1 clippy --all-targets --all-features -- -D warnings
cargo +1.98.1 test --all-features
RUSTDOCFLAGS='-D warnings' cargo +1.98.1 doc --no-deps --all-features
cargo +1.85 check --locked --lib --all-features
```

The fresh-sweep command is expected to exit 101 with the preserved failures. Repeating it
is a regression replay, not a new holdout. Audio files are generated locally under
`target/`; committed SHA256 manifests bind them without committing large WAV binaries.
The 133 SWISS knockout WAVs include every sounding pad, keys and lead note separately
at b028/b036/b044 and control b060, plus four unmuted continuous-window baselines. Their
[manifest](fixtures/humanmusic-r17/knockouts/SHA256SUMS) and
[control comparisons](fixtures/humanmusic-r17/knockouts/control_checks.txt) are frozen.
Open `target/humanmusic-r17/knockouts/audition.html` for the indexed audition set.

## Review sequence

| Commit | Review purpose |
|---|---|
| `8974dfc` | Freeze lattice/blip falsifiers and historical hashes before production |
| `f82756b` | Render localization knockouts before support selection |
| `4b4e21f` | Explicit linked lifetime and PCM calibration |
| `9229a5c` | Shared support lifetime and finite SWISS no-go |
| `84cf1c1` | Pulse lattice, phase selector and independently gated articulation |
| `a16cbaa` | Opt-in source arm and consumer invariants |
| `3b1cae7` | Phase denominator/consecutive morphology witnesses and complete sweep accounting |
| `f886e16` | Complete lifetime consumers and listening harness; frozen production authority |
| `616a037` | Preserve failed fresh holdout without fitting |

All commits carry `Co-authored-by: ChatGPT Astra <noreply@openai.com>`.


## Listening disposition

- BLACK_ICE: with every ornament on the band's own lattice and slurred into its target,
  does the player now sit in the pocket? **Maintainer: yes, much better; still a little
  grid/robotic. Round closed here.**
- SWISS: which knockout stem removes the horn blast, and does the control voicing keep
  the arrangement already accepted? **Maintainer: the C4 mute at b028 sounds in tune;
  the G5 mute still has the unwanted sound. The proposed control voicing is not admitted.
  Support no-go retained by request; no claim for b036/b044 acceptance.**
