//! Background scenes and a foreground-tracking HUD reticle, evaluated per subpixel.
//! Pure functions of `(lx, ly, t)` — no `gibson`, no state — so a background is
//! just another subpixel closure the compositor blends under the keyed subject, and
//! the reticle is an overlay drawn on the composited value. This is the "designed
//! visual system" the subject lives inside, not a lazy paste over black.

#![allow(dead_code)]

/// The stylistic substrate the keyed subject is composited over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    /// Near-black cybernetic instrument field: cyan telemetry grid, scan sweep.
    BlackIce,
    /// Vaporwave sunset: gradient sky, sliced sun, neon perspective grid.
    Vapor,
    /// Flat black — the honest baseline for "don't just slap it over black".
    Void,
}

impl Scene {
    pub fn parse(s: &str) -> Option<Scene> {
        match s.to_ascii_lowercase().as_str() {
            "blackice" | "black-ice" | "ice" => Some(Scene::BlackIce),
            "vapor" | "vaporwave" | "vapor95" => Some(Scene::Vapor),
            "void" | "black" => Some(Scene::Void),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Scene::BlackIce => "BLACK_ICE",
            Scene::Vapor => "VAPOR",
            Scene::Void => "VOID",
        }
    }
    /// The reticle / HUD accent colour for this scene.
    pub fn accent(self) -> [u8; 3] {
        match self {
            Scene::BlackIce => [80, 240, 255],
            Scene::Vapor => [255, 70, 190],
            Scene::Void => [90, 220, 120],
        }
    }
}

#[inline]
fn lerp3(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
    ]
}

#[inline]
fn add_sat(a: [u8; 3], b: [u8; 3]) -> [u8; 3] {
    [
        a[0].saturating_add(b[0]),
        a[1].saturating_add(b[1]),
        a[2].saturating_add(b[2]),
    ]
}

#[inline]
fn hash2(x: u32, y: u32) -> f32 {
    // Cheap deterministic value hash -> [0,1). Used for stars/dither, not security.
    let mut h = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65_536.0
}

/// The background colour at subpixel `(lx, ly)` in a `subw x subh` grid at time
/// `t` (seconds).
pub fn background(scene: Scene, subw: u32, subh: u32, lx: u32, ly: u32, t: f32) -> [u8; 3] {
    match scene {
        Scene::Void => [0, 0, 0],
        Scene::BlackIce => black_ice(subw, subh, lx, ly, t),
        Scene::Vapor => vapor(subw, subh, lx, ly, t),
    }
}

fn black_ice(_subw: u32, subh: u32, lx: u32, ly: u32, t: f32) -> [u8; 3] {
    let fy = ly as f32 / subh.max(1) as f32;
    // Base: near-black with a faint top-down teal gradient.
    let mut c = lerp3([1, 5, 8], [0, 9, 13], fy);
    // Telemetry grid every 16 subpixels, dim cyan.
    let gx = lx % 16 == 0;
    let gy = ly % 16 == 0;
    if gx || gy {
        c = add_sat(c, [0, 16, 22]);
    }
    // Brighter horizontal rails at fixed fractions with a travelling pulse.
    for (i, ry) in [0.18f32, 0.82].iter().enumerate() {
        let rail_y = (ry * subh as f32) as u32;
        if ly.abs_diff(rail_y) <= 1 {
            let pulse = ((lx as f32 * 0.04 + t * (2.0 + i as f32)).sin() * 0.5 + 0.5) * 40.0;
            c = add_sat(c, [0, 30 + pulse as u8, 42 + pulse as u8]);
        }
    }
    // Downward scan sweep: a soft band brightening the grid it crosses.
    let sweep = ((t * 0.35).fract() * subh as f32) as u32;
    let d = ly.abs_diff(sweep);
    if d < 10 {
        let g = (10 - d) as u8 * 3;
        c = add_sat(c, [0, g, g + 4]);
    }
    // Sparse stars in the dark.
    if hash2(lx, ly) > 0.9975 {
        c = add_sat(c, [30, 50, 60]);
    }
    c
}

fn vapor(subw: u32, subh: u32, lx: u32, ly: u32, t: f32) -> [u8; 3] {
    let w = subw.max(1) as f32;
    let h = subh.max(1) as f32;
    let fx = lx as f32 / w;
    let fy = ly as f32 / h;
    let horizon = 0.62f32;
    if fy < horizon {
        // Sky: indigo -> magenta -> warm horizon.
        let sky = if fy < horizon * 0.5 {
            lerp3([28, 8, 54], [150, 36, 120], fy / (horizon * 0.5))
        } else {
            lerp3(
                [150, 36, 120],
                [255, 120, 96],
                (fy - horizon * 0.5) / (horizon * 0.5),
            )
        };
        // Sliced sun disk, upper-centre.
        let (sx, sy) = (0.5 * w, 0.30 * h);
        let r = ((lx as f32 - sx).powi(2) + (ly as f32 - sy).powi(2)).sqrt();
        let sun_r = 0.16 * h;
        if r < sun_r {
            // Horizontal slices widen toward the bottom of the disk.
            let slice = (ly as f32 - (sy - sun_r)) / (sun_r * 0.5);
            let gap = (ly % 6) as f32 > (slice).min(5.0);
            if gap {
                let tsun = (r / sun_r).clamp(0.0, 1.0);
                return lerp3([255, 240, 120], [255, 80, 150], tsun);
            }
        }
        // Faint stars above.
        if fy < horizon * 0.55 && hash2(lx, ly) > 0.997 {
            return add_sat(sky, [40, 40, 50]);
        }
        sky
    } else {
        // Ground: dark plane with a neon perspective grid scrolling toward viewer.
        let base = lerp3([20, 4, 40], [6, 2, 16], (fy - horizon) / (1.0 - horizon));
        let depth = (fy - horizon) / (1.0 - horizon); // 0 at horizon, 1 at bottom
                                                      // Horizontal lines: spacing shrinks toward the horizon; scroll with time.
        let phase = (depth * depth * 18.0 - t * 1.5).fract();
        let hline = phase < 0.06;
        // Vertical lines converging to the vanishing point at (0.5, horizon).
        let vanish = 0.5;
        let spread = (fx - vanish) / (depth * 0.9 + 0.05);
        let vline = (spread * 8.0).fract() < 0.04 || (spread * 8.0).fract() > 0.96;
        let grid = [255, 40, 160];
        if hline || vline {
            let glow = (1.0 - depth) * 0.6 + 0.4;
            return add_sat(
                base,
                [
                    (grid[0] as f32 * glow) as u8,
                    (grid[1] as f32 * glow) as u8,
                    (grid[2] as f32 * glow) as u8,
                ],
            );
        }
        base
    }
}

/// A subpixel bounding box `(x, y, w, h)` for the reticle to track.
pub type SubBox = (u32, u32, u32, u32);

/// Draw the HUD reticle over an already-composited subpixel value. Corner brackets
/// track the subject's bounding box; a thin baseline and centre ticks read as a
/// live acquisition instrument. Returns the accent colour on a stroke, else `base`.
pub fn hud_overlay(
    base: [u8; 3],
    scene: Scene,
    lx: u32,
    ly: u32,
    bbox: Option<SubBox>,
    t: f32,
) -> [u8; 3] {
    let Some((bx, by, bw, bh)) = bbox else {
        return base;
    };
    if bw == 0 || bh == 0 {
        return base;
    }
    let accent = scene.accent();
    let x0 = bx;
    let y0 = by;
    let x1 = bx + bw;
    let y1 = by + bh;
    // Corner-bracket arm length scales with the box; thickness 1 subpixel.
    let arm = (bw.min(bh) / 5).clamp(3, 14);
    let near = |v: u32, edge: u32| v.abs_diff(edge) == 0;
    let on_h = |x: u32, y: u32, ey: u32, ex0: u32, ex1: u32| near(y, ey) && x >= ex0 && x <= ex1;
    let on_v = |x: u32, y: u32, ex: u32, ey0: u32, ey1: u32| near(x, ex) && y >= ey0 && y <= ey1;
    let corner = on_h(lx, ly, y0, x0, x0 + arm)
        || on_v(lx, ly, x0, y0, y0 + arm)
        || on_h(lx, ly, y0, x1.saturating_sub(arm), x1)
        || on_v(lx, ly, x1, y0, y0 + arm)
        || on_h(lx, ly, y1, x0, x0 + arm)
        || on_v(lx, ly, x0, y1.saturating_sub(arm), y1)
        || on_h(lx, ly, y1, x1.saturating_sub(arm), x1)
        || on_v(lx, ly, x1, y1.saturating_sub(arm), y1);
    if corner {
        // Pulse the reticle brightness gently so it reads as "live".
        let p = ((t * 3.0).sin() * 0.25 + 0.75).clamp(0.0, 1.0);
        return [
            (accent[0] as f32 * p) as u8,
            (accent[1] as f32 * p) as u8,
            (accent[2] as f32 * p) as u8,
        ];
    }
    // Centre crosshair ticks.
    let cx = bx + bw / 2;
    let cy = by + bh / 2;
    if (lx.abs_diff(cx) == 0 && ly.abs_diff(cy) <= 3)
        || (ly.abs_diff(cy) == 0 && lx.abs_diff(cx) <= 3)
    {
        return accent;
    }
    base
}
