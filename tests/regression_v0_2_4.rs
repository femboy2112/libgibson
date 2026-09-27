//! Regression coverage for the v0.2.4 viewport + focus foundations patch.
//!   #39 — `ViewportState::visible_range` / `ensure_visible` / `scroll_to_item`
//!         windowing helpers (partial-item inclusion, zero-height guards,
//!         minimal-scroll contract).
//!   #40 — `FocusRing::remove` incremental eviction with focus transfer to a
//!         surviving neighbour and capture-stack reindexing.

use gibson::{FocusId, FocusRing, ViewportState};

// ── #39 ViewportState windowing ──────────────────────────────────────────────

#[test]
fn issue_39_visible_range_includes_partial_items_at_both_edges() {
    // 20 items, 3 cells each, a 10-cell window at offset_y = 5.
    // Item 1 spans cells [3,6) — offset 5 clips its top; item 4 spans [12,15) —
    // the last visible row (5+10-1 = 14) clips its bottom. Both partial items
    // must be included, so the range is 1..5, not the floor-only 1..4.
    let v = ViewportState::with_offset(0, 5);
    assert_eq!(v.visible_range(20, 3, 10), 1..5);
}

#[test]
fn issue_39_visible_range_guards_degenerate_inputs() {
    let v = ViewportState::with_offset(0, 5);
    // Zero item height must not divide by zero — empty range, no panic.
    assert_eq!(v.visible_range(50, 0, 10), 0..0);
    // Zero items and zero view height are likewise empty.
    assert_eq!(v.visible_range(0, 3, 10), 0..0);
    assert_eq!(v.visible_range(50, 3, 0), 0..0);
    // An offset past the content end yields an empty range at the tail, clamped
    // to total_items (here 4 items of height 3 → content is 12 cells tall).
    let past = ViewportState::with_offset(0, 999);
    assert_eq!(past.visible_range(4, 3, 10), 4..4);
}

#[test]
fn issue_39_ensure_visible_scrolls_minimally() {
    let mut v = ViewportState::new(); // offset_y = 0, window [0,10)
                                      // Item 6 spans [12,14): below the window → scroll down just enough (14-10=4).
    v.ensure_visible(6, 2, 10);
    assert_eq!(v.offset_y, 4);
    // Already fully visible in window [4,14) → no-op (not a recenter).
    v.ensure_visible(6, 2, 10);
    assert_eq!(v.offset_y, 4);
    // Item 1 spans [2,4): above the window → scroll up to reveal its top.
    v.ensure_visible(1, 2, 10);
    assert_eq!(v.offset_y, 2);
    // Zero item height is a no-op rather than a corrupt offset.
    v.ensure_visible(9, 0, 10);
    assert_eq!(v.offset_y, 2);
}

#[test]
fn issue_39_scroll_to_item_pins_to_top() {
    let mut v = ViewportState::new();
    v.scroll_to_item(7, 4); // item 7, 4 cells each → top at 28
    assert_eq!(v.offset_y, 28);
    // Zero item height leaves the camera untouched.
    v.scroll_to_item(3, 0);
    assert_eq!(v.offset_y, 28);
}

// ── #40 FocusRing::remove ─────────────────────────────────────────────────────

#[test]
fn issue_40_remove_before_focus_keeps_same_widget() {
    let mut f = FocusRing::new([FocusId(1), FocusId(2), FocusId(3), FocusId(4)]);
    assert!(f.set(FocusId(3))); // current index 2
    assert_eq!(f.remove(0), Some(FocusId(3))); // ring → [2,3,4], focus stays on id 3
    assert_eq!(f.current(), Some(FocusId(3)));
}

#[test]
fn issue_40_remove_focused_transfers_to_next_survivor() {
    let mut f = FocusRing::new([FocusId(1), FocusId(2), FocusId(3), FocusId(4)]);
    assert!(f.set(FocusId(2))); // current index 1
                                // Remove the focused slot: the next survivor (id 3) slides into it.
    assert_eq!(f.remove(1), Some(FocusId(3)));
    assert_eq!(f.current(), Some(FocusId(3)));
}

#[test]
fn issue_40_remove_focused_last_falls_back_to_new_last() {
    let mut f = FocusRing::new([FocusId(1), FocusId(2), FocusId(3)]);
    assert!(f.set(FocusId(3))); // current index 2 (last)
                                // Removing the last focused item must land on the new last, not lose focus.
    assert_eq!(f.remove(2), Some(FocusId(2)));
    assert_eq!(f.current(), Some(FocusId(2)));
}

#[test]
fn issue_40_remove_down_to_empty_unsets_focus() {
    let mut f = FocusRing::new([FocusId(1)]);
    assert_eq!(f.remove(0), None);
    assert!(f.is_empty());
    assert_eq!(f.current(), None);
    // Out-of-range removal on an empty ring is a harmless no-op.
    assert_eq!(f.remove(5), None);
}

#[test]
fn issue_40_remove_reindexes_capture_stack() {
    let mut f = FocusRing::new([FocusId(1), FocusId(2), FocusId(3)]);
    assert!(f.set(FocusId(2))); // current index 1
    f.capture(); // stack holds index 1 (id 2)
    assert!(f.set(FocusId(3))); // current index 2
                                // Remove id 1 (before the captured slot). Without stack reindexing, release
                                // would restore the wrong (now in-bounds but stale) slot.
    assert_eq!(f.remove(0), Some(FocusId(3))); // ring → [2,3], current id 3
                                               // The capture must have shifted 1 → 0 so release returns id 2, not id 1/3.
    assert_eq!(f.release(), Some(FocusId(2)));
    assert!(!f.is_captured());
}
