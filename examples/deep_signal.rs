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
//! (`gibson::hold::HoldTracker`); deterministic replay from a recorded `(tic, event)` trace
//! through an injected source; capability fallbacks (colour half-blocks, Braille / block /
//! ASCII monochrome) where shape, not colour alone, carries hazards and shards.
//!
//! ```text
//! cargo run --release --example deep_signal                # play (80×24 or larger)
//! cargo run --release --example deep_signal -- --mono      # luminance only (Braille)
//! cargo run --release --example deep_signal -- --ascii     # pure ASCII fallback
//! cargo run --example deep_signal -- --selfcheck           # headless deterministic replay
//! ```
//! Keys: W/S or ↑/↓ move · A/D or ←/→ turn · Z/C strafe · Q or Esc quit.
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
use gibson::input::{Event, KeyCode, KeyEvent, KeyModifiers};
use gibson::node::Node;
use gibson::raster::RgbRaster;
use gibson::surface::{Rect, Surface};
use std::sync::Arc;
use std::time::{Duration, Instant};
use world::{Happening, Intent, Phase, World, TIC_HZ};

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
    Some(match k.code {
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

fn is_quit(e: &Event) -> bool {
    matches!(e, Event::Key(k) if matches!(k.code, KeyCode::Esc)
        || matches!(k.code, KeyCode::Char(c) if c.eq_ignore_ascii_case(&'q')))
}

/// The authoritative simulation: world + held-key model + the trace that reproduces it.
struct Sim {
    world: World,
    holds: HoldTracker<Action>,
    trace: Vec<(u64, Event)>,
    last_message: (u64, &'static str),
}

impl Sim {
    fn new(seed: u64, levels: u32, hold: HoldConfig) -> Self {
        Sim {
            world: World::new(seed, levels),
            holds: HoldTracker::new(hold),
            trace: Vec::new(),
            last_message: (0, ""),
        }
    }

    /// An input event observed while the simulation is at its current tic.
    fn feed(&mut self, e: &Event) {
        let tic = self.world.tic;
        // The tracker never sees time move backwards: `tic` only grows.
        let _ = self.holds.observe_event(tic, e, action_of);
        self.trace.push((tic, e.clone()));
    }

    fn intent(&self) -> Intent {
        let held = |a| self.holds.is_held(a);
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
        for h in self.world.step(self.intent()) {
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
/// HUD layout: `wide` when the terminal has at least 60 columns; `ascii` for pure-ASCII glyphs.
#[derive(Clone, Copy, Debug)]
struct Hud {
    wide: bool,
    ascii: bool,
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
    let bar_len = if hud.wide { 10usize } else { 6 };
    let filled = (pct as usize * bar_len).div_ceil(100).min(bar_len);
    let (full, empty, gem, dot) = if hud.ascii {
        ('#', '-', '*', '|')
    } else {
        ('█', '░', '◆', '·')
    };
    let bar: String = std::iter::repeat_n(full, filled)
        .chain(std::iter::repeat_n(empty, bar_len - filled))
        .collect();
    let secs = w.tic / u64::from(TIC_HZ);
    let got = w.shards_total - w.shards.len() as u32;
    let (total, level, levels) = (w.shards_total, w.level, w.levels);
    let (mm, ss) = (secs / 60, secs % 60);
    let line1 = if hud.wide {
        format!(
            "{gem} SHARDS {got}/{total}  INTEGRITY [{bar}] {pct:>3}%  SECTOR {level}/{levels}  {mm:02}:{ss:02}"
        )
    } else {
        format!("{gem}{got}/{total} [{bar}]{pct:>3}% S{level}/{levels} {mm:02}:{ss:02}")
    };
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
    let line2 = match (phase_msg, hud.wide) {
        (("", _), true) => format!("WASD/ARROWS MOVE {dot} Z/C STRAFE {dot} Q QUIT"),
        (("", _), false) => format!("WASD MOVE {dot} Z/C STRAFE {dot} Q QUIT"),
        ((long, _), true) => long.to_string(),
        ((_, short), false) => short.to_string(),
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
    Node::col()
        .percent_width(100.0)
        .percent_height(100.0)
        .child(view)
        .child(Node::text(line1, accent))
        .child(Node::text(line2, warn))
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
                wide: cols >= 60,
                ascii: look == Look::Mono(SubcellGlyphMode::Ascii),
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
    println!(
        "deep_signal selfcheck: OK world={:016x} tics={} frames={} first_frame={:016x}",
        a.world_digest,
        a.tics,
        a.frames.len(),
        a.frames[0]
    );
    Ok(())
}

struct Opts {
    seed: u64,
    levels: u32,
    look: Look,
    ascii_hud: bool,
    hold: HoldConfig,
    max_seconds: Option<u64>,
    record: Option<String>,
}

fn parse() -> Result<Option<Opts>, String> {
    let mut o = Opts {
        seed: 2112,
        levels: 3,
        look: Look::Color,
        ascii_hud: false,
        hold: HoldConfig::new(9, 4),
        max_seconds: None,
        record: None,
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
            "--hold-grace-ms" => grace_ms = Some(num()?),
            "--hold-repeat-ms" => repeat_ms = Some(num()?),
            "--record" => o.record = it.next(),
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
            Duration::from_millis(grace_ms.unwrap_or(257)),
            Duration::from_millis(repeat_ms.unwrap_or(114)),
        );
    }
    Ok(Some(o))
}

fn play(o: Opts) -> std::io::Result<()> {
    let mut ctx = Context::new(RenderMode::Fullscreen)?;
    if o.look != Look::Color {
        ctx.set_color_depth(ColorDepth::Mono);
    }
    let tic = Duration::from_secs(1) / TIC_HZ;
    let mut clock = FixedTimestep::exact(tic, 4);
    let mut sim = Sim::new(o.seed, o.levels, o.hold);
    let mut cols = crossterm::terminal::size().map(|(c, _)| c).unwrap_or(80);
    let started = Instant::now();
    let mut last = started;
    let (mut presented, mut behind_frames) = (0u64, 0u64);
    let mut outcome: std::io::Result<()> = Ok(());
    'run: loop {
        if o.max_seconds
            .is_some_and(|s| started.elapsed() >= Duration::from_secs(s))
        {
            break;
        }
        let wait = tic.saturating_sub(last.elapsed());
        let mut first = true;
        loop {
            let polled = ctx.poll_event(if first { wait } else { Duration::ZERO });
            first = false;
            match polled {
                Ok(Some(e)) if is_quit(&e) => break 'run,
                Ok(Some(e)) => {
                    if let Event::Resize(c, _) = e {
                        cols = c;
                    }
                    sim.feed(&e);
                }
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
                wide: cols >= 60,
                ascii: o.ascii_hud,
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
    if let Some(path) = &o.record {
        let body: String = sim
            .trace
            .iter()
            .map(|(t, e)| format!("{t}\t{e:?}\n"))
            .collect();
        std::fs::write(path, body)?;
    }
    eprintln!(
        "deep_signal: tics={} presented={} frames_behind={} world={:016x} sector={}/{}",
        sim.world.tic,
        presented,
        behind_frames,
        sim.world.digest(),
        sim.world.level,
        sim.world.levels
    );
    outcome.and(restored)
}

fn main() {
    match parse() {
        Ok(None) => {}
        Ok(Some(o)) => {
            if let Err(e) = play(o) {
                eprintln!("deep_signal: {e}");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("deep_signal: {e}");
            std::process::exit(2);
        }
    }
}
