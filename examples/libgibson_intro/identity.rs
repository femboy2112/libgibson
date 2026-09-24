//! Four enduring marks: the same computation becomes a row, a building,
//! a courier and, finally, a planetary signal. These are film art, not core API.
use gibson::cell::Color;
use gibson::raster::Rgb;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisualIdentity {
    pub accent: Rgb,
    pub signature: &'static str,
    pub short: &'static str,
}
pub const IDENTITIES: [VisualIdentity; 4] = [
    VisualIdentity {
        accent: (76, 218, 244),
        signature: "⌂",
        short: "A",
    },
    VisualIdentity {
        accent: (249, 193, 95),
        signature: "◇",
        short: "S",
    },
    VisualIdentity {
        accent: (174, 143, 251),
        signature: "▥",
        short: "B",
    },
    VisualIdentity {
        accent: (166, 246, 215),
        signature: "⊞",
        short: "V",
    },
];
impl VisualIdentity {
    pub const fn color(self) -> Color {
        Color::Rgb(self.accent.0, self.accent.1, self.accent.2)
    }
}
/// Normalized node sites shared by the orchestration graph and its membrane.
pub const GRAPH_POINTS: [(f32, f32); 4] = [(0.12, 0.50), (0.38, 0.25), (0.63, 0.70), (0.88, 0.40)];

/// The completed agent rows are the first blisters in a wide composition.
/// Narrow layouts omit those rows, so their visible graph becomes the source.
/// Coordinates are normalized screen positions, independent of raster samples.
pub fn membrane_anchors(width: u16, height: u16) -> [(f32, f32); 4] {
    if width >= 74 {
        std::array::from_fn(|i| {
            (
                if width >= 110 { 0.15 } else { 0.17 },
                ((9.5 + i as f32 * 4.0) / height.max(1) as f32).clamp(0.18, 0.88),
            )
        })
    } else {
        GRAPH_POINTS.map(|(x, y)| (0.06 + x * 0.88, 0.63 + y * 0.18))
    }
}
