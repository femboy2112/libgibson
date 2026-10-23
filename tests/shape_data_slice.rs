//! R4-C falsifiers (issue #132): an immutable 2D shape data-layer whose
//! exporters are pure reads that NEVER mutate the stored geometry.
//!
//! The load-bearing law (the one the whole interactive-instrument chain rests
//! on): a shape is authoritative, immutable world-space geometry, and every
//! *exporter* — bounds, vertices, length — is a derived, non-mutating `&self`
//! read. No display/derivation operation secretly rewrites the model. Alongside
//! that: construction is honest (non-finite geometry and an empty stroke are
//! refused, not silently sanitized to a lie), a rectangle normalizes inverted
//! corners, a line preserves its direction, and a [`ShapeId`] carries identity
//! independent of geometry so the same shape survives every projection.

use std::collections::HashSet;

use gibson::shape::{Bounds, Point, Shape, ShapeId, ShapeKind};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

// ===========================================================================
// Point / Bounds — the primitives
// ===========================================================================

#[test]
fn point_tuple_roundtrip_and_distance_and_finiteness() {
    let a = Point::from((3.0, 4.0));
    assert_eq!(a, p(3.0, 4.0));
    let t: (f64, f64) = a.into();
    assert_eq!(t, (3.0, 4.0));
    // 3-4-5 right triangle: exact.
    assert_eq!(a.distance(p(0.0, 0.0)), 5.0);
    assert!(a.is_finite());
    assert!(!p(f64::NAN, 0.0).is_finite());
    assert!(!p(0.0, f64::INFINITY).is_finite());
}

#[test]
fn bounds_contains_is_inclusive_and_rejects_nonfinite() {
    let b = Bounds {
        min: p(0.0, 0.0),
        max: p(10.0, 5.0),
    };
    assert_eq!(b.width(), 10.0);
    assert_eq!(b.height(), 5.0);
    // Inclusive on every edge and corner.
    assert!(b.contains(p(0.0, 0.0)));
    assert!(b.contains(p(10.0, 5.0)));
    assert!(b.contains(p(10.0, 0.0)));
    assert!(b.contains(p(5.0, 2.5)));
    // Just outside on each side.
    assert!(!b.contains(p(10.0001, 2.5)));
    assert!(!b.contains(p(-0.0001, 2.5)));
    assert!(!b.contains(p(5.0, 5.0001)));
    // A non-finite query point is never contained.
    assert!(!b.contains(p(f64::NAN, 2.5)));
    assert!(!b.contains(p(f64::INFINITY, 2.5)));
}

// ===========================================================================
// Construction — honest invariants
// ===========================================================================

#[test]
fn rect_normalizes_inverted_corners() {
    // Corners handed in "backwards" still produce a valid normalized rect.
    let r = Shape::rect(ShapeId(1), p(10.0, 8.0), p(0.0, 3.0)).expect("finite rect");
    assert_eq!(r.kind(), ShapeKind::Rect);
    assert_eq!(r.rect_corners(), Some((p(0.0, 3.0), p(10.0, 8.0))));
    let b = r.bounds();
    assert_eq!(b.min, p(0.0, 3.0));
    assert_eq!(b.max, p(10.0, 8.0));
}

#[test]
fn rect_rejects_nonfinite_corner() {
    assert!(Shape::rect(ShapeId(1), p(f64::NAN, 0.0), p(1.0, 1.0)).is_none());
    assert!(Shape::rect(ShapeId(1), p(0.0, 0.0), p(f64::INFINITY, 1.0)).is_none());
}

#[test]
fn line_preserves_direction_not_normalized() {
    // A line is DIRECTED: start/end order is meaningful and must survive.
    let l = Shape::line(ShapeId(2), p(5.0, 5.0), p(0.0, 0.0)).expect("finite line");
    assert_eq!(l.kind(), ShapeKind::Line);
    assert_eq!(l.line_ends(), Some((p(5.0, 5.0), p(0.0, 0.0))));
    assert_eq!(l.vertices(), vec![p(5.0, 5.0), p(0.0, 0.0)]);
    // But its bounding box IS normalized.
    let b = l.bounds();
    assert_eq!(b.min, p(0.0, 0.0));
    assert_eq!(b.max, p(5.0, 5.0));
}

#[test]
fn line_rejects_nonfinite() {
    assert!(Shape::line(ShapeId(2), p(0.0, 0.0), p(f64::NAN, 1.0)).is_none());
}

#[test]
fn stroke_rejects_empty_and_nonfinite() {
    assert!(Shape::stroke(ShapeId(3), vec![]).is_none());
    assert!(Shape::stroke(ShapeId(3), vec![p(0.0, 0.0), p(f64::NAN, 1.0)]).is_none());
}

#[test]
fn stroke_single_point_is_degenerate_but_valid() {
    let s = Shape::stroke(ShapeId(3), vec![p(2.0, 2.0)]).expect("one point is a valid stroke");
    assert_eq!(s.kind(), ShapeKind::Stroke);
    assert_eq!(s.length(), 0.0);
    assert_eq!(s.stroke_points(), Some([p(2.0, 2.0)].as_slice()));
    let b = s.bounds();
    assert_eq!(b.min, p(2.0, 2.0));
    assert_eq!(b.max, p(2.0, 2.0));
    assert_eq!(s.vertices(), vec![p(2.0, 2.0)]);
}

// ===========================================================================
// Exporters — geometric correctness
// ===========================================================================

#[test]
fn rect_vertices_are_four_corners_and_length_is_perimeter() {
    let r = Shape::rect(ShapeId(1), p(0.0, 0.0), p(4.0, 3.0)).unwrap();
    // CW-from-min convention: min, (max.x,min.y), max, (min.x,max.y).
    assert_eq!(
        r.vertices(),
        vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0), p(0.0, 3.0)]
    );
    // Perimeter 2*(w+h) = 2*(4+3) = 14.
    assert_eq!(r.length(), 14.0);
    let b = r.bounds();
    assert_eq!(b.width(), 4.0);
    assert_eq!(b.height(), 3.0);
}

#[test]
fn line_length_is_euclidean() {
    let l = Shape::line(ShapeId(2), p(0.0, 0.0), p(3.0, 4.0)).unwrap();
    assert_eq!(l.length(), 5.0);
    assert_eq!(l.vertices(), vec![p(0.0, 0.0), p(3.0, 4.0)]);
}

#[test]
fn stroke_length_sums_segments_and_bounds_span_all_points() {
    let s = Shape::stroke(ShapeId(3), vec![p(0.0, 0.0), p(3.0, 4.0), p(3.0, 9.0)]).unwrap();
    // seg1 = 5 (3-4-5), seg2 = 5 (vertical 4->9) => 10.
    assert_eq!(s.length(), 10.0);
    assert_eq!(s.vertices(), vec![p(0.0, 0.0), p(3.0, 4.0), p(3.0, 9.0)]);
    let b = s.bounds();
    assert_eq!(b.min, p(0.0, 0.0));
    assert_eq!(b.max, p(3.0, 9.0));
}

// ===========================================================================
// THE LAW — exporters never mutate the authoritative shape
// ===========================================================================

#[test]
fn exporters_never_mutate_the_shape() {
    let shapes = [
        Shape::rect(ShapeId(1), p(1.0, 2.0), p(7.0, 9.0)).unwrap(),
        Shape::line(ShapeId(2), p(5.0, 5.0), p(0.0, 1.0)).unwrap(),
        Shape::stroke(ShapeId(3), vec![p(0.0, 0.0), p(1.0, 1.0), p(2.0, 0.0)]).unwrap(),
    ];
    for shape in shapes {
        let before = shape.clone();
        // Call every read/exporter. None takes &mut self; none may change state.
        let _ = shape.id();
        let _ = shape.kind();
        let _ = shape.rect_corners();
        let _ = shape.line_ends();
        let _ = shape.stroke_points();
        let b1 = shape.bounds();
        let v1 = shape.vertices();
        let n1 = shape.length();
        // The model is byte-identical after being observed.
        assert_eq!(shape, before, "an exporter mutated the shape");
        // And exporters are deterministic: a second read equals the first.
        assert_eq!(shape.bounds(), b1);
        assert_eq!(shape.vertices(), v1);
        assert_eq!(shape.length(), n1);
    }
}

// ===========================================================================
// Identity across projections (one authoritative source identity)
// ===========================================================================

#[test]
fn shape_id_is_stable_across_clone_and_export() {
    let r = Shape::rect(ShapeId(42), p(0.0, 0.0), p(1.0, 1.0)).unwrap();
    assert_eq!(r.id(), ShapeId(42));
    let clone = r.clone();
    let _ = r.bounds();
    let _ = r.vertices();
    assert_eq!(r.id(), ShapeId(42), "id must survive observation");
    assert_eq!(clone.id(), ShapeId(42), "id must survive clone");
}

#[test]
fn shape_id_is_hashable_and_distinct() {
    // ShapeId keys an identity set: equal ids collide, distinct ids don't.
    let mut seen: HashSet<ShapeId> = HashSet::new();
    assert!(seen.insert(ShapeId(1)));
    assert!(seen.insert(ShapeId(2)));
    assert!(!seen.insert(ShapeId(1)), "duplicate id must not re-insert");
    assert!(seen.contains(&ShapeId(2)));
    assert_eq!(seen.len(), 2);
}

// ===========================================================================
// Accessors discriminate by kind without exposing raw storage
// ===========================================================================

#[test]
fn typed_accessors_match_kind_and_are_none_otherwise() {
    let r = Shape::rect(ShapeId(1), p(0.0, 0.0), p(1.0, 1.0)).unwrap();
    assert!(r.rect_corners().is_some());
    assert!(r.line_ends().is_none());
    assert!(r.stroke_points().is_none());

    let l = Shape::line(ShapeId(2), p(0.0, 0.0), p(1.0, 1.0)).unwrap();
    assert!(l.rect_corners().is_none());
    assert!(l.line_ends().is_some());
    assert!(l.stroke_points().is_none());

    let s = Shape::stroke(ShapeId(3), vec![p(0.0, 0.0)]).unwrap();
    assert!(s.rect_corners().is_none());
    assert!(s.line_ends().is_none());
    assert!(s.stroke_points().is_some());
}
