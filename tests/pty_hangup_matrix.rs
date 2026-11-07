//! Issue #269: a terminal hangup must end LibGibson's input poll with `BrokenPipe`
//! wherever it lands — before a poll, inside one, or while events are queued — without
//! a spin, an unbounded block, lost decoded input, or a switch of input source.
//!
//! Every case runs `examples/tty_hangup_matrix_probe` as a child on a real PTY (or two)
//! and hangs up by closing the master. After the hangup the terminal cannot carry
//! output, so the child appends its outcome to a result file and reports through its
//! exit code: 3 = `BrokenPipe`, 4 = another error, 42 = its 3 s watchdog fired (the
//! poll never returned). Bounds are liveness bounds: a correct poll returns within
//! milliseconds of the hangup; the defect never returns at all.
//!
//! The in-process half of the matrix (fragmented sequences, coalesced resizes, a closed
//! descriptor, timeouts) is in `src/tty_source.rs`'s unit tests.
#![cfg(unix)]

use portable_pty::CommandBuilder;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[path = "common/pty_capture.rs"]
mod pty_capture;

const PROBE: &str = "tty_hangup_matrix_probe";

/// A fresh result-file path for one child.
fn result_path(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let p = std::env::temp_dir().join(format!(
        "libgibson-hangup-matrix-{}-{}-{tag}.txt",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_file(&p);
    p
}

fn read_result(p: &PathBuf) -> String {
    let s = std::fs::read_to_string(p).unwrap_or_default();
    let _ = std::fs::remove_file(p);
    s.trim().to_owned()
}

fn spawn(args: &[&str], result: &PathBuf) -> pty_capture::Capture {
    let mut cmd = CommandBuilder::new(pty_capture::example_path(PROBE));
    cmd.arg(args[0]);
    cmd.arg(result);
    cmd.args(&args[1..]);
    pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_secs(30))
}

/// Waits up to `bound` for the child to exit; `None` if it is still running.
fn wait_exit(cap: &mut pty_capture::Capture, bound: Duration) -> Option<portable_pty::ExitStatus> {
    let t0 = Instant::now();
    while t0.elapsed() < bound {
        if let Some(status) = cap.try_wait().expect("poll child") {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

fn ready(cap: &mut pty_capture::Capture) {
    cap.collect_until(|b| String::from_utf8_lossy(b).contains("READY"))
        .expect("probe should reach READY");
}

/// Runs a hangup case: READY, optional settle, hang up, and the exit within `bound`.
fn hang_up_and_wait(args: &[&str], settle: Duration, bound: Duration) -> (Option<u32>, String) {
    let result = result_path(args[0]);
    let mut cap = spawn(args, &result);
    ready(&mut cap);
    if !settle.is_zero() {
        cap.collect_for(settle).expect("settle");
    }
    cap.hang_up();
    let status = wait_exit(&mut cap, bound);
    let code = status.as_ref().map(|s| s.exit_code());
    drop(cap); // kills a child that is still running
    (code, read_result(&result))
}

// ---- rows 2 and 3: the backend meets an already-dead terminal ----------------------

#[test]
fn row2_a_poll_that_meets_an_already_hung_up_terminal_returns_brokenpipe() {
    // The state a poll is in when the hangup lands after any precheck: the descriptor is
    // already dead when the input backend runs.
    let (code, result) = hang_up_and_wait(
        &["backend-after-hangup", "--backend", "gibson"],
        Duration::ZERO,
        Duration::from_secs(10),
    );
    assert_eq!(code, Some(3), "result: {result:?}");
    assert_eq!(result, "BROKEN_PIPE backend=gibson");
}

#[test]
fn row2_control_crossterms_reader_spins_in_that_same_state() {
    // The control that makes row 2 discriminating: in the identical state, the backend
    // LibGibson used before #269 was fixed (crossterm 0.29's event reader) never returns,
    // and the probe's 3 s watchdog ends it. If this ever returns, the scenario no longer
    // reaches a dead terminal and row 2 proves nothing.
    let (code, result) = hang_up_and_wait(
        &["backend-after-hangup", "--backend", "crossterm"],
        Duration::ZERO,
        Duration::from_secs(10),
    );
    assert_eq!(code, Some(42), "result: {result:?}");
    assert!(result.starts_with("WATCHDOG"), "{result:?}");
}

// ---- row 12: events decoded before the hangup -------------------------------------

#[test]
fn row12_events_decoded_before_the_hangup_are_delivered_before_brokenpipe() {
    // One read decodes "xyz"; the first poll returns x. After the hangup, y and z must
    // still come out, then BrokenPipe. (Before #269 was fixed they were stranded: the
    // hangup check ran ahead of crossterm's queue.)
    let result = result_path("decoded");
    let mut cap = spawn(&["decoded-then-hangup"], &result);
    ready(&mut cap);
    cap.write(b"xyz").expect("send burst");
    cap.collect_until(|b| String::from_utf8_lossy(b).contains("GOT1"))
        .expect("probe should take the first key");
    cap.hang_up();
    let code = wait_exit(&mut cap, Duration::from_secs(10)).map(|s| s.exit_code());
    drop(cap);
    let result = read_result(&result);
    assert_eq!(code, Some(3), "{result:?}");
    assert_eq!(result, "BROKEN_PIPE first=x after=yz");
}

// ---- rows 8, 9, 10: SIGHUP ignored, handled, default --------------------------------

#[test]
fn row8_row9_sighup_ignored_or_handled_ends_the_poll_with_brokenpipe() {
    for signal in ["ignore", "handle"] {
        for timeout_ms in ["100", "0"] {
            let (code, result) = hang_up_and_wait(
                &["loop", "--timeout-ms", timeout_ms, "--signal", signal],
                Duration::from_millis(150),
                Duration::from_secs(10),
            );
            assert_eq!(code, Some(3), "{signal} {timeout_ms} ms: {result:?}");
            assert!(result.starts_with("BROKEN_PIPE"), "{signal}: {result:?}");
            let handled = if signal == "handle" { "1" } else { "0" };
            assert!(
                result.ends_with(&format!("sighups_handled={handled}")),
                "{signal}: {result:?}"
            );
        }
    }
}

#[test]
fn row10_default_sighup_terminates_the_process() {
    let result = result_path("default");
    let mut cap = spawn(
        &["loop", "--timeout-ms", "100", "--signal", "default"],
        &result,
    );
    ready(&mut cap);
    cap.collect_for(Duration::from_millis(150)).expect("settle");
    cap.hang_up();
    let status = wait_exit(&mut cap, Duration::from_secs(10)).expect("terminated");
    drop(cap);
    assert!(
        status
            .signal()
            .is_some_and(|s| s.contains("Hangup") || s.contains("HUP")),
        "the default SIGHUP action must kill it: {status:?}"
    );
    assert_eq!(read_result(&result), "", "it never reached a result");
}

// ---- row 2/3 under load: a tight loop, hangups at many instants ---------------------

#[test]
fn tight_loop_hangups_at_many_instants_all_end_in_brokenpipe() {
    // A zero-timeout loop spends nearly all its time inside the poll, so hangups at
    // spread-out instants land in every part of it. Every iteration is accounted for.
    const N: u32 = 16;
    let mut outcomes = Vec::new();
    for i in 0..N {
        let settle = Duration::from_millis(u64::from(i * 37 % 160));
        let (code, result) = hang_up_and_wait(
            &["loop", "--timeout-ms", "0", "--signal", "ignore"],
            settle,
            Duration::from_secs(10),
        );
        outcomes.push((settle, code, result));
    }
    let brokenpipe = outcomes.iter().filter(|o| o.1 == Some(3)).count();
    assert_eq!(
        brokenpipe, N as usize,
        "every hangup must end the poll: {outcomes:#?}"
    );
}

// ---- canonical mode: an EOF character is input, not a hangup ------------------------

#[test]
fn canonical_mode_eof_character_neither_hangs_up_nor_blocks_a_poll() {
    // No raw mode: ^D on an empty line makes read() return 0 on a live terminal. Each
    // 100 ms poll must still return in time (crossterm's reader blocked until the next
    // line: 1.5 s here), the terminal stays usable, and z arrives.
    let result = result_path("canonical");
    let mut cap = spawn(&["canonical-eof"], &result);
    ready(&mut cap);
    cap.collect_for(Duration::from_millis(200)).expect("settle");
    cap.write(b"\x04").expect("send ^D");
    cap.collect_for(Duration::from_millis(1500)).expect("wait");
    cap.write(b"z\n").expect("send z");
    let status = wait_exit(&mut cap, Duration::from_secs(10)).expect("probe exits");
    drop(cap);
    let result = read_result(&result);
    assert_eq!(status.exit_code(), 0, "{result:?}");
    let slowest: u64 = result
        .split("slowest_poll_ms=")
        .nth(1)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or_else(|| panic!("no slowest_poll_ms in {result:?}"));
    assert!(result.contains("got_z=true"), "{result:?}");
    assert!(
        slowest < 1000,
        "a 100 ms poll took {slowest} ms after ^D: it blocked until the next line"
    );
}

// ---- row 4: stdin and the controlling terminal are different terminals --------------

/// A pty pair `(master, slave)`, both close-on-exec.
fn openpty() -> (std::os::fd::OwnedFd, std::os::fd::OwnedFd) {
    use std::os::fd::FromRawFd;
    // posix_openpt + O_CLOEXEC on both ends: race-free close-on-exec, so no child
    // spawned by any test (in this or a parallel thread) inherits a master and keeps
    // a terminal alive that a test means to hang up.
    // SAFETY: plain libc pty calls; every fd returned is checked and then owned here.
    unsafe {
        let m = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC);
        assert!(m >= 0, "posix_openpt: {}", std::io::Error::last_os_error());
        let master = std::os::fd::OwnedFd::from_raw_fd(m);
        assert_eq!(libc::grantpt(m), 0);
        assert_eq!(libc::unlockpt(m), 0);
        let mut name = [0 as libc::c_char; 128];
        assert_eq!(libc::ptsname_r(m, name.as_mut_ptr(), name.len()), 0);
        let s = libc::open(
            name.as_ptr(),
            libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC,
        );
        assert!(s >= 0, "open slave: {}", std::io::Error::last_os_error());
        (master, std::os::fd::OwnedFd::from_raw_fd(s))
    }
}

fn keys_seen(p: &PathBuf) -> String {
    std::fs::read_to_string(p)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.strip_prefix("KEY "))
        .collect()
}

#[test]
fn row4_input_and_hangup_follow_stdin_not_the_controlling_terminal() {
    // stdin is pty A; the controlling terminal (and stdout) is pty B. LibGibson reads
    // and watches stdin, so: a key on A arrives; a key on B does not; B hanging up does
    // not end the poll; A hanging up does.
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    let (master_a, slave_a) = openpty();
    let (master_b, slave_b) = openpty();
    let result = result_path("identity");
    let slave_b_fd = slave_b.as_raw_fd();
    let mut cmd = std::process::Command::new(pty_capture::example_path(PROBE));
    cmd.arg("keys").arg(&result).env("TERM", "xterm-256color");
    cmd.stdin(Stdio::from(slave_a));
    cmd.stdout(Stdio::from(slave_b.try_clone().unwrap()));
    cmd.stderr(Stdio::from(slave_b.try_clone().unwrap()));
    // SAFETY: setsid and TIOCSCTTY are async-signal-safe calls made between fork and exec.
    unsafe {
        cmd.pre_exec(move || {
            if libc::setsid() < 0 || libc::ioctl(slave_b_fd, libc::TIOCSCTTY, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = cmd.spawn().expect("spawn identity child");
    drop(slave_b);

    let write = |fd: &std::os::fd::OwnedFd, b: &[u8]| {
        // SAFETY: valid fd and buffer.
        let n = unsafe { libc::write(fd.as_raw_fd(), b.as_ptr().cast(), b.len()) };
        assert_eq!(n, b.len() as isize);
    };
    let wait_for = |want: &str| {
        let t = Instant::now();
        while keys_seen(&result) != want && t.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(10));
        }
        keys_seen(&result)
    };
    // Wait for READY on B (the child's stdout), read here and nowhere else: no other
    // descriptor of B's master may stay open, or dropping `master_b` below would not hang
    // B up. After READY the child writes nothing more to B.
    let mut seen = Vec::new();
    let t = Instant::now();
    while !String::from_utf8_lossy(&seen).contains("READY") {
        assert!(t.elapsed() < Duration::from_secs(10), "no READY: {seen:?}");
        let mut p = libc::pollfd {
            fd: master_b.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd.
        if unsafe { libc::poll(&mut p, 1, 50) } > 0 {
            let mut buf = [0u8; 4096];
            // SAFETY: valid fd and buffer.
            let n = unsafe { libc::read(master_b.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
            if n > 0 {
                seen.extend_from_slice(&buf[..n as usize]);
            }
        }
    }

    write(&master_a, b"a");
    assert_eq!(wait_for("a"), "a", "a key on stdin's terminal arrives");
    write(&master_b, b"b");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        keys_seen(&result),
        "a",
        "a key on the controlling terminal is not input"
    );

    drop(master_b); // the controlling terminal hangs up (the probe ignores SIGHUP)
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        child.try_wait().unwrap().is_none(),
        "stdin is still alive: keep polling"
    );
    write(&master_a, b"c");
    assert_eq!(
        wait_for("ac"),
        "ac",
        "stdin still delivers after the ctty hung up"
    );

    drop(master_a); // the input terminal hangs up
    let t = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break Some(s);
        }
        if t.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            break None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let log = std::fs::read_to_string(&result).unwrap_or_default();
    let _ = std::fs::remove_file(&result);
    assert_eq!(status.and_then(|s| s.code()), Some(3), "{log:?}");
    assert!(log.trim_end().ends_with("BROKEN_PIPE"), "{log:?}");
}

// ---- row 4b: stdin redirected, the terminal reached through /dev/tty ---------------

#[test]
fn row4b_with_stdin_redirected_input_and_hangup_follow_the_controlling_terminal() {
    // `app </dev/null` on a terminal: the session is interactive through /dev/tty. Keys typed
    // there must arrive (crossterm read /dev/tty here, and so must LibGibson's own source),
    // and that terminal hanging up must end the poll. Before #269 was fixed the hangup check
    // watched stdin (/dev/null, which never hangs up) while crossterm read the dead /dev/tty:
    // a spin. An early version of the fix read stdin instead: no keys at all.
    let result = result_path("devtty");
    let mut cmd = CommandBuilder::new("/bin/sh");
    cmd.args(["-c", "exec \"$0\" \"$@\" </dev/null"]);
    cmd.arg(pty_capture::example_path(PROBE));
    cmd.arg("keys");
    cmd.arg(&result);
    let mut cap = pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_secs(30));
    ready(&mut cap);
    cap.write(b"t").expect("type on the terminal");
    let t = Instant::now();
    while !std::fs::read_to_string(&result)
        .unwrap_or_default()
        .contains("KEY t")
        && t.elapsed() < Duration::from_secs(10)
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        std::fs::read_to_string(&result)
            .unwrap_or_default()
            .contains("KEY t"),
        "a key typed on the controlling terminal must arrive with stdin redirected"
    );
    cap.hang_up();
    let code = wait_exit(&mut cap, Duration::from_secs(10)).map(|s| s.exit_code());
    drop(cap);
    let log = read_result(&result);
    assert_eq!(code, Some(3), "{log:?}");
    assert!(log.ends_with("BROKEN_PIPE"), "{log:?}");
}

#[test]
fn row4c_stdin_that_hangs_up_before_the_first_poll_is_reported_not_swapped_for_the_ctty() {
    // stdin is pty A, the controlling terminal is pty B. The child acquires the terminal
    // (latching stdin as its identity), then A hangs up and a key is typed on B, all before
    // the first poll. The poll must report A's hangup; reading B instead would silently
    // switch input to another terminal (an early version of the #269 fix chose the
    // descriptor lazily: isatty(0) was false by then, so it opened /dev/tty = B).
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    let (master_a, slave_a) = openpty();
    let (master_b, slave_b) = openpty();
    let result = result_path("acquire");
    let go = result_path("acquire-go");
    let slave_b_fd = slave_b.as_raw_fd();
    let mut cmd = std::process::Command::new(pty_capture::example_path(PROBE));
    cmd.arg("acquire-then-poll")
        .arg(&result)
        .arg("--go")
        .arg(&go);
    cmd.env("TERM", "xterm-256color");
    cmd.stdin(Stdio::from(slave_a));
    cmd.stdout(Stdio::from(slave_b.try_clone().unwrap()));
    cmd.stderr(Stdio::from(slave_b.try_clone().unwrap()));
    // SAFETY: setsid and TIOCSCTTY are async-signal-safe calls made between fork and exec.
    unsafe {
        cmd.pre_exec(move || {
            if libc::setsid() < 0 || libc::ioctl(slave_b_fd, libc::TIOCSCTTY, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = cmd.spawn().expect("spawn child");
    drop(slave_b);
    drop(cmd);
    // READY on B, read here only.
    let mut seen = Vec::new();
    let t = Instant::now();
    while !String::from_utf8_lossy(&seen).contains("READY") {
        assert!(t.elapsed() < Duration::from_secs(10), "no READY: {seen:?}");
        let mut p = libc::pollfd {
            fd: master_b.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd.
        if unsafe { libc::poll(&mut p, 1, 50) } > 0 {
            let mut buf = [0u8; 4096];
            // SAFETY: valid fd and buffer.
            let n = unsafe { libc::read(master_b.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
            if n > 0 {
                seen.extend_from_slice(&buf[..n as usize]);
            }
        }
    }
    drop(master_a); // stdin's terminal hangs up before the first poll
                    // SAFETY: valid fd and buffer.
    assert_eq!(
        unsafe { libc::write(master_b.as_raw_fd(), b"b".as_ptr().cast(), 1) },
        1
    );
    std::thread::sleep(Duration::from_millis(100));
    std::fs::write(&go, b"go").unwrap();
    let t = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break Some(s);
        }
        if t.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            break None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let _ = std::fs::remove_file(&go);
    let log = std::fs::read_to_string(&result).unwrap_or_default();
    let _ = std::fs::remove_file(&result);
    drop(master_b);
    assert!(
        !log.contains("KEY b"),
        "input was read from the controlling terminal: {log:?}"
    );
    assert_eq!(status.and_then(|s| s.code()), Some(3), "{log:?}");
    assert_eq!(log.trim_end(), "BROKEN_PIPE");
}
