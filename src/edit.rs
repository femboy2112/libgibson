//! Grouped, reversible edit lifecycles and a bounded undo/redo history.
//!
//! An interactive edit is not one event; it is an *episode* — a begin, any
//! number of previews, and exactly one authoritative ending (a commit or a
//! cancel). This module owns that grouping and ordering. It is deliberately
//! blind to *meaning*: the consumer supplies the intent type `I` and the
//! reversible command type `C`, owns the model, and reads the phases the
//! library emits. We guarantee the shape of the episode; the consumer decides
//! what a move or a selection actually does.
//!
//! Two instruments live here:
//!
//! * [`EditSession`] — the one-at-a-time episode grouper. It emits
//!   [`EditPhase`] values and refuses a second concurrent begin.
//! * [`UndoLog`] — a bounded, forking undo/redo stack over a [`Reversible`]
//!   command. Recording a fresh command amputates the redo branch; a failed
//!   revert is surfaced, never swallowed.
//!
//! Everything here is pure: no clock, no I/O, no global state. The same calls
//! in the same order produce the same phases every time — which is exactly what
//! lets a gesture layer sit on top of it and stay testable.

use std::marker::PhantomData;

/// An opaque identity for one edit episode.
///
/// The library mints it and threads it through every [`EditPhase`] of the same
/// episode; the consumer only compares it. Begin, its previews, and its one
/// ending all carry the same `EditId` — that is how a late, stale commit is
/// recognised and ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditId(pub u64);

/// Why an edit episode ended without committing.
///
/// `#[non_exhaustive]`: the roster of things that can interrupt a drag grows
/// (a new modal regime, a device event), so downstream `match`es carry a `_`
/// arm and adding a reason stays a non-breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CancelReason {
    /// The user pressed Escape.
    Escape,
    /// The surface lost focus, so the matching release may never arrive.
    FocusLost,
    /// The surface resized, invalidating the geometry the edit was anchored to.
    Resize,
    /// A modal layer opened over the captured target and took the surface.
    ModalTakeover,
    /// The captured target was removed out from under the edit.
    TargetRemoved,
    /// A newer edit superseded this one before it could end.
    Superseded,
}

/// One observable moment in an edit episode, carrying the consumer's intent `I`.
///
/// The phases of a single episode share one [`EditId`]: a `Begin`, zero or more
/// `Preview`s, then exactly one `Commit` **or** one `Cancel`. This is the only
/// surface a consumer reacts to — the session's internals are never exposed.
///
/// Not `Eq`: `I` may carry an `f64` (a world coordinate), which has no total
/// equality. `PartialEq` is enough for the tests that compare phases, and it is
/// honest about floats rather than lying with a fabricated `Eq`.
#[derive(Debug, Clone, PartialEq)]
pub enum EditPhase<I> {
    /// The episode opened. `label` is an optional human name for an undo entry.
    Begin {
        /// The episode's identity.
        id: EditId,
        /// An optional human-readable label (e.g. for an undo menu).
        label: Option<String>,
    },
    /// A non-authoritative, in-flight proposal. Fires any number of times.
    Preview {
        /// The episode's identity.
        id: EditId,
        /// The consumer's current proposed intent.
        intent: I,
    },
    /// The authoritative ending: apply this intent.
    Commit {
        /// The episode's identity.
        id: EditId,
        /// The consumer's final intent.
        intent: I,
    },
    /// The episode ended with no authoritative change.
    Cancel {
        /// The episode's identity.
        id: EditId,
        /// Why it ended.
        reason: CancelReason,
    },
}

/// Why an [`EditSession`] rejected a call.
///
/// `#[non_exhaustive]`: more preconditions may appear; downstream `match`es
/// carry a `_` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditError {
    /// A begin was attempted while an episode was already open. The open
    /// episode is left entirely untouched.
    AlreadyOpen,
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditError::AlreadyOpen => f.write_str("an edit episode is already open"),
        }
    }
}

impl std::error::Error for EditError {}

/// A one-at-a-time grouper for edit episodes over an intent type `I`.
///
/// At most one episode is open at a time. [`begin`](EditSession::begin) opens
/// one, [`preview`](EditSession::preview) emits proposals while it is open, and
/// [`commit`](EditSession::commit) or [`cancel`](EditSession::cancel) closes it.
/// A call that has no open episode to act on emits `None` rather than
/// fabricating a phase — that is what makes a late, post-cancel commit inert.
pub struct EditSession<I> {
    // The whole of the mutable state: the open episode's id and its label, or
    // `None`. One `Option` is the entire "at most one open edit" invariant —
    // there is no room for a second episode to exist.
    open: Option<(EditId, Option<String>)>,
    // Marks `I` without owning one and without borrowing the `I: Trait` bounds a
    // derive would smuggle in through it. `fn() -> I` keeps the session `Send`
    // and `Sync` whatever `I` is — the session never touches an `I` value.
    _intent: PhantomData<fn() -> I>,
}

// Hand-cut Debug/Clone/Default: deriving them would demand `I: Debug` /
// `I: Clone` / `I: Default` through the PhantomData even though no `I` is ever
// stored. We refuse that spurious bound.
impl<I> std::fmt::Debug for EditSession<I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditSession")
            .field("open", &self.open)
            .finish()
    }
}

impl<I> Clone for EditSession<I> {
    fn clone(&self) -> Self {
        Self {
            open: self.open.clone(),
            _intent: PhantomData,
        }
    }
}

impl<I> Default for EditSession<I> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I> EditSession<I> {
    /// A session with no episode open.
    pub fn new() -> Self {
        Self {
            open: None,
            _intent: PhantomData,
        }
    }

    /// `true` while an episode is open.
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// The open episode's id, or `None`.
    pub fn current(&self) -> Option<EditId> {
        self.open.as_ref().map(|(id, _)| *id)
    }

    /// Opens an episode, returning its `Begin` phase.
    ///
    /// If one is already open this is [`EditError::AlreadyOpen`] and the open
    /// episode is left exactly as it was — a second begin cannot clobber an
    /// edit in flight. The label is cloned once: the session keeps a copy for
    /// its records and hands the original back in the phase.
    pub fn begin(&mut self, id: EditId, label: Option<String>) -> Result<EditPhase<I>, EditError> {
        if self.open.is_some() {
            return Err(EditError::AlreadyOpen);
        }
        self.open = Some((id, label.clone()));
        Ok(EditPhase::Begin { id, label })
    }

    /// Emits a `Preview` for the open episode, or `None` if none is open.
    ///
    /// A preview is a proposal, not a decision: it never changes the open
    /// state, so previews can fire as fast as the pointer moves.
    pub fn preview(&mut self, intent: I) -> Option<EditPhase<I>> {
        self.current().map(|id| EditPhase::Preview { id, intent })
    }

    /// Closes the open episode with a `Commit`, or returns `None` if none is
    /// open. Exactly one commit per episode: once closed, a second commit (or a
    /// late preview) finds nothing open and emits `None`.
    pub fn commit(&mut self, intent: I) -> Option<EditPhase<I>> {
        self.open
            .take()
            .map(|(id, _)| EditPhase::Commit { id, intent })
    }

    /// Closes the open episode with a `Cancel`, or returns `None` if none is
    /// open. Cancel means no authoritative change — a commit arriving after it
    /// has nothing to close and commits nothing.
    pub fn cancel(&mut self, reason: CancelReason) -> Option<EditPhase<I>> {
        self.open
            .take()
            .map(|(id, _)| EditPhase::Cancel { id, reason })
    }
}

/// A command that can be applied to a model and faithfully reverted.
///
/// The consumer owns both the command and the model. The log calls
/// [`revert`](Reversible::revert) to undo and [`apply`](Reversible::apply) to
/// redo; a revert that detects the model has diverged underneath it should
/// return `Err` rather than guess — the log surfaces that error intact.
pub trait Reversible {
    /// The consumer-owned model this command mutates.
    type Model;
    /// The error a faithless apply/revert reports.
    type Error;
    /// Applies the command forward.
    fn apply(&self, model: &mut Self::Model) -> Result<(), Self::Error>;
    /// Reverts the command, inverting a prior [`apply`](Reversible::apply).
    fn revert(&self, model: &mut Self::Model) -> Result<(), Self::Error>;
}

/// Whether a [`record`](UndoLog::record) displaced older history.
///
/// `#[non_exhaustive]`: future bounding policies may report differently;
/// downstream `match`es carry a `_` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EvictionStatus {
    /// The new entry fit within capacity; nothing was dropped.
    Kept,
    /// The log was at capacity; this many oldest entries were evicted.
    Evicted(usize),
}

/// A bounded, forking undo/redo history of [`Reversible`] commands.
///
/// Two stacks: `done` (undoable, oldest at the front) and `undone` (redoable).
/// [`record`](UndoLog::record) pushes onto `done` and discards the redo branch
/// — history forks at the present, it does not keep a dead future around.
/// Capacity bounds `done` from the front, so the oldest edits age out first.
#[derive(Debug, Clone)]
pub struct UndoLog<C> {
    done: Vec<C>,
    undone: Vec<C>,
    cap: usize,
}

impl<C> Default for UndoLog<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C> UndoLog<C> {
    /// The default undo depth when none is specified.
    pub const DEFAULT_CAPACITY: usize = 128;

    /// A log with [`DEFAULT_CAPACITY`](UndoLog::DEFAULT_CAPACITY) entries.
    pub fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }

    /// A log bounded to `max_entries` undoable entries, floored at 1 — a log
    /// that can hold nothing would silently drop every edit, so we refuse a
    /// zero capacity.
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            done: Vec::new(),
            undone: Vec::new(),
            cap: max_entries.max(1),
        }
    }

    /// The maximum number of undoable entries this log retains.
    pub fn capacity(&self) -> usize {
        self.cap
    }

    /// How many entries can currently be undone.
    pub fn undo_depth(&self) -> usize {
        self.done.len()
    }

    /// How many entries can currently be redone.
    pub fn redo_depth(&self) -> usize {
        self.undone.len()
    }

    /// `true` if there is anything to undo.
    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    /// `true` if there is anything to redo.
    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    /// Records an already-applied command as the newest undoable entry.
    ///
    /// Recording forks history: the redo branch is discarded, because a fresh
    /// edit after an undo invalidates any future that undo had parked. If this
    /// pushes `done` over capacity, the oldest entries are drained from the
    /// front and reported as [`EvictionStatus::Evicted`]; otherwise
    /// [`EvictionStatus::Kept`]. The caller is expected to have already applied
    /// the command to the model — the log only remembers how to reverse it.
    pub fn record(&mut self, command: C) -> EvictionStatus {
        self.done.push(command);
        self.undone.clear();
        let excess = self.done.len().saturating_sub(self.cap);
        if excess > 0 {
            self.done.drain(0..excess);
            EvictionStatus::Evicted(excess)
        } else {
            EvictionStatus::Kept
        }
    }
}

impl<C: Reversible> UndoLog<C> {
    /// Reverts the newest undoable entry, moving it to the redo branch on
    /// success. `None` if there is nothing to undo.
    ///
    /// A revert that fails (the model diverged under the log) surfaces its
    /// `Err` and *keeps the entry on `done`*: the failure is the caller's to
    /// resolve, and we will not quietly lose history over it. The result is
    /// computed while the immutable borrow of the entry is live, then the stack
    /// is mutated — so the borrow checker stays satisfied without a clone.
    pub fn undo(&mut self, model: &mut C::Model) -> Option<Result<(), C::Error>> {
        let result = match self.done.last() {
            None => return None,
            Some(cmd) => cmd.revert(model),
        };
        match result {
            Ok(()) => {
                let c = self.done.pop().unwrap();
                self.undone.push(c);
                Some(Ok(()))
            }
            Err(e) => Some(Err(e)),
        }
    }

    /// Re-applies the newest redoable entry, moving it back to the undo branch
    /// on success. `None` if there is nothing to redo. Symmetric with
    /// [`undo`](UndoLog::undo): a failed apply surfaces `Err` and leaves the
    /// entry on the redo branch.
    pub fn redo(&mut self, model: &mut C::Model) -> Option<Result<(), C::Error>> {
        let result = match self.undone.last() {
            None => return None,
            Some(cmd) => cmd.apply(model),
        };
        match result {
            Ok(()) => {
                let c = self.undone.pop().unwrap();
                self.done.push(c);
                Some(Ok(()))
            }
            Err(e) => Some(Err(e)),
        }
    }
}
