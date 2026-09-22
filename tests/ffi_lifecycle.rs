use gibson::ffi::*;
use std::ffi::{CStr, CString};
use std::ptr;

#[test]
fn test_ffi_create_destroy_repeated() {
    unsafe {
        for _ in 0..10 {
            let mut ctx = ptr::null_mut();
            let status = gibson_create_context(GibsonRenderMode::Inline, &mut ctx);
            assert_eq!(status, GibsonStatus::Ok);
            assert!(!ctx.is_null());

            gibson_destroy_context(ctx);
        }
    }
}

#[test]
fn test_ffi_null_pointer_error_reporting() {
    unsafe {
        let status = gibson_create_context(GibsonRenderMode::Inline, ptr::null_mut());
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
            gibson_node_text(text_cstr.as_ptr(), ptr::null(), 0, &mut text_node),
            GibsonStatus::Ok
        );

        assert_eq!(gibson_node_add_child(root, text_node), GibsonStatus::Ok);

        // Freeing the root frees the entire tree
        gibson_node_free(root);
    }
}

#[test]
fn test_ffi_render_and_commit() {
    unsafe {
        let mut ctx = ptr::null_mut();
        assert_eq!(
            gibson_create_context(GibsonRenderMode::Inline, &mut ctx),
            GibsonStatus::Ok
        );

        let msg = CString::new("Committed via C ABI").unwrap();
        assert_eq!(gibson_commit(ctx, msg.as_ptr()), GibsonStatus::Ok);

        let mut root = ptr::null_mut();
        assert_eq!(gibson_node_box_row(&mut root), GibsonStatus::Ok);
        assert_eq!(gibson_set_root_node(ctx, root), GibsonStatus::Ok);
        assert_eq!(gibson_render(ctx), GibsonStatus::Ok);

        let mut stats = gibson::scheduler::RenderStats::default();
        assert_eq!(gibson_get_stats(ctx, &mut stats), GibsonStatus::Ok);

        gibson_destroy_context(ctx);
    }
}
