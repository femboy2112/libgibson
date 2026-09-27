//! Experimental temporal cell modulation.
//!
//! This module is deliberately **not** a terminal protocol and does not change
//! [`crate::Cell`]. It generates ordinary [`crate::Surface`] frames whose glyph
//! masks vary over time. The first implementation is intentionally narrow:
//! phase-decorrelated sigma-delta over logical 2x4 subcell duty cycles.
//!
//! The practical target is residual temporal dithering (small corrections over a
//! good static frame), not general spatial super-resolution. Presentation cadence
//! cannot be observed through a PTY, so callers must provide an externally
//! measured [`PresentationProfile`] and fall back to static rendering when its
//! safety/quality gate fails.

use crate::cell::{Cell, Color, Glyph, Style};
use crate::glyph::SubcellGlyphMode;
use crate::surface::Surface;

/// Measured properties of the terminal/display presentation path.
///
/// These are **presentation** measurements, not application emission settings.
/// LibGibson cannot infer them from PTY writes; use an external cadence beacon or
/// other measurement process and store the result as a profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PresentationProfile {
    /// Observed terminal presentation cadence.
    pub presentation_hz: f32,
    /// Fraction of emitted frames observed as distinct presented frames (0..=1).
    pub survival_rate: f32,
    /// 95th-percentile inter-presentation jitter, in milliseconds.
    pub jitter_p95_ms: f32,
    /// Whether this profile came from a real measurement rather than a guess.
    pub measured: bool,
}

impl PresentationProfile {
    pub const fn unmeasured() -> Self {
        Self {
            presentation_hz: 0.0,
            survival_rate: 0.0,
            jitter_p95_ms: 0.0,
            measured: false,
        }
    }

    pub fn measured(presentation_hz: f32, survival_rate: f32, jitter_p95_ms: f32) -> Self {
        Self {
            presentation_hz: presentation_hz.max(0.0),
            survival_rate: survival_rate.clamp(0.0, 1.0),
            jitter_p95_ms: jitter_p95_ms.max(0.0),
            measured: true,
        }
    }
}

impl Default for PresentationProfile {
    fn default() -> Self {
        Self::unmeasured()
    }
}

/// Conservative gate for residual **luminance** modulation.
///
/// Defaults encode the research-stage contract: never luminance-modulate an
/// unmeasured path; require a high presentation cadence and high frame survival;
/// cap requested linear-light modulation depth to 10%. This is an engineering
/// guardrail, not medical certification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemporalSafetyPolicy {
    pub min_luminance_hz: f32,
    pub min_survival_rate: f32,
    pub max_luminance_depth: f32,
}

impl Default for TemporalSafetyPolicy {
    fn default() -> Self {
        Self {
            min_luminance_hz: 100.0,
            min_survival_rate: 0.90,
            max_luminance_depth: 0.10,
        }
    }
}

/// Why an application should fall back to its static realization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemporalGate {
    Enabled,
    ReducedMotion,
    Unmeasured,
    CadenceTooLow,
    SurvivalTooLow,
    DepthTooHigh,
}

impl TemporalSafetyPolicy {
    /// Gates the **presentation** conditions only: reduced-motion preference, a
    /// measured profile, adequate cadence and adequate frame survival. It does
    /// **not** bound modulation depth.
    ///
    /// This is the check [`TemporalDisplayProcessor`] applies before enforcing its
    /// own per-cell bound computed from the actual emitted fg/bg luminance swing.
    /// Profile-gating alone is *not* sufficient for safe output.
    pub fn gate_profile(self, profile: PresentationProfile, reduced_motion: bool) -> TemporalGate {
        if reduced_motion {
            return TemporalGate::ReducedMotion;
        }
        if !profile.measured {
            return TemporalGate::Unmeasured;
        }
        if !profile.presentation_hz.is_finite() || profile.presentation_hz < self.min_luminance_hz {
            return TemporalGate::CadenceTooLow;
        }
        if !profile.survival_rate.is_finite() || profile.survival_rate < self.min_survival_rate {
            return TemporalGate::SurvivalTooLow;
        }
        TemporalGate::Enabled
    }

    /// Evaluates the profile gate plus a caller-supplied scalar depth.
    ///
    /// **This depth check is only a lint.** `requested_depth` is a number the
    /// caller asserts; it has no mechanical link to what is emitted. A small
    /// requested depth does not imply a small *instantaneous* swing — if a cell's
    /// foreground and background are far apart in luminance, a single dot flip is
    /// a large step regardless of the requested duty. For real safety, drive
    /// modulation through [`TemporalDisplayProcessor`], which computes the swing
    /// from the actual cell colors it intends to emit and freezes cells that
    /// exceed the cap. `requested_depth` is a 0..=1 linear-light swing.
    pub fn gate_luminance(
        self,
        profile: PresentationProfile,
        requested_depth: f32,
        reduced_motion: bool,
    ) -> TemporalGate {
        match self.gate_profile(profile, reduced_motion) {
            TemporalGate::Enabled => {
                if !requested_depth.is_finite()
                    || requested_depth < 0.0
                    || requested_depth > self.max_luminance_depth
                {
                    TemporalGate::DepthTooHigh
                } else {
                    TemporalGate::Enabled
                }
            }
            gated => gated,
        }
    }
}

/// Result of projecting eight RGB subpixels onto one stable two-color cell basis.
///
/// `style.fg` and `style.bg` are selected once in linear light. `duty[i]` is
/// the continuous coordinate of target subpixel `i` along the bg->fg segment.
/// A temporal modulator can therefore improve the binary `static_mask` while
/// leaving SGR color state unchanged across phases.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemporalCellProjection {
    pub style: Style,
    pub duty: [f32; 8],
    pub static_mask: u8,
    /// **Ideal** static RMSE (linear RGB) of the best binary two-color fit using
    /// the *unquantized* centroids: an error floor, not what actually ships. For
    /// acceptance claims use [`TemporalCellProjection::emitted_static_rmse`],
    /// which measures the colors after 8-bit sRGB quantization.
    pub static_rmse: f32,
    /// **Ideal** RMSE after allowing continuous mixture along the *unquantized*
    /// bg->fg segment: the infinite-time floor for this one-dimensional color
    /// basis. Finite temporal sequences, and 8-bit color, may be worse.
    pub line_rmse: f32,
    /// **Emitted** static RMSE (linear RGB) after the chosen fg/bg are quantized
    /// to the 8-bit sRGB colors LibGibson actually emits. This is the honest
    /// static-fidelity number for acceptance decisions; it is always
    /// `>= static_rmse` because quantization can only add error.
    pub emitted_static_rmse: f32,
}

/// Fits one logical Braille cell's eight RGB targets to a stable fg/bg basis.
///
/// The search is exact. A mask and its bitwise complement describe the same
/// two-group partition (foreground/background swapped) and yield identical SSE,
/// so only the 127 complement-pair representatives — the even masks `2..=254`,
/// i.e. those with subpixel 0 assigned to the background — are evaluated, at half
/// the cost of a full 254-mask scan and with a bit-identical fit (verified
/// against the full scan by the `projector_127_representatives_match_full_254_scan_fit`
/// oracle test). Optimal linear-light centroids are taken per partition; the
/// chosen colors define the best static two-color cell, and each target is then
/// orthogonally projected onto the bg->fg segment to produce a temporal duty
/// cycle.
///
/// A numerically-stabler sufficient-statistics form was evaluated and rejected:
/// the naive `Σ‖x‖² − ‖Σx‖²/n` identity flips the argmin on roughly 3 in a
/// million f32 tiles via catastrophic cancellation, so this keeps the exact
/// direct-distance SSE as the canonical CPU reference.
pub fn project_rgb_subcells(target: [[u8; 3]; 8]) -> TemporalCellProjection {
    let linear = target.map(rgb8_to_linear);
    let mut best_mask = 0u8;
    let mut best_fg = [0.0f32; 3];
    let mut best_bg = [0.0f32; 3];
    let mut best_sse = f32::INFINITY;

    for mask in (2u8..=254u8).step_by(2) {
        let (fg, bg) = partition_centroids(&linear, mask);
        let mut sse = 0.0f32;
        for (i, pixel) in linear.iter().enumerate() {
            let centroid = if mask & (1u8 << i) != 0 { fg } else { bg };
            sse += rgb_distance_squared(*pixel, centroid);
        }
        if sse < best_sse {
            best_sse = sse;
            best_mask = mask;
            best_fg = fg;
            best_bg = bg;
        }
    }

    let delta = [
        best_fg[0] - best_bg[0],
        best_fg[1] - best_bg[1],
        best_fg[2] - best_bg[2],
    ];
    let denom = dot3(delta, delta);

    let mut duty = [0.0f32; 8];
    let mut line_sse = 0.0f32;
    if denom > 1.0e-12 {
        for (i, pixel) in linear.iter().enumerate() {
            let relative = [
                pixel[0] - best_bg[0],
                pixel[1] - best_bg[1],
                pixel[2] - best_bg[2],
            ];
            let alpha = (dot3(relative, delta) / denom).clamp(0.0, 1.0);
            duty[i] = alpha;
            let reconstructed = [
                best_bg[0] + delta[0] * alpha,
                best_bg[1] + delta[1] * alpha,
                best_bg[2] + delta[2] * alpha,
            ];
            line_sse += rgb_distance_squared(*pixel, reconstructed);
        }
    } else {
        // Uniform/degenerate cell: the background alone already carries the
        // color, so avoid pointless glyph churn.
        best_mask = 0;
        line_sse = best_sse;
    }

    // Emitted error: re-linearize the fg/bg after the 8-bit sRGB quantization
    // LibGibson actually emits, and measure the static realization against that.
    // This is the honest number for acceptance claims (see `emitted_static_rmse`).
    let emitted_fg8 = linear_to_rgb8(best_fg);
    let emitted_bg8 = linear_to_rgb8(best_bg);
    let emitted_fg = rgb8_to_linear(emitted_fg8);
    let emitted_bg = rgb8_to_linear(emitted_bg8);
    let mut emitted_sse = 0.0f32;
    for (i, pixel) in linear.iter().enumerate() {
        let centroid = if best_mask & (1u8 << i) != 0 {
            emitted_fg
        } else {
            emitted_bg
        };
        emitted_sse += rgb_distance_squared(*pixel, centroid);
    }

    TemporalCellProjection {
        style: Style::default()
            .fg(Color::Rgb(emitted_fg8[0], emitted_fg8[1], emitted_fg8[2]))
            .bg(Color::Rgb(emitted_bg8[0], emitted_bg8[1], emitted_bg8[2])),
        duty,
        static_mask: best_mask,
        static_rmse: (best_sse / 24.0).sqrt(),
        line_rmse: (line_sse / 24.0).sqrt(),
        emitted_static_rmse: (emitted_sse / 24.0).sqrt(),
    }
}

fn partition_centroids(target: &[[f32; 3]; 8], mask: u8) -> ([f32; 3], [f32; 3]) {
    let mut fg = [0.0f32; 3];
    let mut bg = [0.0f32; 3];
    let mut fg_n = 0.0f32;
    let mut bg_n = 0.0f32;
    for (i, pixel) in target.iter().enumerate() {
        if mask & (1u8 << i) != 0 {
            for channel in 0..3 {
                fg[channel] += pixel[channel];
            }
            fg_n += 1.0;
        } else {
            for channel in 0..3 {
                bg[channel] += pixel[channel];
            }
            bg_n += 1.0;
        }
    }
    for channel in 0..3 {
        fg[channel] /= fg_n;
        bg[channel] /= bg_n;
    }
    (fg, bg)
}

fn rgb8_to_linear(rgb: [u8; 3]) -> [f32; 3] {
    rgb.map(|value| {
        let value = value as f32 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    })
}

fn linear_to_rgb8(rgb: [f32; 3]) -> [u8; 3] {
    rgb.map(|value| {
        let value = value.clamp(0.0, 1.0);
        let srgb = if value <= 0.003_130_8 {
            value * 12.92
        } else {
            1.055 * value.powf(1.0 / 2.4) - 0.055
        };
        (srgb.clamp(0.0, 1.0) * 255.0).round() as u8
    })
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn rgb_distance_squared(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    dot3(d, d)
}

/// Maps a logical Braille subpixel `(dx in 0..2, dy in 0..4)` to its index in the
/// 8-element subcell array (whose bit is `1 << index`). Matches the Unicode
/// Braille dot bits used by [`SubcellGlyphMode::subcell_glyph`] and
/// [`crate::BrailleCanvas`]: left column rows 0-3 -> 0x01/0x02/0x04/0x40, right
/// column rows 0-3 -> 0x08/0x10/0x20/0x80.
const BRAILLE_DOT_INDEX: [[usize; 2]; 4] = [[0, 3], [1, 4], [2, 5], [6, 7]];

/// Per-cell two-color projections for a whole logical Braille image.
///
/// Produced by [`project_braille_image`]. The image is a grid of `width x height`
/// terminal cells; each cell fits eight logical 2x4 subpixels to a stable fg/bg
/// pair via [`project_rgb_subcells`]. The result can build the exact static
/// [`Surface`] (the always-valid fallback), install temporal targets into a
/// [`TemporalBrailleField`], and report emitted (post-quantization) fidelity.
#[derive(Debug, Clone)]
pub struct BrailleImageProjection {
    width: u16,
    height: u16,
    cells: Vec<TemporalCellProjection>,
}

impl BrailleImageProjection {
    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    /// The projection for cell `(x, y)`, or `None` if out of bounds.
    pub fn cell(&self, x: u16, y: u16) -> Option<&TemporalCellProjection> {
        if x < self.width && y < self.height {
            Some(&self.cells[(y as usize) * self.width as usize + x as usize])
        } else {
            None
        }
    }

    /// Builds the exact static two-color [`Surface`]: each cell's SSE-optimal mask
    /// realized through `mode` with its stable fg/bg style. This is the image's
    /// static fallback and is valid without any temporal modulation.
    pub fn static_surface(&self, mode: SubcellGlyphMode) -> Surface {
        let mut surface = Surface::new(self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                let proj = &self.cells[(y as usize) * self.width as usize + x as usize];
                let cell = match mode.subcell_glyph(proj.static_mask) {
                    Some(ch) => Cell::new(Glyph::from_char(ch), proj.style),
                    None => Cell::space(proj.style),
                };
                surface.set_cell(x, y, cell);
            }
        }
        surface
    }

    /// Installs every cell's duty targets and stable style into a same-sized
    /// [`TemporalBrailleField`] for temporal advancement, preserving each cell's
    /// optimal static mask. With [`ResetPolicy::Reset`] the accumulators are
    /// reseeded (use this when the image is new content). Returns `false` without
    /// modifying the field if its dimensions differ from this projection.
    pub fn install_into(&self, field: &mut TemporalBrailleField, reset: ResetPolicy) -> bool {
        if field.width() != self.width || field.height() != self.height {
            return false;
        }
        for y in 0..self.height {
            for x in 0..self.width {
                let proj = self.cells[(y as usize) * self.width as usize + x as usize];
                field.set_cell_projection(x, y, proj);
                if reset == ResetPolicy::Reset {
                    field.reset_cell(x, y);
                }
            }
        }
        true
    }

    /// Mean emitted (post-quantization) static RMSE over all cells: the honest
    /// static-fidelity figure for the whole image (0 for an empty projection).
    pub fn mean_emitted_static_rmse(&self) -> f32 {
        if self.cells.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.cells.iter().map(|c| c.emitted_static_rmse).sum();
        sum / self.cells.len() as f32
    }
}

/// Projects a logical Braille RGB image into per-cell two-color projections.
///
/// The image has `width x height` terminal cells and therefore `2*width` by
/// `4*height` **logical** subpixels — these are logical Braille samples, not
/// calibrated physical font pixels. `sample(lx, ly)` returns the 8-bit sRGB
/// color at logical subpixel `(lx in 0..2*width, ly in 0..4*height)`; each cell's
/// eight dots are gathered in the Unicode Braille dot order (matching
/// [`SubcellGlyphMode::subcell_glyph`]) and fitted by [`project_rgb_subcells`].
pub fn project_braille_image(
    width: u16,
    height: u16,
    sample: impl Fn(u16, u16) -> [u8; 3],
) -> BrailleImageProjection {
    let count = (width as usize) * (height as usize);
    let mut cells = Vec::with_capacity(count);
    for cy in 0..height {
        for cx in 0..width {
            let mut target = [[0u8; 3]; 8];
            for dy in 0u16..4 {
                for dx in 0u16..2 {
                    let lx = cx * 2 + dx;
                    let ly = cy * 4 + dy;
                    target[BRAILLE_DOT_INDEX[dy as usize][dx as usize]] = sample(lx, ly);
                }
            }
            cells.push(project_rgb_subcells(target));
        }
    }
    BrailleImageProjection {
        width,
        height,
        cells,
    }
}

/// Stateful temporal modulation over a grid of logical 2x4 subcell masks.
///
/// Each of the eight logical dots in each terminal cell stores a target duty
/// cycle in `0..=1`. [`TemporalBrailleField::advance`] runs one first-order
/// sigma-delta step per dot, packs the resulting mask through the selected
/// [`SubcellGlyphMode`], and returns an ordinary [`Surface`].
///
/// Accumulators receive deterministic per-dot starting phases so a uniform field
/// does not flash in perfect lockstep. This is phase decorrelation, not a claim
/// of a perceptually optimal spatiotemporal blue-noise-sequence.
/// Whether changing a cell's target keeps or discards its accumulated residual.
///
/// A first-order sigma-delta accumulator integrates error over time. When a cell
/// keeps showing the *same* logical content, that history is exactly what makes
/// the time-average converge, so it must be [`ResetPolicy::Keep`]. When the
/// content *changes* (a new image, a moved viewport, a repainted region), the old
/// accumulated error is stale and would smear the previous target into the new
/// one, so the caller must ask for [`ResetPolicy::Reset`]. Motion is an explicit
/// caller decision here, never inferred from incidental RGB differences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetPolicy {
    /// Preserve the accumulator so an unchanged target keeps converging.
    Keep,
    /// Reseed the accumulator to its deterministic decorrelated phase, erasing
    /// any residual from prior content.
    Reset,
}

#[derive(Debug, Clone)]
pub struct TemporalBrailleField {
    width: u16,
    height: u16,
    seed: u64,
    duty: Vec<[f32; 8]>,
    styles: Vec<Style>,
    /// Authoritative per-cell static fallback mask. For a cell set from a
    /// [`TemporalCellProjection`] this is the projector's SSE-optimal mask; for a
    /// directly-set duty it is that duty thresholded at 0.5. Static surfaces emit
    /// this exact mask so the fallback is deterministic and independent of the
    /// (proven, but tie-fragile) equivalence between thresholding and the optimum.
    static_mask: Vec<u8>,
    accumulator: Vec<[f32; 8]>,
    frame_index: u64,
}

impl TemporalBrailleField {
    pub fn new(width: u16, height: u16, seed: u64) -> Self {
        let cells = (width as usize).saturating_mul(height as usize);
        let mut accumulator = vec![[0.0; 8]; cells];
        for (cell_index, phases) in accumulator.iter_mut().enumerate() {
            for (dot, phase) in phases.iter_mut().enumerate() {
                *phase = unit_hash(seed, cell_index as u64, dot as u64);
            }
        }
        Self {
            width,
            height,
            seed,
            duty: vec![[0.0; 8]; cells],
            styles: vec![Style::default(); cells],
            static_mask: vec![0u8; cells],
            accumulator,
            frame_index: 0,
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }

    /// Sets all eight target subcell duty cycles for one terminal cell.
    ///
    /// Non-finite values become zero; finite values clamp to `0..=1`.
    pub fn set_cell_duty(&mut self, x: u16, y: u16, duty: [f32; 8]) -> bool {
        let Some(index) = self.index(x, y) else {
            return false;
        };
        self.duty[index] = duty.map(sanitize_duty);
        self.static_mask[index] = threshold_mask(&self.duty[index]);
        true
    }

    pub fn cell_duty(&self, x: u16, y: u16) -> Option<[f32; 8]> {
        self.index(x, y).map(|i| self.duty[i])
    }

    /// Sets a stable foreground/background style for one temporal cell.
    ///
    /// The recommended residual path keeps this style unchanged across phases
    /// and modulates only the glyph mask, minimizing ANSI style churn.
    pub fn set_cell_style(&mut self, x: u16, y: u16, style: Style) -> bool {
        let Some(index) = self.index(x, y) else {
            return false;
        };
        self.styles[index] = style;
        true
    }

    /// Applies an RGB projection produced by `project_rgb_subcells`.
    pub fn set_cell_projection(
        &mut self,
        x: u16,
        y: u16,
        projection: TemporalCellProjection,
    ) -> bool {
        let Some(index) = self.index(x, y) else {
            return false;
        };
        self.duty[index] = projection.duty.map(sanitize_duty);
        self.styles[index] = projection.style;
        // Store the projector's SSE-optimal mask as the authoritative static
        // fallback, rather than re-deriving it by thresholding duty at 0.5 (which
        // is provably equal only away from exact ties, where the two break the
        // tie in opposite directions).
        self.static_mask[index] = projection.static_mask;
        true
    }

    /// Sets both the target duty cycles and stable style for one cell, keeping
    /// the accumulator (equivalent to [`ResetPolicy::Keep`]).
    ///
    /// Use [`TemporalBrailleField::set_cell_target_with`] with
    /// [`ResetPolicy::Reset`] when the cell's underlying content changes, so the
    /// previous target's accumulated sigma-delta error does not leak forward.
    pub fn set_cell_target(&mut self, x: u16, y: u16, duty: [f32; 8], style: Style) -> bool {
        self.set_cell_target_with(x, y, duty, style, ResetPolicy::Keep)
    }

    /// Sets a cell's target and stable style, choosing whether to preserve or
    /// reseed the accumulator via `reset`.
    pub fn set_cell_target_with(
        &mut self,
        x: u16,
        y: u16,
        duty: [f32; 8],
        style: Style,
        reset: ResetPolicy,
    ) -> bool {
        let Some(index) = self.index(x, y) else {
            return false;
        };
        self.duty[index] = duty.map(sanitize_duty);
        self.styles[index] = style;
        self.static_mask[index] = threshold_mask(&self.duty[index]);
        if reset == ResetPolicy::Reset {
            self.reseed_accumulator(index);
        }
        true
    }

    /// Snaps a cell's duty to its stored static-mask baseline, so residual
    /// modulation produces zero flips and the cell holds its exact static frame.
    /// Used by the safety controller to freeze cells whose emitted luminance swing
    /// is too large to modulate. Returns `false` if `(x, y)` is out of bounds.
    pub fn freeze_cell_to_static(&mut self, x: u16, y: u16) -> bool {
        let Some(index) = self.index(x, y) else {
            return false;
        };
        let mask = self.static_mask[index];
        for (bit, d) in self.duty[index].iter_mut().enumerate() {
            *d = if mask & (1u8 << bit) != 0 { 1.0 } else { 0.0 };
        }
        true
    }

    pub fn cell_style(&self, x: u16, y: u16) -> Option<Style> {
        self.index(x, y).map(|i| self.styles[i])
    }

    pub fn clear(&mut self) {
        self.duty.fill([0.0; 8]);
        self.static_mask.fill(0);
    }

    /// Reseeds one cell's accumulator to its deterministic decorrelated phase,
    /// erasing accumulated residual without changing the cell's target or style.
    ///
    /// Call this when the cell's underlying content changes but the target values
    /// happen to be reused, so stale sigma-delta error does not smear the old
    /// content into the new one. Returns `false` if `(x, y)` is out of bounds.
    pub fn reset_cell(&mut self, x: u16, y: u16) -> bool {
        match self.index(x, y) {
            Some(index) => {
                self.reseed_accumulator(index);
                true
            }
            None => false,
        }
    }

    /// Reseeds every cell overlapping the rectangle `(x, y, w, h)`, clamped to the
    /// field, and returns how many cells were reset. Intended for invalidating a
    /// moved or repainted region (e.g. a scrolled viewport) in one call.
    pub fn reset_region(&mut self, x: u16, y: u16, w: u16, h: u16) -> usize {
        let x1 = x.min(self.width);
        let y1 = y.min(self.height);
        let x2 = x.saturating_add(w).min(self.width);
        let y2 = y.saturating_add(h).min(self.height);
        let mut reset = 0;
        for cy in y1..y2 {
            for cx in x1..x2 {
                if let Some(index) = self.index(cx, cy) {
                    self.reseed_accumulator(index);
                    reset += 1;
                }
            }
        }
        reset
    }

    /// Reseeds all accumulators, erasing residual across the whole field while
    /// leaving targets, styles and the frame index untouched.
    pub fn reset_all(&mut self) {
        for index in 0..self.accumulator.len() {
            self.reseed_accumulator(index);
        }
    }

    fn reseed_accumulator(&mut self, index: usize) {
        let seed = self.seed;
        for (dot, phase) in self.accumulator[index].iter_mut().enumerate() {
            *phase = unit_hash(seed, index as u64, dot as u64);
        }
    }

    /// A deterministic static control: dots at or above 50% duty are on.
    pub fn static_surface(&self, style: Style, mode: SubcellGlyphMode) -> Surface {
        let masks = self.static_masks();
        self.surface_from_masks_uniform(masks, style, mode)
    }

    /// Static control using each cell's stable foreground/background style.
    pub fn static_styled_surface(&self, mode: SubcellGlyphMode) -> Surface {
        self.surface_from_masks_styled(self.static_masks(), mode)
    }

    /// Advances one temporal phase and returns one ordinary LibGibson surface
    /// using one uniform style.
    pub fn advance(&mut self, style: Style, mode: SubcellGlyphMode) -> Surface {
        let masks = self.advance_masks();
        self.surface_from_masks_uniform(masks, style, mode)
    }

    /// Advances one temporal phase using each cell's stable style.
    ///
    /// This is the preferred path for image experiments: choose per-cell fg/bg
    /// once, then let most frames change only the glyph mask.
    pub fn advance_styled(&mut self, mode: SubcellGlyphMode) -> Surface {
        let masks = self.advance_masks();
        self.surface_from_masks_styled(masks, mode)
    }

    /// Advances one temporal phase as a **bounded residual correction** over the
    /// stored static masks, using each cell's stable style.
    ///
    /// Unlike [`Self::advance_styled`], which sigma-deltas the full duty from an all-off
    /// baseline, this holds each dot at its static-mask value and only flips it to
    /// nudge the time-average toward the continuous `duty` target. Most frames
    /// therefore equal the static fallback, so the modulation is a small
    /// correction over an already-good frame — the residual-dithering path Fable's
    /// analysis favours. The time-average of each dot still converges to its duty.
    ///
    /// When the static mask is the duty thresholded at 0.5 (the default for
    /// directly-set duties), each dot's per-frame flip probability is at most 50%.
    pub fn advance_residual_styled(&mut self, mode: SubcellGlyphMode) -> Surface {
        let masks = self.advance_residual_masks();
        self.surface_from_masks_styled(masks, mode)
    }

    fn static_masks(&self) -> Vec<u8> {
        self.static_mask.clone()
    }

    fn advance_masks(&mut self) -> Vec<u8> {
        let mut masks = Vec::with_capacity(self.duty.len());
        for (targets, accumulators) in self.duty.iter().zip(self.accumulator.iter_mut()) {
            let mut mask = 0u8;
            for bit in 0..8 {
                let target = targets[bit];
                let acc = &mut accumulators[bit];
                *acc += target;
                if *acc >= 1.0 {
                    mask |= 1u8 << bit;
                    *acc -= 1.0;
                }
            }
            masks.push(mask);
        }
        self.frame_index = self.frame_index.wrapping_add(1);
        masks
    }

    fn advance_residual_masks(&mut self) -> Vec<u8> {
        let mut masks = Vec::with_capacity(self.duty.len());
        for ((targets, &baseline), acc) in self
            .duty
            .iter()
            .zip(self.static_mask.iter())
            .zip(self.accumulator.iter_mut())
        {
            let mut mask = 0u8;
            for bit in 0..8 {
                let baseline_on = baseline & (1u8 << bit) != 0;
                let residual = targets[bit] - if baseline_on { 1.0 } else { 0.0 };
                acc[bit] += residual;
                let on = if baseline_on {
                    // Baseline on (residual <= 0): flip OFF when the accumulator
                    // crosses -1, otherwise hold the static dot.
                    if acc[bit] <= -1.0 {
                        acc[bit] += 1.0;
                        false
                    } else {
                        true
                    }
                } else {
                    // Baseline off (residual >= 0): flip ON when the accumulator
                    // crosses +1, otherwise hold the static dot.
                    if acc[bit] >= 1.0 {
                        acc[bit] -= 1.0;
                        true
                    } else {
                        false
                    }
                };
                if on {
                    mask |= 1u8 << bit;
                }
            }
            masks.push(mask);
        }
        self.frame_index = self.frame_index.wrapping_add(1);
        masks
    }

    fn surface_from_masks_uniform(
        &self,
        masks: Vec<u8>,
        style: Style,
        mode: SubcellGlyphMode,
    ) -> Surface {
        self.surface_from_masks_with(masks, mode, |_| style)
    }

    fn surface_from_masks_styled(&self, masks: Vec<u8>, mode: SubcellGlyphMode) -> Surface {
        self.surface_from_masks_with(masks, mode, |index| self.styles[index])
    }

    fn surface_from_masks_with(
        &self,
        masks: Vec<u8>,
        mode: SubcellGlyphMode,
        style_at: impl Fn(usize) -> Style,
    ) -> Surface {
        let mut surface = Surface::new(self.width, self.height);
        if self.width == 0 {
            return surface;
        }
        for (index, mask) in masks.into_iter().enumerate() {
            let x = (index % self.width as usize) as u16;
            let y = (index / self.width as usize) as u16;
            let style = style_at(index);
            let cell = match mode.subcell_glyph(mask) {
                Some(ch) => Cell::new(Glyph::from_char(ch), style),
                None => Cell::space(style),
            };
            surface.set_cell(x, y, cell);
        }
        surface
    }

    fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x < self.width && y < self.height {
            Some((y as usize) * self.width as usize + x as usize)
        } else {
            None
        }
    }
}

/// Rec.709 relative luminance of a color in linear light (`0..=1`), or `None`
/// when the exact emitted luminance is not known here — any non-RGB color (named,
/// indexed, or reset). Temporal cells produced by [`project_rgb_subcells`] always
/// carry RGB colors, so the modulated path always has a defined luminance; `None`
/// is treated conservatively (frozen to static) by [`TemporalDisplayProcessor`].
fn color_linear_luminance(color: Color) -> Option<f32> {
    match color {
        Color::Rgb(r, g, b) => {
            let lin = rgb8_to_linear([r, g, b]);
            Some(0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2])
        }
        _ => None,
    }
}

/// Worst-case instantaneous linear-light luminance swing when one dot of a cell
/// with `style` flips between background and foreground. `None` means at least one
/// color's emitted luminance is unknown (non-RGB or unset), which the processor
/// treats as unbounded and freezes to static.
fn style_luminance_swing(style: Style) -> Option<f32> {
    let fg = color_linear_luminance(style.fg?)?;
    let bg = color_linear_luminance(style.bg?)?;
    Some((fg - bg).abs())
}

/// A snapshot of [`TemporalDisplayProcessor`]'s current realization decision.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemporalDiagnostics {
    /// The top-level profile/reduced-motion gate (independent of the cadence hold).
    pub gate: TemporalGate,
    /// Whether the most recent `advance` emitted temporal output (`true`) or the
    /// static fallback (`false`).
    pub modulating: bool,
    /// Cells eligible to modulate (emitted swing within the depth cap).
    pub modulatable_cells: usize,
    /// Cells frozen to static because their emitted swing exceeds the cap or is
    /// unknown.
    pub frozen_cells: usize,
    /// Worst per-cell emitted luminance swing over the target (`0` if none known).
    pub worst_cell_swing: f32,
    /// Mean emitted (post-quantization) static RMSE of the current target.
    pub mean_emitted_static_rmse: f32,
    /// Frames remaining in the cadence-degradation hysteresis hold (`0` = healthy).
    pub degraded_hold_frames: u32,
}

/// Frames of forced-static hysteresis after any observed missed deadline, so a
/// brief cadence stall cannot cause rapid temporal/static toggling (itself a
/// luminance step).
const CADENCE_HYSTERESIS_FRAMES: u32 = 30;

/// Experimental high-level temporal realization: static-first, safety-gated and
/// cadence-aware.
///
/// It owns a static projection and a residual modulator and emits ordinary
/// [`Surface`] frames — there is **no** second renderer. The default and the
/// fallback is the static two-color image. Residual temporal modulation runs only
/// when all of the following hold: a credible measured [`PresentationProfile`]
/// passes the profile gate, local cadence is healthy (no recent missed
/// deadlines), reduced-motion is off, and — evaluated *per cell* — the emitted
/// foreground/background luminance swing is within the policy depth cap. Cells
/// exceeding the cap (or whose colors have unknown luminance) are frozen to their
/// static frame, so a high-contrast cell can never strobe regardless of any
/// requested depth.
pub struct TemporalDisplayProcessor {
    width: u16,
    height: u16,
    mode: SubcellGlyphMode,
    field: TemporalBrailleField,
    profile: PresentationProfile,
    policy: TemporalSafetyPolicy,
    reduced_motion: bool,
    has_target: bool,
    mean_emitted_static_rmse: f32,
    worst_cell_swing: f32,
    modulatable_cells: usize,
    frozen_cells: usize,
    degraded_hold: u32,
    last_modulating: bool,
    last_gate: TemporalGate,
}

impl TemporalDisplayProcessor {
    pub fn new(width: u16, height: u16, mode: SubcellGlyphMode, seed: u64) -> Self {
        Self {
            width,
            height,
            mode,
            field: TemporalBrailleField::new(width, height, seed),
            profile: PresentationProfile::unmeasured(),
            policy: TemporalSafetyPolicy::default(),
            reduced_motion: false,
            has_target: false,
            mean_emitted_static_rmse: 0.0,
            worst_cell_swing: 0.0,
            modulatable_cells: 0,
            frozen_cells: 0,
            degraded_hold: 0,
            last_modulating: false,
            last_gate: TemporalGate::Unmeasured,
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    /// Sets the externally measured presentation profile. Takes effect on the next
    /// [`Self::advance`].
    pub fn set_profile(&mut self, profile: PresentationProfile) {
        self.profile = profile;
    }

    /// Sets the safety policy. Per-cell freeze decisions are recomputed on the
    /// next [`Self::set_target_image`], so set the policy before the target.
    pub fn set_policy(&mut self, policy: TemporalSafetyPolicy) {
        self.policy = policy;
    }

    /// Sets the accessibility reduced-motion preference. When `true`, output is
    /// always the static fallback.
    pub fn set_reduced_motion(&mut self, reduced: bool) {
        self.reduced_motion = reduced;
    }

    /// Projects a logical Braille RGB image (`2*width` by `4*height` samples) as
    /// the new target, installs it, and enforces the per-cell emitted-swing safety
    /// bound: any cell whose fg/bg luminance swing exceeds the cap (or is unknown)
    /// is snapped to its static baseline so it can only ever show its static frame.
    /// `reset` controls whether accumulators are reseeded (use [`ResetPolicy::Reset`]
    /// for genuinely new content).
    pub fn set_target_image(&mut self, sample: impl Fn(u16, u16) -> [u8; 3], reset: ResetPolicy) {
        let projection = project_braille_image(self.width, self.height, sample);
        self.mean_emitted_static_rmse = projection.mean_emitted_static_rmse();
        projection.install_into(&mut self.field, reset);

        let cap = self.policy.max_luminance_depth;
        let (mut modulatable, mut frozen, mut worst) = (0usize, 0usize, 0.0f32);
        for y in 0..self.height {
            for x in 0..self.width {
                let style = self.field.cell_style(x, y).unwrap_or_default();
                let swing = style_luminance_swing(style);
                if let Some(s) = swing {
                    worst = worst.max(s);
                }
                if matches!(swing, Some(s) if s <= cap) {
                    modulatable += 1;
                } else {
                    frozen += 1;
                    self.field.freeze_cell_to_static(x, y);
                }
            }
        }
        self.modulatable_cells = modulatable;
        self.frozen_cells = frozen;
        self.worst_cell_swing = worst;
        self.has_target = true;
    }

    /// The top-level profile/reduced-motion gate (independent of the cadence hold).
    pub fn gate(&self) -> TemporalGate {
        self.policy.gate_profile(self.profile, self.reduced_motion)
    }

    /// The always-valid static realization of the current target.
    pub fn static_fallback(&self) -> Surface {
        self.field.static_styled_surface(self.mode)
    }

    /// Produces the next frame.
    ///
    /// `missed_periods` is the scheduler's most recent phase-locked missed-deadline
    /// count (`0` under completion-relative pacing). A nonzero value trips a
    /// hysteresis hold that forces the static fallback for a fixed number of
    /// frames (30), preventing rapid temporal/static toggling.
    /// Returns the static fallback whenever the gate is not `Enabled`, the cadence
    /// hold is active, or no cell is eligible to modulate; otherwise the
    /// residual-modulated frame (in which frozen cells still render static).
    pub fn advance(&mut self, missed_periods: u32) -> Surface {
        if missed_periods > 0 {
            self.degraded_hold = CADENCE_HYSTERESIS_FRAMES;
        } else if self.degraded_hold > 0 {
            self.degraded_hold -= 1;
        }

        let gate = self.policy.gate_profile(self.profile, self.reduced_motion);
        self.last_gate = gate;
        let modulate = gate == TemporalGate::Enabled
            && self.has_target
            && self.modulatable_cells > 0
            && self.degraded_hold == 0;
        self.last_modulating = modulate;
        if modulate {
            self.field.advance_residual_styled(self.mode)
        } else {
            self.static_fallback()
        }
    }

    /// A snapshot of the current realization decision and target statistics.
    pub fn diagnostics(&self) -> TemporalDiagnostics {
        TemporalDiagnostics {
            gate: self.last_gate,
            modulating: self.last_modulating,
            modulatable_cells: self.modulatable_cells,
            frozen_cells: self.frozen_cells,
            worst_cell_swing: self.worst_cell_swing,
            mean_emitted_static_rmse: self.mean_emitted_static_rmse,
            degraded_hold_frames: self.degraded_hold,
        }
    }

    /// Reseeds all residual accumulators (e.g. after a discontinuity).
    pub fn reset(&mut self) {
        self.field.reset_all();
    }
}

fn sanitize_duty(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Deterministic static mask for a duty array: dots at or above 50% are on.
///
/// This is the sensible fallback for directly-set duties. Projections instead
/// store the SSE-optimal mask from [`project_rgb_subcells`], which coincides
/// with this threshold except at exact 0.5 ties (see the projector's docs).
fn threshold_mask(duty: &[f32; 8]) -> u8 {
    duty.iter().enumerate().fold(
        0u8,
        |mask, (bit, d)| {
            if *d >= 0.5 {
                mask | (1u8 << bit)
            } else {
                mask
            }
        },
    )
}

/// Deterministic SplitMix-style hash mapped to `[0, 1)`.
fn unit_hash(seed: u64, cell: u64, dot: u64) -> f32 {
    let mut z =
        seed ^ cell.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ dot.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    ((z >> 40) as f32) / ((1u32 << 24) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_projector_exactly_handles_binary_black_white_pattern() {
        let mut target = [[0u8; 3]; 8];
        for (i, pixel) in target.iter_mut().enumerate() {
            if i % 2 == 1 {
                *pixel = [255, 255, 255];
            }
        }
        let projection = project_rgb_subcells(target);
        assert!(projection.static_rmse < 1.0e-6);
        assert!(projection.line_rmse < 1.0e-6);
        let dark = projection.duty[0];
        let light = projection.duty[1];
        assert!(
            (dark - light).abs() > 1.0 - 1.0e-5,
            "the two binary colors must land on opposite segment endpoints"
        );
        for (i, duty) in projection.duty.iter().enumerate() {
            let expected = if i % 2 == 1 { light } else { dark };
            assert!(
                (duty - expected).abs() < 1.0e-5,
                "fg/bg labels are symmetric, but equal target colors must share an endpoint"
            );
        }
    }

    #[test]
    fn rgb_projector_temporal_line_never_worse_than_static_partition() {
        let target = [
            [0, 0, 0],
            [32, 20, 10],
            [64, 70, 80],
            [96, 100, 110],
            [128, 125, 120],
            [160, 170, 180],
            [210, 205, 200],
            [255, 255, 255],
        ];
        let projection = project_rgb_subcells(target);
        assert!(projection.line_rmse <= projection.static_rmse + 1.0e-6);
        assert!(projection.duty.iter().all(|d| (0.0..=1.0).contains(d)));
    }

    #[test]
    fn uniform_rgb_projector_uses_stable_background_without_temporal_churn() {
        let projection = project_rgb_subcells([[73, 109, 181]; 8]);
        assert_eq!(projection.static_mask, 0);
        assert_eq!(projection.duty, [0.0; 8]);
        assert!(projection.static_rmse < 1.0e-6);
        assert!(projection.line_rmse < 1.0e-6);
    }

    #[test]
    fn conservative_gate_rejects_unmeasured_low_rate_and_excess_depth() {
        let policy = TemporalSafetyPolicy::default();
        assert_eq!(
            policy.gate_luminance(PresentationProfile::unmeasured(), 0.02, false),
            TemporalGate::Unmeasured
        );
        assert_eq!(
            policy.gate_luminance(PresentationProfile::measured(60.0, 1.0, 0.1), 0.02, false),
            TemporalGate::CadenceTooLow
        );
        assert_eq!(
            policy.gate_luminance(PresentationProfile::measured(120.0, 0.80, 0.1), 0.02, false),
            TemporalGate::SurvivalTooLow
        );
        assert_eq!(
            policy.gate_luminance(PresentationProfile::measured(120.0, 0.99, 0.1), 0.20, false),
            TemporalGate::DepthTooHigh
        );
        assert_eq!(
            policy.gate_luminance(PresentationProfile::measured(120.0, 0.99, 0.1), 0.02, true),
            TemporalGate::ReducedMotion
        );
        assert_eq!(
            policy.gate_luminance(PresentationProfile::measured(120.0, 0.99, 0.1), 0.02, false),
            TemporalGate::Enabled
        );
    }

    #[test]
    fn sigma_delta_mean_converges_to_requested_duty() {
        let mut field = TemporalBrailleField::new(1, 1, 7);
        assert!(field.set_cell_duty(0, 0, [0.25, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]));
        let mut on = 0usize;
        let frames = 400usize;
        for _ in 0..frames {
            let mask = field.advance_masks()[0];
            if mask & 1 != 0 {
                on += 1;
            }
        }
        assert!(
            (on as isize - 100).abs() <= 1,
            "25% duty should converge to 100/400 on frames, got {on}"
        );
    }

    #[test]
    fn residual_modulation_converges_to_duty_from_static_baseline() {
        // Cell with dot0 ON in the static baseline (duty 0.8 => flip OFF ~20% of
        // frames) and dot1 OFF in the baseline (duty 0.3 => flip ON ~30%).
        let mut field = TemporalBrailleField::new(1, 1, 5);
        let proj = TemporalCellProjection {
            style: Style::default(),
            duty: [0.8, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            static_mask: 0b0000_0001, // dot0 on, dot1 off
            static_rmse: 0.0,
            line_rmse: 0.0,
            emitted_static_rmse: 0.0,
        };
        assert!(field.set_cell_projection(0, 0, proj));

        let frames = 1000usize;
        let (mut on0, mut on1) = (0usize, 0usize);
        for _ in 0..frames {
            let m = field.advance_residual_masks()[0];
            if m & 0b0000_0001 != 0 {
                on0 += 1;
            }
            if m & 0b0000_0010 != 0 {
                on1 += 1;
            }
        }
        let f = frames as f32;
        assert!(
            (on0 as f32 / f - 0.8).abs() < 0.02,
            "dot0 time-average should converge to duty 0.8, got {on0}/{frames}"
        );
        assert!(
            (on1 as f32 / f - 0.3).abs() < 0.02,
            "dot1 time-average should converge to duty 0.3, got {on1}/{frames}"
        );
        // Residual path stays near the static baseline: dot0 is ON most frames
        // (its baseline), dot1 is OFF most frames.
        assert!(
            on0 as f32 / f > 0.5,
            "dot0 should hold its ON baseline most frames"
        );
        assert!(
            (frames - on1) as f32 / f > 0.5,
            "dot1 should hold its OFF baseline most frames"
        );
    }

    #[test]
    fn zero_and_full_duty_are_temporally_stable() {
        let mut field = TemporalBrailleField::new(1, 1, 11);
        assert!(field.set_cell_duty(0, 0, [0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0]));
        for _ in 0..64 {
            let mask = field.advance_masks()[0];
            assert_eq!(mask, 0b1010_1010);
        }
    }

    #[test]
    fn seeded_phases_decorrelate_uniform_half_duty_field() {
        let mut field = TemporalBrailleField::new(8, 4, 0xC0FFEE);
        for y in 0..field.height() {
            for x in 0..field.width() {
                field.set_cell_duty(x, y, [0.5; 8]);
            }
        }
        let masks = field.advance_masks();
        assert!(
            masks.windows(2).any(|w| w[0] != w[1]),
            "uniform targets should not start every cell in the same temporal phase"
        );
    }

    #[test]
    fn advance_emits_ordinary_surface_cells() {
        let mut field = TemporalBrailleField::new(2, 1, 1);
        field.set_cell_duty(0, 0, [1.0; 8]);
        field.set_cell_duty(1, 0, [0.0; 8]);
        let surface = field.advance(Style::default(), SubcellGlyphMode::Braille2x4);
        assert_eq!(surface.width, 2);
        assert_eq!(surface.height, 1);
        assert_ne!(surface.get(0, 0).unwrap().glyph.grapheme.as_str(), " ");
        assert_eq!(surface.get(1, 0).unwrap().glyph.grapheme.as_str(), " ");
    }

    #[test]
    fn static_surface_uses_stored_optimal_mask_not_duty_threshold() {
        use crate::cell::Color;
        // A projection whose optimal static mask differs from thresholding its
        // duty at 0.5: the field must emit the stored optimal mask, proving it
        // keeps the authoritative fallback instead of re-deriving from duty.
        let mut field = TemporalBrailleField::new(1, 1, 0);
        let projection = TemporalCellProjection {
            style: Style::default()
                .fg(Color::Rgb(255, 255, 255))
                .bg(Color::Rgb(0, 0, 0)),
            duty: [0.0; 8],           // threshold@0.5 => blank mask 0
            static_mask: 0b1111_0000, // but the optimal static mask is non-blank
            static_rmse: 0.0,
            line_rmse: 0.0,
            emitted_static_rmse: 0.0,
        };
        assert!(field.set_cell_projection(0, 0, projection));

        let surface = field.static_styled_surface(SubcellGlyphMode::Braille2x4);
        let glyph = surface.get(0, 0).unwrap().glyph.grapheme.clone();
        let expected = SubcellGlyphMode::Braille2x4
            .subcell_glyph(0b1111_0000)
            .expect("non-empty mask realizes a glyph");
        assert_eq!(
            glyph.as_str(),
            expected.to_string(),
            "static fallback must emit the stored optimal mask, not the duty@0.5 blank"
        );
    }

    #[test]
    fn batch_projects_logical_braille_image_with_exact_two_color_cell() {
        // One cell: left column white, right column black — perfectly two-color.
        let proj = project_braille_image(
            1,
            1,
            |lx, _ly| {
                if lx == 0 {
                    [255, 255, 255]
                } else {
                    [0, 0, 0]
                }
            },
        );
        assert_eq!(proj.width(), 1);
        assert_eq!(proj.height(), 1);
        let cell = proj.cell(0, 0).expect("cell present");
        assert!(
            cell.emitted_static_rmse < 1e-6,
            "perfectly two-color cell must have ~zero emitted error"
        );
        // Left column dots are indices {0,1,2,6}; right column {3,4,5,7}. The
        // optimal mask groups the columns, so it is one of those two (complement
        // pair), each with popcount 4.
        assert!(
            cell.static_mask == 0b1011_1000 || cell.static_mask == 0b0100_0111,
            "mask must separate the two columns, got {:#010b}",
            cell.static_mask
        );
        // Static surface renders the stored mask as a Braille glyph.
        let surface = proj.static_surface(SubcellGlyphMode::Braille2x4);
        let ch = surface
            .get(0, 0)
            .unwrap()
            .glyph
            .grapheme
            .chars()
            .next()
            .unwrap();
        let bits = ch as u32 - 0x2800;
        assert_eq!(
            bits, cell.static_mask as u32,
            "surface must emit the stored mask"
        );
        assert_eq!(bits.count_ones(), 4, "four dots per color group");
    }

    #[test]
    fn batch_install_into_field_round_trips_static_fallback() {
        let proj = project_braille_image(3, 2, |lx, ly| [(lx * 20) as u8, (ly * 20) as u8, 128]);
        let mut field = TemporalBrailleField::new(3, 2, 1);
        assert!(proj.install_into(&mut field, ResetPolicy::Reset));

        // The field's static fallback must match the projection's static surface
        // exactly (glyph + style), proving install_into preserved the optimal
        // masks and stable palettes.
        let a = proj.static_surface(SubcellGlyphMode::Braille2x4);
        let b = field.static_styled_surface(SubcellGlyphMode::Braille2x4);
        for y in 0..2 {
            for x in 0..3 {
                assert_eq!(
                    a.get(x, y).unwrap().glyph.grapheme,
                    b.get(x, y).unwrap().glyph.grapheme
                );
                assert_eq!(a.get(x, y).unwrap().style, b.get(x, y).unwrap().style);
            }
        }

        // Dimension mismatch is rejected without modifying the field.
        let mut wrong = TemporalBrailleField::new(2, 2, 1);
        assert!(!proj.install_into(&mut wrong, ResetPolicy::Keep));
    }

    #[test]
    fn processor_defaults_to_static_when_unmeasured() {
        let mut p = TemporalDisplayProcessor::new(1, 1, SubcellGlyphMode::Braille2x4, 7);
        p.set_target_image(
            |_lx, ly| {
                let v = 100u8 + 3 * ly as u8;
                [v, v, v]
            },
            ResetPolicy::Reset,
        );
        let frame = p.advance(0);
        let d = p.diagnostics();
        assert_eq!(d.gate, TemporalGate::Unmeasured);
        assert!(!d.modulating, "an unmeasured profile must stay static");
        let stat = p.static_fallback();
        assert_eq!(
            frame.get(0, 0).unwrap().glyph.grapheme,
            stat.get(0, 0).unwrap().glyph.grapheme
        );
    }

    #[test]
    fn processor_modulates_low_swing_cells_and_freezes_high_swing_cells() {
        let mut p = TemporalDisplayProcessor::new(2, 1, SubcellGlyphMode::Braille2x4, 3);
        p.set_profile(PresentationProfile::measured(120.0, 0.98, 0.2));
        p.set_target_image(
            |lx, ly| {
                if lx < 2 {
                    // cell 0: a low-contrast gray gradient -> small luminance swing
                    // with fractional duty (so there is residual to modulate).
                    let v = 100u8 + 3 * ly as u8;
                    [v, v, v]
                } else if lx == 2 {
                    [255, 255, 255] // cell 1: white vs black column -> ~full swing
                } else {
                    [0, 0, 0]
                }
            },
            ResetPolicy::Reset,
        );
        let d = p.diagnostics();
        assert_eq!(d.modulatable_cells, 1, "low-swing cell is modulatable");
        assert_eq!(d.frozen_cells, 1, "white/black cell is frozen");
        assert!(
            d.worst_cell_swing > 0.9,
            "worst swing tracks the white/black cell, got {}",
            d.worst_cell_swing
        );
        assert_eq!(p.gate(), TemporalGate::Enabled);

        // The frozen high-contrast cell must NEVER strobe (evil-morty repro guard);
        // the low-swing cell should actually vary across frames.
        let frozen_glyph = p
            .static_fallback()
            .get(1, 0)
            .unwrap()
            .glyph
            .grapheme
            .clone();
        let mut cell0_glyphs = std::collections::HashSet::new();
        for _ in 0..200 {
            let f = p.advance(0);
            assert_eq!(
                f.get(1, 0).unwrap().glyph.grapheme,
                frozen_glyph,
                "frozen high-contrast cell must hold static every frame"
            );
            cell0_glyphs.insert(f.get(0, 0).unwrap().glyph.grapheme.clone());
        }
        assert!(
            cell0_glyphs.len() > 1,
            "the low-swing cell should modulate (vary over frames)"
        );
    }

    #[test]
    fn processor_reduced_motion_forces_static() {
        let mut p = TemporalDisplayProcessor::new(1, 1, SubcellGlyphMode::Braille2x4, 1);
        p.set_profile(PresentationProfile::measured(120.0, 0.99, 0.1));
        p.set_reduced_motion(true);
        p.set_target_image(
            |_lx, ly| {
                let v = 100u8 + 3 * ly as u8;
                [v, v, v]
            },
            ResetPolicy::Reset,
        );
        assert_eq!(p.gate(), TemporalGate::ReducedMotion);
        let f = p.advance(0);
        assert!(!p.diagnostics().modulating);
        let stat = p.static_fallback();
        assert_eq!(
            f.get(0, 0).unwrap().glyph.grapheme,
            stat.get(0, 0).unwrap().glyph.grapheme
        );
    }

    #[test]
    fn processor_cadence_hysteresis_holds_static_after_missed_period() {
        let mut p = TemporalDisplayProcessor::new(1, 1, SubcellGlyphMode::Braille2x4, 2);
        p.set_profile(PresentationProfile::measured(120.0, 0.99, 0.1));
        p.set_target_image(
            |_lx, ly| {
                let v = 100u8 + 3 * ly as u8;
                [v, v, v]
            },
            ResetPolicy::Reset,
        );
        p.advance(0);
        assert!(
            p.diagnostics().modulating,
            "healthy cadence should modulate"
        );

        // A missed deadline trips the hold: static now and through the window.
        p.advance(1);
        assert!(
            !p.diagnostics().modulating,
            "missed period must force static"
        );
        assert!(p.diagnostics().degraded_hold_frames > 0);
        p.advance(0);
        assert!(!p.diagnostics().modulating, "hysteresis holds static");

        // After enough clean frames the hold clears and modulation resumes.
        for _ in 0..CADENCE_HYSTERESIS_FRAMES {
            p.advance(0);
        }
        assert!(
            p.diagnostics().modulating,
            "cadence recovered => modulating again"
        );
        assert_eq!(p.diagnostics().degraded_hold_frames, 0);
    }

    #[test]
    fn projector_127_representatives_match_full_254_scan_fit() {
        // Slow oracle: the full 254-mask exact scan (both groups non-empty),
        // direct-distance SSE. The shipped projector evaluates only the 127
        // complement-pair representatives; the resulting fit (best SSE, hence
        // static_rmse) must be identical, since a mask and its complement share
        // an SSE.
        fn reference_static_rmse(target: [[u8; 3]; 8]) -> f32 {
            let linear = target.map(rgb8_to_linear);
            let mut best = f32::INFINITY;
            for mask in 1u16..=254 {
                let mask = mask as u8;
                let (fg, bg) = partition_centroids(&linear, mask);
                let mut sse = 0.0f32;
                for (i, px) in linear.iter().enumerate() {
                    let c = if mask & (1u8 << i) != 0 { fg } else { bg };
                    sse += rgb_distance_squared(*px, c);
                }
                if sse < best {
                    best = sse;
                }
            }
            (best / 24.0).sqrt()
        }

        // Deterministic xorshift over 2000 tiles.
        let mut state = 0x1234_5678_9abc_def0u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        };
        for _ in 0..2000 {
            let mut t = [[0u8; 3]; 8];
            for px in t.iter_mut() {
                for c in px.iter_mut() {
                    *c = next();
                }
            }
            let got = project_rgb_subcells(t).static_rmse;
            let want = reference_static_rmse(t);
            assert!(
                (got - want).abs() <= 1e-5 * (1.0 + want),
                "127-rep fit {got} != full 254-scan fit {want} for {t:?}"
            );
        }
    }

    #[test]
    fn projector_emitted_error_is_at_least_ideal_error() {
        // Perfect binary black/white: ideal and emitted error are both ~0.
        let mut bw = [[0u8; 3]; 8];
        for (i, p) in bw.iter_mut().enumerate() {
            if i % 2 == 1 {
                *p = [255, 255, 255];
            }
        }
        let proj = project_rgb_subcells(bw);
        assert!(proj.static_rmse < 1e-6);
        assert!(proj.emitted_static_rmse < 1e-6);

        // Non-8-bit-representable centroids: quantization can only add error, so
        // the emitted RMSE must be >= the ideal RMSE, and finite.
        let target = [
            [10, 20, 30],
            [200, 130, 60],
            [15, 240, 90],
            [77, 88, 99],
            [123, 45, 210],
            [5, 5, 6],
            [250, 249, 1],
            [130, 131, 132],
        ];
        let proj = project_rgb_subcells(target);
        assert!(proj.emitted_static_rmse.is_finite());
        assert!(
            proj.emitted_static_rmse >= proj.static_rmse - 1e-6,
            "emitted {} must be >= ideal {}",
            proj.emitted_static_rmse,
            proj.static_rmse
        );
    }

    #[test]
    fn reset_erases_prior_residual_while_keep_retains_it() {
        let seed = 42;
        let new_target = [0.3; 8];
        let old_target = [1.0, 0.0, 0.7, 0.0, 0.9, 0.0, 0.2, 0.0];

        // Reference field showing the new target from a clean start.
        let mut fresh = TemporalBrailleField::new(1, 1, seed);
        fresh.set_cell_duty(0, 0, new_target);

        // Showed different content, accumulated residual, then switched with Reset.
        let mut reset = TemporalBrailleField::new(1, 1, seed);
        reset.set_cell_duty(0, 0, old_target);
        for _ in 0..5 {
            reset.advance_masks();
        }
        assert!(reset.set_cell_target_with(0, 0, new_target, Style::default(), ResetPolicy::Reset));

        // Same history, switched with Keep.
        let mut keep = TemporalBrailleField::new(1, 1, seed);
        keep.set_cell_duty(0, 0, old_target);
        for _ in 0..5 {
            keep.advance_masks();
        }
        assert!(keep.set_cell_target_with(0, 0, new_target, Style::default(), ResetPolicy::Keep));

        // Structural: Reset restores exactly the fresh seeded phase; Keep drifted.
        assert_eq!(
            reset.accumulator, fresh.accumulator,
            "Reset must restore the deterministic seeded phase"
        );
        assert_ne!(
            keep.accumulator, fresh.accumulator,
            "Keep must retain the drifted residual from the prior target"
        );

        // Observable: the Reset field emits exactly the clean-start sequence.
        let fresh_seq: Vec<u8> = (0..12).map(|_| fresh.advance_masks()[0]).collect();
        let reset_seq: Vec<u8> = (0..12).map(|_| reset.advance_masks()[0]).collect();
        assert_eq!(
            reset_seq, fresh_seq,
            "a Reset target must reproduce a clean start, not smear old content"
        );
    }

    #[test]
    fn reset_all_restores_every_accumulator_to_fresh() {
        let seed = 9;
        let fresh = TemporalBrailleField::new(3, 2, seed);
        let mut used = TemporalBrailleField::new(3, 2, seed);
        for y in 0..2 {
            for x in 0..3 {
                used.set_cell_duty(x, y, [0.4; 8]);
            }
        }
        for _ in 0..7 {
            used.advance_masks();
        }
        assert_ne!(used.accumulator, fresh.accumulator);
        used.reset_all();
        assert_eq!(
            used.accumulator, fresh.accumulator,
            "reset_all must reseed every cell to its fresh phase"
        );
    }

    #[test]
    fn reset_region_clamps_to_bounds_and_counts() {
        let mut field = TemporalBrailleField::new(4, 3, 1);
        assert_eq!(field.reset_region(1, 1, 2, 2), 4, "2x2 block fully inside");
        assert_eq!(
            field.reset_region(3, 2, 10, 10),
            1,
            "overhang clamped to field"
        );
        assert_eq!(
            field.reset_region(9, 9, 2, 2),
            0,
            "fully outside resets nothing"
        );
        assert_eq!(
            field.reset_region(0, 0, 0, 0),
            0,
            "zero-size resets nothing"
        );
        assert!(
            !field.reset_cell(4, 0),
            "out-of-bounds reset_cell returns false"
        );
        assert!(field.reset_cell(3, 2), "in-bounds reset_cell returns true");
    }

    #[test]
    fn styled_path_preserves_per_cell_palette_while_masks_change() {
        use crate::cell::Color;

        let mut field = TemporalBrailleField::new(2, 1, 3);
        let left = Style::default()
            .fg(Color::Rgb(240, 240, 240))
            .bg(Color::Rgb(12, 12, 12));
        let right = Style::default()
            .fg(Color::Rgb(20, 180, 220))
            .bg(Color::Rgb(10, 20, 30));
        assert!(field.set_cell_target(0, 0, [0.5; 8], left));
        assert!(field.set_cell_target(1, 0, [0.5; 8], right));

        for _ in 0..4 {
            let surface = field.advance_styled(SubcellGlyphMode::Braille2x4);
            assert_eq!(surface.get(0, 0).unwrap().style, left);
            assert_eq!(surface.get(1, 0).unwrap().style, right);
        }
    }
}
