//! Deterministic *between-polls* hangup falsifier for issue #79 — the terminal
//! identity-vs-liveness race.
//!
//! The sibling `hangup_probe` parks inside `poll_event` and hangs up mid-wait, where
//! the per-slice `POLLHUP` check catches it. The nastier case is a hangup that lands
//! *between* two polls: the next `poll_event` re-probes `isatty` (now false, because
//! the master is gone) and — before the identity latch — demoted to crossterm's
//! spin-on-a-hung-fd path, hanging forever. This probe forces that exact ordering
//! deterministically, with no reliance on timing luck.
//!
//! argv: `hangup_between_polls_probe <release-barrier-path> <result-path>`.
//!
//! Protocol:
//!   1. Ignore SIGHUP (the bug's precondition: a process that survives the hangup).
//!   2. Settle three quiet polls on the live terminal, then print `READY`.
//!   3. Print `ARMED` and block on a filesystem barrier — parked *outside*
//!      `poll_event`. The test closes the PTY master (the hangup) and only then
//!      creates the barrier file, so the hangup strictly precedes the next poll.
//!   4. Record `isatty(stdin)` to the result file (a side channel; stdout is dead
//!      once the master is closed) so the test can prove the demotion condition was
//!      genuinely present — if `isatty` were still true the falsifier is vacuous.
//!   5. Re-enter the loop: the next `run_once` is the between-polls poll. On the
//!      fixed library it returns `BrokenPipe` and the probe exits `3`; the old
//!      re-probe-every-call dispatcher would spin here forever.

use std::io::Write;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let release_path = args.get(1).expect("argv[1]: release barrier path").clone();
    let result_path = args.get(2).expect("argv[2]: result path").clone();

    #[cfg(unix)]
    {
        // SAFETY: installing SIG_IGN for SIGHUP is a simple signal-setup call.
        unsafe {
            libc::signal(libc::SIGHUP, libc::SIG_IGN);
        }
    }

    let mut ctx = gibson::Context::new(gibson::RenderMode::Inline).expect("create Context");
    let mut settled = 0u32;
    let mut armed = false;
    loop {
        match ctx.run_once(Duration::from_millis(100)) {
            Ok(_) => {
                settled += 1;
                if settled == 3 {
                    println!("READY");
                    let _ = std::io::stdout().flush();
                }
                if settled >= 3 && !armed {
                    armed = true;
                    // Announce we are between polls, parked off the input path.
                    println!("ARMED");
                    let _ = std::io::stdout().flush();
                    // Deterministic barrier: block here (NOT inside poll_event) until
                    // the test has closed the master AND created the release file.
                    wait_for_file(&release_path);
                    // The master is now closed. Record the liveness probe to the side
                    // channel: isatty should report non-tty, yet the next poll must
                    // still surface the hangup. (stdout is broken now, so this cannot
                    // go over the PTY.)
                    #[cfg(unix)]
                    let isatty = unsafe { libc::isatty(libc::STDIN_FILENO) };
                    #[cfg(not(unix))]
                    let isatty = -1;
                    let _ = std::fs::write(&result_path, format!("isatty={isatty}\n"));
                    // Fall through: the next run_once is the between-polls poll.
                }
            }
            // The issue #79 contract: a hangup surfaces as an error, not a spin.
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => std::process::exit(3),
            Err(_) => std::process::exit(4),
        }
    }
}

/// Blocks until `path` exists, polling cheaply. The barrier is created by the test
/// only after it has delivered the hangup, which fixes the between-polls ordering.
fn wait_for_file(path: &str) {
    let p = std::path::Path::new(path);
    loop {
        if p.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
