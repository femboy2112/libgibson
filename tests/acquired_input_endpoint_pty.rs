//! Acceptance for the acquired input endpoint (#345): raw mode, its restore and the input
//! source act on the terminal the session acquired, never on a descriptor that replaced stdin
//! afterwards. Each case runs in a fresh child (stdin/stdout = PTY A slave, fd 4 = PTY B
//! slave, fd 5 = A master, fd 6 = B master), so process-global terminal state never leaks
//! between cases. Termios equality covers flags, control characters, line discipline and
//! speeds; every state is observed before the child resets its PTYs.
#![cfg(target_os = "linux")]

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const CHILD: &str = "GIBSON_ACQUIRED_ENDPOINT_CHILD";

fn pty(cols: u16, rows: u16) -> (OwnedFd, OwnedFd) {
    let (mut master, mut slave) = (-1, -1);
    let size = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: valid out-pointers; successful descriptors become owned.
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                &size,
            )
        },
        0
    );
    unsafe { (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) }
}

fn high(fd: &OwnedFd) -> i32 {
    let d = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 32) };
    assert!(d >= 32);
    d
}

/// Runs `role` in a child and returns its stderr; panics (with that stderr) unless it passed.
fn run(role: &str) -> String {
    let (master_a, slave_a) = pty(40, 15);
    let (master_b, slave_b) = pty(120, 40);
    let fds = [high(&slave_b), high(&master_a), high(&master_b)];
    let controlling = role.starts_with("ctty-");
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "endpoint_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, role)
        .stdin(Stdio::from(slave_a.try_clone().unwrap()))
        .stdout(Stdio::from(slave_a))
        .stderr(Stdio::piped());
    // SAFETY: only async-signal-safe calls between fork and exec.
    unsafe {
        command.pre_exec(move || {
            if libc::setsid() == -1
                || libc::dup2(fds[0], 4) != 4
                || libc::dup2(fds[1], 5) != 5
                || libc::dup2(fds[2], 6) != 6
            {
                return Err(std::io::Error::last_os_error());
            }
            if controlling && libc::ioctl(0, libc::TIOCSCTTY, 0) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().unwrap();
    // The child holds the only master copies it needs; the hangup case relies on that.
    for fd in fds {
        unsafe { libc::close(fd) };
    }
    drop((master_a, master_b));
    let deadline = Instant::now() + Duration::from_secs(20);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let out = child.wait_with_output().unwrap();
            panic!(
                "{role}: timed out: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let out = child.wait_with_output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(out.status.success(), "{role}: {err}");
    assert!(err.contains("CASE_OK"), "{role}: {err}");
    err
}

macro_rules! case {
    ($name:ident, $role:literal) => {
        #[test]
        fn $name() {
            run($role);
        }
    };
}

case!(retarget_restores_a_and_leaves_b_without_ctty, "retarget");
case!(
    retarget_restores_a_and_leaves_b_with_controlling_a,
    "ctty-retarget"
);
case!(a_to_b_to_a_restores_a, "aba");
case!(repeated_restore_and_drop_never_touch_b, "repeat");
case!(caught_owner_panic_restores_a_not_b, "panic");
case!(fresh_owner_on_b_after_restore_owns_b_only, "fresh-owner");
case!(host_crossterm_raw_is_still_disabled_on_restore, "host-raw");
case!(
    host_crossterm_raw_with_retarget_is_refused_without_mutation,
    "host-raw-retarget"
);
case!(hangup_of_a_then_restore_never_touches_b, "hangup");
case!(
    input_comes_from_the_acquired_terminal_after_retarget,
    "input"
);
case!(
    poll_blocked_before_acquisition_is_not_delivered_to_the_owner,
    "blocked-poll"
);
case!(raw_lf_is_ctrl_j_and_cr_is_enter_under_the_lease, "lf");
case!(
    host_crossterm_raw_after_acquisition_still_ends_cooked,
    "mixed"
);

fn attrs(fd: i32) -> libc::termios {
    let mut v = std::mem::MaybeUninit::<libc::termios>::uninit();
    assert_eq!(
        unsafe { libc::tcgetattr(fd, v.as_mut_ptr()) },
        0,
        "tcgetattr({fd})"
    );
    unsafe { v.assume_init() }
}

fn same(a: &libc::termios, b: &libc::termios) -> bool {
    a.c_iflag == b.c_iflag
        && a.c_oflag == b.c_oflag
        && a.c_cflag == b.c_cflag
        && a.c_lflag == b.c_lflag
        && a.c_line == b.c_line
        && a.c_cc == b.c_cc
        && unsafe {
            libc::cfgetispeed(a) == libc::cfgetispeed(b)
                && libc::cfgetospeed(a) == libc::cfgetospeed(b)
        }
}

fn is_raw(t: &libc::termios) -> bool {
    t.c_lflag & (libc::ECHO | libc::ICANON) == 0
}

fn set(fd: i32, t: &libc::termios) {
    assert_eq!(unsafe { libc::tcsetattr(fd, libc::TCSANOW, t) }, 0);
}

fn write_all(fd: i32, bytes: &[u8]) {
    assert_eq!(
        unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) },
        bytes.len() as isize
    );
}

/// The syscall a thread is blocked in, from /proc (first field of `syscall`).
fn blocked_syscall(tid: i32) -> Option<libc::c_long> {
    let text = std::fs::read_to_string(format!("/proc/self/task/{tid}/syscall")).ok()?;
    text.split_whitespace().next()?.parse().ok()
}

fn is_poll_syscall(nr: libc::c_long) -> bool {
    nr == libc::SYS_ppoll || nr == libc::SYS_pselect6 || {
        #[cfg(target_arch = "x86_64")]
        {
            nr == libc::SYS_poll || nr == libc::SYS_select
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            false
        }
    }
}

fn key_char(e: &gibson::input::Event) -> Option<char> {
    match e {
        gibson::input::Event::Key(k) => match k.code {
            gibson::input::KeyCode::Char(c) => Some(c),
            _ => None,
        },
        _ => None,
    }
}

#[test]
fn endpoint_child() {
    let Ok(role) = std::env::var(CHILD) else {
        return;
    };
    let role = role.strip_prefix("ctty-").unwrap_or(&role).to_owned();
    let (b, master_a, master_b) = (4, 5, 6);
    let saved_a = unsafe { libc::fcntl(0, libc::F_DUPFD_CLOEXEC, 10) };
    assert!(saved_a >= 0);
    let before_a = attrs(saved_a);
    // Make B's snapshot distinguishable from A's even when both start alike.
    let mut before_b = attrs(b);
    before_b.c_iflag ^= libc::IXON;
    set(b, &before_b);
    let report = |what: &str| eprintln!("{what}");

    match role.as_str() {
        "retarget" | "aba" | "repeat" => {
            let mut ctx = gibson::Context::fullscreen().unwrap();
            ctx.enter_interactive().unwrap();
            assert!(is_raw(&attrs(saved_a)), "raw not entered on A");
            assert_eq!(unsafe { libc::dup2(b, 0) }, 0);
            if role == "aba" {
                assert_eq!(unsafe { libc::dup2(saved_a, 0) }, 0);
            }
            let r = ctx.restore();
            let (ra, rb) = (same(&before_a, &attrs(saved_a)), same(&before_b, &attrs(b)));
            report(&format!("A_RESTORED={ra} B_UNCHANGED={rb} RESULT={r:?}"));
            assert!(r.is_ok() && ra && rb);
            if role == "repeat" {
                let r2 = ctx.restore();
                drop(ctx);
                let (ra, rb) = (same(&before_a, &attrs(saved_a)), same(&before_b, &attrs(b)));
                report(&format!(
                    "AGAIN A_RESTORED={ra} B_UNCHANGED={rb} RESULT={r2:?}"
                ));
                assert!(r2.is_ok() && ra && rb);
            }
        }
        "panic" => {
            let mut ctx = gibson::Context::fullscreen().unwrap();
            ctx.enter_interactive().unwrap();
            assert!(is_raw(&attrs(saved_a)));
            assert_eq!(unsafe { libc::dup2(b, 0) }, 0);
            let caught = std::panic::catch_unwind(|| panic!("owner panic"));
            assert!(caught.is_err());
            let (ra, rb) = (same(&before_a, &attrs(saved_a)), same(&before_b, &attrs(b)));
            report(&format!("HOOK A_RESTORED={ra} B_UNCHANGED={rb}"));
            assert!(ra && rb, "panic hook restored the wrong terminal");
            // The caught panic invalidated input modes: re-entry raws A again, not B.
            ctx.enter_interactive().unwrap();
            let (raw_a, rb) = (is_raw(&attrs(saved_a)), same(&before_b, &attrs(b)));
            report(&format!("REENTER A_RAW={raw_a} B_UNCHANGED={rb}"));
            assert!(raw_a && rb);
            let _ = ctx.restore();
            assert!(same(&before_a, &attrs(saved_a)) && same(&before_b, &attrs(b)));
        }
        "fresh-owner" => {
            let mut first = gibson::Context::fullscreen().unwrap();
            first.enter_interactive().unwrap();
            assert_eq!(unsafe { libc::dup2(b, 0) }, 0);
            first.restore().unwrap();
            drop(first);
            let mut second = gibson::Context::fullscreen().unwrap();
            second.enter_interactive().unwrap();
            let (raw_b, ra) = (is_raw(&attrs(b)), same(&before_a, &attrs(saved_a)));
            report(&format!("SECOND B_RAW={raw_b} A_UNCHANGED={ra}"));
            assert!(raw_b && ra, "fresh owner must acquire the stdin it finds");
            second.restore().unwrap();
            let (ra, rb) = (same(&before_a, &attrs(saved_a)), same(&before_b, &attrs(b)));
            report(&format!("SECOND_RESTORED A={ra} B={rb}"));
            assert!(ra && rb);
        }
        "host-raw" | "host-raw-retarget" => {
            crossterm::terminal::enable_raw_mode().unwrap();
            let host_raw = attrs(saved_a);
            assert!(is_raw(&host_raw));
            let mut ctx = gibson::Context::fullscreen().unwrap();
            ctx.enter_interactive().unwrap();
            if role == "host-raw" {
                // Documented contract kept: restore disables raw mode (crossterm's baseline).
                let r = ctx.restore();
                let ra = same(&before_a, &attrs(saved_a));
                report(&format!("HOST A_RESTORED={ra} RESULT={r:?}"));
                assert!(r.is_ok() && ra);
            } else {
                assert_eq!(unsafe { libc::dup2(b, 0) }, 0);
                let r = ctx.restore();
                let (a_still_host, rb) =
                    (same(&host_raw, &attrs(saved_a)), same(&before_b, &attrs(b)));
                report(&format!(
                    "REFUSED={} A_UNCHANGED={a_still_host} B_UNCHANGED={rb} RESULT={r:?}",
                    r.is_err()
                ));
                assert!(
                    r.is_err() && a_still_host && rb,
                    "must refuse without mutation"
                );
                // The host's own crossterm state is the host's to clean; reset A for the record.
                set(saved_a, &before_a);
            }
        }
        "hangup" => {
            let mut ctx = gibson::Context::fullscreen().unwrap();
            ctx.enter_interactive().unwrap();
            assert_eq!(unsafe { libc::dup2(b, 0) }, 0);
            unsafe { libc::close(master_a) }; // last master of A: A hangs up
            let r = ctx.restore();
            let rb = same(&before_b, &attrs(b));
            report(&format!("HANGUP B_UNCHANGED={rb} RESULT={r:?}"));
            assert!(rb, "restore after A's hangup must never touch B");
            set(b, &before_b);
            report("CASE_OK");
            return;
        }
        "input" => {
            let mut ctx = gibson::Context::fullscreen().unwrap();
            ctx.enter_interactive().unwrap();
            assert_eq!(unsafe { libc::dup2(b, 0) }, 0);
            write_all(master_b, b"b\n"); // canonical B: a whole line, so FIONREAD counts it
            write_all(master_a, b"a");
            let mut got = Vec::new();
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline && got.len() < 2 {
                if let Ok(Some(e)) = ctx.poll_event(Duration::from_millis(200)) {
                    if let Some(c) = key_char(&e) {
                        got.push(c);
                    }
                }
            }
            report(&format!("INPUT={got:?}"));
            assert_eq!(
                got,
                vec!['a'],
                "input must come only from the acquired terminal A"
            );
            // B's byte is still queued for whoever owns B.
            let mut n: libc::c_int = 0;
            assert_eq!(unsafe { libc::ioctl(b, libc::FIONREAD, &mut n) }, 0);
            report(&format!("B_PENDING={n}"));
            assert_eq!(n, 2);
            ctx.restore().unwrap();
        }
        "blocked-poll" => {
            let (tx, rx) = std::sync::mpsc::channel();
            let poller = std::thread::spawn(move || {
                tx.send(unsafe { libc::gettid() }).unwrap();
                gibson::input::poll_event(Duration::from_secs(10))
            });
            let tid = rx.recv().unwrap();
            // Syscall-entry evidence, not a sleep: wait until the thread is in poll(2).
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if blocked_syscall(tid).is_some_and(is_poll_syscall) {
                    break;
                }
                assert!(Instant::now() < deadline, "poller never entered poll(2)");
                std::thread::yield_now();
            }
            report("POLLER_BLOCKED=true");
            let mut ctx = gibson::Context::fullscreen().unwrap();
            ctx.enter_interactive().unwrap();
            write_all(master_a, b"x");
            let old = poller.join().unwrap();
            let old_got = old
                .as_ref()
                .ok()
                .and_then(|o| o.as_ref())
                .and_then(key_char);
            report(&format!("OLD_POLL={old:?}"));
            assert_eq!(
                old_got, None,
                "an event from a pre-acquisition poll reached the owner"
            );
            write_all(master_a, b"y");
            let mut owner = None;
            let deadline = Instant::now() + Duration::from_secs(3);
            while owner.is_none() && Instant::now() < deadline {
                if let Ok(Some(e)) = ctx.poll_event(Duration::from_millis(200)) {
                    owner = key_char(&e);
                }
            }
            report(&format!("OWNER={owner:?}"));
            assert_eq!(owner, Some('y'));
            ctx.restore().unwrap();
        }
        "lf" => {
            // The lease's raw mode (not crossterm's flag) decides: in raw mode ICRNL is off,
            // so CR is Enter and a bare LF byte is Ctrl+J. LF and Ctrl+J are the same byte; no
            // physical-key provenance is claimed beyond that.
            let mut ctx = gibson::Context::fullscreen().unwrap();
            ctx.enter_interactive().unwrap();
            write_all(master_a, b"\r\n");
            let mut got = Vec::new();
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline && got.len() < 2 {
                if let Ok(Some(gibson::input::Event::Key(k))) =
                    ctx.poll_event(Duration::from_millis(200))
                {
                    got.push(k);
                }
            }
            report(&format!("KEYS={got:?}"));
            assert_eq!(got.len(), 2);
            assert_eq!(got[0].code, gibson::input::KeyCode::Enter);
            assert_eq!(got[1].code, gibson::input::KeyCode::Char('j'));
            assert!(got[1]
                .modifiers
                .contains(gibson::input::KeyModifiers::CONTROL));
            ctx.restore().unwrap();
        }
        "mixed" => {
            // A host that also calls crossterm's raw-mode functions after acquisition (as
            // examples/event_pressure_lab does): crossterm's record must agree with the lease,
            // so its later disable is a no-op rather than re-applying a raw "baseline".
            let mut ctx = gibson::Context::fullscreen().unwrap();
            ctx.enter_interactive().unwrap();
            crossterm::terminal::enable_raw_mode().unwrap();
            ctx.restore().unwrap();
            crossterm::terminal::disable_raw_mode().unwrap();
            let ra = same(&before_a, &attrs(saved_a));
            report(&format!("MIXED A_RESTORED={ra}"));
            assert!(
                ra,
                "terminal left raw after a mixed crossterm host restored"
            );
        }
        other => panic!("unknown role {other}"),
    }
    set(saved_a, &before_a);
    set(b, &before_b);
    report("CASE_OK");
}
