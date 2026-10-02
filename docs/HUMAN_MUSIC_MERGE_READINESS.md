# HumanMusic — merge readiness

This document exists so the maintainer can decide whether `feat/v0.4-humanmusic-audio` goes to `main`.
It does **not** decide that. Green engineering gates are necessary, not sufficient: the listening
questions at the end are open, and the reasons to postpone are listed without softening.

Labels: **Observed** (read from a committed receipt or a command run at the stated source),
**Corroborated** (two independent routes agree), **Conjectured**, **UNVERIFIED** (no evidence either way),
**Refuted**, **Disclosed** (a stated design/decision, not a measurement).

## 1. Branch and head

- **Machine state before this round (VAPORIZE):** head `ef6a7e0`, final closeout library source
  `abc3f9f`; holdout v6 28/28 (503 checks, 0 failing); 485,568 / 485,568 fresh BAND performances
  admitted; R17 48/48 WAV + 757/757 receipts and the 16-WAV cover corpus byte-identical; hosted CI and
  Release Preflight green. The semantic architecture was closed.
- **Why VAPOR95 was reopened (Disclosed):** the maintainer's listen called the VAPOR95 palette "kinda
  trash" — generic lush synthwave, not classic vaporwave. This round is the one deliberate pre-main
  aesthetic intervention: the VAPOR95 *world* (its dialect) and the production physics every world may
  declare. No feature, no composer change, no song change (§6).
- Commits (all pushed in order): `8e99c14` v1 before-image · `1268e1d` audit · `081512f`
  `WorldProduction` (byte-exact refactor) · `d175090` the VAPOR95 revision · `e5e5969` search device ·
  `64664bc` post-freeze search · `6512cd3` holdout v7 declaration · `7d5399d` v7 first contact ·
  `c02a3e3` known R17 sweep on the archival world · this document. Nothing was merged to `main`,
  tagged, released or version-bumped; no history was rewritten; PR #70 stays a draft.
- **Final library source: `d175090`** (frozen; every later commit is test/docs only).
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

World production law (`world::WorldProduction`, declared by the world, executed by the synth; a
`None` stage is not computed — BLACK_ICE and SWISS_SIGNAL declare none of the new stages):

```
pad + keys + lead ─► MemoryChorus ─┐
bass ──────────────────────────────┼─► + drums ─► tanh ─► + TempoEcho returns ─► Space (input low-cut) ─► comp ─► limiter
keys + lead (send) ─► tone LP ─► L/R damped delays at note values (EchoTime → seconds at the DSP boundary)
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
| VAPORIZE additions | `world::{WorldProduction, Space, MemoryChorus, TempoEcho, EchoTime}` (the `MusicWorld` fields `reverb_size`/`reverb_damp`/`reverb_mix`/`saturation` moved into `production` — a breaking change inside the experimental module), `synth::ProductionControl::{no_chorus, no_echo, full_band_space}` (`nochorus`, `noecho`, `fullband`), `dsp::fx::Delay::set_damping`, `dsp::fx::Reverb::process_stereo_send` | Neutral production is skipped, not computed: BLACK_ICE, SWISS_SIGNAL and the archival VAPOR95 v1 render byte-identically. |
| Closeout additions | `pitch::{classify_earned, classify_under}`, `identity::keeps_held_identity`, `Score::keys_voicing_edits` (printed only when non-empty; outside the canonical event fingerprint), `comp::PadVoicingReason::Release`, `motif::LineRequest::earned` | Archived paths call the unchanged `classify` / `keeps_identity`; `keys_voicing_edits` is empty off BAND. |

## 4. Fingerprints and historical compatibility (Observed)

- Canonical v2 fingerprints: every new field is encoded **only when non-historical** — profile `harmony`,
  `functions`; plan `rehearsal`, `functions`; `CoverMap.projection` (Lane unencoded). `SongMap.composed_by`
  is encoded in neither song fingerprint (provenance is not identity). Every historical profile, plan and
  v1 cover map keeps its hash.
- Legacy Debug/FNV: `PerformancePlan` and `Score` print new fields (`rehearsal`, `functions`,
  `stroke_origins`) only when present; pinned characterization fingerprints are unchanged.
- Historical entry points remain thin adapters over `PerformanceProfile`; nothing historical reads a new law.
- **VAPOR95 v1 (archival) → VAPOR95 (current)** is an intentional sonic-world revision
  (`docs/HUMAN_MUSIC_VAPOR95.md` §4). The archival world is a fixture, never a `WorldId`:
  `tests/common/vapor95_v1.rs` (tests and lab examples, via `#[path]`) and a `#[cfg(test)]` copy for unit
  tests, both pinned to one committed dump (`humanmusic-vaporize/v1-baseline/world-v1.txt`). Every record
  derived under v1 reproduces through it (§5); every test whose VAPOR95 case was derived under v1 (R7b/R8
  pins, closeout and earlier falsifiers, the consolidation flip witnesses, the R15 expression window,
  the closeout listening device, the known R17 sweep) runs under it.

## 5. Historical evidence after the intentional world change (distinct gates) — Observed

| Evidence | Disposition | Result (library source `d175090`) |
| --- | --- | --- |
| R17 accepted pocket (BLACK_ICE + SWISS_SIGNAL) | must stay exact | `pocket_music_lab --render` + `verify-pocket-freeze.py`: **48/48 WAV + 757/757 receipts byte-identical** (after `081512f` and `d175090`) |
| Cover listening corpus (16 WAVs, 3 VAPOR95) | exact via the archival world | `cover_music_lab --vapor95=v1` ×3: **16/16 byte-identical** |
| VAPOR95 v1 style baseline (30 WAVs, 5 BAND takes) | exact via the archival world | `vapor95_style_lab --record --world=v1`: **30/30 + every fingerprint** |
| Closeout listening corpus | exact via the archival world | falsifiers **25/25**, drum A/B **20/20**, Ode fidelity **8/8** (`--vapor95=v1`) |
| Known R17 sweep | exact via the archival world | 120 / 61 / 59 (14/38/5/2); every first reason and passing label identical (`c02a3e3`) |
| Default VAPOR95 renders, VAPOR95 listening hashes | **intentionally replaced** | new corpus `humanmusic-vaporize/corpus/` (110 WAV hashes) |
| Holdouts v1–v6 | preserved at their declaration commits | their harnesses call the product world; re-running them at a later head is not their evidence |

The `WorldProduction` refactor (`081512f`) was proven byte-exact before any VAPOR95 value changed
(R17, cover 16/16, v1 30/30, bit-exact unit tests of the extended `Delay`/`Reverb`). An adversarial
regression audit (fork B) found no production leak into the Score: nothing outside `synth.rs`/`world.rs`
reads `WorldProduction` or `ProductionControl`; timbre and mix knobs are read only by the synth (plus one
report-only exposure proxy); the legitimate world-relative changes are the patch envelopes (sounding
tails feed the BAND laws) and the tempo. It found six v1-derived falsifier tables still on the product
world; all were moved to the fixture before the revision was committed.

## 6. What VAPORIZE changed

Full record: `docs/HUMAN_MUSIC_VAPOR95.md` (audit §1, production law §2, ranges §3, revision record §4).

- **Production law** (`081512f`): `WorldProduction { saturation, space { size, damp, mix, low_cut_hz },
  chorus: Option<MemoryChorus>, echo: Option<TempoEcho> }`; echo times are note values (`EchoTime`),
  converted with the Score's tempo at the DSP boundary; every repeat darker (`Delay::set_damping`); the
  room's input low-cut keeps the low end dry and mono (`Reverb::process_stereo_send`); bass and drums
  never enter the chorus or the echo. Reuses `Chorus`, `Delay`, `Reverb`, `Svf`, `soft_saturate`,
  `Compressor`, `Limiter`; new glue: the types and the synth's `TempoEchoUnit`.
- **The VAPOR95 world** (`d175090`): keys become the source object (glassy ratio-3 FM EP, bright
  filter-envelope attack); a supportive two-saw pad; a soft `Fm{2, .15}` lead; a filtered-saw bass at
  the written pitch (no sub-octave); a softer machine kit; memory chorus 0.32 Hz / 3.5 ms; tempo echo
  L dotted eighth / R quarter, feedback .38, darker each repeat; room 1.2 / damp .55 / .28 with a
  180 Hz low-cut; saturation 2.2 → 1.4; mix level-matched to v1 within ~1 dB; **tempo 71 → 84 BPM, swing
  kept 0.16 (54 %)**. Key, mode, vocabulary, voicing spread, groove family, drum density and dynamics
  are unchanged; the SongMap is untouched (a world is a dialect, not a composer).
- **Measured** (five takes, v1 → current): low band L/R correlation 0.66–0.80 → 0.97–0.99; pad − keys
  where both sound −0.1…−1.3 → −5.6…−7.0 dB; bass 120–500 Hz energy 1.2–2.7 % → 23–27 %; drums centroid
  1.0–1.7 → 1.9–3.0 kHz; drums − keys +2.0…+9.3 → −1.7…+4.4 dB; ceiling contact 0 → 0.
- **Same-Score palette A/B:** the five v1 Scores rendered through the new palette keep their `score=`
  fingerprints. **Full world:** 5/5 BAND-admitted under the new world, SongMap and PerformancePlan
  fingerprints unchanged. **Natural transformation** (`tests/audio_vapor95_palette.rs`): v1 / VAPOR95 /
  BLACK_ICE / SWISS_SIGNAL takes of each SongMap keep identical song coordinates.
- Aesthetic success: **UNVERIFIED** — the maintainer listens (§17).

## 6b. What the closeout round changed — every repair red-first

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

- **v1-v6** are immutable known evidence; nothing this round was fitted to them (the closeout-round
  entries below are kept as they were recorded).
- **v5** (`holdout-v5/`): 28 rows declared at `7110cee` (config SHA256 `c52d0421…`; source `3146438`)
  after an adversarial pre-contact declaration review (no row change; harness corrections applied).
  First contact **28/28, 503 checks, 0 failing** — preserved. Not final-head evidence: the source changed
  after contact (§6, ring-out).
- **v6** (`holdout-v6/`): v5's design on fresh `978xxxxx` seeds, declared at `04c5c26` (config SHA256
  `3a5c29e1…`; final source `abc3f9f`). First contact **28/28, 503 checks, 0 failing**: all 21 generated
  BAND sources pass their own receipt; 6/6 predicted lifts admitted; every admitted cover passes
  `PerformanceReceipt + CoverConformance`; 4 lawful refusals in the family (V08 pinned attacks; V16,
  V21, O02 vocabulary); V17/S03 nothing to cover; cross-swing quotient PASS on all 12 Groove covers.
- **v7** (`humanmusic-vaporize/holdout-v7/`): v6's design on fresh `984xxxxx` seeds, three rows biased
  toward the revised VAPOR95 (V02 SWISS → VAPOR95, V06 VAPOR95 one-bar bass figure, V17 VAPOR95 →
  SWISS straight; VAPOR95 in 15/28 rows), declared at `6512cd3` (config SHA256 `3eb9b3a2…`) after a
  pre-contact review that ran v7's own `run_case` on 112 probe cases (no 984 seed; 112/112, no row
  change). First contact at the final library source `d175090`: **28/28, 510 checks, 0 failing** — 21/21
  sources pass their receipt; 6/6 predicted lifts admitted (incl. V12 VAPOR95 → VAPOR95); lawful
  refusals V16, V21, O02 (vocabulary); V17, S03 nothing to cover; cross-swing quotient 12/12.
- **Post-freeze broad BAND search** at `d175090` (`humanmusic-vaporize/search/`): **1,824,756 /
  1,824,768 admitted, 0 refused, 12 rejected, 0 panicked** — F all worlds nominal 165,888/165,888;
  G VAPOR95 nominal/66/96/120/150 552,948/552,960; H VAPOR95 + archival v1 at 96–150 884,736/884,736;
  I BLACK_ICE + SWISS at 132/150 221,184/221,184. The 12 are one pre-existing song (§11).
- **Exploratory BAND searches** (`tests/audio_band_search.rs`, `perform_checked` under BAND, every
  world × language × grammar × composer × trace × length × drum restraint; seeds `968xxxxx`-`969xxxxx`,
  unused by v1-v6): at the final source, sets A-D (re-validation of the sets that guided the third
  wave; tempi 60/66/96/150, lengths 1.75-64 incl. partial bars) **340,416 / 340,416 admitted**, and the
  never-fitted set E (12 seeds, nominal + 132 BPM) **145,152 / 145,152 admitted** — **0 refused, 0
  rejected, 0 panicked**.

## 11. Remaining internal reds

**One pre-existing family, preserved and classified, not repaired** (`humanmusic-vaporize/search/`):
VAPOR95 above ~90 BPM, one song in the post-freeze search — seed `98300007`, deflected trace, 33.25 beats,
DeflectedLift, StructuralR9, fusion — rejected with one held-identity flip at 96, 120 and 150 BPM under
every drum restraint (12 / 1,824,768); admitted at 66 and the nominal 84. **Not introduced by this
round:** the archival v1 world rejects the same song at the same tempi (present at the closeout
source), and 884,736 further fast-tempo performances (set H, both worlds) admit. Mechanism (instrumented
probe, nothing kept): a Thin bar (pad silent), the keys hold rootless F4-C5-E5, the bass leaves D for
A2 → A minor held 1.4 s; the keys' identity law offers the right root (E5 → D5, accepted at 84 BPM), but
the hold-law acceptance vetoes it because the D5 release tail crosses 0.72 beats into the next harmony,
where four instants turn from Rooted to a sub-hold Passing F major (longest 0.498 s). The closeout rule
permits only Implied → Passing ("a root lost anywhere vetoes"). Repairing it means widening that
deliberate rule — a semantic change this aesthetic round was not licensed to make. No v7 row exercised it.

Historical arms keep their archived behaviour by design (POCKET/R17 byte-exact; the known R17 sweep's
59 first failures are characterized, not regressions). Cover: `UnprojectableTiming` on DeflectedLift
grooves and `ConflictingPins` for a kit heard only in action windows remain known (outside the
lawful family by design — v5/v6/v7 never hit them).

## 12. Lawful explicit refusals (not bugs)

- `VOCABULARY_REFUSAL` — a chart chord outside the world/language vocabulary (BAND planner).
- `pinned harmony [family] outside target vocabulary`; `no lawful harmony contains the pinned simultaneous attacks`.
- `a pinned lead/bass event has no lawful pitch function in the target harmony`.
- `a pinned theme has no seated phrase to be stated in the target form`.
- `ConflictingPins`, `MissingAxis` (the source never established the axis), and "nothing to cover".

## 13. Fixture footprint (Observed)

VAPORIZE adds `humanmusic-vaporize/` (396 KB: `v1-baseline/` record + 30 hashes + world dump,
`corpus/` 5 lab reports + 110 hashes, `search/` summary + 12 rows, `holdout-v7/` config, README,
CLASSIFICATION and 31 first-contact files); every WAV stays local under `target/`. The closeout round
added `holdout-v5/` and `holdout-v6/` (config, README, CLASSIFICATION, 31 first-contact files
each, ~284 KB each) and `listening-closeout/` (README, CHANGES, WAV SHA256SUMS; WAVs stay local).

## 14. Gates at the final source

| Gate | Result | Where |
| --- | --- | --- |
| Full test suite (all features, no fail-fast) | **1,519 passed, 0 failed, 17 ignored** (97 binaries; the known PTY flake did not fire) | local, library source `d175090` (before the test-only commits; the preflight re-runs it at the final head) |
| Strict clippy, all targets/features | clean (1.98.1) | local |
| R17 PCM freeze | **48/48 WAV + 757/757 receipts byte-identical** | local, after `081512f` and `d175090` |
| Cover listening corpus | **16/16 WAVs byte-identical** (`--vapor95=v1`) | local, after `081512f` and `d175090` |
| VAPOR95 v1 baseline / closeout corpus | 30/30; 25/25 + 20/20 + 8/8 (archival world) | local, `d175090` |
| Known R17 sweep replay | 120 / 61 / 59 (14/38/5/2); all first reasons and passing labels identical | local, `c02a3e3` |
| Holdout v7 | **28/28 at first contact, 510 checks, 0 failing** | `6512cd3` (library `d175090`) |
| Post-freeze broad BAND search | 1,824,756 / 1,824,768 admitted; 12 rejected = one pre-existing song (§11) | `d175090` |
| Release preflight (13 steps), GitHub CI, Release Preflight | see the PR / final report (run at the exact final commit) | local + hosted |

## 15. Unresolved technical debt (exact)

- **Fast-tempo VAPOR95 identity veto (§11).** The keys' hold-law acceptance forbids trading a held
  flip for sub-hold passing rivals in a neighbouring harmony; at ≥ 96 BPM a long keys release can make
  the right answer look like that trade. 12 / 1,824,768 performances, one song; pre-existing.
- **Production is outside the Score model (Disclosed).** BAND receipts reason over the Score's notes
  and patch release tails; echo repeats, chorus and room tails are acoustic treatment, as the reverb
  always was. An echo of a previous chord can sound into the next harmony; no receipt hears it.
- **Render tail.** The synth renders a fixed 2.5 s after the last beat; with the new echo the last
  second of a render sits about 38 dB below the render's mean level (hookarc; v1: about 51 dB), so the
  final repeats are still decaying when the file ends. The tail length is not production-aware.
- **Historical harnesses on the product world.** Holdout harnesses v1–v6 and the ignored
  `r16_hostile_world_seed_tempo_sweep` call `MusicWorld::vapor95()`; re-running them at a later head is
  not their recorded evidence (each is preserved at its declaration commit).
- **API break inside the experimental module.** `MusicWorld.{reverb_size, reverb_damp, reverb_mix,
  saturation}` moved into `MusicWorld.production`.
- **Approach side of the earned appoggiatura.** The source bounds the destination (≤ 1 beat, core tone,
  in domain); the observer also requires the approach note to end ≤ 1 beat before and come from an
  unambiguous bundle. Pre-existing; never observed.
- **Bass-figure guard vs the bass realizer.** The planner's guard ignores unison lines; a unison on
  the only free downbeat would make the receipt reject (never falsely admit). Never observed.
- **A tail release triggered inside a rooting trial** is itemized only as the Rooting edit.
- **Responsibility bounds:** `PAD_ANSWER_PASSES = 4`, `KEYS_ANSWER_PASSES = 4`.
- **PTY demo flake** (`pty_demos::hack_shell_commands_trigger_real_effects`, a 3 s PTY timing
  assertion under full-suite load; passes alone) — environmental, unrelated to audio.
- Library default is still `WRITTEN`; `BAND` is the hardened candidate — a maintainer decision.
- `cover.rs` / `cover_fidelity.rs` / `comp.rs` not split (semantics first); human feel remains a typed seam.

## 16. Reasons NOT to merge (if any hold for you)

- **Listening is the acceptance test and it is open.** No machine result shows that the new VAPOR95
  sounds like vaporwave, that BAND sounds better than POCKET, or that the drummer serves the band.
- **VAPOR95 changed this round**, deliberately and audibly; the maintainer may want a final parameter
  adjustment (tempo cell, echo amount, pad level) before it ships.
- **One known internal BAND red** (§11), pre-existing and rare, in the fast-tempo VAPOR95 regime.
- BAND ships as an opt-in, not the default; the module is large and experimental; its API is not
  stabilized.

## 17. Listening — the files the maintainer actually needs (8)

All under `target/humanmusic-vaporize/corpus/` (regenerate with `vapor95_style_lab`; hashes in
`docs/fixtures/humanmusic-vaporize/corpus/WAV_SHA256SUMS`). One song (`hookarc`) through every step, then
a familiar tune:

1. `v1/hookarc.full.wav` — the old VAPOR95.
2. `palette/hookarc.full.wav` — **the same notes**, new palette.
3. `current/hookarc.full.wav` — the new world (84 BPM, light swing): the landed default.
4. `candidates/hookarc.t78_s16.full.wav` — candidate: slower, 78 BPM.
5. `candidates/hookarc.t84_s32.full.wav` — candidate: more swing (58 %).
6. `candidates/hookarc.t90_s00.full.wav` — candidate: 90 BPM straight.
7. `v1/ode_faithful.full.wav` and 8. `current/ode_faithful.full.wav` — Ode to Joy, old vs new world.

Stems and single-factor ablations beside them localize anything that sounds wrong. Questions:

1. Does new VAPOR95 finally feel like a half-remembered euphoric 80s/R&B recording being reconstituted by
   a computer, rather than a generic "vapor synth" preset?
2. Is the pulse slow and narcotic while still bouncing?
3. Are chorus/echo/reverb creating MEMORY rather than mud?
4. Are the FM/keys/source-object foreground enough, with the pad supporting rather than swallowing it?
5. Does the drum machine sit behind the remembered song rather than trying to be the song?
6. Which candidate, if more than one survives, deserves to be canonical VAPOR95?

Still open from earlier rounds: the closeout's BAND changes (`listening-closeout/`), drums, Ode fidelity
levels, Swing partial, BAND vs POCKET, and whether this branch goes to main.
