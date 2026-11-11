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

use crate::input::{KeyReleaseReporting, SourceEvent};
use crossterm::event::{
    Event as CtEvent, KeyCode as CtKeyCode, KeyEvent as CtKeyEvent, KeyEventKind,
    KeyboardEnhancementFlags,
};
use parse::{InternalEvent, Parser};
use std::collections::VecDeque;
use std::io::{self, Read};
use std::os::fd::RawFd;
use std::os::unix::net::UnixStream;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// One read's worth of terminal bytes; crossterm uses the same size. A full buffer
/// tells the decoder more bytes may follow (a lone `ESC` then waits for them), as does a read
/// ending in `ESC` while key-release reporting is confirmed (the Esc key is then `CSI 27u`) or its
/// answer is pending, for at most [`LONE_ESC_WAIT`].
const READ_BUF: usize = 1024;

/// How long a lone `ESC` held for the rest of a split sequence (see [`READ_BUF`]) waits before it
/// is delivered as the Esc key. The rest of a split sequence is the remainder of one terminal write
/// and follows at once; nothing following means the `ESC` was the key itself (the flags were
/// cleared behind LibGibson's back, by kitty's reset for instance). 50 ms is Neovim's default
/// `ttimeoutlen`, the same decision for the same bytes, and well under human reaction time.
const LONE_ESC_WAIT: Duration = Duration::from_millis(50);

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
    /// Decoded events not yet handed out, oldest first, each with the key-release epoch it was
    /// decoded in (an answer from an earlier epoch is dropped when handed out).
    pending: VecDeque<(u64, SourceEvent)>,
    /// Keys currently down, as their presses were decoded (#275): see [`PressedKeys`].
    pressed: PressedKeys,
    /// Read end of the `SIGWINCH` self-pipe (non-blocking), if resizes are watched.
    winch: Option<UnixStream>,
    size: SizeFn,
    /// Bytes of unfinished sequences discarded because a hangup ended them.
    dropped_partial_bytes: usize,
    /// When a held lone `ESC` (see [`LONE_ESC_WAIT`]) is delivered as Esc if nothing follows.
    esc_deadline: Option<Instant>,
    /// Set once a hangup has been reported: from then on every poll reports it, and nothing
    /// observed afterwards (a later resize, say) is delivered as if the terminal were alive.
    hung_up: bool,
    /// The `SIGWINCH` registration feeding `winch`, removed when the source is dropped.
    winch_id: Option<signal_hook::SigId>,
    /// Tests: name keys as while key releases are delivered, without the process-global switch.
    #[cfg(test)]
    track_keys: bool,
    /// Tests: an epoch to use instead of the process-global one.
    #[cfg(test)]
    test_epoch: Option<u64>,
}

impl TtySource {
    /// The key-release epoch now (see `crate::input::key_release_epoch`).
    fn epoch(&self) -> u64 {
        #[cfg(test)]
        if let Some(e) = self.test_epoch {
            return e;
        }
        crate::input::key_release_epoch()
    }

    fn queue(&mut self, ev: SourceEvent) {
        let epoch = self.epoch();
        self.pending.push_back((epoch, ev));
    }

    /// The next queued event to hand out. An answer or a key release decoded before reporting was
    /// last turned off or on is dropped (#275 review): the answer is to a request no one is waiting
    /// for, and disabling drops undelivered releases (an application then falls back to inferred
    /// holds). Presses and everything else are handed out whatever the epoch: they happened. So is
    /// the notice that reporting ended. Each comes with the epoch it was decoded in, which ties an
    /// answer to its request at hand-out (#275 review 8).
    fn next_pending(&mut self) -> Option<(u64, SourceEvent)> {
        let now = self.epoch();
        while let Some((epoch, ev)) = self.pending.pop_front() {
            let stale = epoch != now
                && match &ev {
                    SourceEvent::KeyReleaseReporting(_) => true,
                    SourceEvent::Terminal(CtEvent::Key(k)) => k.kind == KeyEventKind::Release,
                    SourceEvent::Terminal(_) | SourceEvent::ReportingEnded => false,
                };
            if !stale {
                return Some((epoch, ev));
            }
        }
        None
    }

    /// Queues an answer decoded in `epoch`, after the notice that reporting ended if the panic
    /// hook left one (#275 review 8): taken here or by the owner's next `Context` poll, whichever
    /// comes first, so it is said once, and never after the answer that follows the end.
    /// (Takes the queue, not `self`: it runs while the parser is being drained.)
    fn queue_answer(
        pending: &mut VecDeque<(u64, SourceEvent)>,
        epoch: u64,
        answer: KeyReleaseReporting,
    ) {
        if crate::session::take_key_releases_ended_notice() {
            pending.push_back((epoch, SourceEvent::ReportingEnded));
        }
        pending.push_back((epoch, SourceEvent::KeyReleaseReporting(answer)));
    }
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
            pressed: PressedKeys::default(),
            winch,
            size,
            dropped_partial_bytes: 0,
            esc_deadline: None,
            hung_up: false,
            winch_id: None,
            #[cfg(test)]
            track_keys: false,
            #[cfg(test)]
            test_epoch: None,
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

    /// [`poll_source`](Self::poll_source) for tests that expect terminal events only.
    #[cfg(test)]
    pub(crate) fn poll(&mut self, timeout: Duration) -> io::Result<Option<CtEvent>> {
        Ok(self.poll_source(timeout)?.map(|e| match e {
            SourceEvent::Terminal(e) => e,
            other => panic!("unexpected source event {other:?}"),
        }))
    }

    /// [`poll_tagged`](Self::poll_tagged) without the epochs.
    #[cfg(test)]
    pub(crate) fn poll_source(&mut self, timeout: Duration) -> io::Result<Option<SourceEvent>> {
        Ok(self.poll_tagged(timeout)?.map(|(_, e)| e))
    }

    /// See the module documentation for the contract. Each event comes with the key-release
    /// epoch it was decoded in.
    pub(crate) fn poll_tagged(
        &mut self,
        timeout: Duration,
    ) -> io::Result<Option<(u64, SourceEvent)>> {
        let deadline = deadline_after(timeout);
        if self.hung_up {
            return Err(hung_up());
        }
        loop {
            if let Some(event) = self.next_pending() {
                return Ok(Some(event));
            }
            if self.esc_deadline.is_some_and(|at| Instant::now() >= at) {
                // The rest may already be waiting (the events of a full read took longer than the
                // wait to hand out): read it before deciding the ESC was the Esc key.
                if fd_readable(self.fd) {
                    match self.read_once()? {
                        ReadOutcome::Bytes => {
                            self.decode();
                            continue;
                        }
                        ReadOutcome::HungUp => return self.report_hangup(),
                        ReadOutcome::Nothing | ReadOutcome::NotNow => {}
                    }
                }
                self.esc_deadline = None;
                self.parser.flush_lone_esc();
                self.decode();
                continue;
            }
            let left = match deadline {
                Some(d) => d.saturating_duration_since(Instant::now()),
                None => Duration::MAX,
            };
            let forever = deadline.is_none();
            // A held lone ESC shortens the wait to its own deadline.
            let (wait, wait_forever) = match self.esc_deadline {
                Some(at) => {
                    let esc_left = at.saturating_duration_since(Instant::now());
                    if forever || esc_left < left {
                        (esc_left, false)
                    } else {
                        (left, forever)
                    }
                }
                None => (left, forever),
            };

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
            let rc =
                unsafe { libc::poll(fds.as_mut_ptr(), 2, poll_timeout_ms(wait, wait_forever)) };
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
                // A held lone ESC that is now due is delivered before the poll times out.
                if self.esc_deadline.is_some_and(|at| Instant::now() >= at) {
                    continue;
                }
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
    fn report_hangup(&mut self) -> io::Result<Option<(u64, SourceEvent)>> {
        if let Some(event) = self.next_pending() {
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
            // While the flags are confirmed (or may be: the answer is pending) the Esc key is
            // `CSI 27u`, so a read ending in a lone ESC is a sequence split across reads: it waits
            // for the rest, up to `LONE_ESC_WAIT`.
            let split = buf[n - 1] == 0x1B && self.esc_may_be_split();
            self.parser.advance(&buf[..n], n == READ_BUF || split);
            self.esc_deadline =
                (split && self.parser.pending_bytes() == 1).then(|| Instant::now() + LONE_ESC_WAIT);
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

    /// Whether a read ending in a lone ESC may be a split sequence: key releases are delivered and
    /// the terminal's latest answer is `Active`, or a request's answer is still pending (it may be
    /// in this very read, decoded only after it).
    fn esc_may_be_split(&self) -> bool {
        #[cfg(test)]
        let on = self.track_keys || crate::input::key_releases_delivered();
        #[cfg(not(test))]
        let on = crate::input::key_releases_delivered();
        let q = lock_queries();
        on && (q.last == Some(KeyReleaseReporting::Active) || q.outstanding > 0)
    }

    /// Moves every event the parser completed into the hand-out queue.
    fn decode(&mut self) {
        let delivered = crate::input::key_releases_delivered();
        let epoch = self.epoch();
        #[cfg(test)]
        let track = self.track_keys || delivered;
        #[cfg(not(test))]
        let track = delivered;
        // Off, no release is reported, so no record can be trusted to still be down.
        if !track {
            self.pressed.clear();
        }
        for internal in self.parser.by_ref() {
            let event = match internal {
                InternalEvent::Event(CtEvent::Key(k)) if track => {
                    CtEvent::Key(self.pressed.name(KeyId::Code(k.code), k, epoch))
                }
                InternalEvent::KittyKey(k, base) if track => {
                    CtEvent::Key(self.pressed.name(KeyId::Base(base), k, epoch))
                }
                InternalEvent::Event(event) => event,
                InternalEvent::KittyKey(k, _) => CtEvent::Key(k),
                // The answers to a key-release reporting request (#275): see `KEYBOARD_QUERIES`.
                InternalEvent::KeyboardEnhancementFlags(flags) => {
                    let answer = lock_queries().flags_reply(flags, delivered);
                    if let Some(r) = answer {
                        Self::queue_answer(&mut self.pending, epoch, r);
                    }
                    continue;
                }
                InternalEvent::PrimaryDeviceAttributes => {
                    let answer = lock_queries().device_attributes_reply();
                    if let Some(r) = answer {
                        Self::queue_answer(&mut self.pending, epoch, r);
                    }
                    continue;
                }
                // A cursor position reply is decoded so it never surfaces as keystrokes, then
                // dropped: LibGibson does not send that query on this path.
                InternalEvent::CursorPosition(..) => continue,
            };
            self.pending
                .push_back((epoch, SourceEvent::Terminal(event)));
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
            self.queue(SourceEvent::Terminal(CtEvent::Resize(cols, rows)));
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

/// Whether `fd` has bytes to read right now (non-blocking).
fn fd_readable(fd: RawFd) -> bool {
    let mut p = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: one valid pollfd; a zero timeout never blocks.
    let rc = unsafe { libc::poll(&mut p, 1, 0) };
    rc > 0 && p.revents & libc::POLLIN != 0
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
///
/// Under a managed session the descriptor is the session's **acquired** input endpoint
/// (`crate::input_endpoint`, #345), never descriptor 0 as it is now: a stdin replaced after
/// acquisition is not read in the acquired terminal's place.
fn terminal_input_fd(stdin_is_terminal: bool) -> io::Result<(RawFd, Option<std::os::fd::OwnedFd>)> {
    use std::os::fd::AsRawFd;
    if let Some(acquired) = crate::input_endpoint::input_duplicate() {
        let (dup, stdin_kind) = acquired?;
        if stdin_kind {
            // As for stdin: a provably-same non-blocking reopen, else the shared description.
            if let Some(own) = reopen_same_terminal(dup.as_raw_fd()) {
                return Ok((own.as_raw_fd(), Some(own)));
            }
        }
        // Our own /dev/tty description (non-blocking), or stdin's shared one (blocking).
        return Ok((dup.as_raw_fd(), Some(dup)));
    }
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

/// A key's identity across its press, repeats and release (#275): the kitty unicode-key-code
/// when the key came as `CSI ... u`, otherwise its decoded code (an arrow's release is
/// `CSI 1;1:3D`, a legacy press of `w` is the byte `w`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyId {
    Base(u32),
    Code(CtKeyCode),
}

/// The keys currently down while key releases are delivered, each as its press was decoded, so
/// every repeat and the release of a key are named exactly as its press was (#275). A terminal
/// can name them differently: with shift let go first, kitty reports the release of a `W`
/// pressed with shift as `w`, and of a `!` as `1`.
/// The kitty functional-key codes: the Unicode private-use area U+E000-U+F8FF, where kitty sends
/// keypad keys (57399-57454), media keys and modifier keys. Codes above it are ordinary characters.
const KITTY_PRIVATE_USE_KEYS: std::ops::RangeInclusive<u32> = 57344..=63743;

/// Each record: the key, its press, a sequence number (recording order) and the key-release epoch
/// it was recorded in.
#[derive(Debug, Default)]
struct PressedKeys(Vec<(KeyId, CtKeyEvent, u64, u64)>, u64);

impl PressedKeys {
    /// More keys than any keyboard holds at once; the oldest is forgotten beyond it.
    const CAP: usize = 32;

    fn clear(&mut self) {
        self.0.clear();
    }

    /// The newest record (by sequence) that `want` accepts.
    fn newest(&self, want: impl Fn(&(KeyId, CtKeyEvent, u64, u64)) -> bool) -> Option<usize> {
        self.0
            .iter()
            .enumerate()
            .filter(|(_, r)| want(r))
            .max_by_key(|(_, r)| r.2)
            .map(|(n, _)| n)
    }

    /// The record a repeat or release of `id` (decoded as `code`) belongs to, the most recently
    /// recorded of: its own identity; a legacy record of the text key it decodes to, or of a
    /// character whose lowercase is its kitty base key (a legacy `W` is the key kitty releases as
    /// `w`); and a legacy record of any key it decodes to recorded in a later epoch than its own
    /// record (that one went up unreported while reporting was off, and the key was pressed again
    /// since).
    ///
    /// Only its own identity for a key kitty sends from the private-use area (57344-63743: the
    /// keypad, media and modifier keys): keypad Left decodes as the arrow Left and keypad `1` as the
    /// top-row `1`, so a legacy record of that code belongs to a different physical key (#275
    /// review 4).
    ///
    /// A legacy-recorded shifted *symbol* has no match here (#312): a `!` typed while the terminal
    /// is briefly in legacy mode — the pop/re-push window of a screen switch — is released by kitty
    /// under its base key `1`, and shift does not lowercase a symbol, so `!` does not fold to `1`.
    /// The release is delivered as `1`, so a held-key model keeps `!` held until a focus loss, the
    /// key's next kitty press/release (which moves the record), or reporting is disabled. It is
    /// deliberately NOT renamed: base `1` cannot be mapped back to `!` without assuming a keyboard
    /// layout, and pairing the unmatched release with the newest orphan would mis-identify an
    /// unrelated key's release. A legacy `W` pairs when the terminal reports `w` as its base;
    /// Unicode lowercase may expand to multiple characters, so it cannot match a single base key
    /// in every layout.
    /// None of those recovery events is guaranteed: the hold can persist indefinitely while
    /// reporting stays active. The `issue_312_*` tests pin both the failure and conditional recovery.
    fn find_down(&self, id: KeyId, code: CtKeyCode) -> Option<usize> {
        let legacy = KeyId::Code(code);
        let exact = self.newest(|r| r.0 == id);
        if matches!(id, KeyId::Base(b) if KITTY_PRIVATE_USE_KEYS.contains(&b)) {
            return exact;
        }
        let base = match id {
            KeyId::Base(b) => char::from_u32(b),
            KeyId::Code(_) => None,
        };
        let folds_to_base = |r: &(KeyId, CtKeyEvent, u64, u64)| match (r.0, base) {
            (KeyId::Code(CtKeyCode::Char(c)), Some(b)) => c.to_lowercase().eq([b]),
            _ => false,
        };
        let text = matches!(code, CtKeyCode::Char(_))
            .then(|| self.newest(|r| r.0 == legacy || folds_to_base(r)))
            .flatten();
        let superseding = exact.and_then(|e| {
            let epoch = self.0[e].3;
            self.newest(|r| r.0 == legacy && r.3 > epoch)
        });
        [exact, text, superseding]
            .into_iter()
            .flatten()
            .max_by_key(|&n| self.0[n].2)
    }

    /// `k` as the event to deliver: a press is recorded (replacing an earlier record of the same
    /// key); a repeat or release of a recorded key takes the recorded press's code and modifiers,
    /// and a release forgets it. A repeat of a key with no record (already down when reporting
    /// began) is delivered as decoded and recorded like a press, so its release takes that name; a
    /// release with no record is delivered as decoded. A kitty repeat that matched a legacy record
    /// moves the record to its own identity, so the release finds it whatever kitty names it then
    /// (a legacy `!`, released as `1`).
    ///
    /// A press is matched by its own identity only. A repeat or release also matches a legacy
    /// record of the text key it decodes to (a `w` typed before the flags were pushed, released as
    /// kitty's `CSI 119;1:3u`), taking the most recently recorded candidate, so a record left
    /// behind by a key that went up while reporting was off loses to the key's newer press. A
    /// non-text key never matches another key's record: keypad Left (kitty 57417) and the arrow
    /// Left decode to the same code.
    fn name(&mut self, id: KeyId, k: CtKeyEvent, epoch: u64) -> CtKeyEvent {
        self.1 += 1;
        let seq = self.1;
        let found = match k.kind {
            KeyEventKind::Press => self.0.iter().position(|r| r.0 == id),
            _ => self.find_down(id, k.code),
        };
        match (k.kind, found) {
            (KeyEventKind::Press, Some(i)) => {
                self.0[i] = (id, k, seq, epoch);
                k
            }
            (KeyEventKind::Press | KeyEventKind::Repeat, None) => {
                if self.0.len() == Self::CAP {
                    if let Some(oldest) = (0..self.0.len()).min_by_key(|&n| self.0[n].2) {
                        self.0.remove(oldest);
                    }
                }
                self.0.push((id, k, seq, epoch));
                k
            }
            (KeyEventKind::Repeat, Some(i)) => {
                let p = self.0[i].1;
                if self.0[i].0 != id {
                    // One record per key: any other record of `id` went up unreported.
                    let mut r = self.0.remove(i);
                    self.0.retain(|o| o.0 != id);
                    r.0 = id;
                    self.0.push(r);
                }
                CtKeyEvent::new_with_kind_and_state(p.code, p.modifiers, k.kind, k.state)
            }
            (KeyEventKind::Release, Some(i)) => {
                let (_, p, _, _) = self.0.remove(i);
                CtKeyEvent::new_with_kind_and_state(p.code, p.modifiers, k.kind, k.state)
            }
            (KeyEventKind::Release, None) => k,
        }
    }
}

/// Outstanding key-release reporting requests (#275). Each request is written as push flags,
/// `CSI ? u` (query the flags) and `CSI c` (primary device attributes). Every terminal answers the
/// last; only a kitty-protocol terminal answers the second, and it answers first. So per request:
/// a flags reply with every flag pushed (`KEY_RELEASE_FLAGS`) => `Active`; one without them all
/// => `Unsupported` (with fewer flags, Enter, Tab, Backspace and Esc release as a second press,
/// and shift+1 reads as `1`); the attributes reply with no flags reply before it => `Unsupported`.
///
/// A device attributes reply cannot be told from one to somebody else's earlier query, and a
/// stray one arriving first reads as `Unsupported`. A flags reply, though, states the flags in
/// force on the terminal, whoever asked: while reporting is enabled, one that changes the answer
/// is reported too (#275 review). The first `Unsupported` handed out, by any poll, is final: it
/// withdraws the request, and a later answer is dropped with it.
#[derive(Debug, Default)]
struct KeyboardQueries {
    /// Requests whose device attributes reply has not arrived.
    outstanding: u32,
    /// Whether the oldest outstanding request already got its flags reply.
    answered: bool,
    /// The last answer reported, so a repeated flags reply adds nothing.
    last: Option<KeyReleaseReporting>,
}

impl KeyboardQueries {
    /// A flags reply; `enabled`: key-release reporting is on.
    fn flags_reply(
        &mut self,
        flags: KeyboardEnhancementFlags,
        enabled: bool,
    ) -> Option<KeyReleaseReporting> {
        let pushed =
            KeyboardEnhancementFlags::from_bits_truncate(crate::session::KEY_RELEASE_FLAGS);
        let r = if flags.contains(pushed) {
            KeyReleaseReporting::Active
        } else {
            KeyReleaseReporting::Unsupported
        };
        if self.outstanding > 0 && !self.answered {
            self.answered = true;
        } else if !(enabled && self.last != Some(r)) {
            return None;
        }
        self.last = Some(r);
        Some(r)
    }

    fn device_attributes_reply(&mut self) -> Option<KeyReleaseReporting> {
        if self.outstanding == 0 {
            return None;
        }
        self.outstanding -= 1;
        if std::mem::take(&mut self.answered) {
            None
        } else {
            self.last = Some(KeyReleaseReporting::Unsupported);
            Some(KeyReleaseReporting::Unsupported)
        }
    }
}

static KEYBOARD_QUERIES: Mutex<KeyboardQueries> = Mutex::new(KeyboardQueries {
    outstanding: 0,
    answered: false,
    last: None,
});

fn lock_queries() -> std::sync::MutexGuard<'static, KeyboardQueries> {
    KEYBOARD_QUERIES
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

/// Records that a key-release reporting request was written; its answers arrive as
/// [`SourceEvent::KeyReleaseReporting`].
pub(crate) fn expect_keyboard_reply() {
    lock_queries().outstanding += 1;
}

/// Forgets outstanding requests: the session that wrote them restored the terminal.
pub(crate) fn forget_keyboard_replies() {
    *lock_queries() = KeyboardQueries::default();
}

/// Polls the controlling terminal. See the module documentation. `stdin_is_terminal` is the
/// identity latched when the terminal was acquired (see [`terminal_input_fd`]).
///
/// Waiting for another thread's poll counts against `timeout`: with the source busy past the
/// deadline this returns `Ok(None)` rather than queueing behind it.
pub(crate) fn poll(
    timeout: Duration,
    stdin_is_terminal: bool,
) -> io::Result<Option<(u64, SourceEvent)>> {
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
    let result = guard
        .as_mut()
        .expect("initialised above")
        .1
        .poll_tagged(left);
    if GENERATION.load(Ordering::Acquire) != generation {
        // The source was retired while this poll waited on it (a session acquired or
        // released the terminal). Its result belongs to no current lease: never deliver it
        // as the new owner's input. The retired source is dropped here.
        *guard = None;
        return Ok(None);
    }
    result
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

/// Serialises tests across modules (here and e.g. `session`) that mutate the process-global
/// keyboard-query record (`KEYBOARD_QUERIES`) or the key-release reporting globals, so they
/// never interleave.
#[cfg(test)]
static QUERIES_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Acquires [`QUERIES_LOCK`], tolerating poisoning from a panicking test.
#[cfg(test)]
pub(crate) fn queries_lock() -> std::sync::MutexGuard<'static, ()> {
    QUERIES_LOCK.lock().unwrap_or_else(|p| p.into_inner())
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

    // ------------------------------------------------ #275: key-release reporting (kitty)

    use crossterm::event::{KeyEventKind as Kind, KeyboardEnhancementFlags as F, ModifierKeyCode};

    fn kind(k: K, m: M, kind: Kind) -> CtEvent {
        CtEvent::Key(KeyEvent::new_with_kind(k, m, kind))
    }

    /// Every event `bytes` decodes to, polled on a worker with key naming on or off.
    fn decode_all(bytes: &'static [u8], track: bool) -> Vec<SourceEvent> {
        let (master, slave) = pty();
        write_all(&master, bytes);
        let got = on_worker(slave, None, move |s| {
            s.track_keys = track;
            let mut out = Vec::new();
            while let Ok(Some(e)) = s.poll_source(Duration::from_millis(200)) {
                out.push(e);
            }
            out
        });
        drop(master);
        got
    }

    fn terminal(events: Vec<SourceEvent>) -> Vec<CtEvent> {
        events
            .into_iter()
            .map(|e| match e {
                SourceEvent::Terminal(e) => e,
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    #[test]
    fn kitty_272_a_release_is_named_as_its_press_when_shift_goes_up_first() {
        // Bytes kitty 0.32.2 sent with flags 15 (captured under Xvfb, PR evidence): shift down,
        // w down (as W), shift up, then w up, which kitty names plain `w`.
        let got = terminal(decode_all(
            b"\x1b[57441;2u\x1b[119:87;2u\x1b[57441;1:3u\x1b[119;1:3u\
              \x1b[57441;2u\x1b[49:33;2u\x1b[57441;1:3u\x1b[49;1:3u",
            true,
        ));
        let shift = |kd| kind(K::Modifier(ModifierKeyCode::LeftShift), M::SHIFT, kd);
        assert_eq!(
            got,
            vec![
                shift(Kind::Press),
                kind(K::Char('W'), M::NONE, Kind::Press),
                kind(
                    K::Modifier(ModifierKeyCode::LeftShift),
                    M::SHIFT,
                    Kind::Release
                ),
                kind(K::Char('W'), M::NONE, Kind::Release),
                shift(Kind::Press),
                kind(K::Char('!'), M::NONE, Kind::Press),
                kind(
                    K::Modifier(ModifierKeyCode::LeftShift),
                    M::SHIFT,
                    Kind::Release
                ),
                kind(K::Char('!'), M::NONE, Kind::Release),
            ],
        );
    }

    #[test]
    fn kitty_272_without_naming_a_release_is_delivered_as_the_terminal_names_it() {
        // track=false reads the process-global delivery flag; serialise with anything that sets it.
        let _serial = queries_lock();
        let got = terminal(decode_all(b"\x1b[119:87;2u\x1b[119;1:3u", false));
        assert_eq!(
            got,
            vec![
                kind(K::Char('W'), M::NONE, Kind::Press),
                kind(K::Char('w'), M::NONE, Kind::Release),
            ],
        );
    }

    #[test]
    fn kitty_272_repeats_and_releases_of_special_keys_keep_the_press_name() {
        // Left with shift held at the press, repeated and released after shift; F5; Enter.
        let got = terminal(decode_all(
            b"\x1b[1;2D\x1b[1;2:2D\x1b[1;1:2D\x1b[1;1:3D\x1b[15~\x1b[15;1:3~\x1b[13u\x1b[13;1:3u",
            true,
        ));
        assert_eq!(
            got,
            vec![
                kind(K::Left, M::SHIFT, Kind::Press),
                kind(K::Left, M::SHIFT, Kind::Repeat),
                kind(K::Left, M::SHIFT, Kind::Repeat),
                kind(K::Left, M::SHIFT, Kind::Release),
                kind(K::F(5), M::NONE, Kind::Press),
                kind(K::F(5), M::NONE, Kind::Release),
                kind(K::Enter, M::NONE, Kind::Press),
                kind(K::Enter, M::NONE, Kind::Release),
            ],
        );
    }

    #[test]
    fn kitty_272_a_key_down_before_reporting_began_is_released_as_reported() {
        // A legacy `w` (pressed before the flags were pushed), then kitty's release of it.
        let got = terminal(decode_all(b"w\x1b[119;1:3u", true));
        assert_eq!(
            got,
            vec![key('w'), kind(K::Char('w'), M::NONE, Kind::Release)]
        );
    }

    #[test]
    fn kitty_272_the_record_of_keys_down_is_bounded() {
        let mut p = PressedKeys::default();
        for cp in 0..1000u32 {
            let c = char::from_u32(0x4e00 + cp).unwrap();
            p.name(KeyId::Base(cp), KeyEvent::new(K::Char(c), M::NONE), 0);
        }
        assert_eq!(p.0.len(), PressedKeys::CAP);
        // The newest is still named; the oldest was forgotten and is released as reported.
        let newest = p.name(
            KeyId::Base(999),
            KeyEvent::new_with_kind(K::Char('x'), M::NONE, Kind::Release),
            0,
        );
        assert_eq!(newest.code, K::Char(char::from_u32(0x4e00 + 999).unwrap()));
        let oldest = p.name(
            KeyId::Base(0),
            KeyEvent::new_with_kind(K::Char('x'), M::NONE, Kind::Release),
            0,
        );
        assert_eq!(oldest.code, K::Char('x'));
    }

    #[test]
    fn kitty_272_answers_are_paired_with_requests_in_order() {
        let active = F::DISAMBIGUATE_ESCAPE_CODES
            | F::REPORT_EVENT_TYPES
            | F::REPORT_ALTERNATE_KEYS
            | F::REPORT_ALL_KEYS_AS_ESCAPE_CODES;
        let mut q = KeyboardQueries::default();
        // Unsolicited replies (no request written) are ignored.
        assert_eq!(q.flags_reply(active, false), None);
        assert_eq!(q.device_attributes_reply(), None);
        // kitty: flags reply, then device attributes.
        q.outstanding += 1;
        assert_eq!(
            q.flags_reply(active, false),
            Some(KeyReleaseReporting::Active)
        );
        assert_eq!(q.device_attributes_reply(), None);
        // A terminal that only answers device attributes.
        q.outstanding += 1;
        assert_eq!(
            q.device_attributes_reply(),
            Some(KeyReleaseReporting::Unsupported)
        );
        // A flags reply without event types (the push was not honoured).
        q.outstanding += 1;
        assert_eq!(
            q.flags_reply(F::empty(), false),
            Some(KeyReleaseReporting::Unsupported)
        );
        assert_eq!(q.device_attributes_reply(), None);
        // Two requests in flight (an alternate-screen move re-asks): answered one by one.
        q.outstanding += 2;
        assert_eq!(
            q.flags_reply(active, false),
            Some(KeyReleaseReporting::Active)
        );
        assert_eq!(q.flags_reply(active, false), None, "one answer per request");
        assert_eq!(q.device_attributes_reply(), None);
        assert_eq!(
            q.flags_reply(active, false),
            Some(KeyReleaseReporting::Active)
        );
        assert_eq!(q.device_attributes_reply(), None);
        assert_eq!(q.outstanding, 0);
        assert_eq!(q.device_attributes_reply(), None);
    }

    #[test]
    fn kitty_272_the_answer_is_delivered_in_input_order() {
        // Touches the process-global request record: see `QUERIES_LOCK`.
        let _serial = queries_lock();
        forget_keyboard_replies();
        // An answer decoded before a toggle is dropped when handed out after it (#275 re-review).
        expect_keyboard_reply();
        let (master, slave) = pty();
        let stale = on_worker(slave, None, move |s| {
            s.test_epoch = Some(1);
            write_all(&master, b"a\x1b[?15u\x1b[?62;c");
            let a = s.poll_source(Duration::from_millis(500)).unwrap();
            s.test_epoch = Some(2);
            let next = s.poll_source(Duration::from_millis(200)).unwrap();
            drop(master);
            (a, next)
        });
        assert_eq!(stale, (Some(SourceEvent::Terminal(key('a'))), None));
        forget_keyboard_replies();
        expect_keyboard_reply();
        let got = decode_all(b"a\x1b[?15u\x1b[?62;cb\x1b[?15u\x1b[?62;c", false);
        forget_keyboard_replies();
        assert_eq!(
            got,
            vec![
                SourceEvent::Terminal(key('a')),
                SourceEvent::KeyReleaseReporting(KeyReleaseReporting::Active),
                SourceEvent::Terminal(key('b')),
                // A second, unsolicited pair of replies produces nothing, and no keystrokes.
            ],
        );
    }

    #[test]
    fn kitty_272_a_stray_device_attributes_reply_is_corrected_by_the_flags_reply() {
        // #275 review: somebody else's earlier `CSI c` is answered after the request was written.
        let active = F::DISAMBIGUATE_ESCAPE_CODES
            | F::REPORT_EVENT_TYPES
            | F::REPORT_ALTERNATE_KEYS
            | F::REPORT_ALL_KEYS_AS_ESCAPE_CODES;
        let mut q = KeyboardQueries::default();
        q.outstanding += 1;
        // stray DA1, then kitty's own flags reply and DA1
        assert_eq!(
            q.device_attributes_reply(),
            Some(KeyReleaseReporting::Unsupported)
        );
        assert_eq!(
            q.flags_reply(active, true),
            Some(KeyReleaseReporting::Active)
        );
        assert_eq!(q.device_attributes_reply(), None);
        // A repeated flags reply adds nothing; with reporting off, nothing at all.
        assert_eq!(q.flags_reply(active, true), None);
        let mut off = KeyboardQueries::default();
        assert_eq!(off.flags_reply(active, false), None);
    }

    #[test]
    fn kitty_272_a_keypad_key_never_takes_over_the_arrow_with_the_same_code() {
        // #275 review: shift+Left down, then keypad Left with ctrl (kitty 57417), then the arrow
        // released after the modifiers: its release keeps the shift+Left name.
        let got = terminal(decode_all(
            b"\x1b[1;2D\x1b[57417;5u\x1b[1;1:3D\x1b[57417;1:3u",
            true,
        ));
        let names: Vec<(K, M, Kind)> = got
            .iter()
            .map(|e| match e {
                CtEvent::Key(k) => (k.code, k.modifiers, k.kind),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(names[0], (K::Left, M::SHIFT, Kind::Press));
        assert_eq!(names[1], (K::Left, M::CONTROL, Kind::Press));
        assert_eq!(
            names[2],
            (K::Left, M::SHIFT, Kind::Release),
            "the arrow's own name"
        );
        assert_eq!(
            names[3],
            (K::Left, M::CONTROL, Kind::Release),
            "the keypad key's own name"
        );
    }

    #[test]
    fn kitty_272_a_newer_legacy_press_wins_over_a_stale_record() {
        // #275 review: W pressed with shift while reporting was on; reporting toggled (W went up
        // unreported); then a legacy `w` and kitty's release of it. The release names the newer
        // `w`, not the stale `W`.
        let (master, slave) = pty();
        let got = on_worker(slave, None, move |s| {
            s.track_keys = true;
            s.test_epoch = Some(1);
            write_all(&master, b"\x1b[119:87;2u");
            let first = s.poll_source(Duration::from_millis(500));
            s.test_epoch = Some(2); // disabled and re-enabled since
            write_all(&master, b"w\x1b[119;1:3u");
            let mut rest = Vec::new();
            while let Ok(Some(e)) = s.poll_source(Duration::from_millis(200)) {
                rest.push(e);
            }
            drop(master);
            (first, rest)
        });
        assert_eq!(
            got.0.unwrap(),
            Some(SourceEvent::Terminal(kind(
                K::Char('W'),
                M::NONE,
                Kind::Press
            )))
        );
        assert_eq!(
            terminal(got.1),
            vec![key('w'), kind(K::Char('w'), M::NONE, Kind::Release)]
        );
    }

    #[test]
    fn kitty_272_a_press_queued_across_a_toggle_keeps_its_release_name() {
        // #275 re-review: one read decodes `x` and shifted W; only `x` is handed out; reporting is
        // turned off and on; W is handed out, then its release (shift up first) must name W.
        let (master, slave) = pty();
        let got = on_worker(slave, None, move |s| {
            s.track_keys = true;
            s.test_epoch = Some(1);
            write_all(&master, b"x\x1b[119:87;2u");
            let mut out = vec![s.poll_source(Duration::from_millis(500))];
            s.test_epoch = Some(3);
            out.push(s.poll_source(Duration::from_millis(500)));
            write_all(&master, b"\x1b[119;1:3u");
            out.push(s.poll_source(Duration::from_millis(500)));
            drop(master);
            out
        });
        let got: Vec<_> = got.into_iter().map(|r| r.unwrap()).collect();
        assert_eq!(
            got,
            vec![
                Some(SourceEvent::Terminal(key('x'))),
                Some(SourceEvent::Terminal(kind(
                    K::Char('W'),
                    M::NONE,
                    Kind::Press
                ))),
                Some(SourceEvent::Terminal(kind(
                    K::Char('W'),
                    M::NONE,
                    Kind::Release
                ))),
            ]
        );
    }

    #[test]
    fn kitty_272_an_unrecorded_keypad_release_leaves_the_arrow_alone() {
        // #275 re-review: keypad Left went down before reporting began (no record); shift+Left is
        // pressed after; the keypad release must not take the arrow's record.
        let got = terminal(decode_all(b"\x1b[1;2D\x1b[57417;1:3u\x1b[1;1:3D", true));
        let names: Vec<(K, M, Kind)> = got
            .iter()
            .map(|e| match e {
                CtEvent::Key(k) => (k.code, k.modifiers, k.kind),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            names,
            vec![
                (K::Left, M::SHIFT, Kind::Press),
                (K::Left, M::NONE, Kind::Release), // as decoded: no record of its own
                (K::Left, M::SHIFT, Kind::Release), // the arrow's own name
            ]
        );
    }

    #[test]
    fn kitty_272_a_newer_legacy_special_key_supersedes_a_stale_record() {
        // #275 re-review 3: ctrl+Enter recorded (epoch 1), released unreported while reporting was
        // off, a legacy Enter pressed (epoch 2), then kitty's release of Enter: it names the plain
        // Enter, not the stale ctrl+Enter.
        let (master, slave) = pty();
        let got = on_worker(slave, None, move |s| {
            s.track_keys = true;
            s.test_epoch = Some(1);
            write_all(&master, b"\x1b[13;5u");
            let a = s.poll_source(Duration::from_millis(500));
            s.test_epoch = Some(2);
            write_all(&master, b"\r\x1b[13;1:3u");
            let mut rest = Vec::new();
            while let Ok(Some(e)) = s.poll_source(Duration::from_millis(200)) {
                rest.push(e);
            }
            drop(master);
            (a, rest)
        });
        assert_eq!(
            got.0.unwrap(),
            Some(SourceEvent::Terminal(kind(
                K::Enter,
                M::CONTROL,
                Kind::Press
            )))
        );
        assert_eq!(
            terminal(got.1),
            vec![
                CtEvent::Key(K::Enter.into()),
                kind(K::Enter, M::NONE, Kind::Release)
            ]
        );
    }

    #[test]
    fn kitty_272_a_release_queued_across_a_toggle_is_dropped() {
        // #275 re-review 3: a press and its release decoded together; only the press handed out;
        // reporting turned off and on; the old release is dropped, as disabling promises.
        let (master, slave) = pty();
        let got = on_worker(slave, None, move |s| {
            s.track_keys = true;
            s.test_epoch = Some(1);
            write_all(&master, b"\x1b[119u\x1b[119;1:3u");
            let a = s.poll_source(Duration::from_millis(500)).unwrap();
            s.test_epoch = Some(3);
            let b = s.poll_source(Duration::from_millis(200)).unwrap();
            drop(master);
            (a, b)
        });
        assert_eq!(got.0, Some(SourceEvent::Terminal(key('w'))));
        assert_eq!(got.1, None);
    }

    #[test]
    fn kitty_272_a_keypad_key_keeps_its_own_record_across_epochs() {
        // #275 review 4: ctrl+keypad Left recorded (epoch 1); reporting toggled with it still held;
        // shift+arrow Left pressed (epoch 2); the keypad release keeps the keypad's own name, and
        // the arrow's release keeps the arrow's. Likewise keypad `1` (kitty 57400) and a top-row `1`
        // typed as legacy text.
        let (master, slave) = pty();
        let got = on_worker(slave, None, move |s| {
            s.track_keys = true;
            s.test_epoch = Some(1);
            write_all(&master, b"\x1b[57417;5u\x1b[57400;5u");
            let mut out = Vec::new();
            for _ in 0..2 {
                out.push(s.poll_source(Duration::from_millis(500)).unwrap());
            }
            s.test_epoch = Some(2);
            write_all(
                &master,
                b"\x1b[1;2D1\x1b[57417;1:3u\x1b[1;1:3D\x1b[57400;1:3u\x1b[49;1:3u",
            );
            while let Ok(Some(e)) = s.poll_source(Duration::from_millis(200)) {
                out.push(Some(e));
            }
            drop(master);
            out
        });
        let names: Vec<(K, M, Kind)> = got
            .into_iter()
            .map(|e| match e {
                Some(SourceEvent::Terminal(CtEvent::Key(k))) => (k.code, k.modifiers, k.kind),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            names,
            vec![
                (K::Left, M::CONTROL, Kind::Press),        // keypad Left
                (K::Char('1'), M::CONTROL, Kind::Press),   // ctrl+keypad 1
                (K::Left, M::SHIFT, Kind::Press),          // arrow Left
                (K::Char('1'), M::NONE, Kind::Press),      // top-row 1, legacy
                (K::Left, M::CONTROL, Kind::Release),      // keypad Left: its own name
                (K::Left, M::SHIFT, Kind::Release),        // arrow Left: its own name
                (K::Char('1'), M::CONTROL, Kind::Release), // keypad 1: its own name
                (K::Char('1'), M::NONE, Kind::Release),    // top-row 1
            ]
        );
    }

    #[test]
    fn kitty_272_a_character_above_the_private_use_area_keeps_the_text_fallback() {
        // #275 review 5: ctrl+`ｶ` (U+FF76, 65398) recorded (epoch 1) and released unreported while
        // reporting was off; a legacy `ｶ` typed (epoch 2); kitty's release names the newer `ｶ`.
        let (master, slave) = pty();
        let got = on_worker(slave, None, move |s| {
            s.track_keys = true;
            s.test_epoch = Some(1);
            write_all(&master, b"\x1b[65398;5u");
            let a = s.poll_source(Duration::from_millis(500));
            s.test_epoch = Some(2);
            write_all(&master, "ｶ\x1b[65398;1:3u".as_bytes());
            let mut rest = Vec::new();
            while let Ok(Some(e)) = s.poll_source(Duration::from_millis(200)) {
                rest.push(e);
            }
            drop(master);
            (a, rest)
        });
        assert_eq!(
            got.0.unwrap(),
            Some(SourceEvent::Terminal(kind(
                K::Char('ｶ'),
                M::CONTROL,
                Kind::Press
            )))
        );
        assert_eq!(
            terminal(got.1),
            vec![key('ｶ'), kind(K::Char('ｶ'), M::NONE, Kind::Release)]
        );
    }

    /// The keys a hold tracker keyed by code and modifiers (a release matches its press by
    /// equality) still holds after `events`, one tic each, with releases reported throughout.
    /// Modifier keys alone are skipped: they are never delivered.
    fn still_held(events: &[CtEvent]) -> Vec<(K, M)> {
        use crate::hold::{HoldConfig, HoldTracker};
        let mut holds = HoldTracker::new(HoldConfig::new(1, 1));
        holds.set_releases_reported(true);
        for (tic, e) in events.iter().enumerate() {
            let CtEvent::Key(k) = e else { continue };
            if matches!(k.code, K::Modifier(_)) {
                continue;
            }
            let (tic, key) = (tic as u64, (k.code, k.modifiers));
            match k.kind {
                Kind::Release => holds.observe_release(tic, key),
                _ => holds.observe(tic, key),
            }
            .unwrap();
        }
        holds.advance(1_000_000).unwrap();
        holds.held().map(|(k, _)| k).collect()
    }

    #[test]
    fn kitty_272_a_repeat_with_no_record_names_its_release() {
        // Shift+w already down when reporting began (or across a disable/enable, which forgets
        // every record): kitty's autorepeat is the first thing seen of it. Shift goes up, then w,
        // which kitty names `w`. The release must end the hold the repeat started.
        let got = terminal(decode_all(
            b"\x1b[119:87;2:2u\x1b[57441;1:3u\x1b[119;1:3u",
            true,
        ));
        assert_eq!(
            got,
            vec![
                kind(K::Char('W'), M::NONE, Kind::Repeat),
                kind(
                    K::Modifier(ModifierKeyCode::LeftShift),
                    M::SHIFT,
                    Kind::Release
                ),
                kind(K::Char('W'), M::NONE, Kind::Release),
            ]
        );
        assert_eq!(still_held(&got), vec![]);
        // Ctrl+w likewise: ctrl goes up first, and kitty names the release plain `w`.
        let got = terminal(decode_all(
            b"\x1b[119;5:2u\x1b[57442;1:3u\x1b[119;1:3u",
            true,
        ));
        assert_eq!(got[0], kind(K::Char('w'), M::CONTROL, Kind::Repeat));
        assert_eq!(got[2], kind(K::Char('w'), M::CONTROL, Kind::Release));
        assert_eq!(still_held(&got), vec![]);
    }

    #[test]
    fn kitty_272_a_legacy_shifted_or_control_press_names_its_release() {
        // Typed while delivery was on but before the push took effect: a legacy `W` (shift+w), then
        // shift up and kitty's release of w, named `w`; again with a kitty autorepeat in between.
        // A legacy ctrl+w (0x17) released after ctrl already matched; it must still.
        let got = terminal(decode_all(
            b"W\x1b[57441;1:3u\x1b[119;1:3u\
              W\x1b[119:87;2:2u\x1b[57441;1:3u\x1b[119;1:3u\
              \x17\x1b[57442;1:3u\x1b[119;1:3u",
            true,
        ));
        let names: Vec<(K, M, Kind)> = got
            .iter()
            .filter_map(|e| match e {
                CtEvent::Key(k) if !matches!(k.code, K::Modifier(_)) => {
                    Some((k.code, k.modifiers, k.kind))
                }
                CtEvent::Key(_) => None,
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            names,
            vec![
                (K::Char('W'), M::SHIFT, Kind::Press),
                (K::Char('W'), M::SHIFT, Kind::Release),
                (K::Char('W'), M::SHIFT, Kind::Press),
                (K::Char('W'), M::SHIFT, Kind::Repeat),
                (K::Char('W'), M::SHIFT, Kind::Release),
                (K::Char('w'), M::CONTROL, Kind::Press),
                (K::Char('w'), M::CONTROL, Kind::Release),
            ]
        );
        assert_eq!(still_held(&got), vec![]);
    }

    #[test]
    fn kitty_272_a_legacy_shifted_symbol_repeated_by_kitty_names_its_release() {
        // A legacy `!` (shift+1) recorded before the push took effect, autorepeated by kitty as
        // `CSI 49:33;2:2u`, then shift up and kitty's release of the key, named `1`.
        let got = terminal(decode_all(
            b"!\x1b[49:33;2:2u\x1b[57441;1:3u\x1b[49;1:3u",
            true,
        ));
        assert_eq!(got[1], kind(K::Char('!'), M::NONE, Kind::Repeat));
        assert_eq!(got[3], kind(K::Char('!'), M::NONE, Kind::Release));
        assert_eq!(still_held(&got), vec![]);
    }

    #[test]
    fn issue_312_origin_is_not_recoverable_from_the_delivered_observations() {
        // Legacy US-style shift+1 was released; in the other trace a genuinely exact `!`
        // remains down while the unrelated base key `1` was released. A layout may have `!`
        // as its unshifted key, so this exact key can have base identity 33, independent of 49.
        let legacy = terminal(decode_all(b"!\x1b[49;1:3u", true));
        let exact = terminal(decode_all(b"\x1b[33;1u\x1b[49;1:3u", true));
        assert_eq!(legacy, exact);
        assert_eq!(legacy[0], kind(K::Char('!'), M::NONE, Kind::Press));
        assert_eq!(legacy[1], kind(K::Char('1'), M::NONE, Kind::Release));

        // An exact consumer should keep `!` down while the genuinely exact key is held.
        // The tracker sees identical data in the legacy case, where `!` has already gone up.
        // A deadline selected solely from these events cannot satisfy both cases. This is true
        // for any delay, including one that depends on the unrelated `1` release.
        assert_eq!(still_held(&legacy), vec![(K::Char('!'), M::NONE)]);
        assert_eq!(still_held(&exact), vec![(K::Char('!'), M::NONE)]);

        // Positive control: the later actual release of the exact `!` is named as its press and
        // clears it; the earlier unrelated base-key release must not be assigned to `!`.
        let exact_up = terminal(decode_all(b"\x1b[33;1u\x1b[49;1:3u\x1b[33;1:3u", true));
        assert_eq!(&exact_up[..2], &exact);
        assert_eq!(exact_up[2], kind(K::Char('!'), M::NONE, Kind::Release));
        assert!(still_held(&exact_up).is_empty());
    }

    #[test]
    fn issue_312_code_identity_is_not_a_legacy_wire_format_marker() {
        // Kitty can report press/release kinds for special keys without `CSI ... u`: the parser
        // emits an ordinary Event for both, so TtySource uses KeyId::Code for the exact arrow.
        // Treating every Code record as legacy would time out a real exact held arrow.
        let mut parser = Parser::default();
        parser.advance(b"\x1b[1;1:1A\x1b[1;1:3A", false);
        let events: Vec<_> = parser.collect();
        assert!(matches!(
            events.as_slice(),
            [InternalEvent::Event(CtEvent::Key(a)), InternalEvent::Event(CtEvent::Key(b))]
                if a.code == K::Up && a.kind == Kind::Press
                && b.code == K::Up && b.kind == Kind::Release
        ));
    }

    #[test]
    fn issue_312_a_shifted_symbol_from_the_legacy_switch_window_releases_under_its_base_key() {
        // DOCUMENTED LIMITATION (#312). A shifted symbol pressed while the terminal is briefly in
        // legacy mode — the pop/re-push window of a screen switch — arrives as the bare byte `!`,
        // which carries no physical-key identity. With no intervening kitty autorepeat to move the
        // record to the kitty identity (contrast the test above), kitty reports its release under
        // the base key `1`. Shift does not lowercase a symbol, so `!` does not fold to `1`, nothing
        // matches, and the release is delivered as decoded (`1`). A held-key model therefore keeps
        // `!` held unless a recovery event occurs. A layout-agnostic rename is
        // impossible: base `1` cannot be mapped back to the shifted `!` without assuming a keyboard
        // layout, and pairing the unmatched release with the newest orphan would mis-identify an
        // unrelated key's release. Some non-ASCII letters are also unmatched; see the next test.
        let got = terminal(decode_all(b"!\x1b[57441;1:3u\x1b[49;1:3u", true));
        assert_eq!(got[0], kind(K::Char('!'), M::NONE, Kind::Press));
        // The release is delivered under the base key — the limitation.
        assert_eq!(got[2], kind(K::Char('1'), M::NONE, Kind::Release));
        // The consumer's held-key model keeps `!` held (the stuck state the issue describes).
        assert_eq!(still_held(&got), vec![(K::Char('!'), M::NONE)]);
    }

    #[test]
    fn issue_312_unicode_lowercase_expansion_does_not_match_a_single_base_key() {
        // A decoder model of a Turkish-layout `İ` press in legacy mode followed by an `i` release
        // in the Kitty protocol. This is not a real terminal or keyboard-layout test. Unicode's
        // default lowercase for U+0130 has two code points, `i` + COMBINING DOT ABOVE, so the
        // source's single-base-key fallback cannot match it to `i`. No repeat or recovery follows.
        let got = terminal(decode_all("İ\x1b[105;1:3u".as_bytes(), true));
        assert_eq!(got[0], kind(K::Char('İ'), M::SHIFT, Kind::Press));
        assert_eq!(got[1], kind(K::Char('i'), M::NONE, Kind::Release));
        assert_eq!(still_held(&got), vec![(K::Char('İ'), M::SHIFT)]);

        // Positive control: legacy `W` pairs when the terminal reports `w` as its base.
        let ascii = terminal(decode_all(b"W\x1b[119;1:3u", true));
        assert_eq!(ascii[0], kind(K::Char('W'), M::SHIFT, Kind::Press));
        assert_eq!(ascii[1], kind(K::Char('W'), M::SHIFT, Kind::Release));
        assert!(still_held(&ascii).is_empty());
    }

    #[test]
    fn issue_312_no_further_events_leave_the_legacy_hold_stuck() {
        // Model a legacy shifted symbol from the screen-switch epoch using the parser and hold
        // tracker directly; this does not exercise a terminal or an actual screen transition.
        // Reporting is active for the hold tracker and the modeled release uses the base key. There
        // is no later repeat, focus loss, mode disable or unrelated key. A genuine Kitty press/release
        // is the control: it must not remain held.
        let legacy = terminal(decode_all(b"!\x1b[49;1:3u", true));
        assert_eq!(legacy[0], kind(K::Char('!'), M::NONE, Kind::Press));
        assert_eq!(legacy[1], kind(K::Char('1'), M::NONE, Kind::Release));
        let mut holds = crate::hold::HoldTracker::new(crate::hold::HoldConfig::new(1, 1));
        holds.set_releases_reported(true);
        let CtEvent::Key(press) = legacy[0] else {
            panic!("expected press")
        };
        let CtEvent::Key(release) = legacy[1] else {
            panic!("expected release")
        };
        holds.observe(0, (press.code, press.modifiers)).unwrap();
        holds
            .observe_release(1, (release.code, release.modifiers))
            .unwrap();
        assert!(holds.advance(1_000_000).unwrap().is_empty());
        assert!(holds.is_held((K::Char('!'), M::NONE)));
        let exact = terminal(decode_all(b"\x1b[49:33;2u\x1b[49;1:3u", true));
        assert_eq!(exact[0], kind(K::Char('!'), M::NONE, Kind::Press));
        assert_eq!(exact[1], kind(K::Char('!'), M::NONE, Kind::Release));
        assert!(still_held(&exact).is_empty());
    }

    #[test]
    fn issue_312_focus_loss_conditionally_ends_the_stuck_hold() {
        // Conditional recovery: with releases reported, `!` is
        // pressed (no deadline) and its release arrives named `1` (the base key), which does not
        // match, so `!` stays held — then a focus loss ends it (the terminal reports focus loss,
        // and DEEP SIGNAL / #276 acts on it, if a focus loss is actually reported).
        let mut holds = crate::hold::HoldTracker::new(crate::hold::HoldConfig::new(1, 1));
        holds.set_releases_reported(true);
        holds.observe(0, (K::Char('!'), M::NONE)).unwrap();
        holds.observe_release(1, (K::Char('1'), M::NONE)).unwrap();
        holds.advance(1_000).unwrap();
        assert!(
            holds.is_held((K::Char('!'), M::NONE)),
            "stuck before focus loss"
        );
        let ended = holds.release_all(crate::hold::ReleaseCause::FocusLost);
        assert_eq!(ended.len(), 1);
        assert!(
            !holds.is_held((K::Char('!'), M::NONE)),
            "#312: focus loss ends the stuck hold"
        );
    }

    #[test]
    fn issue_312_disabling_reporting_conditionally_ends_the_stuck_hold() {
        // Another conditional recovery: when key-release reporting is disabled, every hold that
        // was waiting for a release (ends_at == u64::MAX) is given the inferred grace deadline, so
        // the stuck `!` times out instead of waiting forever.
        let mut holds = crate::hold::HoldTracker::new(crate::hold::HoldConfig::new(1, 5));
        holds.set_releases_reported(true);
        holds.observe(0, (K::Char('!'), M::NONE)).unwrap();
        holds.observe_release(1, (K::Char('1'), M::NONE)).unwrap();
        assert!(holds.is_held((K::Char('!'), M::NONE)));
        holds.set_releases_reported(false);
        let ended = holds.advance(1_000).unwrap();
        assert!(
            ended.iter().any(|e| e.key == (K::Char('!'), M::NONE)),
            "#312: disabling reporting bounds the stuck hold"
        );
        assert!(!holds.is_held((K::Char('!'), M::NONE)));
    }

    #[test]
    fn kitty_272_active_needs_every_flag_pushed() {
        // LibGibson pushes 15. Without 8, Enter/Tab/Backspace/Esc release as their legacy byte (a
        // second press); without 4, shift+1 reads as `1` with shift. A reply missing any of them is
        // not the protocol LibGibson decodes. 16 (associated text) only appends a field the
        // decoder ignores, so a superset is Active.
        for (bits, want) in [
            (2, KeyReleaseReporting::Unsupported),
            (3, KeyReleaseReporting::Unsupported),
            (7, KeyReleaseReporting::Unsupported),
            (11, KeyReleaseReporting::Unsupported),
            (14, KeyReleaseReporting::Unsupported),
            (15, KeyReleaseReporting::Active),
            (31, KeyReleaseReporting::Active),
        ] {
            let mut q = KeyboardQueries::default();
            q.outstanding += 1;
            assert_eq!(
                q.flags_reply(F::from_bits_truncate(bits), true),
                Some(want),
                "flags reply {bits}"
            );
        }
    }

    #[test]
    fn kitty_272_a_read_split_after_esc_while_active_keeps_the_sequence() {
        // While the flags are confirmed, kitty sends the Esc key as `CSI 27u`: a read that ends
        // in a lone ESC is a sequence split across reads, not the Esc key.
        let _serial = queries_lock();
        forget_keyboard_replies();
        expect_keyboard_reply();
        let (master, slave) = pty();
        let got = on_worker(slave, None, move |s| {
            s.track_keys = true;
            let mut out = Vec::new();
            let drain = |s: &mut TtySource, out: &mut Vec<SourceEvent>| {
                while let Ok(Some(e)) = s.poll_source(Duration::from_millis(200)) {
                    out.push(e);
                }
            };
            write_all(&master, b"\x1b[?15u\x1b[?62;c\x1b[119u");
            drain(s, &mut out);
            write_all(&master, b"\x1b");
            // One read takes the lone ESC; it is held (nothing delivered) while the rest follows
            // within `LONE_ESC_WAIT`, in a second read.
            assert_eq!(s.poll_source(Duration::from_millis(10)).unwrap(), None);
            write_all(&master, b"[119;1:3u");
            drain(s, &mut out);
            drop(master);
            out
        });
        forget_keyboard_replies();
        assert_eq!(
            got,
            vec![
                SourceEvent::KeyReleaseReporting(KeyReleaseReporting::Active),
                SourceEvent::Terminal(key('w')),
                SourceEvent::Terminal(kind(K::Char('w'), M::NONE, Kind::Release)),
            ]
        );
        // Not confirmed: a read ending in a lone ESC is the Esc key, at once, as before.
        let (master, slave) = pty();
        let got = on_worker(slave, None, move |s| {
            s.track_keys = true;
            write_all(&master, b"\x1b");
            let esc = s.poll_source(Duration::from_millis(200)).unwrap();
            drop(master);
            esc
        });
        assert_eq!(
            got,
            Some(SourceEvent::Terminal(CtEvent::Key(K::Esc.into())))
        );
    }

    #[test]
    fn kitty_272_what_flags_15_change_in_decoded_keys() {
        // The rustdoc of `set_key_release_reporting` states what reporting changes in the keys
        // delivered. Each pair: what kitty sends with flags 15, and the legacy bytes for the same
        // keystroke without them.
        // track=false reads the process-global delivery flag; serialise with anything that sets it.
        let _serial = queries_lock();
        let names = |bytes: &'static [u8]| -> Vec<(K, M)> {
            terminal(decode_all(bytes, false))
                .into_iter()
                .map(|e| match e {
                    CtEvent::Key(k) => (k.code, k.modifiers),
                    other => panic!("{other:?}"),
                })
                .collect()
        };
        // The same: Esc, Enter, Tab, Backspace, ctrl and alt chords, shifted symbols.
        assert_eq!(
            names(b"\x1b[13u\x1b[9u\x1b[127u\x1b[97;5u\x1b[97;3u\x1b[49:33;2u\x1b[9;2u\x1b[27u"),
            names(b"\r\t\x7f\x01\x1ba!\x1b[Z\x1b"),
        );
        // Different: a shifted letter carries no SHIFT; Caps Lock does not change the letter;
        // ctrl+i, ctrl+m and ctrl+[ are ctrl chords, not Tab, Enter and Esc.
        assert_eq!(
            names(b"\x1b[97:65;2u\x1b[119:87;6u\x1b[97;65u\x1b[105;5u\x1b[109;5u\x1b[91;5u"),
            vec![
                (K::Char('A'), M::NONE),
                (K::Char('W'), M::CONTROL),
                (K::Char('a'), M::NONE),
                (K::Char('i'), M::CONTROL),
                (K::Char('m'), M::CONTROL),
                (K::Char('['), M::CONTROL),
            ]
        );
        assert_eq!(
            names(b"A\x17A\t\r"),
            vec![
                (K::Char('A'), M::SHIFT),
                (K::Char('w'), M::CONTROL),
                (K::Char('A'), M::SHIFT),
                (K::Tab, M::NONE),
                (K::Enter, M::NONE),
            ]
        );
    }

    #[test]
    fn kitty_272_a_held_lone_esc_is_the_esc_key_once_nothing_follows() {
        // Confirmed, but the flags were cleared behind LibGibson's back (kitty's reset): the Esc
        // key arrives as a lone ESC with nothing after it. It is held only briefly, then delivered
        // as Esc, and the next key is not taken for an Alt chord.
        let _serial = queries_lock();
        forget_keyboard_replies();
        expect_keyboard_reply();
        let (master, slave) = pty();
        let got = on_worker(slave, None, move |s| {
            s.track_keys = true;
            write_all(&master, b"\x1b[?15u\x1b[?62;c");
            let active = s.poll_source(Duration::from_millis(500)).unwrap();
            write_all(&master, b"\x1b");
            let start = Instant::now();
            let esc = s.poll_source(Duration::from_millis(1000)).unwrap();
            let waited = start.elapsed();
            write_all(&master, b"a");
            let next = s.poll_source(Duration::from_millis(500)).unwrap();
            drop(master);
            (active, esc, waited, next)
        });
        forget_keyboard_replies();
        assert_eq!(
            got.0,
            Some(SourceEvent::KeyReleaseReporting(
                KeyReleaseReporting::Active
            ))
        );
        assert_eq!(
            got.1,
            Some(SourceEvent::Terminal(CtEvent::Key(K::Esc.into())))
        );
        assert!(
            got.2 >= LONE_ESC_WAIT && got.2 < LONE_ESC_WAIT + Duration::from_millis(400),
            "{:?}",
            got.2
        );
        assert_eq!(got.3, Some(SourceEvent::Terminal(key('a'))));
    }

    #[test]
    fn kitty_272_an_expired_esc_hold_reads_what_is_already_waiting_first() {
        // #275 review 8: a full read ends in a lone ESC whose continuation is already on the fd;
        // the 145 events it decoded are handed out over more than `LONE_ESC_WAIT`. The expired
        // hold must read the waiting bytes before deciding the ESC was the Esc key.
        let _serial = queries_lock();
        forget_keyboard_replies();
        expect_keyboard_reply();
        let mut first = Vec::new();
        for _ in 0..67 {
            first.extend_from_slice(b"\x1b[97u\x1b[97;1:3u");
        }
        for _ in 0..5 {
            first.extend_from_slice(b"\x1b[119u\x1b[119;1:3u");
        }
        first.extend_from_slice(b"\x1b[97u\x1b");
        assert_eq!(first.len(), READ_BUF);
        first.extend_from_slice(b"[97;1:3u");
        let (master, slave) = pty();
        let got = on_worker(slave, None, move |s| {
            s.track_keys = true;
            write_all(&master, b"\x1b[?15u\x1b[?62;c");
            let active = s.poll_source(Duration::from_millis(500)).unwrap();
            write_all(&master, &first);
            let mut out = vec![s.poll_source(Duration::from_millis(500)).unwrap()];
            thread::sleep(LONE_ESC_WAIT + Duration::from_millis(30));
            while let Ok(Some(e)) = s.poll_source(Duration::from_millis(200)) {
                out.push(Some(e));
            }
            drop(master);
            (active, out)
        });
        forget_keyboard_replies();
        assert_eq!(
            got.0,
            Some(SourceEvent::KeyReleaseReporting(
                KeyReleaseReporting::Active
            ))
        );
        let got = terminal(got.1.into_iter().flatten().collect());
        assert_eq!(got.len(), 146, "{:?}", &got[140..]);
        assert_eq!(got[144], key('a'));
        assert_eq!(got[145], kind(K::Char('a'), M::NONE, Kind::Release));
    }

    #[test]
    fn kitty_272_a_split_esc_in_the_read_that_confirms_reporting_is_held() {
        // #275 review 8: the answer and a lone ESC arrive in one read; the rest of the ESC's
        // sequence in the next. Reporting is not yet confirmed when the read is decoded, but its
        // answer is pending: the ESC is held.
        let _serial = queries_lock();
        forget_keyboard_replies();
        expect_keyboard_reply();
        let (master, slave) = pty();
        let got = on_worker(slave, None, move |s| {
            s.track_keys = true;
            write_all(&master, b"\x1b[?15u\x1b[?62;c\x1b[119u\x1b");
            let mut out = vec![s.poll_source(Duration::from_millis(500)).unwrap()];
            write_all(&master, b"[119;1:3u");
            while let Ok(Some(e)) = s.poll_source(Duration::from_millis(200)) {
                out.push(Some(e));
            }
            drop(master);
            out
        });
        forget_keyboard_replies();
        assert_eq!(
            got.into_iter().flatten().collect::<Vec<_>>(),
            vec![
                SourceEvent::KeyReleaseReporting(KeyReleaseReporting::Active),
                SourceEvent::Terminal(key('w')),
                SourceEvent::Terminal(kind(K::Char('w'), M::NONE, Kind::Release)),
            ]
        );
    }
}
