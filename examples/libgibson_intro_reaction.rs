//! LibGibson intro — reaction cut. A keyed greenscreen subject is composited
//! *into* the existing `libgibson_intro` short film as though reacting to it while
//! it unfolds. Not a video in a box, and not "intro then clip": one directed edit
//! where a live-action keyed reaction is edited into the terminal-native story
//! without degrading the story underneath.
//!
//! Architecture (see docs/FRANK_REACTION_CUT_PLAN.md):
//!   * the original intro Surface is generated exactly as `libgibson_intro` does
//!     (`intro::frame`, a pure function of narrative time) — one renderer;
//!   * a two-clock reaction [`director`] maps edit time -> base-intro time
//!     (continue/hold/slow) and -> a source window, deterministically;
//!   * a surface-local [`compositor`] patches ONLY the cells the transformed keyed
//!     subject covers, leaving the rest of the intro byte-identical.
//!
//! Deterministic + seekable: `--at=<edit s>`, `--stage=<cue>`, `--freeze`,
//! `--dump=PATH`. Run live with no args (needs a Braille-capable, ideally truecolor
//! terminal). The Filthy Frank source clip is a LOCAL stress-test asset and is not
//! shipped; point at any greenscreen clip with `--clip=`.
//!
//! Run: cargo run --release --example libgibson_intro_reaction

#[path = "temporal_video_compositor/bake.rs"]
mod bake;
#[path = "libgibson_intro_reaction/compositor.rs"]
mod compositor;
#[path = "libgibson_intro_reaction/director.rs"]
mod director;
#[path = "temporal_video_compositor/film.rs"]
mod film;
#[allow(dead_code)]
#[path = "libgibson_intro.rs"]
pub mod intro;
#[path = "temporal_video_compositor/key.rs"]
mod key;
#[path = "libgibson_intro_reaction/transform.rs"]
mod transform;

use compositor::{CompositeOpts, CompositeStats};
use film::KeyedFilm;
use gibson::cell::Color;
use gibson::input::{Event, KeyCode, KeyModifiers};
use gibson::{ColorDepth, Context, Node, SubcellGlyphMode, Surface};
use key::KeyParams;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Default bake resolution for a cue window: 16:9, enough source detail for a
/// terminal-cell render, small enough that ten short windows stay cheap.
const FILM_W: u32 = 240;
const FILM_H: u32 = 135;
const FILM_FPS: f32 = 24.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let has = |s: &str| args.iter().any(|a| a == s);
    let value = |p: &str| args.iter().find_map(|a| a.strip_prefix(p));

    if has("--help") {
        println!(
            "libgibson_intro_reaction — a keyed reaction edited into the LibGibson intro\n\n\
             --at=EDIT_SECONDS      seek the edit clock (deterministic)\n\
             --stage=CUE            jump to a cue: {}\n\
             --freeze               hold the sought frame\n\
             --dump=PATH            export a developer RGB PPM and exit\n\
             --width=120 --height=32  dump/render size in cells\n\
             --color=truecolor|ansi256|ansi16|mono\n\
             --glyphs=auto|braille|halfblock|block|ascii\n\
             --clip=PATH            greenscreen source (default: search ~/Downloads)\n\
             --film-width/--film-height/--film-fps  bake resolution\n\
             --smooth               bilinear source sampling (premultiplied)\n\
             --linear               composite the matte in linear light\n\
             --no-reaction          base intro only (the honest baseline)\n\
             --profile              headless performance receipts, then exit\n\
             --deterministic --auto --seconds=N\n\n\
             The source clip is a local stress-test asset and is not shipped.",
            director::CUES
                .iter()
                .map(|c| c.name)
                .collect::<Vec<_>>()
                .join(", ")
        );
        return Ok(());
    }

    let depth = match value("--color=") {
        Some("mono") => Some(ColorDepth::Mono),
        Some("ansi16") => Some(ColorDepth::Ansi16),
        Some("ansi256") => Some(ColorDepth::Ansi256),
        Some("truecolor") => Some(ColorDepth::TrueColor),
        None => None,
        Some(s) => return Err(format!("unknown color: {s}").into()),
    };
    let glyph_mode = gibson::detect_glyph_mode(value("--glyphs="), |k| std::env::var(k).ok())?;
    let opts = CompositeOpts {
        bilinear: has("--smooth"),
        linear_light: has("--linear"),
    };
    let kp = KeyParams::default();
    let film_w = value("--film-width=")
        .and_then(|s| s.parse().ok())
        .unwrap_or(FILM_W);
    let film_h = value("--film-height=")
        .and_then(|s| s.parse().ok())
        .unwrap_or(FILM_H);
    let film_fps = value("--film-fps=")
        .and_then(|s| s.parse().ok())
        .unwrap_or(FILM_FPS);
    let reaction = !has("--no-reaction");

    // Resolve the initial edit time from --at / --stage.
    let mut edit_seconds = 0.0f32;
    if let Some(stage) = value("--stage=") {
        let i = director::stage_index(stage).ok_or_else(|| format!("unknown stage: {stage}"))?;
        edit_seconds = director::cue_edit_start(i);
    }
    if let Some(s) = value("--at=") {
        edit_seconds = s.parse()?;
    }

    // Bake only the cue windows we actually use (never the full ~302 s clip).
    let films: Vec<KeyedFilm> = if reaction {
        let clip = match value("--clip=") {
            Some(p) => PathBuf::from(p),
            None => default_clip().ok_or_else(|| {
                "no greenscreen clip found in ~/Downloads; pass --clip=PATH \
                 (or --no-reaction for the base intro only)"
                    .to_string()
            })?,
        };
        match bake_cues(&clip, film_w, film_h, film_fps) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("reaction bake failed: {e}");
                eprintln!("hint: pass --clip=PATH to a greenscreen clip, or --no-reaction.");
                return Err(e.into());
            }
        }
    } else {
        Vec::new()
    };

    if has("--profile") {
        run_profile(&films, glyph_mode, &kp, opts, reaction);
        return Ok(());
    }

    // Developer witness: deterministic single-frame RGB PPM (mirrors libgibson_intro
    // --dump, so the two films are inspected the same way).
    if let Some(path) = value("--dump=") {
        let w = value("--width=").unwrap_or("120").parse::<u16>()?.min(320);
        let h = value("--height=").unwrap_or("32").parse::<u16>()?.min(100);
        let (surface, _r, _s) = render_frame(
            &films,
            w,
            h,
            depth.unwrap_or(ColorDepth::TrueColor),
            glyph_mode,
            edit_seconds,
            &kp,
            opts,
            reaction,
        );
        dump_ppm(&surface, w, h, path)?;
        return Ok(());
    }

    // First-contact bootup prologue — the same one the original film opens with,
    // reused verbatim. It owns its own Context and releases the terminal before the
    // fullscreen lease below. Skipped for direct-inspection / non-interactive paths
    // (--stage/--at/--seconds already set an entry point, and a non-tty can't show it).
    let run_prologue = !has("--no-prologue")
        && value("--stage=").is_none()
        && value("--at=").is_none()
        && value("--seconds=").is_none()
        && std::io::stdout().is_terminal();
    if run_prologue {
        match intro::prologue::run(depth, has("--deterministic"))? {
            intro::prologue::Outcome::Quit => return Ok(()),
            intro::prologue::Outcome::Proceed => {}
        }
    }

    // Live playback.
    let mut ctx = Context::fullscreen()?;
    if let Some(depth) = depth {
        ctx.set_color_depth(depth);
    }
    ctx.set_max_fps(60);
    let cadence = Duration::from_secs_f64(1.0 / 60.0);
    ctx.set_animation_interval(cadence);
    let started = Instant::now();
    let mut last = started;
    let mut paused = has("--freeze");
    let limit = value("--seconds=")
        .and_then(|s| s.parse::<f32>().ok())
        .filter(|n| n.is_finite() && *n > 0.0);
    let fixed = gibson::FixedStepClock::new(cadence);
    let total = director::total_edit_seconds();
    let mut realized: Option<(u32, u16, u16)> = None;
    loop {
        let (w, h) = ctx.session.terminal_size();
        let key = (edit_seconds.to_bits(), w, h);
        if realized != Some(key) {
            let (surface, _r, _s) = render_frame(
                &films,
                w,
                h,
                ctx.session.color_depth(),
                glyph_mode,
                edit_seconds,
                &kp,
                opts,
                reaction,
            );
            ctx.set_root(Node::raster(surface));
            realized = Some(key);
        }
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
                KeyCode::Char(' ') => paused = !paused,
                KeyCode::Char('r' | 'R') => edit_seconds = 0.0,
                KeyCode::Right => edit_seconds = (edit_seconds + 2.0).min(total),
                KeyCode::Left => edit_seconds = (edit_seconds - 2.0).max(0.0),
                _ => {}
            }
        }
        if (has("--auto") && edit_seconds >= total)
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
        if !paused {
            edit_seconds += dt;
            // Hold the final sting rather than looping unless asked.
            if edit_seconds > total {
                edit_seconds = total;
            }
        }
    }
    ctx.restore()?;
    Ok(())
}

/// Render one reaction-cut frame deterministically for an edit time. Returns the
/// patched surface, the resolved director state, and the composite stats.
#[allow(clippy::too_many_arguments)]
fn render_frame(
    films: &[KeyedFilm],
    w: u16,
    h: u16,
    depth: ColorDepth,
    glyph_mode: SubcellGlyphMode,
    edit_seconds: f32,
    kp: &KeyParams,
    opts: CompositeOpts,
    reaction: bool,
) -> (Surface, director::Resolved, CompositeStats) {
    let r = director::resolve(edit_seconds);
    // Base intro, generated exactly as the original film does.
    let mut d = intro::director::Director::default();
    d.seek(r.intro_seconds);
    let mut surface = intro::frame(&d, w, h, depth, false);
    gibson::transcode_surface_glyphs(&mut surface, glyph_mode);

    let mut stats = CompositeStats::default();
    if reaction {
        if let Some(ci) = r.cue_index {
            if let Some(film) = films.get(ci) {
                if film.nframes > 0 {
                    let cue = &director::CUES[ci];
                    let p = r.src_progress; // normalized cue progress
                    let beat = transform::resolve_effect(cue.effect, p);
                    let src_p = beat.src_progress.unwrap_or(p);
                    let idx = compositor::frame_index(film.nframes, src_p);
                    let t = transform::Transform::resolve(
                        &cue.framing,
                        &beat,
                        film.film_w,
                        film.film_h,
                        w as u32 * 2,
                        h as u32 * 4,
                    );
                    stats = compositor::composite_into(
                        &mut surface,
                        film,
                        film.frame(idx),
                        &t,
                        kp,
                        glyph_mode,
                        opts,
                    );
                }
            }
        }
    }
    (surface, r, stats)
}

/// Bake every cue's source window into a cached raw-RGB film (index-parallel to
/// [`director::CUES`]).
fn bake_cues(
    clip: &std::path::Path,
    film_w: u32,
    film_h: u32,
    fps: f32,
) -> Result<Vec<KeyedFilm>, String> {
    let mut films = Vec::with_capacity(director::CUES.len());
    for cue in director::CUES.iter() {
        let spec = bake::BakeSpec {
            clip: clip.to_path_buf(),
            start_s: cue.src_start,
            dur_s: cue.src_dur,
            film_w,
            film_h,
            fps,
        };
        let (film, _path) = bake::bake(&spec)?;
        films.push(film);
    }
    Ok(films)
}

/// Discover a greenscreen clip in ~/Downloads: prefer a name containing "frank"
/// and "stop", else the first `*.webm`.
fn default_clip() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let downloads = PathBuf::from(home).join("Downloads");
    let mut webms: Vec<PathBuf> = std::fs::read_dir(&downloads)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("webm"))
        .collect();
    webms.sort();
    webms
        .iter()
        .find(|p| {
            let n = p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            n.contains("frank") && n.contains("stop")
        })
        .cloned()
        .or_else(|| webms.first().cloned())
}

/// Write a developer RGB PPM (two vertical samples per cell, decoding the emitted
/// subcell glyph) — the same witness format `libgibson_intro --dump` uses.
fn dump_ppm(
    surface: &Surface,
    w: u16,
    h: u16,
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut raster = gibson::raster::RgbRaster::new(w, h.saturating_mul(2));
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = surface.get(x, y) {
                let bg = c.style.bg.unwrap_or(Color::Black).to_rgb();
                let fg = c.style.fg.unwrap_or(Color::White).to_rgb();
                let (top, bottom) = match c.glyph.grapheme.as_str() {
                    "\u{2580}" => (fg, bg),
                    "\u{2584}" => (bg, fg),
                    " " => (bg, bg),
                    _ => (fg, bg),
                };
                raster.set(x as i32, y as i32 * 2, top);
                raster.set(x as i32, y as i32 * 2 + 1, bottom);
            }
        }
    }
    raster.write_ppm(std::fs::File::create(path)?)?;
    Ok(())
}

/// Headless performance receipts: base-intro generation vs the surface-local
/// composite patch, and the local patch vs a full-grid reprojection, at two sizes.
fn run_profile(
    films: &[KeyedFilm],
    glyph_mode: SubcellGlyphMode,
    kp: &KeyParams,
    opts: CompositeOpts,
    reaction: bool,
) {
    use gibson::temporal::project_rgb_subcells;
    println!("libgibson_intro_reaction --profile");
    println!(
        "reaction={reaction} smooth={} linear={} cues={}\n",
        opts.bilinear,
        opts.linear_light,
        director::CUES.len()
    );
    for &(w, h) in &[(120u16, 32u16), (160u16, 40u16)] {
        println!(
            "== {w}x{h} cells ({} logical subpixels) ==",
            (w as u32 * 2) * (h as u32 * 4)
        );
        // A full-grid reprojection cost: what "reproject the whole composited frame"
        // would pay every frame (the path the surface-local compositor avoids).
        let full_start = Instant::now();
        let iters = 30;
        for _ in 0..iters {
            for _ in 0..(w as u32 * h as u32) {
                std::hint::black_box(project_rgb_subcells(std::hint::black_box(
                    [[80u8, 90, 110]; 8],
                )));
            }
        }
        let full_us = full_start.elapsed().as_secs_f64() * 1e6 / iters as f64;

        for &(label, edit_seconds) in &[
            ("normal (city_reveal)", director::cue_edit_start(4) + 2.0),
            ("punch  (escalate)", director::cue_edit_start(2) + 1.5),
            ("quiet  (ascent)", director::cue_edit_start(7) + 2.0),
        ] {
            let mut base_us = 0.0;
            let mut comp_us = 0.0;
            let mut stats = CompositeStats::default();
            let n = 30;
            for _ in 0..n {
                let r = director::resolve(edit_seconds);
                let mut d = intro::director::Director::default();
                d.seek(r.intro_seconds);
                let t0 = Instant::now();
                let mut surface = intro::frame(&d, w, h, ColorDepth::TrueColor, false);
                gibson::transcode_surface_glyphs(&mut surface, glyph_mode);
                base_us += t0.elapsed().as_secs_f64() * 1e6;
                if let Some(film) = r.cue_index.and_then(|ci| films.get(ci)) {
                    if film.nframes > 0 {
                        let cue = &director::CUES[r.cue_index.unwrap()];
                        let p = r.src_progress;
                        let beat = transform::resolve_effect(cue.effect, p);
                        let src_p = beat.src_progress.unwrap_or(p);
                        let idx = compositor::frame_index(film.nframes, src_p);
                        let tr = transform::Transform::resolve(
                            &cue.framing,
                            &beat,
                            film.film_w,
                            film.film_h,
                            w as u32 * 2,
                            h as u32 * 4,
                        );
                        let t1 = Instant::now();
                        stats = compositor::composite_into(
                            &mut surface,
                            film,
                            film.frame(idx),
                            &tr,
                            kp,
                            glyph_mode,
                            opts,
                        );
                        comp_us += t1.elapsed().as_secs_f64() * 1e6;
                    }
                }
            }
            base_us /= n as f64;
            comp_us /= n as f64;
            let grid = w as u32 * h as u32;
            println!(
                "  {label:22} base {base_us:8.1} us | patch {comp_us:8.1} us | \
                 composited {}/{} cells ({:.1}% of grid) | local+base {:.1} us",
                stats.cells_composited,
                grid,
                100.0 * stats.cells_composited as f64 / grid as f64,
                base_us + comp_us
            );
        }
        println!("  full-grid reproject (avoided): {full_us:8.1} us/frame\n");
    }
    println!(
        "The surface-local compositor reprojects only the subject's cells; the base intro\n\
         is generated natively (no reprojection). Wire bytes/missed-deadlines require the\n\
         live differential renderer — see temporal_video_compositor --profile for the\n\
         projection-cost economics on the pure keyed-video path."
    );
}
