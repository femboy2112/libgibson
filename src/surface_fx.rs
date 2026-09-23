//! Ordered endomorphisms of a realized entity surface (experimental, Rust-only).
//!
//! A chain applies left to right. The empty chain is identity; concatenation is
//! associative, but effects need not commute. These are cell transformations,
//! never terminal operations. Transparent holes stay holes, style-only cells
//! remain style-only, and a wide grapheme is indivisible. There is no alpha.

use crate::{Cell, Glyph, Rng, Style, Surface};

/// A fully realized operation: no clocks, story state, or hidden randomness.
#[derive(Debug, Clone, PartialEq)]
pub enum SurfaceFx {
    StyleOverlay(Style),
    /// Apply a style to a stable seeded fraction of painted graphemes, leaving
    /// unselected cells intact. A terminal-native invasion/reveal, not alpha.
    StyleMask {
        style: Style,
        seed: u64,
        fraction: f32,
    },
    Dim,
    Reverse,
    /// Shift each row; a nonzero seed adds deterministic +/- one cell jitter.
    /// Vacated cells become transparent, exposing the layer beneath.
    RowShift {
        amount: i32,
        seed: u64,
    },
    /// Shift a horizontal strip in entity-local coordinates.
    Tear {
        row: u16,
        height: u16,
        amount: i32,
    },
    /// Substitute narrow opaque glyphs, preserving their styles.
    Scramble {
        seed: u64,
        intensity: f32,
    },
    /// Keep a stable seeded fraction of graphemes: 0 hides, 1 is exact identity.
    /// Wide pairs share their lead's threshold. NaN is treated as zero.
    Dissolve {
        seed: u64,
        fraction: f32,
    },
    /// Style one row, positioned from top (0) to bottom (1).
    Scanline {
        position: f32,
        style: Style,
    },
}

impl SurfaceFx {
    /// Apply this operation to the whole entity-local surface.
    pub fn apply(&self, surface: &mut Surface) {
        match *self {
            Self::StyleOverlay(style) => overlay(surface, style, None),
            Self::StyleMask {
                style,
                seed,
                fraction,
            } => {
                masked(surface, seed, fraction, Some(style));
            }
            Self::Dim => overlay(surface, Style::new().dim(), None),
            Self::Reverse => crate::glitch::invert_rect(surface, surface.area()),
            Self::Scanline { position, style } => {
                if surface.height > 0 {
                    let row =
                        (unit(position) * surface.height.saturating_sub(1) as f32).round() as u16;
                    overlay(surface, style, Some(row));
                }
            }
            Self::RowShift { amount, seed } => {
                let mut rng = Rng::new(seed);
                for y in 0..surface.height {
                    let jitter = if seed == 0 {
                        0
                    } else {
                        (rng.next_u64() % 3) as i64 - 1
                    };
                    shift_row(surface, y, i64::from(amount) + jitter);
                }
            }
            Self::Tear {
                row,
                height,
                amount,
            } => {
                for y in row..row.saturating_add(height).min(surface.height) {
                    shift_row(surface, y, i64::from(amount));
                }
            }
            Self::Scramble { seed, intensity } => {
                let mut rng = Rng::new(seed);
                for cell in &mut surface.cells {
                    if !cell.transparent
                        && !cell.style_only
                        && !cell.is_continuation
                        && cell.glyph.display_width == 1
                        && rng.next_f32() < unit(intensity)
                    {
                        let alphabet = crate::transition::SCRAMBLE_GLYPHS;
                        cell.glyph =
                            Glyph::new(alphabet[(rng.next_u64() % alphabet.len() as u64) as usize]);
                    }
                }
            }
            Self::Dissolve { seed, fraction } => {
                masked(surface, seed, fraction, None);
            }
        }
    }

    /// Ordered composition. Concatenating chains is associative; [] is identity.
    pub fn apply_chain(chain: &[Self], surface: &mut Surface) {
        for effect in chain {
            effect.apply(surface);
        }
    }
}

fn unit(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

fn overlay(surface: &mut Surface, style: Style, row: Option<u16>) {
    for y in 0..surface.height {
        if row.is_some_and(|r| r != y) {
            continue;
        }
        for x in 0..surface.width {
            let cell = surface.get_mut(x, y).unwrap();
            if !cell.transparent {
                cell.style = cell.style.overlay(style);
            }
        }
    }
}

fn shift_row(surface: &mut Surface, y: u16, amount: i64) {
    if amount == 0 {
        return;
    }
    let start = y as usize * surface.width as usize;
    let end = start + surface.width as usize;
    let source = surface.cells[start..end].to_vec();
    surface.cells[start..end].fill(Cell::transparent());
    for (x, cell) in source.iter().enumerate() {
        if cell.is_continuation || cell.transparent {
            continue;
        }
        let target = x as i64 + amount;
        let width = if cell.style_only {
            1
        } else {
            cell.glyph.display_width.max(1) as i64
        };
        if target < 0 || target + width > surface.width as i64 {
            continue;
        }
        let dest = start + target as usize;
        surface.cells[dest] = cell.clone();
        if width == 2 && x + 1 < source.len() {
            surface.cells[dest + 1] = source[x + 1].clone();
        }
    }
}

fn masked(surface: &mut Surface, seed: u64, fraction: f32, style: Option<Style>) {
    let fraction = unit(fraction);
    if style.is_none() && fraction == 1.0 {
        return;
    }
    for y in 0..surface.height {
        let mut x: u16 = 0;
        while x < surface.width {
            let idx = y as usize * surface.width as usize + x as usize;
            let cell = &surface.cells[idx];
            let width = if !cell.transparent && !cell.style_only && cell.glyph.display_width == 2 {
                2
            } else {
                1
            };
            // Stable local coordinates: changing glyphs or clipping does not
            // reshuffle unrelated cells. Wide pairs use one threshold.
            let key = seed ^ ((y as u64) << 32) ^ x as u64;
            let selected =
                fraction > 0.0 && (fraction == 1.0 || Rng::new(key).next_f32() < fraction);
            for dx in 0..width.min(surface.width - x) {
                let cell = &mut surface.cells[idx + dx as usize];
                match style {
                    Some(style) if selected && !cell.transparent => {
                        cell.style = cell.style.overlay(style)
                    }
                    None if !selected => *cell = Cell::transparent(),
                    _ => {}
                }
            }
            x = x.saturating_add(width);
        }
    }
}
