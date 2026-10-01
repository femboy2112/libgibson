# HumanMusic — merge readiness

This document exists so the maintainer can decide whether `feat/v0.4-humanmusic-audio` goes to `main`.
It does **not** decide that. Green engineering gates are necessary, not sufficient: the listening
questions at the end are open, and the reasons to postpone are listed without softening.

Labels: **Observed** (read from a committed receipt or a command run at the stated source),
**Corroborated** (two independent routes agree), **Conjectured**, **UNVERIFIED** (no evidence either way),
**Refuted**, **Disclosed** (a stated design/decision, not a measurement).

## 1. Branch and head

- This round (the semantic consolidation) started at `1cd122f` (the head of `feat/v0.4-humanmusic-audio`,
  draft PR #70) and was developed on `claude/humanmusic-semantic-consolidation-cc4k3o`, which contains
  `1cd122f` unchanged; fast-forwarding `feat/v0.4-humanmusic-audio` to it is the maintainer's call. Nothing
  was merged, tagged, released or marked ready; no history was rewritten.
- The last commit that changed library source is `41fc2eb`; every later commit is documentation.
- Package `libgibson` 0.3.1, ABI 1, MSRV 1.85 — **unchanged** (Observed: `check-versions.sh`, `check-abi.sh`).
- Everything HumanMusic is under the experimental `gibson::audio::human_music` module (now 67,969 lines);
  no C/Python/Go ABI surface was added.

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

Disclosed change to **unaccepted** audio: the BAND hardening-listening corpus (`drum_restraint_lab` +
`cover_fidelity_lab`, 30 WAVs) now has 23 identical, 5 changed (Ode `0_v1-motif-only` ×2,
`2_interpretive.vapor95`, `3_faithful` ×2) and 2 missing (Ode `4_strict` ×2, now an explicit lawful refusal:
"a pinned bass event has no lawful pitch function in the target harmony").

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
  **46/48 pass** (853 checks, 2 failing receipts). 36/36 generated BAND sources hold their own
  PerformanceReceipt and record their declared composer and grammar; identity unchanged under a hostile
  non-identity event 29/29; 9/9 refusals lawful at their stage; 12/12 `lift` predictions lifted and admitted;
  35/37 lifts admitted. Failures: **V19** (a pinned passing tone labelled chromatic in a scale that holds it —
  a false claim in a BAND cover) and **V36** (a Loose theme lifted into a form with no theme site). Both
  families were repaired after contact on fresh seeds (§6); v3's numbers stand. Coverage gaps stated in
  `CLASSIFICATION.md`: no swung groove crossed a swing change (Groove alone is not song identity), and no
  natural identity overlap occurred.

## 11. Remaining internal reds (each named)

1. **BAND sweep** (1,080 fresh performances, `perform_checked`): 1006 admitted before this round's support
   laws → **1079** after. The one
   rejection is a held-identity flip POCKET shares: a rootless keys comp (D–C–G) over a bass quote on E while
   the pad is silent and the previous pad voicing's release tail rings over the change (Am7 heard as C).
2. **POCKET / historical arms** keep their archived behaviour by design (byte-exact): unperformed verbs under
   `Planned`, the half-beat support tail, bar-end approaches toward roots the bass does not sound, the carry
   without root — 58/1080 false-claim and 13/1080 flip performances in the same sweep at `41fc2eb` (61 before
   the pedal judge read whole chains).
3. Known R17 sweep: 59 first-assertion failures (unchanged, classified historically).
4. Cover: `UnprojectableTiming` on DeflectedLift grooves (v1 G18 family); Groove + Orchestration from a kit
   that played only in action windows (`ConflictingPins`); a swung groove pinned across a swing change is
   untested by any holdout.
5. Environment: `tests/pty_demos::hack_shell_commands_trigger_real_effects` fails identically at `1cd122f`
   and at this head in the release profile whenever its example is already built (it passed in the
   preflight's debug run); the demo has no audio code. Not addressed.

## 12. Lawful explicit refusals (not bugs)

- `VOCABULARY_REFUSAL` — a chart chord outside the world/language vocabulary (BAND planner).
- `pinned harmony [family] outside target vocabulary`; `no lawful harmony contains the pinned simultaneous attacks`.
- `a pinned lead/bass event has no lawful pitch function in the target harmony` (e.g. Ode Strict).
- `a pinned theme has no seated phrase to be stated in the target form`.
- `ConflictingPins`, `MissingAxis` (the source never established the axis), and "nothing to cover" (the
  selection pins no song identity).

## 13. Fixture footprint (Observed)

Unchanged from the hardening round except this round's holdout v3 declaration and first-contact results
(56 files, 382,552 bytes under `docs/fixtures/humanmusic-consolidation2/`).

## 14. Gates at the final source

| Gate | Result | Where |
| --- | --- | --- |
| Release preflight (13 steps) | **PASS 13/13** — whitespace, version/ABI consistency, `fmt --check`, strict clippy (all targets, all features, rustc 1.98.1), `cargo test` (full suite), rustdoc `-D warnings`, MSRV 1.85 `check --locked --lib`, MSRV declared-range consumer, `cargo package`, ABI v1 symbol baseline, clean-room consumers (C, C++, Python, Go, Rust), third-party notices, license files | local, `41fc2eb`, clean tree (Observed). Disclosed: this container's system setuptools 68.1.2 cannot build any wheel (`AttributeError: install_layout`), so the run sets `SETUPTOOLS_USE_DISTUTILS=stdlib`; without it only the clean-room Python step fails (bindings untouched this round). A first run failed several steps for lack of disk and was discarded. |
| GitHub CI | **success** (run 36804212022, `workflow_dispatch`) | `41fc2eb` (Observed) |
| GitHub Release Preflight | **success** (run 36804214484, `workflow_dispatch`) | `41fc2eb` (Observed) |
| R17 PCM freeze (distinct gate) | **48/48 WAV + 757/757 receipts byte-identical** | local, `41fc2eb` (Observed) |
| Cover listening corpus | **16/16 WAVs byte-identical** | local, `41fc2eb` (Observed) |
| Known R17 sweep replay | 120 executed / 61 / 59 (14/38/5/2); every first reason and every passing label identical | local, `41fc2eb`, rustc 1.98.1 (Observed) |
| Holdout v3 | 46/48 at first contact (§10) | `721fffe` (Observed) |
| Package version / ABI | 0.3.1 / ABI 1, unchanged | `check-versions.sh`, `check-abi.sh` |

Not covered by the preflight: `tests/pty_demos::hack_shell_commands_trigger_real_effects` in the
**release** profile with a prebuilt example (§11.5); it passed in the preflight's debug `cargo test`.

## 15. Unresolved TODOs

- Choose the library default (`WRITTEN` today; `BAND` is the hardened candidate) — Disclosed decision.
- The named reds in §11; a swung-groove-across-swing holdout row with song identity.
- `cover.rs` (2,286 lines) and `cover_fidelity.rs` (1,033) were not split: the semantics are now fixed, so a
  split is a pure move for a later, separately reviewable commit.
- A passing-tone relation for pinned attacks; recording action windows in the seat quotient.
- Human feel remains only a typed identity seam.
- PR #70 body/title are stale (not edited; the PR stays draft).

## 16. Reasons NOT to merge (if any hold for you)

- **Listening is the acceptance test and it is open.** No machine result shows that BAND sounds better than
  POCKET, that the drummer serves the band, or that any Ode fidelity "is the same song".
- **The recommended profile is not the default.** Merging ships `BAND` as an opt-in.
- **Holdout v3 is 46/48 at contact**; its two failure families were repaired only after contact, so they are
  validated by fresh-seed falsifiers, not by a holdout.
- BAND's sound changed this round (support release, approaches, pad rooting) without a listen; only POCKET's
  bytes are frozen.
- The module is large (67,969 lines) and experimental; its API is not stabilized.

## 17. Listening questions for the maintainer

1. **Drums:** at Balanced, does the drummer feel like part of the band?
2. **BAND vs POCKET:** do the earned support laws (no tails into foreign chords, approaches into what the bass
   actually plays, the carried root) sound like a tighter band, or more cautious?
3. **Ode:** which fidelity crosses from "new music using Ode material" to "obviously the same song"?
4. **Swing partial:** are the partial-reference outputs still useful as fresh songs?
5. **Pocket preservation:** does the accepted BLACK_ICE result still sound right? (It is byte-identical.)
6. **Merge review:** another round, or is this the branch to merge? *(Not answered here.)*
