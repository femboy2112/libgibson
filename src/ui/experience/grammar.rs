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

/// What a presented frame asks of the scheduler: when, if ever, the *next* frame
/// is needed assuming the semantic state and environment do not change. This is
/// the smallest generic temporal contract (milestone §7) — it replaces hardcoding
/// grammar categories, so a grammar that settles can declare itself finished and a
/// static grammar never asks for a repaint. The ordinary `Context`/`App` scheduler
/// executes the demand; this type does not schedule anything itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameDemand {
    /// No new frame is needed until the semantics or environment change. A static
    /// grammar — or a settling one that has settled — returns this, and the
    /// scheduler may stop repainting the experience entirely (0 further bytes).
    OnChange,
    /// Another frame is needed after this delay: a grammar still settling toward a
    /// target, or one that is continuously animated. Missed deadlines coalesce —
    /// the next frame simply advances motion to the then-current presentation time,
    /// never replaying a backlog of stale intermediate states.
    After(Duration),
}

impl FrameDemand {
    /// A continuous/settling cadence expressed as frames per second. `fps(60)` is
    /// `After(≈16ms)`. Clamped to at least 1 fps.
    pub fn fps(fps: u32) -> Self {
        FrameDemand::After(Duration::from_nanos(1_000_000_000 / fps.max(1) as u64))
    }

    /// Whether this frame wants another frame later (vs. resting until change).
    pub fn is_animating(self) -> bool {
        matches!(self, FrameDemand::After(_))
    }
}

/// A presentation plus the semantic receipt that lets the preservation law be
/// checked, and the temporal demand that says when the next frame is due. The
/// application renders `element`; tests and inspectors read `receipt`; the
/// scheduler reads `demand`.
pub struct Presented<A> {
    pub element: Element<A>,
    pub receipt: PresentationReceipt,
    pub demand: FrameDemand,
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
    /// A presentation that rests until the semantics or environment change
    /// ([`FrameDemand::OnChange`]) — the right default for a static grammar. A
    /// moving grammar builds this and then declares its cadence with
    /// [`Presented::with_demand`].
    pub fn new(element: Element<A>, receipt: PresentationReceipt) -> Self {
        Self {
            element,
            receipt,
            demand: FrameDemand::OnChange,
        }
    }

    /// Set the temporal demand (e.g. [`FrameDemand::fps`] while settling or
    /// animating). Returns `self` for chaining off [`Presented::new`].
    pub fn with_demand(mut self, demand: FrameDemand) -> Self {
        self.demand = demand;
        self
    }

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
