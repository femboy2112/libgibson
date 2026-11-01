//! Cross-ABI behavioral parity on the **non-TTY plain-scrollback path** (#142, v0.6 Gate H).
//!
//! The scope, stated honestly and narrowly: this proves that the C and Python bindings marshal
//! into the *same* engine, whose non-TTY scrollback accounting (`commit_text`,
//! `insert_text_before_live`, `render` with a root, `get_stats`) and committed byte stream match
//! the native Rust API, and that the `gibson_stats_t` struct layout and field mapping agree
//! across all three. It is **not** pixel or ANSI parity — in a non-TTY session `render` is a
//! suppressed no-op, so `frames` only proves the call reached the scheduler — and it is **not**
//! full language feature parity (styles, rich text, node trees, events, resize), which remains a
//! separate, later gate. `last_render_duration_micros` is excluded as a timing quantity (L10).
//!
//! THE PARITY TRIANGLE, split across two CI jobs so each leg is checked where it can be:
//!   - the Rust job runs THIS test (`cargo test`, no cdylib): native stats+wire == the committed
//!     golden files (`bindings/parity/golden_{stats,wire}.txt`);
//!   - the Bindings job (which builds the cdylib) runs `bindings/parity/parity.c` and
//!     `parity.py` and `cmp`s C == Python == the same golden files.
//!
//! Transitively: the C-ABI and Python bindings produce byte-identical engine accounting to the
//! native API for the shared script. The golden lives in ONE place, so there is no triple-hardcode
//! drift.
//!
//! Why non-TTY: `gibson_create_context` is literally `Context::new`, which samples
//! `stdout().is_terminal()` at create time; the Bindings job's stdout is a pipe, so the ABI runs
//! the non-TTY branch. A golden built from a stock (TTY) `Context::headless` would NOT match it.
//! So the native side forces `session.is_tty = false`, and the ABI harnesses `dup2` a file onto
//! fd 1 before creating the context — both land on the identical non-TTY plain path.

use std::fs;
use std::path::PathBuf;

use gibson::context::Context;
use gibson::node::Node;
use gibson::renderer::RenderMode;
use gibson::Style;

fn parity_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bindings/parity")
}

/// The canonical op-script. SHORT ASCII strings only, so wrap and terminal width never matter.
/// This is the exact sequence `parity.c` and `parity.py` run against the C-ABI.
fn run_script() -> (String, String) {
    let mut ctx = Context::headless(RenderMode::Inline, 80, 24);
    // The ABI path is non-TTY in CI; match it exactly (pub field, same trick as
    // tests/non_tty_redirection.rs). A headless session uses a Buffer sink, so this takes no
    // process-global terminal lease and is safe to run anywhere, terminal or pipe.
    ctx.session.is_tty = false;

    ctx.commit_text("alpha").unwrap();
    ctx.commit_text("beta gamma").unwrap();
    ctx.insert_text_before_live("delta").unwrap();
    ctx.set_root(Node::col().child(Node::text("live", Style::default())));
    ctx.render().unwrap();
    ctx.commit_text("omega").unwrap();

    let s = ctx.stats();
    let wire = ctx.take_output();

    // Canonical key=value report, one field per line, in gibson_stats_t order, EXCLUDING the
    // timing field `last_render_duration_micros`. parity.c and parity.py emit the identical shape.
    let report = format!(
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
    );
    (report, wire)
}

/// The native engine's accounting + committed byte stream for the canonical script must match the
/// committed golden that the C and Python ABI harnesses are `cmp`'d against. Run once with
/// `WRITE_PARITY_GOLDEN=1` to (re)freeze the golden, inspect it, commit it; thereafter this is a
/// pure assertion (and the only thing the Rust CI job needs — no cdylib).
#[test]
fn native_golden_matches_the_committed_parity_golden() {
    let (report, wire) = run_script();
    let dir = parity_dir();
    let stats_path = dir.join("golden_stats.txt");
    let wire_path = dir.join("golden_wire.txt");

    if std::env::var_os("WRITE_PARITY_GOLDEN").is_some() {
        fs::create_dir_all(&dir).unwrap();
        fs::write(&stats_path, &report).unwrap();
        fs::write(&wire_path, &wire).unwrap();
        eprintln!(
            "wrote golden stats:\n{report}--- wire ({} bytes) ---\n{wire:?}",
            wire.len()
        );
    }

    let golden_stats = fs::read_to_string(&stats_path).expect(
        "bindings/parity/golden_stats.txt missing — run with WRITE_PARITY_GOLDEN=1 to create it",
    );
    let golden_wire =
        fs::read_to_string(&wire_path).expect("bindings/parity/golden_wire.txt missing");

    assert_eq!(
        report, golden_stats,
        "native stats diverged from the committed golden — the engine's accounting changed"
    );
    assert_eq!(
        wire, golden_wire,
        "native committed byte stream diverged from the committed golden"
    );
}

/// Teeth: the golden must carry real, non-zero signal, so a stub binding that returned zeros (or
/// an empty stream) could not pass the cross-ABI `cmp` vacuously. Most non-TTY stat fields are
/// legitimately zero (no paint), so the committed byte stream is the load-bearing signal.
#[test]
fn the_golden_carries_real_signal_not_an_empty_stream() {
    let (report, wire) = run_script();
    assert!(
        wire.contains("alpha") && wire.contains("beta gamma") && wire.contains("omega"),
        "the committed byte stream must carry the committed lines, got {wire:?}"
    );
    // commit_bytes counts real committed text; it must not be zero, or the parity cmp would be
    // satisfied by a do-nothing binding.
    assert!(
        !report.contains("commit_bytes=0\n"),
        "commit_bytes must be non-zero for a script that commits text:\n{report}"
    );
}
