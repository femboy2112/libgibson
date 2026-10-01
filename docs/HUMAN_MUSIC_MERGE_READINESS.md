# HumanMusic — merge readiness

This document exists so the maintainer can decide whether `feat/v0.4-humanmusic-audio` goes to `main`.
It does **not** decide that. Green engineering gates are necessary, not sufficient: the listening
questions at the end are open, and the reasons to postpone are listed without softening.

Labels: **Observed** (read from a committed receipt or a command run at the stated source),
**Corroborated** (two independent routes agree), **Conjectured**, **UNVERIFIED** (no evidence either way),
**Refuted**, **Disclosed** (a stated design/decision, not a measurement).

## 1. Branch and head

- The closeout round started at `9204ef4` (remote = local; the semantic-consolidation and pre-main
  rounds' head). Everything below was authored on `feat/v0.4-humanmusic-audio`, pushed in coherent
  increments. Nothing was merged to `main`, tagged, released or version-bumped; no history was
  rewritten; PR #70 stays a draft.
- **Final library source: `abc3f9f`** (frozen; every later commit is test/docs only). Holdout v6 was
  declared at `04c5c26` and contacted once there; the listening corpus and this document follow.
- Package `libgibson` 0.3.1, ABI 1, MSRV 1.85 — **unchanged**. Everything HumanMusic is under the
  experimental `gibson::audio::human_music` module; no C/Python/Go ABI surface was added.

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

Closeout laws (all under `FunctionPolicy::Earned`, i.e. BAND; every archived path unchanged):

```
support identity:   pad answers first (rooting, remove-first, hold-law acceptance, to a fixed
                    point; lets go of tails that complete another chord) → keys answer what the
                    band, with the pad's answer, still holds as another chord → keys ↔ bass reach
                    a fixed point (the bass always hears the final keys) → pad hears the final band
relational function: pitch::classify_earned — destination sounds inside PerformanceDomain;
                    appoggiatura resolves ≤ 1 beat, by step, onto a CORE tone at the destination
bass figure room:   interaction planner — the bass's borrowed material never takes the last
                    downbeat its own line could sound on where BassFigure is declared
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
| Closeout additions | `pitch::{classify_earned, classify_under}`, `identity::keeps_held_identity`, `Score::keys_voicing_edits` (printed only when non-empty; outside the canonical event fingerprint), `comp::PadVoicingReason::Release`, `motif::LineRequest::earned` | Archived paths call the unchanged `classify` / `keeps_identity`; `keys_voicing_edits` is empty off BAND. |

## 4. Fingerprints and historical compatibility (Observed)

- Canonical v2 fingerprints: every new field is encoded **only when non-historical** — profile `harmony`,
  `functions`; plan `rehearsal`, `functions`; `CoverMap.projection` (Lane unencoded). `SongMap.composed_by`
  is encoded in neither song fingerprint (provenance is not identity). Every historical profile, plan and
  v1 cover map keeps its hash.
- Legacy Debug/FNV: `PerformancePlan` and `Score` print new fields (`rehearsal`, `functions`,
  `stroke_origins`) only when present; pinned characterization fingerprints are unchanged.
- Historical entry points remain thin adapters over `PerformanceProfile`; nothing historical reads a new law.

## 5. R17 accepted pocket and historical evidence (distinct gates) — Observed

After **every** library commit of this round (`a8a527f`, `6652663`, `fb8e907`, `239da10`, `938ab3e`,
`abc3f9f`): `pocket_music_lab --render` + `scripts/verify-pocket-freeze.py` → **48/48 WAVs and 757/757
receipts byte-identical**; the 16 cover-listening WAVs byte-identical. The known R17 120-case sweep
(`r17_fresh_world_seed_tempo_sweep`, rustc 1.98.1) replays exactly at the final source (run at `184ad4b`,
whose library source is `abc3f9f`; also at `938ab3e`): 61 passes / 59 first failures (14/38/5/2), every
first reason and every passing label identical (calibrated `parse` of
`humanmusic-consolidation/known-r17-replay/classify.py`). Every
new branch is gated on `FunctionPolicy::Earned`; un-gated refactors (`rooting_candidates_in`,
`choose_rooting`, `flipped_over`, the keys `*_on` realizers, `PitchContext` call sites) are
behaviour-identical, as the byte gates show.

## 6. What the closeout round changed — every repair red-first

| Family | Mechanism (Observed) | Law (source level, BAND) | Red → fix |
| --- | --- | --- | --- |
| Bass-figure room (NEW, not named by the brief; 282/38,880 short-form performances) | In one-bar fusion forms a bass-initiated verb's borrowed motif fragment starts on the bar's only downbeat — the one onset the bass's own line is guaranteed — so the declared `BassFigure` never sounds; under `Planned` a lone root survived only as part of an ensemble Push nobody else performed, which the rehearsal rightly strikes. | The bass's borrowed material (figures, answers) never takes the last bar downbeat its own line could sound on where the contract declares `BassFigure` — decided in the interaction planner; RNG-neutral (responses judged after their draw). | `ef7e429` → `a8a527f` |
| Truncated "Appoggiatura" (60/7,200 in the family slice) | **The brief's hypothesis is Refuted:** truncation does not destroy the resolution. The destination E6 sounds a step away, mid-form (beat 25.5), inside the domain; the source claimed `Appoggiatura` because E is Dm9's SPELLED 9th, while the observer requires a CORE tone. Truncation only moves the cadential Dm9 under the motif. | `pitch::classify_earned`: every stepwise-path function needs its destination to sound inside `PerformanceDomain`; an appoggiatura resolves within one beat, by step, onto a core tone of the harmony at the destination. Same relation as the observer, implemented independently; the line engine's search picks the next justified pitch (no relabelling, no special ending). Reaches expression's `reclassify` and the cover's `classify_source`. | `37d5c4b` → `6652663` |
| Held-identity flips (two shapes, then a second and third wave from fresh searches) | (P) the pad's rooting law found the root and its strict pointwise acceptance vetoed it over a 0.15-beat sub-hold overlap; (K) a Thin bar silences the pad and the keys hold a rootless shell — nobody answers for the chord; (2nd wave) a rival completed by the pad's release tail, and a swell whose root enters late; (3rd wave) a root vetoed because its tail joins a neighbour's flip the same sweep fixes next, a root whose tail trades one flip for another, and a silent bar whose rival is the pad's consonant tails. | Harmonic identity is the responsibility of the support that sounds the harmony: the pad answers first (hold-law acceptance `keeps_held_identity` — Implied→sub-hold Passing is the only permitted worsening; remove-first preference; a fixed point of sweeps; tails let go where they complete another chord), then the keys answer whatever the band with the pad's answer still holds (same candidates and acceptance, `choose_rooting` shared; a keys edit counts only if the root sounds), the keys and the bass reach a fixed point. No forced pad, no global root, rootless voicings kept wherever nobody hears another chord. | `74e55cd` → `fb8e907`; audit `239da10`; `22c8a82` → `938ab3e` |
| Consolidation audit (one adversarial pass over the changed boundaries) | No observed defect; four latent ones: keys judged against the bass's first take; a keys "rooting" recorded with no sounding root; duplicated selection loops; the bass guard stated inexactly. | Keys ↔ bass fixed point; root must sound; one `choose_rooting`; exact guard. Recorded, not changed: the source's appoggiatura bounds only the destination side (the observer also bounds the approach gap) — pre-existing, never observed. | `239da10` |
| Final-chord ring-out (found by the listening diff AFTER holdout v5) | `938ab3e`'s refactor replaced `context_at` (which answers the final harmony past the piece's end) with an index lookup that has none, so the final harmony's consonant pad tail counted as excluded and was cut — 360/450 BAND performances (0/450 before). No law states it; every receipt still passed. | Restore the archived lookup exactly; a release concerns only another harmony a tail rings into. | `c875d0d` → `abc3f9f` |

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

Unchanged APIs. **Correction (Observed):** "Ode Strict is an explicit lawful refusal" holds only in
`cover_fidelity_lab` (seeds 901/904, 95 BPM); in holdouts v5/v6, O03 (Ode Strict → BLACK_ICE fusion)
**lifts and conforms**. Which Ode level "is the same song", and whether Balanced sounds like a band
member: **UNVERIFIED** (listen).

## 10. Holdouts and searches

- **v1-v4** are immutable known evidence; nothing this round was fitted to them.
- **v5** (`holdout-v5/`): 28 rows declared at `7110cee` (config SHA256 `c52d0421…`; source `3146438`)
  after an adversarial pre-contact declaration review (no row change; harness corrections applied).
  First contact **28/28, 503 checks, 0 failing** — preserved. Not final-head evidence: the source changed
  after contact (§6, ring-out).
- **v6** (`holdout-v6/`): v5's design on fresh `978xxxxx` seeds, declared at `04c5c26` (config SHA256
  `3a5c29e1…`; final source `abc3f9f`). First contact **28/28, 503 checks, 0 failing**: all 21 generated
  BAND sources pass their own receipt; 6/6 predicted lifts admitted; every admitted cover passes
  `PerformanceReceipt + CoverConformance`; 4 lawful refusals in the family (V08 pinned attacks; V16,
  V21, O02 vocabulary); V17/S03 nothing to cover; cross-swing quotient PASS on all 12 Groove covers.
- **Exploratory BAND searches** (`tests/audio_band_search.rs`, `perform_checked` under BAND, every
  world × language × grammar × composer × trace × length × drum restraint; seeds `968xxxxx`-`969xxxxx`,
  unused by v1-v6): at the final source, sets A-D (re-validation of the sets that guided the third
  wave; tempi 60/66/96/150, lengths 1.75-64 incl. partial bars) **340,416 / 340,416 admitted**, and the
  never-fitted set E (12 seeds, nominal + 132 BPM) **145,152 / 145,152 admitted** — **0 refused, 0
  rejected, 0 panicked**.

## 11. Remaining internal reds

**None observed** at the final source: 485,568 BAND performances on seeds unused by any holdout admitted
(0 refused, 0 rejected), holdout v6 28/28.
Historical arms keep their archived behaviour by design (POCKET/R17 byte-exact; the known R17 sweep's
59 first failures are characterized, not regressions). Cover: `UnprojectableTiming` on DeflectedLift
grooves and `ConflictingPins` for a kit heard only in action windows remain known (outside the
lawful family by design — v5/v6 never hit them).

## 12. Lawful explicit refusals (not bugs)

- `VOCABULARY_REFUSAL` — a chart chord outside the world/language vocabulary (BAND planner).
- `pinned harmony [family] outside target vocabulary`; `no lawful harmony contains the pinned simultaneous attacks`.
- `a pinned lead/bass event has no lawful pitch function in the target harmony`.
- `a pinned theme has no seated phrase to be stated in the target form`.
- `ConflictingPins`, `MissingAxis` (the source never established the axis), and "nothing to cover".

## 13. Fixture footprint (Observed)

This round adds `holdout-v5/` and `holdout-v6/` (config, README, CLASSIFICATION, 31 first-contact files
each, ~284 KB each) and `listening-closeout/` (README, CHANGES, WAV SHA256SUMS; WAVs stay local).

## 14. Gates at the final source

| Gate | Result | Where |
| --- | --- | --- |
| Full test suite (release, all features, no fail-fast) | **1,516 passed, 1 failed, 17 ignored**: the one failure is `pty_demos::hack_shell_commands_trigger_real_effects` (a 3 s PTY round-trip timing assertion under full-suite load; no audio code; unchanged since `9204ef4`), which **passes when run alone** (Observed twice) | local, `abc3f9f` |
| R17 PCM freeze | **48/48 WAV + 757/757 receipts byte-identical** | local, after every library commit |
| Cover listening corpus | **16/16 WAVs byte-identical** | local, after every library commit |
| Known R17 sweep replay | 120 / 61 / 59 (14/38/5/2); all first reasons and passing labels identical | local, `184ad4b` (library source `abc3f9f`); also at `334a70b` |
| Hosted toolchain drift (Disclosed) | The GitHub runners moved `stable` to **Rust 1.99.0** on 2026-10-01; clippy 1.99's `needless_borrows_for_generic_args` fired on pre-existing, non-audio code (`examples/temporal_video_compositor/bake.rs:163`, `map_err(&scrub)`), turning every hosted CI and Release Preflight run from `239da10` to `39d838d` red (the `a8a527f` CI red is the PTY flake on 1.98.1). Fixed by passing the `Copy` closure by value; strict clippy and fmt are clean locally on **both** 1.99.0 and 1.98.1, rustdoc `-D warnings` clean on 1.99.0. | local |
| Release preflight (13 steps) | see the PR / final report (run at the final commit) | local |
| GitHub CI / Release Preflight | see the PR (must be green at the exact final commit) | hosted |
| Holdout v6 | 28/28 at first contact | `04c5c26` |
| Broad fresh BAND search | 485,568 / 485,568 admitted | `abc3f9f` |

## 15. Unresolved technical debt (exact)

- **Approach side of the earned appoggiatura.** The source bounds the destination (≤ 1 beat, core tone,
  in domain); the observer also requires the approach note to end ≤ 1 beat before and come from an
  unambiguous bundle. Pre-existing; never observed in 485,568 performances.
- **Bass-figure guard vs the bass realizer.** The planner's guard reads the planner's stage and the
  bass's own calls/answers; the realizer additionally excludes unison lines. A unison on the only free
  downbeat would make the receipt reject (never falsely admit). Never observed.
- **A tail release triggered inside a rooting trial** is carried on the pad's path but itemized only as
  the Rooting edit at the harmony before (no separate `Release` edit for the next harmony).
- **Responsibility bounds:** `PAD_ANSWER_PASSES = 4`, `KEYS_ANSWER_PASSES = 4` (fixed points that
  converged in every observed case; a non-converging case would be a receipt rejection, not an admit).
- **PTY demo flake** (§14) is environmental and unrelated to audio.
- Library default is still `WRITTEN`; `BAND` is the hardened candidate — a maintainer decision.
- `cover.rs` / `cover_fidelity.rs` / `comp.rs` not split (semantics first); human feel remains a typed seam.

## 16. Reasons NOT to merge (if any hold for you)

- **Listening is the acceptance test and it is open.** No machine result shows that BAND sounds better
  than POCKET, that the keys' or pad's identity answers sound musical, or that the drummer serves the band.
- **BAND changed again this round** (12 audition songs in `listening-closeout/`), and the merge ships
  `BAND` as an opt-in, not the default.
- **The round found an accidental change after its first holdout** (the ring-out) — caught by listening
  diffs, not by any gate. Repaired and re-held out (v6), but it shows the gates do not see everything
  that sounds.
- The module is large and experimental; its API is not stabilized.

## 17. Listening questions for the maintainer

1. **Support identity:** do the keys' and the pad's added roots (`identity_*`, before/after) sound like
   a band keeping the chord, or like a correction?
2. **Bass figure:** in the one-bar forms, is the bass stating its root on the downbeat (instead of
   quoting the lead) right — and is the keys' resulting hold acceptable?
3. **Relational function:** the lead's F#6→E6 became E6→D6 at beat 25 — a loss or a fix?
4. Unchanged and still open: drums (does Balanced feel like part of the band?), Ode fidelity (which
   level is "the same song"?), Swing partial usefulness, BAND vs POCKET, whether this branch goes to main.
