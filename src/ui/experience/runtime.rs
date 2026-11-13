//! [`ExperienceRuntime`] — the ergonomic orchestration layer over presentation
//! grammars.
//!
//! The proof-of-concept lab owned a `Vec<RefCell<Box<dyn Grammar>>>`, a current
//! index and the switch logic by hand. That proved the architecture but is not an
//! API. This runtime owns *presentation* orchestration — the grammar-independent
//! [`PresentationState`] and a set of **persistent** grammar instances — so an
//! application switches style with one call and never touches a trait-object
//! registry. It does **not** own the application's domain data: the semantic
//! [`Experience`] is passed in on every call, exactly as the application owns it.
//!
//! ```ignore
//! let mut ui = ExperienceRuntime::with_builtins(&experience);
//! ui.set_style(ExperienceStyle::Orbital);
//! let presented = ui.present(&experience, env, now);   // Element + receipt + FrameDemand
//! if let Some(msg) = ui.handle_key(&experience, &key) { /* dispatch msg */ }
//! ```
//!
//! Because the grammar instances are persistent, switching away from a grammar and
//! back preserves its private camera / spring state; the shared semantic selection
//! lives in [`PresentationState`] and survives every switch regardless (see
//! [`super::intent`]). Custom grammars remain possible via
//! [`ExperienceRuntime::register`] — the built-in set is not a closed
//! world.

use super::blades::Blades;
use super::cross_media::CrossMedia;
use super::grammar::{Grammar, Presented};
use super::intent::PresentationState;
use super::media_shelf::MediaShelf;
use super::model::Experience;
use super::orbital::Orbital;
use super::panorama::Panorama;
use super::standard::Standard;
use crate::input::KeyEvent;
use crate::ui::skin::UiEnvironment;
use std::time::Duration;

/// A built-in presentation style. The canonical set the library ships; an
/// application names one to switch to it in a single [`ExperienceRuntime::set_style`]
/// call. This enum is *not* a closed world — a custom grammar is registered under a
/// [`StyleId::Custom`] name (see [`ExperienceRuntime::register`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExperienceStyle {
    /// The plain reference oracle (`STANDARD`).
    Standard,
    /// The Cover-Flow collection shelf (`MEDIA_SHELF`).
    MediaShelf,
    /// The cross-bar fusion (`CROSS_MEDIA`).
    CrossMedia,
    /// The typographic panorama world (`PANORAMA`).
    Panorama,
    /// The orbital focal field (`ORBITAL`).
    Orbital,
    /// The occluding blade stack (`BLADES`).
    Blades,
}

impl ExperienceStyle {
    /// Every built-in style, in the canonical cycle order (the reference
    /// `Standard` first, then the cinematic grammars).
    pub const ALL: [ExperienceStyle; 6] = [
        ExperienceStyle::Standard,
        ExperienceStyle::MediaShelf,
        ExperienceStyle::CrossMedia,
        ExperienceStyle::Panorama,
        ExperienceStyle::Orbital,
        ExperienceStyle::Blades,
    ];

    /// The grammar's stable style name (matches [`Grammar::name`]).
    pub fn name(self) -> &'static str {
        match self {
            ExperienceStyle::Standard => "STANDARD",
            ExperienceStyle::MediaShelf => "MEDIA_SHELF",
            ExperienceStyle::CrossMedia => "CROSS_MEDIA",
            ExperienceStyle::Panorama => "PANORAMA",
            ExperienceStyle::Orbital => "ORBITAL",
            ExperienceStyle::Blades => "BLADES",
        }
    }

    fn build<A: Clone + 'static>(self) -> Box<dyn Grammar<A>> {
        match self {
            ExperienceStyle::Standard => Box::new(Standard::new()),
            ExperienceStyle::MediaShelf => Box::new(MediaShelf::new()),
            ExperienceStyle::CrossMedia => Box::new(CrossMedia::new()),
            ExperienceStyle::Panorama => Box::new(Panorama::new()),
            ExperienceStyle::Orbital => Box::new(Orbital::new()),
            ExperienceStyle::Blades => Box::new(Blades::new()),
        }
    }
}

/// The identity of a style slot in an [`ExperienceRuntime`]: a built-in style or a
/// custom grammar registered under a name. Used to report and select the active
/// style without a closed enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StyleId {
    /// One of the library's built-in styles.
    Builtin(ExperienceStyle),
    /// A custom grammar registered under this name.
    Custom(String),
}

impl StyleId {
    /// The style's display name (the built-in name, or the registered custom name).
    pub fn name(&self) -> &str {
        match self {
            StyleId::Builtin(style) => style.name(),
            StyleId::Custom(name) => name,
        }
    }
}

impl From<ExperienceStyle> for StyleId {
    fn from(style: ExperienceStyle) -> Self {
        StyleId::Builtin(style)
    }
}

struct Slot<A> {
    id: StyleId,
    grammar: Box<dyn Grammar<A>>,
}

/// Owns presentation orchestration for one application: the grammar-independent
/// [`PresentationState`] and a set of persistent grammar instances, with the active
/// one switchable by a single call. The application still owns its [`Experience`]
/// and passes it in on every [`ExperienceRuntime::present`] /
/// [`ExperienceRuntime::handle_key`].
pub struct ExperienceRuntime<A> {
    state: PresentationState,
    slots: Vec<Slot<A>>,
    current: usize,
}

impl<A: Clone + 'static> ExperienceRuntime<A> {
    /// A runtime pre-loaded with every built-in style in canonical order, with
    /// fresh [`PresentationState`] for `experience` and the reference `Standard`
    /// style active. The experience is read only to seed the initial state; the
    /// runtime does not retain it.
    pub fn with_builtins(experience: &Experience<A>) -> Self {
        let slots = ExperienceStyle::ALL
            .iter()
            .map(|&style| Slot {
                id: StyleId::Builtin(style),
                grammar: style.build::<A>(),
            })
            .collect();
        Self {
            state: PresentationState::new(experience),
            slots,
            current: 0,
        }
    }

    /// An empty runtime (no styles). Use [`ExperienceRuntime::register`]
    /// to add grammars; [`ExperienceRuntime::present`] renders a plain
    /// fallback until at least one is registered.
    pub fn empty(experience: &Experience<A>) -> Self {
        Self {
            state: PresentationState::new(experience),
            slots: Vec::new(),
            current: 0,
        }
    }

    /// Register a custom grammar under `name`, appending it after the current
    /// styles. If this is the first style registered, it becomes active. Returns
    /// `&mut self` for chaining.
    pub fn register(&mut self, name: impl Into<String>, grammar: Box<dyn Grammar<A>>) -> &mut Self {
        self.slots.push(Slot {
            id: StyleId::Custom(name.into()),
            grammar,
        });
        self
    }

    /// Switch to a built-in style. Returns `false` (leaving the active style
    /// unchanged) if that built-in is not present in this runtime.
    pub fn set_style(&mut self, style: ExperienceStyle) -> bool {
        self.set_style_id(&StyleId::Builtin(style))
    }

    /// Switch to any style by its [`StyleId`] (built-in or custom). Returns `false`
    /// if no slot has that id.
    pub fn set_style_id(&mut self, id: &StyleId) -> bool {
        if let Some(index) = self.slots.iter().position(|slot| &slot.id == id) {
            self.current = index;
            true
        } else {
            false
        }
    }

    /// Advance to the next style in registration order, wrapping. No-op if empty.
    pub fn next_style(&mut self) {
        if !self.slots.is_empty() {
            self.current = (self.current + 1) % self.slots.len();
        }
    }

    /// Step to the previous style in registration order, wrapping. No-op if empty.
    pub fn previous_style(&mut self) {
        if !self.slots.is_empty() {
            self.current = (self.current + self.slots.len() - 1) % self.slots.len();
        }
    }

    /// The active style's id, or `None` if the runtime has no styles.
    pub fn current_style(&self) -> Option<&StyleId> {
        self.slots.get(self.current).map(|slot| &slot.id)
    }

    /// The active style's display name (`"—"` if the runtime has no styles).
    pub fn style_name(&self) -> &str {
        self.current_style().map(StyleId::name).unwrap_or("—")
    }

    /// The ids of every registered style, in order — for a style picker.
    pub fn styles(&self) -> impl Iterator<Item = &StyleId> {
        self.slots.iter().map(|slot| &slot.id)
    }

    /// The number of registered styles.
    pub fn style_count(&self) -> usize {
        self.slots.len()
    }

    /// Read access to the grammar-independent presentation state (active
    /// destination + per-destination selection, all by identity).
    pub fn state(&self) -> &PresentationState {
        &self.state
    }

    /// Mutable access to the presentation state, for an application that drives
    /// navigation programmatically (e.g. a show director) rather than only through
    /// [`ExperienceRuntime::handle_key`].
    pub fn state_mut(&mut self) -> &mut PresentationState {
        &mut self.state
    }

    /// Present the shared semantic value through the active grammar at the current
    /// state, viewport and time, returning the element to render, the preservation
    /// receipt, and the temporal [`FrameDemand`](crate::ui::experience::FrameDemand). Advances the active grammar's
    /// private motion to `now` (missed deadlines coalesce — no backlog of stale
    /// intermediate frames). If the runtime has no styles, yields an `OnChange`
    /// plain fallback.
    pub fn present(
        &mut self,
        experience: &Experience<A>,
        env: &UiEnvironment,
        now: Duration,
    ) -> Presented<A> {
        match self.slots.get_mut(self.current) {
            Some(slot) => slot.grammar.present(experience, &self.state, env, now),
            None => Presented::new(
                crate::ui::element::screen::<A>()
                    .child(crate::ui::element::text::<A>("(no presentation style)")),
                super::receipt::PresentationReceipt::new(
                    "NONE",
                    crate::ui::element::Key::named("∅"),
                ),
            ),
        }
    }

    /// Drive one physical key through the active grammar's interaction metaphor into
    /// the shared presentation state, returning the typed application action to
    /// dispatch if one was activated. The semantic effect is grammar-independent
    /// (see [`super::apply_intent`]); only the key binding is the grammar's. A key
    /// the active grammar does not bind (or an empty runtime) returns `None`.
    pub fn handle_key(&mut self, experience: &Experience<A>, key: &KeyEvent) -> Option<A> {
        let slot = self.slots.get(self.current)?;
        super::handle_key(slot.grammar.as_ref(), experience, &mut self.state, key)
    }
}
