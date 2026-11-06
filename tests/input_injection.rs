//! v0.7 node C — bounded headless event injection (`gibson::event_source::InjectedQueue`).
//!
//! A game loop must be testable without a terminal: injected input drives a headless
//! `Context`, and the rendered frames are a deterministic function of the injected sequence.
use gibson::cell::Style;
use gibson::context::{Context, RenderMode};
use gibson::event_source::{EventSource, InjectedQueue, InjectionReceipts, ScriptedSource};
use gibson::input::{Event, KeyCode, KeyEvent, KeyModifiers};
use gibson::node::Node;
use std::collections::VecDeque;
use std::time::Duration;

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::empty()))
}

// ---------------------------------------------------------------- queue contract

#[test]
fn delivery_is_first_in_first_out_one_event_per_poll() {
    let mut q = InjectedQueue::with_capacity(8);
    let events = [
        key(KeyCode::Up),
        Event::Resize(40, 15),
        key(KeyCode::Char('x')),
        Event::Tick,
    ];
    for e in &events {
        q.push(e.clone()).unwrap();
    }
    for e in &events {
        assert_eq!(
            q.poll_event(Duration::from_secs(5)).unwrap().as_ref(),
            Some(e)
        );
    }
    assert_eq!(q.poll_event(Duration::ZERO).unwrap(), None);
}

#[test]
fn a_full_queue_refuses_and_hands_the_event_back_without_displacing_anything() {
    let mut q = InjectedQueue::with_capacity(2);
    q.push(key(KeyCode::Left)).unwrap();
    q.push(key(KeyCode::Right)).unwrap();
    let refused = q.push(key(KeyCode::Up)).unwrap_err();
    assert_eq!(refused.event, key(KeyCode::Up));
    assert_eq!(refused.capacity, 2);
    assert_eq!(q.len(), 2);
    assert_eq!(
        q.poll_event(Duration::ZERO).unwrap(),
        Some(key(KeyCode::Left))
    );
    assert_eq!(
        q.poll_event(Duration::ZERO).unwrap(),
        Some(key(KeyCode::Right))
    );
    assert_eq!(
        q.receipts(),
        InjectionReceipts {
            accepted: 2,
            refused: 1,
            delivered: 2
        }
    );
}

#[test]
fn receipts_conserve_across_interleaved_pushes_and_polls() {
    let mut q = InjectedQueue::with_capacity(3);
    let mut delivered = Vec::new();
    for i in 0..50u16 {
        let _ = q.push(Event::Resize(i, i));
        if i % 3 == 0 {
            if let Some(e) = q.poll_event(Duration::ZERO).unwrap() {
                delivered.push(e);
            }
        }
        let r = q.receipts();
        assert_eq!(r.accepted, r.delivered + q.len() as u64, "after push {i}");
        assert!(q.len() <= q.capacity());
    }
    // Whatever was delivered came out in push order (strictly increasing payloads).
    let order: Vec<u16> = delivered
        .iter()
        .map(|e| match e {
            Event::Resize(w, _) => *w,
            _ => unreachable!(),
        })
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
}

#[test]
fn capacity_is_at_least_one_and_an_empty_poll_never_waits() {
    let mut q = InjectedQueue::with_capacity(0);
    assert_eq!(q.capacity(), 1);
    q.push(Event::Tick).unwrap();
    assert!(q.push(Event::Tick).is_err());
    let t0 = std::time::Instant::now();
    q.poll_event(Duration::ZERO).unwrap();
    let quiet = q.poll_event_timed(Duration::from_secs(10)).unwrap();
    assert_eq!(quiet.event, None);
    assert_eq!(
        quiet.elapsed,
        Duration::ZERO,
        "the queue reports that it did not wait"
    );
    assert!(
        t0.elapsed() < Duration::from_secs(1),
        "an empty poll must not block"
    );
}

#[test]
fn injected_events_never_appear_on_the_terminal_path() {
    // Synthetic input stays distinguishable: a headless context's own `poll_event` (the
    // terminal path) knows nothing of the queue; only `poll_from(queue)` delivers it.
    let mut ctx = Context::headless(RenderMode::Fullscreen, 20, 4);
    let mut q = InjectedQueue::with_capacity(4);
    q.push(key(KeyCode::Enter)).unwrap();
    assert_eq!(ctx.poll_event(Duration::ZERO).unwrap(), None);
    assert_eq!(q.len(), 1);
    assert_eq!(
        ctx.poll_from(&mut q, Duration::ZERO).unwrap(),
        Some(key(KeyCode::Enter))
    );
}

// ---------------------------------------------------------------- headless game loop

/// A tiny deterministic "game": a cursor on a 16x4 field moved by arrow keys; one rendered
/// frame per simulation step, digested from the exact bytes the renderer emitted.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
struct World {
    x: i32,
    y: i32,
    moves: u32,
}

impl World {
    fn apply(&mut self, e: &Event) {
        let (dx, dy) = match e {
            Event::Key(k) => match k.code {
                KeyCode::Left => (-1, 0),
                KeyCode::Right => (1, 0),
                KeyCode::Up => (0, -1),
                KeyCode::Down => (0, 1),
                _ => return,
            },
            _ => return,
        };
        self.x = (self.x + dx).clamp(0, 15);
        self.y = (self.y + dy).clamp(0, 3);
        self.moves += 1;
    }

    fn view(&self) -> Node {
        let mut rows = Node::col();
        for y in 0..4 {
            let row: String = (0..16)
                .map(|x| if (x, y) == (self.x, self.y) { '@' } else { '.' })
                .collect();
            rows = rows.child(Node::text(row, Style::default()));
        }
        rows.child(Node::text(
            format!("moves {}", self.moves),
            Style::default(),
        ))
    }
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Runs one simulation step per batch. Before each step the driver injects that step's batch
/// into `source`; the loop drains everything available, applies it, renders and digests the
/// exact bytes the renderer emitted.
fn run_with<S: EventSource>(
    batches: &[Vec<Event>],
    mut source: S,
    mut inject: impl FnMut(&mut S, &[Event]),
) -> Vec<u64> {
    let mut ctx = Context::headless(RenderMode::Fullscreen, 24, 6);
    let mut world = World::default();
    let mut digests = Vec::new();
    for batch in batches {
        inject(&mut source, batch);
        while let Some(e) = ctx.poll_from(&mut source, Duration::ZERO).unwrap() {
            world.apply(&e);
        }
        ctx.set_root(world.view());
        ctx.render().unwrap();
        digests.push(fnv(ctx.take_output().as_bytes()));
    }
    digests
}

fn run_queue(batches: &[Vec<Event>]) -> Vec<u64> {
    run_with(batches, InjectedQueue::with_capacity(16), |q, batch| {
        for e in batch {
            q.push(e.clone()).expect("within capacity");
        }
    })
}

fn script() -> Vec<Vec<Event>> {
    use KeyCode::*;
    vec![
        vec![key(Right), key(Right)],
        vec![],
        vec![key(Down), key(Char('z')), key(Right)],
        vec![key(Up)],
        vec![key(Right), key(Right), key(Right), key(Down), key(Down)],
        vec![],
        vec![key(Left)],
    ]
}

#[test]
fn the_same_injected_sequence_gives_identical_frame_digests() {
    let a = run_queue(&script());
    let b = run_queue(&script());
    assert_eq!(a, b);
    assert_eq!(a.len(), script().len());
    let distinct: std::collections::BTreeSet<_> = a.iter().collect();
    assert!(distinct.len() > 3, "the frames really changed: {a:?}");
}

#[test]
fn digests_do_not_depend_on_how_injection_is_batched() {
    // Capacity 1 forces one event per push, interleaved with the loop's polls.
    struct Trickle {
        q: InjectedQueue,
        staged: VecDeque<Event>,
    }
    impl EventSource for Trickle {
        fn poll_event(&mut self, t: Duration) -> std::io::Result<Option<Event>> {
            if self.q.is_empty() {
                if let Some(e) = self.staged.pop_front() {
                    self.q.push(e).expect("capacity 1 is free");
                }
            }
            self.q.poll_event(t)
        }
    }
    let trickle = Trickle {
        q: InjectedQueue::with_capacity(1),
        staged: VecDeque::new(),
    };
    let digests = run_with(&script(), trickle, |t, batch| {
        t.staged.extend(batch.iter().cloned())
    });
    assert_eq!(digests, run_queue(&script()));
}

#[test]
fn the_scripted_replay_route_agrees_with_the_injection_route() {
    // Two routes, one answer: the existing `ScriptedSource`, rebuilt from each step's events.
    let digests = run_with(
        &script(),
        ScriptedSource::from_events(Vec::new()),
        |s, batch| *s = ScriptedSource::from_events(batch.iter().cloned()),
    );
    assert_eq!(digests, run_queue(&script()));
}

#[test]
fn dropping_or_delaying_one_event_changes_the_digests() {
    let base = run_queue(&script());
    let mut dropped = script();
    dropped[3].clear(); // the lone Up is lost
    assert_ne!(
        run_queue(&dropped),
        base,
        "dropping one event must change a frame"
    );
    let mut delayed = script();
    let e = delayed[0].remove(0);
    delayed[1].push(e); // one Right arrives a step later
    assert_ne!(
        run_queue(&delayed),
        base,
        "delaying one event must change a frame"
    );
}

#[test]
fn reordering_within_a_step_changes_the_digest_when_order_matters() {
    use KeyCode::*;
    // At the right edge, Right-then-Left ends at x=14 but Left-then-Right ends at x=15.
    // 16 Rights (within the queue's capacity of 16): 15 reach the edge, the 16th clamps.
    let to_edge: Vec<Event> = (0..16).map(|_| key(Right)).collect();
    let a = vec![to_edge.clone(), vec![key(Right), key(Left)]];
    let b = vec![to_edge, vec![key(Left), key(Right)]];
    assert_ne!(run_queue(&a), run_queue(&b));
    // A commuting reorder (same end state, same count) is invisible in the frame by design:
    // one frame per step shows the step's result, not its internal order.
    let c = vec![vec![key(Right), key(Down)]];
    let d = vec![vec![key(Down), key(Right)]];
    assert_eq!(run_queue(&c), run_queue(&d));
}
