//! Hostile, red-first tests for the *bounded* [`SnapshotBridge`] (Finding A, issue #141).
//!
//! Every test here is red-by-construction against the pre-fix module: the old
//! `SnapshotBridge` had no [`Capacity`], no typed [`Admission`]/[`FramePublish`], no
//! `retire_source`/`restart_source`, and no `retained_frame_bytes`. The old `issue`
//! returned `()` and could never refuse; the old `publish_frame` returned `bool` and had
//! no source cap. So these assertions cannot even be *written* against the old API — they
//! name types and methods that did not exist — which is precisely the point: the bugs were
//! unbounded growth with no surface on which to express a bound. Each test states, in its
//! own comment, exactly which missing capability made it impossible before.
//!
//! I don't trust a fix I can only describe. I trust one I can falsify. These are the cuts
//! that would bleed if the bound were a lie.

use gibson::frame::{Frame, FrameSource};
use gibson::snapshot::{
    Admission, Capacity, FramePublish, RefuseReason, SnapshotBridge, MAX_FRAME_BYTES,
};

/// A minimal `w`x`h` frame from `source` at `seq`. Bytes are packed RGB, so its raster
/// pins `w * h * 3` bytes — the real cost `retained_frame_bytes` must account for.
fn frame_wh(source: u64, seq: u64, w: u16, h: u16) -> Frame {
    let len = w as usize * h as usize * 3;
    let bytes = vec![(seq as u8); len];
    Frame::from_rgb_bytes(FrameSource(source), seq, w, h, w as usize * 3, &bytes)
        .expect("frame ingests")
}

/// A 1x1 frame — the cheapest distinct observation, for source-count tests.
fn frame(source: u64, seq: u64) -> Frame {
    frame_wh(source, seq, 1, 1)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Cmd {
    Edit(u32),
}

#[test]
fn flood_past_command_cap_is_typed_refused_prefix_preserved_in_order() {
    // RED ON OLD CODE: the old `issue(&mut self, C)` returned `()` — it could not refuse,
    // and there was no `Capacity::max_pending_commands` to bound it. A flood simply grew
    // the VecDeque forever. You cannot express "the 4th is Refused" against a `()` return.
    //
    // Here: cap pending at 3, offer 5 distinct edits with a stalled consumer (no drain).
    // The first 3 are Accepted in order; the last 2 are typed-Refused (NOT silently
    // dropped, NOT coalesced); the accepted prefix survives intact and conservation holds.
    let cap = Capacity::new(8, 3);
    let mut bridge: SnapshotBridge<Cmd> = SnapshotBridge::with_capacity(cap);

    assert_eq!(bridge.issue(Cmd::Edit(0)), Admission::Accepted);
    assert_eq!(bridge.issue(Cmd::Edit(1)), Admission::Accepted);
    assert_eq!(bridge.issue(Cmd::Edit(2)), Admission::Accepted);
    assert_eq!(
        bridge.issue(Cmd::Edit(3)),
        Admission::Refused(RefuseReason::CommandQueueFull { capacity: 3 })
    );
    assert_eq!(
        bridge.issue(Cmd::Edit(4)),
        Admission::Refused(RefuseReason::CommandQueueFull { capacity: 3 })
    );

    // Lifecycle: 5 offered = 3 accepted + 2 refused; 3 accepted = 0 delivered + 3 pending.
    assert_eq!(bridge.commands_offered(), 5);
    assert_eq!(bridge.commands_accepted(), 3);
    assert_eq!(bridge.commands_refused(), 2);
    assert_eq!(bridge.pending_commands(), 3);
    assert!(bridge.commands_conserved());

    // The accepted prefix is preserved in EXACT issue order — nothing silently dropped.
    assert_eq!(
        bridge.drain_commands(),
        vec![Cmd::Edit(0), Cmd::Edit(1), Cmd::Edit(2)]
    );
    assert_eq!(bridge.commands_delivered(), 3);
    assert!(bridge.commands_conserved());
}

#[test]
fn mint_sources_past_source_cap_is_refused_live_sources_untouched() {
    // RED ON OLD CODE: the old `latest: HashMap` had NO source cap and `publish_frame`
    // returned `bool` with no "refused new source" value. Minting fresh FrameSources grew
    // the map without limit (the leak). There was no `FramePublish::SourceCapReached` to
    // assert, and no cap to hit.
    //
    // Here: cap sources at 2. Publish src0, src1 (both stored). A THIRD distinct source is
    // refused — and crucially no live source is evicted to make room.
    let cap = Capacity::new(2, 16);
    let mut bridge: SnapshotBridge<Cmd> = SnapshotBridge::with_capacity(cap);

    assert_eq!(bridge.publish_frame(frame(0, 1)), FramePublish::Stored);
    assert_eq!(bridge.publish_frame(frame(1, 1)), FramePublish::Stored);
    assert_eq!(bridge.live_sources(), 2);

    assert_eq!(
        bridge.publish_frame(frame(2, 1)),
        FramePublish::SourceCapReached { capacity: 2 }
    );

    // Growth is bounded and the live sources are exactly the two we admitted first.
    assert_eq!(bridge.live_sources(), 2);
    assert_eq!(bridge.latest_frame(FrameSource(0)).map(Frame::seq), Some(1));
    assert_eq!(bridge.latest_frame(FrameSource(1)).map(Frame::seq), Some(1));
    assert!(bridge.latest_frame(FrameSource(2)).is_none());

    // An EXISTING source still coalesces normally under the cap — the cap only gates
    // newcomers, never the latest-wins update of someone already admitted.
    assert_eq!(bridge.publish_frame(frame(0, 2)), FramePublish::Stored);
    assert_eq!(bridge.latest_frame(FrameSource(0)).map(Frame::seq), Some(2));
    assert_eq!(bridge.live_sources(), 2);
}

#[test]
fn retire_source_frees_bytes_and_allows_fresh_republish() {
    // RED ON OLD CODE: there was no `retire_source` and no `retained_frame_bytes`. Once a
    // source was in the map its frame bytes were pinned forever (no retirement). You could
    // not observe bytes dropping because you could not observe bytes at all.
    //
    // Here: a 4x4 frame pins 4*4*3 = 48 bytes. Retire frees exactly those; the receipt
    // reports them; and the retired source can be re-published fresh afterward.
    let mut bridge: SnapshotBridge<Cmd> = SnapshotBridge::new();
    assert_eq!(
        bridge.publish_frame(frame_wh(0, 10, 4, 4)),
        FramePublish::Stored
    );
    assert_eq!(bridge.retained_frame_bytes(), 48);

    let receipt = bridge.retire_source(FrameSource(0));
    assert!(receipt.had_frame);
    assert_eq!(receipt.freed_bytes, 48);
    assert_eq!(receipt.source, FrameSource(0));
    assert_eq!(bridge.retained_frame_bytes(), 0);
    assert_eq!(bridge.live_sources(), 0);

    // Re-publishing a retired source is a first frame again: accepted at any seq.
    assert_eq!(
        bridge.publish_frame(frame_wh(0, 1, 2, 2)),
        FramePublish::Stored
    );
    assert_eq!(bridge.retained_frame_bytes(), 2 * 2 * 3);
    assert_eq!(bridge.latest_frame(FrameSource(0)).map(Frame::seq), Some(1));

    // Retiring an unknown source is an honest no-op receipt, not a panic.
    let none = bridge.retire_source(FrameSource(99));
    assert!(!none.had_frame);
    assert_eq!(none.freed_bytes, 0);
}

#[test]
fn producer_restart_resets_seq_and_fresh_low_seq_is_accepted() {
    // RED ON OLD CODE: strict latest-wins (`seq <= stored => stale`) with no epoch/restart
    // mechanism would WRONGLY reject a restarted producer's fresh low-seq frame as stale,
    // and there was no `restart_source` to arm the exception. The old code had no way to
    // ever accept a lower seq for an existing source.
    //
    // Here: publish high seq 1000; the producer "restarts" (restart_source); publish a
    // fresh low seq 1. Without the epoch arm this would be Superseded (stale). With it, the
    // fresh frame is Stored and becomes the new baseline.
    let mut bridge: SnapshotBridge<Cmd> = SnapshotBridge::new();
    assert_eq!(bridge.publish_frame(frame(0, 1000)), FramePublish::Stored);

    // Sanity: without a restart, a low seq IS stale (strict latest-wins still rules).
    assert_eq!(bridge.publish_frame(frame(0, 1)), FramePublish::Superseded);
    assert_eq!(
        bridge.latest_frame(FrameSource(0)).map(Frame::seq),
        Some(1000)
    );

    // Arm the restart; now the fresh low seq is accepted, not rejected.
    assert!(bridge.restart_source(FrameSource(0)));
    assert_eq!(bridge.publish_frame(frame(0, 1)), FramePublish::Stored);
    assert_eq!(bridge.latest_frame(FrameSource(0)).map(Frame::seq), Some(1));

    // The arm disarms after one accept: the new low seq is the baseline, so an even lower
    // seq (0) is stale again. Exactly one fresh-start frame is tolerated per restart.
    assert_eq!(bridge.publish_frame(frame(0, 0)), FramePublish::Superseded);
    assert_eq!(bridge.latest_frame(FrameSource(0)).map(Frame::seq), Some(1));

    // restart on an unknown source is a no-op (its next frame is a first frame anyway).
    assert!(!bridge.restart_source(FrameSource(42)));
}

#[test]
fn law_iv_heavy_frame_churn_never_perturbs_the_command_stream() {
    // LAW IV (preserved): a storm of frames — supersessions, a stale drop, a refused new
    // source, a retirement, a restart — must leave the command stream exactly as issued.
    // Frame coalescing is lossy by design; the command channel is not. If any frame event
    // leaked into the queue, the drained order or count would be wrong.
    let cap = Capacity::new(1, 64); // one source only, so newcomers get refused mid-storm
    let mut bridge: SnapshotBridge<Cmd> = SnapshotBridge::with_capacity(cap);

    assert_eq!(bridge.issue(Cmd::Edit(0)), Admission::Accepted);
    for seq in 1..=50 {
        bridge.publish_frame(frame(0, seq)); // supersessions on the one live source
    }
    assert_eq!(bridge.issue(Cmd::Edit(1)), Admission::Accepted);
    // A newcomer is refused (cap 1), a stale frame is dropped, a restart + retire happen —
    // none of it may touch the commands.
    assert_eq!(
        bridge.publish_frame(frame(7, 5)),
        FramePublish::SourceCapReached { capacity: 1 }
    );
    assert_eq!(bridge.publish_frame(frame(0, 10)), FramePublish::Superseded);
    assert!(bridge.restart_source(FrameSource(0)));
    assert_eq!(bridge.issue(Cmd::Edit(2)), Admission::Accepted);
    let _ = bridge.retire_source(FrameSource(0));

    assert!(bridge.commands_conserved());
    assert_eq!(
        bridge.drain_commands(),
        vec![Cmd::Edit(0), Cmd::Edit(1), Cmd::Edit(2)]
    );
    assert_eq!(bridge.commands_delivered(), 3);
    assert!(bridge.commands_conserved());
}

#[test]
fn retained_bytes_respect_the_documented_worst_case_bound() {
    // RED ON OLD CODE: no `retained_frame_bytes`, no `worst_case_retained_bytes`, no cap —
    // "retained bytes are bounded" was not a statement the old API could even make. Here
    // the whole thesis: retained bytes are capped by max_sources * MAX_FRAME_BYTES, always.
    let cap = Capacity::new(3, 16);
    let mut bridge: SnapshotBridge<Cmd> = SnapshotBridge::with_capacity(cap);

    assert_eq!(bridge.worst_case_retained_bytes(), 3 * MAX_FRAME_BYTES);
    assert!(bridge.retained_frame_bytes() <= bridge.worst_case_retained_bytes());

    // Fill all three sources with real rasters; still under the worst case by construction.
    assert_eq!(
        bridge.publish_frame(frame_wh(0, 1, 8, 8)),
        FramePublish::Stored
    );
    assert_eq!(
        bridge.publish_frame(frame_wh(1, 1, 8, 8)),
        FramePublish::Stored
    );
    assert_eq!(
        bridge.publish_frame(frame_wh(2, 1, 8, 8)),
        FramePublish::Stored
    );
    assert_eq!(bridge.retained_frame_bytes(), 3 * (8 * 8 * 3));
    assert!(bridge.retained_frame_bytes() <= bridge.worst_case_retained_bytes());

    // A fourth distinct source cannot push past the bound — it's refused, not grown.
    assert_eq!(
        bridge.publish_frame(frame_wh(3, 1, 8, 8)),
        FramePublish::SourceCapReached { capacity: 3 }
    );
    assert!(bridge.retained_frame_bytes() <= bridge.worst_case_retained_bytes());
}

#[test]
fn acknowledged_is_distinct_from_delivered_and_clamped() {
    // RED ON OLD CODE: there was no `acknowledge_delivered` and no `acked` counter — the
    // downstream "delivered vs acknowledged" contract had no surface. You cannot ack what
    // was never delivered; the ack count clamps to delivered.
    let mut bridge: SnapshotBridge<Cmd> = SnapshotBridge::new();
    assert_eq!(bridge.issue(Cmd::Edit(0)), Admission::Accepted);
    assert_eq!(bridge.issue(Cmd::Edit(1)), Admission::Accepted);
    let _ = bridge.drain_commands(); // delivered == 2

    // Over-acking is clamped: only 2 were delivered, so only 2 can be acknowledged.
    assert_eq!(bridge.acknowledge_delivered(5), 2);
    assert_eq!(bridge.commands_acked(), 2);
    assert_eq!(bridge.commands_delivered(), 2);
    assert!(bridge.commands_acked() <= bridge.commands_delivered());
    assert!(bridge.commands_conserved());

    // A further ack takes nothing — there is no undelivered command to acknowledge.
    assert_eq!(bridge.acknowledge_delivered(1), 0);
    assert_eq!(bridge.commands_acked(), 2);
}
