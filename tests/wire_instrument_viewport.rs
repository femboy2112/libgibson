//! Viewport-sizing proofs for `wire_instrument` (Renaissance VIII, Gate G).
//!
//! Scope boundary: these certify the `u16`-wrap defect in viewport *sizing* — a legal field
//! dimension past `u16::MAX` must saturate to the display cap, not truncate to a sliver. The
//! broader Gate G obligations (nonzero source origin, anisotropic cell dimensions, pan/zoom, and
//! deriving probe coordinates from the same geometry as the display, with a real-terminal
//! walkthrough) are NOT covered here and remain open.

#[path = "../examples/wire_instrument/viewport.rs"]
mod viewport;

use viewport::viewport_dims;

/// The exact defect, executed: the pre-fix formula narrowed `cols` to `u16` *before* clamping, so a
/// 65536-column field wrapped to 0 and then clamped *up* to width 1. The fix clamps wide first.
#[test]
fn the_pre_fix_u16_cast_wrapped_a_huge_field_and_the_fix_does_not() {
    // Pre-fix: `(ext.cols() as u16).saturating_mul(ppc).clamp(1, 240)` with cols = 65536, ppc = 1.
    let buggy = (65536usize as u16).saturating_mul(1).clamp(1, 240);
    assert_eq!(
        buggy, 1,
        "documents the wrap: 65536 as u16 == 0 → clamped up to 1"
    );

    // Fixed: wide arithmetic, clamped before the narrowing cast.
    assert_eq!(
        viewport_dims(65536, 4, 1).0,
        240,
        "a 65536-wide field saturates to the display cap, not a sliver"
    );
}

/// Both `u16`-boundary field widths, and a full 2^20-cell field, size to the cap — never wrap.
#[test]
fn legal_u16_overflow_widths_saturate_not_wrap() {
    assert_eq!(viewport_dims(65536, 4, 1).0, 240);
    assert_eq!(viewport_dims(65537, 4, 1).0, 240);
    // 2^20 cells as 1024x1024 — the max legal field.
    assert_eq!(viewport_dims(1024, 1024, 1), (240, 80));
}

/// Small fields are untouched by the fix (regression anchor on the common case).
#[test]
fn small_fields_are_unchanged() {
    assert_eq!(viewport_dims(10, 5, 2), (20, 10));
    assert_eq!(viewport_dims(1, 1, 1), (1, 1));
}

/// Dimensions clamp to the display budget at and past the cap; `ppc` multiplies before clamping.
#[test]
fn dimensions_clamp_to_the_display_budget() {
    assert_eq!(viewport_dims(240, 80, 1), (240, 80));
    assert_eq!(viewport_dims(241, 81, 1), (240, 80));
    assert_eq!(viewport_dims(200, 50, 2), (240, 80)); // 400x100 → capped
}

/// Width and height clamp independently — a wide, short field keeps its exact (small) height.
#[test]
fn anisotropic_dimensions_clamp_independently() {
    assert_eq!(viewport_dims(100_000, 3, 1), (240, 3));
    assert_eq!(viewport_dims(2, 100_000, 1), (2, 80));
}

/// A zero zoom floors to 1 rather than collapsing the viewport to nothing.
#[test]
fn ppc_zero_is_floored_to_one() {
    assert_eq!(viewport_dims(10, 5, 0), (10, 5));
}
