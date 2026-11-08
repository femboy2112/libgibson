use crate::capability::{Capability, ColorDepth, TerminalCapabilities};
use crossterm::{
    cursor::Show,
    event::{
        DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
        EnableFocusChange, EnableMouseCapture,
    },
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

/// Reporting modes the lease owner currently has enabled, so the panic hook can
/// disable exactly those that were turned on (it cannot reach the session). Set by
/// the owning session's `set_mouse_capture` / `set_focus_reporting`, cleared by
/// `restore()`. Lease-scoped: only the single interactive owner ever writes it.
static REPORTING_MODES: AtomicU8 = AtomicU8::new(0);

const REPORT_MOUSE: u8 = 0b01;
const REPORT_FOCUS: u8 = 0b10;
const REPORT_KEY_RELEASE: u8 = 0b100;

/// Set by the panic hook when it ends key-release reporting; consumed by the next poll, which
/// delivers `KeyReleaseReporting::Unsupported` once (see
/// [`TerminalSession::take_key_releases_ended`]). An atomic: the hook takes no lock.
static KEY_RELEASES_ENDED: AtomicBool = AtomicBool::new(false);

/// The kitty keyboard flags pushed when key-release reporting is enabled (#275): disambiguate
/// escape codes (1), report event types (2), report alternate keys (4), report all keys as escape
/// codes (8). Measured on kitty 0.32.2: without 8, Enter, Tab, Backspace and Esc report their
/// release as their legacy byte, which reads as a second press; without 4, shift+1 reads as `1`
/// with shift, not `!`.
pub(crate) const KEY_RELEASE_FLAGS: u8 = 15;

/// Pushes [`KEY_RELEASE_FLAGS`] onto the kitty keyboard stack of the current screen, then asks
/// for the flags now in force (`CSI ? u`) and the primary device attributes (`CSI c`). Every
/// terminal answers the last query; a kitty-protocol terminal answers the flags query before it.
/// The answers reach the application as `Event::KeyReleaseReporting`.
fn request_key_releases(out: &mut impl Write) -> io::Result<()> {
    #[cfg(unix)]
    crate::tty_source::expect_keyboard_reply();
    write!(out, "\x1b[>{KEY_RELEASE_FLAGS}u\x1b[?u\x1b[c")?;
    out.flush()
}

/// Pops the entry [`request_key_releases`] pushed. The kitty stacks are per screen (main and
/// alternate, each kept across switches), so this must run on the screen the push ran on.
fn pop_key_releases(out: &mut impl Write) -> io::Result<()> {
    out.write_all(b"\x1b[<u")?;
    out.flush()
}

/// Whether this process's push is still on the terminal: the `REPORT_KEY_RELEASE` bit, set
/// before the push and cleared by whichever path pops first (the panic hook, restore, disable),
/// so the push is popped exactly once; a second pop would remove another program's entry.
fn key_releases_pushed() -> bool {
    REPORTING_MODES.load(Ordering::SeqCst) & REPORT_KEY_RELEASE != 0
}

/// Claims the pop: true for exactly one caller after a push.
fn take_key_releases_pushed() -> bool {
    REPORTING_MODES.fetch_and(!REPORT_KEY_RELEASE, Ordering::SeqCst) & REPORT_KEY_RELEASE != 0
}

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
    /// Opt-in SGR mouse-capture reporting is on. Tracked so `restore`, `Drop` and
    /// the panic hook unwind it (the raw `input::set_mouse_capture` free function
    /// does not, so its enable leaks on any early return or panic).
    mouse_capture_enabled: bool,
    /// Opt-in focus-change reporting is on, tracked and unwound for the same reason.
    focus_reporting_enabled: bool,
    /// Opt-in key-release reporting is on (#275): kitty keyboard flags pushed on the current
    /// screen, popped by `restore`, `Drop` and the panic hook.
    key_release_reporting_enabled: bool,
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
            mouse_capture_enabled: false,
            focus_reporting_enabled: false,
            key_release_reporting_enabled: false,
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
            mouse_capture_enabled: false,
            focus_reporting_enabled: false,
            key_release_reporting_enabled: false,
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
                    // Disable exactly the reporting modes the owner turned on; the
                    // hook can't see the session, so it reads the lease-scoped flags.
                    if take_key_releases_pushed() {
                        // Before leaving the alternate screen: the push is on its stack. Claimed,
                        // so the restore that unwinding runs next does not pop again.
                        let _ = pop_key_releases(&mut out);
                        // No release will be reported now: stop delivering them (and late
                        // answers) even if the panic is caught and the caller keeps polling.
                        crate::input::set_key_releases_delivered(false);
                        KEY_RELEASES_ENDED.store(true, Ordering::SeqCst);
                    }
                    let modes = REPORTING_MODES.load(Ordering::SeqCst);
                    if modes & REPORT_FOCUS != 0 {
                        let _ = execute!(out, DisableFocusChange);
                    }
                    if modes & REPORT_MOUSE != 0 {
                        let _ = execute!(out, DisableMouseCapture);
                    }
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

        // Identity, not liveness: record that input is now a live controlling
        // terminal, so a later hangup (after which `isatty` reports false) still
        // routes through the hangup-aware poll path instead of demoting to the
        // spin-on-a-hung-fd backend (issue #79). Sampled BEFORE raw mode is entered, so a
        // hangup during the acquisition itself cannot recast a hung-up stdin as "reach the
        // terminal through /dev/tty" (#269 review); a no-op once latched.
        let latched_now = crate::input::note_tty_source_acquired();

        if !self.raw_mode_enabled {
            if let Err(e) = enable_raw_mode() {
                // Not acquired after all: withdraw the identity this call latched, if it
                // latched one. A latch (and source, with its queued events) established
                // earlier, by a public poll for instance, is not this call's to undo.
                if let Some(identity) = latched_now {
                    crate::input::forget_tty_source_acquisition(identity);
                }
                return Err(e);
            }
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
        // Key-release reporting follows the screen: the kitty flag stacks are per screen.
        let follow = self.key_release_reporting_live();
        if follow {
            pop_key_releases(&mut out)?;
        }
        execute!(out, EnterAlternateScreen)?;
        self.alt_screen_active = true;
        if follow {
            request_key_releases(&mut out)?;
        }
        Ok(())
    }

    /// Leaves alternate screen mode.
    pub fn leave_alternate_screen(&mut self) -> io::Result<()> {
        if !self.is_tty || !self.alt_screen_active {
            return Ok(());
        }

        let mut out = stdout();
        let follow = self.key_release_reporting_live();
        if follow {
            pop_key_releases(&mut out)?;
        }
        execute!(out, LeaveAlternateScreen)?;
        self.alt_screen_active = false;
        if follow {
            request_key_releases(&mut out)?;
        }
        Ok(())
    }

    /// Enables or disables SGR mouse-capture reporting, tracked by the session so
    /// [`restore`](TerminalSession::restore), `Drop` and the panic hook all unwind
    /// it — unlike the raw `input::set_mouse_capture` free function, whose
    /// enable leaks on any early return or panic. Idempotent; a no-op on a non-TTY
    /// or headless session. On enable the flag is set *before* the write, so even a
    /// failed or partial enable is still disabled on restore.
    pub fn set_mouse_capture(&mut self, enable: bool) -> io::Result<()> {
        if !self.is_tty || self.headless || enable == self.mouse_capture_enabled {
            return Ok(());
        }
        let mut out = stdout();
        if enable {
            self.mouse_capture_enabled = true;
            REPORTING_MODES.fetch_or(REPORT_MOUSE, Ordering::SeqCst);
            execute!(out, EnableMouseCapture)?;
        } else {
            execute!(out, DisableMouseCapture)?;
            self.mouse_capture_enabled = false;
            REPORTING_MODES.fetch_and(!REPORT_MOUSE, Ordering::SeqCst);
        }
        Ok(())
    }

    /// Whether session-owned mouse-capture reporting is currently enabled.
    pub fn mouse_capture_enabled(&self) -> bool {
        self.mouse_capture_enabled
    }

    /// Enables or disables focus-change reporting, tracked and unwound exactly like
    /// [`set_mouse_capture`](TerminalSession::set_mouse_capture).
    pub fn set_focus_reporting(&mut self, enable: bool) -> io::Result<()> {
        if !self.is_tty || self.headless || enable == self.focus_reporting_enabled {
            return Ok(());
        }
        let mut out = stdout();
        if enable {
            self.focus_reporting_enabled = true;
            REPORTING_MODES.fetch_or(REPORT_FOCUS, Ordering::SeqCst);
            execute!(out, EnableFocusChange)?;
        } else {
            execute!(out, DisableFocusChange)?;
            self.focus_reporting_enabled = false;
            REPORTING_MODES.fetch_and(!REPORT_FOCUS, Ordering::SeqCst);
        }
        Ok(())
    }

    /// Whether session-owned focus reporting is currently enabled.
    pub fn focus_reporting_enabled(&self) -> bool {
        self.focus_reporting_enabled
    }

    /// Asks the terminal to report key releases, or stops asking (#275). **Off by default.**
    ///
    /// A terminal reports key presses and autorepeats, not releases, so a program that acts
    /// while a key is held has to infer the hold ([`crate::hold`]), and two keys cannot be held
    /// at once (autorepeat follows only the last key pressed). A terminal that speaks the kitty
    /// keyboard protocol can report releases (tested in kitty 0.32.2; foot, Ghostty and WezTerm
    /// with its kitty-keyboard option implement the protocol but are not tested). On
    /// enable this pushes the protocol's flags for the current screen and asks the terminal
    /// whether it took them; the answer arrives in input order as
    /// [`Event::KeyReleaseReporting`](crate::input::Event::KeyReleaseReporting): `Active`, after
    /// which every key that goes up is reported as an
    /// [`Event::KeyRelease`](crate::input::Event::KeyRelease), or `Unsupported`: the terminal
    /// answered only the device attributes query, or took only some of the flags (without them
    /// all, Enter, Tab, Backspace and Esc report their release as a second press). `Unsupported`
    /// is final: as a [`Context`](crate::context::Context) poll (`poll_event`, `poll_from`,
    /// `run_once`) hands it out, reporting is withdrawn as by disabling it, with one pop of the
    /// push (never repeated by `restore`, `Drop` or the panic hook), and no
    /// [`Event::KeyRelease`](crate::input::Event::KeyRelease) is delivered from then on, including
    /// any already queued behind the answer. A device attributes reply to somebody else's earlier
    /// query, arriving first, cannot be told apart and reads as `Unsupported` too. A terminal
    /// that answers neither query leaves the session press-only with no answer at all.
    ///
    /// While enabled the terminal sends every key as an escape sequence. LibGibson decodes most
    /// into the same [`Event::Key`](crate::input::Event::Key)s as before (`Esc`, `Enter`, `Tab`,
    /// `Backspace`, Ctrl and Alt chords, shifted symbols such as `!`), with these differences: a
    /// shifted letter is its capital without `SHIFT` (Ctrl+Shift+W is `W` with Ctrl, not `w`);
    /// Caps Lock does not change a letter (`a`, not `A`); and Ctrl+I, Ctrl+M and Ctrl+[ are Ctrl
    /// chords of `i`, `m` and `[`, not `Tab`, `Enter` and `Esc`. Modifier keys alone are reported,
    /// and not delivered. Text typed through an input method may not arrive: enable this for
    /// direct key control (a game), not for text entry.
    ///
    /// Enabling enters interactive (raw) mode first ([`enter_interactive`](Self::enter_interactive)),
    /// so the terminal's answer is never echoed. Tracked and unwound like
    /// [`set_mouse_capture`](TerminalSession::set_mouse_capture):
    /// `restore`, `Drop` and the panic hook pop the flags. Once the panic hook has popped them,
    /// even for a panic that is caught, no release or terminal answer is delivered,
    /// [`key_release_reporting_enabled`](Self::key_release_reporting_enabled) is false, and the
    /// next `Context` poll delivers `Unsupported` once, so an application tracking holds switches
    /// back to inferred holds. A process killed outright (SIGKILL)
    /// leaves them pushed and the shell then receives escape sequences for Enter; kitty's reset
    /// (Ctrl+Shift+Delete) clears them. Moving between the main and alternate screen while enabled
    /// pops the flags on the screen being left and asks again on the new one, so a second answer
    /// follows. Disabling pops the flags and drops anything not yet delivered: an answer, a
    /// release; an application tracking holds then calls
    /// [`HoldTracker::set_releases_reported(false)`](crate::hold::HoldTracker::set_releases_reported)
    /// itself. A release names its key as the press did for the 32 most recent keys still down.
    /// Idempotent; a no-op off Unix, on a non-TTY or headless session, and on a session that has
    /// been restored (it no longer holds the terminal).
    pub fn set_key_release_reporting(&mut self, enable: bool) -> io::Result<()> {
        self.key_release_reporting_live();
        if !cfg!(unix)
            || !self.is_tty
            || self.headless
            || !self.owns_lease
            || self.restore_attempted
            || enable == self.key_release_reporting_enabled
        {
            return Ok(());
        }
        if enable {
            // Raw mode first: the terminal answers at once, and a cooked terminal would echo the
            // answer onto the screen (and hold it back until a newline).
            self.enter_interactive()?;
        }
        let mut out = stdout();
        if enable {
            // Flags set before the write, so even a failed or partial enable is unwound.
            self.key_release_reporting_enabled = true;
            // A new answer follows; an earlier end of reporting is no longer news.
            KEY_RELEASES_ENDED.store(false, Ordering::SeqCst);
            REPORTING_MODES.fetch_or(REPORT_KEY_RELEASE, Ordering::SeqCst);
            crate::input::set_key_releases_delivered(true);
            request_key_releases(&mut out)
        } else {
            let result = if take_key_releases_pushed() {
                pop_key_releases(&mut out)
            } else {
                Ok(())
            };
            self.key_release_reporting_enabled = false;
            crate::input::set_key_releases_delivered(false);
            // An answer still on its way belongs to a request no one is waiting for.
            #[cfg(unix)]
            crate::tty_source::forget_keyboard_replies();
            result
        }
    }

    /// Whether session-owned key-release reporting is currently enabled (requested; whether the
    /// terminal confirmed it is the answer event's business). False once the panic hook has popped
    /// the flags, even if the panic was caught.
    pub fn key_release_reporting_enabled(&self) -> bool {
        self.key_release_reporting_enabled && key_releases_pushed()
    }

    /// Whether reporting is still on, after noticing that the panic hook popped the flags (a
    /// caught panic; the hook already stopped delivery): then it is off, and enabling again pushes
    /// anew (#275 re-review).
    fn key_release_reporting_live(&mut self) -> bool {
        if self.key_release_reporting_enabled && !key_releases_pushed() {
            self.key_release_reporting_enabled = false;
            crate::input::set_key_releases_delivered(false);
            // The hook pops only after disabling raw mode: say so, so that enabling again (which
            // enters interactive mode first) restores raw mode rather than trusting a stale flag.
            self.raw_mode_enabled = false;
        }
        self.key_release_reporting_enabled
    }

    /// Whether the panic hook ended key-release reporting since the last poll (#275); true once
    /// per end, for the lease owner only. The poll then delivers `Unsupported`, so a client still
    /// waiting for releases switches back to inferred holds. Notes the end here too: raw mode is
    /// marked off (the hook left it), so the poll's `enter_interactive` restores it.
    pub(crate) fn take_key_releases_ended(&mut self) -> bool {
        if !self.owns_lease || !KEY_RELEASES_ENDED.swap(false, Ordering::SeqCst) {
            return false;
        }
        self.key_release_reporting_live();
        true
    }

    /// Called with each event a [`Context`](crate::context::Context) poll hands out (#275). An
    /// `Unsupported` answer is final: reporting is withdrawn as by disabling it, with exactly one
    /// pop of this session's push (claimed, so restore, `Drop` and the panic hook never pop it
    /// again), and no release is delivered from then on, including any queued behind the answer.
    pub(crate) fn key_release_answer_delivered(&mut self, ev: &Option<crate::input::Event>) {
        use crate::input::{Event, KeyReleaseReporting};
        if matches!(
            ev,
            Some(Event::KeyReleaseReporting(KeyReleaseReporting::Unsupported))
        ) {
            // Best effort: the answer itself must still reach the caller. A failed pop write
            // leaves the terminal as a failed disable does.
            let _ = self.set_key_release_reporting(false);
        }
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

        // Unwind opt-in reporting FIRST, reverse of enable order (key releases, focus,
        // then mouse). Each disable is emitted only when the mode was actually enabled,
        // so a session that never turned these on writes the exact same restore
        // bytes as before (the PTY/golden streams are unchanged). The key-release pop
        // must precede leaving the alternate screen: the push is on that screen's stack.
        if self.key_release_reporting_enabled {
            // The panic hook may already have popped (it claims the pop): never pop twice.
            if take_key_releases_pushed() {
                record(pop_key_releases(&mut out));
            }
            self.key_release_reporting_enabled = false;
            crate::input::set_key_releases_delivered(false);
        }
        #[cfg(unix)]
        crate::tty_source::forget_keyboard_replies();
        if self.focus_reporting_enabled {
            record(execute!(out, DisableFocusChange));
            self.focus_reporting_enabled = false;
        }
        if self.mouse_capture_enabled {
            record(execute!(out, DisableMouseCapture));
            self.mouse_capture_enabled = false;
        }

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

        // Relinquish the latched terminal identity with the terminal itself, so a
        // later acquisition re-establishes it and separate sessions stay isolated.
        crate::input::clear_tty_source();

        // Release last, once the ops are done: clear the owner thread and hand
        // the lease back to the pool only after we've stopped touching the
        // terminal. Even if the writes above failed, raw mode was still restored
        // via termios (an ioctl, not the failed output fd) and the lease returns
        // to Available: a broken output stream is the caller's to see (via the
        // returned Err), not a reason to wedge the lease shut (issue #11, B1/B2).
        if self.owns_lease {
            REPORTING_MODES.store(0, Ordering::SeqCst);
            KEY_RELEASES_ENDED.store(false, Ordering::SeqCst);
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
