//! Child fixture for `tests/terminal_ownership.rs`. Not a demo — a patient on
//! the table. A real `is_tty=true` needs a real terminal, so the ownership
//! contract of issue #11 can only be exercised from a process whose stdout is a
//! PTY slave. Each scenario runs one incision and exits with a marker the parent
//! reads off the wire.

use gibson::{TerminalLease, TerminalSession};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

fn main() {
    let scenario = std::env::args().nth(1).unwrap_or_default();
    match scenario.as_str() {
        "second-owner" => second_owner(),
        "drop-reacquire" => drop_reacquire(),
        "panic-when-owned" => panic_when_owned(),
        "worker-panic" => worker_panic(),
        "restore-output-failure" => restore_output_failure(),
        "host-hook-chain" => host_hook_chain(),
        "duel-trace" => duel_trace(),
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

/// Runtime Observatory's Ownership-Duel fixture source. Two contexts fight
/// over the process-global lease for real: ctx#1 acquires, ctx#2 is rejected
/// while ctx#1 still holds it, ctx#1 restores, ctx#2 reacquires, teardown.
/// Every line printed is `seq\tus\tsource\tkind\tvalue`, same shape the other
/// release-lab fixtures use — this is measured, not staged.
///
/// A background thread spins on `TerminalSession::lease_state()` the whole
/// run and records every state it actually catches (with the real timestamp
/// it caught it at). `restore()` is synchronous on the main thread, so
/// whether that poller ever actually witnesses the transient `Restoring`
/// value is a real race, not a guarantee — whatever it measures is what gets
/// printed, including if it measures nothing.
fn duel_trace() {
    let epoch = Instant::now();
    let mut seq: u64 = 0;
    let emit_at = |seq: &mut u64, us: u64, source: &str, kind: &str, value: &str| {
        println!("{seq}\t{us}\t{source}\t{kind}\t{value}");
        *seq += 1;
    };
    let emit = |seq: &mut u64, source: &str, kind: &str, value: &str| {
        let us = epoch.elapsed().as_micros() as u64;
        println!("{seq}\t{us}\t{source}\t{kind}\t{value}");
        *seq += 1;
    };

    let witness: Arc<Mutex<Vec<(u64, TerminalLease)>>> = Arc::new(Mutex::new(Vec::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let poller = {
        let witness = witness.clone();
        let stop = stop.clone();
        std::thread::spawn(move || {
            let mut last = None;
            while !stop.load(Ordering::Relaxed) {
                let now = TerminalSession::lease_state();
                if Some(now) != last {
                    let us = epoch.elapsed().as_micros() as u64;
                    witness.lock().unwrap().push((us, now));
                    last = Some(now);
                }
            }
        })
    };

    emit(&mut seq, "SESSION", "LeaseState", "start:Available");

    let mut ctx1 = TerminalSession::new().expect("ctx1 must acquire the terminal");
    emit(&mut seq, "SESSION", "Acquire", "ctx1:ok");
    emit(
        &mut seq,
        "SESSION",
        "LeaseState",
        &format!("ctx1:{:?}", TerminalSession::lease_state()),
    );

    match TerminalSession::new() {
        Ok(_) => {
            eprintln!("ctx2 unexpectedly acquired while ctx1 owns the terminal");
            std::process::exit(1);
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            emit(
                &mut seq,
                "SESSION",
                "Acquire",
                "ctx2:rejected:AlreadyExists",
            );
        }
        Err(err) => {
            eprintln!("ctx2 failed with unexpected error kind: {err:?}");
            std::process::exit(1);
        }
    }
    emit(
        &mut seq,
        "SESSION",
        "LeaseState",
        &format!("ctx2-attempt:{:?}", TerminalSession::lease_state()),
    );

    emit(&mut seq, "SESSION", "RestoreBegin", "ctx1");
    ctx1.restore().expect("ctx1 restore must succeed");
    emit(&mut seq, "SESSION", "RestoreResult", "ctx1:ok");
    emit(
        &mut seq,
        "SESSION",
        "LeaseState",
        &format!("post-restore:{:?}", TerminalSession::lease_state()),
    );

    let ctx2 = TerminalSession::new().expect("ctx2 must reacquire after ctx1 restore");
    emit(&mut seq, "SESSION", "Acquire", "ctx2:ok");
    emit(
        &mut seq,
        "SESSION",
        "LeaseState",
        &format!("ctx2:{:?}", TerminalSession::lease_state()),
    );

    drop(ctx2);
    emit(&mut seq, "SESSION", "Teardown", "ctx2");
    emit(
        &mut seq,
        "SESSION",
        "LeaseState",
        &format!("final:{:?}", TerminalSession::lease_state()),
    );

    stop.store(true, Ordering::Relaxed);
    poller.join().expect("witness poller must not panic");
    let samples = witness.lock().unwrap();
    for (us, state) in samples.iter() {
        emit_at(
            &mut seq,
            *us,
            "SESSION",
            "LeaseStateWitness",
            &format!("{state:?}"),
        );
    }

    println!("DUEL_DONE");
    let _ = std::io::stdout().flush();
    std::process::exit(0);
}

/// Issue #11 / B3: a recoverable panic on a NON-owner worker thread must not
/// dismantle the owner thread's live terminal. Main owns the terminal and holds
/// raw mode + alternate screen; a worker thread panics and is joined (recovered,
/// the process continues). The restoration byte sequence (`\x1b[?25h`) must NOT
/// hit the wire until main restores explicitly — its early appearance is the bug.
fn worker_panic() {
    let mut session = TerminalSession::new().expect("main must acquire the terminal");
    session.enter_interactive().expect("enter interactive");
    session
        .enter_alternate_screen()
        .expect("enter alternate screen");
    let _ = session.hide_cursor();

    println!("OWNER_ACTIVE");
    let _ = std::io::stdout().flush();

    // A worker thread panics; joining it recovers (the process continues). The
    // global panic hook runs on the WORKER thread — it must see that the worker
    // does not own the lease and keep its hands off main's terminal.
    let handle = std::thread::spawn(|| {
        panic!("intentional recoverable panic on a non-owner worker thread");
    });
    let joined = handle.join();
    assert!(joined.is_err(), "worker thread was expected to panic");

    // We survived the worker panic with the terminal still ours.
    println!("WORKER_PANIC_RECOVERED");
    let _ = std::io::stdout().flush();

    // Only now do we relinquish the terminal, explicitly.
    println!("OWNER_RESTORING");
    let _ = std::io::stdout().flush();
    session.restore().expect("owner restore must succeed");

    println!("OWNER_RESTORED");
    let _ = std::io::stdout().flush();
    std::process::exit(0);
}

/// Issue #11 / B1: an explicit restore whose terminal WRITES fail must still
/// attempt every cleanup step and report the first error. We own the terminal,
/// enter raw mode + alt screen, then redirect fd 1 (stdout) to `/dev/full` so
/// every write returns ENOSPC, and call `restore()`. Results are reported on fd 2
/// (stderr, still the PTY) since stdout is now a black hole. Raw-mode restoration
/// goes through a termios ioctl, not fd 1, so it must succeed even though the
/// writes fail — proving "every remaining cleanup step is still attempted".
fn restore_output_failure() {
    let mut session = TerminalSession::new().expect("must acquire the terminal");
    session.enter_interactive().expect("enter interactive");
    session
        .enter_alternate_screen()
        .expect("enter alternate screen");
    let _ = session.hide_cursor();

    let raw_before = crossterm::terminal::is_raw_mode_enabled().unwrap_or(false);
    eprintln!("RAW_BEFORE={raw_before}");

    // Redirect stdout (fd 1) to /dev/full: writes now fail with ENOSPC. stderr
    // (fd 2) still points at the PTY, so our markers survive the black hole.
    let devfull = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .expect("open /dev/full");
    let rc = unsafe { libc::dup2(devfull.as_raw_fd(), 1) };
    assert!(rc != -1, "dup2 /dev/full onto stdout failed");

    let result = session.restore();

    let raw_after = crossterm::terminal::is_raw_mode_enabled().unwrap_or(true);
    let lease_after = TerminalSession::lease_state();
    match &result {
        Ok(()) => eprintln!("RESTORE_RESULT=OK"),
        Err(e) => eprintln!("RESTORE_RESULT=ERR kind={:?}", e.kind()),
    }
    eprintln!("RAW_AFTER={raw_after}");
    eprintln!("LEASE_AFTER={lease_after:?}");
    eprintln!("RESTORE_FAILURE_DONE");
    std::process::exit(0);
}

/// Issue #11 / B5: LibGibson installs its panic hook once and CHAINS whatever
/// hook was present at install time. A host hook installed BEFORE the first
/// session must still run on panic, AND LibGibson's terminal restoration must
/// also run. The owner (main) panics, so restoration is in scope.
fn host_hook_chain() {
    // Host installs its own hook FIRST, before any TerminalSession exists.
    let prior = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // On stderr, so ordering vs. the restore blob (stdout) does not matter.
        eprintln!("HOST_HOOK_RAN");
        prior(info);
    }));

    let mut session = TerminalSession::new().expect("acquire");
    session.enter_interactive().expect("enter interactive");
    session
        .enter_alternate_screen()
        .expect("enter alternate screen");
    let _ = session.hide_cursor();
    println!("HOOK_CHAIN_OWNED");
    let _ = std::io::stdout().flush();

    panic!("owner panic to exercise the chained hooks");
}
