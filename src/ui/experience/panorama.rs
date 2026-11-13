//! `PANORAMA` — a wide rendered world (Zune/Metro-panorama inspired).
//!
//! The library's visual thesis in one grammar: **(Qt-level UI + OpenGL-level
//! graphics) seen through a screen door.** The whole frame is one rendered
//! [`RgbRaster`] — a panned gradient sky, a luminous horizon, the selected item's
//! focal cover (glow + reflection), and the *menu surface itself* (a translucent
//! slab with an accent spine, a header strip and a selection bar) — and the text is
//! **baked into that surface as crisp glyphs whose backgrounds match the raster
//! behind them**, so a glyph's cell blends into the scene and only the character
//! shows. Nothing paints an opaque terminal block: the UI is drawn, not typed over.
//!
//! The two layers move in **coherent phase**: pan the world (Left/Right) and the
//! hue, horizon glow and focal cover all shift together; the selected item's art
//! *is* the focal raster. Because the semantics are rastered, every destination,
//! item and action the receipt claims is attested in
//! [`PresentationReceipt::rastered`] (the ORBITAL pattern). Below a minimum width
//! the letter-spaced hero collapses to plain capitals (a declared typographic
//! reduction).

use super::grammar::{Grammar, Presented};
use super::intent::{Intent, PresentationState, SemanticInput};
use super::model::{Content, CustomCx, Experience};
use super::paint::{bake, cover_art, glow, hsv, mix, scale_blit, seed_for};
use super::receipt::PresentationReceipt;
use crate::capability::ColorDepth;
use crate::input::{KeyCode, KeyEvent};
use crate::raster::{Rgb, RgbRaster};
use crate::surface::Surface;
use crate::ui::element::{raster, screen, Key};
use crate::ui::skin::UiEnvironment;
use std::time::Duration;

/// Below this width the letter-spaced "large type" collapses to plain capitals —
/// a declared, purely typographic reduction (never an omitted id).
const LARGE_TYPE_MIN_WIDTH: u16 = 100;

/// Left indent of baked text inside the menu slab, in cells.
const INDENT: u16 = 3;

// Baked-glyph foreground palette (backgrounds are sampled from the raster).
const FG_DIM: Rgb = (152, 154, 174);
const FG_FAINT: Rgb = (120, 122, 144);
const FG_BODY: Rgb = (212, 215, 232);
const FG_STRONG: Rgb = (247, 248, 253);

/// The panorama grammar. Stateless: the world is a pure function of the active
/// destination and the selection, so a live switch can never strand the camera.
#[derive(Debug, Clone, Copy, Default)]
pub struct Panorama;

impl Panorama {
    pub fn new() -> Self {
        Self
    }
}

// ---- the GL layer: a rendered world in raster pixels --------------------------------

/// A short, fading mirror of `src` beneath it — the Metro "hero" reflection.
fn blit_reflection(dst: &mut RgbRaster, src: &RgbRaster, dx: i32, dy: i32, dw: i32, dh: i32) {
    if dw <= 0 || dh <= 0 {
        return;
    }
    let sh = src.height() as f32;
    let sw = src.width() as f32;
    for yy in 0..dh {
        let sy = (yy as f32 / dh as f32 * sh * 0.5) as i32;
        let fade = (0.34 * (1.0 - yy as f32 / dh as f32)).max(0.0);
        for xx in 0..dw {
            let sx = (xx as f32 / dw as f32 * sw) as i32;
            if let Some(c) = src.get(sx, sy) {
                dst.blend(dx + xx, dy + yy, c, fade);
            }
        }
    }
}

/// Cell-space geometry of the rasterised menu surface, so the GL layer draws the
/// panel, header strip and selection bar at exactly the cells the baked glyphs land
/// on (one terminal cell spans two raster rows: cell `cy` → pixel rows `2cy,2cy+1`).
struct Menu {
    w: i32,        // panel width in cells (= raster pixels in x)
    top: i32,      // first content cell-row
    list0: i32,    // first list cell-row
    selected: i32, // selected list index, or -1
    hero_row: i32, // cell-row the hero sits on (accent header strip)
    bottom: i32,   // last content cell-row
    accent: Rgb,   // the world's current accent
}

/// Fill the raster cells `[x0,x1) × [cy0,cy1)` by compositing `color` at `alpha`.
fn fill_cells(dst: &mut RgbRaster, x0: i32, x1: i32, cy0: i32, cy1: i32, color: Rgb, alpha: f32) {
    for cy in cy0..cy1 {
        for py in (cy * 2)..(cy * 2 + 2) {
            for x in x0..x1 {
                dst.blend(x, py, color, alpha);
            }
        }
    }
}

/// Draw the rasterised menu surface: a translucent slab (sky still tinting it), a
/// bright left spine and frame (a "building face" edge), an accent header strip, a
/// divider, and the accent selection bar. Baked glyphs blend into this later.
fn draw_menu(world: &mut RgbRaster, m: &Menu) {
    let w = world.width() as i32;
    let px = m.w.min(w);
    let cy0 = (m.top - 1).max(0);
    let cy1 = m.bottom + 2;
    let yp0 = cy0 * 2;
    let yp1 = (cy1 * 2).min(world.height() as i32);
    for y in yp0..yp1 {
        let v = (y - yp0) as f32 / ((yp1 - yp0).max(1)) as f32;
        let base = mix((16, 12, 28), (8, 6, 16), v);
        for x in 0..px {
            world.blend(x, y, base, 0.82);
        }
    }
    fill_cells(world, 1, px - 1, m.hero_row, m.hero_row + 1, m.accent, 0.42);
    fill_cells(world, 2, px - 2, m.list0 - 1, m.list0, m.accent, 0.22);
    if m.selected >= 0 {
        let sy = m.list0 + m.selected;
        fill_cells(world, 1, px - 1, sy, sy + 1, m.accent, 0.68);
    }
    for y in yp0..yp1 {
        world.blend(0, y, m.accent, 0.95);
        world.blend(1, y, m.accent, 0.45);
        world.blend(px - 1, y, (0, 0, 0), 0.40);
    }
    for x in 0..px {
        world.blend(x, yp0, (225, 225, 240), 0.45);
        world.blend(x, (yp1 - 1).max(0), (0, 0, 0), 0.45);
    }
}

/// Paint the whole panorama world: a panned gradient sky with a luminous horizon,
/// the selected item's focal cover (glow + reflection), a faint screen-door
/// scanline, and the rasterised menu surface.
fn paint_world(
    world: &mut RgbRaster,
    active_idx: usize,
    total: usize,
    cover: Option<&RgbRaster>,
    menu: Option<&Menu>,
) {
    let w = world.width() as i32;
    let h = world.height() as i32;
    let fw = w.max(1) as f32;
    let fh = h.max(1) as f32;
    let span = total.max(1) as f32;
    // Pure function of the active section (no `now`): the world is time-invariant, so
    // the runtime can paint once and coalesce every later frame until state changes.
    let base_hue = 255.0 + (active_idx as f32 / span) * 95.0;
    let horizon = 0.66;
    let top = hsv(base_hue, 0.62, 0.13);
    let hor = hsv(base_hue + 35.0, 0.72, 0.46);
    let bot = hsv(base_hue - 25.0, 0.5, 0.05);

    for y in 0..h {
        let v = y as f32 / fh;
        let band = if v < horizon {
            mix(top, hor, (v / horizon).powf(1.7))
        } else {
            mix(hor, bot, ((v - horizon) / (1.0 - horizon)).powf(0.7))
        };
        for x in 0..w {
            let u = x as f32 / fw;
            let d = (((u - 0.70) * fw / fh).powi(2) + (v - horizon).powi(2)).sqrt();
            let g = (1.0 - d / 0.55).clamp(0.0, 1.0).powi(2);
            let c = mix(band, hsv(base_hue + 55.0, 0.45, 1.0), g * 0.55);
            world.set(x, y, c);
        }
    }

    if let Some(cov) = cover {
        let size = ((fh * 0.56) as i32).max(2);
        let cx = (fw * 0.66) as i32 - size / 2;
        let cy = (fh * 0.24) as i32;
        glow(
            world,
            cx + size / 2,
            cy + size / 2,
            size as f32 * 0.85,
            hsv(base_hue + 40.0, 0.5, 1.0),
            0.5,
        );
        scale_blit(world, cov, cx, cy, size, size);
        blit_reflection(world, cov, cx, cy + size + 1, size, size / 3);
    }

    for y in (1..h).step_by(2) {
        for x in 0..w {
            if let Some(c) = world.get(x, y) {
                world.set(
                    x,
                    y,
                    (
                        (c.0 as f32 * 0.90) as u8,
                        (c.1 as f32 * 0.90) as u8,
                        (c.2 as f32 * 0.90) as u8,
                    ),
                );
            }
        }
    }

    if let Some(m) = menu {
        draw_menu(world, m);
    }
}

/// Set a title in "large type": capitals, letter-spaced when there is room.
fn large_type(title: &str, spaced: bool) -> String {
    let upper = title.to_uppercase();
    if !spaced {
        return upper;
    }
    upper
        .split_whitespace()
        .map(|word| {
            word.chars()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("   ")
}

/// A line of baked text: `(x, y, text, fg, bold)`.
type Line = (u16, u16, String, Rgb, bool);

impl<A: Clone> Grammar<A> for Panorama {
    fn name(&self) -> &'static str {
        "PANORAMA"
    }

    fn present(
        &mut self,
        experience: &Experience<A>,
        state: &PresentationState,
        env: &UiEnvironment,
        now: Duration,
    ) -> Presented<A> {
        let active_idx = state.active_index(experience);
        let active = experience.destinations.get(active_idx);
        let active_key = active
            .map(|d| d.key.clone())
            .unwrap_or_else(|| Key::named("∅"));
        let mut receipt = PresentationReceipt::new("PANORAMA", active_key);
        let total = experience.destinations.len();
        let spaced = env.width >= LARGE_TYPE_MIN_WIDTH;
        if !spaced {
            receipt
                .degraded
                .push("large type collapsed to plain capitals".to_string());
        }

        let w = env.width.max(1);
        let cells_h = env.height.max(1) as i32;
        let ph = ((cells_h as u32) * 2).min(4096) as u16;
        let span = total.max(1) as f32;
        let base_hue = 255.0 + (active_idx as f32 / span) * 95.0;
        let accent = hsv(base_hue + 50.0, 0.58, 0.88);
        let accent_text = hsv(base_hue + 52.0, 0.55, 1.0);

        let menu_w = ((w as f32 * 0.52) as i32).clamp(24, 60).min(w as i32);
        let top = 2i32;
        let hero_row = top + 3;
        let list0 = top + 5;

        // Assemble the baked lines, attest every semantic id (it is rastered), and
        // derive the focal cover and the menu geometry — all from the active content.
        let mut lines: Vec<Line> = Vec::new();

        // Kicker: experience title + position.
        lines.push((INDENT, top as u16, experience.title.clone(), FG_DIM, false));
        if total > 0 {
            let pos = format!("{:02} / {:02}", active_idx + 1, total);
            let px = INDENT + experience.title.chars().count() as u16 + 3;
            lines.push((px, top as u16, pos, FG_FAINT, false));
        }

        // Section rail: every destination attested, active bright, others chevroned.
        let mut rx = INDENT;
        for (index, destination) in experience.destinations.iter().enumerate() {
            receipt.destinations.push(destination.key.clone());
            receipt.rastered.push(destination.key.clone());
            let caps = destination.title.to_uppercase();
            let (seg, fg, bold) = if index == active_idx {
                (caps, accent_text, true)
            } else if index < active_idx {
                (format!("\u{2039} {caps}"), FG_FAINT, false)
            } else {
                (format!("{caps} \u{203a}"), FG_FAINT, false)
            };
            let width = seg.chars().count() as u16;
            lines.push((rx, (top + 1) as u16, seg, fg, bold));
            rx = rx.saturating_add(width + 3);
        }

        // Hero (decoration — the active id is attested in the rail).
        lines.push((
            INDENT,
            hero_row as u16,
            large_type(active.map(|d| d.title.as_str()).unwrap_or(""), spaced),
            accent_text,
            true,
        ));

        // Content → list lines + cover + menu geometry + any instrument to composite.
        let mut cover: Option<RgbRaster> = None;
        let mut instrument: Option<(Surface, u16, u16)> = None;
        let menu = match active.map(|d| &d.content) {
            Some(Content::Collection(items)) if !items.is_empty() => {
                let sel = state.selected_index(experience).unwrap_or(0);
                cover = items
                    .get(sel)
                    .map(|item| cover_art(seed_for(&item.key, item.media.as_ref())));
                let n = items.len() as i32;
                for (index, item) in items.iter().enumerate() {
                    receipt.items.push(item.key.clone());
                    receipt.rastered.push(item.key.clone());
                    let (glyph, fg, bold) = if index == sel {
                        receipt.selected = Some(item.key.clone());
                        (format!("\u{25b8} {}", item.title), FG_STRONG, true)
                    } else {
                        (format!("  {}", item.title), FG_BODY, false)
                    };
                    lines.push((INDENT, (list0 + index as i32) as u16, glyph, fg, bold));
                }
                let mut bottom = list0 + n;
                if let Some(item) = items.get(sel) {
                    if let Some(subtitle) = &item.subtitle {
                        bottom += 1;
                        lines.push((
                            INDENT,
                            bottom as u16,
                            format!("  {subtitle}"),
                            FG_DIM,
                            false,
                        ));
                    }
                    if let Some(primary) = item.primary() {
                        receipt.actions.push(primary.key.clone());
                        receipt.rastered.push(primary.key.clone());
                        bottom += 1;
                        lines.push((
                            INDENT,
                            bottom as u16,
                            format!("[ {} ]", primary.label.to_uppercase()),
                            accent_text,
                            true,
                        ));
                    }
                }
                Menu {
                    w: menu_w,
                    top,
                    list0,
                    selected: sel as i32,
                    hero_row,
                    bottom,
                    accent,
                }
            }
            Some(Content::Detail { facets, actions }) => {
                let mut r = list0;
                for facet in facets {
                    lines.push((
                        INDENT,
                        r as u16,
                        facet.label.to_uppercase(),
                        FG_FAINT,
                        false,
                    ));
                    r += 1;
                    lines.push((INDENT, r as u16, facet.value.clone(), FG_BODY, true));
                    r += 1;
                }
                for action in actions {
                    receipt.actions.push(action.key.clone());
                    receipt.rastered.push(action.key.clone());
                    lines.push((
                        INDENT,
                        r as u16,
                        format!("[ {} ]", action.label.to_uppercase()),
                        accent_text,
                        true,
                    ));
                    r += 1;
                }
                Menu {
                    w: menu_w,
                    top,
                    list0,
                    selected: -1,
                    hero_row,
                    bottom: r,
                    accent,
                }
            }
            Some(Content::Prose(prose)) => {
                let mut r = list0;
                for line in prose {
                    lines.push((INDENT, r as u16, line.clone(), FG_BODY, false));
                    r += 1;
                }
                Menu {
                    w: menu_w,
                    top,
                    list0,
                    selected: -1,
                    hero_row,
                    bottom: r,
                    accent,
                }
            }
            Some(Content::Custom(custom)) => {
                receipt
                    .degraded
                    .push(format!("custom instrument '{}' composited", custom.label));
                let iw = (w as i32 - INDENT as i32 - 1).max(1) as u16;
                let ih = (cells_h - list0 - 1).max(1) as u16;
                instrument = Some((
                    custom.render(&CustomCx::new(iw, ih, now, env.color_depth)),
                    INDENT,
                    list0 as u16,
                ));
                Menu {
                    w: menu_w,
                    top,
                    list0,
                    selected: -1,
                    hero_row,
                    bottom: cells_h - 3,
                    accent,
                }
            }
            _ => Menu {
                w: menu_w,
                top,
                list0,
                selected: -1,
                hero_row,
                bottom: list0,
                accent,
            },
        };

        // GL layer: render the world (sky + cover + menu surface).
        let mut world = RgbRaster::new(w, ph);
        paint_world(&mut world, active_idx, total, cover.as_ref(), Some(&menu));
        let mut surface = if env.color_depth == ColorDepth::Mono {
            world.to_mono_surface()
        } else {
            world.to_surface()
        };

        // Composite any custom instrument, then bake the crisp glyphs on top.
        if let Some((instrument, dx, dy)) = instrument {
            surface.blit_transparent_at(&instrument, dx, dy);
        }
        for (x, y, text, fg, bold) in &lines {
            bake(&mut surface, *x, *y, text, *fg, *bold);
        }

        let root = screen::<A>().child(raster::<A>(surface).grow(1.0));
        Presented::new(root, receipt)
    }

    fn interpret(
        &self,
        key: &KeyEvent,
        _experience: &Experience<A>,
        _state: &PresentationState,
    ) -> Option<SemanticInput<A>> {
        // The panorama's axes: Left/Right pan the world, Up/Down walk the column.
        let intent = match key.code {
            KeyCode::Left => Intent::PreviousGroup,
            KeyCode::Right => Intent::NextGroup,
            KeyCode::Up => Intent::Previous,
            KeyCode::Down => Intent::Next,
            KeyCode::Enter => Intent::Enter,
            KeyCode::Esc | KeyCode::Backspace => Intent::Back,
            KeyCode::Home => Intent::Home,
            KeyCode::End => Intent::End,
            _ => return None,
        };
        Some(SemanticInput::Navigate(intent))
    }
}
