//! Experimental semantic composition helpers over the existing UI runtime.
//!
//! Mount semantic subtrees as ordinary [`Element`](crate::ui::Element) children,
//! before compilation.
//! One [`crate::ui::UiRuntime`] then reconciles all keys, targets and modal focus.
//! This module owns no input dispatcher, terminal session or application state.
//! Resolved raster composition uses layout-derived anchors and signed pixel offsets.
//! Presentation timing uses explicit caller time and absolute non-renewing demands.

use crate::ui::Element;
use crate::{Event, TextInputState};
use std::sync::Arc;

/// Adapts every action channel in a semantic subtree to the host's action type.
///
/// Keys, layout, input snapshots, selection, revisions and presentation intent
/// are preserved. Press, edit, custom-event and modal-dismissal actions all pass
/// through `map`; the mapper is shared by the tree and its deferred callbacks.
/// Existing `()` actions remain unit actions: this cannot recover distinctions
/// erased by a producer or reconstruct controls from an already painted raster.
///
/// Add the result to the host's ordinary semantic tree, then compile that whole
/// tree once. Explicit keys must be globally unique; duplicates are rejected by
/// the existing runtime, never renamed. Unkeyed positional paths still depend
/// on structure. Input routing stays with the host's `UiRuntime` and `Context`.
///
/// **Screen padding and density of a mounted producer.** A mounted subtree is
/// laid out inside the host's one [`crate::ui::UiRuntime`], so a nested `screen()`
/// resolves its skin padding and density from the host's global
/// [`UiEnvironment`](crate::ui::UiEnvironment), not from the smaller area the
/// producer was budgeted for. A producer that fits its local allocation (for
/// example 16 rows of an 80x24 host) can therefore overflow it, because its root
/// Screen adds the host skin's padding on top, and a wrapper height alone does
/// not remove that padding. To budget it, clear the producer-root padding:
/// `map_actions(presented.element, Host::Inner).padding(0)`; height stays automatic.
/// The pinned evidence is the integration test
/// `blades_mount_records_inherited_padding_overflow_and_explicit_zero_padding` in
/// `tests/stage_integrated_fixture.rs`, which records inherited padding (Screen
/// height 20, action clipped) against explicit zero padding (height 16, visible).
/// See `docs/STAGE.md`.
///
/// ```
/// use gibson::stage::map_actions;
/// use gibson::ui::prelude::*;
/// use std::time::Duration;
/// #[derive(Clone, Debug, PartialEq)]
/// enum Host { Inspect, Cancel }
/// let inspector = button("Inspect").key("inspect").on_press(());
/// let tree = column()
///     .child(map_actions(inspector, |()| Host::Inspect))
///     .child(button("Cancel").key("cancel").on_press(Host::Cancel));
/// let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
/// let frame = ui.frame(&tree, UiEnvironment::default(), Duration::ZERO)?;
/// assert!(frame.keys.contains_key(&Key::named("inspect")));
/// # Ok::<(), gibson::ui::UiError>(())
/// ```
pub fn map_actions<A: 'static, B: 'static>(
    element: Element<A>,
    map: impl Fn(A) -> B + 'static,
) -> Element<B> {
    map_tree(element, Arc::new(map))
}

fn map_tree<A: 'static, B: 'static>(element: Element<A>, map: Arc<dyn Fn(A) -> B>) -> Element<B> {
    Element {
        kind: element.kind,
        children: element
            .children
            .into_iter()
            .map(|child| map_tree(child, Arc::clone(&map)))
            .collect(),
        overlays: element
            .overlays
            .into_iter()
            .map(|child| map_tree(child, Arc::clone(&map)))
            .collect(),
        key: element.key,
        on_press: element.on_press.map(|action| map(action)),
        on_edit: element.on_edit.map(|handler| {
            let map = Arc::clone(&map);
            let mapped: Arc<dyn Fn(TextInputState) -> B> =
                Arc::new(move |state| map(handler(state)));
            mapped
        }),
        on_event: element.on_event.map(|handler| {
            let map = Arc::clone(&map);
            let mapped: Arc<dyn Fn(Event) -> B> = Arc::new(move |event| map(handler(event)));
            mapped
        }),
        on_dismiss: element.on_dismiss.map(|action| map(action)),
        selected: element.selected,
        disabled: element.disabled,
        revision: element.revision,
        motion: element.motion,
        tone: element.tone,
        emphasis: element.emphasis,
        density: element.density,
        elevation: element.elevation,
        number: element.number,
        layout: element.layout,
        effects: element.effects,
        source: element.source,
    }
}

mod raster_plane;
pub use raster_plane::{
    resolve_compiled_raster, resolve_raster, RasterAnchor, RasterError, RasterLimits,
    RasterPlacement, RasterPlane, ResolvedRaster,
};

mod driver;
pub use driver::{
    FrameParts, KeySnapshot, PixelRect, PlaneSnapshot, PresentationDriver, PresentationOptions,
    ResolvedFrameSnapshot, SourceDemand, StageError, StageFrame,
};
