//! An identity-keyed selection of shapes.
//!
//! A [`Selection`] records *which* shapes are selected by their `ShapeId` —
//! never by geometry — so a selected shape stays selected through any
//! projection, transform, or edit. Membership is a
//! set, but the order of insertion is preserved (a stable list / z-order the UI
//! can rely on), and the most-recently-inserted still-present id is the
//! "primary": the active target an editor acts on first.
//!
//! This type owns no shapes and no model; it is a pure set of identities. The
//! consumer maps an id back to its authoritative shape.

use crate::shape::ShapeId;

/// An insertion-ordered, identity-keyed set of selected shapes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Selection {
    ids: Vec<ShapeId>,
}

impl Selection {
    /// An empty selection.
    pub fn new() -> Self {
        Self { ids: Vec::new() }
    }

    /// Selects `id`. Returns `true` if it was newly added, `false` if it was
    /// already selected (a no-op that does not disturb the existing order).
    pub fn insert(&mut self, id: ShapeId) -> bool {
        if self.ids.contains(&id) {
            false
        } else {
            self.ids.push(id);
            true
        }
    }

    /// Deselects `id`. Returns `true` if it was present (and the order of the
    /// remaining ids is preserved), `false` if it was not selected.
    pub fn remove(&mut self, id: ShapeId) -> bool {
        if let Some(pos) = self.ids.iter().position(|&x| x == id) {
            self.ids.remove(pos);
            true
        } else {
            false
        }
    }

    /// Flips `id`'s membership, returning its membership *after* the flip:
    /// `true` if it is now selected, `false` if it is now deselected.
    pub fn toggle(&mut self, id: ShapeId) -> bool {
        if self.remove(id) {
            false
        } else {
            self.ids.push(id);
            true
        }
    }

    /// Whether `id` is currently selected.
    pub fn contains(&self, id: ShapeId) -> bool {
        self.ids.contains(&id)
    }

    /// Deselects everything.
    pub fn clear(&mut self) {
        self.ids.clear();
    }

    /// The number of selected shapes.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Whether nothing is selected.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// The selected ids in insertion order.
    pub fn ids(&self) -> &[ShapeId] {
        &self.ids
    }

    /// The primary selection: the most-recently-inserted still-present id, or
    /// `None` when the selection is empty.
    pub fn primary(&self) -> Option<ShapeId> {
        self.ids.last().copied()
    }
}
