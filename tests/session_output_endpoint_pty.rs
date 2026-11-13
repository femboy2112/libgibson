//! Linux managed-endpoint acceptance: frame, cleanup and descriptor lifetime.
#![cfg(target_os = "linux")]
#[path = "common/pty_capture.rs"]
mod pty_capture;

use gibson::{Context, Node, Style};
use portable_pty::CommandBuilder;
use std::io::Write;
use std::time::{Duration, Instant};

const CHILD: &str = "GIBSON_CONTEXT_PIN_CHILD";
const START: &str = "\x1b]0;PIN_START\x07";
const AFTER_A: &str = "\x1b]0;PIN_AFTER_A\x07";
const END: &str = "\x1b]0;PIN_END\x07";

fn capture(role: &str) -> String {
    let mut cmd = CommandBuilder::new(std::env::current_exe().unwrap());
    cmd.args(["--exact", "pin_child", "--nocapture", "--test-threads=1"]);
    cmd.env(CHILD, role);
    let mut cap = pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_secs(10));
    cap.collect_until(|bytes| bytes.windows(8).any(|window| window == b"PIN_DONE"))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        cap.collect_for(Duration::from_millis(20)).unwrap();
        if let Some(status) = cap.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline);
    };
    cap.collect_all().unwrap();
    assert!(status.success(), "{}", String::from_utf8_lossy(cap.raw()));
    String::from_utf8_lossy(cap.raw()).into_owned()
}

fn snapshots(output: &str) -> (vt100::Screen, vt100::Screen) {
    let start = output.find(START).unwrap() + START.len();
    let after_a = output.find(AFTER_A).unwrap();
    let end = output.find(END).unwrap();
    let mut parser = vt100::Parser::new(24, 80, 0);
    parser.process(&output.as_bytes()[start..after_a]);
    let initial = parser.screen().clone();
    parser.process(&output.as_bytes()[after_a + AFTER_A.len()..end]);
    (initial, parser.screen().clone())
}

#[test]
fn pinned_context_renders_to_original_terminal_after_stdout_redirect() {
    let control = capture("control");
    let redirected = capture("redirect");
    assert!(redirected.contains("PIN_PIPE_LEN=0"), "{redirected:?}");
    let (initial, actual) = snapshots(&redirected);
    let (_, expected) = snapshots(&control);
    assert!(initial.contents().contains("FRAME_A"));
    assert!(expected.contents().contains("FRAME_B"));
    for row in 0..24 {
        for col in 0..80 {
            assert_eq!(
                actual.cell(row, col),
                expected.cell(row, col),
                "cell {row},{col}"
            );
        }
    }
    assert_eq!(actual.cursor_position(), expected.cursor_position());
    assert_eq!(actual.hide_cursor(), expected.hide_cursor());
}

#[test]
fn explicit_restore_closes_the_owned_terminal_descriptor() {
    let output = capture("fd-release");
    assert!(output.contains("PIN_FD_RELEASED=true"), "{output:?}");
}

#[test]
fn direct_session_restore_closes_the_owned_terminal_descriptor() {
    let output = capture("fd-direct");
    assert!(output.contains("PIN_FD_RELEASED=true"), "{output:?}");
}

#[test]
fn context_restore_during_stdout_redirect_restores_physical_cursor() {
    let output = capture("restore-redirect");
    let (_, actual) = snapshots(&output);
    assert!(output.contains("RESTORE_RETURNED_OK=true"));
    assert!(output.contains("RESTORE_PIPE_SHOW=false"));
    assert!(output.contains("PIN_PIPE_LEN=0"));
    assert!(
        !actual.hide_cursor(),
        "physical terminal still has a hidden cursor"
    );
}

#[test]
fn managed_modes_and_caught_panic_keep_the_original_output_endpoint() {
    let control = capture("modes-control");
    let redirected = capture("modes-redirect");
    assert!(redirected.contains("PIN_PIPE_LEN=0"), "{redirected:?}");
    assert!(
        redirected.contains("FINAL_CANONICAL=true"),
        "{redirected:?}"
    );
    for sequence in [
        b"\x1b[?1004h".as_slice(),
        b"\x1b[?1004l",
        b"\x1b[?2004h",
        b"\x1b[?2004l",
        b"\x1b[>15u",
        b"\x1b[<u",
    ] {
        let count = |text: &str| {
            text.as_bytes()
                .windows(sequence.len())
                .filter(|s| *s == sequence)
                .count()
        };
        assert_eq!(count(&control), 2, "control sequence {sequence:?}");
        assert_eq!(count(&redirected), 2, "redirected sequence {sequence:?}");
    }
    let (_, actual) = snapshots(&redirected);
    assert!(
        !actual.hide_cursor(),
        "restoration must show the original terminal cursor"
    );
}

#[test]
fn queued_stdout_precedes_the_first_managed_frame() {
    let control = capture("prelude-control");
    let actual = capture("prelude-buffered");
    let marker = "\x1b]0;BUFFERED_PRELUDE\x07";
    assert!(
        actual.find(marker).unwrap() < actual.find("\x1b[?2026h").unwrap(),
        "{actual:?}"
    );
    let (_, expected) = snapshots(&control);
    let (_, observed) = snapshots(&actual);
    assert!(expected.contents().contains("FRAME_B"));
    for row in 0..24 {
        for col in 0..80 {
            assert_eq!(
                observed.cell(row, col),
                expected.cell(row, col),
                "cell {row},{col}"
            );
        }
    }
    assert_eq!(observed.cursor_position(), expected.cursor_position());
    assert_eq!(observed.hide_cursor(), expected.hide_cursor());
}

#[test]
fn closed_stdout_still_allows_a_non_terminal_session() {
    let output = capture("closed-stdout");
    assert!(output.contains("NON_TTY_CONSTRUCTION=true"), "{output:?}");
}

#[test]
fn rejected_second_owner_cannot_flush_the_current_owners_buffer() {
    let output = capture("second-owner-buffer");
    assert!(
        output.find("SECOND_OWNER_REFUSED").unwrap() < output.find("PENDING_SECOND_OWNER").unwrap(),
        "{output:?}"
    );
}

#[test]
fn held_descriptor_is_not_inherited_by_children() {
    // A child spawned during a lease must not receive the session's held copy of the
    // terminal (it is opened close-on-exec), or a contained runner could write to or
    // ioctl the operator's terminal after the lease ends.
    let output = capture("cloexec");
    assert!(output.contains("CHILD_SEES_HELD=false"), "{output:?}");
    assert!(output.contains("PARENT_HOLDS=true"), "{output:?}");
}

#[test]
fn restore_and_drop_close_the_held_descriptor_exactly_once() {
    // After restore, the descriptor number is free; a file that reuses it must survive
    // the session's Drop (which runs cleanup again), so nothing is closed twice.
    let output = capture("double-close");
    assert!(output.contains("REUSED_FD_SURVIVES=true"), "{output:?}");
}

#[test]
fn piped_stdout_at_acquisition_holds_no_descriptor() {
    let output = capture("piped-stdout");
    assert!(output.contains("PIPED_NON_TTY=true"), "{output:?}");
}

/// Descriptors in `pid` that refer to the same file as this process's fd 1.
fn descriptors_matching_stdout(pid: &str) -> usize {
    let mut reference = std::mem::MaybeUninit::<libc::stat>::uninit();
    assert_eq!(unsafe { libc::fstat(1, reference.as_mut_ptr()) }, 0);
    let reference = unsafe { reference.assume_init() };
    std::fs::read_dir(format!("/proc/{pid}/fd"))
        .unwrap()
        .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
        .filter(|meta| {
            use std::os::unix::fs::MetadataExt;
            meta.dev() == reference.st_dev && meta.ino() == reference.st_ino
        })
        .count()
}

#[test]
fn pin_child() {
    use std::io::Read;
    use std::os::fd::FromRawFd;
    let Ok(role) = std::env::var(CHILD) else {
        return;
    };
    if role == "second-owner-buffer" {
        let mut ctx = Context::inline().unwrap();
        print!("\x1b]0;PENDING_SECOND_OWNER\x07");
        assert!(
            matches!(gibson::TerminalSession::new(), Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists)
        );
        eprintln!("SECOND_OWNER_REFUSED");
        std::io::stdout().flush().unwrap();
        ctx.restore().unwrap();
        eprintln!("PIN_DONE");
        return;
    }
    if role == "cloexec" {
        let before = descriptors_matching_stdout("self");
        let mut ctx = Context::inline().unwrap();
        let during = descriptors_matching_stdout("self");
        // The child inherits fds 0-2 (the same terminal) but must not get the held copy:
        // it counts its own matching descriptors and must see exactly `before`.
        let mut child = std::process::Command::new("sleep")
            .arg("2")
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .unwrap();
        let in_child = descriptors_matching_stdout(&child.id().to_string());
        let _ = child.kill();
        let _ = child.wait();
        ctx.restore().unwrap();
        eprintln!("PARENT_HOLDS={}", during == before + 1);
        eprintln!("CHILD_SEES_HELD={}", in_child != before);
        eprintln!("PIN_DONE");
        return;
    }
    if role == "double-close" {
        let mut ctx = Context::inline().unwrap();
        ctx.restore().unwrap();
        // The lowest free number is the one the held copy just released (or lower).
        let file = std::fs::File::open("/dev/null").unwrap();
        let fd = std::os::fd::AsRawFd::as_raw_fd(&file);
        drop(ctx);
        let alive = unsafe { libc::fcntl(fd, libc::F_GETFD) } != -1;
        drop(file);
        eprintln!("REUSED_FD_SURVIVES={alive}");
        eprintln!("PIN_DONE");
        return;
    }
    if role == "piped-stdout" {
        let saved = unsafe { libc::dup(1) };
        let mut pipe = [0; 2];
        assert_eq!(unsafe { libc::pipe(pipe.as_mut_ptr()) }, 0);
        assert_eq!(unsafe { libc::dup2(pipe[1], 1) }, 1);
        let session = gibson::TerminalSession::new();
        let ok = session.as_ref().is_ok_and(|s| !s.is_tty)
            && gibson::TerminalSession::lease_state() == gibson::TerminalLease::Available;
        drop(session);
        assert_eq!(unsafe { libc::dup2(saved, 1) }, 1);
        unsafe {
            libc::close(saved);
            libc::close(pipe[0]);
            libc::close(pipe[1]);
        }
        eprintln!("PIPED_NON_TTY={ok}");
        eprintln!("PIN_DONE");
        return;
    }
    if role == "closed-stdout" {
        let saved = unsafe { libc::dup(1) };
        assert!(saved >= 0);
        assert_eq!(unsafe { libc::close(1) }, 0);
        let result = gibson::TerminalSession::new();
        let valid = result.as_ref().is_ok_and(|s| !s.is_tty);
        drop(result);
        assert_eq!(unsafe { libc::dup2(saved, 1) }, 1);
        unsafe {
            libc::close(saved);
        }
        eprintln!("NON_TTY_CONSTRUCTION={valid}");
        eprintln!("PIN_DONE");
        return;
    }
    if role.starts_with("prelude-") {
        let mut out = std::io::stdout();
        out.write_all(START.as_bytes()).unwrap();
        out.flush().unwrap();
        print!("\x1b]0;BUFFERED_PRELUDE\x07");
        if role == "prelude-control" {
            out.flush().unwrap();
        }
        let mut ctx = Context::inline().unwrap();
        ctx.set_root(Node::text("FRAME_B", Style::default()));
        ctx.render_now().unwrap();
        out.flush().unwrap();
        out.write_all(AFTER_A.as_bytes()).unwrap();
        out.write_all(END.as_bytes()).unwrap();
        out.flush().unwrap();
        ctx.restore().unwrap();
        eprintln!("PIN_DONE");
        return;
    }
    if role == "fd-release" || role == "fd-direct" {
        fn matching_stdout_descriptors() -> usize {
            let mut reference = std::mem::MaybeUninit::<libc::stat>::uninit();
            assert_eq!(unsafe { libc::fstat(1, reference.as_mut_ptr()) }, 0);
            let reference = unsafe { reference.assume_init() };
            (0..1024)
                .filter(|fd| {
                    let mut current = std::mem::MaybeUninit::<libc::stat>::uninit();
                    if unsafe { libc::fstat(*fd, current.as_mut_ptr()) } != 0 {
                        return false;
                    }
                    let current = unsafe { current.assume_init() };
                    current.st_dev == reference.st_dev
                        && current.st_ino == reference.st_ino
                        && current.st_rdev == reference.st_rdev
                })
                .count()
        }
        let before = matching_stdout_descriptors();
        let mut ctx = Context::inline().unwrap();
        let during = matching_stdout_descriptors();
        if role == "fd-direct" {
            ctx.session.restore().unwrap();
        } else {
            ctx.restore().unwrap();
        }
        let after = matching_stdout_descriptors();
        eprintln!(
            "PIN_FD_RELEASED={}",
            during == before + 1 && after == before
        );
        eprintln!("PIN_DONE");
        return;
    }
    if role.starts_with("modes-") {
        // Suppress only this dedicated child's host hook; LibGibson still installs/chains its own.
        std::panic::set_hook(Box::new(|_| {}));
    }
    let mut ctx = Context::fullscreen().unwrap();
    ctx.set_color_depth(gibson::capability::ColorDepth::TrueColor);
    let mut out = std::io::stdout();
    out.write_all(b"\x1b[2J\x1b[H\x1b]0;PIN_START\x07").unwrap();
    out.flush().unwrap();
    ctx.set_root(Node::text("FRAME_A", Style::default()));
    ctx.render_now().unwrap();
    out.write_all(AFTER_A.as_bytes()).unwrap();
    out.flush().unwrap();
    let mut pipe_bytes = Vec::new();
    if role == "redirect" || role == "restore-redirect" || role == "modes-redirect" {
        let saved = unsafe { libc::dup(1) };
        assert!(saved >= 0);
        let mut pipe = [0; 2];
        assert_eq!(unsafe { libc::pipe(pipe.as_mut_ptr()) }, 0);
        assert_eq!(unsafe { libc::dup2(pipe[1], 1) }, 1);
        unsafe {
            libc::close(pipe[1]);
        }
        if role == "modes-redirect" {
            exercise_modes(&mut ctx);
        } else if role == "redirect" {
            ctx.set_root(Node::text("FRAME_B", Style::default()));
            ctx.render_now().unwrap();
        } else {
            let ok = ctx.restore().is_ok();
            eprintln!("RESTORE_RETURNED_OK={ok}");
        }
        assert_eq!(unsafe { libc::dup2(saved, 1) }, 1);
        unsafe {
            libc::close(saved);
        }
        let mut redirected = unsafe { std::fs::File::from_raw_fd(pipe[0]) };
        redirected.read_to_end(&mut pipe_bytes).unwrap();
    } else if role == "modes-control" {
        exercise_modes(&mut ctx);
    } else {
        ctx.set_root(Node::text("FRAME_B", Style::default()));
        ctx.render_now().unwrap();
    }
    out.write_all(END.as_bytes()).unwrap();
    out.flush().unwrap();
    eprintln!("PIN_PIPE_LEN={}", pipe_bytes.len());
    eprintln!(
        "RESTORE_PIPE_SHOW={}",
        pipe_bytes.windows(6).any(|w| w == b"\x1b[?25h")
    );
    ctx.restore().unwrap();
    eprintln!("PIN_DONE");
}

fn exercise_modes(ctx: &mut Context) {
    ctx.enter_interactive().unwrap();
    ctx.session.set_focus_reporting(true).unwrap();
    ctx.session.set_mouse_capture(true).unwrap();
    ctx.session.leave_alternate_screen().unwrap();
    ctx.session.enter_alternate_screen().unwrap();
    ctx.session.set_key_release_reporting(true).unwrap();
    assert!(std::panic::catch_unwind(|| panic!("endpoint owner panic")).is_err());
    ctx.enter_interactive().unwrap();
    ctx.session.set_focus_reporting(true).unwrap();
    ctx.session.set_mouse_capture(true).unwrap();
    ctx.session.set_key_release_reporting(true).unwrap();
    ctx.restore().unwrap();
    let mut state = std::mem::MaybeUninit::<libc::termios>::uninit();
    // SAFETY: stdin is the live slave of this dedicated test child's PTY.
    assert_eq!(unsafe { libc::tcgetattr(0, state.as_mut_ptr()) }, 0);
    let state = unsafe { state.assume_init() };
    let canonical = state.c_lflag & (libc::ICANON | libc::ECHO) == libc::ICANON | libc::ECHO;
    assert!(canonical);
    eprintln!("FINAL_CANONICAL={canonical}");
}
