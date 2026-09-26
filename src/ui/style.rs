//! Semantic design roles for the experimental UI layer.
//!
//! Roles carry application intent. A [`super::Skin`] translates that intent
//! into the existing [`crate::Style`] and [`crate::Theme`] vocabulary.

/// The meaning of a piece of information, independent of its color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Tone {
    #[default]
    Neutral,
    Accent,
    Info,
    Success,
    Warning,
    Danger,
}

/// Information hierarchy within a semantic tone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Emphasis {
    Faint,
    Muted,
    #[default]
    Normal,
    Strong,
}

/// The spacing budget. Constrained environments may resolve to Compact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Density {
    Compact,
    #[default]
    Normal,
    Spacious,
}

/// Visual depth; this does not change application state or z-order by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Elevation {
    #[default]
    Flat,
    Raised,
    Overlay,
}
