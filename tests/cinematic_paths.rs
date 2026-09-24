//! Algebraic/hostile-input contracts for shared camera and courier paths.
use gibson::geom::{CubicPath3, Vec3};

fn path() -> CubicPath3 {
    CubicPath3 {
        start: Vec3::new(-2., 1., 4.),
        control1: Vec3::new(1., 5., 4.),
        control2: Vec3::new(6., 3., -2.),
        end: Vec3::new(9., 3., -6.),
    }
}
fn close(actual: Vec3, expected: Vec3, epsilon: f32) {
    assert!(
        (actual.x - expected.x).abs() <= epsilon,
        "{actual:?} != {expected:?}"
    );
    assert!(
        (actual.y - expected.y).abs() <= epsilon,
        "{actual:?} != {expected:?}"
    );
    assert!(
        (actual.z - expected.z).abs() <= epsilon,
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn endpoints_and_parameter_clamping_preserve_route_anchors() {
    let p = path();
    assert_eq!(p.sample(0.), p.start);
    assert_eq!(p.sample(1.), p.end);
    assert_eq!(p.sample(-20.), p.start);
    assert_eq!(p.sample(20.), p.end);
    for t in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(p.sample(t), p.start);
        assert_eq!(p.tangent(t), p.tangent(0.));
    }
}

#[test]
fn endpoint_tangents_and_interior_finite_difference_agree() {
    let p = path();
    close(p.tangent(0.), Vec3::new(0.6, 0.8, 0.), 1e-6);
    close(p.tangent(1.), Vec3::new(0.6, 0., -0.8), 1e-6);
    for t in [0.2, 0.5, 0.8] {
        let derivative = p.sample(t + 0.001).minus(p.sample(t - 0.001)).normalize();
        close(p.tangent(t), derivative, 0.001);
        assert!((p.tangent(t).length() - 1.).abs() < 1e-6);
    }
}

#[test]
fn reversed_route_has_reversed_motion_without_changing_geometry() {
    let p = path();
    let reversed = CubicPath3 {
        start: p.end,
        control1: p.control2,
        control2: p.control1,
        end: p.start,
    };
    for t in [0., 0.125, 0.25, 0.5, 0.75, 1.] {
        close(p.sample(t), reversed.sample(1. - t), 1e-5);
        close(p.tangent(t), reversed.tangent(1. - t).scale(-1.), 1e-5);
    }
}

#[test]
fn stationary_and_nonfinite_routes_have_explicit_safe_results() {
    let origin = Vec3::new(4., 3., 2.);
    let stationary = CubicPath3 {
        start: origin,
        control1: origin,
        control2: origin,
        end: origin,
    };
    assert_eq!(stationary.sample(0.5), origin);
    assert_eq!(stationary.tangent(0.5), Vec3::default());
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut p = path();
        p.control2.y = invalid;
        assert_eq!(p.sample(0.5), Vec3::default());
        assert_eq!(p.tangent(0.5), Vec3::default());
    }
}

#[test]
fn hostile_finite_control_points_do_not_overflow() {
    let m = f32::MAX;
    let p = CubicPath3 {
        start: Vec3::new(m, -m, m),
        control1: Vec3::new(-m, m, m),
        control2: Vec3::new(m, -m, m),
        end: Vec3::new(-m, m, m),
    };
    for t in [0., 0.1, 0.3, 0.5, 0.9, 1.] {
        assert!(p.sample(t).is_finite());
        assert!(p.tangent(t).is_finite());
        assert_eq!(p.sample(t).z, m);
    }
}
