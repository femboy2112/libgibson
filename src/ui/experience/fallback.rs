//! A grammar that steps down to [`Standard`] when its primary grammar is not
//! appropriate for the viewport or the user's preferences.
//!
//! Spatial grammars (blades, shelves, orbits) need width, colour and motion to
//! carry their metaphor. [`Fallback`] presents its primary grammar only when all
//! three are available and otherwise presents the plain [`Standard`] list:
//!
//! - the viewport is at most [`Fallback::DEFAULT_MAX_FALLBACK_WIDTH`] columns wide
//!   (configurable with [`Fallback::fallback_at_or_below`]);
//! - the colour depth is [`ColorDepth::Mono`];
//! - the motion preference is anything other than [`MotionPreference::Full`].
//!
//! The decision is re-made on every [`Grammar::present`], so a resize or a
//! preference change switches grammar on the next frame, and the selection is
//! kept because [`PresentationState`] is grammar-independent. Keys are
//! interpreted by whichever grammar was presented last, so bindings always match
//! what is on screen.
use super::grammar::{Grammar, Presented};
use super::intent::{PresentationState, SemanticInput};
use super::model::Experience;
use super::standard::Standard;
use crate::capability::ColorDepth;
use crate::input::KeyEvent;
use crate::ui::motion::MotionPreference;
use crate::ui::skin::UiEnvironment;
use std::cell::Cell;
use std::time::Duration;

/// Presents `primary`, or [`Standard`] when the viewport or preferences call for
/// it. See the module documentation.
pub struct Fallback<A> {
    primary: Box<dyn Grammar<A>>,
    standard: Standard,
    max_fallback_width: u16,
    using_standard: Cell<bool>,
}

impl<A> Fallback<A> {
    /// Viewports this many columns wide or narrower use [`Standard`].
    pub const DEFAULT_MAX_FALLBACK_WIDTH: u16 = 40;

    pub fn new(primary: Box<dyn Grammar<A>>) -> Self {
        Self {
            primary,
            standard: Standard::new(),
            max_fallback_width: Self::DEFAULT_MAX_FALLBACK_WIDTH,
            using_standard: Cell::new(false),
        }
    }

    /// Use [`Standard`] when the viewport is `columns` wide or narrower.
    pub fn fallback_at_or_below(mut self, columns: u16) -> Self {
        self.max_fallback_width = columns;
        self
    }

    /// Whether `env` calls for the [`Standard`] fallback.
    pub fn uses_standard(&self, env: &UiEnvironment) -> bool {
        env.width <= self.max_fallback_width
            || env.color_depth == ColorDepth::Mono
            || env.motion != MotionPreference::Full
    }
}

impl<A: Clone> Grammar<A> for Fallback<A> {
    /// The primary grammar's name; the [`super::PresentationReceipt`] of each
    /// frame names the grammar actually presented.
    fn name(&self) -> &'static str {
        self.primary.name()
    }

    fn present(
        &mut self,
        experience: &Experience<A>,
        state: &PresentationState,
        env: &UiEnvironment,
        now: Duration,
    ) -> Presented<A> {
        let standard = self.uses_standard(env);
        self.using_standard.set(standard);
        if standard {
            self.standard.present(experience, state, env, now)
        } else {
            self.primary.present(experience, state, env, now)
        }
    }

    fn interpret(
        &self,
        key: &KeyEvent,
        experience: &Experience<A>,
        state: &PresentationState,
    ) -> Option<SemanticInput<A>> {
        if self.using_standard.get() {
            <Standard as Grammar<A>>::interpret(&self.standard, key, experience, state)
        } else {
            self.primary.interpret(key, experience, state)
        }
    }
}
