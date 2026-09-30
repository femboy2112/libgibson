# HumanMusic — merge readiness

This document exists so the maintainer can decide whether `feat/v0.4-humanmusic-audio` goes to `main`.
It does **not** decide that. Green engineering gates are necessary, not sufficient: the listening
questions at the end are open, and the reasons to postpone are listed without softening.

Labels: **Observed** (read from a committed receipt or a command run at the stated source),
**Corroborated** (two independent routes agree), **Conjectured**, **UNVERIFIED** (no evidence either way),
**Refuted**, **Disclosed** (a stated design/decision, not a measurement).

## 1. Branch and head

- Branch `feat/v0.4-humanmusic-audio`, draft PR #70 (not ready-for-review, not merged, not tagged).
- This hardening round: `a84ca08` (mission-writing head) → the commit adding this document. The last commit that changed
  library source is `1389e71`; every later commit is tests, fixtures or documentation. Gates below ran at
  `45ffe62` (local) and `5cece5a` (GitHub), which contain all of it.
- Package `libgibson` 0.3.1, ABI 1, MSRV 1.85 — **unchanged** by this round (Observed: `check-versions.sh`).
- Everything HumanMusic is under the experimental `gibson::audio::human_music` module; no C/Python/Go ABI
  surface was added (Observed: the ABI v1 symbol baseline gate).

## 2. Architecture (current)

```
SemanticTrace ──► SongMap (the song: IntentTimeline, CompositionPlan{form, discourse+ledger,
                   arrangement, backbone}, ThematicMap, HarmonicMap) ──────────────┐
MusicWorld + MusicalLanguage + PerformanceOptions                                   │
                                                                                     ▼
PerformancePlan::build  (harmony → actions → [cover constraints] → [settlements / rehearsal veto]
                         → stage admission → harmonic edits → accent grid → statements &
                         interactions → ensemble → obligation witnesses → complexity budget)
                                                                                     │
realize_policy(profile) : lead → keys → bass → pad → drums ──► Score ──► HumanMusicSynth ──► PCM
                          (drums: historical producers | ONE arbitrated percussion surface)
PerformanceProfile = { pitch, expression, occupancy, support, lifetime, observation, evidence,
                       admission: Planned|Rehearsed, percussion: Unarbitrated|Arbitrated(restraint) }

Cover:  Composition | ReferenceSong(+DerivedHarmony) | OrderedChart
          ──► CoverMap (exact quotient κ_I at a CoverFidelityProfile, + FidelityReport ceiling)
          ──► cover / cover_candidate / cover_skeleton (receives ONLY the map + CoverTarget)
          ──► CoverConformance (each axis at its own relation) + CoverPipelineReceipt
Timing: MetricPosition ─► GrooveTransport (pocket) ─► FeelTransport (identity only) ─► PerformedPosition
```

## 3. Public API (experimental module)

| Surface | Entry points | Default behaviour |
| --- | --- | --- |
| "Jam" (trace → music) | `human_music::compose`, `render`, `compose_full` | Historical `WRITTEN` realization (the Round XI control). Unchanged. There is no function named `jam`. |
| Song | `SongMap::build` / `SongMap::compose(trace, seed, grammar, Composer)`, `functor::perform_with_profile(song, world, opts, profile)` | `perform` = historical `WRITTEN`; every newer law is explicit. |
| Profiles | `PerformanceProfile::{WRITTEN, TEMPORAL, HEARD, EXPRESSIVE, PHRASED, POCKET, BAND}`, `with_admission`, `with_percussion`, `with_drum_restraint` | `POCKET` = the accepted R17 arm (byte-exact). `BAND` = POCKET + rehearsed admission + Balanced drummer (**new general profile, opt-in**). |
| Cover | `cover` (checked), `cover_candidate` (receipts), `cover_skeleton` (unmetered chart), `CoverMap::{extract, extract_on_lane, extract_fidelity, from_ordered_chart, from_ordered_chart_fidelity}`, `ReferenceSong::{extract, extract_fidelity, derive_harmony}`, `CoverSpec::{from_contract, established}` | Binary `CoverSpec` = v1 relations (byte-exact maps). |
| Diagnostics | `witness::audit`, `SongMapConformance`, `song::AnchorReport`, `percussion::PercussionReport`, `cover::{CoverConformance, FidelityReport}` | — |

**Default vs explicit POCKET (Disclosed):** the library default is still the oldest control (`WRITTEN`).
The accepted pocket and the new hardened profile are explicit opt-ins. Promoting `BAND` (or `POCKET`) to the
default is a maintainer decision and was deliberately not made.

## 4. Fingerprints and historical compatibility (Observed)

- Canonical v2 fingerprints: new profile fields (`admission`, `percussion`) and `CoverMap.fidelity` are
  encoded **only when non-historical**, so every historical profile and every v1 cover map keeps its v2
  hash (tests: `audio_canonical_fingerprint`, `audio_cover_fidelity::the_binary_spec_is_the_v1_profile_byte_for_byte`).
- Legacy Debug/FNV fingerprints: no field was added to a Debug-fingerprinted struct (`PerformancePlan`,
  `DrumHit`, `Provenance`, `MusicalLanguage`); `Score` gained `percussion`, printed only when `Some`.
  Pinned characterization fingerprints (`audio_consolidation_characterization`) unchanged.
- Historical entry points (`perform`, `perform_*`) remain thin adapters over `PerformanceProfile`.

## 5. R17 accepted pocket (distinct gate) — Observed

At the final source: `pocket_music_lab --render` + `scripts/verify-pocket-freeze.py` → **48/48 WAVs and
757/757 receipts byte-identical** (BLACK_ICE mix unchanged). The 16 earlier cover-listening WAVs are also
byte-identical. The same gate passed after every source commit of the round (U1, U2, hold fix, drums,
fidelity). The known R17 120-case sweep replays exactly: 61 passes / 59 first failures (14/38/5/2), every
reason and label identical (`docs/fixtures/humanmusic-hardening/known-r17-replay/`).

## 6. What this round changed

| Area | Result | Evidence |
| --- | --- | --- |
| U1 partial final bar | Repaired at the bass planner; checker unchanged | `tests/audio_source_contracts.rs` (red at `d873961`), `dfa6879` |
| U2 declared anchor never sounded | Groove seated where a full bar exists; unstatable theme sites never planned; `AnchorReport` {Realized, StructurallyInapplicable, DeclaredButMissing}; `CoverSpec::established` | `a042992` red → `5ad153e` |
| U3 action receipts / obligations | `ActionAdmission::Rehearsed` (strike unperformed verbs before the take, recorded) + ordinary settlement planner; historical `Planned` characterized, byte-exact | `ad5d7d5` → `fc5c5e2`; 5,656/5,656 verbs performed on a 480-performance probe |
| G02/G22 held keys | Nonmember hold voices lift at the change | `28e5a06` red → `366dff0` |
| Cover side | Riff lane, groove-vs-silent-seat refusal, one harmony authority | `07f0b8c` red → `0a6ccea` |
| Drummer | One arbitrated percussion surface + restraint dial; rate guard for surface verbs | `e397d7f` red → `1b59e59`, `d40dc09` |
| Fidelity | `CoverFidelityPreset/Profile`, exact relations, ceiling report, v1-compatible | `997544e`, `1389e71` |
| Derived Ode harmony | Declared analyzer v2 (triads; structural sevenths only) | `c4d5cde` |
| Feel seam | Typed identity `FeelTransport`, wired nowhere | `7b4bd6d` |
| Fixtures | 1,491 files / 95.2 MB → 571 files / 8.0 MB (incl. this round's new evidence), all bytes recoverable | `cbd0479`..`abc55e7`, `b09333e`; `docs/fixtures/HUMANMUSIC_FIXTURE_MANIFEST.md` |

## 7. Drum restraint API and evidence

`PercussionPolicy::{Unarbitrated, Arbitrated(DrumRestraint::{Foundation, Balanced, Expressive, Busy})}`.
Required strokes (pocket anchors, time-line hats, stamped action witnesses, a fill's first stroke and
landing, pinned cover strokes) always sound; optional strokes compete for one per-bar allowance chosen by
an ordered rule (floor > phrase-end space > band already speaking > spoke last bar > support), shifted by
the restraint; no weighted score. Contracts (Observed): ornaments monotone in the dial; a "band already
speaking" bar admits none; talking-over-a-busy-lead bars 25 (historical) → 9 (Balanced) on 12 fresh
songs × 2 worlds; no action witnessed by the historical drummer loses its witness at any restraint;
pocket anchors identical at every restraint; pinned cover grooves ignore the dial; at Foundation an
ornament appears only where the drummer has the floor (≤1) or for the rate guard.

Listening A/B (same plan and band per row, proven): generated BLACK_ICE song ornament strokes
33 (historical) → 0 / 10 / 15 / 35 (Foundation / Balanced / Expressive / Busy); VAPOR95 RiffDrive
38 → 2 / 13 / 22 / 47. Whether Balanced *sounds* like a band member: **UNVERIFIED** (listen).

## 8. Cover fidelity API and Ode experiment

Relations per axis and presets: see `docs/HUMAN_MUSIC_COVER.md` § Fidelity dial. Ode (Mutopia #528,
Public Domain) at the v1 map and every preset, BLACK_ICE seed 901 and VAPOR95 seed 904 at 95 BPM, profile
`BAND` (Observed, `docs/fixtures/humanmusic-hardening/listening/ode-fidelity/report.txt`):

| Level | Pins (effective) | Observational ceiling | Conformance (both worlds) |
| --- | --- | --- | --- |
| v1 motif-only | melody with notated rests | everything else Unknown | PASS |
| Loose | the opening 8 beats as the song's identity motif | — | PASS |
| Interpretive | whole melody (metric), derived harmony by triad family | groove → unknown | PASS |
| Faithful | melody with rests, bass line (metric), exact derived harmony | groove, form → unknown | PASS |
| Strict | + bass line with rests | groove, form, seating → unknown | PASS |

Faithful and Strict are **byte-identical** renders for the Ode (the bass line's notated rests change
nothing the Faithful lift does not already produce) — Observed. Harmony is **derived analysis**
(`satb-window-triad/v2`), never presented as the score's. Which level "is the same song": **UNVERIFIED**.

## 9. Swing & A Miss partial

Unchanged behaviour at the v1 path (the maintainer's liked outputs stay byte-identical). Under the dial:
melody, bass and groove are Unknown at every preset; Loose/Interpretive leave nothing to cover (the chart's
harmony is pinned only with its section topology); Faithful/Strict pin exactly the v1 partial map and the
report states the ceiling. No recognition claim; no artist-style code.

## 10. Holdout v1 (unchanged) and holdout v2

- **v1** (`docs/fixtures/humanmusic-consolidation/fresh/`): immutable; 7/36 at first contact. Its families'
  dispositions are in `docs/HUMAN_MUSIC_CONSOLIDATION.md` § Hardening round.
- **v2** (`docs/fixtures/humanmusic-hardening/holdout-v2/`): 48 rows declared at `dd924e7` before contact
  (config SHA256 `3217c518…`), pre-freeze adversarial review, run once: **33/48 pass** (22/36 generated,
  7/8 Ode, 4/4 Swing; 1,685 checks, 19 failing receipts). No repaired v1 family recurred as such. See
  `CLASSIFICATION.md` there for each family.

## 11. Known reds (not repaired; each named)

Source generation:
1. `POCKET`/historical `Planned` admission still admits unperformed verbs (by design: the historical arm is
   byte-exact); use `BAND` / `ActionAdmission::Rehearsed`. (H18 in v2.)
2. Remaining unwitnessed song obligations under `Rehearsed`: cadence/departure debts settled where no home
   chord arrives, planned Resolves no bass/keys attack performs, Simple-language motif questions with no
   answer (65/480 performances in the development probe; H16/H28/H30 in v2).
3. Partial-final-bar **Bass ownership span** can be invalid (H12) — adjacent to U1, new.
4. SWISS_SIGNAL PropulsiveReturn sources sound `Maj6`/`Maj7` Reset chords outside SWISS's no-sevenths
   vocabulary (H05) — new.
5. One false temporal function claim on a 48-beat SWISS DeflectedLift source (H16).
6. Known R17 sweep: 59 first-assertion failures remain (unchanged, classified historically).

Cover:
7. Pinned bass lines leave a few notes without a pitch function (E04: 3/60; H16, H18) — new.
8. `UnprojectableTiming` on DeflectedLift grooves (H23/H31/H35; v1 G18) — extraction limit.
9. Non-monophonic lead at canonical onsets blocks line extraction (H06).
10. Groove + Orchestration from a source whose kit sounded only in action windows: refused
    (`ConflictingPins`) — the seat quotient does not record windows (G23/G28).

## 12. Lawful explicit refusals (not bugs)

- `pinned harmony outside target vocabulary` (Exact) and `pinned harmony family outside target vocabulary`
  (QualityFamily): e.g. a mode change BLACK_ICE Aeolian → SWISS_SIGNAL Ionian.
- `no lawful harmony contains the pinned simultaneous attacks`: every pinned attack must be a chord tone of
  its window (strict; a candidate for a future passing-tone relation).
- `ConflictingPins` (contradictory pins), `MissingAxis` (the source never sounds the requested axis),
  "nothing to cover" (the dial's effective profile pins nothing).

## 13. Fixture footprint (Observed)

Tracked `docs/fixtures/humanmusic-*`: **1,491 files / 95,220,730 bytes** at `a84ca08` → **571 files /
8,013,694 bytes** now, including this round's new evidence (holdout v2, sweep replay, listening receipts).
994 archival files (manifest: 331 exact duplicates of retained copies, 663 archived as 246 unique blobs) live in a deterministic
`tar.xz` + manifest; `scripts/fixtures/expand-humanmusic-archives.py` restores every original byte
(round trip checked independently: 1,483/1,491 byte-identical, the other 8 are READMEs with additive
"Storage" notes). Git history still holds every old blob (no rewrite).

## 14. Gates at the final source

| Gate | Result | Where |
| --- | --- | --- |
| Release preflight (13 steps) | **PASS 13/13** — whitespace, version/ABI consistency, `fmt --check`, strict clippy (all targets, all features), `cargo test` (full suite), rustdoc `-D warnings`, MSRV 1.85 `check --locked --lib`, MSRV declared-range consumer, `cargo package`, ABI v1 symbol baseline, clean-room consumers (C, C++, Python, Go, Rust; all toolchains present, none skipped), third-party notices, license files | local, `45ffe62`, clean tree (Observed) |
| GitHub CI | **success** | `5cece5a` (Observed) |
| GitHub Release Preflight | **success** | `5cece5a` (Observed) |
| R17 PCM freeze (distinct gate) | **48/48 WAV + 757/757 receipts byte-identical** | local, `5cece5a` source (Observed) |
| Cover listening corpus | **16/16 WAVs byte-identical** to `listening/WAV_SHA256SUMS` | local, `5cece5a` source (Observed) |
| Known R17 sweep replay | 61/59, all reasons identical | local, `96566a2` source; no library change since except `1389e71` (cover fidelity extraction only) (Observed) |
| Package version / ABI | 0.3.1 / ABI 1, unchanged | `check-versions.sh`, `check-abi.sh` |


## 15. Unresolved TODOs

- Choose the library default (`WRITTEN` today; `BAND` is the hardened candidate) — Disclosed decision.
- The named reds in §11 (each needs its counterexample and a holdout v3).
- A passing-tone relation for pinned attacks; recording action windows in the seat quotient.
- `cover.rs` (2,148 lines) + `cover_fidelity.rs` (1,066) were not split further (judged churn before review).
- `SongMap::composer()` self-report ambiguity (PropulsiveReturn) remains.
- PR #70 body/title are stale; a suggested replacement is drafted (not posted).
- Human feel remains only a typed identity seam.

## 16. Reasons NOT to merge (if any hold for you)

- **Listening is the acceptance test and it is open.** No machine result here shows that the drummer sounds
  better, that any Ode fidelity is "the same song", or that the Swing outputs remain useful.
- **The recommended profile is not the default.** Merging ships `BAND` as an opt-in; `compose`/`perform`
  still use the oldest control. If the intent of merging is "the hardened band is what users get", the
  default decision must be made first.
- **Holdout v2 is 33/48**, with new source-side reds (H05 vocabulary leak, H12 ownership span, pinned-bass
  pitch functions) found only at contact. They are named and preserved, not repaired.
- The module is large and experimental (HumanMusic `src/audio/human_music` is 63,246 lines); its API is not stabilized.

## 17. Listening questions for the maintainer

1. **Drums:** at the default restraint (Balanced), does the drummer feel like part of the band rather than a
   talented player constantly demonstrating technique?
2. **Ode:** which fidelity crosses from "new interpretation using Ode material" to "obviously the same song
   being covered", and which becomes too literal?
3. **Swing partial:** do the partial-reference outputs remain useful as fresh songs over the supplied
   harmonic/form skeleton?
4. **Pocket preservation:** does the accepted BLACK_ICE result still sound right? (It is byte-identical.)
5. **Merge review:** another hardening round, or is this the branch to merge? *(Not answered here.)*
