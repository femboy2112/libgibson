//! Large collections: virtualization + list selection, composed from primitives.
//!
//! Run: cargo run --example large_collection
//!
//! LibGibson deliberately ships **no** `VirtualList`/`List`/`Table` *widget* that
//! owns your data's lifecycle — that would be a retained-mode framework. Instead a
//! scrollable, selectable, filterable list over an arbitrarily large dataset is a
//! few lines of *your* app code over three primitives you already have:
//!
//!   * [`ViewportState`] — camera offset + the windowing math
//!     ([`ViewportState::visible_range`] tells you which items are on screen;
//!     [`ViewportState::ensure_visible`] / [`ViewportState::scroll_to_item`] scroll).
//!   * [`FocusRing`] — reused here as a *list-scoped selection cursor*, keyed by a
//!     **stable row identity** (`FocusId(row.id)`, not the row's shifting index).
//!     `focus_next`/`focus_prev` are up/down, `set` is home/end/page, and `remove`
//!     deletes a filtered row while transferring the selection to a surviving
//!     neighbour *by identity* — so the cursor is never silently lost.
//!   * [`Node::col`] — you build **only the visible rows** into the tree each frame,
//!     so the node count is the viewport height, not the dataset size.
//!
//! The key subtlety this demonstrates: key the selection ring by a **stable id**,
//! not the row's position. Positions shift when you filter; identities do not, so
//! `FocusRing::remove` lands the cursor on the right surviving row.
//!
//! This example drives that composition headlessly over 10,000 rows and prints the
//! visible window after each interaction. See `docs/UI_LAYER.md` ("Large
//! collections") for wiring it into a real render/input loop.

use gibson::{FocusId, FocusRing, Node, Style, ViewportState};

/// One row of application data with a stable identity.
struct Row {
    id: u64,
    label: String,
}

/// A scrollable, selectable list over `rows`. This is application code you would
/// write over the library's primitives — it is intentionally *not* in the library.
struct Collection {
    rows: Vec<Row>,
    item_height: u16,
    view_height: u16,
    scroll: ViewportState,
    /// Selection cursor scoped to this list, keyed by stable row identity.
    selection: FocusRing,
}

impl Collection {
    fn new(rows: Vec<Row>, view_height: u16) -> Self {
        let selection = FocusRing::new(rows.iter().map(|r| FocusId(r.id)));
        Self {
            rows,
            item_height: 1,
            view_height,
            scroll: ViewportState::new(),
            selection,
        }
    }

    /// The stable identity of the selected row, if any.
    fn selected_id(&self) -> Option<u64> {
        self.selection.current().map(|FocusId(id)| id)
    }

    /// The current Vec position of the selected row, found by identity. O(n) here
    /// for clarity; a real app with huge lists would keep an id -> index map.
    fn selected_pos(&self) -> Option<usize> {
        let id = self.selected_id()?;
        self.rows.iter().position(|r| r.id == id)
    }

    /// The row positions at least partially on screen (edge-clipped rows included).
    /// The whole point of virtualization: only these get built into the node tree.
    fn visible(&self) -> std::ops::Range<usize> {
        self.scroll
            .visible_range(self.rows.len(), self.item_height, self.view_height)
    }

    fn move_down(&mut self) {
        self.selection.focus_next();
        self.reveal();
    }

    fn move_up(&mut self) {
        self.selection.focus_prev();
        self.reveal();
    }

    /// Select the row at Vec position `pos` (by looking up its stable id).
    fn goto(&mut self, pos: usize) {
        if let Some(row) = self.rows.get(pos) {
            self.selection.set(FocusId(row.id));
        }
        self.reveal();
    }

    fn home(&mut self) {
        self.goto(0);
    }

    fn end(&mut self) {
        if let Some(last) = self.rows.len().checked_sub(1) {
            self.goto(last);
        }
    }

    fn page_down(&mut self) {
        if let Some(p) = self.selected_pos() {
            let last = self.rows.len().saturating_sub(1);
            self.goto((p + self.view_height as usize).min(last));
        }
    }

    fn page_up(&mut self) {
        if let Some(p) = self.selected_pos() {
            self.goto(p.saturating_sub(self.view_height as usize));
        }
    }

    /// Scroll the minimum needed to keep the selected row on screen.
    fn reveal(&mut self) {
        if let Some(p) = self.selected_pos() {
            self.scroll
                .ensure_visible(p, self.item_height, self.view_height);
        }
    }

    /// Remove the row at position `pos` (e.g. it was filtered out of a search).
    /// The selection transfers to a surviving neighbour *by identity* rather than
    /// being lost, and the scroll offset is re-bounded against the shorter content.
    fn remove(&mut self, pos: usize) {
        if pos >= self.rows.len() {
            return;
        }
        self.rows.remove(pos);
        // The ring is in positional lockstep with `rows`, so removing the same
        // position from both keeps them aligned; `FocusRing::remove` moves the
        // cursor to the surviving neighbour.
        self.selection.remove(pos);
        let content_h = (self.rows.len() as u16).saturating_mul(self.item_height);
        self.scroll.clamp(0, content_h, 0, self.view_height);
        self.reveal();
    }

    /// Build **only** the visible rows into a node subtree. With `item_height == 1`
    /// the visible slice starts exactly at the scroll offset, so it renders
    /// top-aligned with no translation. (For taller rows, wrap in
    /// `Node::viewport(0, self.scroll.offset_y - range.start as i32 * item_height)`
    /// to absorb a partially-scrolled top row.) The returned tree holds at most
    /// `view_height` nodes regardless of how many rows exist.
    fn window(&self) -> Node {
        let range = self.visible();
        let sel = self.selected_id();
        let mut col = Node::col();
        for pos in range {
            let row = &self.rows[pos];
            let marker = if sel == Some(row.id) { "> " } else { "  " };
            col = col.child(Node::text(
                format!("{marker}{}", row.label),
                Style::default(),
            ));
        }
        col
    }
}

fn print_window(c: &Collection, label: &str) {
    let range = c.visible();
    let built = c.window().children.len();
    println!(
        "\n{label}: {} rows | window {:?} ({built} nodes built) | selected id {:?} @ pos {:?} | offset_y {}",
        c.rows.len(),
        range,
        c.selected_id(),
        c.selected_pos(),
        c.scroll.offset_y,
    );
    for pos in c.visible() {
        let row = &c.rows[pos];
        let marker = if c.selected_id() == Some(row.id) {
            '>'
        } else {
            ' '
        };
        println!("  {marker} {}", row.label);
    }
}

fn main() {
    let rows: Vec<Row> = (0..10_000)
        .map(|i| Row {
            id: i,
            label: format!("row #{i:05}"),
        })
        .collect();
    let mut c = Collection::new(rows, 8);

    print_window(&c, "initial");

    for _ in 0..2 {
        c.page_down();
    }
    print_window(&c, "after two PageDown");

    c.page_up();
    c.move_up();
    print_window(&c, "after PageUp then Up");

    c.end();
    print_window(&c, "after End (jump to last)");

    c.home();
    print_window(&c, "after Home");

    // Filter: drop the selected row and the four after it (as a search would),
    // demonstrating the selection lands on a surviving neighbour by identity.
    c.move_down();
    c.move_down();
    let victim_pos = c.selected_pos().unwrap();
    let victim_id = c.selected_id().unwrap();
    println!("\nfiltering out 5 rows starting at position {victim_pos} (id {victim_id})");
    for _ in 0..5 {
        c.remove(victim_pos);
    }
    print_window(&c, "after filtering 5 rows at the cursor");

    println!(
        "\nThroughout, the node tree held at most {} nodes for a {}-row dataset.",
        c.view_height,
        c.rows.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(n: u64, view: u16) -> Collection {
        let rows = (0..n)
            .map(|i| Row {
                id: i,
                label: format!("row {i}"),
            })
            .collect();
        Collection::new(rows, view)
    }

    #[test]
    fn window_is_bounded_by_viewport_not_dataset() {
        let c = sample(10_000, 8);
        assert!(c.visible().len() <= 8);
        assert_eq!(c.visible(), 0..8);
        // The materialized subtree is viewport-sized, not dataset-sized.
        assert!(c.window().children.len() <= 8);
    }

    #[test]
    fn selection_cursor_scrolls_to_stay_visible() {
        let mut c = sample(100, 8);
        for _ in 0..10 {
            c.move_down();
        }
        let pos = c.selected_pos().unwrap();
        assert_eq!(pos, 10);
        assert!(
            c.visible().contains(&pos),
            "selected pos {pos} must be within window {:?}",
            c.visible()
        );
    }

    #[test]
    fn end_and_home_jump_and_reveal() {
        let mut c = sample(100, 8);
        c.end();
        assert_eq!(c.selected_id(), Some(99));
        assert!(c.visible().contains(&99));
        c.home();
        assert_eq!(c.selected_id(), Some(0));
        assert_eq!(c.visible(), 0..8);
    }

    #[test]
    fn filter_remove_transfers_selection_to_surviving_neighbour_by_identity() {
        let mut c = sample(10, 8);
        c.move_down(); // select row id 1 at position 1
        assert_eq!(c.selected_id(), Some(1));
        c.remove(1); // filter that row out
                     // The cursor moves to the surviving neighbour (row id 2), which now sits
                     // at position 1 — selection preserved by identity, never lost.
        assert_eq!(c.selected_id(), Some(2));
        assert_eq!(c.selected_pos(), Some(1));
        assert_eq!(c.rows.len(), 9);
    }

    #[test]
    fn removing_the_last_selected_row_falls_back_within_bounds() {
        let mut c = sample(5, 8);
        c.end(); // select id 4 at position 4 (last)
        assert_eq!(c.selected_id(), Some(4));
        c.remove(4);
        // Falls back to the new last surviving row, never off the end.
        assert_eq!(c.selected_id(), Some(3));
        assert_eq!(c.rows.len(), 4);
    }
}
