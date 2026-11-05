//! RXIII Checkpoint B / Gate C — the supervised PTY proof for `field_cinema --live`.
//!
//! The oracle drives the arc over pipes; this drives the WHOLE executable down a real pseudo-
//! terminal — the actual human path — and supervises its lifecycle: it walks the six beats, exits
//! 0 on its own, and its direct child is reaped within an explicit deadline (no orphan, no hang).
//! `field_cinema --live` itself spawns the Python producer and `link.finish()`s it (dropping
//! stdin → producer EOF → it reaps the producer), so the executable's clean exit 0 is the evidence
//! that it owned and released the producer process. Poll-based and bounded: no fixed sleeps gate
//! the proof, and if python3 or the producer is missing the executable exits nonzero and the test
//! fails loudly rather than skipping.
#![cfg(unix)]

use portable_pty::CommandBuilder;
use std::time::{Duration, Instant};

#[path = "common/pty_capture.rs"]
mod pty_capture;

/// Spawn `field_cinema` with `extra` args on a real PTY, capture until the final beat line, then
/// supervise a clean exit-0 and the direct child's reaping. Returns the decoded output.
fn run_live_film(extra: &[&str]) -> String {
    let mut cmd = CommandBuilder::new(pty_capture::example_path("field_cinema"));
    cmd.arg("--live");
    for a in extra {
        cmd.arg(a);
    }
    // The executable spawns `bindings/python/producer.py` by a repo-relative path.
    cmd.cwd(env!("CARGO_MANIFEST_DIR"));

    let mut capture = pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_secs(10));
    // `RESOLVE]` (bracketed) is the final beat line; the banner's six-beat list has a bare
    // `RESOLVE` with no bracket, so this fires only at the end.
    capture
        .collect_until(|bytes| String::from_utf8_lossy(bytes).contains("RESOLVE]"))
        .expect("capture the live film through to RESOLVE");

    // Supervise the exit: the film must finish on its own, exit 0, within a bounded deadline.
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        if let Some(s) = capture.try_wait().expect("poll field_cinema --live") {
            break s;
        }
        if Instant::now() >= deadline {
            panic!("field_cinema --live did not exit after reaching RESOLVE");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(
        status.success(),
        "field_cinema --live must exit 0 after a clean film (status {status:?})"
    );

    // `finish` asserts the direct child reaped before the cleanup deadline (no orphan).
    String::from_utf8_lossy(&capture.finish()).into_owned()
}

/// Assert the six beats appear in arc order somewhere in the output.
fn assert_beats_in_order(out: &str) {
    let mut cursor = 0usize;
    for tag in [
        "ESTABLISH]",
        "DISCOVER]",
        "INTERVENE]",
        "AWAIT]",
        // TRANSFORM or AWAIT resolution happens here depending on the history; checked by caller.
        "RESOLVE]",
    ] {
        let rest = &out[cursor..];
        let at = rest
            .find(tag)
            .unwrap_or_else(|| panic!("beat line [{tag} missing or out of order in:\n{out}"));
        cursor += at + tag.len();
    }
}

/// The accepted film: a real PTY session walks the six beats, earns the transform (the observed
/// field moved), exits 0, and reaps cleanly.
#[test]
fn the_live_film_walks_the_arc_and_exits_cleanly_on_a_real_pty() {
    let out = run_live_film(&[]);
    assert_beats_in_order(&out);
    assert!(
        out.contains("TRANSFORM]"),
        "the accepted film must reach the TRANSFORM beat:\n{out}"
    );
    assert!(
        out.contains("transformation earned"),
        "the accepted film announces the earned transform:\n{out}"
    );
    assert!(
        out.contains("resolves on a transformed field"),
        "the accepted film resolves transformed:\n{out}"
    );
}

/// The suppressed film: the identical acknowledged edit, but the model is frozen — the honest
/// non-transform. It, too, must run to RESOLVE and exit 0 cleanly on a real terminal.
#[test]
fn the_suppressed_film_honestly_refuses_to_transform_on_a_real_pty() {
    let out = run_live_film(&["--suppress"]);
    assert_beats_in_order(&out);
    assert!(
        out.contains("producer verdict: Acknowledged"),
        "the suppressed edit is still acknowledged on the wire:\n{out}"
    );
    assert!(
        !out.contains("transformation earned"),
        "a suppressed edit must NOT announce a transform:\n{out}"
    );
    assert!(
        out.contains("no authoritative change, no transformation (honest)"),
        "the suppressed film stays honest:\n{out}"
    );
    assert!(
        out.contains("resolves unchanged"),
        "the suppressed film resolves unchanged:\n{out}"
    );
}
