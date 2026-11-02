//! Gate F5 — freeze the public C ABI layout that v0.6 commits to.
//!
//! The Gate J/K additions grew the ABI from 52 to 59 symbols, all ADDITIVE (new functions; no
//! existing signature changed), so an older ABI-v1 client keeps linking and `GIBSON_ABI_VERSION`
//! stays `1`. The new public *struct* `gibson_cell_t` (with `gibson_style_t` / `gibson_color_t`) is
//! now a compatibility commitment: its size, alignment, and field offsets are frozen here so an
//! accidental reorder/resize turns this suite red and forces a conscious ABI decision (bump the
//! version, or don't change the struct). The numbers mirror the committed `include/gibson.h`; the
//! Bindings CI proves C == Rust == Python on *behavior*, and this proves the *layout* the header
//! promises cannot drift unnoticed.

use std::mem::{align_of, offset_of, size_of};

use gibson::ffi::{
    GibsonCell, GibsonColor, GibsonStyle, GIBSON_ABI_VERSION, GIBSON_CELL_GLYPH_CAP,
};

#[test]
fn abi_version_is_one_and_glyph_cap_is_frozen() {
    assert_eq!(
        GIBSON_ABI_VERSION, 1,
        "the Gate J/K additions are purely additive; the ABI version must not bump"
    );
    assert_eq!(GIBSON_CELL_GLYPH_CAP, 16);
}

#[test]
fn gibson_color_layout_is_frozen() {
    assert_eq!(size_of::<GibsonColor>(), 8);
    assert_eq!(align_of::<GibsonColor>(), 4);
    assert_eq!(offset_of!(GibsonColor, color_type), 0);
    assert_eq!(offset_of!(GibsonColor, r), 4);
    assert_eq!(offset_of!(GibsonColor, g), 5);
    assert_eq!(offset_of!(GibsonColor, b), 6);
    assert_eq!(offset_of!(GibsonColor, _pad), 7);
}

#[test]
fn gibson_style_layout_is_frozen() {
    assert_eq!(size_of::<GibsonStyle>(), 24);
    assert_eq!(align_of::<GibsonStyle>(), 4);
    assert_eq!(offset_of!(GibsonStyle, fg), 0);
    assert_eq!(offset_of!(GibsonStyle, bg), 8);
    assert_eq!(offset_of!(GibsonStyle, bold), 16);
    assert_eq!(offset_of!(GibsonStyle, dim), 17);
    assert_eq!(offset_of!(GibsonStyle, italic), 18);
    assert_eq!(offset_of!(GibsonStyle, underline), 19);
    assert_eq!(offset_of!(GibsonStyle, reverse), 20);
    assert_eq!(offset_of!(GibsonStyle, _reserved), 21);
}

#[test]
fn gibson_cell_layout_is_frozen() {
    assert_eq!(
        size_of::<GibsonCell>(),
        48,
        "gibson_cell_t is a public ABI-v1 compatibility commitment"
    );
    assert_eq!(align_of::<GibsonCell>(), 4);
    assert_eq!(offset_of!(GibsonCell, glyph), 0);
    assert_eq!(offset_of!(GibsonCell, glyph_len), 16);
    assert_eq!(offset_of!(GibsonCell, width), 17);
    assert_eq!(offset_of!(GibsonCell, is_continuation), 18);
    assert_eq!(offset_of!(GibsonCell, transparent), 19);
    assert_eq!(offset_of!(GibsonCell, _reserved), 20);
    assert_eq!(offset_of!(GibsonCell, style), 24);
}
