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
