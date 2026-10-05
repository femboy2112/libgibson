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
//!
//! State is keyed by **identity**, not position: the active destination and each
//! destination's selection are stored as [`Key`]s and re-resolved against the
//! current [`Experience`] on every access through one canonical resolver. So an
//! application that inserts, removes, reorders or filters items between frames
//! (the Elm rebuild-in-view pattern) never has its selection silently retargeted
//! to a different entity, and `required_semantics` and every grammar always agree
//! on what is selected — there is only one clamping rule, and it lives here.

use super::model::Experience;
use crate::ui::element::Key;
use std::collections::HashMap;

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
/// intent, or a typed application action to dispatch unchanged. A grammar binds
/// a key directly to [`SemanticInput::Invoke`] to reach a *secondary* action that
/// the core navigation vocabulary (which only activates the primary action via
/// [`Intent::Enter`]) does not.
pub enum SemanticInput<A> {
    Navigate(Intent),
    Invoke(A),
}

/// Bounded presentation state owned by the runtime — not the application's domain
/// state, and not the grammar's camera state. It holds only the active
/// destination and a retained selection *per destination*, both by identity.
/// Style-specific camera / transition interpolation lives inside the grammar
/// instance, never here, so this state is identical across every grammar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PresentationState {
    /// Active destination key; `None` resolves to the first destination.
    active: Option<Key>,
    /// Selected item key per destination key. A missing or stale entry resolves
    /// to the first item (clamped), identically for every reader.
    selection: HashMap<Key, Key>,
}

impl PresentationState {
    /// Fresh state for an experience: first destination active, every destination
    /// at its first item (resolved lazily).
    pub fn new<A>(experience: &Experience<A>) -> Self {
        Self {
            active: experience.destinations.first().map(|d| d.key.clone()),
            selection: HashMap::new(),
        }
    }

    /// The resolved active-destination index, clamped to the current experience.
    pub fn active_index<A>(&self, experience: &Experience<A>) -> usize {
        let n = experience.destinations.len();
        if n == 0 {
            return 0;
        }
        self.active
            .as_ref()
            .and_then(|key| experience.destination_index(key))
            .unwrap_or(0)
            .min(n - 1)
    }

    /// The resolved active-destination key, if the experience is non-empty.
    pub fn active_key<A>(&self, experience: &Experience<A>) -> Option<Key> {
        experience
            .destinations
            .get(self.active_index(experience))
            .map(|d| d.key.clone())
    }

    /// The resolved selection index within the active destination, clamped to its
    /// current collection length. `None` when the active destination is empty or
    /// not a collection. This is the *one* clamping rule every reader shares.
    pub fn selected_index<A>(&self, experience: &Experience<A>) -> Option<usize> {
        let destination = experience.destinations.get(self.active_index(experience))?;
        let len = destination.content.selectable_len();
        if len == 0 {
            return None;
        }
        let stored = self.selection.get(&destination.key);
        let index = match stored {
            Some(item_key) => item_key_index(destination, item_key).unwrap_or(0),
            None => 0,
        };
        Some(index.min(len - 1))
    }

    /// The resolved selected-item key within the active destination, if any.
    pub fn selected_key<A>(&self, experience: &Experience<A>) -> Option<Key> {
        let destination = experience.destinations.get(self.active_index(experience))?;
        let index = self.selected_index(experience)?;
        collection_item_key(destination, index)
    }

    /// Set the active destination by key; retains each destination's selection.
    /// Returns `true` if the destination exists.
    pub fn activate<A>(&mut self, experience: &Experience<A>, key: &Key) -> bool {
        if experience.destination_index(key).is_some() {
            self.active = Some(key.clone());
            true
        } else {
            false
        }
    }

    /// Store the selection of the active destination by item index (resolving the
    /// item key so the choice survives later reorders).
    fn set_selected_index<A>(&mut self, experience: &Experience<A>, index: usize) {
        let active_index = self.active_index(experience);
        if let Some(destination) = experience.destinations.get(active_index) {
            if let Some(item_key) = collection_item_key(destination, index) {
                self.selection.insert(destination.key.clone(), item_key);
            }
        }
    }

    fn set_active_index<A>(&mut self, experience: &Experience<A>, index: usize) {
        if let Some(destination) = experience.destinations.get(index) {
            self.active = Some(destination.key.clone());
        }
    }
}

fn collection_item_key<A>(destination: &super::model::Destination<A>, index: usize) -> Option<Key> {
    match &destination.content {
        super::model::Content::Collection(items) => items.get(index).map(|item| item.key.clone()),
        _ => None,
    }
}

fn item_key_index<A>(destination: &super::model::Destination<A>, key: &Key) -> Option<usize> {
    match &destination.content {
        super::model::Content::Collection(items) => items.iter().position(|item| &item.key == key),
        _ => None,
    }
}

/// Apply a semantic navigation intent to the presentation state, returning the
/// application action to dispatch if the intent activated one (only `Enter`
/// does, and only the selection's *primary* action). The effect is deterministic
/// and identical for every grammar.
///
/// Item motion clamps at the ends — it never wraps and never teleports — so
/// direction reversal mid-motion and rapid input stay coherent. Destination
/// motion clamps likewise; every destination is still reachable by walking.
pub fn apply_intent<A: Clone>(
    experience: &Experience<A>,
    state: &mut PresentationState,
    intent: Intent,
) -> Option<A> {
    let active_index = state.active_index(experience);
    let len = experience
        .destinations
        .get(active_index)
        .map(|d| d.content.selectable_len())
        .unwrap_or(0);
    let current = state.selected_index(experience).unwrap_or(0);

    match intent {
        Intent::Next => {
            if len > 0 {
                state.set_selected_index(experience, (current + 1).min(len - 1));
            }
            None
        }
        Intent::Previous => {
            if len > 0 {
                state.set_selected_index(experience, current.saturating_sub(1));
            }
            None
        }
        Intent::Home => {
            if len > 0 {
                state.set_selected_index(experience, 0);
            }
            None
        }
        Intent::End => {
            if len > 0 {
                state.set_selected_index(experience, len - 1);
            }
            None
        }
        Intent::NextGroup => {
            let n = experience.destinations.len();
            if n > 0 {
                state.set_active_index(experience, (active_index + 1).min(n - 1));
            }
            None
        }
        Intent::PreviousGroup => {
            state.set_active_index(experience, active_index.saturating_sub(1));
            None
        }
        Intent::Back => None,
        Intent::Enter => experience
            .destinations
            .get(active_index)
            .and_then(|destination| match &destination.content {
                super::model::Content::Collection(items) => {
                    items.get(current).and_then(|item| item.primary())
                }
                super::model::Content::Detail { actions, .. } => actions.first(),
                _ => None,
            })
            .map(|action| action.invoke.clone()),
    }
}
