//! R4-C continuation (issue #132, the R4 "manipulate selections" goal): an
//! identity-keyed selection set.
//!
//! A [`Selection`] tracks which shapes are selected, by [`ShapeId`] — never by
//! geometry, so a selected shape stays selected through any projection or edit.
//! Insertion is idempotent and order-preserving (so a stable, repeatable z-order
//! / list order survives), removal keeps the order of what remains, and the
//! most-recently-inserted still-present id is the "primary" (the active target
//! an editor acts on first).

use gibson::selection::Selection;
use gibson::ShapeId;

#[test]
fn insert_is_idempotent_and_reports_novelty() {
    let mut sel = Selection::new();
    assert!(sel.is_empty());
    assert!(sel.insert(ShapeId(1))); // newly added
    assert!(!sel.insert(ShapeId(1))); // already present, no-op
    assert_eq!(sel.len(), 1);
}

#[test]
fn preserves_insertion_order() {
    let mut sel = Selection::new();
    sel.insert(ShapeId(3));
    sel.insert(ShapeId(1));
    sel.insert(ShapeId(2));
    sel.insert(ShapeId(1)); // duplicate must not reorder
    assert_eq!(sel.ids(), &[ShapeId(3), ShapeId(1), ShapeId(2)]);
}

#[test]
fn remove_reports_presence_and_keeps_order_of_the_rest() {
    let mut sel = Selection::new();
    sel.insert(ShapeId(3));
    sel.insert(ShapeId(1));
    sel.insert(ShapeId(2));
    // Removing the middle element leaves the rest in order.
    assert!(sel.remove(ShapeId(1)));
    assert_eq!(sel.ids(), &[ShapeId(3), ShapeId(2)]);
    // Removing an absent id is a reported no-op.
    assert!(!sel.remove(ShapeId(9)));
    assert_eq!(sel.ids(), &[ShapeId(3), ShapeId(2)]);
}

#[test]
fn toggle_adds_then_removes_and_reports_membership() {
    let mut sel = Selection::new();
    // toggle returns membership AFTER the flip.
    assert!(sel.toggle(ShapeId(5))); // now selected
    assert!(sel.contains(ShapeId(5)));
    assert!(!sel.toggle(ShapeId(5))); // now deselected
    assert!(!sel.contains(ShapeId(5)));
    assert!(sel.is_empty());
}

#[test]
fn contains_and_clear() {
    let mut sel = Selection::new();
    sel.insert(ShapeId(1));
    sel.insert(ShapeId(2));
    assert!(sel.contains(ShapeId(2)));
    assert!(!sel.contains(ShapeId(7)));
    sel.clear();
    assert!(sel.is_empty());
    assert_eq!(sel.len(), 0);
    assert_eq!(sel.primary(), None);
}

#[test]
fn primary_is_the_most_recently_inserted_still_present() {
    let mut sel = Selection::new();
    assert_eq!(sel.primary(), None);
    sel.insert(ShapeId(1));
    sel.insert(ShapeId(2));
    assert_eq!(sel.primary(), Some(ShapeId(2)));
    // Removing the primary promotes the previous one.
    sel.remove(ShapeId(2));
    assert_eq!(sel.primary(), Some(ShapeId(1)));
}

#[test]
fn len_and_is_empty_track_membership() {
    let mut sel = Selection::new();
    assert_eq!(sel.len(), 0);
    assert!(sel.is_empty());
    sel.insert(ShapeId(1));
    sel.insert(ShapeId(2));
    assert_eq!(sel.len(), 2);
    assert!(!sel.is_empty());
}
