//! A deliberately tiny focus system.
//!
//! This is **not** a DOM/event router. It is just enough to answer "which widget
//! owns the keyboard right now?", to cycle focus (Tab / Shift-Tab) and to let a
//! modal capture focus and restore the previous owner afterwards.
//!
//! Applications own their own event dispatch; the ring only tracks identity.

/// Identifies a focusable widget within an application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FocusId(pub u64);

/// A small ordered set of focusable ids with a current owner and a modal stack.
///
/// * [`FocusRing::next`] / [`FocusRing::prev`] cycle with wraparound.
/// * [`FocusRing::capture`] remembers the current owner (modal open).
/// * [`FocusRing::release`] restores it (modal close).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FocusRing {
    ring: Vec<FocusId>,
    current: usize,
    stack: Vec<usize>,
}

impl FocusRing {
    /// Creates a ring over `ids` (order is focus order). Duplicates are ignored.
    pub fn new(ids: impl IntoIterator<Item = FocusId>) -> Self {
        let mut ring = Vec::new();
        for id in ids {
            if !ring.contains(&id) {
                ring.push(id);
            }
        }
        Self {
            ring,
            current: 0,
            stack: Vec::new(),
        }
    }

    /// The currently focused id, if the ring has any members.
    pub fn current(&self) -> Option<FocusId> {
        self.ring.get(self.current).copied()
    }

    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    /// Focuses a specific known id. Returns `false` if the id is not in the ring.
    pub fn set(&mut self, id: FocusId) -> bool {
        match self.ring.iter().position(|&x| x == id) {
            Some(i) => {
                self.current = i;
                true
            }
            None => false,
        }
    }

    pub fn contains(&self, id: FocusId) -> bool {
        self.ring.contains(&id)
    }

    /// Advances to the next member (wraparound). Returns the new owner.
    pub fn focus_next(&mut self) -> Option<FocusId> {
        if self.ring.is_empty() {
            return None;
        }
        self.current = (self.current + 1) % self.ring.len();
        self.current()
    }

    /// Moves to the previous member (wraparound). Returns the new owner.
    pub fn focus_prev(&mut self) -> Option<FocusId> {
        if self.ring.is_empty() {
            return None;
        }
        self.current = (self.current + self.ring.len() - 1) % self.ring.len();
        self.current()
    }

    /// Saves the current owner (e.g. when opening a modal). Nestable.
    pub fn capture(&mut self) {
        self.stack.push(self.current);
    }

    /// Restores the owner saved by the matching [`FocusRing::capture`].
    ///
    /// Extra releases are ignored, so a modal can always release safely.
    pub fn release(&mut self) -> Option<FocusId> {
        if let Some(prev) = self.stack.pop() {
            self.current = prev.min(self.ring.len().saturating_sub(1));
        }
        self.current()
    }

    /// True while at least one capture is outstanding.
    pub fn is_captured(&self) -> bool {
        !self.stack.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_with_wraparound() {
        let mut f = FocusRing::new([FocusId(1), FocusId(2), FocusId(3)]);
        assert_eq!(f.current(), Some(FocusId(1)));
        assert_eq!(f.focus_next(), Some(FocusId(2)));
        assert_eq!(f.focus_next(), Some(FocusId(3)));
        assert_eq!(f.focus_next(), Some(FocusId(1)));
        assert_eq!(f.focus_prev(), Some(FocusId(3)));
    }

    #[test]
    fn set_only_known_ids() {
        let mut f = FocusRing::new([FocusId(7)]);
        assert!(f.set(FocusId(7)));
        assert!(!f.set(FocusId(99)));
        assert_eq!(f.current(), Some(FocusId(7)));
    }

    #[test]
    fn capture_and_release_restores_previous_focus() {
        let mut f = FocusRing::new([FocusId(1), FocusId(2)]);
        f.set(FocusId(2));
        f.capture();
        assert!(f.is_captured());
        f.set(FocusId(1)); // modal moves focus
        assert_eq!(f.release(), Some(FocusId(2)));
        assert!(!f.is_captured());
    }

    #[test]
    fn nested_capture_restores_in_order() {
        let mut f = FocusRing::new([FocusId(1), FocusId(2), FocusId(3)]);
        f.set(FocusId(1));
        f.capture();
        f.set(FocusId(2));
        f.capture();
        f.set(FocusId(3));
        assert_eq!(f.release(), Some(FocusId(2)));
        assert_eq!(f.release(), Some(FocusId(1)));
        // Extra release is harmless.
        assert_eq!(f.release(), Some(FocusId(1)));
    }

    #[test]
    fn empty_ring_is_safe() {
        let mut f = FocusRing::default();
        assert!(f.is_empty());
        assert_eq!(f.current(), None);
        assert_eq!(f.focus_next(), None);
        assert_eq!(f.focus_prev(), None);
    }

    #[test]
    fn duplicate_ids_are_ignored() {
        let f = FocusRing::new([FocusId(1), FocusId(1), FocusId(2)]);
        assert_eq!(f.ring.len(), 2);
    }
}
