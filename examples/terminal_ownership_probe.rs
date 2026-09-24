//! Child fixture for `tests/terminal_ownership.rs`. Not a demo — a patient on
//! the table. A real `is_tty=true` needs a real terminal, so the ownership
//! contract of issue #11 can only be exercised from a process whose stdout is a
//! PTY slave. Each scenario runs one incision and exits with a marker the parent
//! reads off the wire.

use gibson::{TerminalLease, TerminalSession};
use std::io::Write;

fn main() {
    let scenario = std::env::args().nth(1).unwrap_or_default();
    match scenario.as_str() {
        "second-owner" => second_owner(),
        "drop-reacquire" => drop_reacquire(),
        "panic-when-owned" => panic_when_owned(),
        other => {
            eprintln!("unknown terminal-ownership scenario: {other:?}");
            std::process::exit(2);
        }
    }
}

/// First session takes the lease; a second construction must be rejected with
/// `AlreadyExists` rather than fighting over the same fd.
fn second_owner() {
    let first = TerminalSession::new().expect("first session must acquire the terminal");
    assert_eq!(TerminalSession::lease_state(), TerminalLease::Owned);

    match TerminalSession::new() {
        Ok(_) => {
            eprintln!("second session unexpectedly acquired the terminal");
            std::process::exit(1);
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            println!("SECOND_OWNER_REJECTED");
            let _ = std::io::stdout().flush();
        }
        Err(err) => {
            eprintln!("second session failed with the wrong error kind: {err:?}");
            std::process::exit(1);
        }
    }

    // Release explicitly so the marker is on the wire before the reset blob.
    drop(first);
    std::process::exit(0);
}

/// Dropping the owner returns the lease to the pool; a fresh session reacquires.
fn drop_reacquire() {
    {
        let first = TerminalSession::new().expect("first session must acquire the terminal");
        assert_eq!(TerminalSession::lease_state(), TerminalLease::Owned);
        drop(first);
    }
    assert_eq!(
        TerminalSession::lease_state(),
        TerminalLease::Available,
        "drop did not release the lease"
    );

    let second = TerminalSession::new().expect("second session must reacquire after drop");
    assert_eq!(TerminalSession::lease_state(), TerminalLease::Owned);
    println!("REACQUIRED");
    let _ = std::io::stdout().flush();

    drop(second);
    std::process::exit(0);
}

/// A session owns the terminal, does real terminal ops, then panics. The panic
/// hook must best-effort restore and the process must exit rather than hang.
fn panic_when_owned() {
    let mut session = TerminalSession::new().expect("session must acquire the terminal");
    session.enter_interactive().expect("enter interactive");
    session
        .enter_alternate_screen()
        .expect("enter alternate screen");
    let _ = session.hide_cursor();

    println!("OWNED_BEFORE_PANIC");
    let _ = std::io::stdout().flush();

    panic!("intentional panic while owning the terminal");
}
