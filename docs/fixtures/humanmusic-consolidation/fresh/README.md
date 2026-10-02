# Consolidated implementation holdout v1

Status: **configuration and harness committed after the architecture froze at `3216021` (repair `1a06575`); musical cases not executed at commit time.** First-contact results are recorded separately under `results/`. Do not run this fixture as part of ordinary development, automatic ignored-test discovery, or tuning. Preserve every first-contact receipt, including failures and panics.

The declared configuration SHA256 is `c45fa6de3a441d8549870750c0431a67224044871a5e81bfa9fe0cb8935800ee`. The imported `docs/fixtures/humanmusic-cover/ode-import/reference.tsv` SHA256 is `4724a1e4f9e3202578aa3b7156f603ec63b90d17a1770e8fc7a9d77d04bd8678`; freeze that committed input with the harness.

`config.tsv` declares exactly **36 cases: 30 generated sources and 6 external Ode melody cases**. A case creates at most one generated source and one cover candidate. It never tries a replacement seed, profile, world, or selection after failure. The source and target realization policy is the explicitly selected `PerformanceProfile::POCKET`; this does not change the library default. The harness is `tests/audio_consolidated_holdout.rs`.

Coverage declared before contact:

- Ten generated sources in each of BLACK_ICE, SWISS_SIGNAL and VAPOR95; all three target worlds, including same-world and cross-world lifts.
- StructuralR9, MeaningDirected and StablePropulsion source composers; HookArc, LoopEvolution, RiffDrive, DeflectedLift and PropulsiveReturn grammar choices. StablePropulsion rows explicitly declare the PropulsiveReturn grammar selected by that composer.
- Demo and deflected traces; 4.5–32.5 beat generated pieces, with short and partial final bars. Source and target tempos include both named-world nominal values and explicit alternatives.
- Six generated cases each for contract default, default with Groove freed, all axes, motif-only, and explicit Riff/BassFigure/Groove/Form. Both FusionConversation and Simple performance languages, straight and swung worlds, and explicit target tonic changes.
- Six covers of the already imported external Ode soprano: every world at nominal and altered tempo, several target keys and both languages. Only the observed motif is selected; unobserved harmony, groove, form, bass and orchestration stay Unknown. No Ode row requests `all`, so the all-axis Unknown behaviour is not exercised by this set.

There are 66 distinct seeds above ten million (30 source seeds and 36 target seeds). A static exact-token search found none in `src`, `tests`, `examples` or `docs` before this configuration was written. This is a repository-scope novelty check, not a claim that no private experiment ever used one. The configuration contains actual tab separators and 17 fields per row.

## Checks and failure retention

The harness uses `cover_candidate` so an admission-red candidate remains available to all observers. It does not invoke checked `cover` a second time or regenerate the plan. Source failures do not prevent extraction and cover generation when a source object exists. Source-generation and selection errors remain failed cases; downstream claims are unavailable when the prerequisite object does not exist.

Observers run under separate panic boundaries and record all independent failures they can evaluate:

- Score domains, SongMap projection, pitch-function presence, temporal-function claims, held-identity flips, action receipts, stage boundaries, stale hearings, and required causal-ledger coverage.
- Continuation graph identity and the direct-voice law: unlinked notes retain their envelope lifetime; only explicit matching edges can reduce it to the linked destination plus choke.
- Exactly one Lead and Bass semantic owner, reservation protection even with an intentionally empty acoustic input, and non-yielding owned/rest/action span interiors.
- The shared occupancy contract independently checks missing authored attacks, exact pinned canonical reservations, and reservation/span domains. This does not replace the separate empty-acoustic and owned-midpoint mutations.
- Surviving phrase destinations, selected-map validity and availability, per-axis cover conformance, canonical metric extent, and some remaining free realization versus generated sources.

The pipeline constituent checks use the same existing diagnostic authorities as `CoverPipelineReceipt`, evaluated separately so a panic does not hide sibling results. Stage admission uses `orchestration_violations`, including its existing percussion boundary convention; exact performed-onset disagreements are also retained as explicitly diagnostic information. No new onset tolerance is fitted. There is no R16 pitch-equality requirement and no mistaken rule that every authored structural note must lie on a world's transported hat lattice. Existing temporal and held-identity thresholds are unchanged.

Every row is appended and flushed before the next operation. Output includes an exact copy of the executed configuration, one TSV receipt per case, and `summary.tsv`. Fields use escaped tabs/newlines; failures retain full diagnostic detail. The test returns failure if any case has a failing receipt. The output directory must not already exist, so a rerun cannot overwrite the first evidence. There is no render, WAV comparison, quality score, or human-listening claim in this harness.

## Pre-freeze harness review

Before the configuration commit and before any case executed, an adversarial read-only review of the harness (no execution) found, and root corrected, the following. `config.tsv` rows were not changed, so its declared SHA256 above still holds.

- **Composer self-report.** `SongMap::composer()` infers `StablePropulsion` from the PropulsiveReturn phenomenal target, so rows G14, G19, G23 and G28 (StructuralR9/MeaningDirected composer on the PropulsiveReturn grammar) would have failed a `composer() == declared` check regardless of the architecture. The check is now `declared-grammar`; the declared composer is the call authority and the self-report is retained as detail. This is a recorded library provenance ambiguity, not a musical result.
- **Execution profile.** Under debug assertions `realize_policy` asserts the stage boundary itself, so a stage failure would become a generation panic erasing every sibling observer, and the `stage-boundary` observer could never fail. The declared profile is therefore `--release` (the known R17 replay also ran release), and the harness refuses to run with debug assertions before creating any output. `provenance.tsv` records the profile, package version and configuration/fixture fingerprints.
- **Structural guards.** `guard:map-validation`, `guard:direct-voice-lifetime`, `guard:semantic-occupancy`, `guard:hearing-ledger-coverage` and `guard:anti-replay-vs-source` either repeat a check the library already made or can only pass by construction. They are regression guards, not independent evidence, and are labelled so.
- **Unexercised claim removed.** No Ode row requests `all`; the README no longer claims that behaviour is tested.

Retained risks, declared before contact: there is no per-case timeout (the process runs under an outer `timeout`, and every row is flushed before the next operation); swung VAPOR95 pocket drums may fall outside the legacy stroke projection window and fail Groove-pinned extraction; a 4.5-beat source has never been lifted.

## Execution boundary

Compilation alone is permitted before freezing and does not execute any case:

```sh
CARGO_BUILD_JOBS=1 cargo +1.98.1 test --release --test audio_consolidated_holdout --no-run
```

Only after the architecture and this configuration are committed, root may execute once with a new output directory and preserve the complete process log and exit status:

```sh
HUMANMUSIC_HOLDOUT_OUT=target/humanmusic-consolidation-holdout-v1 \
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 \
cargo +1.98.1 test --release --test audio_consolidated_holdout \
  consolidated_fresh_holdout -- --ignored --exact --nocapture
```

Record the configuration SHA256, source commit, toolchain, command, output path, case totals, all failed families and stdout/stderr with the result. Do not change source behavior, gates or configuration after observing these results. If a catastrophic universal defect requires a repair, first preserve its counterexample and declare a second untouched holdout for the repaired implementation.

## Independence limits

The generated-source cover path was repaired (`1a06575`: no groove debt for a pinned kit) after the cover lab exposed `unwitnessed_song_obligations: 1` on the lab's own seeds 901/902. That defect, its diagnosis and its falsifiers never touched this configuration's seeds, worlds or rows; the repair preceded this configuration's commit and first contact. This is fresh input coverage for the consolidated implementation, not an independent implementation or blinded human audition. The generator and most observers share repository provenance. The external Ode symbolic fixture and its parser have already been inspected and used for calibration; only these new target seed/world/language/key/tempo combinations are held out. The old R17 120-case sweep is known data and is not part of this fresh set. The finite, deliberately selected coverage matrix is not a statistical population sample and proves no universal generalization. No result establishes whether a person recognizes or likes a cover.
