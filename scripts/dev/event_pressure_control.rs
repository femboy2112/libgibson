//! Independent upstream control: compiled by crossterm_resize_repro.py, no LibGibson.
use std::{
    fs::File,
    io::{self, Write},
    path::Path,
    thread,
    time::{Duration, Instant},
};

struct RawMode;
impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

fn main() -> io::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let mode = &args[1];
    let mut log = File::create(&args[2])?;
    let gate = Path::new(&args[3]);
    crossterm::terminal::enable_raw_mode()?;
    let _restore = RawMode;
    if mode == "crossterm" {
        // Install SIGWINCH handling and register readiness before parent acts.
        let _ = crossterm::event::poll(Duration::ZERO)?;
    }
    writeln!(log, "READY")?;
    let gate_start = Instant::now();
    while !gate.exists() {
        if gate_start.elapsed() > Duration::from_secs(2) {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "gate deadline"));
        }
        // Signal handlers run, but event polling is intentionally suspended.
        thread::sleep(Duration::from_millis(2));
    }
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(3) {
        let mut readable = 0;
        // Non-consuming measurement of the child TTY's queued input bytes.
        if unsafe { libc::ioctl(0, libc::FIONREAD, &mut readable) } == -1 {
            return Err(io::Error::last_os_error());
        }
        writeln!(log, "{} TTY_BYTES {readable}", start.elapsed().as_micros())?;
        if mode == "raw" {
            let mut fd = libc::pollfd {
                fd: 0,
                events: libc::POLLIN,
                revents: 0,
            };
            let ready = unsafe { libc::poll(&mut fd, 1, 20) };
            if ready < 0 {
                if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(io::Error::last_os_error());
            }
            if ready > 0 && fd.revents & libc::POLLIN != 0 {
                let mut bytes = [0_u8; 128];
                let count = unsafe { libc::read(0, bytes.as_mut_ptr().cast(), bytes.len()) };
                if count < 0 {
                    return Err(io::Error::last_os_error());
                }
                for byte in &bytes[..count as usize] {
                    writeln!(log, "{} KEY_BYTE {byte}", start.elapsed().as_micros())?;
                }
            }
        } else if crossterm::event::poll(Duration::from_millis(20))? {
            let event = crossterm::event::read()?;
            writeln!(log, "{} DECODED {event:?}", start.elapsed().as_micros())?;
            if let crossterm::event::Event::Key(key) = event {
                if let crossterm::event::KeyCode::Char(c) = key.code {
                    writeln!(
                        log,
                        "{} KEY_BYTE {}",
                        start.elapsed().as_micros(),
                        u32::from(c)
                    )?;
                }
            }
        }
    }
    Ok(())
}
