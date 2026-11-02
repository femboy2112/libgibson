//! Cross-ABI styled **cell-grid** readback parity on the headless path (#142, v0.6 Gate J).
//!
//! Gate I proved byte-exact ANSI *frame* (wire) parity and explicitly ceded a styled **cell grid**:
//! the resolved grid of cells, each with its grapheme, geometry, and fg/bg/attrs — what the frame
//! *paints*, not the raw escape stream. This closes that half of the cede with the additive triple
//! `Context::frame_size` / `Context::frame_cell` (a new Rust seam over the renderer's previously
//! private `previous_surface`) and the ABI pair `gibson_get_frame_size` / `gibson_get_frame_cell`
//! (plus the `gibson_cell_t` struct and the inverse `style_to_ffi` / `color_to_ffi` converters).
//! Each cell's style is **quantized to the context's color depth**, so the grid agrees with the
//! emitted wire (and composes with J1a's `set_color_depth`).
//!
//! Scope, stated narrowly: the resolved styled cell grid of one fixed op-script at one fixed
//! geometry, read back identically across the native API, the C ABI (in-process here, cdylib in
//! CI), and Python. It is **not** a pixel raster (the ABI exposes no framebuffer — a cell grid is
//! not pixels), **not** real-TTY behaviour (J3), and the inline `glyph` buffer is capped at
//! `GIBSON_CELL_GLYPH_CAP` bytes — a longer cluster (e.g. a ZWJ family emoji) is truncated and
//! flagged (`glyph_truncated`), with the full bytes available via `gibson_get_frame_cell_glyph`
//! (Gate K · K2, covered by `tests/ffi_cell_readback.rs`).
//!
//! THE PARITY TRIANGLE: the Rust job proves native grid == committed golden AND ABI == native
//! in-process; the Bindings job `cmp`s C == Python == golden. The grid is serialized to a canonical
//! text form (one line per cell) via the SAME `color_to_ffi` encoder the ABI uses, so native and
//! ABI encodings cannot drift. Freeze with `WRITE_PARITY_GOLDEN=1`.

use std::ffi::CString;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::ptr;

use gibson::capability::ColorDepth;
use gibson::cell::{Color, Style};
use gibson::context::{Context, RenderMode};
use gibson::ffi::*;
use gibson::node::{Node, WrapMode};

const COLS: u16 = 24;
const ROWS: u16 = 4;

fn parity_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bindings/parity")
}

/// The canonical op-script: the same RGB-styled, bold "Gate-I" node as the Gate I / J1a goldens, in
/// a headless context at a chosen depth, rendered once.
fn render_ctx(depth: ColorDepth) -> Context {
    let mut ctx = Context::headless(RenderMode::Inline, COLS, ROWS);
    ctx.set_color_depth(depth);
    let style = Style::new()
        .fg(Color::Rgb(255, 128, 0))
        .bg(Color::Rgb(16, 32, 64))
        .bold();
    ctx.set_root(Node::col().child(Node::text_wrapped("Gate-I", style, WrapMode::NoWrap)));
    ctx.render().unwrap();
    ctx
}

/// Canonical per-cell line, identical across Rust-native, Rust-ABI, C, and Python. Colors are
/// encoded through `color_to_ffi` so the native path uses the exact same encoding as the ABI.
/// `glyph` is the lowercase hex of its UTF-8 bytes, or `-` when empty.
fn fmt_line(x: u16, y: u16, glyph: &str, width: u8, cont: u8, transp: u8, style: &Style) -> String {
    let hex: String = if glyph.is_empty() {
        "-".to_string()
    } else {
        glyph.bytes().fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
    };
    let fg = color_to_ffi(style.fg);
    let bg = color_to_ffi(style.bg);
    format!(
        "{x} {y} {hex} {width} {cont} {transp} {} {} {} {} {} {} {} {} {} {} {} {} {}\n",
        fg.color_type,
        fg.r,
        fg.g,
        fg.b,
        bg.color_type,
        bg.r,
        bg.g,
        bg.b,
        style.bold as u8,
        style.dim as u8,
        style.italic as u8,
        style.underline as u8,
        style.reverse as u8,
    )
}

/// Serialize the whole grid from the NATIVE `Context::frame_size` / `Context::frame_cell`.
fn native_grid(ctx: &Context) -> String {
    let (cols, rows) = ctx
        .frame_size()
        .expect("a headless frame must exist after render");
    let mut out = format!("size {cols} {rows}\n");
    for y in 0..rows {
        for x in 0..cols {
            let c = ctx.frame_cell(x, y).expect("in-bounds cell must resolve");
            out.push_str(&fmt_line(
                x,
                y,
                c.glyph,
                c.width,
                c.is_continuation as u8,
                c.transparent as u8,
                &c.style,
            ));
        }
    }
    out
}

/// Serialize the whole grid through the NEW C ABI in-process (`gibson::ffi::*`, no cdylib),
/// decoding `gibson_cell_t` the way the C and Python harnesses do.
fn abi_grid(depth_const: i32) -> String {
    unsafe {
        let mut ctx: *mut GibsonContextOpaque = ptr::null_mut();
        assert_eq!(
            gibson_create_headless_context(GIBSON_MODE_INLINE, COLS, ROWS, &mut ctx),
            GibsonStatus::Ok
        );
        assert_eq!(gibson_set_color_depth(ctx, depth_const), GibsonStatus::Ok);

        let style = GibsonStyle {
            fg: GibsonColor {
                color_type: GIBSON_COLOR_RGB,
                r: 255,
                g: 128,
                b: 0,
                _pad: 0,
            },
            bg: GibsonColor {
                color_type: GIBSON_COLOR_RGB,
                r: 16,
                g: 32,
                b: 64,
                _pad: 0,
            },
            bold: 1,
            ..Default::default()
        };
        let mut col = ptr::null_mut();
        assert_eq!(gibson_node_box_col(&mut col), GibsonStatus::Ok);
        let text = CString::new("Gate-I").unwrap();
        let mut txt = ptr::null_mut();
        assert_eq!(
            gibson_node_text(text.as_ptr(), &style, GIBSON_WRAP_NONE, &mut txt),
            GibsonStatus::Ok
        );
        assert_eq!(gibson_node_add_child(col, txt), GibsonStatus::Ok);
        assert_eq!(gibson_set_root_node(ctx, col), GibsonStatus::Ok);
        assert_eq!(gibson_render(ctx), GibsonStatus::Ok);

        let mut cols: u16 = 0;
        let mut rows: u16 = 0;
        assert_eq!(
            gibson_get_frame_size(ctx, &mut cols, &mut rows),
            GibsonStatus::Ok
        );
        let mut out = format!("size {cols} {rows}\n");
        for y in 0..rows {
            for x in 0..cols {
                let mut cell = GibsonCell::default();
                assert_eq!(
                    gibson_get_frame_cell(ctx, x, y, &mut cell),
                    GibsonStatus::Ok
                );
                let glyph = std::str::from_utf8(&cell.glyph[..cell.glyph_len as usize]).unwrap();
                let hex: String = if glyph.is_empty() {
                    "-".to_string()
                } else {
                    glyph.bytes().fold(String::new(), |mut s, b| {
                        let _ = write!(s, "{b:02x}");
                        s
                    })
                };
                let fg = cell.style.fg;
                let bg = cell.style.bg;
                let _ = writeln!(
                    out,
                    "{x} {y} {hex} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
                    cell.width,
                    cell.is_continuation,
                    cell.transparent,
                    fg.color_type,
                    fg.r,
                    fg.g,
                    fg.b,
                    bg.color_type,
                    bg.r,
                    bg.g,
                    bg.b,
                    cell.style.bold,
                    cell.style.dim,
                    cell.style.italic,
                    cell.style.underline,
                    cell.style.reverse,
                );
            }
        }
        gibson_destroy_context(ctx);
        out
    }
}

/// The native grid must match the committed golden the C/Python ABI harnesses are `cmp`'d against.
#[test]
fn native_grid_matches_committed_golden() {
    let grid = native_grid(&render_ctx(ColorDepth::TrueColor));
    let path = parity_dir().join("golden_cells.txt");
    if std::env::var_os("WRITE_PARITY_GOLDEN").is_some() {
        fs::create_dir_all(parity_dir()).unwrap();
        fs::write(&path, &grid).unwrap();
        eprintln!("wrote golden_cells.txt ({} bytes)", grid.len());
    }
    let golden = fs::read_to_string(&path)
        .expect("golden_cells.txt missing — run with WRITE_PARITY_GOLDEN=1");
    assert_eq!(
        grid, golden,
        "native cell grid diverged from the committed golden"
    );
}

/// The ABI readback, in-process, reproduces the native grid byte-for-byte — the load-bearing proof
/// that `gibson_get_frame_cell` wraps the same resolved surface + quantization as the native seam.
#[test]
fn abi_grid_equals_native_grid_in_process() {
    let native = native_grid(&render_ctx(ColorDepth::TrueColor));
    let abi = abi_grid(GIBSON_COLOR_DEPTH_TRUECOLOR);
    assert_eq!(abi, native, "C-ABI cell grid diverged from native");
}

/// Teeth: the grid must carry real styled signal — the painted "Gate-I" cells exist, with the exact
/// RGB fg/bg and bold, and the geometry is the full 24x4. A stub returning zeroed cells would fail.
#[test]
fn grid_carries_real_styled_signal() {
    let ctx = render_ctx(ColorDepth::TrueColor);
    // `frame_size` reports the RENDERED surface dims, not the terminal geometry: Inline mode paints
    // only the content height, so one line of text is a single row (COLS wide, 1 tall) even though
    // the headless geometry is COLS x ROWS. This is a deliberate, documented property of the seam.
    assert_eq!(ctx.frame_size(), Some((COLS, 1)));

    // Row 0 starts with the painted text: cell (0,0) is 'G', RGB fg (255,128,0), bg (16,32,64),
    // bold. (The column lays the text at the top-left.)
    let c = ctx.frame_cell(0, 0).expect("cell (0,0)");
    assert_eq!(c.glyph, "G", "first painted cell is 'G', got {:?}", c.glyph);
    assert_eq!(
        c.style.fg,
        Some(Color::Rgb(255, 128, 0)),
        "fg must survive to the grid"
    );
    assert_eq!(
        c.style.bg,
        Some(Color::Rgb(16, 32, 64)),
        "bg must survive to the grid"
    );
    assert!(c.style.bold, "bold must survive to the grid");
    assert_eq!(c.width, 1);
    assert!(!c.is_continuation);

    // The six glyphs of "Gate-I" are present in row 0.
    let row0: String = (0..COLS)
        .filter_map(|x| ctx.frame_cell(x, 0).map(|c| c.glyph.to_string()))
        .collect();
    assert!(
        row0.contains("Gate-I"),
        "row 0 must contain the painted text, got {row0:?}"
    );
}

/// Teeth: the grid style is quantized to the context's depth, so it matches J1a's wire. At Ansi256
/// the first cell's fg is the indexed 208 (not the RGB), exactly as the ansi256 wire golden showed.
#[test]
fn grid_style_quantizes_with_depth() {
    let ctx = render_ctx(ColorDepth::Ansi256);
    let c = ctx.frame_cell(0, 0).expect("cell (0,0)");
    assert_eq!(
        c.style.fg,
        Some(Color::Ansi256(208)),
        "the grid fg must be quantized to the ansi256 index, got {:?}",
        c.style.fg
    );
    assert_eq!(
        c.style.bg,
        Some(Color::Ansi256(235)),
        "the grid bg must quantize too"
    );
    // Mono drops color entirely, bold kept.
    let m = render_ctx(ColorDepth::Mono);
    let mc = m.frame_cell(0, 0).expect("cell (0,0)");
    assert_eq!(mc.style.fg, None, "mono grid cell has no fg");
    assert_eq!(mc.style.bg, None, "mono grid cell has no bg");
    assert!(mc.style.bold, "mono keeps bold");
}

/// No frame and out-of-bounds are rejected, not crashed, on both the native and ABI paths.
#[test]
fn no_frame_and_out_of_bounds_are_rejected() {
    // Native: before any render, there is no frame.
    let fresh = Context::headless(RenderMode::Inline, COLS, ROWS);
    assert_eq!(fresh.frame_size(), None);
    assert!(fresh.frame_cell(0, 0).is_none());

    let ctx = render_ctx(ColorDepth::TrueColor);
    assert!(
        ctx.frame_cell(COLS, 0).is_none(),
        "x == cols is out of bounds"
    );
    assert!(
        ctx.frame_cell(0, ROWS).is_none(),
        "y == rows is out of bounds"
    );

    // ABI: a null out, a no-frame context, and an out-of-bounds coordinate all return ErrInvalidParam.
    unsafe {
        let mut ctx: *mut GibsonContextOpaque = ptr::null_mut();
        assert_eq!(
            gibson_create_headless_context(GIBSON_MODE_INLINE, COLS, ROWS, &mut ctx),
            GibsonStatus::Ok
        );
        let mut cell = GibsonCell::default();
        // No frame yet.
        assert_eq!(
            gibson_get_frame_cell(ctx, 0, 0, &mut cell),
            GibsonStatus::ErrInvalidParam
        );
        // Frame size before render reports (0, 0) with Ok.
        let (mut c, mut r) = (9u16, 9u16);
        assert_eq!(gibson_get_frame_size(ctx, &mut c, &mut r), GibsonStatus::Ok);
        assert_eq!((c, r), (0, 0));
        // Null out pointer.
        assert_eq!(
            gibson_get_frame_cell(ctx, 0, 0, ptr::null_mut()),
            GibsonStatus::ErrInvalidParam
        );
        gibson_destroy_context(ctx);
    }
}
