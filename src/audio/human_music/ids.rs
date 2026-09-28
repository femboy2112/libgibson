//! **Typed identities** shared by the plan, the performance and the Score's provenance.
//!
//! Round VII grew several id spaces that were all raw `u32`s — action ids, the `pays` of an
//! answer, the `pays` of a discourse phrase (an *obligation* id), `u32::MAX` sentinels and
//! `action: 0` placeholders that silently aliased the first real action. A receipt is only as
//! exact as the identity it cites, so each space gets its own newtype: an [`ActionId`] can never
//! be mistaken for an [`ObligationId`], and an event in the Score can carry the exact action,
//! interaction and material it realizes ([`ActionStamp`], see
//! [`super::score::Provenance`]).

use std::fmt;

macro_rules! id_type {
    ($(#[$m:meta])* $name:ident, $prefix:literal) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u32);

        impl $name {
            /// The id as a vector index.
            pub fn index(self) -> usize {
                self.0 as usize
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!($prefix, "{}"), self.0)
            }
        }
    };
}

id_type!(
    /// A [`super::action::MusicalAction`] — its index in [`super::action::ActionPlan::actions`].
    ActionId,
    "a"
);
id_type!(
    /// An [`super::interaction::Interaction`] — its index in the performance's interaction list.
    InteractionId,
    "i"
);
id_type!(
    /// A piece of [`super::material::InteractionMaterial`] — its index in the performance's
    /// material bank.
    MaterialId,
    "m"
);
id_type!(
    /// A discourse obligation (a debt one phrase opens and a later one must settle).
    ObligationId,
    "ob"
);

/// The actions one Score event realizes — at most three (a lead note can be part of a call, a
/// fragment and a resolution at once). `Copy`, so [`super::score::Provenance`] stays `Copy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ActionStamp {
    ids: [Option<ActionId>; 3],
}

impl ActionStamp {
    /// No action.
    pub const NONE: ActionStamp = ActionStamp { ids: [None; 3] };

    /// A stamp carrying exactly `id`.
    pub fn of(id: ActionId) -> ActionStamp {
        ActionStamp::NONE.with(id)
    }

    /// This stamp plus `id` (a no-op when `id` is already present or all three slots are full).
    #[must_use]
    pub fn with(mut self, id: ActionId) -> ActionStamp {
        if self.has(id) {
            return self;
        }
        if let Some(slot) = self.ids.iter_mut().find(|s| s.is_none()) {
            *slot = Some(id);
        }
        self
    }

    /// This stamp plus `id`, when there is one.
    #[must_use]
    pub fn with_opt(self, id: Option<ActionId>) -> ActionStamp {
        match id {
            Some(id) => self.with(id),
            None => self,
        }
    }

    /// Whether the event realizes `id`.
    pub fn has(&self, id: ActionId) -> bool {
        self.ids.contains(&Some(id))
    }

    /// The first action stamped.
    pub fn primary(&self) -> Option<ActionId> {
        self.ids[0]
    }

    /// Every action stamped, in stamping order.
    pub fn iter(&self) -> impl Iterator<Item = ActionId> + '_ {
        self.ids.iter().flatten().copied()
    }

    /// Whether no action is stamped.
    pub fn is_empty(&self) -> bool {
        self.ids[0].is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stamp_holds_three_distinct_actions_and_ignores_repeats() {
        let s = ActionStamp::of(ActionId(4))
            .with(ActionId(4))
            .with(ActionId(9))
            .with(ActionId(1))
            .with(ActionId(7));
        assert_eq!(
            s.iter().collect::<Vec<_>>(),
            vec![ActionId(4), ActionId(9), ActionId(1)]
        );
        assert!(s.has(ActionId(1)) && !s.has(ActionId(7)));
        assert_eq!(s.primary(), Some(ActionId(4)));
        assert!(ActionStamp::NONE.is_empty() && !s.is_empty());
        assert_eq!(format!("{} {}", ActionId(3), ObligationId(2)), "a3 ob2");
    }
}
