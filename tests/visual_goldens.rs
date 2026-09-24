//! Deterministic visual golden tests.
//!
//! Capture uses Unix poll/nonblocking reads, not a detached reader thread.
//! These tests do not claim Windows/ConPTY coverage.
//!
//! Each demo is run with `--deterministic --freeze-at=<frame> --no-color`, so
//! the frame sequence is exactly `frame * 16ms` and the captured frame is
//! stable. The full byte stream is replayed through `vt100` and the reconstructed
//! **plain screen text** is compared against a small golden file.
//!
//! Regenerate intentionally with:
//!
//! ```text
//! UPDATE_GOLDENS=1 cargo test --test visual_goldens
//! ```
//!
//! or `scripts/dev/update_visual_goldens.sh`.

#![cfg(unix)]

#[path = "common/pty_capture.rs"]
mod pty_capture;

use portable_pty::CommandBuilder;
use std::path::PathBuf;
use std::time::Duration;

struct Case {
    demo: &'static str,
    /// Distinguishes multiple goldens that share a demo/size/freeze.
    label: &'static str,
    cols: u16,
    rows: u16,
    freeze: usize,
    secs: f64,
    extra: &'static [&'static str],
}

const CASES: &[Case] = &[
    // polished_agent — inline is the default identity; one fullscreen proof.
    Case {
        demo: "polished_agent",
        cols: 80,
        rows: 24,
        freeze: 50,
        secs: 1.4,
        label: "",
        extra: &[],
    },
    Case {
        demo: "polished_agent",
        cols: 120,
        rows: 32,
        freeze: 50,
        secs: 1.4,
        label: "",
        extra: &[],
    },
    Case {
        demo: "polished_agent",
        cols: 40,
        rows: 20,
        freeze: 40,
        secs: 1.4,
        label: "",
        extra: &[],
    },
    Case {
        demo: "polished_agent",
        cols: 110,
        rows: 30,
        freeze: 20,
        secs: 1.4,
        label: "fullscreen",
        extra: &["--fullscreen"],
    },
    // hack_the_gibson — representative beats via the deterministic act jump.
    Case {
        demo: "hack_the_gibson",
        cols: 80,
        rows: 24,
        freeze: 12,
        secs: 1.2,
        label: "boot",
        extra: &["--act=boot"],
    },
    Case {
        demo: "hack_the_gibson",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 1.2,
        label: "login",
        extra: &["--act=login"],
    },
    Case {
        demo: "hack_the_gibson",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 1.2,
        label: "cyberdelia",
        extra: &["--act=cyberdelia"],
    },
    Case {
        demo: "hack_the_gibson",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 1.2,
        label: "city",
        extra: &["--act=city"],
    },
    Case {
        demo: "hack_the_gibson",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 1.2,
        label: "plague",
        extra: &["--act=plague"],
    },
    Case {
        demo: "hack_the_gibson",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 1.2,
        label: "davinci",
        extra: &["--act=davinci"],
    },
    Case {
        demo: "hack_the_gibson",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 1.2,
        label: "razor_blade",
        extra: &["--act=razor_blade"],
    },
    Case {
        demo: "hack_the_gibson",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 1.2,
        label: "grand_central",
        extra: &["--act=attack"],
    },
    Case {
        demo: "hack_the_gibson",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 1.2,
        label: "download",
        extra: &["--act=download"],
    },
    Case {
        demo: "hack_the_gibson",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 1.2,
        label: "crash",
        extra: &["--act=crash"],
    },
    Case {
        demo: "hack_the_gibson",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 1.2,
        label: "broadcast",
        extra: &["--act=broadcast"],
    },
    Case {
        demo: "hack_the_gibson",
        cols: 120,
        rows: 32,
        freeze: 20,
        secs: 1.2,
        label: "pool",
        extra: &["--act=pool"],
    },
    Case {
        demo: "hack_the_gibson",
        cols: 120,
        rows: 32,
        freeze: 30,
        secs: 1.2,
        label: "crash_and_burn",
        extra: &["--act=curtain"],
    },
    Case {
        demo: "hack_the_gibson",
        cols: 100,
        rows: 30,
        freeze: 12,
        secs: 1.2,
        label: "root_shell",
        extra: &["--act=shell"],
    },
    Case {
        demo: "hack_the_gibson",
        cols: 56,
        rows: 24,
        freeze: 12,
        secs: 1.2,
        label: "narrow",
        extra: &["--act=attack"],
    },
    // fx_lab — one representative frame per new regression scene.
    Case {
        demo: "fx_lab",
        label: "torus",
        cols: 100,
        rows: 28,
        freeze: 30,
        secs: 1.2,
        extra: &["--scene=5"],
    },
    Case {
        demo: "fx_lab",
        label: "plasma",
        cols: 100,
        rows: 28,
        freeze: 40,
        secs: 1.2,
        extra: &["--scene=2"],
    },
    Case {
        demo: "fx_lab",
        label: "near_clip",
        cols: 100,
        rows: 28,
        freeze: 20,
        secs: 1.2,
        extra: &["--scene=11"],
    },
    Case {
        demo: "fx_lab",
        label: "wide_clip",
        cols: 100,
        rows: 28,
        freeze: 10,
        secs: 1.2,
        extra: &["--scene=12"],
    },
    Case {
        demo: "fx_lab",
        label: "logical_damage",
        cols: 100,
        rows: 28,
        freeze: 10,
        secs: 1.2,
        extra: &["--scene=13"],
    },
    Case {
        demo: "fx_lab",
        label: "dither",
        cols: 100,
        rows: 28,
        freeze: 20,
        secs: 1.2,
        extra: &["--scene=14"],
    },
    Case {
        demo: "fx_lab",
        label: "city",
        cols: 100,
        rows: 28,
        freeze: 30,
        secs: 1.2,
        extra: &["--scene=15"],
    },
    Case {
        demo: "fx_lab",
        label: "packets",
        cols: 100,
        rows: 28,
        freeze: 30,
        secs: 1.2,
        extra: &["--scene=16"],
    },
    Case {
        demo: "fx_lab",
        label: "water",
        cols: 100,
        rows: 28,
        freeze: 30,
        secs: 1.2,
        extra: &["--scene=17"],
    },
    Case {
        demo: "fx_lab",
        label: "scene_algebra",
        cols: 100,
        rows: 28,
        freeze: 30,
        secs: 1.2,
        extra: &["--scene=18"],
    },
    Case {
        demo: "fx_lab",
        label: "story_graph",
        cols: 100,
        rows: 28,
        freeze: 90,
        secs: 1.2,
        extra: &["--scene=19"],
    },
    Case {
        demo: "acid_vs_crash",
        label: "quiet",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 0.8,
        extra: &["--stage=quiet"],
    },
    Case {
        demo: "acid_vs_crash",
        label: "signature",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 0.8,
        extra: &["--stage=signature"],
    },
    Case {
        demo: "acid_vs_crash",
        label: "route_contested",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 0.8,
        extra: &["--stage=route-contested"],
    },
    Case {
        demo: "acid_vs_crash",
        label: "panel_infected",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 0.8,
        extra: &["--stage=first-breach"],
    },
    Case {
        demo: "acid_vs_crash",
        label: "display_intrusion",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 0.8,
        extra: &["--stage=display-intrusion"],
    },
    Case {
        demo: "acid_vs_crash",
        label: "decoy",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 0.8,
        extra: &["--stage=decoy"],
    },
    Case {
        demo: "acid_vs_crash",
        label: "trace",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 0.8,
        extra: &["--stage=trace"],
    },
    Case {
        demo: "acid_vs_crash",
        label: "climax",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 0.8,
        extra: &["--stage=climax"],
    },
    Case {
        demo: "acid_vs_crash",
        label: "crash_win",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 0.8,
        extra: &["--stage=crash-win"],
    },
    Case {
        demo: "acid_vs_crash",
        label: "acid_win",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 0.8,
        extra: &["--stage=acid-win"],
    },
    Case {
        demo: "acid_vs_crash",
        label: "stalemate",
        cols: 120,
        rows: 32,
        freeze: 12,
        secs: 0.8,
        extra: &["--stage=stalemate"],
    },
    Case {
        demo: "acid_vs_crash",
        label: "narrow",
        cols: 56,
        rows: 24,
        freeze: 12,
        secs: 0.8,
        extra: &["--stage=display-intrusion"],
    },
    Case {
        demo: "acid_vs_crash",
        label: "mono_takeover",
        cols: 80,
        rows: 24,
        freeze: 12,
        secs: 0.8,
        extra: &["--stage=takeover"],
    },
];

fn capture_screen(case: &Case) -> String {
    let exe = pty_capture::example_path(case.demo);
    let mut cmd = CommandBuilder::new(&exe);
    cmd.arg("--no-color");
    cmd.arg("--deterministic");
    if case.demo == "acid_vs_crash" {
        cmd.arg("--manual");
        cmd.arg("--visual=flat"); // Inspect the authored stage without defender input.
    }
    cmd.arg(format!("--freeze-at={}", case.freeze));
    for a in case.extra {
        cmd.arg(a);
    }
    cmd.env("TERM", "xterm-256color");
    let mut capture = pty_capture::Capture::spawn(
        cmd,
        case.cols,
        case.rows,
        Duration::from_secs_f64(case.secs),
    );
    capture
        .collect_until(|_| false)
        .expect("capture golden frame");
    let raw = capture.finish();

    let mut parser = vt100::Parser::new(case.rows, case.cols, 0);
    parser.process(&raw);
    // Normalise trailing whitespace per line for stable goldens.
    parser
        .screen()
        .rows(0, case.cols)
        .map(|l| l.trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn golden_path(case: &Case) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens");
    let _ = std::fs::create_dir_all(&dir);
    dir.join(format!(
        "{}_{}x{}_f{}{}.txt",
        case.demo,
        case.cols,
        case.rows,
        case.freeze,
        if case.label.is_empty() {
            String::new()
        } else {
            format!("_{}", case.label)
        }
    ))
}

#[test]
fn visual_goldens() {
    check_goldens(false);
}

#[test]
fn acid_cinematic_goldens() {
    check_goldens(true);
}

fn check_goldens(acid: bool) {
    let update = std::env::var("UPDATE_GOLDENS").is_ok();
    let mut failures = Vec::new();
    for case in CASES
        .iter()
        .filter(|case| (case.demo == "acid_vs_crash") == acid)
    {
        let actual = capture_screen(case);
        let path = golden_path(case);
        if update || !path.exists() {
            std::fs::write(&path, &actual).expect("write golden");
            if !update {
                failures.push(format!(
                    "golden created (review + commit): {}",
                    path.display()
                ));
            }
            continue;
        }
        let expected = std::fs::read_to_string(&path).unwrap_or_default();
        // Compare while ignoring pure-trailing-blank differences.
        let norm = |s: &str| {
            s.lines()
                .map(|l| l.trim_end())
                .collect::<Vec<_>>()
                .join("\n")
                .trim_end()
                .to_string()
        };
        if norm(&actual) != norm(&expected) {
            failures.push(format!(
                "golden mismatch: {}\n--- expected\n{}\n--- actual\n{}",
                path.display(),
                expected,
                actual
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "visual golden failures:\n{}",
        failures.join("\n\n")
    );
}
