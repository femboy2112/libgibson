//! LibGibson's own Unix terminal input source (issue #269).
//!
//! When stdin is the controlling terminal, LibGibson reads and decodes it here instead
//! of through crossterm's event reader. crossterm 0.29.0's Unix (mio) source retries a
//! read that returns `0` or `EIO` forever, so a terminal that hangs up while a poll is
//! inside it spins the calling thread at 100% CPU and the poll never returns (#269).
//! Checking for a hangup *before* calling crossterm cannot close that: the hangup can
//! land after the check.
//!
//! Here the hangup is a **read result**, not a precheck: `read` failing with `EIO`, or
//! returning `0` while the descriptor reports `POLLHUP`, is a hangup, reported as
//! [`io::ErrorKind::BrokenPipe`]. (A `0` without `POLLHUP` is a canonical-mode EOF
//! character, ^D: consumed as input, not a hangup.) So
//! a hangup at any instant — before a poll, between the readiness wait and the read, or
//! during the read — ends the poll.
//!
//! # Contract
//!
//! [`poll`] waits at most `timeout` (a zero timeout checks once and never blocks) and:
//!
//! * returns `Ok(Some(event))` for the next decoded event, oldest first. Everything
//!   one read decodes is queued and handed out on later polls before the fd is consulted
//!   again, so no decoded event is ever left behind;
//! * returns `Ok(None)` when `timeout` passes with nothing decoded;
//! * returns `Err` with [`io::ErrorKind::BrokenPipe`] once the terminal has hung up.
//!   Events decoded **before** the hangup was read are still delivered first; every poll
//!   after that reports the hangup again. An unfinished escape or UTF-8 sequence that
//!   the hangup cut off is dropped (the source counts the bytes it drops).
//!
//! Every pass of the loop either returns, blocks in `poll(2)` for the time left, or
//! consumes at least one byte, so a poll is bounded by `timeout` plus the decoding of
//! what is already readable. There are no helper threads.
//!
//! # Identity
//!
//! It reads the descriptor crossterm's own sources read (`tty_fd` in crossterm 0.29's
//! `terminal/sys/file_descriptor.rs`): **stdin when stdin is a terminal, otherwise
//! `/dev/tty`**, chosen once when the source is created. The same descriptor is both
//! read and watched for the hangup. A session can be interactive with stdin redirected
//! (`app </dev/null` on a terminal): its keys then come from `/dev/tty`, as they did
//! through crossterm, and a hangup of that terminal is reported. (Before #269 the
//! hangup check watched stdin in that case, which never hangs up.)
//!
//! # Resize
//!
//! `SIGWINCH` is delivered through a `signal-hook` self-pipe, as in crossterm's own
//! sources (signal-hook chains with any other handler, crossterm's included). A resize
//! is queued as one `Resize(cols, rows)` with the size read when the signal is drained.
//! Terminal bytes and a resize that are ready together are both taken in the same pass:
//! level-triggered `poll(2)` cannot strand one behind the other (the crossterm mio
//! source can: issue #15).

mod parse;

use crossterm::event::Event as CtEvent;
use parse::{InternalEvent, Parser};
use std::collections::VecDeque;
use std::io::{self, Read};
use std::os::fd::RawFd;
use std::os::unix::net::UnixStream;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// One read's worth of terminal bytes; crossterm uses the same size. A full buffer
/// tells the decoder more bytes may follow (a lone `ESC` then waits for them).
const READ_BUF: usize = 1024;

/// The error a hung-up terminal reports. Same kind and message as before #269.
fn hung_up() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "terminal hung up (SIGHUP)")
}

/// How the terminal size is read for a resize event. Real sources ask crossterm
/// (`TIOCGWINSZ`); tests inject a fixed size.
type SizeFn = fn() -> io::Result<(u16, u16)>;

/// The decoding state for one terminal descriptor.
pub(crate) struct TtySource {
    fd: RawFd,
    /// Keeps an opened `/dev/tty` open for the source's lifetime (`None` for stdin).
    _owned: Option<std::os::fd::OwnedFd>,
    parser: Parser,
    /// Decoded events not yet handed out, oldest first.
    pending: VecDeque<CtEvent>,
    /// Read end of the `SIGWINCH` self-pipe (non-blocking), if resizes are watched.
    winch: Option<UnixStream>,
    size: SizeFn,
    /// Bytes of unfinished sequences discarded because a hangup ended them.
    dropped_partial_bytes: usize,
    /// Set once a hangup has been reported: from then on every poll reports it, and nothing
    /// observed afterwards (a later resize, say) is delivered as if the terminal were alive.
    hung_up: bool,
    /// The `SIGWINCH` registration feeding `winch`, removed when the source is dropped.
    winch_id: Option<signal_hook::SigId>,
}

impl Drop for TtySource {
    fn drop(&mut self) {
        if let Some(id) = self.winch_id.take() {
            signal_hook::low_level::unregister(id);
        }
    }
}

/// What one read of the terminal produced.
enum ReadOutcome {
    /// At least one byte was read and fed to the decoder.
    Bytes,
    /// `read` failed with `EIO`, or returned `0` on a descriptor reporting a hangup:
    /// the terminal is gone.
    HungUp,
    /// Readiness without data (`EAGAIN`) or an interrupted read: nothing this pass.
    Nothing,
    /// The terminal refuses the read for now without hanging up (`EIO` from a background
    /// read with `SIGTTIN` ignored): nothing this pass, retried at a slower pace.
    NotNow,
}

impl TtySource {
    /// A source over `fd` (not owned: never closed here). `winch` is the read end of a
    /// `SIGWINCH` self-pipe, if resizes should be reported.
    pub(crate) fn new(fd: RawFd, winch: Option<UnixStream>, size: SizeFn) -> io::Result<Self> {
        if let Some(w) = &winch {
            w.set_nonblocking(true)?;
        }
        Ok(Self {
            fd,
            _owned: None,
            parser: Parser::default(),
            pending: VecDeque::new(),
            winch,
            size,
            dropped_partial_bytes: 0,
            hung_up: false,
            winch_id: None,
        })
    }

    /// Bytes of unfinished escape/UTF-8 sequences dropped because a hangup ended them.
    #[cfg(test)]
    pub(crate) fn dropped_partial_bytes(&self) -> usize {
        self.dropped_partial_bytes
    }

    /// Decoded events waiting to be handed out.
    #[cfg(test)]
    pub(crate) fn queued(&self) -> usize {
        self.pending.len()
    }

    /// See the module documentation for the contract.
    pub(crate) fn poll(&mut self, timeout: Duration) -> io::Result<Option<CtEvent>> {
        let deadline = deadline_after(timeout);
        if self.hung_up {
            return Err(hung_up());
        }
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Ok(Some(event));
            }
            let left = match deadline {
                Some(d) => d.saturating_duration_since(Instant::now()),
                None => Duration::MAX,
            };
            let forever = deadline.is_none();

            let mut fds = [
                libc::pollfd {
                    fd: self.fd,
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: self.winch.as_ref().map_or(-1, |w| {
                        use std::os::fd::AsRawFd;
                        w.as_raw_fd()
                    }),
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            // SAFETY: `fds` is a valid array of two pollfds; a negative fd is ignored.
            let rc = unsafe { libc::poll(fds.as_mut_ptr(), 2, poll_timeout_ms(left, forever)) };
            if rc < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    // A signal (SIGWINCH, a handled SIGHUP, ...): re-check with the time
                    // that is left, or give up now if none is.
                    if left.is_zero() {
                        return Ok(None);
                    }
                    continue;
                }
                return Err(e);
            }
            if rc == 0 {
                // Timed out, unless this was one chunk of a wait longer than poll(2) can
                // express (or a forever wait): then wait again.
                if !forever && deadline.is_some_and(|d| Instant::now() >= d) {
                    return Ok(None);
                }
                continue;
            }

            if fds[1].revents & libc::POLLIN != 0 {
                self.take_resize();
            }

            let tty = fds[0].revents;
            if tty & libc::POLLNVAL != 0 {
                // The descriptor itself is gone (closed): nothing can be read again.
                return self.report_hangup();
            }
            if tty & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
                // A hangup is read, not assumed: on a pty whose other side closed,
                // bytes typed before the hangup are still returned first, then EIO.
                match self.read_once()? {
                    ReadOutcome::Bytes => {
                        self.decode();
                        // Input that keeps arriving must not stretch the call past its
                        // deadline (an unterminated paste decodes nothing until it ends).
                        if self.pending.is_empty() && deadline.is_some_and(|d| Instant::now() >= d)
                        {
                            return Ok(None);
                        }
                        continue;
                    }
                    ReadOutcome::HungUp => return self.report_hangup(),
                    ReadOutcome::NotNow => {
                        // Readable but refused (not in the foreground): wait out a slice
                        // rather than spin on a level-set POLLIN, until the deadline.
                        let now_left = remaining(deadline);
                        if now_left.is_zero() {
                            return Ok(None);
                        }
                        std::thread::sleep(now_left.min(Duration::from_millis(25)));
                    }
                    ReadOutcome::Nothing => {
                        // Readiness without data. With no time left, report the timeout;
                        // otherwise pace the retry so a level-set POLLIN that keeps
                        // yielding nothing cannot spin until the deadline.
                        if self.pending.is_empty() {
                            let now_left = remaining(deadline);
                            if now_left.is_zero() {
                                return Ok(None);
                            }
                            std::thread::sleep(now_left.min(Duration::from_millis(1)));
                        }
                    }
                }
            } else if left.is_zero() && self.pending.is_empty() {
                // Only the resize pipe was ready and it yielded no event (the size read
                // failed): nothing more to do in a zero-timeout poll.
                return Ok(None);
            }
        }
    }

    /// Reports the hangup, after any event decoded before it was seen. Once reported it is
    /// sticky: see `hung_up`.
    fn report_hangup(&mut self) -> io::Result<Option<CtEvent>> {
        if let Some(event) = self.pending.pop_front() {
            return Ok(Some(event));
        }
        self.hung_up = true;
        let cut_off = self.parser.pending_bytes();
        if cut_off > 0 {
            // An unfinished sequence can never complete now. Feed nothing more; just
            // forget it, so a later poll does not keep it as if more could come.
            self.dropped_partial_bytes += cut_off;
            self.parser = Parser::default();
        }
        Err(hung_up())
    }

    fn read_once(&mut self) -> io::Result<ReadOutcome> {
        let mut buf = [0u8; READ_BUF];
        // SAFETY: `buf` is a valid writable buffer of READ_BUF bytes; `self.fd` is a
        // descriptor the caller keeps open for the source's lifetime.
        let n = unsafe { libc::read(self.fd, buf.as_mut_ptr().cast(), READ_BUF) };
        if n > 0 {
            let n = n as usize;
            self.parser.advance(&buf[..n], n == READ_BUF);
            return Ok(ReadOutcome::Bytes);
        }
        if n == 0 {
            // End of file. On a hung-up terminal (or a pipe whose writer is gone) the fd
            // also reports POLLHUP, and every later read returns 0 again. A terminal in
            // canonical mode also returns 0 for the EOF character (^D at the start of a
            // line), once: that is input, not a hangup, and it has now been consumed.
            return Ok(if fd_reports_hangup(self.fd) {
                ReadOutcome::HungUp
            } else {
                ReadOutcome::Nothing
            });
        }
        let e = io::Error::last_os_error();
        read_error_outcome(e, fd_reports_hangup(self.fd))
    }

    /// Moves every event the parser completed into the hand-out queue.
    fn decode(&mut self) {
        for internal in self.parser.by_ref() {
            if let InternalEvent::Event(event) = internal {
                self.pending.push_back(event);
            }
            // Replies to terminal queries (cursor position, keyboard flags, device
            // attributes) are decoded so they never surface as keystrokes, then dropped:
            // LibGibson does not send those queries on this path.
        }
    }

    /// Drains the `SIGWINCH` pipe and queues one resize with the current size.
    fn take_resize(&mut self) {
        let Some(w) = self.winch.as_mut() else {
            return;
        };
        let mut sink = [0u8; 64];
        // Coalesce: any number of signals since the last drain is one resize.
        while matches!(w.read(&mut sink), Ok(n) if n > 0) {}
        // A size that cannot be read (the terminal is going away) yields no event; the
        // hangup itself is reported by the read.
        if let Ok((cols, rows)) = (self.size)() {
            self.pending.push_back(CtEvent::Resize(cols, rows));
        }
    }
}

/// How a failed `read` is classified. `EIO` is a hangup only when the descriptor also reports
/// one (`POLLHUP`, `POLLERR` or `POLLNVAL`): a background process reading its terminal with `SIGTTIN` ignored gets `EIO` too, and
/// that is recoverable (it reads normally once it is in the foreground again), so it is
/// "nothing yet", never a sticky hangup.
fn read_error_outcome(e: io::Error, fd_hung_up: bool) -> io::Result<ReadOutcome> {
    match e.raw_os_error() {
        Some(libc::EIO) if fd_hung_up => Ok(ReadOutcome::HungUp),
        Some(libc::EIO) => Ok(ReadOutcome::NotNow),
        Some(libc::EINTR) | Some(libc::EAGAIN) => Ok(ReadOutcome::Nothing),
        #[allow(unreachable_patterns)] // EWOULDBLOCK == EAGAIN on Linux
        Some(libc::EWOULDBLOCK) => Ok(ReadOutcome::Nothing),
        _ => Err(e),
    }
}

/// Time left until `deadline` right now (`Duration::MAX` without one).
fn remaining(deadline: Option<Instant>) -> Duration {
    deadline.map_or(Duration::MAX, |d| {
        d.saturating_duration_since(Instant::now())
    })
}

/// `now + timeout`, or a deadline so far ahead it is never reached (about 136 years) if that
/// overflows `Instant`: a finite timeout must never become an unconditional infinite wait.
/// `None` only if even that is unrepresentable.
fn deadline_after(timeout: Duration) -> Option<Instant> {
    let now = Instant::now();
    now.checked_add(timeout)
        .or_else(|| now.checked_add(Duration::from_secs(u64::from(u32::MAX))))
}

/// Whether `fd` reports a hangup, error or invalid descriptor right now (non-blocking).
fn fd_reports_hangup(fd: RawFd) -> bool {
    let mut p = libc::pollfd {
        fd,
        events: 0,
        revents: 0,
    };
    // SAFETY: one valid pollfd; a zero timeout never blocks.
    let rc = unsafe { libc::poll(&mut p, 1, 0) };
    rc > 0 && p.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0
}

/// `poll(2)` milliseconds for `left`: rounded **up**, so a short positive wait does not
/// become a zero-timeout spin. `-1` (wait indefinitely) only for a `forever` wait; a finite
/// wait longer than `c_int::MAX` ms is cut to that, and the caller re-checks its deadline.
fn poll_timeout_ms(left: Duration, forever: bool) -> libc::c_int {
    if forever {
        return -1;
    }
    if left.is_zero() {
        return 0;
    }
    let ms = left.as_nanos().div_ceil(1_000_000);
    libc::c_int::try_from(ms).unwrap_or(libc::c_int::MAX)
}

/// The process-wide source for the controlling terminal, created on first use. One per
/// process, like crossterm's global reader: there is one controlling terminal and one
/// `SIGWINCH`. The `u64` is the [`GENERATION`] it was created in.
static TERMINAL_SOURCE: Mutex<Option<(u64, TtySource)>> = Mutex::new(None);

/// Bumped by [`reset`]: a source from an older generation is dropped and rebuilt by the next
/// poll. Lets `reset` (session restore) return at once even while another thread is inside a
/// long poll holding the source.
static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The terminal input descriptor, chosen as crossterm's `tty_fd` chooses the terminal: stdin
/// when it is a terminal, otherwise `/dev/tty`. Whether stdin is the terminal is the identity
/// **latched when the session acquired the terminal** (`stdin_is_terminal`), not a fresh
/// `isatty`: stdin that hangs up before the first poll is still the terminal to report on, never
/// a reason to switch to `/dev/tty` (which may be another terminal).
///
/// It is our own open file description, **non-blocking**, where Linux lets us prove it is the
/// same terminal: `TIOCGDEV` (the real tty behind a descriptor, so a `/dev/tty` or `/dev/ptmx`
/// alias cannot pass for its target) must match. Then a read can never block (readiness can be
/// lost between `poll(2)` and `read`, for example when Ctrl-C flushes a canonical line) and
/// stdin's shared description keeps its flags. Where the identity cannot be proven (another OS,
/// no `/proc`, an exclusive-mode terminal that refuses a second open, a multiplexer alias),
/// stdin itself is read, blocking: the pre-#269 exposure to that lost-readiness race.
fn terminal_input_fd(stdin_is_terminal: bool) -> io::Result<(RawFd, Option<std::os::fd::OwnedFd>)> {
    select_input_fd(libc::STDIN_FILENO, stdin_is_terminal)
}

/// [`terminal_input_fd`] for a given stdin descriptor (testable without the process's own).
fn select_input_fd(
    stdin: RawFd,
    stdin_is_terminal: bool,
) -> io::Result<(RawFd, Option<std::os::fd::OwnedFd>)> {
    use std::os::fd::AsRawFd;
    if stdin_is_terminal {
        return Ok(match reopen_same_terminal(stdin) {
            Some(own) => (own.as_raw_fd(), Some(own)),
            None => (stdin, None),
        });
    }
    use std::os::unix::fs::OpenOptionsExt;
    let own = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY)
        .open("/dev/tty")
        .map_err(|e| match e.raw_os_error() {
            // No controlling terminal any more (ENXIO), or it is gone (EIO): it hung up.
            Some(libc::ENXIO) | Some(libc::EIO) => hung_up(),
            _ => e,
        })?;
    let own = std::os::fd::OwnedFd::from(own);
    Ok((own.as_raw_fd(), Some(own)))
}

/// A new non-blocking open file description of the terminal `fd` refers to, if it provably is
/// the same terminal; `None` otherwise. Proof, all three:
///
/// 1. `fd`'s file **is** its terminal, not an alias: its inode's device number equals
///    `TIOCGDEV` (the real tty behind the descriptor). `/dev/tty`, `/dev/console` and
///    `/dev/ptmx` fail this, since reopening them resolves afresh (to the current controlling
///    terminal, or to a new pty), and terminals of different devpts mounts can share numbers.
/// 2. The reopened file is the **same inode** (`st_dev` and `st_ino`).
/// 3. The reopened descriptor reports the same `TIOCGDEV`.
#[cfg(target_os = "linux")]
pub(crate) fn reopen_same_terminal(fd: RawFd) -> Option<std::os::fd::OwnedFd> {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;
    let dev = |fd: RawFd| {
        let mut d: libc::c_uint = 0;
        // SAFETY: TIOCGDEV writes one unsigned int; `fd` is a descriptor we hold.
        (unsafe { libc::ioctl(fd, libc::TIOCGDEV, &mut d) } == 0).then_some(d)
    };
    let stat = |fd: RawFd| {
        // SAFETY: fstat writes into a zeroed, correctly sized struct.
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        (unsafe { libc::fstat(fd, &mut st) } == 0).then_some(st)
    };
    // The kernel's encoding of a dev_t into TIOCGDEV's unsigned int (new_encode_dev).
    let encode = |rdev: libc::dev_t| {
        let (major, minor) = (libc::major(rdev), libc::minor(rdev));
        (minor & 0xff) | (major << 8) | ((minor & !0xff) << 12)
    };
    let real = dev(fd)?;
    let st = stat(fd)?;
    if encode(st.st_rdev) != real {
        return None; // an alias (/dev/tty, /dev/console, /dev/ptmx, ...): never reopened
    }
    let own: std::os::fd::OwnedFd = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY)
        .open(format!("/proc/self/fd/{fd}"))
        .ok()?
        .into();
    let st2 = stat(own.as_raw_fd())?;
    (st2.st_dev == st.st_dev && st2.st_ino == st.st_ino && dev(own.as_raw_fd()) == Some(real))
        .then_some(own)
}

/// See the Linux version: elsewhere the identity cannot be proven, so the caller reads `fd`.
#[cfg(not(target_os = "linux"))]
pub(crate) fn reopen_same_terminal(_fd: RawFd) -> Option<std::os::fd::OwnedFd> {
    None
}

/// Polls the controlling terminal. See the module documentation. `stdin_is_terminal` is the
/// identity latched when the terminal was acquired (see [`terminal_input_fd`]).
///
/// Waiting for another thread's poll counts against `timeout`: with the source busy past the
/// deadline this returns `Ok(None)` rather than queueing behind it.
pub(crate) fn poll(timeout: Duration, stdin_is_terminal: bool) -> io::Result<Option<CtEvent>> {
    use std::sync::atomic::Ordering;
    let deadline = deadline_after(timeout);
    let mut guard = loop {
        match TERMINAL_SOURCE.try_lock() {
            Ok(g) => break g,
            Err(std::sync::TryLockError::Poisoned(p)) => break p.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                if deadline.is_some_and(|d| Instant::now() >= d) {
                    return Ok(None);
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    };
    let generation = GENERATION.load(Ordering::Acquire);
    if guard.as_ref().is_some_and(|(g, _)| *g != generation) {
        *guard = None; // reset since it was created: a new session chooses afresh
    }
    if guard.is_none() {
        let (fd, owned) = terminal_input_fd(stdin_is_terminal)?;
        let (receiver, sender) = UnixStream::pair()?;
        sender.set_nonblocking(true)?;
        let id = signal_hook::low_level::pipe::register(libc::SIGWINCH, sender)?;
        let mut source = TtySource::new(fd, Some(receiver), crossterm::terminal::size)?;
        source._owned = owned;
        source.winch_id = Some(id);
        *guard = Some((generation, source));
    }
    let left = deadline.map_or(Duration::MAX, |d| {
        d.saturating_duration_since(Instant::now())
    });
    guard.as_mut().expect("initialised above").1.poll(left)
}

/// Retires the terminal source: called when the session gives the terminal up, so a later
/// session chooses its descriptor afresh with no stale events or hangup. Never blocks: a
/// source busy in another thread's poll is dropped by the next poll after that one. It retires,
/// it does not cancel: a poll already running keeps reading that source until it returns.
pub(crate) fn reset() {
    use std::sync::atomic::Ordering;
    GENERATION.fetch_add(1, Ordering::AcqRel);
    if let Ok(mut guard) = TERMINAL_SOURCE.try_lock() {
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    //! The hostile matrix of #269, in process, on real pseudo-terminal pairs. Every
    //! poll runs on a worker thread with a watchdog: a regression to the old behaviour
    //! is a spin, and it must fail the test, not hang it.
    use super::*;
    use crossterm::event::{KeyCode as K, KeyEvent, KeyModifiers as M};
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::sync::mpsc;
    use std::thread;

    const WATCHDOG: Duration = Duration::from_secs(5);
    const FIXED_SIZE: SizeFn = || Ok((100, 40));

    /// A raw-mode pty pair: `(master, slave)`, both close-on-exec.
    fn pty() -> (OwnedFd, OwnedFd) {
        let (m, s) = {
            use std::os::fd::FromRawFd;
            // posix_openpt + O_CLOEXEC on both ends: race-free close-on-exec, so no child
            // spawned by any test (in this or a parallel thread) inherits a master and keeps
            // a terminal alive that a test means to hang up.
            // SAFETY: plain libc pty calls; every fd returned is checked and then owned here.
            unsafe {
                let m = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC);
                assert!(m >= 0, "posix_openpt: {}", std::io::Error::last_os_error());
                let master = std::os::fd::OwnedFd::from_raw_fd(m);
                assert_eq!(libc::grantpt(m), 0);
                assert_eq!(libc::unlockpt(m), 0);
                let mut name = [0 as libc::c_char; 128];
                assert_eq!(libc::ptsname_r(m, name.as_mut_ptr(), name.len()), 0);
                let s = libc::open(
                    name.as_ptr(),
                    libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC,
                );
                assert!(s >= 0, "open slave: {}", std::io::Error::last_os_error());
                (master, std::os::fd::OwnedFd::from_raw_fd(s))
            }
        };
        // SAFETY: tcgetattr/tcsetattr on a valid tty fd with a zeroed termios.
        unsafe {
            let mut t: libc::termios = std::mem::zeroed();
            assert_eq!(libc::tcgetattr(s.as_raw_fd(), &mut t), 0);
            libc::cfmakeraw(&mut t);
            assert_eq!(libc::tcsetattr(s.as_raw_fd(), libc::TCSANOW, &t), 0);
        }
        (m, s)
    }

    fn write_all(fd: &OwnedFd, bytes: &[u8]) {
        // SAFETY: valid fd and buffer.
        let n = unsafe { libc::write(fd.as_raw_fd(), bytes.as_ptr().cast(), bytes.len()) };
        assert_eq!(
            n,
            bytes.len() as isize,
            "write: {}",
            io::Error::last_os_error()
        );
    }

    fn key(c: char) -> CtEvent {
        CtEvent::Key(KeyEvent::new(K::Char(c), M::NONE))
    }

    /// A source on `slave`, moved to a worker thread that runs `f` against it; the
    /// result must arrive within the watchdog or the test fails (a spin).
    fn on_worker<T: Send + 'static>(
        slave: OwnedFd,
        winch: Option<UnixStream>,
        f: impl FnOnce(&mut TtySource) -> T + Send + 'static,
    ) -> T {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut src = TtySource::new(slave.as_raw_fd(), winch, FIXED_SIZE).unwrap();
            let out = f(&mut src);
            drop(slave);
            let _ = tx.send(out);
        });
        rx.recv_timeout(WATCHDOG)
            .expect("the poll did not return within the watchdog: a spin or a block (#269)")
    }

    fn is_hangup(r: &io::Result<Option<CtEvent>>) -> bool {
        matches!(r, Err(e) if e.kind() == io::ErrorKind::BrokenPipe)
    }

    #[test]
    fn matrix_1_hangup_before_the_poll_is_reported_for_zero_and_finite_timeouts() {
        for timeout in [Duration::ZERO, Duration::from_millis(300), Duration::MAX] {
            let (master, slave) = pty();
            drop(master); // the terminal hangs up before anything polls it
            let (r1, r2, elapsed) = on_worker(slave, None, move |s| {
                let t = Instant::now();
                let r1 = s.poll(timeout);
                let r2 = s.poll(timeout);
                (r1, r2, t.elapsed())
            });
            assert!(is_hangup(&r1), "{timeout:?}: first poll {r1:?}");
            assert!(
                is_hangup(&r2),
                "{timeout:?}: the hangup is reported again: {r2:?}"
            );
            assert!(
                elapsed < Duration::from_secs(1),
                "{timeout:?}: took {elapsed:?}"
            );
        }
    }

    #[test]
    fn matrix_3_hangup_while_a_poll_is_blocked_ends_it() {
        // The poll is already waiting inside poll(2) when the terminal goes away.
        let (master, slave) = pty();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut src = TtySource::new(slave.as_raw_fd(), None, FIXED_SIZE).unwrap();
            let t = Instant::now();
            let r = src.poll(Duration::from_secs(30));
            let _ = tx.send((r, t.elapsed()));
            drop(slave);
        });
        thread::sleep(Duration::from_millis(150));
        drop(master);
        let (r, elapsed) = rx
            .recv_timeout(WATCHDOG)
            .expect("blocked poll never returned");
        assert!(is_hangup(&r), "{r:?}");
        assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");
    }

    #[test]
    fn matrix_5_keys_are_decoded_in_order_and_a_zero_timeout_still_reads() {
        // The zero-timeout read is the case crossterm's `use-dev-tty` source starves
        // (crossterm #839): it must deliver input that is already waiting.
        let (master, slave) = pty();
        write_all(&master, b"q\x1b[A\r\x7f\x03");
        let got = on_worker(slave, None, |s| {
            (0..6).map(|_| s.poll(Duration::ZERO)).collect::<Vec<_>>()
        });
        let want = [
            key('q'),
            CtEvent::Key(K::Up.into()),
            CtEvent::Key(K::Enter.into()),
            CtEvent::Key(K::Backspace.into()),
            CtEvent::Key(KeyEvent::new(K::Char('c'), M::CONTROL)),
        ];
        for (i, w) in want.iter().enumerate() {
            assert_eq!(
                got[i].as_ref().unwrap().as_ref(),
                Some(w),
                "event {i}: {got:?}"
            );
        }
        assert_eq!(got[5].as_ref().unwrap(), &None, "then nothing: {got:?}");
        drop(master);
    }

    #[test]
    fn matrix_5_6_a_resize_and_a_key_ready_together_are_both_delivered() {
        // #15's collision: a key and a SIGWINCH ready in the same wakeup. Neither may be
        // stranded behind the other, with no further input.
        let (master, slave) = pty();
        let (winch_rx, mut winch_tx) = UnixStream::pair().unwrap();
        use std::io::Write;
        write_all(&master, b"r");
        winch_tx.write_all(&[1]).unwrap();
        let got = on_worker(slave, Some(winch_rx), |s| {
            let a = s.poll(Duration::from_millis(500));
            let b = s.poll(Duration::ZERO);
            let c = s.poll(Duration::ZERO);
            (a, b, c)
        });
        let mut events = vec![got.0.unwrap(), got.1.unwrap()];
        assert_eq!(got.2.unwrap(), None, "exactly two events");
        events.sort_by_key(|e| format!("{e:?}"));
        assert_eq!(events, vec![Some(key('r')), Some(CtEvent::Resize(100, 40))]);
        drop(master);
    }

    #[test]
    fn matrix_5_resize_signals_coalesce_into_one_event() {
        let (_master, slave) = pty();
        let (winch_rx, mut winch_tx) = UnixStream::pair().unwrap();
        use std::io::Write;
        winch_tx.write_all(&[1, 1, 1]).unwrap();
        let got = on_worker(slave, Some(winch_rx), |s| {
            (s.poll(Duration::from_millis(500)), s.poll(Duration::ZERO))
        });
        assert_eq!(got.0.unwrap(), Some(CtEvent::Resize(100, 40)));
        assert_eq!(got.1.unwrap(), None);
    }

    #[test]
    fn matrix_7_fragmented_utf8_and_escape_sequences_reassemble() {
        // Lockstep: each fragment is written only after the worker has polled the
        // previous one, so every poll sees exactly the bytes written so far.
        let (master, slave) = pty();
        let (go_tx, go_rx) = mpsc::channel::<()>();
        let (res_tx, res_rx) = mpsc::channel();
        thread::spawn(move || {
            let mut src = TtySource::new(slave.as_raw_fd(), None, FIXED_SIZE).unwrap();
            for () in go_rx {
                let _ = res_tx.send(src.poll(Duration::from_millis(200)));
            }
            drop(slave);
        });
        // 'é' (C3 A9) split across two writes; ESC [ 1 ; 5 A split across three.
        let mut events = Vec::new();
        for part in [&b"\xc3"[..], b"\xa9", b"\x1b[", b"1;5", b"A"] {
            write_all(&master, part);
            go_tx.send(()).unwrap();
            events.push(
                res_rx
                    .recv_timeout(WATCHDOG)
                    .expect("poll returned")
                    .unwrap(),
            );
        }
        assert_eq!(
            events,
            vec![
                None, // first byte of é: incomplete, no event
                Some(key('é')),
                None, // ESC [ : incomplete
                None, // 1;5   : incomplete
                Some(CtEvent::Key(KeyEvent::new(K::Up, M::CONTROL))),
            ]
        );
        drop(go_tx);
        drop(master);
    }

    #[test]
    fn a_zero_timeout_poll_with_an_unfinished_sequence_does_not_block() {
        let (master, slave) = pty();
        write_all(&master, b"\x1b[1;");
        let (r, elapsed) = on_worker(slave, None, |s| {
            let t = Instant::now();
            (s.poll(Duration::ZERO), t.elapsed())
        });
        assert_eq!(r.unwrap(), None);
        assert!(elapsed < Duration::from_millis(100), "{elapsed:?}");
        drop(master);
    }

    #[test]
    fn matrix_7_a_sequence_cut_off_by_the_hangup_is_dropped_not_misdelivered() {
        let (master, slave) = pty();
        write_all(&master, b"\x1b[1;5"); // an unfinished CSI
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut src = TtySource::new(slave.as_raw_fd(), None, FIXED_SIZE).unwrap();
            let first = src.poll(Duration::from_millis(100)); // reads the fragment
            let _ = tx.send(("first", first, 0));
            thread::sleep(Duration::from_millis(200)); // master closes meanwhile
            let second = src.poll(Duration::from_millis(100));
            let _ = tx.send(("second", second, src.dropped_partial_bytes()));
            drop(slave);
        });
        let (_, first, _) = rx.recv_timeout(WATCHDOG).unwrap();
        assert_eq!(
            first.unwrap(),
            None,
            "an unfinished sequence is not an event"
        );
        drop(master);
        let (_, second, dropped) = rx.recv_timeout(WATCHDOG).unwrap();
        assert!(is_hangup(&second), "{second:?}");
        assert_eq!(
            dropped, 5,
            "the five cut-off bytes are dropped, not decoded"
        );
    }

    #[test]
    fn matrix_11_an_idle_poll_waits_its_timeout_without_spinning() {
        let (master, slave) = pty();
        let (r, elapsed, cpu) = on_worker(slave, None, |s| {
            let cpu0 = thread_cpu();
            let t = Instant::now();
            let r = s.poll(Duration::from_millis(300));
            (r, t.elapsed(), thread_cpu() - cpu0)
        });
        assert_eq!(r.unwrap(), None);
        assert!(
            elapsed >= Duration::from_millis(300),
            "returned early: {elapsed:?}"
        );
        assert!(
            cpu < Duration::from_millis(50),
            "idle poll used {cpu:?} of CPU"
        );
        drop(master);
    }

    #[test]
    fn a_key_ends_a_long_wait_at_once() {
        // The wait is in poll(2) on the terminal itself, so a key ends it as soon as it is
        // readable: no slice to sleep out (the pre-#269 loop slept 25 ms there, so every
        // trial took at least that). The best of five trials is bounded, so one trial
        // delayed by a loaded machine cannot fail the test.
        let mut latencies = Vec::new();
        for _ in 0..5 {
            let (master, slave) = pty();
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                let mut src = TtySource::new(slave.as_raw_fd(), None, FIXED_SIZE).unwrap();
                let r = src.poll(Duration::from_secs(10));
                let _ = tx.send((r, Instant::now()));
                drop(slave);
            });
            thread::sleep(Duration::from_millis(100)); // the worker is inside poll(2) by now
            let sent = Instant::now();
            write_all(&master, b"k");
            let (r, returned) = rx.recv_timeout(WATCHDOG).expect("the key ended the wait");
            assert_eq!(r.unwrap(), Some(key('k')));
            latencies.push(returned.saturating_duration_since(sent));
            drop(master);
        }
        let best = *latencies.iter().min().unwrap();
        assert!(
            best < Duration::from_millis(20),
            "key-to-return latencies {latencies:?}"
        );
    }

    #[test]
    fn matrix_12_events_decoded_before_a_hangup_are_delivered_first() {
        // Three keys arrive in one read; the terminal then hangs up. All three come out,
        // in order, and only then the hangup.
        let (master, slave) = pty();
        write_all(&master, b"xyz");
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut src = TtySource::new(slave.as_raw_fd(), None, FIXED_SIZE).unwrap();
            let first = src.poll(Duration::from_millis(500));
            let queued = src.queued();
            let _ = tx.send((vec![first], queued));
            thread::sleep(Duration::from_millis(200)); // master closes meanwhile
            let rest: Vec<_> = (0..4)
                .map(|_| src.poll(Duration::from_millis(100)))
                .collect();
            let _ = tx.send((rest, 0));
            drop(slave);
        });
        let (first, queued) = rx.recv_timeout(WATCHDOG).unwrap();
        assert_eq!(first[0].as_ref().unwrap(), &Some(key('x')));
        assert_eq!(
            queued, 2,
            "y and z were decoded by the same read and are queued"
        );
        drop(master);
        let (rest, _) = rx.recv_timeout(WATCHDOG).unwrap();
        assert_eq!(rest[0].as_ref().unwrap(), &Some(key('y')));
        assert_eq!(rest[1].as_ref().unwrap(), &Some(key('z')));
        assert!(is_hangup(&rest[2]), "{:?}", rest[2]);
        assert!(is_hangup(&rest[3]), "{:?}", rest[3]);
    }

    #[test]
    fn bytes_typed_before_the_hangup_but_not_yet_read_are_still_delivered() {
        // The bytes are in the pty's input queue when the master closes: Linux returns
        // them before EIO, and they must not be lost to an early hangup report.
        let (master, slave) = pty();
        write_all(&master, b"ok");
        drop(master);
        let got = on_worker(slave, None, |s| {
            (0..3)
                .map(|_| s.poll(Duration::from_millis(100)))
                .collect::<Vec<_>>()
        });
        let delivered: Vec<_> = got
            .iter()
            .take_while(|r| matches!(r, Ok(Some(_))))
            .map(|r| r.as_ref().unwrap().clone().unwrap())
            .collect();
        // Linux's pty keeps the queued input readable after the master closes. If a
        // platform discards it, the hangup must still follow at once (no spin), so only
        // the order is asserted strictly.
        assert!(
            delivered.is_empty() || delivered == vec![key('o'), key('k')],
            "{got:?}"
        );
        assert!(is_hangup(&got[delivered.len()]), "{got:?}");
    }

    #[test]
    fn a_canonical_mode_eof_character_is_input_not_a_hangup() {
        // ^D at the start of a line makes read() return 0 on a canonical-mode terminal
        // that is still alive. It must neither be reported as a hangup nor block the poll
        // past its timeout (the crossterm mio source blocks inside read() there).
        let (master, slave) = pty();
        // Back to canonical mode (pty() made it raw).
        // SAFETY: tcgetattr/tcsetattr on a valid tty fd.
        unsafe {
            let mut t: libc::termios = std::mem::zeroed();
            assert_eq!(libc::tcgetattr(slave.as_raw_fd(), &mut t), 0);
            t.c_lflag |= libc::ICANON;
            assert_eq!(libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &t), 0);
        }
        let (go_tx, go_rx) = mpsc::channel::<()>();
        let (res_tx, res_rx) = mpsc::channel();
        thread::spawn(move || {
            let mut src = TtySource::new(slave.as_raw_fd(), None, FIXED_SIZE).unwrap();
            for () in go_rx {
                let t = Instant::now();
                let r = src.poll(Duration::from_millis(200));
                let _ = res_tx.send((r, t.elapsed()));
            }
            drop(slave);
        });
        write_all(&master, b"\x04"); // VEOF on an empty line
        go_tx.send(()).unwrap();
        let (r, elapsed) = res_rx.recv_timeout(WATCHDOG).expect("poll returned");
        assert_eq!(r.unwrap(), None, "an EOF character is not a hangup");
        assert!(
            elapsed < Duration::from_secs(1),
            "the poll blocked: {elapsed:?}"
        );
        write_all(&master, b"z\n"); // the terminal is still alive and still delivers
        go_tx.send(()).unwrap();
        let (r, _) = res_rx.recv_timeout(WATCHDOG).expect("poll returned");
        assert_eq!(r.unwrap(), Some(key('z')));
        drop(go_tx);
        drop(master);
    }

    #[test]
    fn eof_on_a_pipe_is_a_hangup_not_a_spin() {
        // The other EOF shape: a descriptor that reads 0 forever.
        let mut fds = [0; 2];
        // SAFETY: pipe writes two fds.
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        // SAFETY: both fds are fresh and owned here.
        let (r, w) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
        drop(w);
        let res = on_worker(r, None, |s| s.poll(Duration::ZERO));
        assert!(is_hangup(&res), "{res:?}");
    }

    #[test]
    fn a_descriptor_that_is_not_open_is_a_hangup() {
        // POLLNVAL. A just-closed fd number cannot be used here: in a multi-threaded test
        // process another test may reopen it at once. A number above any open-file limit
        // is never open.
        let fd: RawFd = 1 << 30;
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut src = TtySource::new(fd, None, FIXED_SIZE).unwrap();
            let _ = tx.send(src.poll(Duration::from_millis(100)));
        });
        let r = rx.recv_timeout(WATCHDOG).expect("no spin on a closed fd");
        assert!(is_hangup(&r), "{r:?}");
    }

    #[test]
    fn a_reported_hangup_is_sticky_even_if_a_resize_follows() {
        // Codex review of #282: once BrokenPipe has been returned, a later SIGWINCH must not
        // make the source deliver a Resize as if the terminal were alive.
        let (master, slave) = pty();
        let (winch_rx, mut winch_tx) = UnixStream::pair().unwrap();
        drop(master);
        let (tx, rx) = mpsc::channel();
        let (go_tx, go_rx) = mpsc::channel::<()>();
        thread::spawn(move || {
            let mut src = TtySource::new(slave.as_raw_fd(), Some(winch_rx), FIXED_SIZE).unwrap();
            let first = src.poll(Duration::from_millis(100));
            let _ = tx.send(first);
            go_rx.recv().unwrap(); // the resize signal arrives now
            let later = src.poll(Duration::from_millis(100));
            let _ = tx.send(later);
            drop(slave);
        });
        assert!(is_hangup(&rx.recv_timeout(WATCHDOG).unwrap()));
        use std::io::Write;
        winch_tx.write_all(&[1]).unwrap();
        go_tx.send(()).unwrap();
        let later = rx.recv_timeout(WATCHDOG).unwrap();
        assert!(is_hangup(&later), "after the hangup: {later:?}");
    }

    #[test]
    fn input_that_keeps_arriving_does_not_stretch_a_poll_past_its_deadline() {
        // Codex review of #282: an unterminated bracketed paste decodes nothing, and a writer
        // that keeps it coming must not keep a 100 ms poll reading for its whole duration. A
        // pipe written at memory speed outpaces the decoder, so input is always ready: without
        // the deadline check after each read, the poll would run until the writer stops.
        let mut fds = [0; 2];
        // SAFETY: pipe writes two fds.
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        // SAFETY: both fds are fresh and owned here.
        let (r, w) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_w = stop.clone();
        let writer = thread::spawn(move || {
            write_all(&w, b"\x1b[200~");
            let chunk = [b'x'; 16 * 1024];
            let t = Instant::now();
            while !stop_w.load(std::sync::atomic::Ordering::Relaxed)
                && t.elapsed() < Duration::from_millis(1500)
            {
                // SAFETY: valid fd and buffer. Blocking: it waits while the pipe is full, and
                // fails (EPIPE) once the reader is gone, which ends the loop.
                let n = unsafe { libc::write(w.as_raw_fd(), chunk.as_ptr().cast(), chunk.len()) };
                if n < 0 {
                    break;
                }
            }
        });
        let (res, elapsed) = on_worker(r, None, |s| {
            thread::sleep(Duration::from_millis(50)); // the pipe is full and flowing
            let t = Instant::now();
            (s.poll(Duration::from_millis(100)), t.elapsed())
        });
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        writer.join().unwrap();
        assert_eq!(res.unwrap(), None, "an unfinished paste is not an event");
        assert!(
            elapsed < Duration::from_millis(600),
            "the 100 ms poll ran {elapsed:?} while input kept arriving"
        );
    }

    #[cfg(target_os = "linux")] // reopens through /proc/self/fd
    #[test]
    fn a_read_on_the_nonblocking_description_never_blocks_when_readiness_is_lost() {
        // Codex review of #282: in canonical mode Ctrl-C (or tcflush) can discard a line
        // between poll(2) reporting it and the read. On a blocking descriptor that read waits
        // for the next input. The source reads its own O_NONBLOCK description of the terminal,
        // where the same read reports nothing and returns.
        let (master, slave) = pty();
        // SAFETY: tcgetattr/tcsetattr/tcflush on a valid tty fd.
        unsafe {
            let mut t: libc::termios = std::mem::zeroed();
            assert_eq!(libc::tcgetattr(slave.as_raw_fd(), &mut t), 0);
            t.c_lflag |= libc::ICANON;
            assert_eq!(libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &t), 0);
        }
        write_all(&master, b"line\n");
        // Our own non-blocking description of the same terminal, as `terminal_input_fd` opens.
        use std::os::unix::fs::OpenOptionsExt;
        let own: OwnedFd = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY)
            .open(format!("/proc/self/fd/{}", slave.as_raw_fd()))
            .unwrap()
            .into();
        let mut p = libc::pollfd {
            fd: own.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd.
        assert_eq!(
            unsafe { libc::poll(&mut p, 1, 1000) },
            1,
            "the line is ready"
        );
        // SAFETY: valid tty fd.
        assert_eq!(
            unsafe { libc::tcflush(slave.as_raw_fd(), libc::TCIFLUSH) },
            0
        );
        let outcome = on_worker(own, None, |s| {
            matches!(s.read_once(), Ok(ReadOutcome::Nothing))
        });
        assert!(outcome, "the read must report nothing, not block or fail");
        drop((master, slave));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn reopening_proves_the_same_terminal_and_refuses_aliases() {
        // Codex re-review of #282: an equal st_rdev does not prove identity (a /dev/tty or
        // /dev/ptmx alias reopens as ANOTHER terminal). TIOCGDEV names the real tty.
        let (master, slave) = pty();
        let own = reopen_same_terminal(slave.as_raw_fd()).expect("a pts slave reopens");
        // SAFETY: fcntl F_GETFL on a valid descriptor.
        let fl = unsafe { libc::fcntl(own.as_raw_fd(), libc::F_GETFL) };
        assert_ne!(
            fl & libc::O_NONBLOCK,
            0,
            "the reopened description is non-blocking"
        );
        // SAFETY: as above, on the original.
        let orig = unsafe { libc::fcntl(slave.as_raw_fd(), libc::F_GETFL) };
        assert_eq!(
            orig & libc::O_NONBLOCK,
            0,
            "the original description keeps its flags"
        );
        // It reads the same terminal.
        write_all(&master, b"k");
        let got = on_worker(own, None, |s| s.poll(Duration::from_millis(500)));
        assert_eq!(got.unwrap(), Some(key('k')));
        // The multiplexer master: reopening /dev/ptmx would create a different pty.
        assert!(
            reopen_same_terminal(master.as_raw_fd()).is_none(),
            "a ptmx master must not be 'reopened' as a new pty"
        );
        // Not a terminal at all.
        let mut fds = [0; 2];
        // SAFETY: pipe writes two fds.
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        // SAFETY: both fds are fresh and owned here.
        let (r, _w) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
        assert!(reopen_same_terminal(r.as_raw_fd()).is_none());
        drop(slave);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_selector_reads_its_own_nonblocking_description_of_a_stdin_terminal() {
        // Codex review of #282: the lost-readiness fix depends on the SELECTOR returning a
        // non-blocking description, not on any helper. A selector that returned stdin itself
        // (blocking) fails here.
        let (master, slave) = pty();
        let (fd, owned) = select_input_fd(slave.as_raw_fd(), true).expect("select");
        assert!(
            owned.is_some(),
            "a reopened description, kept open by the source"
        );
        assert_ne!(
            fd,
            slave.as_raw_fd(),
            "not stdin's own (shared, blocking) description"
        );
        // SAFETY: fcntl F_GETFL on a valid descriptor.
        assert_ne!(
            unsafe { libc::fcntl(fd, libc::F_GETFL) } & libc::O_NONBLOCK,
            0
        );
        // And an alias of a terminal is never reopened: the multiplexer master falls back.
        let (fd2, owned2) = select_input_fd(master.as_raw_fd(), true).expect("select");
        assert!(owned2.is_none() && fd2 == master.as_raw_fd());
        drop((owned, slave));
    }

    /// Child half of [`a_dev_tty_alias_is_never_reopened`]: runs only in the child process
    /// that test spawns, whose controlling terminal it sets in `pre_exec`.
    #[cfg(target_os = "linux")]
    #[test]
    fn dev_tty_alias_child() {
        if std::env::var_os("GIBSON_ALIAS_CHILD").is_none() {
            return;
        }
        let tty: OwnedFd = std::fs::OpenOptions::new()
            .read(true)
            .open("/dev/tty")
            .expect("the child has a controlling terminal")
            .into();
        assert!(
            reopen_same_terminal(tty.as_raw_fd()).is_none(),
            "a /dev/tty alias must never be reopened (reopening re-resolves the controlling \
             terminal, which across devpts mounts can be another terminal with the same number)"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_dev_tty_alias_is_never_reopened() {
        // Codex round 4 of #282: an fd opened through /dev/tty, with the controlling terminal
        // unchanged, passes the TIOCGDEV and same-inode checks (both name /dev/tty and its
        // terminal); only the alias guard refuses it. The child test runs in a fresh process
        // whose controlling terminal is a pty slave (setsid + TIOCSCTTY before exec).
        use std::os::unix::process::CommandExt;
        let (_master, slave) = pty();
        let fd = slave.as_raw_fd();
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "--exact",
            "tty_source::tests::dev_tty_alias_child",
            "--nocapture",
        ])
        .env("GIBSON_ALIAS_CHILD", "1")
        .stdin(std::process::Stdio::null());
        // SAFETY: setsid and ioctl are async-signal-safe; nothing else runs between fork and
        // exec. The slave fd is close-on-exec but still open here.
        unsafe {
            cmd.pre_exec(move || {
                if libc::setsid() < 0 || libc::ioctl(fd, libc::TIOCSCTTY, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let out = cmd.output().expect("spawn the child test");
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success() && text.contains("1 passed"),
            "child: {:?}\n{text}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn eio_is_a_hangup_only_with_pollhup() {
        // Codex re-review of #282: a background read with SIGTTIN ignored also fails with EIO,
        // recoverably; only EIO on a descriptor that reports a hangup is one.
        let eio = || io::Error::from_raw_os_error(libc::EIO);
        assert!(matches!(
            read_error_outcome(eio(), true),
            Ok(ReadOutcome::HungUp)
        ));
        assert!(matches!(
            read_error_outcome(eio(), false),
            Ok(ReadOutcome::NotNow)
        ));
        let again = io::Error::from_raw_os_error(libc::EAGAIN);
        assert!(matches!(
            read_error_outcome(again, false),
            Ok(ReadOutcome::Nothing)
        ));
        let other = io::Error::from_raw_os_error(libc::EBADF);
        assert!(read_error_outcome(other, false).is_err());
    }

    #[test]
    fn reset_never_blocks_behind_a_poll_holding_the_source() {
        // Codex re-review of #282: session restore calls reset(); another thread may be inside
        // a long poll holding the source. reset must return at once and retire the source.
        use std::sync::atomic::Ordering;
        let (held_tx, held_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let holder = thread::spawn(move || {
            let _g = TERMINAL_SOURCE.lock().unwrap_or_else(|p| p.into_inner());
            held_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        held_rx.recv().unwrap();
        let before = GENERATION.load(Ordering::Acquire);
        let (done_tx, done_rx) = mpsc::channel();
        thread::spawn(move || {
            reset();
            let _ = done_tx.send(());
        });
        done_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("reset blocked behind the holder");
        assert_eq!(GENERATION.load(Ordering::Acquire), before + 1);
        release_tx.send(()).unwrap();
        holder.join().unwrap();
    }

    #[test]
    fn a_finite_timeout_never_becomes_an_unconditional_infinite_wait() {
        // Codex re-review of #282: `now + Duration::MAX` overflows Instant; that must still be
        // a (far) deadline, not "forever".
        assert!(deadline_after(Duration::MAX).is_some());
        assert!(deadline_after(Duration::from_secs(u64::MAX - 1)).is_some());
    }

    #[test]
    fn an_endless_unfinished_paste_cannot_hold_a_poll_past_its_deadline() {
        // Deterministic form of the deadline test: /dev/zero is always readable and never
        // ends, and inside a bracketed paste its NUL bytes decode to nothing. Without the
        // deadline check after each read, this poll would never return (the watchdog fails).
        let zero: OwnedFd = std::fs::File::open("/dev/zero").unwrap().into();
        let (res, elapsed) = on_worker(zero, None, |s| {
            s.parser.advance(b"\x1b[200~", true); // inside a paste that never ends
            let t = Instant::now();
            (s.poll(Duration::from_millis(100)), t.elapsed())
        });
        assert_eq!(res.unwrap(), None);
        assert!(elapsed < Duration::from_secs(2), "{elapsed:?}");
    }

    #[test]
    fn poll_timeout_rounds_up_and_saturates() {
        assert_eq!(poll_timeout_ms(Duration::ZERO, false), 0);
        assert_eq!(poll_timeout_ms(Duration::from_nanos(1), false), 1);
        assert_eq!(poll_timeout_ms(Duration::from_millis(1), false), 1);
        assert_eq!(poll_timeout_ms(Duration::from_micros(1500), false), 2);
        assert_eq!(poll_timeout_ms(Duration::from_secs(3), false), 3000);
        // A long finite wait is chunked, never turned into an infinite one.
        assert_eq!(
            poll_timeout_ms(Duration::from_secs(30 * 86_400), false),
            libc::c_int::MAX
        );
        assert_eq!(poll_timeout_ms(Duration::MAX, false), libc::c_int::MAX);
        assert_eq!(poll_timeout_ms(Duration::ZERO, true), -1);
    }

    fn thread_cpu() -> Duration {
        // SAFETY: getrusage writes into a zeroed, correctly sized struct.
        let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
        unsafe { libc::getrusage(libc::RUSAGE_THREAD, &mut ru) };
        Duration::from_micros(
            (ru.ru_utime.tv_sec + ru.ru_stime.tv_sec) as u64 * 1_000_000
                + (ru.ru_utime.tv_usec + ru.ru_stime.tv_usec) as u64,
        )
    }
}
