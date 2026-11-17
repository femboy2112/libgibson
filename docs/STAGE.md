# Stage: semantic mounting (experimental)

The first Stage seam is `gibson::stage::map_actions`. Mount the adapted `Element`
as a child of your existing semantic tree, then reconcile the complete tree once
with the host's `UiRuntime`. This keeps action identity, targets and modal focus
in one domain. Context continues to own the terminal and event loop.

```rust
use gibson::stage::map_actions;
use gibson::ui::prelude::*;
use std::time::Duration;

#[derive(Clone)]
enum InspectorAction { OpenEvidence, Close }
#[derive(Clone)]
enum Action { Inspector(InspectorAction), Cancel }

let inspector = column()
    .key("inspector")
    .child(button("Evidence").key("evidence")
        .on_press(InspectorAction::OpenEvidence))
    .overlay(modal("Details").key("details")
        .on_dismiss(InspectorAction::Close));
let view = column()
    .child(map_actions(inspector, Action::Inspector))
    .child(button("Cancel").key("cancel").on_press(Action::Cancel));
let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
let frame = ui.frame(&view, UiEnvironment::default(), Duration::ZERO)?;
// Supply frame.node to the existing Context. Route input through this same ui.
# Ok::<(), gibson::ui::UiError>(())
```

The adapter maps press, edit, custom-event and modal-dismissal payloads, including
those in overlays. It preserves layout, keys, input snapshots, selected/disabled
state, revisions, motion intent and paint content. The host still owns editor
state and acts on emitted actions. Explicit keys must be globally unique; the
existing compiler rejects duplicates. Positional keys retain their structural
meaning and are unsuitable for identity across arbitrary reordering.

For an Experience grammar, adapt its `Presented.element` before compilation and
retain its receipt and demand. Keep its navigation state. Give modal, editor and
custom-handler capture priority. For arrow/selection keys owned by the active
Experience pane, route to Experience before generic UI arrow traversal when UI
focus belongs to that frame's `Presented.receipt.actions`, or the active pane has
no UI focus. An unrelated focused pane retains its keys. Enter/pointer/Tab go
UI-first; forward only unconsumed owned keys as Experience fallback. Use
`handle_key_outcome` so consumed navigation stops routing even without an action.
Never forward an already consumed event. Hosts choose pane ownership and preserve
it after rebuilding; LG does not enforce this routing contract or add a second
focus owner.
Action mapping cannot distinguish actions a producer already erased to `()` or
restore nested controls flattened into a Surface. Producers needing distinct
controls must expose the semantic tree and meaningful action payloads.

Do not use `raw(inner_compiled.node)` as a semantic mount. Visual imports clear
foreign hit tags and do not import interaction sidecars. Ordinary Scene movement
of an already compiled tree remains available when the node and its original
sidecars stay together; see [UI integration](UI_LAYER.md).

The semantic adapter adds no separate mount wrapper, focus registry, input
dispatcher, terminal owner or cache. No physical inline-pointer or aesthetic
guarantee is implied by the headless semantic checks.

## Resolved raster composition (experimental)

**Signed basis:** `resolve_raster` owns the Node/taffy layout call under a declared
cell viewport. An anchor's resolved cell origin maps to raster pixels `(x, 2*y)`;
`RasterPlane::offset_px` is then added in signed i32 pixel space. An odd vertical
offset remains an odd pixel offset until the final half-block realization.
Existing painter traversal supplies ancestor clips and positioned/camera origins.
No negative coordinate is passed to `WorldView::raster_to_cell`. There is no
additional world transform.

Use `RasterAnchor::Key` with `resolve_compiled_raster` and the same `Compiled`
frame that owns the semantic tree. The compiler records an experimental
`geometry_paths` join during that exact lowering pass for every explicit Key,
including noninteractive layout elements. It follows the actual nodes through
chrome and overlay wrappers. No persistent identity registry or guessed semantic
path conversion is added. Missing keys are refused before layout; duplicate keys
remain compiler errors. Reorder/resize uses the new frame's join, so the raster
follows its element. Do not structurally edit `compiled.node` before resolution;
structural edits invalidate this frame-local join.

`RasterAnchor::FramePath` remains a low-level escape hatch for raw Node trees.
It is explicitly unstable across reorder and must be re-resolved for every frame
or structural edit. `resolve_raster` accepts these raw paths; a Key passed without
its Compiled frame is refused. Semantic consumers should use the keyed entry.

Presentation offsets apply after layout:
the anchor still reserves its original flow allocation, and moving its raster
does not reflow siblings. Sources are never implicitly scaled. Pixels outside
the translated allocation or ancestor/viewport clip are omitted, with the exact
painted pixel clip reported. Planes paint in ascending z, then input order.

```rust
use gibson::{Node, raster::{RgbRaster, RgbaRaster}, stage::*};
use std::sync::Arc;
let tree = Node::row().width(8.).height(2.)
    .child(Node::col().width(2.).height(2.))
    .child(Node::col().flex_grow(1.));
let mut underlay = RgbRaster::new(8, 4);
underlay.clear((20, 40, 60)); // The actual, current RGB substrate.
let pixels = RgbaRaster::from_pixels(1, 1, vec![[200, 0, 0, 128]]).unwrap();
let plane = RasterPlane { anchor: RasterAnchor::FramePath(vec![0]), source: Arc::new(pixels),
    offset_px: (-1, 1), z: 0 };
let frame = resolve_raster(&tree, (8, 2), &underlay, &[plane],
    RasterLimits { planes: 8, nodes: 64, pixels: 4096 })?;
assert_eq!(frame.surface.width, 8);
# Ok::<(), RasterError>(())
```

For semantic placement, compile the complete action-typed tree once with the
host UiRuntime, then use the same resulting frame:

```rust
use gibson::{raster::{RgbRaster, RgbaRaster}, stage::*, ui::prelude::*};
use std::{sync::Arc, time::Duration};
let tree = row::<()>().width(8).height(2).gap(0)
    .child(column().key("presence").width(2).height(2))
    .child(column().key("reading").grow(1.));
let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
let compiled = ui.frame(&tree, UiEnvironment { width: 8, height: 2,
    ..Default::default() }, Duration::ZERO)?;
let plane = RasterPlane { anchor: RasterAnchor::Key(Key::named("presence")),
    source: Arc::new(RgbaRaster::new(2, 4)), offset_px: (0, 1), z: 0 };
let raster = resolve_compiled_raster(&compiled, (8, 2), &RgbRaster::new(8, 4),
    &[plane], RasterLimits { planes: 8, nodes: 64, pixels: 4096 })?;
// Paint semantic content above raster.surface; retain compiled interaction sidecars.
# Ok::<(), Box<dyn std::error::Error>>(())
```

The underlay must be opaque RGB, exactly one pixel per cell horizontally and two
vertically, within the existing 2048-pixel per-axis ceiling. `Surface` and text
underlays are unsupported by this API: it cannot infer glyph pixels, blend alpha
over arbitrary text or invent a matte. Keep semantic text above the resolved RGB
background with the existing painter and host UI runtime. `frame.layout` is the
laid-out tree; this function does not paint its text/canvas callbacks or create
hit targets. Source Arcs remain immutable and are read anew each resolution.

Explicit limits cover plane count, Node count, and canvas pixels plus twice all
source pixel counts (source and worst-case scratch allowance). Shared/offscreen
sources count for every plane. This is a bound for this composition pass, not a
budget for arbitrary caller Node callbacks or caller-owned source allocations.
Admission rejects invalid dimensions, missing anchors and aggregate excess before
cloning/layout. Resolution returns no partial frame on error. Fully clipped
planes have an empty clip receipt. RGB composition reads the current destination
under each plane; only after all planes are composed is the entire RGB frame
converted to half-block cells once. Capability quantization stays with Renderer.

These RGB receipts do not clear physical inline resize debt or establish
performance or appearance on a consumer's private artwork.

## Explicit-time presentation and inspection (experimental)

`PresentationDriver` retains temporal metadata only: keyed rectangle segments and
absolute source deadlines. It owns no Node, RGBA source, Surface, decoder cache,
terminal, focus ring or event loop. There is no input/event/invalidate method.
`FrameParts` consumes the current whole-tree `Compiled<A>` and returns its action
sidecars in `StageFrame<A>`. Build all producers and the one UiRuntime at the same
caller-supplied `Duration`; backward time is refused. For a backward seek, replay
with fresh driver, UiRuntime and any stateful producers.

The driver admits the existing raster limits and solves Node/taffy layout once
under the declared viewport. It transports this frame's `geometry_paths` through
an ordinary fixed viewport wrapper. Graphical planes require `RasterAnchor::Key`;
raw `FramePath` remains available through seam 2, not as temporal identity. All
planes sharing a key follow that key's layout rectangle. Their explicit residual
`offset_px` remains pixel alignment, not an implicitly animated target.

A changed layout rectangle retargets a linear graphical segment from the OLD
segment sampled at exactly `now`. Both origin and extent interpolate in the
existing 1x2 raster basis; the extent crops unchanged source pixels, never scales
or resamples them. The transition is at most 400 ms of caller time (longer options
are refused). Zero duration snaps. Late samples coalesce to current presentation,
not a replay backlog. Current ancestor/viewport clipping still applies. A graphical
pose can differ from its target layout allocation while moving; both are reported.
Semantic text and hit geometry take their current layout immediately, reserving
room at once. Animated semantic-pane translation is not implemented.

Reduced/None Stage motion snaps immediately and schedules zero further graphical
animation frames. The strict reduced host fixture compiles UI and grammars with
`MotionPreference::None`, rather than UiRuntime's optional brief-emphasis Reduced
mode. This is an explicit host realization, not a change to global UI behaviour.
External `After` requests stay literal: an informational timer is not silently
removed by a placement preference. With all producers `OnChange` and no active
segments, `next_deadline()` is `None`.

Supply the complete live `SourceDemand` roster each resolved frame. Source roots
must be explicit keys in that frame; duplicates and missing roots are errors.
`After(d)` arms `now+d` only when that source has no pending request. Re-observing
it before the deadline does not renew it: 100 set at 0 stays due at 100 after an
unrelated frame at 90. A due request is consumed once on a resolved frame at or
after its deadline; a fresh demand can then arm from current time. `OnChange`
clears it and omission from the roster retires it. The separate animation pulse
also does not renew before due and never extends beyond its active segment end.
The next deadline is the minimum of all pending requests.

Use that absolute deadline in the host's existing Context scheduler. A real input
or due request can dirty Context; a settled idle tick must not rebuild/dirty it.
Context still applies its ordinary FPS budget. Do not busy-poll an already-due
Stage request while Context is throttled: wait on Context's pending frame budget,
then resolve once at the current caller time. Stage creates no loop or thread.
A zero external delay is an explicit immediate request, not an internal loop.

RGB composition and half-block realization happen once, then the existing painter
paints semantic UI above the RGB layer for snapshot cells. Opaque UI backgrounds
occlude normally; arbitrary alpha over glyph pixels remains unsupported.
`StageFrame::to_node()` sends ordinary background-plus-semantic Nodes to Context,
preserving focused editor caret/scroll metadata that a flat Surface would lose.
Its pure callbacks replay at the captured frame time. Each emitted frame therefore
has a driver target-layout solve and Context's ordinary render-layout solve; the
snapshot paint and Context paint are also separate. No caching/optimization or
CPU budget is claimed. Arbitrary Node callbacks retain their existing contract.

`ResolvedFrameSnapshot` joins current RGB and final pre-capability cells, semantic
key state, the same-frame keyed geometry, focus, existing paint-derived targets,
graphical poses/clips/z/source dimensions, caller-declared omissions, due source
keys and the next absolute deadline. Implicit keys lack explicit locators;
children pruned by ancestor clipping have no fabricated painted region. Existing
`fx` disclosures remain pre-effect bounds. The viewport and pointer targets are
logical, not certification of physical inline origin. Optional `FrameReport` is
attached only AFTER successful Context submission and describes Context's own
generation/write, not total Stage/producer cost or terminal presentation time.

The synthetic tests retain action identity and a single UiRuntime. They do not
clear the consumer integration, physical resize/pointer, private-art appearance,
performance or fresh-author gates.
