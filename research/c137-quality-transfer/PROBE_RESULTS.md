# C137 transfer probes — executed evidence, 2026-10-09

Read [the audit and recovery ladder](README.md) and [the executable probe](probe.py).

**Source snapshot:** `229b397b46a800342a58ac5c38c8bbad3d2cfb87` on `feat/v0.5-humanmusic-beefup`. **Scope:** isolated Python equations and a transcribed, source-inspected handcrafted phrase. These results are NOT execution of the Rust implementation, an end-to-end musical test, or a human listening verdict.

## Commands actually executed in the audit environment

The probe was written and executed locally before being committed. Local directory: `/mnt/data/c137-transfer-audit`. Its bytes were then published through the GitHub connector and the returned Git blob ID was compared with the locally computed ID.

```text
python /mnt/data/c137-transfer-audit/probe.py \
  --out /mnt/data/c137-transfer-audit/probe_results.json
# exit 0; 9 tests run, 0 failures, 0 errors

python -m py_compile /mnt/data/c137-transfer-audit/probe.py
# exit 0

python /mnt/data/c137-transfer-audit/probe.py \
  --repo /mnt/data/c137-transfer-audit \
  --out /mnt/data/c137-transfer-audit/source_drift_negative_control.json
# exit 2, as intended: this directory contains NO repository source files
# status = SOURCE_DRIFT_REVIEW_REQUIRED
```

The missing-checkout run is a deliberate negative control: it demonstrates that the script does not fabricate source verification from a directory name or missing files. In the successful equation-only run, source verification is explicitly `NOT_RUN_NO_CHECKOUT`.

**Probe identity verified after publication:**

- Git blob SHA-1: `3cd18e933b34e0ba32b85aa988e0ff975aabc630`
- SHA-256: `d9994306b975e2d496b769a0c0a8a18fa4d99f3814abc3a3db0e9a93b858134d`
- UTF-8 bytes: 9,034

## Raw unittest result

```text
test_count_delta_does_not_detect_valid_replacement ... ok
test_diagnostic_leap_collapses_on_tetrad ... ok
test_duration_chain_loses_teacher_onsets ... ok
test_explicit_event_payload_roundtrips_teacher ... ok
test_legacy_quote_is_not_full_carriage ... ok
test_ordered_modulo_is_still_noninjective ... ok
test_pitch_only_pass_cannot_read_germ_rhythm ... ok
test_teacher_has_explicit_rests_and_accents ... ok
test_using_onset_gaps_instead_changes_gates ... ok

Ran 9 tests in 0.000s
OK
```

The passing tests certify the asserted counterexamples in this small script, not the claim that the production engine is repaired. Python execution and source inspection are separate evidence routes, and share the same transcribed definitions where indicated.

## Results and precise interpretations

| Probe | Observed result | What follows, within scope |
|---|---|---|
| Handcrafted VERSE payload | 24 notes; 3 internal gaps totaling 2.5 beats; 11 distinct velocities | The accepted source contains timing/articulation detail absent from the two-motif seed interface. |
| Encode note gates as Motif spacing | Maximum onset shift 2.5 beats | Using one vector as both authored onset spacing and note gate cannot preserve this fixture. |
| Encode inter-onset gaps instead | Starts preserved; sounding durations gain 2.5 beats in total | Correct onsets alone do not establish correct note-off times or silence. Later heuristic gate/rest processing is not a lossless source encoding. |
| Explicit onset/gate/accent event payload | Exact JSON roundtrip of all 24 events | A richer representation can preserve this finite fixture; this is NOT proof that the current Rust pipeline already accepts it. |
| Quote's final-four crop | At most 4 of the 24 source events survive, before room clipping | A legitimate short-response transform cannot serve as an unqualified full-phrase transfer. |
| Ordered modulo collision | `[0,1,2,3]` and `[0,5,2,7]` map to identical tetrad indices | Checking the ordered output of a lossy map does not prove the original interval sequence survived. |
| Latest diagnostic seed on illustrative C-major tetrad | `[0,4,7,6,4,4]` maps to pitches `[0,0,11,7,0,0]` | The initial source leap becomes repetition. The test uses a declared example chord, NOT a claim about a particular actual rendered bar. |
| Hold backing fixed, change germ rhythm | Isolated pitch-only post-pass output unchanged | That post-pass does not transport the source rhythm. Upstream interactions may still respond indirectly; whole-engine rhythm independence is NOT claimed. |
| Same-count carrier replacement | Four backing notes replaced by four correct thematic notes, delta zero | A positive note-count delta is not a necessary condition for successful musical carriage. |

## Reproduction from a real checkout

```bash
# At the audited source snapshot or the audit branch, whose runtime sources are unchanged:
python research/c137-quality-transfer/probe.py --repo . --out /tmp/c137-transfer-probes.json
```

With `--repo`, the script compares eight source files against their pinned Git blob hashes before reporting its narrow static profile-wiring checks. If newer work changes those files, it returns `SOURCE_DRIFT_REVIEW_REQUIRED`: inspect the differences and update the audit deliberately; do not weaken the guard merely to get a green report. These static text checks are not a substitute for compiling and testing the effective Rust profile.

## Separate hosted evidence

The GitHub connector reported **CI run [38004209581](https://github.com/femboy2112/libgibson/actions/runs/38004209581), completed/success**, for the exact music source SHA `229b397b46a800342a58ac5c38c8bbad3d2cfb87`. This supersedes the old rustdoc failure described in earlier discussions. It proves only the scope of that hosted workflow; it does not prove the new research assertions against the live Rust generator or the musical target.

## Unrun evidence and next instrument

This environment had no usable Cargo/Rust toolchain, and direct container access to GitHub failed DNS resolution. Repository reading and writing succeeded through the GitHub connector. No Cargo build/test, actual seeded-vs-handwritten render, waveform comparison, benchmark, or independent audio audition was run locally. Source-aware checks were not executed on a full local checkout.

**Next decisive run:** execute the effective-profile four-cell Rust audition (narrative OFF/ON x lead development+dynamics OFF/ON), then import one actual reference phrase through a reusable timed-source adapter and compare the declared event payload at each compiler/realizer boundary. Preserve both the original teacher audio and the controlled candidate audio. Human acceptance remains pending until the maintainer hears the actual files.
