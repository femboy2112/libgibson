//! Child fixture for `tests/ffi_realtty_pty.rs` (#142, v0.6 Gate J). Not a demo — a patient on the
//! table. Gate I's headless ABI pair forces `is_tty = true` + truecolor + a Buffer sink, so the
//! REAL-TTY construction path through the C ABI (`gibson_create_context` → `Context::new` →
//! `TerminalSession::new` → crossterm → alternate screen → the stdout sink → restore) was never
//! exercised by any ABI test. A real `is_tty` needs a real terminal, so this can only run from a
//! process whose stdout is a PTY slave.
//!
//! Each scenario drives ONE deterministic op-script — the same styled node tree — either through
//! the C ABI (`abi`) or the native API (`native`), and exits 0 on success. It emits NO stdout
//! markers: the terminal wire (alt-screen enter, the painted frame, the restore tail) IS the
//! observable, and the parent compares the two scenarios' wires byte-for-byte. The lease contract
//! (Owned after create, Available after destroy) is asserted internally and reported via the exit
//! code, not the wire, so it does not perturb the parity comparison.

use std::ffi::CString;
use std::ptr;

use gibson::cell::{Color, Style};
use gibson::context::{Context, RenderMode};
use gibson::ffi::*;
use gibson::node::{Node, WrapMode};
use gibson::{TerminalLease, TerminalSession};

const TEXT: &str = "J3-RealTTY";

fn styled_node_native() -> Node {
    let style = Style::new().fg(Color::Rgb(0, 200, 100)).bold();
    Node::col().child(Node::text_wrapped(TEXT, style, WrapMode::NoWrap))
}

/// Native real-TTY path: `Context::new(Fullscreen)` acquires the terminal lease, enters the
/// alternate screen, paints one frame to stdout, and restores on drop.
fn native() {
    assert_eq!(
        TerminalSession::lease_state(),
        TerminalLease::Available,
        "a fresh process must start with the lease free"
    );
    let mut ctx = Context::new(RenderMode::Fullscreen).expect("Context::new on a real TTY");
    assert_eq!(
        TerminalSession::lease_state(),
        TerminalLease::Owned,
        "native create on a real TTY must own the lease"
    );
    ctx.set_root(styled_node_native());
    ctx.render().expect("render on a real TTY");
    drop(ctx); // restore
    assert_eq!(
        TerminalSession::lease_state(),
        TerminalLease::Available,
        "native drop must release the lease"
    );
    std::process::exit(0);
}

/// The SAME op-script through the C ABI: `gibson_create_context(FULLSCREEN)` hits exactly the same
/// `Context::new` real-TTY path, and `gibson_destroy_context` restores. This is the path Gate I
/// could not cover headlessly.
fn abi() {
    unsafe {
        assert_eq!(
            TerminalSession::lease_state(),
            TerminalLease::Available,
            "a fresh process must start with the lease free"
        );
        let mut ctx: *mut GibsonContextOpaque = ptr::null_mut();
        assert_eq!(
            gibson_create_context(GIBSON_MODE_FULLSCREEN, &mut ctx),
            GibsonStatus::Ok,
            "gibson_create_context on a real TTY must succeed"
        );
        assert!(!ctx.is_null());
        assert_eq!(
            TerminalSession::lease_state(),
            TerminalLease::Owned,
            "ABI create on a real TTY must own the lease"
        );

        let style = GibsonStyle {
            fg: GibsonColor {
                color_type: GIBSON_COLOR_RGB,
                r: 0,
                g: 200,
                b: 100,
                _pad: 0,
            },
            bold: 1,
            ..Default::default()
        };
        let mut col = ptr::null_mut();
        assert_eq!(gibson_node_box_col(&mut col), GibsonStatus::Ok);
        let text = CString::new(TEXT).unwrap();
        let mut txt = ptr::null_mut();
        assert_eq!(
            gibson_node_text(text.as_ptr(), &style, GIBSON_WRAP_NONE, &mut txt),
            GibsonStatus::Ok
        );
        assert_eq!(gibson_node_add_child(col, txt), GibsonStatus::Ok);
        assert_eq!(gibson_set_root_node(ctx, col), GibsonStatus::Ok);
        assert_eq!(gibson_render(ctx), GibsonStatus::Ok);

        gibson_destroy_context(ctx);
        assert_eq!(
            TerminalSession::lease_state(),
            TerminalLease::Available,
            "ABI destroy must release the lease"
        );
        std::process::exit(0);
    }
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("abi") => abi(),
        Some("native") => native(),
        other => {
            eprintln!("unknown ffi-realtty scenario: {other:?}");
            std::process::exit(2);
        }
    }
}
