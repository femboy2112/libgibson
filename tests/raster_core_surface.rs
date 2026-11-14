//! Guard for the `raster` subset that is CORE from 0.7.1 (decision 0003).
//!
//! It pins `Rgb`, `MAX_RASTER_DIMENSION`, `FrameError` and `RgbRaster` (except
//! `to_mono_surface`, which stays EXPERIMENTAL):
//! - every signature, through function-pointer coercions (and, for `write_ppm`, a
//!   generic call-shape helper), so a changed parameter or return type stops this
//!   file compiling;
//! - the trait set, including the auto traits;
//! - the documented edge behaviour: clamping, out-of-bounds no-ops, row-major
//!   layout, blend rounding and clamping, invalid-input no-ops, half-block
//!   realization and the PPM header.
//!
//! A failure here is a break of the CORE surface. Fix the code, or record
//! the change in RELEASE_CONTRACT §2 and the CHANGELOG under the 0.y rules.
use gibson::cell::{Cell, Color, Glyph, Style};
use gibson::raster::{FrameError, Rgb, RgbRaster, MAX_RASTER_DIMENSION};
use gibson::surface::Surface;
use std::fmt::{Debug, Display};
use std::hash::Hash;

#[test]
fn signatures_are_pinned() {
    let _: fn(u16, u16) -> RgbRaster = RgbRaster::new;
    let _: fn(u16, u16, usize, &[u8]) -> Result<RgbRaster, FrameError> = RgbRaster::from_rgb_bytes;
    let _: fn(&RgbRaster) -> u16 = RgbRaster::width;
    let _: fn(&RgbRaster) -> u16 = RgbRaster::height;
    let _: fn(&RgbRaster) -> &[Rgb] = RgbRaster::pixels;
    let _: fn(&mut RgbRaster) -> &mut [Rgb] = RgbRaster::pixels_mut;
    let _: fn(&RgbRaster, i32, i32) -> Option<Rgb> = RgbRaster::get;
    let _: fn(&mut RgbRaster, i32, i32, Rgb) = RgbRaster::set;
    let _: fn(&mut RgbRaster, Rgb) = RgbRaster::clear;
    let _: fn(&mut RgbRaster, i32, i32, Rgb, f32) = RgbRaster::blend;
    let _: fn(&mut RgbRaster, i32, i32, i32, i32, Rgb) = RgbRaster::line;
    let _: fn(&mut RgbRaster, f32, f32, f32, Rgb) = RgbRaster::disc;
    let _: fn(&RgbRaster) -> Surface = RgbRaster::to_surface;
    let _: u16 = MAX_RASTER_DIMENSION;

    // `write_ppm` takes `impl Write`, which cannot coerce to a fn pointer. Pin its
    // call shape instead: any `W: Write` passed by value, returning `io::Result<()>`.
    fn ppm<W: std::io::Write>(r: &RgbRaster, w: W) -> std::io::Result<()> {
        r.write_ppm(w)
    }
    let _: fn(&RgbRaster, Vec<u8>) -> std::io::Result<()> = ppm::<Vec<u8>>;
    let _: fn(&RgbRaster, &mut Vec<u8>) -> std::io::Result<()> = ppm::<&mut Vec<u8>>;
    let _: fn(&RgbRaster, std::io::Sink) -> std::io::Result<()> = ppm::<std::io::Sink>;
}

#[test]
fn trait_sets_are_pinned() {
    fn rgb<T: Copy + Eq + Ord + Hash + Default + Debug + Send + Sync + 'static>() {}
    fn raster<T: Clone + Eq + Debug + Send + Sync + 'static>() {}
    fn error<T: Clone + Eq + Debug + Display + std::error::Error + Send + Sync + 'static>() {}
    rgb::<Rgb>();
    raster::<RgbRaster>();
    error::<FrameError>();
}

#[test]
fn rgb_is_the_plain_tuple() {
    // `Rgb` is a type alias for `(u8, u8, u8)`, and consumers build and destructure it
    // as a tuple. Turning it into a struct would break them, so this pins the alias.
    let c: Rgb = (1, 2, 3);
    let (r, g, b): (u8, u8, u8) = c;
    assert_eq!((r, g, b), (1, 2, 3));
    assert_eq!(Rgb::default(), (0, 0, 0));
}

#[test]
fn new_clamps_to_the_dimension_bound_and_starts_black() {
    assert_eq!(MAX_RASTER_DIMENSION, 2048);
    let r = RgbRaster::new(MAX_RASTER_DIMENSION + 1, 3);
    assert_eq!((r.width(), r.height()), (MAX_RASTER_DIMENSION, 3));
    assert_eq!(r.pixels().len(), MAX_RASTER_DIMENSION as usize * 3);
    assert!(r.pixels().iter().all(|&p| p == (0, 0, 0)));

    let empty = RgbRaster::new(0, 0);
    assert_eq!((empty.width(), empty.height()), (0, 0));
    assert!(empty.pixels().is_empty());
    let s = empty.to_surface();
    assert_eq!((s.width, s.height), (0, 0));
}

#[test]
fn pixels_are_row_major_and_pixels_mut_cannot_resize() {
    let mut r = RgbRaster::new(3, 2);
    r.set(1, 0, (10, 0, 0));
    r.set(0, 1, (20, 0, 0));
    assert_eq!(r.pixels()[1], (10, 0, 0));
    assert_eq!(r.pixels()[3], (20, 0, 0));

    let px = r.pixels_mut();
    assert_eq!(px.len(), 6);
    px[5] = (30, 40, 50);
    assert_eq!(r.get(2, 1), Some((30, 40, 50)));
}

#[test]
fn out_of_bounds_reads_are_none_and_writes_are_no_ops() {
    let mut r = RgbRaster::new(4, 3);
    r.clear((7, 8, 9));
    let before = r.clone();
    for (x, y) in [
        (-1, 0),
        (0, -1),
        (4, 0),
        (0, 3),
        (i32::MIN, i32::MIN),
        (i32::MAX, i32::MAX),
    ] {
        assert_eq!(r.get(x, y), None, "get({x}, {y})");
        r.set(x, y, (1, 1, 1));
        r.blend(x, y, (1, 1, 1), 1.0);
    }
    assert_eq!(r, before);
    assert_eq!(r.get(3, 2), Some((7, 8, 9)));
}

#[test]
fn blend_rounds_clamps_and_ignores_nonfinite_amounts() {
    let mut r = RgbRaster::new(1, 1);
    r.blend(0, 0, (255, 255, 255), 0.5);
    // 0 + 255 * 0.5 = 127.5, rounded half away from zero.
    assert_eq!(r.get(0, 0), Some((128, 128, 128)));

    r.blend(0, 0, (0, 0, 0), -3.0); // clamps to 0: unchanged
    assert_eq!(r.get(0, 0), Some((128, 128, 128)));
    r.blend(0, 0, (0, 10, 20), 7.0); // clamps to 1: replaced
    assert_eq!(r.get(0, 0), Some((0, 10, 20)));

    for amount in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        r.blend(0, 0, (255, 255, 255), amount);
        assert_eq!(r.get(0, 0), Some((0, 10, 20)), "amount {amount}");
    }
}

#[test]
fn line_is_inclusive_and_bounded_for_extreme_endpoints() {
    let mut r = RgbRaster::new(5, 2);
    r.line(0, 0, 3, 0, (9, 9, 9));
    let lit: Vec<i32> = (0..5).filter(|&x| r.get(x, 0) == Some((9, 9, 9))).collect();
    assert_eq!(lit, vec![0, 1, 2, 3]);
    assert_eq!(r.get(4, 0), Some((0, 0, 0)));
    assert!((0..5).all(|x| r.get(x, 1) == Some((0, 0, 0))));

    // Endpoints far outside the raster are clipped first, so this returns quickly.
    let mut r = RgbRaster::new(4, 4);
    r.line(i32::MIN, i32::MIN, i32::MAX, i32::MAX, (1, 2, 3));
    assert!(r.pixels().contains(&(1, 2, 3)));

    let mut empty = RgbRaster::new(0, 0);
    empty.line(0, 0, 10, 10, (1, 2, 3));
    assert!(empty.pixels().is_empty());
}

#[test]
fn disc_covers_the_closed_disc_and_ignores_invalid_input() {
    let mut r = RgbRaster::new(5, 5);
    r.disc(2.0, 2.0, 1.0, (5, 5, 5));
    let lit: Vec<(i32, i32)> = (0..5)
        .flat_map(|y| (0..5).map(move |x| (x, y)))
        .filter(|&(x, y)| r.get(x, y) == Some((5, 5, 5)))
        .collect();
    // Distance <= radius: the centre and its four neighbours (distance 1), not the
    // diagonals (distance sqrt 2).
    assert_eq!(lit, vec![(2, 1), (1, 2), (2, 2), (3, 2), (2, 3)]);

    let before = r.clone();
    r.disc(2.0, 2.0, -1.0, (6, 6, 6));
    r.disc(f32::NAN, 2.0, 1.0, (6, 6, 6));
    r.disc(2.0, f32::INFINITY, 1.0, (6, 6, 6));
    r.disc(2.0, 2.0, f32::NAN, (6, 6, 6));
    assert_eq!(r, before);
}

#[test]
fn to_surface_is_one_upper_half_block_per_two_rows() {
    let mut r = RgbRaster::new(2, 3);
    r.set(0, 0, (1, 2, 3));
    r.set(0, 1, (4, 5, 6));
    r.set(1, 2, (7, 8, 9));
    let s = r.to_surface();
    assert_eq!((s.width, s.height), (2, 2));

    let half = |top: Rgb, bottom: Rgb| {
        Cell::new(
            Glyph::new("▀"),
            Style::new()
                .fg(Color::Rgb(top.0, top.1, top.2))
                .bg(Color::Rgb(bottom.0, bottom.1, bottom.2)),
        )
    };
    assert_eq!(s.get(0, 0), Some(&half((1, 2, 3), (4, 5, 6))));
    assert_eq!(s.get(1, 0), Some(&half((0, 0, 0), (0, 0, 0))));
    // Odd height: the last cell row has no bottom pixel, and its background is black.
    assert_eq!(s.get(1, 1), Some(&half((7, 8, 9), (0, 0, 0))));
}

#[test]
fn write_ppm_emits_a_binary_p6_image() {
    let mut r = RgbRaster::new(2, 1);
    r.set(0, 0, (1, 2, 3));
    r.set(1, 0, (250, 251, 252));
    let mut out = Vec::new();
    r.write_ppm(&mut out).unwrap();
    let mut want = b"P6\n2 1\n255\n".to_vec();
    want.extend_from_slice(&[1, 2, 3, 250, 251, 252]);
    assert_eq!(out, want);
}

#[test]
fn from_rgb_bytes_copies_row_major_and_rejects_bad_input() {
    let r = RgbRaster::from_rgb_bytes(2, 1, 8, &[1, 2, 3, 4, 5, 6, 0xFF, 0xFF]).unwrap();
    assert_eq!(r.pixels(), &[(1, 2, 3), (4, 5, 6)]);

    let err = |w, h, stride, len| RgbRaster::from_rgb_bytes(w, h, stride, &vec![0; len]);
    assert!(matches!(
        err(0, 1, 3, 3),
        Err(FrameError::ZeroDimension {
            width: 0,
            height: 1
        })
    ));
    assert!(matches!(
        err(MAX_RASTER_DIMENSION + 1, 1, 0, 0),
        Err(FrameError::TooLarge { max: 2048, .. })
    ));
    assert!(matches!(
        err(2, 1, 5, 6),
        Err(FrameError::StrideTooSmall {
            stride: 5,
            min_row_bytes: 6
        })
    ));
    assert!(matches!(
        err(1, 2, usize::MAX, 3),
        Err(FrameError::DimensionsOverflow { height: 2, .. })
    ));
    assert!(matches!(
        err(2, 2, 6, 11),
        Err(FrameError::ShortBuffer { have: 11, need: 12 })
    ));
}
