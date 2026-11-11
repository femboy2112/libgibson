//! Check geometry against the output endpoint when stdout is retargeted.
#![cfg(target_os = "linux")]

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const CHILD: &str = "GIBSON_ENDPOINT_GEOMETRY_CHILD";

fn pty(cols: u16, rows: u16) -> (OwnedFd, OwnedFd) {
    let (mut master, mut slave) = (-1, -1);
    let size = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: valid output pointers and initialized winsize; successful descriptors become owned.
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

fn child(role: &str, b_size: (u16, u16)) -> Output {
    let (_master_a, slave_a) = pty(40, 15);
    let (_master_b, slave_b) = pty(b_size.0, b_size.1);
    // Avoid collisions with the standard library's child-side descriptor setup.
    let b_fd = unsafe { libc::fcntl(slave_b.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 32) };
    assert!(b_fd >= 32);
    let b_owned = unsafe { OwnedFd::from_raw_fd(b_fd) };
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "geometry_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, role)
        .stdin(Stdio::from(slave_a.try_clone().unwrap()))
        .stdout(Stdio::from(slave_a))
        .stderr(Stdio::piped());
    // SAFETY: only async-signal-safe syscalls before exec; the duplicated fd survives exec.
    unsafe {
        let controlling = role == "controlling" || role == "resize-control";
        command.pre_exec(move || {
            if libc::setsid() == -1 || libc::dup2(b_fd, 4) != 4 {
                return Err(std::io::Error::last_os_error());
            }
            if controlling && libc::ioctl(0, libc::TIOCSCTTY, 0) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let result = child.wait_with_output().unwrap();
            panic!(
                "{role}: timed out: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let result = child.wait_with_output().unwrap();
    drop(b_owned);
    result
}

#[test]
fn output_geometry_tracks_the_acquired_endpoint() {
    for (role, b_size) in [
        ("matched", (40, 15)),
        ("different", (120, 40)),
        ("controlling", (120, 40)),
    ] {
        let result = child(role, b_size);
        let err = String::from_utf8_lossy(&result.stderr);
        assert!(result.status.success(), "{role}: {err}");
        assert!(
            err.contains(if role == "controlling" {
                "CONTROLLING_TTY=true"
            } else {
                "NO_CTTY_ENXIO=true"
            }),
            "{role}: {err}"
        );
        assert!(err.contains("ACQUIRED=40x15"), "{role}: {err}");
        assert!(err.contains("AFTER=40x15"), "{role}: {err}");
        eprintln!("{role} control passed: {err}");
    }
}

#[test]
fn raw_cleanup_tracks_the_terminal_where_raw_was_enabled() {
    for role in ["raw-control", "raw-retarget"] {
        let result = child(role, (120, 40));
        let err = String::from_utf8_lossy(&result.stderr);
        assert!(result.status.success(), "{role}: {err}");
        assert!(err.contains("RAW_OBSERVED=true"), "{role}: {err}");
        assert!(err.contains("A_RESTORED=true"), "{role}: {err}");
        assert!(err.contains("B_UNCHANGED=true"), "{role}: {err}");
        eprintln!("{role} control passed: {err}");
    }
}

#[test]
fn host_crossterm_raw_mode_survives_managed_session() {
    for role in ["host-no-session", "host-crossterm-raw"] {
        let result = child(role, (120, 40));
        let err = String::from_utf8_lossy(&result.stderr);
        assert!(result.status.success(), "{role}: {err}");
        assert!(err.contains("HOST_RAW_AFTER=true"), "{role}: {err}");
        eprintln!("{role}: {err}");
    }
}

#[test]
fn resize_event_matches_the_acquired_output_geometry() {
    for role in [
        "resize-control",
        "resize-noctty",
        "resize-before-poll",
        "resize-preexisting-source",
    ] {
        let result = child(role, (120, 40));
        let err = String::from_utf8_lossy(&result.stderr);
        assert!(result.status.success(), "{role}: {err}");
        assert!(err.contains("GETTER=50x20 EVENT=50x20"), "{role}: {err}");
        eprintln!("{role} control passed: {err}");
    }
}

#[test]
fn prior_unmanaged_inflight_poll_must_not_report_a_different_terminal() {
    let result = child("resize-inflight", (120, 40));
    let err = String::from_utf8_lossy(&result.stderr);
    assert!(err.contains("INFLIGHT_POLL_OBSERVED=true"), "{err}");
    assert!(result.status.success(), "{err}");
    assert!(err.contains("GETTER=50x20 EVENT=50x20"), "{err}");
}

#[test]
fn geometry_child() {
    let Ok(role) = std::env::var(CHILD) else {
        return;
    };
    let controlling = std::fs::File::open("/dev/tty");
    if role == "controlling" || role == "resize-control" {
        eprintln!("CONTROLLING_TTY={}", controlling.is_ok());
        assert!(controlling.is_ok());
    } else {
        let no_ctty = controlling.unwrap_err().raw_os_error() == Some(libc::ENXIO);
        eprintln!("NO_CTTY_ENXIO={no_ctty}");
        assert!(no_ctty);
    }
    if role == "host-crossterm-raw" || role == "host-no-session" {
        crossterm::terminal::enable_raw_mode().unwrap();
        assert!(crossterm::terminal::is_raw_mode_enabled().unwrap());
        if role == "host-crossterm-raw" {
            let mut context = gibson::Context::fullscreen().unwrap();
            context.enter_interactive().unwrap();
            context.restore().unwrap();
        }
        let host_raw = crossterm::terminal::is_raw_mode_enabled().unwrap();
        let mut attr = std::mem::MaybeUninit::<libc::termios>::uninit();
        assert_eq!(unsafe { libc::tcgetattr(0, attr.as_mut_ptr()) }, 0);
        let attr = unsafe { attr.assume_init() };
        let physical_raw = attr.c_lflag & (libc::ECHO | libc::ICANON) == 0;
        eprintln!("HOST_RAW_AFTER={host_raw} PHYSICAL_RAW_AFTER={physical_raw}");
        // This call is safe whether the session restored the host mode or not.
        crossterm::terminal::disable_raw_mode().unwrap();
        assert!(
            host_raw && physical_raw,
            "session released host-owned raw mode"
        );
        return;
    }
    if role.starts_with("raw-") {
        fn attrs(fd: libc::c_int) -> libc::termios {
            let mut value = std::mem::MaybeUninit::<libc::termios>::uninit();
            assert_eq!(unsafe { libc::tcgetattr(fd, value.as_mut_ptr()) }, 0);
            unsafe { value.assume_init() }
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
        let saved_a = unsafe { libc::dup(0) };
        assert!(saved_a >= 0);
        let before_a = attrs(saved_a);
        let mut before_b = attrs(4);
        // Make the B snapshot distinguishable even when both PTYs start with the same settings.
        before_b.c_iflag ^= libc::IXON;
        assert_eq!(unsafe { libc::tcsetattr(4, libc::TCSANOW, &before_b) }, 0);
        let mut context = gibson::Context::fullscreen().unwrap();
        context.enter_interactive().unwrap();
        let raw_a = attrs(saved_a);
        let raw = raw_a.c_lflag & (libc::ECHO | libc::ICANON) == 0;
        eprintln!("RAW_OBSERVED={raw}");
        assert!(raw);
        if role == "raw-retarget" {
            assert_eq!(unsafe { libc::dup2(4, 0) }, 0);
        }
        let restore_result = context.restore();
        let restored = same(&before_a, &attrs(saved_a));
        let unchanged = same(&before_b, &attrs(4));
        eprintln!("A_RESTORED={restored} B_UNCHANGED={unchanged}");
        // Restore explicitly even after a failed library cleanup so the test leaves no raw PTY.
        assert_eq!(
            unsafe { libc::tcsetattr(saved_a, libc::TCSANOW, &before_a) },
            0
        );
        assert_eq!(unsafe { libc::tcsetattr(4, libc::TCSANOW, &before_b) }, 0);
        unsafe { libc::close(saved_a) };
        assert!(restore_result.is_ok(), "restore failed: {restore_result:?}");
        assert!(
            restored && unchanged,
            "raw cleanup changed the wrong terminal"
        );
        return;
    }
    if role.starts_with("resize-") {
        let inflight = if role == "resize-inflight" {
            let (tx, rx) = std::sync::mpsc::channel();
            let worker = std::thread::spawn(move || {
                let tid = unsafe { libc::syscall(libc::SYS_gettid) };
                tx.send(tid).unwrap();
                gibson::poll_event(Duration::from_secs(5))
            });
            let tid = rx.recv_timeout(Duration::from_secs(1)).unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let syscall = std::fs::read_to_string(format!("/proc/self/task/{tid}/syscall"))
                    .unwrap_or_default();
                if syscall
                    .split_whitespace()
                    .next()
                    .and_then(|n| n.parse::<libc::c_long>().ok())
                    == Some(libc::SYS_poll)
                {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "worker never entered poll: {syscall}"
                );
                std::thread::yield_now();
            }
            eprintln!("INFLIGHT_POLL_OBSERVED=true");
            Some(worker)
        } else {
            None
        };
        // A public free-function poll can create the process-global source before
        // the managed session exists. The session must not inherit its size reader.
        if role == "resize-preexisting-source" {
            assert!(gibson::poll_event(Duration::ZERO).unwrap().is_none());
        }
        let mut context = gibson::Context::fullscreen().unwrap();
        context.enter_interactive().unwrap();
        // Also test replacement before the lazily created input source exists.
        if role == "resize-before-poll" {
            assert_eq!(unsafe { libc::dup2(4, 1) }, 1);
        }
        // Initialize the input source and its signal pipe.
        if inflight.is_none() {
            assert!(context.poll_event(Duration::ZERO).unwrap().is_none());
        }
        if role != "resize-before-poll" {
            assert_eq!(unsafe { libc::dup2(4, 1) }, 1);
        }
        let size = libc::winsize {
            ws_row: 20,
            ws_col: 50,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        assert_eq!(unsafe { libc::ioctl(0, libc::TIOCSWINSZ, &size) }, 0);
        // Explicitly trigger the signal on both controlling and non-controlling cases.
        assert_eq!(unsafe { libc::raise(libc::SIGWINCH) }, 0);
        let observed = if let Some(worker) = inflight {
            worker.join().unwrap().unwrap()
        } else {
            context.poll_event(Duration::from_secs(2)).unwrap()
        };
        let getter = context.session.terminal_size();
        let event = match observed {
            Some(gibson::Event::Resize(c, r)) => (c, r),
            other => panic!("expected a resize event, got {other:?}"),
        };
        eprintln!(
            "GETTER={}x{} EVENT={}x{}",
            getter.0, getter.1, event.0, event.1
        );
        context.restore().unwrap();
        assert_eq!(getter, (50, 20));
        assert_eq!(event, getter, "resize event must describe acquired output");
        return;
    }
    let mut session = gibson::TerminalSession::new().unwrap();
    let before = session.terminal_size();
    eprintln!("ACQUIRED={}x{}", before.0, before.1);
    assert_eq!(before, (40, 15));
    // SAFETY: fd 4 is an open slave of the second PTY, installed by the parent.
    assert_eq!(unsafe { libc::dup2(4, 1) }, 1);
    let after = session.terminal_size();
    eprintln!("AFTER={}x{}", after.0, after.1);
    session.restore().unwrap();
    assert_eq!(
        after,
        (40, 15),
        "geometry must describe the acquired output terminal"
    );
}
