//! Derive spatial hit targets from the *painted* layout (issue #127 / Law L2).
//!
//! The spatial substrate (`crate::hit_test`) answers "which target is at this
//! cell?" — but it is fed caller-supplied rectangles, which proves nothing about
//! whether the *actually painted* UI agrees with pointer targeting. This module
//! closes that gap. It walks the laid-out node tree with the **exact** geometry
//! the painter uses (the shared `visible_rect` / `child_clip_rect` /
//! `child_origin` helpers in `crate::painter`), so every target's rectangle is
//! the one the surface really painted — same clipping, offsets, z-order and modal
//! occlusion. The painter and the hit map cannot disagree, because they read the
//! same geometry functions; a `tests/ui_hit_paint_unification.rs` fixture pins it
//! against the painted `Surface` itself.
//!
//! Precondition: `crate::layout::compute_layout` has run on `Compiled::node`.
//! Nothing here mutates anything (Law L1) — it is a pure read of resolved layout.

use super::compile::{Compiled, TargetSpec};
use super::element::Key;
use crate::hit_test::{HitLayers, TargetId};
use crate::node::Node;
use crate::painter::{child_clip_rect, child_origin, visible_rect};
use crate::surface::Rect;

/// One node's painted geometry, in paint order.
///
/// `natural` is where the node would sit clipped only to the surface; `visible`
/// adds the parent clip chain. Their difference is exactly how much the node is
/// clipped. Public fields so a test can tamper one and watch the oracle bleed.
#[derive(Debug, Clone)]
pub struct PaintedRegion {
    /// Pre-order paint index; a later `order` paints *above* an earlier one.
    pub order: u32,
    /// Exclusive paint index past this node's whole subtree (for modal scoping).
    pub subtree_end: u32,
    /// Index into [`Compiled::targets`] when this node is an eligible target.
    pub tag: Option<u32>,
    /// The node's rect offset onto the screen, clipped only to the surface.
    pub natural: Rect,
    /// The actually-visible rect: `natural` further clipped by the clip chain.
    pub visible: Rect,
    /// This node (or an ancestor) applies a `SurfaceFx` that may move/erase
    /// pixels, so its targeting is the pre-effect extent — disclosed, not exact.
    pub fx: bool,
}

/// Whether a resolved target can take an action, or merely occludes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eligibility {
    /// A live target: a pointer here acts on it.
    Hittable,
    /// Painted and occluding — it swallows the click rather than letting it fall
    /// through — but disabled, so the action itself is inert.
    Disabled,
}

/// A target resolved against the painted layout: its identity, where it actually
/// is on screen, its z, and whether it is eligible / modal / clipped.
#[derive(Debug, Clone)]
pub struct ResolvedTarget {
    pub id: TargetId,
    pub key: Key,
    /// The visible (painted, clipped) rectangle the target occupies.
    pub rect: Rect,
    /// Paint order = z: higher wins when rects overlap.
    pub order: u32,
    pub modal: bool,
    pub eligibility: Eligibility,
    /// `true` when the visible rect is smaller than the natural rect: the target
    /// is partly or wholly clipped. A partly-clipped target is hit over its
    /// visible cells only; a wholly-clipped one is recorded but unhittable.
    pub clipped: bool,
    /// `true` when this target (or an ancestor) applies a `SurfaceFx` that may
    /// move or erase pixels after layout. Its rect is the *pre-effect* extent, so
    /// an exact-hit guarantee does not hold for affected content: the
    /// qualification is disclosed here, never silently dropped (Law L7).
    pub fx: bool,
}

/// The painted layout resolved into a queryable hit surface: the layered spatial
/// index plus the full resolved-target list (including wholly-clipped targets,
/// which are absent from `layers` but kept here so nothing is silently erased —
/// Law L7).
#[derive(Debug, Clone)]
pub struct HitMap {
    layers: HitLayers,
    targets: Vec<ResolvedTarget>,
}

impl HitMap {
    /// The target that receives a pointer at cell `(x, y)`, honouring z-order and
    /// modal occlusion, or `None` for empty space (or a blocking modal miss).
    pub fn hit(&self, x: u16, y: u16) -> Option<&ResolvedTarget> {
        let id = self.layers.hit_test(x, y)?;
        self.targets.iter().find(|t| t.id == id)
    }

    /// The layered spatial index, ready to drive `PointerCapture` / `DragGesture`.
    pub fn layers(&self) -> &HitLayers {
        &self.layers
    }

    /// Every resolved target, in paint order, including wholly-clipped ones.
    pub fn targets(&self) -> &[ResolvedTarget] {
        &self.targets
    }
}

/// Why a hit map could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HitMapError {
    /// The root node has no resolved area on a non-empty surface: layout almost
    /// certainly has not run. Call `compute_layout` on `Compiled::node` first.
    LayoutNotResolved,
}

impl<A> Compiled<A> {
    /// Walks the painted node tree, recording each node's screen geometry in paint
    /// order. Pure; runs no second layout pass (reads `computed_rect` only).
    pub fn painted_regions(&self, surface_area: Rect) -> Vec<PaintedRegion> {
        let mut out = Vec::new();
        let mut order = 0u32;
        walk(
            &self.node,
            surface_area,
            0,
            0,
            surface_area,
            false,
            &mut order,
            &mut out,
        );
        out
    }

    /// Resolves the painted layout into a [`HitMap`]: a layered spatial index plus
    /// the full target list. See the module docs for the precondition.
    pub fn hit_map(&self, surface_area: Rect) -> Result<HitMap, HitMapError> {
        let regions = self.painted_regions(surface_area);
        // If layout never ran, the root collapses to a zero rect. Refuse rather
        // than hand back a silently-empty map that would answer every query None.
        if !surface_area.is_empty() && regions.first().is_none_or(|r| r.visible.is_empty()) {
            return Err(HitMapError::LayoutNotResolved);
        }
        Ok(self.hit_map_from_regions(regions))
    }

    /// Assembles a [`HitMap`] from an explicit region list (no precondition check).
    /// Exposed so a conformance test can feed *tampered* regions and confirm the
    /// painted-surface oracle actually bleeds — i.e. the pipeline is non-vacuous.
    pub fn hit_map_from_regions(&self, regions: Vec<PaintedRegion>) -> HitMap {
        let mut layers = HitLayers::new();
        layers.push_layer(false);
        let mut targets = Vec::new();
        // While `order < modal_end` we are inside the current modal's subtree, and
        // its targets belong to the open modal layer; once past it we reopen a
        // plain layer so a later overlay floats *above* the modal (see-through for
        // capture) rather than silently joining its blocking layer.
        let mut top_is_modal = false;
        let mut modal_end = u32::MAX;

        for r in &regions {
            let Some(tag) = r.tag else { continue };
            let spec: &TargetSpec = &self.targets[tag as usize];

            if r.order >= modal_end && top_is_modal {
                layers.push_layer(false);
                top_is_modal = false;
                modal_end = u32::MAX;
            }
            if spec.modal {
                // A modal layer ends the hit walk: a cell that misses every target
                // in it yields None instead of falling through to the base. That is
                // exactly global modal capture — no full-surface backdrop target
                // needed (issue #127 open decision 1).
                layers.push_layer(true);
                top_is_modal = true;
                modal_end = r.subtree_end;
            }

            let id = TargetId(tag as u64);
            targets.push(ResolvedTarget {
                id,
                key: spec.key.clone(),
                rect: r.visible,
                order: r.order,
                modal: spec.modal,
                eligibility: if spec.disabled {
                    Eligibility::Disabled
                } else {
                    Eligibility::Hittable
                },
                clipped: r.visible != r.natural,
                fx: r.fx,
            });

            // A wholly-clipped target paints nothing, so it cannot be hit: it stays
            // in `targets` (disclosure) but is left out of the spatial index. A
            // *disabled* target IS indexed — it occludes and swallows the click —
            // but as an INERT target, so the gesture layer refuses to authorize an
            // edit on it: eligibility is carried into the index, not stripped.
            if !r.visible.is_empty() {
                if spec.disabled {
                    layers.add_inert_target(id, r.visible, r.order as i32);
                } else {
                    layers.add_target(id, r.visible, r.order as i32);
                }
            }
        }

        HitMap { layers, targets }
    }
}

/// Pre-order walk reproducing the painter's traversal exactly (same `visible_rect`,
/// `child_clip_rect`, `child_origin`, and the same early-return on an empty rect).
#[allow(clippy::too_many_arguments)]
fn walk(
    node: &Node,
    clip: Rect,
    ox: i32,
    oy: i32,
    surface_area: Rect,
    fx_ancestor: bool,
    order: &mut u32,
    out: &mut Vec<PaintedRegion>,
) {
    let visible = visible_rect(node, clip, ox, oy);
    let natural = visible_rect(node, surface_area, ox, oy);
    let fx = fx_ancestor || !node.surface_fx.is_empty();
    let my_order = *order;
    *order += 1;
    let me = out.len();
    out.push(PaintedRegion {
        order: my_order,
        subtree_end: my_order + 1, // provisional; fixed up after children
        tag: node.tag,
        natural,
        visible,
        fx,
    });

    // The painter early-returns on an empty rect — it paints nothing and descends
    // into no children — so the walk must stop here too, or it would invent
    // targets the surface never drew.
    if visible.is_empty() {
        out[me].subtree_end = *order;
        return;
    }

    let child_clip = child_clip_rect(node, visible);
    for child in &node.children {
        let (cx, cy) = child_origin(node, child, ox, oy);
        walk(child, child_clip, cx, cy, surface_area, fx, order, out);
    }
    out[me].subtree_end = *order;
}
