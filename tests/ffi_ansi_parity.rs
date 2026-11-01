//! Cross-ABI pixel/ANSI **frame** parity on the headless path (#142, v0.6 Gate I).
//!
//! Gate H (`tests/ffi_stats_parity.rs`) proved stats+wire parity on the NON-TTY plain-scrollback
//! path and explicitly **ceded** pixel/ANSI parity: the stock ABI (`gibson_create_context` ==
//! `Context::new`) samples `stdout().is_terminal()` at create time, so in CI (a pipe) it runs the
//! non-TTY branch and `render` is a suppressed no-op — `frames` only proves the call reached the
//! scheduler, and nothing is painted. This gate closes that cede with the new **additive** ABI
//! pair `gibson_create_headless_context` + `gibson_get_rendered_output`: a deterministic in-memory
//! headless context (always TTY semantics, truecolor, fixed geometry, no terminal lease) whose
//! painted ANSI frame the caller reads back byte-for-byte.
//!
//! Scope, stated narrowly: this proves the C and Python bindings marshal a *styled node tree* into
//! the SAME renderer and read back a BYTE-IDENTICAL ANSI frame stream (SGR truecolor, cursor
//! motion, sync markers, diff output) plus identical *painted* stats, for one fixed op-script and
//! fixed headless geometry. It is **not** a pixel raster (the ABI exposes no framebuffer), **not**
//! a styled cell grid, **not** real-TTY behaviour (headless forces `is_tty = true` + truecolor + a
//! Buffer sink, so `Context::new` / the crossterm size query / the terminal lease / raw mode are
//! not exercised — that path stays covered by the Gate H non-TTY leg), and **not** full language
//! feature parity. `last_render_duration_micros` is excluded as a timing quantity (L10).
//!
//! THE PARITY TRIANGLE, split across two CI jobs so each leg is checked where it can be:
//!   - the Rust job runs THIS test (`cargo test`, no cdylib): the native frame bytes + painted
//!     stats == the committed golden (`bindings/parity/golden_ansi.bin`, `golden_ansi_stats.txt`),
//!     AND the new ABI functions, driven in-process via `gibson::ffi::*`, reproduce the native
//!     frame byte-for-byte;
//!   - the Bindings job (which builds the cdylib) runs `bindings/parity/parity_ansi.c` and
//!     `parity_ansi.py` and `cmp`s C == Python == the same golden.
//!
//! The golden lives in ONE place, so there is no triple-hardcode drift. Run once with
//! `WRITE_PARITY_GOLDEN=1` to (re)freeze it, inspect it, commit it; thereafter this is a pure
//! assertion.

use std::ffi::CString;
use std::fs;
use std::path::PathBuf;
use std::ptr;

use gibson::cell::{Color, Style};
use gibson::context::{Context, RenderMode};
use gibson::ffi::*;
use gibson::node::{Node, WrapMode};

/// Fixed, deterministic headless geometry. Pinned in all three legs.
const COLS: u16 = 24;
const ROWS: u16 = 4;

fn parity_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bindings/parity")
}

/// Canonical 13-field stats report (gibson_stats_t order, EXCLUDING the timing field
/// `last_render_duration_micros`). Byte-identical shape to `parity_ansi.c` / `parity_ansi.py`.
fn stats_report(ctx: &Context) -> String {
    let s = ctx.stats();
    format!(
        "frames={}\n\
         skipped_frames={}\n\
         dirty_cells={}\n\
         total_cells={}\n\
         frame_bytes={}\n\
         full_repaints={}\n\
         history_insertions={}\n\
         insertion_repaints={}\n\
         fast_insertions={}\n\
         insertion_bytes={}\n\
         anchor_resyncs={}\n\
         commit_bytes={}\n\
         control_bytes={}\n",
        s.frames,
        s.skipped_frames,
        s.dirty_cells,
        s.total_cells,
        s.frame_bytes,
        s.full_repaints,
        s.history_insertions,
        s.insertion_repaints,
        s.fast_insertions,
        s.insertion_bytes,
        s.anchor_resyncs,
        s.commit_bytes,
        s.control_bytes,
    )
}

/// The canonical frame op-script via the NATIVE API: one RGB-styled, bold text node in a column,
/// rendered once into a headless (TTY-semantics, Buffer-backed) context. Returns the raw painted
/// ANSI frame bytes and the painted-stats report. `parity_ansi.c` and `parity_ansi.py` build the
/// identical tree through the C ABI and must reproduce both outputs.
fn native_frame() -> (Vec<u8>, String) {
    let mut ctx = Context::headless(RenderMode::Inline, COLS, ROWS);
    let style = Style::new()
        .fg(Color::Rgb(255, 128, 0))
        .bg(Color::Rgb(16, 32, 64))
        .bold();
    ctx.set_root(Node::col().child(Node::text_wrapped("Gate-I", style, WrapMode::NoWrap)));
    ctx.render().unwrap();
    (ctx.rendered_bytes().to_vec(), stats_report(&ctx))
}

/// The SAME op-script driven through the NEW C-ABI functions in-process (`gibson::ffi::*`, no
/// cdylib), so the Rust CI job alone proves ABI-vs-native frame identity even though the
/// cross-language `cmp` runs in the Bindings job.
fn abi_frame_in_process() -> Vec<u8> {
    unsafe {
        let mut ctx: *mut GibsonContextOpaque = ptr::null_mut();
        assert_eq!(
            gibson_create_headless_context(GIBSON_MODE_INLINE, COLS, ROWS, &mut ctx),
            GibsonStatus::Ok
        );
        assert!(!ctx.is_null());

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

        // Size-query form: null buffer returns OK and the needed length, copies nothing.
        let mut need: usize = 123;
        assert_eq!(
            gibson_get_rendered_output(ctx, ptr::null_mut(), 0, &mut need),
            GibsonStatus::Ok
        );
        assert!(
            need > 0,
            "a painted headless frame must have bytes to read back"
        );

        // An undersized buffer must NOT be written: OK, need unchanged, sentinel intact.
        let mut tiny = [0xABu8; 1];
        let mut need2: usize = 0;
        assert_eq!(
            gibson_get_rendered_output(ctx, tiny.as_mut_ptr(), 1, &mut need2),
            GibsonStatus::Ok
        );
        assert_eq!(
            need2, need,
            "size query must report the full length even when cap is too small"
        );
        assert_eq!(tiny[0], 0xAB, "an undersized buffer must be left untouched");

        // Real read.
        let mut buf = vec![0u8; need];
        let mut got: usize = 0;
        assert_eq!(
            gibson_get_rendered_output(ctx, buf.as_mut_ptr(), buf.len(), &mut got),
            GibsonStatus::Ok
        );
        assert_eq!(got, need);
        buf.truncate(got);

        gibson_destroy_context(ctx);
        buf
    }
}

/// The native frame + painted stats must match the committed golden that the C and Python ABI
/// harnesses are `cmp`'d against.
#[test]
fn native_ansi_frame_matches_the_committed_golden() {
    let (bytes, report) = native_frame();
    let dir = parity_dir();
    let bin_path = dir.join("golden_ansi.bin");
    let stats_path = dir.join("golden_ansi_stats.txt");

    if std::env::var_os("WRITE_PARITY_GOLDEN").is_some() {
        fs::create_dir_all(&dir).unwrap();
        fs::write(&bin_path, &bytes).unwrap();
        fs::write(&stats_path, &report).unwrap();
        eprintln!(
            "wrote golden ansi frame ({} bytes):\n{:?}\n--- stats ---\n{report}",
            bytes.len(),
            String::from_utf8_lossy(&bytes)
        );
    }

    let golden_bytes = fs::read(&bin_path).expect(
        "bindings/parity/golden_ansi.bin missing — run with WRITE_PARITY_GOLDEN=1 to create it",
    );
    let golden_stats =
        fs::read_to_string(&stats_path).expect("bindings/parity/golden_ansi_stats.txt missing");

    assert_eq!(
        bytes, golden_bytes,
        "native painted ANSI frame diverged from the committed golden — the renderer's byte output changed"
    );
    assert_eq!(
        report, golden_stats,
        "native painted stats diverged from the committed golden"
    );
}

/// Teeth: the golden must carry real painted signal, so a stub binding returning zeros (or an
/// empty stream) could not pass the cross-ABI `cmp` vacuously. Unlike the Gate H non-TTY golden
/// (where most stat fields are legitimately zero because render was a no-op), the headless TTY
/// path actually paints, so `dirty_cells`/`total_cells`/`frame_bytes` are non-zero AND the stream
/// carries truecolor SGR for the RGB style.
#[test]
fn the_ansi_golden_carries_real_painted_signal() {
    let (bytes, report) = native_frame();
    let text = String::from_utf8_lossy(&bytes);

    assert!(!bytes.is_empty(), "the painted frame must not be empty");
    assert!(
        text.contains("Gate-I"),
        "the painted frame must carry the rendered text, got {text:?}"
    );
    // Truecolor SGR for the fg Rgb(255,128,0) and bg Rgb(16,32,64) proves the STYLE marshalled all
    // the way to the wire — the thing a divergent binding would get wrong.
    assert!(
        text.contains("38;2;255;128;0"),
        "missing truecolor fg SGR in the frame: {text:?}"
    );
    assert!(
        text.contains("48;2;16;32;64"),
        "missing truecolor bg SGR in the frame: {text:?}"
    );
    // The painted-stats weakness from Gate H is retired: these are non-zero on the TTY path.
    for zero in ["dirty_cells=0\n", "total_cells=0\n", "frame_bytes=0\n"] {
        assert!(
            !report.contains(zero),
            "a painted headless frame must have non-zero {zero:?}:\n{report}"
        );
    }
}

/// The new ABI functions, driven in-process, reproduce the native frame byte-for-byte. This is the
/// load-bearing proof that `gibson_create_headless_context` + `gibson_get_rendered_output` wrap the
/// SAME renderer pipeline as the native API — provable in the Rust job without a cdylib.
#[test]
fn abi_frame_equals_native_frame_in_process() {
    let (native, _) = native_frame();
    let abi = abi_frame_in_process();
    assert_eq!(
        abi, native,
        "the C-ABI headless frame readback diverged from the native frame — the ABI does not wrap the same pipeline"
    );
}

/// The determinism the parity golden relies on: two independent headless contexts rendering the
/// same tree emit byte-identical frames. (No existing test rendered the same tree twice and
/// compared raw bytes.)
#[test]
fn headless_ansi_render_is_deterministic() {
    let a = native_frame().0;
    let b = native_frame().0;
    assert_eq!(
        a, b,
        "two headless renders of the same tree must be byte-identical"
    );
    assert!(!a.is_empty());
}
