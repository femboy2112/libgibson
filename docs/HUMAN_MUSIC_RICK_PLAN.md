# HumanMusic Rick-level plan — lifting the proven relations into the generator

Companion to [`HUMAN_MUSIC_SONGCRAFT.md`](HUMAN_MUSIC_SONGCRAFT.md). That note records the ear-first
hand-authored ladder that reached **Morty-level** musicality. This note is the engineering plan for the
next frontier — **Rick-level** — and, critically, the map from *what the probes proved* to *where each
relation belongs in the real generator*. Engine-facing only; the maintainer's private study stays out of
the repo.

> **The standard is unchanged.** Machine metrics may only **REJECT** a candidate; only the maintainer's
> **ear** may ACCEPT that it sounds good. Every mechanism below is a hypothesis until a render confirms it.
> Feel is never achieved by random onset jitter — structured/relational only.

## 0. What Morty-level proved — and what it did NOT

The ladder (`song_form → ensemble → groove → songcraft`) proved, rung by rung by ear, that a *sufficiently
relational Score* rendered through the **existing** `HumanMusicSynth` + worlds reads as a coherent song.

It proved this by **bypassing the generator**: the probes hand-author a `Score` IR directly and render it.
They did **not** prove `SemanticTrace → generator → Morty-level Score`. The generator was never in the loop.

**So the engineering problem is not "compose better music" — it is "teach the generator the RELATIONS the
probes hand-authored."** We lift *relations*, never literal note arrays. "Am–F–C–G" is not the songwriting
algorithm; *recurring harmonic identity* is. "Keys carry the 3rd+7th" is not a rule; *distributed harmonic
responsibility* is.

One more load-bearing fact from the architecture survey: the default `compose()`/`perform()` path still runs
the historical **`PerformanceProfile::WRITTEN`** with `Archived` functions (`functor.rs:120`). The modern
**`BAND`** route (`perform_with_profile` → `plan_and_realize` → `perform_checked`) is **opt-in**
(`policy.rs:209` WRITTEN default, `:253` BAND). Every new relation lands as an **opt-in policy field**, so
v0.4 stays byte-exact. See §6 for the fingerprint hazard that makes "byte-exact" literal.

## 1. The probe-ingredient audit — relation → generic law → upstream home

For each ingredient: what the probe did, the ear verdict, the **generic law** to promote, the pipeline level,
the **real** existing machinery it belongs in (verified file:line), and what must **not** be copied literally.

| Ingredient (probe) | Ear | Generic law | Level | Upstream home (verified) | Never copy |
|---|---|---|---|---|---|
| Literal returning verse/hook over a cycling loop (`song_form`) | song-form real, not the disease | **recurring heard identity / return** | SONG | `MotifRepetition::{Develop,Return,Restate}` is already a song-level dial (`song.rs:100`); `Return`/`Restate` guarantee an early thesis and restate the germ at theme sites | the specific loop; the 4-bar length |
| No voice states a full triad; guide tones carry quality; pad withheld→arrives (`ensemble`) | the disease was common-mode agreement; interdependence is the cure (**load-bearing**) | **distributed harmonic responsibility — redundancy must be *intentional*** | PERFORMANCE-PLAN | **the key seam (§3).** Today reactive/root-only: `keys_answer_for_identity` (`comp.rs:1759`), `RolePath.rooted` (`voicing.rs`), `SonorityPlan` has `core`+`colors(owner)` but no owner per identity tone (`sonority.rs:400`); `voicing::required_pcs` forces *every* support voice to carry both guide tones (`voicing.rs:362`) — the blocker | "no player may EVER state the full chord" (that was a probe manipulation, not a law — see §4) |
| One shared pocket; whole band leans behind the kick (`groove`) | pocket "more natural" (a dial); per-voice swing *galloped* and was rejected | **feel is a *correlated*, phrase/role-aware lean — never independent jitter** | PERFORMANCE-FIBER | the **pre-declared dormant seam**: `FeelTransport` (`rhythm.rs:176`, `Identity`-only), whose own doc demands "correlated and phrase/player-aware, never independent per-note noise" | the ~13 ms `LEAN` constant; per-instrument offset tables |
| Germ antecedent asks / consequent answers (`songcraft`) | motifs still "a little robotic" but the direction is right | **shared germ DNA across a question/answer pair** | SONG | `ThematicTrajectory::next_for` has live `Question`(fragment)/`Answer`(tail) but *role-driven*, not antecedent/consequent *phrase pairing* (`motif.rs:540`); `Handoff::Consequent` exists but only `Composer::MeaningDirected` emits it (`composer.rs:867`, non-default) | the exact VERSE/HOOK/BRIDGE pitch arrays |
| Bridge on Dm–F–C–**E** (colored dominant) into a bigger final chorus (`songcraft`) | bridge wanted; useful | **departure → colored harmonic region → return; color is FUNCTIONAL (V-ish→I-ish)** | SONG | `SectionFamily::B` is the existing contrasting-section hook (`plan.rs`); a raised-G# applied dominant needs a `Tonicize`/`Modulate` verb via `ActionPlan` → `apply_harmonic_actions` (`performance.rs:670`) or the song chart (`HarmonicMap`, `song.rs:216`) | "add dominant sevenths"; the specific Dm–F–C–E cells |
| Velocity shaped over structure: phrase arc + section terrace (`songcraft`) | **shaped > flat, CONFIRMED** | **expression is causal: role · phrase progress · accent · section trajectory · culmination** | PERFORMANCE-PLAN | **no counterpart exists** — per-voice velocity is a per-phrase `energy_target` step with no within-phrase arc or section terrace (`melody.rs:185`, `bass.rs:269`, `comp.rs:124`); seam = `PhraseGoal.energy_target` (`discourse.rs:623`, already live for lead+drums) or a new gated `EnsembleBar`-sibling field | the literal `verse .90 / chorus 1.0 / final 1.12` constants |

## 2. Rick-level, defined (a vector, never a scalar)

Rick-level is **not** notes-per-second. It is a **greater rate of meaningful relational change under
preserved long-horizon identity** — semantic throughput up, *subject to* song coherence staying readable.

There is **no scalar `rick_score`.** We keep a **diagnostic vector** (descriptive, never a quality verdict):
motif transformation/recurrence, harmonic-function change, bass melodic motion, inner-voice motion, rhythmic
complementarity, role/responsibility exchange, call/response, dynamic trajectory, articulation, section
transition, cadence/release. Build it from machinery that already exists — don't invent a parallel system:
`RigidityDiagnostics.onset_vectors` (16-bit per-bar onset masks, `diagnostics.rs:1617`), `ComplexityReport`
(planned-vs-realized onsets, `budget.rs:216`), `witness::kinetic_curve` (`witness.rs:738`),
`CoherenceDiagnostics` (`diagnostics.rs:27`, "preserve the components"), `RealizationDiagnostics`
(`thesis_return_similarity`, `diagnostics.rs:407`). **Never sum them.** Use them to compare Morty baseline vs
Rick probes vs generated candidates.

## 3. Core invariant: density must have provenance

Every added event must answer **WHY it exists**: thematic material, harmonic responsibility, voice-leading
obligation, rhythmic complement, interaction/call-response, preparation/resolution, phrase dynamic, section
transition, cadence, or *deliberate* reinforcement. **No event exists merely because "density = high."** Stamp
provenance through existing identity — `MaterialId`/`ActionId`/`InteractionId`, phrase, `DiscourseRole`, pitch
function, thematic `MotifBank` material — never a new provenance system.

**Silence is information** (§30 of the brief): a voice intentionally *not* playing is often what lets another
relation read. Do not optimize occupancy upward. The goal is *meaningful information / time*, not *notes / time*.

## 4. The redundancy policy (NOT "nobody may state the full chord")

The successful `ensemble` manipulation — no voice states a full triad — is **not** a universal law. A full
piano chord or an ensemble unison can be exactly right. The real principle: **redundancy must be intentional.**
Model it as a policy the discourse/form chooses per phrase:

- **REINFORCED** — multiple players intentionally agree (an earned unison/tutti).
- **DISTRIBUTED** — ensemble meaning is split across players (the probe's default).
- **CONTRAPUNTAL** — players carry interacting independent lines.

(Names provisional; don't commit a type until code evidence requires it.) The existing harmony-identity check
already supports this: `identity.rs` is **root-centric, pointwise, role-agnostic — not simultaneous-full-chord**
(any role may supply the root; the root may be in the bass alone). So distributing the chord does **not** trip
it. What the check *cannot* yet do is **reward a deferred or distributed 3rd/7th** — `guide_heard` and
`root_last_heard` are recorded but unused in computing `status` (`identity.rs:74`). That is the hook for §1's
deferred-harmony and distributed-responsibility work, and it must be *added*, not have existing laws *weakened*.

## 5. The coherence budget (the anti-mush law)

High-information music needs a limiter — not a scalar score, but a planning concept: **how many independent
structural changes a phrase may carry at once.** Axes: thematic mutation, harmonic departure, rhythmic
displacement, interaction density, register/elevation, dynamic culmination. The rule: **when several axes move
aggressively, at least one strong identity anchor stays stable.** Harmony goes wild → the hook/rhythm holds;
rhythm gets dense → harmony/form stays legible; counterpoint intensifies → the motif family stays shared.
This is the single law that keeps throughput from decohering the song.

## 6. How a lift stays safe: opt-in policy + the fingerprint hazard

Two hard mechanical constraints, both verified:

1. **Opt-in only.** New relations are fields on a `PerformanceProfile` policy, default `Archived`, reached only
   via the `BAND` route. With every new option off, v0.4 is byte-for-byte preserved (R17 fixtures, covers,
   VAPOR95 controls, fingerprints).
2. **The fingerprint hazard.** `PerformancePlan::fingerprint` is FNV over a **hand-written `Debug`**
   (`performance.rs:949`, `:1593`) that deliberately omits non-archived fields (the `functions` precedent). Any
   new field on `PerformancePlan`/`AccentGrid`/`StepWeight`/`EnsembleBar`/`Score` **silently changes archived
   fingerprints** unless added as a gated `Option` printed only when non-default. Every lift obeys this.

## 7. The Rick probe ladder (ear-first, before any generator work)

Repeat the method that worked. `examples/rick_probe.rs` **starts from the accepted `songcraft` probe** and adds
**one** Rick mechanism per flag, same underlying song/form/germ, each rung a clean A/B, **no generator yet**:

1. **Complementary rhythm** (`--no-rhythm-complement`) — voices own different metric regions; more events, zero
   added collision. Machine: onset coincidences, per-role occupied lattice classes, silent-space distribution.
2. **Melodic bass** (`--root-bass`) — bass keeps harmonic duty *and* becomes a line (contour, destination,
   approach vocabulary, reason) — never a random walk.
3. **Counter-comp / inner voice** (`--block-comp`) — support becomes a counterline: retained common tones,
   stepwise guide motion, suspensions, holes around the lead.
4. **Deferred / temporal harmony** — root before 3rd; a 3rd that arrives a bar late; a common tone surviving the
   barline. Does the existing observer accept temporal identity? If not, isolate the exact mismatch (do **not**
   weaken the held-identity laws globally to admit the probe).
5. **Harmonic reinterpretation** — a sustained pitch changes *function* as the root moves under it.
6. **Sequence / planing / reharmonized cell** — one known cell through several harmonic locations; still heard
   as *one idea*.
7. **Interaction density** (`--low-conversation`) — one voice calls, another transforms/responds, a third leaves
   space or reinforces only the landing. Multiple overlapping relations, all traceable.

Every rung renders to `target/humanmusic-beefup/rick_probe/` (full mix + stems + structural receipt + metric
diagnostics). The maintainer classifies each mechanism **ACCEPT / NEUTRAL-DIAL / REJECT / UNCLEAR**. Only
**ACCEPT** mechanisms become generator candidates. A rejected mechanism is not implemented because it is
"sophisticated." `rick_probe.rs` is disposable apparatus — its pitches/rhythms/velocities are never promoted,
only its relations.

## 8. The generator-lift ladder (only ACCEPTED mechanisms; one at a time)

First build the **generated Morty baseline** (best current production pipeline + accepted `MotifRepetition`),
a fixed seed/story/world matrix — distinct from the hand-authored probe, which remains the attainable oracle.
If the generator can't yet reproduce the basic accepted Morty properties (coherent form, interdependent
voicing, shared pocket, phrase dynamics), **close that gap before adding Rick density.**

Then, in order, each a same-seed before/after with a listen and receipts, continuing only if neutral-or-better
and structurally valid:

- **GEN-1 — ensemble harmonic responsibility (first, per the strongest Morty result).** A **planned** per-bar
  per-harmony allocation inside `PerformancePlan::build`, in the window **between `plan_ensemble` (`:809`) and
  `budget::allocate` (`:876`)**, stored as a gated `Option` field (§6). It decides *up front* which seat sounds
  root/3rd/7th per `HarmonicContext`; the reactive `keys_answer_for_identity`/`heard_pad_path` loops
  (`functor.rs:608–790`) become the **audit**, not the decision. Closest existing schema: `SonorityPlan`
  (`sonority.rs:400`, add an owner per identity tone, build at plan time); relax `voicing::required_pcs` via the
  already-parameterized `required_pcs_with(ctx, triad_color=false)` under policy. **Decide responsibility BEFORE
  realization** (source-authority law) — never post-process a finished Score to delete duplicate tones.
- **GEN-2 — complementary rhythmic ownership.** Extend/reinterpret the shared `AccentGrid` (`performance.rs:79`)
  into role-specific subsets *before* events sound; anticipation/delay via explicit transforms, never an
  independent per-player clock.
- **GEN-3 — melodic bass / counter-comp from shared material.** Derive counterlines from the `MotifBank`,
  current `InteractionMaterial`, harmonic context, destination, phrase role — same DNA, different function. Not
  a fresh per-player melody generator.
- **GEN-4 — coordinated `FeelTransport`.** Activate the dormant seam (`rhythm.rs:176`) conservatively: a common
  band/phrase lean + bounded role lean, deterministic and low-dimensional, correlated over phrase time (settle,
  approach-to-cadence, collective breath). Needs a metric-source record on pitched notes and must **subordinate
  the drummer's existing per-stroke jitter** (`groove.rs:205/293/1294`) under the new feel — all new microtiming
  gets explicit provenance, no hidden RNG after the transport.
- **GEN-5 — phrase-causal dynamics.** Promote the *law*, not the numbers (§1): a bounded style/dial off
  `PhraseGoal.energy_target` and a new gated expression field; historical default preserved.
- **GEN-6 — temporal harmonic realization / reinterpretation** — only if §7 rungs 4–5 earned it by ear.

HumanMusic must still write **songs**, not jam: every local-density mechanism stays subordinate to `SongMap`,
`FormGraph`, `ThematicMap`, discourse, closure. A return still feels like a return; a climax is distinguishable
from setup; the final section is not "same texture + 30% more notes." **Do not build a `RickComposer`** — if the
existing architecture can't express an accepted relation, document exactly *why* first.

## 9. Hard constraints (standing)

- **v0.4 control is sacred.** All new options off ⇒ historical behavior byte-exact (R17, covers, VAPOR95,
  fingerprints). The fingerprint hazard (§6) is the mechanism; honor it on every field.
- **MIDI holdout stays sealed** through the first generated Rick freeze (`RICK_CANDIDATE_1`); use the calibration
  corpus, synthetic controls, and hand probes until then, then execute the holdout once — no post-contact fitting.
- **Reference music is structural evidence, not a target.** Casiopea and the GOOD_MIDI corpus motivate generic
  relations (melodic bass, counterpoint, throughput, recurrence, coordinated groove, dynamic phrasing); the output
  stays original HumanMusic. No copied melodies, voicings, progressions, or velocity curves.
- **Ear = oracle.** Metrics reject, never certify. Blind/anonymized A/B when a dial is subtle.
- **Stems matter.** "Full mix sounds busy" is not actionable — render bass/keys/lead/drums stems so we can hear
  whether the bass is *melodic*, the comp is a *counterline*, the bass/drums *lock*, the lead keeps *hierarchy*.

## 10. Final acceptance — Rick-level is real only if

1. the **generator** produces it (not hand-authored Score probes); 2. long-range identity stays obvious;
3. the bass has independent musical meaning; 4. the comp has horizontal motion; 5. rhythmic density is
complementary, not collision; 6. harmonic complexity has function over time; 7. feel is coordinated and
attributable; 8. dynamics breathe with form; 9. `PerformanceReceipt` + `SongMapConformance` stay valid;
10. v0.4 control stays intact; 11. the positive MIDI holdout exposes no generic collapse; and — above all —
**12. the maintainer hears it and says it actually fucks.** Density alone is never Rick. A coherent leading
mechanism is favored but non-identified until her ear accepts it.
