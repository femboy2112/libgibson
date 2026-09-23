//! HACK THE GIBSON — maximalist full-screen Gibson terminal.
//!
//! Owns the framebuffer and renders a live dashboard: a gradient/shimmer banner,
//! mainframe gauges, an animated `garbage.bin` hex dump, a sweeping Da Vinci
//! scan, a network route animation, a scrolling trace feed, a big gradient
//! transfer meter with a throughput sparkline, a tactical payload selector and a
//! root shell. Zero raw ANSI literals — everything is RichText/Span/Theme.
//!
//! `--auto` is deterministic; `--inline` uses the scrollback path; `--light`,
//! `--dark`, `--no-color` are theme proofs.

use gibson::cell::{Color, Line, RichText, Span, Style, Theme};
use gibson::context::Context;
use gibson::input::{Event, KeyCode, KeyModifiers, TextInputState};
use gibson::node::{Node, WrapMode};
use gibson::show;
use gibson::story::{Beat, Condition, Story, StoryAction, StoryDirector, StoryEvent};
use gibson::{
    BorderType, BrailleCanvas, Mesh, ParticleSystem, Projector, Replication, ThemeStyles,
    TimeSource, Transform3, Vec3,
};
use std::collections::HashMap;
use std::env;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Neon effects
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Fx {
    color: bool,
    /// Electric blue.
    blue: Color,
    /// Acid green.
    green: Color,
    cyan: Color,
    /// Hot magenta.
    magenta: Color,
    violet: Color,
    /// Amber/copper data structures.
    amber: Color,
    red: Color,
    dim: Color,
    st: ThemeStyles,
}

impl Fx {
    fn new(theme: Theme, color: bool) -> Self {
        let st = theme.styles();
        let c = |t: (u8, u8, u8)| {
            if color {
                Color::Rgb(t.0, t.1, t.2)
            } else {
                Color::Reset
            }
        };
        // The *Hackers* (1995) palette: exuberant 1995 technicolor, not
        // black+neon-green. Native terminal background is preserved throughout.
        Self {
            color,
            blue: c((64, 150, 255)),
            green: c((70, 255, 150)),
            cyan: c((80, 235, 255)),
            magenta: c((255, 70, 200)),
            violet: c((170, 110, 255)),
            amber: c((255, 195, 80)),
            red: c((255, 70, 90)),
            dim: c((70, 80, 120)),
            st,
        }
    }

    fn bar(&self, f: f32, w: usize, a: Color, b: Color) -> Line {
        if self.color {
            show::progress(f, w, a, b, self.dim)
        } else {
            let filled = ((f.clamp(0.0, 1.0)) * w as f32).round() as usize;
            Line::raw(format!(
                "{}{}",
                "█".repeat(filled.min(w)),
                "─".repeat(w.saturating_sub(filled))
            ))
        }
    }

    fn spark(&self, v: &[f32], w: usize) -> Line {
        if self.color {
            show::sparkline(v, w, self.green, self.cyan)
        } else {
            Line::raw("─".repeat(w))
        }
    }
}

fn tpanel(title: &str, style: Style) -> Node {
    let mut p = Node::panel(title.to_string(), BorderType::Rounded, style);
    p.layout_style.padding_top = 1.0;
    p.layout_style.padding_bottom = 0.0;
    p
}

fn truncate(s: &str, w: usize) -> String {
    use unicode_segmentation::UnicodeSegmentation as _;
    if unicode_width::UnicodeWidthStr::width(s) <= w {
        return s.to_string();
    }
    let mut out = String::new();
    let mut used = 0usize;
    for g in s.graphemes(true) {
        let gw = unicode_width::UnicodeWidthStr::width(g);
        if used + gw > w.saturating_sub(1) {
            break;
        }
        out.push_str(g);
        used += gw;
    }
    out.push('…');
    out
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Narrative acts. Boot → … → Curtain model the *Hackers* (1995) story shape:
/// a playful cyberculture heist with a corporate mainframe, a garbage file, a
/// worm, a rival, a broadcast and an absurd rooftop payoff.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Boot,
    Login,
    Cyberdelia,
    Breach,
    Transfer,
    Plague,
    DaVinci,
    Broadcast,
    Attack,
    Tactical,
    Download,
    Crash,
    Endgame,
    Pool,
    Curtain,
    Shell,
    Done,
}

const TACTICAL: &[&str] = &[
    "🌊 Override the Olympic-sized pool on the roof",
    "🐛 Neutralize the Da Vinci virus",
    "🛹 Summon Acid Burn, Cereal Killer & Lord Nikon",
    "🕶  Rollerblade away before Agent Gill arrives",
];

/// The tactical choice as a free category: each arrow sets local facts, and all
/// arrows reconverge on the Download beat. No combinatorial state explosion.
fn tactical_story() -> Story {
    Story::new("tactical")
        .beat(
            Beat::new("tactical")
                .transition(Condition::user("pool"), "pool")
                .transition(Condition::user("davinci"), "davinci")
                .transition(Condition::user("crew"), "crew")
                .transition(Condition::user("skate"), "skate")
                .after(Duration::from_secs(30), "download"),
        )
        .beat(
            Beat::new("pool")
                .on_enter(StoryAction::set_bool("pool-distraction", true))
                .after(Duration::ZERO, "download"),
        )
        .beat(
            Beat::new("davinci")
                .on_enter(StoryAction::set_bool("worm-frozen", true))
                .on_enter(StoryAction::set_bool("deadline-slow", true))
                .after(Duration::ZERO, "download"),
        )
        .beat(
            Beat::new("crew")
                .on_enter(StoryAction::set_bool("packet-boost", true))
                .on_enter(StoryAction::set_bool("crew-online", true))
                .after(Duration::ZERO, "download"),
        )
        .beat(
            Beat::new("skate")
                .on_enter(StoryAction::set_number("camera-escape", 1.0))
                .after(Duration::ZERO, "download"),
        )
        .beat(Beat::new("download").terminal())
}

/// Crew nodes for the Grand Central assault; each has a subsystem target.
const CREW: &[(&str, &str, f32)] = &[
    ("CRASH", "gibson-core", 0.92),
    ("ACID", "da-vinci", 0.88),
    ("NIKON", "ellingson-db", 0.95),
    ("CEREAL", "broadcast", 0.81),
    ("JOEY", "garbage.bin", 0.97),
];

/// The root-shell command surface. Each command triggers a real effect.
const SHELL_COMMANDS: &[&str] = &[
    "help",
    "status",
    "crew",
    "crash",
    "burn",
    "gibson",
    "city",
    "route",
    "modem",
    "cyberdelia",
    "garbage",
    "da-vinci",
    "tanker",
    "plague",
    "rabbit",
    "cookie",
    "broadcast",
    "wireframe",
    "torus",
    "plasma",
    "particles",
    "glitch",
    "damage",
    "pool",
    "planet",
    "metrics",
    "exit",
];

struct App {
    time: TimeSource,
    fx: Fx,
    phase: Phase,
    breach: f32,
    transfer: f32,
    download: f32,
    plague: f32,
    crash: f32,
    countdown: f32,
    scan: f32,
    hex_seed: u64,
    events: Vec<Line>,
    throughput: Vec<f32>,
    selected: usize,
    input: TextInputState,
    shell_log: Vec<Line>,
    frames: u64,
    last_total: u64,
    done: bool,
    seen: [bool; 16],

    // Narrative state
    connection_lost: bool,
    download_done: bool,
    broadcast_frames: u32,
    pool_frames: u32,
    curtain_frames: u32,
    /// Semantic presence: the Plague is active from its introduction until it is
    /// explicitly defeated. Rendering attaches to this fact, not to a timer.
    plague_active: bool,
    /// Short entrance flare only (never the source of truth for presence).
    plague_pulse: u32,
    /// Real replication graph (the "rabbit" effect) and its neutralization.
    rabbit: Replication,
    pool_granted: bool,
    crew_joined: usize,
    city_angle: f32,
    /// Elapsed time when the current act began (deterministic under FixedStepClock).
    phase_start: f32,

    // Tactical-choice consequences (local facts; all reconverge on Download).
    pool_distraction: bool,
    worm_frozen: bool,
    deadline_slow: bool,
    packet_boost: bool,
    camera_escape: f32,
    /// Tactical branch selection as a real StoryGraph (branch, then rejoin).
    tactical: StoryDirector,

    // FX state
    particles: ParticleSystem,
    damage: HashMap<(u16, u16), u32>,
    glitch_frames: u32,
    planet_frames: u32,
    show_wireframe: bool,
    show_plasma: bool,
    show_damage: bool,
    /// Wireframe shape selector: 0 octahedron core, 1 torus, 2 cube.
    wire_mesh: u8,
}

/// Human-readable act names for the banner/status.
fn phase_name(p: Phase) -> &'static str {
    match p {
        Phase::Boot => "BOOT",
        Phase::Login => "LOGIN",
        Phase::Cyberdelia => "CYBERDELIA",
        Phase::Breach => "JOEY / GIBSON",
        Phase::Transfer => "GARBAGE DL",
        Phase::Plague => "THE PLAGUE",
        Phase::DaVinci => "DA VINCI",
        Phase::Broadcast => "RAZOR & BLADE",
        Phase::Attack => "GRAND CENTRAL",
        Phase::Tactical => "SELECT",
        Phase::Download => "DOWNLOAD",
        Phase::Crash => "GIBSON CRASH",
        Phase::Endgame => "BROADCAST",
        Phase::Pool => "ROOFTOP POOL",
        Phase::Curtain => "CRASH AND BURN",
        Phase::Shell => "ROOT SHELL",
        Phase::Done => "DONE",
    }
}

fn select_theme(light: bool, dark: bool, no_color: bool) -> Theme {
    if no_color {
        Theme::no_color()
    } else if light {
        Theme::light()
    } else if dark {
        Theme::dark()
    } else {
        Theme {
            accent: Color::BrightCyan,
            success: Color::BrightGreen,
            warning: Color::BrightMagenta,
            error: Color::BrightRed,
            border: Color::BrightBlack,
            rail: Color::BrightCyan,
            code: Color::BrightYellow,
            ..Theme::default()
        }
    }
}

impl App {
    fn new(fx: Fx, deterministic: bool) -> Self {
        let time = if deterministic {
            TimeSource::fixed(Duration::from_millis(16))
        } else {
            TimeSource::real()
        };
        Self {
            time,
            fx,
            phase: Phase::Boot,
            breach: 0.0,
            transfer: 0.0,
            download: 0.0,
            plague: 0.0,
            crash: 0.0,
            countdown: 0.0,
            scan: 0.0,
            hex_seed: 1,
            events: Vec::new(),
            throughput: vec![0.0; 48],
            selected: 0,
            input: TextInputState::new(),
            shell_log: Vec::new(),
            frames: 0,
            last_total: 0,
            done: false,
            seen: [false; 16],
            connection_lost: false,
            download_done: false,
            broadcast_frames: 0,
            pool_frames: 0,
            curtain_frames: 0,
            plague_active: false,
            plague_pulse: 0,
            rabbit: Replication::new(5, 63),
            pool_granted: false,
            crew_joined: 0,
            city_angle: 0.0,
            phase_start: 0.0,
            pool_distraction: false,
            worm_frozen: false,
            deadline_slow: false,
            packet_boost: false,
            camera_escape: 0.0,
            tactical: tactical_story().start(),
            particles: ParticleSystem::new(0x9E37_79B9),
            damage: HashMap::new(),
            glitch_frames: 0,
            planet_frames: 0,
            show_wireframe: true,
            show_plasma: false,
            show_damage: false,
            wire_mesh: 0,
        }
    }

    /// Whether the current act has an "impossible data city" centerpiece.
    fn city_act(&self) -> bool {
        matches!(
            self.phase,
            Phase::Boot
                | Phase::Login
                | Phase::Cyberdelia
                | Phase::Breach
                | Phase::Transfer
                | Phase::Plague
                | Phase::Attack
                | Phase::Download
                | Phase::Crash
                | Phase::Curtain
        )
    }

    /// Current tanker-file deadline pressure in `[0, 1]` (1 = imminent).
    fn deadline(&self) -> f32 {
        self.countdown.clamp(0.0, 1.0)
    }

    fn elapsed(&self) -> f32 {
        self.time.now().as_secs_f32()
    }

    fn sample(&mut self, ctx: &mut Context) {
        self.frames += 1;
        // Renderer frames, not application loop iterations, drive the wire-byte
        // throughput graph.
        let stats = ctx.stats();
        let delta = stats.frame_bytes.saturating_sub(self.last_total) as f32;
        self.last_total = stats.frame_bytes;
        self.throughput.push(delta);
        if self.throughput.len() > 48 {
            self.throughput.remove(0);
        }

        // Deterministic starfield / data tunnel.
        if self.phase != Phase::Done && self.particles.len() < 180 {
            for i in 0..10 {
                self.particles.particles.push(gibson::Particle {
                    x: ((i * 53 + self.frames as usize * 7) % 100) as f32,
                    y: 0.0,
                    vx: 0.0,
                    vy: 4.0,
                    life: 3.0,
                    max_life: 3.0,
                    intensity: 1.0,
                });
            }
        }
        self.particles.update(1.0 / 60.0);
        if self.particles.len() > 2200 {
            self.particles.particles.drain(0..800);
        }
        if self.show_damage {
            for (x, y) in ctx.last_dirty_cells() {
                *self.damage.entry((*x, *y)).or_insert(0) += 1;
            }
        }
        self.glitch_frames = self.glitch_frames.saturating_sub(1);
        self.planet_frames = self.planet_frames.saturating_sub(1);
        self.plague_pulse = self.plague_pulse.saturating_sub(1);
        self.rabbit.update(1.0 / 60.0);
    }

    fn add_event(&mut self, text: &str, color: Color) {
        let stamp = format!("{:04}", self.frames);
        let style = if self.fx.color && color != Color::Reset {
            Style::new().fg(color)
        } else {
            self.fx.st.muted
        };
        self.events.push(
            Line::new()
                .span(Span::styled(format!("{stamp} "), self.fx.st.faint))
                .span(Span::styled(text, style)),
        );
        if self.events.len() > 60 {
            self.events.remove(0);
        }
    }

    fn once(&mut self, idx: usize, text: &str, color: Color) {
        if !self.seen[idx] {
            self.seen[idx] = true;
            self.add_event(text, color);
        }
    }

    /// Transitions to a new act and stamps its start time.
    fn enter(&mut self, p: Phase) {
        if p == Phase::Tactical {
            // Fresh branch point each time.
            self.tactical = tactical_story().start();
        }
        self.phase = p;
        self.phase_start = self.elapsed();
    }

    /// Deterministically jumps straight to an act (for goldens and PTY tests).
    ///
    /// This fast-forwards only *state*, not time: `phase_start` is reset so the
    /// act behaves as if it just began. Unknown names are ignored.
    fn jump_to(&mut self, act: &str) {
        let phase = match act {
            "boot" => Phase::Boot,
            "login" => Phase::Login,
            "cyberdelia" => Phase::Cyberdelia,
            "breach" | "joey" | "city" => Phase::Breach,
            "transfer" | "garbage" => Phase::Transfer,
            "plague" => Phase::Plague,
            "davinci" | "da-vinci" => Phase::DaVinci,
            "razor_blade" | "razor" => Phase::Broadcast,
            "attack" | "grand_central" => Phase::Attack,
            "tactical" => Phase::Tactical,
            "download" => Phase::Download,
            "crash" => Phase::Crash,
            "broadcast" => Phase::Endgame,
            "pool" => Phase::Pool,
            "curtain" | "crash_and_burn" => Phase::Curtain,
            "shell" => Phase::Shell,
            _ => return,
        };
        self.phase = phase;
        self.phase_start = 0.0;
        self.city_angle = 0.7;
        match phase {
            Phase::Breach => self.breach = 0.6,
            Phase::Transfer => {
                self.transfer = 0.42;
                self.connection_lost = true;
            }
            Phase::Plague => {
                self.plague = 0.6;
                self.plague_active = true;
                self.plague_pulse = 60;
            }
            Phase::DaVinci => self.scan = 55.0,
            Phase::Broadcast => self.broadcast_frames = 120,
            Phase::Attack => self.countdown = 0.5,
            Phase::Tactical => {
                self.countdown = 0.6;
                self.tactical = tactical_story().start();
            }
            Phase::Download => {
                // The golden beat is the completed download (Joey's callback).
                self.download = 1.0;
                self.download_done = true;
                self.countdown = 0.85;
                self.crew_joined = 5;
            }
            Phase::Crash => {
                self.crash = 0.5;
                self.download = 1.0;
                self.download_done = true;
                // The Gibson falls and the Plague is defeated.
                self.plague_active = false;
            }
            Phase::Endgame => self.broadcast_frames = 120,
            Phase::Pool => {
                self.pool_frames = 120;
                self.pool_granted = true;
            }
            Phase::Curtain => self.curtain_frames = 40,
            Phase::Shell => {
                self.crew_joined = 5;
                self.countdown = 1.0;
                self.pool_granted = true;
                // The shell is a playground: the Plague is present and can be
                // inspected (and neutralized) through real commands.
                self.plague_active = true;
                self.plague_pulse = 60;
            }
            _ => {}
        }
    }

    fn since(&self) -> f32 {
        self.elapsed() - self.phase_start
    }

    fn animate(&mut self) {
        // The camera never stops panning the data city — but a quarantined
        // (frozen) worm calms it, and a rollerblade escape throws it wide.
        if self.city_act() {
            let base = if self.worm_frozen { 0.0015 } else { 0.006 };
            self.city_angle += base;
            if self.camera_escape > 0.0 {
                self.city_angle += self.camera_escape * 0.05;
                self.camera_escape = (self.camera_escape - 0.01).max(0.0);
            }
        }
        let since = self.since();
        match self.phase {
            // ACT 0 — boot / title. Cinematic reveal, no dashboard yet.
            Phase::Boot => {
                self.once(
                    0,
                    "[boot] ELLINGSON MINERAL CORPORATION · internal access node",
                    self.fx.cyan,
                );
                if since > 0.9 {
                    self.once(1, "GIBSON — data city materialising", self.fx.magenta);
                    self.enter(Phase::Login);
                }
            }
            // ACT 1 — Zero Cool / Crash Override login.
            Phase::Login => {
                self.once(2, "[modem] 28.8k acoustic coupler locked", self.fx.cyan);
                if since > 1.5 {
                    self.once(
                        3,
                        "CRASH OVERRIDE connected · previous identity: ZERO COOL",
                        self.fx.green,
                    );
                    self.enter(Phase::Cyberdelia);
                }
            }
            // ACT 2 — Cyberdelia; the crew join as network nodes.
            Phase::Cyberdelia => {
                if since > 0.0 {
                    self.once(
                        4,
                        "ACID BURN competing connection — packet race",
                        self.fx.magenta,
                    );
                }
                let joined = ((since / 0.3) as usize + 1).min(4);
                self.crew_joined = joined;
                if since > 1.2 {
                    self.once(
                        5,
                        "crew linked: Cereal Killer · Lord Nikon · Phantom Phreak · Joey",
                        self.fx.violet,
                    );
                    self.enter(Phase::Breach);
                }
            }
            // ACT 3 — Joey hacks the Gibson.
            Phase::Breach => {
                self.breach = (self.breach + 0.025).min(1.0);
                self.scan = (self.scan + 0.10) % 100.0;
                self.hex_seed = self.hex_seed.wrapping_add(1);
                let p = self.breach;
                if p > 0.25 {
                    self.once(
                        6,
                        "[trace] moving camera through directory geometry",
                        self.fx.cyan,
                    );
                }
                if p > 0.6 {
                    self.once(7, "GARBAGE selected — /usr/spool/garbage", self.fx.amber);
                }
                if self.breach >= 1.0 {
                    self.once(8, "[net] download started · 28.8k", self.fx.green);
                    self.enter(Phase::Transfer);
                }
            }
            // ACT 3b — the download begins, then is violently interrupted.
            Phase::Transfer => {
                self.transfer = (self.transfer + 0.018).min(0.42);
                self.hex_seed = self.hex_seed.wrapping_add(1);
                if self.transfer >= 0.42 && !self.connection_lost {
                    self.connection_lost = true;
                    self.glitch_frames = 8;
                    self.once(
                        9,
                        "CONNECTION LOST — foreign host severed the route",
                        self.fx.red,
                    );
                }
                if self.connection_lost && since > 1.2 {
                    self.enter(Phase::Plague);
                }
            }
            // ACT 4 — The Plague arrives and invades the scene.
            Phase::Plague => {
                self.plague = (self.plague + 0.03).min(1.0);
                // Semantic truth: the Plague is now active and stays active until
                // explicitly defeated. `plague_pulse` is only the entrance flare.
                self.plague_active = true;
                self.plague_pulse = 60;
                self.glitch_frames = self.glitch_frames.max(1);
                if since > 1.2 {
                    self.once(
                        10,
                        "THE PLAGUE — corporate access overlay sweeping Gibson",
                        self.fx.red,
                    );
                    self.enter(Phase::DaVinci);
                }
            }
            // ACT 5 — forensics: garbage fragments reveal WORM / DA VINCI.
            Phase::DaVinci => {
                if self.worm_frozen {
                    // Quarantined: the scan halts at its current head.
                    self.scan = self.scan.max(42.0);
                } else {
                    self.scan = (self.scan + 0.08) % 100.0;
                }
                if since > 0.8 {
                    self.once(11, "reconstructed fragments reveal: WORM", self.fx.amber);
                }
                if since > 1.4 {
                    self.once(
                        12,
                        "payload identity: DA VINCI · ballast control",
                        self.fx.red,
                    );
                    self.enter(Phase::Broadcast);
                }
            }
            // ACT 7 — Razor & Blade pirate broadcast.
            Phase::Broadcast => {
                self.broadcast_frames = 120;
                if since > 1.0 {
                    self.broadcast_frames = 0;
                    self.enter(Phase::Attack);
                }
            }
            // ACT 8 — Grand Central-style coordinated assault.
            Phase::Attack => {
                let rate = if self.deadline_slow { 0.0016 } else { 0.004 };
                self.countdown = (self.countdown + rate).min(1.0);
                if since > 1.8 {
                    self.enter(Phase::Tactical);
                }
            }
            Phase::Tactical => {}
            // ACT 10 — Joey finishes the download.
            Phase::Download => {
                self.download = (self.download + 0.02).min(1.0);
                let rate = if self.deadline_slow { 0.0024 } else { 0.006 };
                self.countdown = (self.countdown + rate).min(1.0);
                if self.download >= 1.0 && !self.download_done {
                    self.download_done = true;
                    self.enter(Phase::Crash);
                }
            }
            // ACT 11 — the Gibson fails. Geometry destabilises; protocol stays valid.
            Phase::Crash => {
                self.crash = (self.crash + 0.012).min(1.0);
                self.glitch_frames = 4;
                // The Plague is defeated as the Gibson falls.
                self.plague_active = false;
                self.plague_pulse = 0;
                if since > 1.6 {
                    self.enter(Phase::Endgame);
                }
            }
            // ACT 12/13 — broadcast + Plague escape attempt.
            Phase::Endgame => {
                self.broadcast_frames = 120;
                if since > 1.6 {
                    self.broadcast_frames = 0;
                    self.enter(Phase::Pool);
                }
            }
            // ACT 14 — rooftop pool payoff (sprinkler "anomaly").
            Phase::Pool => {
                self.pool_frames = 120;
                if since > 0.6 && !self.pool_granted {
                    self.pool_granted = true;
                    self.once(13, "ROOFTOP POOL ACCESS: GRANTED", self.fx.cyan);
                }
                if since > 2.0 {
                    self.pool_frames = 0;
                    self.enter(Phase::Curtain);
                }
            }
            // ACT 15 — CRASH AND BURN curtain call.
            Phase::Curtain => {
                self.curtain_frames = self.curtain_frames.saturating_add(1).min(120);
                if since > 2.2 {
                    self.curtain_frames = 0;
                    self.enter(Phase::Shell);
                }
            }
            Phase::Shell => {}
            Phase::Done => {}
        }
    }

    fn auto_step(&mut self) {
        match self.phase {
            Phase::Tactical => {
                self.selected = 0;
                self.commit_tactical();
            }
            Phase::Shell => {
                // Let the interactive shell own the screen for a moment before
                // scripted mode fires its command sweep, so the shell state is
                // visible and capturable.
                if self.since() < 0.6 {
                    return;
                }
                for cmd in [
                    "status",
                    "crew",
                    "route",
                    "city",
                    "garbage",
                    "da-vinci",
                    "plague",
                    "rabbit",
                    "cookie",
                    "broadcast",
                    "wireframe",
                    "torus",
                    "plasma",
                    "particles",
                    "damage",
                    "pool",
                    "planet",
                    "crash",
                    "burn",
                    "metrics",
                    "exit",
                ] {
                    self.input = TextInputState::with_text(cmd);
                    self.run_command(cmd);
                    self.input = TextInputState::new();
                }
                self.phase = Phase::Done;
                self.done = true;
            }
            Phase::Done => self.done = true,
            _ => {}
        }
    }

    fn commit_tactical(&mut self) {
        self.selected = self.selected.min(TACTICAL.len() - 1);
        let choice = TACTICAL[self.selected];
        let key = ["pool", "davinci", "crew", "skate"][self.selected];
        // The StoryGraph selects the arrow and sets semantic facts; the app does
        // not branch on the raw selection index.
        self.tactical
            .update(Duration::ZERO, &[StoryEvent::user_selected(key)]);
        let f = self.tactical.facts();
        if f.bool("pool-distraction") {
            self.pool_distraction = true;
            self.pool_frames = 120;
            self.pool_granted = true;
            // Water appears on the peripheral region of the city.
            for i in 0..48 {
                let x = if i % 2 == 0 { 1.0 } else { 96.0 };
                self.particles.particles.push(gibson::Particle {
                    x: x + (i as f32 % 5.0),
                    y: 6.0 + (i as f32 % 12.0),
                    vx: if i % 2 == 0 { 1.2 } else { -1.2 },
                    vy: 0.6,
                    life: 2.4,
                    max_life: 2.4,
                    intensity: 0.8,
                });
            }
        }
        if f.bool("worm-frozen") {
            self.worm_frozen = true;
        }
        if f.bool("deadline-slow") {
            self.deadline_slow = true;
        }
        if f.bool("packet-boost") {
            self.packet_boost = true;
        }
        if f.bool("crew-online") {
            self.crew_joined = CREW.len();
        }
        if f.number("camera-escape") > 0.0 {
            self.camera_escape = 1.0;
            self.glitch_frames = 12;
        }
        // The branch beat is a discrete story object: it advanced out of the
        // tactical beat in the update above, and now (as a second arrow) advances
        // to Download, so the trace is tactical > branch > download.
        self.tactical.update(Duration::from_millis(1), &[]);

        let consequence = match key {
            "pool" => "pool sprinkler distraction · Plague scan quality −40%",
            "davinci" => "Da Vinci quarantined · worm geometry frozen · deadline slowed",
            "crew" => "crew summoned · route density up · packet graph expanded",
            _ => "rollerblade escape · camera pan engaged · attack continues remotely",
        };
        let short = match key {
            "pool" => "pool distraction active · water on the periphery",
            "davinci" => "worm frozen · Da Vinci quarantine active",
            "crew" => "crew reinforced · route density boosted",
            _ => "camera escape engaged · attack continues remotely",
        };
        self.add_event(short, self.fx.amber);
        let line = Line::new()
            .span(Span::styled("[DIRECTIVE] ", self.fx.st.warning))
            .span(Span::styled(choice, self.fx.st.text));
        self.shell_log.push(line);
        self.shell_log.push(Line::new().span(Span::styled(
            format!("           ↳ {consequence}"),
            self.fx.st.muted,
        )));
        self.once(14, "\"HACK THE PLANET! HACK THE PLANET!\"", self.fx.magenta);
        self.glitch_frames = self.glitch_frames.max(6);
        self.enter(Phase::Download);
    }

    fn run_command(&mut self, cmd: &str) {
        // Commands trigger real, reusable effects rather than printing a themed
        // string. Each returns (copy, style) added to the shell log.
        let style = |s: Style| s;
        let out: Vec<(String, Style)> = match cmd {
            "help" => vec![(
                format!("commands: {}", SHELL_COMMANDS.join(" ")),
                self.fx.st.muted,
            )],
            "status" => vec![(
                format!(
                    "banks 04/07 ONLINE · route 7 hops · deadline {:.0}%",
                    self.deadline() * 100.0
                ),
                self.fx.st.code,
            )],
            "crew" => CREW
                .iter()
                .map(|(name, target, q)| {
                    (
                        format!("{name:<7} → {target:<12} link {:.0}%", q * 100.0),
                        self.fx.st.text,
                    )
                })
                .collect(),
            "crash" => {
                self.glitch_frames = 10;
                self.crash = self.crash.max(0.6);
                vec![(
                    "safe geometry-collapse transition engaged".into(),
                    self.fx.st.error,
                )]
            }
            "burn" => {
                self.curtain_frames = 60;
                vec![("CRASH AND BURN".into(), self.fx.st.warning)]
            }
            "gibson" | "city" => {
                // Camera actually enters the data city.
                self.city_angle = 0.7;
                vec![(
                    "camera on the Gibson data city · towers + circuit plane".into(),
                    self.fx.st.accent,
                )]
            }
            "route" => {
                // Route graph becomes the focal layer: packet density up.
                self.packet_boost = true;
                vec![(
                    "route 66 → gibson-core → zero-cool (graph focal)".into(),
                    self.fx.st.code,
                )]
            }
            "modem" => {
                // Acoustic waveform: a short particle ring in the periphery.
                for i in 0..30 {
                    let a = i as f32 * 0.4;
                    self.particles.particles.push(gibson::Particle {
                        x: 4.0 + (i as f32 % 20.0),
                        y: 4.0,
                        vx: a.cos() * 0.4,
                        vy: a.sin() * 0.4,
                        life: 1.2,
                        max_life: 1.2,
                        intensity: 0.7,
                    });
                }
                vec![(
                    "acoustic coupler 28.8k · carrier stable".into(),
                    Style::new().fg(self.fx.cyan),
                )]
            }
            "cyberdelia" => {
                self.show_plasma = true;
                vec![(
                    "club visualizer engaged — plasma + spinner".into(),
                    Style::new().fg(self.fx.magenta),
                )]
            }
            "garbage" => {
                // The garbage.bin object becomes visible/animated.
                self.transfer = self.transfer.max(0.2);
                self.show_wireframe = true;
                vec![(
                    "garbage.bin /usr/spool/garbage (256 MB) · file object animating".into(),
                    self.fx.st.text,
                )]
            }
            "da-vinci" => {
                // Mount the worm entity as structured geometry.
                self.worm_frozen = false;
                self.scan = 55.0;
                self.show_wireframe = true;
                self.wire_mesh = 1;
                vec![(
                    "da-vinci worm · ballast control · $25,000,000 siphon".into(),
                    self.fx.st.warning,
                )]
            }
            "tanker" => {
                self.countdown = self.deadline().max(0.7);
                vec![("tanker fleet deadline advanced".into(), self.fx.st.error)]
            }
            "plague" => {
                self.plague_active = true;
                self.plague_pulse = 90;
                self.glitch_frames = 6;
                vec![(
                    "THE PLAGUE enters — hostile scan entity (persists)".into(),
                    self.fx.st.error,
                )]
            }
            "rabbit" => {
                self.rabbit.start();
                self.planet_frames = 30;
                vec![(
                    format!(
                        "rabbit replication started — {} nodes, gen {} (bounded)",
                        self.rabbit.node_count(),
                        self.rabbit.generation()
                    ),
                    self.fx.st.warning,
                )]
            }
            "cookie" => {
                let was = self.rabbit.node_count();
                self.rabbit.neutralize();
                vec![(
                    format!("cookie neutralized the rabbit ({was} nodes collapsing)"),
                    self.fx.st.success,
                )]
            }
            "broadcast" => {
                self.broadcast_frames = 120;
                vec![(
                    "Razor & Blade pirate broadcast hijacked the terminal".into(),
                    self.fx.st.accent,
                )]
            }
            "wireframe" => {
                self.show_wireframe = !self.show_wireframe;
                vec![("3D core projection toggled".into(), self.fx.st.code)]
            }
            "torus" => {
                self.show_wireframe = true;
                self.wire_mesh = (self.wire_mesh + 1) % 3;
                vec![(
                    "wireframe shape cycled (octahedron → torus → cube)".into(),
                    self.fx.st.code,
                )]
            }
            "plasma" => {
                self.show_plasma = !self.show_plasma;
                vec![("core temperature field toggled".into(), self.fx.st.warning)]
            }
            "particles" => {
                self.planet_frames = 45;
                vec![("deterministic particle burst".into(), self.fx.st.accent)]
            }
            "glitch" => {
                self.glitch_frames = 8;
                vec![(
                    "framebuffer-only glitch (protocol untouched)".into(),
                    self.fx.st.error,
                )]
            }
            "damage" => {
                self.show_damage = !self.show_damage;
                vec![(
                    "LIVE DAMAGE MAP toggled — logical damage, not wire bytes".into(),
                    self.fx.st.accent,
                )]
            }
            "pool" => {
                self.pool_frames = 120;
                self.pool_granted = true;
                vec![
                    (
                        "There is no pool on the roof of Ellingson Mineral!".into(),
                        self.fx.st.accent,
                    ),
                    (
                        "…sprinkler override initiated. Water pressure critical.".into(),
                        self.fx.st.warning,
                    ),
                ]
            }
            "planet" => {
                self.planet_frames = 120;
                vec![("HACK THE PLANET".into(), self.fx.st.accent)]
            }
            "metrics" => vec![(
                "metrics available after exit (see stdout summary)".into(),
                self.fx.st.muted,
            )],
            "exit" | "quit" => vec![(
                "connection severed by foreign host. Skate fast.".into(),
                self.fx.st.error,
            )],
            "" => vec![],
            other => vec![
                (
                    "bash: command not found in /usr/local/bin".into(),
                    self.fx.st.error,
                ),
                (other.to_string(), self.fx.st.muted),
            ],
        };
        let _ = style;
        for (text, sty) in out {
            self.shell_log.push(
                Line::new()
                    .span(Span::styled("  ", self.fx.st.faint))
                    .span(Span::styled(text, sty)),
            );
        }
        if self.shell_log.len() > 200 {
            self.shell_log.remove(0);
        }
    }

    fn handle(&mut self, event: &Event) -> bool {
        if is_cancel(event) {
            self.done = true;
            return true;
        }
        if let Event::Key(k) = event {
            match self.phase {
                Phase::Tactical => match k.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.selected = if self.selected == 0 {
                            TACTICAL.len() - 1
                        } else {
                            self.selected - 1
                        };
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.selected = (self.selected + 1) % TACTICAL.len()
                    }
                    KeyCode::Char(c @ '1'..='4') => self.selected = (c as usize) - ('1' as usize),
                    KeyCode::Enter => {
                        self.commit_tactical();
                        return true;
                    }
                    KeyCode::Esc => self.done = true,
                    _ => {}
                },
                Phase::Shell => match k.code {
                    KeyCode::Enter => {
                        let cmd = self.input.text.trim().to_lowercase();
                        self.shell_log.push(
                            Line::new()
                                .span(Span::styled("root@gibson:~# ", self.fx.st.success))
                                .span(Span::styled(self.input.text.clone(), self.fx.st.text)),
                        );
                        self.run_command(&cmd);
                        self.input = TextInputState::new();
                        if cmd == "exit" || cmd == "quit" {
                            self.done = true;
                        }
                        // A command changes the shell log; request a repaint.
                        return true;
                    }
                    KeyCode::Esc => self.done = true,
                    _ => return self.input.handle_event(event),
                },
                _ => {}
            }
        } else if let Event::Paste(_) = event {
            if self.phase == Phase::Shell {
                return self.input.handle_event(event);
            }
        }
        false
    }
}

fn is_cancel(event: &Event) -> bool {
    matches!(event, Event::Key(k) if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL))
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn build_root(app: &App, ctx: &Context) -> Node {
    let (cols, rows) = ctx.session.terminal_size();
    let compact = cols < 72;
    let wide = cols >= 118;

    let mut root = Node::col()
        .percent_width(100.0)
        .percent_height(100.0)
        .gap(0.0)
        .child(banner(app, cols, wide));

    if compact {
        if app.city_act() {
            root = root.child(panel_city(app, cols).flex_grow(3.0).min_width(0.0));
        }
        root = root.child(panel_mainframe(app, cols).flex_grow(2.0).min_width(0.0));
        root = root.child(panel_garbage(app, cols).flex_grow(2.0).min_width(0.0));
        if rows >= 26 {
            root = root.child(panel_davinci(app, cols).flex_grow(2.0).min_width(0.0));
        }
    } else {
        let right_w: u16 = if wide { 46 } else { 38 };
        let left_w = cols.saturating_sub(right_w + 1);

        let mut left = Node::col().flex_grow(2.0).min_width(0.0).gap(0.0);
        if app.city_act() {
            left = left.child(panel_city(app, left_w).flex_grow(3.0).min_width(0.0));
        }
        left = left
            .child(panel_mainframe(app, left_w).flex_grow(1.0).min_width(0.0))
            .child(panel_garbage(app, left_w).flex_grow(2.0).min_width(0.0));

        let right = Node::col()
            .width(right_w as f32)
            .flex_shrink(0.0)
            .min_width(0.0)
            .gap(0.0)
            .child(panel_davinci(app, right_w).flex_grow(1.0).min_width(0.0))
            .child(panel_trace(app, right_w).flex_grow(2.0).min_width(0.0));

        root = root.child(
            Node::row()
                .percent_width(100.0)
                .gap(1.0)
                .flex_grow(2.0)
                .child(left)
                .child(right),
        );
    }

    root = root.child(panel_transfer(app, cols));
    root = root.child(bottom_panel(app, cols));

    // CRT-ish scanline overlay: a dim band sweeps vertically without touching
    // the layout beneath it (style-only composite). Cheap: only a few rows.
    let band = (app.elapsed() * 0.9) as u16;
    let y = (band % (rows.max(1))) as f32;
    let mut scene = Node::stack()
        .percent_width(100.0)
        .percent_height(100.0)
        .child(root)
        .child(
            Node::col()
                .percent_width(100.0)
                .percent_height(100.0)
                .padding_top(y)
                .child(Node::dim().percent_width(100.0).height(1.0))
                .child(Node::dim().percent_width(100.0).height(2.0)),
        );

    // Optional effects, composited as real layers.
    if app.show_damage {
        scene = scene.child(damage_overlay(app, cols, rows));
    }
    if app.planet_frames > 0 {
        scene = scene.child(planet_overlay(app, cols, rows));
    }
    if app.rabbit.is_active() {
        scene = scene.child(rabbit_overlay(app, cols, rows));
    }
    if app.broadcast_frames > 0 {
        scene = scene.child(broadcast_overlay(app, cols, rows));
    }
    if app.pool_frames > 0 {
        scene = scene.child(pool_overlay(app, cols, rows));
    }
    if app.curtain_frames > 0 {
        scene = scene.child(curtain_overlay(app, cols, rows));
    }
    scene
}

/// Renders the renderer's own dirty-cell history as a Braille heatmap.
fn damage_overlay(app: &App, cols: u16, rows: u16) -> Node {
    let fx = &app.fx;
    let mut canvas = gibson::BrailleCanvas::new(cols, rows);
    let max = app.damage.values().copied().max().unwrap_or(1).max(1);
    let dots = [
        (0, 0),
        (0, 1),
        (0, 2),
        (1, 0),
        (1, 1),
        (1, 2),
        (0, 3),
        (1, 3),
    ];
    for (&(x, y), &count) in &app.damage {
        if x >= cols || y >= rows {
            continue;
        }
        let lit = ((count as f32 / max as f32) * 8.0).ceil() as usize;
        for &(dx, dy) in dots.iter().take(lit) {
            canvas.set(x as i32 * 2 + dx, y as i32 * 4 + dy);
        }
    }
    let style = if fx.color {
        fx.st.warning
    } else {
        Style::default()
    };
    // Transparent layer: an opaque surface here would blank the whole screen.
    let mut layer = gibson::surface::Surface::new_transparent(cols, rows);
    canvas.paint_into(&mut layer, (0, 0), style);
    Node::raster(layer).width(cols as f32).height(rows as f32)
}

/// The rabbit replication graph as a real scene entity: a bounded branching
/// tree drawn in braille, with a truthful status label. Cookie neutralization is
/// visible as a collapse, not a generic particle burst.
fn rabbit_overlay(app: &App, cols: u16, rows: u16) -> Node {
    let fx = &app.fx;
    let mut canvas = BrailleCanvas::new(cols, rows);
    app.rabbit.render_braille(&mut canvas);
    let style = if fx.color {
        fx.st.warning
    } else {
        Style::default()
    };
    let mut layer = gibson::surface::Surface::new_transparent(cols, rows);
    canvas.paint_into(&mut layer, (0, 0), style);
    let label = if app.rabbit.is_collapsing() {
        format!(
            "RABBIT NEUTRALIZED (cookie) · collapsing {:.0}%",
            app.rabbit.collapse() * 100.0
        )
    } else {
        format!(
            "RABBIT REPLICATION · gen {} · {} nodes (bounded)",
            app.rabbit.generation(),
            app.rabbit.node_count()
        )
    };
    Node::stack()
        .percent_width(100.0)
        .percent_height(100.0)
        .child(Node::raster(layer).width(cols as f32).height(rows as f32))
        .child(Node::text(label, fx.st.warning).offset(2.0, 1.0))
}

/// Deterministic particle burst ("HACK THE PLANET"). No persistent state: the
/// burst is reconstructed from the countdown, so it is reproducible.
fn planet_overlay(app: &App, cols: u16, rows: u16) -> Node {
    let fx = &app.fx;
    let mut ps = ParticleSystem::new(0x9E37_0000);
    ps.burst(260, cols as f32, rows as f32 * 0.6, 26.0, 1.4);
    let progress = 1.0 - (app.planet_frames as f32 / 90.0).clamp(0.0, 1.0);
    ps.update(progress * 1.4);
    let mut canvas = gibson::BrailleCanvas::new(cols, rows);
    ps.render_braille(&mut canvas);
    let style = if fx.color {
        fx.st.accent
    } else {
        Style::default()
    };
    let mut layer = gibson::surface::Surface::new_transparent(cols, rows);
    canvas.paint_into(&mut layer, (0, 0), style);
    Node::raster(layer).width(cols as f32).height(rows as f32)
}

fn banner(app: &App, cols: u16, wide: bool) -> Node {
    let fx = &app.fx;
    let mut line = shimmer(
        "▚▚▚ GIBSON MAINFRAME ▞▞▞",
        fx,
        (app.elapsed() * 26.0) as usize,
    );
    line = line.span(Span::styled("   ", fx.st.text));
    line = line.span(Span::styled(
        format!(" ACT {} ", phase_name(app.phase)),
        Style::new()
            .fg(if app.plague_active {
                fx.red
            } else {
                fx.magenta
            })
            .bold(),
    ));
    line = line.span(Span::styled(" ● LINK ", fx.st.success));
    if wide {
        line = line
            .span(Span::styled("  baud ", fx.st.muted))
            .span(Span::styled("28.8k", fx.st.code))
            .span(Span::styled("  ·  crew ", fx.st.muted))
            .span(Span::styled(
                format!("{}/5", app.crew_joined.min(5)),
                fx.st.text,
            ))
            .span(Span::styled("  ·  deadline ", fx.st.muted))
            .span(Span::styled(
                format!("{:>3.0}%", app.deadline() * 100.0),
                if app.deadline() > 0.7 {
                    fx.st.error
                } else {
                    fx.st.warning
                },
            ));
    } else if cols > 50 {
        line = line
            .span(Span::styled("  ", fx.st.muted))
            .span(Span::styled(phase_name(app.phase), fx.st.muted));
    }
    Node::line(line).height(1.0)
}

fn shimmer(text: &str, fx: &Fx, offset: usize) -> Line {
    if !fx.color {
        return Line::styled(text, Style::new().bold());
    }
    use unicode_segmentation::UnicodeSegmentation as _;
    let gs: Vec<&str> = text.graphemes(true).collect();
    let n = gs.len().max(1);
    let mut spans = Vec::with_capacity(n);
    for (i, g) in gs.iter().enumerate() {
        let t = i as f32 / (n - 1).max(1) as f32;
        // Electric blue → hot magenta, white-hot at the highlight.
        let base = fx.blue.lerp(fx.magenta, t);
        let d = ((i + offset) % 20) as f32;
        let boost = (1.0 - (d.min(20.0 - d) / 10.0)).clamp(0.0, 1.0) * 0.6;
        spans.push(Span::styled(
            *g,
            Style::new()
                .fg(base.lerp(Color::Rgb(255, 255, 255), boost))
                .bold(),
        ));
    }
    Line::from_spans(spans)
}

/// Tower layout for the reusable Gibson data city.
const TOWERS: &[(f32, f32, f32, f32, f32)] = &[
    (-4.0, -3.0, 0.8, 2.2, 0.8),
    (-2.0, 2.0, 1.0, 3.4, 1.0),
    (1.0, -2.0, 0.9, 2.8, 0.9),
    (3.5, 2.5, 1.2, 4.0, 1.2),
    (0.0, 4.0, 0.7, 1.8, 0.7),
    (-4.5, 3.5, 0.6, 2.6, 0.6),
];

fn city_mesh() -> Mesh {
    let mut mesh = Mesh::grid_xz(6.0, 6.0, 10);
    for &(x, z, w, h, d) in TOWERS {
        mesh.append(&Mesh::data_tower(x, z, w, h, d));
    }
    mesh.append(&Mesh::octahedron(0.7).translated(Vec3::new(0.0, 2.8, 0.0)));
    mesh
}

/// Composites the data city into a transparent raster with cheap depth cues:
/// near edges bright, far edges dim. A crash destabilises the geometry.
fn data_city_surface(app: &App, cw: u16, ch: u16) -> gibson::surface::Surface {
    let fx = &app.fx;
    let mesh = city_mesh();
    let tf = Transform3 {
        rx: -0.75,
        ry: app.city_angle,
        rz: 0.0,
        scale: 1.0,
        offset: Vec3::new(0.0, -1.1, 0.0),
    };
    let pw = cw as f32 * 2.0;
    let ph = ch as f32 * 4.0;
    let edges = Projector::default().project_mesh(&mesh, &tf, pw, ph, 1.0);
    let mut depths: Vec<f32> = edges.iter().map(|e| e.depth).collect();
    depths.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = depths.get(depths.len() / 2).copied().unwrap_or(0.0);

    let bright = if fx.color {
        Style::new().fg(fx.cyan)
    } else {
        Style::default().bold()
    };
    let dim = if fx.color {
        Style::new().fg(fx.violet)
    } else {
        Style::default().dim()
    };

    let mut far = BrailleCanvas::new(cw, ch);
    let mut near = BrailleCanvas::new(cw, ch);
    for e in &edges {
        // The Plague constrains the geometry: far edges collapse toward the core.
        let collapse = app.plague_active && e.depth > median;
        if e.depth <= median {
            near.line(e.a.0, e.a.1, e.b.0, e.b.1);
        } else if !collapse {
            far.line(e.a.0, e.a.1, e.b.0, e.b.1);
        }
    }
    let mut layer = gibson::surface::Surface::new_transparent(cw, ch);
    far.paint_into(&mut layer, (0, 0), dim);
    near.paint_into(&mut layer, (0, 0), bright);

    if app.crash > 0.0 {
        let r = gibson::surface::Rect::new(0, 0, cw, ch);
        gibson::tear(&mut layer, r, app.frames ^ 0x51B5_0A11);
    }
    layer
}

/// Paints a hostile scan beam sweeping the city onto an existing layer.
fn paint_plague_beam(layer: &mut gibson::surface::Surface, app: &App, cw: u16, ch: u16) {
    let fx = &app.fx;
    let mut canvas = BrailleCanvas::new(cw, ch);
    let phase = (app.elapsed() * 0.6).fract();
    let x = (phase * cw.saturating_sub(1) as f32) as i32 * 2;
    canvas.line(x, 0, x, canvas.pixel_height() as i32 - 1);
    let style = if fx.color {
        Style::new().fg(fx.red)
    } else {
        Style::default()
    };
    canvas.paint_into(layer, (0, 0), style);
}

fn panel_city(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let cw = width.saturating_sub(4).clamp(8, 96);
    let ch = 8u16;
    // Composite everything into one transparent layer: the Stack node would
    // collapse because its children are percent-sized, so the panel is sized by
    // the raster's explicit cell dimensions instead.
    let mut layer = data_city_surface(app, cw, ch);
    if matches!(app.phase, Phase::Attack | Phase::Download) {
        paint_packets(&mut layer, app, cw, ch);
    }
    if app.plague_active {
        paint_plague_beam(&mut layer, app, cw, ch);
    }
    let title = if app.plague_active {
        "GIBSON · DATA CITY ⚠ PLAGUE"
    } else {
        "GIBSON · DATA CITY"
    };
    tpanel(
        title,
        if app.plague_active {
            Style::new().fg(fx.red)
        } else {
            fx.st.border
        },
    )
    .percent_width(100.0)
    .height((ch + 2) as f32)
    .flex_shrink(0.0)
    .child(Node::raster(layer).width(cw as f32).height(ch as f32))
}

/// Paints moving packet particles along crew routes onto an existing layer.
/// Deterministic from `t`.
fn paint_packets(layer: &mut gibson::surface::Surface, app: &App, cw: u16, ch: u16) {
    let fx = &app.fx;
    let pw = cw as f32 * 2.0;
    let ph = ch as f32 * 4.0;
    let nodes = [
        (0.15, 0.25),
        (0.40, 0.15),
        (0.70, 0.30),
        (0.85, 0.65),
        (0.55, 0.85),
        (0.20, 0.70),
    ];
    let pos = |i: usize| (nodes[i].0 * pw, nodes[i].1 * ph);
    // Crew links plus Plague counter-routes. A pool distraction degrades the
    // hostile scan, so the counter-routes thin out; crew reinforcement increases
    // packet density along the friendly routes.
    let routes = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 0), (0, 3)];
    let packets_per_route = if app.packet_boost { 6 } else { 3 };
    let show_counter = app.plague_active && !app.pool_distraction;
    let mut canvas = BrailleCanvas::new(cw, ch);
    for &(a, b) in &routes {
        let (x0, y0) = pos(a);
        let (x1, y1) = pos(b);
        canvas.line(x0 as i32, y0 as i32, x1 as i32, y1 as i32);
        if show_counter {
            // Hostile counter-route offset by a couple of dots.
            canvas.line(x0 as i32 + 2, y0 as i32, x1 as i32 + 2, y1 as i32);
        }
    }
    for (k, &(a, b)) in routes.iter().enumerate() {
        let (x0, y0) = pos(a);
        let (x1, y1) = pos(b);
        for j in 0..packets_per_route {
            let phase = (app.elapsed() * 0.7
                + k as f32 * 0.14
                + j as f32 * (1.0 / packets_per_route as f32))
                .fract();
            let x = x0 + (x1 - x0) * phase;
            let y = y0 + (y1 - y0) * phase;
            canvas.set(x.round() as i32, y.round() as i32);
            canvas.set(x.round() as i32 + 1, y.round() as i32);
        }
    }
    let style = if fx.color {
        Style::new().fg(fx.magenta)
    } else {
        Style::default()
    };
    canvas.paint_into(layer, (0, 0), style);
}

/// Pirate-TV broadcast overlay (Razor & Blade). Scanlines, no dialogue.
fn broadcast_overlay(app: &App, cols: u16, _rows: u16) -> Node {
    let fx = &app.fx;
    let title = "PIRATE BROADCAST · RAZOR & BLADE";
    let mut body = RichText::new()
        .line(
            Line::new()
                .span(Span::styled(
                    "HACK THE PLANET",
                    Style::new().fg(fx.magenta).bold(),
                ))
                .span(Span::styled(
                    "   evidence package · worm analysis",
                    fx.st.text,
                )),
        )
        .line(Line::new().span(Span::styled(
            "Ellingson routing exposed · Plague identity sealed",
            fx.st.muted,
        )));
    // Scanlines inside the TV frame (style-only dim rows, bounded to the panel).
    for _ in 0..2 {
        body = body.line(Line::raw(""));
    }
    let w = (cols.saturating_sub(2)).clamp(24, 60);
    Node::col()
        .percent_width(100.0)
        .percent_height(100.0)
        .align_items(gibson::node::AlignItems::Center)
        .justify_content(gibson::node::JustifyContent::End)
        .padding_bottom(3.0)
        .child(
            Node::panel(title, BorderType::Double, fx.st.accent)
                .width(w as f32)
                .background(Color::Reset)
                .child(Node::rich_text_wrapped(body, WrapMode::NoWrap)),
        )
        .child(Node::text(
            "▚▞▚▞▚▞▚▞▚▞▚▞  TRANSMISSION SOURCE: UNKNOWN  ▚▞▚▞▚▞▚▞▚▞",
            fx.st.muted,
        ))
        .child(Node::dim().percent_width(100.0).height(1.0))
        .child(Node::dim().percent_width(100.0).height(1.0))
}

/// Rooftop pool payoff: particle water spilling through the lower viewport.
fn pool_overlay(app: &App, cols: u16, rows: u16) -> Node {
    let fx = &app.fx;
    let ch = (rows / 2).max(4);
    let mut ps = ParticleSystem::new(0x900D_5EED);
    ps.burst_directional(
        240,
        cols as f32 * 0.5,
        0.0,
        14.0,
        2.2,
        std::f32::consts::FRAC_PI_2,
        1.5,
    );
    ps.update((app.elapsed() * 1.3) % 2.2);
    let mut canvas = BrailleCanvas::new(cols, ch);
    ps.render_braille(&mut canvas);
    let style = if fx.color {
        Style::new().fg(fx.cyan)
    } else {
        Style::default()
    };
    let mut layer = gibson::surface::Surface::new_transparent(cols, ch);
    canvas.paint_into(&mut layer, (0, 0), style);

    Node::col()
        .percent_width(100.0)
        .percent_height(100.0)
        .justify_content(gibson::node::JustifyContent::End)
        // Keep the spill above the root shell so the shell stays readable.
        .padding_bottom(8.0)
        .child(Node::text(
            "ROOFTOP POOL ACCESS: GRANTED  ·  apparently there is a pool on the roof.",
            fx.st.accent,
        ))
        .child(Node::raster(layer).width(cols as f32).height(ch as f32))
}

/// CRASH AND BURN curtain call: building columns illuminate, then the tagline.
fn curtain_overlay(app: &App, cols: u16, rows: u16) -> Node {
    let fx = &app.fx;
    // Illuminated window columns across the terminal.
    let mut columns = gibson::surface::Surface::new_transparent(cols, 3);
    let lit = (app.curtain_frames as usize * 3).min(cols as usize);
    for x in 0..cols as usize {
        let style = if x < lit {
            if fx.color {
                Style::new().fg(fx.amber)
            } else {
                Style::default().bold()
            }
        } else if fx.color {
            Style::new().fg(fx.dim)
        } else {
            Style::default().dim()
        };
        for y in 0..3u16 {
            columns.set_cell(
                x as u16,
                y,
                gibson::cell::Cell::new(gibson::cell::Glyph::new("▓"), style),
            );
        }
    }

    let mut rt = RichText::new()
        .line(Line::styled(
            "C R A S H",
            Style::new().fg(fx.magenta).bold(),
        ))
        .line(Line::styled("A N D", Style::new().fg(fx.violet).bold()))
        .line(Line::styled("B U R N", Style::new().fg(fx.amber).bold()));
    if app.curtain_frames > 30 {
        rt = rt.line(Line::styled(
            "HACK THE PLANET",
            Style::new().fg(fx.cyan).bold(),
        ));
    }

    let _ = rows;
    Node::stack()
        .percent_width(100.0)
        .percent_height(100.0)
        .child(Node::raster(columns))
        .child(
            Node::col()
                .percent_width(100.0)
                .percent_height(100.0)
                .align_items(gibson::node::AlignItems::Center)
                .justify_content(gibson::node::JustifyContent::Center)
                .child(
                    Node::panel("", BorderType::Rounded, fx.st.border)
                        .background(Color::Reset)
                        .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap)),
                ),
        )
}

fn panel_mainframe(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let inner = width.saturating_sub(4) as usize;
    let bar_w = inner.saturating_sub(12).clamp(8, 26);
    let pulse = (app.elapsed() * 2.0).sin() * 0.5 + 0.5;
    let cpu = (0.55 + 0.35 * pulse).clamp(0.0, 1.0);
    let mem = (0.42 + 0.30 * ((app.elapsed() * 1.3).sin() * 0.5 + 0.5)).clamp(0.0, 1.0);

    let mut rt = RichText::new();
    for (label, value) in [("CPU", cpu), ("MEM", mem)] {
        let mut line = Line::new().span(Span::styled(format!("{label} "), fx.st.muted));
        for s in fx.bar(value, bar_w, fx.green, fx.cyan).spans {
            line = line.span(s);
        }
        line = line.span(Span::styled(
            format!(" {:>3}%", (value * 100.0) as usize),
            fx.st.text,
        ));
        rt = rt.line(line);
    }
    rt = rt.line(
        Line::new()
            .span(Span::styled("BANKS ", fx.st.muted))
            .span(Span::styled("04/07 ONLINE", fx.st.success))
            .span(Span::styled("   ENTROPY ", fx.st.muted))
            .span(Span::styled(
                format!("{:.3}", 0.99 + 0.009 * pulse),
                fx.st.code,
            )),
    );
    rt = rt.line(
        Line::new()
            .span(Span::styled("BREACH ", fx.st.muted))
            .span(Span::styled(
                gibson::node::SPINNER_BRAILLE
                    [(app.elapsed() * 12.0) as usize % gibson::node::SPINNER_BRAILLE.len()],
                fx.st.accent,
            ))
            .span(Span::styled(
                format!(" {:>3}%", (app.breach * 100.0) as usize),
                fx.st.warning,
            )),
    );
    let mut body = Node::col()
        .percent_width(100.0)
        .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap));

    // Rotating 3D core: real perspective projection to sub-cell Braille lines.
    if app.show_wireframe {
        let cw = inner.clamp(8, 40) as u16;
        let ch = 4u16;
        let mut canvas = gibson::BrailleCanvas::new(cw, ch);
        let e = app.elapsed();
        let t = Transform3::rotation(e * 0.9, e * 1.4, e * 0.3);
        let mesh = match app.wire_mesh {
            1 => Mesh::torus(1.0, 0.38, 18, 10),
            2 => Mesh::cube(1.7),
            _ => Mesh::octahedron(1.25),
        };
        Projector::default().draw(&mesh, &t, &mut canvas, 1.0);
        let mut surf = canvas.to_surface(fx.st.accent);
        if app.glitch_frames > 0 {
            gibson::tear(
                &mut surf,
                gibson::surface::Rect::new(0, 0, cw, ch),
                app.frames ^ 0xABCD,
            );
        }
        body = body.child(Node::raster(surf));
    }

    tpanel("MAINFRAME", fx.st.border)
        .percent_width(100.0)
        .child(body)
}

fn panel_garbage(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let inner = width.saturating_sub(4) as usize;
    let cols_hex = (inner / 3).clamp(4, 18);
    let mut rt = RichText::new();
    let rows = 8u64;
    for i in 0..rows {
        rt = rt.line(show::hex_dump_line(
            app.hex_seed.wrapping_add(i * 2_654_435_761),
            cols_hex,
            fx.dim,
            fx.green,
        ));
    }
    tpanel("garbage.bin", fx.st.border)
        .percent_width(100.0)
        .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
}

fn panel_davinci(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let inner = width.saturating_sub(4) as usize;
    let scan_cols = inner.clamp(6, 20) as u16;

    // Sub-cell Braille instrument: a moving worm plus a sweeping scanner beam.
    let rows = 4u16;
    let mut worm = gibson::BrailleCanvas::new(scan_cols, rows);
    let mut beam = gibson::BrailleCanvas::new(scan_cols, rows);
    let pw = worm.pixel_width() as f32;
    let ph = worm.pixel_height() as f32;
    let t = app.elapsed();
    let mut prev: Option<(i32, i32)> = None;
    let mut x = 0i32;
    while x < worm.pixel_width() as i32 {
        let fx1 = x as f32 / pw;
        let y = (ph * 0.5
            + (fx1 * 9.0 + t * 2.2).sin() * (ph * 0.32)
            + (fx1 * 23.0 + t * 1.1).sin() * (ph * 0.08))
            .clamp(0.0, ph - 1.0);
        let yi = y.round() as i32;
        if let Some((px, py)) = prev {
            worm.line(px, py, x, yi);
        } else {
            worm.set(x, yi);
        }
        prev = Some((x, yi));
        x += 1;
    }
    // Scanner beam sweeps across the instrument.
    let beam_x = ((app.scan / 100.0) * (worm.pixel_width().saturating_sub(1) as f32)) as i32;
    beam.line(beam_x, 0, beam_x, worm.pixel_height() as i32 - 1);

    let mut rt = RichText::new().line(
        Line::new()
            .span(Span::styled("SCAN ", fx.st.muted))
            .span(Span::styled(
                gibson::node::SPINNER_BRAILLE
                    [(app.scan as usize) % gibson::node::SPINNER_BRAILLE.len()],
                fx.st.warning,
            ))
            .span(Span::styled("  sweep ", fx.st.muted))
            .span(Span::styled(format!("{:>3.0}%", app.scan), fx.st.text)),
    );

    for cy in 0..rows {
        let mut line = Line::new();
        for cx in 0..scan_cols {
            let b = beam.glyph_at(cx, cy);
            let w = worm.glyph_at(cx, cy);
            if let Some(ch) = b {
                let style = if fx.color {
                    Style::new().fg(fx.magenta)
                } else {
                    Style::default()
                };
                line = line.span(Span::styled(ch.to_string(), style));
            } else if let Some(ch) = w {
                let style = if fx.color {
                    Style::new().fg(fx.green)
                } else {
                    Style::default()
                };
                line = line.span(Span::styled(ch.to_string(), style));
            } else {
                line = line.span(Span::raw(" "));
            }
        }
        rt = rt.line(line);
    }
    rt = rt.line(
        Line::new()
            .span(Span::styled("SIG ", fx.st.muted))
            .span(Span::styled("da-vinci", fx.st.warning))
            .span(Span::styled("  financial worm ", fx.st.muted))
            .span(Span::styled("QUARANTINED", fx.st.success)),
    );
    tpanel("DA VINCI SCAN", fx.st.border)
        .percent_width(100.0)
        .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
}

fn panel_trace(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let inner = width.saturating_sub(4) as usize;
    let mut rt = RichText::new();
    if app.events.is_empty() {
        rt = rt.line(Line::new().span(Span::styled("no trace yet…", fx.st.faint)));
    } else {
        for e in app.events.iter().rev().take(10).rev() {
            let mut line = Line::new();
            for s in &e.spans {
                line = line.span(Span::styled(truncate(s.text.as_str(), inner), s.style));
            }
            rt = rt.line(line);
        }
    }
    tpanel("EVENT TRACE", fx.st.border)
        .percent_width(100.0)
        .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
}

fn panel_transfer(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let inner = width.saturating_sub(4) as usize;
    let bar_w = inner.saturating_sub(20).clamp(10, 48);
    // Before the first attempt fails the bar tracks the garbage discovery;
    // afterwards it tracks Joey's retry of the same file (the callback).
    let active = app.download > 0.0;
    let pct = if active { app.download } else { app.transfer }.clamp(0.0, 1.0);
    let mb = (pct * 256.0) as usize;
    let mut line = Line::new().span(Span::styled("garbage.bin ", fx.st.code));
    for s in fx.bar(pct, bar_w, fx.green, fx.magenta).spans {
        line = line.span(s);
    }
    let status = if active && pct >= 1.0 {
        format!(" {:>3}%  FILE VERIFIED", (pct * 100.0) as usize)
    } else {
        format!(" {:>3}%  {} / 256 MB", (pct * 100.0) as usize, mb)
    };
    line = line.span(Span::styled(status, fx.st.text));
    let mut rt = RichText::new()
        .line(line)
        .line(
            Line::new()
                .span(Span::styled("throughput ", fx.st.muted))
                .span(Span::styled(
                    format!("{:.1} MB/s", 18.0 + 22.0 * pct),
                    fx.st.success,
                ))
                .span(Span::styled("  │  checksum ", fx.st.muted))
                .span(Span::styled(
                    if active && pct >= 1.0 {
                        "sha256:ok ✓"
                    } else {
                        "sha256:e3b0…b855"
                    },
                    fx.st.code,
                )),
        )
        .line(fx.spark(&app.throughput, bar_w.min(40)));

    // Half-block RGB spectrum / core temperature field: one horizontal and two
    // vertical samples per cell, no graphics protocol. Under mono the color
    // quantizer strips the RGB and the block shapes remain as a density fallback.
    {
        let spec_w = bar_w.clamp(8, 40) as u16;
        let mut canvas = gibson::HalfBlockCanvas::new(spec_w, 2);
        if app.show_plasma {
            // Animated procedural plasma field (demoscene in a shell).
            gibson::render_plasma_halfblock(&mut canvas, app.elapsed(), 0.3);
        } else {
            let pw = canvas.pixel_width() as f32;
            let ph = canvas.pixel_height() as f32;
            for x in 0..canvas.pixel_width() as i32 {
                let t = x as f32 / pw;
                let energy = (t * 18.0 + app.elapsed() * 3.0).sin() * 0.5 + 0.5;
                let bars = (energy * (ph - 1.0)).round() as i32;
                for y in 0..=bars {
                    let yy = ph as i32 - 1 - y;
                    let g = (180.0 + 75.0 * (y as f32 / ph)).min(255.0) as u8;
                    let c = fx.green.lerp(fx.cyan, y as f32 / ph);
                    let (r, gg, b) = match c {
                        Color::Rgb(r, gg, b) => (r, gg, b),
                        _ => (g, g, g),
                    };
                    canvas.set_pixel(x, yy, (r, gg, b));
                }
            }
        }
        rt = rt.line(Line::raw(""));
        for l in canvas.to_rich_text().lines {
            rt = rt.line(l);
        }
    }

    tpanel("TRANSFER", fx.st.border)
        .percent_width(100.0)
        .height(8.0)
        .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
}

fn bottom_panel(app: &App, width: u16) -> Node {
    let fx = &app.fx;
    let inner = width.saturating_sub(4) as usize;
    match app.phase {
        Phase::Tactical => {
            let mut rt = RichText::new().line(
                Line::new()
                    .span(Span::styled("SELECT PAYLOAD ", fx.st.accent))
                    .span(Span::styled(
                        "↑/↓ or j/k · Enter to fire · 1-4",
                        fx.st.muted,
                    )),
            );
            for (i, opt) in TACTICAL.iter().enumerate() {
                let sel = i == app.selected;
                rt = rt.line(
                    Line::new()
                        .span(Span::styled(
                            if sel { "▶ " } else { "  " },
                            if sel { fx.st.warning } else { fx.st.faint },
                        ))
                        .span(Span::styled(format!("[{}] ", i + 1), fx.st.muted))
                        .span(Span::styled(
                            truncate(opt, inner.saturating_sub(8)),
                            if sel { Style::new().bold() } else { fx.st.text },
                        )),
                );
            }
            tpanel("TACTICAL PAYLOAD", fx.st.warning)
                .percent_width(100.0)
                .height(7.0)
                .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
        }
        Phase::Shell | Phase::Done => {
            let mut rt = RichText::new();
            for line in app.shell_log.iter().rev().take(4).rev() {
                rt = rt.line(line.clone());
            }
            let caret = if ((app.elapsed() * 2.0) as usize).is_multiple_of(2) {
                "▌"
            } else {
                " "
            };
            rt = rt.line(
                Line::new()
                    .span(Span::styled("root@gibson:~# ", fx.st.success))
                    .span(Span::styled(app.input.text.clone(), fx.st.text))
                    .span(Span::styled(caret, fx.st.accent)),
            );
            tpanel("ROOT SHELL", fx.st.success)
                .percent_width(100.0)
                .height(7.0)
                .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
        }
        _ => {
            let mut rt = RichText::new().line(
                Line::new()
                    .span(Span::styled(
                        format!("ACT · {} ", phase_name(app.phase)),
                        fx.st.accent,
                    ))
                    .span(Span::styled(
                        gibson::node::SPINNER_BRAILLE
                            [(app.elapsed() * 12.0) as usize % gibson::node::SPINNER_BRAILLE.len()],
                        fx.st.warning,
                    ))
                    .span(Span::styled("  hold tight…", fx.st.muted)),
            );
            rt = rt.line(Line::new().span(Span::styled(
                truncate(
                    &format!(
                        "breach {:>3}%  ·  garbage {:>3}%  ·  download {:>3}%  ·  deadline {:>3}%",
                        (app.breach * 100.0) as usize,
                        (app.transfer * 100.0) as usize,
                        (app.download * 100.0) as usize,
                        (app.deadline() * 100.0) as usize,
                    ),
                    inner,
                ),
                fx.st.text,
            )));
            if app.plague_active {
                rt = rt.line(Line::new().span(Span::styled(
                    truncate(
                        "⚠ THE PLAGUE active · hostile scan beam crossing the data city",
                        inner,
                    ),
                    fx.st.error,
                )));
            }
            tpanel("STATUS", fx.st.border)
                .percent_width(100.0)
                .height(4.0)
                .child(Node::rich_text_wrapped(rt, WrapMode::NoWrap))
        }
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

/// Capability overrides for fallback proofs: `--mono`, `--ansi16`, `--ansi256`,
/// `--truecolor`, `--no-sync`, `--no-insert-line`.
fn apply_capability_flags(ctx: &mut Context, args: &[String], no_color: bool) {
    use gibson::capability::ColorDepth;
    let has = |f: &str| args.iter().any(|a| a == f);
    let depth = if has("--mono") {
        Some(ColorDepth::Mono)
    } else if has("--ansi16") {
        Some(ColorDepth::Ansi16)
    } else if has("--ansi256") {
        Some(ColorDepth::Ansi256)
    } else if has("--truecolor") {
        Some(ColorDepth::TrueColor)
    } else if no_color {
        Some(ColorDepth::Mono)
    } else {
        None
    };
    if let Some(d) = depth {
        ctx.set_color_depth(d);
    }
    if has("--no-sync") {
        ctx.set_sync_updates(false);
    }
    if has("--no-insert-line") {
        let mut caps = ctx.capabilities();
        caps.insert_line = gibson::Capability::Unsupported;
        ctx.set_capabilities(caps);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let deterministic = args.iter().any(|a| a == "--deterministic");
    let freeze_at: Option<usize> = args
        .iter()
        .find_map(|a| a.strip_prefix("--freeze-at=").and_then(|v| v.parse().ok()));
    let auto = deterministic
        || args
            .iter()
            .any(|a| a == "--auto" || a == "--scripted" || a == "--headless");
    let inline = args.iter().any(|a| a == "--inline");
    let light = args.iter().any(|a| a == "--light");
    let dark = args.iter().any(|a| a == "--dark");
    let no_color = args.iter().any(|a| a == "--no-color");

    let fx = Fx::new(select_theme(light, dark, no_color), !no_color);
    let mut ctx = if inline {
        Context::inline()?
    } else {
        Context::fullscreen()?
    };
    if no_color {
        // Central color ladder: Mono strips every color attribute, even those
        // produced by sub-cell canvases.
        ctx.set_color_depth(gibson::ColorDepth::Mono);
    }
    ctx.set_max_fps(if auto { 240 } else { 60 });
    ctx.set_animation_interval(Duration::from_millis(if auto { 8 } else { 45 }));
    apply_capability_flags(&mut ctx, &args, no_color);
    // The damage map reads the renderer's own dirty-cell history.
    ctx.set_capture_damage(true);

    let mut app = App::new(fx, deterministic);
    if let Some(act) = args.iter().find_map(|a| a.strip_prefix("--act=")) {
        app.jump_to(act);
    }

    let mut iterations = 0usize;
    let cap = if auto { 8000 } else { usize::MAX };
    while !app.done && iterations < cap {
        iterations += 1;
        let frozen = deterministic && freeze_at.is_some_and(|n| iterations > n);
        if !frozen {
            app.time.advance();
            app.sample(&mut ctx);
            app.animate();
            if auto {
                app.auto_step();
            }
        }

        ctx.set_root(build_root(&app, &ctx));

        if auto {
            ctx.request_render();
            ctx.run_once(ctx.animation_interval())?;
        } else if let Some(event) = ctx.run_once(Duration::from_millis(50))? {
            if app.handle(&event) {
                ctx.request_render();
            }
        }
        if frozen {
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    // Flush one final frame so terminal/exit state is visible before leaving
    // the alternate screen.
    ctx.request_render();
    let _ = ctx.run_once(ctx.animation_interval());
    ctx.restore()?;

    let stats = ctx.stats();
    println!(
        "✔ Gibson connection closed. {} frames · {} B frames · {} B inserts · {} anchor resyncs",
        stats.frames, stats.frame_bytes, stats.insertion_bytes, stats.anchor_resyncs
    );
    Ok(())
}
