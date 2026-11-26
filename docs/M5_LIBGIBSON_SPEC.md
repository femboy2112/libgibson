# M5 (embodied motion): what libgibson provides and the smallest additions

Status: **PROPOSAL, docs only.** Nothing here is implemented. The proposal waits for the consumer to agree, then
for the release coordinator's build order. Written 2026-11-26 against `main` `29ca8de` (v0.9.1).

## Scope and rules

A consumer wants low-cost authored terminal motion for a bust avatar. That covers neutral and
eyelid states, bounded gaze, one acknowledgment, one reaction, interruption and settling frames,
and a mouth that may follow a text reveal without claiming lip-sync. The consumer owns the art,
its approval and every policy decision. libgibson's part is generic:

- **No inference.** Nothing here calls a model or reads semantic content to decide motion.
- **No busy loop.** Every primitive is a pure function of caller time. A primitive returns the
  next deadline, or `None`. Nothing owns a thread, a timer or a clock.
- **No policy.** The host decides *what* moves and *when* a cue is admitted. libgibson renders
  the admitted frame at the host's deadline.
- **No private art.** Every test uses synthetic `RgbaRaster` fixtures generated in the test,
  following the existing synthetic-sprite rule. No asset, digest or approval logic moves into
  libgibson.

## 1. What already exists

| Need | Existing primitive | Where | Notes |
|---|---|---|---|
| Explicit host deadlines, no implicit clock | `FrameDemand::{OnChange, After(d)}` | `src/ui/experience/grammar.rs:31` | `OnChange` means no further frame. Missed deadlines coalesce and never replay a backlog. |
| | `PresentationDriver::next_deadline` / `resolve_frame(now, ..)` | `src/stage/driver.rs:223`, `:244`, `:261` | Absolute caller time; it refuses backward time. Transitions are capped at 400 ms (`:231`). |
| | `Context::time_until_frame_needed` / `render_if_due` / `run_once(max_wait)` | `src/context.rs:352`, `:367`, `:535` | `None` when nothing is pending, so the input wait is unbounded and nothing busy-loops. |
| Explicit-time finite motion (no loop) | `MotionPlan::{is_settled, effects}` | `src/ui/motion.rs:91`, `:100`, `:106` | Finite semantic motion as `SurfaceFx`. The empty chain is the settled state. |
| | `clock::{phase, pulse, saw, triangle, ease_*, spring}`, `FixedStepClock` | `src/clock.rs:341`–`:396`, `:47` | Pure functions of `Duration`. |
| | Camera rigs `Dolly::at(t_ms, reduced_motion)` / `settled` | `src/rig.rs:107`, `:118` | The precedent for "pure function of `t_ms` plus a reduced-motion flag". The rigs are 3D only. |
| | `SurfaceFx::Offset { dx, dy }` | `src/surface_fx.rs:126` | A bounded cell displacement, enough for a nod or a lift. |
| **Explicit-time finite frame sequence** | **none** | | No flipbook type: nothing maps elapsed time to one of N authored rasters with a rest frame. **Gap G1.** |
| Layered sprites | `RasterPlane { anchor, source: Arc<RgbaRaster>, offset_px, z }` | `src/stage/raster_plane.rs:21`–`:30` | Straight-alpha planes anchored to a semantic `Key`, with a signed pixel offset after layout. They paint in `z` order, with ties in input order (`:333`). No scaling, and clipping goes to the anchor's allocation (`:96`–`:99`). |
| | `composite_over` | `src/raster/rgba.rs:172` | Exact integer alpha compositing. |
| **Registered geometry for overlays** | **none** | | Planes can stack, but nothing checks that an eyelid or mouth overlay matches a declared slot of its base. **Gap G2.** |
| Deterministic seeded randomness | `particles::Rng::new(seed)` | `src/particles.rs:13`–`:42` | Stateful and sequential. It is not a pure "occurrence k of cue c" function. |
| | Seed mixing with stable identity | `src/scene.rs:500`–`:514`, `:747`–`:750` | Internal to scene effects. |
| **Deadline-driven seeded cue schedule (blink/gaze/nod)** | **none** | | **Gap G3.** |
| Reduced-motion precedence | `MotionPreference::{Full, Reduced, None}` | `src/ui/motion.rs:13` | Applied before any effect is built. With `None` there is no effect allocation. |
| | Stage snaps under Reduced/None but keeps external timers | `src/stage/driver.rs:70`–`:71` | A strict reduced host compiles with `MotionPreference::None`. |
| Hide/detach quiescence | Complete live roster: an omitted source retires, and `OnChange` clears its request | `src/stage/driver.rs:78`–`:79` (`FrameParts::demands`) | An omitted source and no planes leave `next_deadline() == None`. |
| | `FrameDemand::OnChange` means 0 further bytes | `src/ui/experience/grammar.rs:32`–`:35` | |
| | **No public test pins "hidden means zero scheduled work"** | | **Gap G4**, a contract test only. |
| Clearing on detach | `Context::clear_live_region` | `src/context.rs:759` | Erases the live region before the terminal is released. |

## 2. Gaps: the smallest generic additions

All of these would go in a new experimental module, `gibson::stage::motion` (Release Contract tier
`stage`, EXPERIMENTAL). Each is a pure value type. New public structs with public fields are
`#[non_exhaustive]`, so later additions are not breaking.

### G1. `FrameSequence`: a finite authored flipbook with a rest frame

```rust
pub struct FrameSequence { /* private */ }
#[non_exhaustive] pub struct SequenceFrame { pub source: Arc<RgbaRaster>, pub hold: Duration }
#[non_exhaustive] pub struct SequenceSample {
    pub index: usize,
    pub source: Arc<RgbaRaster>,
    /// Time until the next frame change; `None` once at rest (no further work).
    pub next_in: Option<Duration>,
}
impl FrameSequence {
    pub fn new(frames: Vec<SequenceFrame>, rest: usize) -> Result<Self, SequenceError>;
    pub fn sample(&self, elapsed: Duration, motion: MotionPreference) -> SequenceSample;
}
```

- The sequence plays **once** in order, and then holds `frames[rest]` with `next_in: None`.
  There is no implicit loop: a repeat is a new host cue with a new origin.
- With `MotionPreference::Reduced` or `None`, `sample` returns the rest frame and `None`
  immediately. The host picks a rest frame that keeps a static expressive state.
- Interruption needs nothing new. The host samples a different sequence from its own new origin,
  and the sequence holds no state.
- `new` refuses an empty sequence, more than 64 frames, `hold` under 1 ms, an out-of-range
  `rest`, a total duration that overflows (`Σ hold > Duration::MAX`, checked with
  `checked_add`), and frames whose dimensions differ. Equal dimensions are the registration invariant: a
  frame swap never moves the ground line.
- The host feeds `next_in` to `SourceDemand { demand: FrameDemand::After(next_in) }`. The sampled
  `source` becomes a `RasterPlane`.

**Tests (synthetic fixtures):**
- sampling at every boundary, half-open intervals;
- rest after the total duration, with `next_in == None`;
- `Reduced`/`None` return the rest frame at t=0 with no deadline;
- every refusal case, including the `Σ hold` overflow (two holds near `Duration::MAX`);
- determinism: the same inputs give the same output, and there is no clock dependency;
- missed deadlines coalesce: sampling late jumps to the current frame and never replays.

### G2. `LayerSlots`: registered overlay geometry

```rust
#[non_exhaustive] pub struct LayerSlot { pub name: String, pub rect_px: PixelRect }
pub struct LayerSlots { /* private: base size + slots */ }
impl LayerSlots {
    pub fn new(base_size: (u32, u32), slots: Vec<LayerSlot>) -> Result<Self, SlotError>;
    /// Base plane at `z`, then each filled slot at `z + 1 + i`, all on `anchor`.
    pub fn planes(&self, anchor: Key, base: Arc<RgbaRaster>,
                  fills: &[(&str, Arc<RgbaRaster>)], z: i32) -> Result<Vec<RasterPlane>, SlotError>;
}
```

- `new` refuses slots outside the base, duplicate names, and an empty rect.
- `planes` refuses a base of the wrong size, an unknown slot, and a fill whose size is not
  **exactly** the slot size. There is no scaling, matching `RasterPlane`'s contract.
- It builds ordinary `RasterPlane`s, so compositing, clipping, admission limits and the
  driver all stay exactly as they are. This is a validation helper, not a renderer.
- An unfilled slot shows the base. "Eyes open" is then the base, and an eyelid overlay is a
  filled slot.

**Tests:**
- plane order and offsets;
- each refusal;
- a composite through the existing resolver equals the hand composite (`composite_over`);
- an unfilled slot is byte-identical to the base alone.

### G3. `CueSchedule`: a seeded, deadline-driven jittered cue

```rust
pub struct CueSchedule { /* seed, cue id, origin, period, min_gap */ }
impl CueSchedule {
    pub fn new(seed: u64, cue: u64, origin: Duration, period: Duration, min_gap: Duration)
        -> Result<Self, CueError>;
    /// Occurrence k: slot start `origin + k*period` plus a jitter in `[0, period - min_gap]`
    /// that is a pure hash of (seed, cue, k). `None` only if the time overflows `Duration`.
    pub fn nth(&self, k: u64) -> Option<Duration>;
    /// The first occurrence strictly after `t`, as (k, time). O(1), no iteration from origin.
    /// For `t < origin` this is k = 0. `None` only on `Duration` overflow.
    pub fn next_after(&self, t: Duration) -> Option<(u64, Duration)>;
}
```

- Consecutive occurrences are always at least `min_gap` apart and at most `2*period - min_gap`
  apart.
- The cue id stands for blink, gaze or nod; it is the host's event id. Different cues with one
  seed are independent.
- The schedule only *proposes* times. The host admits or drops each occurrence: urgent status,
  focus or STOP may suppress it, and a hidden avatar never asks.
- The host then samples a G1 sequence from the admitted time. There is no state, so hiding and
  re-showing needs nothing: the host calls `next_after(now)`.
- **Before the origin:** for any `t < origin`, `next_after(t)` returns k = 0, that is
  `(0, nth(0))`. Since `nth(0) >= origin > t`, that is strictly after `t`.
- **The jitter hash is contract.** Occurrence times are part of observable behavior (hosts and
  goldens depend on them), so a 0.9.x patch must not change the hash. It is fixed as:
  - `mix(x)` = the SplitMix64 finalizer: `x ^= x >> 30; x *= 0xbf58476d1ce4e5b9;
    x ^= x >> 27; x *= 0x94d049bb133111eb; x ^= x >> 31` (wrapping multiplies);
  - `h = mix(mix(mix(seed) ^ cue) ^ k)`;
  - `jitter = h % (span_ns + 1)` nanoseconds, where `span_ns = (period - min_gap)` in whole
    nanoseconds. The modulo bias is accepted and is part of the contract.
- `new` refuses a zero `period`, `min_gap > period`, and a `period` above `u64::MAX`
  nanoseconds (about 584 years), so `span_ns` fits in a `u64`.
- The return types are `Option` so a huge `k` or `origin` overflows to `None`, never wraps or
  saturates to a false time. (A signature change from the first draft, made while adding the
  t < origin rule.)

**Tests:**
- `nth` is fixed for a given seed (golden values) and differs across cues;
- **a hash golden test** pins `h` and `nth` for a fixed table of (seed, cue, k) values, with a
  comment that changing it needs a minor release;
- `next_after(t)` for `t < origin` (including `t = 0` with a large origin) returns k = 0;
- overflow returns `None` (`k = u64::MAX`, and an origin near `Duration::MAX`);
- the gap bounds hold over 10⁵ occurrences;
- `next_after` is consistent with `nth`, including at boundaries;
- `next_after` is O(1): no loop proportional to `t`;
- a seeded property test with the seed printed.

### G4. A contract test that hidden means zero scheduled work (no new API)

A public test pins the existing law. Take a driver with a G1-sampled plane and an
`After(d)` demand. The host then hides the avatar by omitting its source from `demands` and its
plane from `planes`. The test asserts three things:
- `PresentationDriver::next_deadline() == None`;
- `Context::time_until_frame_needed() == None` after the frame;
- the next idle `run_once` emits 0 bytes.

Re-showing at a later `now` resumes from host time and replays nothing.

### Out of scope for libgibson (host policy)

- Choosing the motion: phase or disposition to cue.
- Admission precedence: urgent status, permission text, input focus and STOP.
- Mouth timing against a text reveal, and any lip-sync claim.
- Art approval and digests.

libgibson only guarantees that planes paint inside their anchor's allocation and never take
focus. Precedence of the UI itself is the host's layout.

## 3. Placement: patch (0.9.x) or minor (0.10.0)

None of G1–G4 needs a breaking change. They are new types in an experimental module, and
nothing is added to `FrameParts`, `RasterPlane`, `PresentationOptions`, `MotionPreference` or any
other existing all-public struct or exhaustive enum. Under Release Contract §1 ("additive API
(new fn/type/module)") they are **patch-eligible**, so they could ship in 0.9.2 independently of
#444.

If agreement instead wants any of the following, that change is a 0.10.0 item and rides with #444:
- a new field on `FrameParts` (for example a "hidden" flag in place of omission);
- a new field on `RasterPlane`;
- a new `MotionPreference` variant (the enum is exhaustive).

This proposal avoids all three on purpose.

## 4. The resize question: stale avatar pixels on regrow (consumer issue #294)

**Finding:** the transient is terminal-side retained text. There are no image placements to
clear.

- libgibson's live path emits **no image protocol**.
  - The kitty graphics route is bytes and `Surface`s only: `src/graphics/kitty.rs:74`–`:126`
    (`kitty_transmit_rgba`, `kitty_place_virtual`, `kitty_delete`, `kitty_delete_all`), plus the
    per-frame plan in `src/graphics/frame.rs`.
  - Neither is referenced from `src/renderer.rs` or `src/session.rs`, and the Release Contract
    records the route as "not wired into the renderer".
  - Sixel appears only in the capability probe's parsers (`src/capability/probe.rs`); there is
    no sixel emitter.
- The avatar reaches the terminal as ordinary half-block text cells, so a protocol-level clear of
  retained placements has nothing to address on this path.
- The mechanism is consistent with libgibson's own resize contract (`src/renderer.rs`, the
  `ReanchorReport` docs):
  - "a shrink drops blank rows below the cursor and then pushes rows off the top into
    scrollback, and a grow may pull scrollback rows back down".
  - Live-region rows painted before a shrink can be pushed into the terminal's scrollback. A
    later grow pulls them back into view *before the application can write anything*.
  - That is the reported reproduction with the application stopped.
  - The first post-resize frame then repaints and clears them. `ReclaimLiveRows` rewinds and
    erases to the end of the screen. The consumer measured about 134 ms to clean.

**What libgibson cannot do:** act inside the window between the terminal's own resize and the
first byte the application writes. Rows that have reached scrollback cannot be withdrawn by an
inline renderer without erasing committed history.

**Mitigations to evaluate.** None is claimed to work until probed on the target VTE:

1. **Fullscreen on the alternate screen** for the living view. The alternate screen has no
   scrollback for the terminal to pull rows back from. This is conjectured for VTE 0.76 and must
   be probed with the application stopped: shrink, then grow, then capture.
   - **The probe must record the `RenderMode` the original repro used.** On the consumer side,
     the fullscreen path restores with `leave_alternate_screen` (GHOST `living_terminal.rs:321`)
     and `clear_live_region` runs on the Inline path only (`:340`).
   - **If the repro was Inline:** the fix may be just choosing fullscreen, or the existing
     clear on hide/detach (mitigation 2).
   - **If the repro was already fullscreen:** this alternate-screen conjecture is refuted, and
     the transient needs a new explanation before any change.
   - The consumer team owns this probe on the GHOST side; libgibson supplies no code for it.
2. **Clear the live region on hide or detach** with `Context::clear_live_region`
   (`src/context.rs:759`), so a detached or hidden avatar leaves no rows for a later resize to
   reveal. This does not help a resize while the avatar is visible.
3. **Wiring kitty or sixel later.** If the route is ever wired, `kitty_delete_all`
   (`src/graphics/kitty.rs:120`) is the teardown and detach payload. It deletes by id, because
   `a=d,d=a` does not affect virtual placements. This is irrelevant on VTE today: no image
   protocol is in use there, so nothing to place or delete.

**Recommendation:**
- Treat the stopped-application regrow transient as terminal-only.
- Run probe 1 (alternate screen), recording the repro's `RenderMode`, before any libgibson
  change. If it removes the transient, the
  fix is a consumer mode choice, not libgibson code.
- Mitigation 2 is a one-line host call that already exists.

## 5. Proposed order, if agreed

- One PR, RED then GREEN, in the experimental `stage::motion` module: G1, then G3, then G2, then
  the G4 test.
- Public synthetic fixtures only.
- A patch release, 0.9.2, on the coordinator's go.
- No C ABI change: the C API exposes no Stage today.
