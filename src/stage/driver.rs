//! Explicit-time presentation and inspection. No terminal, input or render cache.
//!
//! ```
//! use gibson::{Context, RenderMode, raster::{RgbRaster, RgbaRaster}, stage::*,
//!     ui::{prelude::*, experience::FrameDemand}};
//! use std::{sync::Arc, time::Duration};
//! let now = Duration::ZERO;
//! let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
//! let tree = column::<()>().key("root").width(20).height(8)
//!     .child(column().key("art").width(4).height(2));
//! let compiled = ui.frame(&tree, UiEnvironment { width: 20, height: 8,
//!     motion: MotionPreference::None, ..Default::default() }, now)?;
//! let underlay = RgbRaster::new(20, 16);
//! let plane = RasterPlane { anchor: RasterAnchor::Key(Key::named("art")),
//!     source: Arc::new(RgbaRaster::new(4, 4)), offset_px: (0, 0), z: 0 };
//! let mut driver = PresentationDriver::new(PresentationOptions::default())?;
//! let mut frame = driver.resolve_frame(now, FrameParts { compiled,
//!     viewport: (20, 8), underlay: &underlay, planes: &[plane], focus: ui.focus(),
//!     demands: &[SourceDemand { key: Key::named("root"), demand: FrameDemand::OnChange }],
//!     omissions: &[], motion: MotionPreference::None },
//!     RasterLimits { planes: 4, nodes: 64, pixels: 4096 })?;
//! assert_eq!(driver.next_deadline(), None);
//! let mut ctx = Context::headless(RenderMode::Fullscreen, 20, 8);
//! ctx.set_root(frame.to_node());
//! ctx.render_now()?;
//! frame.snapshot.frame_report = Some(ctx.last_frame_report());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
use super::raster_plane::{self, RasterAnchor, RasterError, RasterLimits, RasterPlane};
use crate::raster::RgbRaster;
use crate::ui::experience::FrameDemand;
use crate::ui::{Compiled, ElementState, Key, MotionPreference, PaintedRegion, ResolvedTarget};
use crate::{FrameReport, Node, Rect, Surface};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

/// Viewport-relative raster geometry: one cell is 1x2 pixels. Extent is crop, not scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Finite graphical transitions, with no implicit clock or loop.
#[derive(Debug, Clone, Copy)]
pub struct PresentationOptions {
    pub transition: Duration,
    pub cadence: Duration,
}
impl Default for PresentationOptions {
    fn default() -> Self {
        Self {
            transition: Duration::from_millis(200),
            cadence: Duration::from_millis(16),
        }
    }
}
/// One live producer's demand, named by its explicit semantic root key.
#[derive(Debug, Clone)]
pub struct SourceDemand {
    pub key: Key,
    pub demand: FrameDemand,
}

/// Complete frame-coherent data. Build the whole tree with the host UiRuntime at
/// the same `now`. This consumes and returns its Compiled sidecars, never erases them.
/// A strict reduced host compiles UI/grammars with MotionPreference::None; Stage
/// Reduced/None snaps graphical segments but preserves explicit external timers.
pub struct FrameParts<'a, A> {
    pub compiled: Compiled<A>,
    pub viewport: (u16, u16),
    pub underlay: &'a RgbRaster,
    pub planes: &'a [RasterPlane],
    pub focus: Option<&'a Key>,
    /// Complete live roster: omitted sources retire, OnChange clears a request.
    pub demands: &'a [SourceDemand],
    /// Caller-declared semantic omissions, not a Stage certification.
    pub omissions: &'a [Key],
    pub motion: MotionPreference,
}

/// Refusal without publishing partial timing state or a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageError {
    Raster(RasterError),
    Options,
    BackwardTime,
    TimeOverflow,
    Source(Key),
    DuplicateSource(Key),
    Geometry(Key),
    Focus(Key),
    UnstableAnchor(usize),
    Target(u32),
}
impl From<RasterError> for StageError {
    fn from(e: RasterError) -> Self {
        Self::Raster(e)
    }
}
impl std::fmt::Display for StageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "stage resolution refused: {self:?}")
    }
}
impl std::error::Error for StageError {}

/// Semantic state joined to this frame's locator and exact existing painter walk.
#[derive(Debug, Clone)]
pub struct KeySnapshot {
    pub state: ElementState,
    /// None for implicit semantic paths, which have no explicit-key locator.
    pub layout_rect: Option<Rect>,
    /// None if no locator/target exists or a clipped ancestor pruned this node.
    /// An empty visible rect in Some means the node itself was visited and clipped.
    pub painted: Option<PaintedRegion>,
}
/// Graphical presentation is separate from semantic UI target geometry.
#[derive(Debug, Clone)]
pub struct PlaneSnapshot {
    pub plane: usize,
    pub anchor: RasterAnchor,
    pub layout_rect: Rect,
    pub presented: PixelRect,
    pub clip_px: Rect,
    pub z: i32,
    pub source_size: (u16, u16),
}
/// One resolved frame, before terminal capability lowering. No physical-origin claim.
#[derive(Debug, Clone)]
pub struct ResolvedFrameSnapshot {
    pub now: Duration,
    pub viewport: Rect,
    pub motion: MotionPreference,
    pub keys: BTreeMap<Key, KeySnapshot>,
    pub focus: Option<Key>,
    pub targets: Vec<ResolvedTarget>,
    pub planes: Vec<PlaneSnapshot>,
    pub omissions: Vec<Key>,
    pub due_sources: Vec<Key>,
    pub next_deadline: Option<Duration>,
    pub pixels: RgbRaster,
    pub cells: Surface,
    pub cursor_position: Option<(u16, u16)>,
    pub text_scroll_offset: Option<usize>,
    /// Attach only after successful Context submission. This is that Context's
    /// accounting, not Stage generation cost, display time or proof of physical origin.
    pub frame_report: Option<FrameReport>,
}
/// The same action sidecars and a snapshot; frame products are not cached by the driver.
pub struct StageFrame<A> {
    pub compiled: Compiled<A>,
    pub snapshot: ResolvedFrameSnapshot,
    raster_cells: Arc<Surface>,
}
impl<A> StageFrame<A> {
    /// Ordinary Node composition for the existing Context. Keep the semantic
    /// subtree so focused editor cursor/scroll metadata survives emission; a flat
    /// Surface alone cannot carry it. Context remains the only terminal owner.
    /// Pure callbacks replay at this frame's captured time. Context performs its
    /// normal render/layout; no second Stage loop or persistent output cache exists.
    pub fn to_node(&self) -> Node {
        Node::stack()
            .width(self.snapshot.viewport.width as f32)
            .height(self.snapshot.viewport.height as f32)
            .child(Node::surface(Arc::clone(&self.raster_cells)))
            .child(self.compiled.node.clone())
    }
}

#[derive(Debug, Clone, Copy)]
struct Segment {
    from: PixelRect,
    to: PixelRect,
    start: Duration,
    end: Duration,
}
impl Segment {
    fn snapped(rect: PixelRect, now: Duration) -> Self {
        Self {
            from: rect,
            to: rect,
            start: now,
            end: now,
        }
    }
    fn active(&self, now: Duration) -> bool {
        now < self.end && self.from != self.to
    }
    fn sample(&self, now: Duration) -> PixelRect {
        if now >= self.end {
            return self.to;
        }
        if now <= self.start {
            return self.from;
        }
        let e = (now - self.start).as_nanos() as i128;
        let span = (self.end - self.start).as_nanos() as i128;
        let lerp = |a: i128, b: i128| {
            let n = (b - a) * e;
            a + if n >= 0 {
                (n + span / 2) / span
            } else {
                (n - span / 2) / span
            }
        };
        PixelRect {
            x: lerp(self.from.x.into(), self.to.x.into()) as i32,
            y: lerp(self.from.y.into(), self.to.y.into()) as i32,
            width: lerp(self.from.width.into(), self.to.width.into()) as u32,
            height: lerp(self.from.height.into(), self.to.height.into()) as u32,
        }
    }
}

/// Temporal metadata only: no Nodes, pixels, callbacks, terminal or input owner.
/// Resolve only on a real update or a due deadline using the existing Context
/// scheduler. Idle resolution is not an invalidation or cache API.
#[derive(Debug, Clone)]
pub struct PresentationDriver {
    options: PresentationOptions,
    last_now: Option<Duration>,
    tracks: BTreeMap<Key, Segment>,
    pending: BTreeMap<Key, Duration>,
    animation_due: Option<Duration>,
}
impl PresentationDriver {
    pub fn new(options: PresentationOptions) -> Result<Self, StageError> {
        if options.transition > Duration::from_millis(400) || options.cadence.is_zero() {
            return Err(StageError::Options);
        }
        Ok(Self {
            options,
            last_now: None,
            tracks: BTreeMap::new(),
            pending: BTreeMap::new(),
            animation_due: None,
        })
    }
    /// Absolute caller-time deadline; None means no timed frame demand.
    pub fn next_deadline(&self) -> Option<Duration> {
        self.pending
            .values()
            .copied()
            .chain(self.animation_due)
            .min()
    }
    /// Resolves one complete frame at explicit monotone time. Late work coalesces.
    ///
    /// Graphical rects retarget from the old segment sampled at exactly `now`.
    /// Their interpolated extent crops immutable source pixels; semantic glyphs
    /// take their new layout immediately. Raw FramePath anchors are refused by
    /// this keyed temporal API; seam 2's raw resolver remains available.
    ///
    /// Supply fresh same-frame compilation, producer roster and focus. Reusing
    /// the returned wrapped Compiled as a new semantic compilation is unsupported.
    /// Backward seek requires replay with fresh driver/UI/stateful producers.
    pub fn resolve_frame<A>(
        &mut self,
        now: Duration,
        parts: FrameParts<'_, A>,
        limits: RasterLimits,
    ) -> Result<StageFrame<A>, StageError> {
        if self.last_now.is_some_and(|old| now < old) {
            return Err(StageError::BackwardTime);
        }
        let FrameParts {
            mut compiled,
            viewport,
            underlay,
            planes,
            focus,
            demands,
            omissions,
            motion,
        } = parts;
        if demands.len() > limits.nodes {
            return Err(RasterError::Budget.into());
        }
        let mut live = BTreeSet::new();
        for source in demands {
            if !compiled.geometry_paths.contains_key(&source.key) {
                return Err(StageError::Source(source.key.clone()));
            }
            if !live.insert(source.key.clone()) {
                return Err(StageError::DuplicateSource(source.key.clone()));
            }
        }
        for (i, p) in planes.iter().enumerate() {
            if !matches!(p.anchor, RasterAnchor::Key(_)) {
                return Err(StageError::UnstableAnchor(i));
            }
        }
        if let Some(key) = focus {
            let active = compiled.interactions.active_modal().map(|m| &m.key);
            if !compiled
                .interactions
                .get(key)
                .is_some_and(|i| !i.disabled && i.modal.as_ref() == active)
            {
                return Err(StageError::Focus(key.clone()));
            }
        }
        // A fixed viewport wrapper makes replay through Context's ordinary Stack
        // preserve root auto sizing and editor cursor metadata. Transport the
        // existing frame locators through this exact wrapper, not a new registry.
        let original = std::mem::replace(&mut compiled.node, Node::col());
        compiled.node = Node::col()
            .width(viewport.0 as f32)
            .height(viewport.1 as f32)
            .child(original);
        for path in compiled.geometry_paths.values_mut() {
            path.insert(0, 0);
        }
        raster_plane::admit(
            &compiled.node,
            Some(&compiled.geometry_paths),
            viewport,
            underlay,
            planes,
            limits,
        )?;
        for (key, path) in &compiled.geometry_paths {
            if node_at(&compiled.node, path).is_none() {
                return Err(StageError::Geometry(key.clone()));
            }
        }
        crate::compute_layout(&mut compiled.node, viewport.0, viewport.1)
            .map_err(|e| StageError::Raster(RasterError::Layout(e)))?;
        let area = Rect::new(0, 0, viewport.0, viewport.1);
        let mut next = self.clone();
        next.last_now = Some(now);
        next.pending.retain(|key, _| live.contains(key));
        let due_sources: Vec<Key> = next
            .pending
            .iter()
            .filter(|(_, at)| **at <= now)
            .map(|(k, _)| k.clone())
            .collect();
        for key in &due_sources {
            next.pending.remove(key);
        }
        for source in demands {
            match source.demand {
                FrameDemand::OnChange => {
                    next.pending.remove(&source.key);
                }
                FrameDemand::After(delay) => {
                    if !next.pending.contains_key(&source.key) {
                        let at = now.checked_add(delay).ok_or(StageError::TimeOverflow)?;
                        next.pending.insert(source.key.clone(), at);
                    }
                }
            }
        }
        let mut targets = BTreeMap::new();
        for (i, p) in planes.iter().enumerate() {
            let RasterAnchor::Key(key) = &p.anchor else {
                unreachable!()
            };
            let path = &compiled.geometry_paths[key];
            let g = raster_plane::locate(&compiled.node, path, area, i)?;
            targets.insert(
                key.clone(),
                PixelRect {
                    x: i32::try_from(g.origin_px.0).map_err(|_| RasterError::Coordinate(i))?,
                    y: i32::try_from(g.origin_px.1).map_err(|_| RasterError::Coordinate(i))?,
                    width: u32::from(g.rect.width),
                    height: 2 * u32::from(g.rect.height),
                },
            );
        }
        next.tracks.retain(|key, _| targets.contains_key(key));
        let mut poses = BTreeMap::new();
        for (key, target) in targets {
            let segment = match next.tracks.get(&key).copied() {
                _ if motion != MotionPreference::Full || next.options.transition.is_zero() => {
                    Segment::snapped(target, now)
                }
                None => Segment::snapped(target, now),
                Some(old) if old.to != target => Segment {
                    from: old.sample(now),
                    to: target,
                    start: now,
                    end: now
                        .checked_add(next.options.transition)
                        .ok_or(StageError::TimeOverflow)?,
                },
                Some(old) => old,
            };
            poses.insert(key.clone(), segment.sample(now));
            next.tracks.insert(key, segment);
        }
        let end = next
            .tracks
            .values()
            .filter(|s| s.active(now))
            .map(|s| s.end)
            .min();
        next.animation_due = match end {
            None => None,
            Some(end) => Some(
                next.animation_due
                    .filter(|at| *at > now)
                    .unwrap_or_else(|| {
                        now.checked_add(next.options.cadence)
                            .unwrap_or(Duration::MAX)
                    })
                    .min(end),
            ),
        };
        let (pixels, raster_cells, placements) = raster_plane::compose_prepared(
            &compiled.node,
            Some(&compiled.geometry_paths),
            viewport,
            underlay,
            planes,
            Some(&poses),
        )?;
        let raster_cells = Arc::new(raster_cells);
        let mut cells = raster_cells.as_ref().clone();
        let paint = crate::paint(&compiled.node, &mut cells);
        let (regions, painted) = compiled.painted_key_regions(area);
        let mut target_regions = BTreeMap::new();
        for region in &regions {
            if let Some(tag) = region.tag {
                let spec = compiled
                    .targets
                    .get(tag as usize)
                    .ok_or(StageError::Target(tag))?;
                target_regions.insert(spec.key.clone(), region.clone());
            }
        }
        let hit = compiled.hit_map_from_regions(regions);
        let keys = compiled
            .keys
            .iter()
            .map(|(key, state)| {
                let layout_rect = compiled
                    .geometry_paths
                    .get(key)
                    .and_then(|p| node_at(&compiled.node, p))
                    .map(|n| n.computed_rect);
                (
                    key.clone(),
                    KeySnapshot {
                        state: state.clone(),
                        layout_rect,
                        painted: painted
                            .get(key)
                            .or_else(|| target_regions.get(key))
                            .cloned(),
                    },
                )
            })
            .collect();
        let plane_snapshots = placements
            .into_iter()
            .map(|p| {
                let plane = &planes[p.plane];
                let RasterAnchor::Key(key) = &plane.anchor else {
                    unreachable!()
                };
                let pose = poses[key];
                PlaneSnapshot {
                    plane: p.plane,
                    anchor: plane.anchor.clone(),
                    layout_rect: p.layout_rect,
                    presented: PixelRect {
                        x: p.origin_px.0,
                        y: p.origin_px.1,
                        ..pose
                    },
                    clip_px: p.clip_px,
                    z: p.z,
                    source_size: (plane.source.width(), plane.source.height()),
                }
            })
            .collect();
        let snapshot = ResolvedFrameSnapshot {
            now,
            viewport: area,
            motion,
            keys,
            focus: focus.cloned(),
            targets: hit.targets().to_vec(),
            planes: plane_snapshots,
            omissions: omissions.to_vec(),
            due_sources,
            next_deadline: next.next_deadline(),
            pixels,
            cells,
            cursor_position: paint.cursor_position,
            text_scroll_offset: paint.text_scroll_offset,
            frame_report: None,
        };
        *self = next;
        Ok(StageFrame {
            compiled,
            snapshot,
            raster_cells,
        })
    }
}
fn node_at<'a>(mut node: &'a Node, path: &[usize]) -> Option<&'a Node> {
    for &i in path {
        node = node.children.get(i)?;
    }
    Some(node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::RgbaRaster;
    use crate::ui::prelude::*;
    use crate::{Context, RenderMode, TextInputState};
    fn frame(
        d: &mut PresentationDriver,
        ui: &mut UiRuntime<()>,
        now: Duration,
        x: u16,
    ) -> StageFrame<()> {
        let tree = column()
            .key("root")
            .width(20)
            .height(8)
            .gap(0)
            .padding(0)
            .child(
                row()
                    .height(2)
                    .gap(0)
                    .padding(0)
                    .child(spacer().width(x).grow(0.))
                    .child(column().key("art").width(2).height(2)),
            )
            .child(
                text_input(&TextInputState::with_text("draft"))
                    .key("editor")
                    .on_edit(|_| ()),
            );
        let compiled = ui
            .frame(
                &tree,
                UiEnvironment {
                    width: 20,
                    height: 8,
                    motion: MotionPreference::None,
                    ..Default::default()
                },
                now,
            )
            .unwrap();
        let underlay = RgbRaster::new(20, 16);
        let mut image = RgbaRaster::new(2, 4);
        image.pixels_mut().fill([220, 0, 0, 255]);
        let plane = RasterPlane {
            anchor: RasterAnchor::Key(Key::named("art")),
            source: Arc::new(image),
            offset_px: (0, 0),
            z: 0,
        };
        d.resolve_frame(
            now,
            FrameParts {
                compiled,
                viewport: (20, 8),
                underlay: &underlay,
                planes: &[plane],
                focus: ui.focus(),
                demands: &[],
                omissions: &[],
                motion: MotionPreference::Full,
            },
            RasterLimits {
                planes: 4,
                nodes: 128,
                pixels: 4096,
            },
        )
        .unwrap()
    }
    #[test]
    fn layout_counts_are_one_driver_one_context_and_zero_on_settled_tick() {
        let mut d = PresentationDriver::new(PresentationOptions::default()).unwrap();
        let mut ui = UiRuntime::new(skins::SWISS_SIGNAL);
        let mut ctx = Context::headless(RenderMode::Fullscreen, 20, 8);
        ctx.scheduler.set_max_fps(u32::MAX);
        frame(&mut d, &mut ui, Duration::ZERO, 0);
        crate::layout::reset_layout_passes();
        let animated = frame(&mut d, &mut ui, Duration::ZERO, 8);
        let driver = crate::layout::layout_passes();
        assert_eq!(driver, 1);
        assert!(d.next_deadline().is_some());
        ctx.set_root(animated.to_node());
        ctx.render_now().unwrap();
        let context = crate::layout::layout_passes() - driver;
        assert_eq!(context, 1);
        let settled = frame(&mut d, &mut ui, Duration::from_millis(200), 8);
        ctx.set_root(settled.to_node());
        ctx.render_now().unwrap();
        assert_eq!(d.next_deadline(), None);
        crate::layout::reset_layout_passes();
        assert!(ctx.scheduler.frame_due());
        assert!(!ctx.render_if_due().unwrap());
        let idle = crate::layout::layout_passes();
        assert_eq!(idle, 0);
        eprintln!("W7 LAYOUT RECEIPT: animated frame driver={driver} Context={context} total={}; settled due-permitted tick total={idle}",driver+context);
    }
}
