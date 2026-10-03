//! Calibrated glyph raster model for the "Terminal -> 1080p" basis experiment.
//!
//! A terminal cell is modelled as a `W x H` grid of *calibrated sample points*
//! (default 8x16, a 2:1 cell). Each glyph is a **coverage** vector `r(g)` in
//! `[0,1]^(W*H)`: the fraction of each sample point's area the glyph inks.
//!
//! This is an explicit, deterministic **MODEL**, not a claim about a particular
//! font's rasterization. It exists so the spatial-basis question can be posed
//! and measured without adding a font dependency to the crate. A real
//! calibration would replace [`Glyph::raster`] with the same-length vector
//! produced by an external rasterizer at a recorded font/size/AA identity; the
//! rest of the analysis is unchanged. The model deliberately gives Braille dots
//! a *round, gapped* shape (discs of radius < half the dot pitch) rather than
//! tiling the cell, because real Braille dots leave gaps between them and that
//! gap structure is exactly what other glyph families may fill.

/// Where a glyph is likely to be available in a common monospace font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Portability {
    /// Box drawing / block elements: CP437/VGA heritage, near-universal.
    Universal,
    /// Braille: present in most graphical terminal fonts, not the Linux VT.
    CommonGraphical,
    /// Plane-1 legacy computing (sextants/octants): sparse coverage.
    Low,
}

impl Portability {
    pub fn as_str(self) -> &'static str {
        match self {
            Portability::Universal => "universal",
            Portability::CommonGraphical => "graphical",
            Portability::Low => "low",
        }
    }
}

/// One candidate glyph and its calibrated coverage raster.
#[derive(Debug, Clone)]
pub struct Glyph {
    pub name: &'static str,
    pub ch: Option<char>,
    pub family: &'static str,
    pub portability: Portability,
    pub raster: Vec<f32>,
}

/// Cell sample grid, in calibrated sample points.
#[derive(Debug, Clone, Copy)]
pub struct Grid {
    pub w: usize,
    pub h: usize,
}

impl Grid {
    pub const fn new(w: usize, h: usize) -> Self {
        Self { w, h }
    }
    pub fn points(&self) -> usize {
        self.w * self.h
    }
}

impl Default for Grid {
    fn default() -> Self {
        Self::new(8, 16)
    }
}

fn rect(g: &Grid, f: impl Fn(usize, usize) -> bool) -> Vec<f32> {
    let mut v = vec![0.0f32; g.points()];
    for y in 0..g.h {
        for x in 0..g.w {
            if f(x, y) {
                v[y * g.w + x] = 1.0;
            }
        }
    }
    v
}

/// Centre of Braille dot `(dy, dx)` in calibrated samples.
fn dot_center(g: &Grid, dy: usize, dx: usize) -> (f64, f64) {
    let cw = g.w as f64 / 2.0;
    let ch = g.h as f64 / 4.0;
    (dx as f64 * cw + cw * 0.5, dy as f64 * ch + ch * 0.5)
}

/// Braille dot shape used by the raster model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DotShape {
    /// Round, gapped discs (the default): no two dots tile the cell.
    Disc,
    /// Axis-aligned squares of half-width `radius`. With `radius` at or above
    /// half the dot pitch they tile, removing the inter-dot gaps. Exists purely
    /// to bracket how much the round-gap assumption drives the results.
    Square,
}

/// The eight Braille dot blobs with a selectable shape, disks or squares of
/// size `radius` samples.
pub fn braille_dots_shaped(g: &Grid, radius: f64, shape: DotShape) -> Vec<Vec<f32>> {
    let mut out = Vec::with_capacity(8);
    for dy in 0..4 {
        for dx in 0..2 {
            let (cx, cy) = dot_center(g, dy, dx);
            out.push(rect(g, move |x, y| {
                let px = x as f64 + 0.5 - cx;
                let py = y as f64 + 0.5 - cy;
                match shape {
                    DotShape::Disc => px * px + py * py <= radius * radius,
                    DotShape::Square => px.abs() <= radius && py.abs() <= radius,
                }
            }));
        }
    }
    out
}

/// The eight Braille dot blobs: disks of radius `r` samples.
pub fn braille_dots(g: &Grid, radius: f64) -> Vec<Vec<f32>> {
    braille_dots_shaped(g, radius, DotShape::Disc)
}

/// The 256 static Braille masks as coverage unions of the eight dot blobs.
pub fn braille_mask_unions(dots: &[Vec<f32>]) -> Vec<Vec<f32>> {
    let p = dots[0].len();
    let mut out = Vec::with_capacity(256);
    for mask in 0u16..256 {
        let mut v = vec![0.0f32; p];
        for (i, dot) in dots.iter().enumerate() {
            if mask & (1 << i) != 0 {
                for (o, s) in v.iter_mut().zip(dot) {
                    *o = o.max(*s);
                }
            }
        }
        out.push(v);
    }
    out
}

/// Builds the structured candidate glyph bank for grid `g`.
pub fn candidates(g: &Grid) -> Vec<Glyph> {
    let mut out = Vec::new();
    let mut push = |name, ch, family, portability, raster: Vec<f32>| {
        out.push(Glyph {
            name,
            ch,
            family,
            portability,
            raster,
        });
    };

    let w = g.w;
    let h = g.h;
    let hw = w / 2;
    let hh = h / 2;
    let qw = w / 4;
    let qh = h / 4;

    // --- Full and half blocks (U+2580..U+2588) ---
    push(
        "upper-half",
        Some('▀'),
        "block",
        Portability::Universal,
        rect(g, |_, y| y < hh),
    );
    push(
        "lower-half",
        Some('▄'),
        "block",
        Portability::Universal,
        rect(g, |_, y| y >= hh),
    );
    push(
        "full-block",
        Some('█'),
        "block",
        Portability::Universal,
        rect(g, |_, _| true),
    );
    push(
        "left-half",
        Some('▌'),
        "block",
        Portability::Universal,
        rect(g, |x, _| x < hw),
    );
    push(
        "right-half",
        Some('▐'),
        "block",
        Portability::Universal,
        rect(g, |x, _| x >= hw),
    );

    // --- Quadrants (U+2596..U+259F) ---
    for (name, ch, ul, ur, ll, lr) in [
        ("quad-upper-left", '▘', true, false, false, false),
        ("quad-upper-right", '▝', false, true, false, false),
        ("quad-lower-left", '▖', false, false, true, false),
        ("quad-lower-right", '▗', false, false, false, true),
    ] {
        push(
            name,
            Some(ch),
            "quadrant",
            Portability::Universal,
            rect(g, move |x, y| {
                let left = x < hw;
                let top = y < hh;
                (top && left && ul)
                    || (top && !left && ur)
                    || (!top && left && ll)
                    || (!top && !left && lr)
            }),
        );
    }

    // --- Fractional blocks: left eighths (U+2588..U+258F) and lower eighths ---
    for n in 1..8usize {
        let name: &'static str = match n {
            1 => "left-1/8",
            2 => "left-1/4",
            3 => "left-3/8",
            4 => "left-1/2b",
            5 => "left-5/8",
            6 => "left-3/4",
            7 => "left-7/8",
            _ => "left-x",
        };
        let cut = w * n / 8;
        let ch = char::from_u32(0x2590 - n as u32); // U+258F..U+2589
        push(
            name,
            ch,
            "fraction-block",
            Portability::Universal,
            rect(g, move |x, _| x < cut),
        );
    }
    for n in 1..4usize {
        let name: &'static str = match n {
            1 => "lower-1/4",
            2 => "lower-1/2b",
            3 => "lower-3/4",
            _ => "lower-x",
        };
        let cut = h - h * n / 4;
        let ch = char::from_u32(0x2580 + ((4 - n) as u32)); // rough family representative
        push(
            name,
            ch,
            "fraction-block",
            Portability::Universal,
            rect(g, move |_, y| y >= cut),
        );
    }

    // --- Thin box-drawing primitives (1-sample rules) ---
    let t = (h as f64 / 16.0).max(1.0);
    push(
        "hbar",
        Some('─'),
        "box",
        Portability::Universal,
        rect(g, move |_, y| {
            (y as f64 + 0.5 - h as f64 / 2.0).abs() <= t / 2.0
        }),
    );
    push(
        "vbar",
        Some('│'),
        "box",
        Portability::Universal,
        rect(g, move |x, _| {
            (x as f64 + 0.5 - w as f64 / 2.0).abs() <= t / 2.0
        }),
    );
    push(
        "corner-tl",
        Some('┌'),
        "box",
        Portability::Universal,
        rect(g, move |x, y| {
            let on_v = x < hw && (x as f64 + 0.5 - (w as f64 / 4.0)).abs() <= t / 2.0;
            let on_h = y < hh && (y as f64 + 0.5 - (h as f64 / 4.0)).abs() <= t / 2.0;
            on_v || on_h
        }),
    );
    push(
        "corner-br",
        Some('┘'),
        "box",
        Portability::Universal,
        rect(g, move |x, y| {
            let on_v = x >= hw && (x as f64 + 0.5 - (3.0 * w as f64 / 4.0)).abs() <= t / 2.0;
            let on_h = y >= hh && (y as f64 + 0.5 - (3.0 * h as f64 / 4.0)).abs() <= t / 2.0;
            on_v || on_h
        }),
    );
    push(
        "tee-left",
        Some('├'),
        "box",
        Portability::Universal,
        rect(g, move |x, y| {
            let on_v = (x as f64 + 0.5 - (w as f64 / 4.0)).abs() <= t / 2.0;
            let on_h = (y as f64 + 0.5 - h as f64 / 2.0).abs() <= t / 2.0;
            on_v || on_h
        }),
    );

    // --- Diagonals (U+2571..U+2573) and a diagonal X ---
    push(
        "diag-fwd",
        Some('╱'),
        "diagonal",
        Portability::Universal,
        rect(g, move |x, y| {
            // line from bottom-left to top-right: x/w ~= 1 - y/h
            let xf = x as f64 + 0.5;
            let yf = y as f64 + 0.5;
            let d = (xf / w as f64 - (1.0 - yf / h as f64)) * (w as f64).min(h as f64);
            d.abs() <= 0.75
        }),
    );
    push(
        "diag-back",
        Some('╲'),
        "diagonal",
        Portability::Universal,
        rect(g, move |x, y| {
            let xf = x as f64 + 0.5;
            let yf = y as f64 + 0.5;
            let d = (xf / w as f64 - yf / h as f64) * (w as f64).min(h as f64);
            d.abs() <= 0.75
        }),
    );

    // --- Shades (ink-density blocks) ---
    for (name, ch, level) in [
        ("shade-light", '░', 0.25f32),
        ("shade-med", '▒', 0.5),
        ("shade-dark", '▓', 0.75),
    ] {
        let mut r = vec![1.0f32; g.points()];
        for v in r.iter_mut() {
            *v *= level;
        }
        push(name, Some(ch), "shade", Portability::Universal, r);
    }

    // --- Sextants (plane-1 legacy computing) modelled as 2x3 solid cells ---
    for (name, bits) in [
        ("sextant-1", 0b000001u8),
        ("sextant-2", 0b000011u8),
        ("sextant-4", 0b001111u8),
    ] {
        push(
            name,
            None,
            "sextant",
            Portability::Low,
            rect(g, move |x, y| {
                let col = if x < hw { 0 } else { 1 };
                let row = (y * 3) / h;
                let bit = row * 2 + col;
                bits & (1 << bit) != 0
            }),
        );
    }
    // --- A single octant representative (plane-1) ---
    push(
        "octant-1",
        None,
        "octant",
        Portability::Low,
        rect(g, |x, y| x < qw && y < qh),
    );

    // --- Geometric shapes with genuinely off-grid geometry ---
    push(
        "disc-small",
        Some('●'),
        "geometric",
        Portability::CommonGraphical,
        rect(g, |x, y| {
            let cx = w as f64 / 2.0;
            let cy = h as f64 / 2.0;
            let r = (w as f64 / 4.0).min(h as f64 / 4.0);
            (x as f64 + 0.5 - cx).powi(2) + (y as f64 + 0.5 - cy).powi(2) <= r * r
        }),
    );
    push(
        "diamond",
        Some('◆'),
        "geometric",
        Portability::CommonGraphical,
        rect(g, |x, y| {
            let cx = w as f64 / 2.0;
            let cy = h as f64 / 2.0;
            let dx = (x as f64 + 0.5 - cx).abs() / (w as f64 / 4.0);
            let dy = (y as f64 + 0.5 - cy).abs() / (h as f64 / 4.0);
            dx + dy <= 1.0
        }),
    );

    out
}

/// A coverage target tile at the calibrated resolution (linear `[0,1]`).
pub fn sample_target(g: &Grid, f: impl Fn(f64, f64) -> f64) -> Vec<f64> {
    let mut v = vec![0.0f64; g.points()];
    for y in 0..g.h {
        for x in 0..g.w {
            v[y * g.w + x] = f(x as f64 + 0.5, y as f64 + 0.5).clamp(0.0, 1.0);
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braille_dots_are_eight_disjoint_blobs() {
        let g = Grid::default();
        let dots = braille_dots(&g, 1.5);
        assert_eq!(dots.len(), 8);
        // Disjoint: no sample is inked by two dots.
        for (i, a) in dots.iter().enumerate() {
            for (j, b) in dots.iter().enumerate().skip(i + 1) {
                for (x, y) in a.iter().zip(b) {
                    assert!(!(*x > 0.0 && *y > 0.0), "dots {i},{j} overlap");
                }
            }
        }
        // Nonempty and not tiling: total inked area well below the cell.
        let total: f32 = dots.iter().map(|d| d.iter().sum::<f32>()).sum();
        assert!(total > 0.0 && total < g.points() as f32 * 0.7);
    }

    #[test]
    fn candidates_are_well_formed() {
        let g = Grid::default();
        let c = candidates(&g);
        assert!(c.len() >= 25, "only {} candidates", c.len());
        for gy in &c {
            assert_eq!(gy.raster.len(), g.points());
            assert!(gy.raster.iter().all(|v| (0.0..=1.0).contains(v)));
        }
    }

    #[test]
    fn full_block_is_all_ones_and_shade_is_scaled() {
        let g = Grid::default();
        let c = candidates(&g);
        let full = c.iter().find(|g| g.name == "full-block").unwrap();
        assert!(full.raster.iter().all(|v| *v == 1.0));
        let light = c.iter().find(|g| g.name == "shade-light").unwrap();
        assert!(
            (light.raster.iter().sum::<f32>() / full.raster.iter().sum::<f32>() - 0.25).abs()
                < 1e-6
        );
    }

    #[test]
    fn square_dots_tile_when_radius_reaches_half_pitch() {
        let g = Grid::default();
        let disc = braille_dots_shaped(&g, 2.0, DotShape::Disc);
        let square = braille_dots_shaped(&g, 2.0, DotShape::Square);
        // Square dots ink at least as much as discs at the same radius, and a
        // larger radius inks more.
        let sd: f32 = disc.iter().map(|d| d.iter().sum::<f32>()).sum();
        let ss: f32 = square.iter().map(|d| d.iter().sum::<f32>()).sum();
        assert!(ss >= sd);
        let square4 = braille_dots_shaped(&g, 4.0, DotShape::Square);
        let s4: f32 = square4.iter().map(|d| d.iter().sum::<f32>()).sum();
        assert!(s4 > ss);
    }

    #[test]
    fn braille_mask_unions_are_monotone_under_masks() {
        let g = Grid::default();
        let dots = braille_dots(&g, 1.5);
        let unions = braille_mask_unions(&dots);
        assert_eq!(unions.len(), 256);
        // Mask 1 (dot 0) is a subset of mask 0xFF (all dots) in coverage.
        for (a, b) in unions[1].iter().zip(&unions[0xFF]) {
            assert!(*a <= *b + 1e-6);
        }
    }
}
