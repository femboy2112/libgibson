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

// The embedded-media codec (GMED), vendored verbatim from gibson::media (PR #186) until it
// lands on main. Always compiled: the `--bake-embed` asset tool uses its encoder. Under
// `embedded-media` the example also uses it at runtime to round-trip an ORIGINAL synthetic
// performer — no third-party media is embedded (see `embedded_films` / MEDIA_PROVENANCE.md).
#[path = "libgibson_intro_reaction/media.rs"]
mod media;

// Live realtime audio is opt-in: it exists only under the `audio-cpal` feature, which pulls
// cpal + ALSA. The default build (and CI) compiles the no-op `LiveAudio` stub below instead,
// so the live loop reads identically with or without sound and stays dependency-free.
#[cfg(feature = "audio-cpal")]
#[path = "libgibson_intro_reaction/live_audio.rs"]
mod live_audio;
#[cfg(feature = "audio-cpal")]
use live_audio::LiveAudio;

/// No-audio stub matching [`live_audio::LiveAudio`]'s surface, compiled when `audio-cpal`
/// is off (the default). `start` always yields `None`, so the live loop below runs silent
/// and clock-driven by wall time exactly as it always has — no `#[cfg]` in the hot loop.
#[cfg(not(feature = "audio-cpal"))]
struct LiveAudio;
#[cfg(not(feature = "audio-cpal"))]
impl LiveAudio {
    fn clock_seconds(&self) -> Option<f64> {
        None
    }
    fn toggle_pause(&self) -> bool {
        false
    }
    fn seek_by(&self, _delta: f64) {}
    fn restart(&self) {}
    fn underruns(&self) -> u64 {
        0
    }
    fn stop(self) {}
}

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

/// Fixed spec for the self-contained embedded assets (eye/ear-accepted at this res/fps).
/// The `--bake-embed` encoder and the `embedded-media` decoder both use these, so the
/// embedded frames and the decoder always agree on geometry.
const EMBED_W: u32 = 160;
const EMBED_H: u32 = 90;
const EMBED_FPS: f32 = 12.0;

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
             --clip=PATH            bake from a live greenscreen clip instead of the embed\n\
             --live                 bake from an auto-discovered ~/Downloads clip\n\
             --music                a Spybreak-style big-beat bed under the dialogue (audio-cpal)\n\
             --music-gain=0.5       how loud the bed sits under speech\n\
             --film-width/--film-height/--film-fps  bake resolution\n\
             --smooth               bilinear source sampling (premultiplied)\n\
             --linear               composite the matte in linear light\n\
             --no-reaction          base intro only (the honest baseline)\n\
             --profile              headless performance receipts, then exit\n\
             --deterministic --auto --seconds=N\n\n\
             The embedded-media build is self-contained: it synthesizes an ORIGINAL reaction\n\
             performer (no third-party media is shipped). For the real greenscreen reaction,\n\
             download the free-use source from the Internet Archive and bake it locally with\n\
             --clip=PATH (see examples/libgibson_intro_reaction/MEDIA_PROVENANCE.md).",
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

    // Embedded vs. live-clip source. With the `embedded-media` feature the self-contained
    // embedded assets are the DEFAULT: the finale runs with no external clip and no flag.
    // Opt into a live greenscreen bake with --clip=PATH (an explicit clip), --live, or a
    // bare --clip (auto-discover one in ~/Downloads); --bake-embed also needs a live clip.
    // The legacy --embedded flag is now the default and kept only as a harmless override.
    // Without the feature there is nothing embedded, so the reaction always needs a clip.
    let want_live = value("--clip=").is_some()
        || has("--clip")
        || has("--live")
        || value("--bake-embed=").is_some();
    let use_embedded =
        cfg!(feature = "embedded-media") && reaction && (has("--embedded") || !want_live);

    // Resolve the greenscreen clip only when one is actually needed (a live bake, or a build
    // without the embed): reused for the video bake and, under the `audio-cpal` feature, for
    // live dialogue audio built from the same source windows.
    let clip_path: Option<PathBuf> = if reaction && !use_embedded {
        Some(match value("--clip=") {
            Some(p) => PathBuf::from(p),
            None => default_clip().ok_or_else(|| {
                "no greenscreen clip found in ~/Downloads; pass --clip=PATH \
                 (or --no-reaction for the base intro only)"
                    .to_string()
            })?,
        })
    } else {
        None
    };

    // Bake the self-contained embedded assets and exit (one-time dev tool at the embed spec).
    if let Some(dir) = value("--bake-embed=") {
        let clip = clip_path
            .as_deref()
            .ok_or("--bake-embed needs a greenscreen clip: pass --clip=PATH")?;
        std::fs::create_dir_all(dir)?;
        bake_embed_assets(clip, dir, &kp)?;
        return Ok(());
    }

    // Source films: the embedded self-contained assets by default (built with `embedded-media`),
    // otherwise baked from the live greenscreen clip.
    let films: Vec<KeyedFilm> =
        load_films(&clip_path, use_embedded, film_w, film_h, film_fps, reaction)?;

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

    // Deterministic frame-sequence export for offline video assembly: render every
    // edit-clock frame at a fixed fps into numbered PPMs, in ONE process (the clip is
    // baked once, above — no per-frame ffmpeg re-read). Each frame uses the same witness
    // format as --dump, so the sequence and a single dump agree pixel-for-pixel.
    if let Some(dir) = value("--sequence=") {
        let w = value("--width=").unwrap_or("320").parse::<u16>()?.min(640);
        let h = value("--height=").unwrap_or("90").parse::<u16>()?.min(240);
        let fps = value("--fps=")
            .and_then(|s| s.parse::<f32>().ok())
            .filter(|n| n.is_finite() && *n > 0.0)
            .unwrap_or(FILM_FPS);
        let secs = value("--seconds=")
            .and_then(|s| s.parse::<f32>().ok())
            .filter(|n| n.is_finite() && *n > 0.0)
            .unwrap_or_else(director::total_edit_seconds);
        std::fs::create_dir_all(dir)?;
        let nframes = (secs * fps).ceil() as usize + 1;
        for i in 0..nframes {
            let t = i as f32 / fps;
            let (surface, _r, _s) = render_frame(
                &films,
                w,
                h,
                depth.unwrap_or(ColorDepth::TrueColor),
                glyph_mode,
                t,
                &kp,
                opts,
                reaction,
            );
            let path = format!("{dir}/frame_{i:06}.ppm");
            dump_ppm(&surface, w, h, &path)?;
        }
        eprintln!("rendered {nframes} frames to {dir} at {fps} fps ({w}x{h} cells)");
        return Ok(());
    }

    // Headless: render ONLY the Spybreak music bed to a WAV, to audition it in isolation
    // (no dialogue, no ducking). `--seconds=N` sets the length; defaults to the full finale.
    #[cfg(feature = "audio-cpal")]
    {
        if let Some(path) = value("--render-music=") {
            use gibson::audio::{SampleRate, StereoBlock};
            let sr = SampleRate::STUDIO;
            let secs = value("--seconds=")
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|n| n.is_finite() && *n > 0.0)
                .unwrap_or_else(|| director::total_edit_seconds() as f64);
            let end = (secs * sr.as_f64()).round() as u64;
            let (l, r) = live_audio::render_bed(sr, end).ok_or("music bed render failed")?;
            let mut sb = StereoBlock::new(l.len());
            sb.left = l;
            sb.right = r;
            let peak = sb
                .left
                .iter()
                .chain(sb.right.iter())
                .fold(0.0f32, |m, &x| m.max(x.abs()));
            gibson::audio::wav::write_wav_i16(std::path::Path::new(path), &sb, sr)?;
            eprintln!(
                "wrote music bed {:.1}s peak={peak:.3} -> {path}",
                sb.frames() as f64 / sr.as_f64()
            );
            return Ok(());
        }
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

    // Live audio (opt-in `audio-cpal`): the embedded dialogue when the embed is in use, else
    // Frank's dialogue from the clip, played through gibson::audio's realtime device with the
    // audio playhead as the master A/V clock. No feature / no device / no source => silent.
    // Opt-in Spybreak-style music bed under the dialogue (audio-cpal only). Off by default so
    // the ear-accepted dialogue mix is unchanged; `--music-gain=` tunes how loud it sits.
    let music_gain = value("--music-gain=")
        .and_then(|s| s.parse::<f32>().ok())
        .filter(|g| g.is_finite() && *g >= 0.0)
        .unwrap_or(0.5);
    let audio: Option<LiveAudio> = start_audio(
        &clip_path,
        use_embedded,
        ctx.session.is_tty,
        edit_seconds,
        !paused,
        has("--music"),
        music_gain,
    );

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
            // Transport keys drive the audio when it's live (the playhead then drives the
            // video clock below); the direct `edit_seconds` edits keep the no-audio path
            // identical and are harmlessly overwritten whenever audio is the clock.
            match k.code {
                KeyCode::Esc => break,
                KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Char(' ') => {
                    paused = !paused;
                    if let Some(a) = &audio {
                        a.toggle_pause();
                    }
                }
                KeyCode::Char('r' | 'R') => {
                    edit_seconds = 0.0;
                    if let Some(a) = &audio {
                        a.restart();
                    }
                }
                KeyCode::Right => {
                    edit_seconds = (edit_seconds + 2.0).min(total);
                    if let Some(a) = &audio {
                        a.seek_by(2.0);
                    }
                }
                KeyCode::Left => {
                    edit_seconds = (edit_seconds - 2.0).max(0.0);
                    if let Some(a) = &audio {
                        a.seek_by(-2.0);
                    }
                }
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
        match audio.as_ref().and_then(|a| a.clock_seconds()) {
            // Audio is the master clock: draw wherever the (drift-free) playhead sits, so a
            // slow terminal frame can never desync speech. Pause/seek already moved it.
            Some(secs) => edit_seconds = (secs as f32).clamp(0.0, total),
            // No audio: advance the edit clock by wall time, as the film always has.
            None if !paused => {
                edit_seconds += dt;
                // Hold the final sting rather than looping unless asked.
                if edit_seconds > total {
                    edit_seconds = total;
                }
            }
            None => {}
        }
    }

    // Stop audio before restoring the terminal; surface any starvation afterward.
    let underran = match audio {
        Some(a) => {
            let u = a.underruns();
            a.stop();
            u
        }
        None => 0,
    };
    ctx.restore()?;
    if underran > 0 {
        eprintln!("live audio: stream starved ({underran} samples zero-filled)");
    }
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

/// Snap fully-keyed background pixels to exact clean green `(0,254,2)` and deinterleave to
/// planar RGB (`RR..GG..BB..`). Only pixels `key_pixel` fully keys out (alpha 0) are snapped
/// — their RGB is discarded by the compositor anyway, and exact green gives PackBits long
/// per-plane runs; every edge and subject pixel is kept verbatim, so the decoder's live key
/// reproduces identical alpha. The planar layout is what lets PackBits crush the background.
fn snap_green_planar(rgb: &[u8], w: u32, h: u32, kp: &KeyParams) -> Vec<u8> {
    let n = (w * h) as usize;
    let mut out = vec![0u8; n * 3];
    for i in 0..n {
        let px = [rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]];
        let (r, g, b) = if key::key_pixel(px, kp).1 <= 0.0 {
            (0u8, 254u8, 2u8)
        } else {
            (px[0], px[1], px[2])
        };
        out[i] = r;
        out[n + i] = g;
        out[2 * n + i] = b;
    }
    out
}

/// Bake each cue's source window into a planar-RGB GMED FrameSeq asset (`NN_name.gmed`) at
/// the embed spec. Local dev tool only (`--bake-embed`, needs a `--clip`): it authors real
/// reaction assets on the maintainer's machine. The public `embedded-media` build does NOT
/// use these files — it synthesizes an original performer (see [`embedded_films`]) so no
/// third-party media is ever committed or shipped.
fn bake_embed_assets(
    clip: &std::path::Path,
    dir: &str,
    kp: &KeyParams,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut total = 0usize;
    for (i, cue) in director::CUES.iter().enumerate() {
        let spec = bake::BakeSpec {
            clip: clip.to_path_buf(),
            start_s: cue.src_start,
            dur_s: cue.src_dur,
            film_w: EMBED_W,
            film_h: EMBED_H,
            fps: EMBED_FPS,
        };
        let (film, _path) = bake::bake(&spec).map_err(|e| format!("bake {}: {e}", cue.name))?;
        let planar: Vec<Vec<u8>> = (0..film.nframes)
            .map(|fi| snap_green_planar(film.frame(fi), EMBED_W, EMBED_H, kp))
            .collect();
        let refs: Vec<&[u8]> = planar.iter().map(|v| v.as_slice()).collect();
        let bytes = media::encode_frame_seq(EMBED_W, EMBED_H, 3, EMBED_FPS, &refs)?;
        let path = format!("{dir}/{i:02}_{}.gmed", cue.name);
        std::fs::write(&path, &bytes)?;
        total += bytes.len();
        eprintln!(
            "baked {:<12} {:>4} frames  {:>6} KiB  -> {path}",
            cue.name,
            film.nframes,
            bytes.len() / 1024
        );
    }
    eprintln!(
        "embed video total: {:.2} MiB across {} cues",
        total as f64 / 1_048_576.0,
        director::CUES.len()
    );
    Ok(())
}

/// Load the source films: the embedded self-contained assets when `use_embedded` (the default
/// for an `embedded-media` build), else baked from the live greenscreen clip.
fn load_films(
    clip_path: &Option<PathBuf>,
    use_embedded: bool,
    film_w: u32,
    film_h: u32,
    film_fps: f32,
    reaction: bool,
) -> Result<Vec<KeyedFilm>, Box<dyn std::error::Error>> {
    if !reaction {
        return Ok(Vec::new());
    }
    #[cfg(feature = "embedded-media")]
    {
        if use_embedded {
            return embedded_films();
        }
    }
    #[cfg(not(feature = "embedded-media"))]
    {
        let _ = use_embedded;
    }
    match clip_path.as_deref() {
        Some(clip) => {
            bake_cues(clip, film_w, film_h, film_fps).map_err(|e| -> Box<dyn std::error::Error> {
                format!(
                    "reaction bake failed: {e}\n\
                     hint: pass --clip=PATH to a greenscreen clip, or --no-reaction."
                )
                .into()
            })
        }
        None => Ok(Vec::new()),
    }
}

/// Build the self-contained reaction films WITHOUT any third-party media: synthesize an
/// ORIGINAL, procedural chroma-key performer for each cue (index-parallel to [`director::CUES`])
/// on the exact clean key green [`snap_green_planar`] emits, then round-trip it through the
/// vendored GMED codec — the same encode -> parse -> decode path a committed asset would take,
/// so the codec stays exercised and the live compositor keys the synthetic subject exactly as
/// it keys a clip bake. Nothing is embedded or committed; real footage is a local `--clip=PATH`
/// only (see MEDIA_PROVENANCE.md). Each cue's frame count mirrors its source duration so the
/// synthetic performer occupies the same windows the directed cut expects.
#[cfg(feature = "embedded-media")]
fn embedded_films() -> Result<Vec<KeyedFilm>, Box<dyn std::error::Error>> {
    let frame_bytes = (EMBED_W * EMBED_H * 3) as usize;
    let mut films = Vec::with_capacity(director::CUES.len());
    let mut planar = Vec::with_capacity(frame_bytes);
    for (ci, cue) in director::CUES.iter().enumerate() {
        let nframes = ((cue.src_dur * EMBED_FPS).round() as usize).max(1);
        // Synthesize the cue's frames (planar RR..GG..BB.. on key green), then encode exactly
        // as `bake_embed_assets` would — the embedded path and the baked path share one codec.
        let synth: Vec<Vec<u8>> = (0..nframes)
            .map(|fi| synth_reaction_planar(EMBED_W, EMBED_H, ci, fi, nframes))
            .collect();
        let refs: Vec<&[u8]> = synth.iter().map(|v| v.as_slice()).collect();
        let bytes = media::encode_frame_seq(EMBED_W, EMBED_H, 3, EMBED_FPS, &refs)?;
        let asset = media::Asset::parse(&bytes)?;
        let decoded = asset
            .frame_count()
            .ok_or("synthetic asset is not a frame sequence")?;
        let mut raw = Vec::with_capacity(decoded * frame_bytes);
        for fi in 0..decoded {
            asset.decode_frame(fi, &mut planar)?;
            // Planar RR..GG..BB.. -> interleaved RGB24 for KeyedFilm::from_raw.
            let np = planar.len() / 3;
            for i in 0..np {
                raw.push(planar[i]);
                raw.push(planar[np + i]);
                raw.push(planar[2 * np + i]);
            }
        }
        films.push(
            KeyedFilm::from_raw(raw, EMBED_W, EMBED_H, EMBED_FPS)
                .ok_or("synthetic frame geometry mismatch")?,
        );
    }
    Ok(films)
}

/// Draw one frame of the original synthetic reaction performer as planar RGB (`RR..GG..BB..`,
/// `w*h*3` bytes) on the exact clean key green `(0,254,2)` the compositor keys out, so only the
/// performer composites over the intro. Fully deterministic in `(cue, frame, nframes)` — no
/// third-party media, no randomness, no I/O. The performer is a simple bobbing face whose mouth
/// opens with a per-cue reaction intensity: a legible, copyright-safe stand-in for a live
/// greenscreen subject (supply the real clip with `--clip=PATH`).
#[cfg(feature = "embedded-media")]
fn synth_reaction_planar(w: u32, h: u32, cue: usize, frame: usize, nframes: usize) -> Vec<u8> {
    use std::f32::consts::TAU;
    let (wi, hi) = (w as i32, h as i32);
    let n = (w * h) as usize;
    let mut out = vec![0u8; n * 3];
    // Background: exact clean key green, so every background pixel keys to alpha 0.
    for i in 0..n {
        out[i] = 0;
        out[n + i] = 254;
        out[2 * n + i] = 2;
    }
    let plot = |out: &mut [u8], x: i32, y: i32, rgb: (u8, u8, u8)| {
        if x >= 0 && x < wi && y >= 0 && y < hi {
            let i = (y * wi + x) as usize;
            out[i] = rgb.0;
            out[n + i] = rgb.1;
            out[2 * n + i] = rgb.2;
        }
    };
    let t = frame as f32 / nframes.max(1) as f32;
    // A gentle per-cue bob so the subject reads as alive.
    let bob = ((t * TAU + cue as f32).sin() * (h as f32 * 0.04)) as i32;
    let cx = wi / 2;
    let cy = hi / 2 + bob;
    let face_r = (h as f32 * 0.34) as i32;
    // Per-cue face tint, kept well clear of the key green so it survives the key.
    const ACCENTS: [(u8, u8, u8); 9] = [
        (236, 206, 178),
        (240, 180, 150),
        (232, 150, 140),
        (236, 196, 170),
        (230, 170, 150),
        (236, 196, 170),
        (224, 210, 196),
        (240, 200, 168),
        (236, 176, 150),
    ];
    let skin = ACCENTS[cue % ACCENTS.len()];
    let r2 = face_r * face_r;
    for y in -face_r..=face_r {
        for x in -face_r..=face_r {
            if x * x + y * y <= r2 {
                plot(&mut out, cx + x, cy + y, skin);
            }
        }
    }
    // Eyes.
    let eye_dx = face_r / 2;
    let eye_dy = -face_r / 4;
    let eye_r = (face_r / 6).max(1);
    for (ex, ey) in [(cx - eye_dx, cy + eye_dy), (cx + eye_dx, cy + eye_dy)] {
        for y in -eye_r..=eye_r {
            for x in -eye_r..=eye_r {
                if x * x + y * y <= eye_r * eye_r {
                    plot(&mut out, ex + x, ey + y, (40, 40, 48));
                }
            }
        }
    }
    // Mouth: a filled ellipse whose height tracks a per-cue reaction intensity over time, so
    // the performer visibly reacts under each beat.
    let intensity = 0.3 + 0.7 * (0.5 - 0.5 * (t * TAU * (1 + cue % 3) as f32).cos());
    let mouth_cy = cy + face_r / 3;
    let mouth_rx = (face_r / 2).max(1);
    let mouth_ry = ((face_r as f32 * 0.28 * intensity) as i32).max(1);
    for y in -mouth_ry..=mouth_ry {
        for x in -mouth_rx..=mouth_rx {
            let fx = x as f32 / mouth_rx as f32;
            let fy = y as f32 / mouth_ry as f32;
            if fx * fx + fy * fy <= 1.0 {
                plot(&mut out, cx + x, mouth_cy + y, (60, 30, 36));
            }
        }
    }
    out
}

/// Embedded dialogue is intentionally empty in the public build: the finale ships NO
/// third-party audio. The self-contained demo plays silent dialogue; supply the real reaction
/// clip with `--clip=PATH` for its dialogue (see MEDIA_PROVENANCE.md). The opt-in `--music`
/// bed is original synthesis and rides the `--clip` path (it is timed against a dialogue track).
#[cfg(feature = "embedded-media")]
const EMBEDDED_AUDIO: &[u8] = &[];

/// Start live audio for the running film: the embedded dialogue when the embed is in use,
/// else Frank's dialogue extracted from the clip. Always `None` without `audio-cpal`, on a
/// non-tty, or when no source/device is available (the film then plays silent).
#[allow(unused_variables)]
fn start_audio(
    clip_path: &Option<PathBuf>,
    use_embedded: bool,
    is_tty: bool,
    initial_seconds: f32,
    playing: bool,
    music: bool,
    music_gain: f32,
) -> Option<LiveAudio> {
    if !is_tty {
        return None;
    }
    #[cfg(feature = "audio-cpal")]
    {
        #[cfg(feature = "embedded-media")]
        {
            if use_embedded {
                // The public build embeds no third-party dialogue; `start_from_embedded` returns
                // None on the empty embed and the film plays silent. (`--music` is timed against a
                // dialogue track, so it rides the `--clip` path below rather than the embed.)
                return LiveAudio::start_from_embedded(
                    EMBEDDED_AUDIO,
                    initial_seconds,
                    playing,
                    music,
                    music_gain,
                );
            }
        }
        clip_path
            .as_deref()
            .and_then(|clip| LiveAudio::start(clip, initial_seconds, playing, music, music_gain))
    }
    #[cfg(not(feature = "audio-cpal"))]
    {
        None
    }
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
