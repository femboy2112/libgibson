//! Finding B (issue #137): a [`Compound`] is the PARTIAL inverse of an admitted
//! transition — not a magic one. The old implementation claimed universal
//! atomicity ("the model is left exactly as it was") while discarding every
//! compensation error with `let _ = revert(..)`, so a half-rolled-back, corrupted
//! model was indistinguishable from a clean rollback. These falsifiers nail the
//! honest contract (Categorical Laws L5 "transaction truth" / L7 "no silent
//! erasure"):
//!
//! * a sub-command failure whose rollback is CLEAN reports [`CompoundError::SubCommand`]
//!   and the model is restored;
//! * a sub-command failure whose rollback ITSELF fails reports
//!   [`CompoundError::RollbackFailed`], names every compensation that failed, and
//!   admits the model is partial — it is never dressed up as success.
//!
//! Every assertion here is expressible ONLY against the fixed API: the old
//! `Compound` returned a bare `C::Error` with no `RollbackFailed` to return, so
//! the swallowed-error path had no name. That is the red-first proof — the old
//! type literally could not say what actually happened.

use gibson::edit::{Compound, CompoundError, Reversible, UndoLog};

// --- Fixture: a running total whose steps can be armed to fail independently ---

#[derive(Debug, Clone, PartialEq, Eq)]
struct Fail(&'static str);

/// One step over an `i64` model, with fully independent apply/revert outcomes so
/// a test can arm a *compensation* to fail without touching the forward path.
#[derive(Debug, Clone)]
struct Step {
    delta: i64,
    /// `apply` returns `Err` and (unless `faithless`) mutates nothing.
    fail_apply: bool,
    /// `revert` returns `Err` and mutates nothing — a refusing compensation.
    fail_revert: bool,
    /// Violates the Reversible precondition: `apply` mutates AND returns `Err`.
    faithless: bool,
    tag: &'static str,
}

impl Step {
    /// A well-behaved step: `apply` adds, `revert` subtracts, both infallible.
    fn add(delta: i64) -> Self {
        Self {
            delta,
            fail_apply: false,
            fail_revert: false,
            faithless: false,
            tag: "add",
        }
    }
    /// Applies fine, but its compensation (revert) refuses — the op that exposes
    /// the swallowed-error bug when it sits in a rollback.
    fn refuse_revert(delta: i64, tag: &'static str) -> Self {
        Self {
            delta,
            fail_apply: false,
            fail_revert: true,
            faithless: false,
            tag,
        }
    }
    /// A trigger: `apply` refuses and touches nothing (honours the precondition).
    fn fail_apply(tag: &'static str) -> Self {
        Self {
            delta: 0,
            fail_apply: true,
            fail_revert: false,
            faithless: false,
            tag,
        }
    }
    /// Faithless: `apply` mutates and THEN errors — violates the precondition.
    fn faithless(delta: i64, tag: &'static str) -> Self {
        Self {
            delta,
            fail_apply: true,
            fail_revert: false,
            faithless: true,
            tag,
        }
    }
}

impl Reversible for Step {
    type Model = i64;
    type Error = Fail;

    fn apply(&self, m: &mut i64) -> Result<(), Fail> {
        if self.faithless {
            *m += self.delta; // mutate THEN fail — the precondition violation
            return Err(Fail(self.tag));
        }
        if self.fail_apply {
            return Err(Fail(self.tag));
        }
        *m += self.delta;
        Ok(())
    }

    fn revert(&self, m: &mut i64) -> Result<(), Fail> {
        if self.fail_revert {
            return Err(Fail(self.tag)); // a compensation that refuses
        }
        *m -= self.delta;
        Ok(())
    }
}

// ============================================================================
// THE counterexample: apply fails, the rollback ITSELF fails
// ============================================================================

#[test]
fn apply_rollback_failure_is_surfaced_not_swallowed() {
    // A (index 0) applies: 0 -> 1, Ok. B (index 1) refuses its apply, touching
    // nothing. The compound rolls back A — and A's compensation REFUSES. The old
    // code did `let _ = A.revert(..)`, dropped that error, and returned a bare
    // Err(B) as if the model were clean. It was not: m == 1. The fix reports
    // exactly which compensation failed and admits the model is partial.
    let group = Compound::new(vec![
        Step::refuse_revert(1, "A-revert"),
        Step::fail_apply("B-apply"),
    ]);
    let mut m = 0i64;

    let result = group.apply(&mut m);

    assert_eq!(
        result,
        Err(CompoundError::RollbackFailed {
            trigger_index: 1,
            trigger_error: Fail("B-apply"),
            failures: vec![(0, Fail("A-revert"))],
        })
    );
    // The model is genuinely partial (A landed, A's rollback refused). The point
    // is not that the fix magically restores it — it cannot — but that the caller
    // is TOLD, instead of handed a corrupted model wearing a success sticker.
    assert_eq!(m, 1);
}

#[test]
fn clean_rollback_reports_subcommand_and_restores_state() {
    // Same shape, but the applied sub-commands revert cleanly. This is the
    // atomicity-held case: model fully restored, failure reported as SubCommand
    // (NOT RollbackFailed — no compensation failed).
    let group = Compound::new(vec![
        Step::add(5),
        Step::add(7),
        Step::fail_apply("C-apply"),
    ]);
    let mut m = 100i64;

    let result = group.apply(&mut m);

    assert_eq!(
        result,
        Err(CompoundError::SubCommand {
            index: 2,
            error: Fail("C-apply")
        })
    );
    assert_eq!(m, 100); // exactly as it was
}

#[test]
fn every_failed_compensation_is_collected_in_attempt_order() {
    // Two applied sub-commands both refuse their rollback. Both failures must be
    // reported — newest-first, the order the rollback attempted them — not just
    // the first or the last. A single-error type could never carry this.
    let group = Compound::new(vec![
        Step::refuse_revert(1, "A"),
        Step::refuse_revert(2, "B"),
        Step::fail_apply("C"),
    ]);
    let mut m = 0i64;

    let result = group.apply(&mut m);

    assert_eq!(
        result,
        Err(CompoundError::RollbackFailed {
            trigger_index: 2,
            trigger_error: Fail("C"),
            // rollback runs newest-first: B (index 1) then A (index 0).
            failures: vec![(1, Fail("B")), (0, Fail("A"))],
        })
    );
    assert_eq!(m, 3); // 0 +1 +2, neither undone — and the error says so
}

// ============================================================================
// The revert direction is symmetric: a failed un-revert re-applies and reports
// ============================================================================

#[test]
fn revert_rollback_failure_is_surfaced_not_swallowed() {
    // Revert walks reverse (index 1 then 0). Q (index 1) reverts cleanly
    // (m -= 3), then P (index 0) refuses its revert — the trigger. The compound
    // re-applies the tail it had already reverted (Q), and Q's RE-APPLY refuses.
    // Old code swallowed that re-apply error; the fix reports it.
    let p = Step::refuse_revert(0, "P-revert"); // revert refuses (delta 0)
                                                // Q: reverts fine, but its apply (the re-apply during rollback) refuses.
    let q = Step {
        delta: 3,
        fail_apply: true,
        fail_revert: false,
        faithless: false,
        tag: "Q-reapply",
    };
    let group = Compound::new(vec![p, q]);
    let mut m = 10i64;

    let result = group.revert(&mut m);

    assert_eq!(
        result,
        Err(CompoundError::RollbackFailed {
            trigger_index: 0,
            trigger_error: Fail("P-revert"),
            failures: vec![(1, Fail("Q-reapply"))],
        })
    );
    assert_eq!(m, 7); // Q reverted (10 -> 7), P's revert refused, Q re-apply refused
}

// ============================================================================
// The editor path: a dirty rollback propagates intact through UndoLog::undo
// ============================================================================

#[test]
fn undolog_undo_surfaces_compound_rollback_failure_and_keeps_the_entry() {
    // A compound recorded in an UndoLog is the drag the editor will undo. If its
    // revert rolls back dirtily, undo() must hand back the RollbackFailed intact
    // AND keep the entry on `done` (a failed undo is the caller's to resolve, not
    // ours to quietly discard).
    let p = Step::refuse_revert(0, "P-revert");
    let q = Step {
        delta: 3,
        fail_apply: true,
        fail_revert: false,
        faithless: false,
        tag: "Q-reapply",
    };
    let group = Compound::new(vec![p, q]);

    let mut log: UndoLog<Compound<Step>> = UndoLog::new();
    log.record(group);
    assert!(log.can_undo());

    let mut m = 10i64;
    let undone = log.undo(&mut m);

    assert_eq!(
        undone,
        Some(Err(CompoundError::RollbackFailed {
            trigger_index: 0,
            trigger_error: Fail("P-revert"),
            failures: vec![(1, Fail("Q-reapply"))],
        }))
    );
    // The failed undo did not silently consume history.
    assert!(log.can_undo());
    assert!(!log.can_redo());
}

// ============================================================================
// BOUNDARY: what a compound CANNOT promise over a faithless Reversible
// ============================================================================

#[test]
fn faithless_apply_defeats_atomicity_and_is_documented_not_detected() {
    // A step that mutates AND returns Err violates the Reversible precondition.
    // The compound trusts `Err` to mean "nothing happened", so it rolls back only
    // the CLEANLY-applied prefix [..i] — the faithless step's own partial mutation
    // is invisible to it. Result: a "clean" SubCommand is reported, yet the model
    // is NOT restored. This test pins that boundary so nobody mistakes the
    // conditional guarantee for an unconditional one: the escape hatch is to apply
    // against a clone when the Reversible cannot promise all-or-nothing.
    let group = Compound::new(vec![Step::add(10), Step::faithless(5, "M")]);
    let mut m = 0i64;

    let result = group.apply(&mut m);

    // Reported as a clean rollback (no compensation failed on the [..i] prefix)...
    assert_eq!(
        result,
        Err(CompoundError::SubCommand {
            index: 1,
            error: Fail("M")
        })
    );
    // ...yet the model is NOT back to 0: the faithless step's +5 survived. The
    // compound cannot detect this; the precondition is the contract that forbids it.
    assert_eq!(m, 5);
}
