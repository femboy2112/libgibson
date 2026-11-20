//! The opt-in cell encoders behind [`RgbRaster::to_surface_with`] (v0.9.0 design:
//! `docs/design/v0.9-cell-encoders.md`).
//!
//! Every encoder reads the raster as its own sample grid and fits each cell with
//! the exact best two-colour partition in linear light
//! ([`crate::temporal::fit_two_colour`], the generalization of the search in
//! `project_rgb_subcells`). A mask and its complement fit identically with fg and
//! bg swapped, so only masks with bit 0 clear are searched: the top-left sample is
//! always the background. A uniform cell is `' '` with fg = bg = the colour.
//!
//! Samples past the raster edge (a partial last cell) read as black, exactly as
//! `to_surface` treats an unmatched bottom pixel.
use super::RgbRaster;
use crate::cell::{Cell, Color, Glyph, Style};
use crate::glyph::{octant_char, sextant_char, CellEncoder, SubcellGlyphMode};
use crate::surface::Surface;
use crate::temporal::{fit_two_colour, linear_to_rgb8, project_braille_image, rgb8_to_linear};

/// Quadrant characters by pattern: bit 0 upper-left, 1 upper-right, 2 lower-left,
/// 3 lower-right.
const QUADRANTS: [char; 16] = [
    ' ', '▘', '▝', '▀', '▖', '▌', '▞', '▛', '▗', '▚', '▐', '▜', '▄', '▙', '▟', '█',
];

fn quadrant_char(pattern: u8) -> char {
    QUADRANTS[usize::from(pattern & 0x0F)]
}

pub(super) fn encode(raster: &RgbRaster, encoder: CellEncoder) -> Surface {
    match encoder {
        CellEncoder::HalfBlock => raster.to_surface(),
        CellEncoder::Quadrant => encode_grid::<4>(raster, (2, 2), quadrant_char),
        CellEncoder::Sextant => encode_grid::<6>(raster, (2, 3), sextant_char),
        CellEncoder::Octant => encode_grid::<8>(raster, (2, 4), octant_char),
        CellEncoder::Braille => encode_braille(raster),
        CellEncoder::Auto => encode_auto(raster),
    }
}

/// The sample at `(x, y)` as an sRGB triple; black past the raster edge.
fn sample(raster: &RgbRaster, x: i32, y: i32) -> [u8; 3] {
    let (r, g, b) = raster.get(x, y).unwrap_or_default();
    [r, g, b]
}

/// The cell's `N = gw * gh` samples, index `gw * row + column`.
fn gather<const N: usize>(raster: &RgbRaster, cx: u16, cy: u16, grid: (u16, u16)) -> [[u8; 3]; N] {
    let (gw, gh) = (usize::from(grid.0), usize::from(grid.1));
    let mut samples = [[0u8; 3]; N];
    for row in 0..gh {
        for col in 0..gw {
            let x = i32::from(cx) * gw as i32 + col as i32;
            let y = i32::from(cy) * gh as i32 + row as i32;
            samples[row * gw + col] = sample(raster, x, y);
        }
    }
    samples
}

fn put(surface: &mut Surface, cx: u16, cy: u16, glyph: char, fg: [u8; 3], bg: [u8; 3]) {
    surface.set_cell(
        cx,
        cy,
        Cell::new(
            Glyph::from_char(glyph),
            Style::new()
                .fg(Color::Rgb(fg[0], fg[1], fg[2]))
                .bg(Color::Rgb(bg[0], bg[1], bg[2])),
        ),
    );
}

fn is_uniform<const N: usize>(samples: &[[u8; 3]; N]) -> bool {
    samples.iter().all(|s| *s == samples[0])
}

/// Canonical masks of an `n`-sample cell: bit 0 clear, never empty, ascending.
/// For `n = 8` this is the `2..=254` step 2 order of `project_rgb_subcells`.
fn canonical_masks(n: usize) -> impl Iterator<Item = u16> + Clone {
    (2u16..(1u16 << n)).step_by(2)
}

/// Quadrant (`N = 4`), sextant (`N = 6`) and octant (`N = 8`) cells.
fn encode_grid<const N: usize>(
    raster: &RgbRaster,
    grid: (u16, u16),
    glyph: fn(u8) -> char,
) -> Surface {
    let mut surface = Surface::new(
        raster.width().div_ceil(grid.0),
        raster.height().div_ceil(grid.1),
    );
    for cy in 0..surface.height {
        for cx in 0..surface.width {
            let samples = gather::<N>(raster, cx, cy, grid);
            if is_uniform(&samples) {
                put(&mut surface, cx, cy, ' ', samples[0], samples[0]);
                continue;
            }
            let linear = samples.map(rgb8_to_linear);
            match fit_two_colour(&linear, canonical_masks(N)) {
                Some(fit) => put(
                    &mut surface,
                    cx,
                    cy,
                    glyph(fit.mask as u8),
                    linear_to_rgb8(fit.fg),
                    linear_to_rgb8(fit.bg),
                ),
                // Unreachable for N >= 2 (the mask list is never empty).
                None => put(&mut surface, cx, cy, ' ', samples[0], samples[0]),
            }
        }
    }
    surface
}

/// Braille: the existing two-colour projector over each cell's 2 x 4 samples,
/// realized as `U+2800 + bits`. Its own mask orientation and uniform rule apply.
fn encode_braille(raster: &RgbRaster) -> Surface {
    project_braille_image(
        raster.width().div_ceil(2),
        raster.height().div_ceil(4),
        |x, y| sample(raster, i32::from(x), i32::from(y)),
    )
    .static_surface(SubcellGlyphMode::Braille2x4)
}

/// The glyph family and pattern an `Auto` candidate stands for.
#[derive(Clone, Copy)]
enum Pick {
    Half,
    Quadrant(u8),
    Sextant(u8),
}

const TILE_COLS: usize = 2;
const TILE_ROWS: usize = 6;
const TILE: usize = TILE_COLS * TILE_ROWS;

/// Expands a family `pattern` onto the 2 x 6 tile (bit `2 * row + column`);
/// `bit_of(row, column)` is the pattern bit the tile sample belongs to.
fn expand(pattern: u16, bit_of: impl Fn(usize, usize) -> usize) -> u16 {
    let mut mask = 0u16;
    for row in 0..TILE_ROWS {
        for col in 0..TILE_COLS {
            if (pattern >> bit_of(row, col)) & 1 == 1 {
                mask |= 1u16 << (TILE_COLS * row + col);
            }
        }
    }
    mask
}

/// Every `Auto` candidate as `(mask on the 2 x 6 tile, family)`: half-block,
/// then the 7 quadrant, then the 31 sextant canonical masks. The order is the
/// tie-break order (the simplest glyph wins equal fits).
fn auto_candidates() -> Vec<(u16, Pick)> {
    // Bit 1 = the bottom half (rows 3..6), so bit 0 (top-left) is clear.
    let half = std::iter::once((expand(0b10, |row, _| row / 3), Pick::Half));
    let quadrants = (2u16..16).step_by(2).map(|pattern| {
        (
            expand(pattern, |row, col| 2 * (row / 3) + col),
            Pick::Quadrant(pattern as u8),
        )
    });
    let sextants = (2u16..64).step_by(2).map(|pattern| {
        (
            expand(pattern, |row, col| 2 * (row / 2) + col),
            Pick::Sextant(pattern as u8),
        )
    });
    half.chain(quadrants).chain(sextants).collect()
}

/// `Auto`: per 2 x 6 tile, the least linear-light SSE over the half-block,
/// quadrant and sextant partitions, measured on all 12 samples. Ties go to the
/// earlier (simpler) candidate, so the half-block is never beaten by an equal fit.
fn encode_auto(raster: &RgbRaster) -> Surface {
    let grid = CellEncoder::Auto.sample_grid();
    let candidates = auto_candidates();
    let mut surface = Surface::new(
        raster.width().div_ceil(grid.0),
        raster.height().div_ceil(grid.1),
    );
    for cy in 0..surface.height {
        for cx in 0..surface.width {
            let samples = gather::<TILE>(raster, cx, cy, grid);
            if is_uniform(&samples) {
                put(&mut surface, cx, cy, ' ', samples[0], samples[0]);
                continue;
            }
            let linear = samples.map(rgb8_to_linear);
            let Some(fit) = fit_two_colour(&linear, candidates.iter().map(|c| c.0)) else {
                put(&mut surface, cx, cy, ' ', samples[0], samples[0]);
                continue;
            };
            let (fg, bg) = (linear_to_rgb8(fit.fg), linear_to_rgb8(fit.bg));
            match candidates[fit.index].1 {
                // The fitted foreground is the bottom half; the half-block glyph
                // draws the top, so swap (the same pixels, the default glyph).
                Pick::Half => put(&mut surface, cx, cy, '▀', bg, fg),
                Pick::Quadrant(pattern) => {
                    put(&mut surface, cx, cy, quadrant_char(pattern), fg, bg)
                }
                Pick::Sextant(pattern) => put(&mut surface, cx, cy, sextant_char(pattern), fg, bg),
            }
        }
    }
    surface
}
