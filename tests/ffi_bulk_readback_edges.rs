//! Gate F4 — error-path coverage for the bulk cell readback `gibson_get_frame_cells`.
//!
//! The parity suites prove the bulk read AGREES with the single-cell read over a full frame. They
//! do not exercise the capacity/offset contract, which is where an ABI consumer's buffer-management
//! bugs bite: a zero-length buffer, a buffer smaller than the region, a start that is not the
//! origin and whose row-major walk crosses a row boundary, an over-large `max_cells` that must
//! clamp to the cells actually remaining, an out-of-bounds start, and a null `out_written`. Each
//! case below pins the documented behavior (`*out_written` always set, row-major order, clamp to
//! `total - start`, `ErrInvalidParam` + nothing written on a bad start/null).
//!
//! The implementation is already correct; these are the characterization + mutation guards the
//! parity suite cannot provide. Guard value, stated per test: remove `.min(total - start)` and the
//! over-cap test reads past the frame / miscounts; break the `idx % cols` / `idx / cols` walk and
//! the row-crossing test's glyph order fails; drop the `*out_written = 0` init and the rejected-
//! start cases leak a stale count.

use std::ffi::CString;
use std::ptr;

use gibson::ffi::*;

/// A 6x3 headless frame whose row 0 is "ABCDEF" and rows 1-2 are blank, so every cell in the first
/// row is a distinct 1-byte glyph and the rest are empty — a crisp row-major discriminator.
/// Fullscreen so the frame fills the full 6x3 canvas (Inline would trim to content height).
unsafe fn build_ctx() -> *mut GibsonContextOpaque {
    let mut ctx: *mut GibsonContextOpaque = ptr::null_mut();
    assert_eq!(
        gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, 6, 3, &mut ctx),
        GibsonStatus::Ok
    );
    let style = GibsonStyle::default();
    let mut col = ptr::null_mut();
    assert_eq!(gibson_node_box_col(&mut col), GibsonStatus::Ok);
    let text = CString::new("ABCDEF").unwrap();
    let mut txt = ptr::null_mut();
    assert_eq!(
        gibson_node_text(text.as_ptr(), &style, GIBSON_WRAP_NONE, &mut txt),
        GibsonStatus::Ok
    );
    assert_eq!(gibson_node_add_child(col, txt), GibsonStatus::Ok);
    assert_eq!(gibson_set_root_node(ctx, col), GibsonStatus::Ok);
    assert_eq!(gibson_render(ctx), GibsonStatus::Ok);
    ctx
}

unsafe fn single(ctx: *mut GibsonContextOpaque, x: u16, y: u16) -> GibsonCell {
    let mut c = GibsonCell::default();
    assert_eq!(gibson_get_frame_cell(ctx, x, y, &mut c), GibsonStatus::Ok);
    c
}

#[test]
fn bulk_zero_capacity_writes_nothing_and_reports_zero() {
    unsafe {
        let ctx = build_ctx();
        // A caller-visible canary: if the call touches the buffer, this changes.
        let mut buf = [GibsonCell::default(); 1];
        buf[0].glyph_len = 77;
        let mut written: usize = 123; // deliberately non-zero to prove it is reset
        assert_eq!(
            gibson_get_frame_cells(ctx, 0, 0, buf.as_mut_ptr(), 0, &mut written),
            GibsonStatus::Ok
        );
        assert_eq!(written, 0, "zero capacity must report zero written");
        assert_eq!(
            buf[0].glyph_len, 77,
            "zero capacity must not touch the output buffer"
        );
        gibson_destroy_context(ctx);
    }
}

#[test]
fn bulk_mid_frame_start_reads_row_major_across_the_row_boundary() {
    unsafe {
        let ctx = build_ctx();
        // Start at (4,0) — two from the end of row 0 — and read 4: E, F, then wrap into row 1 (blank).
        let mut buf = [GibsonCell::default(); 4];
        let mut written: usize = 0;
        assert_eq!(
            gibson_get_frame_cells(ctx, 4, 0, buf.as_mut_ptr(), 4, &mut written),
            GibsonStatus::Ok
        );
        assert_eq!(written, 4);

        // Row-major order, crossing the row-0 → row-1 boundary: row 0 carries letters, row 1 is
        // padded (spaces), so the two cells past the boundary must NOT be the next letters.
        assert_eq!(&buf[0].glyph[..buf[0].glyph_len as usize], b"E");
        assert_eq!(&buf[1].glyph[..buf[1].glyph_len as usize], b"F");
        assert_ne!(
            &buf[2].glyph[..buf[2].glyph_len as usize],
            b"A",
            "walk must cross into row 1, not continue reading row 0"
        );

        // And each bulk cell equals the single-cell readback at the row-major-walked coordinate —
        // this IS the ordering proof: a mis-walk would mismatch (0,1)/(1,1) against row-0 letters.
        let coords = [(4u16, 0u16), (5, 0), (0, 1), (1, 1)];
        for (i, &(x, y)) in coords.iter().enumerate() {
            let s = single(ctx, x, y);
            assert_eq!(
                buf[i].glyph_len, s.glyph_len,
                "glyph_len mismatch at ({x},{y})"
            );
            assert_eq!(
                &buf[i].glyph[..buf[i].glyph_len as usize],
                &s.glyph[..s.glyph_len as usize],
                "glyph mismatch at ({x},{y})"
            );
            assert_eq!(buf[i].width, s.width, "width mismatch at ({x},{y})");
        }
        gibson_destroy_context(ctx);
    }
}

#[test]
fn bulk_over_cap_clamps_to_cells_remaining() {
    unsafe {
        let ctx = build_ctx();
        // Start at the LAST cell (5,2) of a 6x3 frame: exactly one cell remains. Ask for 10.
        let mut buf = [GibsonCell::default(); 10];
        let mut written: usize = 0;
        assert_eq!(
            gibson_get_frame_cells(ctx, 5, 2, buf.as_mut_ptr(), 10, &mut written),
            GibsonStatus::Ok
        );
        assert_eq!(
            written, 1,
            "max_cells must clamp to total - start, not over-read the frame"
        );
        gibson_destroy_context(ctx);
    }
}

#[test]
fn bulk_out_of_bounds_start_is_rejected_and_writes_nothing() {
    unsafe {
        let ctx = build_ctx();
        let mut buf = [GibsonCell::default(); 4];
        // x == cols is out of bounds.
        let mut written: usize = 999;
        assert_eq!(
            gibson_get_frame_cells(ctx, 6, 0, buf.as_mut_ptr(), 4, &mut written),
            GibsonStatus::ErrInvalidParam
        );
        assert_eq!(written, 0, "a rejected start must report zero written");
        // y == rows is out of bounds.
        let mut written2: usize = 999;
        assert_eq!(
            gibson_get_frame_cells(ctx, 0, 3, buf.as_mut_ptr(), 4, &mut written2),
            GibsonStatus::ErrInvalidParam
        );
        assert_eq!(written2, 0);
        gibson_destroy_context(ctx);
    }
}

#[test]
fn bulk_null_pointers_are_rejected() {
    unsafe {
        let ctx = build_ctx();
        let mut buf = [GibsonCell::default(); 4];
        let mut written: usize = 0;
        // Null out_written.
        assert_eq!(
            gibson_get_frame_cells(ctx, 0, 0, buf.as_mut_ptr(), 4, ptr::null_mut()),
            GibsonStatus::ErrInvalidParam
        );
        // Null out buffer.
        assert_eq!(
            gibson_get_frame_cells(ctx, 0, 0, ptr::null_mut(), 4, &mut written),
            GibsonStatus::ErrInvalidParam
        );
        // Null ctx.
        assert_eq!(
            gibson_get_frame_cells(ptr::null_mut(), 0, 0, buf.as_mut_ptr(), 4, &mut written),
            GibsonStatus::ErrInvalidParam
        );
        gibson_destroy_context(ctx);
    }
}
