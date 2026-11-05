use crossterm::event::{
    self, Event as CtEvent, KeyCode as CtKeyCode, KeyModifiers as CtKeyModifiers,
    MouseButton as CtMouseButton, MouseEvent as CtMouseEvent, MouseEventKind as CtMouseKind,
};
use std::borrow::Cow;
use std::io;
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct KeyModifiers: u8 {
        const SHIFT = 0b0000_0001;
        const CONTROL = 0b0000_0010;
        const ALT = 0b0000_0100;
    }
}

impl From<CtKeyModifiers> for KeyModifiers {
    fn from(m: CtKeyModifiers) -> Self {
        let mut flags = KeyModifiers::empty();
        if m.contains(CtKeyModifiers::SHIFT) {
            flags |= KeyModifiers::SHIFT;
        }
        if m.contains(CtKeyModifiers::CONTROL) {
            flags |= KeyModifiers::CONTROL;
        }
        if m.contains(CtKeyModifiers::ALT) {
            flags |= KeyModifiers::ALT;
        }
        flags
    }
}

/// Logical key code.
///
/// `#[non_exhaustive]`: the terminal key repertoire grows (function keys, Insert,
/// media keys, …), so downstream `match`es must include a `_` arm and adding a key
/// is a non-breaking `0.1.z` change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyCode {
    Char(char),
    Enter,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Backspace,
    Delete,
    Esc,
    Tab,
    BackTab,
}

/// A structured keyboard event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyEvent {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    pub fn char(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::empty(),
        }
    }
}

/// A mouse button.
///
/// `#[non_exhaustive]`: terminals may report further buttons, so downstream
/// `match`es must include a `_` arm and adding one is a non-breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl From<CtMouseButton> for MouseButton {
    fn from(b: CtMouseButton) -> Self {
        match b {
            CtMouseButton::Left => MouseButton::Left,
            CtMouseButton::Right => MouseButton::Right,
            CtMouseButton::Middle => MouseButton::Middle,
        }
    }
}

/// What a [`MouseEvent`] did.
///
/// `#[non_exhaustive]`: new kinds may appear, so downstream `match`es must include a
/// `_` arm and adding one is a non-breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MouseKind {
    /// A button was pressed.
    Down(MouseButton),
    /// A button was released.
    Up(MouseButton),
    /// The pointer moved with a button held (a drag).
    Drag(MouseButton),
    /// The pointer moved with no button held.
    Moved,
    /// The scroll wheel moved up.
    ScrollUp,
    /// The scroll wheel moved down.
    ScrollDown,
    /// The scroll wheel moved left.
    ScrollLeft,
    /// The scroll wheel moved right.
    ScrollRight,
}

/// A structured mouse event at a terminal cell.
///
/// `column`/`row` are zero-based terminal cells (same basis as the cells a
/// `gibson::hit_test` hit-tester expects), so a mouse event drops straight into
/// spatial routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseEvent {
    /// What happened (press/release/drag/move/scroll).
    pub kind: MouseKind,
    /// Zero-based column (terminal cell).
    pub column: u16,
    /// Zero-based row (terminal cell).
    pub row: u16,
    /// Modifier keys held during the event.
    pub modifiers: KeyModifiers,
}

/// A terminal focus transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusChange {
    /// The terminal window/tab gained focus.
    Gained,
    /// The terminal window/tab lost focus.
    Lost,
}

/// High-level input event.
///
/// `#[non_exhaustive]`: new event kinds are expected, so downstream `match`es must
/// include a `_` arm and adding an event is a non-breaking `0.1.z` change.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Event {
    Key(KeyEvent),
    Paste(String),
    Resize(u16, u16),
    /// A mouse event at a terminal cell. Only produced when mouse capture is enabled
    /// via [`set_mouse_capture`] — **off by default**, so the terminal's native text
    /// selection keeps working (issue #126).
    Mouse(MouseEvent),
    /// A terminal focus change. Only produced when focus reporting is enabled via
    /// [`set_focus_reporting`] — **off by default** (issue #126).
    Focus(FocusChange),
    /// Reserved animation/timer tick. It is **not** produced by [`poll_event`]:
    /// the core loop is input-driven. A consumer paces animation itself with
    /// `Context::animation_interval` + `Context::render_if_due` (see the
    /// canonical-loop example on `Context::run_once`), or uses the
    /// batteries-included `gibson::ui::App` loop, which delivers per-iteration
    /// ticks as `AppEvent::Tick`. This variant exists so an application that
    /// synthesizes its own ticks can route them through the same `Event`
    /// channel. (Issue #48 E-07)
    Tick,
}

/// Polls for an event with a timeout.
///
/// On a Unix controlling terminal this is **hangup-aware**: if the terminal hangs
/// up (SIGHUP, master closed) it returns `Err(ErrorKind::BrokenPipe)` instead of
/// spinning forever (issue #79). crossterm's backend busy-loops at ~66% CPU on a
/// hung-up tty because its internal read loop never honours the timeout once the fd
/// reports `POLLHUP`; we therefore do the waiting ourselves on stdin and never hand
/// a possibly-hung fd to crossterm with a blocking timeout.
///
/// The dispatch keys off input-source **identity**, not a fresh liveness probe. Once
/// stdin has been seen as a controlling terminal we keep the hangup-aware path even
/// after it hangs up — at which point `isatty` reports false. Re-probing `isatty`
/// per call (as this once did) let a hangup that landed *between* polls demote the
/// very next poll to crossterm's spin-on-a-hung-fd path, the exact #79 failure it is
/// meant to prevent. A source that was never a terminal (redirected input, a pipe)
/// never latches and keeps the portable path, so a closed pipe is not misread as a
/// hung terminal. See `input_source_is_tty`.
pub fn poll_event(timeout: Duration) -> io::Result<Option<Event>> {
    #[cfg(unix)]
    {
        if input_source_is_tty() {
            return poll_event_unix_tty(timeout);
        }
    }
    poll_event_crossterm(timeout)
}

/// Enables or disables mouse-event reporting. **Off by default**, so the terminal's
/// native text selection and copy/paste keep working; enable it only while an
/// instrument needs pointer input, and disable it again afterward (issue #126). When
/// enabled, [`poll_event`] yields [`Event::Mouse`]. Writes the control sequence to
/// stdout; a write error propagates.
///
/// This is an **unmanaged** raw primitive: an enable it performs is not unwound by
/// any session, so it leaks on an early return or panic. Prefer the session-owned
/// `TerminalSession::set_mouse_capture`, which disables the mode on restore, `Drop`
/// and panic.
pub fn set_mouse_capture(enable: bool) -> io::Result<()> {
    use crossterm::ExecutableCommand;
    if enable {
        io::stdout().execute(event::EnableMouseCapture)?;
    } else {
        io::stdout().execute(event::DisableMouseCapture)?;
    }
    Ok(())
}

/// Enables or disables focus-change reporting. **Off by default** (issue #126). When
/// enabled, [`poll_event`] yields [`Event::Focus`]. Writes the control sequence to
/// stdout; a write error propagates.
///
/// An **unmanaged** raw primitive like `set_mouse_capture`; prefer the session-owned
/// `TerminalSession::set_focus_reporting`, which is unwound on restore, `Drop` and
/// panic.
pub fn set_focus_reporting(enable: bool) -> io::Result<()> {
    use crossterm::ExecutableCommand;
    if enable {
        io::stdout().execute(event::EnableFocusChange)?;
    } else {
        io::stdout().execute(event::DisableFocusChange)?;
    }
    Ok(())
}

/// The portable crossterm wait+decode. Used off-Unix, or when stdin is not a tty
/// (crossterm then reads `/dev/tty`, a fd we do not observe for hangup).
fn poll_event_crossterm(timeout: Duration) -> io::Result<Option<Event>> {
    if event::poll(timeout)? {
        Ok(decode_event(event::read()?))
    } else {
        Ok(None)
    }
}

/// Translates a crossterm event into our [`Event`]; `None` for kinds we do not model
/// (unmapped key codes, mouse, focus, …).
fn decode_event(ev: CtEvent) -> Option<Event> {
    match ev {
        CtEvent::Key(k) => {
            let code = match k.code {
                CtKeyCode::Char(c) => KeyCode::Char(c),
                CtKeyCode::Enter => KeyCode::Enter,
                CtKeyCode::Left => KeyCode::Left,
                CtKeyCode::Right => KeyCode::Right,
                CtKeyCode::Up => KeyCode::Up,
                CtKeyCode::Down => KeyCode::Down,
                CtKeyCode::Home => KeyCode::Home,
                CtKeyCode::End => KeyCode::End,
                CtKeyCode::PageUp => KeyCode::PageUp,
                CtKeyCode::PageDown => KeyCode::PageDown,
                CtKeyCode::Backspace => KeyCode::Backspace,
                CtKeyCode::Delete => KeyCode::Delete,
                CtKeyCode::Esc => KeyCode::Esc,
                CtKeyCode::Tab => KeyCode::Tab,
                CtKeyCode::BackTab => KeyCode::BackTab,
                _ => return None,
            };
            Some(Event::Key(KeyEvent {
                code,
                modifiers: k.modifiers.into(),
            }))
        }
        CtEvent::Paste(s) => Some(Event::Paste(s)),
        CtEvent::Resize(cols, rows) => Some(Event::Resize(cols, rows)),
        CtEvent::Mouse(m) => decode_mouse(m),
        CtEvent::FocusGained => Some(Event::Focus(FocusChange::Gained)),
        CtEvent::FocusLost => Some(Event::Focus(FocusChange::Lost)),
    }
}

/// Translates a crossterm mouse event into our [`MouseEvent`]. Always produces an
/// event: crossterm mouse kinds map one-to-one onto [`MouseKind`].
fn decode_mouse(m: CtMouseEvent) -> Option<Event> {
    let kind = match m.kind {
        CtMouseKind::Down(b) => MouseKind::Down(b.into()),
        CtMouseKind::Up(b) => MouseKind::Up(b.into()),
        CtMouseKind::Drag(b) => MouseKind::Drag(b.into()),
        CtMouseKind::Moved => MouseKind::Moved,
        CtMouseKind::ScrollUp => MouseKind::ScrollUp,
        CtMouseKind::ScrollDown => MouseKind::ScrollDown,
        CtMouseKind::ScrollLeft => MouseKind::ScrollLeft,
        CtMouseKind::ScrollRight => MouseKind::ScrollRight,
    };
    Some(Event::Mouse(MouseEvent {
        kind,
        column: m.column,
        row: m.row,
        modifiers: m.modifiers.into(),
    }))
}

/// Hangup-aware poll loop for a Unix controlling terminal (issue #79).
///
/// We never block inside crossterm on a possibly-hung fd. We wait on stdin
/// ourselves in short slices, checking `POLLHUP`/`POLLERR` each slice, and only ask
/// crossterm to decode with a zero timeout once stdin is readable. A hangup is
/// surfaced within one slice; a readable-but-undecodable fd (the upstream crossterm
/// "stranded input" stall, issue #15) is paced rather than busy-looped, preserving
/// the stock stall behaviour without burning CPU.
#[cfg(unix)]
fn poll_event_unix_tty(timeout: Duration) -> io::Result<Option<Event>> {
    use std::time::Instant;

    const SLICE: Duration = Duration::from_millis(25);
    let hung_up = || io::Error::new(io::ErrorKind::BrokenPipe, "terminal hung up (SIGHUP)");
    let deadline = Instant::now().checked_add(timeout);

    loop {
        if tty_hung_up(libc::STDIN_FILENO)? {
            return Err(hung_up());
        }
        // Decode anything crossterm already holds, without letting it block on the fd.
        if event::poll(Duration::ZERO)? {
            return Ok(decode_event(event::read()?));
        }
        let remaining = match deadline {
            Some(d) => d.saturating_duration_since(Instant::now()),
            None => Duration::MAX,
        };
        if remaining.is_zero() {
            return Ok(None);
        }
        let slice = remaining.min(SLICE);
        match wait_stdin_readable(slice)? {
            StdinWait::HungUp => return Err(hung_up()),
            StdinWait::TimedOut | StdinWait::Interrupted => continue,
            StdinWait::Readable => {
                // Readable yet crossterm decoded nothing above: the edge was already
                // consumed (issue #15, an upstream crossterm bug). POLLIN stays level-
                // set, so looping straight back would busy-spin; sleep one slice so we
                // still re-check for a hangup and pick the byte up on the next edge.
                std::thread::sleep(slice);
            }
        }
    }
}

/// Whether stdin is a real terminal *right now* — the liveness probe. After a
/// hangup this reports false even though the session's terminal identity is
/// unchanged, which is why [`poll_event`] dispatches on the latched identity
/// ([`input_source_is_tty`]) rather than on this probe directly.
#[cfg(unix)]
fn stdin_is_tty() -> bool {
    // SAFETY: `isatty` is always safe to call on any fd.
    unsafe { libc::isatty(libc::STDIN_FILENO) == 1 }
}

/// Latched input-source identity: `true` once stdin has been observed as a live
/// controlling terminal. Terminal *identity* is stable across a *liveness*
/// transition — a tty that hangs up is still the terminal we acquired, not a newly
/// discovered non-tty backend — so once latched we never demote off the hangup-aware
/// path. See [`poll_event`] and [`resolve_input_source`].
#[cfg(unix)]
static TTY_SOURCE_LATCHED: AtomicBool = AtomicBool::new(false);

/// Records that a live controlling terminal was acquired for input (raw mode entered
/// on a tty). Idempotent; called from the terminal-session boundary so identity is
/// established at acquisition, not merely on the first poll.
#[cfg(unix)]
pub(crate) fn note_tty_source_acquired() {
    TTY_SOURCE_LATCHED.store(true, Ordering::Relaxed);
}

/// Forgets any latched terminal identity, so a later acquisition re-establishes it.
/// Called when a session relinquishes the terminal (restore), keeping the latch
/// scoped to an interactive lifetime rather than the whole process.
#[cfg(unix)]
pub(crate) fn clear_tty_source() {
    TTY_SOURCE_LATCHED.store(false, Ordering::Relaxed);
}

/// No-op terminal-identity hooks off Unix, where the hangup-aware path and its latch
/// do not exist; the terminal session calls these unconditionally.
#[cfg(not(unix))]
pub(crate) fn note_tty_source_acquired() {}
/// See [`note_tty_source_acquired`].
#[cfg(not(unix))]
pub(crate) fn clear_tty_source() {}

/// The pure dispatch decision for [`poll_event`], factored out so the
/// identity-vs-liveness law is unit-testable without a real descriptor.
///
/// Returns `(use_hangup_aware_path, latched_after)`. A latched identity keeps the
/// hangup-aware path and does **not** consult the liveness probe (the closure is
/// never called). An unlatched source consults the probe once and latches iff it is
/// currently a terminal.
#[cfg(unix)]
fn resolve_input_source(latched: bool, probe_is_tty: impl FnOnce() -> bool) -> (bool, bool) {
    if latched {
        (true, true)
    } else {
        let is_tty = probe_is_tty();
        (is_tty, is_tty)
    }
}

/// Whether [`poll_event`] should use the hangup-aware Unix path, latching tty
/// identity on first sight so a later between-polls hangup cannot demote it.
#[cfg(unix)]
fn input_source_is_tty() -> bool {
    let latched = TTY_SOURCE_LATCHED.load(Ordering::Relaxed);
    let (use_unix, new_latch) = resolve_input_source(latched, stdin_is_tty);
    if new_latch != latched {
        TTY_SOURCE_LATCHED.store(new_latch, Ordering::Relaxed);
    }
    use_unix
}

#[cfg(all(test, unix))]
mod identity_tests {
    use super::resolve_input_source;
    use std::cell::Cell;

    #[test]
    fn latched_tty_stays_on_hangup_path_when_liveness_probe_is_now_false() {
        // The between-polls hangup: identity established, liveness now reports false.
        let (use_unix, latched) = resolve_input_source(true, || false);
        assert!(use_unix, "a latched tty must keep the hangup-aware path");
        assert!(
            latched,
            "identity must remain latched across the liveness drop"
        );
    }

    #[test]
    fn latched_identity_does_not_reprobe_liveness() {
        let probed = Cell::new(false);
        let _ = resolve_input_source(true, || {
            probed.set(true);
            false
        });
        assert!(
            !probed.get(),
            "a latched identity must not consult the probe"
        );
    }

    #[test]
    fn first_live_tty_latches_and_uses_the_hangup_path() {
        let (use_unix, latched) = resolve_input_source(false, || true);
        assert!(use_unix);
        assert!(latched, "a freshly observed tty latches its identity");
    }

    #[test]
    fn a_source_that_was_never_a_tty_stays_portable_and_never_latches() {
        // A redirected pipe is not reclassified as a hung terminal.
        let (use_unix, latched) = resolve_input_source(false, || false);
        assert!(!use_unix);
        assert!(!latched, "a non-tty source must never latch tty identity");
    }
}

/// `true` if `fd` reports a hangup/error condition right now (non-blocking).
#[cfg(unix)]
fn tty_hung_up(fd: i32) -> io::Result<bool> {
    // `events: 0` still has the kernel report POLLHUP/POLLERR/POLLNVAL in `revents`.
    let mut pfd = libc::pollfd {
        fd,
        events: 0,
        revents: 0,
    };
    // SAFETY: one valid pollfd; timeout 0 is non-blocking.
    let rc = unsafe { libc::poll(&mut pfd as *mut libc::pollfd, 1, 0) };
    if rc < 0 {
        let e = io::Error::last_os_error();
        return if e.kind() == io::ErrorKind::Interrupted {
            Ok(false)
        } else {
            Err(e)
        };
    }
    Ok(pfd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0)
}

#[cfg(unix)]
enum StdinWait {
    Readable,
    TimedOut,
    HungUp,
    Interrupted,
}

/// Waits up to `slice` for stdin to become readable, reporting a hangup distinctly.
#[cfg(unix)]
fn wait_stdin_readable(slice: Duration) -> io::Result<StdinWait> {
    let ms = slice.as_millis().min(i32::MAX as u128) as i32;
    let mut pfd = libc::pollfd {
        fd: libc::STDIN_FILENO,
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: one valid pollfd.
    let rc = unsafe { libc::poll(&mut pfd as *mut libc::pollfd, 1, ms) };
    if rc < 0 {
        let e = io::Error::last_os_error();
        return if e.kind() == io::ErrorKind::Interrupted {
            Ok(StdinWait::Interrupted)
        } else {
            Err(e)
        };
    }
    if rc == 0 {
        return Ok(StdinWait::TimedOut);
    }
    if pfd.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
        return Ok(StdinWait::HungUp);
    }
    if pfd.revents & libc::POLLIN != 0 {
        return Ok(StdinWait::Readable);
    }
    Ok(StdinWait::TimedOut)
}

/// Stateful grapheme-aware single-line text input buffer.
///
/// ## Invariant
///
/// After **every** public mutation:
///
/// ```text
/// cursor_grapheme <= text.graphemes(true).count()
/// ```
///
/// Mutations are expressed as byte-range edits and the cursor index is then
/// **re-derived from the segmentation of the complete resulting string**. This
/// matters because grapheme boundaries can merge across an insertion boundary
/// (for example inserting `U+0301 COMBINING ACUTE ACCENT` after `e` produces a
/// single `e◌́` cluster). The old "insert then add the inserted grapheme count"
/// strategy can leave the cursor pointing past the end of the buffer and panic
/// on the next backspace.
///
/// ## Single-line, control-free insertion
///
/// The editor model is single-line and its buffer holds text, never terminal
/// controls. Every insertion path — [`handle_event`](Self::handle_event) (typed
/// keys and bracketed paste), [`insert_str`](Self::insert_str),
/// [`insert_char`](Self::insert_char) and [`with_text`](Self::with_text) —
/// applies [`normalize_paste`](Self::normalize_paste):
///
/// * each run of line breaks — CR, LF, VT, FF, NEL, U+2028 LINE SEPARATOR and
///   U+2029 PARAGRAPH SEPARATOR — becomes **one space**, so multiline content is
///   flattened rather than rejected and words never fuse;
/// * each TAB becomes **one space** (a terminal would otherwise advance it to
///   the next tab stop, which no cell model can predict);
/// * every other control character — C0, DEL and C1 (`char::is_control`),
///   including the ESC that introduces CSI/OSC/DCS sequences — is **dropped**,
///   matching the guard [`Surface::print_str`](crate::surface::Surface::print_str)
///   applies to ordinary text.
///
/// ## Width contract (issue #252)
///
/// One rule assigns every grapheme cluster its terminal cells, and the cursor
/// column, [`total_display_width`](Self::total_display_width), scrolling, the
/// painter and the emitted bytes all use it:
///
/// * a grapheme of display width 1 or 2 (`unicode-width`) is painted
///   **verbatim** in one cell of that width;
/// * a grapheme **wider than 2** — a Devanagari conjunct with a spacing vowel
///   sign (`क्षि`, 3), a run of Hangul leading jamo — is painted **codepoint by
///   codepoint** across consecutive cells (each base codepoint with the
///   zero-width marks that follow it). The emitted bytes are unchanged and the
///   cells advance exactly as `unicode-width` and a per-codepoint terminal
///   measure them, so a long cluster can never overrun the input's cells;
/// * otherwise it is painted as **one `U+FFFD` cell**: a grapheme holding a
///   control character or a line/paragraph separator (raw content written into
///   the public `text` field, or given to
///   [`Node::text_input`](crate::node::Node::text_input) / the C ABI), a
///   zero-width grapheme standing alone (a combining mark with no base, ZWSP,
///   BOM, a bidi format character, a lone variation selector), or a codepoint
///   no cell can hold (wider than 2). Such input is kept rather than rejected —
///   typing a base before a lone combining mark legitimately merges the two —
///   and stays a visible cursor stop that Backspace removes.
///
/// So no C0, DEL or C1 control from content can reach the terminal, and the
/// editor's columns match `unicode-width`'s per-codepoint model. Terminals that
/// size clusters differently (emoji ZWJ / VS16 sequences, ambiguous-wide
/// settings) are a documented limit of the whole cell model, not of the editor.
#[derive(Debug, Clone, Default)]
pub struct TextInputState {
    pub text: String,
    pub cursor_grapheme: usize,
    pub scroll_offset: usize,
}

impl TextInputState {
    pub fn new() -> Self {
        Self::default()
    }

    /// A state holding `text` (normalized like any insertion — see the type
    /// docs) with the cursor at its end.
    pub fn with_text(text: impl Into<String>) -> Self {
        let mut t = text.into();
        if let Cow::Owned(clean) = single_line(&t) {
            t = clean;
        }
        let count = t.graphemes(true).count();
        Self {
            text: t,
            cursor_grapheme: count,
            scroll_offset: 0,
        }
    }

    pub fn grapheme_count(&self) -> usize {
        self.text.graphemes(true).count()
    }

    /// Byte offset of the cursor. Always a valid UTF-8 char boundary.
    fn cursor_byte_pos(&self) -> usize {
        self.text
            .grapheme_indices(true)
            .nth(self.cursor_grapheme)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len())
    }

    /// Re-derives `cursor_grapheme` from a byte offset in the current string.
    fn sync_cursor_from_byte(&mut self, byte: usize) {
        let byte = byte.min(self.text.len());
        debug_assert!(self.text.is_char_boundary(byte));
        self.cursor_grapheme = self.text[..byte].graphemes(true).count();
    }

    /// Clamps the cursor into range. Cheap safety net for deserialized/foreign state.
    pub fn normalize(&mut self) {
        let count = self.grapheme_count();
        if self.cursor_grapheme > count {
            self.cursor_grapheme = count;
        }
    }

    /// Normalizes text to the editor's single-line, control-free form: line-break
    /// runs and TABs become spaces, every other control character is dropped
    /// (see the type-level docs). Applied by every insertion path.
    pub fn normalize_paste(s: &str) -> String {
        single_line(s).into_owned()
    }

    /// Inserts a string at the current grapheme position, normalized like every
    /// insertion (see the type-level docs).
    pub fn insert_str(&mut self, s: &str) {
        let s = single_line(s);
        if s.is_empty() {
            return;
        }
        self.normalize();
        let byte_pos = self.cursor_byte_pos();
        self.text.insert_str(byte_pos, &s);
        // Re-derive from the *complete* resulting string so boundary merges are respected.
        self.sync_cursor_from_byte(byte_pos + s.len());
        self.normalize();
    }

    /// Inserts a single char at the current grapheme position.
    pub fn insert_char(&mut self, c: char) {
        let mut buf = [0u8; 4];
        let s = c.encode_utf8(&mut buf);
        self.insert_str(s);
    }

    /// Deletes the grapheme cluster preceding the cursor (Backspace).
    pub fn backspace(&mut self) -> bool {
        self.normalize();
        if self.cursor_grapheme == 0 {
            return false;
        }

        let cur_byte = self.cursor_byte_pos();
        let prev_start = self.text[..cur_byte]
            .grapheme_indices(true)
            .next_back()
            .map(|(i, _)| i);

        match prev_start {
            Some(start) => {
                self.text.replace_range(start..cur_byte, "");
                self.sync_cursor_from_byte(start);
                true
            }
            None => false,
        }
    }

    /// Deletes the grapheme cluster at the cursor (Delete).
    pub fn delete(&mut self) -> bool {
        self.normalize();
        let cur_byte = self.cursor_byte_pos();
        if cur_byte >= self.text.len() {
            return false;
        }

        let next_end = self.text[cur_byte..]
            .grapheme_indices(true)
            .nth(1)
            .map(|(i, _)| cur_byte + i)
            .unwrap_or(self.text.len());

        self.text.replace_range(cur_byte..next_end, "");
        self.sync_cursor_from_byte(cur_byte);
        true
    }

    pub fn move_left(&mut self) -> bool {
        self.normalize();
        if self.cursor_grapheme > 0 {
            self.cursor_grapheme -= 1;
            true
        } else {
            false
        }
    }

    pub fn move_right(&mut self) -> bool {
        self.normalize();
        if self.cursor_grapheme < self.grapheme_count() {
            self.cursor_grapheme += 1;
            true
        } else {
            false
        }
    }

    pub fn move_to_start(&mut self) {
        self.cursor_grapheme = 0;
    }

    pub fn move_to_end(&mut self) {
        self.cursor_grapheme = self.grapheme_count();
    }

    /// Handles keyboard and paste events. Returns true if modified.
    pub fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::Key(k) => match k.code {
                KeyCode::Char(c) => {
                    let m = k.modifiers;
                    if m.contains(KeyModifiers::ALT) {
                        // Alt+<char> is an application accelerator, not text
                        // (issue #35): do not insert it and do not claim the
                        // event, so an outer handler (e.g. a global hotkey)
                        // still receives it.
                        false
                    } else if m.contains(KeyModifiers::CONTROL) {
                        match c {
                            'a' => {
                                self.move_to_start();
                                true
                            }
                            'e' => {
                                self.move_to_end();
                                true
                            }
                            'u' => {
                                // Clear line before cursor
                                self.normalize();
                                let byte_pos = self.cursor_byte_pos();
                                self.text.replace_range(..byte_pos, "");
                                self.cursor_grapheme = 0;
                                true
                            }
                            'k' => {
                                // Clear line after cursor
                                self.normalize();
                                let byte_pos = self.cursor_byte_pos();
                                self.text.truncate(byte_pos);
                                true
                            }
                            _ => false,
                        }
                    } else {
                        // A control character typed as a key (e.g. a raw ESC
                        // or TAB delivered as `Char`) inserts nothing — or a
                        // space for TAB — so report only a real change.
                        let before = self.text.len();
                        self.insert_char(c);
                        self.text.len() != before
                    }
                }
                KeyCode::Backspace => self.backspace(),
                KeyCode::Delete => self.delete(),
                KeyCode::Left => self.move_left(),
                KeyCode::Right => self.move_right(),
                KeyCode::Home => {
                    self.move_to_start();
                    true
                }
                KeyCode::End => {
                    self.move_to_end();
                    true
                }
                _ => false,
            },
            Event::Paste(s) => {
                // `insert_str` normalizes; a paste of nothing but controls
                // changes nothing and is reported as such.
                let before = self.text.len();
                self.insert_str(s);
                self.text.len() != before
            }
            _ => false,
        }
    }

    /// Adjusts viewport scroll offset (in display columns) so the cursor is visible within `visible_width`.
    ///
    /// "Visible" means every cell of the grapheme under the cursor (or the
    /// one-cell insertion point at the end) — a wide grapheme at the cursor is
    /// never left half outside the viewport.
    pub fn update_scroll(&mut self, visible_width: usize) {
        if visible_width == 0 {
            return;
        }
        let span = self
            .text
            .graphemes(true)
            .nth(self.cursor_grapheme)
            .map_or(1, editor_cell_width);
        self.scroll_offset = scroll_to_cursor(
            self.scroll_offset,
            self.cursor_display_column(),
            span,
            visible_width,
        );
    }

    /// Returns the cursor position in monospace terminal display columns,
    /// under the type's width contract.
    pub fn cursor_display_column(&self) -> usize {
        self.text
            .graphemes(true)
            .take(self.cursor_grapheme)
            .map(editor_cell_width)
            .sum()
    }

    /// Returns the total display width of the buffer in terminal columns,
    /// under the type's width contract — always equal to
    /// [`cursor_display_column`](Self::cursor_display_column) with the cursor
    /// at the end.
    pub fn total_display_width(&self) -> usize {
        self.text.graphemes(true).map(editor_cell_width).sum()
    }
}

/// The cell the single-line editor paints for a grapheme it cannot place
/// verbatim (see [`TextInputState`] — *Width contract*).
pub(crate) const UNRENDERABLE_GRAPHEME: &str = "\u{FFFD}";

/// Line and paragraph breaks the single-line editor flattens to a space.
fn is_line_break(c: char) -> bool {
    matches!(
        c,
        '\r' | '\n' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

/// The pieces of a grapheme wider than two cells: each base codepoint with the
/// zero-width codepoints that follow it (leading zero-width codepoints join the
/// first base).
fn codepoint_pieces(g: &str) -> impl Iterator<Item = &str> {
    let mut rest = g;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let mut seen_base = false;
        let mut end = rest.len();
        for (i, c) in rest.char_indices() {
            if UnicodeWidthChar::width(c).unwrap_or(0) > 0 {
                if seen_base {
                    end = i;
                    break;
                }
                seen_base = true;
            }
        }
        let (piece, tail) = rest.split_at(end);
        rest = tail;
        Some(piece)
    })
}

/// How the editor places grapheme `g` (see [`TextInputState`] — *Width
/// contract*): calls `cell` with each cell's text and width, left to right.
/// Every cell is 1 or 2 wide; the widths sum to [`editor_cell_width`].
pub(crate) fn editor_cells(g: &str, mut cell: impl FnMut(&str, usize)) {
    if g.chars().any(|c| c.is_control() || is_line_break(c)) {
        return cell(UNRENDERABLE_GRAPHEME, 1);
    }
    let w = UnicodeWidthStr::width(g);
    match w {
        0 => cell(UNRENDERABLE_GRAPHEME, 1),
        1 | 2 => cell(g, w),
        _ => {
            if codepoint_pieces(g).any(|p| !(1..=2).contains(&UnicodeWidthStr::width(p))) {
                return cell(UNRENDERABLE_GRAPHEME, 1);
            }
            for p in codepoint_pieces(g) {
                cell(p, UnicodeWidthStr::width(p));
            }
        }
    }
}

/// The terminal cells the editor gives grapheme `g` under the width contract.
pub(crate) fn editor_cell_width(g: &str) -> usize {
    let mut total = 0;
    editor_cells(g, |_, w| total += w);
    total
}

/// The scroll offset (display columns) closest to `scroll` that shows all
/// `span` cells starting at `cursor_col` inside a `visible`-column viewport.
/// A span wider than the viewport is left-aligned.
pub(crate) fn scroll_to_cursor(
    scroll: usize,
    cursor_col: usize,
    span: usize,
    visible: usize,
) -> usize {
    let span = span.clamp(1, visible.max(1));
    if cursor_col < scroll {
        cursor_col
    } else if cursor_col + span > scroll + visible {
        cursor_col + span - visible
    } else {
        scroll
    }
}

/// The single-line, control-free form of `s`, borrowed when already clean.
fn single_line(s: &str) -> Cow<'_, str> {
    if !s.chars().any(|c| c.is_control() || is_line_break(c)) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut in_break = false;
    for c in s.chars() {
        match c {
            c if is_line_break(c) => {
                if !in_break {
                    out.push(' ');
                    in_break = true;
                }
            }
            '\t' => {
                out.push(' ');
                in_break = false;
            }
            // Dropped: an invisible control neither adds text nor ends a break run.
            c if c.is_control() => {}
            c => {
                out.push(c);
                in_break = false;
            }
        }
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_input_typing() {
        let mut input = TextInputState::new();
        input.handle_event(&Event::Key(KeyEvent::char('a')));
        input.handle_event(&Event::Key(KeyEvent::char('b')));
        input.handle_event(&Event::Key(KeyEvent::char('c')));

        assert_eq!(input.text, "abc");
        assert_eq!(input.cursor_grapheme, 3);

        input.handle_event(&Event::Key(KeyEvent::new(
            KeyCode::Backspace,
            KeyModifiers::empty(),
        )));
        assert_eq!(input.text, "ab");
        assert_eq!(input.cursor_grapheme, 2);
    }

    #[test]
    fn test_text_input_grapheme_combining_backspace() {
        let mut input = TextInputState::new();
        // Insert 'e' followed by combining acute accent
        input.insert_str("e\u{0301}");
        assert_eq!(input.grapheme_count(), 1);
        assert_eq!(input.cursor_grapheme, 1);

        // A single backspace MUST delete the entire combining character
        input.backspace();
        assert_eq!(input.text, "");
        assert_eq!(input.cursor_grapheme, 0);
    }

    #[test]
    fn test_text_input_emoji_navigation() {
        let mut input = TextInputState::new();
        input.insert_str("🦀🦀");
        assert_eq!(input.grapheme_count(), 2);
        assert_eq!(input.cursor_grapheme, 2);

        input.move_left();
        assert_eq!(input.cursor_grapheme, 1);

        input.insert_str("x");
        assert_eq!(input.text, "🦀x🦀");
        assert_eq!(input.cursor_grapheme, 2);
    }

    #[test]
    fn test_bracketed_paste() {
        let mut input = TextInputState::new();
        input.insert_str("start-");
        input.handle_event(&Event::Paste("pasted content".to_string()));
        input.insert_str("-end");

        assert_eq!(input.text, "start-pasted content-end");
    }

    #[test]
    fn test_text_input_mixed_display_width_scrolling() {
        let mut input = TextInputState::new();
        // 🦀 = 2 cols, A = 1 col, 你 = 2 cols -> 5 cols total
        input.insert_str("🦀A你");
        assert_eq!(input.cursor_grapheme, 3);
        assert_eq!(input.cursor_display_column(), 5);

        // Visible width of 3 columns
        input.update_scroll(3);
        // cursor is at col 5, visible window must contain 5, so scroll_offset = 5 - 3 + 1 = 3
        assert_eq!(input.scroll_offset, 3);

        // Move cursor back to beginning
        input.move_to_start();
        assert_eq!(input.cursor_display_column(), 0);
        input.update_scroll(3);
        assert_eq!(input.scroll_offset, 0);
    }

    #[test]
    fn update_scroll_keeps_a_wide_cursor_grapheme_wholly_visible() {
        // 漢 sits at columns 2..4. With a 3-column viewport the old rule
        // (cursor column 2 < 0 + 3, so no scroll) left its second half outside
        // the viewport, so the painter could not draw the grapheme under the
        // cursor (#252 review). Scrolling must bring both cells in.
        let mut input = TextInputState::with_text("ab漢");
        input.cursor_grapheme = 2;
        input.update_scroll(3);
        assert_eq!(input.scroll_offset, 1);
        // A grapheme wider than the whole viewport is left-aligned.
        input.scroll_offset = 0;
        input.update_scroll(1);
        assert_eq!(input.scroll_offset, 2);
    }

    #[test]
    fn insertion_is_single_line_and_control_free() {
        assert_eq!(
            TextInputState::normalize_paste("a\r\n\r\nb\tc\x1b[2Jd\u{9b}e\x07"),
            "a b c[2Jde"
        );
        let mut s = TextInputState::new();
        assert!(
            !s.handle_event(&Event::Paste("\x1b\x07".into())),
            "a paste of only controls changes nothing and says so"
        );
        assert!(!s.handle_event(&Event::Key(KeyEvent::char('\x1b'))));
        assert!(s.handle_event(&Event::Key(KeyEvent::char('\t'))));
        assert_eq!(s.text, " ");
        assert_eq!(TextInputState::with_text("x\ny").text, "x y");
        // Every Unicode line break flattens; words never fuse.
        assert_eq!(
            TextInputState::normalize_paste("a\x0bb\x0cc\u{85}d\u{2028}e\u{2029}f"),
            "a b c d e f"
        );
    }

    // ---------------------------------------------------------------------
    // Unicode mutation correctness (merge-across-boundary regressions)
    // ---------------------------------------------------------------------

    fn assert_invariant(input: &TextInputState) {
        assert!(
            input.cursor_grapheme <= input.grapheme_count(),
            "cursor {} exceeded grapheme_count {} for {:?}",
            input.cursor_grapheme,
            input.grapheme_count(),
            input.text
        );
    }

    #[test]
    fn test_combining_accent_inserted_independently_merges() {
        // The canonical adversarial case from the hardening brief:
        // type 'e', then insert U+0301. Inserted alone it is one cluster, but
        // e + combining acute is ONE extended grapheme cluster.
        let mut input = TextInputState::new();
        input.insert_char('e');
        assert_eq!(input.cursor_grapheme, 1);
        assert_eq!(input.grapheme_count(), 1);

        input.insert_char('\u{0301}');
        // Must be re-segmented: one cluster, cursor at 1 -- never 2!
        assert_eq!(input.text, "e\u{0301}");
        assert_eq!(input.grapheme_count(), 1);
        assert_eq!(input.cursor_grapheme, 1);
        assert_invariant(&input);

        // A backspace must remove the whole cluster and must not panic.
        assert!(input.backspace());
        assert_eq!(input.text, "");
        assert_eq!(input.cursor_grapheme, 0);
    }

    #[test]
    fn test_insert_before_following_grapheme_merges_backwards() {
        // Cursor in the middle: inserting a combining mark must merge with the
        // *preceding* character, not the following one.
        let mut input = TextInputState::with_text("ab");
        input.move_left(); // cursor between 'a' and 'b' (index 1)
        input.insert_char('\u{0301}'); // "a" + combining acute + "b"
        assert_eq!(input.text, "a\u{0301}b");
        assert_eq!(input.grapheme_count(), 2); // "á", "b"
        assert_eq!(input.cursor_grapheme, 1);
        assert_invariant(&input);

        // Typing 'b' again yields "áb"; deleting forward removes 'b'.
        assert!(input.delete());
        assert_eq!(input.text, "a\u{0301}");
    }

    #[test]
    fn test_emoji_skin_tone_modifier_merges() {
        let mut input = TextInputState::new();
        input.insert_str("👍");
        assert_eq!(input.grapheme_count(), 1);
        input.insert_str("🏽"); // fitzpatrick type-5 modifier
        assert_eq!(input.grapheme_count(), 1);
        assert_eq!(input.cursor_grapheme, 1);
        assert_invariant(&input);
    }

    #[test]
    fn test_zwj_emoji_sequence_merges() {
        // 👩 + ZWJ + 💻 = woman technologist (one cluster)
        let mut input = TextInputState::new();
        input.insert_str("👩");
        input.insert_char('\u{200D}');
        assert_eq!(input.grapheme_count(), 1);
        input.insert_str("💻");
        assert_eq!(input.grapheme_count(), 1);
        assert_eq!(input.cursor_grapheme, 1);
        assert_invariant(&input);
    }

    #[test]
    fn test_regional_indicator_flags() {
        let mut input = TextInputState::new();
        // Regional indicator U and S separately form a flag; two at a time pair up.
        input.insert_str("\u{1F1FA}\u{1F1F8}"); // 🇺🇸
        assert_eq!(input.grapheme_count(), 1);
        assert_eq!(input.cursor_grapheme, 1);
        input.insert_str("\u{1F1EF}\u{1F1F5}"); // 🇯🇵
        assert_eq!(input.grapheme_count(), 2);
        assert_eq!(input.cursor_grapheme, 2);
        assert_invariant(&input);
    }

    #[test]
    fn test_mixed_ascii_cjk_emoji_arbitrary_edits_keep_invariant() {
        // Deterministic randomized edit fuzzing with a small LCG.
        let alphabet: Vec<&str> = vec![
            "a",
            "Z",
            " ",
            "é",
            "\u{0301}",
            "\u{0308}",
            "你",
            "好",
            "🦀",
            "👍",
            "🏽",
            "👩",
            "\u{200D}",
            "💻",
            "\u{1F1FA}",
            "\u{1F1F8}",
            "\u{1F1EF}",
            "\u{1F1F5}",
        ];
        let mut state: u64 = 0x9E3779B97F4A7C15;
        let mut input = TextInputState::new();

        for step in 0..4000u64 {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let op = (state >> 33) % 8;
            match op {
                0 => {
                    let g = alphabet[((state >> 8) as usize) % alphabet.len()];
                    input.insert_str(g);
                }
                1 => {
                    input.backspace();
                }
                2 => {
                    input.delete();
                }
                3 => {
                    input.move_left();
                }
                4 => {
                    input.move_right();
                }
                5 => {
                    input.move_to_start();
                }
                6 => {
                    input.move_to_end();
                }
                _ => {
                    let g = alphabet[((state >> 16) as usize) % alphabet.len()];
                    input.handle_event(&Event::Key(KeyEvent::char(g.chars().next().unwrap())));
                    // also exercise control-U/control-K occasionally
                    if step % 7 == 0 {
                        input.handle_event(&Event::Key(KeyEvent::new(
                            KeyCode::Char('u'),
                            KeyModifiers::CONTROL,
                        )));
                    }
                }
            }
            assert_invariant(&input);
        }
    }

    #[test]
    fn test_single_line_paste_normalizes_line_breaks() {
        let mut input = TextInputState::new();
        input.handle_event(&Event::Paste(
            "line one\nline two\r\nline three\rline four".into(),
        ));
        assert_eq!(input.text, "line one line two line three line four");
        assert!(!input.text.contains('\n'));
        assert!(!input.text.contains('\r'));
        assert_invariant(&input);
    }

    #[test]
    fn test_paste_merges_with_existing_combining_sequence() {
        let mut input = TextInputState::with_text("e");
        input.handle_event(&Event::Paste("\u{0301}".into()));
        assert_eq!(input.grapheme_count(), 1);
        assert_eq!(input.cursor_grapheme, 1);
        assert_invariant(&input);
    }
}

#[cfg(test)]
mod mouse_focus_tests {
    use super::{
        decode_event, Event, FocusChange, KeyModifiers, MouseButton, MouseEvent, MouseKind,
    };
    use crossterm::event::{
        Event as CtEvent, KeyModifiers as CtKeyModifiers, MouseButton as CtMouseButton,
        MouseEvent as CtMouseEvent, MouseEventKind as CtMouseKind,
    };

    fn ct_mouse(kind: CtMouseKind, column: u16, row: u16, modifiers: CtKeyModifiers) -> CtEvent {
        CtEvent::Mouse(CtMouseEvent {
            kind,
            column,
            row,
            modifiers,
        })
    }

    #[test]
    fn mouse_down_maps_button_position_and_modifiers() {
        // ctrl + left-press at cell (3, 4) normalizes verbatim.
        let got = decode_event(ct_mouse(
            CtMouseKind::Down(CtMouseButton::Left),
            3,
            4,
            CtKeyModifiers::CONTROL,
        ));
        assert_eq!(
            got,
            Some(Event::Mouse(MouseEvent {
                kind: MouseKind::Down(MouseButton::Left),
                column: 3,
                row: 4,
                modifiers: KeyModifiers::CONTROL,
            }))
        );
    }

    #[test]
    fn every_mouse_kind_normalizes() {
        let cases = [
            (
                CtMouseKind::Up(CtMouseButton::Right),
                MouseKind::Up(MouseButton::Right),
            ),
            (
                CtMouseKind::Drag(CtMouseButton::Middle),
                MouseKind::Drag(MouseButton::Middle),
            ),
            (CtMouseKind::Moved, MouseKind::Moved),
            (CtMouseKind::ScrollUp, MouseKind::ScrollUp),
            (CtMouseKind::ScrollDown, MouseKind::ScrollDown),
            (CtMouseKind::ScrollLeft, MouseKind::ScrollLeft),
            (CtMouseKind::ScrollRight, MouseKind::ScrollRight),
        ];
        for (ct, ours) in cases {
            let got = decode_event(ct_mouse(ct, 1, 2, CtKeyModifiers::empty()));
            assert_eq!(
                got,
                Some(Event::Mouse(MouseEvent {
                    kind: ours,
                    column: 1,
                    row: 2,
                    modifiers: KeyModifiers::empty(),
                }))
            );
        }
    }

    #[test]
    fn every_button_maps() {
        for (ct, ours) in [
            (CtMouseButton::Left, MouseButton::Left),
            (CtMouseButton::Right, MouseButton::Right),
            (CtMouseButton::Middle, MouseButton::Middle),
        ] {
            let got = decode_event(ct_mouse(
                CtMouseKind::Down(ct),
                0,
                0,
                CtKeyModifiers::empty(),
            ));
            assert_eq!(
                got,
                Some(Event::Mouse(MouseEvent {
                    kind: MouseKind::Down(ours),
                    column: 0,
                    row: 0,
                    modifiers: KeyModifiers::empty(),
                }))
            );
        }
    }

    #[test]
    fn drag_is_distinct_from_down() {
        // MUTATION CONTROL: a press and a drag of the same button must NOT normalize
        // to the same event — a decoder that collapsed Drag into Down would fail.
        let down = decode_event(ct_mouse(
            CtMouseKind::Down(CtMouseButton::Left),
            5,
            5,
            CtKeyModifiers::empty(),
        ));
        let drag = decode_event(ct_mouse(
            CtMouseKind::Drag(CtMouseButton::Left),
            5,
            5,
            CtKeyModifiers::empty(),
        ));
        assert_ne!(down, drag);
    }

    #[test]
    fn focus_gained_and_lost_normalize() {
        assert_eq!(
            decode_event(CtEvent::FocusGained),
            Some(Event::Focus(FocusChange::Gained))
        );
        assert_eq!(
            decode_event(CtEvent::FocusLost),
            Some(Event::Focus(FocusChange::Lost))
        );
    }
}
