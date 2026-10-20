//! Child probe for issue #79: a terminal hangup (SIGHUP) must make
//! `Context::run_once` **return** (as `ErrorKind::BrokenPipe`) rather than
//! busy-spin forever.
//!
//! argv: `hangup_probe [ignore-sighup]`. With `ignore-sighup` the process ignores
//! SIGHUP so it survives the hangup — the exact condition from the bug report (a
//! process that handled/ignored SIGHUP). Without it, SIGHUP takes its default
//! action (termination); the test harness uses that path to prove the hangup is
//! really delivered.
//!
//! It sets no root, so `run_once` only polls input and paces frames — there is no
//! render/write path to race the hangup. On the fixed library it prints `READY`
//! after three quiet polls, then exits `3` the moment the hangup is observed.

use std::io::Write;
use std::time::Duration;

fn main() {
    #[cfg(unix)]
    {
        if std::env::args().any(|a| a == "ignore-sighup") {
            // SAFETY: installing SIG_IGN for SIGHUP is a simple signal-setup call.
            unsafe {
                libc::signal(libc::SIGHUP, libc::SIG_IGN);
            }
        }
    }

    let mut ctx = gibson::Context::new(gibson::RenderMode::Inline).expect("create Context");
    let mut settled = 0u32;
    loop {
        match ctx.run_once(Duration::from_millis(100)) {
            Ok(_) => {
                settled += 1;
                if settled == 3 {
                    println!("READY");
                    let _ = std::io::stdout().flush();
                }
            }
            // The issue #79 fix: a hangup surfaces as an error instead of a spin.
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => std::process::exit(3),
            Err(_) => std::process::exit(4),
        }
    }
}
