//! Wave D / Finding D2 — `Shape::hit` is narrow-phase, not bounding-box.
//!
//! A diagonal line's axis-aligned bounding box contains points far from the line, so
//! a pick that used `bounds().contains` wrongly selected the line from a click near a
//! box corner; conversely an axis-aligned line has a zero-extent AABB that no off-axis
//! cell centre lands on, so it was unpickable. `Shape::hit(p, tolerance)` tests the
//! ACTUAL geometry with a declared tolerance. These are red-first against the old
//! bounding-box behaviour (each AABB control asserts the old approximation's verdict,
//! then the narrow-phase hit must disagree).

use gibson::shape::{Point, Shape, ShapeId};

fn line(a: (f64, f64), b: (f64, f64)) -> Shape {
    Shape::line(ShapeId(1), Point::new(a.0, a.1), Point::new(b.0, b.1)).unwrap()
}

#[test]
fn diagonal_line_is_not_hit_at_its_bounding_box_corner() {
    let l = line((0.0, 0.0), (10.0, 10.0));
    // (0,10) is inside the AABB [0,10]x[0,10] but ~7.07 from the segment itself.
    assert!(
        l.bounds().contains(Point::new(0.0, 10.0)),
        "the AABB DOES contain the far corner (the broad-phase control)"
    );
    assert!(
        !l.hit(Point::new(0.0, 10.0), 0.5),
        "narrow-phase must reject the far corner (the #D2 defect)"
    );
    assert!(
        !l.hit(Point::new(0.0, 10.0), 1.0),
        "even a full-cell tolerance is nowhere near ~7 units off the line"
    );
}

#[test]
fn a_point_on_or_near_the_diagonal_is_hit() {
    let l = line((0.0, 0.0), (10.0, 10.0));
    assert!(l.hit(Point::new(5.0, 5.0), 0.5), "dead on the line");
    assert!(
        l.hit(Point::new(5.2, 4.8), 0.5),
        "within half a cell of the line"
    );
    assert!(
        !l.hit(Point::new(6.0, 4.0), 0.5),
        "~1.4 units off the line is a miss at half-cell tolerance"
    );
}

#[test]
fn axis_aligned_line_is_pickable_unlike_its_zero_height_aabb() {
    // Horizontal line at y=0: its AABB is zero-height, so a cell centre at y=0.5 is
    // NOT contained — the old pick could never hit it. Narrow-phase with a half-cell
    // tolerance does.
    let h = line((0.0, 0.0), (10.0, 0.0));
    assert!(
        !h.bounds().contains(Point::new(5.0, 0.5)),
        "the zero-height AABB excludes the cell centre (control)"
    );
    assert!(
        h.hit(Point::new(5.0, 0.5), 0.5),
        "narrow-phase picks a cell the line crosses"
    );
    assert!(
        !h.hit(Point::new(5.0, 1.51), 0.5),
        "but not a cell a cell-and-a-half away"
    );
}

#[test]
fn degenerate_zero_length_line_and_single_point_stroke() {
    let dot = line((3.0, 3.0), (3.0, 3.0)); // zero-length segment -> distance to the point
    assert!(
        dot.hit(Point::new(3.2, 3.1), 0.5),
        "within tolerance of the point"
    );
    assert!(!dot.hit(Point::new(5.0, 5.0), 0.5), "far from the point");

    let one = Shape::stroke(ShapeId(2), vec![Point::new(1.0, 1.0)]).unwrap();
    assert!(
        one.hit(Point::new(1.3, 0.8), 0.5),
        "within tolerance of the lone point"
    );
    assert!(
        !one.hit(Point::new(4.0, 4.0), 0.5),
        "far from the lone point"
    );
}

#[test]
fn stroke_is_hit_near_any_segment_not_in_the_bend() {
    let s = Shape::stroke(
        ShapeId(3),
        vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
        ],
    )
    .unwrap();
    assert!(
        s.hit(Point::new(5.0, 0.2), 0.5),
        "near the first (horizontal) segment"
    );
    assert!(
        s.hit(Point::new(9.8, 5.0), 0.5),
        "near the second (vertical) segment"
    );
    assert!(
        !s.hit(Point::new(2.0, 8.0), 0.5),
        "inside the L's empty interior, far from both segments"
    );
}

#[test]
fn rect_is_a_filled_region_grown_by_tolerance() {
    let r = Shape::rect(ShapeId(4), Point::new(2.0, 2.0), Point::new(6.0, 4.0)).unwrap();
    assert!(r.hit(Point::new(4.0, 3.0), 0.0), "interior, exact");
    assert!(r.hit(Point::new(2.0, 2.0), 0.0), "corner, inclusive");
    assert!(
        !r.hit(Point::new(6.6, 3.0), 0.0),
        "just outside, exact tolerance"
    );
    assert!(
        r.hit(Point::new(6.4, 3.0), 0.5),
        "just outside but within tolerance"
    );
}

#[test]
fn dishonest_inputs_are_never_a_hit() {
    let l = line((0.0, 0.0), (10.0, 10.0));
    assert!(!l.hit(Point::new(f64::NAN, 5.0), 0.5), "NaN point");
    assert!(!l.hit(Point::new(5.0, 5.0), f64::NAN), "NaN tolerance");
    assert!(!l.hit(Point::new(5.0, 5.0), -0.1), "negative tolerance");
    assert!(
        !l.hit(Point::new(5.0, 5.0), f64::INFINITY),
        "infinite tolerance is nonsense input, rejected even dead on the line"
    );
}
