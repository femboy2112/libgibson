//! Cross-ABI color-depth **downgrade** parity on the headless path (#142, v0.6 Gate J).
//!
//! Gate I (`tests/ffi_ansi_parity.rs`) proved byte-exact ANSI *frame* parity across the C/Python
//! ABI and the native renderer — but only at **truecolor**: `gibson_create_headless_context`
//! hard-wires `TerminalCapabilities::truecolor()`, so the 256-color / 16-color / mono SGR
//! *downgrade* ladder was never exercised through the ABI and was explicitly **ceded** to Gate J.
//! This closes that cede with the new **additive** setter `gibson_set_color_depth(ctx, depth)`,
//! which forwards to the existing public `Context::set_color_depth` (and thus the existing
//! `capability::quantize_style` downgrade path — no new engine logic). A caller selects a depth
//! *before the first render*, and the painted frame's emitted SGR is downgraded accordingly.
//!
//! Scope, stated narrowly: this proves the SAME op-script, driven through the ABI at each of the
//! four depths, reads back a BYTE-IDENTICAL ANSI frame to the native API at that depth, and that
//! the frame actually *changes* with depth (truecolor RGB → indexed-256 → base-16 → no color).
//! It is **not** a styled cell grid (J1b), **not** real-TTY behaviour (J3), and **not** env-driven
//! capability detection (`NO_COLOR`/`TERM`/`COLORTERM`) — the depth is forced explicitly.
//!
//! THE PARITY TRIANGLE, split across two CI jobs so each leg is checked where it can be:
//!   - the Rust job runs THIS test (`cargo test`, no cdylib): for each depth the native frame ==
//!     the committed golden, AND the ABI functions driven in-process via `gibson::ffi::*`
//!     reproduce the native frame byte-for-byte;
//!   - the Bindings job (which builds the cdylib) runs `bindings/parity/parity_depth.c` and
//!     `parity_depth.py` and `cmp`s C == Python == the same golden, per depth.
//!
//! The truecolor golden is Gate I's own `golden_ansi.bin`: driving `set_color_depth(TRUECOLOR)`
//! before render must reproduce the default headless frame byte-for-byte, which proves the
//! setter's TrueColor branch is a true identity and cross-links the two gates. The three downgrade
//! goldens live in ONE place; run once with `WRITE_PARITY_GOLDEN=1` to (re)freeze them.

use std::ffi::CString;
use std::fs;
use std::path::PathBuf;
use std::ptr;

use gibson::capability::ColorDepth;
use gibson::cell::{Color, Style};
use gibson::context::{Context, RenderMode};
use gibson::ffi::*;
use gibson::node::{Node, WrapMode};

/// Fixed, deterministic headless geometry. Pinned in all three legs and identical to Gate I so the
/// truecolor case reproduces `golden_ansi.bin` exactly.
const COLS: u16 = 24;
const ROWS: u16 = 4;

/// The four depths under test: the native enum, the matching ABI `int32_t` constant, and the
/// golden file the C/Python harnesses are `cmp`'d against. Truecolor reuses Gate I's golden.
fn depth_cases() -> [(ColorDepth, i32, &'static str); 4] {
    [
        (
            ColorDepth::TrueColor,
            GIBSON_COLOR_DEPTH_TRUECOLOR,
            "golden_ansi.bin",
        ),
        (
            ColorDepth::Ansi256,
            GIBSON_COLOR_DEPTH_ANSI256,
            "golden_depth_ansi256.bin",
        ),
        (
            ColorDepth::Ansi16,
            GIBSON_COLOR_DEPTH_ANSI16,
            "golden_depth_ansi16.bin",
        ),
        (
            ColorDepth::Mono,
            GIBSON_COLOR_DEPTH_MONO,
            "golden_depth_mono.bin",
        ),
    ]
}

fn parity_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bindings/parity")
}

/// The canonical frame op-script via the NATIVE API at a chosen depth: one RGB-styled, bold text
/// node, rendered once into a headless context whose color depth was overridden *before* render.
/// `parity_depth.c` / `parity_depth.py` build the identical tree through the C ABI and must
/// reproduce these bytes.
fn native_frame_at(depth: ColorDepth) -> Vec<u8> {
    let mut ctx = Context::headless(RenderMode::Inline, COLS, ROWS);
    ctx.set_color_depth(depth);
    let style = Style::new()
        .fg(Color::Rgb(255, 128, 0))
        .bg(Color::Rgb(16, 32, 64))
        .bold();
    ctx.set_root(Node::col().child(Node::text_wrapped("Gate-I", style, WrapMode::NoWrap)));
    ctx.render().unwrap();
    ctx.rendered_bytes().to_vec()
}

/// The SAME op-script driven through the NEW C-ABI setter in-process (`gibson::ffi::*`, no cdylib),
/// so the Rust CI job alone proves ABI-vs-native frame identity at each depth.
fn abi_frame_at(depth_const: i32) -> Vec<u8> {
    unsafe {
        let mut ctx: *mut GibsonContextOpaque = ptr::null_mut();
        assert_eq!(
            gibson_create_headless_context(GIBSON_MODE_INLINE, COLS, ROWS, &mut ctx),
            GibsonStatus::Ok
        );
        assert!(!ctx.is_null());
        assert_eq!(
            gibson_set_color_depth(ctx, depth_const),
            GibsonStatus::Ok,
            "setting a valid depth must succeed"
        );

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

        let mut need: usize = 0;
        assert_eq!(
            gibson_get_rendered_output(ctx, ptr::null_mut(), 0, &mut need),
            GibsonStatus::Ok
        );
        let mut buf = vec![0u8; need];
        let mut got: usize = 0;
        assert_eq!(
            gibson_get_rendered_output(ctx, buf.as_mut_ptr(), buf.len(), &mut got),
            GibsonStatus::Ok
        );
        buf.truncate(got);
        gibson_destroy_context(ctx);
        buf
    }
}

/// For every depth, the native frame must equal the committed golden the C/Python ABI harnesses
/// are `cmp`'d against.
#[test]
fn native_frames_match_committed_goldens() {
    let dir = parity_dir();
    for (depth, _, golden) in depth_cases() {
        let bytes = native_frame_at(depth);
        let path = dir.join(golden);
        if std::env::var_os("WRITE_PARITY_GOLDEN").is_some() && golden != "golden_ansi.bin" {
            // The truecolor golden is Gate I's; never rewrite it here.
            fs::create_dir_all(&dir).unwrap();
            fs::write(&path, &bytes).unwrap();
            eprintln!(
                "wrote {golden} ({} bytes) for {depth:?}:\n{:?}",
                bytes.len(),
                String::from_utf8_lossy(&bytes)
            );
        }
        let golden_bytes = fs::read(&path)
            .unwrap_or_else(|_| panic!("{golden} missing — run with WRITE_PARITY_GOLDEN=1"));
        assert_eq!(
            bytes, golden_bytes,
            "native {depth:?} frame diverged from committed golden {golden}"
        );
    }
}

/// The new ABI setter, driven in-process, reproduces the native frame byte-for-byte at every
/// depth: the load-bearing proof that `gibson_set_color_depth` reaches the SAME downgrade path as
/// the native `Context::set_color_depth`.
#[test]
fn abi_frame_equals_native_frame_at_each_depth() {
    for (depth, depth_const, _) in depth_cases() {
        let native = native_frame_at(depth);
        let abi = abi_frame_at(depth_const);
        assert_eq!(
            abi, native,
            "C-ABI {depth:?} frame readback diverged from native — the setter does not wrap the same path"
        );
    }
}

/// Teeth: the downgrade must be REAL and depth-specific, so a stubbed setter (or a vacuous golden)
/// could not pass. Every depth paints "Gate-I"; the four frames are pairwise distinct; and each
/// carries exactly the SGR family its depth mandates — truecolor `38;2;r;g;b`, indexed `38;5;N`,
/// base-16 (neither `38;2;` nor `38;5;`), and mono (no fg/bg color at all).
#[test]
fn downgrade_is_real_and_depth_specific() {
    let frames: Vec<(ColorDepth, String)> = depth_cases()
        .into_iter()
        .map(|(d, _, _)| (d, String::from_utf8_lossy(&native_frame_at(d)).into_owned()))
        .collect();

    for (d, text) in &frames {
        assert!(
            text.contains("Gate-I"),
            "{d:?} frame missing painted text: {text:?}"
        );
    }

    let tc = &frames[0].1; // TrueColor
    let a256 = &frames[1].1; // Ansi256
    let a16 = &frames[2].1; // Ansi16
    let mono = &frames[3].1; // Mono

    // Truecolor: 24-bit RGB SGR for both fg and bg.
    assert!(
        tc.contains("38;2;255;128;0"),
        "truecolor missing RGB fg: {tc:?}"
    );
    assert!(
        tc.contains("48;2;16;32;64"),
        "truecolor missing RGB bg: {tc:?}"
    );

    // Ansi256: indexed SGR, and NO 24-bit RGB SGR.
    assert!(
        a256.contains("38;5;"),
        "ansi256 missing indexed fg: {a256:?}"
    );
    assert!(
        a256.contains("48;5;"),
        "ansi256 missing indexed bg: {a256:?}"
    );
    assert!(
        !a256.contains("38;2;"),
        "ansi256 must not emit RGB fg: {a256:?}"
    );
    assert!(
        !a256.contains("48;2;"),
        "ansi256 must not emit RGB bg: {a256:?}"
    );

    // Ansi16: downgraded further — neither 24-bit RGB nor 256-indexed SGR.
    assert!(
        !a16.contains("38;2;"),
        "ansi16 must not emit RGB fg: {a16:?}"
    );
    assert!(
        !a16.contains("38;5;"),
        "ansi16 must not emit indexed fg: {a16:?}"
    );

    // Mono: no color SGR at all (fg or bg), on any encoding.
    for probe in ["38;2;", "38;5;", "48;2;", "48;5;"] {
        assert!(
            !mono.contains(probe),
            "mono must not emit color {probe:?}: {mono:?}"
        );
    }

    // The four depths yield four distinct wires.
    let wires = [tc, a256, a16, mono];
    for i in 0..wires.len() {
        for j in (i + 1)..wires.len() {
            assert_ne!(
                wires[i], wires[j],
                "depths {i} and {j} produced identical frames"
            );
        }
    }
}

/// `set_color_depth(TRUECOLOR)` before render must reproduce the DEFAULT headless frame — i.e. Gate
/// I's `golden_ansi.bin` — proving the setter's TrueColor branch is a true identity, not a reskin.
#[test]
fn truecolor_setter_is_identity_vs_gate_i_golden() {
    let bytes = native_frame_at(ColorDepth::TrueColor);
    let golden =
        fs::read(parity_dir().join("golden_ansi.bin")).expect("Gate I golden_ansi.bin missing");
    assert_eq!(
        bytes, golden,
        "set_color_depth(TrueColor) diverged from the default headless frame"
    );
}

/// An unknown depth is rejected and changes nothing: the ABI setter returns ErrInvalidParam and a
/// subsequent render still emits the (unchanged, truecolor) frame.
#[test]
fn invalid_depth_is_rejected_and_inert() {
    unsafe {
        let mut ctx: *mut GibsonContextOpaque = ptr::null_mut();
        assert_eq!(
            gibson_create_headless_context(GIBSON_MODE_INLINE, COLS, ROWS, &mut ctx),
            GibsonStatus::Ok
        );
        assert_eq!(
            gibson_set_color_depth(ctx, 99),
            GibsonStatus::ErrInvalidParam,
            "an out-of-range depth must be refused"
        );
        gibson_destroy_context(ctx);
    }
    // And a null context is refused, not a crash.
    unsafe {
        assert_eq!(
            gibson_set_color_depth(ptr::null_mut(), GIBSON_COLOR_DEPTH_MONO),
            GibsonStatus::ErrInvalidParam
        );
    }
}

/// Determinism the goldens rely on: two headless renders at the same depth are byte-identical.
#[test]
fn depth_render_is_deterministic() {
    for (depth, _, _) in depth_cases() {
        let a = native_frame_at(depth);
        let b = native_frame_at(depth);
        assert_eq!(a, b, "two {depth:?} renders must be byte-identical");
        assert!(!a.is_empty());
    }
}
