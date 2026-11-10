//! Lease handoff controls using a real PTY. Bytes are requests, not capability evidence.
#![cfg(target_os = "linux")]

#[path = "common/pty_capture.rs"]
mod pty_capture;

use gibson::{cell::Style, context::Context, node::Node, TerminalLease, TerminalSession};
use portable_pty::CommandBuilder;
use std::io::Write;
use std::time::{Duration, Instant};

const CHILD: &str = "GIBSON_SOVEREIGNTY_CHILD";

fn capture(role: &str) -> String {
    let mut cmd = CommandBuilder::new(std::env::current_exe().unwrap());
    cmd.args([
        "--exact",
        "sovereignty_child",
        "--nocapture",
        "--test-threads=1",
    ]);
    cmd.env(CHILD, role);
    let mut cap = pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_secs(10));
    cap.collect_until(|b| b.windows(14).any(|w| w == b"SOVEREIGN_DONE"))
        .expect("child completion marker");
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        cap.collect_for(Duration::from_millis(20)).unwrap();
        if let Some(s) = cap.try_wait().unwrap() {
            break s;
        }
        assert!(Instant::now() < deadline, "child did not exit");
    };
    cap.collect_all().unwrap();
    assert!(status.success(), "{}", String::from_utf8_lossy(cap.raw()));
    String::from_utf8_lossy(cap.raw()).into_owned()
}

#[test]
fn restored_context_cannot_render_or_commit_into_new_owner() {
    let output = capture("stale-render");
    assert!(
        output.contains("RESULT render=true commit=true insert=true clear=true cursor=true"),
        "{output:?}"
    );
    assert!(!output.contains("OLD_RENDER_PAYLOAD"), "{output:?}");
    assert!(!output.contains("OLD_COMMIT_PAYLOAD"), "{output:?}");
    assert!(!output.contains("OLD_INSERT_PAYLOAD"), "{output:?}");
    assert!(
        output.contains("NEW_OWNER_PAYLOAD"),
        "positive owner control: {output:?}"
    );
}

#[test]
fn forged_tty_flag_cannot_restore_a_different_owners_terminal() {
    let output = capture("flag-forge");
    assert!(output.contains("RESULT owner=true raw=true"), "{output:?}");
    let start = output.find("FORGE_START").unwrap();
    let end = output.find("FORGE_END").unwrap();
    assert!(!output[start..end].contains('\u{1b}'), "{output:?}");
}

#[test]
fn public_tty_flag_cannot_leak_the_lease_or_raw_mode() {
    let output = capture("flag-restore");
    assert!(
        output.contains("RESULT lease=true canonical=true"),
        "{output:?}"
    );
}

#[test]
fn headless_session_replacement_keeps_virtual_render_available() {
    let mut ctx = Context::headless(gibson::RenderMode::Inline, 40, 8);
    ctx.session = TerminalSession::headless(40, 8);
    ctx.renderer = gibson::renderer::Renderer::new(gibson::RenderMode::Inline);
    ctx.set_root(Node::text("VIRTUAL_CONTROL", Style::default()));
    ctx.render_now().unwrap();
    assert!(ctx.take_output().contains("VIRTUAL_CONTROL"));
}

#[test]
fn redirected_context_still_commits_plain_text() {
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "sovereignty_child", "--nocapture"])
        .env(CHILD, "redirected")
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "{}",
        String::from_utf8_lossy(&child.stderr)
    );
    assert!(String::from_utf8_lossy(&child.stdout).contains("REDIRECTED_CONTROL"));
}

#[test]
fn replacing_context_session_cannot_reuse_old_renderer_state() {
    let output = capture("session-swap");
    let pre = output.find("\x1b]0;SWAP_PRE\x07").unwrap();
    let post = output.find("\x1b]0;SWAP_POST\x07").unwrap();
    let mut p = vt100::Parser::new(24, 80, 0);
    p.process(output[..pre].as_bytes());
    assert!(
        p.screen().contents().contains("FRAME_B"),
        "positive control"
    );
    p.process(output[pre..post].as_bytes());
    assert!(
        p.screen().contents().contains("FRAME_B"),
        "new owner's screen changed"
    );
    assert!(output.contains("SWAP_REJECTED=true"), "{output:?}");
}

#[test]
fn replacing_context_renderer_cannot_reuse_old_renderer_state() {
    let output = capture("renderer-swap");
    let pre = output.find("\x1b]0;SWAP_PRE\x07").unwrap();
    let post = output.find("\x1b]0;SWAP_POST\x07").unwrap();
    let mut p = vt100::Parser::new(24, 80, 0);
    p.process(output[..pre].as_bytes());
    assert!(
        p.screen().contents().contains("FRAME_B"),
        "positive control"
    );
    p.process(output[pre..post].as_bytes());
    assert!(
        p.screen().contents().contains("FRAME_B"),
        "new owner's screen changed"
    );
    assert!(output.contains("SWAP_REJECTED=true"), "{output:?}");
}

#[test]
fn sovereignty_child() {
    let Ok(role) = std::env::var(CHILD) else {
        return;
    };
    match role.as_str() {
        "redirected" => {
            let mut ctx = Context::inline().unwrap();
            assert!(!ctx.session.is_tty);
            ctx.commit_text("REDIRECTED_CONTROL").unwrap();
        }
        "stale-render" => {
            let mut old = Context::inline().unwrap();
            old.set_root(Node::text("INITIAL_FRAME", Style::default()));
            old.render_now().unwrap();
            old.restore().unwrap();
            let mut owner = Context::inline().unwrap();
            old.set_root(Node::text("OLD_RENDER_PAYLOAD", Style::default()));
            let render = old.render_now().is_err();
            let commit = old.commit_text("OLD_COMMIT_PAYLOAD").is_err();
            let insert = old.insert_text_before_live("OLD_INSERT_PAYLOAD").is_err();
            let clear = old.clear_live_region().is_err();
            let cursor = old.session.hide_cursor().is_none();
            owner.commit_text("NEW_OWNER_PAYLOAD").unwrap();
            eprintln!("RESULT render={render} commit={commit} insert={insert} clear={clear} cursor={cursor}");
            owner.restore().unwrap();
        }
        "flag-restore" => {
            let mut owner = TerminalSession::new().unwrap();
            owner.enter_interactive().unwrap();
            owner.is_tty = false;
            owner.restore().unwrap();
            let lease = TerminalSession::lease_state() == TerminalLease::Available;
            let mut term = std::mem::MaybeUninit::<libc::termios>::uninit();
            assert_eq!(unsafe { libc::tcgetattr(0, term.as_mut_ptr()) }, 0);
            let canonical = unsafe { term.assume_init() }.c_lflag & libc::ICANON != 0;
            eprintln!("RESULT lease={lease} canonical={canonical}");
        }
        "flag-forge" => {
            let saved = unsafe { libc::dup(1) };
            assert!(saved >= 0);
            let mut pipe = [0; 2];
            assert_eq!(unsafe { libc::pipe(pipe.as_mut_ptr()) }, 0);
            assert_eq!(unsafe { libc::dup2(pipe[1], 1) }, 1);
            let mut stranger = TerminalSession::new().unwrap();
            assert!(!stranger.is_tty);
            assert_eq!(unsafe { libc::dup2(saved, 1) }, 1);
            unsafe {
                libc::close(saved);
                libc::close(pipe[0]);
                libc::close(pipe[1]);
            }
            let mut owner = TerminalSession::new().unwrap();
            owner.enter_interactive().unwrap();
            stranger.is_tty = true;
            eprintln!("FORGE_START");
            stranger.restore().unwrap();
            eprintln!("FORGE_END");
            let owns = TerminalSession::lease_state() == TerminalLease::Owned;
            let mut term = std::mem::MaybeUninit::<libc::termios>::uninit();
            assert_eq!(unsafe { libc::tcgetattr(0, term.as_mut_ptr()) }, 0);
            let raw = unsafe { term.assume_init() }.c_lflag & libc::ICANON == 0;
            eprintln!("RESULT owner={owns} raw={raw}");
            owner.restore().unwrap();
        }
        "session-swap" => {
            let mut old = Context::fullscreen().unwrap();
            old.set_root(Node::text("FRAME_A", Style::default()));
            old.render_now().unwrap();
            old.restore().unwrap();
            let mut owner = Context::fullscreen().unwrap();
            owner.set_root(Node::text("FRAME_B", Style::default()));
            owner.render_now().unwrap();
            let mut out = std::io::stdout();
            out.write_all(b"\x1b]0;SWAP_PRE\x07").unwrap();
            out.flush().unwrap();
            std::mem::swap(&mut old.session, &mut owner.session);
            let rejected = old.render_now().is_err();
            out.write_all(b"\x1b]0;SWAP_POST\x07").unwrap();
            out.flush().unwrap();
            eprintln!("SWAP_REJECTED={rejected}");
            old.restore().unwrap();
        }
        "renderer-swap" => {
            let mut old = Context::fullscreen().unwrap();
            old.set_root(Node::text("FRAME_A", Style::default()));
            old.render_now().unwrap();
            old.restore().unwrap();
            let mut owner = Context::fullscreen().unwrap();
            owner.set_root(Node::text("FRAME_B", Style::default()));
            owner.render_now().unwrap();
            let mut out = std::io::stdout();
            out.write_all(b"\x1b]0;SWAP_PRE\x07").unwrap();
            out.flush().unwrap();
            std::mem::swap(&mut old.renderer, &mut owner.renderer);
            owner.set_root(Node::text("FRAME_A", Style::default()));
            let rejected = owner.render_now().is_err();
            out.write_all(b"\x1b]0;SWAP_POST\x07").unwrap();
            out.flush().unwrap();
            eprintln!("SWAP_REJECTED={rejected}");
            owner.restore().unwrap();
        }
        _ => panic!("unexpected role"),
    }
    eprintln!("SOVEREIGN_DONE");
}
