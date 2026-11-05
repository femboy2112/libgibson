//! A1 (Renaissance VI): execute the real `wire_instrument --selfcheck` binary in CI.
//!
//! Milestone A's CI built the example (`cargo build --examples`) and ran the cross-process loop
//! test, but never *executed* the `--selfcheck` CLI composition path (`FieldFrame.surface →
//! Node::raster → Context` under the actual binary). This closes that gap: it builds and runs the
//! binary, driving the producer in deterministic `--lockstep`, and asserts the process exits 0
//! with the PASSED receipt. A missing `python3` is a real failure, not a skip — the loop is the
//! thing under test, so a missing counterpart must fail loudly.
//!
//! Gate H adds two things to this suite:
//!   - the happy-path test now also asserts the #143 command-dispatch latency leg was recorded
//!     end to end through the real loop (not just in the injected-Duration detection test);
//!   - a failing-producer test proves the producer's stderr is now SURFACED, not nulled — the
//!     carried-debt fix in `spawn_producer`: a producer that dies must report its cause, never a
//!     silent, causeless stall.

#![cfg(unix)]

// Reuse the freshness-checked example builder (merely finding a stale binary is insufficient).
#[path = "common/pty_capture.rs"]
mod pty_capture;

use std::io::Read;
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// Run the real `wire_instrument` binary with `args`, from the manifest root (so the default
/// producer path resolves), stdin null, stdout/stderr captured. Bounded deadline so a stalled
/// producer fails loudly instead of hanging CI. Returns `(status, stdout, stderr)`.
fn run_wire_instrument(args: &[&str]) -> (ExitStatus, String, String) {
    let exe = pty_capture::example_path("wire_instrument");
    let python = std::env::var("GIBSON_PYTHON").unwrap_or_else(|_| "python3".to_string());

    let mut child = Command::new(&exe)
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("GIBSON_PYTHON", &python)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn wire_instrument");

    let mut stdout = child.stdout.take().expect("stdout piped");
    let mut stderr = child.stderr.take().expect("stderr piped");

    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(s) = child.try_wait().expect("poll wire_instrument") {
            break s;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("wire_instrument {args:?} did not finish within 60s");
        }
        std::thread::sleep(Duration::from_millis(20));
    };

    let mut out = String::new();
    let mut err = String::new();
    stdout.read_to_string(&mut out).expect("read stdout");
    stderr.read_to_string(&mut err).expect("read stderr");
    (status, out, err)
}

#[test]
fn selfcheck_binary_proves_the_full_application_path() {
    let (status, out, err) = run_wire_instrument(&["--selfcheck"]);

    assert!(
        status.success(),
        "wire_instrument --selfcheck exited {status:?}\n--- stdout ---\n{out}\n--- stderr ---\n{err}"
    );
    assert!(
        out.contains("wire_instrument selfcheck PASSED"),
        "selfcheck did not report PASSED\n--- stdout ---\n{out}\n--- stderr ---\n{err}"
    );
    // The #143 latency funnel ran end to end through the real consumer loop: the command-dispatch
    // leg (issue→confirmed write+flush) recorded exactly the one delivered edit.
    assert!(
        out.contains("command-dispatch leg: count 1"),
        "the command-dispatch latency leg was not recorded end to end\n--- stdout ---\n{out}"
    );
}

#[test]
fn a_failing_producer_surfaces_its_stderr_not_a_silent_stall() {
    // A producer that dies immediately with a distinctive stderr message. With the producer's
    // stderr nulled (the old behavior) this cause would be invisible and the consumer could only
    // report a generic closed-stream stall; inherited, the producer's own diagnostic reaches us.
    const MARKER: &str = "PRODUCER_BOOM_7F3A";
    let dir = std::env::temp_dir().join(format!("wire_boom_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let script = dir.join("boom_producer.py");
    std::fs::write(
        &script,
        format!("import sys\nsys.stderr.write('{MARKER}\\n')\nsys.stderr.flush()\nsys.exit(3)\n"),
    )
    .expect("write boom producer");

    let (status, out, err) =
        run_wire_instrument(&["--selfcheck", "--producer", script.to_str().unwrap()]);
    let _ = std::fs::remove_dir_all(&dir);

    // Loud, bounded failure — never a PASSED and never a hang.
    assert!(
        !status.success(),
        "a dead producer must fail the selfcheck\n--- stdout ---\n{out}\n--- stderr ---\n{err}"
    );
    assert!(
        !out.contains("wire_instrument selfcheck PASSED"),
        "a dead producer must not report PASSED\n--- stdout ---\n{out}"
    );
    // The carried-debt fix: the producer's own stderr is SURFACED (inherited), not swallowed.
    // This assertion is the red-first proof — it fails against the old `Stdio::null()`.
    assert!(
        err.contains(MARKER),
        "the producer's stderr must be surfaced, not nulled\n--- stderr ---\n{err}"
    );
}
