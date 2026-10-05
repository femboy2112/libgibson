# v0.5.0 — Observable Instruments

**Status: PLANNED / IN DEVELOPMENT. EXPERIMENTAL. Rust-only. No C ABI change, no
MSRV change, no version bump, no registry publication.** This document defines the
milestone before the code; it is a coordination artifact, not a release note.

v0.5 unifies three previously separate research lines into one milestone. They are
**independent implementation axes** — each on its own branch, each independently
reviewable and independently green before any integration candidate exists. The
milestone is *not* uncontrolled feature growth; it is three bounded instruments
plus the smallest shared law that makes them one milestone.

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

## Boundaries (what v0.5 is NOT)

- No version bump, tag, release, or registry publish.
- No feature PR merged to `main`; no Reverse-HumanMusic merge.
- The observer does not steer synthesis; no full psychoacoustic simulation.
- No FFT/statistics inside `gibson::plot`; no external image-loader dependency for
  Album Flow; no copyrighted album art; no 50-type texture/material DSL; no enormous
  generic scene framework.
- PR #80 (AI visual authoring) stays research evidence — its doctrine informs this
  milestone internally, but it is not merged as established doctrine (its blinded
  weak-model holdout is a separate later experiment).

## Integration

An `integration/v0.5-observable-instruments` branch is cut from `main` **only after**
all three feature branches are exact-head green, normal-merging the three histories.
It is not merged to `main`, not version-bumped, not tagged. A cross-axis example
(`examples/auditory_observer_lab.rs`) may plot `AuditoryTrace` material, but
`audio::perception` never depends on `gibson::plot` — the example chooses to plot;
the modules stay orthogonal.
