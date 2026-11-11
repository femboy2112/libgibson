//! Source-specific output pinning probe. Session mode/restore writes remain separate.
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

// The public Session can currently relinquish its lease without informing
// Context that its separately owned output descriptor should be closed.
#[test]
#[ignore = "open: direct session restore retains Context's pinned descriptor"]
fn direct_session_restore_closes_the_owned_terminal_descriptor() {
    let output = capture("fd-direct");
    assert!(output.contains("PIN_FD_RELEASED=true"), "{output:?}");
}

// Known lifecycle gap: Session::restore writes fd 1 rather than the pinned fd.
// It is deliberately ignored only on this research branch, not a release gate pass.
#[test]
#[ignore = "open: restore still writes redirected stdout, not pinned terminal"]
fn context_restore_during_stdout_redirect_restores_physical_cursor() {
    let output = capture("restore-redirect");
    let (_, actual) = snapshots(&output);
    assert!(output.contains("RESTORE_RETURNED_OK=true"));
    assert!(output.contains("RESTORE_PIPE_SHOW=true"));
    assert!(
        !actual.hide_cursor(),
        "physical terminal still has a hidden cursor"
    );
}

#[test]
fn pin_child() {
    use std::io::Read;
    use std::os::fd::FromRawFd;
    let Ok(role) = std::env::var(CHILD) else {
        return;
    };
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
    if role == "redirect" || role == "restore-redirect" {
        let saved = unsafe { libc::dup(1) };
        assert!(saved >= 0);
        let mut pipe = [0; 2];
        assert_eq!(unsafe { libc::pipe(pipe.as_mut_ptr()) }, 0);
        assert_eq!(unsafe { libc::dup2(pipe[1], 1) }, 1);
        unsafe {
            libc::close(pipe[1]);
        }
        if role == "redirect" {
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
