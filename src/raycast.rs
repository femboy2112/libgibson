//! Experimental grid raycaster: a first-person view of a 2D tile map, rendered
//! into an [`RgbRaster`](crate::raster::RgbRaster) that a caller can show directly or map onto a 3D surface
//! with [`Rasterizer::textured_quad`](crate::raster3d::Rasterizer::textured_quad).
//!
//! The renderer is the classic DDA wall caster with floor/ceiling casting, taken
//! out of the Deep Signal example so it can be used as a component. It owns no
//! game state: the map, the view and the texel functions are the caller's, so the
//! same inputs always produce the same frame (only basic IEEE arithmetic, with
//! sine and cosine from a fixed polynomial rather than the platform's libm). Work is bounded: at most `2 × width × height` pixel writes
//! (every pixel once for floor/ceiling, at most once more for a wall slice) plus
//! `width × max_steps` grid steps per call, independent of map size.
//!
//! EXPERIMENTAL (see `docs/RELEASE_CONTRACT.md` §2): may change between minor releases.

use crate::raster::{Rgb, RgbRaster};

/// A rectangular tile map. Cell value `0` is open floor; any other value is a wall
/// whose material id is passed to [`Texels::wall`]. Positions outside the map read
/// as walls of material [`GridMap::OUTSIDE`], so a ray can never leave the grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridMap {
    width: u16,
    height: u16,
    cells: Vec<u8>,
}

impl GridMap {
    /// A map of the given size, all open floor.
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: vec![0; width as usize * height as usize],
        }
    }

    /// A map from rows of cells. Returns `None` if the rows are empty, ragged,
    /// or larger than `u16::MAX` in either dimension.
    pub fn from_rows(rows: &[&[u8]]) -> Option<Self> {
        let height = u16::try_from(rows.len()).ok()?;
        let width = u16::try_from(rows.first()?.len()).ok()?;
        if width == 0 || rows.iter().any(|r| r.len() != width as usize) {
            return None;
        }
        Some(Self {
            width,
            height,
            cells: rows.concat(),
        })
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    /// The cell at `(x, y)`. Anything outside the map reads as a wall of material
    /// [`GridMap::OUTSIDE`], so a ray can never escape an unclosed map.
    pub fn get(&self, x: i64, y: i64) -> u8 {
        if x < 0 || y < 0 || x >= i64::from(self.width) || y >= i64::from(self.height) {
            return Self::OUTSIDE;
        }
        self.cells[y as usize * self.width as usize + x as usize]
    }

    /// Sets the cell at `(x, y)`; out-of-range positions are ignored.
    pub fn set(&mut self, x: u16, y: u16, value: u8) {
        if x < self.width && y < self.height {
            self.cells[y as usize * self.width as usize + x as usize] = value;
        }
    }

    /// The material reported for positions outside the map.
    pub const OUTSIDE: u8 = 1;

    /// A deterministic maze for `seed`: odd-sized, closed by an outer wall, every
    /// open cell reachable from `(1, 1)` (randomised depth-first carving with a
    /// fixed 64-bit mixer, no external RNG). Wall cells get materials `1..=3`
    /// chosen from the seed. `width`/`height` are rounded up to odd and clamped
    /// to at least 5 and at most 255.
    pub fn maze(width: u16, height: u16, seed: u64) -> Self {
        let odd = |v: u16| {
            let v = v.clamp(5, 255);
            v | 1
        };
        let (w, h) = (odd(width), odd(height));
        let mut map = Self::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let m = 1 + (mix(seed, u64::from(y) << 16 | u64::from(x)) % 3) as u8;
                map.set(x, y, m);
            }
        }
        let mut rng = seed ^ 0x5DEE_CE66_D1CE_4E5B;
        let mut stack = vec![(1u16, 1u16)];
        map.set(1, 1, 0);
        while let Some(&(cx, cy)) = stack.last() {
            let mut dirs = [(0i32, -2i32), (2, 0), (0, 2), (-2, 0)];
            for i in (1..4).rev() {
                rng = mix(rng, i as u64);
                dirs.swap(i, (rng % (i as u64 + 1)) as usize);
            }
            let next = dirs.iter().find_map(|&(dx, dy)| {
                let nx = i32::from(cx) + dx;
                let ny = i32::from(cy) + dy;
                (nx > 0 && ny > 0 && nx < i32::from(w) - 1 && ny < i32::from(h) - 1)
                    .then_some((nx as u16, ny as u16))
                    .filter(|&(nx, ny)| map.get(i64::from(nx), i64::from(ny)) != 0)
            });
            match next {
                Some((nx, ny)) => {
                    map.set((cx + nx) / 2, (cy + ny) / 2, 0);
                    map.set(nx, ny, 0);
                    stack.push((nx, ny));
                }
                None => {
                    stack.pop();
                }
            }
        }
        map
    }
}

fn mix(a: u64, b: u64) -> u64 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 32)
}

/// Where the viewer stands and looks, in map units (one cell = 1.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayView {
    pub x: f64,
    pub y: f64,
    /// Facing angle in radians; 0 looks along +x, π/2 along +y.
    pub angle: f64,
    /// Half-width of the camera plane at distance 1 (`tan(fov/2)`); 0.66 ≈ 66°.
    pub plane: f64,
    /// Maximum DDA steps per column; walls further than this are not drawn.
    pub max_steps: u32,
}

impl RayView {
    /// Largest accepted `|x|` / `|y|`: far beyond any `u16` map, small enough that
    /// cell indices and DDA steps cannot overflow.
    pub const MAX_COORD: f64 = 1.0e9;
    /// Smallest accepted `plane` (field of view ≈ 0.0001°).
    pub const MIN_PLANE: f64 = 1.0e-6;
    /// Largest accepted `plane` (field of view ≈ 179.99°).
    pub const MAX_PLANE: f64 = 1.0e6;
}

/// The largest `f64` below 1.0.
const LAST_BELOW_ONE: f64 = 1.0 - f64::EPSILON / 2.0;

impl Default for RayView {
    fn default() -> Self {
        Self {
            x: 1.5,
            y: 1.5,
            angle: 0.0,
            plane: 0.66,
            max_steps: 96,
        }
    }
}

/// The caller's surface appearance. `u`/`v` are in `[0, 1)`; `distance` is the
/// perpendicular distance from the camera plane, for the caller's own fog.
pub trait Texels {
    fn wall(&self, material: u8, u: f64, v: f64, distance: f64) -> Rgb;
    /// Floor colour at map position `(x, y)`.
    fn floor(&self, x: f64, y: f64, distance: f64) -> Rgb;
    /// Ceiling colour at map position `(x, y)`.
    fn ceiling(&self, x: f64, y: f64, distance: f64) -> Rgb;
}

/// Counters from one [`render`] call, for work-bound checks and receipts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RayStats {
    pub columns: u32,
    pub pixels_written: u64,
    pub grid_steps: u64,
    /// Columns whose ray found no wall within `max_steps`.
    pub misses: u32,
}

/// Renders `view` of `map` into `out` (any size; a zero-sized raster is a no-op).
/// Every pixel is written by the floor/ceiling pass; wall slices then overwrite
/// their columns. Wall textures run left to right as seen by the viewer (`u` grows
/// to the right on every face) and `u`, `v` stay in `[0, 1)`.
///
/// Refused (nothing rendered, default stats): non-finite view values; a position
/// further than [`RayView::MAX_COORD`] from the origin; a `plane` outside
/// `[RayView::MIN_PLANE, RayView::MAX_PLANE]`.
pub fn render(
    map: &GridMap,
    view: &RayView,
    texels: &impl Texels,
    out: &mut RgbRaster,
) -> RayStats {
    let mut stats = RayStats::default();
    let (w, h) = (out.width() as i32, out.height() as i32);
    if w == 0
        || h == 0
        || ![view.x, view.y, view.angle, view.plane]
            .iter()
            .all(|v| v.is_finite())
        || view.x.abs() > RayView::MAX_COORD
        || view.y.abs() > RayView::MAX_COORD
        || !(RayView::MIN_PLANE..=RayView::MAX_PLANE).contains(&view.plane)
    {
        return stats;
    }
    let (dx, dy) = (cos(view.angle), sin(view.angle));
    let (plx, ply) = (-dy * view.plane, dx * view.plane);
    let focal = f64::from(w) / (2.0 * view.plane);
    let horizon = f64::from(h) / 2.0;

    // Floor and ceiling casting: every pixel of every row once.
    for y in 0..h {
        let p = f64::from(y) + 0.5 - horizon;
        let row_dist = focal * 0.5 / p.abs().max(0.5);
        let (rx0, ry0) = (dx - plx, dy - ply);
        let (rx1, ry1) = (dx + plx, dy + ply);
        for x in 0..w {
            let t = (f64::from(x) + 0.5) / f64::from(w);
            let fx = view.x + row_dist * (rx0 + (rx1 - rx0) * t);
            let fy = view.y + row_dist * (ry0 + (ry1 - ry0) * t);
            let c = if p < 0.0 {
                texels.ceiling(fx, fy, row_dist)
            } else {
                texels.floor(fx, fy, row_dist)
            };
            out.set(x, y, c);
            stats.pixels_written += 1;
        }
    }

    // Walls: one DDA ray per column.
    for x in 0..w {
        stats.columns += 1;
        let cam = 2.0 * (f64::from(x) + 0.5) / f64::from(w) - 1.0;
        let (rdx, rdy) = (dx + plx * cam, dy + ply * cam);
        let (mut mx, mut my) = (view.x.floor() as i64, view.y.floor() as i64);
        let ddx = if rdx == 0.0 {
            f64::INFINITY
        } else {
            (1.0 / rdx).abs()
        };
        let ddy = if rdy == 0.0 {
            f64::INFINITY
        } else {
            (1.0 / rdy).abs()
        };
        let (sx, mut side_x) = if rdx < 0.0 {
            (-1, (view.x - mx as f64) * ddx)
        } else {
            (1, (mx as f64 + 1.0 - view.x) * ddx)
        };
        let (sy, mut side_y) = if rdy < 0.0 {
            (-1, (view.y - my as f64) * ddy)
        } else {
            (1, (my as f64 + 1.0 - view.y) * ddy)
        };
        let mut hit = None;
        for _ in 0..view.max_steps {
            stats.grid_steps += 1;
            let vertical = if side_x < side_y {
                side_x += ddx;
                mx += sx;
                false
            } else {
                side_y += ddy;
                my += sy;
                true
            };
            let cell = map.get(mx, my);
            if cell != 0 {
                hit = Some((cell, vertical));
                break;
            }
        }
        let Some((material, vertical)) = hit else {
            stats.misses += 1;
            continue;
        };
        let dist = if vertical { side_y - ddy } else { side_x - ddx }.max(1e-6);
        let mut u = if vertical {
            view.x + dist * rdx
        } else {
            view.y + dist * rdy
        };
        u -= u.floor();
        // Mirror the faces whose world coordinate runs right-to-left on screen, so the
        // texture reads left to right on every face; keep `u` below 1 after the flip.
        if (!vertical && rdx < 0.0) || (vertical && rdy > 0.0) {
            u = (1.0 - u).min(LAST_BELOW_ONE);
        }
        let line = focal / dist;
        let top = horizon - line / 2.0;
        let y0 = top.max(0.0).floor() as i32;
        let y1 = (horizon + line / 2.0).min(f64::from(h)).ceil() as i32;
        for y in y0..y1 {
            let v = ((f64::from(y) + 0.5 - top) / line).clamp(0.0, LAST_BELOW_ONE);
            out.set(x, y, texels.wall(material, u, v, dist));
            stats.pixels_written += 1;
        }
    }
    stats
}

// `f64::sin`/`cos` are libm calls whose last-bit results are platform-dependent;
// the angle is quantised to 1/65536 of a turn and evaluated by a fixed polynomial
// so frames are bit-reproducible across hosts.
fn sin(a: f64) -> f64 {
    cos(a - core::f64::consts::FRAC_PI_2)
}

fn cos(a: f64) -> f64 {
    let tau = core::f64::consts::TAU;
    let turns = (a / tau).rem_euclid(1.0);
    let q = ((turns * 65536.0).round() as i64).rem_euclid(65536) as f64 / 65536.0;
    // Reduce to [-π, π] and use cos(x) = 1 - 2 sin²(x/2) with a Taylor series in x/2.
    let x = (q * tau + core::f64::consts::PI).rem_euclid(tau) - core::f64::consts::PI;
    let h = x / 2.0;
    let h2 = h * h;
    let s = h
        * (1.0
            - h2 / 6.0
                * (1.0 - h2 / 20.0 * (1.0 - h2 / 42.0 * (1.0 - h2 / 72.0 * (1.0 - h2 / 110.0)))));
    1.0 - 2.0 * s * s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_trig_matches_std_closely() {
        for i in 0..=720 {
            let a = f64::from(i) * core::f64::consts::PI / 180.0 - 2.0 * core::f64::consts::PI;
            assert!((cos(a) - a.cos()).abs() < 2e-4, "cos {a}");
            assert!((sin(a) - a.sin()).abs() < 2e-4, "sin {a}");
        }
    }
}
