//! Wave D / Finding D1 — `NumericField` validates its SCHEMA, not just values.
//!
//! The old field validated a value against its bounds but accepted a malformed schema
//! silently — worst of all, a NaN bound failed OPEN (`value < NaN` and `value > NaN`
//! are both false), so `with_range(NaN, 10).validate(-999)` returned `Ok(-999)`. These
//! pin the checked schema path, the closed fail-open hole, and the preserved
//! no-silent-clamp / anti-over-rejection laws.

use gibson::numeric::{NumericError, NumericField, NumericSchemaError};

#[test]
fn nan_bound_no_longer_fails_open() {
    let f = NumericField::new().with_range(f64::NAN, 10.0);
    // Old behaviour: Ok(-999.0). Now the malformed schema is surfaced, not fail-open.
    assert!(matches!(
        f.validate(-999.0),
        Err(NumericError::InvalidSchema(
            NumericSchemaError::NonFiniteBound
        ))
    ));
    assert_eq!(f.checked(), Err(NumericSchemaError::NonFiniteBound));
}

#[test]
fn infinite_bounds_are_rejected() {
    assert_eq!(
        NumericField::new().with_min(f64::INFINITY).checked(),
        Err(NumericSchemaError::NonFiniteBound)
    );
    assert_eq!(
        NumericField::new().with_max(f64::NEG_INFINITY).checked(),
        Err(NumericSchemaError::NonFiniteBound)
    );
}

#[test]
fn reversed_range_is_rejected() {
    let f = NumericField::new().with_range(8.0, 3.0);
    assert_eq!(
        f.checked(),
        Err(NumericSchemaError::ReversedRange { min: 8.0, max: 3.0 })
    );
    assert!(matches!(
        f.validate(5.0),
        Err(NumericError::InvalidSchema(
            NumericSchemaError::ReversedRange { .. }
        ))
    ));
}

#[test]
fn equal_min_max_is_a_valid_single_value_domain() {
    let f = NumericField::new().with_range(4.0, 4.0);
    assert_eq!(f.checked(), Ok(f));
    assert_eq!(f.validate(4.0), Ok(4.0));
    assert!(matches!(f.validate(4.1), Err(NumericError::Above { .. })));
}

#[test]
fn invalid_steps_are_rejected_positive_steps_pass() {
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            matches!(
                NumericField::new().with_step(bad).checked(),
                Err(NumericSchemaError::InvalidStep { .. })
            ),
            "step {bad} must be rejected"
        );
    }
    assert!(
        NumericField::new().with_step(0.5).checked().is_ok(),
        "a positive step is well-formed"
    );
}

#[test]
fn a_valid_schema_accepts_in_range_and_refuses_out_of_range_never_clamping() {
    let f = NumericField::new().with_range(0.0, 10.0).with_step(1.0);
    assert_eq!(f.checked(), Ok(f));
    assert_eq!(f.validate(7.0), Ok(7.0), "in range, returned unchanged");
    assert!(
        matches!(f.validate(99.0), Err(NumericError::Above { value, .. }) if value == 99.0),
        "out of range is refused with the offending value, never clamped to 10"
    );
}

#[test]
fn negative_and_zero_values_are_not_over_rejected() {
    // A negative-min schema must accept negative and zero values (anti over-rejection).
    let f = NumericField::new().with_range(-5.0, 5.0);
    assert_eq!(f.validate(-3.0), Ok(-3.0));
    assert_eq!(f.validate(0.0), Ok(0.0));
    assert_eq!(f.validate(-5.0), Ok(-5.0), "inclusive lower bound");
}
