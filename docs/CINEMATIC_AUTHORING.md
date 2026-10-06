# Cinematic Authoring with LibGibson

A cold-start guide for humans and coding agents who want to build something
*visually insane* in the terminal — a directed, spatial, time-based show — without
reading the flagship's source, and without the library dictating what the show
looks like.

> **LibGibson helps you make weird things. It does not tell you what weird thing to
> make.**

That sentence is the whole philosophy. Everything below exists to make the
mechanical parts — time, seeking, deterministic replay, compositing, perspective
media, scientific plots, semantic interfaces — *easy and boring*, so that all your
effort goes into the part only you can do: the metaphor, the camera, the weirdness.

---

## 1. The boundary (read this twice)

There is a hard line between what the library owns and what you own. Keeping it is
what prevents LibGibson from becoming a walled garden with one house aesthetic.

| The **library** owns (mechanics) | The **author** owns (art) |
|---|---|
| the deterministic edit clock ([`timeline::Timeline`]) | the visual metaphor |
| cue resolution: who's on screen at time *t* | the semantic identities |
| frame pacing / the app loop ([`ui::App`]) | the camera choreography |
| the compositing substrate ([`Surface`], [`raster::RgbRaster`]) | the world geometry |
| perspective media ([`raster3d::Rasterizer::textured_quad`]) | the pacing and holds |
| semantic interface grammars ([`ui::experience`]) | which grammar, when |
| scientific realization ([`plot`]) | the analysis behind the plot |
| headless capture to ANSI/PPM | the weirdness |

If you ever find yourself wanting a `CameraShot::EpicReveal` enum variant *in the
library*, stop: that is art direction, and it is yours. The library gives you a
`Camera` with a position and a target; the epic reveal is a path you move it along.

---

## 2. The one law: a show is `f(edit)`

Everything hangs off a single stored number — the **edit clock**, in seconds. Every
frame is a pure function of it. Nothing on the frame path reads a wall clock.

```rust
use gibson::timeline::Timeline;

let show = Timeline::new()
    .cut(3.0, "title")            // hard cut, 3 s
    .crossfade(5.0, 1.0, "body")  // dissolves in over 1 s, runs 5 s
    .cut(2.0, "outro");

// At any time t, who is on screen and how far into their own shot are they?
let now = show.top(7.0).unwrap();           // dominant cue at t = 7 s
let _ = (now.payload, now.local, now.progress, now.weight);
```

Because the frame is pure `f(edit)`, you get four things for free, which are
exactly the things weak terminal demos lack:

- **Seeking** — render any instant in isolation (`show -- at 7.0`).
- **Deterministic replay** — the same `t` always yields the same frame.
- **Capture** — dump a frame, or a whole sequence, headlessly for inspection.
- **A music/reaction seam** — a later pass maps the *same* cues to a score or a
  reaction cut. The timeline stores opaque cues, never tones; the projection
  `M : Timeline -> score` is owned elsewhere (see §7).

The payload `T` is yours and opaque — a shot id, an enum, a closure handle. The
timeline never looks inside it; *you* decide what to draw for each cue.

---

## 3. Progressive disclosure: drop in at any level

You are never forced up the stack. Each level is optional; drop to a lower one the
moment you want more control.

- **Level 0 — pixels & geometry.** [`Surface`] (cells), [`raster::RgbRaster`]
  (RGB pixels, 2 px rows → 1 half-block cell via `to_surface()`), [`raster3d`]
  (`Camera`, `Rasterizer`, `textured_quad`). Draw anything.
- **Level 1 — declarative chrome.** [`ui`] nodes (`screen`, `col`/`row`, `panel`,
  `text`) for controls, labels, forms, inspectors. Compose chrome declaratively and
  paint the *drawing* imperatively into a `Surface`.
- **Level 2 — semantic interface.** [`ui::experience`]: one model, many grammars.
  Build an `Experience`, present it in any `ExperienceStyle`; selection and
  semantics survive a change of representation.
- **Level 3 — time.** [`timeline::Timeline`]: cue resolution on the edit clock. The
  one reusable "show" primitive.
- **Level 4 — the kit.** The recipes (§5) and the show plan
  ([`docs/SHOW_PLAN_TEMPLATE.md`]) — patterns, not machinery.

There is deliberately **no mandatory runner and no scene ontology**. A show renders
a raw `Surface`, a `raster3d` world, an `ExperienceRuntime`, a `plot`, or arbitrary
code of yours — the time layer composes with all of them and subsumes none.

---

## 4. The five-step method

A workflow that reliably produces coherent shows instead of slideshows. Do steps
1–3 on paper (or in [`docs/SHOW_PLAN_TEMPLATE.md`]) — **no UI widgets yet**.

1. **Semantic anchors.** What identities must *survive* the whole show? If the
   audience can lose the thread when the visuals change, you have a slideshow. Name
   1–4 anchors.

2. **Representations.** What radically different visual bases can express those
   anchors? A list. A shelf. A ring. A plot. Terrain. The further apart the bases,
   the stronger the argument — *if* the anchor survives the jump.

3. **Directed sequence.** What does the audience *learn* in each act? One lesson per
   act. Order them so each act earns the next.

4. **Transitions.** How does identity survive a change of basis? Name the invariant
   (the anchor) and the transformation. The seam between two acts is where shows
   fail; design it explicitly.

5. **Acceptance.** Actually render and *inspect* — keyframes, the transition
   corridors, a constrained size, mono, and the motion. See §6. The defects live
   between the hero frames; the weak-demo failure mode is *never looking*.

---

## 5. The recipes — one law each

Each recipe is small, self-contained, and built only on public API. Copy one whole;
none requires reading another's internals or the flagship's source.

| Recipe | The one law it teaches | Run |
|---|---|---|
| `show_minimal` | a show is `f(edit)`: `cut`, `top()`, local time, direct seek | `cargo run --example show_minimal` |
| `show_crossfade` | `crossfade` overlap, `resolve()` weights, weighted compositing | `cargo run --example show_crossfade` |
| `show_spatial_plane` | a `Surface` as a textured quad, an explicit moving `Camera` | `cargo run --example show_spatial_plane` |
| `show_observable` | `plot` as one directed scene; the data stays *yours* | `cargo run --example show_observable` |
| `show_experience` | an `ExperienceRuntime` in a timeline; style switches, selection survives | `cargo run --example show_experience` |

Each also takes a headless seek argument (`-- at T`, or `-- shot T out.ppm` for the
spatial one) so you can inspect a single instant without a terminal.

### The compressed spatial recipe, in full shape

`show_spatial_plane` is the heart of "make it cinematic": take any 2-D raster, hang
it in a 3-D world, and give the *camera* the opinion.

```rust
use gibson::geom::Vec3;
use gibson::raster::RgbRaster;
use gibson::raster3d::{Camera, Rasterizer};

let texture: RgbRaster = /* draw anything */ RgbRaster::new(96, 54);
let mut rz = Rasterizer::new(w, h * 2);     // pixel buffer (2 px rows per cell)
rz.clear((6, 5, 14));

let cam = Camera {
    position: Vec3::new(0.0, 0.3, -5.0),    // move THIS over edit time
    target: Vec3::new(0.0, 0.0, 0.0),
    up: Vec3::new(0.0, 1.0, 0.0),
    fov_y: 0.9, near: 0.1, far: 60.0,
};
// A plane in the xy-plane, wound TL -> TR -> BR -> BL to match the texture UVs:
let (hw, hh) = (1.6, 0.9);
rz.textured_quad(
    [Vec3::new(-hw, hh, 0.0), Vec3::new(hw, hh, 0.0),
     Vec3::new(hw, -hh, 0.0), Vec3::new(-hw, -hh, 0.0)],
    &texture, &cam,
);
let surface = rz.raster.to_surface();       // -> terminal cells
```

Perspective is correct, depth-tested, and the quad is not backface-culled. To make
it a *show*, drive `cam.position` from the edit clock (a `Timeline` of camera legs,
each leg eased by its `progress`). That is the entire trick behind the flagship's
fly-by — no texture-mapping framework, just a moving eye.

---

## 6. Capture & inspection is a first-class path

The strongest finding from prior work: weak agent-generated UIs were weak because
**the agent never looked at the output**. Make looking cheap.

Every recipe supports a headless seek. Build the same habit into your own show:

```
myshow                 # live, loops
myshow -- at 7.0       # print the single frame at t = 7 s (text snapshot)
```

For a color-accurate frame, render a `RgbRaster` and `write_ppm`, or capture ANSI
bytes headlessly (the flagship's `cinematic_showcase.rs` shows the
`UiRuntime::frame` → `Context::headless` → `rendered_bytes` path, and a `seq DIR`
mode that writes a whole 20-fps sequence for an MP4).

### The transition corridor

A show's defects do not live on the hero frames — they live in the **corridors
between them**. For every major boundary `b` (a cut, a dissolve, an act change),
inspect three frames:

```
myshow -- at <b - 0.1>     # just before
myshow -- at <b>           # exactly at
myshow -- at <b + 0.1>     # just after
```

and also inspect at a **wide** terminal, a **compact** one, and with color forced to
**mono** (ASCII). A show that only looks right at one size, in truecolor, on the
hero frames, has not been inspected. The show plan's `ACCEPTANCE` block exists to
make you do exactly this.

---

## 7. The music / reaction seam (don't build it here)

A future soundtrack or reaction-cut layer reads the *same* edit you already have:

```rust
for cue in show.cues() {
    // map cue identity / purpose -> a semantic score event (owned by the music module)
}
```

But do **not** store `Tone`, `Density`, `Emphasis`, or reaction kinds inside the
timeline. That would couple the picture to layers that may not exist yet. The
timeline's payload stays opaque; the soundtrack owns the projection
`M : Timeline<Shot> -> SemanticTrace`, exactly as *visual realization* is distinct
from *timeline semantics*. The show plan captures music/reaction **intent** in prose
for that future pass — never as data the generic layer parses.

---

## 8. What stays yours (anti-patterns the library refuses)

These are art direction and must stay in *your* program, never the library:

- spiral/ring geometry, panel arrangement, cover-flow spacing;
- nebula/starfield, bloom values, vignette, the grade;
- camera shot choices, the "dwell" pose, the punch-zoom;
- a specific grammar's visual identity, titles, copy, meme timing.

The flagship (`examples/cinematic_showcase.rs` and its `cinematic_showcase/`
modules) is one *example* of these choices — a reference, not a framework. Read it
for inspiration, but you do not need it to build a show: the recipes and this guide
are the kit. If your show comes out looking like the flagship's nebula-and-ring,
that is a sign you leaned on our aesthetic instead of inventing your own.

---

## 9. Cold-start checklist

1. Write a show plan ([`docs/SHOW_PLAN_TEMPLATE.md`]): anchors → representations →
   acts → transitions → acceptance.
2. Pick the lowest level that does the job (§3). Most spatial shows live at Level 0
   + Level 3 (rasters/quads on a timeline).
3. Build a `Timeline` of your opaque shots; write the `f(edit) -> Surface` frame
   function.
4. Add a headless `at T` seek from day one (§6).
5. Inspect the transition corridors, compact, and mono *before* you believe it.
6. Keep the camera, world, and look in your program; keep time, cues, and capture
   in the library.

Further reading: [`docs/SHOW_PLAN_TEMPLATE.md`] (the thinking fixture),
[`docs/INTRODUCTORY_CINEMA.md`] and [`docs/FRANK_REACTION_CUT_PLAN.md`] (the gold
standard for cue sheets, shot grammar, and the two-clock discipline the timeline
formalizes).
