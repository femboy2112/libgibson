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
retain its receipt and demand. Keep its existing navigation state and route keys
to that grammar only when the host's modal/editor routing leaves them unconsumed.
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

Each plane selects a Node child-index path in the current supplied tree. An empty
path selects the root. Paths describe this layout, not persistent identity; use
UiRuntime's keys for semantic identity. Presentation offsets apply after layout:
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
let plane = RasterPlane { path: vec![0], source: Arc::new(pixels),
    offset_px: (-1, 1), z: 0 };
let frame = resolve_raster(&tree, (8, 2), &underlay, &[plane],
    RasterLimits { planes: 8, nodes: 64, pixels: 4096 })?;
assert_eq!(frame.surface.width, 8);
# Ok::<(), RasterError>(())
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

No caches, input method, terminal ownership, animation clock or demand ledger are
added. The next seam supplies timing and joined inspection. These synthetic RGB
receipts do not clear physical inline resize debt or establish performance or
appearance on a consumer's private artwork.
