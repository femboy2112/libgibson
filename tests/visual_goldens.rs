//! Deterministic visual golden tests.
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

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
    Case {
        demo: "polished_agent",
        cols: 80,
        rows: 24,
        freeze: 50,
        secs: 1.6,
        label: "",
        extra: &[],
    },
    Case {
        demo: "polished_agent",
        cols: 120,
        rows: 32,
        freeze: 50,
        secs: 1.6,
        label: "",
        extra: &[],
    },
    Case {
        demo: "polished_agent",
        cols: 40,
        rows: 20,
        freeze: 40,
        secs: 1.6,
        label: "",
        extra: &[],
    },
    Case {
        demo: "hack_the_gibson",
        cols: 120,
        rows: 32,
        freeze: 120,
        secs: 1.8,
        label: "",
        extra: &[],
    },
    Case {
        demo: "hack_the_gibson",
        cols: 56,
        rows: 24,
        freeze: 90,
        secs: 1.8,
        label: "",
        extra: &[],
    },
    Case {
        demo: "fx_lab",
        label: "torus",
        cols: 100,
        rows: 28,
        freeze: 30,
        secs: 1.6,
        extra: &["--scene=5"],
    },
    Case {
        demo: "fx_lab",
        label: "plasma",
        cols: 100,
        rows: 28,
        freeze: 40,
        secs: 1.6,
        extra: &["--scene=2"],
    },
];

fn example_path(name: &str) -> PathBuf {
    let path = std::env::current_exe()
        .expect("current_exe")
        .parent()
        .expect("deps")
        .parent()
        .expect("target")
        .join("examples")
        .join(name);
    if !path.exists() {
        let status = std::process::Command::new("cargo")
            .args(["build", "--example", name])
            .status()
            .expect("build example");
        assert!(status.success());
    }
    path
}

fn capture_screen(case: &Case) -> String {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: case.rows,
            cols: case.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("openpty");
    let exe = example_path(case.demo);
    let mut cmd = CommandBuilder::new(&exe);
    cmd.arg("--no-color");
    cmd.arg("--deterministic");
    cmd.arg(format!("--freeze-at={}", case.freeze));
    for a in case.extra {
        cmd.arg(a);
    }
    cmd.env("TERM", "xterm-256color");
    let mut child = pair.slave.spawn_command(cmd).expect("spawn");
    let mut killer = child.clone_killer();
    let mut reader = pair.master.try_clone_reader().expect("reader");

    let buf = Arc::new(Mutex::new(Vec::new()));
    let buf2 = Arc::clone(&buf);
    std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => buf2.lock().unwrap().extend_from_slice(&chunk[..n]),
            }
        }
    });

    let start = Instant::now();
    while start.elapsed().as_secs_f64() < case.secs {
        std::thread::sleep(Duration::from_millis(20));
    }
    let raw = buf.lock().unwrap().clone();
    if !matches!(child.try_wait(), Ok(Some(_))) {
        let _ = killer.kill();
    }

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
    let update = std::env::var("UPDATE_GOLDENS").is_ok();
    let mut failures = Vec::new();
    for case in CASES {
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
