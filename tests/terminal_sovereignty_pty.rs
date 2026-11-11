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

// Physical-output contract: a successful redirected update must not advance the
// physical terminal's differential baseline. Compare against a no-redirection
// control, including every cell and the terminal cursor state.
#[test]
fn context_stdout_redirection_does_not_poison_physical_frame() {
    const START: &str = "\x1b]0;REDIRECT_START\x07";
    const AFTER_A: &str = "\x1b]0;REDIRECT_AFTER_A\x07";
    const END: &str = "\x1b]0;REDIRECT_END\x07";
    fn screen(output: &str) -> (vt100::Screen, vt100::Parser) {
        let start = output.find(START).expect("start marker") + START.len();
        let after_a = output.find(AFTER_A).expect("after A marker");
        let end = output.find(END).expect("end marker");
        let mut p = vt100::Parser::new(24, 80, 0);
        p.process(&output.as_bytes()[start..after_a]);
        let initial = p.screen().clone();
        p.process(&output.as_bytes()[after_a + AFTER_A.len()..end]);
        (initial, p)
    }
    let control = capture("stdout-physical-control");
    let redirected = capture("stdout-redirect-cache");
    assert!(
        redirected.contains("REDIRECT_WROTE_B=true"),
        "redirection positive control: {redirected:?}"
    );
    let (initial, actual) = screen(&redirected);
    let (_, expected) = screen(&control);
    assert!(initial.contents().contains("FRAME_A"));
    assert!(expected.screen().contents().contains("FRAME_B"));
    for row in 0..24 {
        for col in 0..80 {
            assert_eq!(
                actual.screen().cell(row, col),
                expected.screen().cell(row, col),
                "cell {row},{col}"
            );
        }
    }
    assert_eq!(
        actual.screen().cursor_position(),
        expected.screen().cursor_position()
    );
    assert_eq!(
        actual.screen().hide_cursor(),
        expected.screen().hide_cursor()
    );
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

fn assert_swap_unchanged(output: &str) {
    let pre = output.find("\x1b]0;SWAP_PRE\x07").unwrap();
    let post = output.find("\x1b]0;SWAP_POST\x07").unwrap();
    let mut p = vt100::Parser::new(24, 80, 0);
    p.process(&output.as_bytes()[..pre]);
    assert!(
        p.screen().contents().contains("FRAME_B"),
        "positive control"
    );
    let snapshot = |p: &vt100::Parser| {
        let cells: Vec<_> = (0..24)
            .flat_map(|row| (0..80).map(move |col| p.screen().cell(row, col).unwrap().clone()))
            .collect();
        (
            cells,
            p.screen().cursor_position(),
            p.screen().hide_cursor(),
        )
    };
    let before = snapshot(&p);
    p.process(&output.as_bytes()[pre..post]);
    assert_eq!(
        before,
        snapshot(&p),
        "new owner's cell/cursor state changed"
    );
    assert!(output.contains("SWAP_REJECTED=true"), "{output:?}");
    // Mutation control: an extra cell must be visible to the state oracle.
    p.process(b"\x1b[24;80H!");
    assert_ne!(before, snapshot(&p), "extra-cell mutant escaped the oracle");
}

#[test]
fn replacing_context_session_cannot_reuse_old_renderer_state() {
    assert_swap_unchanged(&capture("session-swap"));
}

#[test]
fn replacing_context_renderer_cannot_reuse_old_renderer_state() {
    assert_swap_unchanged(&capture("renderer-swap"));
}

#[test]
fn direct_renderer_use_must_not_poison_context_output_state() {
    let output = capture("renderer-direct");
    let pre = output.find("\x1b]0;SWAP_PRE\x07").unwrap();
    let post = output.find("\x1b]0;SWAP_POST\x07").unwrap();
    let mut p = vt100::Parser::new(24, 80, 0);
    p.process(&output.as_bytes()[..pre]);
    assert!(
        p.screen().contents().contains("FRAME_A"),
        "positive control"
    );
    let before = p.screen().clone();
    p.process(&output.as_bytes()[pre..post]);
    for row in 0..24 {
        for col in 0..80 {
            assert_eq!(before.cell(row, col), p.screen().cell(row, col));
        }
    }
    assert_eq!(before.cursor_position(), p.screen().cursor_position());
    assert_eq!(before.hide_cursor(), p.screen().hide_cursor());
    assert!(output.contains("DIRECT_REJECTED=true"), "{output:?}");
}

#[test]
fn managed_frames_still_diff_and_public_physical_fields_fail_closed() {
    let mut ctx = Context::headless(gibson::RenderMode::Fullscreen, 40, 8);
    ctx.set_root(Node::text("STABLE", Style::default()));
    ctx.render_now().unwrap();
    assert!(!ctx.rendered_bytes().is_empty());
    ctx.take_rendered_bytes();
    ctx.render_now().unwrap();
    assert!(ctx.rendered_bytes().is_empty(), "same target should diff");
    ctx.renderer.last_cursor_x = ctx.renderer.last_cursor_x.wrapping_add(1);
    assert!(ctx.render_now().is_err());
    assert!(ctx.rendered_bytes().is_empty());
}

#[test]
fn changing_public_physical_fields_before_output_fails_closed() {
    for field in 0..6 {
        let mut ctx = Context::headless(gibson::RenderMode::Fullscreen, 40, 8);
        match field {
            0 => ctx.renderer.mode = gibson::RenderMode::Inline,
            1 => ctx.renderer.live_region_height = 1,
            2 => ctx.renderer.last_cursor_x = 1,
            3 => ctx.renderer.last_cursor_y = 1,
            4 => ctx.renderer.last_cursor_visible = Some(true),
            5 => {
                ctx.renderer.anchor = gibson::renderer::AnchorState::Stable {
                    cols: 40,
                    rows: 8,
                    live_height: 1,
                }
            }
            _ => unreachable!(),
        }
        ctx.set_root(Node::text("MUST NOT EMIT", Style::default()));
        assert!(ctx.render_now().is_err(), "field {field}");
        assert!(ctx.rendered_bytes().is_empty());
    }
}

#[test]
fn direct_output_with_same_session_and_another_writer_fails_closed() {
    let mut ctx = Context::headless(gibson::RenderMode::Fullscreen, 40, 8);
    ctx.set_root(Node::text("FIRST", Style::default()));
    ctx.render_now().unwrap();
    let mut other = Vec::new();
    let mut node = Node::text("SECOND", Style::default());
    ctx.renderer
        .render(&mut node, &mut ctx.session, &mut other)
        .unwrap();
    assert!(!other.is_empty());
    assert_eq!(ctx.frame_size(), None);
    assert!(ctx.frame_cell(0, 0).is_none());
    assert!(ctx.last_frame_lines().is_empty());
    ctx.set_root(Node::text("SECOND", Style::default()));
    assert!(ctx.render_now().is_err());
}

#[test]
fn swapping_used_headless_renderers_cannot_confuse_destinations() {
    let mut a = Context::headless(gibson::RenderMode::Fullscreen, 40, 8);
    let mut b = Context::headless(gibson::RenderMode::Fullscreen, 40, 8);
    a.set_root(Node::text("FRAME_A", Style::default()));
    b.set_root(Node::text("FRAME_B", Style::default()));
    a.render_now().unwrap();
    b.render_now().unwrap();
    std::mem::swap(&mut a.renderer, &mut b.renderer);
    let before = a.rendered_bytes().to_vec();
    a.set_root(Node::text("FRAME_B", Style::default()));
    assert!(a.render_now().is_err());
    assert_eq!(a.rendered_bytes(), before);
}

#[test]
#[allow(deprecated)] // Exercise the public compatibility alias as well.
fn every_direct_output_entry_invalidates_managed_output() {
    use gibson::{cell::RichText, node::WrapMode};
    for operation in 0..12 {
        let mut ctx = Context::headless(gibson::RenderMode::Fullscreen, 40, 8);
        ctx.set_root(Node::text("BASE", Style::default()));
        ctx.render_now().unwrap();
        let before = ctx.rendered_bytes().to_vec();
        let mut other_session = TerminalSession::headless(40, 8);
        let mut other = Vec::new();
        let mut node = Node::text("OTHER", Style::default());
        let rich = RichText::raw("OTHER");
        match operation {
            0 => {
                let _ =
                    ctx.renderer
                        .commit_raw_ansi_unchecked("OTHER", &mut other_session, &mut other);
            }
            1 => {
                let _ = ctx.renderer.commit("OTHER", &mut other_session, &mut other);
            }
            2 => {
                let _ = ctx
                    .renderer
                    .commit_text("OTHER", &mut other_session, &mut other);
            }
            3 => {
                let _ = ctx.renderer.commit_text_with_mode(
                    "OTHER",
                    WrapMode::WordWrap,
                    &mut other_session,
                    &mut other,
                );
            }
            4 => {
                let _ = ctx
                    .renderer
                    .commit_rich_text(&rich, &mut other_session, &mut other);
            }
            5 => {
                let _ = ctx
                    .renderer
                    .commit_node(&mut node, &mut other_session, &mut other);
            }
            6 => {
                let _ = ctx.renderer.insert_raw_lines_before_live_unchecked(
                    &["OTHER"],
                    &mut other_session,
                    &mut other,
                );
            }
            7 => {
                let _ =
                    ctx.renderer
                        .insert_text_before_live("OTHER", &mut other_session, &mut other);
            }
            8 => {
                let _ = ctx.renderer.insert_text_before_live_with_mode(
                    "OTHER",
                    WrapMode::WordWrap,
                    &mut other_session,
                    &mut other,
                );
            }
            9 => {
                let _ = ctx.renderer.insert_rich_text_before_live(
                    &rich,
                    &mut other_session,
                    &mut other,
                );
            }
            10 => {
                let _ =
                    ctx.renderer
                        .insert_node_before_live(&mut node, &mut other_session, &mut other);
            }
            11 => {
                let _ = ctx
                    .renderer
                    .clear_live_region(&mut other_session, &mut other);
            }
            _ => unreachable!(),
        }
        assert!(
            ctx.commit_text("MUST NOT EMIT").is_err(),
            "operation {operation}"
        );
        assert_eq!(ctx.rendered_bytes(), before, "operation {operation}");
    }
}

// This records the unresolved standalone API boundary. Unlike Context, direct
// Renderer calls have no stable destination identity in their Write argument.
#[test]
#[ignore = "open: standalone Renderer diffs across unrelated writers"]
fn standalone_renderer_must_not_share_cache_between_writers() {
    let mut renderer = gibson::Renderer::new(gibson::RenderMode::Fullscreen);
    let mut session = TerminalSession::headless(80, 24);
    let mut physical = Vec::new();
    let mut other = Vec::new();
    let mut a = Node::text("FRAME_A", Style::default());
    renderer
        .render(&mut a, &mut session, &mut physical)
        .unwrap();
    let mut virtual_b = Node::text("FRAME_B", Style::default());
    renderer
        .render(&mut virtual_b, &mut session, &mut other)
        .unwrap();
    let mut physical_b = Node::text("FRAME_B", Style::default());
    renderer
        .render(&mut physical_b, &mut session, &mut physical)
        .unwrap();
    let mut reference = Vec::new();
    let mut fresh = gibson::Renderer::new(gibson::RenderMode::Fullscreen);
    let mut fresh_session = TerminalSession::headless(80, 24);
    let mut expected_node = Node::text("FRAME_B", Style::default());
    fresh
        .render(&mut expected_node, &mut fresh_session, &mut reference)
        .unwrap();
    let mut actual = vt100::Parser::new(24, 80, 0);
    let mut expected = vt100::Parser::new(24, 80, 0);
    actual.process(&physical);
    expected.process(&reference);
    assert!(
        expected.screen().contents().contains("FRAME_B"),
        "reference control"
    );
    for row in 0..24 {
        for col in 0..80 {
            assert_eq!(
                actual.screen().cell(row, col),
                expected.screen().cell(row, col),
                "cell {row},{col}"
            );
        }
    }
    assert_eq!(
        actual.screen().cursor_position(),
        expected.screen().cursor_position()
    );
    assert_eq!(
        actual.screen().hide_cursor(),
        expected.screen().hide_cursor()
    );
}

// A writer's address is not its destination identity. This single writer can
// redirect internally while retaining exactly the same address for every call.
#[test]
#[ignore = "open: a writer can retarget without changing its address"]
fn standalone_renderer_must_not_assume_writer_address_identifies_target() {
    use std::{
        cell::{Cell, RefCell},
        io,
        rc::Rc,
    };
    struct Retargetable {
        selected: Rc<Cell<usize>>,
        streams: Rc<RefCell<[Vec<u8>; 2]>>,
    }
    impl io::Write for Retargetable {
        fn write(&mut self, data: &[u8]) -> io::Result<usize> {
            self.streams.borrow_mut()[self.selected.get()].extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let selected = Rc::new(Cell::new(0));
    let streams = Rc::new(RefCell::new([Vec::new(), Vec::new()]));
    let mut writer = Retargetable {
        selected: selected.clone(),
        streams: streams.clone(),
    };
    let mut renderer = gibson::Renderer::new(gibson::RenderMode::Fullscreen);
    let mut session = TerminalSession::headless(80, 24);
    for (target, label) in [(0, "FRAME_A"), (1, "FRAME_B"), (0, "FRAME_B")] {
        selected.set(target);
        renderer
            .render(
                &mut Node::text(label, Style::default()),
                &mut session,
                &mut writer,
            )
            .unwrap();
    }
    let mut expected_output = Vec::new();
    let mut fresh = gibson::Renderer::new(gibson::RenderMode::Fullscreen);
    fresh
        .render(
            &mut Node::text("FRAME_B", Style::default()),
            &mut TerminalSession::headless(80, 24),
            &mut expected_output,
        )
        .unwrap();
    let mut actual = vt100::Parser::new(24, 80, 0);
    let mut expected = vt100::Parser::new(24, 80, 0);
    actual.process(&streams.borrow()[0]);
    expected.process(&expected_output);
    assert!(expected.screen().contents().contains("FRAME_B"));
    for row in 0..24 {
        for col in 0..80 {
            assert_eq!(
                actual.screen().cell(row, col),
                expected.screen().cell(row, col),
                "cell {row},{col}"
            );
        }
    }
    assert_eq!(
        actual.screen().cursor_position(),
        expected.screen().cursor_position()
    );
    assert_eq!(
        actual.screen().hide_cursor(),
        expected.screen().hide_cursor()
    );
}

#[test]
fn cursor_visibility_is_retried_after_write_and_flush_failures() {
    use std::io::{self, Write};
    struct Fault {
        limit: usize,
        sink: Vec<u8>,
        fail_flush: bool,
    }
    impl Write for Fault {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            if self.limit == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "scripted write failure",
                ));
            }
            let n = b.len().min(self.limit);
            self.sink.extend_from_slice(&b[..n]);
            self.limit -= n;
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            if self.fail_flush {
                Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "scripted flush failure",
                ))
            } else {
                Ok(())
            }
        }
    }
    fn frame(visible: bool) -> Node {
        if visible {
            Node::text_input("FRAME", 5, None, Style::default())
        } else {
            Node::text("FRAME", Style::default())
        }
    }
    for mode in [gibson::RenderMode::Fullscreen, gibson::RenderMode::Inline] {
        for visible in [false, true] {
            for (limit, fail_flush, deliver) in [
                (0, false, true),
                (1, false, true),
                (usize::MAX, true, true),
                (usize::MAX, true, false),
            ] {
                let mut renderer = gibson::Renderer::new(mode);
                let mut session = TerminalSession::headless(80, 24);
                let mut initial = Vec::new();
                renderer
                    .render(&mut frame(!visible), &mut session, &mut initial)
                    .unwrap();
                let mut fault = Fault {
                    limit,
                    sink: Vec::new(),
                    fail_flush,
                };
                assert!(renderer
                    .render(&mut frame(visible), &mut session, &mut fault)
                    .is_err());
                let mut retry = Vec::new();
                renderer
                    .render(&mut frame(visible), &mut session, &mut retry)
                    .unwrap();
                let mut actual = vt100::Parser::new(24, 80, 0);
                actual.process(&initial);
                if deliver {
                    actual.process(&fault.sink);
                }
                actual.process(&retry);
                assert!(actual.screen().contents().contains("FRAME"));
                assert_eq!(
                    actual.screen().hide_cursor(),
                    !visible,
                    "{mode:?}, visible={visible}, limit={limit}, flush={fail_flush}, deliver={deliver}"
                );
            }
        }
    }
}

#[test]
fn cursor_visibility_recovers_at_every_frame_write_cut() {
    use std::io::{self, Write};
    struct Cut {
        left: usize,
        bytes: Vec<u8>,
    }
    impl Write for Cut {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            if self.left == 0 {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "cut"));
            }
            let n = self.left.min(b.len());
            self.bytes.extend_from_slice(&b[..n]);
            self.left -= n;
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    fn frame(visible: bool) -> Node {
        if visible {
            Node::text_input("FRAME", 5, None, Style::default())
        } else {
            Node::text("FRAME", Style::default())
        }
    }
    for mode in [gibson::RenderMode::Fullscreen, gibson::RenderMode::Inline] {
        for visible in [false, true] {
            let mut measure = gibson::Renderer::new(mode);
            let mut session = TerminalSession::headless(80, 24);
            measure
                .render(&mut frame(!visible), &mut session, &mut Vec::new())
                .unwrap();
            let mut target = Vec::new();
            measure
                .render(&mut frame(visible), &mut session, &mut target)
                .unwrap();
            assert!(target
                .windows(6)
                .any(|w| w == if visible { b"\x1b[?25h" } else { b"\x1b[?25l" }));
            for cut in 0..target.len() {
                let mut renderer = gibson::Renderer::new(mode);
                let mut session = TerminalSession::headless(80, 24);
                let mut initial = Vec::new();
                renderer
                    .render(&mut frame(!visible), &mut session, &mut initial)
                    .unwrap();
                let mut fault = Cut {
                    left: cut,
                    bytes: Vec::new(),
                };
                assert!(renderer
                    .render(&mut frame(visible), &mut session, &mut fault)
                    .is_err());
                let mut retry = Vec::new();
                renderer
                    .render(&mut frame(visible), &mut session, &mut retry)
                    .unwrap();
                let mut actual = vt100::Parser::new(24, 80, 0);
                for bytes in [&initial, &fault.bytes, &retry] {
                    actual.process(bytes);
                }
                assert_eq!(
                    actual.screen().hide_cursor(),
                    !visible,
                    "mode={mode:?}, visible={visible}, cut={cut}/{}",
                    target.len()
                );
            }
        }
    }
}

#[test]
fn inline_history_does_not_forget_uncertain_cursor_visibility() {
    use std::io::{self, Write};
    struct Fault {
        remaining: usize,
        bytes: Vec<u8>,
        fail_flush: bool,
    }
    impl Write for Fault {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "torn"));
            }
            let n = self.remaining.min(b.len());
            self.bytes.extend_from_slice(&b[..n]);
            self.remaining -= n;
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            if self.fail_flush {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "flush"))
            } else {
                Ok(())
            }
        }
    }
    fn frame(visible: bool) -> Node {
        if visible {
            Node::text_input("FRAME", 5, None, Style::default())
        } else {
            Node::text("FRAME", Style::default())
        }
    }
    for visible in [false, true] {
        for insert in [false, true] {
            for (remaining, fail_flush, deliver) in [
                (1, false, true),
                (usize::MAX, true, true),
                (usize::MAX, true, false),
            ] {
                let mut renderer = gibson::Renderer::new(gibson::RenderMode::Inline);
                let mut session = TerminalSession::headless(80, 24);
                let mut initial = Vec::new();
                renderer
                    .render(&mut frame(!visible), &mut session, &mut initial)
                    .unwrap();
                let mut fault = Fault {
                    remaining,
                    bytes: Vec::new(),
                    fail_flush,
                };
                assert!(renderer
                    .render(&mut frame(visible), &mut session, &mut fault)
                    .is_err());
                let mut history = Vec::new();
                if insert {
                    renderer
                        .insert_text_before_live("history", &mut session, &mut history)
                        .unwrap();
                } else {
                    renderer
                        .commit_text("history", &mut session, &mut history)
                        .unwrap();
                }
                assert!(
                    renderer.physical_state_uncertain(),
                    "cursor remains uncertain"
                );
                let mut retry = Vec::new();
                renderer
                    .render(&mut frame(visible), &mut session, &mut retry)
                    .unwrap();
                assert!(!renderer.physical_state_uncertain());
                let mut actual = vt100::Parser::new(24, 80, 0);
                actual.process(&initial);
                if deliver {
                    actual.process(&fault.bytes);
                }
                actual.process(&history);
                actual.process(&retry);
                assert_eq!(
                    actual.screen().hide_cursor(),
                    !visible,
                    "visible={visible}, insert={insert}, flush={fail_flush}, deliver={deliver}"
                );
            }
        }
    }
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
        "stdout-physical-control" | "stdout-redirect-cache" => {
            use std::io::Read;
            use std::os::fd::FromRawFd;
            let mut ctx = Context::fullscreen().unwrap();
            ctx.set_color_depth(gibson::capability::ColorDepth::TrueColor);
            let mut out = std::io::stdout();
            out.write_all(b"\x1b[2J\x1b[H\x1b]0;REDIRECT_START\x07")
                .unwrap();
            out.flush().unwrap();
            ctx.set_root(Node::text("FRAME_A", Style::default()));
            ctx.render_now().unwrap();
            out.write_all(b"\x1b]0;REDIRECT_AFTER_A\x07").unwrap();
            out.flush().unwrap();
            let mut wrote_b = true;
            if role == "stdout-redirect-cache" {
                let saved = unsafe { libc::dup(1) };
                assert!(saved >= 0);
                let mut pipe = [0; 2];
                assert_eq!(unsafe { libc::pipe(pipe.as_mut_ptr()) }, 0);
                assert_eq!(unsafe { libc::dup2(pipe[1], 1) }, 1);
                unsafe {
                    libc::close(pipe[1]);
                }
                ctx.set_root(Node::text("FRAME_B", Style::default()));
                ctx.render_now().unwrap();
                assert_eq!(unsafe { libc::dup2(saved, 1) }, 1);
                unsafe {
                    libc::close(saved);
                }
                let mut redirect = unsafe { std::fs::File::from_raw_fd(pipe[0]) };
                let mut bytes = Vec::new();
                redirect.read_to_end(&mut bytes).unwrap();
                wrote_b = bytes.contains(&b'B');
            }
            ctx.set_root(Node::text("FRAME_B", Style::default()));
            ctx.render_now().unwrap();
            out.write_all(b"\x1b]0;REDIRECT_END\x07").unwrap();
            out.flush().unwrap();
            eprintln!("REDIRECT_WROTE_B={wrote_b}");
            ctx.restore().unwrap();
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
        "renderer-direct" => {
            let mut owner = Context::fullscreen().unwrap();
            owner.set_root(Node::text("FRAME_A", Style::default()));
            owner.render_now().unwrap();
            let mut out = std::io::stdout();
            out.write_all(b"\x1b]0;SWAP_PRE\x07").unwrap();
            out.flush().unwrap();
            let mut virtual_session = TerminalSession::headless(80, 24);
            virtual_session.set_capabilities(owner.session.capabilities());
            let mut virtual_node = Node::text("FRAME_B", Style::default());
            owner
                .renderer
                .render(&mut virtual_node, &mut virtual_session, &mut Vec::new())
                .unwrap();
            owner.set_root(Node::text("FRAME_B", Style::default()));
            let rejected = owner.render_now().is_err();
            out.write_all(b"\x1b]0;SWAP_POST\x07").unwrap();
            out.flush().unwrap();
            eprintln!("DIRECT_REJECTED={rejected}");
            owner.restore().unwrap();
        }
        _ => panic!("unexpected role"),
    }
    eprintln!("SOVEREIGN_DONE");
}
