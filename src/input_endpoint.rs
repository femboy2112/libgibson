//! The acquired input endpoint of a managed terminal session (issue #345).
//!
//! A session that enters interactive mode **acquires** one terminal for input: stdin when it
//! is a terminal, otherwise `/dev/tty` (the same choice crossterm's `tty_fd` makes). This
//! module keeps a descriptor of its own for that terminal, so everything the lease does to
//! input afterwards targets the terminal it acquired, not whatever descriptor 0 or `/dev/tty`
//! names by then:
//!
//! * raw mode is entered on it and its termios baseline (flags, control characters, line
//!   discipline, speeds) is restored to it, never to a replacement stdin;
//! * the input source (`tty_source`) reads it, so a stdin replaced after acquisition is
//!   not read in its place;
//! * the parser's "is raw mode on" question ([`raw_active`]) answers for the lease's raw
//!   mode, which crossterm's process-global flag no longer tracks.
//!
//! crossterm's `enable_raw_mode`/`disable_raw_mode` resolve `tty_fd()` again on **each** call,
//! so after `dup2(B, 0)` its disable applied A's saved settings to B and left A raw (#345).
//!
//! **Host-owned crossterm raw mode.** When the host already entered raw mode through crossterm
//! before acquisition, the documented behaviour (restore disables raw mode) is kept by handing
//! the disable back to crossterm, but only after checking that crossterm would act on the
//! acquired terminal. If it would act on a different one, restore reports an error and changes
//! nothing (a safe refusal) rather than mutating an unrelated terminal.
//!
//! Private: no public type, no ABI change.

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

/// What the lease holds for its input terminal.
struct Acquired {
    /// Our own descriptor for the acquired terminal (a duplicate of stdin, or our own open of
    /// `/dev/tty`). Close-on-exec.
    fd: OwnedFd,
    /// Acquired through stdin (`true`) or `/dev/tty` (`false`).
    stdin_kind: bool,
    /// The termios found at raw entry, while LibGibson's own raw mode is applied.
    baseline: Option<libc::termios>,
    /// The host had entered raw mode through crossterm before acquisition: restore hands the
    /// disable back to crossterm (documented behaviour), guarded by an identity check.
    host_crossterm_raw: bool,
    /// The lease's raw mode was entered through crossterm (it resolved to the acquired
    /// terminal), so crossterm's process-global raw record stays truthful for hosts that also
    /// call crossterm's raw-mode functions.
    via_crossterm: bool,
}

/// crossterm's raw-mode record was left set by a lease whose restore could not go through
/// crossterm (its `tty_fd()` had been retargeted): the record describes a terminal LibGibson
/// already restored itself, so it must not be mistaken for a host's raw mode.
static CROSSTERM_RECORD_STALE: AtomicBool = AtomicBool::new(false);

fn crossterm_raw() -> bool {
    crossterm::terminal::is_raw_mode_enabled().unwrap_or(false)
}

static ACQUIRED: Mutex<Option<Acquired>> = Mutex::new(None);

/// Whether the lease's own raw mode is applied (read lock-free by the input parser).
static RAW_ACTIVE: AtomicBool = AtomicBool::new(false);

fn lock() -> MutexGuard<'static, Option<Acquired>> {
    ACQUIRED.lock().unwrap_or_else(|p| p.into_inner())
}

fn errno_retry(mut f: impl FnMut() -> libc::c_int) -> io::Result<()> {
    loop {
        if f() == 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        if err.kind() != io::ErrorKind::Interrupted {
            return Err(err);
        }
    }
}

fn get_attr(fd: RawFd) -> io::Result<libc::termios> {
    // SAFETY: tcgetattr fully initialises the struct on success.
    let mut t: libc::termios = unsafe { std::mem::zeroed() };
    errno_retry(|| unsafe { libc::tcgetattr(fd, &mut t) })?;
    Ok(t)
}

fn set_attr(fd: RawFd, t: &libc::termios) -> io::Result<()> {
    // TCSANOW, as crossterm applies both directions.
    errno_retry(|| unsafe { libc::tcsetattr(fd, libc::TCSANOW, t) })
}

/// Whether the lease's raw mode is applied. The input parser reads this in place of
/// crossterm's flag (a bare LF is Ctrl+J in raw mode, Enter otherwise).
pub(crate) fn raw_active() -> bool {
    RAW_ACTIVE.load(Ordering::Acquire)
        || (crossterm_raw() && !CROSSTERM_RECORD_STALE.load(Ordering::Acquire))
}

/// Acquires the input terminal, choosing as crossterm's `tty_fd` does. Returns the identity
/// kind (`true` for stdin) and whether this call made the acquisition (`false`: already held).
pub(crate) fn acquire() -> io::Result<(bool, bool)> {
    let mut guard = lock();
    if let Some(a) = guard.as_ref() {
        return Ok((a.stdin_kind, false));
    }
    // SAFETY: isatty/fcntl on descriptor 0 are always safe to call.
    let (fd, stdin_kind) = if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        let raw = unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_DUPFD_CLOEXEC, 3) };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: a fresh descriptor this call owns.
        (unsafe { OwnedFd::from_raw_fd(raw) }, true)
    } else {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open("/dev/tty")?;
        (OwnedFd::from(file), false)
    };
    *guard = Some(Acquired {
        fd,
        stdin_kind,
        baseline: None,
        host_crossterm_raw: false,
        via_crossterm: false,
    });
    Ok((stdin_kind, true))
}

/// A new close-on-exec duplicate of the acquired descriptor and its identity kind, for the
/// input source to read. `None` when nothing is acquired (the unmanaged public poll).
pub(crate) fn input_duplicate() -> Option<io::Result<(OwnedFd, bool)>> {
    let guard = lock();
    let a = guard.as_ref()?;
    // SAFETY: duplicating a descriptor we hold.
    let raw = unsafe { libc::fcntl(a.fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
    Some(if raw < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok((unsafe { OwnedFd::from_raw_fd(raw) }, a.stdin_kind))
    })
}

/// Enters raw mode on the acquired terminal (cfmakeraw, as crossterm does), saving its
/// baseline. Idempotent. A host that already entered crossterm raw mode keeps it untouched.
pub(crate) fn enter_raw() -> io::Result<()> {
    let mut guard = lock();
    let a = guard
        .as_mut()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "no acquired input terminal"))?;
    if a.baseline.is_some() || a.host_crossterm_raw {
        return Ok(());
    }
    let ct_raw = crossterm_raw();
    if !ct_raw {
        CROSSTERM_RECORD_STALE.store(false, Ordering::Release);
    }
    if ct_raw && !CROSSTERM_RECORD_STALE.load(Ordering::Acquire) {
        // crossterm's own enable would have been a no-op here; keep that.
        a.host_crossterm_raw = true;
        return Ok(());
    }
    let base = get_attr(a.fd.as_raw_fd())?;
    if !ct_raw && crossterm_target_is(a.fd.as_raw_fd()) {
        // Same terminal: enter through crossterm, so its global record (baseline + flag) agrees
        // with the lease for hosts that call crossterm's raw-mode functions too.
        crossterm::terminal::enable_raw_mode()?;
        a.via_crossterm = true;
    } else {
        let mut raw = base;
        // SAFETY: cfmakeraw only edits the struct.
        unsafe { libc::cfmakeraw(&mut raw) };
        set_attr(a.fd.as_raw_fd(), &raw)?;
        a.via_crossterm = false;
    }
    a.baseline = Some(base);
    RAW_ACTIVE.store(true, Ordering::Release);
    Ok(())
}

/// The terminal device behind `fd`, comparable across descriptors of the same terminal:
/// `TIOCGDEV` on Linux (resolves `/dev/tty` aliases), `st_rdev` elsewhere.
fn device_of(fd: RawFd) -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let mut d: libc::c_uint = 0;
        // SAFETY: TIOCGDEV writes one unsigned int.
        if unsafe { libc::ioctl(fd, libc::TIOCGDEV, &mut d) } == 0 {
            return Some(u64::from(d));
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        // SAFETY: fstat writes into a zeroed, correctly sized struct.
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        (unsafe { libc::fstat(fd, &mut st) } == 0).then(|| st.st_rdev as u64)
    }
}

/// Whether crossterm's `tty_fd()` would now resolve to the acquired terminal.
fn crossterm_target_is(acquired: RawFd) -> bool {
    let Some(want) = device_of(acquired) else {
        return false;
    };
    // SAFETY: isatty on descriptor 0 is always safe.
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        return device_of(libc::STDIN_FILENO) == Some(want);
    }
    use std::os::unix::fs::OpenOptionsExt;
    match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open("/dev/tty")
    {
        Ok(f) => device_of(f.as_raw_fd()) == Some(want),
        Err(_) => false,
    }
}

fn leave_raw_locked(a: &mut Acquired) -> io::Result<()> {
    if let Some(base) = a.baseline.take() {
        RAW_ACTIVE.store(false, Ordering::Release);
        if std::mem::take(&mut a.via_crossterm) {
            if crossterm_target_is(a.fd.as_raw_fd()) {
                // crossterm still resolves to the acquired terminal: let it restore (its
                // saved baseline is `base`) and clear its record.
                return crossterm::terminal::disable_raw_mode();
            }
            // Retargeted: restore the acquired terminal ourselves. crossterm's record cannot
            // be cleared without writing to the replacement; mark it stale instead.
            if crossterm_raw() {
                CROSSTERM_RECORD_STALE.store(true, Ordering::Release);
            }
        }
        return set_attr(a.fd.as_raw_fd(), &base);
    }
    if a.host_crossterm_raw {
        a.host_crossterm_raw = false;
        if !crossterm_raw() {
            return Ok(()); // the host already left raw mode itself
        }
        if !crossterm_target_is(a.fd.as_raw_fd()) {
            return Err(io::Error::other(
                "raw mode not restored: the host's crossterm raw mode now resolves to a \
                 terminal other than the one this session acquired (stdin or /dev/tty was \
                 replaced); no terminal settings were changed",
            ));
        }
        return crossterm::terminal::disable_raw_mode();
    }
    Ok(())
}

/// Restores the acquired terminal's baseline (or hands a host crossterm raw mode back, see
/// the module docs). The obligation is discharged even when the restore fails: a failed
/// `tcsetattr` on a hung-up terminal will not succeed on retry, and retrying must never
/// target another terminal.
pub(crate) fn leave_raw() -> io::Result<()> {
    match lock().as_mut() {
        Some(a) => leave_raw_locked(a),
        None => Ok(()),
    }
}

/// [`leave_raw`] for the panic hook: never blocks (a panic inside a locked section must not
/// deadlock the hook).
pub(crate) fn leave_raw_from_panic_hook() {
    // Bounded retries: another thread may briefly hold the lock (duplicating the descriptor
    // for its poll). Never a blocking lock: the panicking thread may hold it itself.
    for _ in 0..200 {
        match ACQUIRED.try_lock() {
            Ok(mut guard) => {
                if let Some(a) = guard.as_mut() {
                    let _ = leave_raw_locked(a);
                }
                return;
            }
            Err(std::sync::TryLockError::Poisoned(p)) => {
                if let Some(a) = p.into_inner().as_mut() {
                    let _ = leave_raw_locked(a);
                }
                return;
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                std::thread::sleep(std::time::Duration::from_micros(250));
            }
        }
    }
}

/// Gives the acquired terminal up (after [`leave_raw`]); closes our descriptor.
pub(crate) fn release() {
    let mut guard = lock();
    if let Some(a) = guard.as_mut() {
        // Never leave raw mode applied behind a released lease.
        let _ = leave_raw_locked(a);
    }
    *guard = None;
    RAW_ACTIVE.store(false, Ordering::Release);
}
