# HumanMusic — merge readiness

This document exists so the maintainer can decide whether `feat/v0.4-humanmusic-audio` goes to `main`.
It does **not** decide that. Green engineering gates are necessary, not sufficient: the listening
questions at the end are open, and the reasons to postpone are listed without softening.

Labels: **Observed** (read from a committed receipt or a command run at the stated source),
**Corroborated** (two independent routes agree), **Conjectured**, **UNVERIFIED** (no evidence either way),
**Refuted**, **Disclosed** (a stated design/decision, not a measurement).

## 1. Branch and head

- This round (the final pre-main closeout) started at `15f56d3`, the head of
  `claude/humanmusic-semantic-consolidation-cc4k3o` (the semantic-consolidation round, draft PR #70).
  That branch was a clean descendant of `feat/v0.4-humanmusic-audio` (`1cd122f`, 34 ahead / 0 behind),
  so `feat/v0.4-humanmusic-audio` was **fast-forwarded** to it locally (no merge commit). All work
  below was then authored on `feat/v0.4-humanmusic-audio`. Nothing was merged to `main`, tagged,
  released or marked ready; no history was rewritten.
- The pre-main source was frozen at `006bacf` (the support-lifetime / landed-function / truncated-form
  laws). A post-contact correction (`§11a`) advanced the source past the holdout-v4 declaration
  commit `d869f72`; **no v5 was run in this round**.
- Package `libgibson` 0.3.1, ABI 1, MSRV 1.85 — **unchanged** (`check-versions.sh`, `check-abi.sh`).
- Everything HumanMusic is under the experimental `gibson::audio::human_music` module; no
  C/Python/Go ABI surface was added.

## 2. Architecture (current)

```
SemanticTrace ──► SongMap (IntentTimeline, CompositionPlan{contract, form, discourse+ledger,
                   arrangement, backbone}, ThematicMap, HarmonicMap, composed_by) ─────┐
MusicWorld + MusicalLanguage + PerformanceOptions                                     │
   └─► HarmonicVocabulary (world × language): admits / conforms chords                 │
                                                                                       ▼
PerformancePlan::build (harmony [inside the vocabulary] → actions → [cover constraints]
     → settlements → rehearsal vetoes/recasts → stage → edits → accent → statements &
     interactions → ensemble → obligations → budget), every window fit to
     PerformanceDomain [0, total_beats]
        ▲                                     │
        └── rehearsal (finite normalization: ActionKey, RehearsalTrace) ◄── witness::audit
                                              ▼
realize_policy(profile): lead → keys → bass → pad (hears the band inside the piece) → drums
   ──► Score (+ StrokeOrigin sidecar: metric source → performed, pocket identity) ──► PCM
PerformanceProfile = { pitch, expression, occupancy, support, lifetime, observation, evidence,
   admission: Planned|Rehearsed, percussion: Unarbitrated|Arbitrated(restraint),
   harmony: Archived|Vocabulary, functions: Archived|Earned }

perform_candidate ──► PerformanceReceipt (general laws) ──► perform_checked: admit | Rejected | Refused

Identity:  declared anchor ──► identity-bearing material (theme sites' statements, the bass
           figure, recorded pocket anchors, the lift's pinned events) ──► realized evidence
           AnchorReport = presence + conformance (each anchor's identity relation)

Cover: Composition | ReferenceSong(+DerivedHarmony) | OrderedChart
       ──► CoverMap (extract = identity projection; extract_lane = historical lane quotient)
       ──► cover_candidate / cover_skeleton (map + CoverTarget only)
       ──► CoverAdmission = PerformanceReceipt + CoverConformance
           (conformance from primitive relations on the REALIZED cover, never re-extraction)
Timing: MetricPosition ─► GrooveTransport ─► FeelTransport (identity) ─► PerformedPosition
```

## 3. Public API (experimental module)

| Surface | Entry points | Default behaviour |
| --- | --- | --- |
| "Jam" (trace → music) | `human_music::compose`, `render`, `compose_full` | Historical `WRITTEN` realization. Unchanged. |
| Song | `SongMap::build` / `SongMap::compose(trace, seed, grammar, Composer)`, `SongMap::composer()` (recorded `composed_by`) | Provenance recorded where content is chosen; not in either song fingerprint. |
| Performance | `functor::perform_with_profile`, **`perform_candidate`**, **`perform_checked` → `Result<Composition, PerformanceRejection>`**, `receipt::PerformanceReceipt::{measure, measure_under, passes, failures}` | `perform` = historical `WRITTEN`. |
| Profiles | `PerformanceProfile::{WRITTEN, TEMPORAL, HEARD, EXPRESSIVE, PHRASED, POCKET, BAND}`, `with_admission`, `with_percussion`, `with_drum_restraint`, **`with_harmony`**, **`with_functions`** | `POCKET` = the accepted R17 arm (byte-exact). `BAND` = POCKET + rehearsed admission + Balanced drummer + `HarmonyPolicy::Vocabulary` + `FunctionPolicy::Earned` (**opt-in**). |
| Cover | `cover` (checked: `CoverAdmission`), `cover_candidate`, `cover_skeleton`, `CoverMap::{extract, extract_on_lane, extract_lane, extract_lane_on, extract_fidelity, from_ordered_chart, from_ordered_chart_fidelity}`, `ReferenceSong::{extract, extract_fidelity, derive_harmony}`, `CoverSpec::{from_contract, established, has_song_identity}` | `extract` reads the identity projection; `extract_lane` is the v1 lane quotient (byte-exact v1 maps). |
| Diagnostics | `witness::audit`, `SongMapConformance`, `song::AnchorReport` (presence + conformance), `projection::{IdentityMaterial, identity_notes, groove_strokes}`, `percussion::PercussionReport`, `cover::{CoverConformance, FidelityReport}`, `vocabulary::HarmonicVocabulary` | — |

**Default vs explicit profile (Disclosed):** the library default is still the oldest control (`WRITTEN`).
`POCKET` and `BAND` are explicit opt-ins. Promoting `BAND` to the default is a maintainer decision and was
deliberately not made.

## 4. Fingerprints and historical compatibility (Observed)

- Canonical v2 fingerprints: every new field is encoded **only when non-historical** — profile `harmony`,
  `functions`; plan `rehearsal`, `functions`; `CoverMap.projection` (Lane unencoded). `SongMap.composed_by`
  is encoded in neither song fingerprint (provenance is not identity). Every historical profile, plan and
  v1 cover map keeps its hash.
- Legacy Debug/FNV: `PerformancePlan` and `Score` print new fields (`rehearsal`, `functions`,
  `stroke_origins`) only when present; pinned characterization fingerprints are unchanged.
- Historical entry points remain thin adapters over `PerformanceProfile`; nothing historical reads a new law.

## 5. R17 accepted pocket and historical evidence (distinct gates) — Observed

At `41fc2eb`: `pocket_music_lab --render` + `scripts/verify-pocket-freeze.py` → **48/48 WAVs and 757/757
receipts byte-identical**; the 16 cover-listening WAVs byte-identical. Both gates passed after **every**
library commit of the round. The known R17 120-case sweep (`r17_fresh_world_seed_tempo_sweep`, rustc 1.98.1)
replays exactly: 61 passes / 59 first failures (14/38/5/2), every reason and every passing label identical
(at `41fc2eb`).

**Re-verified at this round's revised source** (`fe4e681`): the R17 gate again reports 48/48 WAV +
757/757 receipts byte-identical, and the 16 cover-listening WAVs are byte-identical. Every this-round
source law that could touch POCKET is gated on `FunctionPolicy::Earned` (BAND); the two un-gated changes
(`SongMap::landmarks` pointer emission, `transform_material` room clamp, the occupancy epsilon) do not
alter these bar-aligned historical fixtures.

Disclosed change to **unaccepted** audio: the BAND hardening-listening corpus (`drum_restraint_lab` +
`cover_fidelity_lab`, 30 WAVs) has changed again this round (the earned support lifetime, the landed
re-pitchings and the metric groove bars); see §14 for the regenerated count.

## 6. What this round changed

Each repair has a falsifier committed red before it (evidence discipline).

| # | Item | Result | Red → fix |
| --- | --- | --- | --- |
| 1a | H05 vocabulary leak | One `HarmonicVocabulary` law (world × language) for sources and covers; BAND conforms every room's own colour choice or refuses a chart chord it cannot admit (`VOCABULARY_REFUSAL`) | `fe5a311` → `5dfd2d2` |
| 1b | H12 span past the end | One `PerformanceDomain [0, total_beats]`; every planned window fit at construction; `occupancy::violations` unchanged | `0db2f34` → `e7477e6` |
| 1c | False settlements (H16/H28/H30) | A settlement stands only when a performed verb discharges it; otherwise the debt stays open, **named** (`OpenDebt`) — never stamped | `df7b729` → `b8c4551` |
| 2–4, 6 | Identity category error | Projection anchor → identity-bearing material → realized evidence (Motif: identity theme statements; BassFigure: the bass's structural line; Riff: lead identity else bass figure; Groove: recorded pocket anchors). `AnchorReport` = presence **and** conformance; `CoverSpec::established` requires both. Cover verification reads primitive relations on the realized cover (`cover_relations.rs`), never the extractor. `StrokeOrigin` sidecar (metric source → performed). | `14a6e12`, `885bba3` → `0a8e7d0` |
| 5 | Pinned functions | Every pinned event carries a justified function in the target harmony, or the lift is refused before realization; no post-hoc relabelling | `5383cfc` → `12fbf84` |
| 8, 10 | Receipts / invariants | General `PerformanceReceipt`; `CoverAdmission = PerformanceReceipt + CoverConformance`; `perform_candidate` / `perform_checked`; release-only `debug_assert` holes are typed errors | `f73e3e2` → `ed2f141` |
| 9 | Rehearsal | Typed `ActionKey`; `RehearsalTrace` (Performed / Recast / Rejected / Deferred); finite normalization (strict progress, fuel 64, refusal on no progress); a rejected verb never erases an obligation | `df7b729` → `b8c4551` |
| 11 | Composer provenance | `SongMap.composed_by` recorded where content is chosen; outside identity | `08f584c` → `f9420b8` |
| 7 | CoherenceContract | Every field's standing documented and proved by perturbation (§7) | `7a833ef` → `c63a660` |
| — | BAND support truth | `FunctionPolicy::Earned`: bass/keys lift off at a foreign harmony; a bar-end approach aims at the pitch the bass's own line sounds on the downbeat | `18eecec` → `98531f1` |
| — | BAND chord identity | The pad's rooting law reaches a common-tone carry; the pad hears the band inside the piece | `bfa163c` → `3e8e5cc`; `ad0e982` → `4afc772` |
| — | Re-struck pedal | The temporal judge reads a pedal by its whole chain, as its stated law says (a forged chain stays false) | `60adbe4` → `3ee353a` |
| — | Sub-bar Groove | No full bar → Groove structurally inapplicable, never Realized-and-Deviating (found by the v3 pre-freeze review) | `5ad4532` → `362a4dd` |
| — | After v3 contact | A pinned event's function is judged in the scale it sounds in (V19's family); a pinned theme is stated or the lift is refused (V36's family) — both on fresh seeds | `3bf20fc` → `a20b4c2`; `56e5417` → `41fc2eb` |
| 12 | Giant files | Not split (semantics first; see §15) | — |

### This round (final pre-main closeout) — each repair red-first

| Law | General statement | Red → fix |
| --- | --- | --- |
| BAND support lifetime | Under `FunctionPolicy::Earned` a support tail must not sound into a structural harmony that **excludes** its pitch (the bass's and keys' `release_support` membership law, extended to the pad). A tail the next harmony admits is consonant and may be load-bearing (the pad's own root). | `3b28215` → `006bacf`; corrected after contact at seed `90_500_002` (a first, too-strict "stated-continuation" version clipped a root-carrying tail and flipped the chart) |
| Landed material functions | A projected material onset that `land_once` moved onto an accent is judged where it actually sounds — the bass's `reland` law, now applied to the keys (`comp::finish_keys`) and to the independent BAND bass (`bass::reland_note`): re-pitch to a stable/chord tone when the landed harmony excludes the pitch, then declare the earned function. | `3b28215` → `006bacf` |
| Truncated-form conformance | A finite form does not promise what it has no room to state: `SongMapConformance` and `AnchorReport` apply `ThemeSite::statable` against the FINAL form, and `SongMap::landmarks()` no longer clamps a Lift pointer into a truncated slot where its arrival was never realized. | `3b28215` → `006bacf` |
| Groove bar assignment | A pocket stroke belongs to the bar of its recorded **metric** source (`StrokeOrigin.metric`), never its microtimed performed float: an anticipated downbeat is the bar it anticipates. | post-contact correction |
| Interaction-material domain | A response material's last event is cut at its own room, never inflated past it by the `0.1`-beat floor; the occupancy reservation check uses the domain's own `1e-6` epsilon. | post-contact correction |

**Hypotheses refuted this round:** H23/H31/H35 were conjectured groove failures — Refuted: they were
non-identity triplet material inside lane quotients (the projection fixes them). The BAND reds were
conjectured to be this round's regressions — Refuted: the same 59–61 / 13 occurred at `1cd122f` and under POCKET.

### Identity projection, before → after (Observed)

Before: a cover's Motif/BassFigure/Riff was "every event the instrument played" (responses, quotes, fills,
approach notes included); a Groove was every kick/snare. After: only identity-bearing material counts, and
two materials on one role at one onset leave the motif well defined (the lead is never forced monophonic).
36 generated songs that declared Motif had no identity theme site; a thesis-restatement law now states it
where a seated phrase can hold it, else presence is `StructurallyInapplicable` with its reason. Hostile tests:
unrelated lead or bass notes never establish Motif, BassFigure or Riff; a wrong harmonic trajectory never
establishes HarmonicContour (`tests/audio_identity_projection.rs`).

### AnchorConformance laws (Observed)

Motif — planned identity statements sound at their authored onsets (recorded expression omissions excused);
BassFigure — every figure note is a chord tone or the pedal; HarmonicContour/Loop — landmarks, else phrase
closures (deferred to a cover's pinned harmony); Groove — some full bar states the pocket kick and backbeat
(no full bar: inapplicable); Form — sections match the form; Orchestration — no violations and the seats are
the arrangement's.

## 7. CoherenceContract standing (Observed, `contract_standing.rs`)

| Field | Standing |
| --- | --- |
| `grammar`, `anchors`, `phrase_bars`, `resolution`, `foreground_budget` | **Load-bearing** |
| `recurrence_bars` | Load-bearing only as DeflectedLift's fixed-tiling rate |
| `max_transform`, `novelty_budget` | **Descriptive** (demoted; kept because legacy fingerprints pin R17) |
| `PhraseGoal::thematic_distance` | **Diagnostic** (discourse diagnostic only) |
| `PhraseGoal::harmonic_distance`, `PhraseGoal::novelty_budget`, `MusicalThesis::anchors`, `TimeScales::phrase_bars` | Descriptive |

Proved under POCKET and BAND across six grammars: moving every Descriptive field leaves plan, score,
receipt, established spec and diagnostics bit-identical; each Load-bearing field moved alone changes what is
heard. A mutation (max_transform widening the foreground budget) turns the audit red.

## 8. Rehearsal normalization (Observed)

720-performance development corpus: 0 refusals, ≤2 passes, 270 settlements discharged, 14 recast onto the
arrival a player made, 30 debts left open — all named ("never sounds home chord"). Fresh 1,080-performance
BAND sweep (§11): every verb performed.

## 9. Drum restraint, Ode fidelity, Swing partial

Unchanged APIs from the hardening round (see `docs/HUMAN_MUSIC_COVER.md`). Ode at BAND (seed 901/904,
95 BPM): v1 motif-only, Loose, Interpretive and Faithful lift and conform; **Strict is now an explicit lawful
refusal** (the frozen SATB bass has no lawful function against the target harmony; formerly it lifted with
unclassified bass notes). Swing partial: unchanged at the v1 path. Which Ode level "is the same song", and
whether Balanced sounds like a band member: **UNVERIFIED** (listen).

## 10. Holdouts

- **v1** and **v2** are immutable known evidence; nothing this round was fitted to them.
- **v3** (`docs/fixtures/humanmusic-consolidation2/holdout-v3/`): 48 rows declared at `721fffe` (config
  SHA256 `cf96dbe1…`, after a pre-freeze adversarial review), executed once at that commit on a clean tree:
  **46/48 pass** (853 checks, 2 failing receipts). Failures: **V19** and **V36**, both repaired after
  contact on fresh seeds (§6); v3's numbers stand.
- **v4** (`docs/fixtures/humanmusic-consolidation2/holdout-v4/`): the merge holdout, 28 rows declared at
  `d869f72` (config SHA256 `27be8e4a…`, library source frozen at `006bacf`), executed once on a clean tree:
  **24/28 pass** (472 checks, 4 failing receipts) — see `CLASSIFICATION.md`. **Every failure is a
  declaration error in the holdout, not an internally invalid performance**: V09/V14/V20 declared a grammar
  the `StablePropulsion` composer overrides to `PropulsiveReturn`, and O02 predicted a lift the target
  lawfully refuses (`pinned harmony outside target vocabulary`). Every source receipt and every admitted
  cover receipt passed. The declared cross-swing gap was exercised: `cross-swing-groove-quotient` PASS on
  every admitted Groove-pinning cover, with V03/V09/V20 carrying strokes to another performed float while
  the canonical metric quotient stayed exact.
- **Broad fresh BAND search** (this round's §1 sweep, at the revised source): **27,624 / 27,648 admitted**,
  0 refused, **24 rejected** (0.087%) — 16 held-identity flips and 8 false temporal function claims, all
  pre-existing (`§11`).

## 11. Remaining internal reds (each named)

1. **Residual held-identity flips (16/27,648, pre-existing).** The broad fresh search still rejects 16
   performances with 1-2 held-identity flips: `StructuralR9 + DeflectedLift + FusionConversation +
   VAPOR95`, at 7.25/13.25/16/33.25 beats. The mechanism is the target defect's sibling shape — a
   **rootless keys comp** (e.g. a held `C F G` over the bass's `A`) completes a rival triad (F major)
   while the chart root (D) is silent. It is not the pad-tail shape this round fixed (the pad is silent),
   and it is present at the pre-round head (`15f56d3`). The general repair would extend the pad's
   root-support repair to the keys (or change the root-support requirement) — a cross-player
   architecture change the brief forbids expanding into, so it is **preserved and named**, not fitted.
2. **Residual false temporal function claims (8/27,648, pre-existing).** `StructuralR9 + DeflectedLift +
   FusionConversation + BLACK_ICE + 33.25` at a truncated length: a lead melody note declares
   `Appoggiatura` with no independently reconstructed resolution. The lead's pitch-path classification
   (`temporal.rs`) is untouched by this round; it needs its own round.
3. **POCKET / historical arms** keep their archived behaviour by design (byte-exact): unperformed verbs
   under `Planned`, the half-beat support tail, bar-end approaches toward roots the bass does not sound,
   the carry without root — 58/1080 false-claim and 13/1080 flip performances in the known sweep at
   `41fc2eb`.
4. Known R17 sweep: 59 first-assertion failures (unchanged, classified historically).
5. Cover: `UnprojectableTiming` on DeflectedLift grooves (v1 G18 family); Groove + Orchestration from a
   kit that played only in action windows (`ConflictingPins`).
6. Environment: `tests/pty_demos::hack_shell_commands_trigger_real_effects` fails identically at
   `1cd122f` and at this head in the release profile whenever its example is already built (it passed in
   the preflight's debug run); the demo has no audio code. Not addressed.

## 12. Lawful explicit refusals (not bugs)

- `VOCABULARY_REFUSAL` — a chart chord outside the world/language vocabulary (BAND planner).
- `pinned harmony [family] outside target vocabulary`; `no lawful harmony contains the pinned simultaneous attacks`.
- `a pinned lead/bass event has no lawful pitch function in the target harmony` (e.g. Ode Strict).
- `a pinned theme has no seated phrase to be stated in the target form`.
- `ConflictingPins`, `MissingAxis` (the source never established the axis), and "nothing to cover" (the
  selection pins no song identity).

## 13. Fixture footprint (Observed)

The semantic-consolidation round added holdout v3 (56 files, 382,552 bytes). This round adds holdout v4
under `docs/fixtures/humanmusic-consolidation2/holdout-v4/` (config, README, CLASSIFICATION, and 32
first-contact receipt files under `results/`).

## 14. Gates at the final source

| Gate | Result | Where |
| --- | --- | --- |
| Release preflight (13 steps) | **PASS 13/13** (see §14a) — whitespace, version/ABI consistency, `fmt --check`, strict clippy (all targets, all features, rustc 1.98.1), `cargo test` (full suite), rustdoc `-D warnings`, MSRV 1.85 `check --locked --lib`, MSRV declared-range consumer, `cargo package`, ABI v1 symbol baseline, clean-room consumers (C, C++, Python, Go, Rust), third-party notices, license files | local, `a4fbb16`, clean tree (Observed). Disclosed: this container's system setuptools 68.1.2 cannot build any wheel (`AttributeError: install_layout`), so the run sets `SETUPTOOLS_USE_DISTUTILS=stdlib`; without it only the clean-room Python step fails. A first run failed several steps for lack of disk and was discarded. |
| GitHub CI | **success** (run 36804212022, `workflow_dispatch`) at `41fc2eb`; **not re-run this round** | `41fc2eb` (Observed) |
| GitHub Release Preflight | **success** (run 36804214484, `workflow_dispatch`) at `41fc2eb`; **not re-run this round** | `41fc2eb` (Observed) |
| R17 PCM freeze (distinct gate) | **48/48 WAV + 757/757 receipts byte-identical** | local, `fe4e681` (the revised source), re-run (Observed) |
| Cover listening corpus | **16/16 WAVs byte-identical** | local, `fe4e681`, re-run (Observed) |
| BAND hardening listening corpus | **20/30 identical, 8 changed, 2 missing** — the 20 drum WAVs are byte-identical (drums are untouched this round); the 8 Ode-fidelity mixes changed (the earned support lifetime and landed re-pitchings); the 2 `4_strict` WAVs remain an explicit lawful refusal | local, `a4fbb16`, regenerated (Observed) |
| Known R17 sweep replay | 120 executed / 61 / 59 (14/38/5/2); every first reason and every passing label identical | local, `41fc2eb`, rustc 1.98.1 (Observed) |
| Holdout v3 | 46/48 at first contact (§10) | `721fffe` (Observed) |
| Holdout v4 | 24/28 at first contact, all 4 declaration errors (§10) | `d869f72` (Observed) |
| Broad fresh BAND search | 27,624/27,648 admitted; 24 rejected (16 flips, 8 temporal-false) — pre-existing (§11) | local, `fe4e681` (Observed) |
| Falsifier suites | `audio_band_support_lifetime` (2), `audio_band_landed_functions` (4), `audio_band_truncated_form` (5) PASS | local, `fe4e681` (Observed) |
| Package version / ABI | 0.3.1 / ABI 1, unchanged | `check-versions.sh`, `check-abi.sh` |

### 14a. Preflight detail

The 13-step preflight was run twice at the revised source. The first run (`fe4e681`) passed 12/13 and
failed only strict clippy on one unused import in the holdout-v4 harness; that import was removed
(`a4fbb16`) and the second run (at `a4fbb16` with this documentation) reports
**`RELEASE PREFLIGHT: PASS` 13/13**. GitHub CI and the GitHub Release Preflight workflow were **not**
re-run this round (no push-triggered job was dispatched).

Not covered by the preflight: `tests/pty_demos::hack_shell_commands_trigger_real_effects` in the
**release** profile with a prebuilt example (§11.6); it passed in the preflight's debug `cargo test`.

## 15. Unresolved TODOs

- **The residual internal reds in §11.1–§11.2** (rootless-support held-identity flips; lead
  `Appoggiatura` false temporal claims). Each needs a round: the first is the cross-player root-support
  requirement; the second is the lead pitch-path classifier at a truncated end.
- **A clean one-contact holdout for the final head.** v4 is preserved as a raw first contact whose 4
  failures are declaration errors; a corrected 28-row declaration (grammar left to `StablePropulsion` for
  its three rows, O02 expectation lowered to `lawful`) is the obvious next contact, and must be declared
  before running.
- Choose the library default (`WRITTEN` today; `BAND` is the hardened candidate) — Disclosed decision.
- `cover.rs` and `cover_fidelity.rs` were not split: the semantics are fixed, so a split is a pure move.
- A passing-tone relation for pinned attacks; recording action windows in the seat quotient.
- Human feel remains only a typed identity seam.
- PR #70 body/title are stale (see §18; not edited; the PR stays draft).

## 16. Reasons NOT to merge (if any hold for you)

- **Residual internal reds remain.** The broad fresh BAND search still rejects 24/27,648 performances
  (§11): 16 pre-existing held-identity flips (rootless keys comp over a bass chord tone, pad silent) and
  8 pre-existing false temporal function claims (lead `Appoggiatura` with no observed resolution). Both
  need their own round; neither is a lawful refusal.
- **The holdout was not clean.** v4 is 24/28 at first contact, with the 4 failures being declaration
  errors in the holdout (not branch defects) — so this round produced no clean one-contact holdout for the
  final head. The source was also corrected after contact (§11), so v4's raw result describes an earlier
  head, and **no v5 was run**.
- **Listening is the acceptance test and it is open.** No machine result shows that BAND sounds better than
  POCKET, that the drummer serves the band, or that any Ode fidelity "is the same song".
- **The recommended profile is not the default.** Merging ships `BAND` as an opt-in.
- BAND's sound changed this round (support tails, landed re-pitchings, groove metric bars) without a listen;
  only POCKET's bytes are frozen.
- The module is large and experimental; its API is not stabilized.

## 17. Listening questions for the maintainer

1. **Drums:** at Balanced, does the drummer feel like part of the band?
2. **BAND vs POCKET:** do the earned support laws (no tails into foreign chords, approaches into what the bass
   actually plays, the carried root) sound like a tighter band, or more cautious?
3. **Ode:** which fidelity crosses from "new music using Ode material" to "obviously the same song"?
4. **Swing partial:** are the partial-reference outputs still useful as fresh songs?
5. **Pocket preservation:** does the accepted BLACK_ICE result still sound right? (It is byte-identical.)
6. **Merge review:** another round, or is this the branch to merge? *(Not answered here.)*

## 18. Draft replacement for PR #70 (not applied)

**Title:** `feat(audio): HumanMusic experimental module — BAND profile, checked performances, identity projection, covers`

**Body (draft):** This PR lands the experimental `gibson::audio::human_music` module: a semantic trace
becomes a song, a song plus a world/language becomes one checked performance, and a checked performance
can be lifted into a cover. The hardened general profile is `PerformanceProfile::BAND` (rehearsed verb
admission, an arbitrated Balanced drummer, the world's harmonic vocabulary as a source law, support
functions earned where they sound); the library default is unchanged (`WRITTEN`), and `POCKET` remains the
byte-exact accepted R17 arm.

Machine evidence on this head: R17 PCM freeze 48/48 WAVs + 757/757 receipts byte-identical; the 16
cover-listening WAVs byte-identical; release preflight green (§14); the known R17 sweep replay unchanged;
holdout v3 46/48 (two families repaired after contact). **Open, and the reason this stays a draft:** the
broad fresh BAND search still admits 27,624/27,648, with 24 pre-existing internal reds (16 rootless-support
held-identity flips, 8 lead false-temporal-function claims) that need their own round (§11); holdout v4 is
24/28 at first contact with all four failures being holdout declaration errors, and no clean one-contact
holdout exists yet for the final head. Human listening (drums, BAND vs POCKET, Ode fidelity, swing partial)
is the acceptance test and is open.
