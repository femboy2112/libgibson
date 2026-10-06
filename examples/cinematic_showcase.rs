//! `cinematic_showcase` — the v0.5 flagship film (Part III).
//!
//! Not a slideshow. One continuous camera flies through a constellation of living
//! instruments: the six experience grammars and three `gibson::plot` Observatory
//! scenes, each rendered to a texture and hung as a **perspective-mapped plane**
//! on a spiral ring around a luminous core. The camera establishes the whole
//! structure, flies in and features each panel in turn, then rises out for the
//! reveal — real `raster3d` perspective, z-buffer occlusion, reflections, a
//! deep-space nebula backdrop, bloom and a final grade. The content is objects in
//! a world; the camera has the opinion. That is the line between PowerPoint and a
//! picture.
//!
//! The architecture keeps the honest seams the gold standard keeps
//! (`docs/FRANK_REACTION_CUT_PLAN.md`, `docs/TEMPORAL_VIDEO_COMPOSITING.md`):
//!
//! * the **edit clock** is the only stored time — every frame is a pure function
//!   of `edit`, nothing on the frame path reads a wall clock;
//! * the reusable *time* law ([`show_timeline::ShowTimeline`]) stays demo-local
//!   and promotable, never hardened into a general video editor;
//! * the camera cue sheet, the ring geometry and the look are **demo-local art
//!   direction** ([`stage`], [`look`]), built on public `raster3d`/`RgbRaster`
//!   primitives only;
//! * the grammars present to a terminal `Surface`; [`texture`] is the honest
//!   bridge to pixels (the "(Qt+OpenGL) through a screen door" ethos, now
//!   perspective-mapped), never a faked screenshot.
//!
//! ## Usage
//!   cargo run --release --example cinematic_showcase                   # live, loops the film
//!   cargo run --release --example cinematic_showcase -- seq DIR [W H]  # 20fps ANSI frames -> DIR
//!   cargo run --release --example cinematic_showcase -- shot T [PATH]  # one raw raster frame -> PPM
//!   cargo run --release --example cinematic_showcase -- dump [W H]     # a few key frames as text

#[path = "cinematic_showcase/look.rs"]
mod look;
#[path = "cinematic_showcase/observatory.rs"]
mod observatory;
#[path = "cinematic_showcase/shot.rs"]
mod shot;
#[path = "cinematic_showcase/show_timeline.rs"]
mod show_timeline;
#[path = "cinematic_showcase/stage.rs"]
mod stage;
#[path = "cinematic_showcase/texture.rs"]
mod texture;

use gibson::capability::ColorDepth;
use gibson::cell::{Cell, Color, Style};
use gibson::context::{Context, RenderMode};
use gibson::input::{Event, KeyCode, KeyModifiers};
use gibson::raster::RgbRaster;
use gibson::ui::experience::apply_intent;
use gibson::ui::experience::{
    Action, Content as ExpContent, Destination, Experience, ExperienceRuntime, Facet, Intent, Item,
    Media,
};
use gibson::ui::prelude::UiRuntime;
use gibson::ui::skin::UiEnvironment;
use gibson::ui::{raster, screen, skins, App, AppEvent, BuildCx, Control, Element};
use gibson::Surface;
use shot::{cinematic_hud, GrammarId, SceneId};
use std::cell::RefCell;
use std::io;
use std::time::Duration;

/// The one application action type (lifted from `experience_lab`: the film shows
/// the same shared semantic library through every grammar).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
    Play(String),
    Toggle(String),
    Open(String),
}

/// Fallback master cell size when the projected hold rect can't be computed (a
/// pathologically tiny terminal): the canonical 120×36-grid value, clamped to fit.
const MASTER_W_FALLBACK: u16 = 104;
const MASTER_H_FALLBACK: u16 = 31;

/// The cell size every panel's UI is laid out at — for **both** the baked plane texture
/// (the coarse approach view) **and** the live crisp hold — for *this* terminal. Rendering
/// one Surface model at one size is what makes the hand-off seamless: a reflowing grammar
/// (cover-flow's side covers, say) lands its elements in identical positions in the RGB
/// texture and the ASCII hold, so the panel only ever goes blocky→crisp, never re-lays-out.
/// Making that size the projected *hold* rectangle (`stage::hold_rect`, uniform across the
/// congruent planes) means the hold is an exact 1:1 copy and every other frame a pure
/// down-scale — so the UI never up-scales, which nearest-neighbour would otherwise show as
/// duplicated rows at a large (e.g. fullscreen) terminal. Its cell aspect (~3.35:1) is ≈2×
/// the plane's world aspect, which the square-celled texture needs to read upright on the
/// plane. (`texture::CELL_PX` pixels per cell in the baked texture.)
fn master_size(w: u16, h: u16) -> (u16, u16) {
    stage::hold_rect(0, w, h)
        .map(|r| (r.width, r.height))
        .unwrap_or((
            MASTER_W_FALLBACK.min(w.max(1)),
            MASTER_H_FALLBACK.min(h.max(1)),
        ))
}

/// Deterministic seed for the starfield backdrop.
const STARFIELD_SEED: u32 = 0x5055_4152;

/// The VAPOR95 skin's screen background (`src/ui/skin.rs`), the fill a grammar's
/// bare screen reads as once rasterised.
const VAPOR95_BG: (u8, u8, u8) = (61, 35, 79);
/// The deep-space ink a plot scene's bare field reads as.
const OBSERVATORY_BG: (u8, u8, u8) = (10, 8, 20);

/// The single semantic description of the application — authored once, never
/// forked per grammar.
fn build_experience() -> Experience<Msg> {
    let library = ExpContent::Collection(vec![
        Item::new("neon-harbour", "Neon Harbour")
            .subtitle("Vapor Cartography")
            .media(Media::new(1))
            .action(Action::new(
                "play-0",
                "Play",
                Msg::Play("neon-harbour".into()),
            )),
        Item::new("glass-arcades", "Glass Arcades")
            .subtitle("Mono Lake")
            .media(Media::new(2))
            .action(Action::new(
                "play-1",
                "Play",
                Msg::Play("glass-arcades".into()),
            )),
        Item::new("tidal-automata", "Tidal Automata")
            .subtitle("Subaqueous")
            .media(Media::new(3))
            .action(Action::new(
                "play-2",
                "Play",
                Msg::Play("tidal-automata".into()),
            )),
        Item::new("low-orbit-choir", "Low Orbit Choir")
            .subtitle("Ascent")
            .media(Media::new(4))
            .action(Action::new(
                "play-3",
                "Play",
                Msg::Play("low-orbit-choir".into()),
            )),
        Item::new("hidden-track", "Hidden Track")
            .subtitle("—")
            .media(Media::new(5))
            .action(Action::new(
                "play-4",
                "Play",
                Msg::Play("hidden-track".into()),
            )),
    ]);

    let now_playing = ExpContent::Detail {
        facets: vec![
            Facet::new("track", "Track", "Neon Harbour"),
            Facet::new("artist", "Artist", "Vapor Cartography"),
            Facet::new("time", "Elapsed", "01:12 / 03:48"),
        ],
        actions: vec![Action::new("open-np", "Open", Msg::Open("now".into()))],
    };

    let settings = ExpContent::Collection(vec![
        Item::new("set-color", "Colour depth")
            .subtitle("TrueColor")
            .action(Action::new(
                "cyc-color",
                "Cycle",
                Msg::Toggle("color".into()),
            )),
        Item::new("set-motion", "Motion")
            .subtitle("Full")
            .action(Action::new(
                "cyc-motion",
                "Cycle",
                Msg::Toggle("motion".into()),
            )),
    ]);

    let about = ExpContent::Prose(vec![
        "Gibson Library — one semantic experience, many grammars.".into(),
        "The application describes itself once; each grammar is a change of".into(),
        "representation, never a fork of application state.".into(),
    ]);

    Experience::new("GIBSON LIBRARY")
        .destination(Destination::new("library", "Library", library))
        .destination(Destination::new("now", "Now Playing", now_playing))
        .destination(Destination::new("settings", "Settings", settings))
        .destination(Destination::new("about", "About", about))
}

/// The nine plane textures and their reflections, plus a label per plane for the
/// HUD. Built once (the panels are a fixed snapshot; the camera supplies the
/// motion), ordered grammars first (`GrammarId::REEL_ORDER`) then the three
/// Observatory scenes (`SceneId::ALL`) — the order the camera features them in.
struct Reel {
    texes: Vec<RgbRaster>,
    refls: Vec<RgbRaster>,
    labels: Vec<(String, String)>,
}

/// The baked reel plus the master cell size it was baked for. The textures are baked
/// for one terminal's hold-rect master size; a resize to a different size re-bakes, so
/// the plane texture and the hold always stay the same congruent layout at 1:1.
struct CachedReel {
    mw: u16,
    mh: u16,
    reel: Reel,
}

/// The film: the shared experience, a persistent runtime, and the (size-cached) reel.
/// During a panel's crisp hold the runtime re-presents that one panel live so it
/// animates in full quality, at the same master size the ring texture was baked at.
struct Film {
    experience: Experience<Msg>,
    ui: RefCell<ExperienceRuntime<Msg>>,
    reel: RefCell<CachedReel>,
}

impl Film {
    fn new() -> Self {
        let experience = build_experience();
        let ui = RefCell::new(ExperienceRuntime::with_builtins(&experience));
        // Park the selection on a strong focal item so cover/orbital panels open on
        // something with a cover rather than the first row.
        apply_intent(&experience, ui.borrow_mut().state_mut(), Intent::Next);
        apply_intent(&experience, ui.borrow_mut().state_mut(), Intent::Next);
        // Bake for the default capture grid; the live path re-bakes on a size change.
        let (mw, mh) = master_size(120, 36);
        let reel = build_reel(&experience, &ui, mw, mh);
        Self {
            experience,
            ui,
            reel: RefCell::new(CachedReel { mw, mh, reel }),
        }
    }

    /// Ensure the cached reel is baked for the master size this terminal implies,
    /// re-baking only when that size changed (a resize). Returns the master cell size.
    fn ensure_reel(&self, w: u16, h: u16) -> (u16, u16) {
        let (mw, mh) = master_size(w, h);
        if self.reel.borrow().mw == mw && self.reel.borrow().mh == mh {
            return (mw, mh);
        }
        let reel = build_reel(&self.experience, &self.ui, mw, mh);
        *self.reel.borrow_mut() = CachedReel { mw, mh, reel };
        (mw, mh)
    }
}

/// The base presentation time the grammar ring textures are baked at; a grammar
/// panel's live hold animates forward from here (so at `local == 0` the baked
/// texture and the live hold are the identical frame — the hand-off is seamless).
const PANEL_T0: f32 = 1.5;

/// The same base, for observatory scenes — the flattering moment their textures are
/// baked at and the moment their live hold animates forward from.
const OBSERVATORY_T0: f32 = 3.2;

/// Lower panel `i` to a full crisp `Surface` at `w × h` cells, animated at hold-local
/// time `local`: grammars re-present through the runtime; observatory scenes re-plot.
/// This is the full-quality render the frame locks onto during a hold.
fn panel_live_surface(film: &Film, i: usize, w: u16, h: u16, local: f32) -> Surface {
    let grammars = GrammarId::REEL_ORDER.len();
    if i < grammars {
        let g = GrammarId::REEL_ORDER[i];
        let now = Duration::from_secs_f32((PANEL_T0 + local).max(0.0));
        grammar_surface(
            &film.experience,
            &film.ui,
            g,
            w,
            h,
            now,
            ColorDepth::TrueColor,
        )
    } else {
        let scene = SceneId::ALL[(i - grammars).min(SceneId::ALL.len() - 1)];
        observatory::render(scene, OBSERVATORY_T0 + local, w, h, ColorDepth::TrueColor)
    }
}

/// Render the nine panels into plane textures and reflections at master size `mw × mh`
/// (the terminal's hold-rect size), so each plane texture is the same layout the crisp
/// hold will render at.
fn build_reel(
    experience: &Experience<Msg>,
    ui: &RefCell<ExperienceRuntime<Msg>>,
    mw: u16,
    mh: u16,
) -> Reel {
    let mut texes = Vec::with_capacity(stage::N_PLANES);
    let mut labels = Vec::with_capacity(stage::N_PLANES);
    let now0 = Duration::from_secs_f32(PANEL_T0);
    for &g in &GrammarId::REEL_ORDER {
        // Bake the approach texture from the SAME size/model the crisp hold renders at
        // (mw × mh), so the two are one layout — coarse vs. crisp, never reflowed — at the
        // texture→UI hand-off.
        let s = grammar_surface(experience, ui, g, mw, mh, now0, ColorDepth::TrueColor);
        texes.push(texture::surface_to_raster(&s, VAPOR95_BG));
        labels.push((g.name().to_string(), g.tagline().to_string()));
    }
    for &scene in &SceneId::ALL {
        // A flattering frozen moment of each instrument, at the shared master size and
        // the same base time the live hold animates forward from (seamless hand-off).
        let s = observatory::render(scene, OBSERVATORY_T0, mw, mh, ColorDepth::TrueColor);
        texes.push(texture::surface_to_raster(&s, OBSERVATORY_BG));
        labels.push((scene.name().to_string(), scene.tagline().to_string()));
    }
    let refls = texes.iter().map(texture::reflection_of).collect();
    Reel {
        texes,
        refls,
        labels,
    }
}

/// Lower one grammar to a `Surface`: present the shared value through the grammar
/// at presentation time `now`, host it on a `screen` so it fills the viewport, then
/// compile / lay out / paint — the headless lowering the flagship's `dump` path uses.
fn grammar_surface(
    experience: &Experience<Msg>,
    ui: &RefCell<ExperienceRuntime<Msg>>,
    g: GrammarId,
    w: u16,
    h: u16,
    now: Duration,
    depth: ColorDepth,
) -> Surface {
    let env = UiEnvironment {
        width: w,
        height: h,
        color_depth: depth,
        ..UiEnvironment::default()
    };
    let element = {
        let mut ui = ui.borrow_mut();
        ui.set_style(g.style());
        let presented = ui.present(experience, &env, now);
        screen::<Msg>().child(presented.element.grow(1.0)).height(h)
    };
    let cx = BuildCx::new(skins::VAPOR95, env);
    // Back the frame with the skin's screen colour so a short widget grammar reads
    // as a framed screen rather than painting onto a black void below its content.
    let mut backing = RgbRaster::new(w, h.saturating_mul(2));
    backing.clear(VAPOR95_BG);
    let mut surface = backing.to_surface();
    if let Ok(compiled) = gibson::ui::compile(&element, &cx) {
        let mut node = compiled.node;
        if gibson::compute_layout(&mut node, w, h).is_ok() {
            gibson::paint(&node, &mut surface);
        }
    }
    surface
}

/// Render one frame of the flight to an `RgbRaster` of `pw × ph` pixels: nebula
/// backdrop, the constellation of panels through the moving camera, then bloom and
/// a final grade. Pure function of `edit`.
fn frame_raster(reel: &Reel, pw: u16, ph: u16, edit: f32) -> RgbRaster {
    let backdrop = look::starfield(pw, ph, edit, STARFIELD_SEED);
    let mut raster = stage::render_frame(edit, pw, ph, backdrop, &reel.texes, &reel.refls);
    look::bloom(&mut raster, 168, 9, 1.8);
    look::grade(&mut raster, 0.28);
    raster
}

/// Compose one full film frame to a terminal `Surface` of `w × h` cells: the flight
/// (rendered at `w × 2h` pixels, half-blocked to cells), the featured panel's HUD
/// lower-third, and the baked cold-open / closing titles over the live frame.
fn showcase_frame(film: &Film, w: u16, h: u16, edit: f32) -> Surface {
    // Ensure the reel is baked for this terminal's hold-rect master size (re-bakes on a
    // resize), then render against it. mw × mh is the size the hold renders at, so the
    // projected rect never exceeds it — the blit is a down-scale on approach and exactly
    // 1:1 at the hold, never an up-scale (which would duplicate rows at a large terminal).
    let (mw, mh) = film.ensure_reel(w, h);
    let focus = stage::focus_at(edit);
    let cache = film.reel.borrow();
    let reel = &cache.reel;

    // The 3-D flight: nebula, the ring of textured panels through the moving camera.
    let mut surface = frame_raster(reel, w, h.saturating_mul(2), edit).to_surface();

    // The intro cinema's facade law, in reverse: while the focused panel's face
    // projects to a large frontal rectangle, resolve the real UI into it — rendered at the
    // same master size the approach texture was baked from, uniformly scaled into the
    // current rectangle. The whole UI zooms as one congruent image: blocky far, 1:1 crisp
    // at the hold, no reflow, no cut, no screen-door tween.
    if let Some(f) = focus {
        if let Some(b) = stage::panel_cell_bounds(f.panel, edit, w, h) {
            let master = panel_live_surface(film, f.panel, mw, mh, f.local);
            texture::blit_surface_scaled(&mut surface, &master, b);
        }
        overlay_hud(&mut surface, reel, &film.experience.title, f.panel, w, h);
    } else {
        overlay_titles(&mut surface, w, h, edit);
    }
    surface
}

/// The HUD lower-third / title bar naming the panel currently in frame.
fn overlay_hud(surface: &mut Surface, reel: &Reel, title: &str, panel: usize, w: u16, h: u16) {
    if let Some((name, tag)) = reel.labels.get(panel) {
        let hud = cinematic_hud(w, h, title, name, tag);
        surface.blit_transparent_at(&hud, 0, 0);
    }
}

/// Bake a centred title line with a tight dark plate behind the glyphs. The bare baked
/// text inherits the bright bloomed nebula as its glyph background and washes out; a
/// one-cell-padded dark plate under just the letters keeps it crisply legible without a
/// full-width opaque block.
fn bake_title(surface: &mut Surface, cx: u16, y: u16, text: &str, fg: (u8, u8, u8), bold: bool) {
    let n = text.chars().count() as u16;
    let x0 = cx.saturating_sub(n / 2).saturating_sub(1);
    let plate = Style::new().bg(Color::Rgb(16, 9, 26));
    for dx in 0..n + 2 {
        surface.set_cell(x0 + dx, y, Cell::space(plate));
    }
    surface.bake_text_centered(cx, y, text, fg, bold);
}

/// The cold-open and closing titles, baked over the establishing / reveal wide shots.
fn overlay_titles(surface: &mut Surface, w: u16, h: u16, edit: f32) {
    let cy = h / 2;
    if edit < stage::EST + 0.5 {
        bake_title(
            surface,
            w / 2,
            cy.saturating_sub(1),
            "GIBSON",
            (245, 238, 255),
            true,
        );
        bake_title(
            surface,
            w / 2,
            cy + 1,
            "one experience · many grammars",
            (210, 182, 240),
            false,
        );
    } else if edit > stage::duration() - stage::REVEAL + 1.0 {
        bake_title(
            surface,
            w / 2,
            cy.saturating_sub(1),
            "gibson::ui · gibson::plot",
            (245, 238, 255),
            true,
        );
        bake_title(
            surface,
            w / 2,
            cy + 1,
            "observable instruments",
            (210, 182, 240),
            false,
        );
    }
}

/// Render one film frame to truecolor ANSI bytes, wrapping the composed `Surface`
/// in a `raster` node and running the flagship capture lowering so the film is
/// captured exactly as it paints.
fn frame_bytes(film: &Film, w: u16, h: u16, edit: f32, depth: ColorDepth) -> Vec<u8> {
    let surface = showcase_frame(film, w, h, edit);
    let env = UiEnvironment {
        width: w,
        height: h,
        color_depth: depth,
        ..UiEnvironment::default()
    };
    let now = Duration::from_secs_f32(edit.max(0.0));
    let element = screen::<Msg>()
        .child(raster::<Msg>(surface).grow(1.0))
        .height(h);
    let mut runtime = UiRuntime::new(skins::VAPOR95);
    let compiled = runtime
        .frame(&element, env, now)
        .expect("film frame lowers");
    let mut ctx = Context::headless(RenderMode::Fullscreen, w, h);
    ctx.set_color_depth(depth);
    ctx.set_root(compiled.node);
    ctx.render_now().expect("headless render");
    ctx.rendered_bytes().to_vec()
}

/// Write the whole film as a truecolor ANSI frame sequence at 20 fps.
fn capture_sequence(outdir: &str, w: u16, h: u16) -> io::Result<u32> {
    std::fs::create_dir_all(outdir)?;
    let film = Film::new();
    let fps = 20.0_f32;
    let dt = 1.0 / fps;
    let mut n: u32 = 0;
    let mut edit = 0.0_f32;
    let total = stage::duration();
    while edit < total {
        let bytes = frame_bytes(&film, w, h, edit, ColorDepth::TrueColor);
        std::fs::write(format!("{outdir}/frame_{n:04}.ans"), &bytes)?;
        n += 1;
        edit += dt;
    }
    Ok(n)
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Animated capture for MP4 visual acceptance: `cinematic_showcase seq DIR [W H]`.
    if args.get(1).map(String::as_str) == Some("seq") {
        let outdir = args.get(2).cloned().unwrap_or_else(|| "frames".to_string());
        let w: u16 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(120);
        let h: u16 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(36);
        let frames = capture_sequence(&outdir, w, h)?;
        let total = stage::duration();
        println!("showcase: {frames} frames -> {outdir} ({w}x{h}, {total:.1}s film)");
        return Ok(());
    }

    // One raw-raster frame to PPM for quick geometry/look sanity (full pixel res,
    // not the half-block terminal render): `cinematic_showcase shot T [PATH]`.
    if args.get(1).map(String::as_str) == Some("shot") {
        let t: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10.0);
        let path = args
            .get(3)
            .cloned()
            .unwrap_or_else(|| "shot.ppm".to_string());
        let film = Film::new();
        film.ensure_reel(120, 36);
        let raster = frame_raster(&film.reel.borrow().reel, 480, 300, t);
        let file = std::fs::File::create(&path)?;
        raster
            .write_ppm(std::io::BufWriter::new(file))
            .expect("write ppm");
        println!("showcase: raw frame at {t:.1}s -> {path}");
        return Ok(());
    }

    // Headless text snapshot for quick eyeballing: `cinematic_showcase dump [W H]`.
    if args.get(1).map(String::as_str) == Some("dump") {
        let w: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100);
        let h: u16 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(30);
        let film = Film::new();
        let total = stage::duration();
        for frac in [0.02, 0.2, 0.4, 0.6, 0.8, 0.98] {
            let edit = total * frac;
            let surface = showcase_frame(&film, w, h, edit);
            println!("\n===== edit {edit:.1}s / {total:.1}s =====");
            for line in surface.to_visible_lines() {
                println!("{line}");
            }
        }
        return Ok(());
    }

    // Live demo: loop the film, driven by the harness clock.
    let film = Film::new();
    App::fullscreen()
        .skin(skins::VAPOR95)
        .fps(30)
        .run(film, update, view)?;
    Ok(())
}

/// Live view: map the harness clock onto the (looping) edit clock and paint.
fn view(film: &Film, cx: &BuildCx) -> Element<Msg> {
    let (w, h) = (cx.environment.width, cx.environment.height);
    let edit = cx.time.as_secs_f32() % stage::duration().max(0.001);
    let surface = showcase_frame(film, w, h, edit);
    screen::<Msg>().child(raster::<Msg>(surface).grow(1.0))
}

/// Live update: quit on `q` / `Ctrl-C`; the film is non-interactive.
fn update(_film: &mut Film, event: AppEvent<Msg>) -> Control {
    if let AppEvent::Input(Event::Key(key)) = event {
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            return Control::Quit;
        }
    }
    Control::Continue
}
