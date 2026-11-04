//! Gate B1 (Renaissance XI): a terminal transport state must preempt input at the UI boundary.
//!
//! The interactive loop drains the transport, may latch a terminal condition (control-lane
//! overflow, fatal decode, or disconnect) into `disconnected`, and then handles the keystroke the
//! same iteration produced. The defect: that keystroke was handled unconditionally — an Enter
//! coincident with the fatal condition still admitted a new command (`issue_tracked`) and wrote
//! its bytes to the producer (`deliver_next`), and overwrote the fatal status the final frame is
//! supposed to display. The contract this proves:
//!
//! ```text
//! fatal transport  >  disconnect  >  command submission  >  routine UI input
//! ```
//!
//! The proof is PTY-free and deterministic: the real `reduce_key` decides, and the real
//! `CommandChannel` issues/delivers into an in-memory `Vec<u8>` whose length and the ledger's
//! `issued()`/`outstanding()` counters are the observable "no command admitted / no bytes written"
//! evidence. `reduce_key` is the single decision shared with the loop; `drive_one_key` mirrors the
//! loop's dispatch (submit iff the decision is `SubmitEdit`), so what it exercises is exactly what
//! the loop does with that same decision.

#[path = "../examples/wire_instrument/channel.rs"]
mod channel;
#[path = "../examples/wire_instrument/input.rs"]
mod input;

use channel::CommandChannel;
use gibson::input::KeyCode;
use input::{reduce_key, InputAction};

const KIND_SET_AMPLITUDE: u32 = 1;

/// One iteration of the loop's input handling: reduce the key with the real `reduce_key`, and do
/// the command I/O exactly as the loop does — issue + deliver — but ONLY when the decision is
/// `SubmitEdit`. `status` is overwritten only on a submit, exactly as the loop overwrites it, so a
/// suppressed submit preserves whatever fatal reason the caller set. Returns whether the loop quits.
fn drive_one_key(
    code: KeyCode,
    disconnected: bool,
    probe: &mut (usize, usize),
    ppc: &mut u16,
    pending_amp: &mut f64,
    ch: &mut CommandChannel,
    writer: &mut Vec<u8>,
    status: &mut String,
) -> bool {
    match reduce_key(code, disconnected, probe, ppc, pending_amp) {
        InputAction::Quit => return true,
        InputAction::SubmitEdit => {
            if let Ok(id) = ch.issue_tracked(KIND_SET_AMPLITUDE, pending_amp.to_le_bytes().to_vec())
            {
                let _ = ch.deliver_next(writer);
                *status = format!("sent set-amplitude {:.1} (id {id})", *pending_amp);
            }
        }
        InputAction::Nothing => {}
    }
    false
}

/// RED-FIRST: an Enter in the same UI iteration a terminal transport state is latched must admit no
/// command, write no bytes, and leave the fatal status intact. Against the baseline reducer (which
/// ignores `disconnected`) this is RED — the edit is submitted, bytes flow, and the fatal reason is
/// overwritten with "sent set-amplitude …". The precedence guard turns it GREEN.
#[test]
fn an_enter_coincident_with_a_terminal_transport_state_admits_no_command_and_writes_no_bytes() {
    let mut ch = CommandChannel::new(7, 1);
    let mut writer: Vec<u8> = Vec::new();
    let mut probe = (0usize, 0usize);
    let mut ppc = 1u16;
    let mut pending_amp = 3.0f64;
    // The loop set this when it latched the terminal condition (control-lane overflow).
    let fatal = "transport fatal: control lane overflow (cap 8, Control) — command fate lost";
    let mut status = fatal.to_string();

    let quit = drive_one_key(
        KeyCode::Enter,
        true, // disconnected: a terminal transport state is latched
        &mut probe,
        &mut ppc,
        &mut pending_amp,
        &mut ch,
        &mut writer,
        &mut status,
    );

    assert!(!quit, "Enter is not a quit");
    assert_eq!(
        ch.issued(),
        0,
        "no command may be admitted once the transport is in a terminal state"
    );
    assert_eq!(
        ch.outstanding(),
        0,
        "nothing outstanding — nothing was issued"
    );
    assert!(
        writer.is_empty(),
        "no command bytes may be written to the producer after a fatal transport condition"
    );
    assert_eq!(
        status, fatal,
        "the fatal reason must retain display precedence — an edit must not overwrite it"
    );
}

/// POSITIVE CONTROL: the guard is not vacuous. On a LIVE transport the very same Enter submits the
/// edit — the ledger admits it and real bytes reach the writer — so the red-first test is proving a
/// suppression that genuinely would have happened, not a dead branch.
#[test]
fn an_enter_on_a_live_transport_submits_the_edit_and_writes_bytes() {
    let mut ch = CommandChannel::new(7, 1);
    let mut writer: Vec<u8> = Vec::new();
    let mut probe = (0usize, 0usize);
    let mut ppc = 1u16;
    let mut pending_amp = 3.0f64;
    let mut status = String::from("connected — streaming…");

    let quit = drive_one_key(
        KeyCode::Enter,
        false, // live transport
        &mut probe,
        &mut ppc,
        &mut pending_amp,
        &mut ch,
        &mut writer,
        &mut status,
    );

    assert!(!quit);
    assert_eq!(ch.issued(), 1, "a live Enter admits exactly one command");
    assert!(
        !writer.is_empty(),
        "a live Enter writes the command's bytes to the producer"
    );
    assert!(
        status.starts_with("sent set-amplitude"),
        "a live submit updates the status: {status:?}"
    );
}

/// A prior outstanding command's fate must remain exactly as it was — unresolved — when a terminal
/// state is latched; the guard must neither resolve it, re-admit it, nor corrupt its accounting.
#[test]
fn a_latched_terminal_state_leaves_a_prior_outstanding_command_unresolved() {
    let mut ch = CommandChannel::new(7, 1);
    let mut writer: Vec<u8> = Vec::new();
    let mut probe = (0usize, 0usize);
    let mut ppc = 1u16;
    let mut pending_amp = 2.0f64;
    let mut status = String::from("connected — streaming…");

    // A real edit issued + delivered on the live transport, now awaiting its ACK.
    drive_one_key(
        KeyCode::Enter,
        false,
        &mut probe,
        &mut ppc,
        &mut pending_amp,
        &mut ch,
        &mut writer,
        &mut status,
    );
    assert_eq!(ch.issued(), 1);
    assert_eq!(ch.outstanding(), 1, "the delivered edit awaits its verdict");
    let bytes_before = writer.len();

    // Now the transport goes terminal and the user mashes Enter again.
    let fatal = "transport fatal: control lane overflow — command fate lost";
    status = fatal.to_string();
    drive_one_key(
        KeyCode::Enter,
        true,
        &mut probe,
        &mut ppc,
        &mut pending_amp,
        &mut ch,
        &mut writer,
        &mut status,
    );

    assert_eq!(ch.issued(), 1, "no second command may be admitted");
    assert_eq!(
        ch.outstanding(),
        1,
        "the prior command stays outstanding — unresolved, not faked resolved and not dropped"
    );
    assert_eq!(writer.len(), bytes_before, "no further bytes written");
    assert_eq!(status, fatal, "the fatal reason still stands");
}

/// Routine navigation is the lowest-priority input: a latched terminal state suppresses it, while a
/// live transport applies it. Both directions proven so the suppression is not a dead reducer.
#[test]
fn routine_navigation_is_suppressed_under_a_terminal_state_but_applies_when_live() {
    // Live: Right/Down move the probe, '+' zooms in.
    let mut probe = (2usize, 2usize);
    let mut ppc = 1u16;
    let mut amp = 2.0f64;
    assert_eq!(
        reduce_key(KeyCode::Right, false, &mut probe, &mut ppc, &mut amp),
        InputAction::Nothing
    );
    assert_eq!(
        reduce_key(KeyCode::Char('+'), false, &mut probe, &mut ppc, &mut amp),
        InputAction::Nothing
    );
    assert_eq!(probe.0, 3, "live Right advances the probe");
    assert_eq!(ppc, 2, "live '+' zooms in");

    // Terminal: the same keys change nothing.
    let mut probe = (2usize, 2usize);
    let mut ppc = 1u16;
    let mut amp = 2.0f64;
    assert_eq!(
        reduce_key(KeyCode::Right, true, &mut probe, &mut ppc, &mut amp),
        InputAction::Nothing
    );
    assert_eq!(
        reduce_key(KeyCode::Char('+'), true, &mut probe, &mut ppc, &mut amp),
        InputAction::Nothing
    );
    assert_eq!(
        probe,
        (2, 2),
        "navigation is suppressed under a terminal state"
    );
    assert_eq!(ppc, 1, "zoom is suppressed under a terminal state");
}

/// Quit outranks even a terminal transport state: the user must always be able to exit and the
/// shutdown path must still complete. Both quit keys, live and terminal, resolve to `Quit`.
#[test]
fn quit_is_honored_even_under_a_terminal_transport_state() {
    let mut probe = (0usize, 0usize);
    let mut ppc = 1u16;
    let mut amp = 2.0f64;
    for disconnected in [false, true] {
        assert_eq!(
            reduce_key(
                KeyCode::Char('q'),
                disconnected,
                &mut probe,
                &mut ppc,
                &mut amp
            ),
            InputAction::Quit,
            "q must quit (disconnected={disconnected})"
        );
        assert_eq!(
            reduce_key(KeyCode::Esc, disconnected, &mut probe, &mut ppc, &mut amp),
            InputAction::Quit,
            "Esc must quit (disconnected={disconnected})"
        );
    }
}
