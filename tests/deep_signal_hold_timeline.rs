//! Issue #275: DEEP SIGNAL's held-key grace against key timelines **measured through a real
//! terminal emulator**, not derived from settings.
//!
//! Provenance (2026-11-06): xterm on a private Xvfb display, keys driven through XTEST
//! (`xdotool keydown Right sleep 1.5 keyup Right`, then a 120 ms tap), DEEP SIGNAL running
//! inside it with `--record`. The tics below are the `key right` lines of those two traces,
//! one per X autorepeat setting:
//!
//! * `-ardelay 500 -arinterval 33` (this desktop's X11/MATE setting): first repeat 18 tics
//!   after the press (514 ms at 35 Hz), then every 1–2 tics.
//! * X's default 660 ms / 25 Hz: first repeat 23 tics after the press (657 ms), then 1–2 tics.
//!
//! In both, the tap is one event, indistinguishable from the first event of a hold until the
//! hold's first repeat arrives, and xterm sends nothing on release.
//!
//! The laws, on `gibson::hold::HoldTracker` with the game's 35 Hz tics:
//! * the default grace (25 tics) keeps every measured hold held from its press to its last
//!   repeat, with no interruption;
//! * the old default (9 tics) interrupts both: the stall #275 predicted, now observed;
//! * the cost of the default, stated, not hidden: a tap is held for exactly 25 tics.
use gibson::hold::{HoldConfig, HoldTracker};

/// `key right` tics of the 500 ms / 30 Hz trace: a 1.5 s hold, then a tap at 148.
const HOLD_500_30: &[u64] = &[
    66, 84, 85, 87, 88, 88, 90, 91, 91, 93, 95, 96, 96, 98, 99, 101, 101, 103, 104, 105, 106, 107,
    109, 110, 110, 112, 113, 115, 116, 116, 118, 119,
];
const TAP_500_30: u64 = 148;

/// `key right` tics of the X-default 660 ms / 25 Hz trace: a 1.5 s hold, then a tap at 149.
const HOLD_660_25: &[u64] = &[
    67, 90, 92, 93, 95, 96, 97, 99, 100, 102, 103, 105, 106, 107, 109, 110, 112, 113, 114, 116,
    117, 119, 120,
];
const TAP_660_25: u64 = 149;

/// DEEP SIGNAL's default since #275 (`examples/deep_signal.rs`, `parse`).
const DEFAULT: HoldConfig = HoldConfig::new(25, 4);
/// The default before #275.
const OLD_DEFAULT: HoldConfig = HoldConfig::new(9, 4);

/// For each tic in `from..=to`, whether the key was held, after feeding `events`.
fn held_tics(config: HoldConfig, events: &[u64], from: u64, to: u64) -> Vec<bool> {
    let mut t = HoldTracker::new(config);
    let mut next = events.iter().peekable();
    let mut out = Vec::new();
    for tic in 0..=to {
        while next.peek().is_some_and(|&&e| e == tic) {
            t.observe(tic, ()).expect("observe");
            next.next();
        }
        t.advance(tic).expect("advance");
        if tic >= from {
            out.push(t.is_held(()));
        }
    }
    out
}

fn stalled_tics(config: HoldConfig, hold: &[u64]) -> usize {
    let (press, last) = (hold[0], *hold.last().unwrap());
    held_tics(config, hold, press, last)
        .iter()
        .filter(|h| !**h)
        .count()
}

#[test]
fn the_default_grace_holds_both_measured_timelines_without_interruption() {
    assert_eq!(stalled_tics(DEFAULT, HOLD_500_30), 0, "500 ms / 30 Hz");
    assert_eq!(stalled_tics(DEFAULT, HOLD_660_25), 0, "660 ms / 25 Hz");
}

#[test]
fn the_old_default_stalls_both_measured_holds() {
    // The control: under the old 9-tic grace the hold drops at press + 9 and resumes at the
    // first repeat. 84 - 75 = 9 tics (257 ms) and 90 - 76 = 14 tics (400 ms).
    assert_eq!(stalled_tics(OLD_DEFAULT, HOLD_500_30), 9);
    assert_eq!(stalled_tics(OLD_DEFAULT, HOLD_660_25), 14);
}

#[test]
fn the_default_bridges_the_measured_delays_with_margin() {
    // The first-repeat delays, in tics, read off the traces; the grace exceeds both.
    let delay_500 = HOLD_500_30[1] - HOLD_500_30[0];
    let delay_660 = HOLD_660_25[1] - HOLD_660_25[0];
    assert_eq!((delay_500, delay_660), (18, 23));
    assert!(DEFAULT.initial_grace() > delay_660.max(delay_500));
    // And the repeat timeout covers the largest measured gap between repeats.
    let max_gap = [HOLD_500_30, HOLD_660_25]
        .iter()
        .flat_map(|h| h[1..].windows(2).map(|w| w[1] - w[0]))
        .max()
        .unwrap();
    assert!(
        max_gap < DEFAULT.repeat_timeout(),
        "max repeat gap {max_gap}"
    );
}

#[test]
fn the_cost_a_tap_is_held_for_exactly_the_grace() {
    // A press-only terminal sends one event for a tap; nothing distinguishes it from a hold's
    // start until a repeat would have come. The tap is therefore held for the whole grace.
    for tap in [TAP_500_30, TAP_660_25] {
        let held = held_tics(DEFAULT, &[tap], tap, tap + 40);
        assert_eq!(held.iter().filter(|h| **h).count(), 25, "tap at {tap}");
        assert!(
            held[..25].iter().all(|h| *h) && !held[25],
            "contiguous from the press"
        );
    }
}

/// The laws above are about DEEP SIGNAL's real default: run the game on a PTY, quit, and read
/// the hold configuration its `--record` trace header names (the replay contract binds it).
#[cfg(unix)]
#[test]
fn the_game_records_the_default_hold_configuration_in_its_trace() {
    #[path = "common/pty_capture.rs"]
    mod pty_capture;
    use std::time::{Duration, Instant};
    let trace = std::env::temp_dir().join(format!(
        "deep_signal_hold_default_{}.trace",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&trace);
    let mut cmd = portable_pty::CommandBuilder::new(pty_capture::example_path("deep_signal"));
    cmd.args(["--seed", "7", "--max-seconds", "10", "--record"]);
    cmd.arg(&trace);
    cmd.env("TERM", "xterm-256color");
    let mut cap = pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_secs(30));
    cap.collect_until(|o| {
        let s = String::from_utf8_lossy(o);
        s.find("SHARDS")
            .is_some_and(|i| s[i..].contains("\x1b[?2026l"))
    })
    .expect("first complete frame");
    cap.write(b"q").expect("quit");
    let t = Instant::now();
    while cap.try_wait().expect("poll child").is_none() {
        assert!(
            t.elapsed() < Duration::from_secs(10),
            "the game did not quit"
        );
        let _ = cap.collect_for(Duration::from_millis(20));
    }
    let body = std::fs::read_to_string(&trace).expect("trace written on quit");
    let _ = std::fs::remove_file(&trace);
    let header = body.lines().next().unwrap_or_default();
    assert!(
        header.contains(&format!(
            "\tgrace={}\trepeat={}",
            DEFAULT.initial_grace(),
            DEFAULT.repeat_timeout()
        )),
        "the game's default hold is not the one the timeline laws are checked for: {header:?}"
    );
}
