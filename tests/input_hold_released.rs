//! #275 — holds when the terminal reports key releases (`gibson::hold`, kitty keyboard protocol).
//!
//! Once releases are reported a hold has no deadline: it lasts from the press to the reported
//! release, at least one tic, and keys held together stay held together (a terminal's autorepeat
//! follows only the last key pressed, so the inferred model drops the first key of a chord). The
//! tracker is compared tic by tic with an independent reference model on seeded random traces,
//! and its records must not depend on how often the caller advances it.
use gibson::hold::{EndedHold, HoldConfig, HoldEnd, HoldTracker, ReleaseCause};
use gibson::input::{Event, FocusChange, KeyCode, KeyEvent, KeyModifiers, KeyReleaseReporting};

const CFG: HoldConfig = HoldConfig::new(18, 3);

fn reported() -> HoldTracker<char> {
    let mut h = HoldTracker::new(CFG);
    h.set_releases_reported(true);
    h
}

fn press(c: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()))
}

fn up(c: char) -> Event {
    Event::KeyRelease(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()))
}

fn char_of(k: &KeyEvent) -> Option<char> {
    match k.code {
        KeyCode::Char(c) => Some(c),
        _ => None,
    }
}

const KEY_RELEASED: HoldEnd = HoldEnd::Released(ReleaseCause::KeyReleased);

#[test]
fn a_reported_hold_has_no_deadline_and_ends_at_its_release() {
    let mut h = reported();
    h.observe(10, 'w').unwrap();
    // Far past any inferred deadline (grace 18, repeat 3), with no autorepeat at all.
    for t in [11, 28, 29, 500, 10_000] {
        assert!(h.advance(t).unwrap().is_empty(), "nothing ends by {t}");
        assert!(h.is_held('w'), "held at {t}");
    }
    let ended = h.observe_release(10_007, 'w').unwrap();
    assert!(!h.is_held('w'));
    assert_eq!(ended.len(), 1);
    assert_eq!(ended[0].ended_at, 10_007);
    assert_eq!(ended[0].end, KEY_RELEASED);
    assert_eq!(ended[0].hold.started, 10);
}

#[test]
fn keys_held_together_stay_held_together() {
    // The chord the inferred model cannot hold: w down, then d tapped while w stays down. A
    // terminal's autorepeat moves to d, so w is never observed again until its release.
    let mut h = reported();
    h.observe(10, 'w').unwrap();
    h.observe(20, 'd').unwrap();
    for t in 21..=40 {
        h.observe(t, 'd').unwrap(); // d autorepeats
        assert!(h.is_held('w') && h.is_held('d'), "both held at {t}");
    }
    let ended = h.observe_release(41, 'd').unwrap();
    assert_eq!(ended.len(), 1);
    assert_eq!(ended[0].key, 'd');
    for t in 42..100 {
        h.advance(t).unwrap();
        assert!(h.is_held('w') && !h.is_held('d'), "w alone at {t}");
    }
    let ended = h.observe_release(100, 'w').unwrap();
    assert_eq!((ended[0].key, ended[0].ended_at), ('w', 100));
    assert_eq!(h.held().count(), 0);
}

#[test]
fn a_tap_inside_one_tic_is_held_for_exactly_that_tic() {
    let mut h = reported();
    h.observe(5, 'w').unwrap();
    assert!(h.observe_release(5, 'w').unwrap().is_empty());
    assert!(h.is_held('w'), "the press is held at its own tic");
    let ended = h.advance(6).unwrap();
    assert!(!h.is_held('w'));
    assert_eq!(ended.len(), 1);
    assert_eq!(ended[0].ended_at, 6);
    assert_eq!(ended[0].end, KEY_RELEASED, "a release, not a timeout");
}

#[test]
fn pressed_again_in_the_tic_it_was_released_stays_held() {
    let mut h = reported();
    h.observe(5, 'w').unwrap();
    h.observe_release(5, 'w').unwrap();
    h.observe(5, 'w').unwrap();
    for t in 6..60 {
        assert!(h.advance(t).unwrap().is_empty());
        assert!(h.is_held('w'), "held at {t}");
    }
}

#[test]
fn a_release_of_a_key_not_held_changes_nothing() {
    let mut h = reported();
    h.observe(3, 'a').unwrap();
    assert!(h.observe_release(4, 'b').unwrap().is_empty());
    assert!(h.is_held('a') && !h.is_held('b'));
    assert_eq!(h.now(), 4, "but time advances");
}

#[test]
fn the_answer_event_switches_the_model_for_keys_observed_from_then_on() {
    let mut h = HoldTracker::new(CFG);
    assert!(!h.releases_reported());
    h.observe_event(10, &press('w'), char_of).unwrap(); // inferred: ends at 28
    h.observe_event(
        12,
        &Event::KeyReleaseReporting(KeyReleaseReporting::Active),
        char_of,
    )
    .unwrap();
    assert!(h.releases_reported());
    assert_eq!(
        h.hold('w').unwrap().ends_at,
        28,
        "an earlier hold keeps its deadline"
    );
    h.observe_event(15, &press('a'), char_of).unwrap();
    assert_eq!(
        h.hold('a').unwrap().ends_at,
        u64::MAX,
        "a later one has none"
    );
    h.observe_event(20, &press('w'), char_of).unwrap(); // w autorepeats
    assert_eq!(
        h.hold('w').unwrap().ends_at,
        u64::MAX,
        "re-observed: no deadline"
    );
    let ended = h.observe_event(30, &up('w'), char_of).unwrap();
    assert_eq!((ended[0].key, ended[0].end), ('w', KEY_RELEASED));
    assert!(h.is_held('a'));
}

#[test]
fn unsupported_leaves_the_inferred_model_in_place() {
    let mut h = HoldTracker::new(CFG);
    h.observe_event(
        0,
        &Event::KeyReleaseReporting(KeyReleaseReporting::Unsupported),
        char_of,
    )
    .unwrap();
    assert!(!h.releases_reported());
    h.observe_event(10, &press('w'), char_of).unwrap();
    assert_eq!(h.hold('w').unwrap().ends_at, 28);
}

#[test]
fn when_reporting_stops_every_hold_falls_back_to_a_deadline() {
    let mut h = reported();
    h.observe(10, 'w').unwrap();
    h.observe(12, 'd').unwrap();
    h.observe_release(12, 'd').unwrap(); // a tap still ending at 13 as a release
    h.advance(50).unwrap();
    h.set_releases_reported(false);
    assert_eq!(h.hold('w').unwrap().ends_at, 50 + 18, "as if pressed now");
    let ended = h.advance(68).unwrap();
    assert_eq!((ended[0].key, ended[0].end), ('w', HoldEnd::Timeout));
    assert_eq!(
        h.held().count(),
        0,
        "nothing waits for a release that will not come"
    );
}

#[test]
fn when_reporting_stops_an_inferred_deadline_is_kept() {
    // #275 review: an inferred press, reporting turned on without the key observed again, then
    // off: the hold keeps its own deadline (it never waited for a release).
    let mut h = HoldTracker::new(CFG);
    h.observe(0, 'w').unwrap();
    h.advance(10).unwrap();
    h.set_releases_reported(true);
    h.advance(12).unwrap();
    h.set_releases_reported(false);
    assert_eq!(h.hold('w').unwrap().ends_at, 18);
    // Even when that deadline is later than a fresh grace would be (a repeat timeout longer than
    // the grace): only a hold waiting for a release is given a new deadline.
    let mut h = HoldTracker::new(HoldConfig::new(3, 20));
    h.observe(0, 'w').unwrap();
    h.observe(1, 'w').unwrap(); // ends 21
    h.set_releases_reported(true);
    h.advance(3).unwrap();
    h.set_releases_reported(false);
    assert_eq!(h.hold('w').unwrap().ends_at, 21);
}

#[test]
fn a_focus_loss_still_ends_every_hold() {
    let mut h = reported();
    h.observe(1, 'w').unwrap();
    h.observe(2, 'd').unwrap();
    h.observe_release(2, 'd').unwrap(); // pending until 3
    let ended = h
        .observe_event(2, &Event::Focus(FocusChange::Lost), char_of)
        .unwrap();
    assert_eq!(ended.len(), 2);
    assert!(ended
        .iter()
        .all(|e| e.end == HoldEnd::Released(ReleaseCause::FocusLost)));
    // The tap's pending release was cleared with it: a new d hold is not ended as a release.
    h.observe(3, 'd').unwrap();
    assert!(h.advance(200).unwrap().is_empty());
    assert!(h.is_held('d'));
}

// ------------------------------------------------- randomized: tracker vs a reference model

/// SplitMix64: a seeded, replayable source.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

#[derive(Clone, Copy, Debug)]
enum Obs {
    Down(char),
    Up(char),
}

/// A trace of observations at non-decreasing tics, with several per tic sometimes.
fn trace(seed: u64, len: usize) -> Vec<(u64, Obs)> {
    let mut r = Rng(seed);
    let mut t = 0;
    let keys = ['w', 'a', 's', 'd'];
    (0..len)
        .map(|_| {
            t += r.below(4); // 0 = same tic
            let k = keys[r.below(4) as usize];
            (
                t,
                if r.below(3) == 0 {
                    Obs::Up(k)
                } else {
                    Obs::Down(k)
                },
            )
        })
        .collect()
}

/// The reference, written from the rule alone: at each tic, apply that tic's observations in
/// order to a down-set. A key is held at tic `t` when it is down after them, or when a press at
/// `t` was released at `t` (a tap lasts one tic). No deadlines: releases are reported.
fn reference(trace: &[(u64, Obs)], end: u64) -> Vec<Vec<char>> {
    let mut down: Vec<char> = Vec::new();
    let mut out = Vec::new();
    let mut i = 0;
    for t in 0..end {
        let mut tapped: Vec<char> = Vec::new();
        let mut pressed_now: Vec<char> = Vec::new();
        while i < trace.len() && trace[i].0 == t {
            match trace[i].1 {
                Obs::Down(k) => {
                    if !down.contains(&k) {
                        down.push(k);
                        pressed_now.push(k);
                    }
                    tapped.retain(|x| *x != k);
                }
                Obs::Up(k) => {
                    if down.contains(&k) {
                        down.retain(|x| *x != k);
                        if pressed_now.contains(&k) {
                            tapped.push(k);
                        }
                    }
                }
            }
            i += 1;
        }
        let mut held: Vec<char> = down.iter().chain(tapped.iter()).copied().collect();
        held.sort_unstable();
        out.push(held);
    }
    out
}

fn run_tracker(
    trace: &[(u64, Obs)],
    end: u64,
    step: u64,
) -> (Vec<Vec<char>>, Vec<EndedHold<char>>) {
    let mut h = reported();
    let mut ended = Vec::new();
    let mut held_per_tic = Vec::new();
    let mut i = 0;
    for t in 0..end {
        while i < trace.len() && trace[i].0 == t {
            ended.extend(match trace[i].1 {
                Obs::Down(k) => h.observe(t, k).unwrap(),
                Obs::Up(k) => h.observe_release(t, k).unwrap(),
            });
            i += 1;
        }
        if t % step == 0 {
            ended.extend(h.advance(t).unwrap());
        }
        // `is_held` reads the current tic's state; advance to `t` before reading.
        ended.extend(h.advance(t).unwrap());
        let mut held: Vec<char> = h.held().map(|(k, _)| k).collect();
        held.sort_unstable();
        held_per_tic.push(held);
    }
    ended.extend(h.advance(end).unwrap());
    (held_per_tic, ended)
}

#[test]
fn the_tracker_matches_the_reference_on_seeded_traces() {
    for seed in 0..300u64 {
        let tr = trace(seed, 120);
        let end = tr.last().unwrap().0 + 3;
        let want = reference(&tr, end);
        let (got, _) = run_tracker(&tr, end, 1);
        for t in 0..end as usize {
            assert_eq!(got[t], want[t], "seed {seed}, tic {t}: trace {tr:?}");
        }
    }
}

#[test]
fn ended_records_do_not_depend_on_how_often_the_caller_advances() {
    for seed in 0..100u64 {
        let tr = trace(seed, 80);
        let end = tr.last().unwrap().0 + 3;
        // Advancing only when an observation arrives versus every tic: same records.
        let mut sparse = reported();
        let mut a = Vec::new();
        for &(t, o) in &tr {
            a.extend(match o {
                Obs::Down(k) => sparse.observe(t, k).unwrap(),
                Obs::Up(k) => sparse.observe_release(t, k).unwrap(),
            });
        }
        a.extend(sparse.advance(end).unwrap());
        let (_, b) = run_tracker(&tr, end, 1);
        // Every field, in the order returned (#275 review: no sorting, nothing left out).
        assert_eq!(a, b, "seed {seed}");
        assert!(
            a.iter().all(|r| r.end == KEY_RELEASED),
            "seed {seed}: only releases end holds"
        );
    }
}
