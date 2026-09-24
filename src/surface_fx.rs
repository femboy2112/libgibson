//! Ordered endomorphisms of a realized entity surface (experimental, Rust-only).
//!
//! A chain applies left to right. The empty chain is identity; concatenation is
//! associative, but effects need not commute. These are cell transformations,
//! never terminal operations. Transparent holes stay holes, style-only cells
//! remain style-only, and a wide grapheme is indivisible. There is no alpha.

use crate::{Cell, Glyph, Rect, Rng, Style, Surface};

/// An entity-local cell selection, independent of color or terminal capability.
///
/// Normalized coordinates sample cell centers. Fractions are clamped to [0, 1];
/// NaN is zero. A wide glyph is selected only when **both** cells are inside the
/// mask, so a boundary never divides a glyph or modifies a cell outside itself.
#[derive(Debug, Clone, PartialEq)]
pub enum FxMask {
    /// A clipped integer cell rectangle.
    Rect(Rect),
    /// A left-to-right frontier. Zero selects nothing; one selects everything.
    HorizontalWipe { fraction: f32 },
    /// A top-to-bottom frontier. Zero selects nothing; one selects everything.
    VerticalWipe { fraction: f32 },
    /// An expanding ellipse in normalized entity coordinates, centered at
    /// `center`. At one its radius reaches the farthest corner; zero is empty.
    Radial { center: (f32, f32), fraction: f32 },
    /// A horizontal band with normalized height `width`. Its position travels
    /// from the topmost to bottommost fitting band; width one selects all rows.
    Band { position: f32, width: f32 },
    /// A stable coordinate-seeded selection. Increasing the fraction only adds
    /// cells; zero is empty and one selects the complete entity.
    Noise { seed: u64, fraction: f32 },
}

impl FxMask {
    fn contains(&self, x: u16, y: u16, width: u16, height: u16) -> bool {
        let nx = (f32::from(x) + 0.5) / f32::from(width);
        let ny = (f32::from(y) + 0.5) / f32::from(height);
        match *self {
            Self::Rect(rect) => rect.contains(x, y),
            Self::HorizontalWipe { fraction } => nx < unit(fraction),
            Self::VerticalWipe { fraction } => ny < unit(fraction),
            Self::Radial { center, fraction } => {
                let fraction = f64::from(unit(fraction));
                let cx = f64::from(unit(center.0));
                let cy = f64::from(unit(center.1));
                // Subtract the center before dividing, preserving reflection
                // symmetry at a half-cell frontier (not nx - cx rounding).
                let dx = (f64::from(x) + 0.5 - cx * f64::from(width)) / f64::from(width);
                let dy = (f64::from(y) + 0.5 - cy * f64::from(height)) / f64::from(height);
                let farthest_squared = cx.max(1.0 - cx).powi(2) + cy.max(1.0 - cy).powi(2);
                fraction > 0.0
                    && (fraction == 1.0
                        || dx.powi(2) + dy.powi(2) <= fraction.powi(2) * farthest_squared)
            }
            Self::Band { position, width } => {
                let width = unit(width);
                let start = unit(position) * (1.0 - width);
                ny >= start && ny < start + width
            }
            Self::Noise { seed, fraction } => {
                let fraction = unit(fraction);
                fraction > 0.0
                    && (fraction == 1.0
                        || Rng::new(seed ^ ((y as u64) << 32) ^ x as u64).next_f32() < fraction)
            }
        }
    }
}

/// A fully realized operation: no clocks, story state, or hidden randomness.
#[derive(Debug, Clone, PartialEq)]
pub enum SurfaceFx {
    /// Apply an ordinary effect only to selected complete glyphs. The input
    /// outside the mask is hidden from the effect and left exactly unchanged.
    /// Moved output is clipped as whole glyphs to the same selection. Nested
    /// scopes intersect; their coordinate system remains the full entity.
    Scoped {
        mask: FxMask,
        effect: Box<SurfaceFx>,
    },
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
            Self::Scoped {
                ref mask,
                ref effect,
            } => scoped(surface, mask, effect),
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

    /// Restrict this effect to an entity-local mask, preserving its order in
    /// the chain. Scopes do not change the entity's layout or dimensions.
    pub fn scoped(self, mask: FxMask) -> Self {
        Self::Scoped {
            mask,
            effect: Box::new(self),
        }
    }

    /// Ordered composition. Concatenating chains is associative; [] is identity.
    pub fn apply_chain(chain: &[Self], surface: &mut Surface) {
        for effect in chain {
            effect.apply(surface);
        }
    }
}

fn scoped(surface: &mut Surface, mask: &FxMask, effect: &SurfaceFx) {
    let width = surface.width;
    let height = surface.height;
    let mut selected: Vec<bool> = (0..surface.height)
        .flat_map(|y| (0..width).map(move |x| mask.contains(x, y, width, height)))
        .collect();
    // Narrow a selection at original wide-glyph boundaries. Cells outside the
    // selection must remain byte-for-byte intact, including their continuation.
    for y in 0..surface.height {
        for x in 0..width {
            let i = y as usize * width as usize + x as usize;
            let cell = &surface.cells[i];
            if !cell.transparent && !cell.style_only && cell.glyph.display_width == 2 {
                let whole = x + 1 < width && selected[i] && selected[i + 1];
                selected[i] = whole;
                if x + 1 < width {
                    selected[i + 1] = whole;
                }
            }
        }
    }
    if !selected.iter().any(|&s| s) {
        return;
    }
    if selected.iter().all(|&s| s) {
        effect.apply(surface);
        return;
    }
    let mut local = surface.clone();
    for (cell, &keep) in local.cells.iter_mut().zip(&selected) {
        if !keep {
            *cell = Cell::transparent();
        }
    }
    effect.apply(&mut local);
    for (cell, &replace) in surface.cells.iter_mut().zip(&selected) {
        if replace {
            *cell = Cell::transparent();
        }
    }
    for y in 0..surface.height {
        let mut x = 0;
        while x < width {
            let i = y as usize * width as usize + x as usize;
            let cell = &local.cells[i];
            let span = if !cell.transparent && !cell.style_only && cell.glyph.display_width == 2 {
                2
            } else {
                1
            };
            if !cell.is_continuation
                && x + span <= width
                && selected[i..i + span as usize].iter().all(|&s| s)
            {
                surface.cells[i..i + span as usize]
                    .clone_from_slice(&local.cells[i..i + span as usize]);
            }
            x += span;
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
