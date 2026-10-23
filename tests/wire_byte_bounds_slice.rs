//! Wave E / Finding A refinement — the wire return channel enforces a per-command BYTE
//! limit, even on locally-constructed commands that bypass the decoder.
//!
//! `SnapshotBridge` bounds the command queue by COUNT; a `WireCommand` built in-process never
//! passes through the decoder's payload cap, so before this a single local command could pin an
//! arbitrarily large `arg`. `wire::issue_command` / `wire::encode_command_checked` close that
//! gap: an oversized `arg` is refused before it is queued or encoded, so the worst-case retained
//! bytes on this path are bounded (`max_pending_commands × max_command_arg_bytes`), not
//! unbounded in the generic payload.

use gibson::snapshot::{Admission, SnapshotBridge};
use gibson::wire::{self, SourceCursor, WireCommand, WireError, WireLimits};

fn cmd(id: u64, arg_len: usize) -> WireCommand {
    WireCommand {
        command_id: id,
        kind: 1,
        arg: vec![0u8; arg_len],
    }
}

#[test]
fn max_command_arg_bytes_leaves_room_for_the_header() {
    let limits = WireLimits::new(1000, 1 << 20);
    // The fixed command header eats 16 bytes of the payload budget.
    assert_eq!(limits.max_command_arg_bytes(), 1000 - 16);
}

#[test]
fn encode_command_checked_refuses_an_oversized_arg_but_accepts_the_limit() {
    let limits = WireLimits::new(1000, 1 << 20);
    let cap = limits.max_command_arg_bytes();
    let cur = SourceCursor {
        epoch: 1,
        sequence: 1,
    };

    // Exactly at the cap encodes; one byte over is refused, never silently emitted.
    assert!(wire::encode_command_checked(7, cur, &cmd(1, cap), limits).is_ok());
    assert_eq!(
        wire::encode_command_checked(7, cur, &cmd(2, cap + 1), limits),
        Err(WireError::EncodeTooLarge)
    );
}

#[test]
fn issue_command_refuses_an_oversized_local_command_before_the_queue() {
    let limits = WireLimits::new(1000, 1 << 20);
    let mut bridge: SnapshotBridge<WireCommand> = SnapshotBridge::new();

    // An in-bounds command is admitted exactly like bridge.issue.
    assert_eq!(
        wire::issue_command(&mut bridge, cmd(1, 10), limits),
        Ok(Admission::Accepted)
    );
    // An oversized LOCAL command is refused before the bridge — it never becomes a pending
    // entry pinning its bytes.
    assert_eq!(
        wire::issue_command(
            &mut bridge,
            cmd(2, limits.max_command_arg_bytes() + 1),
            limits
        ),
        Err(WireError::EncodeTooLarge)
    );
    // The refusal never counted as a queue offer: conservation still holds.
    assert!(bridge.commands_conserved());
}
