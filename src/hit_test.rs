//! Spatial hit testing and pointer capture over layered layout rectangles.
//!
//! Given a stack of layers of rectangular targets, this module answers one
//! question: which target (if any) receives the pointer at a terminal cell? It is
//! deliberately domain-agnostic — a target is just a caller-owned `TargetId`, a
//! [`Rect`] and a z value — so any instrument or widget tree can feed it.
//!
//! Three rules carry the weight:
//!
//! * **Z-order.** Layers are consulted top-first. Inside a layer the highest `z`
//!   wins; on equal `z` the target added **last** wins (painter's order: later
//!   draws on top).
//! * **Modal occlusion.** A modal layer is the last layer ever consulted. A point
//!   that misses every target of a modal layer yields `None`; it never falls
//!   through to the layers beneath.
//! * **Pointer capture.** `PointerCapture` pins the pointer to one target
//!   regardless of position (a drag). Capture is *explicitly* cancelled when the
//!   target disappears, the surface resizes, or focus is lost — a stale capture
//!   would otherwise route input to a ghost.
//!
//! `HitLayers::hit_test_world` bridges to [`WorldView`](crate::world_view::WorldView):
//! a clipped world point is a miss, never a bogus `(0, 0)` hit.
//!
//! Everything here is a pure function of its arguments: no clock, no global
//! state, no panics on any input.

use crate::world_view::{Transport, WorldView};
use crate::Rect;

/// An opaque, caller-assigned identity for a hit target.
///
/// This module never interprets the value; it only compares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TargetId(pub u64);

/// One hit target: an identity, the cells it covers, and its z within its layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Target {
    id: TargetId,
    rect: Rect,
    z: i32,
}

/// One layer of targets; a modal layer blocks everything beneath it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Layer {
    modal: bool,
    targets: Vec<Target>,
}

impl Layer {
    /// The winning target at `(x, y)` in this layer: highest `z`, later insertion
    /// on ties. `max_by_key` returns the last maximal element, which is exactly
    /// the "later insertion wins" rule.
    fn hit(&self, x: u16, y: u16) -> Option<TargetId> {
        self.targets
            .iter()
            .filter(|t| t.rect.contains(x, y))
            .max_by_key(|t| t.z)
            .map(|t| t.id)
    }
}

/// An ordered stack of target layers, bottom to top.
///
/// Build with [`push_layer`](HitLayers::push_layer) and
/// [`add_target`](HitLayers::add_target); query with
/// [`hit_test`](HitLayers::hit_test).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HitLayers {
    layers: Vec<Layer>,
}

impl HitLayers {
    /// An empty stack: every query is a miss.
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes a new, empty top layer. When `modal` is `true` the layer occludes
    /// everything beneath it (no click-through).
    pub fn push_layer(&mut self, modal: bool) {
        self.layers.push(Layer {
            modal,
            targets: Vec::new(),
        });
    }

    /// Adds a target to the current top layer. If no layer exists yet, a
    /// non-modal base layer is opened first so the target is never dropped.
    pub fn add_target(&mut self, id: TargetId, rect: Rect, z: i32) {
        if self.layers.is_empty() {
            self.push_layer(false);
        }
        if let Some(top) = self.layers.last_mut() {
            top.targets.push(Target { id, rect, z });
        }
    }

    /// `true` if any layer holds a target with this id.
    pub fn contains(&self, id: TargetId) -> bool {
        self.layers
            .iter()
            .any(|layer| layer.targets.iter().any(|t| t.id == id))
    }

    /// The target that receives the pointer at cell `(x, y)`, or `None`.
    ///
    /// Layers are tested top-first. A layer's winner is its highest-`z` target
    /// containing the cell (ties: the later-added target). Rect coverage is
    /// half-open — the near edge is inside, `x + width` / `y + height` are
    /// outside — exactly as [`Rect::contains`]. A **modal** layer ends the search:
    /// if it has no target at the cell the result is `None`, even when a layer
    /// beneath would have matched.
    pub fn hit_test(&self, x: u16, y: u16) -> Option<TargetId> {
        for layer in self.layers.iter().rev() {
            if let Some(id) = layer.hit(x, y) {
                return Some(id);
            }
            if layer.modal {
                return None;
            }
        }
        None
    }

    /// Hit-tests a continuous world point through `view`.
    ///
    /// The point is mapped with [`WorldView::world_to_cell`]. A clipped transport
    /// (out of view, non-finite, out of range) carries no cell and is a miss —
    /// it is never coerced into cell `(0, 0)`.
    pub fn hit_test_world(&self, view: &WorldView, wx: f64, wy: f64) -> Option<TargetId> {
        match view.world_to_cell(wx, wy) {
            Transport::Mapped { value, .. } => self.hit_test(value.0, value.1),
            Transport::Clipped(_) => None,
        }
    }
}

/// Which target, if any, currently holds the pointer (e.g. mid-drag).
///
/// While a target is captured, [`route`](PointerCapture::route) sends every
/// pointer position to it. Capture never expires on its own: the caller must end
/// it with [`release`](PointerCapture::release) or one of the `cancel_*` methods.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PointerCapture {
    held: Option<TargetId>,
}

impl PointerCapture {
    /// A capture state holding nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Captures the pointer for `target`, replacing any earlier capture.
    pub fn capture(&mut self, target: TargetId) {
        self.held = Some(target);
    }

    /// Releases the capture, if any.
    pub fn release(&mut self) {
        self.held = None;
    }

    /// The currently captured target, if any.
    pub fn captured(&self) -> Option<TargetId> {
        self.held
    }

    /// Routes a pointer at cell `(x, y)`: the captured target if capture is held
    /// (regardless of position), otherwise [`HitLayers::hit_test`].
    pub fn route(&self, layers: &HitLayers, x: u16, y: u16) -> Option<TargetId> {
        self.held.or_else(|| layers.hit_test(x, y))
    }

    /// Releases the capture if the captured target is no longer in `layers`
    /// (widget removal). A capture on a still-present target is left alone.
    pub fn cancel_if_removed(&mut self, layers: &HitLayers) {
        if let Some(id) = self.held {
            if !layers.contains(id) {
                self.held = None;
            }
        }
    }

    /// Unconditionally releases the capture: a resize invalidates the geometry an
    /// in-flight drag was anchored to.
    pub fn cancel_on_resize(&mut self) {
        self.held = None;
    }

    /// Unconditionally releases the capture: losing focus ends any in-flight
    /// drag, since the matching release event may never arrive.
    pub fn cancel_on_focus_loss(&mut self) {
        self.held = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: TargetId = TargetId(1);
    const B: TargetId = TargetId(2);
    const C: TargetId = TargetId(3);

    fn single(id: TargetId, rect: Rect) -> HitLayers {
        let mut l = HitLayers::new();
        l.add_target(id, rect, 0);
        l
    }

    #[test]
    fn higher_z_wins_when_rects_overlap() {
        // Predicted: A (z 0) and B (z 5) overlap at (2,2); B wins. B is added
        // FIRST so a "last inserted wins" implementation would wrongly pick A.
        let mut l = HitLayers::new();
        l.add_target(B, Rect::new(0, 0, 4, 4), 5);
        l.add_target(A, Rect::new(0, 0, 4, 4), 0);
        assert_eq!(l.hit_test(2, 2), Some(B));
    }

    #[test]
    fn equal_z_later_insertion_wins() {
        // Predicted: tie on z -> the later-added target (B) wins.
        let mut l = HitLayers::new();
        l.add_target(A, Rect::new(0, 0, 4, 4), 1);
        l.add_target(B, Rect::new(0, 0, 4, 4), 1);
        assert_eq!(l.hit_test(1, 1), Some(B));
    }

    #[test]
    fn point_in_empty_space_is_none() {
        // Predicted: (9,9) is outside the only rect -> None. An empty stack too.
        let l = single(A, Rect::new(0, 0, 4, 4));
        assert_eq!(l.hit_test(9, 9), None);
        assert_eq!(HitLayers::new().hit_test(0, 0), None);
    }

    #[test]
    fn rect_edges_are_half_open() {
        // Predicted: rect (2,3) 4x2 covers x in [2,6), y in [3,5).
        let l = single(A, Rect::new(2, 3, 4, 2));
        assert_eq!(l.hit_test(2, 3), Some(A)); // near corner: inside
        assert_eq!(l.hit_test(5, 4), Some(A)); // last covered cell: inside
        assert_eq!(l.hit_test(6, 4), None); // x + width: outside
        assert_eq!(l.hit_test(5, 5), None); // y + height: outside
        assert_eq!(l.hit_test(1, 3), None); // just before near edge: outside
        let empty = single(A, Rect::new(2, 3, 0, 5));
        // Predicted: a zero-width rect covers nothing.
        assert_eq!(empty.hit_test(2, 3), None);
    }

    fn base_with_a() -> HitLayers {
        let mut l = HitLayers::new();
        l.push_layer(false);
        l.add_target(A, Rect::new(0, 0, 10, 10), 0);
        l
    }

    #[test]
    fn modal_layer_blocks_click_through() {
        // Predicted: A fills 10x10 beneath; a modal layer holds only B at
        // (20,20). Cell (3,3) is over A but misses B -> None, NOT Some(A).
        let mut l = base_with_a();
        l.push_layer(true);
        l.add_target(B, Rect::new(20, 20, 4, 4), 0);
        assert_eq!(l.hit_test(3, 3), None);
        // The modal layer's own target is still reachable.
        assert_eq!(l.hit_test(21, 21), Some(B));
    }

    #[test]
    fn nonmodal_top_layer_lets_lower_target_through() {
        // MUTATION CONTROL for the test above: identical geometry, top layer
        // NON-modal. Predicted: (3,3) falls through to A. If this returned None
        // the modal test would be vacuous (hit_test just broken).
        let mut l = base_with_a();
        l.push_layer(false);
        l.add_target(B, Rect::new(20, 20, 4, 4), 0);
        assert_eq!(l.hit_test(3, 3), Some(A));
    }

    #[test]
    fn modal_only_blocks_layers_beneath_it() {
        // Predicted: layers bottom->top = [A], [modal B], [C non-modal].
        // C's cell hits C; a cell over B hits B; a cell over only A is blocked
        // by the modal B layer -> None (modal stops the walk below itself).
        let mut l = HitLayers::new();
        l.push_layer(false);
        l.add_target(A, Rect::new(0, 0, 30, 30), 0);
        l.push_layer(true);
        l.add_target(B, Rect::new(10, 10, 5, 5), 0);
        l.push_layer(false);
        l.add_target(C, Rect::new(12, 12, 1, 1), 0);
        assert_eq!(l.hit_test(12, 12), Some(C));
        assert_eq!(l.hit_test(10, 10), Some(B));
        assert_eq!(l.hit_test(0, 0), None);
    }

    #[test]
    fn capture_routes_out_of_bounds_point_to_captured_target() {
        // Predicted: with B captured, a point far from every rect routes to B;
        // after release the same point is a plain miss (None).
        let mut l = HitLayers::new();
        l.add_target(A, Rect::new(0, 0, 4, 4), 0);
        l.add_target(B, Rect::new(10, 10, 2, 2), 0);
        let mut cap = PointerCapture::new();
        assert_eq!(cap.route(&l, 50, 50), None); // nothing held: plain hit_test
        cap.capture(B);
        assert_eq!(cap.captured(), Some(B));
        assert_eq!(cap.route(&l, 50, 50), Some(B));
        // Capture beats position even over a different target.
        assert_eq!(cap.route(&l, 1, 1), Some(B));
        cap.release();
        assert_eq!(cap.captured(), None);
        assert_eq!(cap.route(&l, 50, 50), None);
        assert_eq!(cap.route(&l, 1, 1), Some(A));
    }

    #[test]
    fn cancel_if_removed_releases_only_when_target_is_gone() {
        // Predicted: A present -> capture kept. Rebuild layers without A ->
        // capture released. Both directions pinned, so "always release" and
        // "never release" implementations each fail one half.
        let mut present = HitLayers::new();
        present.add_target(A, Rect::new(0, 0, 4, 4), 0);
        let mut cap = PointerCapture::new();
        cap.capture(A);
        cap.cancel_if_removed(&present);
        assert_eq!(cap.captured(), Some(A));

        let mut gone = HitLayers::new();
        gone.add_target(B, Rect::new(0, 0, 4, 4), 0);
        cap.cancel_if_removed(&gone);
        assert_eq!(cap.captured(), None);

        // No capture held: stays empty, no panic.
        cap.cancel_if_removed(&gone);
        assert_eq!(cap.captured(), None);
    }

    #[test]
    fn target_in_any_layer_counts_as_present() {
        // Predicted: a captured target living in a lower layer is still present.
        let mut l = base_with_a();
        l.push_layer(true);
        l.add_target(B, Rect::new(20, 20, 2, 2), 0);
        let mut cap = PointerCapture::new();
        cap.capture(A);
        cap.cancel_if_removed(&l);
        assert_eq!(cap.captured(), Some(A));
    }

    #[test]
    fn resize_and_focus_loss_cancel_capture() {
        // Predicted: each cancel clears an active capture, and after it route()
        // falls back to hit_test (miss at (50,50)) instead of the ghost target.
        let l = single(A, Rect::new(0, 0, 4, 4));
        let mut cap = PointerCapture::new();
        cap.capture(A);
        cap.cancel_on_resize();
        assert_eq!(cap.captured(), None);
        assert_eq!(cap.route(&l, 50, 50), None);

        cap.capture(A);
        cap.cancel_on_focus_loss();
        assert_eq!(cap.captured(), None);
        assert_eq!(cap.route(&l, 50, 50), None);
    }

    fn reference_view() -> WorldView {
        // Same reference as world_view tests: 10x10 anchor, zoom 2, ppc 1x2.
        // World (1.3, 1.3) -> cell (2, 1).
        WorldView::new(Rect::new(0, 0, 10, 10))
            .with_zoom(2.0, 2.0)
            .with_pixels_per_cell(1, 2)
    }

    #[test]
    fn world_point_in_view_hits_target_over_its_cell() {
        // Predicted: world (1.3,1.3) -> cell (2,1), which only A covers; B sits
        // elsewhere. Hit A. A neighbouring in-view world point over no target
        // (world (8,8) -> cell (8,4)) misses.
        let mut l = HitLayers::new();
        l.add_target(A, Rect::new(2, 1, 1, 1), 0);
        l.add_target(B, Rect::new(5, 3, 1, 1), 0);
        let v = reference_view();
        assert_eq!(l.hit_test_world(&v, 1.3, 1.3), Some(A));
        assert_eq!(l.hit_test_world(&v, 8.0, 8.0), None);
    }

    #[test]
    fn clipped_world_point_is_a_miss_not_a_cell_zero_hit() {
        // Predicted: A covers the WHOLE anchor including cell (0,0). A clipped
        // world point must still be None; a bridge that coerced Clipped to
        // (0,0) would return Some(A) here.
        let l = single(A, Rect::new(0, 0, 10, 10));
        let v = reference_view();
        assert_eq!(l.hit_test(0, 0), Some(A)); // control: (0,0) really is a hit
        assert_eq!(l.hit_test_world(&v, 100.0, 100.0), None); // OutOfView
        assert_eq!(l.hit_test_world(&v, -1.0, -1.0), None); // OutOfView
        assert_eq!(l.hit_test_world(&v, f64::NAN, 0.0), None); // NonFinite
    }

    #[test]
    fn world_hit_honors_modal_occlusion() {
        // Predicted: world (1.3,1.3) -> cell (2,1) is over A, but a modal layer
        // that misses that cell blocks it -> None through the world bridge too.
        let mut l = base_with_a();
        l.push_layer(true);
        l.add_target(B, Rect::new(8, 8, 1, 1), 0);
        assert_eq!(l.hit_test_world(&reference_view(), 1.3, 1.3), None);
    }
}
