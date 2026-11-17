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

This first seam supplies action adaptation and a compile-once mounting recipe.
It adds no separate mount wrapper, focus registry, input dispatcher, terminal
owner or cache. Resolved raster planes, layout-derived presentation offsets,
timing/demand aggregation and joined snapshots belong to subsequent seams and
are not implemented here. No physical inline-pointer or aesthetic guarantee is
implied by the headless semantic checks.
