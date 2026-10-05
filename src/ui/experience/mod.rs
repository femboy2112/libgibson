//! Experimental **semantic experience grammars** — the layer *above*
//! [`Skin`](crate::ui::skin::Skin).
//!
//! A [`Skin`](crate::ui::skin::Skin) restyles an element tree (palette, chrome,
//! density, glyphs, motion tokens). But by the time a skin sees the tree, the
//! application has already decided its geometry — rows, columns, panels. This
//! layer sits higher: the application describes itself *semantically* as an
//! [`Experience`](crate::ui::experience::Experience) (destinations, collections,
//! items, actions), and a [`Grammar`](crate::ui::experience::Grammar)
//! deterministically realizes that same semantic value as a radically
//! different interface — a cover shelf, a cross-media bar, an orbital field, a
//! blade stack, a typographic panorama — **without the application forking per
//! style**.
//!
//! The governing law (see [`receipt`](crate::ui::experience::receipt)) is the
//! Representation Atlas law shared by
//! the rest of v0.5: a presentation grammar is a change of *representation*, not
//! of application state. Formally, for a grammar `σ`,
//!
//! ```text
//!     π_σ(F_σ(A)) = required_semantics(A)   up to declared responsive omission
//! ```
//!
//! A style may rearrange everything visible; it may not invent actions, silently
//! lose a required action or destination, change entity identity, or move the
//! selected object merely because the presentation changed.
//!
//! # Pipeline
//!
//! ```text
//!   Experience (semantic)
//!       -> Grammar::present  ->  Presented { Element, PresentationReceipt }
//!       -> crate::ui compile -> Node -> layout -> raster -> terminal
//! ```
//!
//! Grammars lower into ordinary [`Element`](crate::ui::element::Element)s and the
//! existing `raster3d` / `Surface` escape hatches; they add no renderer, layout
//! engine or terminal owner (see the crate's substrate contract).
//!
//! # Authoring, in brief
//!
//! ```ignore
//! let experience = Experience::new("LIBRARY")
//!     .destination(Destination::new("library", "Library", Content::Collection(items)))
//!     .destination(Destination::new("about", "About", Content::Prose(lines)));
//! let mut state = PresentationState::new(&experience);
//! let mut grammar: Box<dyn Grammar<Msg>> = Box::new(Standard::new());
//! // view:   grammar.present(&experience, &state, env, now).element
//! // input:  if let Some(msg) = handle_key(&*grammar, &experience, &mut state, &key) { ... }
//! // switch: grammar = Box::new(OtherGrammar::new());  // selection survives
//! ```

pub mod cross_media;
pub mod grammar;
pub mod intent;
pub mod media_shelf;
pub mod model;
pub mod panorama;
pub mod receipt;
pub mod standard;

pub use cross_media::CrossMedia;
pub use grammar::{Grammar, Presented};
pub use intent::{apply_intent, Intent, PresentationState, SemanticInput};
pub use media_shelf::MediaShelf;
pub use model::{Action, Content, Custom, Destination, Experience, Facet, Item, Media, Priority};
pub use panorama::Panorama;
pub use receipt::{required_semantics, LawViolation, PresentationReceipt, RequiredSemantics};
pub use standard::Standard;

use crate::input::KeyEvent;

/// Drive one physical key through the active grammar's interaction metaphor into
/// a grammar-independent state change, returning the typed application action to
/// dispatch if one was activated.
///
/// This is the single entry point an application calls on input: the grammar
/// decides *which* key means *which* intent, [`apply_intent`] decides what that
/// intent *does* (identically for every grammar), and the application receives
/// back only its own typed action — never a key code, never an intent.
pub fn handle_key<A: Clone>(
    grammar: &dyn Grammar<A>,
    experience: &Experience<A>,
    state: &mut PresentationState,
    key: &KeyEvent,
) -> Option<A> {
    match grammar.interpret(key, experience, state)? {
        SemanticInput::Navigate(intent) => apply_intent(experience, state, intent),
        SemanticInput::Invoke(action) => Some(action),
    }
}
