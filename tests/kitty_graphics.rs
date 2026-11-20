//! RED slice for the v0.9 kitty graphics placeholder route
//! (`docs/design/v0.9-kitty-graphics-route.md`).
//!
//! Everything runs on bytes and `Surface`s: no terminal, no escape sequence is ever written to a
//! real device, and every image is a formula-generated synthetic sprite. The API under test is
//! the stub set in `gibson::graphics::kitty`, which returns empty or neutral values, so the
//! `red_*` tests fail on assertions today and are the specification for GREEN. The `guard_*`
//! tests pass today (against the stubs, or because they use only the existing pipeline) and must
//! keep passing.
//!
//! Oracles are independent of the stubs: base64, the APC splitter, the diacritic table prefix and
//! the placeholder cells are written out here from the protocol text, never produced by the code
//! under test.

use gibson::capability::{Capability, ColorDepth};
use gibson::cell::{Cell, Color, Glyph, Style};
use gibson::diff::compute_diff;
use gibson::graphics::kitty::{
    kitty_delete, kitty_delete_all, kitty_place_virtual, kitty_transmit_rgba, pixel_route,
    placeholder_cells, rowcolumn_diacritic, CacheLimits, FallbackReason, ImageCache, PixelRoute,
    MAX_IMAGE_ID, MAX_PLACEHOLDER_DIM, PLACEHOLDER,
};
use gibson::node::Node;
use gibson::raster::RgbaRaster;
use gibson::renderer::Renderer;
use gibson::session::TerminalSession;
use gibson::surface::{Rect, Surface};
use gibson::{AnsiCompiler, RenderMode};
use std::collections::BTreeSet;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

// ---------------------------------------------------------------------------------------------
// Independent oracles
// ---------------------------------------------------------------------------------------------

/// RFC 4648 base64 with padding, written here so the transmit tests do not trust the code under
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

/// Splits a byte stream that must consist only of kitty APC sequences into
/// `(control keys, payload)` pairs. Panics on any byte outside an APC, so a stream that mixes in
/// other output is a failure, not a silent skip.
fn apcs(bytes: &[u8]) -> Vec<(String, String)> {
    let s = std::str::from_utf8(bytes).expect("APC streams are ASCII");
    let mut out = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        let body = rest
            .strip_prefix("\x1b_G")
            .unwrap_or_else(|| panic!("not an APC at {:?}", &rest[..rest.len().min(16)]));
        let end = body.find("\x1b\\").expect("unterminated APC");
        let pair = match body[..end].split_once(';') {
            Some((c, p)) => (c.to_string(), p.to_string()),
            None => (body[..end].to_string(), String::new()),
        };
        out.push(pair);
        rest = &body[end + 2..];
    }
    out
}

/// The first 24 row/column diacritics: UCD 6.0.0 characters with category Mn, combining class
/// 230, bidi NSM and no decomposition, minus the 19 exclusions the protocol documentation lists
/// (see `scripts/gen_kitty_diacritics.py`). Entries 0 and 1 (U+0305, U+030D) match the
/// documentation's own examples.
const DIACRITICS: [char; 24] = [
    '\u{0305}', '\u{030D}', '\u{030E}', '\u{0310}', '\u{0312}', '\u{033D}', '\u{033E}', '\u{033F}',
    '\u{0346}', '\u{034A}', '\u{034B}', '\u{034C}', '\u{0350}', '\u{0351}', '\u{0352}', '\u{0357}',
    '\u{035B}', '\u{0363}', '\u{0364}', '\u{0365}', '\u{0366}', '\u{0367}', '\u{0368}', '\u{0369}',
];

fn id_color(id: u32) -> Color {
    Color::Rgb((id >> 16) as u8, (id >> 8) as u8, id as u8)
}

/// The placeholder cell for `(row, col)` of image `id`, spelled out from the protocol text.
fn oracle_cell(id: u32, col: usize, row: usize) -> Cell {
    let s = format!("{}{}{}", PLACEHOLDER, DIACRITICS[row], DIACRITICS[col]);
    Cell::new(Glyph::new(&s), Style::new().fg(id_color(id)))
}

fn oracle_cells(id: u32, cols: u16, rows: u16) -> Surface {
    let mut s = Surface::new(cols, rows);
    for r in 0..rows {
        for c in 0..cols {
            s.set_cell(c, r, oracle_cell(id, c as usize, r as usize));
        }
    }
    s
}

fn is_placeholder(cell: &Cell) -> bool {
    cell.glyph.grapheme.as_str().starts_with(PLACEHOLDER)
}

/// A synthetic 16x24 "avatar": a head disc over a trapezoid body, all formula-generated.
/// `variant == 1` is a second pose (a mouth bar), so the two poses differ in pixels.
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

/// Expected single-chunk upload for a raster whose base64 fits in one chunk.
fn expected_single_chunk_upload(id: u32, r: &RgbaRaster) -> Vec<u8> {
    let payload = b64(&raw(r));
    assert!(
        payload.len() <= 4096,
        "oracle only covers single-chunk images"
    );
    format!(
        "\x1b_Ga=t,f=32,s={},v={},i={},q=2;{}\x1b\\",
        r.width(),
        r.height(),
        id,
        payload
    )
    .into_bytes()
}

fn patterned(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 7 + 3) as u8).collect()
}

fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let owned: Vec<(String, String)> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |k: &str| owned.iter().find(|(kk, _)| kk == k).map(|(_, v)| v.clone())
}

// ---------------------------------------------------------------------------------------------
// Oracle self-checks (GREEN today)
// ---------------------------------------------------------------------------------------------

#[test]
fn guard_test_base64_matches_rfc4648_vectors() {
    assert_eq!(b64(b""), "");
    assert_eq!(b64(b"f"), "Zg==");
    assert_eq!(b64(b"fo"), "Zm8=");
    assert_eq!(b64(b"foo"), "Zm9v");
    assert_eq!(b64(b"foob"), "Zm9vYg==");
    assert_eq!(b64(b"fooba"), "Zm9vYmE=");
    assert_eq!(b64(b"foobar"), "Zm9vYmFy");
}

#[test]
fn guard_synthetic_avatar_has_opaque_and_transparent_pixels_and_two_poses() {
    let (a, b) = (avatar(0), avatar(1));
    assert_ne!(a, b);
    assert!(a.pixels().iter().any(|p| p[3] == 255));
    assert!(a.pixels().iter().any(|p| p[3] == 0));
    assert_eq!(raw(&a).len(), 16 * 24 * 4);
}

// ---------------------------------------------------------------------------------------------
// Transmit, place, delete bytes
// ---------------------------------------------------------------------------------------------

/// Keys chosen from the protocol page: `a=t` (transmit, do not display: the placeholder route
/// wants no real placement), `f=32` (RGBA), `s`/`v` (width/height are mandatory for raw pixels),
/// `i` (id), `q=2` (quiet: no terminal reply may reach the application's input). `t=d` (direct)
/// is the default and is omitted; `m` is omitted when the whole payload fits one chunk.
#[test]
fn red_transmit_2x2_matches_hand_built_apc() {
    let rgba: [u8; 16] = [
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 128,
    ];
    let expected = "\x1b_Ga=t,f=32,s=2,v=2,i=7,q=2;/wAA/wD/AP8AAP//////gA==\x1b\\";
    assert_eq!(b64(&rgba), "/wAA/wD/AP8AAP//////gA==");
    let got = kitty_transmit_rgba(7, 2, 2, &rgba);
    assert_eq!(
        got,
        expected.as_bytes(),
        "got {:?}",
        String::from_utf8_lossy(&got)
    );
}

/// 32x24 RGBA is 3072 bytes, whose base64 is exactly 4096 characters: the largest payload that
/// is still one chunk.
#[test]
fn red_transmit_payload_of_exactly_4096_is_a_single_chunk() {
    let data = patterned(32 * 24 * 4);
    let enc = b64(&data);
    assert_eq!(enc.len(), 4096);
    let expected = format!("\x1b_Ga=t,f=32,s=32,v=24,i=9,q=2;{enc}\x1b\\");
    let got = kitty_transmit_rgba(9, 32, 24, &data);
    assert_eq!(got, expected.as_bytes());
}

/// 33x24 RGBA is 3168 bytes, base64 4224 characters: one 4096 chunk (`m=1`) and a 128 tail
/// (`m=0`). Later chunks carry only `q` and `m`.
#[test]
fn red_transmit_payload_of_4224_splits_at_the_4096_boundary() {
    let data = patterned(33 * 24 * 4);
    let enc = b64(&data);
    assert_eq!(enc.len(), 4224);
    let expected = format!(
        "\x1b_Ga=t,f=32,s=33,v=24,i=9,q=2,m=1;{}\x1b\\\x1b_Gq=2,m=0;{}\x1b\\",
        &enc[..4096],
        &enc[4096..]
    );
    let got = kitty_transmit_rgba(9, 33, 24, &data);
    assert_eq!(got, expected.as_bytes());
}

#[test]
fn red_transmit_many_chunks_obey_the_chunk_rules() {
    let (w, h) = (64u32, 64u32);
    let data = patterned((w * h * 4) as usize);
    let enc = b64(&data);
    assert_eq!(enc.len(), 21848);
    let got = apcs(&kitty_transmit_rgba(11, w, h, &data));
    assert_eq!(got.len(), 6, "ceil(21848 / 4096) chunks");
    assert_eq!(got[0].0, "a=t,f=32,s=64,v=64,i=11,q=2,m=1");
    for mid in &got[1..5] {
        assert_eq!(mid.0, "q=2,m=1");
    }
    assert_eq!(got[5].0, "q=2,m=0");
    for (i, (_, payload)) in got.iter().enumerate() {
        assert!(
            payload.len() <= 4096,
            "chunk {i} is {} bytes",
            payload.len()
        );
        if i + 1 < got.len() {
            assert_eq!(
                payload.len() % 4,
                0,
                "non-final chunk {i} must be a multiple of 4"
            );
        }
    }
    let joined: String = got.iter().map(|(_, p)| p.as_str()).collect();
    assert_eq!(joined, enc);
}

/// Regression guard. A refused upload (wrong length, zero size, id 0 or an id above 24 bits)
/// emits nothing, so a bad call can never put a malformed APC on the wire. Passes against the
/// stub because the stub emits nothing for everything; it must keep passing when GREEN emits
/// real bytes for good input.
#[test]
fn guard_transmit_refuses_bad_input_with_no_bytes() {
    assert!(
        kitty_transmit_rgba(7, 2, 2, &[0u8; 15]).is_empty(),
        "short buffer"
    );
    assert!(
        kitty_transmit_rgba(7, 2, 2, &[0u8; 17]).is_empty(),
        "long buffer"
    );
    assert!(kitty_transmit_rgba(7, 0, 2, &[]).is_empty(), "zero width");
    assert!(kitty_transmit_rgba(7, 2, 0, &[]).is_empty(), "zero height");
    assert!(
        kitty_transmit_rgba(0, 1, 1, &[0u8; 4]).is_empty(),
        "id 0 is invalid"
    );
    assert!(
        kitty_transmit_rgba(MAX_IMAGE_ID + 1, 1, 1, &[0u8; 4]).is_empty(),
        "ids above 24 bits need the third diacritic and are out of scope"
    );
}

/// The virtual placement the placeholders refer to: `a=p,U=1,i,c,r`. No `C=1`: a virtual
/// placement has no screen position and moves no cursor.
#[test]
fn red_place_virtual_matches_hand_built_apc() {
    assert_eq!(
        kitty_place_virtual(7, 8, 5),
        b"\x1b_Ga=p,U=1,i=7,c=8,r=5,q=2\x1b\\"
    );
}

#[test]
fn red_delete_by_id_without_and_with_freeing_data() {
    assert_eq!(kitty_delete(7, false), b"\x1b_Ga=d,d=i,i=7,q=2\x1b\\");
    assert_eq!(kitty_delete(7, true), b"\x1b_Ga=d,d=I,i=7,q=2\x1b\\");
}

/// Teardown deletes by id and frees data. `d=a`/`d=A` is not used: per the protocol page those
/// keys never affect virtual placements.
#[test]
fn red_delete_all_frees_each_id_once_in_ascending_order() {
    let expected =
        "\x1b_Ga=d,d=I,i=7,q=2\x1b\\\x1b_Ga=d,d=I,i=9,q=2\x1b\\\x1b_Ga=d,d=I,i=70000,q=2\x1b\\";
    assert_eq!(kitty_delete_all(&[70000, 9, 7, 9]), expected.as_bytes());
    assert!(kitty_delete_all(&[]).is_empty());
}

// ---------------------------------------------------------------------------------------------
// Placeholder cells
// ---------------------------------------------------------------------------------------------

#[test]
fn red_rowcolumn_diacritic_table_head_and_end() {
    for (n, want) in DIACRITICS.iter().enumerate() {
        assert_eq!(rowcolumn_diacritic(n as u16), Some(*want), "entry {n}");
    }
    assert_eq!(
        rowcolumn_diacritic(296),
        Some('\u{1D244}'),
        "last of 297 entries"
    );
    assert_eq!(rowcolumn_diacritic(297), None);
    assert_eq!(rowcolumn_diacritic(u16::MAX), None);
}

#[test]
#[allow(clippy::needless_range_loop)] // r and c are also the cell coordinates
fn red_placeholder_cell_carries_row_and_column_diacritics_and_id_in_fg() {
    let id = 42u32;
    let s = placeholder_cells(id, 3, 2);
    assert_eq!((s.width, s.height), (3, 2));
    for r in 0..2usize {
        for c in 0..3usize {
            let cell = s.get(c as u16, r as u16).expect("cell in range");
            let want = format!("{}{}{}", PLACEHOLDER, DIACRITICS[r], DIACRITICS[c]);
            assert_eq!(
                cell.glyph.grapheme.as_str(),
                want,
                "glyph at row {r}, col {c}"
            );
            assert_eq!(cell.glyph.display_width, 1);
            assert_eq!(cell.style, Style::new().fg(Color::Rgb(0, 0, 42)));
            assert!(!cell.is_continuation && !cell.transparent && !cell.style_only);
        }
    }
    // Spot checks against the protocol example: (0,0) is U+0305 U+0305, (1,1) is U+030D U+030D.
    assert_eq!(
        s.get(0, 0).unwrap().glyph.grapheme.as_str(),
        "\u{10EEEE}\u{0305}\u{0305}"
    );
    assert_eq!(
        s.get(1, 1).unwrap().glyph.grapheme.as_str(),
        "\u{10EEEE}\u{030D}\u{030D}"
    );
}

#[test]
fn red_placeholder_fg_packs_the_24_bit_id_big_endian() {
    for (id, rgb) in [
        (0x01_02_03u32, (1u8, 2u8, 3u8)),
        (0x00_00_FF, (0, 0, 255)),
        (MAX_IMAGE_ID, (255, 255, 255)),
        (0x12_34_00, (0x12, 0x34, 0)),
    ] {
        let s = placeholder_cells(id, 1, 1);
        assert_eq!(
            s.get(0, 0).unwrap().style.fg,
            Some(Color::Rgb(rgb.0, rgb.1, rgb.2)),
            "id {id:#x}"
        );
    }
}

#[test]
fn red_placeholder_cells_equal_the_independent_oracle() {
    for (id, cols, rows) in [
        (42u32, 3u16, 2u16),
        (0x0A0B0C, 8, 5),
        (1, 1, 1),
        (9, 24, 24),
    ] {
        assert_eq!(
            placeholder_cells(id, cols, rows),
            oracle_cells(id, cols, rows),
            "id {id} {cols}x{rows}"
        );
    }
}

#[test]
fn red_placeholder_box_is_clamped_to_the_diacritic_table() {
    let s = placeholder_cells(5, MAX_PLACEHOLDER_DIM + 1, 2);
    assert_eq!(s.width, MAX_PLACEHOLDER_DIM);
    assert_eq!(s.height, 2);
}

/// `Glyph::new` must keep a placeholder plus up to three diacritics as one width-1 grapheme.
/// Expected GREEN already (`unicode-width` gives U+10EEEE one column and the combining marks
/// zero; `unicode-segmentation` keeps Extend marks in the cluster), so `cell.rs` needs no change.
/// If this is RED on CI, `Glyph::new` / `Glyph::from_char` need a placeholder special case that
/// pins the width to 1; see the design document, section "What GREEN needs".
#[test]
fn guard_glyph_new_keeps_placeholder_plus_diacritics_as_one_width_1_grapheme() {
    for s in [
        format!("{}{}{}", PLACEHOLDER, '\u{0305}', '\u{030D}'),
        format!("{}{}{}{}", PLACEHOLDER, '\u{030D}', '\u{0310}', '\u{030E}'),
        format!("{}{}{}", PLACEHOLDER, '\u{1D244}', '\u{0369}'),
    ] {
        let g = Glyph::new(&s);
        assert_eq!(g.grapheme.as_str(), s);
        assert_eq!(g.display_width, 1, "display width of {s:?}");
        assert_eq!(UnicodeWidthStr::width(s.as_str()), 1);
        assert_eq!(s.graphemes(true).count(), 1, "one grapheme cluster: {s:?}");
    }
}

// ---------------------------------------------------------------------------------------------
// Id cache
// ---------------------------------------------------------------------------------------------

#[test]
fn red_cache_same_raster_twice_uploads_once_with_the_same_id() {
    let mut cache = ImageCache::new();
    let a = avatar(0);
    let (id1, up1) = cache.ensure(&a);
    let (id2, up2) = cache.ensure(&a);
    assert!((1..=MAX_IMAGE_ID).contains(&id1), "id {id1}");
    assert_eq!(
        up1.expect("the first sight of a raster uploads"),
        expected_single_chunk_upload(id1, &a)
    );
    assert_eq!(id2, id1, "same pixels, same id");
    assert!(
        up2.is_none(),
        "no bytes for an image the terminal already holds"
    );
}

#[test]
fn red_cache_different_raster_gets_a_new_id_and_its_own_upload() {
    let mut cache = ImageCache::new();
    let (a, b) = (avatar(0), avatar(1));
    let (ia, ua) = cache.ensure(&a);
    let (ib, ub) = cache.ensure(&b);
    assert_ne!(ia, 0);
    assert_ne!(ib, 0);
    assert_ne!(ia, ib, "different pixels must not share an id");
    assert!(ua.is_some());
    assert_eq!(
        ub.expect("new pixels upload"),
        expected_single_chunk_upload(ib, &b)
    );
}

/// The same bytes read as 16x24 and as 24x16 are different images.
#[test]
fn red_cache_same_bytes_with_different_dimensions_are_different_images() {
    let a = avatar(0);
    let flipped = RgbaRaster::from_rgba_bytes(24, 16, 24 * 4, &raw(&a)).expect("valid raster");
    let mut cache = ImageCache::new();
    let (ia, ua) = cache.ensure(&a);
    let (ib, ub) = cache.ensure(&flipped);
    assert_ne!(ia, 0);
    assert_ne!(ia, ib);
    assert!(ua.is_some() && ub.is_some());
}

/// Ids are a pure function of the pixels: no clock, no randomness, no per-process state.
#[test]
fn red_cache_ids_are_deterministic_across_caches() {
    let (a, b) = (avatar(0), avatar(1));
    let (ia1, _) = ImageCache::new().ensure(&a);
    let (ia2, _) = ImageCache::new().ensure(&a);
    let (ib1, _) = ImageCache::new().ensure(&b);
    assert_ne!(ia1, 0);
    assert_eq!(ia1, ia2);
    assert_ne!(ia1, ib1);
}

/// Pose switching is by id: going A, B, A re-uploads nothing and the third call returns A's
/// original id.
#[test]
fn red_cache_pose_switch_back_reuses_the_resident_id() {
    let mut cache = ImageCache::new();
    let (a, b) = (avatar(0), avatar(1));
    let (ia, ua) = cache.ensure(&a);
    let (ib, ub) = cache.ensure(&b);
    let (ia_again, ua_again) = cache.ensure(&a);
    assert!(ua.is_some() && ub.is_some());
    assert_ne!(ia, 0);
    assert_ne!(ia, ib);
    assert_eq!(ia_again, ia);
    assert!(ua_again.is_none());
}

#[test]
fn red_cache_teardown_frees_every_transmitted_id() {
    let mut cache = ImageCache::new();
    let (ia, _) = cache.ensure(&avatar(0));
    let (ib, _) = cache.ensure(&avatar(1));
    assert!(ia != 0 && ib != 0 && ia != ib);
    let (lo, hi) = (ia.min(ib), ia.max(ib));
    let expected = format!("\x1b_Ga=d,d=I,i={lo},q=2\x1b\\\x1b_Ga=d,d=I,i={hi},q=2\x1b\\");
    assert_eq!(cache.teardown_bytes(), expected.as_bytes());
}

/// An image over the per-upload cap is refused: id 0, no bytes, so the caller falls back to the
/// cell encoder. A small image in the same cache still uploads (so this is not a stub no-op).
#[test]
#[allow(clippy::field_reassign_with_default)] // CacheLimits is #[non_exhaustive]
fn red_cache_oversize_upload_is_refused_but_small_ones_still_go() {
    let mut limits = CacheLimits::default();
    limits.max_upload_bytes = 100;
    let mut cache = ImageCache::with_limits(limits);
    let tiny = RgbaRaster::from_rgba_bytes(2, 2, 8, &[9u8; 16]).expect("valid raster");
    let (id_small, up_small) = cache.ensure(&tiny);
    assert_ne!(id_small, 0, "a 16-byte image is under the 100-byte cap");
    assert!(up_small.is_some());
    let (id_big, up_big) = cache.ensure(&avatar(0));
    assert_eq!(id_big, 0, "1536 bytes is over the cap: refused, id 0");
    assert!(up_big.is_none());
}

// ---------------------------------------------------------------------------------------------
// Occlusion, clipping and restore through the ordinary diff
// ---------------------------------------------------------------------------------------------

const SPRITE_ID: u32 = 42;
const SPRITE_COLS: u16 = 8;
const SPRITE_ROWS: u16 = 5;
const SPRITE_AT: (u16, u16) = (4, 2);
const MODAL: Rect = Rect::new(8, 4, 12, 4);

fn in_rect(r: &Rect, x: u16, y: u16) -> bool {
    r.contains(x, y)
}

/// Paints `sprite` into a frame, overlays a modal text box over part of it, then removes the
/// modal. The diff that removes it must re-emit exactly the placeholder cells the modal covered
/// and nothing else that is a placeholder, byte for byte. No upload bytes are involved at any
/// point: the image stays resident on the terminal.
///
/// Run with the stub (`red_*`: fails because the stub paints blanks) and with the oracle cells
/// (`guard_*`: passes today, proving the existing diff already gives occlusion and restore for
/// free once cells carry placeholders).
fn run_occlusion(sprite: &Surface) {
    let mut base = Surface::new(30, 12);
    base.blit_transparent_at(sprite, SPRITE_AT.0, SPRITE_AT.1);
    let sprite_rect = Rect::new(SPRITE_AT.0, SPRITE_AT.1, SPRITE_COLS, SPRITE_ROWS);

    // Precondition: the sprite really is placeholders in the frame.
    for r in 0..SPRITE_ROWS {
        for c in 0..SPRITE_COLS {
            let got = base.get(SPRITE_AT.0 + c, SPRITE_AT.1 + r).unwrap();
            assert_eq!(
                got,
                &oracle_cell(SPRITE_ID, c as usize, r as usize),
                "sprite cell ({c},{r}) in the base frame"
            );
        }
    }

    // The modal: an opaque filled box with text, partly over the sprite and partly beside it.
    let mut occluded = base.clone();
    occluded.fill_rect(MODAL, Cell::space(Style::new().bg(Color::Rgb(30, 30, 60))));
    let _ = occluded.print_str(9, 5, "Save?", Style::new().fg(Color::White), Some(10));

    let mut overlap = BTreeSet::new();
    for y in 0..occluded.height {
        for x in 0..occluded.width {
            let cell = occluded.get(x, y).unwrap();
            let in_sprite = in_rect(&sprite_rect, x, y);
            let in_modal = in_rect(&MODAL, x, y);
            if in_sprite && in_modal {
                overlap.insert((x, y));
                assert!(
                    !is_placeholder(cell),
                    "({x},{y}) must be modal text, not placeholder"
                );
            } else if in_sprite {
                assert_eq!(
                    cell,
                    base.get(x, y).unwrap(),
                    "({x},{y}) must stay a placeholder"
                );
                assert!(is_placeholder(cell));
            } else if !in_modal {
                assert_eq!(cell, base.get(x, y).unwrap(), "({x},{y}) is untouched");
            }
        }
    }
    assert_eq!(
        overlap.len(),
        4 * 3,
        "modal covers a 4x3 corner of the sprite"
    );

    // Occluding: the diff from base to occluded writes text only. No placeholder bytes, no APC.
    let forward = compute_diff(Some(&base), &occluded);
    let forward_bytes = AnsiCompiler::new().compile(&forward);
    let forward_text = String::from_utf8(forward_bytes).expect("utf8");
    assert!(
        !forward_text.contains(PLACEHOLDER),
        "occluding re-emits no placeholder"
    );
    assert!(
        !forward_text.contains("\x1b_G"),
        "occluding sends no image bytes"
    );
    assert!(forward_text.contains("Save?"));

    // Restoring: the modal goes away.
    let restore = compute_diff(Some(&occluded), &base);
    let mut placed = BTreeSet::new();
    for patch in &restore.patches {
        for run in &patch.runs {
            for (i, cell) in run.cells.iter().enumerate() {
                let (x, y) = (run.x + i as u16, patch.y);
                assert!(
                    is_placeholder(cell),
                    "restore run cell ({x},{y}) is not a placeholder"
                );
                let want = oracle_cell(
                    SPRITE_ID,
                    (x - SPRITE_AT.0) as usize,
                    (y - SPRITE_AT.1) as usize,
                );
                assert_eq!(cell, &want, "restored cell ({x},{y}) is not byte-exact");
                placed.insert((x, y));
            }
        }
    }
    assert_eq!(
        placed, overlap,
        "restore re-emits exactly the covered placeholder cells"
    );

    let restore_text = String::from_utf8(AnsiCompiler::new().compile(&restore)).expect("utf8");
    assert_eq!(
        restore_text.matches(PLACEHOLDER).count(),
        overlap.len(),
        "one placeholder codepoint on the wire per restored cell"
    );
    assert!(
        !restore_text.contains("\x1b_G"),
        "restoring re-sends no image bytes (no flash)"
    );
    // Cells erased by `CSI K` beside the sprite are text erase: the image data is untouched.
    assert!(
        restore_text.contains("\x1b[38;2;0;0;42m"),
        "id 42 in the foreground colour"
    );
}

#[test]
fn red_modal_over_stub_sprite_occludes_and_restores_exactly() {
    run_occlusion(&placeholder_cells(SPRITE_ID, SPRITE_COLS, SPRITE_ROWS));
}

#[test]
fn guard_modal_over_oracle_sprite_occludes_and_restores_exactly_with_the_existing_diff() {
    run_occlusion(&oracle_cells(SPRITE_ID, SPRITE_COLS, SPRITE_ROWS));
}

/// Row and column diacritics are absolute, so a sprite clipped by the frame edge keeps the right
/// (row, col) on every surviving cell: nothing is re-based by the clip.
fn run_clip(sprite: &Surface) {
    let mut frame = Surface::new(10, 6);
    // Top-left clip: sprite origin at (-2, -1).
    frame.blit_transparent_clipped(sprite, -2, -1, frame.area());
    assert_eq!(
        frame.get(0, 0).unwrap(),
        &oracle_cell(SPRITE_ID, 2, 1),
        "visible (0,0) shows sprite cell col 2, row 1"
    );
    assert_eq!(frame.get(5, 3).unwrap(), &oracle_cell(SPRITE_ID, 7, 4));
    assert!(
        !is_placeholder(frame.get(6, 0).unwrap()),
        "past the sprite's right edge"
    );
    assert!(
        !is_placeholder(frame.get(0, 4).unwrap()),
        "past the sprite's bottom edge"
    );

    // Bottom-right clip: sprite origin at (6, 3), only a 4x3 corner survives.
    let mut frame = Surface::new(10, 6);
    frame.blit_transparent_clipped(sprite, 6, 3, frame.area());
    assert_eq!(frame.get(6, 3).unwrap(), &oracle_cell(SPRITE_ID, 0, 0));
    assert_eq!(frame.get(9, 5).unwrap(), &oracle_cell(SPRITE_ID, 3, 2));
}

#[test]
fn red_clipped_stub_sprite_keeps_absolute_row_and_column_diacritics() {
    run_clip(&placeholder_cells(SPRITE_ID, SPRITE_COLS, SPRITE_ROWS));
}

#[test]
fn guard_clipped_oracle_sprite_keeps_absolute_row_and_column_diacritics() {
    run_clip(&oracle_cells(SPRITE_ID, SPRITE_COLS, SPRITE_ROWS));
}

/// Pose switching by id: the two frames differ only in the foreground colour of the box's cells.
/// The diff rewrites each cell once (the glyph is the same, the id colour changes) and no image
/// byte is involved.
#[test]
fn red_pose_switch_by_id_changes_only_the_foreground_of_each_cell() {
    let (ida, idb) = (1000u32, 2000u32);
    let a = placeholder_cells(ida, SPRITE_COLS, SPRITE_ROWS);
    let b = placeholder_cells(idb, SPRITE_COLS, SPRITE_ROWS);
    for (ca, cb) in a.cells.iter().zip(&b.cells) {
        assert_eq!(
            ca.glyph, cb.glyph,
            "same row/column diacritics for both poses"
        );
        assert_eq!(ca.style.fg, Some(id_color(ida)));
        assert_eq!(cb.style.fg, Some(id_color(idb)));
    }
    let diff = compute_diff(Some(&a), &b);
    assert_eq!(diff.exact_changed_cell_count(), 40);
    let text = String::from_utf8(AnsiCompiler::new().compile(&diff)).expect("utf8");
    assert!(
        !text.contains("\x1b_G"),
        "switching poses sends no image bytes"
    );
    assert!(text.contains("\x1b[38;2;0;7;208m"), "id 2000 = 0x0007D0");
    assert_eq!(text.matches(PLACEHOLDER).count(), 40);
}

// ---------------------------------------------------------------------------------------------
// The whole renderer: an unchanged frame is free
// ---------------------------------------------------------------------------------------------

fn render_frame(
    renderer: &mut Renderer,
    session: &mut TerminalSession,
    sprite_frame: &Surface,
) -> (usize, Vec<u8>) {
    let mut root = Node::col().child(Node::raster(sprite_frame.clone()));
    let mut out = Vec::new();
    let (_, _, bytes, _, _) = renderer
        .render(&mut root, session, &mut out)
        .expect("headless render");
    (bytes, out)
}

fn two_frames(sprite: &Surface) {
    let mut frame = Surface::new(30, 12);
    frame.blit_transparent_at(sprite, SPRITE_AT.0, SPRITE_AT.1);
    let mut session = TerminalSession::headless(30, 12);
    let mut renderer = Renderer::new(RenderMode::Fullscreen);

    let (b1, out1) = render_frame(&mut renderer, &mut session, &frame);
    let text1 = String::from_utf8(out1).expect("utf8");
    assert!(b1 > 0);
    assert_eq!(
        text1.matches(PLACEHOLDER).count(),
        (SPRITE_COLS * SPRITE_ROWS) as usize,
        "the first frame writes every placeholder cell"
    );
    assert!(text1.contains("\x1b[38;2;0;0;42m"));
    assert!(
        !text1.contains("\x1b_G"),
        "the renderer's frame carries cells only; uploads are a separate, explicit write"
    );

    let (b2, out2) = render_frame(&mut renderer, &mut session, &frame);
    assert_eq!(b2, 0, "an unchanged frame emits no bytes");
    assert!(out2.is_empty());
    assert!(
        !String::from_utf8_lossy(&out2).contains("\x1b_G"),
        "and no APC at all"
    );
}

#[test]
fn red_unchanged_stub_placeholder_frame_emits_zero_bytes_the_second_time() {
    two_frames(&placeholder_cells(SPRITE_ID, SPRITE_COLS, SPRITE_ROWS));
}

/// GREEN today: the existing renderer already writes hand-built placeholder cells and skips them
/// when unchanged. This is the evidence that the placeholder route needs no diff change.
#[test]
fn guard_unchanged_oracle_placeholder_frame_emits_zero_bytes_the_second_time() {
    two_frames(&oracle_cells(SPRITE_ID, SPRITE_COLS, SPRITE_ROWS));
}

// ---------------------------------------------------------------------------------------------
// Route gate: opt-in, multiplexers, ssh, colour depth
// ---------------------------------------------------------------------------------------------

#[test]
fn red_gate_supported_truecolor_clean_environment_uses_pixels() {
    assert_eq!(
        pixel_route(
            Capability::Supported,
            ColorDepth::TrueColor,
            false,
            env_of(&[])
        ),
        PixelRoute::Pixels
    );
}

#[test]
fn red_gate_tmux_and_screen_fall_back_unless_forced() {
    for pairs in [
        &[("TMUX", "tmux-socket,123,0")][..],
        &[("STY", "123.pts-0.host")][..],
        &[("TERM", "screen-256color")][..],
        &[("TERM", "tmux-256color")][..],
    ] {
        assert_eq!(
            pixel_route(
                Capability::Supported,
                ColorDepth::TrueColor,
                false,
                env_of(pairs)
            ),
            PixelRoute::Fallback(FallbackReason::Multiplexer),
            "{pairs:?}"
        );
        assert_eq!(
            pixel_route(
                Capability::Supported,
                ColorDepth::TrueColor,
                true,
                env_of(pairs)
            ),
            PixelRoute::Pixels,
            "force overrides the multiplexer check: {pairs:?}"
        );
    }
}

#[test]
fn red_gate_ssh_falls_back_unless_forced() {
    for pairs in [
        &[("SSH_CONNECTION", "10.0.0.2 5000 10.0.0.1 22")][..],
        &[("SSH_TTY", "/dev/pts/3")][..],
    ] {
        assert_eq!(
            pixel_route(
                Capability::Supported,
                ColorDepth::TrueColor,
                false,
                env_of(pairs)
            ),
            PixelRoute::Fallback(FallbackReason::Remote),
            "{pairs:?}"
        );
        assert_eq!(
            pixel_route(
                Capability::Supported,
                ColorDepth::TrueColor,
                true,
                env_of(pairs)
            ),
            PixelRoute::Pixels,
            "{pairs:?}"
        );
    }
}

#[test]
fn red_gate_unsupported_is_never_forced_into_pixels() {
    for force in [false, true] {
        assert_eq!(
            pixel_route(
                Capability::Unsupported,
                ColorDepth::TrueColor,
                force,
                env_of(&[])
            ),
            PixelRoute::Fallback(FallbackReason::Unsupported),
            "force={force}"
        );
    }
}

#[test]
fn red_gate_below_truecolor_falls_back_because_the_id_rides_in_the_colour() {
    for depth in [ColorDepth::Ansi256, ColorDepth::Ansi16, ColorDepth::Mono] {
        assert_eq!(
            pixel_route(Capability::Supported, depth, true, env_of(&[])),
            PixelRoute::Fallback(FallbackReason::NotTrueColor),
            "{depth:?}"
        );
    }
}

/// Opt-in is by the application alone: `Unknown` is never pixels, even forced, and environment
/// hints that say "kitty" never enable the route. Passes against the stub (which always falls
/// back with `NotOptedIn`); must keep passing.
#[test]
fn guard_gate_unknown_never_enables_pixels_and_the_environment_never_upgrades() {
    let kitty_env = [
        ("TERM", "xterm-kitty"),
        ("KITTY_WINDOW_ID", "1"),
        ("TERM_PROGRAM", "ghostty"),
    ];
    for force in [false, true] {
        for pairs in [&[][..], &kitty_env[..]] {
            assert_eq!(
                pixel_route(
                    Capability::Unknown,
                    ColorDepth::TrueColor,
                    force,
                    env_of(pairs)
                ),
                PixelRoute::Fallback(FallbackReason::NotOptedIn),
                "force={force} env={pairs:?}"
            );
        }
    }
}
