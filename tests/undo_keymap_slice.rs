//! R4-D falsifiers (issue #137): compound commands + an undo/redo keymap.
//!
//! Two load-bearing guarantees:
//!
//! * **A compound is one reversible unit.** Grouping N sub-edits (e.g. every
//!   step of a drag) into a [`Compound`] makes them apply forward in order and
//!   revert in REVERSE order, and — recorded in an [`UndoLog`] — collapse to a
//!   SINGLE undo/redo. A compound whose apply fails partway **rolls back** what
//!   it already applied: the model is left exactly as it was, never half-edited.
//! * **The keymap translates keys to intent, nothing more.** [`UndoKeymap`]
//!   classifies a [`KeyEvent`] into [`UndoIntent`] without performing anything,
//!   and it is robust to the terminal ambiguity where Shift+Z arrives either as
//!   `Char('z')`+SHIFT or as `Char('Z')` with no flag.

use gibson::edit::{Compound, Reversible, UndoIntent, UndoKeymap, UndoLog};
use gibson::{KeyCode, KeyEvent, KeyModifiers};

// --- Fixture: a tiny consumer-owned model + reversible ops --------------------

#[derive(Debug, Clone, PartialEq)]
struct Doc {
    chars: Vec<char>,
}

#[derive(Debug, Clone, PartialEq)]
struct EditFail;

#[derive(Debug, Clone, PartialEq)]
enum Op {
    Push(char),
    FailOnApply,
}

impl Reversible for Op {
    type Model = Doc;
    type Error = EditFail;

    fn apply(&self, model: &mut Doc) -> Result<(), EditFail> {
        match self {
            Op::Push(c) => {
                model.chars.push(*c);
                Ok(())
            }
            Op::FailOnApply => Err(EditFail),
        }
    }

    fn revert(&self, model: &mut Doc) -> Result<(), EditFail> {
        match self {
            // LIFO: compound reverts in reverse order, so the last push pops first.
            Op::Push(_) => {
                model.chars.pop();
                Ok(())
            }
            Op::FailOnApply => Ok(()),
        }
    }
}

fn doc(s: &str) -> Doc {
    Doc {
        chars: s.chars().collect(),
    }
}

// ============================================================================
// Compound — forward/reverse order, atomic rollback, single-unit undo
// ============================================================================

#[test]
fn compound_applies_forward_and_reverts_in_reverse() {
    let group = Compound::new(vec![Op::Push('a'), Op::Push('b'), Op::Push('c')]);
    let mut model = doc("");
    assert_eq!(group.apply(&mut model), Ok(()));
    assert_eq!(model, doc("abc"));
    // Revert walks the group backwards: pop c, then b, then a.
    assert_eq!(group.revert(&mut model), Ok(()));
    assert_eq!(model, doc(""));
}

#[test]
fn compound_collapses_into_a_single_undolog_entry() {
    // A drag that produced two sub-edits must undo/redo as ONE step.
    let group = Compound::new(vec![Op::Push('x'), Op::Push('y')]);
    let mut model = doc("");
    group.apply(&mut model).unwrap();
    assert_eq!(model, doc("xy"));

    let mut log: UndoLog<Compound<Op>> = UndoLog::new();
    log.record(group);
    assert!(log.can_undo());
    assert!(!log.can_redo());

    // One undo reverts the whole group.
    assert_eq!(log.undo(&mut model), Some(Ok(())));
    assert_eq!(model, doc(""));
    assert!(!log.can_undo());
    assert!(log.can_redo());

    // One redo re-applies the whole group.
    assert_eq!(log.redo(&mut model), Some(Ok(())));
    assert_eq!(model, doc("xy"));
}

#[test]
fn compound_apply_is_atomic_and_restores_prior_state_on_failure() {
    // Start from a NON-empty model to prove rollback restores the exact prior
    // state, not merely "empty".
    let group = Compound::new(vec![Op::Push('a'), Op::Push('b'), Op::FailOnApply]);
    let mut model = doc("z");
    assert_eq!(group.apply(&mut model), Err(EditFail));
    // a and b were applied then rolled back; the model is untouched.
    assert_eq!(model, doc("z"));
}

#[test]
fn empty_compound_is_an_identity() {
    let group: Compound<Op> = Compound::new(vec![]);
    assert!(group.is_empty());
    assert_eq!(group.len(), 0);
    let mut model = doc("hello");
    assert_eq!(group.apply(&mut model), Ok(()));
    assert_eq!(group.revert(&mut model), Ok(()));
    assert_eq!(model, doc("hello"));
}

#[test]
fn compound_exposes_len_and_commands() {
    let group = Compound::new(vec![Op::Push('a'), Op::Push('b')]);
    assert!(!group.is_empty());
    assert_eq!(group.len(), 2);
    assert_eq!(group.commands(), [Op::Push('a'), Op::Push('b')].as_slice());
}

// ============================================================================
// UndoKeymap — key -> intent translation
// ============================================================================

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

#[test]
fn standard_keymap_classifies_undo_and_redo() {
    let km = UndoKeymap::standard();
    // Ctrl-Z = undo.
    assert_eq!(km.classify(ctrl('z')), Some(UndoIntent::Undo));
    // Ctrl-Shift-Z = redo (shift flag form).
    assert_eq!(
        km.classify(KeyEvent::new(
            KeyCode::Char('z'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT
        )),
        Some(UndoIntent::Redo)
    );
    // Ctrl-Y = redo, regardless of shift.
    assert_eq!(km.classify(ctrl('y')), Some(UndoIntent::Redo));
    assert_eq!(km.classify(ctrl('Y')), Some(UndoIntent::Redo));
}

#[test]
fn standard_keymap_treats_uppercase_letter_as_shift() {
    // Terminals that encode Shift as the uppercase char (no SHIFT flag) must
    // still read Ctrl-Shift-Z as redo, and plain Ctrl-z as undo.
    let km = UndoKeymap::standard();
    assert_eq!(km.classify(ctrl('Z')), Some(UndoIntent::Redo));
    assert_eq!(km.classify(ctrl('z')), Some(UndoIntent::Undo));
}

#[test]
fn standard_keymap_rejects_non_bindings() {
    let km = UndoKeymap::standard();
    // No Ctrl -> not an undo chord.
    assert_eq!(
        km.classify(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::empty())),
        None
    );
    // Alt held disqualifies the chord.
    assert_eq!(
        km.classify(KeyEvent::new(
            KeyCode::Char('z'),
            KeyModifiers::CONTROL | KeyModifiers::ALT
        )),
        None
    );
    // Wrong letter.
    assert_eq!(km.classify(ctrl('a')), None);
    // Not a character key at all.
    assert_eq!(
        km.classify(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL)),
        None
    );
}
