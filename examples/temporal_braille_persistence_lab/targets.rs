//! Deterministic synthetic logical-image targets for the temporal Braille
//! persistence lab.
//!
//! A [`LogicalImage`] is a `2*cols` by `4*rows` grid of 8-bit sRGB samples
//! — exactly the logical Braille subpixel grid the projector consumes. Nothing
//! here touches the terminal; these are pure in-memory targets so every
//! measurement is reproducible from `(kind, cols, rows, seed)` alone.

/// Which family a target belongs to; used to slice error metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    SmoothGradient,
    ShallowGradient,
    SlantedEdge,
    ThinLine,
    Checker,
    Radial,
    ZonePlate,
    Portrait,
    MovingBar,
    /// Full-chroma ramps: per-cell colours non-collinear in RGB.
    ColorRamp,
    ChromaEdge,
    ChromaZone,
}

impl TargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TargetKind::SmoothGradient => "smooth-gradient",
            TargetKind::ShallowGradient => "shallow-gradient",
            TargetKind::SlantedEdge => "slanted-edge",
            TargetKind::ThinLine => "thin-line",
            TargetKind::Checker => "checker",
            TargetKind::Radial => "radial",
            TargetKind::ZonePlate => "zone-plate",
            TargetKind::Portrait => "portrait",
            TargetKind::MovingBar => "moving-bar",
            TargetKind::ColorRamp => "color-ramp",
            TargetKind::ChromaEdge => "chroma-edge",
            TargetKind::ChromaZone => "chroma-zone",
        }
    }
}

/// A deterministic logical RGB image at the Braille subpixel resolution.
#[derive(Debug, Clone)]
pub struct LogicalImage {
    pub cols: u16,
    pub rows: u16,
    pub kind: TargetKind,
    /// Row-major `(2*cols) * (4*rows)` sRGB samples.
    px: Vec<[u8; 3]>,
}

impl LogicalImage {
    pub fn new(cols: u16, rows: u16, kind: TargetKind, f: impl Fn(u16, u16) -> [u8; 3]) -> Self {
        let w = 2 * cols as usize;
        let h = 4 * rows as usize;
        let mut px = Vec::with_capacity(w * h);
        for ly in 0..4 * rows {
            for lx in 0..2 * cols {
                px.push(f(lx, ly));
            }
        }
        Self {
            cols,
            rows,
            kind,
            px,
        }
    }

    #[inline]
    pub fn at(&self, lx: u16, ly: u16) -> [u8; 3] {
        self.px[ly as usize * (2 * self.cols as usize) + lx as usize]
    }

    pub fn sample(&self, lx: u16, ly: u16) -> [u8; 3] {
        self.at(lx, ly)
    }

    /// The eight logical samples of cell `(cx, cy)` in Unicode Braille dot order
    /// (bit `1<<i`), matching `temporal::project_rgb_subcells`.
    pub fn cell_samples(&self, cx: u16, cy: u16) -> [[u8; 3]; 8] {
        // Same mapping as `temporal::BRAILLE_DOT_INDEX`.
        const IDX: [[usize; 2]; 4] = [[0, 3], [1, 4], [2, 5], [6, 7]];
        let mut out = [[0u8; 3]; 8];
        for dy in 0u16..4 {
            for dx in 0u16..2 {
                out[IDX[dy as usize][dx as usize]] = self.at(cx * 2 + dx, cy * 4 + dy);
            }
        }
        out
    }
}

#[inline]
fn gray(v: f32) -> [u8; 3] {
    let v = v.clamp(0.0, 255.0).round() as u8;
    [v, v, v]
}

/// A smooth diagonal grayscale ramp.
pub fn smooth_gradient(cols: u16, rows: u16, _seed: u64) -> LogicalImage {
    let span = (2 * cols as u32 + 4 * rows as u32).max(1);
    LogicalImage::new(cols, rows, TargetKind::SmoothGradient, move |lx, ly| {
        let v = 20.0 + ((lx as u32 + ly as u32) * 215) as f32 / span as f32;
        gray(v)
    })
}

/// A very shallow ramp whose whole range sits inside one 8-bit Braille-duty
/// band, so static two-colour quantization has almost no headroom and the
/// temporal path must carry the tonal detail.
pub fn shallow_gradient(cols: u16, rows: u16, _seed: u64) -> LogicalImage {
    let span = (2 * cols as u32 + 4 * rows as u32).max(1);
    LogicalImage::new(cols, rows, TargetKind::ShallowGradient, move |lx, ly| {
        let v = 90.0 + ((lx as u32 + ly as u32) * 36) as f32 / span as f32;
        gray(v)
    })
}

/// A high-contrast slanted edge at `angle_deg`.
pub fn slanted_edge(cols: u16, rows: u16, angle_deg: f32) -> LogicalImage {
    let (w, h) = (2.0 * cols as f32, 4.0 * rows as f32);
    let (cx, cy) = (w * 0.5, h * 0.5);
    let (ca, sa) = (angle_deg.to_radians().cos(), angle_deg.to_radians().sin());
    LogicalImage::new(cols, rows, TargetKind::SlantedEdge, move |lx, ly| {
        // Soft ~1.5-sample edge so the target is band-limited, not a naive step.
        let d = ca * (lx as f32 - cx) + sa * (ly as f32 - cy);
        let t = (d / 1.5 + 0.5).clamp(0.0, 1.0);
        gray(24.0 + t * 220.0)
    })
}

/// A thin bright line at `angle_deg` over a dark field.
pub fn thin_line(cols: u16, rows: u16, angle_deg: f32) -> LogicalImage {
    let (w, h) = (2.0 * cols as f32, 4.0 * rows as f32);
    let (cx, cy) = (w * 0.5, h * 0.5);
    let (ca, sa) = (angle_deg.to_radians().cos(), angle_deg.to_radians().sin());
    LogicalImage::new(cols, rows, TargetKind::ThinLine, move |lx, ly| {
        let d = (ca * (lx as f32 - cx) + sa * (ly as f32 - cy)).abs();
        let t = (1.0 - d / 1.2).clamp(0.0, 1.0);
        gray(20.0 + t * 225.0)
    })
}

/// A checkerboard at the logical-lattice limit (`period` sample units).
pub fn checker(cols: u16, rows: u16, period: u16) -> LogicalImage {
    let p = period.max(1) as u32;
    LogicalImage::new(cols, rows, TargetKind::Checker, move |lx, ly| {
        let on = ((lx as u32 / p) + (ly as u32 / p)) % 2 == 0;
        gray(if on { 210.0 } else { 40.0 })
    })
}

/// A filled disc with a soft rim.
pub fn radial(cols: u16, rows: u16, _seed: u64) -> LogicalImage {
    let (w, h) = (2.0 * cols as f32, 4.0 * rows as f32);
    let (cx, cy) = (w * 0.5, h * 0.5);
    let r = 0.34 * w.min(h) * 2.0;
    LogicalImage::new(cols, rows, TargetKind::Radial, move |lx, ly| {
        let d = ((lx as f32 - cx).powi(2) + (ly as f32 - cy).powi(2)).sqrt();
        let t = ((r - d) / 1.5 + 0.5).clamp(0.0, 1.0);
        gray(20.0 + t * 225.0)
    })
}

/// A zone plate: a radial spatial-frequency sweep — the hardest case for a
/// fixed 2x4 sampling lattice because the local wavelength sweeps through it.
pub fn zone_plate(cols: u16, rows: u16, _seed: u64) -> LogicalImage {
    let (w, h) = (2.0 * cols as f32, 4.0 * rows as f32);
    let (cx, cy) = (w * 0.5, h * 0.5);
    let k = 0.35;
    LogicalImage::new(cols, rows, TargetKind::ZonePlate, move |lx, ly| {
        let r2 = (lx as f32 - cx).powi(2) + (ly as f32 - cy).powi(2);
        let c = 0.5 + 0.5 * (k * r2 / w).cos();
        gray(20.0 + c * 225.0)
    })
}

/// A procedural portrait-like target: a bright oval face, a darker hair mass
/// and a couple of hard specular highlights, all with soft organic edges.
pub fn portrait(cols: u16, rows: u16, seed: u64) -> LogicalImage {
    let (w, h) = (2.0 * cols as f32, 4.0 * rows as f32);
    let (cx, cy) = (w * 0.5, h * 0.52);
    let jitter = ((seed % 17) as f32) * 0.1;
    LogicalImage::new(cols, rows, TargetKind::Portrait, move |lx, ly| {
        let x = (lx as f32 - cx) / (w * 0.28);
        let y = (ly as f32 - cy) / (h * 0.30);
        let face = 1.0 - (x * x + y * y + jitter * 0.02).sqrt();
        let hair = 1.0 - ((x * 1.15).powi(2) + (y * 1.25).powi(2)).sqrt();
        let mut v = 30.0;
        if hair > 0.0 {
            v = 26.0 + hair * 34.0;
        }
        if face > 0.0 {
            v = 120.0 + face * 105.0;
        }
        // Two small hard speculars in the upper face.
        let e1 = ((x - 0.35).powi(2) + (y + 0.18).powi(2)).sqrt();
        let e2 = ((x + 0.35).powi(2) + (y + 0.18).powi(2)).sqrt();
        if e1 < 0.14 || e2 < 0.14 {
            v = v.max(235.0);
        }
        gray(v)
    })
}

/// A bright vertical bar sweeping horizontally; `phase` in `0..1`.
pub fn moving_bar(cols: u16, rows: u16, phase: f32) -> LogicalImage {
    let w = 2.0 * cols as f32;
    let bar_c = phase.rem_euclid(1.0) * w;
    let half = (w * 0.08).max(1.0);
    LogicalImage::new(cols, rows, TargetKind::MovingBar, move |lx, ly| {
        let d = (lx as f32 - bar_c).abs();
        let bar = (1.0 - d / half).clamp(0.0, 1.0);
        // Static textured background so the moving bar is not the only content.
        let bg = 40.0 + 20.0 * ((lx as f32 * 0.4 + ly as f32 * 0.2).sin());
        gray(bg + bar * (230.0 - bg))
    })
}

/// A full-chroma RGB sweep: each dot's colour is an independent combination of
/// the three channels, so the eight samples of a cell are generally
/// non-collinear in RGB. No single cell-local bg->fg line can fit them, which
/// exposes the two-colour model's true spatial/chromatic floor.
pub fn color_ramp(cols: u16, rows: u16, _seed: u64) -> LogicalImage {
    let span = (2 * cols as u32 + 4 * rows as u32).max(1) as f32;
    LogicalImage::new(cols, rows, TargetKind::ColorRamp, move |lx, ly| {
        let t = (lx as f32 + 0.7 * ly as f32) / span;
        let pi = std::f32::consts::PI;
        let r = 30.0 + 220.0 * (t * pi).sin().abs();
        let g = 30.0 + 220.0 * (t * 3.0 * pi).sin().abs();
        let b = 30.0 + 220.0 * (t * 5.0 * pi).sin().abs();
        [r as u8, g as u8, b as u8]
    })
}

/// A luminance edge that also rotates hue across it: warm above, cool below.
pub fn chroma_edge(cols: u16, rows: u16, angle_deg: f32) -> LogicalImage {
    let (w, h) = (2.0 * cols as f32, 4.0 * rows as f32);
    let (cx, cy) = (w * 0.5, h * 0.5);
    let (ca, sa) = (angle_deg.to_radians().cos(), angle_deg.to_radians().sin());
    LogicalImage::new(cols, rows, TargetKind::ChromaEdge, move |lx, ly| {
        let d = ca * (lx as f32 - cx) + sa * (ly as f32 - cy);
        let t = (d / 1.5 + 0.5).clamp(0.0, 1.0);
        let r = 25.0 + t * 210.0;
        let g = 25.0 + (1.0 - (2.0 * t - 1.0).abs()) * 130.0;
        let b = 25.0 + (1.0 - t) * 210.0;
        [r as u8, g as u8, b as u8]
    })
}

/// A chroma zone plate: a radial frequency sweep in luminance with an
/// independent (faster) sweep in the blue channel.
pub fn chroma_zone(cols: u16, rows: u16, _seed: u64) -> LogicalImage {
    let (w, h) = (2.0 * cols as f32, 4.0 * rows as f32);
    let (cx, cy) = (w * 0.5, h * 0.5);
    LogicalImage::new(cols, rows, TargetKind::ChromaZone, move |lx, ly| {
        let r2 = (lx as f32 - cx).powi(2) + (ly as f32 - cy).powi(2);
        let c = 0.5 + 0.5 * (0.35 * r2 / w).cos();
        let cb = 0.5 + 0.5 * (0.61 * r2 / w).cos();
        let r = 25.0 + c * 215.0;
        let g = 25.0 + (0.5 + 0.5 * (0.48 * r2 / w).cos()) * 190.0;
        let b = 25.0 + cb * 215.0;
        [r as u8, g as u8, b as u8]
    })
}

/// Chromatic targets used only by the energy-decomposition mode. These are
/// deliberately *not* part of [`named_targets`] so the historical modes keep
/// their original corpus and numbers.
pub fn chromatic_targets(cols: u16, rows: u16, _seed: u64) -> Vec<(String, LogicalImage)> {
    vec![
        ("color-ramp".into(), color_ramp(cols, rows, _seed)),
        ("chroma-edge".into(), chroma_edge(cols, rows, 38.0)),
        ("chroma-zone".into(), chroma_zone(cols, rows, _seed)),
    ]
}

/// Per-cell error class used to slice metrics by target structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellClass {
    Flat,
    Edge,
    Detail,
}

/// Classifies a cell from its eight logical samples: a flat cell has tiny
/// within-cell range; an edge cell spans a large luminance range across the
/// cell; a detail cell is high-variance without a single dominant step.
pub fn classify_cell(samples: &[[u8; 3]; 8]) -> CellClass {
    let lum = |c: [u8; 3]| 0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32;
    let vals: Vec<f32> = samples.iter().map(|c| lum(*c)).collect();
    let min = vals.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = vals.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let range = max - min;
    if range < 12.0 {
        CellClass::Flat
    } else if range > 120.0 {
        CellClass::Edge
    } else {
        CellClass::Detail
    }
}

/// The named target matrix used by the static/temporal experiment.
pub fn named_targets(cols: u16, rows: u16, seed: u64) -> Vec<(String, LogicalImage)> {
    vec![
        ("smooth-gradient".into(), smooth_gradient(cols, rows, seed)),
        (
            "shallow-gradient".into(),
            shallow_gradient(cols, rows, seed),
        ),
        ("edge-018".into(), slanted_edge(cols, rows, 18.0)),
        ("edge-045".into(), slanted_edge(cols, rows, 45.0)),
        ("edge-072".into(), slanted_edge(cols, rows, 72.0)),
        ("line-030".into(), thin_line(cols, rows, 30.0)),
        ("line-060".into(), thin_line(cols, rows, 60.0)),
        ("checker-2".into(), checker(cols, rows, 2)),
        ("checker-3".into(), checker(cols, rows, 3)),
        ("radial".into(), radial(cols, rows, seed)),
        ("zone-plate".into(), zone_plate(cols, rows, seed)),
        ("portrait".into(), portrait(cols, rows, seed)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_image_has_expected_shape_and_is_deterministic() {
        let a = smooth_gradient(8, 3, 7);
        let b = smooth_gradient(8, 3, 7);
        assert_eq!(a.px.len(), 16 * 12);
        assert_eq!(a.px, b.px);
        // Every sample within range and monotone along the diagonal.
        assert!(a.at(0, 0)[0] < a.at(15, 11)[0]);
    }

    #[test]
    fn cell_samples_match_dot_order() {
        // Encode each logical sample's own coordinates so the dot-order mapping is
        // observable without clamping: value = 10*lx + ly.
        let img = LogicalImage::new(1, 1, TargetKind::Checker, |lx, ly| {
            let v = (10 * lx + ly) as u8;
            [v, v, v]
        });
        let s = img.cell_samples(0, 0);
        // IDX = [[0,3],[1,4],[2,5],[6,7]] maps [dy][dx] -> dot bit.
        assert_eq!(s[0][0], 0, "dot0: dx0,dy0");
        assert_eq!(s[3][0], 10, "dot3: dx1,dy0");
        assert_eq!(s[1][0], 1, "dot1: dx0,dy1");
        assert_eq!(s[4][0], 11, "dot4: dx1,dy1");
        assert_eq!(s[2][0], 2, "dot2: dx0,dy2");
        assert_eq!(s[5][0], 12, "dot5: dx1,dy2");
        assert_eq!(s[6][0], 3, "dot6: dx0,dy3");
        assert_eq!(s[7][0], 13, "dot7: dx1,dy3");
    }

    #[test]
    fn classify_separates_flat_and_edge() {
        let flat = [[128u8, 128, 128]; 8];
        assert_eq!(classify_cell(&flat), CellClass::Flat);
        let mut edge = [[20u8, 20, 20]; 8];
        for s in edge.iter_mut().take(4) {
            *s = [240, 240, 240];
        }
        assert_eq!(classify_cell(&edge), CellClass::Edge);
    }

    #[test]
    fn edge_target_is_a_soft_step() {
        let img = slanted_edge(4, 2, 0.0);
        let left = img.at(0, 4);
        let right = img.at(7, 4);
        assert!(
            left[0] < 60 && right[0] > 200,
            "left={left:?} right={right:?}"
        );
    }
}
