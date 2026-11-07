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
        let deadline = Instant::now().checked_add(timeout);
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
                    ReadOutcome::Nothing => {
                        // Readiness without data. With no time left, report the timeout;
                        // otherwise pace the retry so a level-set POLLIN that keeps
                        // yielding nothing cannot spin until the deadline.
                        if self.pending.is_empty() {
                            if left.is_zero() {
                                return Ok(None);
                            }
                            std::thread::sleep(left.min(Duration::from_millis(1)));
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
        match e.raw_os_error() {
            Some(libc::EIO) => Ok(ReadOutcome::HungUp),
            Some(libc::EINTR) | Some(libc::EAGAIN) => Ok(ReadOutcome::Nothing),
            #[allow(unreachable_patterns)] // EWOULDBLOCK == EAGAIN on Linux
            Some(libc::EWOULDBLOCK) => Ok(ReadOutcome::Nothing),
            _ => Err(e),
        }
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

/// The process-wide source for the controlling terminal, created on first use and dropped
/// when the session gives the terminal up ([`reset`]). One per process, like crossterm's
/// global reader: there is one controlling terminal and one `SIGWINCH`.
static TERMINAL_SOURCE: Mutex<Option<TtySource>> = Mutex::new(None);

/// The terminal input descriptor, chosen as crossterm's `tty_fd` chooses the terminal:
/// stdin when it is a terminal, otherwise `/dev/tty`. It is our own open file description,
/// **non-blocking**, so a read can never block (readiness can be lost between `poll(2)` and
/// `read`, for example when Ctrl-C flushes a canonical line) and stdin's shared description
/// keeps its flags. Stdin is reopened through `/proc/self/fd/0` and kept only if it is the
/// same terminal device; if it cannot be, stdin itself is read (blocking: the pre-#269
/// exposure to that lost-readiness race).
fn terminal_input_fd() -> io::Result<(RawFd, Option<std::os::fd::OwnedFd>)> {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let reopen = |path: &str| {
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY)
            .open(path)
            .map(std::os::fd::OwnedFd::from)
    };
    // SAFETY: isatty is safe on any descriptor number.
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        let stdin_dev = std::fs::metadata("/proc/self/fd/0").map(|m| m.rdev()).ok();
        if let Ok(own) = reopen("/proc/self/fd/0") {
            let own_dev = std::fs::File::from(own.try_clone()?)
                .metadata()
                .map(|m| m.rdev())
                .ok();
            if own_dev.is_some() && own_dev == stdin_dev {
                return Ok((own.as_raw_fd(), Some(own)));
            }
        }
        return Ok((libc::STDIN_FILENO, None));
    }
    let own = reopen("/dev/tty")?;
    Ok((own.as_raw_fd(), Some(own)))
}

/// Polls the controlling terminal. See the module documentation.
///
/// Waiting for another thread's poll counts against `timeout`: with the source busy past the
/// deadline this returns `Ok(None)` rather than queueing behind it.
pub(crate) fn poll(timeout: Duration) -> io::Result<Option<CtEvent>> {
    let deadline = Instant::now().checked_add(timeout);
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
    if guard.is_none() {
        let (fd, owned) = terminal_input_fd()?;
        let (receiver, sender) = UnixStream::pair()?;
        sender.set_nonblocking(true)?;
        let id = signal_hook::low_level::pipe::register(libc::SIGWINCH, sender)?;
        let mut source = TtySource::new(fd, Some(receiver), crossterm::terminal::size)?;
        source._owned = owned;
        source.winch_id = Some(id);
        *guard = Some(source);
    }
    let left = deadline.map_or(Duration::MAX, |d| {
        d.saturating_duration_since(Instant::now())
    });
    guard.as_mut().expect("initialised above").poll(left)
}

/// Drops the terminal source. Called when a session gives the terminal up, so a later
/// session chooses its descriptor afresh and starts with no stale events or hangup.
pub(crate) fn reset() {
    let mut guard = TERMINAL_SOURCE.lock().unwrap_or_else(|p| p.into_inner());
    *guard = None;
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
