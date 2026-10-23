//! R4-B falsifiers (issue #134): validated numeric editing that NEVER silently
//! clamps or coerces an authoritative value.
//!
//! The load-bearing guarantee: an out-of-range or non-finite edit is *refused*
//! (reported as `NumericEdit::Invalid` / `NumericError`), never quietly rewritten
//! to a boundary. Preview is non-authoritative; a commit only hands back a value
//! when it is valid; cancel changes nothing. The keyboard path (`set_text`) and
//! the pointer/stepper path (`step`) flow through the SAME typed vocabulary and
//! the SAME `NumericField` validation.

use gibson::edit::{CancelReason, EditId};
use gibson::numeric::{NumericEdit, NumericError, NumericField, NumericInput};

fn field_0_10() -> NumericField {
    NumericField::new().with_range(0.0, 10.0)
}

// ===========================================================================
// NumericField — the pure validator
// ===========================================================================

#[test]
fn numericfield_rejects_out_of_range_without_clamping() {
    let f = field_0_10();
    assert_eq!(f.validate(5.0), Ok(5.0));
    assert_eq!(f.validate(0.0), Ok(0.0)); // inclusive
    assert_eq!(f.validate(10.0), Ok(10.0)); // inclusive
                                            // The whole point: out-of-range is an Err carrying the offending value,
                                            // NOT a silently clamped Ok(10.0) / Ok(0.0).
    assert_eq!(
        f.validate(11.0),
        Err(NumericError::Above {
            value: 11.0,
            max: 10.0
        })
    );
    assert_eq!(
        f.validate(-1.0),
        Err(NumericError::Below {
            value: -1.0,
            min: 0.0
        })
    );
}

#[test]
fn numericfield_rejects_nonfinite() {
    let f = field_0_10();
    assert_eq!(f.validate(f64::NAN), Err(NumericError::NonFinite));
    assert_eq!(f.validate(f64::INFINITY), Err(NumericError::NonFinite));
    assert_eq!(f.validate(f64::NEG_INFINITY), Err(NumericError::NonFinite));
}

#[test]
fn numericfield_parse_distinguishes_empty_junk_nonfinite_and_range() {
    let f = field_0_10();
    assert_eq!(f.parse(""), Err(NumericError::Empty));
    assert_eq!(f.parse("   "), Err(NumericError::Empty));
    assert_eq!(f.parse("abc"), Err(NumericError::Unparseable));
    assert_eq!(f.parse("inf"), Err(NumericError::NonFinite));
    assert_eq!(f.parse("7.5"), Ok(7.5));
    assert_eq!(
        f.parse("99"),
        Err(NumericError::Above {
            value: 99.0,
            max: 10.0
        })
    );
}

#[test]
fn numericfield_unbounded_accepts_any_finite() {
    let f = NumericField::new();
    assert_eq!(f.validate(1e300), Ok(1e300));
    assert_eq!(f.validate(-1e300), Ok(-1e300));
    assert_eq!(f.validate(f64::NAN), Err(NumericError::NonFinite));
}

// ===========================================================================
// NumericInput — the controlled editor
// ===========================================================================

#[test]
fn numeric_input_begin_seeds_buffer_and_opens_one_episode() {
    let mut input = NumericInput::new(field_0_10());
    assert!(!input.is_editing());
    assert_eq!(
        input.begin(EditId(1), 5.0),
        Some(NumericEdit::Begin { id: EditId(1) })
    );
    assert!(input.is_editing());
    assert_eq!(input.buffer(), "5");
    // A second begin while editing is rejected; the first episode is untouched.
    assert_eq!(input.begin(EditId(2), 9.0), None);
    assert!(input.is_editing());
    assert_eq!(input.buffer(), "5");
}

#[test]
fn numeric_input_preview_is_not_a_commit() {
    let mut input = NumericInput::new(field_0_10());
    input.begin(EditId(1), 5.0);
    assert_eq!(
        input.set_text("7"),
        Some(NumericEdit::Preview {
            id: EditId(1),
            value: 7.0
        })
    );
    // Still editing — a preview is a proposal, not a decision. Nothing committed.
    assert!(input.is_editing());
}

#[test]
fn numeric_input_commits_only_a_valid_value() {
    let mut input = NumericInput::new(field_0_10());
    input.begin(EditId(1), 5.0);
    input.set_text("7.5");
    assert_eq!(
        input.commit(),
        Some(NumericEdit::Commit {
            id: EditId(1),
            value: 7.5
        })
    );
    assert!(!input.is_editing());
    // Exactly one commit: a second finds nothing open.
    assert_eq!(input.commit(), None);
}

#[test]
fn numeric_input_out_of_range_is_refused_never_clamped() {
    let mut input = NumericInput::new(field_0_10());
    input.begin(EditId(1), 5.0);
    // Typing an out-of-range value previews Invalid, not a clamped Preview.
    assert_eq!(
        input.set_text("99"),
        Some(NumericEdit::Invalid {
            id: EditId(1),
            error: NumericError::Above {
                value: 99.0,
                max: 10.0
            }
        })
    );
    // Committing it REFUSES — no Commit(10.0), and the episode stays open.
    assert_eq!(
        input.commit(),
        Some(NumericEdit::Invalid {
            id: EditId(1),
            error: NumericError::Above {
                value: 99.0,
                max: 10.0
            }
        })
    );
    assert!(input.is_editing());
    // The consumer can recover by fixing the value and committing cleanly.
    input.set_text("8");
    assert_eq!(
        input.commit(),
        Some(NumericEdit::Commit {
            id: EditId(1),
            value: 8.0
        })
    );
    assert!(!input.is_editing());
}

#[test]
fn numeric_input_cancel_makes_no_authoritative_change() {
    let mut input = NumericInput::new(field_0_10());
    input.begin(EditId(1), 5.0);
    input.set_text("7");
    assert_eq!(
        input.cancel(CancelReason::Escape),
        Some(NumericEdit::Cancel {
            id: EditId(1),
            reason: CancelReason::Escape
        })
    );
    assert!(!input.is_editing());
    // A late commit after cancel commits nothing.
    assert_eq!(input.commit(), None);
}

#[test]
fn numeric_input_step_shares_the_typed_vocabulary_and_refuses_overflow() {
    // The stepper (pointer/arrow path) emits the SAME NumericEdit through the
    // SAME NumericField as typed text — and a step past the bound is refused,
    // never silently clamped to the boundary.
    let mut input = NumericInput::new(field_0_10().with_step(1.0));
    input.begin(EditId(1), 9.0);
    assert_eq!(
        input.step(1),
        Some(NumericEdit::Preview {
            id: EditId(1),
            value: 10.0
        })
    );
    assert_eq!(input.buffer(), "10");
    // 10 + 1 = 11 is out of range: Invalid, not clamped back to 10.
    assert_eq!(
        input.step(1),
        Some(NumericEdit::Invalid {
            id: EditId(1),
            error: NumericError::Above {
                value: 11.0,
                max: 10.0
            }
        })
    );
    assert!(input.is_editing());
}

#[test]
fn numeric_input_step_down_past_min_is_refused() {
    let mut input = NumericInput::new(field_0_10().with_step(2.0));
    input.begin(EditId(1), 1.0);
    assert_eq!(
        input.step(-1),
        Some(NumericEdit::Invalid {
            id: EditId(1),
            error: NumericError::Below {
                value: -1.0,
                min: 0.0
            }
        })
    );
}

#[test]
fn numeric_input_actions_before_begin_are_inert() {
    let mut input = NumericInput::new(field_0_10());
    assert_eq!(input.set_text("5"), None);
    assert_eq!(input.step(1), None);
    assert_eq!(input.commit(), None);
    assert_eq!(input.cancel(CancelReason::Escape), None);
    assert!(!input.is_editing());
}
