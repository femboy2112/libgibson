# v0.5.0 — Observable Instruments

**Status: PLANNED / IN DEVELOPMENT. EXPERIMENTAL. Rust-only; no C ABI or MSRV
change currently planned. NOT YET RELEASED.** Feature/research branches do not bump
versions, tag, merge to main or publish without the maintainer's final approval.
This document is a living coordination plan, not a release note.

**FINAL v0.5 RELEASE GATE (owner directive, 2026-10-09):** v0.5 must conclude with
an **epic upgraded meme intro film** showcasing the **actual accepted new features**
with **Filthy Frank chroma-keyed onto the same terminal-rendered visuals, reacting**
like the previous `libgibson_intro_reaction` demo — not merely a separate clip, or
a replay of the old intro. The executable, captured soundtrack, full reaction cut,
source/provenance evidence and the maintainer's watch/listen verdict are mandatory.
See **[v0.5 Meme Intro Finale — release flagship and acceptance](V0_5_MEME_INTRO_FINALE.md)**.
No v0.5 release is considered complete before this finale is actually accepted.

The original three research axes below remain distinct and individually reviewable.
Subsequent v0.5 development also includes the staged cinematic authoring kit
(PR #86, on spatial PR #83) and the independently moving Rick-C137 HumanMusic
performance (PR #87). These are **real integration responsibilities**, not fourth
and fifth data observers to jam into the initial three-axis model. The final movie
is their artistic and technical integration proof. New features may be added to the
milestone if they strengthen it and receive their own truthful evidence.

## The common law

> **DERIVED REPRESENTATIONS DO NOT REWRITE THEIR SOURCE.**

Every axis preserves provenance: an explicit transform from an authoritative source
to a derived view, and a receipt for what the transform did. Pan/zoom never rewrites
plot data; the auditory observer never rewrites Score/PCM; a reflection is derived
from a cover, never a second hand-authored identity. The source is read-only; the
derived thing carries a transform and a receipt back to it.

---

## Axis A — Observable Geometry (`gibson::plot`)

```
scientific observables → validated coordinate transforms → compiled plot geometry → capability realization
```

Applications own analysis (FFT, regression, statistics — permanently out of scope).
The plot library owns *faithful realization* and reports what it did with every
sample via `PlotReport`. Branch `feat/v0.5-scientific-plotting` (PR #81, draft).

**State:** the most mature axis. Kernel hardened through an adversarial audit, an
external integration canary (Project PULSAR-2), and a post-canary fix pass; a
pre-exposure API-hygiene pass (annotation receipt, `PartialEq` determinism seam,
re-exports, label-as-metadata, deferred marker seam) is staged. See
[`PLOT_OBSERVABLE_GEOMETRY.md`](PLOT_OBSERVABLE_GEOMETRY.md).

## Axis B — Auditory Observation (`gibson::audio::perception`)

```
Performance → (physical realization R) → AcousticScene → (context-indexed observer A_c) → AuditoryScene
```

A **forward, observer-only** layer: it listens, it does not synthesize. No synthesis
feedback, no mixing automation, no `PerceptualContract` steering, no Reverse-HumanMusic
inversion until the observer survives controls. Listening context is explicit
(`LevelCalibration::{Relative, Calibrated}`, explicit sample rate and channel
interpretation, no hidden global listener state). The `AuditoryTrace` keeps multiple
witnesses distinct — physical energy, auditory excitation, modeled loudness, transient
salience — never one `human_quality: 0.87` scalar. Branch `feat/v0.5-auditory-observer`
(draft PR when implementation begins). Design:
[`HUMAN_MUSIC_AUDITORY_OBSERVER.md`](HUMAN_MUSIC_AUDITORY_OBSERVER.md).

Generic auditory analysis under `src/audio/perception/` stays free of SongMap,
chords, CoverMap, worlds and semantic role labels; HumanMusic-specific interpretation
lives separately under `src/audio/human_music/`.

## Axis C — Spatial Interface (`gibson::ui` + `raster3d`)

```
semantic object → visual identity → spatial realization → camera / direction → Surface
```

The high-level UI stays excellent for controls, text, forms, inspectors and chrome.
Spatial/cinematic domains stop being forced through a widget-grid mental model. This
axis owns: generic UI defects PULSAR surfaced (first: the modal content-cap, issue
#82), the smallest reusable spatial-graphics primitive (a perspective-correct textured
plane — *only if it earns generality*, otherwise demo-local), and the Album Flow
flagship demo. Branch `feat/v0.5-spatial-ui` (draft PR when pushed).

---

## Cross-axis culmination — the upgraded Filthy Frank meme intro (REQUIRED)

[Read the full executable/cinematic/reaction/audio release gate](V0_5_MEME_INTRO_FINALE.md).
The final v0.5 demonstration is an evolved `libgibson_intro_reaction`:
real plotting and audible observation, changing spatial grammars and album flow,
source-identical cinematic transformations, **C137's genuinely generated band
story**, and the same locally provided Filthy Frank subject *keyed directly into*
those visuals. One authoritative edit clock and one renderer; cue-aware soundtrack
and dialogue with no invented musical evidence; local media kept out of git.

Neither a static montage nor the old v0.3.1 reaction render satisfies this gate.
The **maintainer watches and listens** before accepting the v0.5 release; tests
cannot certify the comedy, timing, visuals or musicality. If the local Frank media
is unavailable, no-reaction/synthetic CI may pass but the **full finale is still
UNVERIFIED**. The film's shot map is editable as further features land.

---

## Boundaries (what v0.5 is NOT)

- No version bump, tag, release, registry publish or merge directly from a feature/research branch **before** integration plus the final meme-intro acceptance and explicit owner approval.
- No automatic merging of feature PRs to `main`; no Reverse-HumanMusic merge.
- The observer does not steer synthesis; no full psychoacoustic simulation.
- No FFT/statistics inside `gibson::plot`; no external image-loader dependency for
  Album Flow; no copyrighted album art; no 50-type texture/material DSL; no enormous
  generic scene framework.
- PR #80 (AI visual authoring) stays research evidence — its doctrine informs this
  milestone internally, but it is not merged as established doctrine (its blinded
  weak-model holdout is a separate later experiment).

## Integration

Cut or update an `integration/v0.5-observable-instruments` branch from the accepted
current `main` only when component heads and their dependency graph are audited,
then reconcile and test the **actual combined commit**. PR #86 is stacked on
spatial PR #83; HumanMusic C137 PR #87 is independently changing. Do not assume
separate green branches imply the merged work is green. Freeze only interfaces
necessary for source-true composition, not the art direction.

First prove a silent integrated cinematic capture, then score the edit with the
accepted HumanMusic generator, overlay and synchronize the actual local keyed
Frank reactions, and run the full artistic/technical release gate documented in
`V0_5_MEME_INTRO_FINALE.md`. **Until the maintainer has watched/listened and
approved, do not tag, release, or merge as a final v0.5 deliverable.** A cross-axis
plot of `AuditoryTrace` can live in a consumer example; `audio::perception`
never depends on `gibson::plot`. Modules remain independently useful.
