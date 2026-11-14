//! Guard for the RGBA `raster` subset that becomes CORE in 0.7.2 (decision 0005).
//!
//! It pins `Rgba`, `RgbaRaster::{new, from_rgba_bytes, width, height, get, pixels}`,
//! `composite_over` and `flatten`:
//! - every signature, through function-pointer coercions, so a changed parameter or return
//!   type stops this file compiling;
//! - the trait sets, including the auto traits;
//! - `Rgba` as the `[u8; 4]` array;
//! - the documented behaviour: transparent and clamped `new`, row-major ingest with every
//!   rejection, out-of-bounds `get`, and `composite_over`'s exact straight-alpha formula,
//!   rounding, fast paths and clipping, plus `flatten`'s equivalence with it.
//!
//! `downscale_area` and `RgbaRaster::{from_pixels, from_rgb, set, pixels_mut}` stay
//! EXPERIMENTAL and are not pinned here.
//!
//! A failure here is a break of the CORE surface. Fix the code, or record the change in
//! RELEASE_CONTRACT §2 and the CHANGELOG under the 0.y rules.
use gibson::raster::{
    composite_over, flatten, FrameError, Rgb, RgbRaster, Rgba, RgbaRaster, MAX_RASTER_DIMENSION,
};
use std::fmt::Debug;
use std::hash::Hash;

#[test]
fn signatures_are_pinned() {
    let _: fn(u16, u16) -> RgbaRaster = RgbaRaster::new;
    let _: fn(u16, u16, usize, &[u8]) -> Result<RgbaRaster, FrameError> =
        RgbaRaster::from_rgba_bytes;
    let _: fn(&RgbaRaster) -> u16 = RgbaRaster::width;
    let _: fn(&RgbaRaster) -> u16 = RgbaRaster::height;
    let _: fn(&RgbaRaster, i32, i32) -> Option<Rgba> = RgbaRaster::get;
    let _: fn(&RgbaRaster) -> &[Rgba] = RgbaRaster::pixels;
    let _: fn(&mut RgbRaster, &RgbaRaster, i32, i32) = composite_over;
    let _: fn(&RgbaRaster, Rgb) -> RgbRaster = flatten;
}

#[test]
fn trait_sets_are_pinned() {
    fn rgba<T: Copy + Eq + Ord + Hash + Default + Debug + Send + Sync + 'static>() {}
    fn raster<T: Clone + Eq + Debug + Send + Sync + 'static>() {}
    rgba::<Rgba>();
    raster::<RgbaRaster>();
}

#[test]
fn rgba_is_the_straight_alpha_array() {
    // `Rgba` is an alias for `[u8; 4]` = `[r, g, b, a]`. Consumers build, index and destructure
    // it as an array; turning it into a tuple or struct would break them.
    let c: Rgba = [1, 2, 3, 4];
    let [r, g, b, a] = c;
    assert_eq!((r, g, b, a), (1, 2, 3, 4));
    assert_eq!(c[3], 4);
    assert_eq!(Rgba::default(), [0, 0, 0, 0]);
}

#[test]
fn new_is_fully_transparent_and_clamped() {
    let r = RgbaRaster::new(MAX_RASTER_DIMENSION + 1, 2);
    assert_eq!((r.width(), r.height()), (MAX_RASTER_DIMENSION, 2));
    assert_eq!(r.pixels().len(), MAX_RASTER_DIMENSION as usize * 2);
    assert!(r.pixels().iter().all(|&p| p == [0, 0, 0, 0]));

    let empty = RgbaRaster::new(0, 0);
    assert_eq!((empty.width(), empty.height()), (0, 0));
    assert!(empty.pixels().is_empty());
}

#[test]
fn from_rgba_bytes_copies_row_major_and_ignores_stride_padding() {
    // 2×2, stride 12: 8 pixel bytes per row plus 4 padding bytes that must be ignored.
    #[rustfmt::skip]
    let bytes = [
        1, 2, 3, 4,      5, 6, 7, 8,      0xEE, 0xEE, 0xEE, 0xEE,
        9, 10, 11, 12,   13, 14, 15, 16,  0xEE, 0xEE, 0xEE, 0xEE,
    ];
    let r = RgbaRaster::from_rgba_bytes(2, 2, 12, &bytes).unwrap();
    assert_eq!(
        r.pixels(),
        &[
            [1, 2, 3, 4],
            [5, 6, 7, 8],
            [9, 10, 11, 12],
            [13, 14, 15, 16]
        ]
    );
    assert_eq!(r.get(1, 0), Some([5, 6, 7, 8]));
    assert_eq!(r.get(0, 1), Some([9, 10, 11, 12]));
}

#[test]
fn from_rgba_bytes_rejects_bad_input_and_never_clamps() {
    let err = |w, h, stride, len| RgbaRaster::from_rgba_bytes(w, h, stride, &vec![0; len]);
    assert!(matches!(
        err(0, 1, 4, 4),
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
        err(2, 1, 7, 8),
        Err(FrameError::StrideTooSmall {
            stride: 7,
            min_row_bytes: 8
        })
    ));
    assert!(matches!(
        err(1, 2, usize::MAX, 4),
        Err(FrameError::DimensionsOverflow { height: 2, .. })
    ));
    assert!(matches!(
        err(2, 2, 8, 15),
        Err(FrameError::ShortBuffer { have: 15, need: 16 })
    ));
}

#[test]
fn get_out_of_bounds_is_none_for_every_i32() {
    let r = RgbaRaster::new(3, 2);
    for (x, y) in [
        (-1, 0),
        (0, -1),
        (3, 0),
        (0, 2),
        (i32::MIN, i32::MIN),
        (i32::MAX, i32::MAX),
    ] {
        assert_eq!(r.get(x, y), None, "get({x}, {y})");
    }
    assert_eq!(r.get(2, 1), Some([0, 0, 0, 0]));
}

/// A `w × h` straight-alpha layer of one colour, built through the ingest path.
fn layer(w: u16, h: u16, c: Rgba) -> RgbaRaster {
    let bytes: Vec<u8> = (0..usize::from(w) * usize::from(h))
        .flat_map(|_| c)
        .collect();
    RgbaRaster::from_rgba_bytes(w, h, usize::from(w) * 4, &bytes).unwrap()
}

fn over(src: Rgba, dst: Rgb) -> Rgb {
    let mut d = RgbRaster::new(1, 1);
    d.clear(dst);
    composite_over(&mut d, &layer(1, 1, src), 0, 0);
    d.get(0, 0).unwrap()
}

#[test]
fn composite_over_is_exact_straight_alpha_rounded_to_nearest() {
    // out = (src·a + dst·(255 − a) + 127) / 255, per channel, integers only: exact
    // round-to-nearest (255 is odd, so there are no ties).
    // 200·100 + 50·155 = 27750; 27750 / 255 = 108.8 → 109.
    assert_eq!(over([200, 200, 200, 100], (50, 50, 50)), (109, 109, 109));
    // 255·128 = 32640 → 128.
    assert_eq!(over([255, 255, 255, 128], (0, 0, 0)), (128, 128, 128));
    // The rounding boundary: 1·128/255 = 0.502 → 1, and 1·127/255 = 0.498 → 0.
    assert_eq!(over([1, 1, 1, 128], (0, 0, 0)), (1, 1, 1));
    assert_eq!(over([1, 1, 1, 127], (0, 0, 0)), (0, 0, 0));
    // Channels are independent.
    assert_eq!(over([255, 0, 10, 51], (0, 255, 200)), (51, 204, 162));
}

#[test]
fn composite_over_fast_paths_are_exact() {
    // a = 0 leaves dst untouched, whatever the source colour holds.
    assert_eq!(over([255, 255, 255, 0], (7, 8, 9)), (7, 8, 9));
    // a = 255 replaces dst exactly.
    assert_eq!(over([1, 2, 3, 255], (200, 200, 200)), (1, 2, 3));
}

#[test]
fn composite_over_touches_only_the_overlap() {
    let base = || {
        let mut d = RgbRaster::new(3, 3);
        d.clear((9, 9, 9));
        d
    };
    let src = layer(2, 2, [1, 1, 1, 255]);
    let lit = |d: &RgbRaster| -> Vec<(i32, i32)> {
        (0..3)
            .flat_map(|y| (0..3).map(move |x| (x, y)))
            .filter(|&(x, y)| d.get(x, y) == Some((1, 1, 1)))
            .collect()
    };

    let mut d = base();
    composite_over(&mut d, &src, -1, -1);
    assert_eq!(lit(&d), vec![(0, 0)], "negative offset clips");

    let mut d = base();
    composite_over(&mut d, &src, 2, 2);
    assert_eq!(lit(&d), vec![(2, 2)], "past-edge offset clips");

    let mut d = base();
    composite_over(&mut d, &src, 1, 0);
    assert_eq!(lit(&d), vec![(1, 0), (2, 0), (1, 1), (2, 1)]);

    for (x, y) in [
        (3, 0),
        (0, 3),
        (-2, 0),
        (i32::MIN, i32::MIN),
        (i32::MAX, i32::MAX),
    ] {
        let mut d = base();
        composite_over(&mut d, &src, x, y);
        assert_eq!(d, base(), "fully outside ({x}, {y}) is a no-op");
    }

    // An empty source or destination is a no-op.
    let mut d = base();
    composite_over(&mut d, &RgbaRaster::new(0, 0), 0, 0);
    assert_eq!(d, base());
    let mut empty = RgbRaster::new(0, 0);
    composite_over(&mut empty, &src, 0, 0);
    assert!(empty.pixels().is_empty());
}

#[test]
fn flatten_is_clear_then_composite_at_the_origin() {
    #[rustfmt::skip]
    let bytes = [
        255, 0, 0, 255,    0, 255, 0, 0,
        0, 0, 255, 128,    10, 20, 30, 51,
    ];
    let src = RgbaRaster::from_rgba_bytes(2, 2, 8, &bytes).unwrap();
    let bg = (40, 50, 60);
    let flat = flatten(&src, bg);

    let mut expected = RgbRaster::new(2, 2);
    expected.clear(bg);
    composite_over(&mut expected, &src, 0, 0);
    assert_eq!(flat, expected);
    assert_eq!((flat.width(), flat.height()), (2, 2));
    assert_eq!(flat.get(0, 0), Some((255, 0, 0)), "opaque replaces");
    assert_eq!(flat.get(1, 0), Some(bg), "transparent keeps the background");

    let empty = flatten(&RgbaRaster::new(0, 0), bg);
    assert!(empty.pixels().is_empty());
}
