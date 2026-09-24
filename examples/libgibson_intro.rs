//! LibGibson's introductory short film: a finite agent harness becomes a city
//! of information, then a planet. Every frame is a pure projection of time.
//! Run: cargo run --release --example libgibson_intro -- --auto
//! Space pause; arrows skip acts; R replay; Esc quit. --stage=city,
//! --at=38 --freeze, --speed=2, --color=mono, --deterministic, --seconds=5.
#[path = "libgibson_intro/director.rs"]
pub mod director;
#[path = "libgibson_intro/harness.rs"]
mod harness;
#[path = "libgibson_intro/membrane.rs"]
mod membrane;
#[path = "libgibson_intro/identity.rs"]
pub mod identity;
#[path = "libgibson_intro/model.rs"]
pub mod model;
#[path = "libgibson_intro/world.rs"]
pub mod world;
use director::{Act, Director};
use gibson::cell::{Color, Style};
use gibson::input::{Event, KeyCode, KeyModifiers};
use gibson::{ColorDepth, Context, Node, Surface};
use std::time::{Duration, Instant};

/// Presentation bounds protect demo generation from pathological PTY sizes;
/// Context still owns actual terminal layout and clears the remainder normally.
pub fn frame(
    director: &Director,
    width: u16,
    height: u16,
    depth: ColorDepth,
    hints: bool,
) -> Surface {
    let w = width.min(320);
    let h = height.min(100);
    let content_h = h.saturating_sub(1);
    let mut out = match director.cue().act {
        Act::Harness => harness::render(w, content_h, director.seconds),
        Act::Membrane => membrane::render(
            &harness::render(w, content_h, 20.0),
            director.seconds,
            depth,
        ),
        _ => world::render(w, content_h, director.seconds, depth),
    };
    out.resize(w, h);
    if h > 0 && hints {
        let label = if director.paused {
            "PAUSED · space resume · ←/→ act · R replay · Esc exit"
        } else {
            "space pause · ←/→ act · R replay · Esc exit"
        };
        out.print_str(
            1,
            h - 1,
            label,
            Style::new().fg(Color::Rgb(93, 116, 140)),
            Some(w.saturating_sub(2)),
        );
    }
    out
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let has = |s: &str| args.iter().any(|a| a == s);
    let value = |p: &str| args.iter().find_map(|a| a.strip_prefix(p));
    if has("--help") {
        println!("libgibson_intro — a 72-second terminal short film\n\n--auto exit after finale; otherwise hold for replay\n--stage=harness|membrane|city|facades|couriers|ascent|planet\n--at=SECONDS --freeze --speed=FACTOR --deterministic\n--color=truecolor|ansi256|ansi16|mono --seconds=SMOKE_LIMIT\n--dump=PATH --width=120 --height=32 exports a developer RGB PPM\nSpace pause; arrows skip; R replay; Esc or Ctrl-C exit.");
        return Ok(());
    }
    let mut d = Director::default();
    if let Some(stage) = value("--stage=") {
        if !d.stage(stage) {
            return Err(format!("unknown stage: {stage}").into());
        }
    }
    if let Some(s) = value("--at=") {
        d.seek(s.parse()?)
    }
    d.paused = has("--freeze");
    let speed = value("--speed=")
        .and_then(|s| s.parse::<f32>().ok())
        .filter(|s| s.is_finite())
        .unwrap_or(1.0)
        .clamp(0.1, 30.0);
    let depth = match value("--color=") {
        Some("mono") => Some(ColorDepth::Mono),
        Some("ansi16") => Some(ColorDepth::Ansi16),
        Some("ansi256") => Some(ColorDepth::Ansi256),
        Some("truecolor") => Some(ColorDepth::TrueColor),
        None => None,
        Some(s) => return Err(format!("unknown color: {s}").into()),
    };
    if let Some(path) = value("--dump=") {
        let w = value("--width=").unwrap_or("120").parse::<u16>()?.min(320);
        let h = value("--height=").unwrap_or("32").parse::<u16>()?.min(100);
        // Developer witness includes actual cell colors, not an image protocol.
        let surface = frame(&d, w, h, depth.unwrap_or(ColorDepth::TrueColor), false);
        let mut raster = gibson::raster::RgbRaster::new(w, h.saturating_mul(2));
        for y in 0..h {
            for x in 0..w {
                if let Some(c) = surface.get(x, y) {
                    let bg = c.style.bg.unwrap_or(Color::Black).to_rgb();
                    let fg = c.style.fg.unwrap_or(Color::White).to_rgb();
                    let (top, bottom) = match c.glyph.grapheme.as_str() {
                        "▀" => (fg, bg),
                        "▄" => (bg, fg),
                        " " => (bg, bg),
                        _ => (fg, bg),
                    };
                    raster.set(x as i32, y as i32 * 2, top);
                    raster.set(x as i32, y as i32 * 2 + 1, bottom);
                }
            }
        }
        raster.write_ppm(std::fs::File::create(path)?)?;
        return Ok(());
    }
    let mut ctx = Context::fullscreen()?;
    if let Some(depth) = depth {
        ctx.set_color_depth(depth)
    }
    ctx.set_max_fps(60);
    let cadence = Duration::from_secs_f64(1.0 / 60.0);
    ctx.set_animation_interval(cadence);
    let mut last = Instant::now();
    let started = last;
    let limit = value("--seconds=")
        .and_then(|s| s.parse::<f32>().ok())
        .filter(|n| n.is_finite() && *n > 0.0);
    let fixed = gibson::FixedStepClock::new(cadence);
    let mut realized = None;
    loop {
        let (w, h) = ctx.session.terminal_size();
        let key = (d.seconds.to_bits(), w, h, d.paused);
        if realized != Some(key) {
            ctx.set_root(Node::raster(frame(
                &d,
                w,
                h,
                ctx.session.color_depth(),
                !has("--no-hints"),
            )));
            realized = Some(key);
        }
        // A frozen image reuses its ordinary Node. Keep input responsive without
        // regenerating the raster or spinning on an expired frame deadline.
        let event = if ctx.scheduler.is_dirty {
            ctx.run_once(cadence)?
        } else if ctx.session.is_tty {
            ctx.poll_event(cadence)?
        } else {
            std::thread::sleep(cadence);
            None
        };
        if let Some(Event::Key(k)) = event {
            match k.code {
                KeyCode::Esc => break,
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Char(' ') => d.paused = !d.paused,
                KeyCode::Char('r' | 'R') => d = Director::default(),
                KeyCode::Right => d.skip(true),
                KeyCode::Left => d.skip(false),
                _ => {}
            }
        }
        if (has("--auto") && d.seconds >= 72.0)
            || limit.is_some_and(|n| started.elapsed().as_secs_f32() >= n)
        {
            break;
        }
        let now = Instant::now();
        let dt = if has("--deterministic") {
            fixed.advance();
            cadence.as_secs_f32()
        } else {
            now.duration_since(last).as_secs_f32().min(0.2)
        };
        last = now;
        d.advance(dt * speed);
    }
    ctx.restore()?;
    Ok(())
}
