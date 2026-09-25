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
