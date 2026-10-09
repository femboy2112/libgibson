# HumanMusic — the Rick-C137 build-design

**Status:** design-of-record, awaiting maintainer green-light. No generator code changes
yet. Refines the generator-lift ladder of [`HUMAN_MUSIC_RICK_PLAN.md`](HUMAN_MUSIC_RICK_PLAN.md)
with the *verified current API* and a concrete, byte-exact gating contract.

---

## 1. The gap, stated honestly

Everything the maintainer has accepted by ear — the dense complementary arrangement, the
rhythmically-alive lead, the earned downward harmonic travel — was **hand-authored** by the
probe (`examples/rick_probe.rs`), note by note, directly in the Score IR. The probe proved
the music is **reachable**. It proved nothing about the **generator**.

- **Citadel-Rick** (reached, `21adb4e`): *a Rick can author it* in Score IR, and the engine's
  synth/world renders it into music the maintainer accepts.
- **Rick-C137** (the target): the **generator** (`SemanticTrace → IntentTimeline → CompositionPlan
  → SongMap → PerformancePlan → realize → HumanMusicSynth`) **produces** this calibre from a
  semantic intent — and the maintainer hears the *generator's own output* and says it fucks.

The job is to **lift the ear-proven relations into the existing planners**, never to write a
second composer. The probe ladder is the *spec*; this doc is the *build order and the safety
contract*.

**Three invariants, non-negotiable:**

1. **Opt-in only.** Every mechanism is a `PerformanceProfile` policy. The default route stays
   on `WRITTEN`.
2. **v0.4 byte-exact.** `perform()`/`compose()` output does not change by a single sample.
   Enforced structurally (below), not by hope.
3. **The ear is the oracle.** Machine metrics may only **reject**. Acceptance is the
   maintainer hearing the generator and ruling. Density without provenance is noise; coherence
   (global + local) is the hard constraint.

---

## 2. What already exists (the de-risk)

The engine was *designed* for exactly this lift. Verified against source this pass:

- **The opt-in surface is built.** `PerformanceProfile` (`policy.rs:180`) is an **orthogonal
  struct** — each mechanism is its own policy field. `WRITTEN` (`policy.rs:209`) composes every
  historical default (byte-exact v0.4). `BAND` (`policy.rs:253`) composes the opt-in variants and
  is explicitly annotated *"new work should start here."* Const `with_*` builders
  (`policy.rs:260–282`) compose cleanly; `validate(coupling)` (`policy.rs:286`) is the pre-planning
  boundary that **rejects** illegal combinations before a note is planned.
- **Every mechanism follows one pattern:** a policy enum with a `#[default]` historical variant
  (`Archived`/`Written`/`Independent`) **plus** an opt-in variant. Already present:
  `HarmonyPolicy::{Archived, Vocabulary}` (`policy.rs:137`), `FunctionPolicy::{Archived, Earned}`
  (`policy.rs:154`), `SupportPolicy::{Independent, HeardHarmony, SourceVoicePath}` (`policy.rs:101`),
  `OccupancyPolicy`, `ActionAdmission::{Planned, Rehearsed}`, `PercussionPolicy`,
  `ExpressionPolicy::Pulse(PulsePolicy)`.
- **The pocket is already a policy.** `POCKET` (`policy.rs:242`) = `ExpressionPolicy::Pulse(
  PulsePolicy::POCKET)` + `VoiceLifetimePolicy::ExplicitContinuations`. The probe's hand-authored
  `LEAN = 0.022` is the generator's `PulsePolicy::POCKET` already. **GEN-pocket may already be
  done in the generator** — R0 confirms by ear.
- **Harmonic vocabulary already names travel.** `HarmonyPolicy::Vocabulary` (`policy.rs:143–148`)
  governs *"an applied dominant, a modulation"* — the machinery to **admit** a secondary dominant /
  key change already exists. The open question is whether the composer **generates** a *prepared*
  modulation or merely admits one when present (that is GEN-4).
- **The byte-exact gating pattern is already in use and proven.** `fingerprint()`
  (`performance.rs:949`) is FNV-1a over the hand-written `Debug` (`performance.rs:1593`). That Debug
  impl **conditionally emits new fields only when non-default** (`performance.rs:1620–1628`):
  `cover_constraints`/`rehearsal` only when `Some`, `functions` only when `!= Archived`. An archived
  config skips those lines → byte-identical Debug string → byte-identical fingerprint. **`functions`
  is the exact precedent for every C137 field.** A canonical v2 fingerprint (`fingerprint.rs`,
  explicit schema, Debug-independent) exists for new profiles.
- **The ensemble seam is clean.** The performance plan is assembled and then
  `perf.budget = budget::allocate(&perf)` (`performance.rs:876`); `budget::allocate`
  (`budget.rs:119`) returns the per-bar complexity allowances. `SonorityPlan` (`sonority.rs:400`) is
  nearly the target schema for stating who owns which pitches per chord.

**Consequence:** most of C137 is *driving and hearing* machinery that already exists, plus a
small number of genuinely new planner behaviors. That is why **R0 (below) comes first** — it
tells us which rungs are real work and which are already done.

---

## 3. The gating contract (how each rung stays v0.4 byte-exact)

Every rung obeys the same five rules. This is the load-bearing safety design.

1. **New mechanism = new policy field**, defaulting to its historical/`Archived`-equivalent
   value, so `WRITTEN` composes to byte-identical behavior automatically.
2. **New `PerformancePlan` fields default to the historical value**, and the `Debug` impl
   (`performance.rs:1593`) emits them **only when `!= default`** — the established
   `functions`/`cover_constraints` pattern. Archived configs keep the exact v1 fingerprint.
3. **New opt-in profiles carry a canonical v2 fingerprint** (`fingerprint.rs`), not the fragile
   Debug-based v1. v1 stays frozen for the archived arms.
4. **New validation rules go in `validate()`** (`policy.rs:286`) so an illegal combination is
   rejected before planning, never rendered.
5. **A byte-exact guard test per rung:** assert `WRITTEN` (and every archived preset) produces
   the identical fingerprint before/after the change, and that a known v0.4 render is
   sample-identical. A rung does not land until this is green.

---

## 4. The rung ladder

### R0 — baseline diagnostic (first; no code change)

Render the **generator's own output** — not the probe — via `WRITTEN` (the v0.4 default) and via
`BAND` (the hardened opt-in), in `black_ice` + `vapor95` + `swiss_signal`, from the same semantic
intent, and deliver for the maintainer's ear. This establishes **where the generator already is**
versus the probe, so every later rung is real work measured against a known baseline rather than a
guess. Also surfaces whether `BAND`'s existing pocket / vocabulary already lands.

**R0 ear verdict (2026-10-09, re-scopes the ladder).** The BAND ensemble "sounds real... in the
pocket"; the information content comes from the **depth/stability of the band's harmonic+rhythmic
interaction orbit**, NOT raw note count. The LEAD VOCAL is the one degenerate axis ("robotic"). So:
GEN-1 as density-stacking is **cancelled** (the band already delivers the orbit); GEN-2 (pocket) is
**confirmed** by ear; **GEN-3 (the lead) is promoted to the priority rung.** "Turning it up to
Casiopea" (tempo/throughput) is a later knob, after the lead.

### GEN-1 — ensemble throughput ✔ banked (do not build)

R0 settled this: the band's information content is the depth of its *interaction orbit*, not onset
count, and BAND already delivers it (bass/keys re-plan per bar from the shared `AccentGrid` +
`perf.ensemble` with per-bar kinetic velocity). No density-stacking. If a later Casiopea push wants
more throughput it rides the *existing* ensemble interaction, never a pile of redundant onsets.

### GEN-2 — pocket ✔ confirmed (do not build)

`POCKET`/`BAND` already apply `PulsePolicy::POCKET`; R0 confirmed the band is in the pocket by ear.
Done. Structured / relational only — **never** per-note onset jitter (standing rule).

### GEN-3 — the alive lead (PRIORITY; verified diagnosis below)

**Verified root cause (against source).** The lead's rhythm is **not generated — it is copied.**
In the `StablePropulsion` regime the thesis is a single pinned vector (`song.rs:300-305`: degrees
`[0,2,4,2,7,4,2,0]`, rhythm `[0.5,0.5,1,0.5,0.5,1,0.5,1.5]`) and `song.rs:313-316` stamps the
*identical* motif onto every site. The statement loop (`interaction.rs:453-576`) plays
`site.motif` verbatim (`:532-536`); realization keeps the motif's onsets exactly
(`motif.rs` `melodic_events` — "its rhythm and identity are kept"), choosing pitch only. Every lead
statement therefore carries the same rhythm; measured IOIs are only eighths + quarters, no
sixteenths, no syncopation, no development. The pocket/expression layer **cannot** fix this: the
contract protects structural-note onset/duration/velocity (`expression.rs:23-24`, `project`
`:37-47`) — it only re-grids *optional connective* notes. The band is alive because it re-plans per
bar from the accent grid; the lead never reads those per-bar signals. **Two confounded drivers of
"robotic"** (per the recon, unresolved): (1) *content repetition* — the same figure ×7; (2)
*metronomic spacing* — the 2-bar boundary snap (`interaction.rs:568`). A clean fix exposes them as
**separate toggles** so the ear isolates the real driver.

**The fix must be a source/plan-time change**, before `melody.rs:254` (`authored = notes.clone()`).
Mirror the `functions: FunctionPolicy` path — the only existing policy that already reaches the lead
planner (`interaction.rs:335,356`). New opt-in `LeadRhythmPolicy { #[default] Written, Alive }` (+
field on `PerformanceProfile` next to `functions`, `policy.rs:198`; `WRITTEN`→`Written`; `BAND`
chains `with_lead_rhythm(Alive)`), threaded via `AdmissionInputs` (`performance.rs:405-419`) into the
plan like `functions`. Sub-axes, each its own toggle for the A/B:

- **3a — rhythmic development:** at `interaction.rs:532-536`, derive a per-statement rhythmic variant
  of `site.motif` — inputs: statement index in the phrase, antecedent/consequent role (first states,
  second *answers*), and the same `accent.at_beat` syncopation/pickup weights the bass reads. Preserve
  the degree contour and total length (must still pass `statable` `song.rs:73` and the fit test
  `interaction.rs:469`). Injecting here propagates to materials/budget/occupancy/answers consistently.
- **3b — cadence/spacing:** relax the rigid 2-bar snap (`interaction.rs:568`) so statements breathe
  with varied phrasing instead of one-per-two-bars.
- **3c — dynamics arc:** at `melody.rs:185-203`, give the lead a per-note velocity arc (phrase contour
  + metric weight + per-bar kinetic, as `bass.rs:350` does) — the lead currently has none.

**Must stay a no-op** on the cover path (`interaction.rs:450`, `cover_constraints.source_notes`) and
under `Language::Simple`. Byte-exact gating per §3: default `Written` + conditional Debug emission
(`if self.lead_rhythm != Written`), exactly the `functions` precedent.

### GEN-4 — prepared harmonic travel (hardest; solves the open residual)

The composer must **plan** a *motivated, prepared* modulation — a pivot / secondary dominant that
earns a new key, **downward / darker** by preference, resolving home — at the
`CompositionPlan`/`SongMap` layer, with `HarmonyPolicy::Vocabulary` admitting it. This is where the
accepted bridge-travel becomes a *generated* behavior, and where the **open residual** (the
bridge's semantic *fit*, §6) is truly resolved — not patched in a probe.

Each rung: opt-in policy → byte-exact guard green → render generator output → maintainer ear-gate →
accept or iterate. Monotonic, one axis at a time, exactly like the probe ladder.

---

## 5. The acceptance vector (never a scalar)

Rick-C137 is a **vector**, never a `rick_score`. Machine dimensions **reject only**; the final
gate is the ear.

| Dimension | Machine check (reject-only) | Oracle |
|---|---|---|
| Throughput | events/sec in range, each with provenance | ear |
| Ensemble interdependence | no voice states the full chord redundantly | ear |
| Pocket coherence | one shared lean, correlated (not independent jitter) | ear |
| Melodic life | rhythmic variety; germ developed, not restated | ear |
| Harmonic travel | a prepared modulation present and resolved home | ear |
| Global + local coherence | song-form intact; no decohered bars | ear |
| **Final** | — | **maintainer hears the *generator* and says it fucks** |

Plus the two standing invariants: **density-must-have-provenance** and the **coherence budget**
(new information rides on a stable identity anchor; it never mushes the song).

---

## 6. Safety, holdout, open residual

- **v0.4 byte-exact** is a release constraint, enforced per §3 rule 5. Any rung that cannot keep
  `WRITTEN` sample-identical does not land.
- **No second composer.** Every change is a policy + a behavior inside an existing planner.
- **MIDI holdout stays sealed** (third-party copyright; never committed) until the first
  generator freeze, then used only as a *reject* instrument, never a target to copy.
- **Open residual (carried from Citadel-Rick):** the accepted bridge is music-theoretically sound
  but its **semantic fit** is not yet fully there (maintainer: *"the bridge doesn't quite fit
  semantically"*). Tracked here as a GEN-4 acceptance item; it is a C137-level concern, not a probe
  blocker.

---

## 7. First action on green-light

**R0.** Render the generator's own `WRITTEN` and `BAND` output across the three worlds and deliver
for the ear, with no code changed — so the ladder starts from measured ground, not a guess.
