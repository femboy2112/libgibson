//! Linux PTY discriminator for input recovery versus reporting-request ordering.
//! A successful focus/mouse request is not terminal support or a promise to enter raw mode.
//! The parent injects reports; a FIFO freezes the child before polling, so the observed
//! ICANON/ECHO state and any echoed key precede input re-entry by construction.
#![cfg(target_os = "linux")]

#[path = "common/pty_capture.rs"]
mod pty_capture;

use gibson::context::{Context, RenderMode};
use gibson::input::{Event, FocusChange, KeyCode};
use portable_pty::CommandBuilder;
use pty_capture::Capture;
use std::ffi::CString;
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const ROLE: &str = "GIBSON_PANIC_FOCUS_ORDER_ROLE";
const GATE: &str = "GIBSON_PANIC_FOCUS_ORDER_GATE";
const READY: &[u8] = b"REPORT_RETRIES_READY";
const POLLING: &[u8] = b"POLL_BEGIN";
const INJECTION: &[u8] = b"\x1b[Iz\x1b[O";

struct Gate {
    dir: PathBuf,
    fifo: PathBuf,
}
impl Gate {
    fn new() -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "libgibson-panic-focus-{}-{stamp}-{sequence}",
            std::process::id()
        ));
        DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let gate = Self {
            fifo: dir.join("continue.fifo"),
            dir,
        };
        let path = CString::new(gate.fifo.as_os_str().as_bytes()).unwrap();
        // SAFETY: path is a live NUL-terminated path inside this newly created private directory.
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        gate
    }
    fn release(&self, cap: &mut Capture) {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
                .open(&self.fifo)
            {
                Ok(mut writer) => {
                    writer.write_all(b"!").unwrap();
                    return;
                }
                Err(e) if e.raw_os_error() == Some(libc::ENXIO) => {
                    assert!(
                        cap.try_wait().unwrap().is_none(),
                        "child exited before FIFO rendezvous"
                    );
                    assert!(
                        Instant::now() < deadline,
                        "child did not open its FIFO gate"
                    );
                    cap.collect_for(Duration::from_millis(5)).unwrap();
                }
                Err(e) => panic!("open FIFO gate: {e}"),
            }
        }
    }
}
impl Drop for Gate {
    fn drop(&mut self) {
        // These are the only two paths this fixture created; never recursively remove a tree.
        let _ = std::fs::remove_file(&self.fifo);
        let _ = std::fs::remove_dir(&self.dir);
    }
}

fn contains(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|part| part == needle)
}

struct Observation {
    raw_at_gate: bool,
    echoed_before_poll: bool,
    events: Vec<String>,
}

fn run_case(role: &str, require_raw: bool) -> Observation {
    let gate = Gate::new();
    let mut command = CommandBuilder::new(std::env::current_exe().unwrap());
    command.args([
        "--exact",
        "panic_focus_order_child",
        "--nocapture",
        "--test-threads=1",
    ]);
    command.env(ROLE, role);
    command.env(GATE, &gate.fifo);
    let mut cap = Capture::spawn(command, 80, 24, Duration::from_secs(15));
    cap.collect_until(|raw| contains(raw, READY)).unwrap();
    let gated_flags = cap.local_flags().unwrap();
    let mode = gated_flags & (libc::ICANON | libc::ECHO) as u64;
    let raw_at_gate = mode == 0;
    if require_raw {
        assert_eq!(
            mode, 0,
            "{role}: explicit input entry/control must be raw before injection"
        );
    } else {
        assert!(
            raw_at_gate || mode == (libc::ICANON | libc::ECHO) as u64,
            "{role}: unexpected half-raw state {mode:#x}"
        );
    }
    let injection_start = cap.raw().len();
    cap.write(INJECTION).unwrap();
    if !raw_at_gate {
        // Positive echo witness while the child is still blocked at its FIFO gate.
        // This waits for a byte; elapsed time is never used as evidence of a state change.
        cap.collect_until(|raw| raw[injection_start..].contains(&b'z'))
            .unwrap();
    }
    gate.release(&mut cap);
    cap.collect_until(|raw| contains(raw, b"ORDER_RESTORED=1"))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        cap.collect_for(Duration::from_millis(10)).unwrap();
        if let Some(status) = cap.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "child did not exit");
    };
    cap.collect_all().unwrap();
    assert!(status.success(), "{}", String::from_utf8_lossy(cap.raw()));
    let raw = cap.raw();
    let poll_start = raw
        .windows(POLLING.len())
        .position(|part| part == POLLING)
        .unwrap();
    assert!(poll_start >= injection_start);
    let echoed_before_poll = raw[injection_start..poll_start].contains(&b'z');
    assert_eq!(
        echoed_before_poll, !raw_at_gate,
        "{role}: echo disagrees with observed ECHO state"
    );
    let events: Vec<String> = String::from_utf8_lossy(raw)
        .split(['\r', '\n'])
        .filter_map(|line| {
            let rest = line.trim().trim_start_matches("\x1b[?2004h");
            if let Some(event) = rest.strip_prefix("ORDER_EVENT ") {
                Some(event.to_owned())
            } else if rest.contains("ORDER_EVENT ") {
                Some(format!("unrecognized-event-prefix:{rest:?}"))
            } else {
                None
            }
        })
        .collect();
    eprintln!("ORDER_OBSERVATION role={role} raw_at_gate={raw_at_gate} echoed_before_poll={echoed_before_poll} events={events:?}");
    let expected_requests = if role.starts_with("owner-") { 2 } else { 1 };
    for request in [
        b"\x1b[?1004h".as_slice(),
        b"\x1b[?1004l",
        b"\x1b[?1000h",
        b"\x1b[?1000l",
    ] {
        assert_eq!(
            raw.windows(request.len())
                .filter(|part| *part == request)
                .count(),
            expected_requests,
            "{role}: reporting request/cleanup count for {request:?}"
        );
    }
    if require_raw {
        assert!(!echoed_before_poll);
        assert_eq!(
            events,
            ["focus:gained", "key:z", "focus:lost"],
            "{role}: lost, duplicated or reordered input"
        );
    }
    Observation {
        raw_at_gate,
        echoed_before_poll,
        events,
    }
}

#[test]
fn owner_retry_before_input_entry_is_observed_without_claiming_input_readiness() {
    let observation = run_case("owner-before-entry", false);
    // This is a boundary characterization, not a requirement that setters remain cooked.
    // If they later enter raw mode, exact delivery is required just as for eager entry.
    if observation.raw_at_gate {
        assert!(!observation.echoed_before_poll);
        assert_eq!(observation.events, ["focus:gained", "key:z", "focus:lost"]);
    }
}

#[test]
fn eager_entry_after_owner_panic_preserves_focus_key_order_without_echo() {
    run_case("owner-eager-entry", true);
}

#[test]
fn no_panic_and_non_owner_panic_preserve_ready_input() {
    for role in ["no-panic", "worker-panic"] {
        run_case(role, true);
    }
}

fn local_flags() -> libc::tcflag_t {
    let mut state = std::mem::MaybeUninit::<libc::termios>::uninit();
    // SAFETY: stdin is this child's PTY; tcgetattr initializes state on success.
    assert_eq!(
        unsafe { libc::tcgetattr(libc::STDIN_FILENO, state.as_mut_ptr()) },
        0
    );
    unsafe { state.assume_init() }.c_lflag
}

#[test]
fn panic_focus_order_child() {
    let Ok(role) = std::env::var(ROLE) else {
        return;
    };
    let original_flags = local_flags();
    let mut ctx = Context::new(RenderMode::Inline).unwrap();
    ctx.enter_interactive().unwrap();
    ctx.session.set_focus_reporting(true).unwrap();
    ctx.session.set_mouse_capture(true).unwrap();
    match role.as_str() {
        "owner-before-entry" | "owner-eager-entry" => {
            assert!(
                std::panic::catch_unwind(|| panic!("caught owner panic for ordering probe"))
                    .is_err()
            );
        }
        "worker-panic" => {
            assert!(
                std::thread::spawn(|| panic!("caught worker panic for ordering probe"))
                    .join()
                    .is_err()
            );
        }
        "no-panic" => {}
        _ => panic!("unknown child role"),
    }
    if role == "owner-eager-entry" {
        ctx.enter_interactive().unwrap();
    }
    ctx.session.set_focus_reporting(true).unwrap();
    ctx.session.set_mouse_capture(true).unwrap();
    eprintln!("REPORT_RETRIES_READY");
    let mut fifo = File::open(std::env::var_os(GATE).unwrap()).unwrap();
    let mut byte = [0];
    fifo.read_exact(&mut byte).unwrap();
    assert_eq!(byte, [b'!']);
    eprintln!("POLL_BEGIN");
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut delivered = 0;
    while Instant::now() < deadline && delivered < 32 {
        let timeout = if delivered >= 3 {
            Duration::from_millis(30)
        } else {
            Duration::from_millis(100)
        };
        match ctx.poll_event(timeout).unwrap() {
            Some(event) => {
                let description = match event {
                    Event::Focus(FocusChange::Gained) => "focus:gained".to_owned(),
                    Event::Focus(FocusChange::Lost) => "focus:lost".to_owned(),
                    Event::Key(key)
                        if key.code == KeyCode::Char('z') && key.modifiers.is_empty() =>
                    {
                        "key:z".to_owned()
                    }
                    other => format!("unexpected:{other:?}"),
                };
                eprintln!("ORDER_EVENT {description}");
                delivered += 1;
            }
            None if delivered >= 3 => break,
            None => {}
        }
    }
    ctx.restore().unwrap();
    let restored = local_flags() == original_flags;
    eprintln!("ORDER_RESTORED={}", u8::from(restored));
    assert!(restored, "restore changed the original termios local flags");
}
