//! Issue #297: a C consumer can bound a headless context's ANSI capture.
//!
//! `gibson_get_rendered_output` copies and never drains, so before #297 a long-running headless C
//! program grew by every frame's bytes. `gibson_take_rendered_output` copies the whole capture and
//! only then empties it (all-or-nothing, so no escape sequence is split); a size query or an
//! undersized buffer removes nothing. `gibson_clear_rendered_output` discards it without copying.
//! Each test pins one clause of that contract; the Rust `Context::take_rendered_bytes` underneath
//! is binary-safe, unlike the lossy-UTF-8 `take_output`.
//!
//! No panic test: the FFI has no panic-injection seam, and the panic clause ("on
//! `GIBSON_ERR_PANIC` the capture is unchanged") follows from the order the implementation keeps
//! (copy, then clear), which the undersized-buffer and size-query tests guard from the outside.

use std::ffi::CString;
use std::ptr;

use gibson::context::{Context, RenderMode};
use gibson::ffi::*;

struct Ctx(*mut GibsonContextOpaque);

impl Ctx {
    fn headless(cols: u16, rows: u16) -> Self {
        let mut ctx = ptr::null_mut();
        assert_eq!(
            unsafe { gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, cols, rows, &mut ctx) },
            GibsonStatus::Ok
        );
        Ctx(ctx)
    }

    /// Sets a one-line text root and renders it.
    fn frame(&self, text: &str) {
        let s = CString::new(text).unwrap();
        let mut node = ptr::null_mut();
        unsafe {
            assert_eq!(
                gibson_node_text(s.as_ptr(), ptr::null(), GIBSON_WRAP_NONE, &mut node),
                GibsonStatus::Ok
            );
            assert_eq!(gibson_set_root_node(self.0, node), GibsonStatus::Ok);
            assert_eq!(gibson_render(self.0), GibsonStatus::Ok);
        }
    }

    /// `gibson_get_rendered_output` in its two-call form.
    fn get(&self) -> Vec<u8> {
        let mut len = usize::MAX;
        unsafe {
            assert_eq!(
                gibson_get_rendered_output(self.0, ptr::null_mut(), 0, &mut len),
                GibsonStatus::Ok
            );
            let mut buf = vec![0u8; len];
            let mut got = usize::MAX;
            assert_eq!(
                gibson_get_rendered_output(self.0, buf.as_mut_ptr(), buf.len(), &mut got),
                GibsonStatus::Ok
            );
            assert_eq!(got, len);
            buf
        }
    }

    fn len(&self) -> usize {
        let mut len = usize::MAX;
        assert_eq!(
            unsafe { gibson_get_rendered_output(self.0, ptr::null_mut(), 0, &mut len) },
            GibsonStatus::Ok
        );
        len
    }

    /// `gibson_take_rendered_output` with an exactly sized buffer.
    fn take(&self) -> Vec<u8> {
        let mut len = usize::MAX;
        unsafe {
            assert_eq!(
                gibson_take_rendered_output(self.0, ptr::null_mut(), 0, &mut len),
                GibsonStatus::Ok
            );
            let mut buf = vec![0u8; len];
            let mut got = usize::MAX;
            assert_eq!(
                gibson_take_rendered_output(self.0, buf.as_mut_ptr(), buf.len(), &mut got),
                GibsonStatus::Ok
            );
            assert_eq!(got, len);
            buf
        }
    }
}

impl Drop for Ctx {
    fn drop(&mut self) {
        unsafe { gibson_destroy_context(self.0) };
    }
}

#[test]
fn a_size_query_does_not_drain() {
    let c = Ctx::headless(20, 4);
    c.frame("hello");
    let before = c.get();
    assert!(!before.is_empty());
    let mut len = 0usize;
    for _ in 0..3 {
        assert_eq!(
            unsafe { gibson_take_rendered_output(c.0, ptr::null_mut(), 0, &mut len) },
            GibsonStatus::Ok
        );
        assert_eq!(len, before.len());
        // A non-null buffer with cap 0 is a size query too.
        let mut one = [0u8; 1];
        assert_eq!(
            unsafe { gibson_take_rendered_output(c.0, one.as_mut_ptr(), 0, &mut len) },
            GibsonStatus::Ok
        );
        assert_eq!(len, before.len());
    }
    assert_eq!(c.get(), before, "a size query removed bytes");
}

#[test]
fn an_undersized_buffer_copies_nothing_and_removes_nothing() {
    let c = Ctx::headless(20, 4);
    c.frame("hello");
    let before = c.get();
    let mut buf = vec![0xAAu8; before.len() - 1];
    let mut len = 0usize;
    assert_eq!(
        unsafe { gibson_take_rendered_output(c.0, buf.as_mut_ptr(), buf.len(), &mut len) },
        GibsonStatus::Ok
    );
    assert_eq!(len, before.len(), "*out_len is the size before the call");
    assert!(
        buf.iter().all(|&b| b == 0xAA),
        "an undersized take wrote into the buffer"
    );
    assert_eq!(c.get(), before, "an undersized take removed bytes");
}

#[test]
fn an_exact_take_copies_everything_and_leaves_zero() {
    let c = Ctx::headless(20, 4);
    c.frame("hello");
    let before = c.get();
    let taken = c.take();
    assert_eq!(taken, before);
    assert_eq!(c.len(), 0);
}

#[test]
fn a_larger_buffer_also_takes_everything() {
    let c = Ctx::headless(20, 4);
    c.frame("hello");
    let before = c.get();
    let mut buf = vec![0u8; before.len() + 64];
    let mut len = 0usize;
    assert_eq!(
        unsafe { gibson_take_rendered_output(c.0, buf.as_mut_ptr(), buf.len(), &mut len) },
        GibsonStatus::Ok
    );
    assert_eq!(len, before.len());
    assert_eq!(&buf[..len], &before[..]);
    assert_eq!(c.len(), 0);
}

#[test]
fn a_repeated_take_returns_zero() {
    let c = Ctx::headless(20, 4);
    c.frame("hello");
    assert!(!c.take().is_empty());
    let mut buf = [0u8; 16];
    let mut len = usize::MAX;
    assert_eq!(
        unsafe { gibson_take_rendered_output(c.0, buf.as_mut_ptr(), buf.len(), &mut len) },
        GibsonStatus::Ok
    );
    assert_eq!(len, 0);
    assert!(c.take().is_empty());
}

#[test]
fn a_take_after_more_frames_returns_only_the_new_frames() {
    let c = Ctx::headless(20, 4);
    c.frame("first");
    let first = c.take();
    assert!(!first.is_empty());
    c.frame("second");
    c.frame("third!");
    let expected = c.get();
    let taken = c.take();
    assert_eq!(taken, expected);
    assert!(!taken.is_empty());
    let text = String::from_utf8_lossy(&taken);
    assert!(text.contains("third!"), "{text:?}");
    assert!(!text.contains("first"), "old bytes came back: {text:?}");
    assert_eq!(c.len(), 0);
}

#[test]
fn get_then_take_are_byte_equal_and_get_never_drains() {
    let c = Ctx::headless(30, 5);
    c.frame("byte \u{1F600} equality");
    let a = c.get();
    let b = c.get();
    assert_eq!(a, b, "gibson_get_rendered_output must not drain");
    assert_eq!(c.take(), a);
}

#[test]
fn a_zero_byte_take_is_ok_and_reports_zero() {
    let c = Ctx::headless(20, 4);
    assert_eq!(c.len(), 0, "no render yet, nothing captured");
    let mut len = usize::MAX;
    assert_eq!(
        unsafe { gibson_take_rendered_output(c.0, ptr::null_mut(), 0, &mut len) },
        GibsonStatus::Ok
    );
    assert_eq!(len, 0);
    let mut buf = [0x55u8; 4];
    len = usize::MAX;
    assert_eq!(
        unsafe { gibson_take_rendered_output(c.0, buf.as_mut_ptr(), buf.len(), &mut len) },
        GibsonStatus::Ok
    );
    assert_eq!(len, 0);
    assert_eq!(buf, [0x55u8; 4]);
}

#[test]
fn null_ctx_or_out_len_is_invalid_and_removes_nothing() {
    let c = Ctx::headless(20, 4);
    c.frame("hello");
    let before = c.get();
    let mut buf = vec![0u8; before.len()];
    let mut len = 0usize;
    assert_eq!(
        unsafe {
            gibson_take_rendered_output(ptr::null_mut(), buf.as_mut_ptr(), buf.len(), &mut len)
        },
        GibsonStatus::ErrInvalidParam
    );
    assert_eq!(
        unsafe { gibson_take_rendered_output(c.0, buf.as_mut_ptr(), buf.len(), ptr::null_mut()) },
        GibsonStatus::ErrInvalidParam
    );
    assert_eq!(c.get(), before, "a refused take removed bytes");
    let mut n = 0usize;
    assert_eq!(
        unsafe { gibson_clear_rendered_output(ptr::null_mut(), &mut n) },
        GibsonStatus::ErrInvalidParam
    );
    assert_eq!(c.get(), before);
}

#[test]
fn clear_reports_the_discarded_count_and_get_then_reports_zero() {
    let c = Ctx::headless(20, 4);
    c.frame("hello");
    c.frame("world");
    let before = c.len();
    assert!(before > 0);
    let mut n = usize::MAX;
    assert_eq!(
        unsafe { gibson_clear_rendered_output(c.0, &mut n) },
        GibsonStatus::Ok
    );
    assert_eq!(n, before);
    assert_eq!(c.len(), 0);
    n = usize::MAX;
    assert_eq!(
        unsafe { gibson_clear_rendered_output(c.0, &mut n) },
        GibsonStatus::Ok
    );
    assert_eq!(n, 0, "a second clear discards nothing");
    // Rendering continues to capture after a clear.
    c.frame("again");
    assert!(c.len() > 0);
}

#[test]
fn clear_with_a_null_count_works() {
    let c = Ctx::headless(20, 4);
    c.frame("hello");
    assert!(c.len() > 0);
    assert_eq!(
        unsafe { gibson_clear_rendered_output(c.0, ptr::null_mut()) },
        GibsonStatus::Ok
    );
    assert_eq!(c.len(), 0);
}

#[test]
fn an_interactive_context_has_nothing_to_take_or_clear() {
    let mut ctx = ptr::null_mut();
    assert_eq!(
        unsafe { gibson_create_context(GIBSON_MODE_INLINE, &mut ctx) },
        GibsonStatus::Ok
    );
    let c = Ctx(ctx);
    let mut buf = [0x11u8; 8];
    let mut len = usize::MAX;
    assert_eq!(
        unsafe { gibson_take_rendered_output(c.0, buf.as_mut_ptr(), buf.len(), &mut len) },
        GibsonStatus::Ok
    );
    assert_eq!(len, 0);
    assert_eq!(buf, [0x11u8; 8]);
    let mut n = usize::MAX;
    assert_eq!(
        unsafe { gibson_clear_rendered_output(c.0, &mut n) },
        GibsonStatus::Ok
    );
    assert_eq!(n, 0);
}

/// The public-API half of the Rust check; the non-UTF-8 half is the unit test in
/// `src/context.rs` (a renderer only emits UTF-8, so invalid bytes need the private sink).
#[test]
fn rust_take_rendered_bytes_returns_the_capture_and_empties_it() {
    use gibson::cell::Style;
    use gibson::node::Node;
    let mut ctx = Context::headless(RenderMode::Inline, 20, 4);
    ctx.set_root(Node::col().child(Node::text("hello", Style::default())));
    ctx.render().unwrap();
    let before = ctx.rendered_bytes().to_vec();
    assert!(!before.is_empty());
    assert_eq!(ctx.take_rendered_bytes(), before);
    assert!(ctx.rendered_bytes().is_empty());
    assert!(ctx.take_rendered_bytes().is_empty());
}
