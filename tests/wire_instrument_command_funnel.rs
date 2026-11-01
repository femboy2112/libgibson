//! Command-funnel + queue-depth high-water battery for the wire instrument (#143, Gate F).
//!
//! Gate E (#204) wired the RENDER-loop funnel (frame / latency / byte / provenance) into the
//! real renderer and explicitly ceded the COMMAND funnel to the wire bridge. #143 also asks for
//! the command funnel — issued / delivered / acknowledged, as DISTINCT quantities (law L7) — and
//! for bounded queues with truthful high-water. The `CommandChannel` (#200) already ships those
//! per-fate counters, and the hostile battery `wire_instrument_command_battery.rs` (c01-c20)
//! already proves cross-store CONSERVATION, per-fate TALLIES, and that saturation REFUSES. This
//! battery adds the parts #143 asks for that those do not yet measure, with Gate E's exact
//! honesty discipline — deterministic COUNTS only, no wall-clock latency VALUE asserted (so it is
//! reproducible on any host, libm-golden-free, no toolchain pin):
//!
//!  * QUEUE-DEPTH HIGH-WATER over a whole session — the peak `pending()` / `outstanding()` a run
//!    ever reached. No existing test tracks a session high-water (the shipped `RetentionEnvelope`
//!    is byte-based, a different quantity). It is tracked in the HARNESS (a max-accumulator), not
//!    the library, exactly as Gate E keeps `total_changed_cells` in the bench rather than the lib.
//!  * The high-water DISTINGUISHES a starved session from a healthy one (the teeth / calibrated
//!    negative control): the same edit volume reads peak == COMMAND_CAP when never drained, and
//!    peak == 1 when promptly drained. A canned number could not tell them apart.
//!  * The structural law `outstanding() >= pending()` holds at EVERY step — a command is acked
//!    only after delivery, so the ledger's live set can never dip below the bridge's queue. This
//!    is why, with equal caps, the ledger ceiling is always hit first and `BridgeFull` is
//!    structurally unreachable — sharpening c15's deliberately hedged `LedgerFull | BridgeFull`.
//!  * The funnel TRACKS REAL WORK and REPLAYS identically: the per-fate counts are measured from
//!    the actual command fates (more acks -> strictly more `acknowledged()`), and a fixed scripted
//!    session replays byte-identically (mirrors Gate E `determinism_counts_replay_exactly`).
//!
//! Boundary ceded in writing (not silently): command LATENCY (wall-clock issue->deliver->ack) is
//! NOT measured here. There is no command `Stage` in the library, and the real round-trip is
//! wall-clock, which #143/L10 forbid asserting; measuring it would mean either a lib change or an
//! injected-distribution histogram that only duplicates Gate E's stall-detection discipline. It is
//! deferred until a consumer needs a command-latency budget. Retention-BYTES is also not re-wired
//! here: #182 already proves the sustained-session `RetentionEnvelope` over `FieldInbox`.

#[path = "../examples/wire_instrument/channel.rs"]
mod channel;

use channel::{CommandChannel, LocalRefusal, COMMAND_CAP};
use gibson::wire::{AckStatus, WireAck};

const KIND: u32 = 1;

fn ack(id: u64, status: AckStatus) -> WireAck {
    WireAck {
        command_id: id,
        status,
    }
}

/// A deterministic funnel receipt for one session — every quantity is a COUNT, never a clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tally {
    issued: u64,
    acknowledged: u64,
    refused: u64,
    failed: u64,
    delivered_frames: u64,
    transport_failed: u64,
    /// The peak `pending()` the session ever reached (queue-depth high-water).
    peak_pending: usize,
    /// The peak `outstanding()` the session ever reached.
    peak_outstanding: usize,
    /// Edits refused locally (backpressure) at `issue_tracked`.
    local_refusals: usize,
}

/// Drives one `CommandChannel` over a healthy (never-faulting) `Vec<u8>` sink, tracking the
/// queue-depth high-water and asserting the session invariants after EVERY mutation. Delivery
/// faults are not exercised here — c01/c02/c03/c17/c18 own that; a `Vec` sink always lands.
struct Session {
    ch: CommandChannel,
    sink: Vec<u8>,
    source: u64,
    peak_pending: usize,
    peak_outstanding: usize,
    local_refusals: usize,
}

impl Session {
    fn new(source: u64, epoch: u32) -> Self {
        let mut s = Session {
            ch: CommandChannel::new(source, epoch),
            sink: Vec::new(),
            source,
            peak_pending: 0,
            peak_outstanding: 0,
            local_refusals: 0,
        };
        s.observe();
        s
    }

    /// Record the high-water and assert the standing session invariants.
    fn observe(&mut self) {
        let pending = self.ch.pending();
        let outstanding = self.ch.outstanding();
        self.peak_pending = self.peak_pending.max(pending);
        self.peak_outstanding = self.peak_outstanding.max(outstanding);
        assert!(
            self.ch.conserves(),
            "cross-store conservation must hold at every step"
        );
        assert!(
            outstanding >= pending,
            "a command is acked only after delivery, so outstanding ({outstanding}) must never \
             dip below pending ({pending})"
        );
        assert!(
            pending <= COMMAND_CAP && outstanding <= COMMAND_CAP,
            "queue depth must never exceed the cap (pending={pending} outstanding={outstanding})"
        );
    }

    /// Issue one edit; `Some(id)` when admitted, `None` when locally refused (counted).
    fn issue(&mut self, arg: Vec<u8>) -> Option<u64> {
        let r = self.ch.issue_tracked(KIND, arg);
        let out = match r {
            Ok(id) => Some(id),
            Err(_) => {
                self.local_refusals += 1;
                None
            }
        };
        self.observe();
        out
    }

    /// Deliver the next pending command onto the sink (never faults); returns the delivered id.
    fn deliver_one(&mut self) -> Option<u64> {
        let id = self
            .ch
            .deliver_next(&mut self.sink)
            .expect("a Vec sink never faults");
        self.observe();
        id
    }

    /// Resolve a delivered id with a terminal verdict.
    fn resolve(&mut self, id: u64, status: AckStatus) {
        self.ch
            .resolve(self.source, ack(id, status))
            .expect("resolve a delivered id with a terminal status");
        self.observe();
    }

    /// Issue -> deliver -> resolve one edit to a terminal fate, in lockstep.
    fn issue_deliver_resolve(&mut self, status: AckStatus) {
        let id = self.issue(Vec::new()).expect("admitted below the cap");
        let delivered = self.deliver_one().expect("the just-issued edit is pending");
        assert_eq!(delivered, id, "FIFO delivery returns the issued id");
        self.resolve(id, status);
    }

    fn tally(&self) -> Tally {
        Tally {
            issued: self.ch.issued(),
            acknowledged: self.ch.acknowledged(),
            refused: self.ch.refused(),
            failed: self.ch.failed(),
            delivered_frames: self.ch.delivered_frames(),
            transport_failed: self.ch.transport_failed(),
            peak_pending: self.peak_pending,
            peak_outstanding: self.peak_outstanding,
            local_refusals: self.local_refusals,
        }
    }
}

/// Queue-depth high-water reaches EXACTLY the cap when edits are issued and never drained, and
/// over-cap issues are refused without growing either store.
#[test]
fn queue_depth_high_water_reaches_the_cap_under_issue_starvation() {
    let mut s = Session::new(1, 1);
    let over = COMMAND_CAP + 50;
    let mut admitted = 0usize;
    for _ in 0..over {
        if s.issue(Vec::new()).is_some() {
            admitted += 1;
        }
    }
    let t = s.tally();
    assert_eq!(
        admitted, COMMAND_CAP,
        "exactly the cap is admitted before backpressure"
    );
    assert_eq!(
        t.local_refusals,
        over - COMMAND_CAP,
        "every over-cap issue is refused"
    );
    assert_eq!(
        t.issued, COMMAND_CAP as u64,
        "refused issues entered neither store"
    );
    assert_eq!(
        t.peak_pending, COMMAND_CAP,
        "pending high-water reaches exactly the cap"
    );
    assert_eq!(
        t.peak_outstanding, COMMAND_CAP,
        "outstanding high-water reaches exactly the cap"
    );
}

/// THE TEETH (calibrated negative control): the SAME edit volume as the starved run, but
/// promptly drained, keeps the queue-depth high-water at 1. The metric distinguishes a starved
/// session from a healthy one — a canned number could not.
#[test]
fn queue_depth_high_water_stays_minimal_under_prompt_drain() {
    let mut s = Session::new(1, 1);
    let n = COMMAND_CAP + 50; // identical volume to the starvation test
    for _ in 0..n {
        s.issue_deliver_resolve(AckStatus::Acknowledged);
    }
    let t = s.tally();
    assert_eq!(t.issued, n as u64, "prompt drain never hits backpressure");
    assert_eq!(t.local_refusals, 0);
    assert_eq!(t.acknowledged, n as u64);
    assert_eq!(
        t.peak_pending, 1,
        "the queue never holds more than the one in flight"
    );
    assert_eq!(
        t.peak_outstanding, 1,
        "outstanding never exceeds the one awaiting its ack"
    );
}

/// The funnel tracks the ACTUAL command fates: a scripted mix lands in the right distinct
/// buckets, and every delivered edit reaches exactly one terminal fate.
#[test]
fn funnel_tracks_the_actual_command_fates() {
    let mut s = Session::new(7, 3);
    for _ in 0..5 {
        s.issue_deliver_resolve(AckStatus::Acknowledged);
    }
    for _ in 0..3 {
        s.issue_deliver_resolve(AckStatus::Refused);
    }
    for _ in 0..2 {
        s.issue_deliver_resolve(AckStatus::Failed);
    }
    let t = s.tally();
    assert_eq!(t.issued, 10);
    assert_eq!(t.acknowledged, 5);
    assert_eq!(t.refused, 3);
    assert_eq!(t.failed, 2);
    assert_eq!(t.delivered_frames, 10, "all ten landed on the sink");
    assert_eq!(t.transport_failed, 0);
    assert_eq!(
        t.acknowledged + t.refused + t.failed,
        t.delivered_frames,
        "every delivered edit reached exactly one terminal fate — none vanished, none double-counted"
    );
}

/// The count is MEASURED from real fates, not canned: more acknowledged edits yield strictly
/// more `acknowledged()` (the #143 anti-pattern, a synthetic number, made impossible).
#[test]
fn more_acknowledged_edits_raise_the_acknowledged_count() {
    fn acked(n: u64) -> u64 {
        let mut s = Session::new(1, 1);
        for _ in 0..n {
            s.issue_deliver_resolve(AckStatus::Acknowledged);
        }
        let t = s.tally();
        assert_eq!(t.acknowledged, n, "the bucket equals the work done");
        t.acknowledged
    }
    assert!(acked(8) > acked(3));
}

/// A fixed scripted session — a burst that drives the queue up, then mixed drains and verdicts,
/// leaving some edits outstanding — replays byte-identically (determinism of COUNTS).
#[test]
fn funnel_and_high_water_replay_identically() {
    fn mixed_session() -> Tally {
        let mut s = Session::new(2, 9);
        // A burst of 100 issues with no delivery drives the queue-depth high-water up...
        let mut ids = Vec::new();
        for _ in 0..100 {
            ids.push(s.issue(Vec::new()).expect("100 < cap"));
        }
        // ...then drain and resolve a deterministic 60 ack / 10 refuse / 5 fail, FIFO.
        for (i, &id) in ids.iter().enumerate() {
            if i >= 75 {
                break; // leave 25 queued/outstanding at the end
            }
            let delivered = s.deliver_one().expect("still pending");
            assert_eq!(delivered, id);
            let status = match i % 15 {
                0 => AckStatus::Refused, // ~5 of the first 75
                7 => AckStatus::Failed,  // ~5 of the first 75
                _ => AckStatus::Acknowledged,
            };
            s.resolve(id, status);
        }
        s.tally()
    }
    let a = mixed_session();
    let b = mixed_session();
    assert_eq!(
        a, b,
        "the whole funnel + high-water replays identically across runs"
    );
    assert_eq!(
        a.peak_pending, 100,
        "the 100-issue burst is the pending high-water"
    );
    assert_eq!(a.issued, 100);
    assert!(a.acknowledged + a.refused + a.failed <= a.delivered_frames);
}

/// SHARPENING c15: under issue-only starvation the refusal is specifically `LedgerFull`, never
/// `BridgeFull` — with equal caps and the `outstanding >= pending` invariant, the ledger ceiling
/// is always reached first, so `BridgeFull` is structurally unreachable on this channel.
#[test]
fn saturation_refusal_is_ledger_full_because_outstanding_never_trails_pending() {
    let mut ch = CommandChannel::new(1, 1);
    for _ in 0..COMMAND_CAP {
        let id = ch.issue_tracked(KIND, Vec::new()).expect("below the cap");
        // Never deliver: outstanding and pending climb together and stay equal.
        assert_eq!(ch.outstanding(), ch.pending());
        let _ = id;
    }
    assert_eq!(ch.pending(), COMMAND_CAP);
    assert_eq!(ch.outstanding(), COMMAND_CAP);
    assert_eq!(
        ch.issue_tracked(KIND, Vec::new()),
        Err(LocalRefusal::LedgerFull),
        "the ledger ceiling is hit first — BridgeFull cannot fire with equal caps"
    );
    assert_eq!(
        ch.pending(),
        COMMAND_CAP,
        "a refused issue never grows the queue"
    );
    assert!(ch.conserves());
}
