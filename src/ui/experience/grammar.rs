//! The presentation-grammar contract: the seam every style implements.
//!
//! A grammar is a *presentation functor* `F_σ`. Given the same semantic
//! [`Experience`] and [`PresentationState`], it produces a [`Presented`] — an
//! [`Element`] to render plus a [`PresentationReceipt`] proving what it
//! represented. It also owns the physical-key → semantic-input binding for its
//! own interaction metaphor. It does **not** own the semantic state (that is the
//! runtime's) or the application's domain data.
//!
//! A grammar may hold private, style-specific camera / transition state and so
//! takes `&mut self` on [`Grammar::present`]; the elapsed `now` lets animated
//! grammars advance continuous motion. The reference grammar is stateless and
//! ignores both.

use super::intent::{PresentationState, SemanticInput};
use super::model::Experience;
use super::receipt::{LawViolation, PresentationReceipt, RequiredSemantics};
use crate::input::KeyEvent;
use crate::ui::element::{Element, Key};
use crate::ui::skin::UiEnvironment;
use std::collections::BTreeSet;
use std::time::Duration;

/// A presentation plus the semantic receipt that lets the preservation law be
/// checked. The application renders `element`; tests and inspectors read
/// `receipt`.
pub struct Presented<A> {
    pub element: Element<A>,
    pub receipt: PresentationReceipt,
}

/// Collect every explicit [`Key`] present in a lowered element tree (children and
/// overlays), so the receipt can be held accountable to what was actually built.
fn collect_element_keys<A>(element: &Element<A>, out: &mut BTreeSet<Key>) {
    if let Some(key) = &element.key {
        out.insert(key.clone());
    }
    for child in &element.children {
        collect_element_keys(child, out);
    }
    for overlay in &element.overlays {
        collect_element_keys(overlay, out);
    }
}

impl<A> Presented<A> {
    /// The real preservation-law gate: the receipt's bookkeeping *and* a
    /// cross-check that every id it claims to represent is actually in the
    /// rendered element tree (as a keyed node) or attested rastered. Empty =
    /// the presentation is faithful. A grammar whose receipt describes a render
    /// it did not produce fails here even if its bookkeeping is internally
    /// consistent.
    pub fn check(&self, required: &RequiredSemantics) -> Vec<LawViolation> {
        let mut keys = BTreeSet::new();
        collect_element_keys(&self.element, &mut keys);
        self.receipt.check_rendered(&keys, required)
    }

    /// Convenience: does this presentation satisfy the full (rendered) law?
    pub fn preserves(&self, required: &RequiredSemantics) -> bool {
        self.check(required).is_empty()
    }
}

/// A presentation grammar: one deterministic way to realize any semantic
/// [`Experience`]. Object-safe, so an application holds `Box<dyn Grammar<A>>` and
/// swaps it live to switch styles without touching its model, view or update.
pub trait Grammar<A> {
    /// Stable style name, surfaced unobtrusively and recorded in the receipt.
    fn name(&self) -> &'static str;

    /// Realize the experience at the current state for the given viewport and
    /// capabilities. `now` is elapsed time for animated grammars.
    fn present(
        &mut self,
        experience: &Experience<A>,
        state: &PresentationState,
        env: &UiEnvironment,
        now: Duration,
    ) -> Presented<A>;

    /// Translate a physical key into a semantic input under this grammar's
    /// interaction metaphor, or `None` if the key is not bound. The *effect* of a
    /// returned [`SemanticInput`] is grammar-independent (see
    /// [`super::intent::apply_intent`]); only the binding is the grammar's choice.
    fn interpret(
        &self,
        key: &KeyEvent,
        experience: &Experience<A>,
        state: &PresentationState,
    ) -> Option<SemanticInput<A>>;
}
