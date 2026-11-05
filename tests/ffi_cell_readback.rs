//! Gate K · K2 — honest cell-grid glyph readback across the C ABI.
//!
//! J1b's `gibson_get_frame_cell` copies a cell's grapheme into a fixed 16-byte buffer, truncating a
//! longer cluster at a codepoint boundary — but it gave the caller NO way to tell a truncated glyph
//! from an honestly short one, and the docs falsely claimed "every grapheme fits". A ZWJ family
//! emoji (👨‍👩‍👧‍👦) is 25 UTF-8 bytes, so the ABI grid silently disagreed with the emitted wire
//! (`src/ansi.rs` writes the whole cluster) for such a cell. This fences the fix:
//!   * the `glyph_truncated` flag (`gibson_cell_t._reserved[0]`),
//!   * the full-bytes escape hatch `gibson_get_frame_cell_glyph`, and
//!   * bulk readback `gibson_get_frame_cells` (one ABI call, same encoding as the single-cell fn).
//!
//! Headless (no TTY). The C/Python cross-ABI `cmp` of the SAME long frame runs in the Bindings CI
//! job against `bindings/parity/golden_cells_long.txt` (the half that can't run here — the cdylib
//! OOMs this box). Freeze the golden with `WRITE_PARITY_GOLDEN=1`.

use std::ffi::CString;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::ptr;

use gibson::cell::{Color, Style};
use gibson::context::{Context, RenderMode};
use gibson::ffi::*;
use gibson::node::{Node, WrapMode};

const COLS: u16 = 24;
const ROWS: u16 = 4;
/// One ZWJ grapheme cluster: four people joined by ZWJ. 25 UTF-8 bytes, display width 2.
const FAMILY: &str = "👨‍👩‍👧‍👦";

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bindings/parity/golden_cells_long.txt")
}

fn long_text() -> String {
    format!("a{FAMILY}Z")
}

fn long_style() -> Style {
    Style::new().fg(Color::Rgb(200, 50, 50)).bold()
}

fn hex(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        "-".to_string()
    } else {
        bytes.iter().fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
    }
}

fn style_fields(st: &GibsonStyle) -> String {
    format!(
        "{} {} {} {} {} {} {} {} {} {} {} {} {}",
        st.fg.color_type,
        st.fg.r,
        st.fg.g,
        st.fg.b,
        st.bg.color_type,
        st.bg.r,
        st.bg.g,
        st.bg.b,
        st.bold,
        st.dim,
        st.italic,
        st.underline,
        st.reverse,
    )
}

/// The canonical long-glyph line, identical across Rust-native, Rust-ABI, C and Python:
///   `<x> <y> <glyph_hex|-> <glyph_len> <trunc> <full_hex|-> <w> <cont> <transp> <style…>`
/// `glyph_hex` is the (possibly truncated) inline bytes; `full_hex` is the untruncated cluster.
#[allow(clippy::too_many_arguments)] // a faithful per-cell line needs every field
fn fmt_long(
    x: u16,
    y: u16,
    glyph: &[u8],
    trunc: u8,
    full: &[u8],
    width: u8,
    cont: u8,
    transp: u8,
    style: &GibsonStyle,
) -> String {
    format!(
        "{x} {y} {} {} {trunc} {} {width} {cont} {transp} {}\n",
        hex(glyph),
        glyph.len(),
        hex(full),
        style_fields(style),
    )
}

unsafe fn build_long_abi_ctx() -> *mut GibsonContextOpaque {
    let mut ctx: *mut GibsonContextOpaque = ptr::null_mut();
    assert_eq!(
        gibson_create_headless_context(GIBSON_MODE_INLINE, COLS, ROWS, &mut ctx),
        GibsonStatus::Ok
    );
    let style = GibsonStyle {
        fg: GibsonColor {
            color_type: GIBSON_COLOR_RGB,
            r: 200,
            g: 50,
            b: 50,
            _pad: 0,
        },
        bold: 1,
        ..Default::default()
    };
    let text = CString::new(long_text()).unwrap();
    let mut root = ptr::null_mut();
    let mut txt = ptr::null_mut();
    assert_eq!(gibson_node_box_col(&mut root), GibsonStatus::Ok);
    assert_eq!(
        gibson_node_text(text.as_ptr(), &style, GIBSON_WRAP_NONE, &mut txt),
        GibsonStatus::Ok
    );
    assert_eq!(gibson_node_add_child(root, txt), GibsonStatus::Ok);
    assert_eq!(gibson_set_root_node(ctx, root), GibsonStatus::Ok);
    assert_eq!(gibson_render(ctx), GibsonStatus::Ok);
    ctx
}

/// Pull the full cluster bytes via the size-query-then-copy escape hatch.
unsafe fn abi_full_glyph(ctx: *mut GibsonContextOpaque, x: u16, y: u16) -> Vec<u8> {
    let mut flen: usize = 0;
    assert_eq!(
        gibson_get_frame_cell_glyph(ctx, x, y, ptr::null_mut(), 0, &mut flen),
        GibsonStatus::Ok
    );
    if flen == 0 {
        return Vec::new();
    }
    let mut fbuf = vec![0u8; flen];
    let mut got = flen;
    assert_eq!(
        gibson_get_frame_cell_glyph(ctx, x, y, fbuf.as_mut_ptr(), flen, &mut got),
        GibsonStatus::Ok
    );
    assert_eq!(got, flen, "copied length must equal the queried length");
    fbuf
}

/// Serialize the whole frame through the NEW bulk ABI plus the glyph accessor — the exact form the
/// C and Python harnesses emit.
fn abi_long_grid() -> String {
    unsafe {
        let ctx = build_long_abi_ctx();
        let mut cols: u16 = 0;
        let mut rows: u16 = 0;
        assert_eq!(
            gibson_get_frame_size(ctx, &mut cols, &mut rows),
            GibsonStatus::Ok
        );
        let n = cols as usize * rows as usize;
        let mut cells = vec![GibsonCell::default(); n];
        let mut written: usize = 0;
        assert_eq!(
            gibson_get_frame_cells(ctx, 0, 0, cells.as_mut_ptr(), n, &mut written),
            GibsonStatus::Ok
        );
        assert_eq!(written, n, "bulk readback must fill the whole frame");

        let mut out = format!("size {cols} {rows}\n");
        for (i, c) in cells.iter().enumerate() {
            let x = (i % cols as usize) as u16;
            let y = (i / cols as usize) as u16;
            let full = abi_full_glyph(ctx, x, y);
            out.push_str(&fmt_long(
                x,
                y,
                &c.glyph[..c.glyph_len as usize],
                c._reserved[0],
                &full,
                c.width,
                c.is_continuation,
                c.transparent,
                &c.style,
            ));
        }
        gibson_destroy_context(ctx);
        out
    }
}

/// Serialize the same frame from the NATIVE API, applying the ABI's documented truncation rule so
/// the two paths must produce byte-identical output.
fn native_long_grid() -> String {
    let mut ctx = Context::headless(RenderMode::Inline, COLS, ROWS);
    ctx.set_root(Node::col().child(Node::text_wrapped(
        long_text(),
        long_style(),
        WrapMode::NoWrap,
    )));
    ctx.render().unwrap();
    let (cols, rows) = ctx.frame_size().expect("headless frame after render");
    let mut out = format!("size {cols} {rows}\n");
    for y in 0..rows {
        for x in 0..cols {
            let c = ctx.frame_cell(x, y).expect("in-bounds cell");
            let full = c.glyph.as_bytes();
            let mut n = full.len().min(GIBSON_CELL_GLYPH_CAP);
            while n > 0 && !c.glyph.is_char_boundary(n) {
                n -= 1;
            }
            let trunc = u8::from(full.len() > GIBSON_CELL_GLYPH_CAP);
            out.push_str(&fmt_long(
                x,
                y,
                &full[..n],
                trunc,
                full,
                c.width,
                c.is_continuation as u8,
                c.transparent as u8,
                &style_to_ffi(&c.style),
            ));
        }
    }
    out
}

/// The native API, the in-process C ABI, and the committed golden all serialize the long frame
/// byte-for-byte identically. `WRITE_PARITY_GOLDEN=1` (re)freezes the golden.
#[test]
fn native_abi_and_golden_agree_on_the_long_frame() {
    let abi = abi_long_grid();
    let native = native_long_grid();
    assert_eq!(
        native, abi,
        "native cell grid diverged from the ABI readback\n--- native ---\n{native}\n--- abi ---\n{abi}"
    );

    if std::env::var("WRITE_PARITY_GOLDEN").is_ok() {
        fs::write(golden_path(), &abi).expect("write golden_cells_long.txt");
        return;
    }
    let golden = fs::read_to_string(golden_path()).expect("read golden_cells_long.txt");
    assert_eq!(
        abi, golden,
        "ABI readback diverged from the committed golden (re-freeze with WRITE_PARITY_GOLDEN=1)"
    );
}

/// Bulk `gibson_get_frame_cells` is byte-identical, cell for cell, to a `gibson_get_frame_cell`
/// loop — the whole point of the shared `encode_cell`.
#[test]
fn bulk_matches_single_cell() {
    unsafe {
        let ctx = build_long_abi_ctx();
        let mut cols: u16 = 0;
        let mut rows: u16 = 0;
        assert_eq!(
            gibson_get_frame_size(ctx, &mut cols, &mut rows),
            GibsonStatus::Ok
        );
        let n = cols as usize * rows as usize;
        let mut bulk = vec![GibsonCell::default(); n];
        let mut written: usize = 0;
        assert_eq!(
            gibson_get_frame_cells(ctx, 0, 0, bulk.as_mut_ptr(), n, &mut written),
            GibsonStatus::Ok
        );
        assert_eq!(written, n);
        for (i, b) in bulk.iter().enumerate() {
            let x = (i % cols as usize) as u16;
            let y = (i / cols as usize) as u16;
            let mut single = GibsonCell::default();
            assert_eq!(
                gibson_get_frame_cell(ctx, x, y, &mut single),
                GibsonStatus::Ok
            );
            assert_eq!(b.glyph, single.glyph, "glyph mismatch at ({x},{y})");
            assert_eq!(
                b.glyph_len, single.glyph_len,
                "glyph_len mismatch at ({x},{y})"
            );
            assert_eq!(b.width, single.width, "width mismatch at ({x},{y})");
            assert_eq!(b.is_continuation, single.is_continuation);
            assert_eq!(b.transparent, single.transparent);
            assert_eq!(b._reserved, single._reserved, "flags mismatch at ({x},{y})");
            assert_eq!(b.style.fg.color_type, single.style.fg.color_type);
            assert_eq!(b.style.bold, single.style.bold);
        }
        // A bounded bulk read returns exactly max_cells, and a row-start offset works.
        let mut just_two = [GibsonCell::default(); 2];
        let mut w2: usize = 0;
        assert_eq!(
            gibson_get_frame_cells(ctx, 0, 0, just_two.as_mut_ptr(), 2, &mut w2),
            GibsonStatus::Ok
        );
        assert_eq!(w2, 2, "a capped bulk read must stop at max_cells");
        gibson_destroy_context(ctx);
    }
}

/// The `glyph_truncated` flag is exact, and the escape hatch returns the FULL cluster: the family
/// emoji's lead cell is truncated (flag 1, <=16 inline bytes) yet `gibson_get_frame_cell_glyph`
/// yields all 25 bytes; the plain ASCII cells are untruncated (flag 0).
#[test]
fn glyph_truncated_flag_and_full_bytes_are_exact() {
    unsafe {
        let ctx = build_long_abi_ctx();
        // Row 0: 'a' at x=0, the family emoji lead at x=1 (width 2) + its continuation at x=2,
        // 'Z' at x=3, blanks after.
        let mut a = GibsonCell::default();
        assert_eq!(gibson_get_frame_cell(ctx, 0, 0, &mut a), GibsonStatus::Ok);
        assert_eq!(a.glyph_len, 1);
        assert_eq!(&a.glyph[..1], b"a");
        assert_eq!(a._reserved[0], 0, "a plain ASCII cell is not truncated");

        let mut fam = GibsonCell::default();
        assert_eq!(gibson_get_frame_cell(ctx, 1, 0, &mut fam), GibsonStatus::Ok);
        assert_eq!(fam.width, 2, "a ZWJ family emoji is a width-2 glyph");
        assert_eq!(
            fam._reserved[0], 1,
            "a 25-byte cluster must be flagged truncated"
        );
        assert!(
            (fam.glyph_len as usize) <= GIBSON_CELL_GLYPH_CAP,
            "the inline buffer holds at most the cap"
        );

        let full = abi_full_glyph(ctx, 1, 0);
        assert_eq!(
            full,
            FAMILY.as_bytes(),
            "the escape hatch must return the whole 25-byte cluster"
        );
        assert_eq!(full.len(), 25);

        gibson_destroy_context(ctx);
    }
}

/// The glyph accessor is size-query-first: a null/short buffer reports the full length and writes
/// nothing; an exact buffer copies. Mirrors `gibson_get_rendered_output`.
#[test]
fn glyph_accessor_is_size_query_first() {
    unsafe {
        let ctx = build_long_abi_ctx();
        // Size query (null buf).
        let mut len: usize = 0;
        assert_eq!(
            gibson_get_frame_cell_glyph(ctx, 1, 0, ptr::null_mut(), 0, &mut len),
            GibsonStatus::Ok
        );
        assert_eq!(len, 25, "size query must report the full cluster length");

        // Too-small cap: length reported, buffer untouched (sentinel preserved).
        let mut small = [0xAAu8; 8];
        let mut len2: usize = 0;
        assert_eq!(
            gibson_get_frame_cell_glyph(ctx, 1, 0, small.as_mut_ptr(), small.len(), &mut len2),
            GibsonStatus::Ok
        );
        assert_eq!(len2, 25, "a too-small cap still reports the full length");
        assert_eq!(
            small, [0xAAu8; 8],
            "a too-small cap must not write any bytes"
        );

        gibson_destroy_context(ctx);
    }
}
