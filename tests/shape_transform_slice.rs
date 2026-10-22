//! R4-C continuation (issue #132): non-mutating shape transforms.
//!
//! The immutability law, re-tested from the hardest angle: a transform must
//! produce a NEW shape and leave the original byte-identical. `translate` and
//! `scale_about` return a fresh [`Shape`] that keeps the source's [`ShapeId`]
//! (moving a shape does not change which shape it is), refuse non-finite
//! arguments (`None`, never a silently-sanitized result), and — for a rectangle
//! — re-normalize so a negative scale still yields a valid `min <= max` box.

use gibson::shape::{Point, Shape, ShapeId};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

// ===========================================================================
// translate
// ===========================================================================

#[test]
fn translate_returns_new_shape_and_leaves_original_untouched() {
    let r = Shape::rect(ShapeId(7), p(0.0, 0.0), p(4.0, 2.0)).unwrap();
    let before = r.clone();
    let moved = r.translate(10.0, 5.0).expect("finite translation");
    // THE LAW, through a transform: the source is byte-identical afterward.
    assert_eq!(r, before);
    // The moved copy is shifted, and keeps the SAME identity.
    assert_eq!(moved.rect_corners(), Some((p(10.0, 5.0), p(14.0, 7.0))));
    assert_eq!(moved.id(), ShapeId(7));
}

#[test]
fn translate_shifts_every_kind() {
    let l = Shape::line(ShapeId(1), p(1.0, 1.0), p(3.0, 4.0))
        .unwrap()
        .translate(1.0, -1.0)
        .unwrap();
    assert_eq!(l.line_ends(), Some((p(2.0, 0.0), p(4.0, 3.0))));

    let s = Shape::stroke(ShapeId(2), vec![p(0.0, 0.0), p(2.0, 2.0)])
        .unwrap()
        .translate(5.0, 5.0)
        .unwrap();
    assert_eq!(
        s.stroke_points(),
        Some([p(5.0, 5.0), p(7.0, 7.0)].as_slice())
    );

    let r = Shape::rect(ShapeId(3), p(0.0, 0.0), p(2.0, 2.0))
        .unwrap()
        .translate(-3.0, 1.0)
        .unwrap();
    let b = r.bounds();
    assert_eq!(b.min, p(-3.0, 1.0));
    assert_eq!(b.max, p(-1.0, 3.0));
}

#[test]
fn translate_rejects_nonfinite() {
    let r = Shape::rect(ShapeId(1), p(0.0, 0.0), p(1.0, 1.0)).unwrap();
    assert!(r.translate(f64::NAN, 0.0).is_none());
    assert!(r.translate(0.0, f64::INFINITY).is_none());
}

#[test]
fn translate_zero_is_an_identity() {
    let r = Shape::rect(ShapeId(9), p(1.0, 2.0), p(5.0, 8.0)).unwrap();
    let same = r.translate(0.0, 0.0).unwrap();
    // Same id AND same geometry => equal shape.
    assert_eq!(same, r);
}

// ===========================================================================
// scale_about
// ===========================================================================

#[test]
fn scale_about_scales_from_the_pivot_and_preserves_id() {
    let r = Shape::rect(ShapeId(4), p(0.0, 0.0), p(2.0, 2.0)).unwrap();
    let before = r.clone();
    let big = r.scale_about(p(0.0, 0.0), 2.0).unwrap();
    assert_eq!(r, before); // original untouched
    assert_eq!(big.rect_corners(), Some((p(0.0, 0.0), p(4.0, 4.0))));
    assert_eq!(big.id(), ShapeId(4));
}

#[test]
fn scale_about_renormalizes_rect_under_negative_factor() {
    // A negative factor flips the corners; the rect must re-normalize to a valid
    // min <= max box rather than storing an inverted one.
    let r = Shape::rect(ShapeId(1), p(0.0, 0.0), p(2.0, 2.0)).unwrap();
    let flipped = r.scale_about(p(0.0, 0.0), -1.0).unwrap();
    assert_eq!(flipped.rect_corners(), Some((p(-2.0, -2.0), p(0.0, 0.0))));
}

#[test]
fn scale_about_line_preserves_endpoint_order() {
    // A line is directed: scaling moves the endpoints but must NOT reorder them.
    let l = Shape::line(ShapeId(1), p(0.0, 0.0), p(2.0, 0.0)).unwrap();
    let scaled = l.scale_about(p(0.0, 0.0), -1.0).unwrap();
    assert_eq!(scaled.line_ends(), Some((p(0.0, 0.0), p(-2.0, 0.0))));
}

#[test]
fn scale_about_factor_zero_collapses_to_pivot() {
    let l = Shape::line(ShapeId(1), p(4.0, 4.0), p(8.0, 0.0)).unwrap();
    let collapsed = l.scale_about(p(1.0, 1.0), 0.0).unwrap();
    assert_eq!(collapsed.line_ends(), Some((p(1.0, 1.0), p(1.0, 1.0))));
    assert_eq!(collapsed.length(), 0.0);
    let b = collapsed.bounds();
    assert_eq!(b.min, p(1.0, 1.0));
    assert_eq!(b.max, p(1.0, 1.0));
}

#[test]
fn scale_about_rejects_nonfinite() {
    let r = Shape::rect(ShapeId(1), p(0.0, 0.0), p(1.0, 1.0)).unwrap();
    assert!(r.scale_about(p(f64::NAN, 0.0), 2.0).is_none());
    assert!(r.scale_about(p(0.0, 0.0), f64::INFINITY).is_none());
}
