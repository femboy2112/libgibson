//! Temporal Video Compositor — the keyed-video moonshot.
//!
//! Chroma-keys a greenscreen clip (the green field is *transparency*, not content),
//! composites the keyed foreground over a LibGibson-rendered background, and feeds
//! the composited RGB to the **same** subcell projector every other example uses.
//! There is **one renderer**: each frame is an ordinary [`gibson::Surface`].
//!
//! The greenscreen is the structural advantage. Only the subject's bounding box is
//! dirty each source frame, so we reproject only the union of the previous and
//! current bbox with [`TemporalDisplayProcessor::set_target_region`]; the pure-key
//! regions (measured ~55–70% of the frame) are never touched. The moving subject is
//! held **static** (motion cannot be temporally accumulated — honest); the static
//! designed background is what acquires a temporal lock and refines.
//!
//! ## Honesty
//! Default output is **static** and looks like the released engine. Temporal
//! refinement is opt-in (`--mode=temporal --measured-hz=<observed Hz>`), because
//! cadence cannot be observed through a PTY (measure it with
//! `temporal_cadence_beacon`). No claim of "video playback", super-resolution, or
//! universal terminal support: the honest win is a holy-shit keyed composite over a
//! temporally-stabilized background, exploiting greenscreen sparsity.
//!
//! Video ingest uses ffmpeg to bake a short segment into a cached raw-RGB film
//! (offline prep); the runtime never touches the codec.
//!
//! Examples:
//!   cargo run --release --example temporal_video_compositor
//!   cargo run --release --example temporal_video_compositor -- --scene=vapor
//!   cargo run --release --example temporal_video_compositor -- --mode=temporal --measured-hz=120
//!   cargo run --release --example temporal_video_compositor -- --profile   # headless perf receipts
//!   cargo run --release --example temporal_video_compositor -- --clip=/path/to/greenscreen.webm

#[path = "temporal_video_compositor/bake.rs"]
mod bake;
#[path = "temporal_video_compositor/film.rs"]
mod film;
#[path = "temporal_video_compositor/key.rs"]
mod key;
#[path = "temporal_video_compositor/scene.rs"]
mod scene;

use bake::BakeSpec;
use film::KeyedFilm;
use key::{Bbox, KeyParams};
use scene::Scene;

use gibson::{
    BorderType, Color, Context, FramePacing, Node, PresentationProfile, ResetPolicy, Style,
    SubcellGlyphMode, TemporalDiagnostics, TemporalDisplayProcessor, TemporalSafetyPolicy,
};
use std::io;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Raw,      // scene = Void, static composite (the "don't just use black" baseline)
    Keyed,    // designed scene, static composite (the hero default)
    Temporal, // designed scene + temporal refinement on the settled background
    Diag,     // keyed + full bbox overlay + verbose telemetry
}

impl Mode {
    fn parse(s: &str) -> Option<Mode> {
        match s.to_ascii_lowercase().as_str() {
            "raw" => Some(Mode::Raw),
            "keyed" | "composite" => Some(Mode::Keyed),
            "temporal" => Some(Mode::Temporal),
            "diag" | "diagnostic" => Some(Mode::Diag),
            _ => None,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Mode::Raw => "RAW/STATIC over VOID",
            Mode::Keyed => "KEYED COMPOSITE (static)",
            Mode::Temporal => "TEMPORAL COMPOSITE",
            Mode::Diag => "DIAGNOSTIC",
        }
    }
}

fn arg_u32(name: &str, default: u32) -> u32 {
    let prefix = format!("--{name}=");
    std::env::args()
        .find_map(|a| a.strip_prefix(&prefix).and_then(|v| v.parse().ok()))
        .unwrap_or(default)
}
fn arg_f32(name: &str) -> Option<f32> {
    let prefix = format!("--{name}=");
    std::env::args().find_map(|a| a.strip_prefix(&prefix).and_then(|v| v.parse().ok()))
}
fn arg_str(name: &str) -> Option<String> {
    let prefix = format!("--{name}=");
    std::env::args().find_map(|a| a.strip_prefix(&prefix).map(|v| v.to_string()))
}
fn has(flag: &str) -> bool {
    std::env::args().any(|a| a == flag)
}
fn arg_wh(name: &str) -> Option<(u32, u32)> {
    let s = arg_str(name)?;
    let (w, h) = s.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?))
}

/// The default clip: the greenscreen template in ~/Downloads, discovered by glob.
fn default_clip() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    let dl = std::path::Path::new(&home).join("Downloads");
    let mut hit: Option<std::path::PathBuf> = None;
    for entry in std::fs::read_dir(&dl).ok()?.flatten() {
        let name = entry.file_name();
        let n = name.to_string_lossy().to_ascii_lowercase();
        if n.ends_with(".webm") && n.contains("frank") && n.contains("stop") {
            return Some(entry.path());
        }
        if hit.is_none() && n.ends_with(".webm") {
            hit = Some(entry.path());
        }
    }
    hit
}

/// One source frame's compositing state; `at(lx, ly)` is the subpixel closure the
/// projector samples. Holding it in a struct keeps the sample call arg-light.
struct FrameComposite<'a> {
    film: &'a KeyedFilm,
    bytes: &'a [u8],
    scene: Scene,
    subw: u32,
    subh: u32,
    kp: KeyParams,
    bbox_sub: Option<scene::SubBox>,
    bg_t: f32,
    diag_box: bool,
}

impl FrameComposite<'_> {
    #[inline]
    fn at(&self, lx: u16, ly: u16) -> [u8; 3] {
        let (lxu, lyu) = (lx as u32, ly as u32);
        let bg = scene::background(self.scene, self.subw, self.subh, lxu, lyu, self.bg_t);
        let comp = self
            .film
            .composite_subpixel(self.bytes, self.subw, self.subh, lxu, lyu, bg, &self.kp);
        let comp = scene::hud_overlay(comp, self.scene, lxu, lyu, self.bbox_sub, self.bg_t);
        if self.diag_box {
            if let Some((bx, by, bw, bh)) = self.bbox_sub {
                let on_edge =
                    (lxu == bx || lxu == bx + bw.saturating_sub(1)) && lyu >= by && lyu < by + bh
                        || (lyu == by || lyu == by + bh.saturating_sub(1))
                            && lxu >= bx
                            && lxu < bx + bw;
                if on_edge {
                    return [255, 90, 90];
                }
            }
        }
        comp
    }
}

/// bbox union in cells.
fn union_cells(a: (u16, u16, u16, u16), b: (u16, u16, u16, u16)) -> (u16, u16, u16, u16) {
    let empty = |r: (u16, u16, u16, u16)| r.2 == 0 || r.3 == 0;
    if empty(a) {
        return b;
    }
    if empty(b) {
        return a;
    }
    let x0 = a.0.min(b.0);
    let y0 = a.1.min(b.1);
    let x1 = (a.0 + a.2).max(b.0 + b.2);
    let y1 = (a.1 + a.3).max(b.1 + b.3);
    (x0, y0, x1 - x0, y1 - y0)
}

struct Cfg {
    scene: Scene,
    mode: Mode,
    kp: KeyParams,
    animate_bg: bool,
    transport: Option<f32>,
}

fn main() -> io::Result<()> {
    if has("--profile") {
        return run_profile();
    }

    let mode = arg_str("mode")
        .and_then(|s| Mode::parse(&s))
        .unwrap_or(Mode::Keyed);
    let scene = if mode == Mode::Raw {
        Scene::Void
    } else {
        arg_str("scene")
            .and_then(|s| Scene::parse(&s))
            .unwrap_or(Scene::BlackIce)
    };
    let cfg = Cfg {
        scene,
        mode,
        kp: KeyParams::default(),
        animate_bg: has("--animate-bg"),
        transport: arg_f32("transport"),
    };

    let clip = arg_str("clip")
        .map(std::path::PathBuf::from)
        .or_else(default_clip)
        .ok_or_else(|| {
            io::Error::other("no clip: pass --clip=PATH (no *.webm found in ~/Downloads)")
        })?;
    let (fw, fh) = arg_wh("film").unwrap_or((320, 180));
    let spec = BakeSpec {
        clip: clip.clone(),
        start_s: arg_f32("start").unwrap_or(2.0),
        dur_s: arg_f32("dur").unwrap_or(6.0),
        film_w: fw,
        film_h: fh,
        fps: arg_f32("fps").unwrap_or(29.0),
    };
    eprintln!("baking {} …", clip.display());
    let (video, cache) = bake::bake(&spec).map_err(io::Error::other)?;
    eprintln!(
        "baked {} frames @ {}x{} ({} fps) -> {}",
        video.nframes,
        video.film_w,
        video.film_h,
        video.fps,
        cache.display()
    );

    let hz = arg_u32("hz", 60).clamp(1, 1000);
    let seconds = arg_u32("seconds", 20).clamp(2, 3600);
    let measured_hz = arg_f32("measured-hz");
    let survival = arg_f32("survival").unwrap_or(0.97);
    let jitter_p95 = arg_f32("jitter-p95").unwrap_or(0.5);
    let depth_cap = arg_f32("depth-cap");
    let dither = !has("--no-dither");
    let sync = !has("--no-sync");
    let still = arg_u32("still", u32::MAX);

    let (tw, th) = arg_wh("region")
        .map(|(w, h)| (w as u16, h as u16))
        .or_else(|| crossterm::terminal::size().ok())
        .unwrap_or((120, 36));
    let cols = tw.saturating_sub(2).clamp(20, 400);
    let rows = th.saturating_sub(5).clamp(10, 200);
    let (subw, subh) = (cols as u32 * 2, rows as u32 * 4);

    let mut processor =
        TemporalDisplayProcessor::new(cols, rows, SubcellGlyphMode::Braille2x4, 0x5A17);
    processor.set_dither(dither);
    if let Some(cap) = depth_cap {
        let mut policy = TemporalSafetyPolicy::default();
        policy.max_luminance_depth = cap;
        processor.set_policy(policy);
    }
    if let Some(l) = cfg.transport {
        processor.set_transport(Some(l), None);
    }
    let temporal = cfg.mode == Mode::Temporal;
    if temporal {
        if let Some(mhz) = measured_hz {
            processor.set_profile(PresentationProfile::measured(mhz, survival, jitter_p95));
        } else {
            eprintln!(
                "note: --mode=temporal needs --measured-hz=<observed Hz>; measure with \
                 temporal_cadence_beacon. Rendering static."
            );
        }
    }

    let mut ctx = Context::fullscreen()?;
    if !ctx.is_interactive() {
        return Err(io::Error::other(
            "temporal_video_compositor requires an interactive TTY (use --profile for headless)",
        ));
    }
    ctx.set_max_fps(hz);
    ctx.set_frame_pacing(FramePacing::PhaseLocked);
    ctx.set_sync_updates(sync);
    processor.set_color_depth(ctx.capabilities().color_depth);

    let diag_box = cfg.mode == Mode::Diag;

    // Seed with the first (or --still) frame, projected in full.
    let first = still.min(video.nframes as u32 - 1) as usize;
    let (fb0, opaque0) = video.foreground_stats(video.frame(first), &cfg.kp);
    let sub0 = video.bbox_to_sub(&fb0, subw, subh);
    let fc0 = FrameComposite {
        film: &video,
        bytes: video.frame(first),
        scene: cfg.scene,
        subw,
        subh,
        kp: cfg.kp,
        bbox_sub: if sub0.is_empty() {
            None
        } else {
            Some((sub0.x, sub0.y, sub0.w, sub0.h))
        },
        bg_t: 0.0,
        diag_box,
    };
    processor.set_target_image(|lx, ly| fc0.at(lx, ly), ResetPolicy::Reset);
    let mut prev_cells = video.bbox_to_cells(&fb0, cols, rows);
    processor.clear_motion();
    processor.set_motion_region(prev_cells.0, prev_cells.1, prev_cells.2, prev_cells.3, true);
    let mut last_opaque = opaque0;
    let mut last_fb = fb0;
    let mut dirty_cells = prev_cells.2 as usize * prev_cells.3 as usize;

    let dt = 1.0 / hz as f32;
    let frames = hz as u64 * seconds as u64;
    let start = Instant::now();
    let mut cur_src = first;
    let held = still != u32::MAX;

    for frame in 0..frames {
        // Which source frame should be showing now?
        let src = if held {
            first
        } else {
            let elapsed = start.elapsed().as_secs_f32();
            (first + (elapsed * video.fps) as usize) % video.nframes
        };

        if src != cur_src || frame == 0 {
            let (fb, opaque) = video.foreground_stats(video.frame(src), &cfg.kp);
            let sub = video.bbox_to_sub(&fb, subw, subh);
            let cur_cells = video.bbox_to_cells(&fb, cols, rows);
            let bg_t = if cfg.animate_bg {
                start.elapsed().as_secs_f32()
            } else {
                0.0
            };
            let fc = FrameComposite {
                film: &video,
                bytes: video.frame(src),
                scene: cfg.scene,
                subw,
                subh,
                kp: cfg.kp,
                bbox_sub: if sub.is_empty() {
                    None
                } else {
                    Some((sub.x, sub.y, sub.w, sub.h))
                },
                bg_t,
                diag_box,
            };
            if cfg.animate_bg {
                // Animated background dirties the whole frame: full reprojection.
                processor.set_target_image(|lx, ly| fc.at(lx, ly), ResetPolicy::Keep);
                dirty_cells = cols as usize * rows as usize;
            } else {
                // Static background: only the union of old+new bbox is dirty.
                let u = union_cells(prev_cells, cur_cells);
                processor.set_target_region(
                    u.0,
                    u.1,
                    u.2,
                    u.3,
                    |lx, ly| fc.at(lx, ly),
                    ResetPolicy::Reset,
                );
                dirty_cells = u.2 as usize * u.3 as usize;
            }
            processor.clear_motion();
            processor.set_motion_region(cur_cells.0, cur_cells.1, cur_cells.2, cur_cells.3, true);
            prev_cells = cur_cells;
            last_opaque = opaque;
            last_fb = fb;
            cur_src = src;
        }

        let missed = ctx.missed_periods_last_frame();
        let hero = if temporal {
            processor.advance(missed)
        } else {
            processor.static_fallback()
        };
        let diag = processor.diagnostics();

        let cover = 100.0 * (1.0 - last_opaque as f32 / (video.film_w * video.film_h) as f32);
        let title = format!(
            "TEMPORAL VIDEO COMPOSITOR  ·  {}  ·  scene={}  ·  {cols}×{rows} cells",
            cfg.mode.label(),
            cfg.scene.label()
        );
        let (acq_text, acq_color) = acq_line(cfg.mode, temporal, &diag);
        let root = Node::col()
            .child(Node::text(
                title,
                Style::default().fg(Color::Rgb(150, 220, 235)),
            ))
            .child(
                Node::panel(
                    "COMPOSITE",
                    BorderType::Rounded,
                    Style::default().fg(acq_color),
                )
                .flex_grow(1.0)
                .child(Node::raster(hero)),
            )
            .child(Node::text(acq_text, Style::default().fg(acq_color)))
            .child(Node::text(
                footer(
                    &diag,
                    ctx.last_exact_changed_cells(),
                    dirty_cells,
                    cover,
                    &last_fb,
                    src,
                    video.nframes,
                    missed,
                ),
                Style::default().fg(Color::Rgb(130, 130, 140)),
            ));
        ctx.set_root(root);
        while !ctx.render_if_due()? {
            let wait = ctx.time_until_next_frame();
            if wait.is_zero() {
                std::thread::yield_now();
            } else {
                std::thread::sleep(wait.min(Duration::from_millis(2)));
            }
        }
        let _ = dt;
    }

    ctx.restore()?;
    Ok(())
}

fn acq_line(mode: Mode, temporal: bool, d: &TemporalDiagnostics) -> (String, Color) {
    if !temporal {
        return (
            format!(
                "◈ {} — keyed foreground static; background static ({} cells). \
                 --mode=temporal --measured-hz to refine the background.",
                mode.label(),
                d.frozen_cells + d.motion_static_cells
            ),
            Color::Rgb(170, 175, 185),
        );
    }
    if d.modulating {
        (
            format!(
                "● TEMPORAL LOCK — subject held static ({} cells), background refining ({} cells), {} frozen sharp",
                d.motion_static_cells, d.active_cells, d.frozen_cells
            ),
            Color::Rgb(120, 240, 150),
        )
    } else {
        (
            format!("◈ STATIC — gate {:?}; background not yet refining", d.gate),
            Color::Rgb(240, 200, 90),
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn footer(
    d: &TemporalDiagnostics,
    exact_changed: usize,
    dirty_cells: usize,
    cover: f32,
    fb: &Bbox,
    src: usize,
    nframes: usize,
    missed: u32,
) -> String {
    format!(
        "src {src}/{nframes} | key {cover:.0}% background | fg bbox {}×{} @({},{}) | dirty {dirty_cells} cells | \
         wire {exact_changed} changed | gate={:?} depth={:?} | subj-static {} / refine {} / frozen {} | missed {missed}",
        fb.w, fb.h, fb.x, fb.y, d.gate, d.color_depth, d.motion_static_cells, d.active_cells, d.frozen_cells
    )
}

/// Headless performance receipts: full-frame vs sparse (bbox) reprojection, dirty
/// cells, and the per-frame temporal advance cost, over the whole baked segment.
/// No TTY required.
fn run_profile() -> io::Result<()> {
    let clip = arg_str("clip")
        .map(std::path::PathBuf::from)
        .or_else(default_clip)
        .ok_or_else(|| io::Error::other("no clip: pass --clip=PATH"))?;
    let (fw, fh) = arg_wh("film").unwrap_or((320, 180));
    let spec = BakeSpec {
        clip,
        start_s: arg_f32("start").unwrap_or(2.0),
        dur_s: arg_f32("dur").unwrap_or(6.0),
        film_w: fw,
        film_h: fh,
        fps: arg_f32("fps").unwrap_or(29.0),
    };
    let (video, _) = bake::bake(&spec).map_err(io::Error::other)?;
    let (cols, rows) = arg_wh("region")
        .map(|(w, h)| (w as u16, h as u16))
        .unwrap_or((120, 36));
    let (subw, subh) = (cols as u32 * 2, rows as u32 * 4);
    let kp = KeyParams::default();
    let scene = arg_str("scene")
        .and_then(|s| Scene::parse(&s))
        .unwrap_or(Scene::BlackIce);

    println!("temporal video compositor — headless perf receipts (release recommended)");
    println!(
        "clip segment: {} frames @ {}x{} film, projected to {cols}×{rows} cells ({subw}×{subh} subpixels), scene={}",
        video.nframes, video.film_w, video.film_h, scene.label()
    );

    fn mkfc<'a>(
        film: &'a KeyedFilm,
        bytes: &'a [u8],
        scene: Scene,
        subw: u32,
        subh: u32,
        kp: KeyParams,
        bbox_sub: Option<scene::SubBox>,
    ) -> FrameComposite<'a> {
        FrameComposite {
            film,
            bytes,
            scene,
            subw,
            subh,
            kp,
            bbox_sub,
            bg_t: 0.0,
            diag_box: false,
        }
    }

    // Coverage stats.
    let mut opaque_sum = 0usize;
    let mut bbox_area_sum = 0usize;
    for f in 0..video.nframes {
        let (fb, opaque) = video.foreground_stats(video.frame(f), &kp);
        opaque_sum += opaque;
        bbox_area_sum += (fb.w * fb.h) as usize;
    }
    let total_px = (video.film_w * video.film_h) as usize * video.nframes;
    println!(
        "\nkey coverage: {:.1}% background (keyed away), subject bbox avg {:.1}% of frame",
        100.0 * (1.0 - opaque_sum as f32 / total_px as f32),
        100.0 * bbox_area_sum as f32 / total_px as f32
    );

    // ---- FULL-FRAME reprojection every source frame (no sparsity exploit) ----
    let mut p = TemporalDisplayProcessor::new(cols, rows, SubcellGlyphMode::Braille2x4, 1);
    let fc = mkfc(&video, video.frame(0), scene, subw, subh, kp, None);
    p.set_target_image(|lx, ly| fc.at(lx, ly), ResetPolicy::Reset);
    let t0 = Instant::now();
    for f in 0..video.nframes {
        let (fb, _) = video.foreground_stats(video.frame(f), &kp);
        let sub = video.bbox_to_sub(&fb, subw, subh);
        let fc = mkfc(
            &video,
            video.frame(f),
            scene,
            subw,
            subh,
            kp,
            (!sub.is_empty()).then_some((sub.x, sub.y, sub.w, sub.h)),
        );
        p.set_target_image(|lx, ly| fc.at(lx, ly), ResetPolicy::Keep);
    }
    let full_us = t0.elapsed().as_secs_f64() * 1e6 / video.nframes as f64;

    // ---- SPARSE reprojection: only the union of prev+cur bbox ----
    let mut p = TemporalDisplayProcessor::new(cols, rows, SubcellGlyphMode::Braille2x4, 1);
    let (fb0, _) = video.foreground_stats(video.frame(0), &kp);
    let fc = mkfc(&video, video.frame(0), scene, subw, subh, kp, None);
    p.set_target_image(|lx, ly| fc.at(lx, ly), ResetPolicy::Reset);
    let mut prev = video.bbox_to_cells(&fb0, cols, rows);
    let mut dirty_sum = 0usize;
    let t1 = Instant::now();
    for f in 0..video.nframes {
        let (fb, _) = video.foreground_stats(video.frame(f), &kp);
        let sub = video.bbox_to_sub(&fb, subw, subh);
        let cur = video.bbox_to_cells(&fb, cols, rows);
        let u = union_cells(prev, cur);
        dirty_sum += u.2 as usize * u.3 as usize;
        let fc = mkfc(
            &video,
            video.frame(f),
            scene,
            subw,
            subh,
            kp,
            (!sub.is_empty()).then_some((sub.x, sub.y, sub.w, sub.h)),
        );
        p.set_target_region(
            u.0,
            u.1,
            u.2,
            u.3,
            |lx, ly| fc.at(lx, ly),
            ResetPolicy::Reset,
        );
        prev = cur;
    }
    let sparse_us = t1.elapsed().as_secs_f64() * 1e6 / video.nframes as f64;
    let cells_total = cols as usize * rows as usize;

    // ---- per-frame temporal advance cost (static bg refining) ----
    let mut out = p.static_fallback();
    let t2 = Instant::now();
    let adv_iters = 2000u32;
    for _ in 0..adv_iters {
        p.advance_into(0, &mut out);
    }
    let adv_us = t2.elapsed().as_secs_f64() * 1e6 / adv_iters as f64;

    println!(
        "\nreprojection cost per source frame ({} cells total):",
        cells_total
    );
    println!("  full-frame reproject : {full_us:8.1} µs/frame");
    println!(
        "  sparse (bbox union)  : {sparse_us:8.1} µs/frame   ({:+.0}% vs full)   avg dirty {} cells ({:.0}% of grid)",
        (sparse_us / full_us - 1.0) * 100.0,
        dirty_sum / video.nframes,
        100.0 * (dirty_sum as f32 / video.nframes as f32) / cells_total as f32
    );
    println!("  temporal advance     : {adv_us:8.1} µs/frame (per presentation frame)");
    let src_budget_us = 1e6 / video.fps as f64;
    println!(
        "\nsource cadence budget: {src_budget_us:.0} µs/frame @ {} fps",
        video.fps
    );
    println!(
        "  sparse reproject + one advance = {:.1} µs -> {:.0}x headroom under source cadence",
        sparse_us + adv_us,
        src_budget_us / (sparse_us + adv_us)
    );
    println!(
        "\nGPU verdict: CPU sparse-update stays ~{:.0}x under the source-frame budget; the \n\
         greenscreen sparsity, not a GPU, is what makes this real. wgpu remains research-only.",
        src_budget_us / (sparse_us + adv_us)
    );
    Ok(())
}
