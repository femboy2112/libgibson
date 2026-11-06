//! v0.7 node B — correctness oracle for the raster-ingest bench's consumer-side fit + lowering.
//! A timing only counts if the pixels it times are right: known patterns must land exactly,
//! an independent floating-point route must agree with the integer box filter bit for bit,
//! and per-cell readback through the one renderer must match the model.
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

/// Independent route: float mean over the same declared rectangles, rounded half-up.
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
fn the_integer_box_filter_matches_an_independent_float_route_bit_for_bit() {
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
fn every_source_pixel_is_counted_exactly_once_when_downsampling() {
    for (cols, rows) in GRIDS {
        let (w, h) = fitted_size(cols, rows);
        let (sw, sh) = (u32::from(SOURCE_W), u32::from(SOURCE_H));
        let span = |d: u32, s: u32, n: u32| {
            let a = d * s / n;
            ((d + 1) * s / n).max(a + 1).min(s) - a
        };
        let xs: u32 = (0..u32::from(w)).map(|d| span(d, sw, u32::from(w))).sum();
        let ys: u32 = (0..u32::from(h)).map(|d| span(d, sh, u32::from(h))).sum();
        assert_eq!(
            (xs, ys),
            (sw, sh),
            "{cols}x{rows}: rectangles tile the source"
        );
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
