//! The cinematic showcase's **shot vocabulary** and shared chrome.
//!
//! Three things live here, all demo-local art direction (not a general framework,
//! per `docs/FRANK_REACTION_CUT_PLAN.md:84`):
//!
//! 1. The [`Shot`] enum the [`crate::reel`] cue sheet is built from — a title
//!    card, one of the six experience [`GrammarId`]s, or one of the Observatory
//!    [`SceneId`]s. These are the stable payload types every cue carries.
//! 2. The cinematic HUD (title card + now-showing lower-third), lifted from the
//!    `experience_lab` flagship so the film frames it the same way — built only on
//!    the public `Surface::bake_text` / `RgbRaster` ethos primitives.
//! 3. The two pure Surface effects the frame driver needs: [`title_card`] (a full
//!    interstitial) and [`dim_toward_black`] (the dip that sells a cut without
//!    needing to blend two glyph grids — driven by a cue's crossfade weight).

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

/// A single interstitial title card: two lines of baked text centred on a scrim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TitleCard {
    pub line1: String,
    pub line2: String,
}

impl TitleCard {
    pub fn new(line1: impl Into<String>, line2: impl Into<String>) -> Self {
        Self {
            line1: line1.into(),
            line2: line2.into(),
        }
    }
}

/// What plays during a cue. The reel is a `ShowTimeline<Shot>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shot {
    /// A full-screen interstitial (the film's own title, act breaks).
    Title(TitleCard),
    /// One of the six experience grammars, presenting the shared library.
    Grammar(GrammarId),
    /// One of the Observatory plot scenes.
    Observatory(SceneId),
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

/// Render a full-screen interstitial title card to its own Surface: a scrim fill
/// with two centred baked lines. No grammar underneath — the card *is* the shot.
pub fn title_card(card: &TitleCard, w: u16, h: u16) -> Surface {
    // A full-height dark scrim with a faint vertical gradient toward the centre.
    let ph = h.saturating_mul(2).max(1);
    let mut raster = RgbRaster::new(w, ph);
    let denom = ph.saturating_sub(1).max(1) as f32;
    for y in 0..ph {
        // Brightest at the vertical centre, falling off toward the edges.
        let d = (y as f32 / denom - 0.5).abs() * 2.0; // 0 centre .. 1 edge
        let base = lerp_rgb(HUD_SCRIM, HUD_DARK, d);
        for x in 0..w {
            raster.set(x as i32, y as i32, base);
        }
    }
    let mut surface = raster.to_surface();
    if w >= 8 && h >= 4 {
        let cx = w / 2;
        let r1 = h / 2 - 1;
        surface.bake_text_centered(cx, r1, &card.line1, HUD_STRONG, true);
        surface.bake_text_centered(cx, r1 + 1, &card.line2, HUD_ACCENT, false);
    }
    surface
}

/// Dim every cell of `surface` toward black by `factor` in `[0, 1]` (1.0 leaves
/// it untouched, 0.0 is black). This is the dip that sells a cut: the frame
/// driver feeds it a cue's crossfade weight, so the screen dips to a partial
/// black at the handover between two shots and rises back up. It blends glyph
/// grids the only honest way at cell resolution — by luminance, not by trying to
/// cross-dissolve two different glyphs in one cell.
pub fn dim_toward_black(surface: &mut Surface, factor: f32) {
    use gibson::cell::Color;
    let f = factor.clamp(0.0, 1.0);
    if f >= 1.0 {
        return;
    }
    let scale = |c: Color| -> Color {
        match c {
            Color::Rgb(r, g, b) => Color::Rgb(
                (r as f32 * f) as u8,
                (g as f32 * f) as u8,
                (b as f32 * f) as u8,
            ),
            other => other, // palette colors left as-is; the film runs truecolor
        }
    };
    for y in 0..surface.height {
        for x in 0..surface.width {
            if let Some(cell) = surface.get(x, y) {
                let mut cell = cell.clone();
                if let Some(fg) = cell.style.fg {
                    cell.style.fg = Some(scale(fg));
                }
                if let Some(bg) = cell.style.bg {
                    cell.style.bg = Some(scale(bg));
                }
                surface.set_cell(x, y, cell);
            }
        }
    }
}
