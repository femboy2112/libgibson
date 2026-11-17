//! Resolved RGB composition. All signed placement is in raster pixels.
use crate::raster::{composite_over, RgbRaster, RgbaRaster, MAX_RASTER_DIMENSION};
use crate::ui::{Compiled, Key};
use crate::{layout, painter, Node, Rect, Surface};
use std::collections::BTreeMap;
use std::sync::Arc;

/// An anchor within the exact frame supplied for resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RasterAnchor {
    /// Explicit semantic identity, resolved using the same Compiled frame's join.
    Key(Key),
    /// Low-level unstable Node path. Re-resolve for every frame and structural edit.
    /// This is not semantic identity and does not follow an element after reorder.
    FramePath(Vec<usize>),
}

/// An immutable source anchored within the supplied layout.
/// Key identity remains with UiRuntime; low-level FramePath anchors are unstable.
#[derive(Debug, Clone)]
pub struct RasterPlane {
    /// Semantic Key, or an explicit frame-local low-level escape hatch.
    pub anchor: RasterAnchor,
    /// Straight-alpha source pixels. No scaling, re-cropping or cache is implicit.
    pub source: Arc<RgbaRaster>,
    /// Signed raster-pixel offset AFTER layout (one cell = 1x2 pixels).
    pub offset_px: (i32, i32),
    /// Higher paints later; equal values preserve input order.
    pub z: i32,
}

/// Aggregate admission limits. Repeated Arc sources count per plane, even offscreen.
#[derive(Debug, Clone, Copy)]
pub struct RasterLimits {
    /// Maximum number of planes.
    pub planes: usize,
    /// Maximum number of nodes inspected/cloned for layout.
    pub nodes: usize,
    /// Canvas pixels plus twice each source's pixels (source + scratch allowance).
    /// This bounds pixel storage/work for composition, not arbitrary Node callbacks.
    pub pixels: u64,
}

/// Explicit refusal: no partially composed result is returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RasterError {
    /// Zero or unsupported viewport, or underlay dimensions differ from 1x2 mapping.
    Viewport,
    /// Aggregate plane/node/pixel limit exceeded.
    Budget,
    /// Plane index has an invalid child-index path.
    MissingAnchor(usize),
    /// Existing Node layout rejected the tree.
    Layout(String),
    /// A signed pixel origin cannot be represented without overflow.
    Coordinate(usize),
}
impl std::fmt::Display for RasterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "raster resolution refused: {self:?}")
    }
}
impl std::error::Error for RasterError {}

/// One plane's source-to-painted mapping, before cell quantization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterPlacement {
    /// Input plane index (receipts are returned in paint order).
    pub plane: usize,
    /// Taffy-resolved allocation, before presentation offset.
    pub layout_rect: Rect,
    /// Viewport-relative signed raster origin after presentation offset.
    pub origin_px: (i32, i32),
    /// Actual painted half-open raster rectangle; empty means fully clipped.
    pub clip_px: Rect,
    /// Resolved layer ordering value.
    pub z: i32,
}

/// A single resolution, with no retained cache or terminal ownership.
#[derive(Debug, Clone)]
pub struct ResolvedRaster {
    /// Clone of the supplied tree after this call's layout. No callbacks are painted.
    pub layout: Node,
    /// Final RGB plane, suitable for pixel inspection.
    pub pixels: RgbRaster,
    /// Exactly one half-block realization of the final RGB plane.
    pub surface: Surface,
    /// Plane geometry in actual paint order, including fully clipped planes.
    pub placements: Vec<RasterPlacement>,
}

/// Resolves layout, then composites RGBA planes over the actual opaque RGB underlay.
///
/// Viewport is in cells; underlay must be exactly `(width, 2*height)` pixels.
/// Anchor allocations and ancestor clips use existing Node/painter geometry.
/// Presentation offsets move the allocation after layout, leaving flow siblings
/// reserved in their original places. Source pixels are cropped to that moved
/// allocation and its ancestor clip; resize never scales the source.
///
/// Only RGB underlays are supported. Arbitrary text/Surface underlays are refused
/// by the type signature: there is no glyph-to-RGB inference or terminal alpha.
/// Paint semantic text ABOVE this plane using the existing painter/runtime.
/// This function adds no hit targets, input route, timer, cache or terminal owner.
/// Key anchors require [`resolve_compiled_raster`]; this raw-Node entry accepts
/// only frame-local paths. Limits and all anchor paths are checked before layout; caller callbacks remain
/// subject to the existing Node layout contract. Empty viewports are refused.
pub fn resolve_raster(
    root: &Node,
    viewport: (u16, u16),
    underlay: &RgbRaster,
    planes: &[RasterPlane],
    limits: RasterLimits,
) -> Result<ResolvedRaster, RasterError> {
    resolve_impl(root, None, viewport, underlay, planes, limits)
}

/// Resolves semantic Key anchors through this same frame's lowering geometry join.
///
/// Use before structurally editing `compiled.node`; such edits invalidate its
/// frame-local join. No previous-frame mapping or external identity table is used.
/// Noninteractive explicit keys are supported, including chrome/overlay wrappers.
pub fn resolve_compiled_raster<A>(
    compiled: &Compiled<A>,
    viewport: (u16, u16),
    underlay: &RgbRaster,
    planes: &[RasterPlane],
    limits: RasterLimits,
) -> Result<ResolvedRaster, RasterError> {
    resolve_impl(
        &compiled.node,
        Some(&compiled.geometry_paths),
        viewport,
        underlay,
        planes,
        limits,
    )
}

fn resolve_impl(
    root: &Node,
    keys: Option<&BTreeMap<Key, Vec<usize>>>,
    viewport: (u16, u16),
    underlay: &RgbRaster,
    planes: &[RasterPlane],
    limits: RasterLimits,
) -> Result<ResolvedRaster, RasterError> {
    let path_for = |i: usize| -> Result<&[usize], RasterError> {
        match &planes[i].anchor {
            RasterAnchor::FramePath(path) => Ok(path),
            RasterAnchor::Key(key) => keys
                .and_then(|keys| keys.get(key))
                .map(Vec::as_slice)
                .ok_or(RasterError::MissingAnchor(i)),
        }
    };
    let (w, h) = viewport;
    let ph = u32::from(h) * 2;
    if w == 0
        || h == 0
        || w > MAX_RASTER_DIMENSION
        || ph > u32::from(MAX_RASTER_DIMENSION)
        || underlay.width() != w
        || u32::from(underlay.height()) != ph
    {
        return Err(RasterError::Viewport);
    }
    if planes.len() > limits.planes {
        return Err(RasterError::Budget);
    }
    let mut charge = u64::from(w) * u64::from(ph);
    for (i, plane) in planes.iter().enumerate() {
        charge = charge
            .checked_add(2 * u64::from(plane.source.width()) * u64::from(plane.source.height()))
            .ok_or(RasterError::Budget)?;
        if charge > limits.pixels {
            return Err(RasterError::Budget);
        }
        let mut anchor = root;
        for &index in path_for(i)? {
            anchor = anchor
                .children
                .get(index)
                .ok_or(RasterError::MissingAnchor(i))?;
        }
    }
    if charge > limits.pixels {
        return Err(RasterError::Budget);
    }
    let mut stack = vec![root];
    let mut count = 0usize;
    while let Some(node) = stack.pop() {
        count += 1;
        if count > limits.nodes {
            return Err(RasterError::Budget);
        }
        // Bound the traversal stack too, before appending a wide child list.
        if node.children.len() > limits.nodes - count {
            return Err(RasterError::Budget);
        }
        stack.extend(&node.children);
    }
    let mut resolved = root.clone();
    layout::compute_layout(&mut resolved, w, h).map_err(RasterError::Layout)?;
    let area = Rect::new(0, 0, w, h);
    let mut placements = Vec::with_capacity(planes.len());
    for (i, plane) in planes.iter().enumerate() {
        let mut anchor = &resolved;
        let mut parent_clip = area;
        let (mut ox, mut oy) = (0, 0);
        for &index in path_for(i)? {
            let visible = painter::visible_rect(anchor, parent_clip, ox, oy);
            parent_clip = painter::child_clip_rect(anchor, visible);
            let child = &anchor.children[index];
            (ox, oy) = painter::child_origin(anchor, child, ox, oy);
            anchor = child;
        }
        let r = anchor.computed_rect;
        let x = (i64::from(r.x) + i64::from(ox)) + i64::from(plane.offset_px.0);
        let y = 2 * (i64::from(r.y) + i64::from(oy)) + i64::from(plane.offset_px.1);
        let origin = (
            i32::try_from(x).map_err(|_| RasterError::Coordinate(i))?,
            i32::try_from(y).map_err(|_| RasterError::Coordinate(i))?,
        );
        let x0 = x.max(i64::from(parent_clip.x));
        let y0 = y.max(2 * i64::from(parent_clip.y));
        let x1 = (x + i64::from(r.width.min(plane.source.width())))
            .min(i64::from(parent_clip.x) + i64::from(parent_clip.width));
        let y1 = (y + (2 * i64::from(r.height)).min(i64::from(plane.source.height())))
            .min(2 * (i64::from(parent_clip.y) + i64::from(parent_clip.height)));
        let clip = if x1 <= x0 || y1 <= y0 {
            Rect::default()
        } else {
            Rect::new(x0 as u16, y0 as u16, (x1 - x0) as u16, (y1 - y0) as u16)
        };
        placements.push(RasterPlacement {
            plane: i,
            layout_rect: r,
            origin_px: origin,
            clip_px: clip,
            z: plane.z,
        });
    }
    placements.sort_by_key(|p| p.z);
    let mut pixels = underlay.clone();
    for placement in &placements {
        let clip = placement.clip_px;
        if clip.width == 0 || clip.height == 0 {
            continue;
        }
        let mut scratch = RgbRaster::new(clip.width, clip.height);
        for y in 0..clip.height {
            for x in 0..clip.width {
                scratch.set(
                    i32::from(x),
                    i32::from(y),
                    pixels
                        .get(i32::from(clip.x + x), i32::from(clip.y + y))
                        .unwrap(),
                );
            }
        }
        // Overlap ensures the relative origin lies within bounded source dimensions.
        composite_over(
            &mut scratch,
            &planes[placement.plane].source,
            (i64::from(placement.origin_px.0) - i64::from(clip.x)) as i32,
            (i64::from(placement.origin_px.1) - i64::from(clip.y)) as i32,
        );
        for y in 0..clip.height {
            for x in 0..clip.width {
                pixels.set(
                    i32::from(clip.x + x),
                    i32::from(clip.y + y),
                    scratch.get(i32::from(x), i32::from(y)).unwrap(),
                );
            }
        }
    }
    let surface = pixels.to_surface();
    Ok(ResolvedRaster {
        layout: resolved,
        pixels,
        surface,
        placements,
    })
}
