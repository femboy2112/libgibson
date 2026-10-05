//! The presentation receipt and the semantic-preservation law.
//!
//! A grammar records, as it lowers, exactly which semantic ids it represented,
//! which required ids it *declared* it omitted, and which decisions it degraded.
//! [`required_semantics`] computes what the application semantically requires at
//! the current state, and [`PresentationReceipt::check`] enforces the law
//!
//! ```text
//!     π_σ(F_σ(A)) = required_semantics(A)   up to declared responsive omission
//! ```
//!
//! In plain terms: a style may rearrange appearance and navigation freely, but it
//! may not invent actions, silently lose a required action or destination, change
//! entity identity, or move the selected object merely because the presentation
//! changed. The receipt is not an aesthetic score; it is a semantic-honesty
//! ledger, and the only omissions it tolerates are ones the grammar *declared*.

use super::intent::PresentationState;
use super::model::{Content, Experience, Priority};
use crate::ui::element::Key;

/// What the application semantically requires at a given state. Destinations and
/// essential items must be represented; the active destination and the current
/// selection identity are fixed facts a grammar must not alter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredSemantics {
    /// Every destination must be represented (reachable) by every grammar.
    pub destinations: Vec<Key>,
    /// The active destination's identity.
    pub active_destination: Option<Key>,
    /// The current selection identity within the active destination, if any.
    pub selected: Option<Key>,
    /// Essential item ids in the active destination — never an allowed omission.
    pub essential_items: Vec<Key>,
    /// All item ids in the active destination (essential or not).
    pub all_items: Vec<Key>,
    /// Required action ids reachable at this state (the active destination's
    /// collection-item primary actions and detail actions).
    pub actions: Vec<Key>,
}

/// Compute the required semantics at the current presentation state.
pub fn required_semantics<A>(
    experience: &Experience<A>,
    state: &PresentationState,
) -> RequiredSemantics {
    let destinations: Vec<Key> = experience
        .destinations
        .iter()
        .map(|d| d.key.clone())
        .collect();
    let active = experience.destinations.get(state.active());
    let active_destination = active.map(|d| d.key.clone());

    let mut essential_items = Vec::new();
    let mut all_items = Vec::new();
    let mut actions = Vec::new();
    let mut selected = None;

    if let Some(destination) = active {
        match &destination.content {
            Content::Collection(items) => {
                for item in items {
                    all_items.push(item.key.clone());
                    if item.priority == Priority::Essential {
                        essential_items.push(item.key.clone());
                    }
                }
                let sel = state.selection();
                if let Some(item) = items.get(sel) {
                    selected = Some(item.key.clone());
                    // Only the *selected* item's actions are reachable right now.
                    for action in &item.actions {
                        actions.push(action.key.clone());
                    }
                }
            }
            Content::Detail { actions: acts, .. } => {
                for action in acts {
                    actions.push(action.key.clone());
                }
            }
            Content::Prose(_) | Content::Custom(_) => {}
        }
    }

    RequiredSemantics {
        destinations,
        active_destination,
        selected,
        essential_items,
        all_items,
        actions,
    }
}

/// A single way a presentation failed the preservation law.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LawViolation {
    /// A required destination is neither represented nor declared omitted.
    MissingDestination(Key),
    /// The receipt's active destination disagrees with the semantic state.
    WrongActiveDestination { expected: Option<Key>, found: Key },
    /// The receipt's selection disagrees with the semantic state.
    WrongSelection {
        expected: Option<Key>,
        found: Option<Key>,
    },
    /// A required (reachable) action is neither represented nor declared omitted.
    MissingAction(Key),
    /// An essential item was omitted — essential items may never be omitted.
    EssentialOmitted(Key),
    /// A required id is missing entirely (not represented, not declared omitted):
    /// a *silent* loss, the specific thing the law forbids.
    SilentLoss(Key),
    /// The receipt represents an id the application never defined: an invented id.
    Invented(Key),
}

/// A grammar's record of what it represented at a given state. Built by the
/// grammar as it lowers; checked against [`required_semantics`] by
/// [`PresentationReceipt::check`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationReceipt {
    /// Grammar name (`STANDARD`, `MEDIA_SHELF`, ...).
    pub style: &'static str,
    /// The active destination the grammar presented.
    pub active_destination: Key,
    /// The selection identity the grammar presented, if any.
    pub selected: Option<Key>,
    /// Destination ids represented (made reachable) by the presentation.
    pub destinations: Vec<Key>,
    /// Item ids represented in the active destination.
    pub items: Vec<Key>,
    /// Action ids represented (surfaced) by the presentation.
    pub actions: Vec<Key>,
    /// Ids deliberately omitted for responsive reasons — declared, not silent.
    pub omitted: Vec<Key>,
    /// Human-readable notes on degraded / unsupported presentation decisions.
    pub degraded: Vec<String>,
}

impl PresentationReceipt {
    /// Start an empty receipt for a style at an active destination.
    pub fn new(style: &'static str, active_destination: Key) -> Self {
        Self {
            style,
            active_destination,
            selected: None,
            destinations: Vec::new(),
            items: Vec::new(),
            actions: Vec::new(),
            omitted: Vec::new(),
            degraded: Vec::new(),
        }
    }

    fn declared_omitted(&self, key: &Key) -> bool {
        self.omitted.contains(key)
    }

    /// Check this receipt against the application's required semantics. Returns
    /// every violation found (empty = the presentation is semantically faithful).
    pub fn check(&self, required: &RequiredSemantics) -> Vec<LawViolation> {
        let mut violations = Vec::new();

        // Active destination and selection are fixed facts, not presentation choices.
        if Some(&self.active_destination) != required.active_destination.as_ref() {
            violations.push(LawViolation::WrongActiveDestination {
                expected: required.active_destination.clone(),
                found: self.active_destination.clone(),
            });
        }
        if self.selected != required.selected {
            violations.push(LawViolation::WrongSelection {
                expected: required.selected.clone(),
                found: self.selected.clone(),
            });
        }

        // Every destination must be reachable; destinations may never be omitted.
        for key in &required.destinations {
            if !self.destinations.contains(key) {
                violations.push(LawViolation::MissingDestination(key.clone()));
            }
        }

        // Essential items may never be omitted; non-essential items must be either
        // represented or *declared* omitted (never silently lost).
        for key in &required.essential_items {
            if self.declared_omitted(key) {
                violations.push(LawViolation::EssentialOmitted(key.clone()));
            } else if !self.items.contains(key) {
                violations.push(LawViolation::SilentLoss(key.clone()));
            }
        }
        for key in &required.all_items {
            if required.essential_items.contains(key) {
                continue;
            }
            if !self.items.contains(key) && !self.declared_omitted(key) {
                violations.push(LawViolation::SilentLoss(key.clone()));
            }
        }

        // Reachable actions must be represented or declared omitted.
        for key in &required.actions {
            if !self.actions.contains(key) && !self.declared_omitted(key) {
                violations.push(LawViolation::MissingAction(key.clone()));
            }
        }

        // A grammar may not invent an id the application never defined.
        let defined: Vec<&Key> = required
            .destinations
            .iter()
            .chain(required.all_items.iter())
            .chain(required.actions.iter())
            .collect();
        for key in self
            .destinations
            .iter()
            .chain(self.items.iter())
            .chain(self.actions.iter())
        {
            if !defined.contains(&key) {
                violations.push(LawViolation::Invented(key.clone()));
            }
        }

        violations
    }

    /// Convenience: does this presentation satisfy the preservation law?
    pub fn preserves(&self, required: &RequiredSemantics) -> bool {
        self.check(required).is_empty()
    }

    /// The semantic projection π_σ: the represented ids this presentation claims,
    /// ignoring layout. Two presentations of the same state preserve semantics iff
    /// their projections (plus declared omissions) cover the same required set.
    pub fn represented(&self) -> impl Iterator<Item = &Key> {
        self.destinations
            .iter()
            .chain(self.items.iter())
            .chain(self.actions.iter())
    }
}
