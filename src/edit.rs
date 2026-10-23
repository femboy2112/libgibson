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

use crate::input::{KeyCode, KeyEvent, KeyModifiers};
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
///
/// # Contract — the all-or-nothing precondition
///
/// `apply` and `revert` must be **all-or-nothing**: if either returns `Err`, it
/// must have left the model **unchanged**. This is the precondition every
/// atomicity claim in this module rests on. An implementation that mutates the
/// model and *then* reports `Err` (a "mutate-and-fail") breaks the atomicity a
/// [`Compound`] can offer, and a compound **cannot detect** that violation — it
/// trusts the `Err` to mean "nothing happened." What a compound *can* still
/// promise even when a sub-command fails, and what it explicitly cannot, is
/// spelled out by [`CompoundError`]. If you cannot meet this precondition
/// cheaply, apply against a clone and commit only on success rather than relying
/// on a compound's rollback.
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
        let result = self.done.last()?.revert(model);
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
        let result = self.undone.last()?.apply(model);
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

/// A group of sub-commands that applies and reverts as ONE reversible unit.
///
/// Every step of a drag, or a batch of edits that logically belong together,
/// becomes one [`Compound`]; recorded in an [`UndoLog`] it collapses to a single
/// undo/redo. [`apply`](Compound::apply) runs the sub-commands in order;
/// [`revert`](Compound::revert) runs them in REVERSE order.
///
/// # Atomicity — exactly what is and is not promised
///
/// When a sub-command fails partway, the compound rolls back the ones already
/// done (apply unwinds newest-first; revert re-applies the tail it had walked
/// back). **Under the [`Reversible`] all-or-nothing precondition**, a *clean*
/// rollback — every compensation succeeding — leaves the model exactly as it was,
/// and the failure is reported as [`CompoundError::SubCommand`].
///
/// But a compensation can itself fail. The old implementation discarded those
/// errors (`let _ = revert(..)`) and returned the trigger error alone, so a
/// half-rolled-back, corrupted model was indistinguishable from a clean one. It
/// no longer does: a compensation failure is collected and surfaced as
/// [`CompoundError::RollbackFailed`], which names every compensation that failed
/// and admits the model is in a known-partial state. **Atomicity is a
/// conditional guarantee, not a magic one** (Laws L5/L7): a compound is the
/// partial inverse of an *admitted* transition, and when it cannot invert, it
/// says so rather than lying. For unconditional atomicity over an untrusted
/// `Reversible`, apply against a clone and commit only on full success.
///
/// A compound does not know what its sub-commands *mean* — it owns only the order
/// and this honest accounting of what rollback achieved.
#[derive(Debug, Clone, PartialEq)]
pub struct Compound<C> {
    commands: Vec<C>,
}

impl<C> Compound<C> {
    /// Groups `commands` into one reversible unit (an empty group is a valid identity).
    pub fn new(commands: Vec<C>) -> Self {
        Self { commands }
    }

    /// The number of sub-commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Whether the group has no sub-commands.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// The sub-commands in forward (apply) order.
    pub fn commands(&self) -> &[C] {
        &self.commands
    }
}

/// What went wrong when a [`Compound`] could not complete — and, decisively,
/// whether the model was put back.
///
/// Under the [`Reversible`] all-or-nothing precondition:
///
/// * [`SubCommand`](CompoundError::SubCommand) — one sub-command failed and every
///   compensation then succeeded, so the model is back to its exact
///   pre-operation state. Atomicity held; the whole compound was a clean no-op.
/// * [`RollbackFailed`](CompoundError::RollbackFailed) — one sub-command failed
///   **and at least one compensation also failed**. The model is in a
///   known-partial state that could not be unwound. The variant carries exactly
///   which compensations failed (by index, in attempt order), so the caller can
///   decide what to do. Atomicity was violated, and this type refuses to dress
///   that up as success (Laws L5/L7 — a failed compensation is never certified
///   as a clean rollback, and the condition is never silently erased).
///
/// `#[non_exhaustive]`: a future checkpoint-based variant (e.g. "restored from a
/// snapshot") may be added without breaking downstream `match`es.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum CompoundError<E> {
    /// A sub-command failed; the rollback that followed was clean, so the model
    /// is restored to its pre-operation state.
    SubCommand {
        /// The forward-order index of the sub-command whose apply/revert failed.
        index: usize,
        /// That sub-command's own error.
        error: E,
    },
    /// A sub-command failed *and* the rollback could not fully complete; the
    /// model is left in a known-partial state.
    RollbackFailed {
        /// The index of the sub-command whose failure triggered the rollback.
        trigger_index: usize,
        /// The triggering sub-command's own error.
        trigger_error: E,
        /// Every compensation that *also* failed, as `(index, error)` in attempt
        /// order. Non-empty whenever this variant is produced — that non-emptiness
        /// is the whole difference from [`SubCommand`](CompoundError::SubCommand).
        failures: Vec<(usize, E)>,
    },
}

impl<C: Reversible> Reversible for Compound<C> {
    type Model = C::Model;
    // The error type is richer than a sub-command's: a compound can fail in a way
    // no single `C::Error` can express — namely "the rollback itself failed" —
    // and this type is what makes that sayable instead of swallowed.
    type Error = CompoundError<C::Error>;

    fn apply(&self, model: &mut C::Model) -> Result<(), CompoundError<C::Error>> {
        for (i, cmd) in self.commands.iter().enumerate() {
            if let Err(trigger_error) = cmd.apply(model) {
                // Sub-command `i` failed. Under the Reversible precondition it
                // mutated nothing, so `[..i]` is exactly the set that landed. Roll
                // them back newest-first, COLLECTING every compensation that fails
                // instead of discarding it (the old `let _ = revert(..)` is the
                // whole bug: it let a corrupted model pass for a clean rollback).
                let failures = unwind(
                    self.commands[..i].iter().enumerate().rev(),
                    model,
                    Op::Revert,
                );
                return Err(resolve(i, trigger_error, failures));
            }
        }
        Ok(())
    }

    fn revert(&self, model: &mut C::Model) -> Result<(), CompoundError<C::Error>> {
        for (i, cmd) in self.commands.iter().enumerate().rev() {
            if let Err(trigger_error) = cmd.revert(model) {
                // Revert `i` failed. The tail `[i+1..]` was already reverted; put
                // the model back where this failed revert found it by re-applying
                // it forward, again collecting — never swallowing — any failure.
                let tail = self.commands.iter().enumerate().skip(i + 1);
                let failures = unwind(tail, model, Op::Apply);
                return Err(resolve(i, trigger_error, failures));
            }
        }
        Ok(())
    }
}

/// Which direction a compensation runs during a rollback.
#[derive(Clone, Copy)]
enum Op {
    Apply,
    Revert,
}

/// Runs the compensating operation over `items` (each an absolute `(index, &C)`),
/// never short-circuiting, and returns every failure as `(index, error)`. The
/// goal of a rollback is to restore as much as possible *and* report everything
/// that could not be — so one failing compensation must not stop the rest.
fn unwind<'c, C: Reversible + 'c>(
    items: impl Iterator<Item = (usize, &'c C)>,
    model: &mut C::Model,
    op: Op,
) -> Vec<(usize, C::Error)> {
    let mut failures = Vec::new();
    for (index, cmd) in items {
        let outcome = match op {
            Op::Apply => cmd.apply(model),
            Op::Revert => cmd.revert(model),
        };
        if let Err(e) = outcome {
            failures.push((index, e));
        }
    }
    failures
}

/// A clean rollback (no compensation failed) reports the trigger alone; any
/// compensation failure escalates to [`CompoundError::RollbackFailed`].
fn resolve<E>(
    trigger_index: usize,
    trigger_error: E,
    failures: Vec<(usize, E)>,
) -> CompoundError<E> {
    if failures.is_empty() {
        CompoundError::SubCommand {
            index: trigger_index,
            error: trigger_error,
        }
    } else {
        CompoundError::RollbackFailed {
            trigger_index,
            trigger_error,
            failures,
        }
    }
}

/// What an undo keybinding resolves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoIntent {
    /// Reverse the most recent command.
    Undo,
    /// Re-apply the most recently undone command.
    Redo,
}

/// Whether a binding requires the Shift modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShiftReq {
    Absent,
    Present,
    Any,
}

impl ShiftReq {
    fn matches(self, effective_shift: bool) -> bool {
        match self {
            ShiftReq::Absent => !effective_shift,
            ShiftReq::Present => effective_shift,
            ShiftReq::Any => true,
        }
    }
}

#[derive(Debug, Clone)]
struct Binding {
    letter: char,
    shift: ShiftReq,
    intent: UndoIntent,
}

/// Translates a [`KeyEvent`] into an [`UndoIntent`] — and nothing else.
///
/// It only classifies; it performs no undo. The standard map binds Ctrl-Z to
/// [`UndoIntent::Undo`] and both Ctrl-Shift-Z and Ctrl-Y to [`UndoIntent::Redo`].
/// It is robust to the terminal ambiguity where Shift+Z arrives either as
/// `Char('z')` with the SHIFT flag or as `Char('Z')` with no flag: an uppercase
/// letter counts as Shift. A chord with Alt held is never an undo binding.
#[derive(Debug, Clone)]
pub struct UndoKeymap {
    bindings: Vec<Binding>,
}

impl UndoKeymap {
    /// The conventional bindings: Ctrl-Z = undo, Ctrl-Shift-Z / Ctrl-Y = redo.
    pub fn standard() -> Self {
        Self {
            bindings: vec![
                Binding {
                    letter: 'z',
                    shift: ShiftReq::Absent,
                    intent: UndoIntent::Undo,
                },
                Binding {
                    letter: 'z',
                    shift: ShiftReq::Present,
                    intent: UndoIntent::Redo,
                },
                Binding {
                    letter: 'y',
                    shift: ShiftReq::Any,
                    intent: UndoIntent::Redo,
                },
            ],
        }
    }

    /// Classifies a key event, returning the bound [`UndoIntent`] or `None`.
    pub fn classify(&self, key: KeyEvent) -> Option<UndoIntent> {
        let KeyCode::Char(c) = key.code else {
            return None;
        };
        // Ctrl is mandatory; Alt poisons the chord. We fold the uppercase-letter
        // form into an effective Shift so a terminal that reports Ctrl-Shift-Z as
        // `Char('Z')` without the flag still reads as redo.
        if !key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::ALT)
        {
            return None;
        }
        let letter = c.to_ascii_lowercase();
        let effective_shift = key.modifiers.contains(KeyModifiers::SHIFT) || c.is_ascii_uppercase();
        self.bindings
            .iter()
            .find(|b| b.letter == letter && b.shift.matches(effective_shift))
            .map(|b| b.intent)
    }
}

impl Default for UndoKeymap {
    fn default() -> Self {
        Self::standard()
    }
}
