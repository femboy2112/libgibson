# HumanMusic — MeatsackProjection: prep & build plan (HOLD)

> **STATUS: PREP ONLY. NOT BUILDING.** This plan is staged and waiting on Leah's explicit **go**
> and **her plan**. Nothing in `src/` gets touched for the projection layer until then. This file
> is the "current plan" — the decision scaffold and a *proposed* build ladder your plan overrides.
>
> Companion to the contract: **`docs/HUMAN_MUSIC_MEATSACK_PROJECTION_BOUNDARY.md`** (the cut,
> `AcceptedPerformance`, the nondestructive invariants, the two-way ADSR leak, the two witnesses).
> This file does **not** restate that; it turns its §9 "Open decisions" into things you can answer,
> and lays a smallest-first ladder so a "go" is executable without re-litigating scope.

---

## A. What I need from you (the decision checklist)

The boundary doc §9 lists four maintainer decisions. Here they are as choices, each with my
recommendation and *why* — and the blast radius, so you know what you're authorizing. These are
yours; I recommend, I do not resolve.

### A1. Scope vs your research branch (§0)
`origin/research/humanmusic-meatsack-projection` is docs-only, "RESEARCH DESIGN ONLY, not merged",
and scopes `P_K : G ⇀ E` (gesture→excitation under a PerformerRig/InstrumentRig) — **narrower** than
this doc's whole symbolic→PCM span.

- **(a) Umbrella — RECOMMENDED.** This boundary stands as the outer contract; your `P_K` is its
  first *internal* sub-stage. Why: it's the only option that needs **zero** changes to your
  unmerged research branch, names the acoustic-only seam *now*, and stays fully reversible.
- (b) Fold-in — this doc becomes the research branch's outer contract. Costs a merge into your
  branch and couples the two; your call to make, not mine to assume.
- (c) Rename — the acoustic-only half gets a distinct name so "MeatsackProjection" stays the
  gesture/rig layer. Cleanest nomenclature, but fragments one idea into two names before either is
  built. YAGNI until a second layer actually exists.

**Blast radius:** docs + naming only. No code.

### A2. `AcceptedPerformance` surface
- **Newtype over a receipt-passing `Composition` (narrative-aware) — RECOMMENDED.** The whole
  band-story round is about μ, carriers, and the withhold; a narrative-aware input keeps that
  witness reachable downstream. Wraps `Composition {score, song, perf}` (`functor.rs:41`, produced
  at `functor.rs:349`) whose `PerformanceReceipt::measure_under(..).passes()` (`receipt.rs:52`) is
  true.
- (alt) Thin `&Score + &MusicWorld + SampleRate` (timing/dynamics only) — throws away the narrative
  the round built. Only pick this if you want the projection deliberately story-blind.

**Blast radius:** one newtype + constructor. No behavior change.

### A3. The fixed `AcousticAssumptions` set (§5)
Which acoustic facts both sides hold constant so the two-way ADSR/lifetime coupling stays sound.
- **RECOMMENDED: the minimal set that closes the known leak** — only the ADSR/lifetime facts the
  symbolic side *already reads* (`voice.rs`, `sonority.rs`, `pocket.rs`, `expression.rs`). Name
  exactly those; don't invent a general acoustics config. Grow it only when a real second consumer
  needs a fact that isn't in it.

**Blast radius:** a small struct of constants + the doc of what's frozen.

### A4. Build any of it in v0.5?
- **RECOMMENDED: no new audio layer in v0.5** — the contract stands, per the mission's own
  "don't build the physical layer unless its contract is unusually small and fully earned." The one
  exception worth considering is **Stage 0 alone** (the `AcceptedPerformance` guard newtype), which
  *is* unusually small and fully earned and changes no audio. Everything past it holds.

---

## B. Proposed build ladder (smallest-first, each reversible, defaults-OFF)

**Your plan overrides this.** Listed so a "go" is immediately executable. Every stage: gated
(fmt + clippy + audio suite), byte-exact v0.4 PCM preserved, canonical fingerprints untouched,
defaults OFF — the §8 non-negotiables.

- **Stage 0 — `AcceptedPerformance` newtype.** A guard wrapping a receipt-passing `Composition`;
  the projection can only run on an accepted performance. Pure type safety, **no audio change, no
  new DSP**, trivially reversible. *This is the only stage I'd consider "fully earned" today.*
- **Stage 1 — name the cut.** `MeatsackProjection::realize(AcceptedPerformance, AcousticAssumptions)
  -> AcousticRealization` that *today* just wraps the existing seam — `HumanMusicSynth::new(&Score,
  &MusicWorld, SampleRate)` then `OfflineRenderer::render(..)` — emitting **byte-identical PCM**.
  Plus the `AcousticRealization` wrapper + a PCM realization fingerprint. Names/constrains/witnesses
  the seam; invents no renderer.
- **Stage 2 (bigger, optional) — the audible-exposure witness.** The per-source window-slice +
  carrier-bus-vs-rest comparison (`human_music::audition`, proposed-only) so a witness can actually
  certify carrier audibility. New code, defaults OFF, read-only on PCM.
- **Stage 3+ — OUT of v0.5 (§8).** The `P_K` gesture/rig layer (needs grouped gesture intent that
  doesn't exist: `Note`/`DrumHit` carry no string/limb/group id), physical modeling, any perceptual
  feedback loop. Not planned here; named so the ladder's top is explicit.

**Ear law still governs:** any stage that changes a sound is opt-in and judged by your ear; machine
metrics may only REJECT. Stages 0–1 change no sound (byte-exact), so they're receipt-gated only.

---

## C. Readiness — the cut points are already mapped (no discovery needed on a go)

Reusable as-is (from §7): the `src/audio/dsp` primitives; `HumanMusicSynth` + `ProductionControl`/
`StemMask`/`BusLevels`; `OfflineRenderer`/`wav`/`SampleRate`/`TempoMap`; the whole `perception`
module as downstream witness; on the symbolic side `PerformanceReceipt`/`perform_checked`/
`projection::IdentityMaterial`/`narrative_receipt`.

Genuinely new (from §7): the `AcceptedPerformance` newtype + `AcousticRealization` wrapper; any event
input richer than `Note.velocity` + `start_beat` (the synth's `NoteEvent` has no attack-shape,
brightness, or per-event pan — velocity is a linear amplitude scalar at `synth.rs:181`); the
audible-exposure glue; a PCM realization fingerprint; and — only if the rig stage is in scope —
grouped gesture intent + rig types + typed infeasibility.

Terminology hazard (don't trip on it): `src/audio/human_music/projection.rs` is **"Identity
projections"** (a symbolic predicate), unrelated to acoustics. Always write **MeatsackProjection**
in full.

---

## D. The hold

I am **not** writing projection code. On your **go + plan** I execute the ladder your plan dictates
(not necessarily mine), gating every stage and keeping every §8 default OFF. Until then this file
and the boundary doc are the whole footprint.
