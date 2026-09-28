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

use crate::capability::{quantize_color, ColorDepth};
use crate::cell::{Cell, Color, Glyph, Style};
use crate::glyph::SubcellGlyphMode;
use crate::surface::Surface;

/// Measured properties of the terminal/display presentation path.
///
/// These are **presentation** measurements, not application emission settings.
/// LibGibson cannot infer them from PTY writes; use an external cadence beacon or
/// other measurement process and store the result as a profile.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
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
#[non_exhaustive]
pub struct TemporalSafetyPolicy {
    pub min_luminance_hz: f32,
    pub min_survival_rate: f32,
    pub max_luminance_depth: f32,
    /// Maximum tolerated 95th-percentile presentation jitter, expressed as a
    /// fraction of the presentation period (`jitter_p95_ms / (1000/hz)`). A
    /// normalized bound so a single policy is meaningful across cadences: at the
    /// default `0.5`, p95 jitter must stay under half a frame period.
    pub max_jitter_fraction: f32,
}

impl Default for TemporalSafetyPolicy {
    fn default() -> Self {
        Self {
            min_luminance_hz: 100.0,
            min_survival_rate: 0.90,
            max_luminance_depth: 0.10,
            max_jitter_fraction: 0.5,
        }
    }
}

/// Why an application should fall back to its static realization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TemporalGate {
    Enabled,
    ReducedMotion,
    Unmeasured,
    CadenceTooLow,
    SurvivalTooLow,
    JitterTooHigh,
    DepthTooHigh,
}

impl TemporalSafetyPolicy {
    /// Gates the **presentation** conditions only: reduced-motion preference, a
    /// measured profile, adequate cadence, adequate frame survival, and bounded
    /// presentation jitter. It does **not** bound modulation depth.
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
        // Normalized jitter: p95 inter-presentation jitter as a fraction of the
        // presentation period. Ragged timing smears the residual duty regardless
        // of nominal cadence, so an otherwise-healthy high-Hz / high-survival path
        // with jittery presentation is gated out. `presentation_hz` passed the
        // cadence check above (>= min_luminance_hz), so the period is finite and
        // positive whenever the default (or any positive) minimum is used.
        let period_ms = 1000.0 / profile.presentation_hz;
        if !profile.jitter_p95_ms.is_finite()
            || profile.jitter_p95_ms > self.max_jitter_fraction * period_ms
        {
            return TemporalGate::JitterTooHigh;
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
#[non_exhaustive]
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

/// 256-entry sRGB8 -> linear-light table, built once on first use. A `u8` channel
/// has only 256 possible values, so this is **bit-identical** to evaluating the
/// transfer function per sample (same expression, same order), but replaces the
/// per-sample `powf` with a table load. The projector linearizes 8 subpixels x 3
/// channels per cell, so this is its hottest arithmetic.
fn srgb8_to_linear_table() -> &'static [f32; 256] {
    static TABLE: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0.0f32; 256];
        for (i, entry) in table.iter_mut().enumerate() {
            let value = i as f32 / 255.0;
            *entry = if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            };
        }
        table
    })
}

fn rgb8_to_linear(rgb: [u8; 3]) -> [f32; 3] {
    let table = srgb8_to_linear_table();
    [
        table[rgb[0] as usize],
        table[rgb[1] as usize],
        table[rgb[2] as usize],
    ]
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
    /// When true, the residual path perturbs each dot's fire threshold by a small
    /// deterministic per-(cell, dot, frame) dither. This breaks the period-2 lock
    /// that a coherent frame-subsampling presentation (e.g. 120 emitted shown as
    /// 60) would otherwise alias into a large DC bias, without changing the
    /// long-run mean. Off by default on the raw primitive.
    dither: bool,
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
            dither: false,
        }
    }

    /// Enables or disables residual threshold dither (see the `dither` field). Off
    /// by default; [`TemporalDisplayProcessor`] turns it on for robustness.
    pub fn set_dither(&mut self, enabled: bool) {
        self.dither = enabled;
    }

    /// Whether residual threshold dither is enabled.
    pub fn dither(&self) -> bool {
        self.dither
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
        // Zero the residual accumulator so it holds exactly at 0: with zero
        // residual it stays there, and 0 can never cross a dithered fire threshold
        // (which lies in [0.5, 1.5) or its negation). This guarantees a frozen cell
        // NEVER flips, even with dither enabled.
        self.accumulator[index] = [0.0; 8];
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

    /// Advances one residual phase like [`Self::advance_residual_styled`], but
    /// applies a per-cell eligibility gate: cells marked ineligible emit their
    /// exact static mask and keep their accumulator frozen, so a caller can
    /// modulate low-swing cells while holding high-swing cells perfectly static in
    /// the same frame. `eligible` is indexed by cell in row-major order; a missing
    /// or out-of-range entry is treated as eligible.
    pub fn advance_residual_styled_gated(
        &mut self,
        eligible: &[bool],
        mode: SubcellGlyphMode,
    ) -> Surface {
        let masks = self.advance_residual_masks_gated(Some(eligible));
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
        self.advance_residual_masks_gated(None)
    }

    /// Residual advance with an optional per-cell eligibility gate. An ineligible
    /// cell emits its static-mask baseline and its accumulator is left untouched
    /// (frozen at its current phase), so the cell holds an exact static frame and
    /// can be re-enabled later without carrying stale residual. `eligible` is
    /// indexed by cell (row-major); `None` treats every cell as eligible, and an
    /// out-of-range or missing entry is treated as eligible.
    fn advance_residual_masks_gated(&mut self, eligible: Option<&[bool]>) -> Vec<u8> {
        let mut masks = Vec::with_capacity(self.duty.len());
        let (seed, frame, dither) = (self.seed, self.frame_index, self.dither);
        for (cell_index, ((targets, &baseline), acc)) in self
            .duty
            .iter()
            .zip(self.static_mask.iter())
            .zip(self.accumulator.iter_mut())
            .enumerate()
        {
            let cell_eligible = eligible.is_none_or(|e| e.get(cell_index).copied().unwrap_or(true));
            if !cell_eligible {
                // Ineligible: emit the static baseline, do not integrate residual.
                masks.push(baseline);
                continue;
            }
            let mut mask = 0u8;
            for bit in 0..8 {
                let baseline_on = baseline & (1u8 << bit) != 0;
                let residual = targets[bit] - if baseline_on { 1.0 } else { 0.0 };
                acc[bit] += residual;
                // Fire threshold, optionally dithered in [0.5, 1.5) to decorrelate
                // firing from any fixed presentation-subsampling phase. Each fire
                // still removes exactly 1.0, so the long-run mean is unchanged.
                let threshold = if dither {
                    0.5 + dither_unit(seed, cell_index as u64, bit as u64, frame)
                } else {
                    1.0
                };
                let on = if baseline_on {
                    // Baseline on (residual <= 0): flip OFF when the accumulator
                    // crosses -threshold, otherwise hold the static dot.
                    if acc[bit] <= -threshold {
                        acc[bit] += 1.0;
                        false
                    } else {
                        true
                    }
                } else {
                    // Baseline off (residual >= 0): flip ON when the accumulator
                    // crosses +threshold, otherwise hold the static dot.
                    if acc[bit] >= threshold {
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
        self.fill_surface_from_masks_with(masks, mode, &mut surface, style_at);
        surface
    }

    /// Writes the masks into an existing surface, reusing its allocation. The
    /// surface is reallocated only if its dimensions do not match the field.
    fn fill_surface_from_masks_with(
        &self,
        masks: Vec<u8>,
        mode: SubcellGlyphMode,
        out: &mut Surface,
        style_at: impl Fn(usize) -> Style,
    ) {
        if out.width != self.width || out.height != self.height {
            *out = Surface::new(self.width, self.height);
        }
        if self.width == 0 {
            return;
        }
        for (index, mask) in masks.into_iter().enumerate() {
            let x = (index % self.width as usize) as u16;
            let y = (index / self.width as usize) as u16;
            let style = style_at(index);
            let cell = match mode.subcell_glyph(mask) {
                Some(ch) => Cell::new(Glyph::from_char(ch), style),
                None => Cell::space(style),
            };
            out.set_cell(x, y, cell);
        }
    }

    /// Residual-gated advance that fills `out` in place (reusing its allocation)
    /// instead of returning a fresh [`Surface`]; see
    /// [`Self::advance_residual_styled_gated`].
    pub fn advance_residual_styled_gated_into(
        &mut self,
        eligible: &[bool],
        mode: SubcellGlyphMode,
        out: &mut Surface,
    ) {
        let masks = self.advance_residual_masks_gated(Some(eligible));
        self.fill_surface_from_masks_with(masks, mode, out, |index| self.styles[index]);
    }

    /// Fills `out` with the styled static realization, reusing its allocation; see
    /// [`Self::static_styled_surface`].
    pub fn static_styled_surface_into(&self, mode: SubcellGlyphMode, out: &mut Surface) {
        let masks = self.static_masks();
        self.fill_surface_from_masks_with(masks, mode, out, |index| self.styles[index]);
    }

    fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x < self.width && y < self.height {
            Some((y as usize) * self.width as usize + x as usize)
        } else {
            None
        }
    }
}

/// Rec.709 relative luminance of a color in linear light (`0..=1`), or `None` for
/// [`Color::Reset`], whose realized luminance (the terminal default) is unknown
/// here. Named and indexed colors resolve through [`Color::to_rgb`], so once a
/// style has been quantized to the wire palette its emitted luminance is defined;
/// `None` is treated conservatively (frozen to static) by
/// [`TemporalDisplayProcessor`].
fn color_linear_luminance(color: Color) -> Option<f32> {
    match color {
        // Reset is "terminal default": its realized luminance is unknown here, so
        // treat it conservatively (the caller freezes such cells to static).
        Color::Reset => None,
        // Every other color — RGB, indexed, or one of the 16 named — has a defined
        // 8-bit realization via `to_rgb`, so its emitted luminance is known.
        other => {
            let (r, g, b) = other.to_rgb();
            let lin = rgb8_to_linear([r, g, b]);
            Some(0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2])
        }
    }
}

/// Worst-case instantaneous linear-light luminance swing when one dot of a cell
/// flips between background and foreground, evaluated on the colors **actually
/// emitted** at `depth` — i.e. after the same [`quantize_color`] step the renderer
/// applies on the wire. `None` means the swing is unbounded or unmodelled and the
/// cell must be frozen to static:
///
/// * [`ColorDepth::Mono`] emits no color attribute at all, so fg and bg carry no
///   luminance difference on the wire; the only modulation signal is glyph
///   coverage, which this luminance model does not bound, so Mono is always frozen.
/// * a color whose quantized realization has unknown luminance (e.g.
///   [`Color::Reset`]) or an unset fg/bg also yields `None`.
///
/// In [`ColorDepth::TrueColor`] quantization is the identity on `Color::Rgb`, so
/// this is exactly the pre-0.3.1 RGB swing; ANSI256/ANSI16 now measure the real
/// palette-snapped luminance gap instead of the pre-quantization RGB gap.
fn style_luminance_swing_resolved(style: Style, depth: ColorDepth) -> Option<f32> {
    if depth == ColorDepth::Mono {
        return None;
    }
    let fg = color_linear_luminance(quantize_color(style.fg?, depth)?)?;
    let bg = color_linear_luminance(quantize_color(style.bg?, depth)?)?;
    Some((fg - bg).abs())
}

/// A snapshot of [`TemporalDisplayProcessor`]'s current realization decision.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
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
    /// Eligible cells currently held static by content motion (region or global).
    pub motion_static_cells: usize,
    /// Cells that actually modulate when the frame gate is open (eligible and not
    /// motion-held).
    pub active_cells: usize,
    /// Worst per-cell emitted luminance swing over the target (`0` if none known).
    pub worst_cell_swing: f32,
    /// Mean emitted (post-quantization) static RMSE of the current target.
    pub mean_emitted_static_rmse: f32,
    /// Frames remaining in the cadence-degradation hysteresis hold (`0` = healthy).
    pub degraded_hold_frames: u32,
    /// The color depth used for the emitted-swing safety evaluation.
    pub color_depth: ColorDepth,
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
    /// The color depth the renderer will quantize to on the wire. Safety is judged
    /// on the colors emitted at this depth, not the raw RGB centroids.
    color_depth: ColorDepth,
    field: TemporalBrailleField,
    /// Per-cell modulation eligibility (row-major). A cell is eligible when its
    /// emitted luminance swing at `color_depth` is within the policy cap. This is
    /// non-destructive — the field keeps every cell's continuous duty — so raising
    /// the cap or changing depth can re-enable a cell via `reclassify`.
    eligible: Vec<bool>,
    /// Per-cell content-motion suppression (row-major). A cell marked here is held
    /// static regardless of eligibility, so a moving region (reticle, scan line,
    /// scrolling label) never temporally modulates while a stationary region can.
    cell_motion: Vec<bool>,
    /// Derived per-cell emission gate: `eligible && !cell_motion && !motion_static`.
    /// This is what the residual advance actually reads; a cell transitioning into
    /// the active set reseeds its accumulator so it never resumes with stale
    /// residual from a frozen/moving phase.
    active: Vec<bool>,
    /// Count of `active` cells, so the per-frame modulate decision needs no scan.
    active_cells: usize,
    /// Per-cell emitted (post-quantization) static RMSE, so the reported mean stays
    /// correct after an incremental regional reprojection touches only some cells.
    cell_rmse: Vec<f32>,
    profile: PresentationProfile,
    policy: TemporalSafetyPolicy,
    reduced_motion: bool,
    /// Content-motion override (distinct from the accessibility `reduced_motion`):
    /// when the application knows the content is scrolling/animating, it forces
    /// static output so moving content is never temporally modulated.
    motion_static: bool,
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
        let mut field = TemporalBrailleField::new(width, height, seed);
        // Robust default for the user-facing path: dither the residual so a
        // coherent presentation subsample cannot alias into a DC bias.
        field.set_dither(true);
        let cells = (width as usize).saturating_mul(height as usize);
        Self {
            width,
            height,
            mode,
            color_depth: ColorDepth::TrueColor,
            field,
            eligible: vec![false; cells],
            cell_motion: vec![false; cells],
            active: vec![false; cells],
            active_cells: 0,
            cell_rmse: vec![0.0; cells],
            profile: PresentationProfile::unmeasured(),
            policy: TemporalSafetyPolicy::default(),
            reduced_motion: false,
            motion_static: false,
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

    /// Sets the safety policy and immediately reclassifies the current target's
    /// per-cell eligibility. Because eligibility is non-destructive (the field
    /// retains every cell's continuous duty), raising `max_luminance_depth` can
    /// re-enable cells a stricter policy had frozen, with no reprojection; a
    /// newly-eligible cell has its residual accumulator reseeded.
    pub fn set_policy(&mut self, policy: TemporalSafetyPolicy) {
        self.policy = policy;
        self.reclassify();
    }

    /// Sets the wire color depth used for the emitted-swing safety evaluation and
    /// reclassifies immediately. Safety is judged on the colors the renderer will
    /// actually emit at this depth: in [`ColorDepth::Mono`] no color reaches the
    /// wire, so every cell is frozen to static; ANSI256/ANSI16 use the
    /// palette-snapped luminance rather than the raw RGB. Defaults to
    /// [`ColorDepth::TrueColor`]; typically fed from `ctx.capabilities().color_depth`.
    pub fn set_color_depth(&mut self, depth: ColorDepth) {
        self.color_depth = depth;
        self.reclassify();
    }

    /// The color depth used for the emitted-swing safety evaluation.
    pub fn color_depth(&self) -> ColorDepth {
        self.color_depth
    }

    /// Sets the accessibility reduced-motion preference. When `true`, output is
    /// always the static fallback.
    pub fn set_reduced_motion(&mut self, reduced: bool) {
        self.reduced_motion = reduced;
    }

    /// Enables or disables residual threshold dither (on by default). Dither keeps
    /// the modulation robust to coherent presentation subsampling.
    pub fn set_dither(&mut self, enabled: bool) {
        self.field.set_dither(enabled);
    }

    /// Forces static output while the content is known to be moving/scrolling.
    ///
    /// Distinct from [`Self::set_reduced_motion`] (an accessibility preference);
    /// both result in the static fallback. For animated content, prefer driving
    /// [`Self::set_target_image`] with [`ResetPolicy::Reset`] each frame, which
    /// reseeds residual state so fast motion degrades to static without trails.
    pub fn set_motion_static(&mut self, motion_static: bool) {
        self.motion_static = motion_static;
        self.refresh_active();
    }

    /// Marks a rectangular region (in cells, clamped to the field) as moving or
    /// settled. Cells in a moving region are held static regardless of safety
    /// eligibility, so a moving reticle/scan line/label stays static while the rest
    /// of the field temporally refines. When a region settles (`motion=false`), its
    /// cells resume modulating with freshly reseeded accumulators, so no residual
    /// from the moving phase leaks in. Returns the number of cells updated.
    pub fn set_motion_region(&mut self, x: u16, y: u16, w: u16, h: u16, motion: bool) -> usize {
        let x2 = x.saturating_add(w).min(self.width);
        let y2 = y.saturating_add(h).min(self.height);
        let mut count = 0;
        for cy in y.min(self.height)..y2 {
            for cx in x.min(self.width)..x2 {
                let index = (cy as usize) * self.width as usize + cx as usize;
                self.cell_motion[index] = motion;
                count += 1;
            }
        }
        self.refresh_active();
        count
    }

    /// Clears all content-motion suppression (marks the whole field settled), so
    /// every safety-eligible cell may modulate again.
    pub fn clear_motion(&mut self) {
        self.cell_motion.iter_mut().for_each(|m| *m = false);
        self.refresh_active();
    }

    /// Projects a logical Braille RGB image (`2*width` by `4*height` samples) as
    /// the new target, installs it, then classifies per-cell modulation
    /// eligibility from the emitted luminance swing at the current color depth.
    ///
    /// This is **non-destructive**: every cell keeps its continuous duty target. A
    /// cell whose emitted swing exceeds the policy cap (or whose colors have unknown
    /// luminance, or any cell in [`ColorDepth::Mono`]) is marked ineligible and
    /// emits only its exact static frame — but its target is preserved, so a later
    /// [`Self::set_policy`] or [`Self::set_color_depth`] can re-enable it without
    /// reprojecting. `reset` controls whether accumulators are reseeded (use
    /// [`ResetPolicy::Reset`] for genuinely new content).
    pub fn set_target_image(&mut self, sample: impl Fn(u16, u16) -> [u8; 3], reset: ResetPolicy) {
        let projection = project_braille_image(self.width, self.height, sample);
        projection.install_into(&mut self.field, reset);
        for y in 0..self.height {
            for x in 0..self.width {
                if let Some(cell) = projection.cell(x, y) {
                    let index = (y as usize) * self.width as usize + x as usize;
                    self.cell_rmse[index] = cell.emitted_static_rmse;
                }
            }
        }
        self.has_target = true;
        self.reclassify();
    }

    /// Reprojects only the cells overlapping the rectangle `(x, y, w, h)` (in cells,
    /// clamped to the field), preserving every other cell's projection and residual
    /// accumulator. This is the incremental path for interactive imagery: a small
    /// change costs a small reprojection instead of the whole frame. `sample` is
    /// evaluated over the same logical grid as [`Self::set_target_image`] (`2*width`
    /// by `4*height` subpixels); `reset` reseeds only the touched cells' residual
    /// (use [`ResetPolicy::Reset`] when the region's content changed). Eligibility
    /// is then reclassified (an O(cells) luminance pass, not a reprojection).
    /// Requires a prior [`Self::set_target_image`]; returns the number of cells
    /// reprojected (`0` if there is no target yet).
    pub fn set_target_region(
        &mut self,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        sample: impl Fn(u16, u16) -> [u8; 3],
        reset: ResetPolicy,
    ) -> usize {
        if !self.has_target {
            return 0;
        }
        let x2 = x.saturating_add(w).min(self.width);
        let y2 = y.saturating_add(h).min(self.height);
        let mut count = 0;
        for cy in y.min(self.height)..y2 {
            for cx in x.min(self.width)..x2 {
                let mut target = [[0u8; 3]; 8];
                for dy in 0u16..4 {
                    for dx in 0u16..2 {
                        target[BRAILLE_DOT_INDEX[dy as usize][dx as usize]] =
                            sample(cx * 2 + dx, cy * 4 + dy);
                    }
                }
                let proj = project_rgb_subcells(target);
                let index = (cy as usize) * self.width as usize + cx as usize;
                self.cell_rmse[index] = proj.emitted_static_rmse;
                self.field.set_cell_projection(cx, cy, proj);
                if reset == ResetPolicy::Reset {
                    self.field.reset_cell(cx, cy);
                }
                count += 1;
            }
        }
        self.reclassify();
        count
    }

    /// The top-level profile/reduced-motion gate (independent of the cadence hold).
    pub fn gate(&self) -> TemporalGate {
        self.policy.gate_profile(self.profile, self.reduced_motion)
    }

    /// The always-valid static realization of the current target.
    pub fn static_fallback(&self) -> Surface {
        self.field.static_styled_surface(self.mode)
    }

    /// Writes the always-valid static realization into `out`, reusing its
    /// allocation (resizing to the processor's dimensions if needed).
    pub fn static_fallback_into(&self, out: &mut Surface) {
        self.field.static_styled_surface_into(self.mode, out);
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
        if self.step(missed_periods) {
            self.field
                .advance_residual_styled_gated(&self.active, self.mode)
        } else {
            self.static_fallback()
        }
    }

    /// Like [`Self::advance`], but writes the next frame into `out`, reusing its
    /// allocation instead of returning a fresh [`Surface`] each frame. `out` is
    /// resized to the processor's dimensions if it does not already match. Prefer
    /// this in a high-cadence loop to avoid a per-frame Surface allocation.
    pub fn advance_into(&mut self, missed_periods: u32, out: &mut Surface) {
        if self.step(missed_periods) {
            self.field
                .advance_residual_styled_gated_into(&self.active, self.mode, out);
        } else {
            self.field.static_styled_surface_into(self.mode, out);
        }
    }

    /// Advances the modulation/hysteresis state for one frame and returns whether
    /// this frame should modulate (vs. emit the static fallback). Shared by
    /// [`Self::advance`] and [`Self::advance_into`].
    fn step(&mut self, missed_periods: u32) -> bool {
        if missed_periods > 0 {
            self.degraded_hold = CADENCE_HYSTERESIS_FRAMES;
        } else if self.degraded_hold > 0 {
            self.degraded_hold -= 1;
        }
        let gate = self.policy.gate_profile(self.profile, self.reduced_motion);
        self.last_gate = gate;
        let modulate = gate == TemporalGate::Enabled
            && self.has_target
            && self.active_cells > 0
            && self.degraded_hold == 0;
        self.last_modulating = modulate;
        modulate
    }

    /// A snapshot of the current realization decision and target statistics.
    pub fn diagnostics(&self) -> TemporalDiagnostics {
        TemporalDiagnostics {
            gate: self.last_gate,
            modulating: self.last_modulating,
            modulatable_cells: self.modulatable_cells,
            frozen_cells: self.frozen_cells,
            motion_static_cells: self.modulatable_cells - self.active_cells,
            active_cells: self.active_cells,
            worst_cell_swing: self.worst_cell_swing,
            mean_emitted_static_rmse: self.mean_emitted_static_rmse,
            degraded_hold_frames: self.degraded_hold,
            color_depth: self.color_depth,
        }
    }

    /// Reseeds all residual accumulators (e.g. after a discontinuity).
    pub fn reset(&mut self) {
        self.field.reset_all();
    }

    /// Recomputes per-cell modulation eligibility from the current target's styles,
    /// the policy depth cap and the color depth, and refreshes the diagnostic
    /// counts. A cell transitioning from ineligible to eligible has its residual
    /// accumulator reseeded (it was frozen while ineligible and would otherwise
    /// carry stale residual). Non-destructive: never alters a cell's duty target.
    fn reclassify(&mut self) {
        let cap = self.policy.max_luminance_depth;
        let depth = self.color_depth;
        let (mut modulatable, mut frozen, mut worst) = (0usize, 0usize, 0.0f32);
        let mut rmse_sum = 0.0f32;
        for y in 0..self.height {
            for x in 0..self.width {
                let index = (y as usize) * self.width as usize + x as usize;
                rmse_sum += self.cell_rmse[index];
                let style = self.field.cell_style(x, y).unwrap_or_default();
                let swing = style_luminance_swing_resolved(style, depth);
                if let Some(s) = swing {
                    worst = worst.max(s);
                }
                let eligible = matches!(swing, Some(s) if s <= cap);
                if eligible {
                    modulatable += 1;
                } else {
                    frozen += 1;
                }
                self.eligible[index] = eligible;
            }
        }
        self.modulatable_cells = modulatable;
        self.frozen_cells = frozen;
        self.worst_cell_swing = worst;
        self.mean_emitted_static_rmse = rmse_sum / self.eligible.len().max(1) as f32;
        self.refresh_active();
    }

    /// Recomputes the derived per-cell emission gate
    /// `active = eligible && !cell_motion && !motion_static`, reseeding any cell
    /// that transitions into the active set (it was held static and would otherwise
    /// resume with stale residual), and updates the active-cell count.
    fn refresh_active(&mut self) {
        let global_static = self.motion_static;
        let width = self.width as usize;
        let mut active_cells = 0;
        for index in 0..self.active.len() {
            let now = self.eligible[index] && !self.cell_motion[index] && !global_static;
            if now {
                active_cells += 1;
                if !self.active[index] {
                    let x = (index % width) as u16;
                    let y = (index / width) as u16;
                    self.field.reset_cell(x, y);
                }
            }
            self.active[index] = now;
        }
        self.active_cells = active_cells;
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

/// Deterministic per-(cell, dot, frame) dither in `[0, 1)` for the residual fire
/// threshold, so firing does not lock to a fixed period that a coherent
/// presentation subsample could alias into a DC bias.
fn dither_unit(seed: u64, cell: u64, dot: u64, frame: u64) -> f32 {
    unit_hash(
        seed ^ 0x5A5A_5A5A_5A5A_5A5A,
        cell.wrapping_mul(8).wrapping_add(dot),
        frame,
    )
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
    fn dither_survives_coherent_frame_subsampling_that_biases_plain_sigma_delta() {
        // A 50%-duty dot is the worst case: plain first-order sigma-delta settles
        // into a period-2 (0,1,0,1) pattern, so a coherent 2:1 presentation
        // subsample (only every other emitted frame is shown) locks onto one
        // parity and reads a ~0/1 DC bias. Threshold dither breaks that lock while
        // preserving the mean. This reproduces Fable's coherent-subsampling result.
        fn shown_mean_coherent_2to1(dither: bool) -> f32 {
            let mut field = TemporalBrailleField::new(1, 1, 0xBEEF);
            field.set_dither(dither);
            // Baseline OFF, duty 0.5 => residual +0.5 on dot 0.
            let proj = TemporalCellProjection {
                style: Style::default(),
                duty: [0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                static_mask: 0,
                static_rmse: 0.0,
                line_rmse: 0.0,
                emitted_static_rmse: 0.0,
            };
            assert!(field.set_cell_projection(0, 0, proj));
            let (mut shown_on, mut shown) = (0usize, 0usize);
            for f in 0..2000usize {
                let bit = field.advance_residual_masks()[0] & 1;
                if f % 2 == 0 {
                    shown += 1;
                    if bit != 0 {
                        shown_on += 1;
                    }
                }
            }
            shown_on as f32 / shown as f32
        }

        let plain_bias = (shown_mean_coherent_2to1(false) - 0.5).abs();
        let dith_bias = (shown_mean_coherent_2to1(true) - 0.5).abs();
        assert!(
            plain_bias > 0.4,
            "plain sigma-delta should alias hard under coherent 2:1 (bias {plain_bias})"
        );
        assert!(
            dith_bias < 0.1,
            "dithered residual should track true duty under coherent 2:1 (bias {dith_bias})"
        );
        assert!(
            dith_bias < plain_bias - 0.2,
            "dither must substantially reduce coherent-subsampling bias (plain {plain_bias}, dithered {dith_bias})"
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
    fn processor_motion_static_forces_static_despite_good_profile() {
        let mut p = TemporalDisplayProcessor::new(1, 1, SubcellGlyphMode::Braille2x4, 4);
        p.set_profile(PresentationProfile::measured(120.0, 0.99, 0.1));
        p.set_target_image(
            |_lx, ly| {
                let v = 100u8 + 3 * ly as u8;
                [v, v, v]
            },
            ResetPolicy::Reset,
        );
        // Good profile + modulatable content: the gate itself is Enabled.
        assert_eq!(p.gate(), TemporalGate::Enabled);

        // But moving content forces static.
        p.set_motion_static(true);
        let f = p.advance(0);
        assert!(
            !p.diagnostics().modulating,
            "moving content must render static"
        );
        let stat = p.static_fallback();
        assert_eq!(
            f.get(0, 0).unwrap().glyph.grapheme,
            stat.get(0, 0).unwrap().glyph.grapheme
        );

        // Clearing the override resumes modulation.
        p.set_motion_static(false);
        p.advance(0);
        assert!(
            p.diagnostics().modulating,
            "clearing motion_static resumes modulation"
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

    #[test]
    fn resolved_swing_is_capability_aware_not_raw_rgb() {
        use crate::capability::{quantize_color, ColorDepth};
        use crate::cell::Color;

        let style = Style::default()
            .fg(Color::Rgb(30, 200, 90))
            .bg(Color::Rgb(60, 40, 210));

        // TrueColor: quantization is the identity on RGB, so the resolved swing is
        // exactly the raw linear-luminance swing (behavior unchanged from 0.3.0).
        let tc = style_luminance_swing_resolved(style, ColorDepth::TrueColor).unwrap();
        let raw = (color_linear_luminance(Color::Rgb(30, 200, 90)).unwrap()
            - color_linear_luminance(Color::Rgb(60, 40, 210)).unwrap())
        .abs();
        assert!(
            (tc - raw).abs() < 1e-6,
            "TrueColor swing must equal the raw RGB swing"
        );

        // ANSI256/ANSI16: the swing is computed on the QUANTIZED colors, i.e. the
        // exact colors the renderer emits on the wire (capability.rs quantize_color).
        for depth in [ColorDepth::Ansi256, ColorDepth::Ansi16] {
            let got = style_luminance_swing_resolved(style, depth).unwrap();
            let qfg = quantize_color(Color::Rgb(30, 200, 90), depth).unwrap();
            let qbg = quantize_color(Color::Rgb(60, 40, 210), depth).unwrap();
            let want =
                (color_linear_luminance(qfg).unwrap() - color_linear_luminance(qbg).unwrap()).abs();
            assert!(
                (got - want).abs() < 1e-6,
                "{depth:?} swing must be measured on the quantized wire colors"
            );
        }

        // Ironclad "not raw RGB": two DISTINCT RGB grays that both quantize to the
        // same ANSI16 entry. Raw swing is > 0, but the emitted swing collapses to
        // exactly 0 because the wire carries one color.
        let near = Style::default()
            .fg(Color::Rgb(95, 95, 95))
            .bg(Color::Rgb(110, 110, 110));
        assert!(
            style_luminance_swing_resolved(near, ColorDepth::TrueColor).unwrap() > 0.0,
            "distinct grays have a nonzero TrueColor swing"
        );
        assert_eq!(
            style_luminance_swing_resolved(near, ColorDepth::Ansi16),
            Some(0.0),
            "both grays map to one ANSI16 entry, so the emitted swing is exactly 0"
        );

        // Mono emits no color at all -> the fg/bg luminance signal is not on the
        // wire -> unmodellable -> None (the processor freezes such cells).
        assert!(style_luminance_swing_resolved(style, ColorDepth::Mono).is_none());

        // Color::Reset is the terminal default: unknown luminance -> None.
        let reset_style = Style::default().fg(Color::Reset).bg(Color::Rgb(0, 0, 0));
        assert!(style_luminance_swing_resolved(reset_style, ColorDepth::TrueColor).is_none());
    }

    #[test]
    fn processor_mono_forces_static_everywhere() {
        use crate::capability::ColorDepth;
        let mut p = TemporalDisplayProcessor::new(2, 1, SubcellGlyphMode::Braille2x4, 7);
        p.set_profile(PresentationProfile::measured(120.0, 0.99, 0.1));
        p.set_color_depth(ColorDepth::Mono);
        p.set_target_image(
            |_lx, ly| {
                let v = 100u8 + 3 * ly as u8;
                [v, v, v]
            },
            ResetPolicy::Reset,
        );
        let d = p.diagnostics();
        assert_eq!(d.color_depth, ColorDepth::Mono);
        assert_eq!(
            d.modulatable_cells, 0,
            "Mono has no wire color to bound: all frozen"
        );
        assert_eq!(d.frozen_cells, 2);
        // The profile gate itself is Enabled, but no cell is eligible -> static.
        assert_eq!(p.gate(), TemporalGate::Enabled);
        p.advance(0);
        assert!(!p.diagnostics().modulating, "Mono must never modulate");
    }

    #[test]
    fn raising_depth_cap_reenables_target_without_reprojection() {
        // A cell with a modest, fractional-duty gray gradient: it has a nonzero
        // emitted swing AND interior duties, so when eligible it genuinely
        // modulates. This proves eligibility is non-destructive: if freezing had
        // overwritten the duty (the 0.3.0 behavior), re-enabling would leave the
        // duty at its baseline endpoints and the cell could not modulate.
        let mut p = TemporalDisplayProcessor::new(1, 1, SubcellGlyphMode::Braille2x4, 9);
        p.set_profile(PresentationProfile::measured(120.0, 0.99, 0.1));
        p.set_target_image(
            |_lx, ly| {
                let v = [120u8, 127, 133, 140][ly as usize];
                [v, v, v]
            },
            ResetPolicy::Reset,
        );
        let swing = p.diagnostics().worst_cell_swing;
        assert!(
            swing > 0.0,
            "gradient cell must have a nonzero swing, got {swing}"
        );

        // Strict cap (half the measured swing) freezes the cell.
        p.set_policy(TemporalSafetyPolicy {
            max_luminance_depth: swing * 0.5,
            ..Default::default()
        });
        assert_eq!(
            p.diagnostics().modulatable_cells,
            0,
            "strict cap freezes the moderate-swing cell"
        );

        // Raising the cap (twice the swing) re-enables it WITHOUT set_target_image.
        p.set_policy(TemporalSafetyPolicy {
            max_luminance_depth: swing * 2.0,
            ..Default::default()
        });
        assert_eq!(
            p.diagnostics().modulatable_cells,
            1,
            "raising the cap must reclassify immediately and re-enable the cell"
        );
        assert_eq!(p.diagnostics().frozen_cells, 0);

        // And the re-enabled cell actually modulates: proof the duty survived the
        // freeze (nondestructive eligibility).
        let mut glyphs = std::collections::HashSet::new();
        for _ in 0..300 {
            glyphs.insert(p.advance(0).get(0, 0).unwrap().glyph.grapheme.clone());
        }
        assert!(
            glyphs.len() > 1,
            "re-enabled cell must modulate; a destructive freeze would have flattened its duty"
        );
    }

    #[test]
    fn jitter_gate_rejects_ragged_presentation() {
        let policy = TemporalSafetyPolicy::default(); // max_jitter_fraction = 0.5
                                                      // 120 Hz => 8.333 ms period. p95 jitter 5 ms => fraction 0.6 > 0.5.
        assert_eq!(
            policy.gate_profile(PresentationProfile::measured(120.0, 0.99, 5.0), false),
            TemporalGate::JitterTooHigh
        );
        // 3 ms => fraction 0.36 < 0.5: healthy.
        assert_eq!(
            policy.gate_profile(PresentationProfile::measured(120.0, 0.99, 3.0), false),
            TemporalGate::Enabled
        );
        // Non-finite jitter is never trusted.
        assert_eq!(
            policy.gate_profile(
                PresentationProfile::measured(120.0, 0.99, f32::INFINITY),
                false
            ),
            TemporalGate::JitterTooHigh
        );
    }

    #[test]
    fn motion_region_holds_static_while_neighbor_modulates() {
        let mut p = TemporalDisplayProcessor::new(2, 1, SubcellGlyphMode::Braille2x4, 3);
        p.set_profile(PresentationProfile::measured(120.0, 0.99, 0.1));
        p.set_target_image(
            |_lx, ly| {
                let v = 120u8 + [0, 7, 13, 20][ly as usize];
                [v, v, v]
            },
            ResetPolicy::Reset,
        );
        assert_eq!(
            p.diagnostics().modulatable_cells,
            2,
            "both gradient cells are eligible"
        );
        assert_eq!(p.diagnostics().active_cells, 2);

        // Mark the left cell as moving: it must hold static; the right cell keeps
        // modulating.
        assert_eq!(p.set_motion_region(0, 0, 1, 1, true), 1);
        assert_eq!(
            p.diagnostics().active_cells,
            1,
            "the moving cell is suppressed"
        );
        assert_eq!(p.diagnostics().motion_static_cells, 1);

        let left_static = p
            .static_fallback()
            .get(0, 0)
            .unwrap()
            .glyph
            .grapheme
            .clone();
        let mut right = std::collections::HashSet::new();
        for _ in 0..200 {
            let f = p.advance(0);
            assert_eq!(
                f.get(0, 0).unwrap().glyph.grapheme,
                left_static,
                "a moving cell must stay static every frame"
            );
            right.insert(f.get(1, 0).unwrap().glyph.grapheme.clone());
        }
        assert!(
            right.len() > 1,
            "the settled neighbor keeps temporally refining"
        );

        // Settling the region resumes modulation.
        p.set_motion_region(0, 0, 1, 1, false);
        assert_eq!(
            p.diagnostics().active_cells,
            2,
            "settled region resumes modulating"
        );
    }

    #[test]
    fn incremental_region_matches_full_reprojection() {
        // `full` and `base` agree everywhere except the cell rect (1,1)-(2,2);
        // updating that rect incrementally from a `base` target must reproduce a
        // full projection of `full`, cell-for-cell (glyph, style, eligibility, mean).
        let base = |lx: u16, ly: u16| -> [u8; 3] { [(lx * 10) as u8, (ly * 10) as u8, 100] };
        let full = |lx: u16, ly: u16| -> [u8; 3] {
            let (cx, cy) = (lx / 2, ly / 4);
            if (1..3).contains(&cx) && (1..3).contains(&cy) {
                [200, (lx * 5) as u8, 50]
            } else {
                base(lx, ly)
            }
        };

        let mut pf = TemporalDisplayProcessor::new(4, 4, SubcellGlyphMode::Braille2x4, 1);
        pf.set_target_image(full, ResetPolicy::Reset);

        let mut pi = TemporalDisplayProcessor::new(4, 4, SubcellGlyphMode::Braille2x4, 1);
        pi.set_target_image(base, ResetPolicy::Reset);
        assert_eq!(
            pi.set_target_region(1, 1, 2, 2, full, ResetPolicy::Reset),
            4
        );

        let sf = pf.static_fallback();
        let si = pi.static_fallback();
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(
                    sf.get(x, y).unwrap().glyph.grapheme,
                    si.get(x, y).unwrap().glyph.grapheme,
                    "cell ({x},{y}) glyph must match a full reprojection"
                );
                assert_eq!(
                    sf.get(x, y).unwrap().style,
                    si.get(x, y).unwrap().style,
                    "cell ({x},{y}) style must match a full reprojection"
                );
            }
        }
        assert_eq!(
            pf.diagnostics().modulatable_cells,
            pi.diagnostics().modulatable_cells,
            "eligibility must match a full reprojection"
        );
        assert!(
            (pf.diagnostics().mean_emitted_static_rmse - pi.diagnostics().mean_emitted_static_rmse)
                .abs()
                < 1e-6,
            "mean emitted RMSE must match a full reprojection"
        );
    }

    #[test]
    fn srgb8_linear_lut_is_bit_identical_to_the_transfer_function() {
        // The LUT must reproduce the sRGB->linear transfer function exactly for
        // every one of the 256 possible channel values, so the projector's SSE and
        // argmin (hence its output) are provably unchanged by the optimization.
        for v in 0u16..=255 {
            let value = v as f32 / 255.0;
            let direct = if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            };
            let lut = rgb8_to_linear([v as u8, v as u8, v as u8])[0];
            assert_eq!(
                lut.to_bits(),
                direct.to_bits(),
                "LUT entry {v} must be bit-identical to the transfer function"
            );
        }
    }

    #[test]
    fn advance_into_matches_owned_advance() {
        let mk = || {
            let mut p = TemporalDisplayProcessor::new(6, 3, SubcellGlyphMode::Braille2x4, 0x1234);
            p.set_profile(PresentationProfile::measured(120.0, 0.99, 0.1));
            p.set_target_image(
                // A low-swing vertical gradient (ly spans 0..4*height): eligible,
                // with fractional per-cell duty so the modulating path is exercised.
                |_lx, ly| {
                    let v = 70u8.wrapping_add((ly as u8).wrapping_mul(3));
                    [v, v, v]
                },
                ResetPolicy::Reset,
            );
            p
        };
        let mut owned = mk();
        let mut into = mk();
        // A wrong-sized buffer must be resized transparently.
        let mut buf = Surface::new(1, 1);
        for frame in 0..64 {
            let a = owned.advance(0);
            into.advance_into(0, &mut buf);
            assert_eq!(buf.width, 6, "advance_into resizes a mismatched buffer");
            assert_eq!(buf.height, 3);
            for y in 0..3 {
                for x in 0..6 {
                    assert_eq!(
                        a.get(x, y).unwrap().glyph.grapheme,
                        buf.get(x, y).unwrap().glyph.grapheme,
                        "frame {frame} cell ({x},{y}) glyph must match the owned path"
                    );
                    assert_eq!(
                        a.get(x, y).unwrap().style,
                        buf.get(x, y).unwrap().style,
                        "frame {frame} cell ({x},{y}) style must match the owned path"
                    );
                }
            }
        }
        assert!(
            into.diagnostics().modulating,
            "this test must exercise the modulating path, not the static fallback"
        );
    }
}
