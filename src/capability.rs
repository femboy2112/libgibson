//! Terminal capability model and a central color-quality ladder.
//!
//! Capabilities are deliberately conservative: [`Capability::Unknown`] is never
//! treated as supported in safety-sensitive behavior. Detection uses safe,
//! passive sources (environment variables); active terminal queries may be added
//! later without changing this type.
//!
//! The color ladder is `TrueColor → Ansi256 → Ansi16 → Mono`. Application code
//! expresses intent (`gradient from A to B`); a central [`quantize_color`] step
//! decides the wire representation from [`TerminalCapabilities::color_depth`].

use crate::cell::Color;

/// Tri-state capability value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Capability {
    Supported,
    Unsupported,
    #[default]
    Unknown,
}

impl Capability {
    /// Only explicit support is trustworthy for safety-sensitive behavior.
    pub fn is_supported(self) -> bool {
        matches!(self, Capability::Supported)
    }
}

/// Color output quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorDepth {
    /// Glyph intensity only; no color attributes are emitted.
    Mono,
    /// The base 16 ANSI colors.
    Ansi16,
    /// The xterm 256-color palette.
    Ansi256,
    /// 24-bit RGB.
    #[default]
    TrueColor,
}

/// Passive capability snapshot for a terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCapabilities {
    pub color_depth: ColorDepth,
    pub synchronized_updates: Capability,
    /// `CSI L` (insert lines), used by the live-region fast path.
    pub insert_line: Capability,
    pub osc8_hyperlinks: Capability,
    pub kitty_keyboard: Capability,
    pub kitty_graphics: Capability,
    pub sixel: Capability,
    pub iterm_images: Capability,
}

impl Default for TerminalCapabilities {
    fn default() -> Self {
        Self::truecolor()
    }
}

impl TerminalCapabilities {
    pub fn mono() -> Self {
        Self {
            color_depth: ColorDepth::Mono,
            synchronized_updates: Capability::Unsupported,
            insert_line: Capability::Unknown,
            osc8_hyperlinks: Capability::Unknown,
            kitty_keyboard: Capability::Unknown,
            kitty_graphics: Capability::Unknown,
            sixel: Capability::Unknown,
            iterm_images: Capability::Unknown,
        }
    }

    pub fn ansi16() -> Self {
        Self {
            color_depth: ColorDepth::Ansi16,
            ..Self::truecolor()
        }
    }

    pub fn ansi256() -> Self {
        Self {
            color_depth: ColorDepth::Ansi256,
            ..Self::truecolor()
        }
    }

    pub fn truecolor() -> Self {
        Self {
            color_depth: ColorDepth::TrueColor,
            synchronized_updates: Capability::Supported,
            insert_line: Capability::Supported,
            osc8_hyperlinks: Capability::Unknown,
            kitty_keyboard: Capability::Unknown,
            kitty_graphics: Capability::Unknown,
            sixel: Capability::Unknown,
            iterm_images: Capability::Unknown,
        }
    }

    /// Detects capabilities from the process environment.
    pub fn detect_from_env() -> Self {
        Self::from_env(|k| std::env::var(k).ok())
    }

    /// Detects capabilities from an injectable environment getter (for tests).
    pub fn from_env(get: impl Fn(&str) -> Option<String>) -> Self {
        let no_color = get("NO_COLOR").map(|v| !v.is_empty()).unwrap_or(false);
        let term = get("TERM").unwrap_or_default();
        let colorterm = get("COLORTERM").unwrap_or_default();

        if no_color || term == "dumb" || term.is_empty() {
            return Self::mono();
        }

        let color_depth = if colorterm.eq_ignore_ascii_case("truecolor")
            || colorterm.eq_ignore_ascii_case("24bit")
        {
            ColorDepth::TrueColor
        } else if term.contains("256color") {
            ColorDepth::Ansi256
        } else {
            ColorDepth::Ansi16
        };

        Self {
            color_depth,
            // `CSI L` is ECMA-48 and supported by all common xterm-likes; we
            // mark it supported for any non-dumb terminal. Active probing can
            // refine this later.
            synchronized_updates: Capability::Unknown,
            insert_line: Capability::Supported,
            osc8_hyperlinks: Capability::Unknown,
            kitty_keyboard: Capability::Unknown,
            kitty_graphics: Capability::Unknown,
            sixel: Capability::Unknown,
            iterm_images: Capability::Unknown,
        }
    }
}

const BASE16: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (205, 49, 49),
    (13, 188, 121),
    (229, 229, 16),
    (36, 114, 200),
    (188, 63, 188),
    (17, 168, 205),
    (229, 229, 229),
    (102, 102, 102),
    (241, 76, 76),
    (35, 209, 139),
    (245, 245, 67),
    (59, 142, 234),
    (214, 112, 214),
    (41, 184, 219),
    (255, 255, 255),
];

fn nearest16(rgb: (u8, u8, u8)) -> u8 {
    let mut best = 0u8;
    let mut best_d = u32::MAX;
    for (i, c) in BASE16.iter().enumerate() {
        let d = dist2(rgb, *c);
        if d < best_d {
            best_d = d;
            best = i as u8;
        }
    }
    best
}

fn dist2(a: (u8, u8, u8), b: (u8, u8, u8)) -> u32 {
    let dr = a.0 as i32 - b.0 as i32;
    let dg = a.1 as i32 - b.1 as i32;
    let db = a.2 as i32 - b.2 as i32;
    (dr * dr + dg * dg + db * db) as u32
}

fn nearest256(rgb: (u8, u8, u8)) -> u8 {
    // Prefer the 6x6x6 color cube (16..=231); gray ramp is handled naturally by
    // the cube except for near-neutral values, which we snap to the ramp.
    let mut best = 16u8;
    let mut best_d = u32::MAX;
    for idx in 16u16..=231 {
        let i = idx as u8;
        let c = crate::cell::xterm_rgb(i);
        let d = dist2(rgb, c);
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    for idx in 232u16..=255 {
        let i = idx as u8;
        let c = crate::cell::xterm_rgb(i);
        let d = dist2(rgb, c);
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    best
}

/// True for the 16 base ANSI colors (no RGB approximation required).
fn is_base16(c: Color) -> bool {
    matches!(
        c,
        Color::Black
            | Color::Red
            | Color::Green
            | Color::Yellow
            | Color::Blue
            | Color::Magenta
            | Color::Cyan
            | Color::White
            | Color::BrightBlack
            | Color::BrightRed
            | Color::BrightGreen
            | Color::BrightYellow
            | Color::BrightBlue
            | Color::BrightMagenta
            | Color::BrightCyan
            | Color::BrightWhite
    )
}

/// Quantizes a color for the given depth. `None` means "emit no color attribute".
pub fn quantize_color(color: Color, depth: ColorDepth) -> Option<Color> {
    if color == Color::Reset {
        // Reset is an explicit "terminal default" instruction, not a color.
        return Some(Color::Reset);
    }
    match depth {
        ColorDepth::Mono => None,
        ColorDepth::TrueColor => Some(color),
        ColorDepth::Ansi256 => match color {
            c if is_base16(c) || matches!(c, Color::Ansi256(_)) => Some(color),
            other => Some(Color::Ansi256(nearest256(other.to_rgb()))),
        },
        ColorDepth::Ansi16 => {
            if is_base16(color) {
                Some(color)
            } else {
                Some(ansi16_from_index(nearest16(color.to_rgb())))
            }
        }
    }
}

fn ansi16_from_index(i: u8) -> Color {
    [
        Color::Black,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::White,
        Color::BrightBlack,
        Color::BrightRed,
        Color::BrightGreen,
        Color::BrightYellow,
        Color::BrightBlue,
        Color::BrightMagenta,
        Color::BrightCyan,
        Color::BrightWhite,
    ][(i & 0x0f) as usize]
}

/// Quantizes a whole style's colors for the given depth. Attributes are kept.
pub fn quantize_style(style: crate::cell::Style, depth: ColorDepth) -> crate::cell::Style {
    crate::cell::Style {
        fg: style.fg.and_then(|c| quantize_color(c, depth)),
        bg: style.bg.and_then(|c| quantize_color(c, depth)),
        ..style
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_env_disables_color() {
        let caps = TerminalCapabilities::from_env(|k| match k {
            "NO_COLOR" => Some("1".into()),
            _ => None,
        });
        assert_eq!(caps.color_depth, ColorDepth::Mono);
    }

    #[test]
    fn dumb_term_is_mono() {
        let caps = TerminalCapabilities::from_env(|k| match k {
            "TERM" => Some("dumb".into()),
            _ => None,
        });
        assert_eq!(caps.color_depth, ColorDepth::Mono);
    }

    #[test]
    fn colorterm_truecolor_detected() {
        let caps = TerminalCapabilities::from_env(|k| match k {
            "TERM" => Some("xterm-256color".into()),
            "COLORTERM" => Some("truecolor".into()),
            _ => None,
        });
        assert_eq!(caps.color_depth, ColorDepth::TrueColor);
    }

    #[test]
    fn plain_xterm_is_ansi16() {
        let caps = TerminalCapabilities::from_env(|k| match k {
            "TERM" => Some("xterm".into()),
            _ => None,
        });
        assert_eq!(caps.color_depth, ColorDepth::Ansi16);
    }

    #[test]
    fn quantization_ladder() {
        let rgb = Color::Rgb(200, 30, 90);
        assert_eq!(quantize_color(rgb, ColorDepth::Mono), None);
        assert_eq!(quantize_color(rgb, ColorDepth::TrueColor), Some(rgb));
        assert!(matches!(
            quantize_color(rgb, ColorDepth::Ansi256),
            Some(Color::Ansi256(_))
        ));
        assert!(matches!(
            quantize_color(rgb, ColorDepth::Ansi16),
            Some(Color::Red | Color::BrightRed | Color::Magenta | Color::BrightMagenta)
        ));
    }

    #[test]
    fn reset_is_preserved_as_default_not_a_color() {
        assert_eq!(
            quantize_color(Color::Reset, ColorDepth::Mono),
            Some(Color::Reset)
        );
        assert_eq!(
            quantize_color(Color::Reset, ColorDepth::Ansi16),
            Some(Color::Reset)
        );
    }

    #[test]
    fn capability_unknown_is_not_supported() {
        assert!(!Capability::Unknown.is_supported());
        assert!(!Capability::Unsupported.is_supported());
        assert!(Capability::Supported.is_supported());
    }
}
