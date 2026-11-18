//! Explicit physical reservation, independent of logical height and resize rewind policy.
//! These fixtures contain synthetic ASCII only and use public Rust SDK APIs.
use gibson::{
    ColorDepth, Context, InlineResizePolicy, Node, RenderMode, Renderer, Style, TerminalSession,
};
use std::io::{self, Write};
use std::num::NonZeroU16;

fn nz(value: u16) -> NonZeroU16 {
    NonZeroU16::new(value).unwrap()
}
fn node(prefix: &str, rows: u16, cols: u16) -> Node {
    Node::text(
        (0..rows)
            .map(|row| format!("{prefix}_{row:02}"))
            .collect::<Vec<_>>()
            .join("\n"),
        Style::default(),
    )
    .width(f32::from(cols))
    .height(f32::from(rows))
}
struct Scene {
    ctx: Context,
    vt: vt100::Parser,
}
impl Scene {
    fn new(cols: u16, rows: u16, policy: InlineResizePolicy, reserve: bool) -> Self {
        let mut ctx = Context::headless(RenderMode::Inline, cols, rows);
        ctx.set_color_depth(ColorDepth::Mono);
        ctx.set_inline_resize_policy(policy);
        ctx.set_inline_resize_bottom_reservation(reserve);
        Self {
            ctx,
            vt: vt100::Parser::new(nz(rows), nz(cols), 4096),
        }
    }
    fn pump(&mut self) -> String {
        let bytes = self.ctx.take_output();
        self.vt.process(bytes.as_bytes());
        bytes
    }
    fn frame(&mut self, prefix: &str, rows: u16, cols: u16) -> String {
        self.ctx.set_root(node(prefix, rows, cols));
        self.ctx.render().unwrap();
        self.pump()
    }
    fn resize(&mut self, cols: u16, rows: u16) {
        self.vt.screen_mut().set_size(nz(rows), nz(cols));
        self.ctx.resize(cols, rows).unwrap();
    }
    fn history(&mut self) -> String {
        let length = self.vt.screen().scrollback_len();
        let (rows, cols) = self.vt.screen().size();
        self.vt.screen_mut().set_scrollback(length);
        let mut lines: Vec<_> = self.vt.screen().rows(0, cols.get()).collect();
        for offset in (0..length).rev() {
            self.vt.screen_mut().set_scrollback(offset);
            lines.push(
                self.vt
                    .screen()
                    .rows(0, cols.get())
                    .nth(usize::from(rows.get() - 1))
                    .unwrap(),
            );
        }
        self.vt.screen_mut().set_scrollback(0);
        lines.join("\n")
    }
}
fn fresh(cols: u16, rows: u16) -> String {
    let mut s = Scene::new(cols, rows, InlineResizePolicy::PreserveAbove, false);
    s.frame("FINAL", rows.saturating_sub(1).max(1), cols);
    s.vt.screen().contents()
}
#[test]
fn default_is_off_and_context_and_renderer_controls_agree() {
    let mut ctx = Context::headless(RenderMode::Inline, 40, 15);
    assert!(!ctx.inline_resize_bottom_reservation());
    assert!(!ctx.renderer.inline_resize_bottom_reservation());
    ctx.set_inline_resize_bottom_reservation(true);
    assert!(ctx.inline_resize_bottom_reservation());
    ctx.set_inline_resize_bottom_reservation(false);
    assert!(!ctx.renderer.inline_resize_bottom_reservation());
}
#[test]
fn first_grow_preserves_full_logical_frame_and_immutable_history() {
    for enabled in [false, true] {
        let mut s = Scene::new(40, 15, InlineResizePolicy::PreserveAbove, enabled);
        s.ctx.commit_text("COMMITTED_A\nCOMMITTED_B").unwrap();
        s.pump();
        s.frame("INITIAL", 14, 40);
        s.resize(80, 24);
        s.frame("FINAL", 23, 80);
        assert_eq!(s.ctx.renderer.live_region_height, 23);
        assert_eq!(
            s.vt.screen().contents() == fresh(80, 24),
            enabled,
            "baseline must retain its old row, explicit opt-in must yield the fresh active frame"
        );
        let h = s.history();
        for marker in ["COMMITTED_A", "COMMITTED_B"] {
            assert_eq!(h.matches(marker).count(), 1);
        }
    }
}
#[test]
fn regrow_reclaims_active_frame_without_deleting_returned_committed_history() {
    for enabled in [false, true] {
        for committed in [false, true] {
            let mut s = Scene::new(160, 48, InlineResizePolicy::ReclaimLiveRows, enabled);
            if committed {
                s.ctx
                    .commit_text(
                        &(0..32)
                            .map(|row| format!("PROTECTED_{row:02}"))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    )
                    .unwrap();
                s.pump();
                s.frame("OLD_OTHER", 15, 160);
            } else {
                s.frame("OLD_LIVE", 47, 160);
            }
            s.resize(40, 15);
            s.frame("SMALL", 14, 40);
            s.resize(80, 24);
            s.frame("FINAL", 23, 80);
            assert_eq!(s.ctx.renderer.live_region_height, 23);
            assert_eq!(s.vt.screen().contents() == fresh(80, 24), enabled);
            s.ctx.clear_live_region().unwrap();
            s.pump();
            let history = s.history();
            if committed {
                let mut position = 0;
                for row in 0..32 {
                    let marker = format!("PROTECTED_{row:02}");
                    assert_eq!(history.matches(&marker).count(), 1);
                    let found = history.find(&marker).unwrap();
                    assert!(found >= position);
                    position = found;
                }
            } else {
                assert!(
                    history.contains("OLD_LIVE"),
                    "earlier live text is still history, not deleted"
                );
                for row in 0..47 {
                    assert!(history.matches(&format!("OLD_LIVE_{row:02}")).count() <= 1);
                }
            }
        }
    }
}
fn excluded_bytes(enabled: bool, mode: RenderMode, short: bool) -> Vec<u8> {
    let mut ctx = Context::headless(mode, 40, 15);
    ctx.set_inline_resize_bottom_reservation(enabled);
    ctx.set_root(node("FIRST", if short { 5 } else { 14 }, 40));
    ctx.render().unwrap();
    let mut bytes = ctx.take_output().into_bytes();
    ctx.set_root(node("SAME_SIZE", if short { 5 } else { 14 }, 40));
    ctx.render().unwrap();
    bytes.extend(ctx.take_output().as_bytes());
    ctx.resize(80, 24).unwrap();
    ctx.set_root(node("AFTER", if short { 5 } else { 23 }, 80));
    ctx.render().unwrap();
    bytes.extend(ctx.take_output().as_bytes());
    bytes
}
#[test]
fn startup_short_frames_and_fullscreen_remain_byte_identical() {
    let mut initial = Vec::new();
    for enabled in [false, true] {
        let mut s = Scene::new(40, 15, InlineResizePolicy::PreserveAbove, enabled);
        initial.push(s.frame("STARTUP", 14, 40));
    }
    assert_eq!(initial[0], initial[1]);
    assert_eq!(
        excluded_bytes(false, RenderMode::Inline, true),
        excluded_bytes(true, RenderMode::Inline, true)
    );
    assert_eq!(
        excluded_bytes(false, RenderMode::Fullscreen, false),
        excluded_bytes(true, RenderMode::Fullscreen, false)
    );
}
struct Torn {
    wrote: bool,
}
impl Write for Torn {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.wrote {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "synthetic torn write",
            ))
        } else {
            self.wrote = true;
            Ok(bytes.len().min(1))
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn recovery_bytes(enabled: bool) -> Vec<u8> {
    let mut renderer = Renderer::new(RenderMode::Inline);
    renderer.set_inline_resize_bottom_reservation(enabled);
    let mut session = TerminalSession::headless(40, 15);
    renderer
        .render(&mut node("INITIAL", 14, 40), &mut session, &mut Vec::new())
        .unwrap();
    session.set_terminal_size(80, 24);
    assert!(renderer
        .render(
            &mut node("TORN", 23, 80),
            &mut session,
            &mut Torn { wrote: false }
        )
        .is_err());
    assert!(renderer.physical_state_uncertain());
    let mut bytes = Vec::new();
    renderer
        .render(&mut node("RECOVER", 23, 80), &mut session, &mut bytes)
        .unwrap();
    bytes
}
#[test]
fn uncertain_write_recovery_keeps_existing_bytes() {
    assert_eq!(recovery_bytes(false), recovery_bytes(true));
}
#[test]
fn one_row_maximum_does_not_consume_another_logical_row() {
    for rows in [1, 2] {
        let mut s = Scene::new(40, 3, InlineResizePolicy::PreserveAbove, true);
        s.frame("INITIAL", 2, 40);
        s.resize(80, rows);
        s.frame("FINAL", 1, 80);
        assert_eq!(s.ctx.renderer.live_region_height, 1);
        assert_eq!(s.vt.screen().contents(), fresh(80, rows));
    }
}
