use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{BufRead, BufReader};
use std::time::Duration;

#[test]
fn test_pty_agent_chat_lifecycle() {
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("Failed to create PTY pair");

    let exe_path = std::env::current_exe()
        .expect("Failed to get test exe path")
        .parent()
        .expect("parent dir")
        .parent()
        .expect("target dir")
        .join("examples")
        .join("agent_chat");

    // If agent_chat hasn't been built yet in target/debug/examples, build it
    if !exe_path.exists() {
        let status = std::process::Command::new("cargo")
            .args(["build", "--example", "agent_chat"])
            .status()
            .expect("Failed to build agent_chat example");
        assert!(status.success());
    }

    let mut cmd = CommandBuilder::new(&exe_path);
    cmd.arg("--auto");

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .expect("Failed to spawn command");

    // Read output from master
    let reader = pair
        .master
        .try_clone_reader()
        .expect("Failed to get reader");
    let mut buf_reader = BufReader::new(reader);

    let mut output = String::new();
    let start = std::time::Instant::now();

    while start.elapsed() < Duration::from_secs(10) {
        let mut line = String::new();
        match buf_reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {
                output.push_str(&line);
                if output.contains("[metrics]") {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    let _ = child.wait();

    // Verify key invariants in output
    assert!(
        output.contains("LibGibson Engine"),
        "Output missing engine banner"
    );
    assert!(
        output.contains("Analyzing codebase architecture"),
        "Output missing assistant message"
    );
    assert!(
        output.contains("Allow command?"),
        "Output missing permission prompt"
    );
    assert!(
        output.contains("Permission granted: Yes, once"),
        "Output missing decision commit"
    );
    assert!(
        output.contains("[metrics]"),
        "Output missing performance metrics"
    );
}

#[test]
fn test_pty_window_resize_safety() {
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("Failed to create PTY pair");

    // Simulate resizing the terminal window during session
    pair.master
        .resize(PtySize {
            rows: 30,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("Failed to resize PTY");

    pair.master
        .resize(PtySize {
            rows: 15,
            cols: 40,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("Failed to shrink PTY");
}
