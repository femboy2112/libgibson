# Resource ownership & retention inventory (issue #10)

**Status: the only engine-retained *unbounded* growth in a long session was the
Story trace (`steps` + `beats`); Round II bounds both. Every other structure
inventoried in this pass is dimension-bounded, hard-capped, schema-bounded, or
caller-owned.**

Issue #10 is not "cap everything that allocates." It is: distinguish a **memory
leak** (the engine retains information the caller never asked for and cannot shed)
from **requested retention** (the caller deliberately built up state the engine
faithfully holds). This document inventories every retaining structure the issue
named, classifies it, and records what actually bounds it.

## Classification

Bound classes: **engine-retained** (the engine grows it on its own as a session
runs) · **application-retained** (grows only when app code inserts) · **hard
capped** · **dimension bounded** · **schema bounded** · **caller controlled** ·
**intentionally unbounded**.

| Structure | Location | Grows per | Bound | Driven by |
|---|---|---|---|---|
| `StoryTrace.steps` | `src/story.rs` | `update()` call | `All`: intentionally unbounded (opt-in default); `Bounded`/`Disabled`: hard-capped window (≤`2·cap`) | **engine** |
| `StoryTrace.beats` | `src/story.rs` | transition (`enter`) | same policy as `steps` (Round II closed this) | **engine** |
| `Facts` (static actions) | `src/story.rs` | — | **schema bounded** — finite key vocabulary baked into the `Story` definition | engine (caller-authored schema) |
| `Facts` via `facts_mut()` | `src/story.rs` | caller insert | **caller controlled** — arbitrary keys, no eviction API; not recorded in the trace | application |
| `StoryDirector.mounted` | `src/story.rs` | `Mount` action | **schema bounded** — ≤ number of defined bundles; `Unmount` removes | engine |
| `Scene.entities` | `src/scene.rs` | `Scene::add` | **caller controlled, add-only** — no removal API at all; caller resets via a fresh `Scene` | application |
| `Scene` tag interning | `src/scene.rs` | new tag string | **caller controlled** — intern table, bounded by distinct strings (dedups) | application |
| `Surface.cells` | `src/surface.rs` | `resize` only | **dimension bounded** — `W×H` (`u16²`); no per-frame growth, `resize` reallocates | caller |
| `RgbRaster.pixels` | `src/raster.rs` | `new` only | **hard capped** — `MAX_RASTER_DIMENSION = 2048` → ≤ ~12.6 MiB | caller |
| `FeedbackBuffer` | `src/raster_fx.rs` | `update()` resize | **hard capped** — shares the 2048² ceiling (~108 MiB worst case, doc'd) | mixed (engine trigger, hard ceiling) |
| `ParticleSystem.particles` | `src/particles.rs` | `spawn`/`burst(n)` | **caller controlled** — `n` uncapped per call; `update()` reaps dead; no engine auto-spawn | application |
| `Mesh` | `src/geom.rs` | construction | **caller controlled** — static once built | application |
| `Replication.nodes` | `src/replication.rs` | `update()` generation | **hard capped** — `max_generation`/`max_nodes`, `clear()` on collapse | engine |
| `FocusRing.stack` | `src/focus.rs` | `capture()` | **caller controlled** — modal depth, balanced by `release()` | application |
| `ViewportState` | `src/viewport.rs` | — | no collection (two `i32`s) | — |
| `TextInputState` | `src/input.rs` | — | single-line editable text, replaced not accumulated | application |

**Provenance.** The `story.rs` rows are authored/verified in Round II. The
`Scene` (no removal API), `RgbRaster`/`FeedbackBuffer` (2048² cap), `Replication`
(`max_generation`/`max_nodes` + `clear()`), `Facts` (no removal API), and
`FocusRing` (balanced push/pop) rows were independently grep-verified against the
source. The `Surface`/`particles`/`Mesh`/`Viewport`/`TextInputState` rows are
OBSERVED from a code survey (file:line confirmed) but not exhaustively re-read
line-by-line.

## The only leak candidate was the Story trace

Exactly one structure on this list was **both** engine-driven **and** reachable at
an unbounded state by doing nothing: `StoryTrace` (`steps` grows every `update`,
`beats` every transition, default policy `All`). Measured worst case (1M ticks,
release, Rust 1.98.1, ping-pong story): **109.5 MB** RSS (40 MiB steps + ~44 MiB
beats). `TraceRetention::Bounded(1024)` on the same workload: **0.43 MB**, both
logs windowed, `dropped_steps`/`dropped_beats` honest, `is_complete()` false.
`Replication.nodes` is the clean contrast — same engine-driven shape, but capped
by construction from day one.

## Facts and Scene are caller-owned domains (A4)

`facts_mut()` and `Scene::add`/tag interning let an application accumulate state
with no engine-side eviction. This is **caller-owned retention, not an engine
leak**: the engine adds nothing on its own, growth is one-to-one with explicit
caller inserts, and the caller controls lifetime (drop the `Scene`/`Facts`, or
build a fresh one). Inventing Scene garbage collection or a Facts TTL to "close
the issue" would be the over-engineering #10 explicitly warns against. The one
sharp edge worth stating: neither `Facts` nor `Scene` exposes a remove API, so an
application that inserts unique keys/entities in an unbounded loop *without*
rebuilding the container will grow — that is the caller's resource to manage, and
it is now documented as such.

## Checkpoint/export is unnecessary now (A6)

Issue #10 mentioned checkpoint/export semantics. It does **not** need new
persistence infrastructure. `TraceRetention` (bound the live buffer) plus
`drain_trace()` (move the retained history out to a caller-chosen sink, leaving
the live buffer honestly `is_complete()`-false) already give applications a
complete external-sink model: cap memory in-process, flush on a cadence, and never
be lied to about completeness. `StoryTrace` is `pub` and `Clone`, so a caller who
wants durable checkpoints serializes a drained trace with their own format. An
engine-owned checkpoint type would add surface and API-stability commitments with
no demonstrated caller need — deferred until one exists.

## Boundary

Byte figures for the Story trace are measured (`long_session_probe`); the rest are
static classification, not per-structure runtime byte counts. FFI-side retention,
the scheduler, and the transaction layer were not inventoried here — if any holds a
caller-facing retained buffer, that is outside this pass.
