//! v0.9.0 RED slice: the source-preserving pixel image element
//! (`docs/design/v0.9-kitty-graphics-route.md`, section 12). Synthetic fixtures only.
//! `red_*` fail against the stubs and are GREEN's specification; `guard_*` pass today.
//!
//! No `red_pixel_image_source_survives_compile` test: `ui::compile` lowers to a
//! `Node` plus interaction maps, and neither `Compiled` nor `Node` has a place that
//! can expose an `Arc<RgbaRaster>`. The path is created by GREEN's lowering change,
//! so a test now would only be a compile error. It gets written with that seam.
use gibson::raster::{RgbRaster, RgbaRaster};
use gibson::ui::prelude::*;
use std::sync::Arc;

/// 64x48 synthetic RGBA: opaque gradient, one fully transparent pixel, one
/// half-transparent pixel with a colour that differs from its neighbours.
fn sprite() -> Arc<RgbaRaster> {
    let (w, h) = (64u16, 48u16);
    let mut px = Vec::new();
    for y in 0..h {
        for x in 0..w {
            px.push([(x * 4) as u8, (y * 5) as u8, ((x + y) * 2) as u8, 255]);
        }
    }
    px[0] = [200, 10, 10, 0];
    px[5 * usize::from(w) + 7] = [10, 220, 30, 128];
    Arc::new(RgbaRaster::from_pixels(w, h, px).expect("valid synthetic raster"))
}

#[test]
fn red_pixel_image_keeps_the_same_source_arc() {
    let source = sprite();
    let element: Element<()> = pixel_image(Arc::clone(&source), (10, 3));
    let kept = element
        .image_source()
        .expect("pixel_image keeps its source");
    assert!(Arc::ptr_eq(&source, kept));
}

#[test]
fn red_pixel_image_source_keeps_alpha_and_full_resolution() {
    let source = sprite();
    // 10 x 3 cells = 10 x 6 half-cell pixels, far smaller than the 64 x 48 source.
    let element: Element<()> = pixel_image(Arc::clone(&source), (10, 3));
    let kept = element
        .image_source()
        .expect("pixel_image keeps its source");
    assert_eq!((kept.width(), kept.height()), (64, 48));
    assert_eq!(kept.pixels(), source.pixels());
    assert_eq!(kept.pixels()[0][3], 0);
    assert_eq!(kept.pixels()[5 * 64 + 7], [10, 220, 30, 128]);
}

#[test]
fn guard_existing_image_has_no_source() {
    let raster = RgbRaster::new(64, 48);
    let fitted: Element<()> = image(&raster, (10, 3), "alt");
    let area: Element<()> = image_area(&raster, (10, 3), "alt");
    assert!(fitted.image_source().is_none());
    assert!(area.image_source().is_none());
}
