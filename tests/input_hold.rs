//! v0.7 node D — held-key inference on a simulation timebase (`gibson::hold`).
//!
//! Two routes, one answer: the event-driven `HoldTracker` is compared with an independent
//! per-tic reference simulator on seeded random traces, and the ended-hold records must not
//! depend on how often the caller advances the tracker. Every trace is seeded and replayable.
use gibson::clock::FixedTimestep;
use gibson::hold::{EndedHold, HoldConfig, HoldEnd, HoldError, HoldTracker, ReleaseCause};
use gibson::input::{Event, FocusChange, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

const CFG: HoldConfig = HoldConfig::new(18, 3);

fn tracker() -> HoldTracker<char> {
    HoldTracker::new(CFG)
}

fn key(c: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()))
}

fn char_of(k: &KeyEvent) -> Option<char> {
    match k.code {
        KeyCode::Char(c) => Some(c),
        _ => None,
    }
}

// ---------------------------------------------------------------- unit behaviour

#[test]
fn a_lone_press_is_held_for_exactly_the_initial_grace() {
    let mut h = tracker();
    h.observe(10, 'w').unwrap();
    for t in 10..28 {
        h.advance(t).unwrap();
        assert!(h.is_held('w'), "held at {t}");
    }
    let ended = h.advance(28).unwrap();
    assert!(!h.is_held('w'));
    assert_eq!(ended.len(), 1);
    let e = ended[0];
    assert_eq!(
        (e.key, e.hold.started, e.hold.last_seen, e.ended_at),
        ('w', 10, 10, 28)
    );
    assert_eq!((e.hold.observations, e.end), (1, HoldEnd::Timeout));
}

#[test]
fn autorepeat_extends_the_hold_by_the_repeat_timeout() {
    let mut h = tracker();
    h.observe(0, 'w').unwrap();
    // A terminal-like stream: autorepeat begins at tic 16 (inside the grace), then every tic.
    for t in 16..=40 {
        h.observe(t, 'w').unwrap();
        assert!(h.is_held('w'));
    }
    for t in 41..43 {
        assert!(h.advance(t).unwrap().is_empty(), "still held at {t}");
    }
    let ended = h.advance(43).unwrap();
    assert_eq!(ended[0].ended_at, 43, "40 + repeat_timeout 3");
    assert_eq!(ended[0].hold.observations, 26);
    assert_eq!(ended[0].hold.started, 0);
}

#[test]
fn an_autorepeat_later_than_the_grace_starts_a_new_hold() {
    // The stated failure mode: a terminal whose autorepeat delay exceeds the configured grace
    // drops the hold and restarts it. The model reports it; it does not paper over it.
    let mut h = tracker();
    h.observe(0, 'w').unwrap();
    let ended = h.observe(20, 'w').unwrap();
    assert_eq!(ended.len(), 1);
    assert_eq!((ended[0].ended_at, ended[0].end), (18, HoldEnd::Timeout));
    let hold = h.hold('w').unwrap();
    assert_eq!((hold.started, hold.observations, hold.ends_at), (20, 1, 38));
}

#[test]
fn an_observation_exactly_at_the_deadline_starts_a_new_hold() {
    let mut h = tracker();
    h.observe(0, 'w').unwrap();
    let ended = h.observe(18, 'w').unwrap();
    assert_eq!(ended[0].ended_at, 18);
    assert_eq!(h.hold('w').unwrap().started, 18);
}

#[test]
fn simultaneous_keys_are_independent() {
    let mut h = tracker();
    h.observe(0, 'w').unwrap();
    h.observe(2, 'd').unwrap();
    for t in 16..20 {
        h.observe(t, 'w').unwrap(); // only `w` autorepeats
    }
    assert!(h.is_held('w') && h.is_held('d'));
    // `d` (pressed at 2, never repeated) ends at 2 + 18 = 20; `w` (last seen 19) at 22.
    let ended = h.observe(20, 'w').unwrap();
    assert_eq!(ended.len(), 1);
    assert_eq!((ended[0].key, ended[0].ended_at), ('d', 20));
    assert!(h.is_held('w'));
    let held: Vec<char> = h.held().map(|(k, _)| k).collect();
    assert_eq!(held, vec!['w']);
}

#[test]
fn resize_paste_mouse_tick_and_focus_gain_never_touch_a_hold() {
    let mut h = tracker();
    h.observe(0, 'w').unwrap();
    let before = h.hold('w').unwrap();
    for (t, ev) in [
        (1, Event::Resize(40, 15)),
        (2, Event::Paste("w".into())),
        (3, Event::Tick),
        (4, Event::Focus(FocusChange::Gained)),
        (5, Event::Resize(200, 60)),
    ] {
        assert!(h.observe_event(t, &ev, char_of).unwrap().is_empty());
        assert_eq!(
            h.hold('w'),
            Some(before),
            "event at tic {t} changed the hold"
        );
    }
    assert_eq!(h.now(), 5);
}

#[test]
fn focus_loss_releases_every_hold_at_that_tic() {
    let mut h = tracker();
    h.observe_event(0, &key('w'), char_of).unwrap();
    h.observe_event(1, &key('a'), char_of).unwrap();
    let ended = h
        .observe_event(7, &Event::Focus(FocusChange::Lost), char_of)
        .unwrap();
    assert_eq!(ended.len(), 2);
    for (e, k) in ended.iter().zip(['w', 'a']) {
        assert_eq!(e.key, k, "release order is start order");
        assert_eq!(e.ended_at, 7);
        assert_eq!(e.end, HoldEnd::Released(ReleaseCause::FocusLost));
    }
    assert_eq!(h.held().count(), 0);
}

#[test]
fn focus_loss_reports_timeouts_that_happened_before_it_first() {
    let mut h = tracker();
    h.observe(0, 'w').unwrap(); // ends at 18
    h.observe(10, 'a').unwrap(); // ends at 28
    let ended = h
        .observe_event(20, &Event::Focus(FocusChange::Lost), char_of)
        .unwrap();
    assert_eq!(
        ended[0],
        EndedHold {
            end: HoldEnd::Timeout,
            ended_at: 18,
            ..ended[0]
        }
    );
    assert_eq!(ended[0].key, 'w');
    assert_eq!((ended[1].key, ended[1].ended_at), ('a', 20));
    assert_eq!(ended[1].end, HoldEnd::Released(ReleaseCause::FocusLost));
}

#[test]
fn an_input_disconnect_ends_holds_at_the_current_tic() {
    let mut h = tracker();
    h.observe(3, 'w').unwrap();
    h.advance(9).unwrap();
    let ended = h.release_all(ReleaseCause::Disconnected);
    assert_eq!(ended.len(), 1);
    assert_eq!(ended[0].ended_at, 9);
    assert_eq!(ended[0].end, HoldEnd::Released(ReleaseCause::Disconnected));
    assert!(!h.is_held('w'));
}

#[test]
fn an_explicit_release_ends_only_that_key() {
    let mut h = tracker();
    h.observe(0, 'w').unwrap();
    h.observe(0, 'd').unwrap();
    let e = h.release('w', ReleaseCause::Explicit).unwrap();
    assert_eq!(e.end, HoldEnd::Released(ReleaseCause::Explicit));
    assert!(h.release('w', ReleaseCause::Explicit).is_none());
    assert!(h.is_held('d'));
}

#[test]
fn unmapped_keys_only_advance_time() {
    let mut h = tracker();
    let esc = Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(h.observe_event(4, &esc, char_of).unwrap().is_empty());
    assert_eq!((h.now(), h.held().count()), (4, 0));
}

#[test]
fn a_regressing_tic_is_an_error_and_changes_nothing() {
    let mut h = tracker();
    h.observe(10, 'w').unwrap();
    let snapshot = (h.now(), h.hold('w'));
    assert_eq!(
        h.observe(9, 'd').unwrap_err(),
        HoldError::TicRegressed { now: 10, got: 9 }
    );
    assert_eq!(
        h.advance(3).unwrap_err(),
        HoldError::TicRegressed { now: 10, got: 3 }
    );
    assert_eq!((h.now(), h.hold('w')), snapshot);
    assert!(!h.is_held('d'));
}

#[test]
fn timeouts_round_up_and_never_reach_zero() {
    let tic = Duration::from_secs(1) / 35;
    let c = HoldConfig::from_durations(tic, Duration::from_millis(500), Duration::from_millis(80));
    assert_eq!(
        (c.initial_grace(), c.repeat_timeout()),
        (18, 3),
        "17.5 -> 18, 2.8 -> 3"
    );
    let z = HoldConfig::new(0, 0);
    assert_eq!((z.initial_grace(), z.repeat_timeout()), (1, 1));
    let zero_tic =
        HoldConfig::from_durations(Duration::ZERO, Duration::from_nanos(5), Duration::ZERO);
    assert_eq!(
        (zero_tic.initial_grace(), zero_tic.repeat_timeout()),
        (5, 1)
    );
}

#[test]
fn the_end_of_time_saturates_instead_of_wrapping() {
    let mut h = tracker();
    h.observe(u64::MAX - 1, 'w').unwrap();
    assert_eq!(h.hold('w').unwrap().ends_at, u64::MAX);
    let ended = h.advance(u64::MAX).unwrap();
    assert_eq!(ended[0].ended_at, u64::MAX);
}

#[test]
fn an_observation_at_the_last_tic_holds_nothing() {
    // [MAX, MAX) is empty: the hold is reported ended at once and never stored, so the
    // invariant `ends_at > now` survives and `is_held` cannot disagree with `advance`.
    let mut h = tracker();
    let ended = h.observe(u64::MAX, 'w').unwrap();
    assert_eq!(ended.len(), 1);
    assert_eq!(
        (ended[0].key, ended[0].ended_at, ended[0].end),
        ('w', u64::MAX, HoldEnd::Timeout)
    );
    assert!(!h.is_held('w'));
    assert_eq!(h.held().count(), 0);
    assert!(h.advance(u64::MAX).unwrap().is_empty());
    // A hold live just before the end saturates to `ends_at == MAX`; observing at MAX first
    // reports that hold's timeout, then the new, empty hold - two records, nothing left held.
    let mut h = tracker();
    h.observe(u64::MAX - 2, 'w').unwrap();
    let ended = h.observe(u64::MAX, 'w').unwrap();
    assert_eq!(ended.len(), 2, "{ended:?}");
    assert_eq!(
        (ended[0].hold.started, ended[0].ended_at),
        (u64::MAX - 2, u64::MAX)
    );
    assert_eq!(
        (ended[1].hold.started, ended[1].ended_at),
        (u64::MAX, u64::MAX)
    );
    assert!(ended.iter().all(|e| e.end == HoldEnd::Timeout));
    assert!(!h.is_held('w'));
}

// ---------------------------------------------------------------- reference oracle

#[derive(Clone, Copy, Debug)]
enum Op {
    Key(u64, char),
    FocusLost(u64),
}

/// Independent per-tic simulator: step every tic, expire, then apply that tic's operations in
/// trace order. Returns, for each tic, the set of held keys (in a canonical order).
fn reference(trace: &[Op], last_tic: u64, grace: u64, repeat: u64) -> Vec<Vec<char>> {
    // (key, held_until) — held while t < held_until.
    let mut state: Vec<(char, u64)> = Vec::new();
    let mut out = Vec::new();
    let mut i = 0;
    for t in 0..=last_tic {
        state.retain(|&(_, until)| until > t);
        while i < trace.len() {
            let tic = match trace[i] {
                Op::Key(tic, _) | Op::FocusLost(tic) => tic,
            };
            if tic != t {
                break;
            }
            match trace[i] {
                Op::Key(_, k) => match state.iter_mut().find(|(s, _)| *s == k) {
                    Some(entry) => entry.1 = t + repeat,
                    None => state.push((k, t + grace)),
                },
                Op::FocusLost(_) => state.clear(),
            }
            i += 1;
        }
        let mut held: Vec<char> = state.iter().map(|&(k, _)| k).collect();
        held.sort_unstable();
        out.push(held);
    }
    out
}

struct XorShift(u64);
impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A seeded trace: up to four keys, bursts of autorepeat with random gaps, rare focus loss.
fn random_trace(seed: u64) -> (Vec<Op>, u64) {
    let mut rng = XorShift(seed.max(1));
    let keys = ['w', 'a', 's', 'd'];
    let mut trace = Vec::new();
    let mut t = 0u64;
    for _ in 0..(20 + rng.below(40)) {
        t += rng.below(25);
        if rng.below(30) == 0 {
            trace.push(Op::FocusLost(t));
            continue;
        }
        let k = keys[rng.below(4) as usize];
        trace.push(Op::Key(t, k));
        // Optional autorepeat burst with jittered spacing (sometimes slower than the timeout).
        for _ in 0..rng.below(12) {
            t += 1 + rng.below(4);
            trace.push(Op::Key(t, k));
        }
    }
    (trace, t + 40)
}

fn feed(h: &mut HoldTracker<char>, op: Op) -> Vec<EndedHold<char>> {
    match op {
        Op::Key(t, k) => h.observe_event(t, &key(k), char_of).unwrap(),
        Op::FocusLost(t) => h
            .observe_event(t, &Event::Focus(FocusChange::Lost), char_of)
            .unwrap(),
    }
}

#[test]
fn the_tracker_matches_an_independent_per_tic_simulator() {
    for seed in 1..=300u64 {
        let (trace, last) = random_trace(seed);
        let expected = reference(&trace, last, CFG.initial_grace(), CFG.repeat_timeout());
        let mut h = tracker();
        let mut i = 0;
        for t in 0..=last {
            h.advance(t).unwrap();
            while i < trace.len() && matches!(trace[i], Op::Key(x, _) | Op::FocusLost(x) if x == t)
            {
                feed(&mut h, trace[i]);
                i += 1;
            }
            let mut held: Vec<char> = h.held().map(|(k, _)| k).collect();
            held.sort_unstable();
            assert_eq!(held, expected[t as usize], "seed {seed}, tic {t}");
        }
    }
}

#[test]
fn ended_records_do_not_depend_on_how_often_the_caller_advances() {
    for seed in 1..=200u64 {
        let (trace, last) = random_trace(seed);
        let run = |stride: u64| -> Vec<EndedHold<char>> {
            let mut h = tracker();
            let mut ended = Vec::new();
            let mut next_advance = 0;
            for op in &trace {
                let t = match *op {
                    Op::Key(t, _) | Op::FocusLost(t) => t,
                };
                while next_advance < t {
                    ended.extend(h.advance(next_advance).unwrap());
                    next_advance += stride;
                }
                ended.extend(feed(&mut h, *op));
            }
            ended.extend(h.advance(last).unwrap());
            ended
        };
        let every_tic = run(1);
        for stride in [2, 3, 5, 7, 1000] {
            assert_eq!(run(stride), every_tic, "seed {seed}, stride {stride}");
        }
    }
}

#[test]
fn a_timeout_lands_exactly_one_timeout_after_the_last_observation() {
    for seed in 1..=200u64 {
        let (trace, last) = random_trace(seed);
        let mut h = tracker();
        let mut ended = Vec::new();
        for op in &trace {
            ended.extend(feed(&mut h, *op));
        }
        ended.extend(h.advance(last).unwrap());
        for e in ended.iter().filter(|e| e.end == HoldEnd::Timeout) {
            let timeout = if e.hold.observations == 1 {
                CFG.initial_grace()
            } else {
                CFG.repeat_timeout()
            };
            assert_eq!(e.ended_at, e.hold.last_seen + timeout, "seed {seed}: {e:?}");
        }
        assert_eq!(
            h.held().count(),
            0,
            "seed {seed}: every hold ends by the last tic"
        );
    }
}

// ---------------------------------------------------------------- simulation vs presentation

/// A fixed-tic simulation (35 Hz) driven by real elapsed time from a render loop at `fps`.
/// Input is a recorded `(tic, key)` trace, so the simulation is a function of tics alone.
fn simulate(fps: u32, trace: &[Op], ticks: u64) -> (i64, Vec<EndedHold<char>>) {
    let mut clock = FixedTimestep::exact(Duration::from_secs(1) / 35, 8);
    let frame = Duration::from_secs(1) / fps;
    let mut h = tracker();
    let (mut x, mut tic, mut i) = (0i64, 0u64, 0usize);
    let mut ended = Vec::new();
    while tic < ticks {
        for _ in 0..clock.advance(frame).executed {
            if tic >= ticks {
                break;
            }
            ended.extend(h.advance(tic).unwrap());
            while i < trace.len()
                && matches!(trace[i], Op::Key(t, _) | Op::FocusLost(t) if t == tic)
            {
                ended.extend(feed(&mut h, trace[i]));
                i += 1;
            }
            if h.is_held('d') {
                x += 1;
            }
            if h.is_held('a') {
                x -= 1;
            }
            tic += 1;
        }
        // (presentation of `x` would happen here, at the render cadence)
    }
    (x, ended)
}

#[test]
fn presentation_cadence_never_changes_the_simulation() {
    for seed in 1..=50u64 {
        let (trace, last) = random_trace(seed);
        let reference = simulate(35, &trace, last);
        for fps in [15, 60, 144] {
            assert_eq!(
                simulate(fps, &trace, last),
                reference,
                "seed {seed}, {fps} fps"
            );
        }
    }
}
