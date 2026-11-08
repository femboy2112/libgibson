//! DEEP SIGNAL — an original first-person terminal maze, built only on LibGibson's public API.
//!
//! You are a recovery drone in a dead data vault. Collect every signal shard (◆ diamonds), avoid
//! the hatched hazard floors while they pulse live, and reach the violet uplink once it opens.
//! Three levels, each larger than the last.
//!
//! What it exercises (v0.7 nodes B–F): a software framebuffer raycast straight into an
//! `RgbRaster` at the terminal's own sub-cell resolution and lowered through the one renderer
//! (`Node::canvas`); a fixed 35 Hz simulation (`FixedTimestep::exact`) that presentation can
//! fall behind but never changes; held-key inference on a press-only terminal
//! (`gibson::hold::HoldTracker`), released at once when the terminal reports a focus loss;
//! deterministic replay from a recorded `(tic, event)` trace
//! through an injected source; capability fallbacks (colour half-blocks, Braille / block /
//! ASCII monochrome) where shape, not colour alone, carries hazards and shards.
//!
//! ```text
//! cargo run --release --example deep_signal                # play (80×24 or larger)
//! cargo run --release --example deep_signal -- --mono      # luminance only (Braille)
//! cargo run --release --example deep_signal -- --ascii     # pure ASCII fallback
//! cargo run --example deep_signal -- --selfcheck           # headless deterministic replay
//! ```
//! Keys: W/S or ↑/↓ move · A/D or ←/→ turn · Z/C strafe · Q, Esc or Ctrl-C quit.
//!
//! Held keys (#275): a terminal reports presses, not releases, so by default a press is held for
//! a grace period (`--hold-grace-ms`) and kept alive by autorepeat. In a terminal that speaks the
//! kitty keyboard protocol (tested in kitty) the game asks for key releases
//! (`TerminalSession::set_key_release_reporting`); once the terminal confirms, a key is held
//! exactly as long as it is down, keys held together stay held, and the HUD shows `KEYS EXACT`.
//! `--no-key-releases` keeps the inferred model.
//!
//! Every ending that leaves a terminal restores it (node H): quitting; a delivered SIGTERM or
//! SIGINT, which stops the loop, restores, writes the `--record` trace, then re-raises the signal
//! so the parent still sees it - within a 3 s grace, after which a watchdog restores cooked mode
//! and applies the signal's default action anyway (a terminal that stopped reading would
//! otherwise hold the game forever; the alternate screen, hidden cursor and focus reporting need
//! writes, so they stay); a panic (the library's panic hook). SIGHUP keeps its default
//! action: the terminal is gone, and handling it would route shutdown through `poll_event`, which
//! can busy-spin on a terminal that hangs up mid-call (#269); the trace is lost instead.
//! Diagnostics that cannot be written are dropped, never a panic. `--fault-panic-key` is a test
//! hook: with it, `!` makes the next simulation tic panic.
#[path = "deep_signal/render.rs"]
mod render;
#[path = "deep_signal/world.rs"]
mod world;

use gibson::capability::ColorDepth;
use gibson::cell::{Color, Style};
use gibson::clock::FixedTimestep;
use gibson::context::{Context, RenderMode};
use gibson::event_source::InjectedQueue;
use gibson::glitch::tear;
use gibson::glyph::{transcode_surface_glyphs, SubcellGlyphMode};
use gibson::hold::{HoldConfig, HoldTracker, ReleaseCause};
use gibson::input::{Event, KeyCode, KeyEvent, KeyModifiers, KeyReleaseReporting};
use gibson::node::Node;
use gibson::raster::RgbRaster;
use gibson::surface::{Rect, Surface};
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};
use world::{Happening, Intent, Phase, World, TIC_HZ};

/// Most simulation tics run before one presentation when the terminal is slow (2 s at 35 Hz).
const MAX_CATCHUP_TICS: u32 = 2 * TIC_HZ;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Forward,
    Back,
    TurnLeft,
    TurnRight,
    StrafeLeft,
    StrafeRight,
}

fn action_of(k: &KeyEvent) -> Option<Action> {
    // A Ctrl- or Alt-chord is never movement (Ctrl-C quits; Ctrl-Z must not strafe).
    if k.modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return None;
    }
    action_of_key(phys_key(k.code))
}

/// The physical key a movement key event names: letters folded to lowercase, so `W` (shift held)
/// and `w` are one key. Holds are kept per physical key, not per action (#275 review): with
/// releases reported, letting go of W while Up is still down must keep the drone moving forward.
fn phys_key(code: KeyCode) -> KeyCode {
    match code {
        KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
        other => other,
    }
}

/// The hold-tracker key of a movement key event; `None` for anything that is not movement.
fn hold_key(k: &KeyEvent) -> Option<KeyCode> {
    action_of(k).map(|_| phys_key(k.code))
}

fn action_of_key(code: KeyCode) -> Option<Action> {
    Some(match code {
        KeyCode::Up => Action::Forward,
        KeyCode::Down => Action::Back,
        KeyCode::Left => Action::TurnLeft,
        KeyCode::Right => Action::TurnRight,
        KeyCode::Char(c) => match c.to_ascii_lowercase() {
            'w' => Action::Forward,
            's' => Action::Back,
            'a' => Action::TurnLeft,
            'd' => Action::TurnRight,
            'z' => Action::StrafeLeft,
            'c' => Action::StrafeRight,
            _ => return None,
        },
        _ => return None,
    })
}

/// Delivered termination signals, turned into an orderly stop of the game loop.
///
/// The orderly stop (restore the terminal, write the trace, re-raise) runs if it can finish within
/// [`GRACE`](stop_signal::GRACE) of the signal being seen. If it cannot - the loop is blocked in a
/// write to a terminal that stopped reading, or the shutdown is simply that slow - a watchdog puts
/// the terminal's saved line settings back (an ioctl, which a stalled output queue does not block)
/// and applies the signal's default action. Restore and trace are best effort past `GRACE`.
#[cfg(unix)]
mod stop_signal {
    use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
    use std::sync::Mutex;
    use std::time::Duration;

    const SIGNALS: [libc::c_int; 2] = [libc::SIGTERM, libc::SIGINT];

    static PENDING: AtomicI32 = AtomicI32::new(0);
    /// The signals whose handler this module installed (bit `n` = signal `n`).
    static INSTALLED: AtomicU64 = AtomicU64::new(0);
    /// The terminal the game puts in raw mode, and its settings before that, for the watchdog's
    /// last resort. It is the one crossterm uses: stdin when stdin is a terminal, otherwise
    /// `/dev/tty` (opened here and kept open for the watchdog).
    static SAVED_TERMIOS: Mutex<Option<(libc::c_int, libc::termios)>> = Mutex::new(None);

    /// How long after the watchdog sees a stop signal the orderly stop may take.
    pub const GRACE: Duration = Duration::from_secs(3);

    extern "C" fn record(sig: libc::c_int) {
        // Async-signal-safe: one atomic store, nothing else.
        PENDING.store(sig, Ordering::SeqCst);
    }

    fn mask(how: libc::c_int, sigs: &[libc::c_int]) {
        // SAFETY: a valid, initialised sigset; pthread_sigmask on the calling thread.
        unsafe {
            let mut set: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&mut set);
            for &sig in sigs {
                libc::sigaddset(&mut set, sig);
            }
            libc::pthread_sigmask(how, &set, std::ptr::null_mut());
        }
    }

    /// Catches SIGTERM and SIGINT, except one the parent left ignored. SIGHUP is deliberately
    /// left alone (see the module docs and #269). Call before the terminal leaves cooked mode.
    ///
    /// Order matters. Both signals are blocked on this thread while the watchdog is spawned, so
    /// it inherits them blocked and every stop signal is handled on the main thread - where the
    /// handler's store happens-before the final `pending()` check. Handlers are installed only
    /// once the watchdog exists: without it, a caught signal could wait forever, so the default
    /// action stays.
    pub fn install() {
        let wanted: Vec<libc::c_int> = SIGNALS
            .into_iter()
            .filter(|&sig| {
                // SAFETY: querying a disposition into a zeroed struct.
                unsafe {
                    let mut old: libc::sigaction = std::mem::zeroed();
                    libc::sigaction(sig, std::ptr::null(), &mut old) == 0
                        && old.sa_sigaction != libc::SIG_IGN
                }
            })
            .collect();
        if wanted.is_empty() {
            return;
        }
        // SAFETY: isatty/open/tcgetattr/close on descriptors this function owns or stdin, with a
        // zeroed termios; every failure just leaves nothing saved.
        unsafe {
            let fd = if libc::isatty(libc::STDIN_FILENO) == 1 {
                libc::STDIN_FILENO
            } else {
                libc::open(
                    c"/dev/tty".as_ptr(),
                    libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC,
                )
            };
            if fd >= 0 {
                let mut t: libc::termios = std::mem::zeroed();
                if libc::tcgetattr(fd, &mut t) == 0 {
                    *SAVED_TERMIOS.lock().unwrap_or_else(|e| e.into_inner()) = Some((fd, t));
                } else if fd != libc::STDIN_FILENO {
                    libc::close(fd);
                }
            }
        }
        mask(libc::SIG_BLOCK, &wanted);
        let watchdog = std::thread::Builder::new()
            .name("stop-watchdog".into())
            .spawn(|| loop {
                std::thread::sleep(Duration::from_millis(20));
                if let Some(sig) = pending() {
                    std::thread::sleep(GRACE);
                    // The orderly stop did not finish in time. Put the line settings back (an
                    // ioctl: unlike the escape sequences, it does not wait on a stalled output
                    // queue), then die by the signal.
                    let saved = *SAVED_TERMIOS.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some((fd, t)) = saved {
                        // SAFETY: restoring a termios read from this same, still-open descriptor.
                        unsafe { libc::tcsetattr(fd, libc::TCSANOW, &t) };
                    }
                    reraise(sig);
                }
            });
        if watchdog.is_ok() {
            for &sig in &wanted {
                // SAFETY: sigaction with a zeroed struct; the handler only stores to an atomic.
                unsafe {
                    let mut sa: libc::sigaction = std::mem::zeroed();
                    sa.sa_sigaction = record as extern "C" fn(libc::c_int) as libc::sighandler_t;
                    sa.sa_flags = libc::SA_RESTART;
                    libc::sigemptyset(&mut sa.sa_mask);
                    if libc::sigaction(sig, &sa, std::ptr::null_mut()) == 0 {
                        INSTALLED.fetch_or(1 << sig, Ordering::SeqCst);
                    }
                }
            }
        }
        // A signal that arrived meanwhile was held pending; it is delivered now, to a handler if
        // one was installed, otherwise by its default action.
        mask(libc::SIG_UNBLOCK, &wanted);
    }

    /// Puts back the default action of every signal this module caught, so one that arrives
    /// after the final [`pending`] check still ends the process instead of being recorded and
    /// never acted on.
    pub fn uninstall() {
        let installed = INSTALLED.swap(0, Ordering::SeqCst);
        for sig in SIGNALS {
            if installed & (1 << sig) != 0 {
                // SAFETY: restoring the default disposition of a signal we installed.
                unsafe {
                    libc::signal(sig, libc::SIG_DFL);
                }
            }
        }
    }

    pub fn pending() -> Option<i32> {
        match PENDING.load(Ordering::SeqCst) {
            0 => None,
            sig => Some(sig),
        }
    }

    /// Dies by `sig` with its default action, so the parent sees the real cause of death. Works
    /// from either thread: the signal is unblocked on the calling thread before it is raised.
    pub fn reraise(sig: i32) -> ! {
        // SAFETY: restoring the default disposition and raising a signal at ourselves.
        unsafe {
            libc::signal(sig, libc::SIG_DFL);
        }
        mask(libc::SIG_UNBLOCK, &[sig]);
        // SAFETY: raising a signal at the calling thread.
        unsafe {
            libc::raise(sig);
        }
        std::process::exit(128 + sig)
    }
}

#[cfg(not(unix))]
mod stop_signal {
    pub fn install() {}
    pub fn uninstall() {}
    pub fn pending() -> Option<i32> {
        None
    }
    pub fn reraise(sig: i32) -> ! {
        std::process::exit(128 + sig)
    }
}

/// Writes a diagnostic line to stderr. Unlike `eprintln!`, a terminal that has hung up (EIO)
/// drops the line instead of panicking.
fn diag(line: std::fmt::Arguments) {
    let _ = writeln!(std::io::stderr(), "{line}");
}

fn is_quit(e: &Event) -> bool {
    let Event::Key(k) = e else { return false };
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    match k.code {
        KeyCode::Esc => true,
        KeyCode::Char(c) if ctrl => c.eq_ignore_ascii_case(&'c'),
        KeyCode::Char(c) => c.eq_ignore_ascii_case(&'q'),
        _ => false,
    }
}

/// The replayable trace (`--record` / `--replay`), version 1: a header with everything the
/// simulation depends on, one line per simulation-relevant input stamped with its tic, and the
/// tic the session ended at. Inputs that cannot change the simulation (paste, mouse, resize)
/// are not recorded. A key release and the terminal's answer to the key-release request (#275)
/// are recorded too (`release` and `keys` lines), so a session with exact keys replays exactly.
const TRACE_MAGIC: &str = "deep_signal-trace v1";

fn encode_key(kind: &str, k: &KeyEvent) -> Option<String> {
    let ctrl = u8::from(k.modifiers.contains(KeyModifiers::CONTROL));
    let alt = u8::from(k.modifiers.contains(KeyModifiers::ALT));
    let code = match k.code {
        KeyCode::Char(c) => format!("char:{}", u32::from(c)),
        KeyCode::Up => "up".into(),
        KeyCode::Down => "down".into(),
        KeyCode::Left => "left".into(),
        KeyCode::Right => "right".into(),
        _ => return None, // no other key affects the simulation
    };
    Some(format!("{kind}\t{code}\t{ctrl}{alt}"))
}

fn encode_event(e: &Event) -> Option<String> {
    match e {
        Event::Key(k) => encode_key("key", k),
        Event::KeyRelease(k) => encode_key("release", k),
        Event::KeyReleaseReporting(KeyReleaseReporting::Active) => Some("keys\treleases".into()),
        Event::KeyReleaseReporting(_) => Some("keys\tpresses".into()),
        Event::Focus(gibson::input::FocusChange::Lost) => Some("focus\tlost".into()),
        _ => None,
    }
}

fn decode_event(fields: &[&str]) -> Result<Event, String> {
    match fields {
        [kind @ ("key" | "release"), code, mods] => {
            let code = match *code {
                "up" => KeyCode::Up,
                "down" => KeyCode::Down,
                "left" => KeyCode::Left,
                "right" => KeyCode::Right,
                c => {
                    let n: u32 = c
                        .strip_prefix("char:")
                        .and_then(|v| v.parse().ok())
                        .ok_or("bad key")?;
                    KeyCode::Char(char::from_u32(n).ok_or("bad char")?)
                }
            };
            let mut m = KeyModifiers::empty();
            if mods.starts_with('1') {
                m |= KeyModifiers::CONTROL;
            }
            if mods.ends_with('1') {
                m |= KeyModifiers::ALT;
            }
            let k = KeyEvent::new(code, m);
            Ok(if *kind == "key" {
                Event::Key(k)
            } else {
                Event::KeyRelease(k)
            })
        }
        ["keys", "releases"] => Ok(Event::KeyReleaseReporting(KeyReleaseReporting::Active)),
        ["keys", "presses"] => Ok(Event::KeyReleaseReporting(KeyReleaseReporting::Unsupported)),
        ["focus", "lost"] => Ok(Event::Focus(gibson::input::FocusChange::Lost)),
        other => Err(format!("unrecognised trace line {other:?}")),
    }
}

/// Replays a recorded trace headlessly to its end tic and returns the final world.
fn replay_trace(text: &str) -> Result<(World, u64), String> {
    let mut lines = text.lines();
    let header = lines.next().ok_or("empty trace")?;
    let mut head = header.split('\t');
    if head.next() != Some(TRACE_MAGIC) {
        return Err(format!("not a {TRACE_MAGIC} file"));
    }
    let mut get = |key: &str| -> Result<u64, String> {
        head.next()
            .and_then(|f| f.strip_prefix(key))
            .and_then(|v| v.parse().ok())
            .ok_or(format!("header lacks {key}"))
    };
    let (seed, levels, grace, repeat) = (
        get("seed=")?,
        get("levels=")?,
        get("grace=")?,
        get("repeat=")?,
    );
    let mut sim = Sim::new(seed, levels as u32, HoldConfig::new(grace, repeat));
    let mut events: Vec<(u64, Event)> = Vec::new();
    let mut end = None;
    for line in lines {
        let fields: Vec<&str> = line.split('\t').collect();
        match fields.as_slice() {
            ["end", t] => end = Some(t.parse::<u64>().map_err(|_| "bad end tic")?),
            [t, rest @ ..] => events.push((t.parse().map_err(|_| "bad tic")?, decode_event(rest)?)),
            [] => {}
        }
    }
    let end = end.ok_or("trace has no end line")?;
    // A recorded trace is in tic order and ends at or after its last input; anything else was
    // edited by hand and is refused rather than partly replayed.
    if let Some(w) = events.windows(2).find(|w| w[1].0 < w[0].0) {
        return Err(format!(
            "trace tics out of order: {} after {}",
            w[1].0, w[0].0
        ));
    }
    if let Some(&(t, _)) = events.last() {
        if t > end {
            return Err(format!("input at tic {t} after the end tic {end}"));
        }
    }
    let mut i = 0;
    while sim.world.tic < end {
        while i < events.len() && events[i].0 == sim.world.tic {
            sim.feed(&events[i].1.clone());
            i += 1;
        }
        sim.tick();
    }
    Ok((sim.world, sim.path))
}

/// The authoritative simulation: world + held-key model + the trace that reproduces it.
struct Sim {
    world: World,
    /// Held movement keys, per physical key ([`phys_key`]).
    holds: HoldTracker<KeyCode>,
    trace: Vec<(u64, Event)>,
    last_message: (u64, &'static str),
    /// Rolling hash of the trajectory (tic, position, angle, integrity after every tic): two
    /// runs agree on it only if they agree at every tic, not merely at the end.
    path: u64,
}

impl Sim {
    fn new(seed: u64, levels: u32, hold: HoldConfig) -> Self {
        Sim {
            world: World::new(seed, levels),
            holds: HoldTracker::new(hold),
            trace: Vec::new(),
            last_message: (0, ""),
            path: 0xcbf2_9ce4_8422_2325,
        }
    }

    /// An input event observed while the simulation is at its current tic.
    fn feed(&mut self, e: &Event) {
        let tic = self.world.tic;
        // The tracker never sees time move backwards: `tic` only grows.
        let _ = self.holds.observe_event(tic, e, hold_key);
        self.trace.push((tic, e.clone()));
    }

    /// The intent the next tick will use: holds advanced to the current tic first, as
    /// [`Sim::tick`] does.
    fn intent_after_advance(&mut self) -> Intent {
        let _ = self.holds.advance(self.world.tic);
        self.intent()
    }

    fn intent(&self) -> Intent {
        let held = |a| self.holds.held().any(|(k, _)| action_of_key(k) == Some(a));
        let axis = |pos, neg| i32::from(held(pos)) - i32::from(held(neg));
        Intent {
            forward: axis(Action::Forward, Action::Back),
            strafe: axis(Action::StrafeRight, Action::StrafeLeft),
            turn: axis(Action::TurnRight, Action::TurnLeft),
        }
    }

    /// Exactly one simulation tic.
    fn tick(&mut self) {
        let _ = self.holds.advance(self.world.tic);
        let happenings = self.world.step(self.intent());
        let w = &self.world;
        for v in [
            w.tic,
            w.x as u64,
            w.y as u64,
            w.angle as u64,
            u64::from(w.integrity),
        ] {
            for b in v.to_le_bytes() {
                self.path = (self.path ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
            }
        }
        for h in happenings {
            let msg = match h {
                Happening::ShardTaken { left: 0 } => "UPLINK OPEN",
                Happening::ShardTaken { .. } => "SHARD RECOVERED",
                Happening::ExitOpened => "UPLINK OPEN",
                Happening::Damaged { .. } => "HAZARD - INTEGRITY DRAINING",
                Happening::LevelEntered { .. } => "NEW SECTOR",
                Happening::SignalLost => "SIGNAL LOST",
                Happening::Cleared => "ALL SECTORS CLEARED",
            };
            self.last_message = (self.world.tic, msg);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Look {
    Color,
    Mono(SubcellGlyphMode),
}

/// The full frame for one presentation: view (raycast into a raster sized to the laid-out rect)
/// plus a two-line HUD. Built from an immutable world snapshot.
/// HUD glyph set: `ascii` for pure-ASCII glyphs. The wide-or-compact choice is made at layout
/// time from the HUD's actual width (>= 60 columns is wide), never from resize events, so it
/// cannot lag the real geometry when a resize event arrives late.
#[derive(Clone, Copy, Debug)]
struct Hud {
    ascii: bool,
    /// The terminal reports key releases (#275): keys are held exactly as long as they are down.
    exact_keys: bool,
}

fn frame(world: Arc<World>, look: Look, message: &'static str, hud: Hud) -> Node {
    let snap = Arc::clone(&world);
    let view = Node::canvas(move |rect: Rect| -> Surface {
        let mut r = RgbRaster::new(rect.width, rect.height.saturating_mul(2));
        render::paint_view(&snap, &mut r);
        let mut s = match look {
            Look::Color => r.to_surface(),
            Look::Mono(mode) => {
                let mut s = r.to_mono_surface();
                if mode != SubcellGlyphMode::Braille2x4 {
                    transcode_surface_glyphs(&mut s, mode);
                }
                s
            }
        };
        if let Some(seed) = render::tear_seed(&snap) {
            let area = Rect {
                x: 0,
                y: 0,
                width: s.width,
                height: s.height,
            };
            tear(&mut s, area, seed);
        }
        s
    })
    .flex_grow(1.0)
    .percent_width(100.0);
    let w = &*world;
    let pct = w.integrity * 100 / World::max_integrity();
    let (full, empty, gem, dot) = if hud.ascii {
        ('#', '-', '*', '|')
    } else {
        ('█', '░', '◆', '·')
    };
    let bar = |len: usize| -> String {
        let filled = (pct as usize * len).div_ceil(100).min(len);
        std::iter::repeat_n(full, filled)
            .chain(std::iter::repeat_n(empty, len - filled))
            .collect()
    };
    let secs = w.tic / u64::from(TIC_HZ);
    let got = w.shards_total - w.shards.len() as u32;
    let (total, level, levels) = (w.shards_total, w.level, w.levels);
    let (mm, ss) = (secs / 60, secs % 60);
    let wide1 = format!(
        "{gem} SHARDS {got}/{total}  INTEGRITY [{}] {pct:>3}%  SECTOR {level}/{levels}  {mm:02}:{ss:02}",
        bar(10)
    );
    let compact1 = format!(
        "{gem}{got}/{total} [{}]{pct:>3}% S{level}/{levels} {mm:02}:{ss:02}",
        bar(6)
    );
    // (wide, compact) texts; compact ones fit 40 columns.
    let phase_msg = match w.phase {
        Phase::Descending { .. } => ("DESCENDING TO THE NEXT SECTOR", "DESCENDING"),
        Phase::SignalLost { .. } => ("SIGNAL LOST - REBOOTING SECTOR", "SIGNAL LOST"),
        Phase::Clear => ("ALL SECTORS CLEARED - Q TO EXIT", "CLEARED - Q TO EXIT"),
        Phase::Playing if w.exit_open() => {
            ("UPLINK OPEN - FIND THE VIOLET APERTURE", "UPLINK OPEN")
        }
        Phase::Playing => (message, message),
    };
    let exact = if hud.exact_keys {
        format!(" {dot} KEYS EXACT")
    } else {
        String::new()
    };
    let (wide2, compact2) = match phase_msg {
        ("", _) => (
            format!("WASD/ARROWS MOVE {dot} Z/C STRAFE {dot} Q QUIT{exact}"),
            format!("WASD MOVE {dot} Z/C STRAFE {dot} Q QUIT{exact}"),
        ),
        (long, short) => (long.to_string(), short.to_string()),
    };
    let mono = look != Look::Color;
    let accent = if mono {
        Style::default()
    } else {
        Style::default().fg(Color::Rgb(120, 240, 210))
    };
    let warn = if mono {
        Style::default()
    } else if pct < 35 {
        Style::default().fg(Color::Rgb(255, 90, 60))
    } else {
        Style::default().fg(Color::Rgb(250, 220, 140))
    };
    let hud_node = Node::canvas(move |rect: Rect| -> Surface {
        let mut s = Surface::new(rect.width, rect.height);
        let (l1, l2) = if rect.width >= 60 {
            (&wide1, &wide2)
        } else {
            (&compact1, &compact2)
        };
        s.print_str(0, 0, l1, accent, Some(rect.width));
        s.print_str(0, 1, l2, warn, Some(rect.width));
        s
    })
    .height(2.0)
    .percent_width(100.0);
    Node::col()
        .percent_width(100.0)
        .percent_height(100.0)
        .child(view)
        .child(hud_node)
}

/// A scripted session for the self-check: hold keys with terminal-like autorepeat.
fn demo_trace() -> Vec<(u64, Event)> {
    let key = |c: char| Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()));
    let mut t = 0u64;
    let mut out = Vec::new();
    let hold = |c: char, tics: u64, out: &mut Vec<(u64, Event)>, t: &mut u64| {
        // Press, then autorepeat every 2 tics after a 14-tic delay.
        out.push((*t, key(c)));
        let mut k = 14;
        while k < tics {
            out.push((*t + k, key(c)));
            k += 2;
        }
        *t += tics + 12;
    };
    for (c, n) in [
        ('w', 70),
        ('d', 22),
        ('w', 60),
        ('a', 30),
        ('w', 90),
        ('z', 20),
        ('d', 40),
        ('w', 120),
        ('c', 16),
        ('a', 50),
        ('w', 140),
    ] {
        hold(c, n, &mut out, &mut t);
    }
    out.push((t / 2, Event::Resize(120, 40)));
    out.push((t / 3, Event::Focus(gibson::input::FocusChange::Lost)));
    out.sort_by_key(|(tic, _)| *tic);
    out
}

struct Replay {
    world_digest: u64,
    frames: Vec<u64>,
    tics: u64,
    moved: bool,
    /// Cells of the view (all rows but the 2-line HUD) the renderer reports as painted.
    view_cells: usize,
}

/// Headless deterministic replay: inject the trace tic by tic through an `InjectedQueue`,
/// step the simulation, present every `cadence` tics into a headless context.
fn replay(
    trace: &[(u64, Event)],
    tics: u64,
    cadence: u64,
    look: Look,
    cols: u16,
    rows: u16,
) -> Replay {
    let mut sim = Sim::new(2112, 3, HoldConfig::new(16, 4));
    let mut ctx = Context::headless(RenderMode::Fullscreen, cols, rows);
    if look != Look::Color {
        ctx.set_color_depth(ColorDepth::Mono);
    }
    let mut q = InjectedQueue::with_capacity(64);
    let start = (sim.world.x, sim.world.y);
    let mut frames = Vec::new();
    let mut i = 0;
    while sim.world.tic < tics {
        while i < trace.len() && trace[i].0 == sim.world.tic {
            q.push(trace[i].1.clone()).expect("queue sized for one tic");
            i += 1;
        }
        while let Some(e) = ctx
            .poll_from(&mut q, Duration::ZERO)
            .expect("injected poll")
        {
            sim.feed(&e);
        }
        sim.tick();
        if sim.world.tic % cadence == 0 {
            let msg = sim.last_message.1;
            let hud = Hud {
                ascii: look == Look::Mono(SubcellGlyphMode::Ascii),
                exact_keys: sim.holds.releases_reported(),
            };
            ctx.set_root(frame(Arc::new(sim.world.clone()), look, msg, hud));
            ctx.render().expect("headless render");
            frames.push(fnv(ctx.take_output().as_bytes()));
        }
    }
    let view_cells = (0..rows.saturating_sub(2))
        .flat_map(|y| (0..cols).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            ctx.frame_cell(x, y)
                .is_some_and(|c| !c.glyph.trim().is_empty())
        })
        .count();
    Replay {
        view_cells,
        world_digest: sim.world.digest(),
        frames,
        tics: sim.world.tic,
        moved: (sim.world.x, sim.world.y) != start || sim.world.level > 1,
    }
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn selfcheck() -> Result<(), String> {
    let trace = demo_trace();
    let a = replay(&trace, 900, 2, Look::Color, 80, 24);
    let b = replay(&trace, 900, 2, Look::Color, 80, 24);
    if a.frames != b.frames || a.world_digest != b.world_digest {
        return Err("two identical replays diverged".into());
    }
    if !a.moved || a.tics != 900 {
        return Err(format!("vacuous replay: moved={} tics={}", a.moved, a.tics));
    }
    // The 80x24 view is 80x22 cells; nearly all of it must be painted (not a collapsed canvas).
    if a.view_cells < 80 * 22 * 9 / 10 {
        return Err(format!(
            "the view painted only {} of {} cells",
            a.view_cells,
            80 * 22
        ));
    }
    for cadence in [1, 3, 7] {
        let c = replay(&trace, 900, cadence, Look::Color, 80, 24);
        if c.world_digest != a.world_digest {
            return Err(format!(
                "presentation cadence {cadence} changed the simulation"
            ));
        }
    }
    let mono = replay(&trace, 900, 5, Look::Mono(SubcellGlyphMode::Ascii), 40, 15);
    if mono.world_digest != a.world_digest {
        return Err("40x15 ASCII presentation changed the simulation".into());
    }
    let exact = selfcheck_reported_releases()?;
    println!(
        "deep_signal selfcheck: OK world={:016x} tics={} frames={} first_frame={:016x} exact_keys={:016x}",
        a.world_digest,
        a.tics,
        a.frames.len(),
        a.frames[0],
        exact
    );
    Ok(())
}

/// The #275 model, end to end through the game: once the terminal confirms key releases, a key
/// moves the drone exactly while it is down, a chord holds both keys, and a tap inside one tic
/// moves it one tic. The trace also survives the `--record` encoding unchanged, and replaying
/// it through a headless context (as `replay` does) gives the same world.
fn selfcheck_reported_releases() -> Result<u64, String> {
    let key = |c: char| KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty());
    // w down 10..100; d (turn right) down 20..40 while w stays down, autorepeating; a tap of a
    // (turn left) pressed and released inside tic 120.
    let mut trace = vec![(0, Event::KeyReleaseReporting(KeyReleaseReporting::Active))];
    trace.push((10, Event::Key(key('w'))));
    trace.push((20, Event::Key(key('d'))));
    trace.extend((34..40).map(|t| (t, Event::Key(key('d')))));
    trace.push((40, Event::KeyRelease(key('d'))));
    trace.push((100, Event::KeyRelease(key('w'))));
    trace.push((120, Event::Key(key('a'))));
    trace.push((120, Event::KeyRelease(key('a'))));
    // Two keys for one action: w down 130..160 and Up down 140..170; forward until both are up.
    let up = KeyEvent::new(KeyCode::Up, KeyModifiers::empty());
    trace.push((130, Event::Key(key('w'))));
    trace.push((140, Event::Key(up)));
    trace.push((160, Event::KeyRelease(key('w'))));
    trace.push((170, Event::KeyRelease(up)));
    let end = 180;
    let mut sim = Sim::new(2112, 3, HoldConfig::new(16, 4));
    let mut i = 0;
    while sim.world.tic < end {
        let t = sim.world.tic;
        while i < trace.len() && trace[i].0 == t {
            sim.feed(&trace[i].1.clone());
            i += 1;
        }
        let want = Intent {
            forward: i32::from((10..100).contains(&t) || (130..170).contains(&t)),
            strafe: 0,
            turn: i32::from((20..40).contains(&t)) - i32::from(t == 120),
        };
        let got = sim.intent_after_advance();
        if (got.forward, got.strafe, got.turn) != (want.forward, want.strafe, want.turn) {
            return Err(format!(
                "reported releases: tic {t}: intent {:?}, want {:?}",
                (got.forward, got.strafe, got.turn),
                (want.forward, want.strafe, want.turn)
            ));
        }
        sim.tick();
    }
    for (_, e) in &trace {
        let line = encode_event(e).ok_or("an exact-keys event is not recorded")?;
        let fields: Vec<&str> = line.split('\t').collect();
        if decode_event(&fields)? != *e {
            return Err(format!("trace line {line:?} does not decode to {e:?}"));
        }
    }
    let replayed = replay(&trace, end, 1, Look::Color, 80, 24);
    if replayed.world_digest != sim.world.digest() {
        return Err("exact keys through a headless context changed the simulation".into());
    }
    Ok(sim.world.digest())
}

struct Opts {
    seed: u64,
    levels: u32,
    look: Look,
    ascii_hud: bool,
    /// Ask the terminal for key releases (#275); `--no-key-releases` turns it off.
    key_releases: bool,
    hold: HoldConfig,
    max_seconds: Option<u64>,
    record: Option<String>,
    fault_panic_key: bool,
}

fn parse() -> Result<Option<Opts>, String> {
    let mut o = Opts {
        seed: 2112,
        levels: 3,
        look: Look::Color,
        ascii_hud: false,
        key_releases: true,
        // 25 tics (714 ms) of initial grace bridges the autorepeat delays measured through a
        // real terminal (#275): first repeat 18 tics after the press at X/MATE 500 ms / 30 Hz,
        // 23 tics at X's default 660 ms / 25 Hz. The cost: a single tap moves for 25 tics,
        // because a press-only terminal cannot tell a tap from the start of a hold. 4 tics
        // (114 ms) between repeats covers the measured 1-2 tic gaps. `--hold-grace-ms` chooses.
        hold: HoldConfig::new(25, 4),
        max_seconds: None,
        record: None,
        fault_panic_key: false,
    };
    let (mut grace_ms, mut repeat_ms) = (None, None);
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut num = || {
            it.next()
                .and_then(|v| v.parse::<u64>().ok())
                .ok_or(format!("{a} needs a number"))
        };
        match a.as_str() {
            "--selfcheck" => {
                selfcheck()?;
                return Ok(None);
            }
            "--seed" => o.seed = num()?,
            "--levels" => o.levels = num()? as u32,
            "--mono" => o.look = Look::Mono(SubcellGlyphMode::Braille2x4),
            "--block" => o.look = Look::Mono(SubcellGlyphMode::Block),
            "--ascii" => {
                o.look = Look::Mono(SubcellGlyphMode::Ascii);
                o.ascii_hud = true;
            }
            "--max-seconds" => o.max_seconds = Some(num()?),
            "--no-key-releases" => o.key_releases = false,
            "--hold-grace-ms" => grace_ms = Some(num()?),
            "--hold-repeat-ms" => repeat_ms = Some(num()?),
            "--record" => o.record = it.next(),
            "--fault-panic-key" => o.fault_panic_key = true,
            "--replay" => {
                let path = it.next().ok_or("--replay needs a path")?;
                let text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
                let (w, path) = replay_trace(&text)?;
                println!(
                    "deep_signal replay: tics={} world={:016x} path={path:016x} sector={}/{}",
                    w.tic,
                    w.digest(),
                    w.level,
                    w.levels
                );
                return Ok(None);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if std::env::var_os("NO_COLOR").is_some() && o.look == Look::Color {
        o.look = Look::Mono(SubcellGlyphMode::Braille2x4);
    }
    if grace_ms.is_some() || repeat_ms.is_some() {
        let tic = Duration::from_secs(1) / TIC_HZ;
        o.hold = HoldConfig::from_durations(
            tic,
            Duration::from_millis(grace_ms.unwrap_or(714)),
            Duration::from_millis(repeat_ms.unwrap_or(114)),
        );
    }
    Ok(Some(o))
}

fn play(o: Opts) -> std::io::Result<()> {
    stop_signal::install();
    let mut ctx = Context::new(RenderMode::Fullscreen)?;
    // Focus reporting, through the session so restore, Drop and the panic hook turn it off
    // again. Raw mode first, so an immediate focus report is never echoed by a cooked terminal.
    // A lost focus releases every held key at once (`ReleaseCause::FocusLost`): the terminal
    // stops autorepeating, but the key may still be down.
    ctx.session.enter_interactive()?;
    ctx.session.set_focus_reporting(true)?;
    // Key releases (#275), through the session so every exit path pops the keyboard flags. The
    // terminal's answer arrives as an event; until then (and on a terminal that never confirms)
    // holds are inferred.
    if o.key_releases {
        ctx.session.set_key_release_reporting(true)?;
    }
    if o.look != Look::Color {
        ctx.set_color_depth(ColorDepth::Mono);
    }
    let tic = Duration::from_secs(1) / TIC_HZ;
    // Backpressure policy (node F): the simulation keeps wall-clock pace and presentation drops
    // frames. A slow terminal stretches one loop iteration (the write is synchronous), so the next
    // advance may owe many tics; up to MAX_CATCHUP_TICS (2 s) run before the next frame, the rest
    // are deferred - never discarded. A terminal that stops reading entirely stalls the loop.
    let mut clock = FixedTimestep::exact(tic, MAX_CATCHUP_TICS);
    let mut sim = Sim::new(o.seed, o.levels, o.hold);
    let started = Instant::now();
    let mut last = started;
    let (mut presented, mut behind_frames) = (0u64, 0u64);
    let mut outcome: std::io::Result<()> = Ok(());
    let mut fault_armed = false;
    'run: loop {
        if stop_signal::pending().is_some() {
            break;
        }
        if o.max_seconds
            .is_some_and(|s| started.elapsed() >= Duration::from_secs(s))
        {
            break;
        }
        let wait = tic.saturating_sub(last.elapsed());
        let mut first = true;
        loop {
            // Also here: a steady stream of input must not keep a stop request waiting.
            if stop_signal::pending().is_some() {
                break 'run;
            }
            let polled = ctx.poll_event(if first { wait } else { Duration::ZERO });
            first = false;
            match polled {
                Ok(Some(e)) if is_quit(&e) => break 'run,
                Ok(Some(Event::Key(k))) if o.fault_panic_key && k.code == KeyCode::Char('!') => {
                    fault_armed = true;
                }
                Ok(Some(e)) => sim.feed(&e),
                Ok(None) => break,
                Err(e) => {
                    // Input source gone (hangup): no further observation is possible.
                    let _ = sim.holds.release_all(ReleaseCause::Disconnected);
                    outcome = Err(e);
                    break 'run;
                }
            }
        }
        let now = Instant::now();
        let demand = clock.advance(now - last);
        last = now;
        for _ in 0..demand.executed {
            sim.tick();
            if fault_armed {
                panic!(
                    "deep_signal: injected fault at tic {} (--fault-panic-key)",
                    sim.world.tic
                );
            }
        }
        if demand.executed > 0 {
            if demand.executed > 1 {
                behind_frames += 1;
            }
            let msg = if sim.world.tic.saturating_sub(sim.last_message.0) < u64::from(TIC_HZ) * 2 {
                sim.last_message.1
            } else {
                ""
            };
            let hud = Hud {
                ascii: o.ascii_hud,
                exact_keys: sim.holds.releases_reported(),
            };
            ctx.set_root(frame(Arc::new(sim.world.clone()), o.look, msg, hud));
            if let Err(e) = ctx.render() {
                outcome = Err(e);
                break;
            }
            presented += 1;
        }
    }
    let restored = ctx.restore();
    let mut recorded = Ok(());
    if let Some(path) = &o.record {
        let mut body = format!(
            "{TRACE_MAGIC}\tseed={}\tlevels={}\tgrace={}\trepeat={}\n",
            o.seed,
            o.levels,
            o.hold.initial_grace(),
            o.hold.repeat_timeout()
        );
        for (t, e) in &sim.trace {
            if let Some(enc) = encode_event(e) {
                body.push_str(&format!("{t}\t{enc}\n"));
            }
        }
        body.push_str(&format!("end\t{}\n", sim.world.tic));
        recorded = std::fs::write(path, body);
    }
    diag(format_args!(
        "deep_signal: tics={} presented={} frames_behind={} wall_ms={} world={:016x} path={:016x} sector={}/{}",
        sim.world.tic,
        presented,
        behind_frames,
        started.elapsed().as_millis(),
        sim.world.digest(),
        sim.path,
        sim.world.level,
        sim.world.levels
    ));
    // The first failure wins: the loop's, then the restore's, then the trace write's.
    let result = outcome.and(restored).and(recorded);
    // From here on a stop signal takes its default action; one recorded before now is acted on
    // just below.
    stop_signal::uninstall();
    if let Some(sig) = stop_signal::pending() {
        // Terminal restored and trace written: now die by the signal that asked us to stop.
        if let Err(e) = result {
            diag(format_args!("deep_signal: {e}"));
        }
        stop_signal::reraise(sig);
    }
    result
}

fn main() {
    match parse() {
        Ok(None) => {}
        Ok(Some(o)) => {
            if let Err(e) = play(o) {
                diag(format_args!("deep_signal: {e}"));
                // play() returned early (startup failed, say) with the handlers still installed:
                // a stop signal recorded before now is still the cause of death, and one arriving
                // from here on takes its default action.
                stop_signal::uninstall();
                if let Some(sig) = stop_signal::pending() {
                    stop_signal::reraise(sig);
                }
                std::process::exit(1);
            }
        }
        Err(e) => {
            diag(format_args!("deep_signal: {e}"));
            std::process::exit(2);
        }
    }
}
