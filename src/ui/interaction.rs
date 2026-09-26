//! Inspectable typed action routing; application state remains application-owned.

use super::element::Key;
use crate::input::{Event, TextInputState};
use std::fmt;
use std::sync::Arc;

/// A lowering error. Invalid trees are rejected before any presentation is built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiError {
    DuplicateKey(Key),
    InvalidChildren(Key),
    InvalidViewport(Key),
    TooDeep { limit: usize },
    TooManyElements { limit: usize },
}

impl fmt::Display for UiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateKey(key) => write!(f, "duplicate UI key: {key:?}"),
            Self::InvalidChildren(key) => write!(f, "leaf UI element cannot have children: {key}"),
            Self::InvalidViewport(key) => write!(f, "UI viewport accepts at most one child: {key}"),
            Self::TooDeep { limit } => write!(f, "UI tree exceeds {limit} nesting levels"),
            Self::TooManyElements { limit } => write!(f, "UI tree exceeds {limit} elements"),
        }
    }
}

impl std::error::Error for UiError {}

/// An interactive element in semantic traversal order.
///
/// `input` is a snapshot supplied by the application each frame. Editing creates
/// a new value for `on_edit`; the runtime never becomes the input's state owner.
#[derive(Clone)]
pub struct Interaction<A> {
    pub key: Key,
    pub disabled: bool,
    /// Innermost modal containing this element, or `None` for the base view.
    pub modal: Option<Key>,
    pub on_press: Option<A>,
    pub input: Option<TextInputState>,
    pub on_edit: Option<Arc<dyn Fn(TextInputState) -> A>>,
    /// Receives unhandled events for a focused custom control.
    pub on_event: Option<Arc<dyn Fn(Event) -> A>>,
}

/// One modal's capture boundary. The last modal in traversal order is active.
#[derive(Clone)]
pub struct ModalScope<A> {
    pub key: Key,
    pub parent: Option<Key>,
    pub on_dismiss: Option<A>,
}

/// The sidecar produced alongside an ordinary [`crate::Node`].
#[derive(Clone)]
pub struct InteractionMap<A> {
    pub entries: Vec<Interaction<A>>,
    pub modals: Vec<ModalScope<A>>,
}

impl<A> Default for InteractionMap<A> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            modals: Vec::new(),
        }
    }
}

impl<A> InteractionMap<A> {
    pub fn get(&self, key: &Key) -> Option<&Interaction<A>> {
        self.entries.iter().find(|entry| &entry.key == key)
    }

    pub fn active_modal(&self) -> Option<&ModalScope<A>> {
        self.modals.last()
    }
}

/// Actions and whether presentation routing consumed the input.
///
/// Applications should dispatch `actions` first and forward the original event
/// to their own shortcuts only when `consumed` is false. This prevents a modal
/// or text field from also triggering a background action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventOutcome<A> {
    pub actions: Vec<A>,
    pub consumed: bool,
}

impl<A> Default for EventOutcome<A> {
    fn default() -> Self {
        Self {
            actions: Vec::new(),
            consumed: false,
        }
    }
}
