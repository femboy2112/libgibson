// Shared by the example (`main.rs`) and by `tests/wire_instrument_input_precedence.rs` via
// `#[path]`; like the sibling `viewport`/`channel`/`transport` modules it depends only on
// `gibson::` and `std`, so it stays independently includable. Quiet dead_code for the module.
#![allow(dead_code)]
//! Input-state reduction for the `wire_instrument` interactive loop (Renaissance XI, Gate B1).
//!
//! The interactive loop drains the transport, may latch a TERMINAL transport state
//! (control-lane overflow / fatal decode / disconnect), and only then handles the keystroke that
//! the same UI iteration returned. The defect this module isolates: that keystroke was handled
//! unconditionally, so an Enter coincident with a fatal condition still admitted a new command and
//! wrote its bytes to the producer, and overwrote the fatal status text the final frame is meant
//! to show — an edit silently overriding the reason for shutdown.
//!
//! The fix makes the precedence contract explicit and, crucially, TESTABLE without PTY timing by
//! separating the *decision* (this reducer) from the *I/O and rendering* (the loop):
//!
//! ```text
//! fatal transport  >  disconnect  >  command submission  >  routine UI input
//! ```

use gibson::input::KeyCode;

/// Whether `source` matches the instrument's CONFIGURED destination `dest` (Gate B3 / A1).
///
/// The `wire_instrument` example drives exactly ONE producer, chosen by configuration
/// (`--source`), and its `CommandChannel` addresses that same configured source. A field / ack /
/// retire from any OTHER source is out of contract — refused (never admitted to the inbox, never
/// displayed, never resolving a command) and counted by the loop, so a second source can neither be
/// silently stored-and-hidden nor silently switched to, and a command is never redirected. This is a
/// deliberate single-source contract; the refusal is what keeps the single-source identity truthful.
/// When the configured source retires it is shown as retired — the display is never reassigned to a
/// live stranger.
pub fn source_in_contract(dest: u64, source: u64) -> bool {
    source == dest
}

/// How an inbound message relates to the instrument's CONFIGURED source `dest` and the producer
/// epoch observed so far (A1).
///
/// `msg_epoch` and `observed_epoch` live in the PRODUCER's epoch space (its run/restart
/// generation), which is distinct from the presenter's command-stamp epoch. Source identity is the
/// authority here; epoch is only a restart *signal*, never a command gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceVerdict {
    /// The configured source at the epoch already being displayed (or the very first observation,
    /// or a stale lower epoch the inbox will drop): admit / display / resolve normally.
    InSession,
    /// The configured source at a STRICTLY HIGHER epoch than observed so far — a producer restart.
    /// The field is still this source's latest (`FieldInbox` `(epoch, sequence)` latest-wins shows
    /// it, so a restart is NOT fatal). It is surfaced as a restart, but acks still correlate by
    /// `command_id` (not epoch), and pre-restart commands may simply remain outstanding.
    Restart,
    /// A different source entirely: out of contract for this single-source instrument. Refuse —
    /// never admit, never display, never resolve a command — and count it.
    Foreign,
}

/// Classify an inbound message against the configured source `dest` and the highest producer epoch
/// observed so far (`observed_epoch`, `None` before the first in-contract observation).
///
/// This is the SINGLE authority over source identity, shared by the interactive loop and its tests.
/// The display binds to `dest`, never to the first source observed — so the displayed source can
/// never disagree with the command destination (the A1 defect: a producer publishing source B first
/// made the display show B while every command stayed addressed to the configured source A). The
/// epoch dimension only distinguishes a producer restart (strictly higher than `observed_epoch`)
/// for display/status; it never gates command resolution.
pub fn classify_source(
    dest: u64,
    observed_epoch: Option<u32>,
    msg_source: u64,
    msg_epoch: u32,
) -> SourceVerdict {
    if !source_in_contract(dest, msg_source) {
        SourceVerdict::Foreign
    } else if matches!(observed_epoch, Some(seen) if msg_epoch > seen) {
        SourceVerdict::Restart
    } else {
        SourceVerdict::InSession
    }
}

/// What a single key press asks the loop to do, after routine state mutation (probe/zoom/pending
/// amplitude) has been applied in place. The loop performs the I/O; this type carries only the
/// *decision*, so the precedence contract can be proven deterministically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAction {
    /// Quit the loop.
    Quit,
    /// Submit the pending edit as a tracked COMMAND (issue + deliver on the producer's stdin).
    SubmitEdit,
    /// Nothing further for the loop to do (routine navigation handled, or input suppressed).
    Nothing,
}

/// Reduce one key press to an [`InputAction`], applying routine state mutation in place. `ppc` is
/// clamped to `[1, 4]`, `pending_amp` to `[0, 12]`; `probe` saturates at 0 on the low side (the
/// loop clamps the high side against the live field).
pub fn reduce_key(
    code: KeyCode,
    disconnected: bool,
    probe: &mut (usize, usize),
    ppc: &mut u16,
    pending_amp: &mut f64,
) -> InputAction {
    // Quit outranks everything, including a latched terminal state: the user must always be able
    // to exit, and the loop tears down on quit regardless.
    if matches!(code, KeyCode::Char('q') | KeyCode::Esc) {
        return InputAction::Quit;
    }
    // Gate B1 precedence: `fatal transport > disconnect > command submission > routine UI input`.
    // Once a terminal transport state is latched, no edit is admitted and no navigation is applied,
    // so input cannot override the terminal condition and the fatal status the loop set survives
    // into the final frame.
    if disconnected {
        return InputAction::Nothing;
    }
    match code {
        KeyCode::Left => probe.0 = probe.0.saturating_sub(1),
        KeyCode::Right => probe.0 += 1,
        KeyCode::Up => probe.1 = probe.1.saturating_sub(1),
        KeyCode::Down => probe.1 += 1,
        KeyCode::Char('+') | KeyCode::Char('=') => *ppc = (*ppc + 1).min(4),
        KeyCode::Char('-') => *ppc = ppc.saturating_sub(1).max(1),
        KeyCode::Char('[') => *pending_amp = (*pending_amp - 0.5).max(0.0),
        KeyCode::Char(']') => *pending_amp = (*pending_amp + 0.5).min(12.0),
        KeyCode::Enter => return InputAction::SubmitEdit,
        _ => {}
    }
    InputAction::Nothing
}
