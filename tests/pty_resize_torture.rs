use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;

#[test]
fn test_pty_resize_torture() {
    let pty_system = native_pty_system();

    // 1. Start PTY at 120x30
    let mut pair = pty_system
        .openpty(PtySize {
            rows: 30,
            cols: 120,
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
        .join("resize_test_app");

    if !exe_path.exists() {
        let status = std::process::Command::new("cargo")
            .args(["build", "--example", "resize_test_app"])
            .status()
            .expect("Failed to build resize_test_app");
        assert!(status.success());
    }

    let mut cmd = CommandBuilder::new(&exe_path);
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .expect("Failed to spawn command");

    let mut writer = pair.master.take_writer().expect("take_writer");

    // 2. Type text
    std::thread::sleep(Duration::from_millis(100));
    writer.write_all(b"Hello world!").unwrap();
    writer.flush().unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // 3. Resize to 40x12
    pair.master
        .resize(PtySize {
            rows: 12,
            cols: 40,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("Failed to resize PTY");

    // 4. Continue typing
    std::thread::sleep(Duration::from_millis(100));
    writer.write_all(b" How are you?").unwrap();
    writer.flush().unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // 5. Insert output before live viewport
    writer.write_all(b"i").unwrap(); // Triggers ctx.insert_before_live
    writer.flush().unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // 6. Resize to 100x24
    pair.master
        .resize(PtySize {
            rows: 24,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("Failed to resize PTY");

    // 7. Continue typing
    std::thread::sleep(Duration::from_millis(100));
    writer.write_all(b" PTY is resizing.").unwrap();
    writer.flush().unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // 8. Commit output & exit
    writer.write_all(b"q").unwrap(); // Triggers exit
    writer.flush().unwrap();

    let _ = child.wait();
}
