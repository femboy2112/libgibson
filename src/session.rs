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
use std::sync::{Arc, Mutex, Weak};
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

/// The owner-thread panic hook attempted to disable mouse reporting. A caught panic leaves
/// cleanup owed, but the next enable must issue a fresh request rather than trust the old one.
static PANIC_MOUSE_CAPTURE_INVALIDATED: AtomicBool = AtomicBool::new(false);
// The panic hook cannot borrow the owning session. It sets this notice before attempting its
// focus disable so a caught panic cannot leave the session believing its previous enable is live.
static FOCUS_REPORTING_INVALIDATED: AtomicBool = AtomicBool::new(false);

/// Set by the panic hook when it ends key-release reporting; taken once, by the owner's next
/// `Context` poll or by the Unix source before the next answer it decodes, which then delivers
/// `KeyReleaseReporting::Unsupported` (see [`TerminalSession::take_key_releases_ended`] and
/// [`take_key_releases_ended_notice`]). An atomic: the hook takes no lock.
static KEY_RELEASES_ENDED: AtomicBool = AtomicBool::new(false);

/// The owner-thread panic hook has attempted to leave raw input and bracketed paste. A caught
/// panic leaves the session alive, so its next interactive entry must not trust its old flags.
/// This notice is lease-scoped; it does not claim recovery of screen, cursor, renderer or capture.
static PANIC_INPUT_MODES_INVALIDATED: AtomicBool = AtomicBool::new(false);

/// Takes the panic hook's notice that reporting ended, for the Unix source to queue just before an
/// answer it decodes (#275 review 8). One swap against the owner's
/// [`take_key_releases_ended`](TerminalSession::take_key_releases_ended): exactly one of them
/// says it, and an answer decoded after the end never overtakes it, whichever poll hands it out.
#[cfg(unix)]
pub(crate) fn take_key_releases_ended_notice() -> bool {
    KEY_RELEASES_ENDED.swap(false, Ordering::SeqCst)
}

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

// A weak reference cannot keep a relinquished session's descriptor alive. The hook upgrades it
// only for the duration of its cleanup; neither this mutex nor OWNER_THREAD is held across I/O.
static OWNER_OUTPUT: Mutex<Weak<SessionEndpoint>> = Mutex::new(Weak::new());

/// One captured output destination, shared only within a managed session's lifetime.
/// On Unix this owns a duplicated kernel descriptor, not an fd-number identity guess.
struct SessionEndpoint {
    #[cfg(unix)]
    file: std::fs::File,
}

impl SessionEndpoint {
    fn capture_stdout() -> io::Result<Arc<Self>> {
        #[cfg(unix)]
        {
            use std::os::fd::AsFd;
            let file = stdout().as_fd().try_clone_to_owned()?.into();
            Ok(Arc::new(Self { file }))
        }
        #[cfg(not(unix))]
        {
            // Native Windows endpoint pinning is not qualified by this Unix experiment.
            Ok(Arc::new(Self {}))
        }
    }

    fn is_terminal(&self) -> bool {
        #[cfg(unix)]
        {
            self.file.is_terminal()
        }
        #[cfg(not(unix))]
        {
            stdout().is_terminal()
        }
    }

    #[cfg(unix)]
    fn terminal_size(&self) -> io::Result<(u16, u16)> {
        use std::os::fd::AsRawFd;
        let mut size = std::mem::MaybeUninit::<libc::winsize>::uninit();
        // SAFETY: the owned descriptor stays alive throughout the call and TIOCGWINSZ
        // initializes the supplied winsize on success.
        if unsafe { libc::ioctl(self.file.as_raw_fd(), libc::TIOCGWINSZ, size.as_mut_ptr()) } == -1
        {
            return Err(io::Error::last_os_error());
        }
        let size = unsafe { size.assume_init() };
        Ok((size.ws_col, size.ws_row))
    }
}

/// A short-lived strong reference used for a single managed operation.
struct SessionWriter(Option<Arc<SessionEndpoint>>);

impl SessionWriter {
    fn endpoint(&self) -> io::Result<&SessionEndpoint> {
        self.0.as_deref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotConnected,
                "session output was relinquished",
            )
        })
    }
}

impl Write for SessionWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        #[cfg(unix)]
        {
            (&self.endpoint()?.file).write(bytes)
        }
        #[cfg(not(unix))]
        {
            stdout().write(bytes)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        #[cfg(unix)]
        {
            (&self.endpoint()?.file).flush()
        }
        #[cfg(not(unix))]
        {
            stdout().flush()
        }
    }
}

/// Context output borrows the session endpoint without extending descriptor ownership.
pub(crate) struct SessionOutput(Weak<SessionEndpoint>);

impl Write for SessionOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writer()?.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer()?.flush()
    }
}

impl SessionOutput {
    fn writer(&self) -> io::Result<SessionWriter> {
        self.0
            .upgrade()
            .map(|p| SessionWriter(Some(p)))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotConnected,
                    "session output was relinquished",
                )
            })
    }
}

fn set_owner_output(output: Weak<SessionEndpoint>) {
    *OWNER_OUTPUT.lock().unwrap_or_else(|p| p.into_inner()) = output;
}

fn owner_output() -> Option<SessionWriter> {
    OWNER_OUTPUT
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .upgrade()
        .map(|p| SessionWriter(Some(p)))
}

/// Capture the current managed output identity for a resize poll. If this
/// captured endpoint is relinquished, the reader does not fall back to stdout.
#[cfg(unix)]
pub(crate) fn resize_size_reader() -> Box<dyn Fn() -> io::Result<(u16, u16)> + Send> {
    if TERMINAL_LEASE.load(Ordering::SeqCst) == LEASE_AVAILABLE {
        return Box::new(crossterm::terminal::size);
    }
    let endpoint = OWNER_OUTPUT
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    Box::new(move || {
        endpoint
            .upgrade()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "output lease ended"))?
            .terminal_size()
    })
}

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
    /// Presentation flag; changing it does not acquire or release a terminal lease.
    pub is_tty: bool,
    /// Stable instance identity, including across moves. Contexts bind their
    /// renderer state to the session that produced it.
    identity: Arc<()>,
    /// The interactive lease owns its captured output descriptor until restore or Drop.
    output_endpoint: Option<Arc<SessionEndpoint>>,
    /// When true, all direct OS terminal manipulation is suppressed while the
    /// session still behaves like a TTY for the renderer. This enables
    /// byte-stream tests against a virtual terminal (e.g. `vt100`).
    headless: bool,
    headless_size: (u16, u16),
    raw_mode_enabled: bool,
    alt_screen_active: bool,
    cursor_hidden: bool,
    /// A paste-enable may have reached the terminal and requires a disable on cleanup.
    bracketed_paste_enabled: bool,
    /// A paste-enable write and flush succeeded since the last owner-thread panic.
    bracketed_paste_confirmed: bool,
    /// Opt-in SGR mouse-capture reporting is on. Tracked so `restore`, `Drop` and
    /// the panic hook unwind it (the raw `input::set_mouse_capture` free function
    /// does not, so its enable leaks on any early return or panic).
    mouse_capture_enabled: bool,
    /// A mouse-enable write and flush completed since the last uncertain transition.
    mouse_capture_confirmed: bool,
    /// A focus enable may have reached the terminal; a disable is required on cleanup.
    focus_reporting_enabled: bool,
    /// The latest focus-enable write and flush succeeded. A failed transition invalidates this
    /// without clearing cleanup ownership, because a partial write may have reached the terminal.
    focus_reporting_confirmed: bool,
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
        let output = stdout();
        let mut output_lock = output.lock();
        // Preserve non-TTY construction, including a closed stdout. Capture a descriptor only
        // for a candidate interactive session, before any terminal effects or lease publication.
        let endpoint = if output_lock.is_terminal() {
            Some(SessionEndpoint::capture_stdout()?)
        } else {
            None
        };
        let is_tty = endpoint.as_ref().is_some_and(|p| p.is_terminal());

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
            // Drain pre-existing Rust stdout buffering before bypassing it with the captured
            // descriptor. Do this only after the lease CAS: a rejected second owner must not
            // flush another owner's pending output. No reporting state has been published yet.
            if let Err(error) = output_lock.flush() {
                TERMINAL_LEASE.store(LEASE_AVAILABLE, Ordering::SeqCst);
                return Err(error);
            }
            // Sole owner now: record which thread holds it, so the panic hook
            // only restores for a panic on *this* thread (issue #11, B3).
            set_owner_thread(Some(std::thread::current().id()));
            set_owner_output(Arc::downgrade(endpoint.as_ref().unwrap()));
            true
        } else {
            false
        };

        let session = Self {
            is_tty,
            identity: Arc::new(()),
            output_endpoint: if owns_lease { endpoint } else { None },
            headless: false,
            headless_size: (80, 24),
            raw_mode_enabled: false,
            alt_screen_active: false,
            cursor_hidden: false,
            bracketed_paste_enabled: false,
            bracketed_paste_confirmed: false,
            mouse_capture_enabled: false,
            mouse_capture_confirmed: false,
            focus_reporting_enabled: false,
            focus_reporting_confirmed: false,
            key_release_reporting_enabled: false,
            sync_updates_enabled: true,
            capabilities: TerminalCapabilities::detect_from_env(),
            owns_lease,
            restore_attempted: false,
        };

        drop(output_lock);
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
            identity: Arc::new(()),
            output_endpoint: None,
            headless: true,
            headless_size: (cols, rows),
            raw_mode_enabled: false,
            alt_screen_active: false,
            cursor_hidden: false,
            bracketed_paste_enabled: false,
            bracketed_paste_confirmed: false,
            mouse_capture_enabled: false,
            mouse_capture_confirmed: false,
            focus_reporting_enabled: false,
            focus_reporting_confirmed: false,
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

    /// True once [`restore`](TerminalSession::restore) has begun on this session (explicitly or
    /// from `Drop`): it has handed the terminal back and released its lease.
    pub(crate) fn restore_attempted(&self) -> bool {
        self.restore_attempted
    }

    /// Guard managed rendering before it changes renderer state or emits bytes.
    /// The public `is_tty` presentation flag is not evidence of lease ownership.
    pub(crate) fn ensure_render_authorized(&self) -> io::Result<()> {
        if !self.headless && (self.restore_attempted || (self.is_tty && !self.owns_lease)) {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "terminal session has no active ownership lease",
            ));
        }
        Ok(())
    }

    pub(crate) fn owns_terminal_lease(&self) -> bool {
        self.owns_lease && !self.restore_attempted
    }

    fn output_writer(&self) -> SessionWriter {
        SessionWriter(self.output_endpoint.clone())
    }

    pub(crate) fn output_sink(&self) -> Option<SessionOutput> {
        self.output_endpoint
            .as_ref()
            .map(|p| SessionOutput(Arc::downgrade(p)))
    }

    pub(crate) fn identity(&self) -> Arc<()> {
        Arc::clone(&self.identity)
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
                    PANIC_INPUT_MODES_INVALIDATED.store(true, Ordering::SeqCst);
                    let _ = disable_raw_mode();
                    if let Some(mut out) = owner_output() {
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
                            FOCUS_REPORTING_INVALIDATED.store(true, Ordering::SeqCst);
                            let _ = execute!(out, DisableFocusChange);
                        }
                        if modes & REPORT_MOUSE != 0 {
                            PANIC_MOUSE_CAPTURE_INVALIDATED.store(true, Ordering::SeqCst);
                            let _ = execute!(out, DisableMouseCapture);
                        }
                        let _ = execute!(out, Show, LeaveAlternateScreen, DisableBracketedPaste);
                        let _ = out.write_all(b"\x1b[0m\x1b[?2026l\x1b[?7h\x1b[?25h\r\n");
                        let _ = out.flush();
                    }
                }

                default_hook(info);
            }));
        }
    }

    /// Enables raw mode and bracketed paste for interactive operation. If the owner-thread panic
    /// hook ran and the panic was caught, requests both again on the next interactive entry.
    /// This input recovery does not restore alternate-screen, cursor, renderer or capture state.
    pub fn enter_interactive(&mut self) -> io::Result<()> {
        if !self.is_tty || self.headless {
            return Ok(());
        }

        if self.restore_attempted || !self.owns_lease {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "enter_interactive after restore: this session has released the terminal",
            ));
        }

        self.reconcile_panic_input_modes();

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

        self.enable_bracketed_paste_on(&mut self.output_writer())?;

        Ok(())
    }

    fn reconcile_panic_input_modes(&mut self) {
        if self.owns_lease && PANIC_INPUT_MODES_INVALIDATED.swap(false, Ordering::SeqCst) {
            self.raw_mode_enabled = false;
            // A failed panic-hook disable may have left paste enabled. Keep cleanup ownership
            // even if raw-mode re-entry fails before another paste request can be made.
            self.bracketed_paste_confirmed = false;
        }
    }

    fn enable_bracketed_paste_on(&mut self, out: &mut impl Write) -> io::Result<()> {
        if !self.bracketed_paste_confirmed {
            // A failed write or flush may still have enabled paste at the terminal. Retain a
            // cleanup obligation, but retry the request until a write and flush both succeed.
            self.bracketed_paste_enabled = true;
            execute!(out, EnableBracketedPaste)?;
            self.bracketed_paste_confirmed = true;
        }
        Ok(())
    }

    fn disable_bracketed_paste_on(&mut self, out: &mut impl Write) -> io::Result<()> {
        if !self.bracketed_paste_enabled {
            return Ok(());
        }
        let result = execute!(out, DisableBracketedPaste);
        self.bracketed_paste_enabled = false;
        self.bracketed_paste_confirmed = false;
        result
    }

    /// Enters alternate screen mode (fullscreen). A restored session no longer owns the
    /// terminal and returns `NotConnected` rather than changing a later owner's screen.
    pub fn enter_alternate_screen(&mut self) -> io::Result<()> {
        if !self.is_tty || self.headless {
            return Ok(());
        }
        self.ensure_mode_owner()?;
        if self.alt_screen_active {
            return Ok(());
        }
        let mut out = self.output_writer();
        self.switch_screen_on(&mut out, |o| execute!(o, EnterAlternateScreen), true)
    }

    /// Leaves alternate screen mode; a restored session returns `NotConnected`.
    pub fn leave_alternate_screen(&mut self) -> io::Result<()> {
        if !self.is_tty || self.headless {
            return Ok(());
        }
        self.ensure_mode_owner()?;
        if !self.alt_screen_active {
            return Ok(());
        }
        let mut out = self.output_writer();
        self.switch_screen_on(&mut out, |o| execute!(o, LeaveAlternateScreen), false)
    }

    /// A relinquished interactive session must not operate on a later owner's terminal.
    fn ensure_mode_owner(&self) -> io::Result<()> {
        if self.restore_attempted || !self.owns_lease {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "terminal session has released its ownership lease",
            ));
        }
        Ok(())
    }

    /// Relocates this process's key-release push across a screen switch, then performs the
    /// switch, for [`enter_alternate_screen`](Self::enter_alternate_screen) and
    /// [`leave_alternate_screen`](Self::leave_alternate_screen). The kitty keyboard-flag
    /// stacks are per screen, so the push on the old screen is popped and a fresh one placed
    /// on the new one. Generic over the writer so the failure paths can be tested without a
    /// real terminal.
    ///
    /// #305: the push bit is CLAIMED (via [`take_key_releases_pushed`]) the moment the pop
    /// *succeeds*, so if the switch below then fails, no later `restore`/`Drop`/retry
    /// pops a second time and removes another program's keyboard-stack entry. A pop that *fails*
    /// leaves the bit set and propagates, allowing restore to try a pop. A failed write or flush
    /// does not prove whether the terminal applied that pop. The re-push sets the bit *before*
    /// its write, because a torn re-push may already have placed our push on the new screen and
    /// so must stay
    /// poppable (the same defensive unwinding the initial enable uses). A failed switch marks
    /// reporting ended via [`on_key_release_push_lost`](Self::on_key_release_push_lost) without
    /// touching raw mode.
    ///
    /// This closes the specific #305 case — a successful pop followed by a failed switch. Under a
    /// buffered pop whose flush tears then recovers, or a re-push that writes nothing, the
    /// best-effort bit can still mis-pop once on restore: the same non-atomic-write limitation the
    /// initial enable carries and `docs/RELEASE_CONTRACT.md` §9 discloses, which a per-screen
    /// push/pop protocol over buffered stdout cannot fully avoid.
    fn switch_screen_on<W: Write>(
        &mut self,
        out: &mut W,
        switch: impl FnOnce(&mut W) -> io::Result<()>,
        alt_active_after: bool,
    ) -> io::Result<()> {
        let follow = self.key_release_reporting_live();
        if follow {
            // Leave the bit set on failure, so restore can try to pop. A failed write or flush
            // cannot establish whether the terminal has already applied the pop.
            pop_key_releases(out)?;
            // The pop succeeded: claim the bit now, so a failed switch does not cause a later
            // restore/Drop to pop a second time and remove another program's entry (#305).
            take_key_releases_pushed();
        }
        if let Err(e) = switch(&mut *out) {
            if follow {
                self.on_key_release_push_lost();
            }
            return Err(e);
        }
        self.alt_screen_active = alt_active_after;
        if follow {
            // Set the bit BEFORE the re-push: a torn or failed re-push write may already have
            // placed our push on the new screen's stack, so it must stay poppable by restore/Drop
            // (the same defensive unwinding the initial enable uses). This is deliberately the
            // opposite of the pop above, which clears the bit after success. Neither choice
            // guarantees ownership after an ambiguous I/O failure.
            REPORTING_MODES.fetch_or(REPORT_KEY_RELEASE, Ordering::SeqCst);
            request_key_releases(out)?;
        }
        Ok(())
    }

    /// Reconciles session state after a key-release push was lost mid screen-switch (#305):
    /// the `REPORT_KEY_RELEASE` bit is already clear after the successful pop and failed switch,
    /// so stop delivering releases and mark reporting disabled. Raw mode is still on, so its
    /// cleanup obligation must stay intact. Panic-triggered input invalidation is handled
    /// separately on the next interactive entry.
    fn on_key_release_push_lost(&mut self) {
        self.key_release_reporting_enabled = false;
        crate::input::set_key_releases_delivered(false);
        #[cfg(unix)]
        crate::tty_source::forget_keyboard_replies();
    }

    /// Enables or disables SGR mouse-capture reporting, tracked by the session so
    /// [`restore`](TerminalSession::restore), `Drop` and the panic hook all unwind
    /// it — unlike the raw `input::set_mouse_capture` free function, whose
    /// enable leaks on any early return or panic. Idempotent; a no-op on a non-TTY
    /// or headless session. On enable the flag is set *before* the write, so even a
    /// failed or partial enable is still disabled on restore. An unconfirmed enable, an uncertain
    /// disable, or a caught owner-thread panic causes the next enable to send a fresh request.
    /// Success means the request was written and flushed, not that the terminal supports it.
    /// A restored interactive session returns `NotConnected` without changing the current owner's
    /// reporting modes.
    pub fn set_mouse_capture(&mut self, enable: bool) -> io::Result<()> {
        if !self.is_tty || self.headless {
            return Ok(());
        }
        self.ensure_mode_owner()?;
        self.set_mouse_capture_on(enable, &mut self.output_writer())
    }

    fn set_mouse_capture_on(&mut self, enable: bool, out: &mut impl Write) -> io::Result<()> {
        if self.owns_lease && PANIC_MOUSE_CAPTURE_INVALIDATED.swap(false, Ordering::SeqCst) {
            self.mouse_capture_confirmed = false;
        }
        if enable {
            if self.mouse_capture_confirmed {
                return Ok(());
            }
            // Retain cleanup ownership even if any part of the write or flush fails.
            self.mouse_capture_enabled = true;
            REPORTING_MODES.fetch_or(REPORT_MOUSE, Ordering::SeqCst);
            execute!(out, EnableMouseCapture)?;
            self.mouse_capture_confirmed = true;
        } else {
            if !self.mouse_capture_enabled {
                return Ok(());
            }
            // A failed disable could have taken effect; do not treat the old enable as confirmed.
            self.mouse_capture_confirmed = false;
            execute!(out, DisableMouseCapture)?;
            self.mouse_capture_enabled = false;
            REPORTING_MODES.fetch_and(!REPORT_MOUSE, Ordering::SeqCst);
        }
        Ok(())
    }

    /// Whether the session's most recent mouse-enable request was written and flushed without
    /// an intervening uncertain transition. This does not establish terminal support.
    pub fn mouse_capture_enabled(&self) -> bool {
        self.mouse_capture_confirmed
            && !(self.owns_lease && PANIC_MOUSE_CAPTURE_INVALIDATED.load(Ordering::SeqCst))
    }

    /// Requests or disables focus-change reporting, tracked for cleanup like
    /// [`set_mouse_capture`](TerminalSession::set_mouse_capture). If a write or flush fails,
    /// the terminal state is unknown and the next enable retries the request. A caught owner-thread
    /// panic also invalidates the prior request. A successful write does not establish that the
    /// terminal supports focus reporting. Headless and non-TTY sessions are no-ops; enabling a
    /// restored interactive session returns `NotConnected`, and disabling it is a no-op.
    pub fn set_focus_reporting(&mut self, enable: bool) -> io::Result<()> {
        if !self.is_tty || self.headless {
            return Ok(());
        }
        if !self.owns_lease || self.restore_attempted {
            return if enable {
                Err(io::Error::new(
                    io::ErrorKind::NotConnected,
                    "terminal session is not active",
                ))
            } else {
                Ok(())
            };
        }
        self.set_focus_reporting_with_writer(enable, &mut self.output_writer())
    }

    fn set_focus_reporting_with_writer(
        &mut self,
        enable: bool,
        out: &mut impl Write,
    ) -> io::Result<()> {
        if FOCUS_REPORTING_INVALIDATED.swap(false, Ordering::SeqCst) {
            self.focus_reporting_confirmed = false;
        }
        if (enable && self.focus_reporting_confirmed) || (!enable && !self.focus_reporting_enabled)
        {
            return Ok(());
        }
        // Until the flush succeeds the terminal's state is unknown: retain ownership for cleanup,
        // but do not let a retry report success without sending another request.
        self.focus_reporting_confirmed = false;
        if enable {
            self.focus_reporting_enabled = true;
            REPORTING_MODES.fetch_or(REPORT_FOCUS, Ordering::SeqCst);
            execute!(out, EnableFocusChange)?;
            self.focus_reporting_confirmed = true;
        } else {
            execute!(out, DisableFocusChange)?;
            self.focus_reporting_enabled = false;
            REPORTING_MODES.fetch_and(!REPORT_FOCUS, Ordering::SeqCst);
        }
        Ok(())
    }

    /// Whether this session owns a focus-reporting request and must disable it during cleanup.
    /// After a failed write the terminal's actual state is unknown; `true` does not confirm delivery.
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
    /// is final: once any poll hands it out (a [`Context`](crate::context::Context) poll,
    /// [`input::poll_event`](crate::input::poll_event) or a
    /// [`TerminalSource`](crate::event_source::TerminalSource), whatever context reads it), no
    /// [`Event::KeyRelease`](crate::input::Event::KeyRelease) is delivered from then on, including
    /// any already queued behind the answer, and reporting is withdrawn as by disabling it, with
    /// one pop of the push, never repeated: by this session's `Context` poll that handed it out,
    /// else on this session's next `Context` poll, enable, disable or screen switch, or at
    /// `restore`, `Drop` or the panic hook. An answer counts only for the request it answers: one
    /// decoded before reporting was last enabled or disabled is dropped by whichever poll reads
    /// it, and changes nothing. A device attributes reply to somebody else's earlier
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
    /// [`key_release_reporting_enabled`](Self::key_release_reporting_enabled) is false, and
    /// `Unsupported` is delivered once: by the next `Context` poll, or, if the terminal answers
    /// first, just before that answer by whichever poll hands it out. So even if reporting was
    /// enabled again meanwhile, it comes before the new answer, and an application tracking holds
    /// switches back to inferred holds. A `Context` poll that fails to re-enter raw mode leaves
    /// it for the next. A process killed outright (SIGKILL)
    /// leaves them pushed and the shell then receives escape sequences for Enter; kitty's reset
    /// (Ctrl+Shift+Delete) clears them. Moving between the main and alternate screen while enabled
    /// pops the flags on the screen being left and asks again on the new one, so a second answer
    /// follows. Disabling pops the flags (the one entry this session pushed, which restores the
    /// flags in force before it: legacy keys only if nothing else, such as the host of an inline
    /// session, had pushed its own) and drops anything not yet delivered: an answer, a
    /// release; an application tracking holds then calls
    /// [`HoldTracker::set_releases_reported(false)`](crate::hold::HoldTracker::set_releases_reported)
    /// itself. A release names its key as the press did for the 32 most recent keys still down,
    /// except a shifted symbol pressed just before the flags take effect and let go without an
    /// autorepeat (`!` is released as `1`;
    /// [#312](https://github.com/femboy2112/libgibson/issues/312)).
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
        let mut out = self.output_writer();
        if enable {
            // Flags set before the write, so even a failed or partial enable is unwound.
            self.key_release_reporting_enabled = true;
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
    /// the flags, even if the panic was caught, and once an `Unsupported` answer has been handed
    /// out, by any poll.
    pub fn key_release_reporting_enabled(&self) -> bool {
        self.key_release_reporting_enabled
            && key_releases_pushed()
            && crate::input::key_releases_delivered()
    }

    /// Whether reporting is still on, after noticing that the panic hook popped the flags (a
    /// caught panic; the hook already stopped delivery) or that an `Unsupported` answer was handed
    /// out (delivery is off; the push is popped here): then it is off, and enabling again pushes
    /// anew (#275 re-review).
    fn key_release_reporting_live(&mut self) -> bool {
        if self.key_release_reporting_enabled && !key_releases_pushed() {
            self.key_release_reporting_enabled = false;
            crate::input::set_key_releases_delivered(false);
            // The panic-input notice independently reconciles raw mode on interactive entry.
            // That entry may already have re-enabled raw mode before Kitty is reconciled;
            // clearing its bookkeeping here would forget a live cleanup obligation.
        } else if self.key_release_reporting_enabled && !crate::input::key_releases_delivered() {
            // An `Unsupported` answer was handed out (by whichever poll): withdraw this session's
            // push, once (claimed, so restore, `Drop` and the panic hook never pop it again).
            // Best effort, like the pop in a disable: a failed write is not retried.
            if take_key_releases_pushed() {
                let _ = pop_key_releases(&mut self.output_writer());
            }
            self.key_release_reporting_enabled = false;
            #[cfg(unix)]
            crate::tty_source::forget_keyboard_replies();
        }
        self.key_release_reporting_enabled
    }

    /// Notes what the panic hook or an `Unsupported` answer ended (see
    /// [`key_release_reporting_live`](Self::key_release_reporting_live)); a `Context` poll calls
    /// this before and after polling, so the push is withdrawn right after the answer is handed
    /// out, or on the owner's next poll if another reader handed it out.
    pub(crate) fn sync_key_release_reporting(&mut self) {
        // A restored session holds nothing to sync: the terminal, and any push on it, may be
        // another session's now. (Restore already cleared what this would read; this says so
        // where the next reader looks.)
        if self.restore_attempted {
            return;
        }
        self.key_release_reporting_live();
    }

    /// Whether the panic hook ended key-release reporting and nobody has said so yet (#275); true
    /// once per end, for the lease owner only, unless the Unix source took the notice first to
    /// queue it before an answer ([`take_key_releases_ended_notice`]). The poll then delivers
    /// `Unsupported`, so a client still waiting for releases switches back to inferred holds.
    /// Raw input is reconciled independently on the next interactive entry.
    /// Re-enabling does not clear it: the client must see `Unsupported` before any later answer.
    pub(crate) fn take_key_releases_ended(&mut self) -> bool {
        if !self.owns_lease || !KEY_RELEASES_ENDED.swap(false, Ordering::SeqCst) {
            return false;
        }
        self.key_release_reporting_live();
        true
    }

    /// Puts back a notice [`take_key_releases_ended`](Self::take_key_releases_ended) took when
    /// the poll that would have delivered it failed first.
    pub(crate) fn keep_key_releases_ended(&mut self) {
        KEY_RELEASES_ENDED.store(true, Ordering::SeqCst);
    }

    /// Hides the hardware cursor.
    pub fn hide_cursor(&mut self) -> Option<&'static [u8]> {
        if !self.is_tty || self.cursor_hidden || self.ensure_render_authorized().is_err() {
            return None;
        }
        self.cursor_hidden = true;
        Some(b"\x1b[?25l")
    }

    /// Shows the hardware cursor.
    pub fn show_cursor(&mut self) -> Option<&'static [u8]> {
        if !self.is_tty || !self.cursor_hidden || self.ensure_render_authorized().is_err() {
            return None;
        }
        self.cursor_hidden = false;
        Some(b"\x1b[?25h")
    }

    // Rendering builds a transaction before writing it. Cursor bookkeeping
    // must follow the write result, not merely the construction of bytes.
    pub(crate) fn cursor_hidden_for_render(&self) -> bool {
        self.cursor_hidden
    }

    pub(crate) fn reconcile_failed_cursor_write(&mut self, before: bool, uncertain: bool) {
        // A partially delivered hide may still need cleanup; a partially
        // delivered show may not have reached the terminal at all.
        self.cursor_hidden = if uncertain {
            before || self.cursor_hidden
        } else {
            before
        };
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
        if self.headless || self.restore_attempted || !self.owns_lease {
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

        let mut out = self.output_writer();

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
            self.focus_reporting_confirmed = false;
        }
        if self.mouse_capture_enabled {
            record(execute!(out, DisableMouseCapture));
            self.mouse_capture_enabled = false;
            self.mouse_capture_confirmed = false;
        }

        if self.cursor_hidden {
            record(execute!(out, Show));
            self.cursor_hidden = false;
        }

        record(self.disable_bracketed_paste_on(&mut out));

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
            FOCUS_REPORTING_INVALIDATED.store(false, Ordering::SeqCst);
            KEY_RELEASES_ENDED.store(false, Ordering::SeqCst);
            PANIC_MOUSE_CAPTURE_INVALIDATED.store(false, Ordering::SeqCst);
            PANIC_INPUT_MODES_INVALIDATED.store(false, Ordering::SeqCst);
            set_owner_output(Weak::new());
            self.output_endpoint = None;
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
            #[cfg(unix)]
            {
                self.output_endpoint
                    .as_ref()
                    .and_then(|endpoint| endpoint.terminal_size().ok())
                    .unwrap_or((80, 24))
            }
            #[cfg(not(unix))]
            {
                crossterm::terminal::size().unwrap_or((80, 24))
            }
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

#[cfg(all(test, unix))]
mod key_release_switch_tests {
    //! #305: a failed screen-switch write after a successful kitty-flag pop must not strand
    //! the `REPORT_KEY_RELEASE` bit set — otherwise a later `restore`/`Drop`/retry pops a
    //! second time and removes another program's keyboard-stack entry. These drive the
    //! writer-seam [`TerminalSession::switch_screen_on`] directly (a real PTY cannot fail only
    //! the second of two back-to-back writes), asserting the global push bit and the bytes.
    use super::*;
    use std::io::{self, Write};

    // These mutate process-global key-release state (the push bit, the delivery flag, and the
    // keyboard-query record `expect_keyboard_reply`/`forget_keyboard_replies` use), so they share
    // the one cross-module lock the `tty_source` query tests also hold — never interleaving with
    // them (#305 review: P3).
    fn lock() -> std::sync::MutexGuard<'static, ()> {
        crate::tty_source::queries_lock()
    }

    fn reset_globals() {
        REPORTING_MODES.store(0, Ordering::SeqCst);
        KEY_RELEASES_ENDED.store(false, Ordering::SeqCst);
        crate::input::set_key_releases_delivered(false);
        // Clear any `outstanding` left by a re-push, so a later `tty_source` query test is not
        // perturbed (review: P3).
        crate::tty_source::forget_keyboard_replies();
    }

    /// A headless session reporting itself as key-release-live, with raw mode genuinely on,
    /// and the process-global push bit set — the state at the top of a screen switch.
    fn reporting_live_session() -> TerminalSession {
        reset_globals();
        let mut s = TerminalSession::headless(80, 24);
        s.raw_mode_enabled = true;
        s.key_release_reporting_enabled = true;
        REPORTING_MODES.fetch_or(REPORT_KEY_RELEASE, Ordering::SeqCst);
        crate::input::set_key_releases_delivered(true);
        // Precondition: reporting is live, so a switch follows it.
        assert!(key_releases_pushed());
        s
    }

    /// Fails the first write whose bytes contain `needle`; captures the rest.
    struct NeedleFail {
        buf: Vec<u8>,
        needle: u8,
        armed: bool,
    }
    impl Write for NeedleFail {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            if self.armed && b.contains(&self.needle) {
                self.armed = false;
                return Err(io::Error::other("injected write failure"));
            }
            self.buf.extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn switch_write_failure_clears_push_bit_and_averts_double_pop() {
        let _g = lock();
        let mut s = reporting_live_session();
        let mut buf: Vec<u8> = Vec::new();
        // Pop succeeds; the screen-switch write fails (EnterAlternateScreen).
        let r = s.switch_screen_on(&mut buf, |_o| Err(io::Error::other("switch failed")), true);
        assert!(r.is_err());
        assert!(
            !key_releases_pushed(),
            "#305: the push bit must be cleared after a failed switch"
        );
        assert_eq!(buf, b"\x1b[<u", "exactly one pop on the wire, no re-push");
        assert!(
            !take_key_releases_pushed(),
            "#305: restore/Drop must find no push to pop a second time"
        );
        assert!(
            !s.key_release_reporting_enabled,
            "reporting marked disabled"
        );
        assert!(!crate::input::key_releases_delivered(), "delivery stopped");
        assert!(
            s.raw_mode_enabled,
            "raw mode must stay ON so restore still disables it (the #305 nag)"
        );
        assert!(
            !s.alt_screen_active,
            "switch failed: not on the alternate screen"
        );
        reset_globals();
    }

    #[test]
    fn pop_failure_keeps_the_push_live_for_restore() {
        let _g = lock();
        let mut s = reporting_live_session();
        let mut w = NeedleFail {
            buf: Vec::new(),
            needle: b'<',
            armed: true,
        };
        let mut switched = false;
        let r = s.switch_screen_on(
            &mut w,
            |o| {
                switched = true;
                o.write_all(b"SW")
            },
            true,
        );
        assert!(r.is_err());
        assert!(
            !switched,
            "the switch must not run when the pop write fails"
        );
        // This writer accepted no pop bytes, so the push is still live in this controlled case.
        // The bit stays set so restore can attempt a pop. An I/O error in general does not
        // establish whether a terminal applied a partially written or flushed command.
        assert!(
            key_releases_pushed(),
            "#305 item 1: a failed pop leaves the still-live push claimable"
        );
        assert!(
            take_key_releases_pushed(),
            "restore pops the still-live push exactly once"
        );
        assert!(w.buf.is_empty(), "the failed pop wrote nothing");
        assert!(
            s.key_release_reporting_enabled,
            "a failed pop does not change reporting state"
        );
        assert!(s.raw_mode_enabled, "raw mode preserved");
        assert!(!s.alt_screen_active);
        reset_globals();
    }

    #[test]
    fn pop_flush_failure_leaves_ownership_uncertain() {
        struct FailFlush(Vec<u8>);
        impl Write for FailFlush {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.0.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::Error::other("injected flush failure"))
            }
        }
        let _g = lock();
        let mut s = reporting_live_session();
        let mut writer = FailFlush(Vec::new());
        let mut switched = false;
        let result = s.switch_screen_on(
            &mut writer,
            |_| {
                switched = true;
                Ok(())
            },
            true,
        );
        assert!(result.is_err());
        assert_eq!(
            writer.0, b"\x1b[<u",
            "the entire pop was accepted before flush failed"
        );
        assert!(!switched);
        assert!(key_releases_pushed(), "restore will attempt another pop");
        // This writer does not model a terminal: the bytes might have reached it. A retry can
        // therefore pop a foreign entry. The test pins bookkeeping, not successful restoration.
        reset_globals();
        s.raw_mode_enabled = false;
        s.key_release_reporting_enabled = false;
    }

    #[test]
    fn happy_path_relocates_the_push_and_restores_the_bit() {
        let _g = lock();
        let mut s = reporting_live_session();
        let mut buf: Vec<u8> = Vec::new();
        let r = s.switch_screen_on(&mut buf, |o| o.write_all(b"SW"), true);
        assert!(r.is_ok());
        assert!(
            key_releases_pushed(),
            "push re-set after a confirmed re-push"
        );
        assert_eq!(
            buf, b"\x1b[<uSW\x1b[>15u\x1b[?u\x1b[c",
            "pop, then switch, then re-push — unchanged happy-path byte order"
        );
        assert!(s.alt_screen_active);
        assert!(s.key_release_reporting_enabled);
        reset_globals();
    }

    #[test]
    fn repush_failure_keeps_the_bit_set_so_restore_pops_the_possibly_live_push() {
        let _g = lock();
        let mut s = reporting_live_session();
        // Pop ok, switch writes "SW" (ok), the re-push (`\x1b[>15u…`) fails on '>'.
        let mut w = NeedleFail {
            buf: Vec::new(),
            needle: b'>',
            armed: true,
        };
        let r = s.switch_screen_on(&mut w, |o| o.write_all(b"SW"), true);
        assert!(r.is_err());
        // A torn re-push may have placed our push on the new screen's stack, so the bit is set
        // before the write and restore can attempt a pop. If no push was applied, that attempt
        // may instead pop a foreign entry; the writer does not resolve terminal ownership.
        assert!(
            key_releases_pushed(),
            "#305: the re-push claim remains set after an error"
        );
        assert!(
            take_key_releases_pushed(),
            "restore can claim the bit for one pop attempt"
        );
        assert!(s.alt_screen_active, "the switch itself succeeded");
        assert!(
            s.key_release_reporting_enabled,
            "reporting stays on defensively, like a failed initial enable"
        );
        reset_globals();
    }

    #[test]
    fn failed_switch_leaves_raw_mode_on_for_restore() {
        let _g = lock();
        let mut s = reporting_live_session();
        let mut buf: Vec<u8> = Vec::new();
        let _ = s.switch_screen_on(&mut buf, |_o| Err(io::Error::other("switch failed")), true);
        // #305 nag: the loss marked reporting disabled, so the next reconciliation must NOT
        // enter the `enabled && !pushed` branch that assumes the panic hook already dropped
        // raw mode. The hook did not run here; raw mode is still on and restore must disable it.
        let _ = s.key_release_reporting_live();
        assert!(
            s.raw_mode_enabled,
            "#305 nag: a failed switch must not route into the branch that drops raw mode"
        );
        reset_globals();
    }
}

#[cfg(all(test, unix))]
mod mouse_capture_fault_tests {
    use super::*;

    const ON: &[u8] = b"\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1015h\x1b[?1006h";
    const OFF: &[u8] = b"\x1b[?1006l\x1b[?1015l\x1b[?1003l\x1b[?1002l\x1b[?1000l";

    struct RestoreMouseBit(bool);
    impl Drop for RestoreMouseBit {
        fn drop(&mut self) {
            if self.0 {
                REPORTING_MODES.fetch_or(REPORT_MOUSE, Ordering::SeqCst);
            } else {
                REPORTING_MODES.fetch_and(!REPORT_MOUSE, Ordering::SeqCst);
            }
        }
    }

    struct FailPartial(Vec<u8>);
    impl Write for FailPartial {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if !self.0.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "injected partial write",
                ));
            }
            let n = bytes.len().min(4);
            self.0.extend_from_slice(&bytes[..n]);
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct FailFlush(Vec<u8>);
    impl Write for FailFlush {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected flush failure",
            ))
        }
    }

    fn check_failed_enable(partial: bool) {
        let _serial = crate::tty_source::queries_lock();
        let _restore = RestoreMouseBit(REPORTING_MODES.load(Ordering::SeqCst) & REPORT_MOUSE != 0);
        let mut s = TerminalSession::headless(80, 24);
        REPORTING_MODES.fetch_and(!REPORT_MOUSE, Ordering::SeqCst);
        if partial {
            let mut out = FailPartial(Vec::new());
            assert!(s.set_mouse_capture_on(true, &mut out).is_err());
            assert!(!out.0.is_empty() && out.0.len() < ON.len());
        } else {
            let mut out = FailFlush(Vec::new());
            assert!(s.set_mouse_capture_on(true, &mut out).is_err());
            assert_eq!(out.0, ON);
        }
        assert!(s.mouse_capture_enabled, "cleanup remains owed");
        assert_ne!(REPORTING_MODES.load(Ordering::SeqCst) & REPORT_MOUSE, 0);
        let mut retry = Vec::new();
        s.set_mouse_capture_on(true, &mut retry).unwrap();
        assert_eq!(retry, ON, "retry must reissue the entire enable");
        let mut duplicate = Vec::new();
        s.set_mouse_capture_on(true, &mut duplicate).unwrap();
        assert!(duplicate.is_empty());
        let mut cleanup = Vec::new();
        s.set_mouse_capture_on(false, &mut cleanup).unwrap();
        assert_eq!(cleanup, OFF);
    }

    #[test]
    fn failed_enable_partial_write_retries_and_retains_cleanup() {
        check_failed_enable(true);
    }

    #[test]
    fn failed_enable_flush_retries_and_retains_cleanup() {
        check_failed_enable(false);
    }

    #[test]
    fn failed_disable_flush_makes_enable_retry_and_preserves_cleanup() {
        let _serial = crate::tty_source::queries_lock();
        let _restore = RestoreMouseBit(REPORTING_MODES.load(Ordering::SeqCst) & REPORT_MOUSE != 0);
        let mut s = TerminalSession::headless(80, 24);
        s.set_mouse_capture_on(true, &mut Vec::new()).unwrap();
        let mut fail = FailFlush(Vec::new());
        assert!(s.set_mouse_capture_on(false, &mut fail).is_err());
        assert_eq!(fail.0, OFF);
        assert!(s.mouse_capture_enabled, "failed disable retains cleanup");
        assert_ne!(REPORTING_MODES.load(Ordering::SeqCst) & REPORT_MOUSE, 0);
        let mut retry = Vec::new();
        s.set_mouse_capture_on(true, &mut retry).unwrap();
        assert_eq!(retry, ON, "failed disable made the prior enable uncertain");
        let mut cleanup = Vec::new();
        s.set_mouse_capture_on(false, &mut cleanup).unwrap();
        assert_eq!(cleanup, OFF);
    }

    #[test]
    fn failed_enable_can_be_cleaned_up_without_an_enable_retry() {
        let _serial = crate::tty_source::queries_lock();
        let _restore = RestoreMouseBit(REPORTING_MODES.load(Ordering::SeqCst) & REPORT_MOUSE != 0);
        for partial in [true, false] {
            let mut s = TerminalSession::headless(80, 24);
            if partial {
                assert!(s
                    .set_mouse_capture_on(true, &mut FailPartial(Vec::new()))
                    .is_err());
            } else {
                assert!(s
                    .set_mouse_capture_on(true, &mut FailFlush(Vec::new()))
                    .is_err());
            }
            let mut cleanup = Vec::new();
            s.set_mouse_capture_on(false, &mut cleanup).unwrap();
            assert_eq!(cleanup, OFF);
            assert_eq!(REPORTING_MODES.load(Ordering::SeqCst) & REPORT_MOUSE, 0);
        }
    }

    #[test]
    fn failed_disable_retries_disable_and_keeps_cleanup_until_success() {
        let _serial = crate::tty_source::queries_lock();
        let _restore = RestoreMouseBit(REPORTING_MODES.load(Ordering::SeqCst) & REPORT_MOUSE != 0);
        let mut s = TerminalSession::headless(80, 24);
        s.set_mouse_capture_on(true, &mut Vec::new()).unwrap();
        assert!(s
            .set_mouse_capture_on(false, &mut FailFlush(Vec::new()))
            .is_err());
        assert!(
            !s.mouse_capture_enabled(),
            "failed disable is not confirmed"
        );
        assert_ne!(REPORTING_MODES.load(Ordering::SeqCst) & REPORT_MOUSE, 0);
        let mut cleanup = Vec::new();
        s.set_mouse_capture_on(false, &mut cleanup).unwrap();
        assert_eq!(cleanup, OFF);
        assert_eq!(REPORTING_MODES.load(Ordering::SeqCst) & REPORT_MOUSE, 0);
    }

    #[test]
    fn restore_clears_mouse_panic_notice_before_next_lease() {
        const CHILD: &str = "GIBSON_MOUSE_NOTICE_CHILD";
        if std::env::var_os(CHILD).is_some() {
            // Isolated process: exercise the real restore path without disturbing the test runner's
            // process-global lease or terminal. Its stdout is redirected by the parent below.
            let mut owner = TerminalSession::headless(80, 24);
            owner.headless = false;
            owner.owns_lease = true;
            owner.output_endpoint = Some(SessionEndpoint::capture_stdout().unwrap());
            TERMINAL_LEASE.store(LEASE_OWNED, Ordering::SeqCst);
            set_owner_thread(Some(std::thread::current().id()));
            PANIC_MOUSE_CAPTURE_INVALIDATED.store(true, Ordering::SeqCst);
            owner.restore().unwrap();
            assert!(!PANIC_MOUSE_CAPTURE_INVALIDATED.load(Ordering::SeqCst));
            assert_eq!(TerminalSession::lease_state(), TerminalLease::Available);
            let mut next = TerminalSession::headless(80, 24);
            next.owns_lease = true;
            next.mouse_capture_confirmed = true;
            assert!(
                next.mouse_capture_enabled(),
                "old notice affected the next owner"
            );
            next.owns_lease = false;
            return;
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "session::mouse_capture_fault_tests::restore_clears_mouse_panic_notice_before_next_lease",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[cfg(all(test, unix))]
mod panic_input_fault_tests {
    use super::*;

    struct NoticeGuard(bool);
    impl Drop for NoticeGuard {
        fn drop(&mut self) {
            PANIC_INPUT_MODES_INVALIDATED.store(self.0, Ordering::SeqCst);
        }
    }

    struct FailFlush(Vec<u8>);
    impl Write for FailFlush {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected flush failure",
            ))
        }
    }

    struct PartialThenFail(Vec<u8>);
    impl Write for PartialThenFail {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.0.is_empty() {
                let n = bytes.len().min(3);
                self.0.extend_from_slice(&bytes[..n]);
                Ok(n)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "injected partial write",
                ))
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn partial_paste_enable_write_retains_cleanup_and_retries() {
        let mut session = TerminalSession::headless(80, 24);
        let mut fail = PartialThenFail(Vec::new());
        assert!(session.enable_bracketed_paste_on(&mut fail).is_err());
        assert_eq!(fail.0, b"\x1b[?");
        assert!(session.bracketed_paste_enabled);
        let mut retry = Vec::new();
        session.enable_bracketed_paste_on(&mut retry).unwrap();
        assert_eq!(retry, b"\x1b[?2004h");
        let mut cleanup = Vec::new();
        session.disable_bracketed_paste_on(&mut cleanup).unwrap();
        assert_eq!(cleanup, b"\x1b[?2004l");
    }

    #[test]
    fn panic_invalidation_keeps_uncertain_paste_cleanup_when_raw_reentry_fails() {
        let _serial = crate::tty_source::queries_lock();
        let _notice = NoticeGuard(PANIC_INPUT_MODES_INVALIDATED.load(Ordering::SeqCst));
        let mut session = TerminalSession::headless(80, 24);
        session.owns_lease = true; // only exercise the reconciliation; no real terminal lease
        session.raw_mode_enabled = true;
        session.bracketed_paste_enabled = true;
        // Model a panic-hook disable that may have failed. No new raw/paste request succeeds.
        PANIC_INPUT_MODES_INVALIDATED.store(true, Ordering::SeqCst);
        session.reconcile_panic_input_modes();
        assert!(!session.raw_mode_enabled);
        assert!(
            session.bracketed_paste_enabled,
            "cleanup ownership cannot be forgotten"
        );
        let mut cleanup = Vec::new();
        session.disable_bracketed_paste_on(&mut cleanup).unwrap();
        assert_eq!(cleanup, b"\x1b[?2004l");
    }

    #[test]
    fn failed_paste_enable_flush_retains_cleanup_and_retries() {
        let mut session = TerminalSession::headless(80, 24);
        let mut fail = FailFlush(Vec::new());
        assert!(session.enable_bracketed_paste_on(&mut fail).is_err());
        assert_eq!(fail.0, b"\x1b[?2004h");
        assert!(
            session.bracketed_paste_enabled,
            "the full enable might have arrived"
        );
        let mut retry = Vec::new();
        session.enable_bracketed_paste_on(&mut retry).unwrap();
        assert_eq!(retry, b"\x1b[?2004h", "retry must request paste again");
        let mut duplicate = Vec::new();
        session.enable_bracketed_paste_on(&mut duplicate).unwrap();
        assert!(duplicate.is_empty());
        let mut cleanup = Vec::new();
        session.disable_bracketed_paste_on(&mut cleanup).unwrap();
        assert_eq!(cleanup, b"\x1b[?2004l");
    }
}

#[cfg(all(test, unix))]
mod focus_retry_tests {
    use super::*;

    // Restore only this bit even if an assertion panics. All tests in this module share the
    // query lock with the keyboard transition tests that reset REPORTING_MODES.
    struct FocusBitGuard(u8);
    impl Drop for FocusBitGuard {
        fn drop(&mut self) {
            REPORTING_MODES.fetch_and(!REPORT_FOCUS, Ordering::SeqCst);
            REPORTING_MODES.fetch_or(self.0 & REPORT_FOCUS, Ordering::SeqCst);
        }
    }

    struct FailWrite;
    impl Write for FailWrite {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected write failure",
            ))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct FailFlush(Vec<u8>);

    impl Write for FailFlush {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected flush failure",
            ))
        }
    }

    #[test]
    fn failed_focus_enable_must_retry_and_retain_cleanup_ownership() {
        let _serial = crate::tty_source::queries_lock();
        let initial = REPORTING_MODES.load(Ordering::SeqCst);
        let _restore_focus_bit = FocusBitGuard(initial);
        assert_eq!(
            initial & REPORT_FOCUS,
            0,
            "focus ownership leaked into test"
        );
        let mut session = TerminalSession::headless(80, 24);

        // A write can fail before sending a complete sequence; cleanup still owns the mode.
        assert!(session
            .set_focus_reporting_with_writer(true, &mut FailWrite)
            .is_err());
        assert!(session.focus_reporting_enabled());
        assert_ne!(REPORTING_MODES.load(Ordering::SeqCst) & REPORT_FOCUS, 0);
        let mut cleanup = Vec::new();
        session
            .set_focus_reporting_with_writer(false, &mut cleanup)
            .unwrap();
        assert_eq!(cleanup, b"\x1b[?1004l");
        assert_eq!(REPORTING_MODES.load(Ordering::SeqCst) & REPORT_FOCUS, 0);

        let mut failing = FailFlush(Vec::new());
        assert!(session
            .set_focus_reporting_with_writer(true, &mut failing)
            .is_err());
        assert_eq!(failing.0, b"\x1b[?1004h");
        assert!(
            session.focus_reporting_enabled(),
            "failed write still needs cleanup"
        );
        assert_ne!(REPORTING_MODES.load(Ordering::SeqCst) & REPORT_FOCUS, 0);

        let mut retry = Vec::new();
        session
            .set_focus_reporting_with_writer(true, &mut retry)
            .unwrap();
        assert_eq!(
            retry, b"\x1b[?1004h",
            "retry must actually request reporting"
        );

        let mut duplicate = Vec::new();
        session
            .set_focus_reporting_with_writer(true, &mut duplicate)
            .unwrap();
        assert!(duplicate.is_empty(), "confirmed enable remains idempotent");

        // A failed disable may have reached the terminal; another enable must be sent.
        let mut failing_disable = FailFlush(Vec::new());
        assert!(session
            .set_focus_reporting_with_writer(false, &mut failing_disable)
            .is_err());
        assert_eq!(failing_disable.0, b"\x1b[?1004l");
        assert!(session.focus_reporting_enabled());
        assert_ne!(REPORTING_MODES.load(Ordering::SeqCst) & REPORT_FOCUS, 0);
        let mut reenable = Vec::new();
        session
            .set_focus_reporting_with_writer(true, &mut reenable)
            .unwrap();
        assert_eq!(reenable, b"\x1b[?1004h");

        let mut disable = Vec::new();
        session
            .set_focus_reporting_with_writer(false, &mut disable)
            .unwrap();
        assert_eq!(disable, b"\x1b[?1004l");
        assert!(!session.focus_reporting_enabled());
        assert_eq!(REPORTING_MODES.load(Ordering::SeqCst) & REPORT_FOCUS, 0);
        let mut duplicate_disable = Vec::new();
        session
            .set_focus_reporting_with_writer(false, &mut duplicate_disable)
            .unwrap();
        assert!(duplicate_disable.is_empty());
    }
}
