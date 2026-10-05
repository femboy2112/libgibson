//! `BLADES` — a depth-plane grammar (structurally inspired by early console
//! "blades"; no assets reproduced, every plane is procedural).
//!
//! Every destination is a **blade**: a full-width plane in a vertical stack. The
//! current destination is the dominant, foremost plane; its siblings are
//! partially-occluded planes layered *behind* it, peeking out above (earlier
//! destinations) and below (later ones) as thin labelled strips. Planes are
//! painted back-to-front with a per-pixel owner buffer, so occlusion is real: a
//! deeper plane is narrower, darker, shadowed by the plane in front, and its label
//! is printed only where its own pixels survive. Changing destination slides the
//! whole stack — a damped spring on the fractional stack position — rather than
//! swapping a panel.
//!
//! The foremost plane carries the destination's title in large pixel typography
//! and its content: a scrolling item list (Collection), a facet sheet (Detail),
//! wrapped lines (Prose) or the attached instrument composited verbatim (Custom).
//!
//! Law accounting: every destination plane and every item of the active
//! destination is *drawn into the raster*, so each is attested in
//! [`PresentationReceipt::rastered`]. Items are reachable by walking
//! Previous/Next (the list scrolls to keep the selection in view), so — like the
//! shelf — nothing is omitted. The one declared omission is the primary action's
//! keyed button, dropped (and recorded) only when the viewport is too short for
//! the footer row.
//!
//! Colour is never the sole identity channel: stack position, a numbered
//! `▲ 01 NAME` / `▼ 03 NAME` label, a per-blade surface pattern, a bright rim
//! silhouette, and (in Mono) a reverse-video selection row all survive without it.

use super::grammar::{Grammar, Presented};
use super::intent::{Intent, PresentationState, SemanticInput};
use super::model::{Content, Experience, Item};
use super::receipt::PresentationReceipt;
use crate::capability::ColorDepth;
use crate::cell::{Cell, Color, Glyph, Style};
use crate::input::{KeyCode, KeyEvent};
use crate::raster::{Rgb, RgbRaster};
use crate::surface::{Rect, Surface};
use crate::ui::element::{button, column, label, raster, row, screen, text, Key};
use crate::ui::skin::UiEnvironment;
use std::time::Duration;
use unicode_segmentation::UnicodeSegmentation;

// ---- the arrangement ------------------------------------------------------------------

const BG: Rgb = (10, 11, 20);
/// How many peeking strips are shown on each side before deeper blades collapse
/// into the last strip (still stacked, still reachable by walking).
const CAP: f32 = 3.0;
const OMEGA: f32 = 12.0;
const ZETA: f32 = 0.86;
const WHITE: Rgb = (255, 255, 255);
const INK: Rgb = (246, 247, 252);
const INK_DIM: Rgb = (200, 204, 222);
const INK_FAR: Rgb = (176, 180, 200);
const PAD_X: i32 = 2;

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

// ---- damped-spring motion (the stack offset) -------------------------------------------

#[derive(Clone, Copy)]
struct Spring {
    x: f32,
    v: f32,
}

impl Spring {
    fn step(&mut self, target: f32, dt: f32) {
        let accel = -2.0 * ZETA * OMEGA * self.v - OMEGA * OMEGA * (self.x - target);
        self.v += accel * dt;
        self.x += self.v * dt;
    }

    fn settled(&self, target: f32) -> bool {
        (self.x - target).abs() < 1e-3 && self.v.abs() < 1e-3
    }
}

// ---- the plane law ---------------------------------------------------------------------

/// One blade's rectangle (cell rows / columns) at the current fractional stack
/// position. `depth` is the blade's distance from the front, `|index - stack|`.
#[derive(Clone, Copy)]
struct Plane {
    top: i32,
    bot: i32,
    xl: i32,
    xr: i32,
    depth: f32,
}

struct Stack {
    n: usize,
    p: f32,
    sr: f32,
    rows: f32,
    cap: f32,
    ix: f32,
    w: i32,
}

impl Stack {
    fn new(n: usize, p: f32, w: i32, rows: i32) -> Self {
        let sr = if rows >= 36 { 2.0 } else { 1.0 };
        // Never let the strips eat the front plane: keep >= 6 rows for it.
        let cap = ((rows as f32 - 6.0) / (2.0 * sr)).floor().clamp(0.0, CAP);
        let ix = (w as f32 / 24.0).floor().clamp(2.0, 6.0);
        Self {
            n,
            p,
            sr,
            rows: rows as f32,
            cap,
            ix,
            w,
        }
    }

    /// The plane law `Φ(i, p)`. Blade `i` sits `k = i - p` away from the front:
    /// later blades (`k > 0`) peek below, earlier ones (`k < 0`) above. Strip
    /// edges are static in `i`, so a settled stack never shimmers.
    fn plane(&self, i: usize) -> Plane {
        let k = i as f32 - self.p;
        let above = self.p.clamp(0.0, self.cap);
        let below = (self.n as f32 - 1.0 - self.p).clamp(0.0, self.cap);
        let f_top = self.sr * above;
        let f_bot = self.rows - self.sr * below;
        let top = if k <= 0.0 {
            f_top - self.sr * (-k).min(self.cap)
        } else {
            f_top
        };
        let bot = if k >= 0.0 {
            f_bot + self.sr * k.min(self.cap)
        } else {
            f_bot
        };
        let inset = ((self.ix * k.abs().min(self.cap)).round() as i32).min(self.w / 4);
        Plane {
            top: (top.round() as i32).max(0),
            bot: (bot.round() as i32).min(self.rows as i32),
            xl: inset,
            xr: self.w - inset,
            depth: k.abs(),
        }
    }

    fn nearest(&self, active: usize) -> usize {
        let mut near = active.min(self.n.saturating_sub(1));
        for i in 0..self.n {
            if (i as f32 - self.p).abs() + 1e-4 < (near as f32 - self.p).abs() {
                near = i;
            }
        }
        near
    }
}

// ---- the canvas: pixels plus who owns each one -------------------------------------------

const NONE: u16 = u16::MAX;

struct Canvas {
    raster: RgbRaster,
    owner: Vec<u16>,
    w: i32,
    h: i32,
}

impl Canvas {
    fn new(w: i32, rows: i32) -> Self {
        let (w, h) = (w.max(1), (rows * 2).max(2));
        let mut raster = RgbRaster::new(w as u16, h as u16);
        raster.clear(BG);
        Self {
            raster,
            owner: vec![NONE; (w * h) as usize],
            w,
            h,
        }
    }

    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.w && y < self.h).then(|| (y * self.w + x) as usize)
    }

    fn put(&mut self, id: usize, x: i32, y: i32, c: Rgb) {
        if let Some(i) = self.idx(x, y) {
            self.raster.set(x, y, c);
            self.owner[i] = id as u16;
        }
    }

    /// Does blade `id` own both pixel rows of cell `(x, row)`?
    fn owns(&self, id: usize, x: i32, row: i32) -> bool {
        [row * 2, row * 2 + 1].iter().all(|&y| {
            self.idx(x, y)
                .map(|i| self.owner[i] == id as u16)
                .unwrap_or(false)
        })
    }

    fn px(&self, x: i32, y: i32) -> Rgb {
        self.raster.get(x, y).unwrap_or(BG)
    }

    fn owns_px(&self, id: usize, x: i32, y: i32) -> bool {
        self.idx(x, y)
            .map(|i| self.owner[i] == id as u16)
            .unwrap_or(false)
    }
}

/// The per-blade procedural identity: a golden-angle hue plus one of four surface
/// patterns, so neighbours never rhyme in colour *or* in texture.
fn blade_hue(index: usize) -> f32 {
    (index as f32 * 137.508 + 24.0) % 360.0
}

fn pattern(index: usize, x: i32, y: i32) -> f32 {
    let hit = match index % 4 {
        0 => false,
        1 => y % 6 == 0,
        2 => (x + y) % 7 == 0,
        _ => x % 5 == 0 && y % 4 == 0,
    };
    if hit {
        0.075
    } else {
        0.0
    }
}

fn plane_base(index: usize, depth: f32, t: f32, x: i32, y: i32) -> Rgb {
    let shade = (1.0 - 0.17 * depth.min(3.0)).max(0.42);
    // A glossier vertical gradient — brighter near the top, a soft specular band,
    // then falling away toward the base — so the blade reads as a lit surface, not a
    // flat colour block.
    let grad = 0.60 - 0.26 * t;
    let gloss = (1.0 - ((t - 0.16) * 4.2).abs()).clamp(0.0, 1.0) * 0.13;
    let v = (grad + gloss + pattern(index, x, y)) * shade;
    hsv(blade_hue(index), 0.56, v)
}

/// Paint one plane: a soft shadow cast on whatever lies behind it, the gradient +
/// pattern body, a one-pixel highlight under the top edge and a bright rim (the
/// silhouette that survives Mono).
/// The corner radius for a blade, in raster pixels — shared by the painter and the
/// content layout, so text is always inset clear of the rounded corners.
fn corner_rad(plane: &Plane) -> i32 {
    let (w, h) = (plane.xr - plane.xl, (plane.bot - plane.top) * 2);
    (w.min(h) / 5).clamp(3, 7)
}

fn paint_plane(canvas: &mut Canvas, index: usize, plane: &Plane, mono: bool) {
    let (xl, xr) = (plane.xl, plane.xr);
    let (yt, yb) = (plane.top * 2, plane.bot * 2);
    if yb - yt < 2 || xr - xl < 2 {
        return;
    }
    // Rounded corners with a one-pixel anti-aliased band: coverage is 1 in the body,
    // ramps to 0 across the quarter-circle boundary, so the silhouette reads as a
    // smooth rounded rectangle rather than a stair-stepped box.
    let rad = corner_rad(plane);
    let coverage = |x: i32, y: i32| -> f32 {
        if x < xl || x >= xr || y < yt || y >= yb {
            return 0.0;
        }
        let center = match (x < xl + rad, x >= xr - rad, y < yt + rad, y >= yb - rad) {
            (true, _, true, _) => Some((xl + rad, yt + rad)),
            (_, true, true, _) => Some((xr - 1 - rad, yt + rad)),
            (true, _, _, true) => Some((xl + rad, yb - 1 - rad)),
            (_, true, _, true) => Some((xr - 1 - rad, yb - 1 - rad)),
            _ => None,
        };
        match center {
            Some((cx, cy)) => {
                let (dx, dy) = ((x - cx) as f32, (y - cy) as f32);
                (rad as f32 + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0)
            }
            None => 1.0,
        }
    };
    // Soft drop shadow, inset at the rounded top/bottom so it hugs the silhouette.
    for (d, amount) in [(1, 0.46), (2, 0.28), (3, 0.14)] {
        for x in (xl + rad)..(xr - rad) {
            canvas.raster.blend(x, yt - d, (0, 0, 0), amount);
            canvas.raster.blend(x, yb - 1 + d, (0, 0, 0), amount);
        }
    }
    let span = (yb - yt).max(2) as f32;
    for y in yt..yb {
        let t = (y - yt) as f32 / span;
        for x in xl..xr {
            let cov = coverage(x, y);
            if cov <= 0.0 {
                continue;
            }
            let mut base = plane_base(index, plane.depth, t, x, y);
            if mono {
                // Braille density follows luminance: keep the body sparse so the
                // bright rim, the pattern and the typography read cleanly.
                base = mix(BG, base, 0.42);
            }
            // The rim follows the (rounded) silhouette: a straight outer edge or any
            // partially-covered corner pixel.
            let edge = x == xl || x == xr - 1 || y == yt || y == yb - 1 || cov < 1.0;
            let c = if edge {
                mix(base, WHITE, if mono { 0.85 } else { 0.58 })
            } else if y == yt + 1 {
                mix(base, WHITE, 0.16)
            } else {
                base
            };
            if cov >= 1.0 {
                canvas.put(index, x, y, c);
            } else {
                // Anti-aliased corner: blend over whatever lies behind.
                let blended = mix(canvas.px(x, y), c, cov);
                canvas.put(index, x, y, blended);
            }
        }
    }
}

// ---- large typography: a 3x5 pixel face, baked into the plane texture ---------------------

fn glyph3x5(c: char) -> [u8; 5] {
    match c.to_ascii_uppercase() {
        'A' => [2, 5, 7, 5, 5],
        'B' => [6, 5, 6, 5, 6],
        'C' => [3, 4, 4, 4, 3],
        'D' => [6, 5, 5, 5, 6],
        'E' => [7, 4, 6, 4, 7],
        'F' => [7, 4, 6, 4, 4],
        'G' => [3, 4, 5, 5, 3],
        'H' => [5, 5, 7, 5, 5],
        'I' => [7, 2, 2, 2, 7],
        'J' => [1, 1, 1, 5, 2],
        'K' => [5, 5, 6, 5, 5],
        'L' => [4, 4, 4, 4, 7],
        'M' => [5, 7, 7, 5, 5],
        'N' => [6, 5, 5, 5, 5],
        'O' => [2, 5, 5, 5, 2],
        'P' => [6, 5, 6, 4, 4],
        'Q' => [2, 5, 5, 6, 3],
        'R' => [6, 5, 6, 5, 5],
        'S' => [3, 4, 2, 1, 6],
        'T' => [7, 2, 2, 2, 2],
        'U' => [5, 5, 5, 5, 7],
        'V' => [5, 5, 5, 5, 2],
        'W' => [5, 5, 7, 7, 5],
        'X' => [5, 5, 2, 5, 5],
        'Y' => [5, 5, 2, 2, 2],
        'Z' => [7, 1, 2, 4, 7],
        '0' => [7, 5, 5, 5, 7],
        '1' => [2, 6, 2, 2, 7],
        '2' => [6, 1, 2, 4, 7],
        '3' => [6, 1, 2, 1, 6],
        '4' => [5, 5, 7, 1, 1],
        '5' => [7, 4, 6, 1, 6],
        '6' => [3, 4, 6, 5, 2],
        '7' => [7, 1, 2, 2, 2],
        '8' => [2, 5, 2, 5, 2],
        '9' => [2, 5, 3, 1, 6],
        ' ' => [0, 0, 0, 0, 0],
        '-' => [0, 0, 7, 0, 0],
        '.' => [0, 0, 0, 0, 2],
        ':' => [0, 2, 0, 2, 0],
        '!' => [2, 2, 2, 0, 2],
        '/' => [1, 1, 2, 4, 4],
        '\'' => [2, 2, 0, 0, 0],
        _ => [6, 1, 2, 0, 2],
    }
}

/// Cell rows a title of `scale` occupies (a pixel is one column wide, half a row
/// tall, so the 5-pixel face is `ceil(5*scale/2)` rows).
fn title_rows(scale: i32) -> i32 {
    if scale == 0 {
        1
    } else {
        (5 * scale + 1) / 2
    }
}

fn title_width(chars: i32, scale: i32) -> i32 {
    chars * 4 * scale - scale
}

/// Bake `text` into the plane's texture at pixel `(x, y)`: a dark offset shadow,
/// then the bright face. Both belong to blade `id`.
fn bake_title(canvas: &mut Canvas, id: usize, text: &str, x: i32, y: i32, scale: i32) {
    // Full glyph height in pixels, for the face's top-lit vertical sheen.
    let title_h = (5 * scale).max(1) as f32;
    for (pass, (dx, dy, color)) in [(1, 1, (8, 8, 18)), (0, 0, INK)].into_iter().enumerate() {
        for (ci, ch) in text.chars().enumerate() {
            let glyph = glyph3x5(ch);
            for (r, bits) in glyph.iter().enumerate() {
                for c in 0..3 {
                    if bits & (4 >> c) == 0 {
                        continue;
                    }
                    let px = x + (ci as i32 * 4 + c) * scale + dx;
                    let py = y + r as i32 * scale + dy;
                    for sy in 0..scale {
                        for sx in 0..scale {
                            // The shadow only darkens this blade; the face overwrites.
                            if pass == 0 && !canvas.owns_px(id, px + sx, py + sy) {
                                continue;
                            }
                            // The face carries a top-lit gradient so the big letters
                            // read as a lit surface rather than flat white blocks.
                            let paint = if pass == 0 {
                                color
                            } else {
                                let gy = ((r as i32 * scale + sy) as f32 / title_h).clamp(0.0, 1.0);
                                mix(WHITE, (184, 188, 216), gy)
                            };
                            canvas.put(id, px + sx, py + sy, paint);
                        }
                    }
                }
            }
        }
    }
}

// ---- the foremost plane's layout ---------------------------------------------------------

struct Front {
    scale: i32,
    title_row: i32,
    body_row: i32,
    body_rows: i32,
    footer_row: Option<i32>,
    x0: i32,
    x1: i32,
}

/// Choose the largest title that fits the plane while leaving room for `want`
/// body lines (up to five), then the body window and an optional footer row.
fn front_layout(plane: &Plane, title: &str, want: usize) -> Front {
    let (x0, x1) = (plane.xl + PAD_X, plane.xr - PAD_X);
    let (row0, row1) = (plane.top, plane.bot);
    // Inset the title below the rounded top corners (a cell is two pixels) so the big
    // face is never clipped by the corner carve.
    let title_row = row0 + 1 + (corner_rad(plane) + 1) / 2;
    let chars = title.chars().count() as i32;
    let need = want.min(5) as i32;
    let mut chosen = None;
    for scale in [3, 2, 1, 0] {
        // The biggest face is for tall viewports only; it must also fit the width.
        if scale == 3 && row1 - row0 < 30 {
            continue;
        }
        if scale > 0 && title_width(chars, scale) > x1 - x0 {
            continue;
        }
        let tb = title_rows(scale);
        let avail = row1 - 1 - (title_row + tb + 1);
        if scale == 0 || avail >= need {
            chosen = Some((scale, tb, avail));
            break;
        }
    }
    let (scale, tb, avail) = chosen.unwrap_or((0, 1, row1 - 1 - (title_row + 2)));
    let (footer_row, body_rows) = if avail >= 4 {
        (Some(row1 - 2), avail - 1)
    } else {
        (None, avail.max(0))
    };
    Front {
        scale,
        title_row,
        body_row: title_row + tb + 1,
        body_rows,
        footer_row,
        x0,
        x1,
    }
}

/// The scroll window over `len` rows that keeps `sel` in view.
fn window(len: usize, sel: usize, rows: usize) -> (usize, usize) {
    if rows == 0 {
        (0, 0)
    } else if len <= rows {
        (0, len)
    } else {
        (sel.saturating_sub(rows / 2).min(len - rows), rows)
    }
}

fn wrap(line: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in line.split_whitespace() {
        let extra = if cur.is_empty() { 0 } else { 1 };
        if !cur.is_empty() && cur.chars().count() + extra + word.chars().count() > width {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() || out.is_empty() {
        out.push(cur);
    }
    out
}

// ---- cell-layer text, honoring occlusion ---------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sample {
    Both,
    Upper,
    Lower,
}

struct Ink {
    fg: Rgb,
    bold: bool,
    reverse: bool,
}

impl Ink {
    fn new(fg: Rgb) -> Self {
        Self {
            fg,
            bold: false,
            reverse: false,
        }
    }
    fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
    fn reverse(mut self, on: bool) -> Self {
        self.reverse = on;
        self
    }
}

/// Print `text` at cell `(x, row)` as blade `id`'s own text: a glyph is written only
/// where `id` owns the pixels beneath it, so a nearer plane really does hide it.
/// Colour backgrounds are sampled from the plane texture; Mono carries no colour.
#[allow(clippy::too_many_arguments)]
fn put_text(
    surface: &mut Surface,
    canvas: &Canvas,
    id: usize,
    x: i32,
    row: i32,
    text: &str,
    ink: &Ink,
    sample: Sample,
    mono: bool,
    max_x: i32,
) -> i32 {
    let mut cx = x;
    for grapheme in text.graphemes(true) {
        let glyph = Glyph::new(grapheme);
        let w = glyph.display_width as i32;
        if w == 0 {
            continue;
        }
        if cx + w > max_x {
            break;
        }
        if cx >= 0
            && row >= 0
            && cx < surface.width as i32
            && row < surface.height as i32
            && canvas.owns(id, cx, row)
        {
            let mut style = Style::new();
            if ink.bold {
                style = style.bold();
            }
            if ink.reverse {
                style = style.reverse();
            }
            if !mono {
                let up = canvas.px(cx, row * 2);
                let lo = canvas.px(cx, row * 2 + 1);
                let bg = match sample {
                    Sample::Both => mix(up, lo, 0.5),
                    Sample::Upper => up,
                    Sample::Lower => lo,
                };
                style = style.fg(Color::Rgb(ink.fg.0, ink.fg.1, ink.fg.2));
                style = style.bg(Color::Rgb(bg.0, bg.1, bg.2));
            }
            surface.set_cell(cx as u16, row as u16, Cell::new(glyph, style));
        }
        cx += w;
    }
    cx
}

// ---- the grammar ---------------------------------------------------------------------------

/// The depth-plane grammar. Holds the private stack-offset spring.
pub struct Blades {
    stack: Spring,
    last: Option<Duration>,
    initialized: bool,
}

impl Default for Blades {
    fn default() -> Self {
        Self::new()
    }
}

impl Blades {
    pub fn new() -> Self {
        Self {
            stack: Spring { x: 0.0, v: 0.0 },
            last: None,
            initialized: false,
        }
    }

    /// Whether the stack has settled on the given destination index. Used by
    /// temporal pacing to stop rebuilding frames.
    pub fn is_settled(&self, target: usize) -> bool {
        self.stack.settled(target as f32)
    }

    fn advance(&mut self, target: f32, max: f32, now: Duration) {
        if !self.initialized {
            self.stack = Spring { x: target, v: 0.0 };
            self.initialized = true;
            self.last = Some(now);
            return;
        }
        let dt = match self.last {
            Some(prev) => now.saturating_sub(prev).as_secs_f32().min(0.1),
            None => 0.0,
        };
        self.last = Some(now);
        let steps = (dt / 0.004).ceil().max(1.0) as u32;
        let sub = dt / steps as f32;
        for _ in 0..steps {
            self.stack.step(target, sub);
        }
        // The stack never slides past its first or last blade.
        self.stack.x = self.stack.x.clamp(0.0, max);
    }
}

/// What the foremost plane shows, resolved from the semantic state.
struct View<'a, A> {
    experience: &'a Experience<A>,
    active: usize,
    selected: Option<usize>,
}

fn compose<A>(view: &View<'_, A>, p: f32, w: i32, rows: i32, mono: bool) -> Surface {
    let experience = view.experience;
    let n = experience.destinations.len();
    let stack = Stack::new(n, p, w, rows);
    let near = stack.nearest(view.active);
    let planes: Vec<Plane> = (0..n).map(|i| stack.plane(i)).collect();
    let mut canvas = Canvas::new(w, rows);

    let near_dest = &experience.destinations[near];
    let title = near_dest.title.to_uppercase();
    let showing_active = near == view.active;
    let want = match &near_dest.content {
        Content::Collection(items) => items.len().max(1),
        Content::Detail { facets, .. } => facets.len().max(1),
        Content::Prose(lines) => lines.len().max(1),
        Content::Custom(_) => 5,
    };
    let front = front_layout(&planes[near], &title, want);

    // The selection row's highlight window, resolved once so the bar can be baked
    // into the texture before nearer planes and text land on it.
    let list = match (&near_dest.content, showing_active) {
        (Content::Collection(items), true) if !items.is_empty() => {
            let sel = view.selected.unwrap_or(0).min(items.len() - 1);
            let (start, count) = window(items.len(), sel, front.body_rows.max(0) as usize);
            Some((sel, start, count))
        }
        _ => None,
    };

    // Back to front: the deepest plane first, the foremost last (active wins ties).
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        planes[b]
            .depth
            .partial_cmp(&planes[a].depth)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then((a == near).cmp(&(b == near)))
    });
    for &i in &order {
        paint_plane(&mut canvas, i, &planes[i], mono);
        if i == near {
            if front.scale > 0 {
                bake_title(
                    &mut canvas,
                    i,
                    &title,
                    front.x0,
                    front.title_row * 2,
                    front.scale,
                );
            }
            if let (Some((sel, start, _)), false) = (list, mono) {
                let r = front.body_row + (sel - start) as i32;
                for y in r * 2..r * 2 + 2 {
                    for x in (front.x0 - 1)..(front.x1 + 1) {
                        canvas.raster.blend(x, y, WHITE, 0.22);
                    }
                }
            }
        }
    }

    let mut surface = if mono {
        canvas.raster.to_mono_surface()
    } else {
        canvas.raster.to_surface()
    };

    // Strip labels for every blade that is not the foremost.
    // (put_text drops any hidden glyph.)
    for (i, plane) in planes.iter().enumerate() {
        if i == near {
            continue;
        }
        let dest = &experience.destinations[i];
        let above = (i as f32) < p;
        let (arrow, row, sample) = if above {
            ('▲', plane.top, Sample::Lower)
        } else {
            ('▼', plane.bot - stack.sr as i32, Sample::Upper)
        };
        let ink = if plane.depth <= 1.5 {
            Ink::new(INK).bold()
        } else {
            Ink::new(INK_FAR)
        };
        let label_text = format!("{arrow} {:02}  {}", i + 1, dest.title.to_uppercase());
        put_text(
            &mut surface,
            &canvas,
            i,
            plane.xl + PAD_X,
            row,
            &label_text,
            &ink,
            sample,
            mono,
            plane.xr - PAD_X,
        );
    }

    if front.scale == 0 {
        put_text(
            &mut surface,
            &canvas,
            near,
            front.x0,
            front.title_row,
            &title,
            &Ink::new(INK).bold(),
            Sample::Both,
            mono,
            front.x1,
        );
    }

    if showing_active && front.body_rows > 0 {
        paint_body(
            &mut surface,
            &canvas,
            near,
            near_dest_content(view),
            &front,
            list,
            mono,
        );
    }
    surface
}

fn near_dest_content<'a, A>(view: &View<'a, A>) -> &'a Content<A> {
    &view.experience.destinations[view.active].content
}

fn item_row<A>(item: &Item<A>, selected: bool) -> (String, Option<String>) {
    let marker = if selected { "▸ " } else { "  " };
    let title = format!("{marker}{}", item.title);
    let sub = item.subtitle.as_ref().map(|s| format!("  {s}"));
    (title, sub)
}

fn paint_body<A>(
    surface: &mut Surface,
    canvas: &Canvas,
    id: usize,
    content: &Content<A>,
    front: &Front,
    list: Option<(usize, usize, usize)>,
    mono: bool,
) {
    let width = (front.x1 - front.x0).max(1);
    match content {
        Content::Collection(items) if items.is_empty() => {
            put_text(
                surface,
                canvas,
                id,
                front.x0,
                front.body_row,
                "(empty)",
                &Ink::new(INK_DIM),
                Sample::Both,
                mono,
                front.x1,
            );
        }
        Content::Collection(items) => {
            let Some((sel, start, count)) = list else {
                return;
            };
            for (offset, item) in items.iter().skip(start).take(count).enumerate() {
                let index = start + offset;
                let row = front.body_row + offset as i32;
                let selected = index == sel;
                let (title, sub) = item_row(item, selected);
                let ink = Ink::new(INK).reverse(selected && mono);
                let ink = if selected { ink.bold() } else { ink };
                let next = put_text(
                    surface,
                    canvas,
                    id,
                    front.x0,
                    row,
                    &title,
                    &ink,
                    Sample::Both,
                    mono,
                    front.x1 - 2,
                );
                if let Some(sub) = sub {
                    put_text(
                        surface,
                        canvas,
                        id,
                        next,
                        row,
                        &sub,
                        &Ink::new(INK_DIM).reverse(selected && mono),
                        Sample::Both,
                        mono,
                        front.x1 - 2,
                    );
                }
                // Honest scroll cues: more rows exist beyond the window.
                let cue = if offset == 0 && start > 0 {
                    Some("▲")
                } else if offset + 1 == count && start + count < items.len() {
                    Some("▼")
                } else {
                    None
                };
                if let Some(cue) = cue {
                    put_text(
                        surface,
                        canvas,
                        id,
                        front.x1 - 1,
                        row,
                        cue,
                        &Ink::new(INK_DIM),
                        Sample::Both,
                        mono,
                        front.x1,
                    );
                }
            }
            if let (Some(footer), true) = (front.footer_row, items.len() > 1) {
                let counter = format!("◂ {}/{} ▸", sel + 1, items.len());
                let at = front.x1 - counter.chars().count() as i32;
                put_text(
                    surface,
                    canvas,
                    id,
                    at.max(front.x0),
                    footer,
                    &counter,
                    &Ink::new(INK_DIM),
                    Sample::Both,
                    mono,
                    front.x1,
                );
            }
        }
        Content::Detail { facets, .. } => {
            let label_w = facets
                .iter()
                .map(|f| f.label.chars().count())
                .max()
                .unwrap_or(0)
                .min(16) as i32;
            for (offset, facet) in facets.iter().take(front.body_rows as usize).enumerate() {
                let row = front.body_row + offset as i32;
                put_text(
                    surface,
                    canvas,
                    id,
                    front.x0,
                    row,
                    &facet.label,
                    &Ink::new(INK_DIM),
                    Sample::Both,
                    mono,
                    front.x1,
                );
                put_text(
                    surface,
                    canvas,
                    id,
                    front.x0 + label_w + 2,
                    row,
                    &facet.value,
                    &Ink::new(INK).bold(),
                    Sample::Both,
                    mono,
                    front.x1,
                );
            }
        }
        Content::Prose(lines) => {
            let wrapped: Vec<String> = lines
                .iter()
                .flat_map(|line| wrap(line, width as usize))
                .collect();
            for (offset, line) in wrapped.iter().take(front.body_rows as usize).enumerate() {
                put_text(
                    surface,
                    canvas,
                    id,
                    front.x0,
                    front.body_row + offset as i32,
                    line,
                    &Ink::new(INK),
                    Sample::Both,
                    mono,
                    front.x1,
                );
            }
        }
        Content::Custom(custom) => {
            // Composited verbatim onto the foremost plane — never reinterpreted.
            let instrument = custom.render(width as u16, front.body_rows as u16);
            let clip = Rect {
                x: front.x0.max(0) as u16,
                y: front.body_row.max(0) as u16,
                width: width as u16,
                height: front.body_rows as u16,
            };
            surface.blit_transparent_clipped(&instrument, front.x0, front.body_row, clip);
        }
    }
}

impl<A: Clone> Grammar<A> for Blades {
    fn name(&self) -> &'static str {
        "BLADES"
    }

    fn present(
        &mut self,
        experience: &Experience<A>,
        state: &PresentationState,
        env: &UiEnvironment,
        now: Duration,
    ) -> Presented<A> {
        let active_idx = state.active_index(experience);
        let Some(active) = experience.destinations.get(active_idx) else {
            return Presented {
                element: screen::<A>().child(text::<A>("(no destinations)")),
                receipt: PresentationReceipt::new("BLADES", Key::named("∅")),
            };
        };
        let mut receipt = PresentationReceipt::new("BLADES", active.key.clone());

        // Every destination is a blade — a plane drawn into the raster, occluded or
        // not — so every one is represented and attested.
        for destination in &experience.destinations {
            receipt.destinations.push(destination.key.clone());
            receipt.rastered.push(destination.key.clone());
        }

        let selected = state.selected_index(experience);
        let mut primary: Option<(Key, String)> = None;
        match &active.content {
            Content::Collection(items) => {
                // Every item is drawn on the foremost plane (the list scrolls to keep
                // the selection in view), so each is attested; Previous/Next walks all.
                for item in items {
                    receipt.items.push(item.key.clone());
                    receipt.rastered.push(item.key.clone());
                }
                if let Some(item) = selected.and_then(|i| items.get(i)) {
                    receipt.selected = Some(item.key.clone());
                    primary = item
                        .primary()
                        .map(|action| (action.key.clone(), action.label.clone()));
                }
            }
            Content::Detail { actions, .. } => {
                primary = actions
                    .first()
                    .map(|action| (action.key.clone(), action.label.clone()));
            }
            Content::Prose(_) => {}
            Content::Custom(custom) => {
                receipt
                    .degraded
                    .push(format!("custom instrument '{}' composited", custom.label));
            }
        }

        // Footer: the hint line and the one keyed primary action. Dropped (and the
        // action declared omitted) only when the viewport has no row to spare.
        let footer = env.height >= 10;
        let mut footer_row = row::<A>().gap(2);
        if footer {
            if let Some((key, text_label)) = &primary {
                receipt.actions.push(key.clone());
                footer_row = footer_row.child(button::<A>(text_label.clone()).key(key.to_string()));
            }
            footer_row = footer_row.child(label::<A>("↑↓ blade   ←→ item   ↵ select"));
        } else if let Some((key, _)) = &primary {
            receipt.omitted.push(key.clone());
            receipt
                .degraded
                .push("primary action button omitted (viewport too short)".into());
        }

        let n = experience.destinations.len();
        self.advance(active_idx as f32, (n - 1) as f32, now);
        let w = i32::from(env.width.max(8));
        let rows = i32::from(env.height)
            .saturating_sub(i32::from(footer))
            .max(8);
        let mono = env.color_depth == ColorDepth::Mono;
        let view = View {
            experience,
            active: active_idx,
            selected,
        };
        let surface = compose(&view, self.stack.x, w, rows, mono);

        let mut body = column::<A>().gap(0).child(raster::<A>(surface).grow(1.0));
        if footer {
            body = body.child(footer_row);
        }
        Presented {
            element: screen::<A>().child(body),
            receipt,
        }
    }

    fn interpret(
        &self,
        key: &KeyEvent,
        _experience: &Experience<A>,
        _state: &PresentationState,
    ) -> Option<SemanticInput<A>> {
        // The stack is vertical: Up/Down change blade (destination); Left/Right
        // move within the foremost blade's items.
        let intent = match key.code {
            KeyCode::Up => Intent::PreviousGroup,
            KeyCode::Down => Intent::NextGroup,
            KeyCode::Left => Intent::Previous,
            KeyCode::Right => Intent::Next,
            KeyCode::Enter => Intent::Enter,
            KeyCode::Esc | KeyCode::Backspace => Intent::Back,
            KeyCode::Home => Intent::Home,
            KeyCode::End => Intent::End,
            _ => return None,
        };
        Some(SemanticInput::Navigate(intent))
    }
}
