use crate::capability::{Capability, ColorDepth, TerminalCapabilities};
use crossterm::{
    cursor::Show,
    event::{DisableBracketedPaste, EnableBracketedPaste},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, stdout, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;
use std::thread::ThreadId;

static PANIC_HOOK_SET: AtomicBool = AtomicBool::new(false);

/// Process-global terminal-ownership lease. Exactly one interactive
/// `TerminalSession` may hold the real terminal at a time; the panic hook and
/// restore path consult it so a session that never took the terminal can't have
/// its stdout mangled by someone else's cleanup. One patient on the table.
static TERMINAL_LEASE: AtomicU8 = AtomicU8::new(LEASE_AVAILABLE);

const LEASE_AVAILABLE: u8 = 0;
const LEASE_OWNED: u8 = 1;
const LEASE_RESTORING: u8 = 2;

/// The thread that currently holds the terminal lease, if any. The panic hook
/// restores the terminal only when the *panicking* thread is this owner, so a
/// recoverable panic on a non-owner worker thread cannot tear down the owner's
/// live terminal (issue #11, B3). Terminal ownership is therefore thread-affine:
/// the owner is whichever thread called [`TerminalSession::new`], and the session
/// is expected to be created and driven on that one thread. Moving a session to
/// another thread and driving it there is outside the supported contract — a
/// panic on the driving thread would not be recognised as the owner.
static OWNER_THREAD: Mutex<Option<ThreadId>> = Mutex::new(None);

/// Records (or clears) the lease-owning thread. The lock is held only for this
/// one assignment — never across terminal I/O — so it cannot be poisoned by a
/// panic mid-restore, and the panic hook can always read it back.
fn set_owner_thread(id: Option<ThreadId>) {
    let mut guard = OWNER_THREAD
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    *guard = id;
}

/// The thread that currently owns the terminal lease, if any.
fn owner_thread() -> Option<ThreadId> {
    OWNER_THREAD
        .lock()
        .map(|guard| *guard)
        .unwrap_or_else(|poison| *poison.into_inner())
}

/// Observable state of the process-global terminal lease. Read-only diagnosis:
/// you don't get to hand-set it, the session takes and releases it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalLease {
    /// No session currently owns the terminal.
    Available,
    /// An interactive session holds the terminal.
    Owned,
    /// The owning session is mid-restoration and about to release.
    Restoring,
}

/// Manages the terminal lifecycle, raw mode, alternate screen, and restoration guards.
pub struct TerminalSession {
    pub is_tty: bool,
    /// When true, all direct OS terminal manipulation is suppressed while the
    /// session still behaves like a TTY for the renderer. This enables
    /// byte-stream tests against a virtual terminal (e.g. `vt100`).
    headless: bool,
    headless_size: (u16, u16),
    raw_mode_enabled: bool,
    alt_screen_active: bool,
    cursor_hidden: bool,
    bracketed_paste_enabled: bool,
    sync_updates_enabled: bool,
    capabilities: TerminalCapabilities,
    /// True when this instance holds the process-global terminal lease. Only an
    /// interactive (`is_tty`, non-headless) session ever takes it.
    owns_lease: bool,
    /// One-shot latch: `restore()` sets this the moment it *begins*, so the
    /// later `Drop` (and any repeat `restore()`) is a clean no-op instead of
    /// emitting the reset blob a second time. Named for what it means — restore
    /// was *attempted* once — not that it succeeded: a restore that returns
    /// `Err` still latches this and still releases the lease (issue #11, B2).
    restore_attempted: bool,
}

impl TerminalSession {
    /// Creates a new terminal session, detecting whether stdout is an interactive terminal.
    pub fn new() -> io::Result<Self> {
        let is_tty = stdout().is_terminal();

        // `new()` is never headless, so a real TTY means we must be the sole
        // owner of this process's terminal. The CAS is the whole contract: lose
        // the race and we refuse to operate rather than fight another session
        // over the same fd. A non-TTY stdout owns nothing and never errors.
        let owns_lease = if is_tty {
            TERMINAL_LEASE
                .compare_exchange(
                    LEASE_AVAILABLE,
                    LEASE_OWNED,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                )
                .map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "terminal already owned by another LibGibson TerminalSession in this process",
                    )
                })?;
            // Sole owner now: record which thread holds it, so the panic hook
            // only restores for a panic on *this* thread (issue #11, B3).
            set_owner_thread(Some(std::thread::current().id()));
            true
        } else {
            false
        };

        let session = Self {
            is_tty,
            headless: false,
            headless_size: (80, 24),
            raw_mode_enabled: false,
            alt_screen_active: false,
            cursor_hidden: false,
            bracketed_paste_enabled: false,
            sync_updates_enabled: true,
            capabilities: TerminalCapabilities::detect_from_env(),
            owns_lease,
            restore_attempted: false,
        };

        Self::ensure_panic_hook();

        Ok(session)
    }

    /// Current process-global terminal-lease state. A vitals monitor, not a
    /// control: it reports who holds the terminal, it does not hand it over.
    pub fn lease_state() -> TerminalLease {
        match TERMINAL_LEASE.load(Ordering::SeqCst) {
            LEASE_OWNED => TerminalLease::Owned,
            LEASE_RESTORING => TerminalLease::Restoring,
            _ => TerminalLease::Available,
        }
    }

    /// Creates a deterministic, non-destructive session that reports itself as a
    /// TTY of the requested geometry but never touches the real terminal.
    ///
    /// This is the controlled geometry abstraction used to drive the *whole*
    /// renderer pipeline (`Renderer -> TerminalTransaction -> bytes`) through a
    /// virtual terminal in tests.
    pub fn headless(cols: u16, rows: u16) -> Self {
        Self {
            is_tty: true,
            headless: true,
            headless_size: (cols, rows),
            raw_mode_enabled: false,
            alt_screen_active: false,
            cursor_hidden: false,
            bracketed_paste_enabled: false,
            sync_updates_enabled: true,
            capabilities: TerminalCapabilities::truecolor(),
            // A headless session is a mannequin, not a patient: it performs no OS
            // terminal ops, so it never takes the lease and never restores one.
            owns_lease: false,
            restore_attempted: false,
        }
    }

    /// Current passive capability snapshot.
    pub fn capabilities(&self) -> TerminalCapabilities {
        self.capabilities
    }

    /// Overrides capabilities (used by capability-injection tests and callers
    /// that know better than the environment).
    pub fn set_capabilities(&mut self, caps: TerminalCapabilities) {
        self.capabilities = caps;
    }

    /// Convenience: current color depth.
    pub fn color_depth(&self) -> ColorDepth {
        self.capabilities.color_depth
    }

    /// Convenience: override color depth while keeping other capabilities.
    pub fn set_color_depth(&mut self, depth: ColorDepth) {
        self.capabilities.color_depth = depth;
    }

    /// Whether `CSI L` insertion is explicitly known to be supported.
    pub fn insert_line_supported(&self) -> bool {
        self.capabilities.insert_line == Capability::Supported
    }

    /// Updates the geometry reported by a headless session. Used to simulate
    /// terminal resize without a real PTY.
    pub fn set_terminal_size(&mut self, cols: u16, rows: u16) {
        self.headless_size = (cols, rows);
    }

    /// True when this session must not perform direct OS terminal manipulation.
    pub fn is_headless(&self) -> bool {
        self.headless
    }

    /// Installs the process-global panic hook that best-effort restores the
    /// terminal on panic. Host-integration contract (issue #11, B4/B5):
    ///
    /// * Installed **once** per process (guarded by `PANIC_HOOK_SET`), and it
    ///   **chains** whatever hook was present at install time, so a host hook set
    ///   *before* the first [`TerminalSession::new`] still runs after LibGibson's
    ///   cleanup. A host that installs its hook *after* the first session replaces
    ///   LibGibson's and takes responsibility for restoration itself.
    /// * The restoration body is owner-thread-scoped (see [`owner_thread`]): it
    ///   fires only for a panic on the lease-owning thread, so a recoverable
    ///   non-owner worker-thread panic never touches the terminal (B3).
    fn ensure_panic_hook() {
        if !PANIC_HOOK_SET.swap(true, Ordering::SeqCst) {
            let default_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                // Best-effort terminal restoration on panic, but only when the
                // panicking thread is the lease owner. Two gates: the lease must
                // be held (or mid-release), AND this must be the thread that took
                // it. A recoverable panic on a non-owner worker thread must not
                // tear down the owner's live terminal (issue #11, B3); if nobody
                // owns the terminal, the panic came from code that never touched
                // it. Either way, keep our hands off stdout and just chain through.
                if TERMINAL_LEASE.load(Ordering::SeqCst) != LEASE_AVAILABLE
                    && owner_thread() == Some(std::thread::current().id())
                {
                    let _ = disable_raw_mode();
                    let mut out = stdout();
                    let _ = execute!(out, Show, LeaveAlternateScreen, DisableBracketedPaste);
                    let _ = out.write_all(b"\x1b[0m\x1b[?2026l\x1b[?7h\x1b[?25h\r\n");
                    let _ = out.flush();
                }

                default_hook(info);
            }));
        }
    }

    /// Enables raw mode and bracketed paste for interactive operation.
    pub fn enter_interactive(&mut self) -> io::Result<()> {
        if !self.is_tty || self.headless {
            return Ok(());
        }

        if !self.raw_mode_enabled {
            enable_raw_mode()?;
            self.raw_mode_enabled = true;
        }

        if !self.bracketed_paste_enabled {
            let mut out = stdout();
            execute!(out, EnableBracketedPaste)?;
            self.bracketed_paste_enabled = true;
        }

        Ok(())
    }

    /// Enters alternate screen mode (fullscreen).
    pub fn enter_alternate_screen(&mut self) -> io::Result<()> {
        if !self.is_tty || self.headless || self.alt_screen_active {
            return Ok(());
        }

        let mut out = stdout();
        execute!(out, EnterAlternateScreen)?;
        self.alt_screen_active = true;
        Ok(())
    }

    /// Leaves alternate screen mode.
    pub fn leave_alternate_screen(&mut self) -> io::Result<()> {
        if !self.is_tty || !self.alt_screen_active {
            return Ok(());
        }

        let mut out = stdout();
        execute!(out, LeaveAlternateScreen)?;
        self.alt_screen_active = false;
        Ok(())
    }

    /// Hides the hardware cursor.
    pub fn hide_cursor(&mut self) -> Option<&'static [u8]> {
        if !self.is_tty || self.cursor_hidden {
            return None;
        }
        self.cursor_hidden = true;
        Some(b"\x1b[?25l")
    }

    /// Shows the hardware cursor.
    pub fn show_cursor(&mut self) -> Option<&'static [u8]> {
        if !self.is_tty || !self.cursor_hidden {
            return None;
        }
        self.cursor_hidden = false;
        Some(b"\x1b[?25h")
    }

    /// Sets whether synchronized-update mode (CSI ? 2026) is used.
    pub fn set_sync_updates(&mut self, enabled: bool) {
        self.sync_updates_enabled = enabled;
    }

    pub fn sync_updates(&self) -> bool {
        self.sync_updates_enabled && self.is_tty
    }

    /// Restores all modified terminal settings back to normal.
    ///
    /// One-shot, best-effort, first-error-reporting (issue #11, B1/B2):
    ///
    /// * It attempts *every* cleanup step exactly once — show cursor, disable
    ///   bracketed paste, leave the alternate screen, reset styling/sync/wrap,
    ///   disable raw mode — even if an earlier step fails, and returns the
    ///   **first** error encountered (or `Ok(())`). A half-restored terminal is
    ///   worse than a fully-attempted, reported one.
    /// * It is a one-shot latch: the first call marks the session
    ///   `restore_attempted`, so `Drop` and any later `restore()` are clean
    ///   no-ops. Transient failures are **not** retried — the mode that actually
    ///   breaks a shell (raw mode) is restored via a termios ioctl independent of
    ///   the output stream, so retrying a broken stdout could not do better.
    /// * The lease is released to [`TerminalLease::Available`] regardless of
    ///   write success. A failed restore does not poison ownership: another
    ///   session may acquire, and if the output stream is genuinely broken its
    ///   writes fail the same way — that is the caller's signal to handle,
    ///   surfaced through the returned `Err`.
    pub fn restore(&mut self) -> io::Result<()> {
        // Nothing to undo on a non-terminal or headless session, and a session
        // already restored is a no-op: this is what makes Drop-after-explicit a
        // clean close instead of a second reset blob on the wire.
        if !self.is_tty || self.headless || self.restore_attempted {
            return Ok(());
        }

        self.restore_attempted = true;
        if self.owns_lease {
            TERMINAL_LEASE.store(LEASE_RESTORING, Ordering::SeqCst);
        }

        // Best-effort completion, first-error reporting: attempt every op even
        // after one fails, but surface the first wound rather than swallow it.
        // A half-restored terminal is worse than a reported one.
        let mut first_err: Option<io::Error> = None;
        let mut record = |result: io::Result<()>| {
            if let Err(err) = result {
                if first_err.is_none() {
                    first_err = Some(err);
                }
            }
        };

        let mut out = stdout();

        if self.cursor_hidden {
            record(execute!(out, Show));
            self.cursor_hidden = false;
        }

        if self.bracketed_paste_enabled {
            record(execute!(out, DisableBracketedPaste));
            self.bracketed_paste_enabled = false;
        }

        if self.alt_screen_active {
            record(execute!(out, LeaveAlternateScreen));
            self.alt_screen_active = false;
        }

        // Reset styling, synchronized update state, enable autowrap, show cursor
        record(out.write_all(b"\x1b[0m\x1b[?2026l\x1b[?7h\x1b[?25h"));
        record(out.flush());

        if self.raw_mode_enabled {
            record(disable_raw_mode());
            self.raw_mode_enabled = false;
        }

        // Release last, once the ops are done: clear the owner thread and hand
        // the lease back to the pool only after we've stopped touching the
        // terminal. Even if the writes above failed, raw mode was still restored
        // via termios (an ioctl, not the failed output fd) and the lease returns
        // to Available: a broken output stream is the caller's to see (via the
        // returned Err), not a reason to wedge the lease shut (issue #11, B1/B2).
        if self.owns_lease {
            set_owner_thread(None);
            TERMINAL_LEASE.store(LEASE_AVAILABLE, Ordering::SeqCst);
            self.owns_lease = false;
        }

        match first_err {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    /// Returns current terminal dimensions (columns, rows).
    pub fn terminal_size(&self) -> (u16, u16) {
        if self.headless {
            self.headless_size
        } else if self.is_tty {
            crossterm::terminal::size().unwrap_or((80, 24))
        } else {
            (80, 24)
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
