//! A1 (Renaissance VI): execute the real `wire_instrument --selfcheck` binary in CI.
//!
//! Milestone A's CI built the example (`cargo build --examples`) and ran the cross-process loop
//! test, but never *executed* the `--selfcheck` CLI composition path (`FieldFrame.surface →
//! Node::raster → Context` under the actual binary). This closes that gap: it builds and runs the
//! binary, driving the producer in deterministic `--lockstep`, and asserts the process exits 0
//! with the PASSED receipt. A missing `python3` is a real failure, not a skip — the loop is the
//! thing under test, so a missing counterpart must fail loudly.

#![cfg(unix)]

// Reuse the freshness-checked example builder (merely finding a stale binary is insufficient).
#[path = "common/pty_capture.rs"]
mod pty_capture;

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn selfcheck_binary_proves_the_full_application_path() {
    let exe = pty_capture::example_path("wire_instrument");
    let python = std::env::var("GIBSON_PYTHON").unwrap_or_else(|_| "python3".to_string());

    // The binary resolves `bindings/python/producer.py` relative to its CWD, so run it from the
    // manifest root. stdin is null (selfcheck needs no TTY); stdout/stderr are small (a handful
    // of receipt lines), well under the pipe buffer, so undrained pipes cannot deadlock it.
    let mut child = Command::new(&exe)
        .arg("--selfcheck")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("GIBSON_PYTHON", &python)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn wire_instrument --selfcheck");

    let mut stdout = child.stdout.take().expect("stdout piped");
    let mut stderr = child.stderr.take().expect("stderr piped");

    // Bounded deadline: the lockstep selfcheck is near-instant (two frames). Fail loudly, never
    // hang CI, if the producer stalls.
    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(s) = child.try_wait().expect("poll selfcheck") {
            break s;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("wire_instrument --selfcheck did not finish within 60s");
        }
        std::thread::sleep(Duration::from_millis(20));
    };

    let mut out = String::new();
    let mut err = String::new();
    stdout
        .read_to_string(&mut out)
        .expect("read selfcheck stdout");
    stderr
        .read_to_string(&mut err)
        .expect("read selfcheck stderr");

    assert!(
        status.success(),
        "wire_instrument --selfcheck exited {status:?}\n--- stdout ---\n{out}\n--- stderr ---\n{err}"
    );
    assert!(
        out.contains("wire_instrument selfcheck PASSED"),
        "selfcheck did not report PASSED\n--- stdout ---\n{out}\n--- stderr ---\n{err}"
    );
}
