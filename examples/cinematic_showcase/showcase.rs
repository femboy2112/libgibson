//! The shared **showcase core** — the one semantic experience, its grammars and
//! Observatory scenes baked onto the ring, and the congruence law that makes the
//! blocky-approach → crisp-hold hand-off seamless. Both the scripted film
//! ([`crate`] `cinematic_showcase`) and the user-driven `interactive_showcase`
//! build on this, so there is exactly *one* experience model and *one* congruence
//! law, never two that could drift.
//!
//! The congruence law: the baked plane texture (the coarse approach view) and the
//! live crisp hold are rendered from **one** [`Surface`] model at **one** per-terminal
//! master cell size ([`master_size`], the projected hold rectangle). A reflowing
//! grammar lands its elements in identical positions in the RGB texture and the ASCII
//! hold, so a panel only ever goes blocky→crisp, never re-lays-out, and the hold is a
//! 1:1 blit — every other frame a pure down-scale, never an up-scale (which
//! nearest-neighbour would show as duplicated rows at a large terminal).

#![allow(dead_code)] // each driver uses a subset of the core.

use crate::{look, observatory, stage, texture};
use gibson::capability::ColorDepth;
use gibson::raster::RgbRaster;
use gibson::raster3d::Camera;
use gibson::ui::experience::{
    apply_intent, Action, Content as ExpContent, Destination, Experience, ExperienceRuntime, Facet,
    Intent, Item, Media,
};
use gibson::ui::skin::UiEnvironment;
use gibson::ui::{screen, skins, BuildCx};
use gibson::Surface;
use std::cell::RefCell;
use std::time::Duration;

use crate::shot::{cinematic_hud, GrammarId, SceneId};

/// The one application action type: the showcase presents the same shared semantic
/// library through every grammar, so there is a single `Msg` for the whole reel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
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
/// duplicated rows at a large (e.g. fullscreen) terminal.
pub fn master_size(w: u16, h: u16) -> (u16, u16) {
    stage::hold_rect(0, w, h)
        .map(|r| (r.width, r.height))
        .unwrap_or((
            MASTER_W_FALLBACK.min(w.max(1)),
            MASTER_H_FALLBACK.min(h.max(1)),
        ))
}

/// Deterministic seed for the starfield backdrop.
pub const STARFIELD_SEED: u32 = 0x5055_4152;

/// The VAPOR95 skin's screen background (`src/ui/skin.rs`), the fill a grammar's
/// bare screen reads as once rasterised.
pub const VAPOR95_BG: (u8, u8, u8) = (61, 35, 79);
/// The deep-space ink a plot scene's bare field reads as.
pub const OBSERVATORY_BG: (u8, u8, u8) = (10, 8, 20);

/// The base presentation time the grammar ring textures are baked at; a grammar
/// panel's live hold animates forward from here (so at `local == 0` the baked
/// texture and the live hold are the identical frame — the hand-off is seamless).
pub const PANEL_T0: f32 = 1.5;

/// The same base, for observatory scenes — the flattering moment their textures are
/// baked at and the moment their live hold animates forward from.
pub const OBSERVATORY_T0: f32 = 3.2;

/// The single semantic description of the application — authored once, never forked
/// per grammar.
pub fn build_experience() -> Experience<Msg> {
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
/// HUD, ordered grammars first (`GrammarId::REEL_ORDER`) then the three Observatory
/// scenes (`SceneId::ALL`) — the order the ring hangs them in.
pub struct Reel {
    pub texes: Vec<RgbRaster>,
    pub refls: Vec<RgbRaster>,
    pub labels: Vec<(String, String)>,
}

/// The baked reel plus the master cell size it was baked for. The textures are baked
/// for one terminal's hold-rect master size; a resize to a different size re-bakes, so
/// the plane texture and the hold always stay the same congruent layout at 1:1.
pub struct CachedReel {
    pub mw: u16,
    pub mh: u16,
    pub reel: Reel,
}

/// The shared live state: the authored experience, a persistent runtime that holds
/// the single navigation state driven through every grammar, and the size-cached reel.
/// During a panel's crisp hold the runtime re-presents that one panel live so it
/// animates in full quality, at the same master size the ring texture was baked at.
pub struct Showcase {
    pub experience: Experience<Msg>,
    pub ui: RefCell<ExperienceRuntime<Msg>>,
    pub reel: RefCell<CachedReel>,
}

impl Showcase {
    pub fn new() -> Self {
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
    pub fn ensure_reel(&self, w: u16, h: u16) -> (u16, u16) {
        let (mw, mh) = master_size(w, h);
        if self.reel.borrow().mw == mw && self.reel.borrow().mh == mh {
            return (mw, mh);
        }
        let reel = build_reel(&self.experience, &self.ui, mw, mh);
        *self.reel.borrow_mut() = CachedReel { mw, mh, reel };
        (mw, mh)
    }

    /// Apply a navigation intent to the one shared presentation state, returning the
    /// action it produced (only `Intent::Enter` yields one). The same state is then
    /// re-presented through whichever grammar is in frame — the fusion thesis.
    pub fn apply(&self, intent: Intent) -> Option<Msg> {
        let experience = &self.experience;
        let mut ui = self.ui.borrow_mut();
        apply_intent(experience, ui.state_mut(), intent)
    }
}

impl Default for Showcase {
    fn default() -> Self {
        Self::new()
    }
}

/// Lower panel `i` to a full crisp `Surface` at `w × h` cells, animated at hold-local
/// time `local`: grammars re-present through the runtime; observatory scenes re-plot.
/// This is the full-quality render a frame locks onto during a hold.
pub fn panel_live_surface(show: &Showcase, i: usize, w: u16, h: u16, local: f32) -> Surface {
    let grammars = GrammarId::REEL_ORDER.len();
    if i < grammars {
        let g = GrammarId::REEL_ORDER[i];
        let now = Duration::from_secs_f32((PANEL_T0 + local).max(0.0));
        grammar_surface(
            &show.experience,
            &show.ui,
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
pub fn build_reel(
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

/// Lower one grammar to a `Surface`: present the shared value through the grammar at
/// presentation time `now`, host it on a `screen` so it fills the viewport, then
/// compile / lay out / paint — the headless lowering the flagship's `dump` path uses.
pub fn grammar_surface(
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

/// Render one frame of the ring to an `RgbRaster` of `pw × ph` pixels through an
/// arbitrary camera (the scripted clock, or interactive drive): nebula backdrop (animated
/// by `time`), the constellation of panels through `cam`, then bloom and a final grade.
pub fn frame_raster_cam(reel: &Reel, pw: u16, ph: u16, cam: &Camera, time: f32) -> RgbRaster {
    let backdrop = look::starfield(pw, ph, time, STARFIELD_SEED);
    let mut raster = stage::render_frame_cam(cam, pw, ph, backdrop, &reel.texes, &reel.refls);
    look::bloom(&mut raster, 168, 9, 1.8);
    look::grade(&mut raster, 0.28);
    raster
}

/// [`frame_raster_cam`] on the scripted edit clock: the camera and the backdrop
/// animation both follow `edit`. Pure function of `edit`.
pub fn frame_raster(reel: &Reel, pw: u16, ph: u16, edit: f32) -> RgbRaster {
    frame_raster_cam(reel, pw, ph, &stage::camera_at(edit), edit)
}

/// The HUD lower-third / title bar naming the panel currently in frame.
pub fn overlay_hud(surface: &mut Surface, reel: &Reel, title: &str, panel: usize, w: u16, h: u16) {
    if let Some((name, tag)) = reel.labels.get(panel) {
        let hud = cinematic_hud(w, h, title, name, tag);
        surface.blit_transparent_at(&hud, 0, 0);
    }
}
