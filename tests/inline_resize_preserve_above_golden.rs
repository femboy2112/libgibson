//! Issue #403: the default `InlineResizePolicy::PreserveAbove` is the v0.7.2 inline resize
//! behavior, byte for byte.
//!
//! One scripted session (frames, grow, shrink, threshold jitter, `clear_live_region` after a
//! resize, a commit after a resize, a history insertion after a resize, plus a fullscreen resize)
//! is compared with the exact bytes v0.7.2 emitted for it. The script uses only API that predates
//! the policy, so the golden is recorded by running this same file against tag `v0.7.2`:
//!
//! ```text
//! UPDATE_PRESERVE_ABOVE_GOLDEN=1 cargo test --test inline_resize_preserve_above_golden
//! ```
//!
//! (That recording used this file without its last test, which needs the post-v0.7.2 policy
//! API.) Never re-record it on a later tree. A mismatch means the default changed.

use gibson::cell::Style;
use gibson::context::{Context, RenderMode};
use gibson::node::Node;

const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/goldens/inline_resize_preserve_above_v0.7.2.ans"
);

fn frame(step: usize, rows: u16) -> Node {
    Node::col().children(
        (0..rows)
            .map(|i| Node::text(format!("f{step}-r{i}"), Style::default()))
            .collect(),
    )
}

fn script() -> Vec<u8> {
    script_with(|_| {})
}

/// The script, with `setup` applied to the inline context before anything is drawn.
fn script_with(setup: impl FnOnce(&mut Context)) -> Vec<u8> {
    let mut out = Vec::new();
    let mut ctx = Context::headless(RenderMode::Inline, 40, 15);
    setup(&mut ctx);
    let mut take = |ctx: &mut Context| out.extend_from_slice(ctx.take_output().as_bytes());

    ctx.commit_text("SYNTHETIC-HISTORY").unwrap();
    ctx.set_root(frame(0, 14));
    ctx.render().unwrap();
    take(&mut ctx);

    // Plain resizes: grow, shrink, threshold jitter.
    let mut step = 1;
    for (cols, rows) in [(80, 24), (40, 15), (120, 40), (79, 23), (80, 23), (80, 24)] {
        ctx.resize(cols, rows).unwrap();
        ctx.set_root(frame(step, rows - 1));
        ctx.render().unwrap();
        take(&mut ctx);
        step += 1;
    }
    // clear_live_region() after a resize, before the next frame.
    for (cols, rows) in [(60, 18), (90, 30)] {
        ctx.resize(cols, rows).unwrap();
        ctx.clear_live_region().unwrap();
        take(&mut ctx);
        ctx.set_root(frame(step, rows - 1));
        ctx.render().unwrap();
        take(&mut ctx);
        step += 1;
    }
    // A commit after a resize.
    ctx.resize(70, 20).unwrap();
    ctx.commit_text("COMMIT-AFTER-RESIZE").unwrap();
    take(&mut ctx);
    ctx.set_root(frame(step, 5));
    ctx.render().unwrap();
    take(&mut ctx);
    step += 1;
    // A history insertion after a resize (a short region, so the fast path is eligible).
    ctx.resize(72, 21).unwrap();
    ctx.insert_text_before_live("INSERT-AFTER-RESIZE").unwrap();
    take(&mut ctx);
    ctx.set_root(frame(step, 5));
    ctx.render().unwrap();
    take(&mut ctx);
    step += 1;
    // A widget cursor mid-region, then a shrink.
    ctx.set_root(
        Node::col()
            .child(Node::text("above", Style::default()))
            .child(Node::text_input("typed", 3, None, Style::default()))
            .child(Node::text("below", Style::default())),
    );
    ctx.render().unwrap();
    take(&mut ctx);
    ctx.resize(50, 10).unwrap();
    ctx.set_root(frame(step, 3));
    ctx.render().unwrap();
    take(&mut ctx);

    // Fullscreen is outside the policy; pin it too.
    let mut fs = Context::headless(RenderMode::Fullscreen, 40, 15);
    fs.set_root(frame(100, 4));
    fs.render().unwrap();
    out.extend_from_slice(fs.take_output().as_bytes());
    fs.resize(60, 20).unwrap();
    fs.render().unwrap();
    out.extend_from_slice(fs.take_output().as_bytes());
    out
}

#[test]
fn preserve_above_is_v0_7_2_byte_for_byte() {
    let bytes = script();
    if std::env::var_os("UPDATE_PRESERVE_ABOVE_GOLDEN").is_some() {
        std::fs::write(GOLDEN, &bytes).expect("write golden");
        return;
    }
    let golden = std::fs::read(GOLDEN).expect("golden missing (record it on tag v0.7.2 only)");
    if bytes != golden {
        let at = bytes
            .iter()
            .zip(&golden)
            .position(|(a, b)| a != b)
            .unwrap_or(bytes.len().min(golden.len()));
        let lo = at.saturating_sub(40);
        panic!(
            "PreserveAbove diverged from v0.7.2 at byte {at} (len {} vs golden {})\n now: {:?}\n was: {:?}",
            bytes.len(),
            golden.len(),
            String::from_utf8_lossy(&bytes[lo..(at + 40).min(bytes.len())]),
            String::from_utf8_lossy(&golden[lo..(at + 40).min(golden.len())]),
        );
    }
}

/// The golden discriminates: the opt-in policy emits different bytes for the same script, so a
/// default that silently became `ReclaimLiveRows` would fail the test above. (Not part of the
/// v0.7.2 recording: that run used the file before this test existed.)
#[test]
fn the_golden_would_catch_reclaim_live_rows() {
    let golden = std::fs::read(GOLDEN).expect("golden");
    let reclaim = script_with(|ctx| {
        ctx.set_inline_resize_policy(gibson::InlineResizePolicy::ReclaimLiveRows)
    });
    assert_ne!(
        reclaim, golden,
        "ReclaimLiveRows must differ from the v0.7.2 bytes"
    );
}
