//! `interactive_showcase` — the hands-on sibling of the `cinematic_showcase` film.
//!
//! The film flies *itself*; this one you fly. Same constellation of living
//! instruments (the six experience grammars and three `gibson::plot` Observatory
//! scenes on a perspective ring), same deep-space look, and — crucially — the same
//! shared [`showcase`] core, so there is exactly one experience model and one
//! congruence law behind both.
//!
//! Two modes:
//!
//! * **Orbit** — the camera rests at a wide pose framing one panel. `←`/`→` rotate
//!   the ring to the previous/next instrument; `Enter` flies the camera in.
//! * **Zoom** — the camera settles on the panel's frontal face and its **real UI
//!   resolves live** into the projected rectangle (the facade law), driven by hand.
//!   On a grammar: `←`/`→` move the selection, `↑`/`↓` switch destination, `Enter`
//!   activates. The one shared navigation state is driven through whichever grammar
//!   is in frame — rotate to another grammar and the *same* selection re-presents in
//!   the new grammar. That is the v0.5 fusion thesis, made interactive. `[`/`]` tour
//!   to the neighbouring panel without leaving zoom; `Esc` pulls back to orbit.
//!
//! The camera eases toward its target pose with frame-rate-independent exponential
//! smoothing, so motion stays smooth however input arrives. When settled on a
//! panel's dwell pose the projected rectangle equals the baked [`stage::hold_rect`],
//! so the live UI blits 1:1 over its own texture — blocky far, crisp near, no reflow.
//!
//! ## Usage
//!   cargo run --release --example interactive_showcase

#[path = "cinematic_showcase/look.rs"]
mod look;
#[path = "cinematic_showcase/observatory.rs"]
mod observatory;
#[path = "cinematic_showcase/shot.rs"]
mod shot;
#[path = "cinematic_showcase/showcase.rs"]
mod showcase;
#[path = "cinematic_showcase/stage.rs"]
mod stage;
#[path = "cinematic_showcase/texture.rs"]
mod texture;

use gibson::cell::{Cell, Color, Style};
use gibson::geom::Vec3;
use gibson::input::{Event, KeyCode, KeyModifiers};
use gibson::raster3d::Camera;
use gibson::ui::experience::Intent;
use gibson::ui::{raster, screen, skins, App, AppEvent, BuildCx, Control, Element};
use gibson::Surface;
use shot::GrammarId;
use showcase::{frame_raster_cam, overlay_hud, panel_live_surface, Msg, Showcase};
use std::io;

/// How fast the camera eases toward its target pose (1/seconds). Exponential
/// smoothing: a higher constant settles sooner. Tuned to arrive in roughly 3/4 s.
const EASE_K: f32 = 6.0;

/// How long an activation ("▶ Play …") stays in the status line, in seconds.
const STATUS_TTL: f32 = 2.2;

/// The live, user-driven state: the shared showcase core plus the camera and which
/// panel is framed / zoomed.
struct Interactive {
    show: Showcase,
    /// Which panel `[0, N_PLANES)` is currently framed.
    focus: usize,
    /// Settled close on the panel's face (interacting) vs. resting at the wide orbit.
    zoomed: bool,
    /// The current (eased) camera.
    cam: Camera,
    /// Previous `Tick` elapsed seconds, for a wall-clock `dt`.
    last_tick: f32,
    /// Live animation clock for the focused panel (springs, scopes). Reset on zoom-in
    /// so the hand-off starts congruent with the baked texture, then advances.
    play_time: f32,
    /// A transient action read-out (text, seconds remaining) shown after `Enter`.
    status: Option<(String, f32)>,
}

impl Interactive {
    fn new() -> Self {
        let show = Showcase::new();
        Self {
            show,
            focus: 0,
            zoomed: false,
            cam: stage::orbit_camera(0),
            last_tick: 0.0,
            play_time: 0.0,
            status: None,
        }
    }

    /// True when the focused panel is one of the interactive grammar panels (the
    /// first `REEL_ORDER.len()`), as opposed to a watch-only Observatory scene.
    fn focus_is_grammar(&self) -> bool {
        self.focus < GrammarId::REEL_ORDER.len()
    }

    /// The pose the camera is easing toward right now.
    fn target_camera(&self) -> Camera {
        if self.zoomed {
            stage::dwell_camera(self.focus)
        } else {
            stage::orbit_camera(self.focus)
        }
    }
}

fn lerp3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    Vec3::new(
        a.x + (b.x - a.x) * t,
        a.y + (b.y - a.y) * t,
        a.z + (b.z - a.z) * t,
    )
}

/// Ease `cam` toward `target` by one `dt` step of exponential smoothing. The blend
/// factor `1 - e^(-k·dt)` is frame-rate independent, so the glide looks the same
/// whatever the frame cadence, and it approaches the dwell pose monotonically (never
/// overshooting) so the projected rect only ever grows toward the hold rect.
fn ease_camera(cam: &mut Camera, target: &Camera, dt: f32) {
    let a = (1.0 - (-EASE_K * dt).exp()).clamp(0.0, 1.0);
    cam.position = lerp3(cam.position, target.position, a);
    cam.target = lerp3(cam.target, target.target, a);
    cam.fov_y += (target.fov_y - cam.fov_y) * a;
}

/// Turn a kebab/lower id into a display label: `"neon-harbour"` → `"Neon Harbour"`.
fn humanize(id: &str) -> String {
    id.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The status line an activated action reads as.
fn render_msg(msg: &Msg) -> String {
    match msg {
        Msg::Play(id) => format!("▶ Play · {}", humanize(id)),
        Msg::Toggle(key) => format!("⟳ Cycle · {}", humanize(key)),
        Msg::Open(key) => format!("⤢ Open · {}", humanize(key)),
    }
}

/// Compose one interactive frame: the ring through the live camera, the focused
/// panel's real UI resolved in when its face is large and frontal, the panel HUD, and
/// a control/status strip.
fn compose(model: &Interactive, w: u16, h: u16, backdrop_time: f32) -> Surface {
    let (mw, mh) = model.show.ensure_reel(w, h);
    let cache = model.show.reel.borrow();
    let reel = &cache.reel;

    let mut surface =
        frame_raster_cam(reel, w, h.saturating_mul(2), &model.cam, backdrop_time).to_surface();

    // The facade law: while the focused panel's face projects large and frontal,
    // resolve its real UI into the rectangle at the master size the texture was baked
    // from — blocky far, 1:1 crisp at the settled dwell pose, no reflow.
    if let Some(b) = stage::panel_cell_bounds_cam(model.focus, &model.cam, w, h) {
        let master = panel_live_surface(&model.show, model.focus, mw, mh, model.play_time);
        texture::blit_surface_scaled(&mut surface, &master, b);
    }
    overlay_hud(
        &mut surface,
        reel,
        &model.show.experience.title,
        model.focus,
        w,
        h,
    );
    overlay_controls(model, &mut surface, w, h);
    surface
}

/// Bake the control/status strip just above the HUD lower-third: left is the active
/// mode's key hints (or a transient action read-out), right is the mode and panel index.
fn overlay_controls(model: &Interactive, surface: &mut Surface, w: u16, h: u16) {
    if w < 24 || h < 8 {
        return; // too small to frame; the HUD guards itself
    }
    let (mode, hint) = if !model.zoomed {
        ("ORBIT", "←/→ browse   Enter zoom   q quit")
    } else if model.focus_is_grammar() {
        (
            "ZOOM",
            "←/→ select   ↑/↓ section   Enter play   [ ] tour   Esc back",
        )
    } else {
        ("ZOOM", "live instrument   [ ] tour   Esc back")
    };
    let left = match &model.status {
        Some((text, _)) => text.clone(),
        None => hint.to_string(),
    };
    let right = format!("{mode} · {}/{}", model.focus + 1, stage::N_PLANES);

    let y = h.saturating_sub(shot::HUD_BAND + 1);
    let plate = Style::new().bg(Color::Rgb(14, 9, 24));
    for x in 0..w {
        surface.set_cell(x, y, Cell::space(plate));
    }
    surface.bake_text(2, y, &left, (226, 214, 245), false);
    let rn = right.chars().count() as u16;
    let rx = w.saturating_sub(rn + 2);
    surface.bake_text(rx, y, &right, (236, 170, 246), true);
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Headless structural probe for eyeballing without a TTY: render a few
    // representative (focus, mode) states with the camera snapped to its settled
    // pose. `interactive_showcase probe [W H]`.
    if args.get(1).map(String::as_str) == Some("probe") {
        let w: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(120);
        let h: u16 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(36);
        probe(w, h);
        return Ok(());
    }

    let model = Interactive::new();
    App::fullscreen()
        .skin(skins::VAPOR95)
        .fps(30)
        .run(model, update, view)?;
    Ok(())
}

/// Render a handful of representative states to text, camera settled on each target
/// pose, so the composition can be verified off a live terminal.
fn probe(w: u16, h: u16) {
    let states: &[(usize, bool, &str)] = &[
        (0, false, "orbit · panel 1 (STANDARD)"),
        (0, true, "zoom · STANDARD (live list)"),
        (2, true, "zoom · MEDIA SHELF (cover flow, live)"),
        (6, true, "zoom · OBSERVATORY · SCOPE (live)"),
    ];
    for &(focus, zoomed, label) in states {
        let mut model = Interactive::new();
        model.focus = focus;
        model.zoomed = zoomed;
        model.play_time = 0.6;
        model.cam = model.target_camera(); // snap to the settled pose
        let surface = compose(&model, w, h, 2.0);
        println!("\n===== {label} ({w}x{h}) =====");
        for line in surface.to_visible_lines() {
            println!("{line}");
        }
    }
}

/// Render from the current model state; the starfield twinkles on the harness clock.
fn view(model: &Interactive, cx: &BuildCx) -> Element<Msg> {
    let (w, h) = (cx.environment.width, cx.environment.height);
    let surface = compose(model, w, h, cx.time.as_secs_f32());
    screen::<Msg>().child(raster::<Msg>(surface).grow(1.0))
}

/// Advance the camera / clocks on `Tick`; route keys on `Input`.
fn update(model: &mut Interactive, event: AppEvent<Msg>) -> Control {
    match event {
        AppEvent::Tick(elapsed) => {
            let t = elapsed.as_secs_f32();
            let dt = (t - model.last_tick).clamp(0.0, 0.25);
            model.last_tick = t;
            model.play_time += dt;
            if let Some((_, ttl)) = &mut model.status {
                *ttl -= dt;
            }
            if matches!(&model.status, Some((_, ttl)) if *ttl <= 0.0) {
                model.status = None;
            }
            let target = model.target_camera();
            ease_camera(&mut model.cam, &target, dt);
            Control::Continue
        }
        AppEvent::Input(Event::Key(key)) => handle_key(model, key),
        _ => Control::Continue,
    }
}

/// The input grammar. Arrows are *contextual* (rotate the ring in orbit; drive the UI
/// in zoom); `[`/`]` always tour panels; `Enter`/`Esc` change mode.
fn handle_key(model: &mut Interactive, key: gibson::input::KeyEvent) -> Control {
    let n = stage::N_PLANES;
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => return Control::Quit,
        KeyCode::Char('c') if ctrl => return Control::Quit,
        KeyCode::Esc => {
            if model.zoomed {
                model.zoomed = false;
            } else {
                return Control::Quit;
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            if !model.zoomed {
                model.zoomed = true;
                model.play_time = 0.0; // start the hand-off congruent with the texture
            } else if model.focus_is_grammar() {
                if let Some(msg) = model.show.apply(Intent::Enter) {
                    model.status = Some((render_msg(&msg), STATUS_TTL));
                }
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if model.zoomed && model.focus_is_grammar() {
                model.show.apply(Intent::Previous);
            } else if !model.zoomed {
                model.focus = (model.focus + n - 1) % n;
            }
        }
        KeyCode::Right | KeyCode::Char('l') => {
            if model.zoomed && model.focus_is_grammar() {
                model.show.apply(Intent::Next);
            } else if !model.zoomed {
                model.focus = (model.focus + 1) % n;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if model.zoomed && model.focus_is_grammar() {
                model.show.apply(Intent::PreviousGroup);
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if model.zoomed && model.focus_is_grammar() {
                model.show.apply(Intent::NextGroup);
            }
        }
        // Tour to the neighbouring panel without leaving the current zoom.
        KeyCode::Char('[') => model.focus = (model.focus + n - 1) % n,
        KeyCode::Char(']') => model.focus = (model.focus + 1) % n,
        _ => {}
    }
    Control::Continue
}
