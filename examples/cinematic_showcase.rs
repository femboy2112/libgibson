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
//! The experience model, the ring reel, and the congruence law that makes the
//! blocky→crisp hand-off seamless all live in the shared [`showcase`] core — the
//! same core the user-driven `interactive_showcase` flies by hand. This file owns
//! only the *film*: the scripted edit clock, the titles, and the capture modes.
//!
//! Honest seams kept from the gold standard
//! (`docs/FRANK_REACTION_CUT_PLAN.md`, `docs/TEMPORAL_VIDEO_COMPOSITING.md`):
//!
//! * the **edit clock** is the only stored time — every frame is a pure function
//!   of `edit`, nothing on the frame path reads a wall clock;
//! * the reusable *time* law ([`show_timeline::ShowTimeline`]) stays demo-local
//!   and promotable, never hardened into a general video editor;
//! * the camera cue sheet, the ring geometry and the look are **demo-local art
//!   direction** ([`stage`], [`look`]), built on public `raster3d`/`RgbRaster`
//!   primitives only.
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
#[path = "cinematic_showcase/showcase.rs"]
mod showcase;
#[path = "cinematic_showcase/stage.rs"]
mod stage;
#[path = "cinematic_showcase/texture.rs"]
mod texture;

use gibson::capability::ColorDepth;
use gibson::cell::{Cell, Color, Style};
use gibson::context::{Context, RenderMode};
use gibson::input::{Event, KeyCode, KeyModifiers};
use gibson::ui::prelude::UiRuntime;
use gibson::ui::skin::UiEnvironment;
use gibson::ui::{raster, screen, skins, App, AppEvent, BuildCx, Control, Element};
use gibson::Surface;
use showcase::{frame_raster, overlay_hud, panel_live_surface, Msg, Showcase};
use std::io;
use std::time::Duration;

/// Compose one full film frame to a terminal `Surface` of `w × h` cells: the flight
/// (rendered at `w × 2h` pixels, half-blocked to cells), the featured panel's HUD
/// lower-third, and the baked cold-open / closing titles over the live frame.
fn showcase_frame(show: &Showcase, w: u16, h: u16, edit: f32) -> Surface {
    // Ensure the reel is baked for this terminal's hold-rect master size (re-bakes on a
    // resize), then render against it. mw × mh is the size the hold renders at, so the
    // projected rect never exceeds it — the blit is a down-scale on approach and exactly
    // 1:1 at the hold, never an up-scale (which would duplicate rows at a large terminal).
    let (mw, mh) = show.ensure_reel(w, h);
    let focus = stage::focus_at(edit);
    let cache = show.reel.borrow();
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
            let master = panel_live_surface(show, f.panel, mw, mh, f.local);
            texture::blit_surface_scaled(&mut surface, &master, b);
        }
        overlay_hud(&mut surface, reel, &show.experience.title, f.panel, w, h);
    } else {
        overlay_titles(&mut surface, w, h, edit);
    }
    surface
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
fn frame_bytes(show: &Showcase, w: u16, h: u16, edit: f32, depth: ColorDepth) -> Vec<u8> {
    let surface = showcase_frame(show, w, h, edit);
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
    let show = Showcase::new();
    let fps = 20.0_f32;
    let dt = 1.0 / fps;
    let mut n: u32 = 0;
    let mut edit = 0.0_f32;
    let total = stage::duration();
    while edit < total {
        let bytes = frame_bytes(&show, w, h, edit, ColorDepth::TrueColor);
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
        let show = Showcase::new();
        show.ensure_reel(120, 36);
        let raster = frame_raster(&show.reel.borrow().reel, 480, 300, t);
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
        let show = Showcase::new();
        let total = stage::duration();
        for frac in [0.02, 0.2, 0.4, 0.6, 0.8, 0.98] {
            let edit = total * frac;
            let surface = showcase_frame(&show, w, h, edit);
            println!("\n===== edit {edit:.1}s / {total:.1}s =====");
            for line in surface.to_visible_lines() {
                println!("{line}");
            }
        }
        return Ok(());
    }

    // Live demo: loop the film, driven by the harness clock.
    let show = Showcase::new();
    App::fullscreen()
        .skin(skins::VAPOR95)
        .fps(30)
        .run(show, update, view)?;
    Ok(())
}

/// Live view: map the harness clock onto the (looping) edit clock and paint.
fn view(show: &Showcase, cx: &BuildCx) -> Element<Msg> {
    let (w, h) = (cx.environment.width, cx.environment.height);
    let edit = cx.time.as_secs_f32() % stage::duration().max(0.001);
    let surface = showcase_frame(show, w, h, edit);
    screen::<Msg>().child(raster::<Msg>(surface).grow(1.0))
}

/// Live update: quit on `q` / `Ctrl-C`; the film is non-interactive.
fn update(_show: &mut Showcase, event: AppEvent<Msg>) -> Control {
    if let AppEvent::Input(Event::Key(key)) = event {
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            return Control::Quit;
        }
    }
    Control::Continue
}
