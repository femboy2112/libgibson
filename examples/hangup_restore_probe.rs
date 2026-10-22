//! Child probe for defect F (issue #131): a consumer that handles a terminal
//! hangup *gracefully* — by driving [`gibson::Context::restore`] on
//! `BrokenPipe` rather than hard-exiting — must exercise a restore path that
//! runs to completion without panicking or hanging, even though the terminal
//! is already gone.
//!
//! This is the deliberate contrast to `hangup_probe` (issue #79), which calls
//! `std::process::exit(3)` and so *skips every destructor*: the restore path
//! never runs on that path. Here we model the pattern a real application should
//! use, so raw mode is torn down on the way out whenever the OS still permits
//! it, and the best-effort failure is reported rather than swallowed.
//!
//! Because a hangup severs the PTY, anything written to stdout/stderr after it
//! vanishes into `/dev/null`. So the probe records its evidence to a regular
//! file named by the `GIBSON_HANGUP_LOG` environment variable — a channel that
//! outlives the terminal. The host test reads that file after the child exits.
//!
//! Evidence (one `KEY=VALUE` line each, flushed immediately):
//!   * `A_LFLAG=<hex>`   slave `c_lflag` before raw mode (canonical).
//!   * `B_LFLAG=<hex>`   slave `c_lflag` after the first poll enables raw mode.
//!   * `BROKENPIPE_SEEN` the hangup surfaced as `ErrorKind::BrokenPipe`.
//!   * `RESTORE_RESULT=<Ok|Err:kind>` the result of `restore()` on the dead
//!     terminal — informational: a `tcsetattr` on a hung-up slave may fail with
//!     `EIO`, which is honest best-effort cleanup, not a defect.
//!   * `C_LFLAG=<hex|ERR:kind>` slave `c_lflag` after restore — informational.
//!   * `RESTORE_DONE`    restore returned without panicking or hanging.
//!
//! It prints `READY` to stdout after three quiet polls so the host can settle
//! the child mid-wait before delivering the hangup, exactly like `hangup_probe`.

use std::io::Write;
use std::time::Duration;

#[cfg(unix)]
fn lflag() -> std::io::Result<libc::tcflag_t> {
    // SAFETY: tcgetattr fills a fully owned, zeroed termios from stdin (the PTY
    // slave). On a hung-up terminal it fails; we surface that as an error.
    let mut termios: libc::termios = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::tcgetattr(0, &mut termios) };
    if rc == 0 {
        Ok(termios.c_lflag)
    } else {
        Err(std::io::Error::last_os_error())
    }
}

fn main() {
    #[cfg(unix)]
    {
        // Ignore SIGHUP so the hangup surfaces through `run_once` as BrokenPipe
        // (the graceful path under test) instead of terminating us by default.
        // SAFETY: installing SIG_IGN for SIGHUP is a simple signal-setup call.
        unsafe {
            libc::signal(libc::SIGHUP, libc::SIG_IGN);
        }

        let log_path =
            std::env::var("GIBSON_HANGUP_LOG").expect("GIBSON_HANGUP_LOG must name the log file");
        let mut log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .expect("open hangup evidence log");
        let mut record = |line: &str| {
            let _ = writeln!(log, "{line}");
            let _ = log.flush();
        };

        macro_rules! lflag_line {
            ($key:expr) => {
                match lflag() {
                    Ok(f) => format!("{}={:#x}", $key, f),
                    Err(e) => format!("{}=ERR:{:?}", $key, e.kind()),
                }
            };
        }

        let mut ctx = gibson::Context::new(gibson::RenderMode::Inline).expect("create Context");
        // Raw mode is lazy — it engages on the first poll — so the slave is still
        // canonical here. This is the pre-raw control for the restore claim.
        record(&lflag_line!("A_LFLAG"));

        let mut settled = 0u32;
        loop {
            match ctx.run_once(Duration::from_millis(100)) {
                Ok(_) => {
                    settled += 1;
                    if settled == 1 {
                        // The first poll entered interactive mode (raw on).
                        record(&lflag_line!("B_LFLAG"));
                    }
                    if settled == 3 {
                        println!("READY");
                        let _ = std::io::stdout().flush();
                    }
                }
                // The graceful path: a hangup is a `BrokenPipe`, not a reason to
                // skip cleanup. Restore, report, and only then exit.
                Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                    record("BROKENPIPE_SEEN");
                    let result = ctx.restore();
                    record(&format!(
                        "RESTORE_RESULT={}",
                        match &result {
                            Ok(()) => "Ok".to_string(),
                            Err(e) => format!("Err:{:?}", e.kind()),
                        }
                    ));
                    record(&lflag_line!("C_LFLAG"));
                    record("RESTORE_DONE");
                    std::process::exit(3);
                }
                Err(_) => std::process::exit(4),
            }
        }
    }

    #[cfg(not(unix))]
    {
        eprintln!("hangup_restore_probe is Unix-only");
        std::process::exit(2);
    }
}
