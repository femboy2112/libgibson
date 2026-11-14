//! v0.7 node B — correctness oracle for the raster-ingest bench's consumer-side fit + lowering.
//! What it pins: known patterns land exactly; `fit_box` partitions the source (each source pixel
//! reaches exactly one sample) at every bench grid; its averaging matches a float route; and for
//! **every** timed configuration the ANSI bytes the renderer emits, parsed back by a terminal
//! model, show exactly the lowered cells (truecolor also exact RGB). One frame per configuration
//! is checked, not every timed frame.
// The shared bench module carries items only the example binary uses.
#![allow(dead_code)]
#[path = "../examples/raster_ingest_bench/lower.rs"]
mod lower;

use gibson::cell::Color;
use gibson::context::{Context, RenderMode};
use gibson::node::Node;
use gibson::raster::RgbRaster;
use lower::{fit_box, fitted_size, lower, source_frame, Lowering, SOURCE_H, SOURCE_W};

const GRIDS: [(u16, u16); 3] = [(80, 25), (120, 40), (200, 60)];

fn filled(c: (u8, u8, u8)) -> RgbRaster {
    let mut r = RgbRaster::new(SOURCE_W, SOURCE_H);
    r.clear(c);
    r
}

#[test]
fn a_uniform_source_fits_to_the_same_colour_everywhere() {
    let src = filled((37, 200, 91));
    for (cols, rows) in GRIDS {
        let (w, h) = fitted_size(cols, rows);
        let f = fit_box(&src, w, h);
        assert_eq!((f.width(), f.height()), (w, h));
        assert!(
            f.pixels().iter().all(|&p| p == (37, 200, 91)),
            "{cols}x{rows}"
        );
    }
}

#[test]
fn an_aligned_checkerboard_lands_exactly_at_80x25() {
    // 80x25 half-blocks need 80x50 samples: each sample is exactly a 4x4 source block.
    let mut src = RgbRaster::new(SOURCE_W, SOURCE_H);
    for y in 0..SOURCE_H as i32 {
        for x in 0..SOURCE_W as i32 {
            let on = (x / 4 + y / 4) % 2 == 0;
            src.set(x, y, if on { (250, 10, 10) } else { (10, 10, 250) });
        }
    }
    let f = fit_box(&src, 80, 50);
    for y in 0..50 {
        for x in 0..80 {
            let want = if (x + y) % 2 == 0 {
                (250, 10, 10)
            } else {
                (10, 10, 250)
            };
            assert_eq!(f.get(x, y), Some(want), "sample ({x},{y})");
        }
    }
}

#[test]
fn a_single_lit_corner_pixel_stays_in_its_corner() {
    for (cx, cy) in [(0, 0), (319, 0), (0, 199), (319, 199)] {
        let mut src = filled((0, 0, 0));
        src.set(cx, cy, (255, 255, 255));
        let f = fit_box(&src, 80, 50);
        let (dx, dy) = (cx / 4, cy / 4);
        // One lit pixel in a 4x4 block: round(255 / 16) = 16.
        assert_eq!(f.get(dx, dy), Some((16, 16, 16)), "corner ({cx},{cy})");
        let lit = f.pixels().iter().filter(|&&p| p != (0, 0, 0)).count();
        assert_eq!(lit, 1, "corner ({cx},{cy}) leaked into other samples");
    }
}

/// Second route for the AVERAGING: a float mean, rounded half-up, over the same declared
/// rectangles (the rectangle bounds are shared with `fit_box`; the partition test below pins
/// them through `fit_box` itself).
fn fit_reference(src: &RgbRaster, dw: u16, dh: u16) -> Vec<(u8, u8, u8)> {
    let (sw, sh) = (src.width() as usize, src.height() as usize);
    let mut out = Vec::with_capacity(dw as usize * dh as usize);
    for dy in 0..dh as usize {
        let ya = dy * sh / dh as usize;
        let yb = ((dy + 1) * sh / dh as usize).max(ya + 1).min(sh);
        for dx in 0..dw as usize {
            let xa = dx * sw / dw as usize;
            let xb = ((dx + 1) * sw / dw as usize).max(xa + 1).min(sw);
            let px: Vec<(u8, u8, u8)> = (ya..yb)
                .flat_map(|y| (xa..xb).map(move |x| (x, y)))
                .map(|(x, y)| src.get(x as i32, y as i32).unwrap())
                .collect();
            let n = px.len() as f64;
            let m = |f: fn(&(u8, u8, u8)) -> u8| {
                (px.iter().map(|p| f64::from(f(p))).sum::<f64>() / n).round() as u8
            };
            out.push((m(|p| p.0), m(|p| p.1), m(|p| p.2)));
        }
    }
    out
}

#[test]
fn the_integer_averaging_matches_a_float_route_bit_for_bit() {
    for n in [0u32, 1, 17, 63, 200] {
        let src = source_frame(n);
        for (cols, rows) in GRIDS {
            let (w, h) = fitted_size(cols, rows);
            assert_eq!(
                fit_box(&src, w, h).pixels(),
                fit_reference(&src, w, h).as_slice(),
                "frame {n} at {cols}x{rows}"
            );
        }
    }
}

#[test]
fn every_source_pixel_lands_in_exactly_one_sample() {
    // Driven through `fit_box` itself: light ONE source pixel on black and count the samples it
    // reaches. Overlapping rectangles would light two; a gap would light none. The fit is
    // separable (x-interval x y-interval), so a full row and a full column cover every bound.
    for (cols, rows) in GRIDS {
        let (w, h) = fitted_size(cols, rows);
        let lit_samples = |px: i32, py: i32| {
            let mut src = filled((0, 0, 0));
            src.set(px, py, (255, 255, 255));
            fit_box(&src, w, h)
                .pixels()
                .iter()
                .filter(|&&p| p != (0, 0, 0))
                .count()
        };
        for x in 0..i32::from(SOURCE_W) {
            assert_eq!(lit_samples(x, 0), 1, "{cols}x{rows}: source column {x}");
        }
        for y in 0..i32::from(SOURCE_H) {
            assert_eq!(lit_samples(0, y), 1, "{cols}x{rows}: source row {y}");
        }
    }
}

/// Horizontal and vertical extent (in fitted samples) of a centred source disc of radius 60.
fn disc_extent(cols: u16, rows: u16) -> (u32, u32) {
    let mut src = filled((0, 0, 0));
    src.disc(160.0, 100.0, 60.0, (255, 255, 255));
    let (w, h) = fitted_size(cols, rows);
    let f = fit_box(&src, w, h);
    let lit = |x: i32, y: i32| f.get(x, y).is_some_and(|p| p.0 >= 128);
    let horiz = (0..w as i32).filter(|&x| lit(x, h as i32 / 2)).count() as u32;
    let vert = (0..h as i32).filter(|&y| lit(w as i32 / 2, y)).count() as u32;
    (horiz, vert)
}

#[test]
fn a_source_circle_stays_round_at_80x25_and_the_stretch_error_elsewhere_is_bounded() {
    // A half-block sample is square on a 1:2 cell. At 80x25 one sample is exactly 4x4 source
    // pixels, so a source circle stays a circle (within one sample).
    let (h80, v80) = disc_extent(80, 25);
    assert!(h80.abs_diff(v80) <= 1, "80x25: {h80} x {v80}");
    // Other grids stretch the axes unequally (120x40: 2.67 x 2.5 source px per sample;
    // 200x60: 1.6 x 1.67). Measured distortion is declared, bounded at 8%, not hidden: an
    // aspect-preserving (letterbox) fit is the open #128 question, not something this bench
    // pretends to do.
    for (cols, rows) in [(120, 40), (200, 60)] {
        let (hz, vt) = disc_extent(cols, rows);
        let ratio = f64::from(hz) / f64::from(vt);
        assert!(
            (0.92..=1.08).contains(&ratio),
            "{cols}x{rows}: {hz} x {vt} = {ratio:.3}"
        );
    }
}

#[test]
fn per_cell_readback_through_the_renderer_matches_the_model() {
    let src = source_frame(42);
    let fitted = fit_box(&src, 80, 50);
    let mut ctx = Context::headless(RenderMode::Fullscreen, 80, 25);
    ctx.set_root(Node::raster(lower(&fitted, Lowering::HalfBlockColor)));
    ctx.render().unwrap();
    for y in 0..25u16 {
        for x in 0..80u16 {
            let cell = ctx.frame_cell(x, y).expect("cell in frame");
            let top = fitted.get(i32::from(x), i32::from(y) * 2).unwrap();
            let bottom = fitted.get(i32::from(x), i32::from(y) * 2 + 1).unwrap();
            assert_eq!(cell.glyph, "▀", "({x},{y})");
            assert_eq!(
                cell.style.fg,
                Some(Color::Rgb(top.0, top.1, top.2)),
                "({x},{y}) fg"
            );
            assert_eq!(
                cell.style.bg,
                Some(Color::Rgb(bottom.0, bottom.1, bottom.2)),
                "({x},{y}) bg"
            );
        }
    }
}

#[test]
fn every_timed_configuration_emits_exactly_the_lowered_cells() {
    use gibson::capability::ColorDepth;
    use gibson::glyph::SubcellGlyphMode;
    let src = source_frame(42);
    let mut configs = Vec::new();
    for depth in [
        ColorDepth::TrueColor,
        ColorDepth::Ansi256,
        ColorDepth::Ansi16,
    ] {
        configs.push((Lowering::HalfBlockColor, depth));
    }
    for mode in [
        SubcellGlyphMode::Braille2x4,
        SubcellGlyphMode::HalfBlock1x2,
        SubcellGlyphMode::Block,
        SubcellGlyphMode::Ascii,
    ] {
        configs.push((Lowering::Mono(mode), ColorDepth::Mono));
    }
    let blank = |g: &str| g.is_empty() || g == " ";
    // Each lowering's declared repertoire (from `SubcellGlyphMode`'s documentation), checked
    // independently of `lower()` so a wrong glyph family cannot pass by self-consistency.
    let in_repertoire = |lowering: Lowering, g: &str| -> bool {
        let mut chars = g.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            return blank(g);
        };
        match lowering {
            Lowering::HalfBlockColor => c == '▀',
            Lowering::Mono(SubcellGlyphMode::Braille2x4) => ('\u{2800}'..='\u{28FF}').contains(&c),
            Lowering::Mono(SubcellGlyphMode::HalfBlock1x2) => " ▀▄█".contains(c),
            Lowering::Mono(SubcellGlyphMode::Block) => " ░▒▓█".contains(c),
            Lowering::Mono(SubcellGlyphMode::Ascii) => " .:-=+*#@".contains(c),
            Lowering::Mono(_) => false,
        }
    };
    for (cols, rows) in GRIDS {
        let (w, h) = fitted_size(cols, rows);
        let fitted = fit_box(&src, w, h);
        for &(lowering, depth) in &configs {
            let surface = lower(&fitted, lowering);
            let mut ctx = Context::headless(RenderMode::Fullscreen, cols, rows);
            ctx.set_color_depth(depth);
            ctx.set_root(Node::raster(surface.clone()));
            ctx.render().unwrap();
            let bytes = ctx.take_output();
            let mut term = vt100::Parser::new(
                core::num::NonZeroU16::new(rows).expect("vt100 size is non-zero"),
                core::num::NonZeroU16::new(cols).expect("vt100 size is non-zero"),
                0,
            );
            term.process(bytes.as_bytes());
            let screen = term.screen();
            let tag = format!("{cols}x{rows} {} {depth:?}", lowering.name());
            for y in 0..rows {
                for x in 0..cols {
                    let want = surface.get(x, y).unwrap();
                    let got = screen.cell(y, x).expect("cell on screen");
                    let (g, e) = (got.contents(), want.glyph.grapheme.as_str());
                    assert!(
                        g == e || (blank(g) && blank(e)),
                        "{tag} ({x},{y}): {g:?} != {e:?}"
                    );
                    assert!(
                        in_repertoire(lowering, g),
                        "{tag} ({x},{y}): {g:?} is outside the family"
                    );
                    if depth == ColorDepth::TrueColor {
                        let rgb = |c: Option<Color>| match c {
                            Some(Color::Rgb(r, g, b)) => vt100::Color::Rgb(r, g, b),
                            other => panic!("{tag}: expected RGB, got {other:?}"),
                        };
                        assert_eq!(got.fgcolor(), rgb(want.style.fg), "{tag} ({x},{y}) fg");
                        assert_eq!(got.bgcolor(), rgb(want.style.bg), "{tag} ({x},{y}) bg");
                    }
                }
            }
        }
    }
}
