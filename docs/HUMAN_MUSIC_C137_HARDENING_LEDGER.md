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
- **Status:** NOT yet probed (specified). Probe NEXT: a unit test over the degree→tone map showing
  (i) two distinct germ degrees map to one pc, and (ii) a monotonic germ-degree run need not map to
  monotonic output pitches.
- **CRITICAL — accepted sound:** the §4 render is **ear-accepted** ("in pocket", 2026-10-09,
  `f5f8137`). Resolution per the program's own rule (*repair smallest mechanism OR narrow its
  claim; preserve the accepted sound unless a change earns a fresh verdict*):
  - **(a) narrow the claim** — correct `bass.rs` docstring to "approximates the germ's register
    drift over the chord's tones, not its exact interval contour." **Docs-only, sound-preserving.**
    Recommended now.
  - **(b) contour-preserving map** — sort chord tones ascending and index by the germ degree's
    *rank* so ascending germ steps give ascending pitches. **Changes bass pitches → changes sound →
    opt-in A/B for Leah's ear, never a silent default.** Offered later as an eared experiment.

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
- **Status:** NOT yet probed (specified). Probe NEXT: render the Ensemble story; among bass notes a
  bass-carrier phrase germ-substituted (`function == ChordTone`), assert ≥1 retains a
  pre-substitution `role_note`; then test whether any gate's decision actually flips on it.
- **Resolution:** if no gate decision flips → make `role_note` honest in a provably-inert way
  (sound-preserving). If a gate flips → a real story-path bug; fix and re-ear the story render.

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
- **Repair (NEXT):** per-carrier outcome — each named carrier must add its own material; a Payoff
  tutti is fully realized only when every named carrier carries. **Blast radius:** receipt
  outcome type + method; test updates; no sound change. A tighter witness may reveal the fixture's
  `p2`/`p6` Payoffs are only *partially* realized — that is a finding to record, not a regression.

### C137-A status summary
| risk | site | probe | predeclared | actual | repair |
|---|---|---|---|---|---|
| R1/R4 | `narrative_receipt.rs:137-147` | `c137a_r1_…` | fooled | **PASS→confirmed** | source-aware germ-quoting witness (no sound change) |
| R2a | `bass.rs:689-690` | *next* | collapse/scramble | — | narrow claim (docs) now; contour map as eared A/B |
| R2b | `bass.rs:689-692` | *next* | stale tag | — | inert honesty fix, or ear-gated if a gate flips |
| R3 | `narrative.rs:143-154` | `c137a_r3_…` | global prepare licenses | **PASS→confirmed** | local-predecessor join (ear-gated if a render changes) |
| R5 | `narrative_receipt.rs:137-142` | `c137a_r5_…` | one carrier suffices | **PASS→confirmed** | per-carrier outcome (no sound change) |

**Gate on this checkpoint:** `fmt` 0 · `clippy --all-targets --all-features` 0 · audio lib
**523 passed / 0 failed / 6 ignored** (3 new C137-A probes = the +3 delta from the prior 520).

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
