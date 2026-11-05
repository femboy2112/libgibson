//! Child probe for the ffi_lifecycle terminal-lease race (Gate K · K4).
//!
//! This binary is spawned by `tests/ffi_lifecycle_lease_pty.rs` onto a PTY
//! slave, so `stdout().is_terminal()` is true and `gibson_create_context`
//! actually contends the process-global terminal lease (`TERMINAL_LEASE` in
//! `src/session.rs`). That contention is invisible to CI, whose stdout is a pipe
//! (no lease is ever taken), which is precisely why the lifecycle tests could
//! race on a developer's real terminal without anyone noticing.
//!
//! Two scenarios, selected by argv[1], establish the hazard and the fix
//! MECHANISM on a real controlling terminal:
//!
//! * `race` — two threads take the lease with NO serialization; one holds it
//!   while the other attempts a second create, which MUST be refused
//!   (`AlreadyExists` -> `ErrIo`). This is the hazard CI's pipe hides.
//! * `gated` — several threads take the lease behind a shared serialization
//!   gate; whichever runs second finds the lease free, so no collision occurs.
//!
//! The library behavior is correct and unchanged either way: a single terminal
//! cannot have two owners (locked by `tests/terminal_ownership.rs`). K4 only
//! fixes the TEST hygiene that let concurrent lease-takers collide.

use gibson::ffi::*;
use std::env;
use std::io::Write;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;

fn last_error() -> String {
    let mut buf = [0i8; 256];
    let n = unsafe { gibson_last_error_message(buf.as_mut_ptr(), buf.len()) };
    if n <= 0 {
        return String::new();
    }
    let bytes: Vec<u8> = buf[..(n as usize).min(buf.len())]
        .iter()
        .map(|&b| b as u8)
        .collect();
    String::from_utf8_lossy(&bytes)
        .trim_end_matches('\0')
        .to_string()
}

/// Deterministically force contention. Thread A takes the lease and parks while
/// holding it; the main thread then attempts a second create, which on a real
/// TTY must be refused. No sleeps, no probability — the barrier pins the order.
fn race() {
    let barrier = Arc::new(Barrier::new(2));
    let b2 = barrier.clone();
    let a = thread::spawn(move || unsafe {
        let mut ctx = ptr::null_mut();
        let s = gibson_create_context(GIBSON_MODE_INLINE, &mut ctx);
        println!("A_CREATE status={}", s as i32);
        b2.wait(); // r1: A holds the lease; main may now attempt its create
        b2.wait(); // r2: main has finished attempting; A may release
        gibson_destroy_context(ctx);
        println!("A_DONE");
    });

    barrier.wait(); // r1: proceed only once A holds the lease
    unsafe {
        let mut ctx2 = ptr::null_mut();
        let s = gibson_create_context(GIBSON_MODE_INLINE, &mut ctx2);
        if s == GibsonStatus::Ok {
            // Would mean two owners coexisted — the contract is broken.
            println!("SECOND_UNEXPECTEDLY_OK");
            gibson_destroy_context(ctx2);
        } else {
            println!("SECOND_REJECTED status={} err={}", s as i32, last_error());
        }
    }
    barrier.wait(); // r2: let A release and exit

    a.join().expect("race thread A panicked");
    println!("RACE_SCENARIO_DONE");
    let _ = std::io::stdout().flush();
}

/// Serialize lease acquisition behind a shared gate (the exact mechanism K4
/// applies to the real lifecycle tests). Four threads contend the gate; whoever
/// runs second finds the lease free. A collision here would be a fix failure.
fn gated() {
    static GATE: Mutex<()> = Mutex::new(());
    let collisions = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();
    for i in 0..4 {
        let c = collisions.clone();
        handles.push(thread::spawn(move || unsafe {
            let _gate = GATE.lock().unwrap_or_else(|p| p.into_inner());
            let mut ctx = ptr::null_mut();
            let s = gibson_create_context(GIBSON_MODE_INLINE, &mut ctx);
            if s != GibsonStatus::Ok {
                c.fetch_add(1, Ordering::SeqCst);
                println!("GATED_COLLISION thread={} status={}", i, s as i32);
            } else {
                gibson_destroy_context(ctx);
            }
            // gate released here, before the next thread creates
        }));
    }
    for h in handles {
        h.join().expect("gated worker panicked");
    }
    println!("GATED_COLLISIONS {}", collisions.load(Ordering::SeqCst));
    println!("GATED_SCENARIO_DONE");
    let _ = std::io::stdout().flush();
}

fn main() {
    match env::args().nth(1).as_deref() {
        Some("race") => race(),
        Some("gated") => gated(),
        other => {
            println!("UNKNOWN_SCENARIO {other:?}");
            let _ = std::io::stdout().flush();
            std::process::exit(2);
        }
    }
}
