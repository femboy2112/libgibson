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

use std::ffi::{CStr, CString};
use std::io::Write;
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

// --- Gate K · K3: real-PTY INPUT + RESIZE through the C ABI ---------------
//
// J3 (above) proved the construction/paint/restore wire and explicitly ceded
// input, resize and hangup. ABI v1 already ships `gibson_poll_event` and
// `GIBSON_EVENT_{KEY,PASTE,RESIZE}` — but NO test ever called them, so the whole
// event-decode path was unverified on a real terminal. These two scenarios
// drive it: the terminal wire is no longer the observable (J3 covered that), so
// each emits a line-oriented RECEIPT of the events the ABI surfaced, which the
// parent asserts against the exact keys/paste/resize it injected on the PTY
// master. Input and resize are separate scenarios so a key can never coincide
// with a resize (the known crossterm #15 queue-until-next-key collision).

/// Emit one receipt marker. Raw mode disables OPOST, so terminate explicitly
/// with CRLF and flush; the parent substring-searches the raw PTY capture.
fn mark(s: &str) {
    print!("{s}\r\n");
    let _ = std::io::stdout().flush();
}

/// Minimal live root so the context is actually painting a frame before we poll.
unsafe fn set_minimal_root(ctx: *mut GibsonContextOpaque, label: &str) {
    let mut col = ptr::null_mut();
    assert_eq!(gibson_node_box_col(&mut col), GibsonStatus::Ok);
    let text = CString::new(label).unwrap();
    let mut txt = ptr::null_mut();
    assert_eq!(
        gibson_node_text(text.as_ptr(), ptr::null(), GIBSON_WRAP_NONE, &mut txt),
        GibsonStatus::Ok
    );
    assert_eq!(gibson_node_add_child(col, txt), GibsonStatus::Ok);
    assert_eq!(gibson_set_root_node(ctx, col), GibsonStatus::Ok);
    assert_eq!(gibson_render(ctx), GibsonStatus::Ok);
}

/// Poll keys/paste through the ABI and echo a receipt per event until the `q`
/// sentinel. The FIRST poll is what enters raw mode + enables bracketed paste
/// (and installs crossterm's resize source), so READY is printed only after it:
/// the parent must not inject bytes until raw mode is actually on.
fn abi_input() {
    unsafe {
        let mut ctx: *mut GibsonContextOpaque = ptr::null_mut();
        assert_eq!(
            gibson_create_context(GIBSON_MODE_FULLSCREEN, &mut ctx),
            GibsonStatus::Ok
        );
        set_minimal_root(ctx, "K3-input");

        let mut ev: GibsonEvent = std::mem::zeroed();
        let _ = gibson_poll_event(ctx, 50, &mut ev); // arm raw mode + bracketed paste
        mark("READY");

        loop {
            let mut ev: GibsonEvent = std::mem::zeroed();
            if gibson_poll_event(ctx, 2000, &mut ev) != GibsonStatus::Ok {
                mark("POLL_ERR");
                continue;
            }
            match ev.event_type {
                GIBSON_EVENT_KEY => {
                    mark(&format!("EVKEY code={} mod={}", ev.key_code, ev.modifiers));
                    if ev.key_code == u32::from('q') {
                        break;
                    }
                }
                GIBSON_EVENT_PASTE => {
                    let s = if ev.paste_str.is_null() {
                        String::new()
                    } else {
                        CStr::from_ptr(ev.paste_str).to_string_lossy().into_owned()
                    };
                    mark(&format!("EVPASTE text={s}"));
                }
                GIBSON_EVENT_RESIZE => {
                    mark(&format!("EVRESIZE cols={} rows={}", ev.cols, ev.rows));
                }
                _ => {} // NONE / timeout / (mouse|focus|tick -> NONE in ABI v1)
            }
        }
        mark("DONE");
        gibson_destroy_context(ctx);
        std::process::exit(0);
    }
}

/// Issue #274: the kind-aware poll on a real terminal. Like `abi-input`, but through
/// `gibson_poll_event_kind`, so each key is marked with its kind: a typed U+1001–U+100D comes
/// back CHAR and a special key with the same code comes back SPECIAL.
fn abi_input_kind() {
    unsafe {
        let mut ctx: *mut GibsonContextOpaque = ptr::null_mut();
        assert_eq!(
            gibson_create_context(GIBSON_MODE_FULLSCREEN, &mut ctx),
            GibsonStatus::Ok
        );
        set_minimal_root(ctx, "274-kind");
        let mut ev: GibsonEvent = std::mem::zeroed();
        let mut kind = 0u32;
        let _ = gibson_poll_event_kind(ctx, 50, &mut ev, &mut kind); // arm raw mode
        mark("READY");
        loop {
            let mut ev: GibsonEvent = std::mem::zeroed();
            if gibson_poll_event_kind(ctx, 2000, &mut ev, &mut kind) != GibsonStatus::Ok {
                mark("POLL_ERR");
                continue;
            }
            if ev.event_type == GIBSON_EVENT_KEY {
                mark(&format!(
                    "EVKEY code={:#x} kind={kind} mod={}",
                    ev.key_code, ev.modifiers
                ));
                if ev.key_code == u32::from('q') && kind == GIBSON_KEY_KIND_CHAR {
                    break;
                }
            }
        }
        mark("DONE");
        gibson_destroy_context(ctx);
        std::process::exit(0);
    }
}

/// Surface a real SIGWINCH resize through the ABI: after READY the parent resizes
/// the PTY; the ABI must yield one `GIBSON_EVENT_RESIZE` carrying the new
/// geometry, and a subsequent `gibson_render` must still succeed at the new size.
fn abi_resize() {
    unsafe {
        let mut ctx: *mut GibsonContextOpaque = ptr::null_mut();
        assert_eq!(
            gibson_create_context(GIBSON_MODE_FULLSCREEN, &mut ctx),
            GibsonStatus::Ok
        );
        set_minimal_root(ctx, "K3-resize");

        let mut ev: GibsonEvent = std::mem::zeroed();
        let _ = gibson_poll_event(ctx, 50, &mut ev); // install the resize source
        mark("READY");

        loop {
            let mut ev: GibsonEvent = std::mem::zeroed();
            if gibson_poll_event(ctx, 2000, &mut ev) != GibsonStatus::Ok {
                mark("POLL_ERR");
                continue;
            }
            match ev.event_type {
                GIBSON_EVENT_RESIZE => {
                    mark(&format!("EVRESIZE cols={} rows={}", ev.cols, ev.rows));
                    let r = gibson_render(ctx);
                    mark(&format!("RENDERED_AFTER_RESIZE status={}", r as i32));
                    break;
                }
                GIBSON_EVENT_KEY if ev.key_code == u32::from('q') => break,
                _ => {}
            }
        }
        mark("DONE");
        gibson_destroy_context(ctx);
        std::process::exit(0);
    }
}

/// RXIV Gate G1 — a controlled real-TTY input -> state -> render loop entirely through the C ABI:
/// every Right arrow polled by `gibson_poll_event` increments application state, which is rebuilt
/// into a node tree and painted with `gibson_render`; the parent verifies the PAINTED frame on a
/// vt100 parse of the alternate screen, not just the receipt. `q` ends the loop.
fn abi_loop() {
    unsafe {
        let mut ctx: *mut GibsonContextOpaque = ptr::null_mut();
        assert_eq!(
            gibson_create_context(GIBSON_MODE_FULLSCREEN, &mut ctx),
            GibsonStatus::Ok
        );
        let mut count = 0u32;
        set_minimal_root(ctx, &format!("abi-loop count {count}"));
        let mut ev: GibsonEvent = std::mem::zeroed();
        let _ = gibson_poll_event(ctx, 50, &mut ev); // arm raw mode
        mark("READY");
        loop {
            let mut ev: GibsonEvent = std::mem::zeroed();
            if gibson_poll_event(ctx, 2000, &mut ev) != GibsonStatus::Ok {
                mark("POLL_ERR");
                continue;
            }
            if ev.event_type != GIBSON_EVENT_KEY {
                continue;
            }
            if ev.key_code == u32::from('q') {
                break;
            }
            if ev.key_code == 0x1003 {
                // GIBSON_KEY_RIGHT: input -> state -> render, all through the ABI.
                count += 1;
                set_minimal_root(ctx, &format!("abi-loop count {count}"));
                mark(&format!("STATE count={count}"));
            }
        }
        mark("DONE");
        gibson_destroy_context(ctx);
        std::process::exit(0);
    }
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("abi-loop") => abi_loop(),
        Some("abi") => abi(),
        Some("native") => native(),
        Some("abi-input") => abi_input(),
        Some("abi-input-kind") => abi_input_kind(),
        Some("abi-resize") => abi_resize(),
        other => {
            eprintln!("unknown ffi-realtty scenario: {other:?}");
            std::process::exit(2);
        }
    }
}
