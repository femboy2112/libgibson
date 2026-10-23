//! Immutable 2D shape geometry in WORLD f64 space (issue #132).
//!
//! A [`Shape`] is authoritative, immutable world-space geometry — a rectangle,
//! a directed line, or a freehand stroke — tagged with a [`ShapeId`] that
//! carries identity independently of the geometry it names. Coordinates live in
//! the same WORLD f64 convention the rest of the crate projects through to
//! VIEW/RASTER/CELL; nothing in here touches a view, a raster, a clock, or a
//! global. Every function is a pure function of its inputs.
//!
//! The load-bearing law of this module — the one the interactive-instrument
//! chain leans on — is twofold:
//!
//! * **Immutable model.** A built [`Shape`] exposes no `&mut` path to its
//!   geometry. There is deliberately no mutating method; the stored geometry is
//!   private and only ever read.
//! * **Non-mutating exporters.** [`Shape::bounds`], [`Shape::vertices`], and
//!   [`Shape::length`] are derived `&self` reads. A projection observes the
//!   model; it never secretly rewrites it. Observe a shape a thousand times and
//!   it is byte-identical afterward.
//!
//! Construction is honest: non-finite geometry and an empty stroke are refused
//! (returned as `None`), never silently sanitized into a plausible lie. A
//! rectangle normalizes inverted corners; a line keeps its direction.

/// Stable identity for a [`Shape`], independent of its geometry.
///
/// Two shapes with equal ids are the same source object across every
/// projection and clone; the value is a hashable key, not a coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShapeId(
    /// The opaque identity value.
    pub u64,
);

/// A point in WORLD f64 space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// World-space x coordinate.
    pub x: f64,
    /// World-space y coordinate.
    pub y: f64,
}

impl Point {
    /// Construct a point from its world coordinates. No sanitizing: a non-finite
    /// coordinate in is a non-finite coordinate out — the constructors above are
    /// where honesty is enforced, not here.
    pub fn new(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    /// True iff *both* coordinates are finite. One NaN or infinity poisons the
    /// whole point; a half-finite point is still unusable geometry.
    pub fn is_finite(&self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// Euclidean distance to another point, via [`f64::hypot`] so the
    /// intermediate square never overflows a representable separation.
    pub fn distance(&self, other: Point) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

impl From<(f64, f64)> for Point {
    fn from(t: (f64, f64)) -> Point {
        Point { x: t.0, y: t.1 }
    }
}

impl From<Point> for (f64, f64) {
    fn from(p: Point) -> (f64, f64) {
        (p.x, p.y)
    }
}

/// An axis-aligned bounding box in WORLD f64 space.
///
/// `min`/`max` are the per-axis extremes; a well-formed box has `min <= max` on
/// each axis, which every exporter that produces one guarantees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    /// Lower-left corner (per-axis minimum).
    pub min: Point,
    /// Upper-right corner (per-axis maximum).
    pub max: Point,
}

impl Bounds {
    /// Width of the box on the x axis.
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }

    /// Height of the box on the y axis.
    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }

    /// Inclusive hit test: true iff `p` lies on or inside the box. A non-finite
    /// query point is never contained — we refuse to let a NaN smuggle itself
    /// past the edges rather than letting the range comparison decide it.
    pub fn contains(&self, p: Point) -> bool {
        if !p.is_finite() {
            return false;
        }
        (self.min.x..=self.max.x).contains(&p.x) && (self.min.y..=self.max.y).contains(&p.y)
    }
}

/// Euclidean distance from `p` to the segment `a`-`b` in WORLD space. A degenerate
/// (zero-length) segment is the distance to its shared endpoint. Uses [`f64::hypot`]
/// for the final magnitude so a representable separation never overflows.
fn point_segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let (abx, aby) = (b.x - a.x, b.y - a.y);
    let len2 = abx * abx + aby * aby;
    if len2 == 0.0 {
        return p.distance(a);
    }
    // Project p onto the infinite line, then clamp the parameter to the segment.
    let t = (((p.x - a.x) * abx + (p.y - a.y) * aby) / len2).clamp(0.0, 1.0);
    p.distance(Point::new(a.x + t * abx, a.y + t * aby))
}

/// The fieldless discriminant of a [`Shape`]'s geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    /// An axis-aligned rectangle, stored normalized.
    Rect,
    /// A directed line segment (start/end order is meaningful).
    Line,
    /// A freehand polyline of one or more points, in order.
    Stroke,
}

/// Private authoritative storage. Held by value inside [`Shape`] and never
/// handed out by reference-mut — this enum *is* the immutability guarantee.
#[derive(Debug, Clone, PartialEq)]
enum Geom {
    /// Normalized rectangle: `min <= max` on each axis.
    Rect { min: Point, max: Point },
    /// Directed segment: `start`/`end` preserved exactly as constructed.
    Line { start: Point, end: Point },
    /// Ordered polyline of at least one finite point.
    Stroke { points: Vec<Point> },
}

/// An immutable 2D shape: an identity plus authoritative WORLD f64 geometry.
///
/// Both fields are private. Read it through the typed accessors and the
/// exporters; there is no way to mutate a built shape, by design.
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    id: ShapeId,
    geom: Geom,
}

impl Shape {
    /// Build a rectangle from two opposite corners, stored **normalized**: the
    /// result's `min` is the per-axis minimum of `a`/`b` and its `max` the
    /// per-axis maximum, so corners handed in "backwards" still produce a valid
    /// box. Returns `None` if any coordinate is non-finite — a rect with a NaN
    /// edge is not a smaller rect, it is not a rect.
    pub fn rect(id: ShapeId, a: Point, b: Point) -> Option<Shape> {
        if !a.is_finite() || !b.is_finite() {
            return None;
        }
        let min = Point::new(a.x.min(b.x), a.y.min(b.y));
        let max = Point::new(a.x.max(b.x), a.y.max(b.y));
        Some(Shape {
            id,
            geom: Geom::Rect { min, max },
        })
    }

    /// Build a **directed** line from `start` to `end`. Direction is meaningful
    /// and preserved exactly — no normalization. Returns `None` if any
    /// coordinate is non-finite.
    pub fn line(id: ShapeId, start: Point, end: Point) -> Option<Shape> {
        if !start.is_finite() || !end.is_finite() {
            return None;
        }
        Some(Shape {
            id,
            geom: Geom::Line { start, end },
        })
    }

    /// Build a freehand stroke from an ordered point list. Returns `None` if the
    /// list is empty or any point is non-finite. A single-point stroke is valid
    /// (degenerate but honest): it has zero length and a point-sized box.
    pub fn stroke(id: ShapeId, points: Vec<Point>) -> Option<Shape> {
        if points.is_empty() || points.iter().any(|p| !p.is_finite()) {
            return None;
        }
        Some(Shape {
            id,
            geom: Geom::Stroke { points },
        })
    }

    /// This shape's stable identity.
    pub fn id(&self) -> ShapeId {
        self.id
    }

    /// This shape's geometric kind.
    pub fn kind(&self) -> ShapeKind {
        match self.geom {
            Geom::Rect { .. } => ShapeKind::Rect,
            Geom::Line { .. } => ShapeKind::Line,
            Geom::Stroke { .. } => ShapeKind::Stroke,
        }
    }

    /// The normalized `(min, max)` corners — `Some` only for a rectangle.
    pub fn rect_corners(&self) -> Option<(Point, Point)> {
        match self.geom {
            Geom::Rect { min, max } => Some((min, max)),
            _ => None,
        }
    }

    /// The directed `(start, end)` endpoints — `Some` only for a line.
    pub fn line_ends(&self) -> Option<(Point, Point)> {
        match self.geom {
            Geom::Line { start, end } => Some((start, end)),
            _ => None,
        }
    }

    /// The ordered stroke points — `Some` only for a stroke. Borrowed, never
    /// copied out and never mutable.
    pub fn stroke_points(&self) -> Option<&[Point]> {
        match &self.geom {
            Geom::Stroke { points } => Some(points.as_slice()),
            _ => None,
        }
    }

    /// Axis-aligned bounding box of the shape — a pure, non-mutating read.
    ///
    /// Rect: its own normalized `{min, max}`. Line: the normalized box over its
    /// two endpoints. Stroke: the box spanning every point.
    pub fn bounds(&self) -> Bounds {
        match &self.geom {
            Geom::Rect { min, max } => Bounds {
                min: *min,
                max: *max,
            },
            Geom::Line { start, end } => Bounds {
                min: Point::new(start.x.min(end.x), start.y.min(end.y)),
                max: Point::new(start.x.max(end.x), start.y.max(end.y)),
            },
            Geom::Stroke { points } => {
                // `stroke` guarantees a non-empty list, so the first point is a
                // sound seed for the fold.
                let mut min = points[0];
                let mut max = points[0];
                for pt in &points[1..] {
                    min = Point::new(min.x.min(pt.x), min.y.min(pt.y));
                    max = Point::new(max.x.max(pt.x), max.y.max(pt.y));
                }
                Bounds { min, max }
            }
        }
    }

    /// Narrow-phase hit test in WORLD space: true iff `p` lies within `tolerance`
    /// of the shape's ACTUAL geometry, not merely its bounding box. Pass half a
    /// world cell for `tolerance` so a click anywhere in the cell over the shape
    /// counts; `0.0` is the exact geometric test. A filled rectangle is hit over
    /// its interior (grown by `tolerance`); a line or stroke is hit within
    /// `tolerance` of its segments — so a diagonal line no longer swallows the far
    /// corners of its bounding box, and an axis-aligned line (whose bounding box is
    /// zero-height and so misses every off-axis cell centre) is still pickable.
    ///
    /// Refuses dishonest inputs like the rest of the module: a non-finite `p`, or a
    /// non-finite or negative `tolerance`, is never a hit.
    pub fn hit(&self, p: Point, tolerance: f64) -> bool {
        if !p.is_finite() || !tolerance.is_finite() || tolerance < 0.0 {
            return false;
        }
        match &self.geom {
            Geom::Rect { min, max } => {
                p.x >= min.x - tolerance
                    && p.x <= max.x + tolerance
                    && p.y >= min.y - tolerance
                    && p.y <= max.y + tolerance
            }
            Geom::Line { start, end } => point_segment_distance(p, *start, *end) <= tolerance,
            Geom::Stroke { points } => match points.as_slice() {
                [only] => p.distance(*only) <= tolerance,
                _ => points
                    .windows(2)
                    .any(|w| point_segment_distance(p, w[0], w[1]) <= tolerance),
            },
        }
    }

    /// The shape's vertices in draw order — a pure, non-mutating read.
    ///
    /// Rect: the four corners clockwise from `min`:
    /// `[min, (max.x, min.y), max, (min.x, max.y)]`. Line: `[start, end]`.
    /// Stroke: the points in order.
    pub fn vertices(&self) -> Vec<Point> {
        match &self.geom {
            Geom::Rect { min, max } => vec![
                *min,
                Point::new(max.x, min.y),
                *max,
                Point::new(min.x, max.y),
            ],
            Geom::Line { start, end } => vec![*start, *end],
            Geom::Stroke { points } => points.clone(),
        }
    }

    /// The shape's length measure — a pure, non-mutating read.
    ///
    /// Rect: perimeter `2 * (width + height)`. Line: the Euclidean distance
    /// between its endpoints. Stroke: the summed length of its consecutive
    /// segments (`0.0` for a single-point stroke — no segments to walk).
    pub fn length(&self) -> f64 {
        match &self.geom {
            Geom::Rect { min, max } => {
                let width = max.x - min.x;
                let height = max.y - min.y;
                2.0 * (width + height)
            }
            Geom::Line { start, end } => start.distance(*end),
            Geom::Stroke { points } => points.windows(2).map(|w| w[0].distance(w[1])).sum(),
        }
    }

    /// Translates the shape by `(dx, dy)`, returning a NEW shape with the same
    /// [`ShapeId`] — a pure, non-mutating transform that leaves `self` untouched.
    /// Returns `None` if `dx` or `dy` is non-finite, or if the shift would push a
    /// coordinate past the finite range.
    pub fn translate(&self, dx: f64, dy: f64) -> Option<Shape> {
        if !dx.is_finite() || !dy.is_finite() {
            return None;
        }
        self.map_points(|pt| Point::new(pt.x + dx, pt.y + dy))
    }

    /// Scales the shape about `pivot` by `factor`, returning a NEW shape with the
    /// same [`ShapeId`]. A rectangle re-normalizes so a negative factor still
    /// yields a valid `min <= max` box; a line keeps its endpoint order. Returns
    /// `None` if `pivot` or `factor` is non-finite, or if the result would be
    /// non-finite.
    pub fn scale_about(&self, pivot: Point, factor: f64) -> Option<Shape> {
        if !pivot.is_finite() || !factor.is_finite() {
            return None;
        }
        self.map_points(|pt| {
            Point::new(
                pivot.x + (pt.x - pivot.x) * factor,
                pivot.y + (pt.y - pivot.y) * factor,
            )
        })
    }

    /// Applies `f` to the shape's defining points and rebuilds through the
    /// validating constructors — which re-check finiteness (an overflow to a
    /// non-finite coordinate becomes `None`) and re-normalize a rectangle — while
    /// preserving the identity.
    fn map_points(&self, f: impl Fn(Point) -> Point) -> Option<Shape> {
        match &self.geom {
            Geom::Rect { min, max } => Shape::rect(self.id, f(*min), f(*max)),
            Geom::Line { start, end } => Shape::line(self.id, f(*start), f(*end)),
            Geom::Stroke { points } => {
                Shape::stroke(self.id, points.iter().map(|p| f(*p)).collect())
            }
        }
    }
}
