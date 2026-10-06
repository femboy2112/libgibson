//! `ORBITAL` — a focal, radial grammar (structurally inspired by the dark, energetic
//! character of early-2000s console dashboards; it reproduces none of their art).
//!
//! Where the shelf is a line and the cross-bar is a cross, ORBITAL is a *world with
//! a centre*. The selected item is the one dominant focus locus: it drifts to the
//! middle of the field, swells toward the viewer, and glows under a slowly
//! rotating dashed halo. Its siblings ride a tilted elliptical **orbit** around it,
//! shrinking and dimming as they swing to the back of the ring. Behind it all, a
//! deterministic ambient field — a slow swirl and a scatter of drifting motes —
//! breathes as a pure function of `now`.
//!
//! The whole world is one [`RgbRaster`] built with 2D polar math (centre +
//! radius · (cos θ, sin θ)); no external images, no copyrighted art. Orbs are
//! procedural (a pattern chosen by the item's media seed, sphere-shaded), and
//! cached by seed. Motion is a damped spring on the fractional selection, so the
//! ring *rotates* from one item to the next and reverses cleanly mid-flight.
//!
//! Identity never rests on colour alone: the selected orb holds the centre, is the
//! largest and brightest, and wears the dashed halo (a silhouette that survives
//! `ColorDepth::Mono`'s Braille luminance); the caption names it, counts it
//! (`3/5`) and shows its neighbours in ring order; the destination strip marks the
//! active destination with a glyph (`◉` / `○`), not just emphasis.
//!
//! Every item is *reachable* (Left/Right walk the whole ring), so ORBITAL never
//! omits an item. Items are drawn as raster orbs and therefore **attested** in
//! [`PresentationReceipt::rastered`]; destinations and the selected item's primary
//! action are keyed nodes. The one declared responsive omission is that action's
//! caption, dropped (and recorded) only when the viewport is too short for it.
//! Detail / Prose / Custom destinations degrade to a faithful plain rendering.

use super::grammar::{FrameDemand, Grammar, Presented};
use super::intent::{Intent, PresentationState, SemanticInput};
use super::model::{Content, CustomCx, Experience};
use super::motion::DampedSpring;
use super::paint::{hsv, mix, pill_rail, seed_for};
use super::receipt::PresentationReceipt;
use crate::capability::ColorDepth;
use crate::input::{KeyCode, KeyEvent};
use crate::raster::{Rgb, RgbRaster};
use crate::ui::element::{button, column, label, raster, row, screen, spacer, text, Element, Key};
use crate::ui::skin::UiEnvironment;
use crate::ui::style::{Emphasis, Tone};
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, TAU};
use std::time::Duration;

// ---- the world, in raster pixels -----------------------------------------------------

/// Deep-space colour at the rim of the field.
const BG_DEEP: Rgb = (3, 4, 10);
/// Colour of the field near the focus.
const BG_CORE: Rgb = (15, 19, 44);
/// The orbit track and halo colour.
const TRACK: Rgb = (120, 160, 255);
/// Spring stiffness / damping for the ring's rotation.
const OMEGA: f32 = 11.0;
const ZETA: f32 = 0.74;

fn scale(c: Rgb, k: f32) -> Rgb {
    let s = |x: u8| (x as f32 * k).round().clamp(0.0, 255.0) as u8;
    (s(c.0), s(c.1), s(c.2))
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn frac(x: f32) -> f32 {
    x - x.floor()
}

// ---- procedural orbs -----------------------------------------------------------------

/// The seed-derived look of one orb: two hues, a pattern, a phase wobble.
#[derive(Clone, Copy)]
struct OrbStyle {
    hue: f32,
    hue2: f32,
    motif: u32,
    wobble: f32,
}

/// A deterministic style for a media seed. Golden-angle hues so neighbours never
/// rhyme; four motifs (rings, bands, wedges, dot). Pure function of the seed.
fn orb_style(seed: u64) -> OrbStyle {
    let seed32 = (seed ^ (seed >> 32)) as u32;
    let mut state = seed32
        .wrapping_mul(2_654_435_761)
        .wrapping_add(40_503)
        .max(1);
    let mut rng = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };
    let jitter = (rng() % 24) as f32;
    let hue = (seed32 as f32 * 137.508 + 14.0 + jitter) % 360.0;
    let hue2 = (hue + 150.0 + (rng() % 60) as f32) % 360.0;
    OrbStyle {
        hue,
        hue2,
        motif: seed32 % 4,
        wobble: (rng() % 100) as f32 / 100.0,
    }
}

/// The shaded colour of an orb at disc coordinates `(u, v)` in `[-1, 1]`.
fn orb_color(p: &OrbStyle, u: f32, v: f32) -> Rgb {
    let d = (u * u + v * v).sqrt();
    let mut c = hsv(p.hue + (p.hue2 - p.hue) * 0.4 * (v * 0.5 + 0.5), 0.74, 0.62);
    let ink = hsv(p.hue2, 0.6, 0.98);
    let pale = hsv(p.hue + 20.0, 0.2, 1.0);
    match p.motif {
        0 => {
            if ((d * 5.0 + p.wobble * 2.0).floor() as i32).rem_euclid(2) == 0 {
                c = mix(c, ink, 0.85);
            }
        }
        1 => {
            let band = ((u * 0.7 + v * 0.7) * 4.0 + p.wobble * 3.0).floor() as i32;
            if band.rem_euclid(2) == 0 {
                c = mix(c, ink, 0.85);
            }
        }
        2 => {
            let ang = v.atan2(u) / TAU + 0.5;
            if d < 0.24 {
                c = pale;
            } else if ((ang * 6.0 + p.wobble).floor() as i32).rem_euclid(2) == 0 {
                c = mix(c, ink, 0.85);
            }
        }
        _ => {
            if d < 0.36 {
                c = pale;
            } else if d < 0.5 {
                c = mix(c, ink, 0.9);
            } else if (d - 0.78).abs() < 0.06 {
                c = mix(c, pale, 0.8);
            }
        }
    }
    // Sphere shading: light from the upper left, a pale rim at the silhouette.
    let nz = (1.0 - d * d).max(0.0).sqrt();
    let light = (0.5 + 0.55 * (-0.35 * u - 0.45 * v + 0.8 * nz)).clamp(0.25, 1.15);
    let mut shaded = scale(c, light);
    if d > 0.86 {
        shaded = mix(shaded, pale, ((d - 0.86) / 0.14).clamp(0.0, 1.0) * 0.5);
    }
    shaded
}

/// Pixel bounds of a disc of radius `r` about `(cx, cy)`, clipped to the raster.
fn bounds(rz: &RgbRaster, cx: f32, cy: f32, r: f32) -> (i32, i32, i32, i32) {
    let w = i32::from(rz.width());
    let h = i32::from(rz.height());
    let lo = |c: f32| (c - r - 1.0).floor().max(-1.0e6) as i32;
    let hi = |c: f32| (c + r + 1.0).ceil().min(1.0e6) as i32;
    (
        lo(cx).clamp(0, w - 1),
        hi(cx).clamp(0, w - 1),
        lo(cy).clamp(0, h - 1),
        hi(cy).clamp(0, h - 1),
    )
}

/// Draw one shaded orb, antialiased at its edge, dimmed toward the field by `bright`.
fn draw_orb(rz: &mut RgbRaster, cx: f32, cy: f32, rad: f32, p: &OrbStyle, bright: f32) {
    if rad < 0.5 {
        return;
    }
    let (x0, x1, y0, y1) = bounds(rz, cx, cy, rad);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let u = (x as f32 + 0.5 - cx) / rad;
            let v = (y as f32 + 0.5 - cy) / rad;
            let d = (u * u + v * v).sqrt();
            let cover = ((1.0 - d) * rad + 0.5).clamp(0.0, 1.0);
            if cover <= 0.0 {
                continue;
            }
            let color = mix(BG_CORE, orb_color(p, u, v), bright.clamp(0.0, 1.0));
            rz.blend(x, y, color, cover);
        }
    }
}

/// A soft radial glow (quadratic falloff) behind the focus.
fn draw_glow(rz: &mut RgbRaster, cx: f32, cy: f32, radius: f32, color: Rgb, strength: f32) {
    let (x0, x1, y0, y1) = bounds(rz, cx, cy, radius);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let t = (1.0 - (dx * dx + dy * dy).sqrt() / radius).max(0.0);
            if t > 0.0 {
                rz.blend(x, y, color, t * t * strength);
            }
        }
    }
}

/// A dashed annulus whose dashes slide around it with `phase` — the focus halo.
fn draw_halo(rz: &mut RgbRaster, cx: f32, cy: f32, radius: f32, phase: f32, amount: f32) {
    let half = 0.8;
    let (x0, x1, y0, y1) = bounds(rz, cx, cy, radius + half);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            if (d - radius).abs() > half {
                continue;
            }
            let dash = (dy.atan2(dx) / TAU * 16.0 + phase).floor() as i32;
            if dash.rem_euclid(2) == 0 {
                rz.blend(x, y, (235, 245, 255), amount);
            }
        }
    }
}

/// The ambient field: a radial gradient, a slow swirl tinted by `accent`, and a
/// scatter of drifting motes. A pure function of `t` (seconds) — deterministic.
fn draw_field(rz: &mut RgbRaster, t: f32, accent: Rgb) {
    let (w, h) = (rz.width() as f32, rz.height() as f32);
    let (cx, cy) = (w * 0.5, h * 0.5);
    for y in 0..rz.height() as i32 {
        for x in 0..rz.width() as i32 {
            let dx = (x as f32 + 0.5 - cx) / cx.max(1.0);
            let dy = (y as f32 + 0.5 - cy) / cy.max(1.0);
            let rn = (dx * dx + dy * dy).sqrt();
            let base = mix(BG_CORE, BG_DEEP, rn.min(1.0).powf(0.8));
            let swirl = 0.5 + 0.5 * (rn * 7.0 - t * 0.7 + dy.atan2(dx) * 3.0).sin();
            let k = swirl * (1.0 - rn).max(0.0) * 0.16;
            rz.set(x, y, mix(base, accent, k));
        }
    }
    let motes = ((w * h) / 180.0).clamp(12.0, 64.0) as u32;
    for k in 0..motes {
        let kf = k as f32;
        let (f, g) = (frac(kf * 0.618_034), frac(kf * 0.754_878));
        let dir = if k % 2 == 0 { 1.0 } else { -1.0 };
        let ang = kf * 2.399_963 + t * (0.03 + 0.07 * f) * dir;
        let rad = 0.2 + 0.8 * g;
        let twinkle = 0.5 + 0.5 * (t * (1.0 + f * 2.0) + kf).sin();
        let px = (cx + ang.cos() * rad * cx).floor() as i32;
        let py = (cy + ang.sin() * rad * cy).floor() as i32;
        rz.blend(px, py, (200, 220, 255), 0.25 + 0.5 * twinkle);
    }
}

/// The dotted orbit track (front arc brighter than the back arc).
fn draw_track(rz: &mut RgbRaster, cx: f32, cy: f32, rx: f32, ry: f32) {
    let samples = ((rx * TAU * 0.7) as usize).clamp(24, 400);
    for j in 0..samples {
        let th = j as f32 / samples as f32 * TAU;
        let front = (th.sin() + 1.0) * 0.5;
        rz.blend(
            (cx + rx * th.cos()).round() as i32,
            (cy + ry * th.sin()).round() as i32,
            TRACK,
            0.16 + 0.24 * front,
        );
    }
}

/// The destination moons: a faint outer track with one small moon per destination,
/// the active one larger and bright. Decorative — destinations are *claimed* by
/// the keyed strip, not by these pixels.
fn draw_moons(rz: &mut RgbRaster, count: usize, active: usize) {
    if count < 2 {
        return;
    }
    let (w, h) = (rz.width() as f32, rz.height() as f32);
    let (cx, cy) = (w * 0.5, h * 0.5);
    let (rx, ry) = (w * 0.5 - 2.0, h * 0.5 - 2.0);
    if rx < 6.0 || ry < 4.0 {
        return;
    }
    for j in 0..count {
        let th = j as f32 / count as f32 * TAU + FRAC_PI_2;
        let (mx, my) = (cx + rx * th.cos(), cy + ry * th.sin());
        if j == active {
            rz.disc(mx, my, 2.2, (235, 245, 255));
        } else {
            rz.disc(mx, my, 1.2, mix(TRACK, BG_DEEP, 0.45));
        }
    }
}

// ---- the grammar ---------------------------------------------------------------------

/// The orbital focal grammar. Holds private spring state (the ring's fractional
/// rotation) and an orb-style cache keyed by media seed.
pub struct Orbital {
    spring: DampedSpring,
    last: Option<Duration>,
    initialized: bool,
    styles: HashMap<u64, OrbStyle>,
}

impl Default for Orbital {
    fn default() -> Self {
        Self::new()
    }
}

impl Orbital {
    pub fn new() -> Self {
        Self {
            spring: DampedSpring::default(),
            last: None,
            initialized: false,
            styles: HashMap::new(),
        }
    }

    fn style(&mut self, seed: u64) -> OrbStyle {
        *self.styles.entry(seed).or_insert_with(|| orb_style(seed))
    }

    fn advance(&mut self, target: f32, now: Duration) {
        if !self.initialized {
            self.spring = DampedSpring::at(target);
            self.initialized = true;
            self.last = Some(now);
            return;
        }
        let dt = match self.last {
            Some(prev) => now.saturating_sub(prev).as_secs_f32().min(0.1),
            None => 0.0,
        };
        self.last = Some(now);
        // Sub-step for stability under large dt.
        let steps = (dt / 0.004).ceil().max(1.0) as u32;
        let sub = dt / steps as f32;
        for _ in 0..steps {
            self.spring.step(target, sub, OMEGA, ZETA);
        }
    }

    /// Render the whole orbital world for `seeds` with the ring rotated to the
    /// fractional selection `spin`, at pixel size `pw × ph`, `t` seconds in.
    fn render_world(
        &mut self,
        seeds: &[u64],
        spin: f32,
        destinations: (usize, usize),
        (pw, ph): (u16, u16),
        t: f32,
    ) -> (RgbRaster, Vec<OrbAnchor>) {
        let mut rz = RgbRaster::new(pw, ph);
        let (w, h) = (rz.width() as f32, rz.height() as f32);
        let (cx, cy) = (w * 0.5, h * 0.5);
        let n = seeds.len().max(1);

        // Ambient field, tinted by whichever orb the ring is closest to.
        let near = (spin.round().max(0.0) as usize).min(seeds.len().saturating_sub(1));
        let accent = seeds
            .get(near)
            .map(|&s| hsv(self.style(s).hue2, 0.6, 0.55))
            .unwrap_or((40, 70, 130));
        draw_field(&mut rz, t, accent);
        draw_moons(&mut rz, destinations.0, destinations.1);

        // The orbit: a tilted ellipse around the focus, sized so sibling orbs fit.
        let base_r = (h * 0.25).min(w * 0.16).max(2.0);
        let rx = (w * 0.5 - base_r * 0.5 - 3.0).max(base_r);
        let ry = (h * 0.5 - base_r * 0.5 - 3.0).min(rx * 0.5).max(1.0);
        draw_track(&mut rz, cx, cy, rx, ry);

        // Pose law: each item's place on the ring depends only on d = index − spin;
        // `focus` pulls an orb off the ring into the centre as |d| → 0.
        struct Pose {
            index: usize,
            x: f32,
            y: f32,
            rad: f32,
            bright: f32,
            focus: f32,
            z: f32,
        }
        let mut poses: Vec<Pose> = (0..seeds.len())
            .map(|i| {
                let d = i as f32 - spin;
                let th = d * TAU / n as f32 + FRAC_PI_2;
                let focus = smoothstep(1.0 - d.abs());
                let front = (th.sin() + 1.0) * 0.5;
                let sib_scale = 0.34 + 0.18 * front;
                let sib_bright = 0.42 + 0.30 * front;
                Pose {
                    index: i,
                    x: (cx + rx * th.cos()) * (1.0 - focus) + cx * focus,
                    y: (cy + ry * th.sin()) * (1.0 - focus) + cy * focus,
                    rad: base_r * (sib_scale + (1.0 - sib_scale) * focus),
                    bright: sib_bright + (1.0 - sib_bright) * focus,
                    focus,
                    z: th.sin() * (1.0 - focus) + 2.0 * focus,
                }
            })
            .collect();
        // Far to near; the focused orb is nearest of all.
        poses.sort_by(|a, b| a.z.partial_cmp(&b.z).unwrap_or(std::cmp::Ordering::Equal));
        for p in &poses {
            let style = self.style(seeds[p.index]);
            if p.focus > 0.02 {
                let glow = mix(hsv(style.hue2, 0.55, 0.9), TRACK, 0.3);
                draw_glow(&mut rz, p.x, p.y, p.rad * 2.4, glow, 0.5 * p.focus);
            }
            draw_orb(&mut rz, p.x, p.y, p.rad, &style, p.bright);
            if p.focus > 0.05 {
                let breathe = 0.85 + 0.15 * (t * 1.6).sin();
                draw_halo(
                    &mut rz,
                    p.x,
                    p.y,
                    p.rad * 1.2 * breathe,
                    t * 0.8,
                    0.8 * p.focus,
                );
            }
        }
        let anchors = poses
            .iter()
            .map(|p| OrbAnchor {
                index: p.index,
                x: p.x,
                y: p.y,
                rad: p.rad,
                focus: p.focus,
            })
            .collect();
        (rz, anchors)
    }
}

/// Where an orb landed on screen (pixel coords), so `present` can bake that item's
/// title at the planet — the label travels with the moving part. `focus` runs
/// 0 (a small ring sibling) to 1 (the centred, focused orb).
struct OrbAnchor {
    index: usize,
    x: f32,
    y: f32,
    rad: f32,
    focus: f32,
}

impl<A: Clone> Grammar<A> for Orbital {
    fn name(&self) -> &'static str {
        "ORBITAL"
    }

    fn present(
        &mut self,
        experience: &Experience<A>,
        state: &PresentationState,
        env: &UiEnvironment,
        now: Duration,
    ) -> Presented<A> {
        let active_idx = state.active_index(experience);
        let active_key = experience
            .destinations
            .get(active_idx)
            .map(|d| d.key.clone())
            .unwrap_or_else(|| Key::named("∅"));
        let mut receipt = PresentationReceipt::new("ORBITAL", active_key);

        // The destination rail: baked rounded pills (the ethos chip) rather than a
        // row of opaque text nodes. The active one is bright and marked by `◉` vs
        // `○` (mono-safe). Rendered, so every destination is attested in
        // `receipt.rastered` instead of as a keyed node.
        let rail_entries: Vec<(String, bool)> = experience
            .destinations
            .iter()
            .enumerate()
            .map(|(index, destination)| {
                receipt.destinations.push(destination.key.clone());
                receipt.rastered.push(destination.key.clone());
                let glyph = if index == active_idx { '◉' } else { '○' };
                (
                    format!("{glyph} {}", destination.title),
                    index == active_idx,
                )
            })
            .collect();
        let strip = raster::<A>(pill_rail(env.width.saturating_sub(2), &rail_entries));

        // Rows the layout leaves for the body: screen padding (2), the strip (1) and
        // the gap under it (1), one row for the caption when it fits.
        let show_caption = env.height >= 10;
        let caption_rows: u16 = u16::from(show_caption);
        let world_rows = env
            .height
            .saturating_sub(4 + caption_rows)
            .max(2)
            .min(env.height);

        // ORBITAL is the one continuously animated grammar: on the orbital field the
        // ambient swirl and the focus halo breathe with `now` and never reach a fixed
        // point, so it asks for frames the whole time it is on the ring. The plain
        // (non-Collection) path draws no field and rests.
        let mut demand = FrameDemand::OnChange;
        let body = match experience.destinations.get(active_idx).map(|d| &d.content) {
            Some(Content::Collection(items)) if !items.is_empty() => {
                demand = FrameDemand::fps(60);
                let selected = state
                    .selected_index(experience)
                    .unwrap_or(0)
                    .min(items.len() - 1);

                // Every item is reachable (Left/Right walk the ring) and drawn as an
                // orb — represented and attested rastered, so the receipt is
                // accountable to the render though an orb is pixels, not a node.
                for item in items {
                    receipt.items.push(item.key.clone());
                    receipt.rastered.push(item.key.clone());
                }
                receipt.selected = items.get(selected).map(|item| item.key.clone());

                // Advance the ring's spring toward the integer selection.
                self.advance(selected as f32, now);

                let seeds: Vec<u64> = items
                    .iter()
                    .map(|item| seed_for(&item.key, item.media.as_ref()))
                    .collect();
                let pw = env.width.saturating_sub(2).max(2);
                let ph = world_rows.saturating_mul(2).max(2);
                let spin = self.spring.x;
                let (world, anchors) = self.render_world(
                    &seeds,
                    spin,
                    (experience.destinations.len(), active_idx),
                    (pw, ph),
                    now.as_secs_f32(),
                );
                let mut surface = if env.color_depth == ColorDepth::Mono {
                    world.to_mono_surface()
                } else {
                    world.to_surface()
                };
                // Bake each orb's item title just beneath its planet, so the label
                // rides the moving part (not only the caption). Brightness tracks
                // focus: the centred orb's title is bright, ring siblings dim.
                let rows = surface.height as i32;
                for a in &anchors {
                    if let Some(item) = items.get(a.index) {
                        let col = a.x.round().max(0.0) as u16;
                        let row =
                            ((((a.y + a.rad) / 2.0).round() as i32) + 1).clamp(0, rows - 1) as u16;
                        let b = 0.4 + 0.6 * a.focus;
                        let v = |base: f32| (base * b).round().clamp(0.0, 255.0) as u8;
                        surface.bake_text_centered(
                            col,
                            row,
                            &item.title,
                            (v(235.0), v(236.0), v(248.0)),
                            a.focus > 0.5,
                        );
                    }
                }

                let mut orbit = column::<A>().gap(0).child(raster::<A>(surface).grow(1.0));

                if let Some(item) = items.get(selected) {
                    if show_caption {
                        let mut centre = row::<A>()
                            .gap(2)
                            .child(
                                text::<A>(format!("{}/{}", selected + 1, items.len()))
                                    .emphasis(Emphasis::Faint),
                            )
                            .child(
                                text::<A>(item.title.clone())
                                    .emphasis(Emphasis::Strong)
                                    .tone(Tone::Accent),
                            );
                        if let Some(subtitle) = &item.subtitle {
                            centre = centre.child(label(subtitle.clone()));
                        }
                        // Only the primary action is reachable (Enter), so only it is
                        // surfaced.
                        if let Some(primary) = item.primary() {
                            receipt.actions.push(primary.key.clone());
                            centre = centre.child(
                                button(primary.label.clone())
                                    .key(primary.key.to_string())
                                    .tone(Tone::Accent),
                            );
                        }
                        let mut caption = row::<A>();
                        // Ring neighbours, wide viewports only (decorative reduction).
                        let wide = env.width >= 72;
                        if wide {
                            let prev = selected
                                .checked_sub(1)
                                .and_then(|i| items.get(i))
                                .map(|i| format!("‹ {}", i.title))
                                .unwrap_or_default();
                            caption = caption.child(text::<A>(prev).emphasis(Emphasis::Faint));
                        }
                        caption = caption
                            .child(spacer::<A>().grow(1.0))
                            .child(centre)
                            .child(spacer::<A>().grow(1.0));
                        if wide {
                            let next = items
                                .get(selected + 1)
                                .map(|i| format!("{} ›", i.title))
                                .unwrap_or_default();
                            caption = caption.child(text::<A>(next).emphasis(Emphasis::Faint));
                        }
                        orbit = orbit.child(caption);
                    } else {
                        // No room for the caption: declare its action omitted (lawful —
                        // the items themselves stay reachable via Left/Right).
                        if let Some(primary) = item.primary() {
                            receipt.omitted.push(primary.key.clone());
                        }
                        receipt
                            .degraded
                            .push("action caption omitted (viewport too short)".into());
                    }
                }
                orbit
            }
            Some(content) => present_plain(content, env, now, &mut receipt),
            None => text::<A>("(no destinations)"),
        };

        let root = screen::<A>().child(
            column::<A>()
                .padding(1)
                .gap(1)
                .child(strip)
                .child(body.grow(1.0)),
        );

        Presented::new(root, receipt).with_demand(demand)
    }

    fn interpret(
        &self,
        key: &KeyEvent,
        _experience: &Experience<A>,
        _state: &PresentationState,
    ) -> Option<SemanticInput<A>> {
        // Left/Right turn the ring; an orbit has no end, so the ring *wraps*
        // (cyclic), unlike a shelf or list. Up/Down change destination.
        let intent = match key.code {
            KeyCode::Left => Intent::PreviousCyclic,
            KeyCode::Right => Intent::NextCyclic,
            KeyCode::Up => Intent::PreviousGroup,
            KeyCode::Down => Intent::NextGroup,
            KeyCode::Enter => Intent::Enter,
            KeyCode::Esc | KeyCode::Backspace => Intent::Back,
            KeyCode::Home => Intent::Home,
            KeyCode::End => Intent::End,
            _ => return None,
        };
        Some(SemanticInput::Navigate(intent))
    }
}

/// Faithful plain rendering for destinations an orbit cannot be made of (Detail /
/// Prose / Custom). Records the represented ids into `receipt`. Only a detail's
/// primary action is surfaced — it alone is reachable through `Enter`.
fn present_plain<A: Clone>(
    content: &Content<A>,
    env: &UiEnvironment,
    now: Duration,
    receipt: &mut PresentationReceipt,
) -> Element<A> {
    match content {
        Content::Detail { facets, actions } => {
            let mut fields = column::<A>().gap(0);
            for facet in facets {
                fields = fields.child(
                    row::<A>()
                        .gap(1)
                        .child(label(format!("{}:", facet.label)))
                        .child(text(facet.value.clone())),
                );
            }
            let mut action_row = row::<A>().gap(2);
            if let Some(primary) = actions.first() {
                receipt.actions.push(primary.key.clone());
                action_row = action_row.child(
                    button(primary.label.clone())
                        .key(primary.key.to_string())
                        .tone(Tone::Accent),
                );
            }
            fields.child(action_row)
        }
        Content::Prose(lines) => {
            let mut body = column::<A>().gap(0);
            for line in lines {
                body = body.child(text(line.clone()));
            }
            body
        }
        Content::Custom(custom) => {
            receipt
                .degraded
                .push(format!("custom instrument '{}' composited", custom.label));
            raster::<A>(custom.render(&CustomCx::new(
                env.width.saturating_sub(2).max(1),
                env.height.saturating_sub(5).max(1),
                now,
                env.color_depth,
            )))
        }
        Content::Collection(_) => text::<A>("(empty)"),
    }
}
