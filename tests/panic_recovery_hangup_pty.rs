//! Coupled caught-panic/input-entry/hangup acceptance on Linux PTYs.
//! Reports are files after hangup; no claim that a disconnected terminal's flags are restored.
#![cfg(target_os = "linux")]
#[path = "common/pty_capture.rs"]
mod pty_capture;
use gibson::{Context, RenderMode, TerminalLease, TerminalSession};
use portable_pty::CommandBuilder;
use std::{
    ffi::CString,
    fs::{DirBuilder, File, OpenOptions},
    io::{Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, OpenOptionsExt},
    },
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const ROLE: &str = "GIBSON_PANIC_RECOVERY_HANGUP_ROLE";
const GATE: &str = "GIBSON_PANIC_RECOVERY_HANGUP_GATE";
const RESULT: &str = "GIBSON_PANIC_RECOVERY_HANGUP_RESULT";
struct Fixture {
    dir: PathBuf,
    gate: PathBuf,
    result: PathBuf,
}
impl Fixture {
    fn new(role: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "libgibson-recovery-hup-{}-{stamp}-{role}",
            std::process::id()
        ));
        DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let fixture = Self {
            gate: dir.join("continue.fifo"),
            result: dir.join("receipt.txt"),
            dir,
        };
        let path = CString::new(fixture.gate.as_os_str().as_bytes()).unwrap();
        // SAFETY: a live NUL-terminated path in this fixture's newly created private directory.
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        fixture
    }
    fn rendezvous(&self, cap: &mut pty_capture::Capture) -> File {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
                .open(&self.gate)
            {
                Ok(writer) => return writer,
                Err(e) if e.raw_os_error() == Some(libc::ENXIO) => {
                    assert!(
                        cap.try_wait().unwrap().is_none(),
                        "child exited before rendezvous"
                    );
                    assert!(Instant::now() < deadline, "FIFO rendezvous timed out");
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(e) => panic!("FIFO open: {e}"),
            }
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("failed recovery fixture retained: {}", self.dir.display());
            return;
        }
        // Remove only the two known files and their empty private directory.
        let _ = std::fs::remove_file(&self.gate);
        let _ = std::fs::remove_file(&self.result);
        let _ = std::fs::remove_dir(&self.dir);
    }
}
fn run_case(role: &str) {
    let fixture = Fixture::new(role);
    let mut cmd = CommandBuilder::new(std::env::current_exe().unwrap());
    cmd.args([
        "--exact",
        "recovery_hangup_child",
        "--nocapture",
        "--test-threads=1",
    ]);
    cmd.env(ROLE, role);
    cmd.env(GATE, &fixture.gate);
    cmd.env(RESULT, &fixture.result);
    let mut cap = pty_capture::Capture::spawn(cmd, 80, 24, Duration::from_secs(12));
    cap.collect_until(|raw| raw.windows(18).any(|part| part == b"RECOVERY_HUP_READY"))
        .unwrap();
    let mut writer = fixture.rendezvous(&mut cap);
    let flags = cap.local_flags().unwrap() & (libc::ICANON | libc::ECHO) as u64;
    let expected = if role == "owner-before-entry" {
        (libc::ICANON | libc::ECHO) as u64
    } else {
        0
    };
    assert_eq!(flags, expected, "pre-hangup state {role}");
    assert!(String::from_utf8_lossy(cap.raw()).contains("INITIAL_RAW=1"));
    cap.hang_up();
    writer.write_all(b"!").unwrap();
    drop(writer);
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        if let Some(status) = cap.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let partial = std::fs::read_to_string(&fixture.result).unwrap_or_default();
            cap.terminate(); // A real hang is killed/reaped, not converted into a timeout pass.
            panic!("{role}: child exceeded hangup bound; partial receipt: {partial:?}");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let receipt = std::fs::read_to_string(&fixture.result).unwrap_or_default();
    assert!(
        status.success(),
        "{role}: child status={status:?}; receipt={receipt:?}"
    );
    assert_eq!(
        receipt
            .lines()
            .filter(|line| *line == "HUP_OBSERVED=1")
            .count(),
        1
    );
    assert!(
        receipt.lines().any(|line| line.starts_with("POLL_ERROR=")),
        "{receipt:?}"
    );
    assert_eq!(
        receipt
            .lines()
            .filter(|line| *line == "LEASE_AVAILABLE=1 SECOND_RESTORE_OK=1")
            .count(),
        1
    );
    assert_eq!(receipt.lines().last(), Some("DONE"));
    eprintln!("RECOVERY_HUP_OBSERVATION role={role} pre_hup_flags={flags:#x}\n{receipt}");
}
#[test]
fn caught_owner_panic_then_hangup_before_raw_reentry() {
    run_case("owner-before-entry");
}
#[test]
fn caught_owner_panic_then_eager_reentry_then_hangup() {
    run_case("owner-eager-entry");
}
#[test]
fn no_panic_hangup_control() {
    run_case("no-panic");
}
fn local_flags() -> libc::tcflag_t {
    let mut state = std::mem::MaybeUninit::<libc::termios>::uninit();
    // SAFETY: stdin is this child's live PTY and tcgetattr initializes state on success.
    assert_eq!(unsafe { libc::tcgetattr(0, state.as_mut_ptr()) }, 0);
    unsafe { state.assume_init() }.c_lflag
}
#[test]
fn recovery_hangup_child() {
    let Ok(role) = std::env::var(ROLE) else {
        return;
    };
    let receipt_path = PathBuf::from(std::env::var_os(RESULT).unwrap());
    let mut receipt = OpenOptions::new()
        .append(true)
        .create_new(true)
        .open(&receipt_path)
        .unwrap();
    writeln!(
        receipt,
        "CHILD_START role={role} pid={}",
        std::process::id()
    )
    .unwrap();
    // SAFETY: SIG_IGN installs no handler code; only this dedicated child is affected.
    assert_ne!(
        unsafe { libc::signal(libc::SIGHUP, libc::SIG_IGN) },
        libc::SIG_ERR
    );
    writeln!(receipt, "SIGHUP_IGNORE_INSTALLED").unwrap();
    let mut ctx = Context::new(RenderMode::Inline).unwrap();
    // Preserve the library's hook and record a failure before it attempts terminal output.
    let prior_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Ok(mut diagnostic) = OpenOptions::new().append(true).open(&receipt_path) {
            let _ = writeln!(diagnostic, "CHILD_PANIC {info}");
        }
        prior_hook(info);
    }));
    writeln!(receipt, "CONTEXT_CREATED").unwrap();
    ctx.enter_interactive().unwrap();
    writeln!(receipt, "INITIAL_FLAGS={:#x}", local_flags()).unwrap();
    assert_eq!(local_flags() & (libc::ICANON | libc::ECHO), 0);
    eprintln!("INITIAL_RAW=1");
    if role.starts_with("owner-") {
        writeln!(receipt, "OWNER_PANIC_BEGIN").unwrap();
        assert!(
            std::panic::catch_unwind(|| panic!("caught owner panic for hangup probe")).is_err()
        );
        writeln!(receipt, "OWNER_PANIC_CAUGHT flags={:#x}", local_flags()).unwrap();
        assert_eq!(
            local_flags() & (libc::ICANON | libc::ECHO),
            libc::ICANON | libc::ECHO
        );
    } else {
        assert_eq!(role, "no-panic");
    }
    if role == "owner-eager-entry" {
        ctx.enter_interactive().unwrap();
    }
    writeln!(receipt, "GATE_OPEN_WAIT flags={:#x}", local_flags()).unwrap();
    eprintln!("RECOVERY_HUP_READY");
    let mut gate = File::open(std::env::var_os(GATE).unwrap()).unwrap();
    writeln!(receipt, "GATE_READ_WAIT").unwrap();
    let mut token = [0];
    gate.read_exact(&mut token).unwrap();
    assert_eq!(token, [b'!']);
    writeln!(receipt, "GATE_RELEASED").unwrap();
    let mut fd = libc::pollfd {
        fd: 0,
        events: libc::POLLIN,
        revents: 0,
    };
    writeln!(receipt, "HUP_PRECHECK_BEGIN").unwrap();
    // Closing the parent's master does not prove the last master reference is gone yet.
    // Observe HUP before testing the already-disconnected input path; a timeout is a failure.
    let hup_deadline = Instant::now() + Duration::from_secs(1);
    let mut attempts = 0;
    loop {
        attempts += 1;
        // SAFETY: one initialized pollfd for the child's open terminal descriptor.
        let ready = unsafe { libc::poll(&mut fd, 1, 10) };
        let error = (ready < 0).then(std::io::Error::last_os_error);
        if attempts == 1 || fd.revents & (libc::POLLHUP | libc::POLLERR) != 0 {
            writeln!(
                receipt,
                "HUP_PRECHECK_RESULT attempt={attempts} rc={ready} revents={:#x} errno={:?}",
                fd.revents,
                error.as_ref().and_then(std::io::Error::raw_os_error)
            )
            .unwrap();
        }
        if let Some(error) = error {
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::Interrupted,
                "HUP observation failed"
            );
        } else if ready > 0 && fd.revents & (libc::POLLHUP | libc::POLLERR) != 0 {
            break;
        }
        assert!(
            Instant::now() < hup_deadline,
            "hangup was not observed within its bound"
        );
    }
    writeln!(receipt, "HUP_OBSERVED=1").unwrap();
    if std::env::var_os("GIBSON_TEST_STALL_RECOVERY_AFTER_HUP").is_some() {
        loop {
            std::thread::park();
        }
    }
    let began = Instant::now();
    let result = ctx.poll_event(Duration::from_millis(100));
    let elapsed = began.elapsed();
    match result {
        Err(error) => {
            writeln!(
                receipt,
                "POLL_ERROR={:?} elapsed_ms={}",
                error.kind(),
                elapsed.as_millis()
            )
            .unwrap();
            if role != "owner-before-entry" {
                assert_eq!(
                    error.kind(),
                    std::io::ErrorKind::BrokenPipe,
                    "an already-raw hung-up source must report BrokenPipe"
                );
            }
        }
        Ok(event) => {
            writeln!(receipt, "UNEXPECTED_POLL={event:?}").unwrap();
            panic!("hung-up input reported success");
        }
    }
    assert!(elapsed < Duration::from_secs(1), "poll was not prompt");
    writeln!(receipt, "RESTORE_BEGIN").unwrap();
    let began = Instant::now();
    let result = ctx.restore();
    writeln!(
        receipt,
        "RESTORE_RESULT={:?} elapsed_ms={}",
        result.as_ref().err().map(std::io::Error::kind),
        began.elapsed().as_millis()
    )
    .unwrap();
    assert!(
        began.elapsed() < Duration::from_secs(1),
        "restore was not prompt"
    );
    let available = TerminalSession::lease_state() == TerminalLease::Available;
    let second = ctx.restore().is_ok();
    writeln!(
        receipt,
        "LEASE_AVAILABLE={} SECOND_RESTORE_OK={}",
        u8::from(available),
        u8::from(second)
    )
    .unwrap();
    assert!(available && second);
    drop(ctx);
    assert_eq!(TerminalSession::lease_state(), TerminalLease::Available);
    writeln!(receipt, "DONE").unwrap();
    // The PTY is gone: avoid the test harness attempting its ordinary terminal summary.
    std::process::exit(0);
}
