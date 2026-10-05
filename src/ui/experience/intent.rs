//! Semantic navigation — grammar-independent intents and the bounded presentation
//! state they act on.
//!
//! Physical keys never reach the application. A grammar translates a key press
//! into a semantic [`Intent`] (or a typed application [`SemanticInput::Invoke`]),
//! and [`apply_intent`] applies the intent to [`PresentationState`] identically
//! for every grammar. Because the state is grammar-independent, switching the
//! active grammar leaves the active destination and per-destination selection
//! exactly where they were — the camera manifestation changes, the semantic
//! selection does not.

use super::model::Experience;
use crate::ui::element::Key;

/// A minimal, grammar-independent navigation vocabulary. Different grammars bind
/// different physical keys to these (a shelf uses Left/Right for Previous/Next; a
/// cross-bar uses them for the destination axis), but the *semantic effect* of
/// each intent is fixed here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Intent {
    /// Next item within the active destination's collection.
    Next,
    /// Previous item within the active destination's collection.
    Previous,
    /// Activate the current selection (returns its primary action, if any).
    Enter,
    /// Leave the current context. Top-level `Back` is a no-op (no history stack).
    Back,
    /// Next destination along the top navigation axis.
    NextGroup,
    /// Previous destination along the top navigation axis.
    PreviousGroup,
    /// First item within the active destination's collection.
    Home,
    /// Last item within the active destination's collection.
    End,
}

/// What a grammar produces from a physical key: either a semantic navigation
/// intent, or a typed application action to dispatch unchanged.
pub enum SemanticInput<A> {
    Navigate(Intent),
    Invoke(A),
}

/// Bounded presentation state owned by the runtime (not the application's domain
/// state, and not the grammar's camera state). It holds only the active
/// destination and a retained selection *per destination*. Style-specific camera
/// / transition interpolation lives inside the grammar instance, never here, so
/// this state is identical across every grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationState {
    active: usize,
    /// Selection index per destination, parallel to `Experience::destinations`.
    selection: Vec<usize>,
}

impl PresentationState {
    /// Fresh state for an experience: first destination active, every destination
    /// selected at its first item.
    pub fn new<A>(experience: &Experience<A>) -> Self {
        Self {
            active: 0,
            selection: vec![0; experience.destinations.len()],
        }
    }

    /// Index of the active destination.
    pub fn active(&self) -> usize {
        self.active
    }

    /// Selection index within a destination (0 when out of range).
    pub fn selection_of(&self, destination: usize) -> usize {
        self.selection.get(destination).copied().unwrap_or(0)
    }

    /// Selection index within the active destination.
    pub fn selection(&self) -> usize {
        self.selection_of(self.active)
    }

    /// Set the active destination by key; retains each destination's selection.
    /// Returns `true` if the destination exists.
    pub fn activate<A>(&mut self, experience: &Experience<A>, key: &Key) -> bool {
        match experience.destination_index(key) {
            Some(index) => {
                self.sync_len(experience);
                self.active = index;
                true
            }
            None => false,
        }
    }

    fn sync_len<A>(&mut self, experience: &Experience<A>) {
        if self.selection.len() != experience.destinations.len() {
            self.selection.resize(experience.destinations.len(), 0);
        }
        if self.active >= experience.destinations.len() {
            self.active = experience.destinations.len().saturating_sub(1);
        }
    }

    fn active_len<A>(&self, experience: &Experience<A>) -> usize {
        experience
            .destinations
            .get(self.active)
            .map(|d| d.content.selectable_len())
            .unwrap_or(0)
    }

    fn clamp_active_selection(&mut self, len: usize) {
        if let Some(sel) = self.selection.get_mut(self.active) {
            *sel = if len == 0 { 0 } else { (*sel).min(len - 1) };
        }
    }
}

/// Apply a semantic navigation intent to the presentation state, returning the
/// application action to dispatch if the intent activated one (only `Enter`
/// does). The effect is deterministic and identical for every grammar.
///
/// Item motion clamps at the ends — it never wraps and never teleports — so
/// direction reversal mid-motion and rapid input stay coherent. Destination
/// motion clamps likewise; every destination is still reachable by walking the
/// axis.
pub fn apply_intent<A: Clone>(
    experience: &Experience<A>,
    state: &mut PresentationState,
    intent: Intent,
) -> Option<A> {
    state.sync_len(experience);
    let len = state.active_len(experience);
    match intent {
        Intent::Next => {
            if len > 0 {
                let sel = state.selection_of(state.active);
                state.selection[state.active] = (sel + 1).min(len - 1);
            }
            None
        }
        Intent::Previous => {
            if len > 0 {
                let sel = state.selection_of(state.active);
                state.selection[state.active] = sel.saturating_sub(1);
            }
            None
        }
        Intent::Home => {
            if len > 0 {
                state.selection[state.active] = 0;
            }
            None
        }
        Intent::End => {
            if len > 0 {
                state.selection[state.active] = len - 1;
            }
            None
        }
        Intent::NextGroup => {
            let n = experience.destinations.len();
            if n > 0 {
                state.active = (state.active + 1).min(n - 1);
            }
            None
        }
        Intent::PreviousGroup => {
            state.active = state.active.saturating_sub(1);
            None
        }
        Intent::Back => None,
        Intent::Enter => {
            let sel = state.selection_of(state.active);
            let action = experience
                .destinations
                .get(state.active)
                .and_then(|destination| match &destination.content {
                    super::model::Content::Collection(items) => {
                        items.get(sel).and_then(|item| item.primary())
                    }
                    super::model::Content::Detail { actions, .. } => actions.first(),
                    _ => None,
                })
                .map(|action| action.invoke.clone());
            // Enter must not leave a stale selection beyond a shrunk collection.
            state.clamp_active_selection(len);
            action
        }
    }
}
