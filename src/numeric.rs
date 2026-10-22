//! Validated numeric editing.
//!
//! A numeric control must never *silently* rewrite the value a user committed.
//! Typing `99` into a 0..=10 field is an error to report, not a quiet clamp to
//! `10`; a non-finite result is refused, not stored. This module owns that
//! discipline and nothing else: the consumer keeps the authoritative value, and
//! only a valid [`NumericEdit::Commit`] ever hands it a new one.
//!
//! Two pieces:
//!
//! * [`NumericField`] — a pure validation schema. [`validate`](NumericField::validate)
//!   and [`parse`](NumericField::parse) accept or reject; they never clamp.
//! * [`NumericInput`] — a controlled editor. It reuses [`TextInputState`] as its
//!   edit buffer and an [`EditSession`] for the begin/commit/cancel episode, and
//!   emits typed [`NumericEdit`] actions. The keyboard path ([`set_text`](NumericInput::set_text))
//!   and the pointer/stepper path ([`step`](NumericInput::step)) flow through the
//!   same vocabulary and the same validation.
//!
//! Everything here is pure: no I/O, no terminal, no global state.

use crate::edit::{CancelReason, EditId, EditSession};
use crate::input::TextInputState;

/// Why a numeric value, or its text form, was rejected.
///
/// The offending value is reported rather than silently corrected, so the caller
/// decides what to do — the editor never clamps or coerces an authoritative value.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum NumericError {
    /// The text was empty or whitespace only.
    Empty,
    /// The text was not a number.
    Unparseable,
    /// The value was `NaN` or infinite.
    NonFinite,
    /// The value was below the inclusive minimum.
    Below {
        /// The rejected value.
        value: f64,
        /// The minimum it fell under.
        min: f64,
    },
    /// The value was above the inclusive maximum.
    Above {
        /// The rejected value.
        value: f64,
        /// The maximum it exceeded.
        max: f64,
    },
}

impl std::fmt::Display for NumericError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NumericError::Empty => f.write_str("no value entered"),
            NumericError::Unparseable => f.write_str("not a number"),
            NumericError::NonFinite => f.write_str("value must be finite"),
            NumericError::Below { value, min } => {
                write!(f, "{value} is below the minimum {min}")
            }
            NumericError::Above { value, max } => {
                write!(f, "{value} is above the maximum {max}")
            }
        }
    }
}

impl std::error::Error for NumericError {}

/// A validation schema for a single numeric field.
///
/// Pure and cheap to copy. It describes the legal values — an optional inclusive
/// range and an optional step — and it only ever *accepts or rejects*; it has no
/// authoritative value of its own and never clamps one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumericField {
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
}

impl NumericField {
    /// An unbounded field with no step: any finite value validates.
    pub fn new() -> Self {
        Self {
            min: None,
            max: None,
            step: None,
        }
    }

    /// Sets the inclusive minimum.
    pub fn with_min(mut self, min: f64) -> Self {
        self.min = Some(min);
        self
    }

    /// Sets the inclusive maximum.
    pub fn with_max(mut self, max: f64) -> Self {
        self.max = Some(max);
        self
    }

    /// Sets both inclusive bounds at once.
    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }

    /// Sets the step used by [`NumericInput::step`].
    pub fn with_step(mut self, step: f64) -> Self {
        self.step = Some(step);
        self
    }

    /// The inclusive minimum, if any.
    pub fn min(&self) -> Option<f64> {
        self.min
    }

    /// The inclusive maximum, if any.
    pub fn max(&self) -> Option<f64> {
        self.max
    }

    /// The step, if any.
    pub fn step(&self) -> Option<f64> {
        self.step
    }

    /// Validates a value: rejects non-finite and out-of-range inputs.
    ///
    /// Bounds are inclusive. On success returns the value unchanged; on failure
    /// returns the specific [`NumericError`] carrying the offending value. It
    /// **never** returns a clamped value.
    pub fn validate(&self, value: f64) -> Result<f64, NumericError> {
        if !value.is_finite() {
            return Err(NumericError::NonFinite);
        }
        if let Some(min) = self.min {
            if value < min {
                return Err(NumericError::Below { value, min });
            }
        }
        if let Some(max) = self.max {
            if value > max {
                return Err(NumericError::Above { value, max });
            }
        }
        Ok(value)
    }

    /// Parses text, then validates it.
    ///
    /// Empty or whitespace-only text is [`NumericError::Empty`]; text that is not
    /// a number is [`NumericError::Unparseable`]; a parsed non-finite or
    /// out-of-range value is rejected by [`validate`](NumericField::validate).
    pub fn parse(&self, text: &str) -> Result<f64, NumericError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(NumericError::Empty);
        }
        match trimmed.parse::<f64>() {
            Ok(value) => self.validate(value),
            Err(_) => Err(NumericError::Unparseable),
        }
    }
}

impl Default for NumericField {
    fn default() -> Self {
        Self::new()
    }
}

/// A typed numeric edit action.
///
/// `Begin`/`Preview`/`Commit`/`Cancel` mirror the generic edit episode;
/// `Invalid` reports a rejected input that made **no** authoritative change and
/// left the episode open. The authoritative value lives with the consumer — only
/// `Commit` hands back a new one, and only when it is valid.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum NumericEdit {
    /// The edit episode opened.
    Begin {
        /// The episode's identity.
        id: EditId,
    },
    /// A valid, non-authoritative in-progress value.
    Preview {
        /// The episode's identity.
        id: EditId,
        /// The proposed value.
        value: f64,
    },
    /// The authoritative ending: apply this value.
    Commit {
        /// The episode's identity.
        id: EditId,
        /// The committed value.
        value: f64,
    },
    /// The episode ended with no authoritative change.
    Cancel {
        /// The episode's identity.
        id: EditId,
        /// Why it ended.
        reason: CancelReason,
    },
    /// The input was rejected; the episode stays open and nothing changed.
    Invalid {
        /// The episode's identity.
        id: EditId,
        /// Why the input was refused.
        error: NumericError,
    },
}

/// A controlled numeric editor.
///
/// It drives an [`EditSession`] for the begin/commit/cancel episode and reuses a
/// [`TextInputState`] as the edit buffer — it does not reimplement text editing.
/// The consumer owns the authoritative value; this type only proposes
/// ([`NumericEdit::Preview`]), reports refusals ([`NumericEdit::Invalid`]), and
/// hands back a value exactly once on a valid [`commit`](NumericInput::commit).
#[derive(Debug, Clone)]
pub struct NumericInput {
    field: NumericField,
    session: EditSession<f64>,
    buffer: TextInputState,
}

impl NumericInput {
    /// A fresh editor for `field`, not yet editing.
    pub fn new(field: NumericField) -> Self {
        Self {
            field,
            session: EditSession::new(),
            buffer: TextInputState::new(),
        }
    }

    /// The validation schema in force.
    pub fn field(&self) -> &NumericField {
        &self.field
    }

    /// `true` while an edit episode is open.
    pub fn is_editing(&self) -> bool {
        self.session.is_open()
    }

    /// The current edit-buffer text.
    pub fn buffer(&self) -> &str {
        &self.buffer.text
    }

    /// Opens an edit episode seeded with `current`, returning [`NumericEdit::Begin`].
    ///
    /// Returns `None` if an episode is already open — the open episode is left
    /// untouched; a second begin cannot clobber an edit in flight.
    pub fn begin(&mut self, id: EditId, current: f64) -> Option<NumericEdit> {
        if self.session.is_open() {
            return None;
        }
        self.session.begin(id, None).ok();
        self.buffer = TextInputState::with_text(format!("{current}"));
        Some(NumericEdit::Begin { id })
    }

    /// Replaces the buffer text (a keystroke batch, paste, or pointer-set value).
    ///
    /// A valid value previews; an invalid one is reported as
    /// [`NumericEdit::Invalid`]. Either way the episode stays open. Returns `None`
    /// if no episode is open.
    pub fn set_text(&mut self, text: &str) -> Option<NumericEdit> {
        let id = self.session.current()?;
        self.buffer = TextInputState::with_text(text);
        match self.field.parse(&self.buffer.text) {
            Ok(value) => Some(NumericEdit::Preview { id, value }),
            Err(error) => Some(NumericEdit::Invalid { id, error }),
        }
    }

    /// Steps the buffer value by `n` steps (the pointer/arrow affordance).
    ///
    /// Uses the field's [`step`](NumericField::step), defaulting to `1.0`. A step
    /// that lands out of range is [`NumericEdit::Invalid`] — never clamped to the
    /// boundary — and leaves the buffer unchanged. Returns `None` if no episode
    /// is open; if the buffer does not currently parse, the parse error is
    /// surfaced.
    pub fn step(&mut self, n: i32) -> Option<NumericEdit> {
        let id = self.session.current()?;
        let current = match self.field.parse(&self.buffer.text) {
            Ok(value) => value,
            Err(error) => return Some(NumericEdit::Invalid { id, error }),
        };
        let stepped = current + f64::from(n) * self.field.step().unwrap_or(1.0);
        match self.field.validate(stepped) {
            Ok(value) => {
                self.buffer = TextInputState::with_text(format!("{value}"));
                Some(NumericEdit::Preview { id, value })
            }
            Err(error) => Some(NumericEdit::Invalid { id, error }),
        }
    }

    /// Commits the buffer.
    ///
    /// A valid value closes the episode with [`NumericEdit::Commit`]; an invalid
    /// one is refused as [`NumericEdit::Invalid`] and the episode stays open — it
    /// will not commit a bad or clamped value. Returns `None` if no episode is open.
    pub fn commit(&mut self) -> Option<NumericEdit> {
        let id = self.session.current()?;
        match self.field.parse(&self.buffer.text) {
            Ok(value) => {
                self.session.commit(value);
                Some(NumericEdit::Commit { id, value })
            }
            Err(error) => Some(NumericEdit::Invalid { id, error }),
        }
    }

    /// Cancels the episode with `reason`, making no authoritative change.
    ///
    /// Returns `None` if no episode is open. A commit arriving after a cancel has
    /// nothing open and commits nothing.
    pub fn cancel(&mut self, reason: CancelReason) -> Option<NumericEdit> {
        let id = self.session.current()?;
        self.session.cancel(reason);
        Some(NumericEdit::Cancel { id, reason })
    }
}
