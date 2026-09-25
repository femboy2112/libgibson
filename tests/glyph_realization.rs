//! Glyph-realization non-regression + fallback witnesses on the REAL intro film.
//!
//! The exhaustive per-mask realization contract lives in `src/glyph.rs`'s unit
//! tests. This file proves the properties hold on actual rendered film frames at
//! the same witness times the cinematic goldens use:
//!
//!   * the Braille (hero) path is byte-identical after the transcode chokepoint
//!     — a transcode in `Braille2x4` mode is a no-op on a genuine frame;
//!   * every fallback family leaves NO Braille code point behind, preserves each
//!     cell's style (glyph axis orthogonal to color), preserves dimensions, and
//!     never drops a lit sub-cell sample;
//!   * the `TERM=linux` Auto POLICY realizes the film without Braille, while an
//!     explicit `--glyphs=braille` override still produces the hero output.
//!
//! This is POLICY / logical evidence, not proof of any physical console font:
//! terminal protocols cannot report a font's glyph repertoire.

#[allow(dead_code)]
#[path = "../examples/libgibson_intro.rs"]
mod intro;

use gibson::cell::Cell;
use gibson::{detect_glyph_mode, transcode_surface_glyphs, ColorDepth, SubcellGlyphMode, Surface};
use intro::director::Director;

/// The cinematic witness times (matching `tests/intro_cinema.rs`).
const WITNESS: [f32; 7] = [12., 23., 31., 39., 47., 58., 68.];
const W: u16 = 120;
const H: u16 = 40;

const FALLBACKS: [SubcellGlyphMode; 3] = [
    SubcellGlyphMode::HalfBlock1x2,
    SubcellGlyphMode::Block,
    SubcellGlyphMode::Ascii,
];

fn at(seconds: f32) -> Director {
    let mut d = Director::default();
    d.seek(seconds);
    d
}

fn hero_frame(seconds: f32) -> Surface {
    intro::frame(&at(seconds), W, H, ColorDepth::TrueColor, false)
}

fn cell_char(c: &Cell) -> Option<char> {
    c.glyph.grapheme.chars().next()
}

fn is_braille(ch: char) -> bool {
    (0x2800..=0x28FF).contains(&(ch as u32))
}

/// A lit Braille cell carries a non-zero 2×4 mask (blank braille U+2800 is not
/// "lit").
fn is_lit_braille(ch: char) -> bool {
    let u = ch as u32;
    (0x2801..=0x28FF).contains(&u)
}

/// The hero (Braille) path must be byte-identical after the transcode chokepoint
/// at every witness time — this is the non-regression contract.
#[test]
fn braille_transcode_is_identity_on_real_frames() {
    for t in WITNESS {
        let base = hero_frame(t);
        let mut same = base.clone();
        transcode_surface_glyphs(&mut same, SubcellGlyphMode::Braille2x4);
        assert_eq!(base, same, "Braille2x4 transcode changed the frame at t={t}");
    }
}

/// Across the witnesses, the film genuinely renders Braille sub-cell graphics —
/// otherwise the fallback tests would be vacuous.
#[test]
fn film_actually_uses_braille() {
    let total_lit: usize = WITNESS
        .iter()
        .map(|&t| {
            hero_frame(t)
                .cells
                .iter()
                .filter_map(cell_char)
                .filter(|&c| is_lit_braille(c))
                .count()
        })
        .sum();
    assert!(
        total_lit > 100,
        "expected substantial Braille content across witnesses, saw {total_lit}"
    );
}

/// Every fallback family: no Braille survives, dimensions and per-cell style are
/// preserved, and no lit sub-cell sample is silently dropped.
#[test]
fn fallback_modes_are_well_formed_on_real_frames() {
    for mode in FALLBACKS {
        for t in WITNESS {
            let base = hero_frame(t);
            let mut f = base.clone();
            transcode_surface_glyphs(&mut f, mode);

            assert_eq!(
                (f.width, f.height),
                (base.width, base.height),
                "{mode:?} changed dimensions at t={t}"
            );

            for (bc, fc) in base.cells.iter().zip(f.cells.iter()) {
                // Glyph realization must never touch color/attributes.
                assert_eq!(bc.style, fc.style, "{mode:?} altered style at t={t}");
                match cell_char(bc) {
                    // A lit Braille cell must become a lit (non-space) glyph.
                    Some(ch) if is_lit_braille(ch) => {
                        let fch = cell_char(fc).expect("realized cell has a glyph");
                        assert_ne!(fch, ' ', "{mode:?} dropped a lit sample at t={t}");
                        assert!(!is_braille(fch), "{mode:?} left braille at t={t}");
                    }
                    // Blank Braille (U+2800) becomes a space.
                    Some(ch) if is_braille(ch) => {
                        assert_eq!(cell_char(fc), Some(' '), "{mode:?} blank braille at t={t}");
                    }
                    // Non-Braille content is untouched.
                    other => {
                        assert_eq!(cell_char(fc), other, "{mode:?} touched non-braille at t={t}");
                    }
                }
            }

            // Belt and suspenders: scan the whole frame for any surviving braille.
            for c in &f.cells {
                if let Some(ch) = cell_char(c) {
                    assert!(!is_braille(ch), "{mode:?} leaked braille {ch:?} at t={t}");
                }
            }
        }
    }
}

/// The `TERM=linux` Auto policy resolves to a console-safe family and realizes
/// the film with no Braille. POLICY evidence, not a font claim.
#[test]
fn linux_vt_auto_policy_emits_no_braille() {
    let get_env = |k: &str| match k {
        "TERM" => Some("linux".to_string()),
        _ => None,
    };
    let mode = detect_glyph_mode(None, get_env).unwrap();
    assert_ne!(mode, SubcellGlyphMode::Braille2x4, "auto+linux must not be Braille");

    for t in WITNESS {
        let mut f = hero_frame(t);
        transcode_surface_glyphs(&mut f, mode);
        for c in &f.cells {
            if let Some(ch) = cell_char(c) {
                assert!(!is_braille(ch), "auto+linux leaked braille at t={t}");
            }
        }
    }
}

/// An explicit `--glyphs=braille` override still produces the hero Braille
/// output even under `TERM=linux`.
#[test]
fn explicit_braille_override_keeps_hero() {
    let get_env = |k: &str| match k {
        "TERM" => Some("linux".to_string()),
        _ => None,
    };
    let mode = detect_glyph_mode(Some("braille"), get_env).unwrap();
    assert_eq!(mode, SubcellGlyphMode::Braille2x4);

    for t in WITNESS {
        let base = hero_frame(t);
        let mut f = base.clone();
        transcode_surface_glyphs(&mut f, mode);
        assert_eq!(base, f, "explicit braille override diverged from hero at t={t}");
    }
}
