//! Isolated #15 reproducer child: a quiet crossterm program, nothing else. It enters raw mode,
//! announces READY, then deliberately does NOT poll for 300 ms so the parent can make a key byte
//! and a SIGWINCH readable in the SAME readiness batch; then it polls and reports every event.
use crossterm::event::{poll, read, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io::Write;
use std::time::{Duration, Instant};

fn say(s: &str) {
    let mut o = std::io::stdout();
    let _ = write!(o, "{s}\r\n");
    let _ = o.flush();
}

fn main() {
    enable_raw_mode().expect("raw mode");
    // Prime crossterm's event source (installs the SIGWINCH source) before READY.
    let _ = poll(Duration::from_millis(10));
    say("READY");
    std::thread::sleep(Duration::from_millis(300));
    let start = Instant::now();
    let mut got_key = false;
    while start.elapsed() < Duration::from_millis(1500) {
        if poll(Duration::from_millis(100)).unwrap_or(false) {
            match read() {
                Ok(Event::Key(k)) => {
                    if let KeyCode::Char(c) = k.code {
                        say(&format!("EV key {c} at {}ms", start.elapsed().as_millis()));
                        got_key = true;
                        break;
                    }
                }
                Ok(Event::Resize(c, r)) => say(&format!("EV resize {c}x{r}")),
                Ok(_) => {}
                Err(e) => say(&format!("EV error {e}")),
            }
        }
    }
    if !got_key {
        say("STALLED");
    }
    let _ = disable_raw_mode();
}
