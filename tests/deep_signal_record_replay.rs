//! DEEP SIGNAL (v0.7 nodes C/E) — live play, recorded, replays to the identical state.
//!
//! A real-PTY session with held movement is recorded (`--record`), quit with Ctrl-C, and the
//! trace is replayed headlessly (`--replay`). The replay must reach the same simulation tic and
//! the same world digest as the live session reported: inputs are stamped with simulation tics,
//! so wall-clock timing and presentation cannot leak into the outcome.
#![cfg(unix)]
#[path = "common/pty_capture.rs"]
mod pty_capture;

use portable_pty::CommandBuilder;
use pty_capture::{example_path, Capture};
use std::time::{Duration, Instant};

fn field<'a>(text: &'a str, prefix: &str, key: &str) -> &'a str {
    let line = text
        .lines()
        .rev()
        .find(|l| l.contains(prefix))
        .unwrap_or_else(|| panic!("no {prefix:?} line in {text:?}"));
    line.split(&format!("{key}="))
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
}

#[test]
fn a_recorded_live_session_replays_to_the_same_world() {
    let trace = std::env::temp_dir().join(format!("deep_signal_trace_{}.tsv", std::process::id()));
    let _ = std::fs::remove_file(&trace);
    let mut cmd = CommandBuilder::new(example_path("deep_signal"));
    cmd.args(["--seed", "7", "--max-seconds", "30", "--record"]);
    cmd.arg(trace.to_str().unwrap());
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    let mut cap = Capture::spawn(cmd, 80, 24, Duration::from_secs(45));
    cap.collect_until(|o| String::from_utf8_lossy(o).contains("SHARDS"))
        .expect("first frame");
    for (key, n) in [(b"w", 30), (b"d", 12), (b"w", 25), (b"a", 8), (b"z", 10)] {
        for _ in 0..n {
            cap.write(key).unwrap();
            cap.collect_for(Duration::from_millis(30)).unwrap();
        }
    }
    // Ctrl-Z is recorded but must not move the player (checked below); Ctrl-C quits.
    cap.write(b"\x1a").unwrap();
    cap.collect_for(Duration::from_millis(200)).unwrap();
    cap.write(b"\x03").unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        cap.collect_for(Duration::from_millis(50)).unwrap();
        if let Some(status) = cap.try_wait().unwrap() {
            assert!(status.success(), "{status:?}");
            break;
        }
        assert!(Instant::now() < deadline, "Ctrl-C did not quit deep_signal");
    }
    cap.collect_for(Duration::from_millis(100)).unwrap();
    let live = String::from_utf8_lossy(&cap.finish()).into_owned();
    let (live_tics, live_world) = (
        field(&live, "deep_signal: tics=", "tics"),
        field(&live, "deep_signal: tics=", "world"),
    );
    let recorded = std::fs::read_to_string(&trace).expect("trace written");
    assert!(
        recorded.starts_with("deep_signal-trace v1\tseed=7\t"),
        "{recorded}"
    );
    assert!(
        recorded.lines().count() > 20,
        "the trace holds the session's inputs"
    );
    let run_replay = |path: &std::path::Path| -> String {
        let out = std::process::Command::new(example_path("deep_signal"))
            .arg("--replay")
            .arg(path)
            .output()
            .expect("run --replay");
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(
            out.status.success(),
            "{text}{}",
            String::from_utf8_lossy(&out.stderr)
        );
        text
    };
    let replay = run_replay(&trace);
    // Ctrl-/Alt-chords are in the trace (the Ctrl-Z above) but must not change the simulation:
    // the same trace without them replays to the same trajectory.
    let chord = |l: &&str| l.contains("\tkey\t") && !l.ends_with("\t00");
    assert!(
        recorded.lines().any(|l| chord(&l)),
        "the Ctrl-Z was recorded"
    );
    let unchorded: String = recorded
        .lines()
        .filter(|l| !chord(l))
        .map(|l| format!("{l}\n"))
        .collect();
    let trace2 = trace.with_extension("nochords.tsv");
    std::fs::write(&trace2, unchorded).unwrap();
    let replay2 = run_replay(&trace2);
    let _ = std::fs::remove_file(&trace2);
    assert_eq!(
        field(&replay2, "deep_signal replay:", "path"),
        field(&replay, "deep_signal replay:", "path"),
        "a Ctrl-/Alt-chord changed the simulation"
    );
    // A hand-edited trace with inputs out of tic order is refused, not partly replayed.
    let mut lines: Vec<&str> = recorded.lines().collect();
    let first = lines.iter().position(|l| l.contains("\tkey\t")).unwrap();
    let last = lines.iter().rposition(|l| l.contains("\tkey\t")).unwrap();
    lines.swap(first, last);
    let trace3 = trace.with_extension("shuffled.tsv");
    std::fs::write(&trace3, lines.join("\n") + "\n").unwrap();
    let out3 = std::process::Command::new(example_path("deep_signal"))
        .arg("--replay")
        .arg(&trace3)
        .output()
        .expect("run --replay");
    let _ = std::fs::remove_file(&trace3);
    assert!(
        !out3.status.success(),
        "an out-of-order trace must be refused"
    );
    assert!(String::from_utf8_lossy(&out3.stderr).contains("out of order"));
    let _ = std::fs::remove_file(&trace);
    assert_eq!(
        field(&replay, "deep_signal replay:", "tics"),
        live_tics,
        "{replay}"
    );
    assert_eq!(
        field(&replay, "deep_signal replay:", "world"),
        live_world,
        "{replay}\n{live}"
    );
    let live_path = field(&live, "deep_signal: tics=", "path");
    assert_eq!(
        field(&replay, "deep_signal replay:", "path"),
        live_path,
        "trajectory differs"
    );
    assert_ne!(live_tics, "0");
}
