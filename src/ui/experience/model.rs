//! The semantic application vocabulary — what an application *means*, with no
//! geometry decided yet.
//!
//! An [`Experience`] is a title and an ordered set of [`Destination`]s. A
//! destination carries one [`Content`] kind. The vocabulary is deliberately tiny
//! (four content kinds, two leaf data types) so that a presentation grammar can
//! realize it freely without the application having pre-committed to rows,
//! columns, panels, cover yaw or blade stacks. The application describes *what
//! exists and what can be done*; the grammar decides *how it looks and how you
//! move through it*.
//!
//! The identity type is the existing [`crate::ui::element::Key`] — reused, not
//! reinvented, so a semantic id is the same currency the rest of `gibson::ui`
//! already speaks.

use crate::ui::element::Key;
use crate::Surface;
use std::sync::Arc;

/// Semantic priority. Drives *declared* responsive omission: an
/// [`Priority::Essential`] element is never silently dropped, a
/// [`Priority::Tertiary`] one may be omitted first under size pressure and the
/// omission is recorded in the presentation receipt. See
/// [`super::receipt::PresentationReceipt`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Priority {
    /// Must always be represented and reachable. Never an allowed omission.
    Essential,
    /// Represented when space permits; the default.
    #[default]
    Normal,
    /// Decorative / supplementary. First to be omitted; omission must be declared.
    Tertiary,
}

/// A procedural, copyright-free visual identity for an item. A grammar that shows
/// media (a cover carousel, a channel tile) derives a deterministic abstract
/// image from `seed`; a grammar that does not simply ignores it. There is no
/// image file, no decoded asset and no dependency — the *application* names the
/// identity, the *grammar* realizes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Media {
    /// Deterministic seed for the procedural visual identity.
    pub seed: u64,
}

impl Media {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }
}

/// A typed, named thing the user can do. `invoke` is the application's own action
/// payload — the grammar never interprets it, it only surfaces the action and, on
/// activation, hands `invoke` back to the application unchanged. This keeps
/// application behavior typed and application-owned (the grammar cannot invent or
/// alter actions).
#[derive(Clone)]
pub struct Action<A> {
    pub key: Key,
    pub label: String,
    pub invoke: A,
}

impl<A> Action<A> {
    pub fn new(key: impl Into<Key>, label: impl Into<String>, invoke: A) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            invoke,
        }
    }
}

/// One selectable entity in a [`Content::Collection`]. A library album, a settings
/// row, a channel tile — the grammar does not know which. `actions[0]`, if any, is
/// the *primary* action, activated by [`super::intent::Intent::Enter`].
#[derive(Clone)]
pub struct Item<A> {
    pub key: Key,
    pub title: String,
    pub subtitle: Option<String>,
    pub media: Option<Media>,
    pub priority: Priority,
    pub actions: Vec<Action<A>>,
}

impl<A> Item<A> {
    pub fn new(key: impl Into<Key>, title: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            subtitle: None,
            media: None,
            priority: Priority::Normal,
            actions: Vec::new(),
        }
    }

    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn media(mut self, media: Media) -> Self {
        self.media = Some(media);
        self
    }

    pub fn priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    pub fn action(mut self, action: Action<A>) -> Self {
        self.actions.push(action);
        self
    }

    /// The primary action (activated on `Enter`), if any.
    pub fn primary(&self) -> Option<&Action<A>> {
        self.actions.first()
    }
}

/// A read-only property of a [`Content::Detail`] destination (a now-playing field,
/// a status line). Label and value are already-formatted strings the application
/// owns; the grammar decides typography and placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facet {
    pub key: Key,
    pub label: String,
    pub value: String,
    pub priority: Priority,
}

impl Facet {
    pub fn new(key: impl Into<Key>, label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            value: value.into(),
            priority: Priority::Normal,
        }
    }

    pub fn priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }
}

/// A custom instrument a destination attaches — the escape hatch for domains the
/// semantic vocabulary deliberately does not model (a dense observatory panel, a
/// live visualization). The grammar composites the produced [`Surface`] verbatim
/// into its world (via [`crate::ui::element::raster`] / `surface` / `presented`);
/// it does not reinterpret the contents. The closure receives the cell rectangle
/// the grammar has allotted.
#[derive(Clone)]
pub struct Custom {
    pub key: Key,
    pub label: String,
    #[allow(clippy::type_complexity)]
    pub(crate) render: Arc<dyn Fn(u16, u16) -> Surface + Send + Sync>,
}

impl Custom {
    pub fn new(
        key: impl Into<Key>,
        label: impl Into<String>,
        render: impl Fn(u16, u16) -> Surface + Send + Sync + 'static,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            render: Arc::new(render),
        }
    }

    /// Render the instrument at a given cell size.
    pub fn render(&self, cols: u16, rows: u16) -> Surface {
        (self.render)(cols.max(1), rows.max(1))
    }
}

/// The one content kind a [`Destination`] holds. Four kinds, chosen as the minimum
/// that lets radically different grammars realize the same application:
///
/// - [`Content::Collection`] — an ordered set of selectable [`Item`]s (a library,
///   a settings list, a channel grid). The carousel / shelf / tile domain.
/// - [`Content::Detail`] — properties and actions for the current thing
///   (now-playing, an inspector).
/// - [`Content::Prose`] — read-only lines (about, help, status).
/// - [`Content::Custom`] — an attached instrument the grammar composites verbatim.
#[derive(Clone)]
pub enum Content<A> {
    Collection(Vec<Item<A>>),
    Detail {
        facets: Vec<Facet>,
        actions: Vec<Action<A>>,
    },
    Prose(Vec<String>),
    Custom(Custom),
}

impl<A> Content<A> {
    /// The number of selectable positions in this content (collection length;
    /// detail and prose and custom are a single position).
    pub fn selectable_len(&self) -> usize {
        match self {
            Content::Collection(items) => items.len(),
            _ => 0,
        }
    }
}

/// A major section of the application — a top-level place you can navigate *to*
/// (LIBRARY, NOW PLAYING, SETTINGS, ABOUT). Destinations form the application's
/// top navigation axis; every grammar must make every destination reachable.
#[derive(Clone)]
pub struct Destination<A> {
    pub key: Key,
    pub title: String,
    pub content: Content<A>,
}

impl<A> Destination<A> {
    pub fn new(key: impl Into<Key>, title: impl Into<String>, content: Content<A>) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            content,
        }
    }
}

/// The whole semantic application: a title and its destinations. Built *once* by
/// the application; every presentation grammar renders this same value, and a
/// live style switch never rebuilds it. This is the `A` of the preservation law
/// `π_σ(F_σ(A)) = required_semantics(A)` (see [`super::receipt`]).
#[derive(Clone)]
pub struct Experience<A> {
    pub title: String,
    pub destinations: Vec<Destination<A>>,
}

impl<A> Experience<A> {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            destinations: Vec::new(),
        }
    }

    pub fn destination(mut self, destination: Destination<A>) -> Self {
        self.destinations.push(destination);
        self
    }

    pub fn destinations(&self) -> &[Destination<A>] {
        &self.destinations
    }

    /// Index of the destination with `key`, if present.
    pub fn destination_index(&self, key: &Key) -> Option<usize> {
        self.destinations.iter().position(|d| &d.key == key)
    }
}
