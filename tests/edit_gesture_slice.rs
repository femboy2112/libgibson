//! R4-A falsifiers (issues #132/#134/#137): the generic edit protocol
//! (`EditSession`/`EditPhase`/`UndoLog`/`Reversible`) and the pure
//! `DragGesture` grouper.
//!
//! The library owns grouping, ordering and delivery; the consumer owns the
//! model and the meaning. These tests assert on the phases the library emits
//! and on a consumer-owned model — never on library internals — and they are
//! hostile by design: exactly-one-commit, exactly-one-cancel, no mutation on
//! cancel, exact undo/redo, bounded eviction, modal/removal/resize/focus
//! cancellation, and honest (never dropped) Clipped transports.

use gibson::edit::{
    CancelReason, EditError, EditId, EditPhase, EditSession, EvictionStatus, Reversible, UndoLog,
};
use gibson::gesture::{DragGesture, DragIntent};
use gibson::hit_test::{HitLayers, TargetId};
use gibson::input::{
    Event, FocusChange, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseKind,
};
use gibson::surface::Rect;
use gibson::world_view::{Transport, WorldView};

// ---------------------------------------------------------------------------
// A tiny consumer-owned model and a reversible command over it.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Model {
    /// Applied moves, in order: (target, dx, dy).
    moves: Vec<(u64, i64, i64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MoveCmd {
    target: u64,
    dx: i64,
    dy: i64,
}

impl Reversible for MoveCmd {
    type Model = Model;
    type Error = ();

    fn apply(&self, m: &mut Model) -> Result<(), ()> {
        m.moves.push((self.target, self.dx, self.dy));
        Ok(())
    }

    fn revert(&self, m: &mut Model) -> Result<(), ()> {
        // A faithful inverse only if the newest applied move is exactly ours.
        // If state diverged, refuse — the caller must see the failure.
        match m.moves.last() {
            Some(&top) if top == (self.target, self.dx, self.dy) => {
                m.moves.pop();
                Ok(())
            }
            _ => Err(()),
        }
    }
}

// ---------------------------------------------------------------------------
// Event + layer + view helpers.
// ---------------------------------------------------------------------------

fn mouse(kind: MouseKind, column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::empty(),
    })
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::empty()))
}

/// One non-modal layer with a single target covering the whole surface.
fn base_layers() -> HitLayers {
    let mut l = HitLayers::new();
    l.push_layer(false);
    l.add_target(
        TargetId(1),
        Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 20,
        },
        0,
    );
    l
}

/// `base_layers` plus a modal layer on top that covers the same region.
fn modal_over_layers() -> HitLayers {
    let mut l = base_layers();
    l.push_layer(true);
    l.add_target(
        TargetId(2),
        Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 20,
        },
        1,
    );
    l
}

/// A layer that does NOT contain target 1 (simulates its removal).
fn without_target1() -> HitLayers {
    let mut l = HitLayers::new();
    l.push_layer(false);
    l.add_target(
        TargetId(9),
        Rect {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        },
        0,
    );
    l
}

fn view() -> WorldView {
    WorldView::new(Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 20,
    })
    .with_pixels_per_cell(1, 1)
}

fn run(
    g: &mut DragGesture,
    layers: &HitLayers,
    view: &WorldView,
    evs: &[Event],
) -> Vec<EditPhase<DragIntent>> {
    evs.iter()
        .filter_map(|e| g.on_event(e, layers, view))
        .collect()
}

fn count<F: Fn(&EditPhase<DragIntent>) -> bool>(ps: &[EditPhase<DragIntent>], f: F) -> usize {
    ps.iter().filter(|p| f(p)).count()
}

// ===========================================================================
// EditSession
// ===========================================================================

#[test]
fn editsession_one_begin_previews_one_commit() {
    let mut s: EditSession<i32> = EditSession::new();
    assert!(!s.is_open());

    let begin = s.begin(EditId(1), Some("drag".to_string())).unwrap();
    assert_eq!(
        begin,
        EditPhase::Begin {
            id: EditId(1),
            label: Some("drag".to_string())
        }
    );
    assert!(s.is_open());
    assert_eq!(s.current(), Some(EditId(1)));

    assert_eq!(
        s.preview(10),
        Some(EditPhase::Preview {
            id: EditId(1),
            intent: 10
        })
    );
    assert_eq!(
        s.preview(20),
        Some(EditPhase::Preview {
            id: EditId(1),
            intent: 20
        })
    );

    assert_eq!(
        s.commit(99),
        Some(EditPhase::Commit {
            id: EditId(1),
            intent: 99
        })
    );
    assert!(!s.is_open());

    // Exactly one commit: a second commit (or a late preview) emits nothing.
    assert_eq!(s.commit(0), None);
    assert_eq!(s.preview(5), None);
}

#[test]
fn editsession_begin_while_open_errors_without_disturbing_the_open_edit() {
    let mut s: EditSession<i32> = EditSession::new();
    s.begin(EditId(1), None).unwrap();
    assert_eq!(s.begin(EditId(2), None), Err(EditError::AlreadyOpen));
    // The first edit is untouched and still commits as itself.
    assert_eq!(s.current(), Some(EditId(1)));
    assert_eq!(
        s.commit(7),
        Some(EditPhase::Commit {
            id: EditId(1),
            intent: 7
        })
    );
}

#[test]
fn editsession_cancel_closes_and_blocks_a_late_commit() {
    let mut s: EditSession<i32> = EditSession::new();
    s.begin(EditId(7), None).unwrap();
    s.preview(1);
    assert_eq!(
        s.cancel(CancelReason::Escape),
        Some(EditPhase::Cancel {
            id: EditId(7),
            reason: CancelReason::Escape
        })
    );
    assert!(!s.is_open());
    // Cancel means no authoritative change: a late commit commits nothing.
    assert_eq!(s.commit(123), None);
    assert_eq!(s.cancel(CancelReason::Resize), None);
}

// ===========================================================================
// UndoLog
// ===========================================================================

#[test]
fn undolog_undo_redo_restores_the_model_exactly() {
    let mut m = Model::default();
    let mut log: UndoLog<MoveCmd> = UndoLog::new();

    let c1 = MoveCmd {
        target: 1,
        dx: 3,
        dy: 0,
    };
    c1.apply(&mut m).unwrap();
    log.record(c1);
    let c2 = MoveCmd {
        target: 1,
        dx: 0,
        dy: 5,
    };
    c2.apply(&mut m).unwrap();
    log.record(c2);

    assert_eq!(m.moves, vec![(1, 3, 0), (1, 0, 5)]);
    assert!(log.can_undo() && !log.can_redo());

    assert_eq!(log.undo(&mut m), Some(Ok(())));
    assert_eq!(m.moves, vec![(1, 3, 0)]);
    assert!(log.can_redo());

    assert_eq!(log.undo(&mut m), Some(Ok(())));
    assert_eq!(m.moves, Vec::<(u64, i64, i64)>::new());
    assert_eq!(log.undo(&mut m), None); // nothing left to undo

    assert_eq!(log.redo(&mut m), Some(Ok(())));
    assert_eq!(m.moves, vec![(1, 3, 0)]);
    assert_eq!(log.redo(&mut m), Some(Ok(())));
    assert_eq!(m.moves, vec![(1, 3, 0), (1, 0, 5)]);
    assert_eq!(log.redo(&mut m), None); // nothing left to redo
}

#[test]
fn undolog_recording_a_new_command_discards_the_redo_branch() {
    let mut m = Model::default();
    let mut log: UndoLog<MoveCmd> = UndoLog::new();

    let c1 = MoveCmd {
        target: 1,
        dx: 1,
        dy: 0,
    };
    c1.apply(&mut m).unwrap();
    log.record(c1);
    log.undo(&mut m).unwrap().unwrap();
    assert!(log.can_redo());

    // A fresh edit after an undo forks history: the redo branch is dropped.
    let c2 = MoveCmd {
        target: 2,
        dx: 0,
        dy: 9,
    };
    c2.apply(&mut m).unwrap();
    log.record(c2);
    assert!(!log.can_redo());
    assert_eq!(log.redo(&mut m), None);
}

#[test]
fn undolog_bounded_capacity_evicts_oldest_and_reports_it() {
    let mut log: UndoLog<MoveCmd> = UndoLog::with_capacity(2);
    let mk = |n: i64| MoveCmd {
        target: 1,
        dx: n,
        dy: 0,
    };
    assert_eq!(log.record(mk(1)), EvictionStatus::Kept);
    assert_eq!(log.record(mk(2)), EvictionStatus::Kept);
    assert_eq!(log.record(mk(3)), EvictionStatus::Evicted(1));
    assert_eq!(log.undo_depth(), 2);
}

#[test]
fn undolog_failed_revert_surfaces_the_error_and_keeps_the_entry() {
    let mut m = Model::default();
    let mut log: UndoLog<MoveCmd> = UndoLog::new();
    let c1 = MoveCmd {
        target: 1,
        dx: 1,
        dy: 1,
    };
    c1.apply(&mut m).unwrap();
    log.record(c1);

    // The model diverges out from under the log (someone else mutated it).
    m.moves.push((9, 9, 9));

    // revert now mismatches: the error is surfaced, the entry is retained.
    assert_eq!(log.undo(&mut m), Some(Err(())));
    assert!(log.can_undo());
}

// ===========================================================================
// DragGesture
// ===========================================================================

#[test]
fn gesture_down_drag_up_is_one_begin_previews_and_one_commit() {
    let (layers, view) = (base_layers(), view());
    let mut g = DragGesture::new();
    let phases = run(
        &mut g,
        &layers,
        &view,
        &[
            mouse(MouseKind::Down(MouseButton::Left), 5, 5),
            mouse(MouseKind::Drag(MouseButton::Left), 6, 5),
            mouse(MouseKind::Drag(MouseButton::Left), 8, 6),
            mouse(MouseKind::Up(MouseButton::Left), 8, 6),
        ],
    );

    assert_eq!(count(&phases, |p| matches!(p, EditPhase::Begin { .. })), 1);
    assert_eq!(
        count(&phases, |p| matches!(p, EditPhase::Preview { .. })),
        2
    );
    assert_eq!(count(&phases, |p| matches!(p, EditPhase::Commit { .. })), 1);
    assert_eq!(count(&phases, |p| matches!(p, EditPhase::Cancel { .. })), 0);
    assert!(!g.is_active());

    match phases.first().unwrap() {
        EditPhase::Begin { id, label } => {
            assert_eq!(*id, EditId(0));
            assert!(label.is_none());
        }
        other => panic!("expected Begin first, got {other:?}"),
    }
    match phases.last().unwrap() {
        EditPhase::Commit { id, intent } => {
            assert_eq!(*id, EditId(0));
            assert_eq!(intent.target, TargetId(1));
            assert_eq!(intent.start_cell, (5, 5));
            assert_eq!(intent.current_cell, (8, 6));
            assert!(matches!(intent.start_world, Transport::Mapped { .. }));
            assert!(matches!(intent.current_world, Transport::Mapped { .. }));
        }
        other => panic!("expected Commit last, got {other:?}"),
    }
}

#[test]
fn gesture_a_click_commits_once_with_equal_start_and_current_cells() {
    // A zero-distance drag (click) is still exactly one commit; whether that
    // means "select" rather than "move" is the consumer's reading, not ours.
    let (layers, view) = (base_layers(), view());
    let mut g = DragGesture::new();
    let phases = run(
        &mut g,
        &layers,
        &view,
        &[
            mouse(MouseKind::Down(MouseButton::Left), 7, 7),
            mouse(MouseKind::Up(MouseButton::Left), 7, 7),
        ],
    );
    assert_eq!(count(&phases, |p| matches!(p, EditPhase::Commit { .. })), 1);
    match phases.last().unwrap() {
        EditPhase::Commit { intent, .. } => {
            assert_eq!(intent.start_cell, (7, 7));
            assert_eq!(intent.current_cell, (7, 7));
        }
        other => panic!("expected Commit, got {other:?}"),
    }
}

#[test]
fn gesture_escape_cancels_and_a_late_up_commits_nothing() {
    let (layers, view) = (base_layers(), view());
    let mut g = DragGesture::new();
    let phases = run(
        &mut g,
        &layers,
        &view,
        &[
            mouse(MouseKind::Down(MouseButton::Left), 5, 5),
            mouse(MouseKind::Drag(MouseButton::Left), 6, 6),
            key(KeyCode::Esc),
            mouse(MouseKind::Up(MouseButton::Left), 6, 6), // late: must not commit
        ],
    );
    assert_eq!(count(&phases, |p| matches!(p, EditPhase::Commit { .. })), 0);
    assert_eq!(count(&phases, |p| matches!(p, EditPhase::Cancel { .. })), 1);
    assert!(!g.is_active());
    assert!(phases.iter().any(|p| matches!(
        p,
        EditPhase::Cancel {
            reason: CancelReason::Escape,
            ..
        }
    )));
}

#[test]
fn gesture_focus_loss_and_resize_each_cancel_without_committing() {
    let (layers, view) = (base_layers(), view());

    for (interrupt, reason) in [
        (Event::Focus(FocusChange::Lost), CancelReason::FocusLost),
        (Event::Resize(30, 10), CancelReason::Resize),
    ] {
        let mut g = DragGesture::new();
        let phases = run(
            &mut g,
            &layers,
            &view,
            &[
                mouse(MouseKind::Down(MouseButton::Left), 5, 5),
                interrupt.clone(),
                mouse(MouseKind::Up(MouseButton::Left), 5, 5),
            ],
        );
        assert_eq!(count(&phases, |p| matches!(p, EditPhase::Commit { .. })), 0);
        assert_eq!(count(&phases, |p| matches!(p, EditPhase::Cancel { .. })), 1);
        assert!(phases
            .iter()
            .any(|p| matches!(p, EditPhase::Cancel { reason: r, .. } if *r == reason)));
    }
}

#[test]
fn gesture_down_on_empty_space_is_inert() {
    let (layers, view) = (without_target1(), view());
    let mut g = DragGesture::new();
    let phases = run(
        &mut g,
        &layers,
        &view,
        &[
            mouse(MouseKind::Down(MouseButton::Left), 10, 10), // hits no target
            mouse(MouseKind::Up(MouseButton::Left), 10, 10),
        ],
    );
    assert!(phases.is_empty());
    assert!(!g.is_active());
}

#[test]
fn gesture_modal_takeover_mid_drag_cancels() {
    let view = view();
    let mut g = DragGesture::new();

    // Begin under the base layer.
    let base = base_layers();
    let begin = g.on_event(
        &mouse(MouseKind::Down(MouseButton::Left), 5, 5),
        &base,
        &view,
    );
    assert!(matches!(begin, Some(EditPhase::Begin { .. })));

    // A modal appears over the captured target before the next drag lands.
    let modal = modal_over_layers();
    let next = g.on_event(
        &mouse(MouseKind::Drag(MouseButton::Left), 6, 6),
        &modal,
        &view,
    );
    assert!(matches!(
        next,
        Some(EditPhase::Cancel {
            reason: CancelReason::ModalTakeover,
            ..
        })
    ));
    assert!(!g.is_active());
}

#[test]
fn gesture_target_removed_mid_drag_cancels() {
    let view = view();
    let mut g = DragGesture::new();

    let base = base_layers();
    g.on_event(
        &mouse(MouseKind::Down(MouseButton::Left), 5, 5),
        &base,
        &view,
    );

    let gone = without_target1();
    let next = g.on_event(&mouse(MouseKind::Up(MouseButton::Left), 5, 5), &gone, &view);
    assert!(matches!(
        next,
        Some(EditPhase::Cancel {
            reason: CancelReason::TargetRemoved,
            ..
        })
    ));
    assert!(!g.is_active());
}

#[test]
fn gesture_drag_outside_the_view_previews_a_clipped_world_without_dropping() {
    // A small view anchor: cell (30,30) falls outside it and maps to Clipped.
    // The preview must still fire (capture holds the target) and carry the
    // Clipped transport honestly rather than silently dropping the event.
    let small = WorldView::new(Rect {
        x: 0,
        y: 0,
        width: 8,
        height: 8,
    })
    .with_pixels_per_cell(1, 1);
    let layers = base_layers();
    let mut g = DragGesture::new();

    g.on_event(
        &mouse(MouseKind::Down(MouseButton::Left), 2, 2),
        &layers,
        &small,
    );
    let preview = g.on_event(
        &mouse(MouseKind::Drag(MouseButton::Left), 30, 30),
        &layers,
        &small,
    );
    match preview {
        Some(EditPhase::Preview { intent, .. }) => {
            assert_eq!(intent.current_cell, (30, 30));
            assert!(matches!(intent.current_world, Transport::Clipped(_)));
        }
        other => panic!("expected a Clipped Preview, got {other:?}"),
    }
}

#[test]
fn gesture_is_deterministic_same_events_same_phases() {
    let (layers, view) = (base_layers(), view());
    let evs = [
        mouse(MouseKind::Down(MouseButton::Left), 4, 4),
        mouse(MouseKind::Drag(MouseButton::Left), 5, 5),
        mouse(MouseKind::Drag(MouseButton::Left), 9, 7),
        mouse(MouseKind::Up(MouseButton::Left), 9, 7),
    ];
    let a = run(&mut DragGesture::new(), &layers, &view, &evs);
    let b = run(&mut DragGesture::new(), &layers, &view, &evs);
    assert_eq!(a, b);
}

#[test]
fn end_to_end_gesture_commit_feeds_an_undoable_command() {
    let (layers, view) = (base_layers(), view());
    let mut g = DragGesture::new();
    let phases = run(
        &mut g,
        &layers,
        &view,
        &[
            mouse(MouseKind::Down(MouseButton::Left), 5, 5),
            mouse(MouseKind::Drag(MouseButton::Left), 8, 9),
            mouse(MouseKind::Up(MouseButton::Left), 8, 9),
        ],
    );

    // The consumer maps the one commit intent into its own validated command.
    let commit = phases
        .iter()
        .find_map(|p| match p {
            EditPhase::Commit { intent, .. } => Some(intent.clone()),
            _ => None,
        })
        .expect("exactly one commit");

    let mut model = Model::default();
    let mut log: UndoLog<MoveCmd> = UndoLog::new();
    let cmd = MoveCmd {
        target: commit.target.0,
        dx: commit.current_cell.0 as i64 - commit.start_cell.0 as i64,
        dy: commit.current_cell.1 as i64 - commit.start_cell.1 as i64,
    };
    cmd.apply(&mut model).unwrap();
    log.record(cmd);
    assert_eq!(model.moves, vec![(1, 3, 4)]);

    // Undo the whole grouped gesture in one step, back to the pre-drag model.
    assert_eq!(log.undo(&mut model), Some(Ok(())));
    assert_eq!(model, Model::default());
}
