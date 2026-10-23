//! Pointer-drag gesture grouping: one begin / preview* / commit-or-cancel.
//!
//! A drag is a story told in scattered events — a press, a stream of moves, a
//! release — any of which may be interrupted by Escape, a resize, lost focus, a
//! modal opening, or the target vanishing. [`DragGesture`] is the pure state
//! machine that turns that stream into a single, well-formed edit episode over
//! an [`EditSession`], emitting [`EditPhase`] values carrying a [`DragIntent`].
//!
//! It pins the pressed target with a [`PointerCapture`](crate::hit_test::PointerCapture) so the drag follows the
//! pointer even off the target's rect, and it **revalidates that capture on
//! every move**: a modal taking the surface, or the target being removed, ends
//! the episode with the matching [`CancelReason`] rather than routing input to
//! a ghost. World coordinates are carried as [`Transport`](crate::world_view::Transport) receipts — a drag
//! off the viewport previews an honest [`Transport::Clipped`](crate::world_view::Transport::Clipped) rather than being
//! silently dropped.
//!
//! Pure: no clock, no I/O. The same event sequence yields the same phases.

use crate::edit::{CancelReason, EditId, EditPhase, EditSession};
use crate::hit_test::{HitLayers, PointerCapture, TargetId};
use crate::input::{Event, FocusChange, KeyCode, KeyEvent, MouseButton, MouseEvent, MouseKind};
use crate::world_view::{Transport, WorldView};

/// A proposed drag, in both cell and continuous-world terms.
///
/// The library fills this in; the consumer reads it and maps it to its own
/// validated command (e.g. a delta move). Both the start and current positions
/// are reported as a terminal cell *and* as a [`Transport`] world receipt, so
/// the consumer can work in whichever space it owns — and a clipped world point
/// is carried honestly, never coerced to a bogus origin.
#[derive(Debug, Clone, PartialEq)]
pub struct DragIntent {
    /// The captured target the drag is acting on.
    pub target: TargetId,
    /// The cell the press landed on.
    pub start_cell: (u16, u16),
    /// The cell the pointer is on now.
    pub current_cell: (u16, u16),
    /// The world point under the press (or a clip receipt).
    pub start_world: Transport<(f64, f64)>,
    /// The world point under the pointer now (or a clip receipt).
    pub current_world: Transport<(f64, f64)>,
}

/// A pure drag-gesture grouper.
///
/// Feed it [`Event`]s with [`on_event`](DragGesture::on_event); it returns at
/// most one [`EditPhase`] per event. The lifecycle is exactly one `Begin`, any
/// number of `Preview`s, then one `Commit` or one `Cancel` — never two
/// endings, never a commit after a cancel.
#[derive(Debug, Clone)]
pub struct DragGesture {
    // The episode grouper. Its openness IS this gesture's "active" state; we
    // keep no separate flag that could drift out of sync with it.
    session: EditSession<DragIntent>,
    // The pin on the pressed target. Routed and revalidated every move.
    capture: PointerCapture,
    // Monotonic id source: each begin mints the next, so no two episodes in a
    // run collide and the sequence is deterministic.
    next_id: u64,
    // The press anchor, remembered until the episode ends. `Some` iff active.
    start_cell: Option<(u16, u16)>,
    start_world: Option<Transport<(f64, f64)>>,
}

impl Default for DragGesture {
    fn default() -> Self {
        Self::new()
    }
}

impl DragGesture {
    /// A fresh, inactive gesture whose first episode will be [`EditId`]`(0)`.
    pub fn new() -> Self {
        Self {
            session: EditSession::new(),
            capture: PointerCapture::new(),
            next_id: 0,
            start_cell: None,
            start_world: None,
        }
    }

    /// `true` while a drag episode is open.
    pub fn is_active(&self) -> bool {
        self.session.is_open()
    }

    /// The target currently captured, if any.
    pub fn target(&self) -> Option<TargetId> {
        self.capture.captured()
    }

    /// Advances the machine by one event, returning any phase it produced.
    ///
    /// Only the left button is primary. Unrelated events while inactive, and
    /// non-left buttons, produce `None` without disturbing anything. The
    /// interrupt events (Escape / focus loss / resize) only bite while active.
    pub fn on_event(
        &mut self,
        event: &Event,
        layers: &HitLayers,
        view: &WorldView,
    ) -> Option<EditPhase<DragIntent>> {
        match event {
            Event::Mouse(MouseEvent {
                kind, column, row, ..
            }) => {
                let (c, r) = (*column, *row);
                match kind {
                    // A press while active is a supersede, not a nested begin:
                    // end the old episode (Superseded) and do NOT open a new one
                    // on this same event.
                    MouseKind::Down(MouseButton::Left) if self.is_active() => {
                        self.cancel_active(CancelReason::Superseded)
                    }
                    // A press on empty space does nothing — no target, no
                    // capture, no begin; the `?` bails before any state is
                    // touched. A press on an INERT (painted-but-disabled) target
                    // is swallowed the same way: it occludes, so the press never
                    // falls through to an enabled target beneath, but it carries
                    // no authority, so no edit episode opens (Finding E / Law L2).
                    MouseKind::Down(MouseButton::Left) => {
                        let t = layers.hit_test(c, r)?;
                        if layers.is_inert(t) {
                            return None;
                        }
                        self.capture.capture(t);
                        self.start_cell = Some((c, r));
                        self.start_world = Some(view.cell_to_world(c, r));
                        let id = EditId(self.next_id);
                        self.next_id += 1;
                        self.session.begin(id, None).ok()
                    }
                    MouseKind::Drag(MouseButton::Left) if self.is_active() => {
                        match self.revalidate(layers, c, r) {
                            Err(reason) => self.cancel_active(reason),
                            Ok(()) => {
                                let intent = self.build_intent(c, r, view);
                                self.session.preview(intent)
                            }
                        }
                    }
                    MouseKind::Up(MouseButton::Left) if self.is_active() => {
                        match self.revalidate(layers, c, r) {
                            Err(reason) => self.cancel_active(reason),
                            Ok(()) => {
                                let intent = self.build_intent(c, r, view);
                                let phase = self.session.commit(intent);
                                self.reset();
                                phase
                            }
                        }
                    }
                    _ => None,
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Esc, ..
            }) if self.is_active() => self.cancel_active(CancelReason::Escape),
            Event::Focus(FocusChange::Lost) if self.is_active() => {
                self.cancel_active(CancelReason::FocusLost)
            }
            Event::Resize(..) if self.is_active() => self.cancel_active(CancelReason::Resize),
            _ => None,
        }
    }

    /// Checks whether the captured target still legitimately holds the pointer.
    ///
    /// A target that has gone inert (disabled) under the drag has lost its
    /// authority first of all — `TargetDisabled` — even though it is still
    /// present and unoccluded. Otherwise, if
    /// [`PointerCapture::route`](crate::hit_test::PointerCapture::route) still routes to the captured target, the
    /// capture is live (`Ok`); if not we diagnose *why*: a target no longer
    /// present in the layers is `TargetRemoved`; a target still present but
    /// out-routed (a modal opened above it) is `ModalTakeover`.
    fn revalidate(&self, layers: &HitLayers, c: u16, r: u16) -> Result<(), CancelReason> {
        let t = self
            .capture
            .captured()
            .expect("revalidate is only called while a capture is held");
        if layers.is_inert(t) {
            return Err(CancelReason::TargetDisabled);
        }
        if self.capture.route(layers, c, r) == Some(t) {
            Ok(())
        } else if !layers.contains(t) {
            Err(CancelReason::TargetRemoved)
        } else {
            Err(CancelReason::ModalTakeover)
        }
    }

    /// Builds the current [`DragIntent`] from the stored anchor and the live
    /// position. Taken by `&self` and returned by value so the caller can hand
    /// it to the `&mut self` session method without aliasing the two borrows.
    fn build_intent(&self, c: u16, r: u16, view: &WorldView) -> DragIntent {
        DragIntent {
            target: self
                .capture
                .captured()
                .expect("an active drag holds a captured target"),
            start_cell: self
                .start_cell
                .expect("an active drag recorded its start cell"),
            current_cell: (c, r),
            start_world: self
                .start_world
                .expect("an active drag recorded its start world point"),
            current_world: view.cell_to_world(c, r),
        }
    }

    /// Cancels the open episode with `reason` and clears the anchor.
    fn cancel_active(&mut self, reason: CancelReason) -> Option<EditPhase<DragIntent>> {
        let phase = self.session.cancel(reason);
        self.reset();
        phase
    }

    /// Drops the capture and the press anchor, returning the gesture to rest.
    fn reset(&mut self) {
        self.capture.release();
        self.start_cell = None;
        self.start_world = None;
    }
}
