//! DEEP SIGNAL (v0.7 node E) — the headless self-check runs in the suite, not only by hand:
//! injected-input replay twice, presentation cadences 1/2/3/7 and a 40x15 ASCII run must all
//! reach the same world digest, and the view must paint.
#[path = "common/pty_capture.rs"]
mod pty_capture;

#[test]
fn deep_signal_selfcheck_passes() {
    let out = std::process::Command::new(pty_capture::example_path("deep_signal"))
        .arg("--selfcheck")
        .output()
        .expect("run deep_signal --selfcheck");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "selfcheck failed: {stdout}{stderr}");
    assert!(
        stdout.contains("deep_signal selfcheck: OK"),
        "{stdout}{stderr}"
    );
}
