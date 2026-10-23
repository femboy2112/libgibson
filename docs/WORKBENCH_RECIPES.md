# Instrument workbench — cold-start authoring kit (#144)

This is the recipe book for building an interactive **visual instrument** on LibGibson —
a drawing/observation surface over caller-owned data — without modifying the renderer,
duplicating the input protocol, or writing a bespoke widget/simulation framework.

The flagship consumer is [`examples/instrument_workbench/`](../examples/instrument_workbench/)
(`core.rs` is the model; `main.rs` is the runnable loop). It is **domain-neutral and
synthetic** on purpose: there is no chemistry, material, or physics in any LibGibson
symbol — those belong to the consumer. Run it:

```console
cargo run --example instrument_workbench              # interactive (needs a TTY)
cargo run --example instrument_workbench -- --selfcheck   # deterministic headless smoke
```

The seam oracles that pin every cross-layer contract are in
[`tests/instrument_workbench_slice.rs`](../tests/instrument_workbench_slice.rs); the real
terminal walkthrough is [`tests/instrument_workbench_pty.rs`](../tests/instrument_workbench_pty.rs).

Everything below uses **public APIs only**. Types: `gibson::ui::*`, `gibson::hit_test::*`,
`gibson::{gesture, edit, numeric, selection, shape, field_view, plot, world_view, clock,
snapshot, wire, input, context, session}`.

---

## Recipe 1 — a spatial canvas (paint-derived targeting)

Make the canvas a real, keyed, hittable UI target and read its **painted** rect back from
the hit-map. Anchor your `WorldView` to that rect — never to an app-computed rectangle — so
pointer targeting and world mapping both come from the same painted layout (Law L2).

```rust
use gibson::ui::prelude::*;                 // screen/column/text/compile/BuildCx/Key/skins
use gibson::ui::skin::UiEnvironment;
use gibson::{compute_layout, Rect};
use gibson::world_view::WorldView;

let el = screen().child(
    column().key("canvas").on_press(())      // a handler makes it an eligible target
        .children(heatmap_lines.into_iter().map(text).collect::<Vec<Element<()>>>()),
);
let env = UiEnvironment { width: w, height: h, ..Default::default() };
let mut compiled = compile(&el, &BuildCx::new(skins::SWISS_SIGNAL, env)).unwrap();
compute_layout(&mut compiled.node, w, h).unwrap();
let hit = compiled.hit_map(Rect::new(0, 0, w, h)).unwrap();

let canvas = hit.targets().iter()
    .find(|t| matches!(&t.key, Key::Named(s) if s == "canvas")).unwrap();
let view = WorldView::new(canvas.rect).with_pixels_per_cell(1, 1);
let layers = hit.layers().clone();           // spatial index for hit-testing clicks
```

## Recipe 2 — pointer-safe drag (one gesture = one undo transaction)

Feed **every** `Event` to one `DragGesture`; act only on `EditPhase::Commit`. The gesture
self-cancels on Esc, focus loss, resize, modal takeover and target removal — so a dropped
release can never leave a dangling capture (Law L5).

```rust
use gibson::gesture::DragGesture;
use gibson::edit::EditPhase;
use gibson::world_view::Transport;

let mut drag = DragGesture::new();
// in your event loop:
if let Some(EditPhase::Commit { intent, .. }) = drag.on_event(&event, &layers, &view) {
    if let (Transport::Mapped { value: (sx, sy), .. },
            Transport::Mapped { value: (ex, ey), .. }) =
        (intent.start_world, intent.current_world)
    {
        let (dx, dy) = (ex - sx, ey - sy);
        // ... apply ONE MoveSelected{dx,dy} transaction ...
    }
}
```

Pick the shape under a click in world space (Law L3) — a shape is hit only if its real
bounds contain the point, never guessed from coordinates:

```rust
let picked = shapes.iter().rev().find(|s| s.bounds().contains(p)).map(|s| s.id());
```

## Recipe 3 — reversible edits + undo/redo keymap

Model each edit as a `Reversible` command; wrap even single edits in a `Compound` so the
atomic rollback is always in force (a failed member leaves the model untouched — Finding B).

```rust
use gibson::edit::{Compound, Reversible, UndoKeymap, UndoLog, UndoIntent};

let mut undo: UndoLog<Compound<Cmd>> = UndoLog::new();
let keymap = UndoKeymap::standard();         // Ctrl-Z = undo, Ctrl-Y / Ctrl-Shift-Z = redo

let cmd = Compound::new(vec![/* Cmd, ... */]);
if cmd.apply(&mut model).is_ok() { undo.record(cmd); }

// on a key:
if let Some(intent) = keymap.classify(key_event) {
    match intent {
        UndoIntent::Undo => { let _ = undo.undo(&mut model); }
        UndoIntent::Redo => { let _ = undo.redo(&mut model); }
    }
}
```

## Recipe 4 — a validated numeric inspector (refuse, never clamp)

```rust
use gibson::numeric::NumericField;

let field = NumericField::new().with_range(0.0, 10.0).with_step(1.0);
match field.validate(requested) {
    Ok(v)  => { /* commit v */ }
    Err(e) => { /* model UNCHANGED; surface e — out-of-range is refused, not clamped */ }
}
```

## Recipe 5 — a scalar field + probe (reconciled with pointer selection)

Build a `FieldView` over caller-owned `&[f64]` + mask and render it through the **same**
`WorldView` the pointer uses, so a probed cell is the cell the renderer painted (Law L3).
The render returns a conservation receipt (Law L7).

```rust
use gibson::field_view::{FieldExtent, FieldView, Colormap, RangePolicy};

let extent = FieldExtent::new(origin, cell_w, cell_h, cols, rows).unwrap();
let fv = FieldView::new(extent, &data, Some(&mask)).unwrap();
let frame = fv.render(&view, area, Colormap::Heat, RangePolicy::Auto);
assert!(frame.report.conserves());           // cells accounted for, masked disclosed
let probe = fv.probe(world_point);           // index / sample / quantization, honestly
```

Linked time-series: feed a `(t, value)` history to `gibson::plot`:

```rust
use gibson::plot::{self, AxisScale, AxisSpec, FiniteRange, PlotSpec, PlotView, Series};
use gibson::SubcellGlyphMode;

let spec = PlotSpec::new(AxisSpec::new(AxisScale::Linear, "t"),
                         AxisSpec::new(AxisScale::Linear, "value"))
    .series(Series::line(series_points));
let pview = PlotView::new(FiniteRange::new(t0, t1).unwrap(),
                          FiniteRange::new(v0, v1).unwrap());
let (surface, report) = plot::plot(&spec, &pview, area, SubcellGlyphMode::Braille2x4).unwrap();
```

## Recipe 6 — a deterministic app-owned clock (play / pause / step / scrub)

The field has its **own** model tick (a `u64`), never wall time (Law L6). Convert wall
deltas to capped ticks with `FixedTimestep`; the demand discloses the debt (Finding C):

```rust
use gibson::FixedTimestep;
use std::time::Duration;

let mut ts = FixedTimestep::new(Duration::from_millis(16));   // default cap 8 ticks/advance
// each frame, while playing:
let d = ts.advance(real_dt);
model_tick += d.executed as u64;
// Law IV holds: d.owed == d.executed + d.deferred + d.discarded
// FixedTimestep::with_max_ticks(step, n) discards overflow; ::exact(step, n) defers it.
```

## Recipe 7 — input / edit replay and still capture

Record the ordered intents your app applies; replaying the same trace onto a fresh model
reproduces the exact state (a stable digest over the model proves it). Capture a still by
rendering the current frame to text or to a `Surface`. See `--selfcheck`, which runs a
scripted trace twice and reports `replay_matches=true`.

## Recipe 8 — a bounded producer channel + an ABI client

For an out-of-process (e.g. Python) producer, use the `gibson::wire` transport: a
`FrameDecoder` (bounded, rejects an oversized length before allocating) feeding a
`FieldInbox` (bounded, latest-wins per source), with a `SnapshotBridge<WireCommand>` as the
ordered, refusable return channel. See [`WIRE_PROTOCOL.md`](WIRE_PROTOCOL.md) and
`examples/smartchem_canary.rs`. For a C/C++/Python/Go client over the C ABI, see
`bindings/` (ABI 1).

## Recipe 9 — mono / keyboard-only / reduced-motion / narrow fallback

* **Mono / reduced colour:** render with `Colormap::Grayscale`; the ASCII heatmap already
  degrades to a glyph ramp. Pick glyph fidelity with `SubcellGlyphMode::{Braille2x4,
  HalfBlock1x2, Block, Ascii}`.
* **Keyboard-only:** every pointer operation has a key: arrows move the selection (the same
  `MoveSelected` intent a drag emits), `[`/`]` edit the numeric parameter, `n`/`l` draw.
  Pointer ≡ keyboard is proven in `s7_keyboard_and_pointer_moves_reach_the_same_state`.
* **Narrow 40×15:** the workbench drops the help line and keeps every core workflow; the
  digest-invariance test exercises a 40×15 terminal.

---

## API stability & known limitations

| API surface | Status | Notes |
|---|---|---|
| `ui::hit` (`Compiled::hit_map`, `HitMap`, `ResolvedTarget`) | **experimental** | painted-frame targeting; shape may change |
| `gesture::{DragGesture, DragIntent}` | **experimental** | single-pointer drag; no multi-touch |
| `edit::{Reversible, UndoLog, Compound, UndoKeymap}` | **experimental** | atomic rollback is honest, but undo is a partial inverse of an *admitted* transition only |
| `numeric::NumericField` | **experimental** | linear bounds + step; no log-domain yet |
| `field_view::{FieldView, FieldExtent}` | **experimental** | scalar fields only; no vector fields |
| `plot` | **experimental** | 2-D line/scatter; see PR #81 lineage |
| `clock::FixedTimestep` | **experimental** | debt is disclosed, not hidden; bounded vs exact is a policy choice |
| `wire` + `snapshot::SnapshotBridge` | **experimental** | in-process, no durability/exactly-once (see WIRE_PROTOCOL.md) |

Known limitations, stated plainly:

* The evolving field is a **synthetic closed form**, not a solver. There is no PDE/physics
  truth here and none belongs in LibGibson.
* The workbench demonstrates rectangles (as compartment "walls") and lines; richer geometry
  (strokes, polygons, rotation) is left to the consumer via the same `Shape` primitives.
* The deterministic digest hashes the **model**, not rendered pixels; realized images may
  differ by declared colour/glyph policy (that is the point of the invariance test).
* Mouse/focus reporting is **opt-in** and leased through the `TerminalSession`; a terminal
  without SGR mouse support still supports the full keyboard workflow.

---

## Cold-start consumer canary (acceptance #4) — a human gate

Acceptance criterion #4 asks that a **fresh author**, given only the installed SDK and these
recipes (not the renderer internals), can build a *different* geometric visual instrument,
with their boilerplate, failures, missing APIs and fixes recorded. That is a human trial, not
something this PR can self-certify — like the listening gates elsewhere in this project, it is
**deferred to the maintainer**. Suggested protocol:

1. Hand the author only this file + the public rustdoc + `examples/instrument_workbench/`.
2. Ask for a *different* instrument (e.g. a 1-D oscilloscope editor, or a grid-of-gauges).
3. Record: lines of boilerplate, every API they reached for and couldn't find, every place
   they had to read source, and the smallest fix that would have unblocked them.
4. A pass is a **runnable, validated** public-API consumer — never a capability checklist.

Until that trial runs, criteria #1–#3, #5 and #6 are met and verified by the tests above;
#4 is **staged, not claimed**.
