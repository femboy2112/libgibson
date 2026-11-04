//! Behavioral MODEL / fixture for issue #15 — coincident resize/key input readiness.
//!
//! **This is a behavioral model, not a fix and not a backend measurement.** The defect is upstream
//! in crossterm (PR #1128, unmerged as of 2026-11-03): when a key arrives COINCIDENT with a resize,
//! crossterm consumes the shared read edge for the resize and leaves the key byte buffered; a
//! subsequent `event::poll(ZERO)` then reports "nothing" even though a key is waiting, and the byte
//! is surfaced only when the NEXT input edge arrives. So a lone key pressed at the instant of a
//! resize is **stranded until the user presses something else.** LibGibson's real-tty poll loop
//! (`src/input.rs::poll_event_unix_tty`) paces around it but cannot un-strand a byte crossterm will
//! not hand over.
//!
//! The acceptance criterion for closing #15 is specific and is encoded here as an executable
//! assertion: **the lone queued key must be delivered WITHOUT requiring another input event.** A
//! workaround where some *later* event shakes the key loose does NOT satisfy it — so this fixture
//! deliberately refuses to let "another event eventually surfaced the key" (or a generic PTY
//! walkthrough that merely boots) masquerade as a fix.
//!
//! **Scope and limits (RXII A3).** [`StrandingSource`] implements the public
//! [`gibson::EventSource`] seam and encodes the stranding we BELIEVE crossterm exhibits, with a
//! `fixed` flag standing in for "crossterm #1128 applied". It is deterministic and terminal-free —
//! which is exactly why it is a MODEL and **not** evidence about the real backend: it does not
//! exercise crossterm's actual event readiness over a real PTY, so a green `the_fix_delivers_*` here
//! is a statement about this model, not a measurement that any real crossterm version delivers the
//! key. Accepting or refuting a backend fix still requires a real PTY/readiness experiment against
//! pinned vs. patched crossterm (RXII Gate E3). This file touches no `src`, claims no fix, and will
//! flip which arm is "known-failing" only when a backend experiment confirms the upstream fix — at
//! which point `issue_15_is_still_reproducible_*` is updated and #15 closed on that evidence, not on
//! this model.

use std::collections::VecDeque;
use std::io;
use std::time::Duration;

use gibson::{Event, EventSource, KeyEvent};

fn key(c: char) -> Event {
    Event::Key(KeyEvent::char(c))
}

fn resize() -> Event {
    Event::Resize(100, 40)
}

/// A read edge: the set of events crossterm reads together in one wakeup. A real terminal hands a
/// resize and a coincident keypress to the same `read`, which is where the stranding happens.
type Burst = Vec<Event>;

/// Deterministic model of the crossterm input-readiness defect (issue #15 / crossterm #1128).
///
/// One `poll_event` call models one `event::poll(ZERO)` + `read`. Rules:
/// * A new burst (edge) is consumed by a poll only when nothing is already surfaced.
/// * Within a burst that contains a `Resize`, the resize is surfaced but any `Key` is **stranded**.
/// * A stranded key is released only on the arrival of the NEXT burst (it piggybacks on that edge),
///   NOT by continued polling of a quiescent stream — unless `fixed` is set, in which case the very
///   next poll surfaces it (a re-read recovers the buffered byte, as crossterm #1128 does).
struct StrandingSource {
    fixed: bool,
    bursts: VecDeque<Burst>,
    ready: VecDeque<Event>,
    stranded: VecDeque<Event>,
}

impl StrandingSource {
    fn new(fixed: bool, bursts: Vec<Burst>) -> Self {
        Self {
            fixed,
            bursts: bursts.into(),
            ready: VecDeque::new(),
            stranded: VecDeque::new(),
        }
    }

    /// Fold one read edge into the surfaced/stranded state. A new edge first releases any stranded
    /// key (the "next edge" that un-stalls it on the real terminal), then surfaces its own events —
    /// stranding a key that shares the burst with a resize.
    fn absorb(&mut self, burst: Burst) {
        while let Some(held) = self.stranded.pop_front() {
            self.ready.push_back(held);
        }
        let has_resize = burst.iter().any(|e| matches!(e, Event::Resize(..)));
        for ev in burst {
            match ev {
                Event::Key(_) if has_resize => self.stranded.push_back(ev),
                _ => self.ready.push_back(ev),
            }
        }
    }
}

impl EventSource for StrandingSource {
    fn poll_event(&mut self, _timeout: Duration) -> io::Result<Option<Event>> {
        if let Some(ev) = self.ready.pop_front() {
            return Ok(Some(ev));
        }
        // Nothing already surfaced. A fixed crossterm re-reads and recovers a stranded byte without
        // needing a fresh edge; the defect cannot, and waits for the next burst.
        if self.fixed {
            if let Some(held) = self.stranded.pop_front() {
                return Ok(Some(held));
            }
        }
        if let Some(burst) = self.bursts.pop_front() {
            self.absorb(burst);
            return Ok(self.ready.pop_front());
        }
        Ok(None)
    }
}

const QUIET: Duration = Duration::from_millis(16);

/// Poll `n` times with NO new input injected, collecting what was delivered.
fn drain_quiet(src: &mut StrandingSource, n: usize) -> Vec<Event> {
    let mut out = Vec::new();
    for _ in 0..n {
        if let Some(ev) = src.poll_event(QUIET).expect("model never errors") {
            out.push(ev);
        }
    }
    out
}

/// THE REPRODUCER. A key coincident with a resize, then silence. On the defect the resize is
/// delivered and the key is stranded: no amount of continued polling delivers it. This test is
/// GREEN while #15 is unfixed (the defect reproduces); when crossterm #1128 lands it must be updated
/// and #15 closed.
#[test]
fn issue_15_is_still_reproducible_a_lone_key_coincident_with_a_resize_is_stranded() {
    let mut src = StrandingSource::new(false, vec![vec![resize(), key('x')]]);

    // Poll once: the resize wins the shared read edge.
    assert_eq!(
        src.poll_event(QUIET).unwrap(),
        Some(resize()),
        "the coincident resize is delivered first"
    );

    // Poll many more times with NO further input. The key never surfaces — this is #15.
    let delivered = drain_quiet(&mut src, 16);
    assert!(
        !delivered.contains(&key('x')),
        "issue #15: a lone key coincident with a resize is NOT delivered without another input \
         event (got {delivered:?})"
    );
}

/// THE ACCEPTANCE CRITERION. With crossterm #1128 applied (`fixed`), the same scenario delivers the
/// resize and then the stranded key on the very next poll — WITHOUT any further input event. This is
/// precisely what closing #15 requires; it is the assertion the upstream fix must satisfy.
#[test]
fn the_fix_delivers_the_lone_key_without_requiring_another_event() {
    let mut src = StrandingSource::new(true, vec![vec![resize(), key('x')]]);

    assert_eq!(src.poll_event(QUIET).unwrap(), Some(resize()));

    // No new input injected — just keep polling the quiescent stream.
    let delivered = drain_quiet(&mut src, 16);
    assert_eq!(
        delivered,
        vec![key('x')],
        "acceptance: the stranded key is delivered without another input event"
    );
}

/// A later event shaking the key loose does NOT satisfy the criterion. On the defect, injecting a
/// *second* key finally surfaces the stranded first key (piggybacked on the new edge) — but it took
/// another input event to do so, which is exactly the failure #15 describes. This guards against
/// "generic PTY success closes #15": a walkthrough that always has a next keystroke would hide the
/// defect, so the reproducer pins that the key appears ONLY after the nudge.
#[test]
fn a_subsequent_event_surfacing_the_key_is_not_acceptance() {
    let mut src = StrandingSource::new(false, vec![vec![resize(), key('x')]]);

    assert_eq!(src.poll_event(QUIET).unwrap(), Some(resize()));

    // Quiescent: the stranded key is withheld.
    assert!(
        drain_quiet(&mut src, 8).is_empty(),
        "while no further input arrives, the stranded key stays stranded"
    );

    // Now the user presses something else — a new edge. Only now does the stranded key appear,
    // ahead of the new key. It required another input event: NOT acceptance.
    src.bursts.push_back(vec![key('y')]);
    let after = drain_quiet(&mut src, 8);
    assert_eq!(
        after,
        vec![key('x'), key('y')],
        "the stranded key surfaces ONLY once another event arrives — the #15 failure, not a fix"
    );
}

/// CONTROL: a key with no coincident resize is delivered immediately on both the defect and the
/// fixed model, so the stranding above is specific to the resize coincidence, not a general delivery
/// failure of the model.
#[test]
fn a_key_with_no_coincident_resize_is_delivered_immediately() {
    for fixed in [false, true] {
        let mut src = StrandingSource::new(fixed, vec![vec![key('a')]]);
        assert_eq!(
            src.poll_event(QUIET).unwrap(),
            Some(key('a')),
            "a lone key (no coincident resize) is delivered at once (fixed={fixed})"
        );
    }
}
