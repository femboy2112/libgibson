//! Drive N trials of the coincident key+resize collision on a real PTY and classify each.
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

fn main() {
    let child_bin = std::env::args().nth(1).expect("child binary path");
    let trials: usize = std::env::args().nth(2).and_then(|n| n.parse().ok()).unwrap_or(30);
    let (mut delivered, mut stalled, mut other) = (0, 0, 0);
    for t in 0..trials {
        let pty = native_pty_system();
        let pair = pty.openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 }).unwrap();
        let mut child = pair.slave.spawn_command(CommandBuilder::new(&child_bin)).unwrap();
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().unwrap();
        let mut writer = pair.master.take_writer().unwrap();
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut b = [0u8; 4096];
            while let Ok(n) = reader.read(&mut b) {
                if n == 0 || tx.send(b[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        let mut out = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !String::from_utf8_lossy(&out).contains("READY") && Instant::now() < deadline {
            if let Ok(c) = rx.recv_timeout(Duration::from_millis(20)) {
                out.extend(c);
            }
        }
        // The calibrated collision (the lab's `coincident`): a resize (SIGWINCH), then 10 ms later a
        // key byte, both while the child is not polling — one readiness batch, resize first.
        let order = std::env::var("ORDER").unwrap_or_else(|_| "resize-key".into());
        let resize = |m: &Box<dyn portable_pty::MasterPty + Send>| {
            m.resize(PtySize { rows: 24 + (t % 3) as u16 + 1, cols: 80, pixel_width: 0, pixel_height: 0 }).unwrap()
        };
        if order == "resize-key" {
            resize(&pair.master);
            std::thread::sleep(Duration::from_millis(10));
            writer.write_all(b"r").unwrap();
            writer.flush().unwrap();
        } else {
            writer.write_all(b"r").unwrap();
            writer.flush().unwrap();
            std::thread::sleep(Duration::from_millis(10));
            resize(&pair.master);
        }
        let end = Instant::now() + Duration::from_secs(4);
        loop {
            if let Ok(c) = rx.recv_timeout(Duration::from_millis(20)) {
                out.extend(c);
            }
            let s = String::from_utf8_lossy(&out);
            if s.contains("EV key") || s.contains("STALLED") || Instant::now() > end {
                break;
            }
        }
        let s = String::from_utf8_lossy(&out).into_owned();
        if s.contains("EV key r") {
            delivered += 1;
        } else if s.contains("STALLED") {
            stalled += 1;
        } else {
            other += 1;
        }
        let _ = child.kill();
        let _ = child.wait();
    }
    println!("trials {trials}: delivered {delivered} · stalled {stalled} · other {other}");
}
