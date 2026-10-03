//! Reconstruction and residual-spectrum metrics.
//!
//! Everything here is deterministic linear algebra over the emitted Braille
//! masks. The reconstruction model is the documented one: for a cell with
//! stable foreground/background `F,B` in linear light and time-averaged per-dot
//! duty `d_hat`, the reconstructed logical sample is `P_i = B + d_hat_i (F-B)`.

use gibson::{Cell, Color, Style, Surface};

/// sRGB 8-bit channel -> linear light.
pub fn srgb8_to_linear(u: u8) -> f32 {
    let v = u as f32 / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub fn rgb8_to_linear(rgb: [u8; 3]) -> [f32; 3] {
    [
        srgb8_to_linear(rgb[0]),
        srgb8_to_linear(rgb[1]),
        srgb8_to_linear(rgb[2]),
    ]
}

/// Linear-light color of a `Color` as actually emitted (8-bit realization).
pub fn color_linear(c: Color) -> [f32; 3] {
    rgb8_to_linear([c.to_rgb().0, c.to_rgb().1, c.to_rgb().2])
}

/// Number of cells changed between two masks-of-the-frame.
pub fn changed_cells(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).filter(|(x, y)| x != y).count()
}

/// Dots flipped between two masks, summed over cells.
pub fn flipped_dots(a: &[u8], b: &[u8]) -> usize {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x ^ y).count_ones() as usize)
        .sum()
}

/// Decodes the per-cell Braille mask from an ordinary `Surface` produced in
/// `SubcellGlyphMode::Braille2x4`. A space (or any non-Braille glyph) is mask 0.
pub fn surface_masks(surface: &Surface) -> Vec<u8> {
    let mut out = Vec::with_capacity(surface.width as usize * surface.height as usize);
    for y in 0..surface.height {
        for x in 0..surface.width {
            let c = surface.get(x, y);
            let mask = c
                .and_then(|cell| cell.glyph.grapheme.chars().next())
                .and_then(|ch| {
                    let u = ch as u32;
                    if (0x2800..=0x28FF).contains(&u) {
                        Some((u - 0x2800) as u8)
                    } else if u == ' ' as u32 {
                        Some(0)
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            out.push(mask);
        }
    }
    out
}

/// Builds an ordinary `Surface` from per-cell Braille masks and styles.
pub fn surface_from_masks(
    width: u16,
    height: u16,
    masks: &[u8],
    style_at: impl Fn(usize) -> Style,
) -> Surface {
    let mut s = Surface::new(width, height);
    for (i, m) in masks.iter().enumerate() {
        let x = (i % width as usize) as u16;
        let y = (i / width as usize) as u16;
        let glyph = if *m == 0 {
            gibson::Glyph::space()
        } else {
            gibson::Glyph::from_char(char::from_u32(0x2800 + *m as u32).unwrap_or(' '))
        };
        s.set_cell(x, y, Cell::new(glyph, style_at(i)));
    }
    s
}

/// Per-cell reconstruction error for one cell given its time-averaged duty.
#[derive(Debug, Clone, Copy)]
pub struct CellMetric {
    /// RMSE against the source logical samples (linear RGB, 24 values).
    pub rmse_source: f32,
    /// RMSE against the projector's ideal continuous line projection.
    pub rmse_line: f32,
    /// Signed mean `d_hat - duty` over the eight dots.
    pub bias: f32,
    /// Mean absolute `d_hat - duty` over the eight dots.
    pub abs_bias: f32,
}

pub fn cell_metric(
    source: &[[u8; 3]; 8],
    style: Style,
    duty: &[f32; 8],
    avg: &[f32; 8],
) -> CellMetric {
    let f = color_linear(style.fg.unwrap_or(Color::Reset));
    let b = color_linear(style.bg.unwrap_or(Color::Reset));
    let mut sse_source = 0.0f64;
    let mut sse_line = 0.0f64;
    let mut bias = 0.0f64;
    let mut abs_bias = 0.0f64;
    for i in 0..8 {
        let src = rgb8_to_linear(source[i]);
        for ch in 0..3 {
            let recon = b[ch] + avg[i] * (f[ch] - b[ch]);
            sse_source += ((recon - src[ch]) as f64).powi(2);
            let ideal = b[ch] + duty[i] * (f[ch] - b[ch]);
            sse_line += ((recon - ideal) as f64).powi(2);
        }
        bias += (avg[i] - duty[i]) as f64;
        abs_bias += (avg[i] - duty[i]).abs() as f64;
    }
    CellMetric {
        rmse_source: (sse_source / 24.0).sqrt() as f32,
        rmse_line: (sse_line / 24.0).sqrt() as f32,
        bias: (bias / 8.0) as f32,
        abs_bias: (abs_bias / 8.0) as f32,
    }
}

/// Aggregate reconstruction result over a whole image.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReconStats {
    pub rmse_source: f32,
    pub rmse_line: f32,
    pub bias: f32,
    pub abs_bias: f32,
    /// RMSE_source restricted to cells the target classifies as `Edge`.
    pub rmse_source_edge: f32,
    pub rmse_source_detail: f32,
    pub rmse_source_flat: f32,
    pub edge_cells: usize,
    pub detail_cells: usize,
    pub flat_cells: usize,
    /// Mean dots flipped between consecutive emitted phases.
    pub flips_per_phase: f32,
    /// Mean terminal cells whose glyph changes between consecutive phases.
    pub changed_cells_per_phase: f32,
    pub cells: usize,
}

/// Aggregates per-cell metrics and transition churn.
///
/// `per_cell`: `(source8, style, duty, class)` for each cell in row-major order.
/// `masks_per_phase`: `K` frames, each `cells` masks.
pub fn recon_stats(
    per_cell: &[([[u8; 3]; 8], Style, [f32; 8], super::targets::CellClass)],
    masks_per_phase: &[Vec<u8>],
) -> ReconStats {
    use super::targets::CellClass;
    let k = masks_per_phase.len().max(1);
    let cells = per_cell.len();
    let mut acc = ReconStats {
        cells,
        ..Default::default()
    };
    let mut counts = [0usize; 3];
    let mut sums = [0f64; 3];

    for c in 0..cells {
        let (source, style, duty, class) = &per_cell[c];
        let mut avg = [0f32; 8];
        for masks in masks_per_phase {
            let m = masks[c];
            for i in 0..8 {
                if m & (1 << i) != 0 {
                    avg[i] += 1.0;
                }
            }
        }
        for a in avg.iter_mut() {
            *a /= k as f32;
        }
        let m = cell_metric(source, *style, duty, &avg);
        acc.rmse_source += m.rmse_source;
        acc.rmse_line += m.rmse_line;
        acc.bias += m.bias;
        acc.abs_bias += m.abs_bias;
        let idx = match class {
            CellClass::Flat => 0,
            CellClass::Edge => 1,
            CellClass::Detail => 2,
        };
        counts[idx] += 1;
        sums[idx] += m.rmse_source as f64;
    }
    if cells > 0 {
        acc.rmse_source /= cells as f32;
        acc.rmse_line /= cells as f32;
        acc.bias /= cells as f32;
        acc.abs_bias /= cells as f32;
    }
    let class_mean = |idx: usize| -> f32 {
        if counts[idx] == 0 {
            0.0
        } else {
            (sums[idx] / counts[idx] as f64) as f32
        }
    };
    acc.rmse_source_flat = class_mean(0);
    acc.rmse_source_edge = class_mean(1);
    acc.rmse_source_detail = class_mean(2);
    acc.flat_cells = counts[0];
    acc.edge_cells = counts[1];
    acc.detail_cells = counts[2];

    let mut flips = 0usize;
    let mut changed = 0usize;
    let mut transitions = 0usize;
    for w in masks_per_phase.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        flips += flipped_dots(a, b);
        changed += changed_cells(a, b);
        transitions += 1;
    }
    if transitions > 0 && cells > 0 {
        acc.flips_per_phase = flips as f32 / transitions as f32;
        acc.changed_cells_per_phase = changed as f32 / transitions as f32;
    }
    acc
}

/// Low-frequency / coherent-flicker statistics of the emitted masks over time.
#[derive(Debug, Clone, Copy, Default)]
pub struct SpectrumStats {
    /// Mean over dots of the per-dot mean (lit fraction), a DC proxy.
    pub mean_lit: f32,
    /// Mean over dots of the fraction of AC energy in the lowest eighth of bins.
    pub lf_ratio: f32,
    /// Mean over dots of the normalized lag-1 autocorrelation of the zero-mean
    /// sequence.
    pub acf1: f32,
    /// Variance over time of the whole-region lit fraction (global breathing).
    pub flash_var: f32,
    /// Mean over cells of the mean pairwise correlation of the eight dot
    /// sequences (within-cell spatial coherence of the temporal residual).
    pub dot_corr: f32,
}

/// Computes [`SpectrumStats`] from `frames` frames of `cells` masks each.
///
/// `frames` is row-major over time of mask vectors; every frame must have the
/// same length. Requires at least 8 frames for a meaningful spectrum.
pub fn spectrum_stats(frames: &[Vec<u8>]) -> SpectrumStats {
    let t = frames.len();
    if t < 8 {
        return SpectrumStats::default();
    }
    let cells = frames[0].len();
    let n = t;
    let low_bins = (n / 8).max(1);
    // Global lit fraction per frame.
    let mut frame_frac = Vec::with_capacity(t);
    for f in frames {
        let lit: u32 = f.iter().map(|m| m.count_ones()).sum();
        frame_frac.push(lit as f64 / (cells.max(1) * 8) as f64);
    }
    let fmean = frame_frac.iter().sum::<f64>() / t as f64;
    let flash_var = frame_frac.iter().map(|x| (x - fmean).powi(2)).sum::<f64>() / t as f64;

    // Per-dot temporal sequences (pooled statistics over all dots of all cells).
    let mut lf_sum = 0.0f64;
    let mut lf_count = 0.0f64;
    let mut acf_sum = 0.0f64;
    let mut acf_count = 0.0f64;
    let mut dot_mean_sum = 0.0f64;
    let mut dot_count = 0.0f64;
    let mut corr_sum = 0.0f64;
    let mut corr_count = 0.0f64;

    for c in 0..cells {
        // Build the eight zero-mean dot sequences for this cell.
        let mut seqs: Vec<Vec<f64>> = (0..8).map(|_| Vec::with_capacity(t)).collect();
        for f in frames {
            let m = f[c];
            for i in 0..8 {
                seqs[i].push(if m & (1 << i) != 0 { 1.0 } else { 0.0 });
            }
        }
        // Per-dot stats.
        for s in &seqs {
            let mean = s.iter().sum::<f64>() / t as f64;
            dot_mean_sum += mean;
            dot_count += 1.0;
            let x: Vec<f64> = s.iter().map(|v| v - mean).collect();
            let total: f64 = x.iter().map(|v| v * v).sum();
            if total <= 1e-12 {
                continue;
            }
            // Low-frequency band energy via a direct DFT on the lowest bins.
            let mut low = 0.0f64;
            for k in 1..=low_bins {
                let (mut re, mut im) = (0.0f64, 0.0f64);
                for (ti, v) in x.iter().enumerate() {
                    let ang = -2.0 * std::f64::consts::PI * k as f64 * ti as f64 / n as f64;
                    re += v * ang.cos();
                    im += v * ang.sin();
                }
                low += re * re + im * im;
            }
            // Normalise so a zero-mean sequence splits its energy over n bins.
            lf_sum += (low / (n as f64 / 2.0)) / total;
            lf_count += 1.0;
            // Lag-1 autocorrelation.
            let num: f64 = x
                .iter()
                .take(t - 1)
                .zip(x.iter().skip(1))
                .map(|(a, b)| a * b)
                .sum();
            let den: f64 = x.iter().map(|v| v * v).sum();
            acf_sum += num / den;
            acf_count += 1.0;
        }
        // Mean pairwise dot correlation within the cell.
        for i in 0..8 {
            for j in (i + 1)..8 {
                let a = &seqs[i];
                let b = &seqs[j];
                let ma = a.iter().sum::<f64>() / t as f64;
                let mb = b.iter().sum::<f64>() / t as f64;
                let (mut num, mut da, mut db) = (0.0f64, 0.0f64, 0.0f64);
                for ti in 0..t {
                    let xa = a[ti] - ma;
                    let xb = b[ti] - mb;
                    num += xa * xb;
                    da += xa * xa;
                    db += xb * xb;
                }
                if da > 1e-12 && db > 1e-12 {
                    corr_sum += num / (da * db).sqrt();
                    corr_count += 1.0;
                }
            }
        }
    }
    SpectrumStats {
        mean_lit: if dot_count > 0.0 {
            (dot_mean_sum / dot_count) as f32
        } else {
            0.0
        },
        lf_ratio: if lf_count > 0.0 {
            (lf_sum / lf_count) as f32
        } else {
            0.0
        },
        acf1: if acf_count > 0.0 {
            (acf_sum / acf_count) as f32
        } else {
            0.0
        },
        flash_var: flash_var as f32,
        dot_corr: if corr_count > 0.0 {
            (corr_sum / corr_count) as f32
        } else {
            0.0
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_endpoints_and_roundtrip_are_sane() {
        assert_eq!(srgb8_to_linear(0), 0.0);
        assert!((srgb8_to_linear(255) - 1.0).abs() < 1e-6);
        assert!(srgb8_to_linear(128) > 0.2 && srgb8_to_linear(128) < 0.25);
    }

    #[test]
    fn static_reconstruction_matches_cell_duty_threshold() {
        // A projection whose static mask reproduces the duty exactly at 0/1.
        let source = [[0u8, 0, 0]; 8];
        let style = Style::default()
            .fg(Color::Rgb(255, 255, 255))
            .bg(Color::Rgb(0, 0, 0));
        let duty = [0.0f32; 8];
        let avg = [0.0f32; 8];
        let m = cell_metric(&source, style, &duty, &avg);
        // White-on-black at duty 0 reconstructs black exactly.
        assert!(m.rmse_source < 1e-6, "{}", m.rmse_source);
    }

    #[test]
    fn spectrum_detects_coherent_flashing() {
        // Fast full-cell toggling (0xFF/0x00, 64 frames): huge lag-1
        // anti-correlation and whole-region breathing.
        let fast: Vec<Vec<u8>> = (0..64)
            .map(|t| vec![if t % 2 == 0 { 0xFFu8 } else { 0x00 }])
            .collect();
        let s = spectrum_stats(&fast);
        assert!(s.flash_var > 0.2, "flash_var={}", s.flash_var);
        assert!(s.acf1 < -0.9, "acf1={}", s.acf1);

        // Slow full-cell flicker (period 16 over 128 frames): the flicker lives in
        // the lowest temporal eighth and must register as low-frequency energy.
        let slow: Vec<Vec<u8>> = (0..128)
            .map(|t| vec![if (t / 8) % 2 == 0 { 0xFFu8 } else { 0x00 }])
            .collect();
        let s2 = spectrum_stats(&slow);
        assert!(s2.flash_var > 0.2, "slow flash_var={}", s2.flash_var);
        assert!(s2.lf_ratio > 0.5, "slow lf_ratio={}", s2.lf_ratio);
    }

    #[test]
    fn spectrum_of_balanced_sequence_is_quieter_than_aligned() {
        // Build the same mean duty two ways across 8 frames: aligned and
        // balanced. The balanced one must have lower flash variance.
        let aligned: Vec<Vec<u8>> = (0..8)
            .map(|t| vec![if t < 4 { 0xFF } else { 0x00 }])
            .collect();
        let balanced: Vec<Vec<u8>> = vec![
            vec![0x55],
            vec![0xAA],
            vec![0x55],
            vec![0xAA],
            vec![0x55],
            vec![0xAA],
            vec![0x55],
            vec![0xAA],
        ];
        let a = spectrum_stats(&aligned);
        let b = spectrum_stats(&balanced);
        assert!(a.flash_var > b.flash_var);
    }

    #[test]
    fn surface_mask_roundtrip() {
        let masks = vec![0u8, 0x01, 0xFF, 0x5A];
        let s = surface_from_masks(4, 1, &masks, |_| Style::default());
        assert_eq!(surface_masks(&s), masks);
    }
}
