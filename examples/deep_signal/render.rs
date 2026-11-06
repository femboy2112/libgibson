//! Deep Signal — presentation. Reads a `World` snapshot and paints pixels; never mutates state.
//! Every visual effect is a function of authoritative world state (phase, tic, last hit), so a
//! transition on screen always corresponds to a transition in the simulation.
//!
//! Floating point here uses basic IEEE operations and `sqrt` only, so frames are reproducible.
use super::world::{cos_q16, sin_q16, Phase, Tile, World, LOST_TICS, ONE, TIC_HZ, TRANSITION_TICS};
use gibson::raster::{Rgb, RgbRaster};

/// Horizontal field of view half-width (camera plane length): ~66°.
const PLANE: f64 = 0.66;
const MAX_STEPS: u32 = 96;

fn hash(a: u64, b: u64) -> u64 {
    let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^ (x >> 32)
}

fn scale(c: Rgb, k: f64) -> Rgb {
    let f = |v: u8| (f64::from(v) * k).clamp(0.0, 255.0) as u8;
    (f(c.0), f(c.1), f(c.2))
}

fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let f =
        |x: u8, y: u8| (f64::from(x) + (f64::from(y) - f64::from(x)) * t).clamp(0.0, 255.0) as u8;
    (f(a.0, b.0), f(a.1, b.1), f(a.2, b.2))
}

/// Rational distance fog: 1 at the eye, falling smoothly (no `exp`).
fn fog(d: f64) -> f64 {
    1.0 / (1.0 + 0.045 * d * d)
}

/// Procedural wall material: `u` across the face, `v` down it, both in [0, 1).
fn wall_texel(material: u8, u: f64, v: f64, tic: u64, exit_open: bool) -> Rgb {
    match material {
        // Outer hull: heavy panels.
        0 => {
            let seam = (u * 4.0).fract() < 0.06 || (v * 3.0).fract() < 0.05;
            if seam {
                (24, 26, 40)
            } else {
                (70, 76, 104)
            }
        }
        // Brick: offset courses with mortar.
        1 => {
            let row = (v * 6.0).floor();
            let off = if row as i64 % 2 == 0 { 0.0 } else { 0.5 };
            let mortar = (v * 6.0).fract() < 0.12 || ((u * 3.0 + off).fract()) < 0.07;
            if mortar {
                (40, 30, 36)
            } else {
                (150, 72, 64)
            }
        }
        // Circuit: traces and pads on teal.
        2 => {
            let trace = ((u * 8.0).fract() < 0.12 && (v * 2.0).fract() < 0.7)
                || ((v * 8.0).fract() < 0.1 && (u * 2.0).fract() > 0.3);
            if trace {
                (120, 240, 210)
            } else {
                (18, 70, 76)
            }
        }
        // The uplink: a pulsing aperture once open, a dead grey slab before.
        255 => {
            if exit_open {
                let p = (tic % 24) as f64 / 24.0;
                let ring = ((v - 0.5) * (v - 0.5) + (u - 0.5) * (u - 0.5)).sqrt();
                let wave = ((ring * 6.0 - p).fract() - 0.5).abs() < 0.18;
                if wave {
                    (250, 240, 140)
                } else {
                    (90, 60, 160)
                }
            } else {
                let grid = (u * 6.0).fract() < 0.08 || (v * 6.0).fract() < 0.08;
                if grid {
                    (60, 60, 60)
                } else {
                    (34, 34, 38)
                }
            }
        }
        // Violet ribbed alloy.
        _ => {
            let rib = (u * 10.0).fract() < 0.3;
            if rib {
                (110, 70, 170)
            } else {
                (64, 40, 110)
            }
        }
    }
}

/// Paints the first-person view of `world` into `r` (any size). `mono` drops chroma for the
/// luminance-only fallbacks; shapes (hatching, diamonds, panel seams) carry the meaning.
pub fn paint_view(world: &World, r: &mut RgbRaster) {
    let (w, h) = (r.width() as i32, r.height() as i32);
    if w == 0 || h == 0 {
        return;
    }
    let tic = world.tic;
    let px = f64::from(world.x) / f64::from(ONE);
    let py = f64::from(world.y) / f64::from(ONE);
    let (dx, dy) = (
        f64::from(cos_q16(world.angle)) / 65536.0,
        f64::from(sin_q16(world.angle)) / 65536.0,
    );
    let (plx, ply) = (-dy * PLANE, dx * PLANE);
    // Square pixels: projected height uses the horizontal focal length.
    let focal = f64::from(w) / (2.0 * PLANE);
    let horizon = f64::from(h) / 2.0;
    let exit_open = world.exit_open();
    let hazard_live = world.hazard_live();
    let mut zbuf = vec![f64::INFINITY; w as usize];

    // Ceiling and floor (floor casting).
    for y in 0..h {
        let p = f64::from(y) + 0.5 - horizon;
        if p.abs() < 0.5 {
            continue;
        }
        let row_dist = focal * 0.5 / p.abs();
        let (rx0, ry0) = (dx - plx, dy - ply);
        let (rx1, ry1) = (dx + plx, dy + ply);
        for x in 0..w {
            let t = (f64::from(x) + 0.5) / f64::from(w);
            let fx = px + row_dist * (rx0 + (rx1 - rx0) * t);
            let fy = py + row_dist * (ry0 + (ry1 - ry0) * t);
            let k = fog(row_dist);
            let c = if p < 0.0 {
                // Ceiling: deep blue gradient with sparse fixed "signal" points.
                let base = mix((6, 8, 22), (18, 22, 52), (-p / horizon).clamp(0.0, 1.0));
                let (cx, cy) = (fx.floor() as i64, fy.floor() as i64);
                let star = hash(cx as u64, cy as u64) % 53 == 0
                    && (fx.fract() - 0.5).abs() < 0.06
                    && (fy.fract() - 0.5).abs() < 0.06;
                if star {
                    (180, 200, 255)
                } else {
                    base
                }
            } else {
                let (tx, ty) = (fx.floor() as i32, fy.floor() as i32);
                let (u, v) = (fx - fx.floor(), fy - fy.floor());
                let edge = u < 0.04 || v < 0.04;
                match world.map.at(tx, ty) {
                    Tile::Hazard => {
                        // Diagonal hatching is the hazard's shape cue; it brightens when live.
                        let stripe = ((u + v) * 4.0).fract() < 0.5;
                        match (stripe, hazard_live) {
                            (true, true) => (250, 120, 30),
                            (true, false) => (110, 70, 30),
                            (false, _) => (30, 18, 10),
                        }
                    }
                    Tile::Exit => (60, 40, 110),
                    _ if edge => (36, 44, 60),
                    _ => (16, 20, 28),
                }
            };
            r.set(x, y, scale(c, k.max(0.08)));
        }
    }

    // Walls (DDA per column).
    for x in 0..w {
        let cam = 2.0 * (f64::from(x) + 0.5) / f64::from(w) - 1.0;
        let (rx, ry) = (dx + plx * cam, dy + ply * cam);
        let (mut mx, mut my) = (px.floor() as i32, py.floor() as i32);
        let ddx = if rx == 0.0 {
            f64::INFINITY
        } else {
            (1.0 / rx).abs()
        };
        let ddy = if ry == 0.0 {
            f64::INFINITY
        } else {
            (1.0 / ry).abs()
        };
        let (sx, mut sdx) = if rx < 0.0 {
            (-1, (px - f64::from(mx)) * ddx)
        } else {
            (1, (f64::from(mx) + 1.0 - px) * ddx)
        };
        let (sy, mut sdy) = if ry < 0.0 {
            (-1, (py - f64::from(my)) * ddy)
        } else {
            (1, (f64::from(my) + 1.0 - py) * ddy)
        };
        let mut side = 0;
        let mut hit = None;
        for _ in 0..MAX_STEPS {
            if sdx < sdy {
                sdx += ddx;
                mx += sx;
                side = 0;
            } else {
                sdy += ddy;
                my += sy;
                side = 1;
            }
            match world.map.at(mx, my) {
                Tile::Wall(m) => {
                    hit = Some(m);
                    break;
                }
                // The uplink is drawn as a surface open or closed; open, it is walkable.
                Tile::Exit => {
                    hit = Some(255);
                    break;
                }
                _ => {}
            }
        }
        let Some(material) = hit else { continue };
        let perp = if side == 0 { sdx - ddx } else { sdy - ddy }.max(1e-4);
        zbuf[x as usize] = perp;
        let wall_u = if side == 0 {
            py + perp * ry
        } else {
            px + perp * rx
        };
        let u = wall_u - wall_u.floor();
        let line = focal / perp;
        let top = horizon - line / 2.0;
        let y0 = top.floor().max(0.0) as i32;
        let y1 = (horizon + line / 2.0).ceil().min(f64::from(h)) as i32;
        let k = fog(perp) * if side == 1 { 0.72 } else { 1.0 };
        for y in y0..y1 {
            let v = ((f64::from(y) + 0.5 - top) / line).clamp(0.0, 0.999);
            let c = wall_texel(material, u, v, tic, exit_open);
            r.set(x, y, scale(c, k.max(0.06)));
        }
    }

    // Shards: depth-tested diamond sprites (shape, not just colour).
    let inv = 1.0 / (plx * dy - dx * ply);
    for &(sx_t, sy_t) in &world.shards {
        let (rel_x, rel_y) = (f64::from(sx_t) + 0.5 - px, f64::from(sy_t) + 0.5 - py);
        let tx = inv * (dy * rel_x - dx * rel_y);
        let depth = inv * (-ply * rel_x + plx * rel_y);
        if depth <= 0.2 {
            continue;
        }
        let screen_x = (f64::from(w) / 2.0) * (1.0 + tx / depth);
        let size = (focal / depth) * 0.42;
        let bob = ((tic % 70) as f64 / 70.0 - 0.5).abs() * 0.25 - 0.06;
        let cy = horizon + (focal / depth) * (0.18 + bob);
        let pulse = 0.75 + 0.25 * (((tic % 20) as f64 / 20.0 - 0.5).abs() * 2.0);
        let x0 = (screen_x - size / 2.0).floor().max(0.0) as i32;
        let x1 = (screen_x + size / 2.0).ceil().min(f64::from(w)) as i32;
        for x in x0..x1 {
            if depth >= zbuf[x as usize] {
                continue;
            }
            let ux = ((f64::from(x) + 0.5 - screen_x) / (size / 2.0)).abs();
            let y0 = (cy - size / 2.0).floor().max(0.0) as i32;
            let y1 = (cy + size / 2.0).ceil().min(f64::from(h)) as i32;
            for y in y0..y1 {
                let vy = ((f64::from(y) + 0.5 - cy) / (size / 2.0)).abs();
                if ux + vy <= 1.0 {
                    let core = ux + vy < 0.45;
                    let c = if core {
                        (255, 255, 255)
                    } else {
                        (80, 230, 255)
                    };
                    r.set(x, y, scale(c, pulse * fog(depth).max(0.3)));
                }
            }
        }
    }

    apply_phase_effects(world, r);
}

/// Effects that depict a state change. Each reads only world state.
fn apply_phase_effects(world: &World, r: &mut RgbRaster) {
    let (w, h) = (r.width() as i32, r.height() as i32);
    let tic = world.tic;
    match world.phase {
        Phase::Descending { since } => {
            // Columns fall dark in a hashed order as the descent progresses.
            let p = ((tic - since) as f64 / TRANSITION_TICS as f64).clamp(0.0, 1.0);
            for x in 0..w {
                let threshold = (hash(x as u64, world.level.into()) % 1000) as f64 / 1000.0;
                if threshold < p {
                    let fall = (((p - threshold) * 3.0).min(1.0) * f64::from(h)) as i32;
                    for y in 0..fall.min(h) {
                        let trail = hash(x as u64, (y as u64) ^ tic) % 9 == 0;
                        r.set(x, y, if trail { (40, 255, 160) } else { (0, 0, 0) });
                    }
                }
            }
        }
        Phase::SignalLost { since } => {
            let fade = 1.0 - ((tic - since) as f64 / LOST_TICS as f64).clamp(0.0, 1.0);
            for y in 0..h {
                for x in 0..w {
                    let n = (hash(((y as u64) << 16) | x as u64, tic) % 256) as u8;
                    r.set(x, y, scale((n, n, n), fade));
                }
            }
        }
        Phase::Clear => {
            for y in 0..h {
                for x in 0..w {
                    let old = r.get(x, y).unwrap_or_default();
                    r.set(x, y, mix(old, (250, 240, 200), 0.35));
                }
            }
        }
        Phase::Playing => {
            // Recent damage: red bloom fading over a third of a second.
            if let Some(hit) = world.last_hit {
                let age = tic.saturating_sub(hit);
                let window = u64::from(TIC_HZ) / 3;
                if age < window {
                    let k = 0.45 * (1.0 - age as f64 / window as f64);
                    for y in 0..h {
                        for x in 0..w {
                            let old = r.get(x, y).unwrap_or_default();
                            r.set(x, y, mix(old, (255, 30, 20), k));
                        }
                    }
                }
            }
        }
    }
}

/// Seed for the surface-level tear effect, or `None` when no hit is recent.
pub fn tear_seed(world: &World) -> Option<u64> {
    let hit = world.last_hit?;
    (world.phase == Phase::Playing && world.tic.saturating_sub(hit) < 4).then_some(hit)
}
