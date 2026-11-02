use gibson::ffi::*;
use std::ffi::{CStr, CString};
use std::mem::size_of;
use std::ptr;

// Gate K · K4 — serialize the lease-taking lifecycle tests.
//
// Each test below takes the process-global terminal lease via
// `gibson_create_context`. libtest runs `#[test]` fns concurrently, so on a real
// TTY they contend the lease and the loser gets `AlreadyExists` (`ErrIo`). CI
// never sees it: CI's stdout is a pipe, so `is_terminal()` is false and no lease
// is ever taken. The library is correct — a terminal cannot have two owners,
// locked by `tests/terminal_ownership.rs::second_owner_errors` — so the fix is
// purely test hygiene: a file-local gate makes the lease-takers sequential.
// `tests/ffi_lifecycle_lease_pty.rs` proves, on a real PTY, both that the hazard
// is real and that this gate removes it. No `src/` or ABI change.
static LEASE_GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Acquire the serialization gate, recovering from a poisoned lock so a prior
/// test's panic cannot cascade into every later lease-taker failing to acquire.
fn lease_gate() -> std::sync::MutexGuard<'static, ()> {
    LEASE_GATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// An owned lifecycle context: holds the gate for its whole span and destroys
/// the context on drop (even on panic), so the lease is always released before
/// the next test. This closes the leak-on-panic cascade a raw `*mut` leaves: a
/// hostile test that panics after create would otherwise wedge the lease OWNED
/// for the rest of the process and redden every later lease-taker.
struct LeasedCtx {
    ctx: *mut GibsonContextOpaque,
    _gate: std::sync::MutexGuard<'static, ()>,
}

impl LeasedCtx {
    fn ptr(&self) -> *mut GibsonContextOpaque {
        self.ctx
    }
}

impl Drop for LeasedCtx {
    fn drop(&mut self) {
        unsafe { gibson_destroy_context(self.ctx) };
    }
}

unsafe fn make_ctx() -> LeasedCtx {
    let _gate = lease_gate();
    let mut ctx = ptr::null_mut();
    assert_eq!(
        gibson_create_context(GIBSON_MODE_INLINE, &mut ctx),
        GibsonStatus::Ok
    );
    LeasedCtx { ctx, _gate }
}

#[test]
fn test_ffi_create_destroy_repeated() {
    unsafe {
        let _gate = lease_gate();
        for _ in 0..10 {
            let mut ctx = ptr::null_mut();
            let status = gibson_create_context(GIBSON_MODE_INLINE, &mut ctx);
            assert_eq!(status, GibsonStatus::Ok);
            assert!(!ctx.is_null());
            gibson_destroy_context(ctx);
        }
    }
}

#[test]
fn test_ffi_abi_version_exposed() {
    assert_eq!(gibson_abi_version(), GIBSON_ABI_VERSION);
    assert!(gibson_abi_version() >= 1);
}

#[test]
fn test_ffi_null_pointer_error_reporting() {
    unsafe {
        let status = gibson_create_context(GIBSON_MODE_INLINE, ptr::null_mut());
        assert_eq!(status, GibsonStatus::ErrInvalidParam);

        let mut buf = [0i8; 128];
        let len = gibson_last_error_message(buf.as_mut_ptr(), buf.len());
        assert!(len > 0);
        let err_str = CStr::from_ptr(buf.as_ptr()).to_str().unwrap();
        assert!(err_str.contains("Null pointer"));
    }
}

#[test]
fn test_ffi_node_creation_and_layout() {
    unsafe {
        let mut root = ptr::null_mut();
        assert_eq!(gibson_node_box_col(&mut root), GibsonStatus::Ok);
        assert!(!root.is_null());

        assert_eq!(gibson_node_set_width(root, 80.0), GibsonStatus::Ok);
        assert_eq!(gibson_node_set_gap(root, 2.0), GibsonStatus::Ok);

        let text_cstr = CString::new("FFI Test String").unwrap();
        let mut text_node = ptr::null_mut();
        assert_eq!(
            gibson_node_text(
                text_cstr.as_ptr(),
                ptr::null(),
                GIBSON_WRAP_NONE,
                &mut text_node
            ),
            GibsonStatus::Ok
        );
        assert_eq!(gibson_node_add_child(root, text_node), GibsonStatus::Ok);
        gibson_node_free(root);
    }
}

#[test]
fn test_ffi_render_and_commit() {
    unsafe {
        let lease = make_ctx();
        let ctx = lease.ptr();

        let msg = CString::new("Committed via C ABI").unwrap();
        assert_eq!(gibson_commit_text(ctx, msg.as_ptr()), GibsonStatus::Ok);

        let mut root = ptr::null_mut();
        assert_eq!(gibson_node_box_row(&mut root), GibsonStatus::Ok);
        assert_eq!(gibson_set_root_node(ctx, root), GibsonStatus::Ok);
        assert_eq!(gibson_render(ctx), GibsonStatus::Ok);

        let mut stats = GibsonStats::default();
        gibson_stats_init(&mut stats);
        assert_eq!(gibson_get_stats(ctx, &mut stats), GibsonStatus::Ok);
        assert_eq!(stats.abi_version, GIBSON_ABI_VERSION);
        assert_eq!(stats.struct_size as usize, size_of::<GibsonStats>());
        assert!(stats.frames >= 1);

        let insert_msg = CString::new("Inserted above live via C ABI").unwrap();
        assert_eq!(
            gibson_insert_raw_lines_before_live_unchecked(ctx, insert_msg.as_ptr()),
            GibsonStatus::Ok
        );
    }
}

#[test]
fn test_ffi_stats_overflow_is_prevented() {
    unsafe {
        let lease = make_ctx();
        let ctx = lease.ptr();
        // Simulate a *legacy* 7-u64 buffer (56 bytes) that still claims version 1.
        // The engine MUST refuse rather than write 88+ bytes through a 56-byte pointer.
        let mut small = GibsonStats {
            struct_size: 56,
            abi_version: GIBSON_ABI_VERSION,
            ..Default::default()
        };
        assert_eq!(
            gibson_get_stats(ctx, &mut small),
            GibsonStatus::ErrInvalidParam,
            "engine must reject an undersized stats buffer instead of overflowing it"
        );
        // Note: `small` was never written past its declared size.
        assert_eq!(small.frames, 0);
    }
}

#[test]
fn test_ffi_stats_rejects_wrong_abi_version() {
    unsafe {
        let lease = make_ctx();
        let ctx = lease.ptr();
        let mut stats = GibsonStats {
            struct_size: size_of::<GibsonStats>() as u32,
            abi_version: 999,
            ..Default::default()
        };
        assert_eq!(
            gibson_get_stats(ctx, &mut stats),
            GibsonStatus::ErrInvalidParam
        );
    }
}

#[test]
fn test_ffi_hostile_invalid_render_mode() {
    unsafe {
        let mut ctx = ptr::null_mut();
        // 7, -1, and i32::MAX are not valid modes. Must be ordinary validated params.
        for bad in [7, -1, i32::MAX, 2] {
            assert_eq!(
                gibson_create_context(bad, &mut ctx),
                GibsonStatus::ErrInvalidParam,
                "mode {} must be rejected",
                bad
            );
            assert!(ctx.is_null());
        }
    }
}

#[test]
fn test_ffi_hostile_invalid_border_type() {
    unsafe {
        let mut node = ptr::null_mut();
        for bad in [5, -1, 12345] {
            assert_eq!(
                gibson_node_border_box(bad, ptr::null(), &mut node),
                GibsonStatus::ErrInvalidParam
            );
            assert!(node.is_null());
        }
        // Valid value still works.
        assert_eq!(
            gibson_node_border_box(GIBSON_BORDER_ROUNDED, ptr::null(), &mut node),
            GibsonStatus::Ok
        );
        gibson_node_free(node);
    }
}

#[test]
fn test_ffi_hostile_invalid_color_type() {
    unsafe {
        let style = GibsonStyle {
            fg: GibsonColor {
                color_type: 999,
                ..Default::default()
            },
            ..Default::default()
        };
        let text = CString::new("x").unwrap();
        let mut node = ptr::null_mut();
        assert_eq!(
            gibson_node_text(text.as_ptr(), &style, GIBSON_WRAP_NONE, &mut node),
            GibsonStatus::ErrInvalidParam
        );
        assert!(node.is_null());

        // Background color is validated too.
        let style2 = GibsonStyle {
            bg: GibsonColor {
                color_type: -42,
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            gibson_node_text(text.as_ptr(), &style2, GIBSON_WRAP_NONE, &mut node),
            GibsonStatus::ErrInvalidParam
        );
    }
}

#[test]
fn test_ffi_hostile_invalid_wrap_value() {
    unsafe {
        let text = CString::new("x").unwrap();
        let mut node = ptr::null_mut();
        assert_eq!(
            gibson_node_text(text.as_ptr(), ptr::null(), 3, &mut node),
            GibsonStatus::ErrInvalidParam
        );
        assert_eq!(
            gibson_node_text(text.as_ptr(), ptr::null(), -1, &mut node),
            GibsonStatus::ErrInvalidParam
        );
    }
}

#[test]
fn test_ffi_hostile_null_pointers() {
    unsafe {
        let lease = make_ctx();
        let ctx = lease.ptr();
        assert_eq!(
            gibson_render(ptr::null_mut()),
            GibsonStatus::ErrInvalidParam
        );
        assert_eq!(
            gibson_commit_text(ctx, ptr::null()),
            GibsonStatus::ErrInvalidParam
        );
        assert_eq!(
            gibson_insert_raw_lines_before_live_unchecked(ctx, ptr::null()),
            GibsonStatus::ErrInvalidParam
        );
        assert_eq!(
            gibson_set_root_node(ctx, ptr::null_mut()),
            GibsonStatus::ErrInvalidParam
        );
        assert_eq!(
            gibson_get_stats(ctx, ptr::null_mut()),
            GibsonStatus::ErrInvalidParam
        );
        // Freeing null is a no-op, never UB.
        gibson_node_free(ptr::null_mut());
        gibson_line_free(ptr::null_mut());
        gibson_rich_text_free(ptr::null_mut());
    }
}

#[test]
fn test_ffi_hostile_malformed_utf8() {
    unsafe {
        let lease = make_ctx();
        let ctx = lease.ptr();
        // 0xFF 0xFE is not valid UTF-8.
        let bad = CString::new(vec![0xFFu8, 0xFEu8]).unwrap();
        assert_eq!(
            gibson_commit_text(ctx, bad.as_ptr()),
            GibsonStatus::ErrInvalidParam
        );
        assert_eq!(
            gibson_node_text(
                bad.as_ptr(),
                ptr::null(),
                GIBSON_WRAP_NONE,
                &mut ptr::null_mut()
            ),
            GibsonStatus::ErrInvalidParam
        );
    }
}

#[test]
fn test_ffi_rich_text_abi() {
    unsafe {
        let lease = make_ctx();
        let ctx = lease.ptr();

        let mut line = ptr::null_mut();
        assert_eq!(gibson_line_new(&mut line), GibsonStatus::Ok);

        let bold = GibsonStyle {
            bold: 1,
            ..Default::default()
        };
        let a = CString::new("Structured ").unwrap();
        let b = CString::new("rich text").unwrap();
        assert_eq!(
            gibson_line_add_span(line, a.as_ptr(), ptr::null()),
            GibsonStatus::Ok
        );
        assert_eq!(
            gibson_line_add_span(line, b.as_ptr(), &bold),
            GibsonStatus::Ok
        );
        assert_eq!(
            gibson_line_set_align(line, GIBSON_ALIGN_CENTER),
            GibsonStatus::Ok
        );
        assert_eq!(
            gibson_line_set_align(line, 42),
            GibsonStatus::ErrInvalidParam
        );

        let mut rich = ptr::null_mut();
        assert_eq!(gibson_rich_text_new(&mut rich), GibsonStatus::Ok);
        assert_eq!(gibson_rich_text_add_line(rich, line), GibsonStatus::Ok);

        // Build a node from rich text (borrows rich) and render it.
        let mut node = ptr::null_mut();
        assert_eq!(
            gibson_node_rich_text(rich, GIBSON_WRAP_WORD, &mut node),
            GibsonStatus::Ok
        );
        assert_eq!(gibson_set_root_node(ctx, node), GibsonStatus::Ok);
        assert_eq!(gibson_render(ctx), GibsonStatus::Ok);

        // Commit rich text directly too.
        assert_eq!(gibson_commit_rich_text(ctx, rich), GibsonStatus::Ok);
        assert_eq!(
            gibson_insert_rich_text_before_live(ctx, rich),
            GibsonStatus::Ok
        );

        // rich is still owned by us; line too.
        gibson_rich_text_free(rich);
        gibson_line_free(line);
    }
}

#[test]
fn test_ffi_bad_align_rejected() {
    unsafe {
        let mut line = ptr::null_mut();
        assert_eq!(gibson_line_new(&mut line), GibsonStatus::Ok);
        assert_eq!(
            gibson_line_set_align(line, -1),
            GibsonStatus::ErrInvalidParam
        );
        assert_eq!(
            gibson_line_set_align(line, i32::MIN),
            GibsonStatus::ErrInvalidParam
        );
        gibson_line_free(line);
    }
}

#[test]
fn test_ffi_non_finite_layout_values_rejected() {
    unsafe {
        let mut root = ptr::null_mut();
        assert_eq!(gibson_node_box_col(&mut root), GibsonStatus::Ok);
        assert_eq!(
            gibson_node_set_width(root, f32::NAN),
            GibsonStatus::ErrInvalidParam
        );
        assert_eq!(
            gibson_node_set_width(root, f32::INFINITY),
            GibsonStatus::ErrInvalidParam
        );
        assert_eq!(
            gibson_node_set_gap(root, f32::NAN),
            GibsonStatus::ErrInvalidParam
        );
        assert_eq!(gibson_node_set_width(root, 10.0), GibsonStatus::Ok);
        gibson_node_free(root);
    }
}

/// A caller compiled against a *larger* stats struct (e.g. a future engine that
/// appended fields, or a caller with private trailing space) must be safe: the
/// engine writes only `min(caller_size, engine_size)` bytes and leaves the
/// trailing bytes untouched.
#[repr(C)]
struct OversizedStats {
    head: GibsonStats,
    trailing: [u64; 8],
}

#[test]
fn test_ffi_stats_oversized_buffer_prefix_write_leaves_trailing_untouched() {
    unsafe {
        let lease = make_ctx();
        let ctx = lease.ptr();
        let sentinel = [0xDEAD_BEEF_u64; 8];
        let mut big = OversizedStats {
            head: GibsonStats::default(),
            trailing: sentinel,
        };
        big.head.struct_size = size_of::<OversizedStats>() as u32;
        big.head.abi_version = GIBSON_ABI_VERSION;

        let status = gibson_get_stats(ctx, &mut big.head as *mut GibsonStats);
        assert_eq!(status, GibsonStatus::Ok);
        assert_eq!(big.trailing, sentinel, "engine wrote past its own struct");
        assert_eq!(big.head.frames, 0);
    }
}

#[test]
fn test_ffi_insert_text_is_safe_raw_is_explicit() {
    unsafe {
        let lease = make_ctx();
        let ctx = lease.ptr();
        let safe = CString::new("safe \x1b[2J text").unwrap();
        assert_eq!(
            gibson_insert_text_before_live(ctx, safe.as_ptr()),
            GibsonStatus::Ok
        );
        let raw = CString::new("\x1b[2J raw").unwrap();
        assert_eq!(
            gibson_insert_raw_lines_before_live_unchecked(ctx, raw.as_ptr()),
            GibsonStatus::Ok
        );
    }
}
