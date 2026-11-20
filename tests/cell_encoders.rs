//! RED slice for the v0.9.0 opt-in cell encoders (`docs/design/v0.9-cell-encoders.md`).
//!
//! `red_*` tests fail on assertions today by design: the encoders, the glyph tables and the
//! capability gate are stubs. `guard_*` tests pass today and must keep passing: they pin the
//! half-block default, the `ui::image` fallbacks and the zero-byte idle frame.
//!
//! Every sprite is a formula; no image or art of any kind is read. Oracles are written here,
//! independently of the crate: the sextant rule, the octant table, the glyph decoder, the
//! colour maths and the area filter.
//!
//! This file was written without compiling it (the slice was prepared under a build lock).
use gibson::raster::RgbRaster;
use gibson::renderer::Renderer;
use gibson::session::TerminalSession;
use gibson::ui::prelude::*;
use gibson::{
    allowed_encoder, compute_layout, octant_char, paint, sextant_char, CellEncoder, Color,
    ColorDepth, Node, RenderMode, Surface,
};
use std::collections::BTreeSet;

type Rgb = (u8, u8, u8);

const ALL: [CellEncoder; 6] = [
    CellEncoder::HalfBlock,
    CellEncoder::Quadrant,
    CellEncoder::Sextant,
    CellEncoder::Octant,
    CellEncoder::Braille,
    CellEncoder::Auto,
];

// ------------------------------------------------------------------ glyph oracles

/// The VTE 0.76 sextant rule, computed here and not by the crate.
fn vte_sextant(p: u8) -> char {
    match p {
        0 => ' ',
        21 => '▌',
        42 => '▐',
        63 => '█',
        1..=62 => {
            let skipped = u32::from(p > 21) + u32::from(p > 42);
            char::from_u32(0x1FB00 + u32::from(p) - 1 - skipped).unwrap()
        }
        _ => panic!("sextant pattern {p} is out of range"),
    }
}

/// Octant patterns (bit `2 * row + column`, row 0 on top) whose shape already has a character.
/// Only the patterns that are certain are listed; see `legacy_uncertain` for the rest.
fn legacy_certain(p: u8) -> Option<char> {
    Some(match p {
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
        _ => return None,
    })
}

#[test]
fn red_sextant_char_matches_the_vte_rule_for_all_64_patterns() {
    let mut seen = BTreeSet::new();
    for p in 0u8..64 {
        let got = sextant_char(p);
        assert_eq!(got, vte_sextant(p), "pattern {p:#08b}");
        seen.insert(got);
    }
    assert_eq!(
        seen.len(),
        64,
        "the 64 patterns must map to 64 distinct characters"
    );
    let sextants = seen
        .iter()
        .filter(|c| (0x1FB00..=0x1FB3B).contains(&(**c as u32)))
        .count();
    assert_eq!(sextants, 60, "60 patterns use U+1FB00..=U+1FB3B");
}

#[test]
fn red_octant_char_is_exact_where_a_block_character_already_exists() {
    for p in 0u8..=255 {
        if let Some(expected) = legacy_certain(p) {
            assert_eq!(octant_char(p), expected, "pattern {p:#010b}");
        }
    }
}

#[test]
fn red_octant_char_is_a_bijection_onto_230_octants_plus_26_existing_characters() {
    let all: Vec<char> = (0u8..=255).map(octant_char).collect();
    let distinct: BTreeSet<char> = all.iter().copied().collect();
    assert_eq!(
        distinct.len(),
        256,
        "256 patterns need 256 distinct characters"
    );
    let octants: BTreeSet<u32> = all
        .iter()
        .map(|c| *c as u32)
        .filter(|u| (0x1CD00..=0x1CDE5).contains(u))
        .collect();
    assert_eq!(octants.len(), 230, "Unicode 16 defines 230 octants");
    assert_eq!(*octants.iter().next().unwrap(), 0x1CD00);
    assert_eq!(*octants.iter().next_back().unwrap(), 0x1CDE5);
    // Every pattern with a certain existing character is outside the octant range.
    for p in 0u8..=255 {
        if legacy_certain(p).is_some() {
            assert!(
                !(0x1CD00..=0x1CDE5).contains(&(octant_char(p) as u32)),
                "{p:#010b}"
            );
        }
    }
}

/// The 26 patterns that already have a character outside the octant block, as listed by the
/// octant table in VTE's built-in glyph renderer (`src/minifont.cc`, tag 0.78.0, which draws
/// U+1CD00..=U+1CDE5 from a 230-entry pattern table). The bit numbering is the same: bit n-1 is
/// BLOCK OCTANT-n, with octants numbered 1 2 / 3 4 / 5 6 / 7 8.
const NON_OCTANT_PATTERNS: [u8; 26] = [
    0, 1, 2, 3, 5, 10, 15, 20, 40, 63, 64, 80, 85, 90, 95, 128, 160, 165, 170, 175, 192, 240, 245,
    250, 252, 255,
];

/// Verified against VTE 0.78.0's octant table: the 230 octants are allocated in ascending
/// pattern order over the patterns not in [`NON_OCTANT_PATTERNS`], so BLOCK OCTANT-3 (pattern 4)
/// is U+1CD00 and BLOCK OCTANT-2345678 (254) is U+1CDE5. This oracle checks every one of them.
#[test]
fn red_octant_char_matches_the_vte_octant_table_for_all_230_octants() {
    let mut next = 0x1CD00u32;
    for p in 0u8..=255 {
        if NON_OCTANT_PATTERNS.contains(&p) {
            continue;
        }
        assert_eq!(
            octant_char(p) as u32,
            next,
            "pattern {p:#010b} must be U+{next:X}"
        );
        next += 1;
    }
    assert_eq!(next, 0x1CDE6, "exactly 230 octants");
    assert_eq!(octant_char(4), '\u{1CD00}');
    assert_eq!(octant_char(254), '\u{1CDE5}');
}

/// Verified against VTE 0.78.0's renderer: the four single-octant corners are the Unicode 16
/// sixteenth blocks, and patterns 20 and 40 are the Unicode 16 middle quarter blocks.
#[test]
fn red_octant_char_remaining_existing_shapes() {
    assert_eq!(octant_char(0x01), '\u{1CEA8}'); // left half upper one quarter block
    assert_eq!(octant_char(0x02), '\u{1CEAB}'); // right half upper one quarter block
    assert_eq!(octant_char(0x40), '\u{1CEA3}'); // left half lower one quarter block
    assert_eq!(octant_char(0x80), '\u{1CEA0}'); // right half lower one quarter block
    assert_eq!(octant_char(0x14), '\u{1FBE6}'); // middle left one quarter block
    assert_eq!(octant_char(0x28), '\u{1FBE7}'); // middle right one quarter block
}

// ------------------------------------------------------------------ colour oracles

fn to_linear(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb8(l: f64) -> u8 {
    let l = l.clamp(0.0, 1.0);
    let c = if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    (c * 255.0).round() as u8
}

/// Linear-light RMSE over all channels of two equally long pixel lists.
fn rmse(a: &[Rgb], b: &[Rgb]) -> f64 {
    assert_eq!(a.len(), b.len());
    let mut sum = 0.0;
    for (p, q) in a.iter().zip(b) {
        for (x, y) in [(p.0, q.0), (p.1, q.1), (p.2, q.2)] {
            let d = to_linear(x) - to_linear(y);
            sum += d * d;
        }
    }
    (sum / (3.0 * a.len() as f64)).sqrt()
}

/// Exact box filter by integer factors, averaging in linear light (the best case for the
/// half-block baseline: its two colours are the means of the top and bottom halves).
fn box_down(src: &RgbRaster, fx: u16, fy: u16) -> RgbRaster {
    let (w, h) = (src.width() / fx, src.height() / fy);
    let mut out = RgbRaster::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0.0f64; 3];
            for dy in 0..fy {
                for dx in 0..fx {
                    let p = src
                        .get(i32::from(x * fx + dx), i32::from(y * fy + dy))
                        .unwrap();
                    acc[0] += to_linear(p.0);
                    acc[1] += to_linear(p.1);
                    acc[2] += to_linear(p.2);
                }
            }
            let n = f64::from(fx * fy);
            out.set(
                i32::from(x),
                i32::from(y),
                (
                    to_srgb8(acc[0] / n),
                    to_srgb8(acc[1] / n),
                    to_srgb8(acc[2] / n),
                ),
            );
        }
    }
    out
}

// ------------------------------------------------------------------ surface decoder

/// Which of a cell's 2 × 6 reference samples (index `row * 2 + column`) show the foreground.
fn cover(f: impl Fn(usize, usize) -> bool) -> [bool; 12] {
    let mut m = [false; 12];
    for r in 0..6 {
        for c in 0..2 {
            m[r * 2 + c] = f(r, c);
        }
    }
    m
}

/// A glyph's foreground coverage on the 2 × 6 tile. Only half-block, quadrant and sextant
/// families (the `Auto` candidates) are decoded; anything else is `None`.
fn glyph_coverage(g: &str) -> Option<[bool; 12]> {
    let mut chars = g.chars();
    let ch = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    // Quadrants as (upper-left, upper-right, lower-left, lower-right).
    let quad = |ul: bool, ur: bool, ll: bool, lr: bool| {
        cover(move |r, c| match (r >= 3, c) {
            (false, 0) => ul,
            (false, _) => ur,
            (true, 0) => ll,
            (true, _) => lr,
        })
    };
    match ch {
        ' ' => Some(cover(|_, _| false)),
        '█' => Some(cover(|_, _| true)),
        '▀' => Some(cover(|r, _| r < 3)),
        '▄' => Some(cover(|r, _| r >= 3)),
        '▌' => Some(cover(|_, c| c == 0)),
        '▐' => Some(cover(|_, c| c == 1)),
        '▖' => Some(quad(false, false, true, false)),
        '▗' => Some(quad(false, false, false, true)),
        '▘' => Some(quad(true, false, false, false)),
        '▙' => Some(quad(true, false, true, true)),
        '▚' => Some(quad(true, false, false, true)),
        '▛' => Some(quad(true, true, true, false)),
        '▜' => Some(quad(true, true, false, true)),
        '▝' => Some(quad(false, true, false, false)),
        '▞' => Some(quad(false, true, true, false)),
        '▟' => Some(quad(false, true, true, true)),
        _ => (1u8..=62)
            .filter(|p| *p != 21 && *p != 42)
            .find(|p| vte_sextant(*p) == ch)
            .map(|p| cover(move |r, c| (p >> (2 * (r / 2) + c)) & 1 == 1)),
    }
}

/// Re-renders a `cells_w × cells_h` surface onto the 2 × 6 per-cell reference grid, row-major.
fn decode(surface: &Surface, cells_w: u16, cells_h: u16) -> Result<Vec<Rgb>, String> {
    if (surface.width, surface.height) != (cells_w, cells_h) {
        return Err(format!(
            "surface is {}x{} cells, expected {cells_w}x{cells_h}",
            surface.width, surface.height
        ));
    }
    let width = usize::from(cells_w) * 2;
    let mut out = vec![(0u8, 0u8, 0u8); width * usize::from(cells_h) * 6];
    for cy in 0..cells_h {
        for cx in 0..cells_w {
            let cell = surface.get(cx, cy).ok_or("cell out of bounds")?;
            let glyph = cell.glyph.grapheme.as_str();
            let mask = glyph_coverage(glyph).ok_or_else(|| format!("cannot decode {glyph:?}"))?;
            let colour = |c: Option<Color>, what: &str| match c {
                Some(Color::Rgb(r, g, b)) => Ok((r, g, b)),
                other => Err(format!("cell {cx},{cy} {glyph:?}: {what} is {other:?}")),
            };
            for r in 0..6usize {
                for c in 0..2usize {
                    let px = if mask[r * 2 + c] {
                        colour(cell.style.fg, "fg")?
                    } else {
                        colour(cell.style.bg, "bg")?
                    };
                    out[(usize::from(cy) * 6 + r) * width + usize::from(cx) * 2 + c] = px;
                }
            }
        }
    }
    Ok(out)
}

// ------------------------------------------------------------------ synthetic sprites

/// Footprint of the RMSE harness: 8 × 4 cells, i.e. a 16 × 24 reference grid at 2 × 6 samples
/// per cell. Every encoder is judged against this same reference over this same footprint.
const CELLS: (u16, u16) = (8, 4);

fn sprite(f: impl Fn(i32, i32) -> Rgb) -> RgbRaster {
    let (w, h) = (CELLS.0 * 2, CELLS.1 * 6);
    let mut r = RgbRaster::new(w, h);
    for y in 0..i32::from(h) {
        for x in 0..i32::from(w) {
            r.set(x, y, f(x, y));
        }
    }
    r
}

/// A hard diagonal edge between a light and a dark flat colour.
fn edge_sprite() -> RgbRaster {
    sprite(|x, y| {
        if x * 24 > y * 16 + 20 {
            (235, 235, 235)
        } else {
            (20, 30, 60)
        }
    })
}

/// A smooth horizontal red-to-blue gradient.
fn gradient_sprite() -> RgbRaster {
    sprite(|x, _| {
        let t = (x * 255 / 15) as u8;
        (t, 128, 255 - t)
    })
}

/// One-pixel vertical stripes.
fn stripes_sprite() -> RgbRaster {
    sprite(|x, _| {
        if x % 2 == 0 {
            (240, 240, 240)
        } else {
            (10, 10, 10)
        }
    })
}

/// Tolerance for 8-bit quantization of the emitted colours (half a code step is up to ~0.005 in
/// linear light near white); the encoders select in linear light and emit 8-bit sRGB.
const QUANT_EPS: f64 = 3.0e-3;

/// RMSE of `encoder` against the reference. The encoder is handed the reference box-filtered to
/// its own sample grid (`Auto` gets the reference itself), so the footprint in cells is equal.
fn encoder_rmse(reference: &RgbRaster, encoder: CellEncoder) -> Result<f64, String> {
    let (gw, gh) = encoder.sample_grid();
    let input = box_down(reference, 2 / gw, 6 / gh);
    let surface = input.to_surface_with(encoder);
    let rendered = decode(&surface, CELLS.0, CELLS.1).map_err(|e| format!("{encoder:?}: {e}"))?;
    let truth: Vec<Rgb> = reference.pixels().to_vec();
    Ok(rmse(&rendered, &truth))
}

fn rmse_or_panic(reference: &RgbRaster, encoder: CellEncoder) -> f64 {
    encoder_rmse(reference, encoder).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn red_sextant_beats_halfblock_on_a_hard_edge_and_on_one_pixel_stripes() {
    for (name, reference) in [("edge", edge_sprite()), ("stripes", stripes_sprite())] {
        let half = rmse_or_panic(&reference, CellEncoder::HalfBlock);
        let sextant = rmse_or_panic(&reference, CellEncoder::Sextant);
        assert!(
            sextant < half,
            "{name}: sextant RMSE {sextant:.5} is not below half-block {half:.5}"
        );
    }
}

#[test]
fn red_auto_is_never_worse_than_halfblock_or_sextant() {
    for (name, reference) in [
        ("edge", edge_sprite()),
        ("gradient", gradient_sprite()),
        ("stripes", stripes_sprite()),
    ] {
        let half = rmse_or_panic(&reference, CellEncoder::HalfBlock);
        let sextant = rmse_or_panic(&reference, CellEncoder::Sextant);
        let auto = rmse_or_panic(&reference, CellEncoder::Auto);
        assert!(
            auto <= half.min(sextant) + QUANT_EPS,
            "{name}: auto {auto:.5} vs half-block {half:.5}, sextant {sextant:.5}"
        );
    }
}

#[test]
fn red_auto_strictly_improves_on_halfblock_where_edges_exist() {
    // Guards against `Auto == HalfBlock` satisfying the "never worse" test vacuously.
    for (name, reference) in [("edge", edge_sprite()), ("stripes", stripes_sprite())] {
        let half = rmse_or_panic(&reference, CellEncoder::HalfBlock);
        let auto = rmse_or_panic(&reference, CellEncoder::Auto);
        assert!(
            auto + QUANT_EPS < half,
            "{name}: auto {auto:.5}, half-block {half:.5}"
        );
    }
}

// ------------------------------------------------------------------ exact encoder output

/// Paints one 2 × 3 cell of a sextant-grid raster: `mask` bit `2 * row + column` selects `fg`.
fn paint_sextant_cell(r: &mut RgbRaster, cx: i32, cy: i32, mask: u8, fg: Rgb, bg: Rgb) {
    for dy in 0..3 {
        for dx in 0..2 {
            let on = (mask >> (2 * dy + dx)) & 1 == 1;
            r.set(2 * cx + dx, 3 * cy + dy, if on { fg } else { bg });
        }
    }
}

#[test]
fn red_sextant_encoder_emits_the_expected_glyph_and_exact_colours() {
    let (red, blue) = ((200, 30, 30), (20, 40, 210));
    let green = (30, 160, 70);
    let (yellow, dark) = ((230, 220, 40), (10, 10, 10));
    let (white, black) = ((250, 250, 250), (0, 0, 0));
    let mut raster = RgbRaster::new(4, 6);
    // Top-left sample is always the background: a pattern and its complement fit equally well,
    // and the canonical choice leaves bit 0 clear (as `temporal::project_rgb_subcells` does).
    paint_sextant_cell(&mut raster, 0, 0, 38, red, blue); // top-right, middle-left, bottom-right
    paint_sextant_cell(&mut raster, 1, 0, 0, green, green); // uniform
    paint_sextant_cell(&mut raster, 0, 1, 10, yellow, dark); // the right column's top two rows
    paint_sextant_cell(&mut raster, 1, 1, 62, white, black); // everything but the top-left

    let surface = raster.to_surface_with(CellEncoder::Sextant);
    assert_eq!(
        (surface.width, surface.height),
        (2, 2),
        "4x6 samples are 2x2 cells"
    );
    let check = |x: u16, y: u16, glyph: char, fg: Option<Rgb>, bg: Rgb| {
        let cell = surface.get(x, y).unwrap();
        assert_eq!(
            cell.glyph.grapheme.as_str(),
            glyph.to_string(),
            "glyph at {x},{y}"
        );
        if let Some((r, g, b)) = fg {
            assert_eq!(cell.style.fg, Some(Color::Rgb(r, g, b)), "fg at {x},{y}");
        }
        assert_eq!(
            cell.style.bg,
            Some(Color::Rgb(bg.0, bg.1, bg.2)),
            "bg at {x},{y}"
        );
    };
    check(0, 0, vte_sextant(38), Some(red), blue);
    check(1, 0, ' ', None, green);
    check(0, 1, vte_sextant(10), Some(yellow), dark);
    check(1, 1, vte_sextant(62), Some(white), black);
}

// ------------------------------------------------------------------ half-block default

fn formula_raster(w: u16, h: u16) -> RgbRaster {
    let mut r = RgbRaster::new(w, h);
    for y in 0..i32::from(h) {
        for x in 0..i32::from(w) {
            r.set(
                x,
                y,
                (
                    (x * 37 + y * 11) as u8,
                    (x * 5 + y * 53) as u8,
                    (x * y) as u8,
                ),
            );
        }
    }
    r
}

#[test]
fn guard_halfblock_default_is_byte_identical() {
    assert_eq!(CellEncoder::default(), CellEncoder::HalfBlock);
    assert_eq!(ImageOptions::default().encoder, CellEncoder::HalfBlock);
    assert_eq!(ImageOptions::new(), ImageOptions::default());
    // An even height, an odd height (the unmatched bottom pixel is black) and an empty raster.
    for (w, h) in [(7u16, 6u16), (5, 5), (3, 1), (0, 0)] {
        let r = formula_raster(w, h);
        assert_eq!(
            r.to_surface_with(CellEncoder::HalfBlock),
            r.to_surface(),
            "{w}x{h}"
        );
        assert_eq!(
            r.to_surface_with(CellEncoder::default()),
            r.to_surface(),
            "{w}x{h}"
        );
    }
}

#[test]
fn guard_sample_grids_are_the_documented_ones() {
    assert_eq!(CellEncoder::HalfBlock.sample_grid(), (1, 2));
    assert_eq!(CellEncoder::Quadrant.sample_grid(), (2, 2));
    assert_eq!(CellEncoder::Sextant.sample_grid(), (2, 3));
    assert_eq!(CellEncoder::Octant.sample_grid(), (2, 4));
    assert_eq!(CellEncoder::Braille.sample_grid(), (2, 4));
    // `Auto`'s tile is divisible by half-block, quadrant and sextant grids.
    let (aw, ah) = CellEncoder::Auto.sample_grid();
    for e in [
        CellEncoder::HalfBlock,
        CellEncoder::Quadrant,
        CellEncoder::Sextant,
    ] {
        let (w, h) = e.sample_grid();
        assert_eq!((aw % w, ah % h), (0, 0), "{e:?}");
    }
}

// ------------------------------------------------------------------ ui::image_with

fn env(width: u16, height: u16, depth: ColorDepth) -> UiEnvironment {
    UiEnvironment {
        width,
        height,
        color_depth: depth,
        motion: MotionPreference::None,
        ..UiEnvironment::default()
    }
}

fn compile_node(el: &Element<()>, e: UiEnvironment) -> Node {
    compile(el, &BuildCx::new(skins::SWISS_SIGNAL, e))
        .unwrap()
        .node
}

fn paint_element(el: &Element<()>, e: UiEnvironment) -> Surface {
    let mut node = compile_node(el, e);
    compute_layout(&mut node, e.width, e.height).unwrap();
    let mut s = Surface::new(e.width, e.height);
    paint(&node, &mut s);
    s
}

fn row_text(s: &Surface, y: u16) -> String {
    (0..s.width)
        .filter_map(|x| {
            let c = s.get(x, y).unwrap();
            (!c.is_continuation).then_some(c.glyph.grapheme.as_str().to_string())
        })
        .collect()
}

fn find(s: &Surface, needle: &str) -> Option<(u16, u16)> {
    (0..s.height).find_map(|y| {
        let row = row_text(s, y);
        row.find(needle)
            .map(|byte| (row[..byte].chars().count() as u16, y))
    })
}

fn occupied_rows(s: &Surface) -> usize {
    (0..s.height)
        .filter(|&y| !row_text(s, y).trim().is_empty())
        .count()
}

fn halves(w: u16, h: u16) -> RgbRaster {
    let mut r = RgbRaster::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let c = if x < w / 2 { (220, 0, 0) } else { (0, 0, 220) };
            r.set(x.into(), y.into(), c);
        }
    }
    r
}

const ALT: &str = "image 640×480 · SYNTHETIC";

fn opts(encoder: CellEncoder) -> ImageOptions {
    ImageOptions::new().with_encoder(encoder)
}

fn picture_with(r: &RgbRaster, max: (u16, u16), encoder: CellEncoder) -> Element<()> {
    column()
        .gap(0)
        .child(image_with(r, max, ALT, opts(encoder)))
        .child(label("BELOW"))
}

#[test]
fn guard_image_with_default_options_is_ui_image() {
    let r = halves(640, 480);
    let plain: Element<()> = column()
        .gap(0)
        .child(image(&r, (40, 12), ALT))
        .child(label("BELOW"));
    let with: Element<()> = column()
        .gap(0)
        .child(image_with(&r, (40, 12), ALT, ImageOptions::default()))
        .child(label("BELOW"));
    for depth in [ColorDepth::TrueColor, ColorDepth::Ansi256, ColorDepth::Mono] {
        let e = env(80, 24, depth);
        assert_eq!(
            paint_element(&plain, e),
            paint_element(&with, e),
            "{depth:?}"
        );
    }
}

#[test]
fn guard_image_with_below_the_minimum_box_shows_alt_text_for_every_encoder() {
    for encoder in ALL {
        for max in [(19, 12), (40, 5)] {
            let el = picture_with(&halves(640, 480), max, encoder);
            let s = paint_element(&el, env(80, 24, ColorDepth::TrueColor));
            assert!(
                find(&s, "image 640").is_some(),
                "{encoder:?} {max:?}: no alt text"
            );
            let drawn = (0..s.height).any(|y| row_text(&s, y).contains(['▀', '▄', '▌', '▐', '█']));
            assert!(
                !drawn,
                "{encoder:?} {max:?}: pixels drawn below the minimum box"
            );
        }
    }
}

#[test]
fn guard_image_with_mono_falls_back_as_ui_image_does() {
    let r = halves(640, 480);
    let plain: Element<()> = column()
        .gap(0)
        .child(image(&r, (40, 12), ALT))
        .child(label("BELOW"));
    let e = env(40, 15, ColorDepth::Mono);
    let expected = paint_element(&plain, e);
    for encoder in ALL {
        let s = paint_element(&picture_with(&r, (40, 12), encoder), e);
        assert_eq!(s, expected, "{encoder:?}");
        assert!(find(&s, "image 640").is_some());
        assert_eq!(occupied_rows(&s), 2, "{encoder:?}: alt line and BELOW only");
    }
}

#[test]
fn guard_image_with_reserves_the_same_box_for_every_encoder() {
    for encoder in ALL {
        for r in [halves(640, 480), halves(10, 10), RgbRaster::new(0, 0)] {
            let s = paint_element(
                &picture_with(&r, (40, 12), encoder),
                env(80, 24, ColorDepth::TrueColor),
            );
            assert_eq!(find(&s, "BELOW").unwrap().1, 12, "{encoder:?}");
        }
    }
}

#[test]
fn guard_image_with_sextant_idle_frame_emits_zero_bytes() {
    let e = env(80, 24, ColorDepth::TrueColor);
    let view = || {
        compile_node(
            &picture_with(&halves(640, 480), (40, 12), CellEncoder::Sextant),
            e,
        )
    };
    let mut renderer = Renderer::new(RenderMode::Fullscreen);
    let mut session = TerminalSession::headless(80, 24);
    let mut out = Vec::new();
    renderer
        .render(&mut view(), &mut session, &mut out)
        .unwrap();
    assert!(!out.is_empty(), "the first frame must draw the picture");
    out.clear();
    let (_, _, bytes, _, _) = renderer
        .render(&mut view(), &mut session, &mut out)
        .unwrap();
    assert_eq!(bytes, 0, "an unchanged picture must cost zero bytes");
    assert!(out.is_empty());
}

// ------------------------------------------------------------------ capability gate

fn tenv(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |k| {
        pairs
            .iter()
            .find(|(key, _)| *key == k)
            .map(|(_, v)| (*v).to_string())
    }
}

const VTE_76: &[(&str, &str)] = &[("TERM", "xterm-256color"), ("VTE_VERSION", "7600")];
const VTE_77: &[(&str, &str)] = &[("TERM", "xterm-256color"), ("VTE_VERSION", "7700")];
const UNKNOWN: &[(&str, &str)] = &[("TERM", "xterm-256color")];
const LINUX_VT: &[(&str, &str)] = &[("TERM", "linux")];
const EMPTY: &[(&str, &str)] = &[];

#[test]
fn red_vte_0_76_has_no_octants_so_octant_degrades_to_sextant() {
    assert_eq!(
        allowed_encoder(CellEncoder::Octant, None, tenv(VTE_76)),
        CellEncoder::Sextant
    );
    assert_eq!(
        allowed_encoder(CellEncoder::Sextant, None, tenv(VTE_76)),
        CellEncoder::Sextant
    );
}

#[test]
fn guard_vte_0_77_90_allows_octant() {
    assert_eq!(
        allowed_encoder(CellEncoder::Octant, None, tenv(VTE_77)),
        CellEncoder::Octant
    );
}

#[test]
fn red_an_unknown_terminal_never_upgrades_to_octant() {
    for vars in [UNKNOWN, EMPTY] {
        let got = allowed_encoder(CellEncoder::Octant, None, tenv(vars));
        assert_ne!(got, CellEncoder::Octant, "{vars:?}");
        assert_eq!(got, CellEncoder::HalfBlock, "{vars:?}");
        assert_eq!(
            allowed_encoder(CellEncoder::Sextant, None, tenv(vars)),
            CellEncoder::HalfBlock,
            "{vars:?}"
        );
    }
}

#[test]
fn red_the_linux_console_degrades_every_dense_encoder_to_halfblock() {
    for requested in [
        CellEncoder::Octant,
        CellEncoder::Sextant,
        CellEncoder::Quadrant,
        CellEncoder::Braille,
    ] {
        assert_eq!(
            allowed_encoder(requested, None, tenv(LINUX_VT)),
            CellEncoder::HalfBlock,
            "{requested:?}"
        );
    }
}

#[test]
fn red_modern_terminals_allow_sextant_but_not_octant() {
    let wezterm: &'static [(&str, &str)] =
        &[("TERM", "xterm-256color"), ("TERM_PROGRAM", "WezTerm")];
    assert_eq!(
        allowed_encoder(CellEncoder::Sextant, None, tenv(wezterm)),
        CellEncoder::Sextant
    );
    assert_eq!(
        allowed_encoder(CellEncoder::Octant, None, tenv(wezterm)),
        CellEncoder::Sextant
    );
}

#[test]
fn guard_kitty_and_ghostty_allow_octant() {
    let kitty: &'static [(&str, &str)] = &[("TERM", "xterm-kitty")];
    let ghostty: &'static [(&str, &str)] =
        &[("TERM", "xterm-256color"), ("TERM_PROGRAM", "ghostty")];
    for vars in [kitty, ghostty] {
        assert_eq!(
            allowed_encoder(CellEncoder::Octant, None, tenv(vars)),
            CellEncoder::Octant,
            "{vars:?}"
        );
    }
}

#[test]
fn guard_an_explicit_glyphs_octant_grants_octant_on_an_unknown_terminal() {
    let by_env: &'static [(&str, &str)] =
        &[("TERM", "xterm-256color"), ("LIBGIBSON_GLYPHS", "octant")];
    assert_eq!(
        allowed_encoder(CellEncoder::Octant, None, tenv(by_env)),
        CellEncoder::Octant
    );
    assert_eq!(
        allowed_encoder(CellEncoder::Octant, Some("octant"), tenv(UNKNOWN)),
        CellEncoder::Octant
    );
}

#[test]
fn red_an_explicit_glyphs_value_caps_a_capable_terminal_and_the_flag_beats_the_env() {
    assert_eq!(
        allowed_encoder(CellEncoder::Sextant, Some("halfblock"), tenv(VTE_77)),
        CellEncoder::HalfBlock
    );
    let both: &'static [(&str, &str)] =
        &[("TERM", "xterm-256color"), ("LIBGIBSON_GLYPHS", "octant")];
    assert_eq!(
        allowed_encoder(CellEncoder::Octant, Some("sextant"), tenv(both)),
        CellEncoder::Sextant
    );
}

#[test]
fn guard_halfblock_passes_every_gate_and_auto_passes_where_sextant_is_allowed() {
    for vars in [LINUX_VT, UNKNOWN, VTE_76, VTE_77, EMPTY] {
        assert_eq!(
            allowed_encoder(CellEncoder::HalfBlock, None, tenv(vars)),
            CellEncoder::HalfBlock,
            "{vars:?}"
        );
    }
    for vars in [VTE_76, VTE_77] {
        assert_eq!(
            allowed_encoder(CellEncoder::Auto, None, tenv(vars)),
            CellEncoder::Auto,
            "{vars:?}"
        );
    }
}

/// `Auto` searches half-block, quadrant and sextant glyphs, so it needs the sextant block: where
/// sextants are not allowed it degrades to half-block rather than to a partial family set.
#[test]
fn red_auto_degrades_to_halfblock_where_sextant_is_not_allowed() {
    for vars in [LINUX_VT, UNKNOWN, EMPTY] {
        assert_eq!(
            allowed_encoder(CellEncoder::Auto, None, tenv(vars)),
            CellEncoder::HalfBlock,
            "{vars:?}"
        );
    }
}
