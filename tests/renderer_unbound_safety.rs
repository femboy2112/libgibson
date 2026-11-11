use gibson::{Node, RenderMode, Renderer, Style, TerminalSession};

#[test]
fn draining_headless_capture_does_not_renew_the_initial_blank_baseline() {
    use gibson::Context;
    let mut ctx = Context::headless(RenderMode::Inline, 20, 8);
    let mut actual = vt100::Parser::new(8, 20, 0);
    ctx.set_root(Node::text("OLD_CONTENT", Style::default()));
    ctx.render().unwrap();
    actual.process(&ctx.take_rendered_bytes());
    ctx.commit_text("history").unwrap();
    actual.process(&ctx.take_rendered_bytes());
    ctx.set_root(Node::text("X", Style::default()));
    let dirty_before = ctx.stats().dirty_cells;
    ctx.render().unwrap();
    let bytes = ctx.take_rendered_bytes();
    assert!(bytes.windows(4).any(|w| w == b"\x1b[2K"));
    assert_eq!(ctx.stats().dirty_cells - dirty_before, 20);
    actual.process(&bytes);
    let mut expected = vt100::Parser::new(8, 20, 0);
    expected.process(b"history\r\nX\r\x1b[?25l");
    for row in 0..8 {
        for col in 0..20 {
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
fn unbound_inline_and_anchor_operations_fail_before_writing() {
    let mut s = TerminalSession::headless(80, 24);
    let mut r = Renderer::new(RenderMode::Inline);
    let mut out = Vec::new();
    let result = r.render(
        &mut Node::text("unsafe", Style::default()),
        &mut s,
        &mut out,
    );
    assert!(result.is_err(), "unbound inline must not assume an origin");
    assert!(out.is_empty());
    let mut r = Renderer::new(RenderMode::Fullscreen);
    let mut frame = Vec::new();
    r.render(
        &mut Node::text("frame", Style::default()),
        &mut s,
        &mut frame,
    )
    .unwrap();
    for operation in 0..3 {
        let mut other = Vec::new();
        let result = match operation {
            0 => r.commit_text("history", &mut s, &mut other).map(|_| ()),
            1 => r
                .insert_text_before_live("history", &mut s, &mut other)
                .map(|_| ()),
            2 => r.clear_live_region(&mut s, &mut other).map(|_| ()),
            _ => unreachable!(),
        };
        assert!(result.is_err(), "operation {operation}");
        assert!(other.is_empty(), "operation {operation}");
    }
}

#[test]
fn bound_target_diffs_unchanged_frames_and_supports_history() {
    for mode in [RenderMode::Fullscreen, RenderMode::Inline] {
        let mut s = TerminalSession::headless(80, 24);
        let mut r = Renderer::new(mode);
        let mut out = Vec::new();
        {
            let mut bound = r.bind_target(&mut s, &mut out).unwrap();
            let (_, _, first, _, _) = bound
                .render(&mut Node::text("FRAME", Style::default()))
                .unwrap();
            let (_, _, second, _, _) = bound
                .render(&mut Node::text("FRAME", Style::default()))
                .unwrap();
            assert!(first > 0);
            assert_eq!(
                second, 0,
                "same bound destination must retain diff efficiency"
            );
            bound.insert_text_before_live("history").unwrap();
            bound.clear_live_region().unwrap();
        }
        assert!(!out.is_empty());
    }
}

#[test]
fn forgotten_bound_target_cannot_trust_a_later_writer() {
    // Wrap the binding in a Drop type so this remains a meaningful leak
    // control even if BoundTarget later gains its own destructor.
    struct Forget<T>(T);
    impl<T> Drop for Forget<T> {
        fn drop(&mut self) {}
    }
    let mut s = TerminalSession::headless(80, 24);
    let mut r = Renderer::new(RenderMode::Fullscreen);
    let mut a = Vec::new();
    let mut b = Vec::new();
    {
        let mut bound = r.bind_target(&mut s, &mut a).unwrap();
        bound
            .render(&mut Node::text("FRAME_A", Style::default()))
            .unwrap();
        std::mem::forget(Forget(bound));
    }
    r.render(&mut Node::text("FRAME_B", Style::default()), &mut s, &mut b)
        .unwrap();
    r.render(&mut Node::text("FRAME_B", Style::default()), &mut s, &mut a)
        .unwrap();
    let mut actual = vt100::Parser::new(24, 80, 0);
    actual.process(&a);
    assert!(actual.screen().contents().contains("FRAME_B"));
}

#[test]
fn caught_writer_panic_poisons_bound_target_before_retry() {
    use std::{
        cell::{Cell, RefCell},
        io::{self, Write},
        rc::Rc,
    };
    struct PanicWriter {
        panic_next: Rc<Cell<bool>>,
        bytes: Rc<RefCell<Vec<u8>>>,
    }
    impl Write for PanicWriter {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            if self.panic_next.replace(false) {
                self.bytes.borrow_mut().push(b[0]);
                panic!("intentional torn write");
            }
            self.bytes.borrow_mut().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let flag = Rc::new(Cell::new(false));
    let bytes = Rc::new(RefCell::new(Vec::new()));
    let mut writer = PanicWriter {
        panic_next: flag.clone(),
        bytes: bytes.clone(),
    };
    let mut s = TerminalSession::headless(80, 24);
    let mut r = Renderer::new(RenderMode::Fullscreen);
    let mut bound = r.bind_target(&mut s, &mut writer).unwrap();
    bound
        .render(&mut Node::text("FRAME_A", Style::default()))
        .unwrap();
    flag.set(true);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bound
            .render(&mut Node::text("FRAME_B", Style::default()))
            .unwrap();
    }))
    .is_err());
    assert!(bound.physical_state_uncertain());
    bound
        .render(&mut Node::text("FRAME_C", Style::default()))
        .unwrap();
    assert!(!bound.physical_state_uncertain());
    let mut actual = vt100::Parser::new(24, 80, 0);
    actual.process(&bytes.borrow());
    assert!(actual.screen().contents().contains("FRAME_C"));
}

#[test]
fn fresh_bound_target_establishes_cells_and_cursor_independent_of_session_history() {
    for mode in [RenderMode::Fullscreen, RenderMode::Inline] {
        let mut session = TerminalSession::headless(20, 8);
        assert!(
            session.hide_cursor().is_some(),
            "earlier target requested hide"
        );
        let mut renderer = Renderer::new(mode);
        let mut bytes = Vec::new();
        let mut bound = renderer.bind_target(&mut session, &mut bytes).unwrap();
        bound
            .render(&mut Node::text("X", Style::default()))
            .unwrap();
        let mut actual = vt100::Parser::new(8, 20, 0);
        actual.process(b"\x1b[2J\x1b[HOLD_CONTENT\x1b[5;1HPRESERVE\x1b[H");
        actual.process(&bytes);
        assert_eq!(
            actual.screen().cell(0, 0).unwrap().contents(),
            "X",
            "{mode:?}"
        );
        assert_eq!(
            actual.screen().cell(0, 1).unwrap().contents(),
            "",
            "{mode:?}"
        );
        assert!(actual.screen().hide_cursor(), "{mode:?}");
        if mode == RenderMode::Inline {
            assert_eq!(actual.screen().cell(4, 0).unwrap().contents(), "P");
        }
    }
}

#[test]
fn forged_fresh_renderer_physical_state_cannot_bind() {
    let mut renderer = Renderer::new(RenderMode::Inline);
    renderer.live_region_height = 7;
    let mut session = TerminalSession::headless(20, 8);
    let mut bytes = Vec::new();
    assert!(renderer.bind_target(&mut session, &mut bytes).is_err());
    assert!(bytes.is_empty());
}

#[test]
fn bound_fresh_region_after_history_erases_stale_cells() {
    for mode in [RenderMode::Inline, RenderMode::Fullscreen] {
        let mut session = TerminalSession::headless(20, 8);
        let mut renderer = Renderer::new(mode);
        let mut bytes = Vec::new();
        let mut bound = renderer.bind_target(&mut session, &mut bytes).unwrap();
        bound
            .render(&mut Node::text("FIRST_LONG_FRAME", Style::default()))
            .unwrap();
        bound.commit_text("H").unwrap();
        bound
            .render(&mut Node::text("X", Style::default()))
            .unwrap();
        let mut actual = vt100::Parser::new(8, 20, 0);
        actual.process(b"\x1b[2J\x1b[2;1HOLD_CONTENT\x1b[H");
        actual.process(&bytes);
        let row = if mode == RenderMode::Inline { 1 } else { 0 };
        assert_eq!(
            actual.screen().cell(row, 0).unwrap().contents(),
            "X",
            "{mode:?}"
        );
        assert_eq!(
            actual.screen().cell(row, 1).unwrap().contents(),
            "",
            "{mode:?}"
        );
    }
}

#[test]
fn fresh_context_reestablishes_cursor_visibility() {
    let mut ctx = gibson::Context::headless(RenderMode::Fullscreen, 20, 8);
    ctx.set_root(Node::text_input("X", 1, None, Style::default()));
    ctx.render_now().unwrap();
    let mut actual = vt100::Parser::new(8, 20, 0);
    actual.process(b"\x1b[?25l");
    actual.process(ctx.rendered_bytes());
    assert!(!actual.screen().hide_cursor());
}

#[test]
fn bound_inline_growth_establishes_new_rows() {
    let mut session = TerminalSession::headless(20, 8);
    let mut renderer = Renderer::new(RenderMode::Inline);
    let mut bytes = Vec::new();
    let mut bound = renderer.bind_target(&mut session, &mut bytes).unwrap();
    bound.set_capture_damage(true);
    bound
        .render(&mut Node::text("A", Style::default()))
        .unwrap();
    let mut grown = Node::col()
        .child(Node::text("A", Style::default()))
        .child(Node::text("X", Style::default()));
    let (dirty, _, _, _, _) = bound.render(&mut grown).unwrap();
    assert!(dirty >= 20);
    assert_eq!(
        bound
            .last_dirty_cells()
            .iter()
            .filter(|&&(_, y)| y == 1)
            .count(),
        20
    );
    let mut actual = vt100::Parser::new(8, 20, 0);
    actual.process(b"\x1b[2J\x1b[2;1HOLD_CONTENT\x1b[5;1HPRESERVE\x1b[H");
    actual.process(&bytes);
    assert_eq!(actual.screen().cell(1, 0).unwrap().contents(), "X");
    assert_eq!(actual.screen().cell(1, 1).unwrap().contents(), "");
    assert_eq!(actual.screen().cell(4, 0).unwrap().contents(), "P");
}

#[test]
fn inline_growth_to_blank_row_still_clears_that_row() {
    let mut session = TerminalSession::headless(20, 8);
    let mut renderer = Renderer::new(RenderMode::Inline);
    let mut bytes = Vec::new();
    let mut bound = renderer.bind_target(&mut session, &mut bytes).unwrap();
    bound
        .render(&mut Node::text("A", Style::default()))
        .unwrap();
    let mut grown = Node::col()
        .child(Node::text("A", Style::default()))
        .child(Node::text(" ", Style::default()));
    let (_, _, emitted, _, _) = bound.render(&mut grown).unwrap();
    assert!(emitted > 0, "blank growth must not be skipped");
    let mut actual = vt100::Parser::new(8, 20, 0);
    actual.process(b"\x1b[2J\x1b[2;1HOLD_CONTENT\x1b[H");
    actual.process(&bytes);
    for col in 0..20 {
        assert_eq!(actual.screen().cell(1, col).unwrap().contents(), "");
    }
}

#[test]
fn inline_restyle_preserves_rows_below_live_region() {
    let mut session = TerminalSession::headless(20, 8);
    let mut renderer = Renderer::new(RenderMode::Inline);
    let mut bytes = Vec::new();
    let mut bound = renderer.bind_target(&mut session, &mut bytes).unwrap();
    bound
        .render(&mut Node::text("A", Style::default()))
        .unwrap();
    bound.set_color_depth(gibson::capability::ColorDepth::Ansi16);
    bound
        .render(&mut Node::text("A", Style::default()))
        .unwrap();
    let mut actual = vt100::Parser::new(8, 20, 0);
    actual.process(b"\x1b[2J\x1b[5;1HPRESERVE\x1b[H");
    actual.process(&bytes);
    assert_eq!(actual.screen().cell(4, 0).unwrap().contents(), "P");
}
