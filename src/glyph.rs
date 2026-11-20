//! Sub-cell glyph *realization*: turning a logical 2×4 dot mask into a terminal
//! glyph through one of several glyph *families*.
//!
//! LibGibson's sub-cell graphics ([`BrailleCanvas`](crate::BrailleCanvas), the
//! intro wireframes, the field renderers, the FX Lab, the observatory
//! sparklines) all synthesize a **Braille** glyph `U+2800 + bits`, where `bits`
//! is an 8-bit mask over a 2×4 dot grid (see [`BrailleCanvas`](crate::BrailleCanvas) and its
//! `dot_bit`). Braille is the highest-resolution single-cell realization and the
//! hero path.
//!
//! # Glyph realization is a distinct capability axis
//!
//! Whether a terminal *realizes* a glyph is separate from what its *protocol*
//! supports (color depth, synchronized updates, `CSI L`, …). A terminal can
//! speak flawless UTF-8 and still render `U+2800..=U+28FF` as unrelated glyphs,
//! because the loaded console font has no Braille repertoire. This was observed
//! directly on the Linux kernel virtual console (`TERM=linux`, reached via
//! Ctrl+Alt+F1): ordinary text rendered fine, but Braille sub-cell graphics came
//! out as corrupt/unrelated glyphs. The kernel VT accepts Unicode input yet
//! renders through its loaded bitmap font + Unicode map, so *"UTF-8 terminal"*
//! does **not** imply *"the Braille block is displayable."* See `docs/GLYPHS.md`.
//!
//! There is no reliable, universal way to *query* a terminal's font repertoire,
//! so this module never pretends to detect it. It offers an explicit override
//! and a conservative `Auto` policy, and leaves visual confirmation to the human
//! (`cargo run --example glyph_capability_lab`).
//!
//! # What this module provides
//!
//! * [`SubcellGlyphMode`] — the concrete glyph families a 2×4 mask can be drawn
//!   with, from highest to lowest sub-cell resolution.
//! * [`SubcellGlyphMode::subcell_glyph`] — the single pure realization function.
//!   Returns `None` **iff** the mask is empty (`bits == 0`), so an empty logical
//!   sample stays empty in *every* mode and any non-empty sample is visible in
//!   every mode.
//! * [`GlyphChoice`] + [`detect_glyph_mode`] — the detection policy, resolving an
//!   explicit override (`--glyphs=`, `LIBGIBSON_GLYPHS`) or [`GlyphChoice::Auto`]
//!   into a concrete mode.
//! * [`transcode_surface_glyphs`] — the generic realization chokepoint. Because a
//!   Braille glyph losslessly encodes its mask (`bits = glyph - 0x2800`), any
//!   surface produced by the Braille path can be re-realized into another family
//!   in one pass, without threading a mode through every renderer.
//!   [`SubcellGlyphMode::Braille2x4`] is a no-op, so the hero path stays
//!   byte-identical.
//!
//! The glyph axis is deliberately **orthogonal to color**: realization never
//! touches a cell's style, and a lower-resolution glyph keeps the color it had.

use crate::cell::Glyph;
use crate::surface::Surface;

/// A glyph family for realizing a logical 2×4 sub-cell dot mask.
///
/// The variants form a fidelity ladder, highest sub-cell resolution first:
///
/// | mode           | sub-cells / cell | glyphs                | typical availability |
/// |----------------|------------------|-----------------------|----------------------|
/// | `Braille2x4`   | 2×4 = 8          | `U+2800..=U+28FF`     | modern UTF-8 fonts   |
/// | `HalfBlock1x2` | 1×2 = 2          | ` ▀▄█`                | ~universal (CP437)   |
/// | `Block`        | 1×1 density      | ` ░▒▓█`               | ~universal (CP437)   |
/// | `Ascii`        | 1×1 density      | ` .:-=+*#@`           | universal            |
///
/// `Block`/`HalfBlock`/`Ascii` reduce resolution by *aggregating* the mask
/// deterministically (vertical halves, or ink density), preserving topology and
/// readability rather than faking resolution.
/// `#[non_exhaustive]`: further sub-cell families (e.g. 2×3 sextants or 2×2
/// quadrants) are plausible future additions, so downstream `match`es must carry a
/// `_` arm and a new mode is a non-breaking `0.1.z` change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SubcellGlyphMode {
    /// Unicode Braille (`U+2800 + bits`): the full 2×4 sub-cell resolution and
    /// the hero path. Exactly what [`crate::BrailleCanvas::glyph_at`] emits.
    Braille2x4,
    /// Upper/lower half blocks (` ▀▄█`): the mask collapses to two vertical
    /// samples per cell. Part of the classic VGA/CP437 repertoire, so reliably
    /// present in Linux console fonts.
    HalfBlock1x2,
    /// Shade blocks (` ░▒▓█`) by ink density (dot popcount). One glyph per cell,
    /// intensity encodes coverage. Also CP437-native.
    Block,
    /// Pure-ASCII density ramp (` .:-=+*#@`): the ultimate fallback, realizable
    /// on any font whatsoever.
    Ascii,
}

/// Monotonic ASCII ink ramp indexed by dot popcount (`0..=8`). Index 0 is unused
/// (an empty mask realizes as `None`); indices `1..=8` map increasing coverage.
const ASCII_RAMP: [char; 9] = [' ', '.', ':', '-', '=', '+', '*', '#', '@'];

impl SubcellGlyphMode {
    /// Realizes a 2×4 dot `mask` as a single glyph in this mode.
    ///
    /// Returns `None` **iff** `mask == 0` (an empty logical sample), for every
    /// mode. Any non-empty mask yields `Some(glyph)`, so a lit sample is never
    /// silently dropped when falling back.
    ///
    /// The mask bit layout matches [`crate::BrailleCanvas`]: left column dots are
    /// `0x01/0x02/0x04` (rows 0–2) + `0x40` (row 3); right column dots are
    /// `0x08/0x10/0x20` (rows 0–2) + `0x80` (row 3).
    #[inline]
    pub fn subcell_glyph(self, mask: u8) -> Option<char> {
        match self {
            // Byte-identical to `BrailleCanvas::glyph_at`'s synthesis. Every
            // value `0x2800..=0x28FF` is an assigned code point, so `from_u32`
            // never yields `None` here for a non-zero mask.
            SubcellGlyphMode::Braille2x4 => {
                if mask == 0 {
                    None
                } else {
                    char::from_u32(0x2800 + mask as u32)
                }
            }
            // Vertical halves. Rows 0–1 (top) are bits 0x01|0x02|0x08|0x10 = 0x1B;
            // rows 2–3 (bottom) are 0x04|0x40|0x20|0x80 = 0xE4. The two masks are
            // disjoint and cover all 8 bits (0x1B ^ 0xE4 == 0xFF).
            SubcellGlyphMode::HalfBlock1x2 => {
                let top = mask & 0x1B != 0;
                let bottom = mask & 0xE4 != 0;
                match (top, bottom) {
                    (false, false) => None,
                    (true, false) => Some('▀'), // U+2580 upper half block
                    (false, true) => Some('▄'), // U+2584 lower half block
                    (true, true) => Some('█'),  // U+2588 full block
                }
            }
            // Ink density by popcount. Thresholds spread the 8 possible coverages
            // across the four shade levels; a full mask is always the full block.
            SubcellGlyphMode::Block => match mask.count_ones() {
                0 => None,
                1..=2 => Some('░'), // U+2591 light shade
                3..=5 => Some('▒'), // U+2592 medium shade
                6..=7 => Some('▓'), // U+2593 dark shade
                _ => Some('█'),     // 8 dots -> U+2588 full block
            },
            // Pure ASCII ramp by popcount.
            SubcellGlyphMode::Ascii => {
                let n = mask.count_ones() as usize;
                if n == 0 {
                    None
                } else {
                    Some(ASCII_RAMP[n])
                }
            }
        }
    }

    /// A human-readable name matching the `--glyphs=` / `LIBGIBSON_GLYPHS` value.
    pub fn as_str(self) -> &'static str {
        match self {
            SubcellGlyphMode::Braille2x4 => "braille",
            SubcellGlyphMode::HalfBlock1x2 => "halfblock",
            SubcellGlyphMode::Block => "block",
            SubcellGlyphMode::Ascii => "ascii",
        }
    }
}

/// A glyph-realization *preference*: a fixed mode or the `Auto` policy.
///
/// This is what a CLI flag or environment variable parses into; call
/// [`GlyphChoice::resolve`] (or use [`detect_glyph_mode`]) to turn it into a
/// concrete [`SubcellGlyphMode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlyphChoice {
    /// Resolve from the environment (currently `TERM`) via a conservative policy.
    Auto,
    /// Use exactly this mode.
    Fixed(SubcellGlyphMode),
}

impl GlyphChoice {
    /// Parses `auto|braille|halfblock|block|ascii` (case-insensitive, with a few
    /// friendly aliases). Returns `None` for an unrecognized value.
    pub fn parse(s: &str) -> Option<GlyphChoice> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(GlyphChoice::Auto),
            "braille" | "braille2x4" => Some(GlyphChoice::Fixed(SubcellGlyphMode::Braille2x4)),
            "halfblock" | "half-block" | "half" => {
                Some(GlyphChoice::Fixed(SubcellGlyphMode::HalfBlock1x2))
            }
            "block" | "blocks" | "shade" => Some(GlyphChoice::Fixed(SubcellGlyphMode::Block)),
            "ascii" | "text" => Some(GlyphChoice::Fixed(SubcellGlyphMode::Ascii)),
            _ => None,
        }
    }

    /// Resolves to a concrete mode, reading the environment via `get_env` when
    /// the choice is [`GlyphChoice::Auto`].
    pub fn resolve(self, get_env: impl Fn(&str) -> Option<String>) -> SubcellGlyphMode {
        match self {
            GlyphChoice::Fixed(m) => m,
            GlyphChoice::Auto => auto_mode(&get_env),
        }
    }
}

/// The `Auto` policy: conservative on the Linux kernel VT, Braille elsewhere.
fn auto_mode(get_env: &impl Fn(&str) -> Option<String>) -> SubcellGlyphMode {
    match get_env("TERM").as_deref() {
        // The Linux kernel virtual console accepts UTF-8 but its default console
        // fonts (e.g. `default8x16`) realize `U+2800..=U+28FF` unreliably, while
        // block elements (`U+2580..=U+259F`, classic VGA/CP437) are reliably
        // present. Auto therefore chooses a conservative, structure-preserving
        // realization here. This is a *policy* default, NOT a claim that Braille
        // is unavailable: a custom console font can contain it, and
        // `--glyphs=braille` requests it explicitly.
        Some("linux") => SubcellGlyphMode::HalfBlock1x2,
        // Ordinary graphical terminals keep the current (Braille) behavior.
        _ => SubcellGlyphMode::Braille2x4,
    }
}

/// Resolves the sub-cell glyph mode with precedence
/// `--glyphs=` (`cli`) **>** `LIBGIBSON_GLYPHS` **>** `Auto`(`TERM`).
///
/// `get_env` is injected for testability (mirroring
/// [`crate::TerminalCapabilities::from_env`]). Returns `Err` with a message for
/// an unrecognized explicit value.
pub fn detect_glyph_mode(
    cli: Option<&str>,
    get_env: impl Fn(&str) -> Option<String>,
) -> Result<SubcellGlyphMode, String> {
    let choice = if let Some(v) = cli {
        GlyphChoice::parse(v).ok_or_else(|| format!("unknown --glyphs value: {v}"))?
    } else if let Some(v) = get_env("LIBGIBSON_GLYPHS") {
        GlyphChoice::parse(&v).ok_or_else(|| format!("unknown LIBGIBSON_GLYPHS value: {v}"))?
    } else {
        GlyphChoice::Auto
    };
    Ok(choice.resolve(&get_env))
}

/// Convenience wrapper over [`detect_glyph_mode`] reading the real process
/// environment.
pub fn detect_glyph_mode_from_env(cli: Option<&str>) -> Result<SubcellGlyphMode, String> {
    detect_glyph_mode(cli, |k| std::env::var(k).ok())
}

/// The 2×4 dot mask carried by a single-character Braille grapheme, if `g` is
/// exactly one Braille code point (`U+2800..=U+28FF`).
#[inline]
fn braille_bits(g: &str) -> Option<u8> {
    let mut it = g.chars();
    let c = it.next()?;
    if it.next().is_some() {
        return None; // multi-char grapheme: not a bare braille cell
    }
    let u = c as u32;
    if (0x2800..=0x28FF).contains(&u) {
        Some((u - 0x2800) as u8)
    } else {
        None
    }
}

/// Re-realizes every Braille cell in `surface` into `mode`, in place.
///
/// This is the single generic realization chokepoint. Any surface produced by
/// the Braille path carries its 2×4 mask losslessly in each `U+2800 + bits`
/// glyph, so one pass converts the whole surface into a lower-resolution family
/// **without** touching non-Braille content (text, box drawing, RGB
/// half-blocks) and **without** touching any cell's style — the glyph axis stays
/// orthogonal to color.
///
/// [`SubcellGlyphMode::Braille2x4`] returns immediately, so the hero path is
/// byte-identical (zero risk of drift). An empty Braille cell (`U+2800` itself,
/// which the dithered raster path can emit) becomes a blank space while keeping
/// its background.
pub fn transcode_surface_glyphs(surface: &mut Surface, mode: SubcellGlyphMode) {
    if mode == SubcellGlyphMode::Braille2x4 {
        return;
    }
    for cell in surface.cells.iter_mut() {
        // Braille glyphs are always narrow (display_width 1) and never wide
        // continuations, but guard anyway so we never rewrite a continuation.
        if cell.is_continuation || cell.transparent {
            continue;
        }
        let Some(bits) = braille_bits(cell.glyph.grapheme.as_str()) else {
            continue;
        };
        match mode.subcell_glyph(bits) {
            Some(ch) => cell.glyph = Glyph::new(&ch.to_string()),
            None => cell.glyph = Glyph::space(),
        }
    }
}

// ---------------------------------------------------------------------------
// v0.9.0 opt-in cell encoders. Design: docs/design/v0.9-cell-encoders.md.
//
// `CellEncoder::HalfBlock` is the default and stays byte-identical to
// `RgbRaster::to_surface`; every other encoder is opt-in.
// ---------------------------------------------------------------------------

/// How an [`RgbRaster`](crate::raster::RgbRaster) is realized as terminal cells.
///
/// Every encoder emits ordinary one-column glyphs with at most two colours per
/// cell (foreground and background), so a denser encoder raises *edge and
/// luminance* detail, not chroma. [`CellEncoder::HalfBlock`] is the default and
/// is the exact, byte-identical `RgbRaster::to_surface` output.
///
/// `#[non_exhaustive]`: further families may be added in a `0.y.z` release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CellEncoder {
    /// `▀` with foreground = top sample and background = bottom sample (1×2).
    #[default]
    HalfBlock,
    /// Quadrant blocks `U+2596..=U+259F` plus ` ▀▄▌▐█` (2×2).
    Quadrant,
    /// Sextants `U+1FB00..=U+1FB3B` plus ` ▌▐█` (2×3).
    Sextant,
    /// Octants `U+1CD00..=U+1CDE5` plus existing block characters (2×4).
    /// Needs a terminal that draws them natively; see [`allowed_encoder`].
    Octant,
    /// Braille `U+2800 + bits`, fitted by the two-colour projector (2×4).
    Braille,
    /// Per cell, the family with the least linear-light error among
    /// half-block, quadrant and sextant (2×6 shared sample tile).
    Auto,
}

impl CellEncoder {
    /// Samples per cell as `(columns, rows)`: the grid a raster must be
    /// supplied at so one cell reads exactly that many samples.
    pub const fn sample_grid(self) -> (u16, u16) {
        match self {
            CellEncoder::HalfBlock => (1, 2),
            CellEncoder::Quadrant => (2, 2),
            CellEncoder::Sextant => (2, 3),
            CellEncoder::Octant | CellEncoder::Braille => (2, 4),
            CellEncoder::Auto => (2, 6),
        }
    }
}

/// The sextant character for a 2×3 `pattern` (bit 0 = top-left, bit 1 =
/// top-right, bit 2 = middle-left, bit 3 = middle-right, bit 4 = bottom-left,
/// bit 5 = bottom-right). Bits above bit 5 are ignored.
///
/// Pattern 0 is `' '`, 21 is `▌`, 42 is `▐`, 63 is `█`; every other pattern
/// `p` is `U+1FB00 + p - 1 - (p > 21) - (p > 42)`.
pub fn sextant_char(pattern: u8) -> char {
    match pattern & 0x3F {
        0 => ' ',
        21 => '▌',
        42 => '▐',
        63 => '█',
        p => {
            let skipped = u32::from(p > 21) + u32::from(p > 42);
            // 1..=62 minus {21, 42} lands in U+1FB00..=U+1FB3B, all assigned.
            char::from_u32(0x1FB00 + u32::from(p) - 1 - skipped).unwrap_or(' ')
        }
    }
}

/// The 26 octant patterns (bit `2 * row + column`, row 0 on top) whose shape
/// already has a character outside `U+1CD00..=U+1CDE5`.
const fn legacy_octant_shape(pattern: u8) -> Option<char> {
    Some(match pattern {
        0x00 => ' ',
        0xFF => '█',
        0x0F => '▀',
        0xF0 => '▄',
        0x55 => '▌',
        0xAA => '▐',
        0x05 => '▘',
        0x0A => '▝',
        0x50 => '▖',
        0xA0 => '▗',
        0xA5 => '▚',
        0x5A => '▞',
        0xF5 => '▙',
        0x5F => '▛',
        0xAF => '▜',
        0xFA => '▟',
        0x03 => '\u{1FB82}', // upper one quarter block
        0x3F => '\u{1FB85}', // upper three quarters block
        0xC0 => '\u{2582}',  // lower one quarter block
        0xFC => '\u{2586}',  // lower three quarters block
        0x01 => '\u{1CEA8}', // left half upper one quarter block
        0x02 => '\u{1CEAB}', // right half upper one quarter block
        0x40 => '\u{1CEA3}', // left half lower one quarter block
        0x80 => '\u{1CEA0}', // right half lower one quarter block
        0x14 => '\u{1FBE6}', // middle left one quarter block
        0x28 => '\u{1FBE7}', // middle right one quarter block
        _ => return None,
    })
}

/// The 256-entry octant table, built at compile time: the 230 patterns without
/// another character take `U+1CD00 + rank` in ascending pattern order.
const OCTANT_TABLE: [char; 256] = build_octant_table();

const fn build_octant_table() -> [char; 256] {
    let mut table = [' '; 256];
    let mut next = 0x1CD00u32;
    let mut pattern = 0usize;
    while pattern < 256 {
        table[pattern] = match legacy_octant_shape(pattern as u8) {
            Some(shape) => shape,
            None => {
                // A bad codepoint is a compile-time error here, not a silent blank.
                let Some(octant) = char::from_u32(next) else {
                    panic!("octant codepoint out of range")
                };
                next += 1;
                octant
            }
        };
        pattern += 1;
    }
    table
}

/// The octant character for a 2×4 `pattern` (bit `2 * row + column`, row 0 on
/// top, column 0 on the left). Total over all 256 patterns and a bijection.
///
/// Patterns with a Unicode 16 octant (230 of them, `U+1CD00..=U+1CDE5`, in
/// ascending pattern order) map there; the other 26 map to the existing block
/// character with the same shape (space, `█`, halves, quadrants and so on).
pub fn octant_char(pattern: u8) -> char {
    OCTANT_TABLE[usize::from(pattern)]
}

/// What a terminal can be trusted to draw: the glyph families above half-block.
#[derive(Clone, Copy)]
struct EncoderCaps {
    quadrant: bool,
    braille: bool,
    sextant: bool,
    octant: bool,
}

impl EncoderCaps {
    /// Half-block only (the CP437 block set).
    const NONE: Self = Self {
        quadrant: false,
        braille: false,
        sextant: false,
        octant: false,
    };
    /// Quadrant and Braille, never sextant or octant.
    const BASIC: Self = Self {
        quadrant: true,
        braille: true,
        sextant: false,
        octant: false,
    };
    /// Up to sextant.
    const SEXTANT: Self = Self {
        quadrant: true,
        braille: true,
        sextant: true,
        octant: false,
    };
    const ALL: Self = Self {
        quadrant: true,
        braille: true,
        sextant: true,
        octant: true,
    };
}

/// An explicit `--glyphs=` / `LIBGIBSON_GLYPHS` value as a ceiling, or `None`
/// for `auto` and for values this gate does not recognise (they never upgrade).
fn explicit_caps(value: &str) -> Option<EncoderCaps> {
    match value.trim().to_ascii_lowercase().as_str() {
        "octant" | "octants" => Some(EncoderCaps::ALL),
        "sextant" | "sextants" => Some(EncoderCaps::SEXTANT),
        "quadrant" | "quadrants" => Some(EncoderCaps {
            quadrant: true,
            ..EncoderCaps::NONE
        }),
        "braille" | "braille2x4" => Some(EncoderCaps {
            braille: true,
            ..EncoderCaps::NONE
        }),
        "halfblock" | "half-block" | "half" | "block" | "blocks" | "shade" | "ascii" | "text" => {
            Some(EncoderCaps::NONE)
        }
        _ => None,
    }
}

/// The passive-environment ceiling. Unknown terminals get [`EncoderCaps::BASIC`].
fn environment_caps(get_env: &impl Fn(&str) -> Option<String>) -> EncoderCaps {
    let term = get_env("TERM").unwrap_or_default().to_ascii_lowercase();
    if term.is_empty() || term == "dumb" || term == "linux" {
        return EncoderCaps::NONE;
    }
    if let Some(vte) = get_env("VTE_VERSION").and_then(|v| v.trim().parse::<u32>().ok()) {
        return if vte >= 7700 {
            EncoderCaps::ALL
        } else if vte >= 7600 {
            EncoderCaps::SEXTANT
        } else {
            EncoderCaps::BASIC
        };
    }
    let program = get_env("TERM_PROGRAM").unwrap_or_default();
    if term.contains("kitty") || term.contains("ghostty") || program.eq_ignore_ascii_case("ghostty")
    {
        return EncoderCaps::ALL;
    }
    let modern_term = term == "alacritty"
        || term.starts_with("alacritty-")
        || term == "foot"
        || term.starts_with("foot-");
    if modern_term
        || program.eq_ignore_ascii_case("wezterm")
        || program.eq_ignore_ascii_case("iterm.app")
        || get_env("WT_SESSION").is_some_and(|v| !v.is_empty())
    {
        return EncoderCaps::SEXTANT;
    }
    EncoderCaps::BASIC
}

/// The most capable encoder the terminal can be trusted with, at most
/// `requested`; passive and pure.
///
/// `cli` is a `--glyphs=` value and `get_env` reads `LIBGIBSON_GLYPHS`, `TERM`,
/// `TERM_PROGRAM`, `VTE_VERSION` and `WT_SESSION`, as [`detect_glyph_mode`]
/// does. An unknown terminal never upgrades. [`CellEncoder::Auto`] searches half-block,
/// quadrant and sextant glyphs, so it is returned unchanged only where sextants are
/// allowed and degrades to half-block elsewhere.
///
/// Ladder: `Octant -> Sextant -> HalfBlock`; `Quadrant` and `Braille` degrade
/// straight to `HalfBlock`. The ceiling comes from the first recognised explicit
/// value (`cli`, then `LIBGIBSON_GLYPHS`, a grant or a cap), else from the
/// environment (`TERM` `linux`/`dumb`/empty: half-block only; `VTE_VERSION` at
/// least 7700: everything, at least 7600: up to sextant, below: quadrant and
/// Braille; kitty or ghostty: everything; WezTerm, iTerm2, Alacritty, foot or
/// Windows Terminal: up to sextant; any other terminal: quadrant and Braille).
pub fn allowed_encoder(
    requested: CellEncoder,
    cli: Option<&str>,
    get_env: impl Fn(&str) -> Option<String>,
) -> CellEncoder {
    let caps = cli
        .and_then(explicit_caps)
        .or_else(|| {
            get_env("LIBGIBSON_GLYPHS")
                .as_deref()
                .and_then(explicit_caps)
        })
        .unwrap_or_else(|| environment_caps(&get_env));
    match requested {
        CellEncoder::HalfBlock => CellEncoder::HalfBlock,
        CellEncoder::Quadrant if caps.quadrant => CellEncoder::Quadrant,
        CellEncoder::Braille if caps.braille => CellEncoder::Braille,
        CellEncoder::Sextant if caps.sextant => CellEncoder::Sextant,
        CellEncoder::Octant if caps.octant => CellEncoder::Octant,
        CellEncoder::Octant if caps.sextant => CellEncoder::Sextant,
        CellEncoder::Auto if caps.sextant => CellEncoder::Auto,
        CellEncoder::Quadrant
        | CellEncoder::Braille
        | CellEncoder::Sextant
        | CellEncoder::Octant
        | CellEncoder::Auto => CellEncoder::HalfBlock,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::{Cell, Color, Style};

    const MODES: [SubcellGlyphMode; 4] = [
        SubcellGlyphMode::Braille2x4,
        SubcellGlyphMode::HalfBlock1x2,
        SubcellGlyphMode::Block,
        SubcellGlyphMode::Ascii,
    ];

    /// Braille2x4 must reproduce `BrailleCanvas::glyph_at`'s formula exactly, for
    /// every possible mask. This is the hero-path non-regression contract.
    #[test]
    fn braille_mode_matches_legacy_formula() {
        for bits in 0u16..=255 {
            let bits = bits as u8;
            let expected = if bits == 0 {
                None
            } else {
                char::from_u32(0x2800 + bits as u32)
            };
            assert_eq!(SubcellGlyphMode::Braille2x4.subcell_glyph(bits), expected);
        }
    }

    /// Empty logical samples stay empty in every mode.
    #[test]
    fn empty_mask_is_none_in_every_mode() {
        for m in MODES {
            assert_eq!(m.subcell_glyph(0), None, "{m:?}");
        }
    }

    /// Any non-empty mask is visible (Some) in every mode: fallback never drops
    /// a lit sample.
    #[test]
    fn nonempty_mask_is_some_in_every_mode() {
        for m in MODES {
            for bits in 1u16..=255 {
                assert!(m.subcell_glyph(bits as u8).is_some(), "{m:?} bits={bits}");
            }
        }
    }

    /// Full masks are the most-solid glyph in every mode.
    #[test]
    fn full_mask_is_full_ink() {
        assert_eq!(SubcellGlyphMode::Braille2x4.subcell_glyph(0xFF), Some('⣿'));
        assert_eq!(
            SubcellGlyphMode::HalfBlock1x2.subcell_glyph(0xFF),
            Some('█')
        );
        assert_eq!(SubcellGlyphMode::Block.subcell_glyph(0xFF), Some('█'));
        assert_eq!(SubcellGlyphMode::Ascii.subcell_glyph(0xFF), Some('@'));
    }

    /// Half-block partitions the mask into vertical halves.
    #[test]
    fn halfblock_vertical_halves() {
        let hb = SubcellGlyphMode::HalfBlock1x2;
        // Top-row-only dots (0x01 left, 0x08 right) -> upper half.
        assert_eq!(hb.subcell_glyph(0x01), Some('▀'));
        assert_eq!(hb.subcell_glyph(0x08), Some('▀'));
        assert_eq!(hb.subcell_glyph(0x02 | 0x10), Some('▀'));
        // Bottom-row-only dots (0x04/0x40 left, 0x20/0x80 right) -> lower half.
        assert_eq!(hb.subcell_glyph(0x04), Some('▄'));
        assert_eq!(hb.subcell_glyph(0x40), Some('▄'));
        assert_eq!(hb.subcell_glyph(0x80), Some('▄'));
        // One dot in each half -> full.
        assert_eq!(hb.subcell_glyph(0x01 | 0x80), Some('█'));
    }

    /// Block shade rises monotonically with coverage.
    #[test]
    fn block_density_ramp() {
        let b = SubcellGlyphMode::Block;
        assert_eq!(b.subcell_glyph(0b0000_0001), Some('░')); // 1 dot
        assert_eq!(b.subcell_glyph(0b0000_0011), Some('░')); // 2
        assert_eq!(b.subcell_glyph(0b0000_0111), Some('▒')); // 3
        assert_eq!(b.subcell_glyph(0b0001_1111), Some('▒')); // 5
        assert_eq!(b.subcell_glyph(0b0011_1111), Some('▓')); // 6
        assert_eq!(b.subcell_glyph(0b0111_1111), Some('▓')); // 7
        assert_eq!(b.subcell_glyph(0b1111_1111), Some('█')); // 8
    }

    /// ASCII ramp is monotonic in ink and pure ASCII.
    #[test]
    fn ascii_ramp_monotonic_and_ascii() {
        let a = SubcellGlyphMode::Ascii;
        let seq: Vec<char> = (1u16..=8)
            .map(|n| {
                // A mask with exactly `n` bits set.
                let bits = ((1u16 << n) - 1) as u8;
                a.subcell_glyph(bits).unwrap()
            })
            .collect();
        assert_eq!(seq, vec!['.', ':', '-', '=', '+', '*', '#', '@']);
        for c in seq {
            assert!(c.is_ascii(), "{c:?} not ASCII");
        }
    }

    #[test]
    fn parse_round_trips_and_rejects_garbage() {
        assert_eq!(GlyphChoice::parse("auto"), Some(GlyphChoice::Auto));
        assert_eq!(GlyphChoice::parse("AUTO"), Some(GlyphChoice::Auto));
        for m in MODES {
            assert_eq!(
                GlyphChoice::parse(m.as_str()),
                Some(GlyphChoice::Fixed(m)),
                "{m:?}"
            );
        }
        assert_eq!(
            GlyphChoice::parse("half-block"),
            GlyphChoice::parse("halfblock")
        );
        assert_eq!(GlyphChoice::parse("nonsense"), None);
    }

    /// Detection precedence: CLI > env > auto(TERM).
    #[test]
    fn detection_precedence_and_auto_policy() {
        fn env(
            term: Option<&'static str>,
            glyphs: Option<&'static str>,
        ) -> impl Fn(&str) -> Option<String> {
            let term = term.map(String::from);
            let glyphs = glyphs.map(String::from);
            move |k: &str| match k {
                "TERM" => term.clone(),
                "LIBGIBSON_GLYPHS" => glyphs.clone(),
                _ => None,
            }
        }
        // Auto on a graphical terminal -> Braille (current behavior preserved).
        assert_eq!(
            detect_glyph_mode(None, env(Some("xterm-256color"), None)).unwrap(),
            SubcellGlyphMode::Braille2x4
        );
        // Auto on the Linux VT -> conservative half-block (POLICY, not a font claim).
        assert_eq!(
            detect_glyph_mode(None, env(Some("linux"), None)).unwrap(),
            SubcellGlyphMode::HalfBlock1x2
        );
        // Env override beats auto.
        assert_eq!(
            detect_glyph_mode(None, env(Some("linux"), Some("ascii"))).unwrap(),
            SubcellGlyphMode::Ascii
        );
        // CLI beats env: explicit braille on the Linux VT still forces Braille.
        assert_eq!(
            detect_glyph_mode(Some("braille"), env(Some("linux"), Some("ascii"))).unwrap(),
            SubcellGlyphMode::Braille2x4
        );
        // Explicit auto re-reads TERM.
        assert_eq!(
            detect_glyph_mode(Some("auto"), env(Some("linux"), None)).unwrap(),
            SubcellGlyphMode::HalfBlock1x2
        );
        // Garbage explicit value errors.
        assert!(detect_glyph_mode(Some("garbage"), env(None, None)).is_err());
    }

    fn braille_surface() -> Surface {
        // A 3x1 surface: [lit-braille, blank-braille U+2800, plain text 'X'].
        let mut s = Surface::new(3, 1);
        let style = Style::new().fg(Color::Rgb(1, 2, 3));
        s.set_cell(0, 0, Cell::new(Glyph::new(&'\u{28FF}'.to_string()), style));
        s.set_cell(1, 0, Cell::new(Glyph::new(&'\u{2800}'.to_string()), style));
        s.set_cell(2, 0, Cell::new(Glyph::new("X"), style));
        s
    }

    #[test]
    fn transcode_braille_is_a_noop() {
        let s = braille_surface();
        let mut t = s.clone();
        transcode_surface_glyphs(&mut t, SubcellGlyphMode::Braille2x4);
        assert_eq!(s, t, "Braille2x4 transcode must be identity");
    }

    #[test]
    fn transcode_lowers_resolution_and_preserves_style_and_text() {
        for mode in [
            SubcellGlyphMode::HalfBlock1x2,
            SubcellGlyphMode::Block,
            SubcellGlyphMode::Ascii,
        ] {
            let mut s = braille_surface();
            transcode_surface_glyphs(&mut s, mode);
            // Lit full mask -> the mode's full-ink glyph.
            assert_eq!(
                s.get(0, 0).unwrap().glyph.grapheme.as_str(),
                mode.subcell_glyph(0xFF).unwrap().to_string(),
                "{mode:?} lit cell",
            );
            // Blank braille (U+2800) -> a space.
            assert_eq!(
                s.get(1, 0).unwrap().glyph.grapheme.as_str(),
                " ",
                "{mode:?} blank"
            );
            // Plain text is untouched.
            assert_eq!(
                s.get(2, 0).unwrap().glyph.grapheme.as_str(),
                "X",
                "{mode:?} text"
            );
            // Style is preserved on every cell (orthogonal to color).
            for x in 0..3 {
                assert_eq!(
                    s.get(x, 0).unwrap().style.fg,
                    Some(Color::Rgb(1, 2, 3)),
                    "{mode:?} style at {x}",
                );
            }
            // No Braille code points survive a non-Braille transcode.
            for c in &s.cells {
                if let Some(ch) = c.glyph.grapheme.chars().next() {
                    assert!(
                        !(0x2800..=0x28FF).contains(&(ch as u32)),
                        "{mode:?} leaked braille {ch:?}",
                    );
                }
            }
        }
    }
}
