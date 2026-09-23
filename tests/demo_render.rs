//! Structural virtual-terminal smoke tests for the full-screen showcase demos.
//!
//! These spawn the demo binaries in a real PTY, feed the complete byte stream
//! through `vt100`, and assert the dashboard structure appears and stays within
//! the terminal width. Run with `--nocapture` to print the reconstructed screen.

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn example_path(name: &str) -> std::path::PathBuf {
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

fn capture(name: &str, args: &[&str], cols: u16, rows: u16, secs: f64) -> (String, Vec<u8>, bool) {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("openpty");
    let exe = example_path(name);
    let mut cmd = CommandBuilder::new(&exe);
    for a in args {
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
    while start.elapsed().as_secs_f64() < secs {
        std::thread::sleep(Duration::from_millis(20));
    }
    let raw = buf.lock().unwrap().clone();
    let exited = matches!(child.try_wait(), Ok(Some(_)));
    if !exited {
        let _ = killer.kill();
    }

    let mut parser = vt100::Parser::new(rows, cols, 200);
    // Reconstruct the final ~screen by replaying the tail of the stream.
    parser.process(&raw);
    let screen = parser.screen().contents();
    (screen, raw, exited)
}

fn assert_fits(screen: &str, cols: u16) {
    for line in screen.lines() {
        let w = unicode_width::UnicodeWidthStr::width(line);
        assert!(
            w <= cols as usize,
            "line exceeds width {cols}: {line:?} ({w})"
        );
    }
}

#[test]
fn polished_agent_fullscreen_structure() {
    let (screen, raw, _) = capture(
        "polished_agent",
        &["--no-color", "--deterministic", "--freeze-at=40"],
        110,
        30,
        1.4,
    );
    if std::env::var("DUMP_SCREEN").is_ok() {
        let s = String::from_utf8_lossy(&raw);
        println!(
            "\n[diag] alt_h={} alt_l={} clear2j={} bytes={}",
            s.matches("\u{1b}[?1049h").count(),
            s.matches("\u{1b}[?1049l").count(),
            s.matches("\u{1b}[2J").count(),
            raw.len()
        );
        println!(
            "\n--- polished_agent @110x30 ---\n{screen}\n--- raw {} bytes ---",
            raw.len()
        );
    }
    assert!(
        screen.contains("LIBGIBSON AGENT"),
        "banner missing: {screen:?}"
    );
    assert!(screen.contains("TASK PLAN"), "plan panel missing");
    assert!(screen.contains("STREAM"), "stream panel missing");
    assert!(screen.contains("TRANSCRIPT"), "transcript panel missing");
    assert!(screen.contains("TELEMETRY"), "telemetry panel missing");
    assert!(
        screen.contains("PROMPT") || screen.contains("PERMISSION"),
        "footer panel missing: {screen:?}"
    );
    assert_fits(&screen, 110);
    assert!(
        !String::from_utf8_lossy(&raw).contains("\u{1b}[38;2;"),
        "--no-color must not emit truecolor foreground sequences"
    );
}

#[test]
fn polished_agent_narrow_structure() {
    let (screen, raw, _) = capture(
        "polished_agent",
        &["--no-color", "--deterministic", "--freeze-at=30"],
        52,
        20,
        1.2,
    );
    if std::env::var("DUMP_SCREEN").is_ok() {
        println!("\n--- polished_agent @52x20 ---\n{screen}\n");
    }
    assert!(screen.contains("TASK PLAN"));
    assert!(screen.contains("STREAM"));
    assert_fits(&screen, 52);
    assert!(!String::from_utf8_lossy(&raw).contains("\u{1b}[38;2;"));
}

#[test]
fn hack_the_gibson_fullscreen_structure() {
    let (screen, raw, _) = capture("hack_the_gibson", &["--no-color"], 120, 32, 3.0);
    if std::env::var("DUMP_SCREEN").is_ok() {
        println!("\n--- hack_the_gibson @120x32 ---\n{screen}\n");
    }
    assert!(
        screen.contains("GIBSON MAINFRAME"),
        "banner missing: {screen:?}"
    );
    assert!(screen.contains("MAINFRAME"), "mainframe panel missing");
    assert!(screen.contains("garbage.bin"), "garbage panel missing");
    assert!(screen.contains("DA VINCI SCAN"), "scan panel missing");
    assert!(screen.contains("TRANSFER"), "transfer panel missing");
    assert!(
        screen.contains("EVENT TRACE")
            || screen.contains("TACTICAL")
            || screen.contains("ROOT SHELL"),
        "side/bottom panels missing"
    );
    assert_fits(&screen, 120);
    assert!(
        !String::from_utf8_lossy(&raw).contains("\u{1b}[38;2;"),
        "--no-color must not emit truecolor foreground sequences"
    );
}

#[test]
fn hack_the_gibson_compact_structure() {
    let (screen, raw, _) = capture("hack_the_gibson", &["--no-color"], 56, 24, 3.0);
    if std::env::var("DUMP_SCREEN").is_ok() {
        println!("\n--- hack_the_gibson @56x24 ---\n{screen}\n");
    }
    assert!(screen.contains("MAINFRAME"), "mainframe missing");
    assert!(screen.contains("garbage.bin"), "garbage missing");
    assert_fits(&screen, 56);
    assert!(!String::from_utf8_lossy(&raw).contains("\u{1b}[38;2;"));
}

#[test]
fn fx_lab_wireframe_scene_structure() {
    let (screen, raw, _) = capture(
        "fx_lab",
        &[
            "--no-color",
            "--deterministic",
            "--freeze-at=30",
            "--scene=5",
        ],
        100,
        28,
        1.6,
    );
    if std::env::var("DUMP_SCREEN").is_ok() {
        println!("\n--- fx_lab scene 4 @100x28 ---\n{screen}\n");
    }
    assert!(screen.contains("FX LAB"), "fx_lab header missing");
    assert!(screen.contains("Wireframe torus"), "scene name missing");
    assert_fits(&screen, 100);
    assert!(!String::from_utf8_lossy(&raw).contains("\u{1b}[38;2;"));
}

#[test]
fn fx_lab_plasma_scene_structure() {
    let (screen, _raw, _) = capture(
        "fx_lab",
        &[
            "--deterministic",
            "--freeze-at=40",
            "--scene=2",
            "--truecolor",
        ],
        100,
        28,
        1.6,
    );
    if std::env::var("DUMP_SCREEN").is_ok() {
        println!("\n--- fx_lab scene 2 (plasma) @100x28 ---\n{screen}\n");
    }
    assert!(screen.contains("FX LAB"));
    assert!(screen.contains("HalfBlock plasma"));
    assert_fits(&screen, 100);
}
