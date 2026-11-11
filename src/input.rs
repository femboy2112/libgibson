use crossterm::event::{
    self, Event as CtEvent, KeyCode as CtKeyCode, KeyEventKind, KeyModifiers as CtKeyModifiers,
    MouseButton as CtMouseButton, MouseEvent as CtMouseEvent, MouseEventKind as CtMouseKind,
};
use std::borrow::Cow;
use std::io;
use std::sync::atomic::AtomicU64;
#[cfg(unix)]
use std::sync::atomic::AtomicU8;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
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

/// The terminal's answer to a key-release reporting request
/// (`TerminalSession::set_key_release_reporting`, #275), delivered as
/// [`Event::KeyReleaseReporting`].
///
/// `#[non_exhaustive]`: downstream `match`es must include a `_` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyReleaseReporting {
    /// The terminal confirmed it (it speaks the kitty keyboard protocol): from this event on,
    /// a key that goes up is reported as an [`Event::KeyRelease`].
    Active,
    /// The terminal answered without confirming the current request: keys stay press-only, and
    /// no [`Event::KeyRelease`] will come for it. Final for that request: once any poll hands it
    /// out, releases stop and the request is withdrawn (see
    /// `TerminalSession::set_key_release_reporting`).
    ///
    /// Also the notice that reporting *ended* because the panic hook popped the flags. That
    /// notice is news about the earlier request, not a verdict on a later one: if the client
    /// enabled reporting again after a caught panic, the notice comes first and the new request
    /// stands, so its own answer (possibly [`Active`](Self::Active)) follows.
    Unsupported,
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
    /// A key went up. Only produced while key-release reporting is enabled with
    /// `TerminalSession::set_key_release_reporting` — **off by default** — on a terminal that
    /// confirmed it ([`Event::KeyReleaseReporting`]); a terminal without the kitty keyboard
    /// protocol reports presses only (#275).
    ///
    /// The key is named exactly as its press was delivered, the same code and modifiers, even
    /// when the terminal names it differently on the way up (shift let go first turns a released
    /// `W` into `w`), so a release matches its press by equality. A key that was already down
    /// when reporting began has no recorded press: its release is named as its first autorepeat
    /// since then was delivered, or, with none, as the terminal reports it. Not covered: a
    /// shifted symbol or a letter whose lowercase does not match its base key as a single Unicode
    /// character, pressed just before the flags take effect (at enable, or as a screen switch
    /// pushes them again) and let go without an autorepeat, is released as the terminal reports it
    /// (for example `!` as `1`), so that release does not match its press
    /// ([#312](https://github.com/femboy2112/libgibson/issues/312)). An autorepeat of a held key is
    /// an [`Event::Key`], as on any terminal.
    KeyRelease(KeyEvent),
    /// The terminal's answer to a key-release reporting request, in input order: every event
    /// after it was produced under that answer.
    KeyReleaseReporting(KeyReleaseReporting),
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

/// One decoded item from a terminal input source, before LibGibson's delivery rules
/// ([`decode_source_event`]): a terminal event, or the answer to a key-release reporting
/// request, which only the library-owned Unix source recognises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceEvent {
    Terminal(CtEvent),
    // Only the Unix source constructs the two variants below; elsewhere they are matched but
    // never built.
    #[cfg_attr(not(unix), allow(dead_code))]
    KeyReleaseReporting(KeyReleaseReporting),
    /// The panic hook ended key-release reporting and nobody has said so yet (#275 review 8).
    /// The Unix source queues it just before the first answer it decodes after the end, so
    /// whichever poll hands that answer out says `Unsupported` first. Delivered as `Unsupported`,
    /// whatever the delivery state, and changes none of it: it is news, not a verdict.
    #[cfg_attr(not(unix), allow(dead_code))]
    ReportingEnded,
}

impl From<CtEvent> for SourceEvent {
    fn from(ev: CtEvent) -> Self {
        SourceEvent::Terminal(ev)
    }
}

/// A source item and, from the Unix source, the generation it was decoded in (see
/// [`KEY_RELEASE_STATE`]): what ties an answer to the request it answers (#275 review 8).
pub(crate) struct SourceItem {
    event: SourceEvent,
    generation: Option<u64>,
}

impl From<CtEvent> for SourceItem {
    fn from(ev: CtEvent) -> Self {
        SourceEvent::Terminal(ev).into()
    }
}

impl From<SourceEvent> for SourceItem {
    fn from(event: SourceEvent) -> Self {
        SourceItem {
            event,
            generation: None,
        }
    }
}

impl From<(u64, SourceEvent)> for SourceItem {
    fn from((generation, event): (u64, SourceEvent)) -> Self {
        SourceItem {
            event,
            generation: Some(generation),
        }
    }
}

/// Key-release delivery as one word, so that what an answer does to it is a single atomic step
/// (#275 review 8): the generation (epoch) shifted left one, ORed with whether a key release is
/// delivered as [`Event::KeyRelease`] (true while a session has key-release reporting enabled;
/// off, a release is dropped, #285). The generation moves every time delivery is turned on or
/// off, so the Unix source forgets the keys it recorded under the previous setting before decoding
/// anything more, and an answer decoded in one generation can never act on a later one.
static KEY_RELEASE_STATE: AtomicU64 = AtomicU64::new(0);

/// [`KEY_RELEASE_STATE`] for `generation` with delivery `on`.
const fn key_release_state(generation: u64, on: bool) -> u64 {
    (generation << 1) | on as u64
}

/// Moves `state` to the next generation with delivery `on`.
fn set_delivered_in(state: &AtomicU64, on: bool) {
    let _ = state.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |s| {
        Some(key_release_state((s >> 1).wrapping_add(1), on))
    });
}

/// Turns delivery of [`Event::KeyRelease`] (and of the terminal's answers) on or off; the session
/// calls this when it enables, disables or restores key-release reporting.
pub(crate) fn set_key_releases_delivered(on: bool) {
    set_delivered_in(&KEY_RELEASE_STATE, on);
}

#[cfg(unix)]
pub(crate) fn key_release_epoch() -> u64 {
    KEY_RELEASE_STATE.load(Ordering::SeqCst) >> 1
}

pub(crate) fn key_releases_delivered() -> bool {
    KEY_RELEASE_STATE.load(Ordering::SeqCst) & 1 != 0
}

/// Hands out the terminal's answer to the request made in `generation`, against `state`
/// ([`KEY_RELEASE_STATE`] outside tests). Only while that generation is still current with
/// delivery on: anything else is a stale answer, and changes nothing. An `Unsupported` also ends
/// delivery, in the same compare-and-swap that checks the generation, so an old verdict read by a
/// slow poller cannot withdraw a request made after it (#275 review 8). Being right once is the
/// whole job; being right about last week's patient is how you get sued.
fn hand_out_answer_in(
    state: &AtomicU64,
    answer: KeyReleaseReporting,
    generation: u64,
) -> Option<Event> {
    let live = key_release_state(generation, true);
    let current = match answer {
        KeyReleaseReporting::Active => state.load(Ordering::SeqCst) == live,
        KeyReleaseReporting::Unsupported => state
            .compare_exchange(
                live,
                key_release_state(generation.wrapping_add(1), false),
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok(),
    };
    current.then_some(Event::KeyReleaseReporting(answer))
}

/// Polls for an event with a timeout.
///
/// On a Unix controlling terminal this is **hangup-aware**: once the terminal hangs
/// up (SIGHUP, master closed) it returns `Err(ErrorKind::BrokenPipe)` instead of
/// spinning forever (issues #79, #269). There LibGibson reads and decodes stdin itself
/// (`tty_source`, with crossterm 0.29.0's decoder) rather than through crossterm's
/// event reader, whose read loop retries a hung-up terminal forever. The hangup is
/// detected from the read itself (`0` bytes or `EIO`), so it is reported whenever it
/// lands, including in the middle of a poll. Events decoded before the hangup are
/// delivered first; every poll after that returns `BrokenPipe`.
///
/// The dispatch keys off input-source **identity**, not a fresh liveness probe. Once
/// stdin has been seen as a controlling terminal we keep the hangup-aware path even
/// after it hangs up — at which point `isatty` reports false. Re-probing `isatty`
/// per call (as this once did) let a hangup that landed *between* polls demote the
/// very next poll to crossterm's spin-on-a-hung-fd path, the exact #79 failure it is
/// meant to prevent. A source that was never a terminal (redirected input, a pipe)
/// never latches and keeps the portable path, so a closed pipe is not misread as a
/// hung terminal. See `latched_terminal_source`.
pub fn poll_event(timeout: Duration) -> io::Result<Option<Event>> {
    #[cfg(unix)]
    {
        if let Some(stdin_is_terminal) = latched_terminal_source() {
            return poll_event_unix_tty(timeout, stdin_is_terminal);
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
    // crossterm 0.29's Windows poll converts the timeout with `as_millis() as u32`; a
    // timeout of 2^32 ms or more would wrap, and 0xFFFF_FFFF means INFINITE there. Cap
    // it at the largest finite value (about 49.7 days).
    let timeout = timeout.min(Duration::from_millis(u64::from(u32::MAX - 1)));
    poll_until_deliverable(timeout, |left| {
        if event::poll(left)? {
            Ok(Some(SourceEvent::Terminal(event::read()?)))
        } else {
            Ok(None)
        }
    })
}

/// How many undelivered events one poll may still consume once its deadline has passed.
///
/// Past the deadline the source is only asked for events that are already available (a zero
/// wait), so a deliverable event queued behind a few dropped ones still comes out of this poll.
/// The cap bounds the work: a terminal that keeps producing undelivered events (a stream of key
/// releases, say) cannot hold a poll past its timeout (#285 review).
const MAX_DROPS_PAST_DEADLINE: u32 = 64;

/// Polls `next` until it yields an event LibGibson delivers, or `timeout` has passed.
///
/// `next(left)` waits at most `left` for one decoded terminal event (`Ok(None)` when it waited
/// that long and none came). An event [`decode_source_event`] does not deliver (a key release, #285,
/// or a key code LibGibson does not model) is consumed and the wait **continues** for the rest
/// of the same deadline: dropping it must not end the poll early as if the timeout had passed,
/// and the events queued behind it are delivered in order. So `Ok(None)` means the timeout
/// passed with nothing delivered. Once the deadline has passed, at most
/// [`MAX_DROPS_PAST_DEADLINE`] further undelivered events, each already available, are consumed
/// before the poll gives up; with a zero timeout that is all the poll does.
fn poll_until_deliverable<T: Into<SourceItem>>(
    timeout: Duration,
    mut next: impl FnMut(Duration) -> io::Result<Option<T>>,
) -> io::Result<Option<Event>> {
    // A timeout too large to add to `now` is effectively unbounded: keep passing it whole.
    let deadline = Instant::now().checked_add(timeout);
    let mut drops_past_deadline = 0u32;
    loop {
        let left = deadline.map_or(timeout, |d| d.saturating_duration_since(Instant::now()));
        match next(left)? {
            None => return Ok(None),
            Some(raw) => {
                if let Some(event) = decode_source_event(raw.into()) {
                    return Ok(Some(event));
                }
                if left.is_zero() {
                    drops_past_deadline += 1;
                    if drops_past_deadline >= MAX_DROPS_PAST_DEADLINE {
                        return Ok(None);
                    }
                }
            }
        }
    }
}

/// Whether a key event of this kind is delivered as a press. Press and repeat are; a release is
/// not, on Unix (#285), where only a kitty-keyboard-protocol terminal reports one. While
/// key-release reporting is enabled a Unix release is delivered as [`Event::KeyRelease`]
/// instead (see [`decode_event_as`]).
///
/// Windows keeps its earlier behaviour, every kind delivered (#285 review). crossterm 0.29 reports
/// a release there for each ordinary key-up, which LibGibson has always delivered as a second press
/// (#292), but it also delivers an Alt+numpad character code **only** as the release of Alt, and
/// a decoded `KeyEvent` does not say which of the two a `Char` release is. Dropping releases there
/// would lose those characters; fixing the doubling needs that distinction and a Windows test.
///
/// **KNOWN, UNVERIFIED LIMITATION (#292): on Windows every key is delivered twice**, once for the
/// key-down (`Press`) and once for the key-up (`Release`, crossterm 0.29 `parse.rs:286-290`), and
/// [`decode_key`] ignores the kind, so both arrive as [`Event::Key`]. No Windows runtime or CI job
/// has exercised this; it is read from the crossterm source and pinned only through the `windows`
/// parameter by a host-independent test. Windows is not a supported platform this round.
///
/// Why the predicate is not simply `kind != Release`: an Alt+numpad Unicode character (Alt+0233
/// gives an e-acute) reaches the console only as a `Release` carrying `KeyCode::Char`, so a blanket
/// release filter would delete real composed input (a surrogate-paired character arrives as a
/// `Press` and would survive, which is the only case that filter would not break). The fix is a
/// LibGibson-owned Windows console source that reads `KEY_EVENT_RECORD`s and can tell the two
/// releases apart. That is future work and needs a native Windows host to write and test.
fn delivers_key_kind(kind: KeyEventKind, windows: bool) -> bool {
    kind != KeyEventKind::Release || windows
}

/// Applies LibGibson's delivery rules to one source item, with key-release delivery as the
/// session set it ([`set_key_releases_delivered`]). Every public poll hands events out through
/// here. An answer is handed out only in the generation of the request it answers, and an
/// `Unsupported` one ends delivery in the same atomic step ([`hand_out_answer_in`]), so it is final
/// for every poll (#275 review 8): releases queued behind it are dropped as they are handed out,
/// and a stale one changes nothing. Popping the push stays the owning session's (its next
/// `Context` poll, enable, disable or screen switch, `restore`, `Drop` or the panic hook): anyone
/// may read the verdict, only the owner may act on it.
fn decode_source_event(item: SourceItem) -> Option<Event> {
    match (item.event, item.generation) {
        (SourceEvent::KeyReleaseReporting(r), Some(generation)) => {
            hand_out_answer_in(&KEY_RELEASE_STATE, r, generation)
        }
        // Only the Unix source decodes answers, and it tags each with its generation.
        (SourceEvent::KeyReleaseReporting(_), None) => None,
        (ev, _) => decode_source_event_as(ev, cfg!(windows), key_releases_delivered()),
    }
}

/// [`decode_source_event`] with delivery given, for an item that is not a tagged answer. Both a
/// release and an answer are decided at delivery: one decoded while reporting was on but handed
/// out after it was turned off is dropped, so a late answer cannot switch a hold model back on
/// (#275 review). The notice that reporting ended is always delivered.
fn decode_source_event_as(ev: SourceEvent, windows: bool, releases: bool) -> Option<Event> {
    match ev {
        SourceEvent::Terminal(ev) => decode_event_as(ev, windows, releases),
        SourceEvent::KeyReleaseReporting(r) => releases.then_some(Event::KeyReleaseReporting(r)),
        SourceEvent::ReportingEnded => {
            Some(Event::KeyReleaseReporting(KeyReleaseReporting::Unsupported))
        }
    }
}

/// Translates a crossterm event into our [`Event`] with key releases **not** delivered (the
/// default; see [`decode_event_as`]).
#[cfg(test)]
fn decode_event(ev: CtEvent) -> Option<Event> {
    decode_event_as(ev, cfg!(windows), false)
}

/// Translates a crossterm event into our [`Event`]; `None` for what LibGibson does not
/// deliver: a key code it does not model, and a key **release** unless `releases` (key-release
/// reporting is enabled) on a non-Windows platform (#285, #275).
///
/// Without key-release reporting the public input model is press-only. A kitty-keyboard-protocol
/// terminal reports an event type with each key: a press and a repeat are delivered as
/// [`Event::Key`] (a repeat is what a held key produces on any terminal), and a release is not
/// delivered, never as a second press (see [`delivers_key_kind`] for the Windows Alt-code
/// exception). With it, a release is delivered as [`Event::KeyRelease`].
fn decode_event_as(ev: CtEvent, windows: bool, releases: bool) -> Option<Event> {
    match ev {
        CtEvent::Key(k) if k.kind == KeyEventKind::Release && releases && !windows => {
            decode_key(&k).map(Event::KeyRelease)
        }
        CtEvent::Key(k) if !delivers_key_kind(k.kind, windows) => None,
        CtEvent::Key(k) => decode_key(&k).map(Event::Key),
        CtEvent::Paste(s) => Some(Event::Paste(s)),
        CtEvent::Resize(cols, rows) => Some(Event::Resize(cols, rows)),
        CtEvent::Mouse(m) => decode_mouse(m),
        CtEvent::FocusGained => Some(Event::Focus(FocusChange::Gained)),
        CtEvent::FocusLost => Some(Event::Focus(FocusChange::Lost)),
    }
}

/// Our [`KeyEvent`] for a crossterm key, whatever its kind; `None` for a key code LibGibson does
/// not model (function keys beyond the set below, keypad and media keys, modifier keys).
fn decode_key(k: &crossterm::event::KeyEvent) -> Option<KeyEvent> {
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
    Some(KeyEvent {
        code,
        modifiers: k.modifiers.into(),
    })
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

/// Hangup-aware poll for a Unix controlling terminal (issues #79, #269): the library-owned
/// source in `tty_source`, which reads stdin when stdin is a terminal and `/dev/tty`
/// otherwise (as crossterm does). It returns `Err(BrokenPipe)` once that terminal has hung
/// up, wherever in the poll the hangup lands.
#[cfg(unix)]
fn poll_event_unix_tty(timeout: Duration, stdin_is_terminal: bool) -> io::Result<Option<Event>> {
    poll_until_deliverable(timeout, |left| {
        crate::tty_source::poll(left, stdin_is_terminal)
    })
}

/// Whether stdin is a real terminal *right now* — the liveness probe. After a
/// hangup this reports false even though the session's terminal identity is
/// unchanged, which is why [`poll_event`] dispatches on the latched identity
/// ([`latched_terminal_source`]) rather than on this probe directly.
#[cfg(unix)]
fn stdin_is_tty() -> bool {
    // SAFETY: `isatty` is always safe to call on any fd.
    unsafe { libc::isatty(libc::STDIN_FILENO) == 1 }
}

/// Latched input-source identity, one atomic value so it is never observed half-written by
/// a concurrent poll (#269 review): [`UNLATCHED`], or latched as [`LATCHED_STDIN`] (stdin itself
/// is the terminal) or [`LATCHED_DEV_TTY`] (the session reached the terminal through
/// `/dev/tty`, stdin being redirected). Terminal *identity* is stable across a *liveness*
/// transition — a tty that hangs up is still the terminal we acquired, not a newly discovered
/// non-tty backend, and never a reason to read another terminal — so once latched we never
/// demote off the hangup-aware path or switch descriptors. See [`poll_event`] and
/// [`resolve_input_source`].
#[cfg(unix)]
static TTY_SOURCE: AtomicU8 = AtomicU8::new(UNLATCHED);
#[cfg(unix)]
const UNLATCHED: u8 = 0;
#[cfg(unix)]
const LATCHED_STDIN: u8 = 1;
#[cfg(unix)]
const LATCHED_DEV_TTY: u8 = 2;

/// Records that a live controlling terminal was acquired for input (raw mode entered
/// on a tty). Idempotent; called from the terminal-session boundary so identity is
/// established at acquisition, not merely on the first poll.
///
/// Returns the identity this call latched, or `None` if one was already latched (by an
/// earlier acquisition or a public poll), so a failed acquisition can undo exactly its own
/// latch ([`forget_tty_source_acquisition`]) and nothing else.
#[cfg(unix)]
pub(crate) fn note_tty_source_acquired() -> Option<u8> {
    // Called on every `enter_interactive` (each `Context::poll_event`), so the identity is
    // sampled only on the transition to acquired: after a hangup `isatty(0)` is false, and
    // re-sampling then would recast a hung-up stdin as "not the terminal" (#269 review).
    if TTY_SOURCE.load(Ordering::Acquire) != UNLATCHED {
        return None;
    }
    let identity = if stdin_is_tty() {
        LATCHED_STDIN
    } else {
        LATCHED_DEV_TTY
    };
    TTY_SOURCE
        .compare_exchange(UNLATCHED, identity, Ordering::AcqRel, Ordering::Acquire)
        .ok()
        .map(|_| identity)
}

/// Undoes a latch that [`note_tty_source_acquired`] just made, after the acquisition it was
/// for failed. Only that identity is withdrawn: no source or queued event is touched, and a
/// latch someone else has made since is left alone.
#[cfg(unix)]
pub(crate) fn forget_tty_source_acquisition(identity: u8) {
    let _ = TTY_SOURCE.compare_exchange(identity, UNLATCHED, Ordering::AcqRel, Ordering::Acquire);
}

/// Forgets any latched terminal identity, so a later acquisition re-establishes it.
/// Called when a session relinquishes the terminal (restore), keeping the latch
/// scoped to an interactive lifetime rather than the whole process.
#[cfg(unix)]
pub(crate) fn clear_tty_source() {
    TTY_SOURCE.store(UNLATCHED, Ordering::Release);
    // The terminal source goes with the terminal: a later session reselects its descriptor
    // and does not inherit stale events or a reported hangup.
    crate::tty_source::reset();
}

/// No-op terminal-identity hooks off Unix, where the hangup-aware path and its latch
/// do not exist; the terminal session calls these unconditionally.
#[cfg(not(unix))]
pub(crate) fn note_tty_source_acquired() -> Option<u8> {
    None
}
/// See [`note_tty_source_acquired`].
#[cfg(not(unix))]
pub(crate) fn forget_tty_source_acquisition(_identity: u8) {}
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

/// The hangup-aware Unix path's input identity, read with one atomic load: `Some(true)` to
/// read stdin, `Some(false)` to read `/dev/tty`, `None` for the portable crossterm path. An
/// unlatched source is probed once and latches as stdin iff stdin is a terminal right now, so
/// a later between-polls hangup cannot demote it.
#[cfg(unix)]
fn latched_terminal_source() -> Option<bool> {
    let state = TTY_SOURCE.load(Ordering::Acquire);
    let (use_unix, latched_after) = resolve_input_source(state != UNLATCHED, stdin_is_tty);
    if state == UNLATCHED {
        if !latched_after {
            return None;
        }
        // Latched here only by observing stdin itself as a terminal. A concurrent acquisition
        // may have latched first; whichever identity won is the one used.
        return match TTY_SOURCE.compare_exchange(
            UNLATCHED,
            LATCHED_STDIN,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Some(true),
            Err(won) => Some(won != LATCHED_DEV_TTY),
        };
    }
    debug_assert!(use_unix);
    Some(state == LATCHED_STDIN)
}

#[cfg(all(test, unix))]
mod identity_tests {
    use super::resolve_input_source;
    use std::cell::Cell;

    #[test]
    fn a_failed_acquisition_undoes_only_its_own_latch() {
        // Codex round 4 of #282: a raw-mode failure must not erase a latch (and the source and
        // queued events behind it) that something else established earlier.
        use super::{
            forget_tty_source_acquisition, note_tty_source_acquired, LATCHED_DEV_TTY,
            LATCHED_STDIN, TTY_SOURCE, UNLATCHED,
        };
        use std::sync::atomic::Ordering;
        let saved = TTY_SOURCE.load(Ordering::SeqCst);
        // Pre-existing latch: this acquisition latches nothing, so it has nothing to undo.
        TTY_SOURCE.store(LATCHED_STDIN, Ordering::SeqCst);
        assert_eq!(note_tty_source_acquired(), None);
        forget_tty_source_acquisition(LATCHED_DEV_TTY); // a different identity: no effect
        assert_eq!(TTY_SOURCE.load(Ordering::SeqCst), LATCHED_STDIN);
        // Fresh: the acquisition latches, and undoing it restores unlatched.
        TTY_SOURCE.store(UNLATCHED, Ordering::SeqCst);
        let mine = note_tty_source_acquired().expect("this call latched");
        assert_eq!(TTY_SOURCE.load(Ordering::SeqCst), mine);
        forget_tty_source_acquisition(mine);
        assert_eq!(TTY_SOURCE.load(Ordering::SeqCst), UNLATCHED);
        TTY_SOURCE.store(saved, Ordering::SeqCst);
    }

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

#[cfg(test)]
mod key_kind_tests {
    //! #285: the event type a kitty-protocol terminal reports with a key.
    use super::{
        decode_event, decode_event_as, decode_source_event_as, delivers_key_kind,
        poll_until_deliverable, Event, KeyCode, KeyEvent, KeyModifiers, KeyReleaseReporting,
        SourceEvent, MAX_DROPS_PAST_DEADLINE,
    };
    use crossterm::event::{
        Event as CtEvent, KeyCode as CtKeyCode, KeyEvent as CtKeyEvent, KeyEventKind,
        KeyModifiers as CtKeyModifiers,
    };
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::time::Duration;

    fn ct_key(c: char, kind: KeyEventKind) -> CtEvent {
        CtEvent::Key(CtKeyEvent::new_with_kind(
            CtKeyCode::Char(c),
            CtKeyModifiers::NONE,
            kind,
        ))
    }

    /// An event no platform delivers: F5, a key code LibGibson does not model.
    fn undelivered() -> CtEvent {
        CtEvent::Key(CtKeyEvent::new_with_kind(
            CtKeyCode::F(5),
            CtKeyModifiers::NONE,
            KeyEventKind::Press,
        ))
    }

    fn key(c: char) -> Option<Event> {
        Some(Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::empty(),
        }))
    }

    #[test]
    fn press_and_repeat_are_presses_and_a_release_is_not_delivered() {
        assert_eq!(decode_event(ct_key('a', KeyEventKind::Press)), key('a'));
        assert_eq!(decode_event(ct_key('a', KeyEventKind::Repeat)), key('a'));
        assert_eq!(decode_event(undelivered()), None);
        if !cfg!(windows) {
            assert_eq!(decode_event(ct_key('a', KeyEventKind::Release)), None);
            // A released special key is not delivered either.
            let up = CtEvent::Key(CtKeyEvent::new_with_kind(
                CtKeyCode::Up,
                CtKeyModifiers::NONE,
                KeyEventKind::Release,
            ));
            assert_eq!(decode_event(up), None);
        }
    }

    #[test]
    fn with_key_release_reporting_a_release_is_a_key_release_event() {
        // #275: delivered only while reporting is on, and never on Windows (#292 unchanged).
        let up = || ct_key('a', KeyEventKind::Release);
        let a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty());
        assert_eq!(
            decode_event_as(up(), false, true),
            Some(Event::KeyRelease(a))
        );
        assert_eq!(decode_event_as(up(), false, false), None);
        assert_eq!(decode_event_as(up(), true, true), Some(Event::Key(a)));
        // A press and a repeat are presses either way; an unmodelled key's release is dropped.
        for kind in [KeyEventKind::Press, KeyEventKind::Repeat] {
            assert_eq!(decode_event_as(ct_key('a', kind), false, true), key('a'));
        }
        let f5_up = CtEvent::Key(CtKeyEvent::new_with_kind(
            CtKeyCode::F(5),
            CtKeyModifiers::NONE,
            KeyEventKind::Release,
        ));
        assert_eq!(decode_event_as(f5_up, false, true), None);
        // The answer passes through while reporting is on, and is dropped once it is off (a late
        // answer must not switch a hold model back on).
        let active = || SourceEvent::KeyReleaseReporting(KeyReleaseReporting::Active);
        assert_eq!(
            decode_source_event_as(active(), false, true),
            Some(Event::KeyReleaseReporting(KeyReleaseReporting::Active))
        );
        assert_eq!(decode_source_event_as(active(), false, false), None);
    }

    /// A scripted source: each call pops the next scripted reply and records the `left` it was
    /// given.
    fn scripted(
        replies: Vec<Option<CtEvent>>,
    ) -> (RefCell<VecDeque<Option<CtEvent>>>, RefCell<Vec<Duration>>) {
        (RefCell::new(replies.into()), RefCell::new(Vec::new()))
    }

    #[test]
    fn a_dropped_event_does_not_end_the_poll_and_the_next_one_is_delivered() {
        let (q, lefts) = scripted(vec![
            Some(undelivered()),
            Some(ct_key('b', KeyEventKind::Press)),
        ]);
        let got = poll_until_deliverable(Duration::from_secs(5), |left| {
            lefts.borrow_mut().push(left);
            Ok(q.borrow_mut().pop_front().flatten())
        })
        .unwrap();
        assert_eq!(got, key('b'));
        assert_eq!(lefts.borrow().len(), 2, "the release did not end the poll");
        assert!(
            lefts.borrow()[1] <= lefts.borrow()[0],
            "the deadline is kept"
        );
    }

    #[test]
    fn only_a_source_timeout_reports_no_event() {
        let (q, lefts) = scripted(vec![Some(undelivered()), Some(undelivered()), None]);
        let got = poll_until_deliverable(Duration::ZERO, |left| {
            lefts.borrow_mut().push(left);
            Ok(q.borrow_mut().pop_front().flatten())
        })
        .unwrap();
        assert_eq!(got, None);
        assert_eq!(
            lefts.borrow().len(),
            3,
            "both releases drained, then the timeout"
        );
        assert!(lefts.borrow().iter().all(|l| l.is_zero()));
    }

    #[test]
    fn an_error_after_a_dropped_event_is_reported() {
        let mut calls = 0;
        let got = poll_until_deliverable(Duration::from_millis(50), |_| {
            calls += 1;
            if calls == 1 {
                Ok(Some(undelivered()))
            } else {
                Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
            }
        });
        assert_eq!(got.unwrap_err().kind(), std::io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn a_release_is_dropped_on_unix_and_windows_is_unchanged() {
        for windows in [false, true] {
            assert!(delivers_key_kind(KeyEventKind::Press, windows));
            assert!(delivers_key_kind(KeyEventKind::Repeat, windows));
        }
        assert!(!delivers_key_kind(KeyEventKind::Release, false));
        // Windows keeps its earlier behaviour (see `delivers_key_kind`, #292).
        assert!(delivers_key_kind(KeyEventKind::Release, true));
    }

    #[test]
    fn windows_delivers_a_press_and_its_release_as_two_key_events_292() {
        // KNOWN LIMITATION (#292), pinned as it behaves today, not as it should: with `windows`
        // set, one physical keystroke (crossterm 0.29 sends a Press then a Release) is delivered
        // twice as `Event::Key`, whether or not key-release reporting is enabled. `windows` is a
        // parameter, so this runs on every host and says nothing about a real Windows console.
        for releases in [false, true] {
            let stroke = [
                ct_key('a', KeyEventKind::Press),
                ct_key('a', KeyEventKind::Release),
            ];
            let got: Vec<_> = stroke
                .into_iter()
                .filter_map(|ev| decode_event_as(ev, true, releases))
                .collect();
            assert_eq!(got, vec![key('a').unwrap(), key('a').unwrap()]);
        }
        // The same stroke on Unix is one press.
        let got: Vec<_> = [
            ct_key('a', KeyEventKind::Press),
            ct_key('a', KeyEventKind::Release),
        ]
        .into_iter()
        .filter_map(|ev| decode_event_as(ev, false, false))
        .collect();
        assert_eq!(got, vec![key('a').unwrap()]);
    }

    #[test]
    fn an_endless_stream_of_dropped_events_cannot_hold_a_poll_past_its_deadline() {
        for timeout in [Duration::ZERO, Duration::from_millis(20)] {
            let mut calls = 0u32;
            let t0 = std::time::Instant::now();
            let got = poll_until_deliverable(timeout, |_| {
                calls += 1;
                Ok(Some(undelivered()))
            })
            .unwrap();
            assert_eq!(got, None);
            assert!(t0.elapsed() < timeout + Duration::from_secs(1));
            if timeout.is_zero() {
                assert_eq!(calls, MAX_DROPS_PAST_DEADLINE);
            }
        }
    }

    #[test]
    fn dropped_events_do_not_restart_the_timeout() {
        // Each call takes 30 ms and yields a dropped event; the fourth call must be offered at most
        // what is left of the ORIGINAL deadline, not a fresh timeout.
        let timeout = Duration::from_millis(500);
        let mut lefts = Vec::new();
        let got = poll_until_deliverable(timeout, |left| {
            lefts.push(left);
            if lefts.len() == 4 {
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(30));
            Ok(Some(undelivered()))
        })
        .unwrap();
        assert_eq!(got, None);
        assert_eq!(lefts.len(), 4);
        assert!(
            lefts[3] <= timeout - Duration::from_millis(90),
            "offered {:?} after three 30 ms drops: the timeout restarted",
            lefts[3]
        );
    }

    #[test]
    fn an_unbounded_timeout_is_passed_through_whole() {
        let mut seen = None;
        let got = poll_until_deliverable(Duration::MAX, |left| {
            seen = Some(left);
            Ok(None::<CtEvent>)
        })
        .unwrap();
        assert_eq!(got, None);
        assert_eq!(seen, Some(Duration::MAX));
    }
}

#[cfg(test)]
mod answer_generation_tests {
    //! #275 review 8: an answer acts only in the generation of the request it answers. These
    //! drive the one step that decides it on a local state word, so nothing races and no other
    //! test's delivery switch is touched.
    use super::{hand_out_answer_in, set_delivered_in, Event, KeyReleaseReporting};
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn a_stale_unsupported_changes_nothing_after_a_disable_and_re_enable() {
        // A poller dequeued generation g's `Unsupported`, then stalled; meanwhile the owner
        // disabled and enabled again (a fresh push and query). The old verdict must not end
        // the new request: delivery and generation stay exactly as the owner left them, so the
        // owner's next sync finds nothing to withdraw and pops nothing.
        let state = AtomicU64::new(0);
        set_delivered_in(&state, true);
        let old = state.load(Ordering::SeqCst) >> 1;
        set_delivered_in(&state, false);
        set_delivered_in(&state, true);
        let renewed = state.load(Ordering::SeqCst);
        assert_eq!(
            hand_out_answer_in(&state, KeyReleaseReporting::Unsupported, old),
            None
        );
        assert_eq!(
            hand_out_answer_in(&state, KeyReleaseReporting::Active, old),
            None
        );
        assert_eq!(
            state.load(Ordering::SeqCst),
            renewed,
            "a stale answer changed state"
        );
        assert_eq!(renewed & 1, 1, "delivery still on for the new request");
    }

    #[test]
    fn the_current_unsupported_ends_delivery_once() {
        let state = AtomicU64::new(0);
        set_delivered_in(&state, true);
        let now = state.load(Ordering::SeqCst) >> 1;
        assert_eq!(
            hand_out_answer_in(&state, KeyReleaseReporting::Active, now),
            Some(Event::KeyReleaseReporting(KeyReleaseReporting::Active))
        );
        assert_eq!(
            hand_out_answer_in(&state, KeyReleaseReporting::Unsupported, now),
            Some(Event::KeyReleaseReporting(KeyReleaseReporting::Unsupported))
        );
        let after = state.load(Ordering::SeqCst);
        assert_eq!(after & 1, 0, "delivery ended");
        assert_eq!(after >> 1, now + 1, "and the generation moved, once");
        // A second copy of the same verdict is now stale itself.
        assert_eq!(
            hand_out_answer_in(&state, KeyReleaseReporting::Unsupported, now),
            None
        );
        assert_eq!(state.load(Ordering::SeqCst), after);
    }
}
