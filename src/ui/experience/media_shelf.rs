//! `MEDIA_SHELF` — a Cover-Flow-inspired grammar for collections.
//!
//! The generic form of the Album Flow flagship: a continuous perspective shelf of
//! textured cards driven by one pose law `Φ(d)`, `d = index − selection`. It knows
//! nothing about "albums" — it realizes any [`Content::Collection`] whose items
//! carry a [`Media`](super::model::Media) identity (and synthesizes one from the
//! item key otherwise). Reflections are *derived* from each cover; motion is a
//! damped spring on the fractional selection; covers are procedural (no external
//! images, no copyrighted art, no image-loader dependency).
//!
//! Non-collection destinations degrade to a faithful plain rendering — the shelf
//! is a collection grammar, and it stays law-faithful everywhere so every
//! destination remains reachable.
//!
//! All items in a carousel are *reachable* (Previous/Next walks the whole set), so
//! the shelf never omits an item. Its one declared responsive omission is the
//! selected item's action caption, dropped (and recorded) only when the viewport
//! is too short to carry it.

use super::grammar::{Grammar, Presented};
use super::intent::{Intent, PresentationState, SemanticInput};
use super::model::{Content, Experience};
use super::receipt::PresentationReceipt;
use crate::capability::ColorDepth;
use crate::geom::Vec3;
use crate::input::{KeyCode, KeyEvent};
use crate::raster::{Rgb, RgbRaster};
use crate::raster3d::{Camera, Rasterizer};
use crate::ui::element::{button, column, label, raster, row, text, Element, Key};
use crate::ui::skin::UiEnvironment;
use crate::ui::style::Emphasis;
use std::collections::HashMap;
use std::time::Duration;

// ---- the arrangement, in world units (the pose law Φ) --------------------------------

const COVER_W: f32 = 2.0;
const COVER_H: f32 = 2.0;
const BASE_Z: f32 = 4.2;
const SPREAD_NEAR: f32 = 2.35;
const SPREAD_FAR: f32 = 0.85;
const DEPTH_NEAR: f32 = 2.3;
const DEPTH_FAR: f32 = 0.45;
const YAW_MAX: f32 = 0.78;
const BG: Rgb = (13, 15, 26);
const ART_SIZE: u16 = 64;
const OMEGA: f32 = 13.0;
const ZETA: f32 = 0.72;

#[derive(Clone, Copy)]
struct Pose {
    center: Vec3,
    yaw: f32,
}

/// The pose law `Φ(d)`, `d = index − selection`. Depends only on `d`, so the
/// arrangement is equivariant under selection shifts.
fn pose(d: f32) -> Pose {
    let side = if d >= 0.0 { 1.0 } else { -1.0 };
    let a = d.abs();
    let near = a.min(1.0);
    let far = (a - 1.0).max(0.0);
    let x = side * (SPREAD_NEAR * near + SPREAD_FAR * far);
    let z = BASE_Z + DEPTH_NEAR * near + DEPTH_FAR * far;
    let yaw = -side * YAW_MAX * near;
    Pose {
        center: Vec3::new(x, 0.0, z),
        yaw,
    }
}

/// Cover corners in TL → TR → BR → BL order (the textured-quad UV order).
fn cover_corners(p: Pose) -> [Vec3; 4] {
    let (hw, hh) = (COVER_W * 0.5, COVER_H * 0.5);
    let (c, s) = (p.yaw.cos(), p.yaw.sin());
    let corner =
        |lx: f32, ly: f32| Vec3::new(p.center.x + lx * c, p.center.y + ly, p.center.z - lx * s);
    [
        corner(-hw, hh),
        corner(hw, hh),
        corner(hw, -hh),
        corner(-hw, -hh),
    ]
}

/// Reflection corners, derived by mirroring the cover across its bottom edge.
fn reflection_corners(p: Pose) -> [Vec3; 4] {
    let bottom = p.center.y - COVER_H * 0.5;
    cover_corners(p).map(|v| Vec3::new(v.x, 2.0 * bottom - v.y, v.z))
}

// ---- procedural cover art (generic over a media seed) --------------------------------

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let m = |x: u8, y: u8| {
        (x as f32 + (y as f32 - x as f32) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

fn hsv(h: f32, s: f32, v: f32) -> Rgb {
    let h = h.rem_euclid(360.0) / 60.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to = |z: f32| ((z + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (to(r), to(g), to(b))
}

/// A deterministic abstract cover for a media seed. Golden-angle hues so neighbours
/// never rhyme; six motifs; a one-texel tonal rim. Pure function of the seed.
fn cover_art(seed: u64) -> RgbRaster {
    let seed32 = (seed ^ (seed >> 32)) as u32;
    let mut r = RgbRaster::new(ART_SIZE, ART_SIZE);
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
    let motif = seed32 % 6;
    let wobble = (rng() % 100) as f32 / 100.0;
    let denom = (ART_SIZE - 1) as f32;
    for y in 0..ART_SIZE {
        for x in 0..ART_SIZE {
            let fx = x as f32 / denom;
            let fy = y as f32 / denom;
            let t = fy * 0.7 + fx * 0.3;
            let mut c = hsv(hue + (hue2 - hue) * 0.35 * t, 0.74, 0.50 + 0.38 * (1.0 - t));
            let (dx, dy) = (fx - 0.5, fy - 0.5);
            let d = (dx * dx + dy * dy).sqrt();
            let ink = hsv(hue2, 0.55, 1.0);
            let pale = hsv(hue + 20.0, 0.18, 0.98);
            match motif {
                0 => {
                    let cy = 0.44 + 0.08 * wobble;
                    let dd = ((fx - 0.5).powi(2) + (fy - cy).powi(2)).sqrt();
                    if dd < 0.25 {
                        c = ink;
                    } else if dd < 0.31 {
                        c = mix(c, ink, 0.35);
                    }
                }
                1 => {
                    let horizon = 0.58;
                    if fy < horizon && ((fx - 0.5).powi(2) + (fy - horizon).powi(2)).sqrt() < 0.30 {
                        c = ink;
                    } else if fy >= horizon {
                        let band = ((fy - horizon) * 17.0) as i32 % 2 == 0;
                        c = mix(c, hsv(hue2, 0.7, 0.35), if band { 0.65 } else { 0.25 });
                    }
                }
                2 => {
                    let k = (fx - fy).abs();
                    if k < 0.13 {
                        c = ink;
                    } else if (k - 0.24).abs() < 0.025 {
                        c = pale;
                    }
                }
                3 => {
                    let ring = (d * 7.0) as i32;
                    if d < 0.43 && ring % 2 == 0 {
                        c = mix(c, ink, if ring == 0 { 1.0 } else { 0.8 });
                    }
                }
                4 => {
                    let apex = 0.22 + 0.10 * wobble;
                    let half = (fy - apex) * 0.62;
                    if fy > apex && fy < 0.82 && dx.abs() < half {
                        c = if fy < apex + 0.12 { pale } else { ink };
                    }
                }
                _ => {
                    let col = (fx * 7.0) as usize;
                    let within = (fx * 7.0).fract();
                    let h = 0.25 + 0.55 * (((col as f32 * 1.7 + wobble * 6.0).sin() + 1.0) * 0.5);
                    if within > 0.18 && within < 0.88 && fy > 0.88 - h && fy < 0.88 {
                        c = if fy < 0.88 - h + 0.05 { pale } else { ink };
                    }
                }
            }
            let edge = x == 0 || y == 0 || x == ART_SIZE - 1 || y == ART_SIZE - 1;
            if edge {
                c = mix(c, (255, 255, 255), 0.28);
            }
            r.set(x as i32, y as i32, c);
        }
    }
    r
}

/// The reflection texture: the cover dimmed and faded toward its far (lower) end.
fn reflection_texture(cover: &RgbRaster) -> RgbRaster {
    let (w, h) = (cover.width(), cover.height());
    let mut r = RgbRaster::new(w, h);
    let denom = (h.max(2) - 1) as f32;
    for y in 0..h {
        let v = y as f32 / denom;
        let f = 0.12 + 0.46 * v;
        for x in 0..w {
            let c = cover.get(x as i32, y as i32).unwrap_or_default();
            r.set(
                x as i32,
                y as i32,
                (
                    (c.0 as f32 * f) as u8,
                    (c.1 as f32 * f) as u8,
                    (c.2 as f32 * f) as u8,
                ),
            );
        }
    }
    r
}

fn camera() -> Camera {
    Camera {
        position: Vec3::new(0.0, 0.35, 0.0),
        target: Vec3::new(0.0, -0.1, BASE_Z),
        up: Vec3::new(0.0, 1.0, 0.0),
        fov_y: std::f32::consts::FRAC_PI_3,
        near: 0.1,
        far: 100.0,
    }
}

// ---- damped-spring motion ------------------------------------------------------------

#[derive(Clone, Copy)]
struct Spring {
    x: f32,
    v: f32,
}

impl Spring {
    fn step(&mut self, target: f32, dt: f32, omega: f32, zeta: f32) {
        let accel = -2.0 * zeta * omega * self.v - omega * omega * (self.x - target);
        self.v += accel * dt;
        self.x += self.v * dt;
    }

    /// Settled when near the target with negligible velocity.
    fn settled(&self, target: f32) -> bool {
        (self.x - target).abs() < 1e-3 && self.v.abs() < 1e-3
    }
}

// ---- the grammar ---------------------------------------------------------------------

/// The Cover-Flow collection grammar. Holds private spring/camera state and a cover
/// cache keyed by media seed.
pub struct MediaShelf {
    spring: Spring,
    last: Option<Duration>,
    initialized: bool,
    covers: HashMap<u64, RgbRaster>,
    reflections: HashMap<u64, RgbRaster>,
}

impl Default for MediaShelf {
    fn default() -> Self {
        Self::new()
    }
}

impl MediaShelf {
    pub fn new() -> Self {
        Self {
            spring: Spring { x: 0.0, v: 0.0 },
            last: None,
            initialized: false,
            covers: HashMap::new(),
            reflections: HashMap::new(),
        }
    }

    /// Whether the spring has settled at the given integer target selection. Used by
    /// temporal pacing to stop rebuilding frames.
    pub fn is_settled(&self, target: usize) -> bool {
        self.spring.settled(target as f32)
    }

    fn seed_for(key: &Key, media_seed: Option<u64>) -> u64 {
        media_seed.unwrap_or_else(|| {
            // Deterministic FNV-1a of the key's display string when no media given.
            let mut hash: u64 = 0xcbf29ce484222325;
            for byte in key.to_string().bytes() {
                hash ^= byte as u64;
                hash = hash.wrapping_mul(0x100000001b3);
            }
            hash
        })
    }

    fn ensure_cover(&mut self, seed: u64) {
        if let std::collections::hash_map::Entry::Vacant(entry) = self.covers.entry(seed) {
            let cover = cover_art(seed);
            self.reflections.insert(seed, reflection_texture(&cover));
            entry.insert(cover);
        }
    }

    fn advance(&mut self, target: f32, now: Duration) {
        if !self.initialized {
            self.spring.x = target;
            self.spring.v = 0.0;
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
}

impl<A: Clone> Grammar<A> for MediaShelf {
    fn name(&self) -> &'static str {
        "MEDIA_SHELF"
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
        let mut receipt = PresentationReceipt::new("MEDIA_SHELF", active_key);
        for destination in &experience.destinations {
            receipt.destinations.push(destination.key.clone());
        }

        // A subtle destination strip: the active title, dim siblings as position.
        // Each tab is keyed so the receipt's destination claims are accountable to
        // the rendered tree.
        let mut strip = row::<A>().gap(2);
        for (index, destination) in experience.destinations.iter().enumerate() {
            let mut tab = text::<A>(destination.title.clone()).key(destination.key.to_string());
            if index == active_idx {
                // Bold (mono-safe) marks the active destination; no filled `selected`
                // bg, so the strip blends into the backdrop rather than boxing a word.
                tab = tab.emphasis(Emphasis::Strong);
            } else {
                tab = tab.emphasis(Emphasis::Faint);
            }
            strip = strip.child(tab);
        }

        let body = match experience.destinations.get(active_idx).map(|d| &d.content) {
            Some(Content::Collection(items)) if !items.is_empty() => {
                let selected = state.selected_index(experience).unwrap_or(0);

                // Cache covers + reflections for every item; collect by index.
                let seeds: Vec<u64> = items
                    .iter()
                    .map(|item| Self::seed_for(&item.key, item.media.map(|m| m.seed)))
                    .collect();
                for &seed in &seeds {
                    self.ensure_cover(seed);
                }
                // Every item is reachable (Previous/Next walks the shelf), and each
                // is drawn as a textured cover — represented and attested rastered,
                // so the receipt is accountable to the render even though a cover is
                // pixels, not a keyed node.
                for item in items {
                    receipt.items.push(item.key.clone());
                    receipt.rastered.push(item.key.clone());
                }
                receipt.selected = items.get(selected).map(|item| item.key.clone());

                // Advance the spring toward the integer selection.
                self.advance(selected as f32, now);

                // Reserve rows for the strip (1) and the caption (1 when it fits).
                let show_caption = env.height >= 10;
                let caption_rows: u16 = if show_caption { 1 } else { 0 };
                let shelf_rows = env.height.saturating_sub(1 + caption_rows).max(2);

                let surface = {
                    let pw = env.width.max(2);
                    let ph = shelf_rows.saturating_mul(2).max(2);
                    let mut rz = Rasterizer::new(pw, ph);
                    rz.clear(BG);
                    let cam = camera();
                    let poses: Vec<Pose> = (0..items.len())
                        .map(|i| pose(i as f32 - self.spring.x))
                        .collect();
                    let mut order: Vec<usize> = (0..poses.len()).collect();
                    order.sort_by(|&a, &b| {
                        poses[b]
                            .center
                            .z
                            .partial_cmp(&poses[a].center.z)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    for i in order {
                        let p = poses[i];
                        let seed = seeds[i];
                        if let (Some(cover), Some(reflection)) =
                            (self.covers.get(&seed), self.reflections.get(&seed))
                        {
                            rz.textured_quad(reflection_corners(p), reflection, &cam);
                            rz.textured_quad(cover_corners(p), cover, &cam);
                        }
                    }
                    if env.color_depth == ColorDepth::Mono {
                        rz.raster.to_mono_surface()
                    } else {
                        rz.raster.to_surface()
                    }
                };

                let mut shelf = column::<A>().gap(0).child(raster::<A>(surface).grow(1.0));

                // The selected item's action caption — the one declared omission.
                if let Some(item) = items.get(selected) {
                    if show_caption {
                        let mut caption = row::<A>()
                            .gap(2)
                            .child(text::<A>(item.title.clone()).emphasis(Emphasis::Strong));
                        if let Some(subtitle) = &item.subtitle {
                            caption = caption.child(label(subtitle.clone()));
                        }
                        for action in &item.actions {
                            receipt.actions.push(action.key.clone());
                            caption = caption
                                .child(button(action.label.clone()).key(action.key.to_string()));
                        }
                        shelf = shelf.child(caption);
                    } else {
                        // No room for the caption: declare its actions omitted (lawful —
                        // the items themselves stay reachable via Previous/Next).
                        for action in &item.actions {
                            receipt.omitted.push(action.key.clone());
                        }
                        receipt
                            .degraded
                            .push("action caption omitted (viewport too short)".into());
                    }
                }
                shelf
            }
            Some(content) => present_noncarousel(content, &mut receipt),
            None => text::<A>("(no destinations)"),
        };

        let root = crate::ui::element::screen::<A>().child(
            column::<A>()
                .padding(1)
                .gap(1)
                .child(strip)
                .child(body.grow(1.0)),
        );

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
        // A shelf is a horizontal carousel: Left/Right move items, Up/Down change
        // destination — the opposite axis binding from STANDARD, same intents.
        let intent = match key.code {
            KeyCode::Left => Intent::Previous,
            KeyCode::Right => Intent::Next,
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

/// Faithful plain rendering for the destinations a shelf cannot make a carousel of
/// (Detail / Prose / Custom). Records the represented ids into `receipt`.
fn present_noncarousel<A: Clone>(
    content: &Content<A>,
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
            for action in actions {
                receipt.actions.push(action.key.clone());
                action_row =
                    action_row.child(button(action.label.clone()).key(action.key.to_string()));
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
            raster::<A>(custom.render(40, 10))
        }
        Content::Collection(_) => text::<A>("(empty)"),
    }
}
