//! `CROSS_MEDIA` — a cross-bar grammar (structurally inspired by XMB).
//!
//! The library's thesis applied to a cross: a rendered world (an XMB-blue gradient,
//! a horizontal aurora ribbon and a vertical beam meeting at the focus, the selected
//! item's cover as the focal icon above the crossing) is the GL layer; the crisp
//! category spine and item column, **baked into the surface with raster-matched
//! backgrounds**, are the Qt layer. Two orthogonal axes meet at one focus:
//! destinations spread **horizontally** across the crossing row (the active one
//! centred on the crossing column), and the active destination's items descend
//! **vertically** from it. One object dominates — the selected item — while
//! neighbours fade. No opaque terminal blocks: the UI is drawn, not typed over.
//!
//! Navigation is the cross: Left/Right move along the destination bar, Up/Down along
//! the item column. The world is a pure function of state (time-invariant). Every
//! destination, item and action is attested in [`PresentationReceipt::rastered`].

use super::grammar::{Grammar, Presented};
use super::intent::{Intent, PresentationState, SemanticInput};
use super::model::{Content, Experience};
use super::paint::{bake, bake_centered, cover_art, glow, hsv, mix, scale_blit, seed_for};
use super::receipt::PresentationReceipt;
use crate::capability::ColorDepth;
use crate::input::{KeyCode, KeyEvent};
use crate::raster::{Rgb, RgbRaster};
use crate::ui::element::{raster, screen, Key};
use crate::ui::skin::UiEnvironment;
use std::time::Duration;

// Baked-glyph foreground palette (backgrounds are sampled from the raster).
const FG_FAINT: Rgb = (122, 126, 150);
const FG_BODY: Rgb = (210, 214, 232);
const FG_STRONG: Rgb = (247, 248, 253);

/// The cross-bar grammar. Stateless.
#[derive(Debug, Clone, Copy, Default)]
pub struct CrossMedia;

impl CrossMedia {
    pub fn new() -> Self {
        Self
    }
}

/// A line of baked text: `(x, y, text, fg, bold)`.
type Line = (u16, u16, String, Rgb, bool);

/// Paint the XMB world: a panned gradient, an aurora ribbon and a vertical beam
/// crossing at the focus, the focal cover above the crossing, a selection underline,
/// and a faint screen-door scanline. Pure function of state (no `now`).
#[allow(clippy::too_many_arguments)]
fn paint_world(
    world: &mut RgbRaster,
    active_idx: usize,
    total: usize,
    cover: Option<&RgbRaster>,
    cross_row: i32,
    cross_x: i32,
    sel_row: Option<i32>,
) {
    let w = world.width() as i32;
    let h = world.height() as i32;
    let fh = h.max(1) as f32;
    let span = total.max(1) as f32;
    let base_hue = 205.0 + (active_idx as f32 / span) * 120.0;
    let accent = hsv(base_hue + 18.0, 0.62, 1.0);
    let top = hsv(base_hue, 0.55, 0.16);
    let bot = hsv(base_hue + 12.0, 0.6, 0.04);

    let ribbon_px = (cross_row * 2) as f32 + 0.5;
    for y in 0..h {
        let v = y as f32 / fh;
        let band = mix(top, bot, v.powf(1.2));
        // Horizontal aurora ribbon centred on the crossing row.
        let dr = (y as f32 - ribbon_px).abs();
        let rib = (1.0 - dr / (fh * 0.11)).clamp(0.0, 1.0).powi(2);
        for x in 0..w {
            let mut c = mix(band, accent, rib * 0.33);
            // A soft vertical beam at the crossing column.
            let db = (x - cross_x).abs() as f32;
            let beam = (1.0 - db / (w as f32 * 0.04)).clamp(0.0, 1.0).powi(2);
            c = mix(c, accent, beam * 0.18);
            world.set(x, y, c);
        }
    }

    // The focal cover: the current item's art as the icon above the crossing.
    if let Some(cov) = cover {
        let th = ((fh * 0.22) as i32 * 2).max(4); // pixels tall (even)
        let tw = th; // square
        let dx = cross_x - tw / 2;
        let dy = (cross_row * 2) - 2 - th;
        glow(world, cross_x, dy + th / 2, tw as f32 * 0.85, accent, 0.5);
        scale_blit(world, cov, dx, dy, tw, th);
    }

    // Selection underline beneath the selected item row (accent, in the raster).
    if let Some(sr) = sel_row {
        let yy = sr * 2 + 1;
        for x in (cross_x - w / 4)..(cross_x + w / 4) {
            world.blend(x, yy, accent, 0.6);
        }
    }

    // Faint screen-door scanline.
    for y in (1..h).step_by(2) {
        for x in 0..w {
            if let Some(c) = world.get(x, y) {
                world.set(
                    x,
                    y,
                    (
                        (c.0 as f32 * 0.9) as u8,
                        (c.1 as f32 * 0.9) as u8,
                        (c.2 as f32 * 0.9) as u8,
                    ),
                );
            }
        }
    }
}

impl<A: Clone> Grammar<A> for CrossMedia {
    fn name(&self) -> &'static str {
        "CROSS_MEDIA"
    }

    fn present(
        &mut self,
        experience: &Experience<A>,
        state: &PresentationState,
        env: &UiEnvironment,
        _now: Duration,
    ) -> Presented<A> {
        let active_idx = state.active_index(experience);
        let active = experience.destinations.get(active_idx);
        let active_key = active
            .map(|d| d.key.clone())
            .unwrap_or_else(|| Key::named("∅"));
        let mut receipt = PresentationReceipt::new("CROSS_MEDIA", active_key);
        let total = experience.destinations.len();

        let w = env.width.max(1);
        let cells_h = env.height.max(1) as i32;
        let ph = ((cells_h as u32) * 2).min(4096) as u16;
        let span = total.max(1) as f32;
        let base_hue = 205.0 + (active_idx as f32 / span) * 120.0;
        let accent_text = hsv(base_hue + 18.0, 0.5, 1.0);

        let cross_x = ((w as f32 * 0.30) as i32).max(6);
        let cross_row = (cells_h as f32 * 0.42) as i32;
        let gap = 3u16;

        let mut lines: Vec<Line> = Vec::new();

        // The horizontal destination spine, active centred on the crossing column.
        for destination in &experience.destinations {
            receipt.destinations.push(destination.key.clone());
            receipt.rastered.push(destination.key.clone());
        }
        let active_label = active
            .map(|d| d.title.to_uppercase())
            .unwrap_or_else(|| "∅".to_string());
        let active_w = active_label.chars().count() as u16;
        let active_x0 = (cross_x as u16).saturating_sub(active_w / 2);
        lines.push((active_x0, cross_row as u16, active_label, accent_text, true));
        // Siblings before the active one, walking left.
        let mut x_right = active_x0;
        for destination in experience.destinations[..active_idx].iter().rev() {
            let label = destination.title.to_uppercase();
            let lw = label.chars().count() as u16;
            let x0 = x_right.saturating_sub(gap).saturating_sub(lw);
            lines.push((x0, cross_row as u16, label, FG_FAINT, false));
            x_right = x0;
        }
        // Siblings after the active one, walking right.
        let mut x_left = active_x0 + active_w;
        for destination in experience.destinations.iter().skip(active_idx + 1) {
            let label = destination.title.to_uppercase();
            let x0 = x_left + gap;
            x_left = x0 + label.chars().count() as u16;
            lines.push((x0, cross_row as u16, label, FG_FAINT, false));
        }

        // The vertical item axis descends from the crossing. Collect the focal cover
        // and the selected row (for the raster underline).
        let mut cover: Option<RgbRaster> = None;
        let mut sel_row: Option<i32> = None;
        let item0 = cross_row + 2;
        match active.map(|d| &d.content) {
            Some(Content::Collection(items)) if !items.is_empty() => {
                let sel = state.selected_index(experience).unwrap_or(0);
                cover = items
                    .get(sel)
                    .map(|item| cover_art(seed_for(&item.key, item.media.as_ref())));
                for (index, item) in items.iter().enumerate() {
                    receipt.items.push(item.key.clone());
                    receipt.rastered.push(item.key.clone());
                    let y = item0 + index as i32;
                    let (glyph, fg, bold) = if index == sel {
                        receipt.selected = Some(item.key.clone());
                        sel_row = Some(y);
                        (format!("\u{25b8} {}", item.title), FG_STRONG, true)
                    } else {
                        (item.title.clone(), FG_BODY, false)
                    };
                    lines.push((0, y as u16, glyph, fg, bold)); // x recomputed as centred below
                }
                // The selected item's subtitle + primary action, below the column.
                let mut y = item0 + items.len() as i32 + 1;
                if let Some(item) = items.get(sel) {
                    if let Some(subtitle) = &item.subtitle {
                        lines.push((0, y as u16, subtitle.clone(), FG_FAINT, false));
                        y += 1;
                    }
                    if let Some(primary) = item.primary() {
                        receipt.actions.push(primary.key.clone());
                        receipt.rastered.push(primary.key.clone());
                        lines.push((
                            0,
                            y as u16,
                            format!("[ {} ]", primary.label.to_uppercase()),
                            accent_text,
                            true,
                        ));
                    }
                }
            }
            Some(Content::Detail { facets, actions }) => {
                let mut y = item0;
                for facet in facets {
                    lines.push((
                        0,
                        y as u16,
                        format!("{}  {}", facet.label, facet.value),
                        FG_BODY,
                        false,
                    ));
                    y += 1;
                }
                for action in actions {
                    receipt.actions.push(action.key.clone());
                    receipt.rastered.push(action.key.clone());
                    lines.push((
                        0,
                        y as u16,
                        format!("[ {} ]", action.label.to_uppercase()),
                        accent_text,
                        true,
                    ));
                    y += 1;
                }
            }
            Some(Content::Prose(prose)) => {
                for (index, line) in prose.iter().enumerate() {
                    let y = item0 + index as i32;
                    lines.push((0, y as u16, line.clone(), FG_BODY, false));
                }
            }
            _ => {}
        }

        // GL layer.
        let mut world = RgbRaster::new(w, ph);
        paint_world(
            &mut world,
            active_idx,
            total,
            cover.as_ref(),
            cross_row,
            cross_x,
            sel_row,
        );
        let mut surface = if env.color_depth == ColorDepth::Mono {
            world.to_mono_surface()
        } else {
            world.to_surface()
        };

        // Bake the spine at its tracked x; bake the item column centred on the cross.
        for (x, y, text, fg, bold) in &lines {
            if *y == cross_row as u16 {
                bake(&mut surface, *x, *y, text, *fg, *bold);
            } else {
                bake_centered(&mut surface, cross_x as u16, *y, text, *fg, *bold);
            }
        }

        let root = screen::<A>().child(raster::<A>(surface).grow(1.0));
        Presented {
            element: root,
            receipt,
        }
    }

    fn interpret(
        &self,
        key: &KeyEvent,
        _experience: &Experience<A>,
        _state: &PresentationState,
    ) -> Option<SemanticInput<A>> {
        // The cross: Left/Right along the destination bar, Up/Down along the items.
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
