# HumanMusic — C137 hardening & MeatsackProjection: evidence ledger

> The reproducible evidence ledger for the principal-agent program that hardens the accepted
> Rick-C137 semantic-band-story frontier (MEGA-ROUNDS **C137-A**, **C137-B**) and then builds the
> minimal **MeatsackProjection** seam (**MP-0+**). Red-first, smallest-first, defaults-OFF. It
> records exact SHAs, source sites, hypotheses, predeclared outcomes, actual results, residual
> reds, and the next verdict-changing step — so any claim here is checkable and nothing rests on a
> story. Companion to `HUMAN_MUSIC_MEATSACK_PROJECTION_BOUNDARY.md` (the cut) and
> `HUMAN_MUSIC_MEATSACK_PROJECTION_PLAN.md` (the ladder).

## Frozen baseline (reconstructed 2026-10-09)

| object | value |
|---|---|
| `origin/main` | `80706c9` — branch-protected (6 required ci.yml checks); never touched |
| active C137 branch | `feat/v0.5-humanmusic-beefup` @ **`530f400`** (local == origin), tree clean |
| C137 PR | **#87**, OPEN, **draft**, base `main`, MERGEABLE — no merge/tag/release |
| research Meatsack | `origin/research/humanmusic-meatsack-projection` @ **`b9cccb5`** — remote-only, **kept UNCHANGED** (independent provenance) |
| v0.4.0 tag | `38ba12d` |
| CI on `530f400` | **7/7 green** (ASan, Bindings, Go, MSRV, PTY, Rust, Symbolic — all `completed/success`) |

**CI note (resolved honestly):** the adoption commit `8036963` shows 5 lanes `cancelled`. That is
**supersession**, not failure — `530f400` was pushed on top and Actions killed the older in-flight
run. `530f400`'s tree is a strict superset of `8036963`'s, so the adoption code is inside the
7/7-green head.

**Authority this program operates under:** feature-branch C137 hardening on the current branch +
a *new* branch off the C137 head for MP-0+. Commit coherent checkpoints, draft PR for review.
**No merge, no tag, no release, no change to historical defaults.** One passing CI run is **not**
the maintainer's approval of changed sound. **Ear = oracle** (machine metrics may only REJECT);
any stage that changes a sound is opt-in and judged by Leah's ear. **Leah holds the launch button.**

---

## MEGA-ROUND C137-A — red-first semantic & narrative audit

Five concrete risks, each grounded in source. Probes predeclare their expected outcome **before**
running. A characterization probe asserts the *current* behaviour and so **PASSES** today — a pass
here means "the weakness is present as described"; the repair flips the assertion.

### R1 / R4 — the receipt credits a CARRY by note-count, not germ identity
- **Source:** `narrative_receipt.rs:137-147`. A carry is `Carried::Realized` iff
  `added = Σ_role (count_story(role,window) − count_control(role,window)) > 0`. `measure` never
  reads pitch, rhythm, or provenance — only note counts per role per phrase window.
- **Hypothesis:** material that is *not* the germ, but preserves the carrier's note count, is
  credited as a realized carry. The witness is comparative (story vs BAND control — good) but the
  `added > 0` test is the non-source-aware weak link.
- **Predeclared:** scrambling a realized carrier's pitches off the germ (count + timing preserved)
  leaves the verdict `Realized`.
- **Probe:** `narrative_receipt::tests::c137a_r1_carry_is_credited_by_count_not_germ_identity`.
- **Actual (2026-10-09):** **PASS → weakness CONFIRMED.** Pitch-scramble is invisible to the receipt.
- **Repair (NEXT):** a source-aware witness that checks the carrier's *added* material quotes the
  germ — pitch-class / interval / contour licensed by the song's germ (`perf.bank.identity`),
  compared against the control. Non-circular: the germ is the source of truth, never the plan's
  claim. **Blast radius:** new read-only method on `NarrativeReceipt`; no sound change; flips this
  assertion. Keep the existing count-based comparative controls beside it (STORY vs BAND).

### R2a — the bass germ-voicing maps degrees MODULO the chord-tone count (non-injective)
- **Source:** `bass.rs:689-690` (the §4 carry-substance pass):
  `deg = germ.degrees[i % germ.degrees.len()]; pc = tones[deg.rem_euclid(tones.len())]`.
- **Hypothesis:** `rem_euclid(tones.len())` is many-to-one — distinct germ degrees collapse onto the
  same chord tone, and `ctx.chord.pitch_classes()` ordering is not guaranteed ascending, so the
  germ's interval/contour identity is not preserved; only a loose register trace survives. The
  docstring (`bass.rs:658-665`) claims the bass "moves in the germ's shape" and "keeps its rhythm,
  tags and dynamics" — the first is loose, the second is contradicted by R2b.
- **Probe:** `bass::tests::c137a_r2a_germ_tone_map_is_non_injective_and_contour_blind` — against
  the extracted source helper `germ_tone_index(deg, n_tones)`.
- **Actual (2026-10-09):** **PASS → CONFIRMED.** Over a triad, degrees 0/3/6 all select tone 0
  (non-injective); a germ step UP (2→3) drops the tone index (2→0), so the modular wrap flips
  contour direction; negatives wrap to the top tone.
- **Resolution — (a) DONE, (b) deferred (ear-gated):** the §4 render is **ear-accepted** ("in
  pocket", `f5f8137`). Per the program's rule (*repair smallest mechanism OR narrow its claim;
  preserve accepted sound unless a change earns a fresh verdict*):
  - **(a) narrow the claim — SHIPPED this checkpoint:** the §4 map extracted to the named, documented
    `germ_tone_index` helper (byte-identical computation); `bass.rs` comments corrected to "register
    DRIFT over the chord's tones, not exact interval contour." Docs + inert refactor, sound-preserving.
  - **(b) contour-preserving map** — sort chord tones ascending, index by germ-degree *rank* so
    ascending germ steps give ascending pitches. **Changes bass pitches → changes sound → opt-in A/B
    for Leah's ear, never a silent default.** Offered later as an eared experiment.

### R2b — stale `prov.role_note` after the germ pitch substitution
- **Source:** `bass.rs:689-692` sets `n.pitch` and `n.function = Some(ChordTone)` but leaves
  `n.prov.role_note` (e.g. `"root"`, `"pedal"`, a figure tag) **untouched**.
- **Load-bearing finding:** `role_note` is **read by gates** (`expression.rs:139-140`,
  `occupancy.rs:87`, `projection.rs:74` — the identity-projection figure test, `temporal.rs:247`,
  `material.rs:890`) **and is in the canonical fingerprint** (`fingerprint/schema.rs:1299`).
  **But** the §4 pass is byte-exact-OFF the default path (`perf.narrative` is `None` unless
  `NarrativePolicy::Ensemble`), so **historical v0.4 fingerprints are NOT corrupted**. The
  inconsistency is internal to the Ensemble *story* path only: a germ-substituted `ChordTone` can
  wear a pre-substitution tag a downstream gate then reads.
- **Probe:** `narrative_receipt::tests::c137a_r2b_germ_substituted_bass_keeps_a_stale_role_note`.
- **Actual (2026-10-09):** **PASS → CONFIRMED, with a finding that CORRECTS the earlier resolution.**
  The fixture germ-substitutes **27** bass notes (`function == ChordTone`) across carrier phrases
  p2/p5/p6, wearing pre-§4 tags: `"root"`, `"fifth"`, `"approach"`, `"quote"`, `"walk"`, `"pedal"`,
  `"answer"`. Several are **gate-read**: `"approach"` → `expression.rs:139`, `"answer"` →
  `temporal.rs:247` + `material.rs:890`, `"pedal"` → fingerprint.
- **Resolution — CORRECTED:** my earlier "inert honesty fix if no gate flips" was too optimistic.
  Those gates **fired on these stale tags inside the ear-accepted render**, so reconciling
  `role_note` would flip a gate branch and **change the accepted sound** — it is **ear-gated, not a
  free fix**. The sound-preserving move is to **narrow the claim** (DONE: the `bass.rs` §4 comment
  now states the pass overwrites `function` but leaves `role_note`); actual reconciliation is an
  opt-in, eared experiment. Historical v0.4 fingerprints remain intact (byte-exact-OFF the default
  path).

### R3 — the harmony join licenses a Miss by a GLOBAL earliest Prepare, not a local predecessor
- **Source:** `narrative.rs:143-154`. `first_prepare = min beat over ALL Prepare events`;
  `prepared = first_prepare < miss.beat`. A Miss is "prepared" if *any* Prepare anywhere earlier
  exists — not one causally relevant to the Miss's own arrival/phrase/domain.
- **Hypothesis:** an early, unrelated (resolved) Prepare wrongly licenses a much later,
  locally-unprepared Miss as a narrative deflection to carry.
- **Predeclared:** with one intro Prepare (beat 6) and a later unrelated Miss (beat 42) over
  phrase 5, the join attaches a `HarmonicCarry` to phrase 5.
- **Probe:** `narrative::tests::c137a_r3_a_global_prepare_licenses_an_unrelated_later_miss`.
- **Actual (2026-10-09):** **PASS → weakness CONFIRMED.**
- **Latency note:** whether this fires on *real* generated μ depends on μ emitting more than one
  Prepare, or a cross-phrase Prepare/Miss. To characterize NEXT: count Prepare events per real
  generated song. If real μ only ever has one Prepare→Miss pair, the weakness is latent (present in
  the mechanism, not yet firing on shipped fixtures).
- **Repair (NEXT):** pair a Miss with the most-recent Prepare in its own harmonic domain / phrase
  window (a local predecessor), not a global min. **Blast radius:** `narrative.rs` join; could
  change which phrase withholds on some songs → a possible story-path sound change → ear-gated if
  any render changes. Inert on fixtures with a single Prepare→Miss pair (verify first).

### R5 — a multi-carrier obligation is credited on the SUM across carriers, not per-carrier
- **Source:** `narrative_receipt.rs:137-142` sums `added` across all carrier roles. A `Payoff`
  names `Keys` **and** `Bass`; the carriage reads `Realized` when only one carries.
- **Predeclared:** zeroing the weaker named carrier's contribution (its story count down to the
  control count) leaves the carriage `Realized` on the other carrier alone.
- **Probe:** `narrative_receipt::tests::c137a_r5_multicarrier_payoff_credited_without_each_carrier`.
- **Actual (2026-10-09):** **PASS → weakness CONFIRMED.** This is the deferred
  single-response/multi-carrier limitation, now characterized in a test.
- **Repair — SHIPPED (sound-preserving):** added `NarrativeReceipt::carrier_shortfalls` — a
  read-only per-carrier witness beside `measure` (the control, kept): for every carriage with named
  theme carriers, it names each carrier whose OWN `added` is `<= 0`. Test
  `c137a_r5_repair_per_carrier_names_the_silent_carrier` proves `measure` still credits the
  one-carrier mutation while `carrier_shortfalls` names the silenced carrier. No sound change (the
  witness only reads the Score).
- **★ FINDING — the fixture's Payoff "tuttis" are SINGLE-carrier in realization.** Natural
  shortfalls on the canonical fixture: **p2 Payoff — Bass `added = −2`** (bass carried *fewer* notes
  than the control; keys alone carried), **p6 Payoff — Keys `added = 0`** (bass alone carried), p5
  Develop — Bass `added = 0`, p8 Recognize — Keys `added = −2` (already `Unrealized` in `measure`).
  So the count-summing `measure` credits both Payoffs as fully Realized, but neither is a true
  two-carrier tutti — the answer to the audit's "do all named payoff carriers actually fulfil the
  obligation?" is **no**. A real characterization of the current state, not a regression; whether to
  make the tuttis genuinely two-carrier is a composition/enactment question (ties to §6), and any
  realization change is ear-gated.

### C137-A status summary
| risk | site | probe | predeclared | actual | repair |
|---|---|---|---|---|---|
| R1/R4 | `narrative_receipt.rs:137-147` | `c137a_r1_…` | fooled | **PASS→confirmed** | source-aware germ-quoting witness (no sound change) |
| R2a | `bass.rs` `germ_tone_index` | `c137a_r2a_…` | non-injective / contour-blind | **PASS→confirmed** | claim narrowed (shipped); contour map = eared A/B |
| R2b | `bass.rs:689-692` | `c137a_r2b_…` | stale gate-read tag | **PASS→confirmed** | claim narrowed (shipped); reconcile = ear-gated |
| R3 | `narrative.rs:143-154` | `c137a_r3_…` | global prepare licenses | **PASS→confirmed** | local-predecessor join (ear-gated if a render changes) |
| R5 | `narrative_receipt.rs:137-142` | `c137a_r5_…` (+repair) | one carrier suffices | **PASS→confirmed** | `carrier_shortfalls` **SHIPPED**; Payoff tuttis found single-carrier |

**Gate:** `fmt` 0 · `clippy --all-targets --all-features` 0 · audio lib **526 passed / 0 failed /
6 ignored** (6 C137-A tests — R1/R3/R5 + R2a/R2b + the R5 repair — the +6 delta from the prior
520). R2a extracts the `germ_tone_index` helper (byte-identical) and narrows the §4 comments; R5
adds the read-only `carrier_shortfalls` witness; the §4 render and the shippable library are
unchanged (behaviour-preserving — the full suite stays green).

---

## MEGA-ROUND C137-B — musical generalization & closeout (NOT STARTED)
Named here so the program's top is explicit. Fresh seeds / lengths / worlds / grammars / tempi /
cover constraints; long-range theme identity, functional harmonic departures, bass/keys
interaction, deliberate lead withholding, ensemble complementarity, controlled dynamic arcs. Revisit
the bridge awkwardness at the composition/meaning level (not performer repair). Build a blind
listening bundle (full mix + lead/bass/keys/drum stems); Leah's ear may ACCEPT / REJECT / AMBIGUOUS
— never invented.

**Adoption scope — kept honest (prompt requirement):** the judged *example* (`examples/rick_story.rs`)
defaults to `DeflectedLift`; this is **not** every composition API's default. `SongMap::compose`
takes `grammar` explicitly; `functor::compose_full` builds a `StructuralR9` control; the
harmony-less `compose(None, MeaningDirected)` contract is pinned by
`narrative_join_probes::judged_voice_has_no_harmony_lane` and the `narrative_receipt` `none` block.
Adoption is the harness front-door policy only.

---

## MEGA-ROUND MP-0+ — the minimal MeatsackProjection seam (NOT STARTED)
Starts a **new feature branch off the C137 head** (not the research branch). Smallest type-safe
checked-performance boundary + an OFF/identity sequencer passthrough before any acoustic effect; the
existing renderer stays the control. See `HUMAN_MUSIC_MEATSACK_PROJECTION_PLAN.md` for the ladder
and `…_BOUNDARY.md` for the contract and the two-way `AcousticAssumptions` leak.

---

## Next verdict-changing step
Build the R2a/R2b probes and the R3 real-μ Prepare-count characterization, then strengthen the
witness (R1/R4 source-aware, R5 per-carrier) with the characterization assertions flipping as the
repairs land. No sound-changing repair ships without a fresh ear verdict.
