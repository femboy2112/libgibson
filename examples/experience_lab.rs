//! `experience_lab` — the Experience-Grammar flagship (milestone §24).
//!
//! **One** application, described once as a semantic [`Experience`], rendered live
//! through **six** radically different interface grammars — a boring reference
//! list, a cover shelf, a cross-media bar, a typographic panorama, an orbital
//! focal field and an occluding blade stack — switchable on a keystroke, with the
//! selection surviving every switch. There is deliberately **no** `match style`
//! anywhere in the application's `view` or `update`: the app speaks semantics, the
//! grammar is swapped underneath it, and that is the whole point.
//!
//! ## Controls
//!
//! - `s` — next grammar; `1`..`6` — jump straight to a grammar
//! - arrows (and `h`/`j`/`k`/`l` where a grammar binds them) — navigate
//! - `Enter` — activate the focused item's primary action
//! - `Esc` / `Backspace` — back
//! - `q` / `Ctrl-C` — quit
//!
//! ## Headless
//!
//! Run without a TTY (or under `cargo run --example experience_lab -- frames DIR`)
//! and it renders deterministic frames instead of taking the terminal. The
//! `#[test]`s below are the CI contract: they prove — headlessly — that the
//! selection and active destination survive a live switch through every grammar,
//! and that every grammar's rendered frame honours the preservation law.

use gibson::capability::ColorDepth;
use gibson::context::{Context, RenderMode};
use gibson::input::{Event, KeyCode, KeyModifiers};
use gibson::raster::RgbRaster;
use gibson::ui::experience::{
    apply_intent, Action, Content, Destination, Experience, ExperienceRuntime, ExperienceStyle,
    Facet, Intent, Item, Media, PresentationState,
};
use gibson::ui::prelude::UiRuntime;
use gibson::ui::skin::UiEnvironment;
use gibson::ui::{raster, screen, skins, App, AppEvent, BuildCx, Control, Element};
use gibson::Surface;
use std::cell::RefCell;
use std::io;
use std::time::Duration;

/// The one application action type. Every grammar hands exactly these back; none
/// of them knows what the actions *mean* — that is the application's business.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Msg {
    Play(String),
    Toggle(String),
    Open(String),
}

/// The single semantic description of the application. Authored once; never forked
/// per grammar. A media library (collection), a now-playing inspector (detail), a
/// settings sheet (collection of value-cycling items) and an about page (prose) —
/// exercising every [`Content`] kind.
fn build_experience() -> Experience<Msg> {
    let library = Content::Collection(vec![
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

    let now_playing = Content::Detail {
        facets: vec![
            Facet::new("track", "Track", "Neon Harbour"),
            Facet::new("artist", "Artist", "Vapor Cartography"),
            Facet::new("time", "Elapsed", "01:12 / 03:48"),
        ],
        actions: vec![Action::new("open-np", "Open", Msg::Open("now".into()))],
    };

    let settings = Content::Collection(vec![
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

    let about = Content::Prose(vec![
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

/// The application model. Note what is *not* here: no per-style branch, no
/// per-style layout, and no hand-rolled grammar registry. Just the semantic value
/// and an [`ExperienceRuntime`] that owns the presentation orchestration (the
/// grammar-independent navigation state and the persistent grammar instances).
///
/// The runtime is behind a `RefCell` only because this App harness hands `view` an
/// immutable `&Lab` while a grammar's `present` needs `&mut` for its spring state —
/// one cell around the whole runtime, not the old `Vec<RefCell<Box<dyn Grammar>>>`.
struct Lab {
    experience: Experience<Msg>,
    ui: RefCell<ExperienceRuntime<Msg>>,
    last_action: Option<Msg>,
    quit: bool,
    /// Whether the HUD carries the live dev affordances (controls hint + last
    /// action). On for the interactive demo; off for filmic captures, which want
    /// a clean cinematic frame, not a keybinding legend burned into the video.
    controls: bool,
}

impl Lab {
    fn new() -> Self {
        let experience = build_experience();
        let ui = ExperienceRuntime::with_builtins(&experience);
        Self {
            experience,
            ui: RefCell::new(ui),
            last_action: None,
            quit: false,
            controls: true,
        }
    }

    /// A lab whose HUD is the clean cinematic frame (no controls legend, no debug
    /// readout) — used by the `seq`/`ansi` capture paths.
    fn capture() -> Self {
        Self {
            controls: false,
            ..Self::new()
        }
    }
}

// ---- the cinematic HUD: title card + lower-third, in the baked-raster ethos ----
//
// Built *entirely on the public API* — `Surface::bake_text` (the ethos primitive:
// crisp glyphs whose cell backgrounds are sampled from the raster beneath, so no
// opaque terminal block is punched), `RgbRaster` + `to_surface` for the scrim
// bands, and `stack()` + `raster()` to layer the HUD over the grammar's own
// presentation. This is the dogfood for the showcase: composing baked overlays on
// a grammar presentation without reaching into the crate. If this got ugly, the
// seam would be wrong — it stays this small because the one missing primitive
// (`bake_text`) now lives on `Surface`.

/// Height, in cells, of each HUD band (title card and lower-third).
const HUD_BAND: u16 = 2;

const HUD_DARK: (u8, u8, u8) = (10, 7, 20);
const HUD_SCRIM: (u8, u8, u8) = (48, 23, 72);
const HUD_SPINE: (u8, u8, u8) = (231, 122, 219);
const HUD_STRONG: (u8, u8, u8) = (246, 247, 253);
const HUD_ACCENT: (u8, u8, u8) = (236, 170, 246);
const HUD_DIM: (u8, u8, u8) = (176, 170, 205);

fn lerp(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let m = |x: u8, y: u8| {
        (x as f32 + (y as f32 - x as f32) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// Blit a scrim band — a vertical gradient with a bright accent spine on the left
/// and an accent edge-line — of `HUD_BAND` cells into `layer` at cell-row `cy`.
/// `bright_bottom` faces the brighter gradient edge toward the grammar (down for
/// the top band, up for the lower-third).
fn blit_scrim(layer: &mut Surface, w: u16, cy: u16, bright_bottom: bool) {
    let ph = (HUD_BAND * 2).max(1);
    let mut band = RgbRaster::new(w, ph);
    let denom = ph.saturating_sub(1).max(1) as f32;
    for y in 0..ph {
        let t = y as f32 / denom;
        let v = if bright_bottom { t } else { 1.0 - t };
        let base = lerp(HUD_DARK, HUD_SCRIM, 0.1 + 0.9 * v);
        for x in 0..w {
            band.set(x as i32, y as i32, base);
        }
    }
    for y in 0..ph as i32 {
        band.blend(0, y, HUD_SPINE, 0.95);
        band.blend(1, y, HUD_SPINE, 0.40);
    }
    let edge = if bright_bottom { ph as i32 - 1 } else { 0 };
    for x in 0..w as i32 {
        band.blend(x, edge, HUD_SPINE, 0.55);
    }
    layer.blit_transparent_at(&band.to_surface(), 0, cy);
}

/// Build the HUD overlay layer: a top title card (experience title · active style)
/// and a bottom lower-third (now-showing caption + optional controls hint),
/// transparent everywhere else so the grammar shows through untouched.
fn cinematic_hud(
    w: u16,
    h: u16,
    title: &str,
    style: &str,
    caption: &str,
    controls: Option<&str>,
) -> Surface {
    let mut layer = Surface::new_transparent(w, h);
    if w < 8 || h < 6 {
        return layer; // too small to frame; leave the grammar bare
    }

    // Top title card: experience title on the spine, active style right-aligned.
    blit_scrim(&mut layer, w, 0, true);
    layer.bake_text(2, 0, title, HUD_STRONG, true);
    let caps = style.to_uppercase();
    let sx = w.saturating_sub(caps.chars().count() as u16 + 2);
    layer.bake_text(sx, 0, &caps, HUD_ACCENT, true);

    // Bottom lower-third: now-showing caption, with the controls hint beneath it.
    let by = h - HUD_BAND;
    blit_scrim(&mut layer, w, by, false);
    layer.bake_text(2, by, caption, HUD_STRONG, true);
    if let Some(c) = controls {
        layer.bake_text(2, by + 1, c, HUD_DIM, false);
    }
    layer
}

/// The now-showing caption for the current destination/selection.
fn hud_caption(exp: &Experience<Msg>, state: &PresentationState) -> String {
    let Some(active) = exp.destinations.get(state.active_index(exp)) else {
        return String::new();
    };
    match &active.content {
        Content::Collection(items) if !items.is_empty() => {
            let si = state.selected_index(exp).unwrap_or(0).min(items.len() - 1);
            let item = &items[si];
            match &item.subtitle {
                Some(s) if !s.is_empty() && s != "\u{2014}" => {
                    format!("\u{266a} {}   \u{00b7}   {}", item.title, s)
                }
                _ => format!("\u{266a} {}", item.title),
            }
        }
        _ => active.title.clone(),
    }
}

/// The one and only view. It picks the active grammar, presents the shared
/// semantic value through it, and overlays a thin lab status line. There is no
/// `match` on the style here — the grammar is a value, swapped underneath.
fn view(lab: &Lab, cx: &BuildCx) -> Element<Msg> {
    // One call: present the shared semantic value through the active grammar. The
    // grammar is a value the runtime swaps underneath — there is no `match style`.
    let presented = lab
        .ui
        .borrow_mut()
        .present(&lab.experience, &cx.environment, cx.time);

    let (w, h) = (cx.environment.width, cx.environment.height);
    let (style_name, caption) = {
        let ui = lab.ui.borrow();
        (
            ui.style_name().to_string(),
            hud_caption(&lab.experience, ui.state()),
        )
    };
    let controls = lab
        .controls
        .then_some("s style · 1-6 pick · arrows nav · ↵ act · Esc back · q quit");

    // Compose the cinematic HUD and layer it over the grammar. The grammar shows
    // through everywhere the HUD is transparent; the two bands sit on top.
    let mut hud = cinematic_hud(w, h, &lab.experience.title, &style_name, &caption, controls);
    if lab.controls && w >= 8 && h >= 6 {
        if let Some(msg) = &lab.last_action {
            let readout = format!("last: {msg:?}");
            let x = w.saturating_sub(readout.chars().count() as u16 + 2);
            hud.bake_text(x, h - HUD_BAND + 1, &readout, HUD_DIM, false);
        }
    }

    // Layer the HUD over the grammar. The overlay host is *our* `screen` (a
    // guaranteed `Screen`, which the overlay lowering fills to the viewport), so
    // the full-height HUD spans top-to-bottom regardless of how tall the grammar's
    // own content is — the lower-third lands at the bottom even for the short
    // STANDARD floor. The HUD's transparent cells show the grammar through; the
    // two scrim bands sit on top.
    screen::<Msg>()
        .child(presented.element.grow(1.0))
        .overlay(raster::<Msg>(hud))
}

/// The one and only update. Lab-global keys (style switch / quit) are handled
/// here; every other key is routed, unmodified, through the active grammar's own
/// interaction metaphor via `handle_key`. The returned `Msg` is the application's
/// to interpret — here we just record it for the status line.
fn update(lab: &mut Lab, event: AppEvent<Msg>) -> Control {
    match event {
        AppEvent::Action(msg) => {
            lab.last_action = Some(msg);
            Control::Continue
        }
        AppEvent::Input(Event::Key(key)) => {
            // Quit.
            if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
                || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
            {
                lab.quit = true;
                return Control::Quit;
            }
            // Live style switch — the heart of the demo. The semantic state is
            // untouched, so the selection survives the switch.
            match key.code {
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    lab.ui.get_mut().next_style();
                    return Control::Continue;
                }
                KeyCode::Char(d @ '1'..='6') => {
                    let style = ExperienceStyle::ALL[d as usize - '1' as usize];
                    lab.ui.get_mut().set_style(style);
                    return Control::Continue;
                }
                _ => {}
            }
            // Everything else: the active grammar decides what the key means, and
            // the runtime applies it to the shared state identically for every
            // grammar. The returned `Msg` is the application's to interpret.
            if let Some(msg) = lab.ui.get_mut().handle_key(&lab.experience, &key) {
                lab.last_action = Some(msg);
            }
            Control::Continue
        }
        AppEvent::Input(_) | AppEvent::Tick(_) => Control::Continue,
    }
}

/// Render one full lab frame (grammar + chrome) to truecolor ANSI bytes, exactly
/// as the live demo paints it, at a given viewport and presentation time. Shared by
/// the `ansi` (single still) and `seq` (animation) capture paths.
fn capture_frame(lab: &Lab, w: u16, h: u16, now: Duration) -> Vec<u8> {
    let env = UiEnvironment {
        width: w,
        height: h,
        color_depth: ColorDepth::TrueColor,
        ..UiEnvironment::default()
    };
    let mut cx = BuildCx::new(skins::VAPOR95, env);
    cx.time = now;
    // `App::fullscreen` forces the root to the viewport; mirror that so the capture
    // shows the real live framing (the skin's screen fills), not a content clump.
    let element = view(lab, &cx).height(h);
    let mut runtime = UiRuntime::new(skins::VAPOR95);
    let compiled = runtime.frame(&element, env, now).expect("frame lowers");
    let mut ctx = Context::headless(RenderMode::Fullscreen, w, h);
    ctx.set_color_depth(ColorDepth::TrueColor);
    ctx.set_root(compiled.node);
    ctx.render_now().expect("headless render");
    ctx.rendered_bytes().to_vec()
}

/// Drive the active grammar's shared navigation state the way a key press would.
fn navigate(lab: &Lab, intent: Intent) {
    apply_intent(&lab.experience, lab.ui.borrow_mut().state_mut(), intent);
}

/// A scripted capture scenario: emit a sequence of truecolor ANSI frames to
/// `outdir/frame_NNNN.ans`, advancing presentation time so springy/continuous
/// grammars actually move, and scripting navigation so settling grammars glide.
/// Returns the number of frames written. (Milestone §52 temporal sequences.)
fn capture_sequence(scenario: &str, outdir: &str, w: u16, h: u16) -> io::Result<u32> {
    std::fs::create_dir_all(outdir)?;
    let lab = Lab::capture();
    let dt = Duration::from_millis(50); // 20fps edit clock
    let mut n: u32 = 0;
    let mut now = Duration::ZERO;
    let shoot = |lab: &Lab, now: Duration, n: &mut u32| -> io::Result<()> {
        let bytes = capture_frame(lab, w, h, now);
        std::fs::write(format!("{outdir}/frame_{:04}.ans", *n), &bytes)?;
        *n += 1;
        Ok(())
    };

    match scenario {
        // Representation Atlas: the SAME selected item through all six grammars.
        "tour" => {
            navigate(&lab, Intent::Next);
            navigate(&lab, Intent::Next); // select the third library item
            for style in ExperienceStyle::ALL {
                lab.ui.borrow_mut().set_style(style);
                for _ in 0..16 {
                    shoot(&lab, now, &mut n)?;
                    now += dt;
                }
            }
        }
        // Continuous orbital motion, with two ring rotations.
        "orbital" => {
            lab.ui.borrow_mut().set_style(ExperienceStyle::Orbital);
            for i in 0..60 {
                if i == 20 || i == 38 {
                    navigate(&lab, Intent::NextCyclic);
                }
                shoot(&lab, now, &mut n)?;
                now += dt;
            }
        }
        // Cover-flow: step through the shelf, watching the spring glide and settle.
        "shelf" => {
            lab.ui.borrow_mut().set_style(ExperienceStyle::MediaShelf);
            for i in 0..66 {
                if i > 0 && i % 12 == 0 {
                    navigate(&lab, Intent::Next);
                }
                shoot(&lab, now, &mut n)?;
                now += dt;
            }
        }
        // Blade stack: glide through the destination blades.
        "blades" => {
            lab.ui.borrow_mut().set_style(ExperienceStyle::Blades);
            for i in 0..56 {
                if i > 0 && i % 13 == 0 {
                    navigate(&lab, Intent::NextGroup);
                }
                shoot(&lab, now, &mut n)?;
                now += dt;
            }
        }
        other => {
            eprintln!("unknown scenario '{other}' (tour|orbital|shelf|blades)");
        }
    }
    Ok(n)
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Animated capture for GIFs / visual acceptance (§52): `experience_lab seq
    // <scenario> <outdir> [W H]` writes a scripted truecolor ANSI frame sequence.
    if args.get(1).map(String::as_str) == Some("seq") {
        let scenario = args.get(2).map(String::as_str).unwrap_or("tour");
        let outdir = args.get(3).cloned().unwrap_or_else(|| "frames".to_string());
        let w: u16 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(100);
        let h: u16 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(30);
        let frames = capture_sequence(scenario, &outdir, w, h)?;
        println!("{scenario}: {frames} frames -> {outdir} ({w}x{h})");
        return Ok(());
    }

    // Headless text snapshot for visual acceptance (§43) and quick eyeballing:
    // `experience_lab dump [W H]` prints every grammar's frame as visible text at
    // a fixed state, deterministically. (A Surface is terminal cells, not pixels,
    // so this is the faithful headless capture; the live demo carries the colour.)
    if args.get(1).map(String::as_str) == Some("dump") {
        let w: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100);
        let h: u16 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(28);
        let lab = Lab::new();
        let env = UiEnvironment {
            width: w,
            height: h,
            color_depth: ColorDepth::TrueColor,
            ..UiEnvironment::default()
        };
        for style in ExperienceStyle::ALL {
            lab.ui.borrow_mut().set_style(style);
            let name = style.name();
            let presented =
                lab.ui
                    .borrow_mut()
                    .present(&lab.experience, &env, Duration::from_millis(500));
            let cx = BuildCx::new(skins::VAPOR95, env);
            let mut node = gibson::ui::compile(&presented.element, &cx)
                .expect("grammar lowers")
                .node;
            gibson::compute_layout(&mut node, w, h).expect("layout");
            let mut surface = gibson::Surface::new(w, h);
            gibson::paint(&node, &mut surface);
            println!("\n===== {name} {w}x{h} =====");
            for line in surface.to_visible_lines() {
                println!("{line}");
            }
        }
        return Ok(());
    }

    // Truecolor capture for real visual acceptance (§43): `experience_lab ansi <1-6>
    // [W H]` renders ONE grammar's full lab frame — grammar *and* chrome, exactly as
    // the live demo paints it — and writes the settled screen as ANSI to stdout. A
    // Surface flattens to colourless text; this path keeps the colour and the raster
    // half-block pixels, so an external renderer (docs/assets/render_ui_skins.py) can
    // turn it into an honest PNG instead of the lossy text dump.
    if args.get(1).map(String::as_str) == Some("ansi") {
        let idx = args
            .get(2)
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        let w: u16 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(100);
        let h: u16 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(28);
        let lab = Lab::capture();
        let style = ExperienceStyle::ALL
            .get(idx)
            .copied()
            .unwrap_or(ExperienceStyle::Standard);
        lab.ui.borrow_mut().set_style(style);
        // A settled clock: springy grammars have parked, so this is the still frame.
        let bytes = capture_frame(&lab, w, h, Duration::from_secs(5));
        use std::io::Write;
        io::stdout().write_all(&bytes)?;
        return Ok(());
    }

    // Live demo.
    let lab = Lab::new();
    App::fullscreen()
        .skin(skins::VAPOR95)
        .fps(60)
        .run(lab, update, view)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gibson::input::KeyEvent;
    use gibson::ui::experience::{apply_intent, required_semantics, Intent};

    fn env(w: u16, h: u16, depth: ColorDepth) -> UiEnvironment {
        UiEnvironment {
            width: w,
            height: h,
            color_depth: depth,
            ..UiEnvironment::default()
        }
    }

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
        }
    }

    /// The §24 invariant: driving the *same* semantic state through every grammar
    /// in turn keeps the selection and the active destination fixed, and every
    /// grammar's rendered frame honours the preservation law. This is the headless
    /// CI proof that a live switch never retargets the selection.
    #[test]
    fn selection_and_active_survive_every_live_switch() {
        let lab = Lab::new();
        // Move off the defaults: select the third library item.
        apply_intent(
            &lab.experience,
            lab.ui.borrow_mut().state_mut(),
            Intent::Next,
        );
        apply_intent(
            &lab.experience,
            lab.ui.borrow_mut().state_mut(),
            Intent::Next,
        );
        let selected = lab.ui.borrow().state().selected_key(&lab.experience);
        let active = lab.ui.borrow().state().active_key(&lab.experience);
        assert_eq!(selected, Some(key_named("tidal-automata")));

        let e = env(120, 40, ColorDepth::TrueColor);
        for style in ExperienceStyle::ALL {
            lab.ui.borrow_mut().set_style(style);
            let name = style.name();
            let presented =
                lab.ui
                    .borrow_mut()
                    .present(&lab.experience, &e, Duration::from_millis(250));
            assert_eq!(
                presented.receipt.selected, selected,
                "{name} moved the selection on a live switch"
            );
            assert_eq!(
                Some(presented.receipt.active_destination.clone()),
                active,
                "{name} moved the active destination on a live switch"
            );
            let required = required_semantics(&lab.experience, lab.ui.borrow().state());
            assert!(
                presented.check(&required).is_empty(),
                "{name} violated the preservation law in the lab"
            );
        }
    }

    /// Every grammar's lab frame (status chrome + presented content) lowers,
    /// lays out and paints across the responsive/capability matrix without panic
    /// or a compile error — the whole demo path, headless.
    #[test]
    fn every_style_renders_through_the_full_lab_frame() {
        let sizes = [(160, 50), (120, 40), (80, 24), (60, 20), (42, 15)];
        let depths = [
            ColorDepth::TrueColor,
            ColorDepth::Ansi256,
            ColorDepth::Ansi16,
            ColorDepth::Mono,
        ];
        for style in ExperienceStyle::ALL {
            let lab = Lab::new();
            lab.ui.borrow_mut().set_style(style);
            for (w, h) in sizes {
                for depth in depths {
                    let e = env(w, h, depth);
                    let cx = BuildCx::new(skins::VAPOR95, e);
                    let element = view(&lab, &cx);
                    let mut node = gibson::ui::compile(&element, &cx)
                        .expect("lab frame lowers to a valid tree")
                        .node;
                    gibson::compute_layout(&mut node, w, h).expect("lab frame lays out");
                    let mut surface = gibson::Surface::new(w, h);
                    gibson::paint(&node, &mut surface);
                    assert!(surface.width == w && surface.height == h);
                }
            }
        }
    }

    /// Lab-global keys switch grammar without touching the semantic state; grammar
    /// keys navigate. Proves the two input layers are cleanly separated.
    #[test]
    fn style_keys_switch_without_disturbing_navigation() {
        let mut lab = Lab::new();
        apply_intent(&lab.experience, lab.ui.get_mut().state_mut(), Intent::Next); // select 2nd
        let before = lab.ui.borrow().state().selected_key(&lab.experience);

        // Direct style pick and cycle leave the selection alone.
        update(
            &mut lab,
            AppEvent::Input(Event::Key(press(KeyCode::Char('4')))),
        );
        assert_eq!(lab.ui.borrow().style_name(), "PANORAMA");
        update(
            &mut lab,
            AppEvent::Input(Event::Key(press(KeyCode::Char('s')))),
        );
        assert_eq!(lab.ui.borrow().style_name(), "ORBITAL");
        assert_eq!(
            lab.ui.borrow().state().selected_key(&lab.experience),
            before,
            "switching grammar must not move the selection"
        );

        // A grammar key navigates. ORBITAL binds Right to Next item.
        update(&mut lab, AppEvent::Input(Event::Key(press(KeyCode::Right))));
        assert_ne!(
            lab.ui.borrow().state().selected_key(&lab.experience),
            before,
            "a navigation key should move the selection"
        );

        // Quit is honoured.
        let control = update(
            &mut lab,
            AppEvent::Input(Event::Key(press(KeyCode::Char('q')))),
        );
        assert_eq!(control, Control::Quit);
        assert!(lab.quit);
    }

    fn key_named(name: &str) -> gibson::ui::element::Key {
        gibson::ui::element::Key::named(name)
    }
}
