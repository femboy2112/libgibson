//! Child probe for issue #269: a terminal hangup must end LibGibson's input poll with
//! `BrokenPipe` at any instant, without a spin or an unbounded block.
//!
//! argv: `tty_hangup_matrix_probe <scenario> <result-file> [options]`
//!
//! After the hangup the terminal can no longer carry output, so every outcome is
//! appended to `<result-file>` as one line, and the exit code says how the poll ended:
//! `3` = `BrokenPipe`, `4` = another error, `42` = the 3 s watchdog fired (the poll
//! never returned: a spin or a block).
//!
//! Scenarios:
//!
//! * `backend-after-hangup --backend gibson|crossterm` — SIGHUP ignored. Arms input on
//!   the live terminal, prints `READY`, then waits (without reading) until `poll(2)`
//!   reports `POLLHUP` on stdin, and only then calls the backend once with a zero
//!   timeout. This is the state a poll is in when the hangup lands **after** a
//!   hangup precheck: the backend meets an already-dead descriptor. `gibson` calls
//!   `gibson::input::poll_event`; `crossterm` calls `crossterm::event::poll` directly,
//!   which is what LibGibson called at that point before #269 was fixed (the control).
//! * `loop --timeout-ms N --signal ignore|handle|default` — polls `Context::poll_event(N)`
//!   in a loop until it errors. `--timeout-ms 0` is a tight loop that spends nearly all
//!   its time inside the poll, where a hangup can land at any instant. `--signal`
//!   chooses what SIGHUP does: ignored, caught by a handler that records it, or the
//!   default action (termination; the probe then never writes a result).
//! * `pending` — SIGHUP ignored. Prints `READY`, then waits (without reading) for the
//!   hangup, then polls until `BrokenPipe`, recording every event delivered first.
//!   The parent writes keys before it hangs up, so they were typed before the hangup.
//! * `decoded-then-hangup` — SIGHUP ignored. Takes the first key of a burst the parent
//!   wrote at once (one read decodes the whole burst), prints `GOT1 <key>`, waits for the
//!   hangup, then polls until `BrokenPipe`, recording the keys delivered first: the rest
//!   of the burst, already decoded when the terminal hung up.
//! * `canonical-eof` — no `Context`, no raw mode: polls `gibson::input::poll_event(100 ms)`
//!   on a canonical-mode terminal while the parent types ^D and, 1.5 s later, `z` + Enter.
//!   Records whether `z` arrived and how long the slowest poll took. A ^D is input, not a
//!   hangup; no poll may block past its timeout because of it.
//! * `keys` — SIGHUP ignored. Records each key to the result file as it arrives, until
//!   `BrokenPipe`. Used with stdin and the controlling terminal on different ptys.

use std::io::Write;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

static SIGHUPS: AtomicU32 = AtomicU32::new(0);

extern "C" fn on_sighup(_: libc::c_int) {
    SIGHUPS.fetch_add(1, Ordering::Relaxed);
}

fn record(path: &str, line: &str) {
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("result file");
    writeln!(f, "{line}").expect("write result");
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn watchdog(path: String) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3));
        record(&path, "WATCHDOG the poll did not return within 3 s");
        std::process::exit(42);
    });
}

/// Waits, without reading stdin, until it reports a hangup. Returns false after 10 s.
fn wait_for_hangup() -> bool {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(10) {
        let mut p = libc::pollfd {
            fd: libc::STDIN_FILENO,
            events: 0,
            revents: 0,
        };
        // SAFETY: one valid pollfd.
        unsafe { libc::poll(&mut p, 1, 20) };
        if p.revents & (libc::POLLHUP | libc::POLLERR) != 0 {
            return true;
        }
    }
    false
}

fn ready() {
    let mut out = std::io::stdout();
    let _ = write!(out, "READY\r\n");
    let _ = out.flush();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scenario = args.get(1).cloned().unwrap_or_default();
    let path = args.get(2).cloned().expect("result file path");
    let signal = arg(&args, "--signal").unwrap_or_else(|| "ignore".into());
    // SAFETY: plain signal-disposition setup before any thread exists.
    unsafe {
        match signal.as_str() {
            "ignore" => {
                libc::signal(libc::SIGHUP, libc::SIG_IGN);
            }
            "handle" => {
                let mut sa: libc::sigaction = std::mem::zeroed();
                sa.sa_sigaction = on_sighup as *const () as usize;
                libc::sigemptyset(&mut sa.sa_mask);
                sa.sa_flags = 0; // no SA_RESTART: a blocked poll sees EINTR
                libc::sigaction(libc::SIGHUP, &sa, std::ptr::null_mut());
            }
            "default" => {}
            other => panic!("--signal {other:?}"),
        }
    }

    if scenario == "canonical-eof" {
        // No Context and no raw mode: the public poll on a canonical-mode terminal. The
        // parent types ^D (EOF on an empty line), waits 1.5 s, then types `z` + Enter.
        // Each poll is bounded at 100 ms; the time the slowest one took is recorded.
        ready();
        let start = Instant::now();
        let mut slowest = Duration::ZERO;
        let mut got_z = false;
        while start.elapsed() < Duration::from_secs(4) && !got_z {
            let t = Instant::now();
            let ev = gibson::input::poll_event(Duration::from_millis(100));
            slowest = slowest.max(t.elapsed());
            match ev {
                Ok(Some(gibson::Event::Key(k))) => got_z = k.code == gibson::KeyCode::Char('z'),
                Ok(_) => {}
                Err(e) => {
                    record(&path, &format!("ERROR {:?} {e}", e.kind()));
                    std::process::exit(if e.kind() == std::io::ErrorKind::BrokenPipe {
                        3
                    } else {
                        4
                    });
                }
            }
        }
        record(
            &path,
            &format!(
                "CANONICAL got_z={got_z} slowest_poll_ms={}",
                slowest.as_millis()
            ),
        );
        std::process::exit(0);
    }

    let mut ctx = gibson::Context::new(gibson::RenderMode::Inline).expect("create Context");
    // Arms raw mode and the input source on the live terminal.
    let _ = ctx
        .poll_event(Duration::ZERO)
        .expect("first poll on a live terminal");

    match scenario.as_str() {
        "backend-after-hangup" => {
            let backend = arg(&args, "--backend").expect("--backend gibson|crossterm");
            if backend == "crossterm" {
                // Warm crossterm's reader on the live terminal, as LibGibson's earlier
                // polls did before #269 was fixed.
                let _ = crossterm::event::poll(Duration::from_millis(10));
            }
            ready();
            assert!(wait_for_hangup(), "the terminal never hung up");
            watchdog(path.clone());
            let r = match backend.as_str() {
                "gibson" => gibson::input::poll_event(Duration::ZERO).map(|e| e.is_some()),
                "crossterm" => crossterm::event::poll(Duration::ZERO),
                other => panic!("--backend {other:?}"),
            };
            match r {
                Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                    record(&path, &format!("BROKEN_PIPE backend={backend}"));
                    std::process::exit(3);
                }
                Err(e) => {
                    record(
                        &path,
                        &format!("ERROR backend={backend} {:?} {e}", e.kind()),
                    );
                    std::process::exit(4);
                }
                Ok(ready) => {
                    record(&path, &format!("RETURNED backend={backend} ready={ready}"));
                    std::process::exit(5);
                }
            }
        }
        "loop" => {
            let ms: u64 = arg(&args, "--timeout-ms")
                .map(|v| v.parse().expect("--timeout-ms"))
                .unwrap_or(100);
            let timeout = Duration::from_millis(ms);
            ready();
            let mut polls = 0u64;
            loop {
                polls += 1;
                match ctx.poll_event(timeout) {
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                        // The read can see the hangup before the kernel has run the SIGHUP
                        // handler (signal delivery is asynchronous to the descriptor's state),
                        // so give a handled SIGHUP up to 1 s to land before recording it.
                        if signal == "handle" {
                            let t = Instant::now();
                            while SIGHUPS.load(Ordering::Relaxed) == 0
                                && t.elapsed() < Duration::from_secs(1)
                            {
                                std::thread::sleep(Duration::from_millis(1));
                            }
                        }
                        record(
                            &path,
                            &format!(
                                "BROKEN_PIPE polls={polls} sighups_handled={}",
                                SIGHUPS.load(Ordering::Relaxed)
                            ),
                        );
                        std::process::exit(3);
                    }
                    Err(e) => {
                        record(&path, &format!("ERROR {:?} {e}", e.kind()));
                        std::process::exit(4);
                    }
                }
            }
        }
        "pending" => {
            ready();
            assert!(wait_for_hangup(), "the terminal never hung up");
            watchdog(path.clone());
            let mut keys = String::new();
            loop {
                match ctx.poll_event(Duration::from_millis(100)) {
                    Ok(Some(gibson::Event::Key(k))) => {
                        if let gibson::KeyCode::Char(c) = k.code {
                            keys.push(c);
                        }
                    }
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                        record(&path, &format!("BROKEN_PIPE keys={keys}"));
                        std::process::exit(3);
                    }
                    Err(e) => {
                        record(&path, &format!("ERROR {:?} {e}", e.kind()));
                        std::process::exit(4);
                    }
                }
            }
        }
        "decoded-then-hangup" => {
            // The parent writes several keys at once; the first poll decodes them all in
            // one read and returns the first. The parent hangs up only after `GOT1`, so
            // the rest are already decoded (queued) when the hangup lands.
            ready();
            let deadline = Instant::now() + Duration::from_secs(5);
            let first = loop {
                assert!(Instant::now() < deadline, "no first key");
                if let Ok(Some(gibson::Event::Key(k))) = ctx.poll_event(Duration::from_millis(100))
                {
                    if let gibson::KeyCode::Char(c) = k.code {
                        break c;
                    }
                }
            };
            let mut out = std::io::stdout();
            let _ = write!(out, "GOT1 {first}\r\n");
            let _ = out.flush();
            assert!(wait_for_hangup(), "the terminal never hung up");
            watchdog(path.clone());
            let mut after = String::new();
            loop {
                match ctx.poll_event(Duration::from_millis(100)) {
                    Ok(Some(gibson::Event::Key(k))) => {
                        if let gibson::KeyCode::Char(c) = k.code {
                            after.push(c);
                        }
                    }
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                        record(&path, &format!("BROKEN_PIPE first={first} after={after}"));
                        std::process::exit(3);
                    }
                    Err(e) => {
                        record(&path, &format!("ERROR {:?} {e}", e.kind()));
                        std::process::exit(4);
                    }
                }
            }
        }
        "keys" => {
            // Records every key as it arrives (the result file outlives the terminal), so
            // a parent can check which terminal the input came from.
            ready();
            loop {
                match ctx.poll_event(Duration::from_millis(100)) {
                    Ok(Some(gibson::Event::Key(k))) => {
                        if let gibson::KeyCode::Char(c) = k.code {
                            record(&path, &format!("KEY {c}"));
                        }
                    }
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                        record(&path, "BROKEN_PIPE");
                        std::process::exit(3);
                    }
                    Err(e) => {
                        record(&path, &format!("ERROR {:?} {e}", e.kind()));
                        std::process::exit(4);
                    }
                }
            }
        }
        other => panic!("unknown scenario {other:?}"),
    }
}
