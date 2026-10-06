//! The cinematic showcase's **shot vocabulary** and shared chrome.
//!
//! Two things live here, both demo-local art direction (not a general framework,
//! per `docs/FRANK_REACTION_CUT_PLAN.md:84`):
//!
//! 1. The [`Shot`] enum the [`crate::reel`] cue sheet is built from — the opening
//!    establish, one of the six experience [`GrammarId`]s, one of the Observatory
//!    [`SceneId`]s, or the closing reveal. These are the stable payload types every
//!    cue on the authoritative edit clock carries.
//! 2. The cinematic HUD ([`cinematic_hud`]: a title bar + now-showing lower-third),
//!    lifted from the `experience_lab` flagship so the film frames every grammar the
//!    same way — built only on the public `Surface::bake_text` / `RgbRaster` ethos
//!    primitives.

#![allow(dead_code)] // the film uses a subset of the chrome per shot kind.

use gibson::raster::RgbRaster;
use gibson::ui::experience::ExperienceStyle;
use gibson::Surface;

/// One of the six experience grammars, as a timeline payload. Maps 1:1 to
/// [`ExperienceStyle`]; kept as its own enum so the cue sheet reads as film
/// direction ("cut to ORBITAL") rather than library internals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrammarId {
    Standard,
    MediaShelf,
    CrossMedia,
    Panorama,
    Orbital,
    Blades,
}

impl GrammarId {
    /// Film order: the deliberate "Norton floor" first, then escalating spatiality.
    pub const REEL_ORDER: [GrammarId; 6] = [
        GrammarId::Standard,
        GrammarId::CrossMedia,
        GrammarId::MediaShelf,
        GrammarId::Panorama,
        GrammarId::Blades,
        GrammarId::Orbital,
    ];

    pub fn style(self) -> ExperienceStyle {
        match self {
            GrammarId::Standard => ExperienceStyle::Standard,
            GrammarId::MediaShelf => ExperienceStyle::MediaShelf,
            GrammarId::CrossMedia => ExperienceStyle::CrossMedia,
            GrammarId::Panorama => ExperienceStyle::Panorama,
            GrammarId::Orbital => ExperienceStyle::Orbital,
            GrammarId::Blades => ExperienceStyle::Blades,
        }
    }

    /// Short name for the HUD title card (upper-cased by the HUD).
    pub fn name(self) -> &'static str {
        match self {
            GrammarId::Standard => "STANDARD",
            GrammarId::MediaShelf => "MEDIA SHELF",
            GrammarId::CrossMedia => "CROSS-MEDIA",
            GrammarId::Panorama => "PANORAMA",
            GrammarId::Orbital => "ORBITAL",
            GrammarId::Blades => "BLADES",
        }
    }

    /// One-line gloss for the lower-third when a grammar shot opens.
    pub fn tagline(self) -> &'static str {
        match self {
            GrammarId::Standard => "the reference list — the GL layer off, on purpose",
            GrammarId::MediaShelf => "cover flow — a shelf that springs and settles",
            GrammarId::CrossMedia => "the cross-media bar — one axis, many media",
            GrammarId::Panorama => "a typographic panorama — text as terrain",
            GrammarId::Orbital => "an orbital focal field — the ring holds the set",
            GrammarId::Blades => "the blade stack — occluding planes of choice",
        }
    }
}

/// One Observatory plot scene, as a timeline payload. The rendering lives in
/// [`crate::observatory`] (filled by the plotting-focused pass); the metadata the
/// HUD needs lives here so the cue sheet and chrome stay together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneId {
    /// A live scrolling oscilloscope — a line series whose time window pans.
    Scope,
    /// A log-frequency power spectrum — Log10 x-axis, resonant peaks.
    Spectrum,
    /// A phase-space scatter that accretes its cloud over the shot.
    PhaseSpace,
}

impl SceneId {
    pub const ALL: [SceneId; 3] = [SceneId::Scope, SceneId::Spectrum, SceneId::PhaseSpace];

    pub fn name(self) -> &'static str {
        match self {
            SceneId::Scope => "OBSERVATORY · SCOPE",
            SceneId::Spectrum => "OBSERVATORY · SPECTRUM",
            SceneId::PhaseSpace => "OBSERVATORY · PHASE SPACE",
        }
    }

    pub fn tagline(self) -> &'static str {
        match self {
            SceneId::Scope => "gibson::plot — a signal read straight off the wire",
            SceneId::Spectrum => "gibson::plot — resonances on a log-frequency axis",
            SceneId::PhaseSpace => "gibson::plot — structure precipitating from noise",
        }
    }
}

/// What plays during a cue on the authoritative edit clock. The reel is a
/// `ShowTimeline<Shot>`; every window is a hard cut, gapless from establish to
/// reveal. `Copy` because each variant is a small value type — the title copy for
/// the two brackets lives in the driver's overlay, not in the payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shot {
    /// The opening establishing wide shot — the ring seen from far, under the film
    /// title overlay. Carries no panel.
    Establish,
    /// One of the six experience grammars, presenting the shared library.
    Grammar(GrammarId),
    /// One of the Observatory plot scenes.
    Observatory(SceneId),
    /// The closing rise-out reveal — the ring opening up, under the sign-off
    /// overlay. Carries no panel.
    Reveal,
}

// ----------------------------- the cinematic HUD -----------------------------
//
// Lifted from `experience_lab` so the film frames every grammar the same way it
// is framed in the flagship: a top title card and a bottom now-showing
// lower-third, built only on `Surface::bake_text` (crisp glyphs whose cell
// backgrounds are sampled from the raster beneath — no opaque block punched) plus
// `RgbRaster` scrim bands.

/// Height, in cells, of each HUD band.
pub const HUD_BAND: u16 = 2;

const HUD_DARK: (u8, u8, u8) = (10, 7, 20);
const HUD_SCRIM: (u8, u8, u8) = (48, 23, 72);
const HUD_SPINE: (u8, u8, u8) = (231, 122, 219);
const HUD_STRONG: (u8, u8, u8) = (246, 247, 253);
const HUD_ACCENT: (u8, u8, u8) = (236, 170, 246);
const HUD_DIM: (u8, u8, u8) = (176, 170, 205);

fn lerp_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let m = |x: u8, y: u8| {
        (x as f32 + (y as f32 - x as f32) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// Blit a `HUD_BAND`-tall scrim band (vertical gradient + bright accent spine and
/// edge line) into `layer` at cell-row `cy`. `bright_bottom` faces the brighter
/// edge toward the content (down for the top band, up for the lower-third).
fn blit_scrim(layer: &mut Surface, w: u16, cy: u16, bright_bottom: bool) {
    let ph = (HUD_BAND * 2).max(1);
    let mut band = RgbRaster::new(w, ph);
    let denom = ph.saturating_sub(1).max(1) as f32;
    for y in 0..ph {
        let t = y as f32 / denom;
        let v = if bright_bottom { t } else { 1.0 - t };
        let base = lerp_rgb(HUD_DARK, HUD_SCRIM, 0.1 + 0.9 * v);
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

/// Build the HUD overlay layer: top title card (experience title · active label)
/// and bottom lower-third (now-showing caption), transparent elsewhere so the
/// shot shows through untouched. `label` is the shot's name (grammar/scene),
/// `caption` the now-showing line.
pub fn cinematic_hud(w: u16, h: u16, title: &str, label: &str, caption: &str) -> Surface {
    let mut layer = Surface::new_transparent(w, h);
    if w < 8 || h < 6 {
        return layer; // too small to frame; leave the shot bare
    }
    blit_scrim(&mut layer, w, 0, true);
    layer.bake_text(2, 0, title, HUD_STRONG, true);
    let caps = label.to_uppercase();
    let sx = w.saturating_sub(caps.chars().count() as u16 + 2);
    layer.bake_text(sx, 0, &caps, HUD_ACCENT, true);

    let by = h - HUD_BAND;
    blit_scrim(&mut layer, w, by, false);
    layer.bake_text(2, by, caption, HUD_STRONG, true);
    layer
}
