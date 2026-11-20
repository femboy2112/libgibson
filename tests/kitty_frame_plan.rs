//! RED slice for the v0.9 kitty frame plan (`docs/design/v0.9-kitty-graphics-route.md`, section
//! 18): the terminal adapter's per-frame upload, delete and placement bookkeeping.
//!
//! Everything runs on bytes and `Surface`s: no terminal, no escape sequence is ever written to a
//! real device, and every image is a formula-generated synthetic sprite. The API under test is
//! the stub in `gibson::graphics::frame` (plus `ImageCache::forget`), which is deliberately
//! wrong, so the `red_*` tests fail on assertions today and are the specification for GREEN. The
//! `guard_*` tests pass against the stub and must keep passing.
//!
//! Oracles (base64, the upload, the delete and place APCs, the diacritic prefix, the placeholder
//! cells) are written out here from the protocol text, never produced by the code under test.
//! Image ids come from a probe `ImageCache` with the same limits, which is the already-GREEN id
//! function.

use gibson::cell::{Cell, Color, Glyph, Style};
use gibson::graphics::frame::{BoxFallback, BoxPlacement, KittyFramePlan};
use gibson::graphics::kitty::{kitty_delete_all, CacheLimits, ImageCache, PLACEHOLDER};
use gibson::raster::RgbaRaster;
use gibson::surface::Surface;

// ---------------------------------------------------------------------------------------------
// Independent oracles
// ---------------------------------------------------------------------------------------------

/// RFC 4648 base64 with padding, written here so the upload oracle does not trust the code under
/// test.
fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let b0 = u32::from(c[0]);
        let b1 = u32::from(*c.get(1).unwrap_or(&0));
        let b2 = u32::from(*c.get(2).unwrap_or(&0));
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if c.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// The first 24 row/column diacritics (see `tests/kitty_graphics.rs` for the derivation).
const DIACRITICS: [char; 24] = [
    '\u{0305}', '\u{030D}', '\u{030E}', '\u{0310}', '\u{0312}', '\u{033D}', '\u{033E}', '\u{033F}',
    '\u{0346}', '\u{034A}', '\u{034B}', '\u{034C}', '\u{0350}', '\u{0351}', '\u{0352}', '\u{0357}',
    '\u{035B}', '\u{0363}', '\u{0364}', '\u{0365}', '\u{0366}', '\u{0367}', '\u{0368}', '\u{0369}',
];

fn id_color(id: u32) -> Color {
    Color::Rgb((id >> 16) as u8, (id >> 8) as u8, id as u8)
}

fn oracle_cells(id: u32, cols: u16, rows: u16) -> Surface {
    let mut s = Surface::new(cols, rows);
    for r in 0..rows {
        for c in 0..cols {
            let text = format!(
                "{}{}{}",
                PLACEHOLDER, DIACRITICS[r as usize], DIACRITICS[c as usize]
            );
            s.set_cell(
                c,
                r,
                Cell::new(Glyph::new(&text), Style::new().fg(id_color(id))),
            );
        }
    }
    s
}

/// A synthetic 16x24 "avatar": a head disc over a trapezoid body, all formula-generated.
/// `variant == 1` is a second pose (a mouth bar), so the two poses differ in pixels. 16x24x4 =
/// 1536 bytes, whose base64 (2048 characters) is one chunk.
fn avatar(variant: u8) -> RgbaRaster {
    let (w, h) = (16u16, 24u16);
    let mut r = RgbaRaster::new(w, h);
    for y in 0..i32::from(h) {
        for x in 0..i32::from(w) {
            let (dx, dy) = (x * 2 - 15, y * 2 - 23);
            let head = dx * dx + (dy + 14) * (dy + 14) < 90;
            let body = dy > -4 && dx.abs() < 6 + (dy + 4) / 3;
            if head {
                r.set(x, y, [224, 172, 105, 255]);
            } else if body {
                r.set(x, y, [(40 + x * 5) as u8, 70, (200 - y * 4) as u8, 255]);
            }
        }
    }
    if variant == 1 {
        for x in 6..10 {
            r.set(x, 6, [120, 20, 30, 255]);
        }
    }
    r
}

fn raw(r: &RgbaRaster) -> Vec<u8> {
    r.pixels().iter().flatten().copied().collect()
}

/// `ESC _ G a=t,f=32,s=<w>,v=<h>,i=<id>,q=2 ; <base64> ESC \` for a single-chunk image.
fn upload(id: u32, r: &RgbaRaster) -> Vec<u8> {
    let payload = b64(&raw(r));
    assert!(payload.len() <= 4096, "oracle only covers single chunks");
    format!(
        "\x1b_Ga=t,f=32,s={},v={},i={},q=2;{}\x1b\\",
        r.width(),
        r.height(),
        id,
        payload
    )
    .into_bytes()
}

/// Lowercase delete: placements go, the data stays.
fn delete_keep(id: u32) -> Vec<u8> {
    format!("\x1b_Ga=d,d=i,i={id},q=2\x1b\\").into_bytes()
}

fn place_virtual(id: u32, cols: u16, rows: u16) -> Vec<u8> {
    format!("\x1b_Ga=p,U=1,i={id},c={cols},r={rows},q=2\x1b\\").into_bytes()
}

#[allow(clippy::field_reassign_with_default)] // CacheLimits is #[non_exhaustive]
fn cap(n: usize) -> CacheLimits {
    let mut limits = CacheLimits::default();
    limits.max_resident_images = n;
    limits
}

/// The ids `rasters` get when shown in this order to a cache with `limits`.
fn ids_for(limits: CacheLimits, rasters: &[&RgbaRaster]) -> Vec<u32> {
    let mut probe = ImageCache::with_limits(limits);
    rasters.iter().map(|r| probe.ensure(r).0).collect()
}

fn expect_pixels(p: BoxPlacement) -> (u32, Surface) {
    match p {
        BoxPlacement::Pixels { id, cells } => (id, cells),
        other => panic!("expected a pixel box, got {other:?}"),
    }
}

/// One whole frame with a single box: begin, place, end. Returns the placement and the bytes.
fn one_box_frame(
    plan: &mut KittyFramePlan,
    cache: &mut ImageCache,
    raster: &RgbaRaster,
    cols: u16,
    rows: u16,
) -> (BoxPlacement, Vec<u8>) {
    plan.begin_frame();
    let placed = plan.place(cache, raster, cols, rows);
    (placed, plan.end_frame())
}

// ---------------------------------------------------------------------------------------------
// Guards: pass against the stub and must keep passing
// ---------------------------------------------------------------------------------------------

#[test]
fn guard_frame_plan_constructs_and_an_empty_frame_emits_nothing() {
    let mut plan = KittyFramePlan::new();
    plan.begin_frame();
    assert!(plan.end_frame().is_empty());
    assert!(plan.fallbacks().is_empty());
    let mut again = KittyFramePlan::default();
    again.begin_frame();
    assert!(again.end_frame().is_empty());
}

#[test]
fn guard_commit_without_a_frame_is_harmless() {
    let mut plan = KittyFramePlan::new();
    plan.commit();
    plan.begin_frame();
    assert!(plan.end_frame().is_empty());
    plan.commit();
    plan.commit();
    plan.begin_frame();
    assert!(plan.end_frame().is_empty());
}

/// `forget` is about residency only: the id stays in the teardown set, because its data may be
/// on the terminal. Teardown bytes are exactly what they were before the frame plan existed.
#[test]
fn guard_forget_never_drops_an_id_from_teardown() {
    let mut cache = ImageCache::new();
    assert!(cache.teardown_bytes().is_empty());
    let (id, up) = cache.ensure(&avatar(0));
    assert!(up.is_some());
    cache.forget(id);
    assert_eq!(cache.teardown_bytes(), kitty_delete_all(&[id]));
    cache.forget(0xAB_CDEF); // an unknown id is a no-op
    assert_eq!(cache.teardown_bytes(), kitty_delete_all(&[id]));
}

// ---------------------------------------------------------------------------------------------
// ImageCache::forget
// ---------------------------------------------------------------------------------------------

#[test]
fn red_cache_forget_makes_the_next_ensure_upload_again() {
    let mut cache = ImageCache::new();
    let a = avatar(0);
    let (id, first) = cache.ensure(&a);
    assert_eq!(first.expect("first sight uploads"), upload(id, &a));
    let (same, hit) = cache.ensure(&a);
    assert_eq!(same, id);
    assert!(hit.is_none());
    cache.forget(id);
    let (again, second) = cache.ensure(&a);
    assert_eq!(again, id, "same pixels, same id");
    assert_eq!(
        second.expect("a forgotten image uploads again"),
        upload(id, &a),
        "no eviction delete: nothing was evicted"
    );
}

// ---------------------------------------------------------------------------------------------
// Frame plan
// ---------------------------------------------------------------------------------------------

/// (1) First sight: the upload, then the virtual placement, nothing else; the box holds
/// placeholder cells that carry the id.
#[test]
fn red_first_frame_emits_upload_then_placement_and_cells_encode_the_id() {
    let a = avatar(0);
    let id = ids_for(CacheLimits::default(), &[&a])[0];
    let mut cache = ImageCache::new();
    let mut plan = KittyFramePlan::new();
    let (placed, bytes) = one_box_frame(&mut plan, &mut cache, &a, 4, 3);

    let mut expected = upload(id, &a);
    expected.extend(place_virtual(id, 4, 3));
    assert_eq!(
        bytes,
        expected,
        "upload then placement, got {:?}",
        String::from_utf8_lossy(&bytes)
    );
    let (got_id, cells) = expect_pixels(placed);
    assert_eq!(got_id, id);
    assert_eq!(cells, oracle_cells(id, 4, 3));
    assert!(plan.fallbacks().is_empty());
}

/// (2) Steady state: after the first frame is committed, the same box emits zero graphics bytes
/// and still returns the same cells.
#[test]
fn red_second_identical_frame_emits_no_graphics_bytes() {
    let a = avatar(0);
    let mut cache = ImageCache::new();
    let mut plan = KittyFramePlan::new();
    let (first, bytes1) = one_box_frame(&mut plan, &mut cache, &a, 4, 3);
    assert!(!bytes1.is_empty(), "the first frame uploads");
    plan.commit();
    let (second, bytes2) = one_box_frame(&mut plan, &mut cache, &a, 4, 3);
    assert!(
        bytes2.is_empty(),
        "steady state is zero bytes, got {:?}",
        String::from_utf8_lossy(&bytes2)
    );
    assert_eq!(second, first, "same cells every frame");
    expect_pixels(second);
    plan.commit();
    let (_, bytes3) = one_box_frame(&mut plan, &mut cache, &a, 4, 3);
    assert!(bytes3.is_empty());
}

/// (3) Resize (design section 7): lowercase delete, then the placement at the new size, in that
/// order, and no re-upload, because the data is still resident.
#[test]
fn red_box_resize_emits_lowercase_delete_then_new_placement_and_no_upload() {
    let a = avatar(0);
    let id = ids_for(CacheLimits::default(), &[&a])[0];
    let mut cache = ImageCache::new();
    let mut plan = KittyFramePlan::new();
    let (_, bytes1) = one_box_frame(&mut plan, &mut cache, &a, 4, 3);
    assert!(!bytes1.is_empty());
    plan.commit();

    let (placed, bytes2) = one_box_frame(&mut plan, &mut cache, &a, 6, 2);
    let mut expected = delete_keep(id);
    expected.extend(place_virtual(id, 6, 2));
    assert_eq!(
        bytes2,
        expected,
        "d=i, then a=p at 6x2, got {:?}",
        String::from_utf8_lossy(&bytes2)
    );
    let (got_id, cells) = expect_pixels(placed);
    assert_eq!(got_id, id);
    assert_eq!(cells, oracle_cells(id, 6, 2));
    plan.commit();

    let (_, bytes3) = one_box_frame(&mut plan, &mut cache, &a, 6, 2);
    assert!(bytes3.is_empty(), "the new size is now the recorded one");
}

/// (4) One box size per id per frame (design section 7): the second, different-size box for the
/// same id takes the fallback, the plan reports it, and it queues nothing. A repeat at the first
/// size is fine.
#[test]
fn red_same_id_at_two_sizes_in_one_frame_second_box_falls_back_and_is_reported() {
    let a = avatar(0);
    let id = ids_for(CacheLimits::default(), &[&a])[0];
    let mut cache = ImageCache::new();
    let mut plan = KittyFramePlan::new();
    plan.begin_frame();
    let first = plan.place(&mut cache, &a, 4, 3);
    let second = plan.place(&mut cache, &a, 5, 3);
    let third = plan.place(&mut cache, &a, 4, 3);
    let bytes = plan.end_frame();

    let (got_id, cells) = expect_pixels(first.clone());
    assert_eq!(got_id, id);
    assert_eq!(cells, oracle_cells(id, 4, 3));
    assert_eq!(
        second,
        BoxPlacement::Fallback(BoxFallback::SizeConflict),
        "a second size for the same id in one frame falls back"
    );
    assert_eq!(third, first, "the same size again is the same box");
    assert_eq!(plan.fallbacks(), &[BoxFallback::SizeConflict]);

    let mut expected = upload(id, &a);
    expected.extend(place_virtual(id, 4, 3));
    assert_eq!(
        bytes,
        expected,
        "only the first size is queued, got {:?}",
        String::from_utf8_lossy(&bytes)
    );

    plan.commit();
    plan.begin_frame();
    assert!(plan.fallbacks().is_empty(), "the report is per frame");
}

/// (5) Resident means written (design section 6): a frame whose bytes were never confirmed does
/// not make the image resident, so the next frame uploads and places it again.
#[test]
fn red_uncommitted_frame_is_not_resident_so_the_next_frame_reuploads() {
    let a = avatar(0);
    let id = ids_for(CacheLimits::default(), &[&a])[0];
    let mut expected = upload(id, &a);
    expected.extend(place_virtual(id, 4, 3));

    let mut cache = ImageCache::new();
    let mut plan = KittyFramePlan::new();
    let (_, bytes1) = one_box_frame(&mut plan, &mut cache, &a, 4, 3);
    assert_eq!(bytes1, expected);
    // The write failed: no commit.
    let (placed, bytes2) = one_box_frame(&mut plan, &mut cache, &a, 4, 3);
    assert_eq!(bytes2, expected, "upload and placement again");
    expect_pixels(placed);
    plan.commit();
    let (_, bytes3) = one_box_frame(&mut plan, &mut cache, &a, 4, 3);
    assert!(bytes3.is_empty(), "committed this time: steady state");
}

/// (5b) A frame that was begun and placed into but never ended is not committed either.
#[test]
fn red_abandoned_open_frame_is_not_resident_so_the_next_frame_reuploads() {
    let a = avatar(0);
    let id = ids_for(CacheLimits::default(), &[&a])[0];
    let mut cache = ImageCache::new();
    let mut plan = KittyFramePlan::new();
    plan.begin_frame();
    expect_pixels(plan.place(&mut cache, &a, 4, 3));
    // No end_frame, no commit: the next begin_frame abandons it and drops its queued bytes.
    let (_, bytes) = one_box_frame(&mut plan, &mut cache, &a, 4, 3);
    let mut expected = upload(id, &a);
    expected.extend(place_virtual(id, 4, 3));
    assert_eq!(bytes, expected, "exactly one upload and one placement");
}

/// (6) Eviction by the residency cap (cap 1): the eviction delete `ensure` returns is written
/// before the new upload, and both before the new placement.
#[test]
fn red_eviction_by_the_residency_cap_precedes_the_new_upload() {
    let (a, b) = (avatar(0), avatar(1));
    let ids = ids_for(cap(1), &[&a, &b]);
    let (ia, ib) = (ids[0], ids[1]);
    let mut cache = ImageCache::with_limits(cap(1));
    let mut plan = KittyFramePlan::new();
    let (_, bytes1) = one_box_frame(&mut plan, &mut cache, &a, 4, 3);
    assert!(!bytes1.is_empty());
    plan.commit();

    let (placed, bytes2) = one_box_frame(&mut plan, &mut cache, &b, 4, 3);
    let mut expected = delete_keep(ia);
    expected.extend(upload(ib, &b));
    expected.extend(place_virtual(ib, 4, 3));
    assert_eq!(
        bytes2,
        expected,
        "evict a (d=i), upload b, place b, got {:?}",
        String::from_utf8_lossy(&bytes2)
    );
    let (got_id, _) = expect_pixels(placed);
    assert_eq!(got_id, ib);
}

/// (6b) The eviction deleted a's placements, so showing a again at the very same size it was
/// last placed at still needs an upload and a fresh placement: the plan must not trust a stale
/// placement record.
#[test]
fn red_reshown_evicted_image_is_uploaded_and_placed_again_at_the_same_size() {
    let (a, b) = (avatar(0), avatar(1));
    let ids = ids_for(cap(1), &[&a, &b]);
    let (ia, ib) = (ids[0], ids[1]);
    let mut cache = ImageCache::with_limits(cap(1));
    let mut plan = KittyFramePlan::new();
    one_box_frame(&mut plan, &mut cache, &a, 4, 3);
    plan.commit();
    one_box_frame(&mut plan, &mut cache, &b, 4, 3);
    plan.commit();

    let (_, bytes) = one_box_frame(&mut plan, &mut cache, &a, 4, 3);
    let mut expected = delete_keep(ib);
    expected.extend(upload(ia, &a));
    expected.extend(place_virtual(ia, 4, 3));
    assert_eq!(bytes, expected, "evict b, upload a, place a");
}

/// (7) A zero-size box takes the fallback, queues nothing and is reported.
#[test]
fn red_empty_box_falls_back_and_is_reported() {
    let a = avatar(0);
    let mut cache = ImageCache::new();
    let mut plan = KittyFramePlan::new();
    plan.begin_frame();
    assert_eq!(
        plan.place(&mut cache, &a, 0, 3),
        BoxPlacement::Fallback(BoxFallback::EmptyBox)
    );
    assert_eq!(
        plan.place(&mut cache, &a, 4, 0),
        BoxPlacement::Fallback(BoxFallback::EmptyBox)
    );
    assert!(plan.end_frame().is_empty());
    assert_eq!(
        plan.fallbacks(),
        &[BoxFallback::EmptyBox, BoxFallback::EmptyBox]
    );
}

/// (8) An image the cache refuses (over `max_upload_bytes`) takes the fallback, queues nothing
/// and is reported.
#[test]
#[allow(clippy::field_reassign_with_default)] // CacheLimits is #[non_exhaustive]
fn red_refused_image_falls_back_and_is_reported() {
    let a = avatar(0);
    let mut limits = CacheLimits::default();
    limits.max_upload_bytes = 16;
    let mut cache = ImageCache::with_limits(limits);
    let mut plan = KittyFramePlan::new();
    plan.begin_frame();
    assert_eq!(
        plan.place(&mut cache, &a, 4, 3),
        BoxPlacement::Fallback(BoxFallback::Refused)
    );
    assert!(plan.end_frame().is_empty());
    assert_eq!(plan.fallbacks(), &[BoxFallback::Refused]);
}
