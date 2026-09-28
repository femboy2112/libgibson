//! Integration tests for the keyed-video compositing path: foreground/background
//! detection, subpixel compositing, and — the load-bearing one — that the sparse
//! dirty-region reprojection the compositor uses is bit-identical to a full
//! reprojection, including clearing the subject's trail. Uses synthetic film only
//! (no ffmpeg, no clip), so CI is hermetic.

#[path = "../examples/temporal_video_compositor/film.rs"]
mod film;
#[path = "../examples/temporal_video_compositor/key.rs"]
mod key;

use film::KeyedFilm;
use key::{Bbox, KeyParams};

use gibson::{ResetPolicy, SubcellGlyphMode, Surface, TemporalDisplayProcessor};

const FILL: [u8; 3] = [200, 180, 170]; // opaque "subject", no green excess
const BG: [u8; 3] = [12, 12, 40];

fn union_cells(a: (u16, u16, u16, u16), b: (u16, u16, u16, u16)) -> (u16, u16, u16, u16) {
    let empty = |r: (u16, u16, u16, u16)| r.2 == 0 || r.3 == 0;
    if empty(a) {
        return b;
    }
    if empty(b) {
        return a;
    }
    let x0 = a.0.min(b.0);
    let y0 = a.1.min(b.1);
    let x1 = (a.0 + a.2).max(b.0 + b.2);
    let y1 = (a.1 + a.3).max(b.1 + b.3);
    (x0, y0, x1 - x0, y1 - y0)
}

fn surfaces_equal(a: &Surface, b: &Surface) -> Option<(u16, u16)> {
    for y in 0..a.height {
        for x in 0..a.width {
            if a.get(x, y) != b.get(x, y) {
                return Some((x, y));
            }
        }
    }
    None
}

#[test]
fn bbox_and_coverage_match_the_rectangle() {
    let rect = Bbox {
        x: 20,
        y: 6,
        w: 16,
        h: 24,
    };
    let film = KeyedFilm::synthetic(64, 36, 1, rect, 0, 0, FILL);
    let (bbox, opaque) = film.foreground_stats(film.frame(0), &KeyParams::default());
    assert_eq!(bbox, rect, "bbox should tightly bound the opaque rectangle");
    assert_eq!(
        opaque,
        (rect.w * rect.h) as usize,
        "opaque count = rect area"
    );
}

#[test]
fn pure_key_frame_is_empty() {
    // Rectangle placed off-screen -> the whole frame is key green.
    let rect = Bbox {
        x: 1000,
        y: 0,
        w: 8,
        h: 8,
    };
    let film = KeyedFilm::synthetic(64, 36, 1, rect, 0, 0, FILL);
    let (bbox, opaque) = film.foreground_stats(film.frame(0), &KeyParams::default());
    assert!(bbox.is_empty());
    assert_eq!(opaque, 0);
}

#[test]
fn background_shows_through_key_foreground_shows_over_it() {
    let rect = Bbox {
        x: 20,
        y: 6,
        w: 16,
        h: 24,
    };
    let film = KeyedFilm::synthetic(64, 36, 1, rect, 0, 0, FILL);
    let kp = KeyParams::default();
    let (subw, subh) = (64u32, 72u32); // 32 cols x 18 rows
                                       // Outside the rect: pure key -> background exactly.
    let out = film.composite_subpixel(film.frame(0), subw, subh, 5, 40, BG, &kp);
    assert_eq!(out, BG, "green must key to background");
    // Inside the rect: opaque subject -> foreground exactly (no green excess).
    let inside = film.composite_subpixel(film.frame(0), subw, subh, 25, 40, BG, &kp);
    assert_eq!(inside, FILL, "subject must composite over background");
}

/// Compose a whole frame over a constant background (no scene, no reticle) — the
/// isolated compositing closure for the projector.
fn compose<'a>(
    film: &'a KeyedFilm,
    bytes: &'a [u8],
    subw: u32,
    subh: u32,
    kp: &'a KeyParams,
) -> impl Fn(u16, u16) -> [u8; 3] + 'a {
    move |lx, ly| film.composite_subpixel(bytes, subw, subh, lx as u32, ly as u32, BG, kp)
}

#[test]
fn sparse_union_reprojection_is_identical_to_full_and_clears_the_trail() {
    let (cols, rows) = (32u16, 18u16);
    let (subw, subh) = (cols as u32 * 2, rows as u32 * 4);
    let kp = KeyParams::default();
    // Subject moves right between frame 0 (x=20) and frame 1 (x=28).
    let film = KeyedFilm::synthetic(
        64,
        36,
        2,
        Bbox {
            x: 20,
            y: 6,
            w: 16,
            h: 24,
        },
        8,
        0,
        FILL,
    );
    let cells_a = film.bbox_to_cells(&film.foreground_bbox(film.frame(0), &kp), cols, rows);
    let cells_b = film.bbox_to_cells(&film.foreground_bbox(film.frame(1), &kp), cols, rows);

    // Reference: frame 1 projected in full.
    let mut full = TemporalDisplayProcessor::new(cols, rows, SubcellGlyphMode::Braille2x4, 7);
    full.set_target_image(
        compose(&film, film.frame(1), subw, subh, &kp),
        ResetPolicy::Reset,
    );
    let full_surface = full.static_fallback();

    // Sparse: frame 0 in full, then only the union of prev+cur bbox reprojected to frame 1.
    let mut sparse = TemporalDisplayProcessor::new(cols, rows, SubcellGlyphMode::Braille2x4, 7);
    sparse.set_target_image(
        compose(&film, film.frame(0), subw, subh, &kp),
        ResetPolicy::Reset,
    );
    let u = union_cells(cells_a, cells_b);
    sparse.set_target_region(
        u.0,
        u.1,
        u.2,
        u.3,
        compose(&film, film.frame(1), subw, subh, &kp),
        ResetPolicy::Reset,
    );
    let sparse_surface = sparse.static_fallback();

    // The sparse update reproduces the full projection everywhere: inside the new
    // bbox (subject drawn) and in the vacated old bbox (trail cleared to background).
    assert_eq!(
        surfaces_equal(&full_surface, &sparse_surface),
        None,
        "sparse union reprojection must match a full reprojection cell-for-cell"
    );
}

#[test]
fn vacated_region_returns_to_background_not_a_ghost() {
    let (cols, rows) = (32u16, 18u16);
    let (subw, subh) = (cols as u32 * 2, rows as u32 * 4);
    let kp = KeyParams::default();
    let film = KeyedFilm::synthetic(
        64,
        36,
        2,
        Bbox {
            x: 20,
            y: 6,
            w: 16,
            h: 24,
        },
        8,
        0,
        FILL,
    );
    let cells_a = film.bbox_to_cells(&film.foreground_bbox(film.frame(0), &kp), cols, rows);
    let cells_b = film.bbox_to_cells(&film.foreground_bbox(film.frame(1), &kp), cols, rows);

    // A pure-background reference (all-key frame projected).
    let all_green = KeyedFilm::synthetic(
        64,
        36,
        1,
        Bbox {
            x: 1000,
            y: 0,
            w: 1,
            h: 1,
        },
        0,
        0,
        FILL,
    );
    let mut bg = TemporalDisplayProcessor::new(cols, rows, SubcellGlyphMode::Braille2x4, 7);
    bg.set_target_image(
        compose(&all_green, all_green.frame(0), subw, subh, &kp),
        ResetPolicy::Reset,
    );
    let bg_surface = bg.static_fallback();

    let mut sparse = TemporalDisplayProcessor::new(cols, rows, SubcellGlyphMode::Braille2x4, 7);
    sparse.set_target_image(
        compose(&film, film.frame(0), subw, subh, &kp),
        ResetPolicy::Reset,
    );
    let u = union_cells(cells_a, cells_b);
    sparse.set_target_region(
        u.0,
        u.1,
        u.2,
        u.3,
        compose(&film, film.frame(1), subw, subh, &kp),
        ResetPolicy::Reset,
    );
    let sparse_surface = sparse.static_fallback();

    // Columns that were subject in frame 0 but background in frame 1 (the trail):
    // cells_a starts left of cells_b, so [cells_a.x, cells_b.x) is vacated.
    for cx in cells_a.0..cells_b.0 {
        for cy in cells_a.1..(cells_a.1 + cells_a.3) {
            assert_eq!(
                sparse_surface.get(cx, cy),
                bg_surface.get(cx, cy),
                "vacated cell ({cx},{cy}) must be background, not a ghost of the subject"
            );
        }
    }
}
