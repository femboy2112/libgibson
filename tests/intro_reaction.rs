//! Integration tests for the `libgibson_intro_reaction` flagship's pure math
//! (transform placement + effect envelopes, the two-clock director) and its
//! load-bearing compositor contract (untouched cells stay byte-identical, a
//! touched Braille tile round-trips exactly through the same projector every
//! other surface uses). Synthetic film only — no ffmpeg, no clip, no TTY — so
//! this is hermetic and fast, same discipline as `tests/video_composite.rs`.
//!
//! Ugh, fine, yes, this is basically me poking every corner of somebody else's
//! moonshot with a stick to make sure it doesn't fall over. That's the job.

#[path = "../examples/libgibson_intro_reaction/compositor.rs"]
mod compositor;
#[path = "../examples/libgibson_intro_reaction/director.rs"]
mod director;
#[path = "../examples/temporal_video_compositor/film.rs"]
mod film;
#[path = "../examples/temporal_video_compositor/key.rs"]
mod key;
#[path = "../examples/libgibson_intro_reaction/transform.rs"]
mod transform;

use compositor::{composite_into, frame_index, reconstruct_tile, CompositeOpts};
use director::{resolve, stage_index, total_edit_seconds, CUES};
use film::KeyedFilm;
use key::{Bbox, KeyParams};
use transform::{resolve_effect, Beat, Effect, Fit, Framing, Transform};

use gibson::cell::{Cell, Color, Glyph, Style};
use gibson::temporal::project_rgb_subcells;
use gibson::{SubcellGlyphMode, Surface};

// ---------------------------------------------------------------------------
// transform: pure effect envelopes
// ---------------------------------------------------------------------------

#[test]
fn normal_effect_is_always_identity() {
    for p in [0.0, 0.1, 0.5, 0.9, 1.0] {
        let b = resolve_effect(Effect::Normal, p);
        assert_eq!(b.scale_mul, Beat::IDENTITY.scale_mul);
        assert_eq!(b.off_dx, Beat::IDENTITY.off_dx);
        assert_eq!(b.off_dy, Beat::IDENTITY.off_dy);
        assert_eq!(b.aspect, Beat::IDENTITY.aspect);
        assert_eq!(b.chroma, Beat::IDENTITY.chroma);
    }
}

#[test]
fn every_effect_is_finite_positive_and_deterministic() {
    let effects = [
        Effect::Normal,
        Effect::PunchZoom { peak: 1.8 },
        Effect::SmashIn { from: 2.4 },
        Effect::AspectStretch { amp: 0.35 },
        Effect::QuietHold,
        Effect::FreezeStutter { steps: 6.0 },
    ];
    for effect in effects {
        for p in [0.0, 0.1, 0.5, 0.9, 1.0] {
            let b1 = resolve_effect(effect, p);
            assert!(b1.scale_mul.is_finite() && b1.scale_mul > 0.0);
            assert!(b1.aspect.is_finite() && b1.aspect > 0.0);
            assert!(b1.off_dx.is_finite());
            assert!(b1.off_dy.is_finite());
            assert!(b1.chroma.is_finite());
            // Same progress twice -> the exact same beat (a reaction beat must be
            // fully determined by edit time, or replaying an `--at` seek lies).
            let b2 = resolve_effect(effect, p);
            assert_eq!(b1.scale_mul, b2.scale_mul);
            assert_eq!(b1.off_dx, b2.off_dx);
            assert_eq!(b1.off_dy, b2.off_dy);
            assert_eq!(b1.aspect, b2.aspect);
            assert_eq!(b1.chroma, b2.chroma);
            assert_eq!(b1.src_progress.is_some(), b2.src_progress.is_some());
        }
    }
}

#[test]
fn punch_zoom_reaches_peak_by_quarter_and_holds() {
    let peak = 1.9;
    for p in [0.25, 0.5, 0.9, 1.0] {
        let b = resolve_effect(Effect::PunchZoom { peak }, p);
        assert!(
            (b.scale_mul - peak).abs() < 1e-4,
            "p={p} scale_mul={} expected ~{peak}",
            b.scale_mul
        );
    }
    // Below the quarter mark it hasn't fully arrived yet.
    let early = resolve_effect(Effect::PunchZoom { peak }, 0.05).scale_mul;
    assert!(early > 1.0 && early < peak);
}

#[test]
fn smash_in_snaps_home_by_18_percent() {
    let from = 2.4;
    for p in [0.18, 0.3, 0.6, 1.0] {
        let b = resolve_effect(Effect::SmashIn { from }, p);
        assert_eq!(b.scale_mul, 1.0, "p={p} should already be home");
    }
    let b0 = resolve_effect(Effect::SmashIn { from }, 0.0);
    assert!(b0.scale_mul > 1.0, "at p=0 the smash starts oversized");
    assert_eq!(b0.scale_mul, from);
}

#[test]
fn aspect_stretch_returns_to_one_at_both_ends_and_swells_mid() {
    let amp = 0.35;
    let start = resolve_effect(Effect::AspectStretch { amp }, 0.0).aspect;
    let end = resolve_effect(Effect::AspectStretch { amp }, 1.0).aspect;
    let mid = resolve_effect(Effect::AspectStretch { amp }, 0.5).aspect;
    assert!((start - 1.0).abs() < 1e-5, "start aspect={start}");
    assert!((end - 1.0).abs() < 1e-5, "end aspect={end}");
    assert!(mid > 1.0 + 1e-3, "mid aspect={mid} should swell above 1.0");
}

// ---------------------------------------------------------------------------
// transform: Transform placement math
// ---------------------------------------------------------------------------

#[test]
fn inverse_round_trips_a_source_pixel_no_mirror() {
    let framing = Framing::new(Fit::Contain, 1.0, 0.0, 0.0, false);
    let (film_w, film_h) = (40u32, 20u32);
    // Same aspect as the film, so Contain scale is exactly 1.0 and offsets are 0.
    let (subw, subh) = (film_w, film_h);
    let t = Transform::resolve(&framing, &Beat::IDENTITY, film_w, film_h, subw, subh);
    assert_eq!(t.sx, 1.0);
    assert_eq!(t.sy, 1.0);
    assert_eq!(t.ox, 0.0);
    assert_eq!(t.oy, 0.0);

    let (fx, fy) = (10.0f32, 5.0f32);
    let dest = (t.ox + fx * t.sx, t.oy + fy * t.sy);
    let (ifx, ify) = t.inverse(dest.0, dest.1, film_w, film_h);
    assert!((ifx - fx).abs() < 1e-3, "ifx={ifx} expected {fx}");
    assert!((ify - fy).abs() < 1e-3, "ify={ify} expected {fy}");
}

#[test]
fn inverse_mirrors_the_x_axis_when_framing_mirrors() {
    let framing = Framing::new(Fit::Contain, 1.0, 0.0, 0.0, true);
    let (film_w, film_h) = (40u32, 20u32);
    let (subw, subh) = (film_w, film_h);
    let t = Transform::resolve(&framing, &Beat::IDENTITY, film_w, film_h, subw, subh);

    let (fx, fy) = (10.0f32, 5.0f32);
    let dest = (fx, fy); // sx=sy=1, ox=oy=0
    let (ifx, ify) = t.inverse(dest.0, dest.1, film_w, film_h);
    let expected_x = (film_w as f32 - 1.0) - fx;
    assert!(
        (ifx - expected_x).abs() < 1e-3,
        "ifx={ifx} expected {expected_x}"
    );
    assert!((ify - fy).abs() < 1e-3);
}

#[test]
fn dest_bbox_sub_is_empty_when_placed_far_off_grid() {
    let framing = Framing::new(Fit::Contain, 1.0, -10.0, 0.0, false);
    let (film_w, film_h) = (40u32, 20u32);
    let (subw, subh) = (40u32, 20u32);
    let t = Transform::resolve(&framing, &Beat::IDENTITY, film_w, film_h, subw, subh);
    let (_x, _y, w, h) = t.dest_bbox_sub(film_w, film_h, subw, subh);
    assert_eq!(
        (w, h),
        (0, 0),
        "a huge negative offset must vanish off-grid"
    );
}

#[test]
fn dest_bbox_sub_clamps_to_the_grid() {
    // On-grid but overscaled: bbox must clamp rather than report a span larger
    // than the destination grid.
    let framing = Framing::new(Fit::Cover, 3.0, 0.0, 0.0, false);
    let (film_w, film_h) = (40u32, 20u32);
    let (subw, subh) = (40u32, 20u32);
    let t = Transform::resolve(&framing, &Beat::IDENTITY, film_w, film_h, subw, subh);
    let (x, y, w, h) = t.dest_bbox_sub(film_w, film_h, subw, subh);
    assert!(x + w <= subw, "x={x} w={w} subw={subw}");
    assert!(y + h <= subh, "y={y} h={h} subh={subh}");
}

#[test]
fn contain_gives_uniform_scale_stretch_fills_independently() {
    let (film_w, film_h) = (40u32, 20u32);
    let (subw, subh) = (60u32, 60u32); // deliberately mismatched aspect

    let contain = Framing::new(Fit::Contain, 1.0, 0.0, 0.0, false);
    let tc = Transform::resolve(&contain, &Beat::IDENTITY, film_w, film_h, subw, subh);
    assert_eq!(tc.sx, tc.sy, "Contain must scale x/y uniformly");

    let stretch = Framing::new(Fit::Stretch, 1.0, 0.0, 0.0, false);
    let ts = Transform::resolve(&stretch, &Beat::IDENTITY, film_w, film_h, subw, subh);
    assert!(
        (ts.sx * film_w as f32 - subw as f32).abs() < 1e-3,
        "Stretch sx*film_w should hit subw exactly"
    );
    assert!(
        (ts.sy * film_h as f32 - subh as f32).abs() < 1e-3,
        "Stretch sy*film_h should hit subh exactly"
    );
}

// ---------------------------------------------------------------------------
// director: the two-clock resolver
// ---------------------------------------------------------------------------

#[test]
fn total_edit_seconds_is_the_edit_end_and_covers_every_cue() {
    assert_eq!(total_edit_seconds(), director::EDIT_END);
    const { assert!(director::EDIT_END >= director::INTRO_LAST) };
    // Every reaction window fits within the cut.
    for (i, c) in CUES.iter().enumerate() {
        assert!(c.edit_start >= 0.0, "cue {i} starts before 0");
        assert!(
            c.edit_end <= director::EDIT_END,
            "cue {i} ends past EDIT_END"
        );
        assert!(c.edit_end > c.edit_start, "cue {i} is not a forward window");
    }
}

#[test]
fn cue_edit_start_is_monotonic_and_windows_do_not_overlap() {
    for i in 1..CUES.len() {
        assert!(
            director::cue_edit_start(i) >= director::cue_edit_start(i - 1),
            "cue_edit_start regressed at {i}"
        );
        // Windows are laid out in order and never overlap (gaps are allowed).
        assert!(
            CUES[i].edit_start >= CUES[i - 1].edit_end,
            "cue {i} overlaps the previous window"
        );
    }
}

#[test]
fn active_cue_is_some_inside_a_window_and_none_in_the_gaps() {
    for (i, c) in CUES.iter().enumerate() {
        // Start is inclusive; a point mid-window is cue i.
        assert_eq!(
            director::active_cue(c.edit_start),
            Some(i),
            "start of cue {i}"
        );
        let mid = (c.edit_start + c.edit_end) * 0.5;
        assert_eq!(director::active_cue(mid), Some(i), "mid of cue {i}");
        // End is exclusive: the boundary belongs to the gap or the next window.
        assert_ne!(
            director::active_cue(c.edit_end),
            Some(i),
            "end of cue {i} must be exclusive"
        );
    }
    // Before the first window the film plays alone (no reaction).
    assert_eq!(director::active_cue(0.0), None);
    assert!(
        CUES[0].edit_start > 0.0,
        "the film should open with no overlay"
    );
}

#[test]
fn resolve_at_zero_plays_the_film_from_the_top_with_no_reaction() {
    let r = resolve(0.0);
    assert_eq!(r.intro_seconds, 0.0, "the film starts at narrative time 0");
    assert_eq!(r.cue_index, None, "no reaction beat at the very start");
    assert_eq!(r.src_progress, 0.0);
}

#[test]
fn intro_plays_continuously_one_to_one_then_holds_for_the_sting() {
    // 1:1 with the edit clock while the film plays.
    for &t in &[0.0f32, 5.0, 12.0, 33.0, 60.0, 71.0] {
        assert!(
            (director::intro_seconds(t) - t).abs() < 1e-6,
            "intro should play 1:1 at t={t}"
        );
        assert!((resolve(t).intro_seconds - t).abs() < 1e-6);
    }
    // Held final frame once the film is over (the sting tail).
    for &t in &[72.5f32, 75.0, director::EDIT_END] {
        assert_eq!(
            director::intro_seconds(t),
            director::INTRO_LAST,
            "intro must hold its final frame during the sting at t={t}"
        );
    }
    assert_eq!(
        director::intro_seconds(-3.0),
        0.0,
        "negative edit clamps to 0"
    );
    // Monotonic nondecreasing across the whole cut (no skips or rewinds).
    let mut prev = director::intro_seconds(0.0);
    let mut t = 0.0f32;
    while t <= director::EDIT_END {
        let cur = director::intro_seconds(t);
        assert!(cur + 1e-6 >= prev, "intro_seconds regressed at t={t}");
        prev = cur;
        t += 0.25;
    }
}

#[test]
fn src_progress_in_range_gaps_are_zeroed_and_resolve_is_deterministic() {
    let total = total_edit_seconds();
    let mut t = 0.0f32;
    while t <= total {
        let r1 = resolve(t);
        assert!(
            (0.0..=1.0).contains(&r1.src_progress),
            "t={t} src_progress out of range"
        );
        // In a gap there is no active beat, and the cue fields are zeroed.
        if r1.cue_index.is_none() {
            assert_eq!(r1.cue_local, 0.0);
            assert_eq!(r1.cue_dur, 0.0);
            assert_eq!(r1.src_progress, 0.0);
        }
        let r2 = resolve(t);
        assert_eq!(r1.intro_seconds, r2.intro_seconds);
        assert_eq!(r1.cue_index, r2.cue_index);
        assert_eq!(r1.cue_local, r2.cue_local);
        assert_eq!(r1.cue_dur, r2.cue_dur);
        assert_eq!(r1.src_progress, r2.src_progress);
        t += 0.37; // an irrational-ish step so it doesn't land on every boundary
    }
}

#[test]
fn stage_index_resolves_known_names_and_rejects_unknown() {
    for (i, c) in CUES.iter().enumerate() {
        assert_eq!(stage_index(c.name), Some(i));
    }
    assert_eq!(stage_index("this_stage_does_not_exist"), None);
}

// ---------------------------------------------------------------------------
// compositor: the load-bearing contract
// ---------------------------------------------------------------------------

#[test]
fn braille_tile_round_trips_exactly_through_the_same_projector() {
    // Bit 0 clear so this mask matches the projector's own canonical
    // (subpixel-0-is-background) representative directly — no complement swap.
    const MASK: u8 = 0b1011_0100;
    let fg = [255u8, 255, 255];
    let bg = [0u8, 0, 0];
    let ch = char::from_u32(0x2800 + MASK as u32).unwrap();
    let cell = Cell::new(
        Glyph::from_char(ch),
        Style::new()
            .fg(Color::Rgb(fg[0], fg[1], fg[2]))
            .bg(Color::Rgb(bg[0], bg[1], bg[2])),
    );

    let tile = reconstruct_tile(&cell);
    for i in 0..8u8 {
        let expected = if MASK & (1 << i) != 0 { fg } else { bg };
        assert_eq!(tile[i as usize], expected, "dot {i}");
    }

    let proj = project_rgb_subcells(tile);
    assert_eq!(
        proj.static_mask, MASK,
        "an exact 2-colour tile must fit its own bipartition exactly"
    );
    let round_tripped = SubcellGlyphMode::Braille2x4
        .subcell_glyph(proj.static_mask)
        .expect("non-zero mask always realizes a glyph");
    assert_eq!(round_tripped, ch, "the glyph must round-trip exactly");
}

fn make_framing_full_frame() -> Framing {
    Framing::new(Fit::Contain, 1.0, 0.0, 0.0, false)
}

fn base_surface(w: u16, h: u16) -> Surface {
    let mut s = Surface::new(w, h);
    let style = Style::new()
        .fg(Color::Rgb(50, 60, 70))
        .bg(Color::Rgb(10, 20, 30));
    for y in 0..h {
        for x in 0..w {
            s.set_cell(x, y, Cell::new(Glyph::from_char('#'), style));
        }
    }
    s
}

#[test]
fn pure_key_film_leaves_every_cell_byte_identical() {
    let (w, h) = (24u16, 12u16);
    let mut surface = base_surface(w, h);
    let before = surface.clone();

    let (film_w, film_h) = (32u32, 16u32);
    // Zero-size rect: `KeyedFilm::synthetic` draws nothing, so every pixel of
    // every frame is pure key green. Nothing in the frame should ever be able
    // to touch the surface.
    let film = KeyedFilm::synthetic(
        film_w,
        film_h,
        3,
        Bbox {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        },
        0,
        0,
        [200, 180, 170],
    );
    let kp = KeyParams::default();
    let framing = make_framing_full_frame();
    let t = Transform::resolve(
        &framing,
        &Beat::IDENTITY,
        film_w,
        film_h,
        w as u32 * 2,
        h as u32 * 4,
    );

    let stats = composite_into(
        &mut surface,
        &film,
        film.frame(0),
        &t,
        &kp,
        SubcellGlyphMode::Braille2x4,
        CompositeOpts::default(),
    );
    assert_eq!(
        stats.cells_composited, 0,
        "a pure-key frame contributes zero alpha everywhere, so nothing should composite"
    );
    assert_eq!(
        surface, before,
        "every cell, including ones inside the subject's bbox, must be untouched"
    );
}

/// Replicates `composite_into`'s own dest-cell-bbox math so the test can check
/// the outside/inside partition without peeking at private internals.
fn expected_cell_bbox(
    t: &Transform,
    film_w: u32,
    film_h: u32,
    surface: &Surface,
) -> (u16, u16, u16, u16) {
    let subw = surface.width as u32 * 2;
    let subh = surface.height as u32 * 4;
    let (bx, by, bw, bh) = t.dest_bbox_sub(film_w, film_h, subw, subh);
    if bw == 0 || bh == 0 {
        return (0, 0, 0, 0);
    }
    let cx0 = bx / 2;
    let cy0 = by / 4;
    let cx1 = (bx + bw).div_ceil(2).min(surface.width as u32);
    let cy1 = (by + bh).div_ceil(4).min(surface.height as u32);
    (
        cx0 as u16,
        cy0 as u16,
        (cx1 - cx0) as u16,
        (cy1 - cy0) as u16,
    )
}

#[test]
fn opaque_subject_composites_inside_bbox_and_leaves_outside_untouched() {
    let (w, h) = (24u16, 12u16);
    let mut surface = base_surface(w, h);
    let before = surface.clone();

    let (film_w, film_h) = (32u32, 16u32);
    let rect = Bbox {
        x: film_w / 4,
        y: film_h / 4,
        w: film_w / 2,
        h: film_h / 2,
    };
    let film = KeyedFilm::synthetic(film_w, film_h, 1, rect, 0, 0, [200, 180, 170]);
    let kp = KeyParams::default();
    let framing = make_framing_full_frame();
    let t = Transform::resolve(
        &framing,
        &Beat::IDENTITY,
        film_w,
        film_h,
        w as u32 * 2,
        h as u32 * 4,
    );

    let (cx0, cy0, cw, ch) = expected_cell_bbox(&t, film_w, film_h, &surface);
    assert!(
        cw > 0 && ch > 0,
        "an opaque rect placed on-grid must yield a real dest bbox"
    );

    let stats = composite_into(
        &mut surface,
        &film,
        film.frame(0),
        &t,
        &kp,
        SubcellGlyphMode::Braille2x4,
        CompositeOpts::default(),
    );

    assert!(
        stats.cells_composited > 0,
        "the opaque subject must show up somewhere"
    );
    assert_eq!(
        stats.cells_touched,
        cw as usize * ch as usize,
        "cells_touched must equal exactly the dest cell-bbox's cell count"
    );

    for y in 0..h {
        for x in 0..w {
            let inside = x >= cx0 && x < cx0 + cw && y >= cy0 && y < cy0 + ch;
            if !inside {
                assert_eq!(
                    surface.get(x, y),
                    before.get(x, y),
                    "cell ({x},{y}) is outside the dest bbox and must stay byte-identical"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// compositor: frame_index
// ---------------------------------------------------------------------------

#[test]
fn frame_index_covers_clamps_endpoints_and_is_nondecreasing() {
    let n = 8usize;
    assert_eq!(frame_index(n, 0.0), 0);
    assert_eq!(frame_index(n, 1.0), n - 1);
    assert_eq!(
        frame_index(n, 1.5),
        n - 1,
        "p>1 must clamp to the last frame"
    );
    assert_eq!(frame_index(n, -0.5), 0, "p<0 must clamp to the first frame");

    let mut prev = frame_index(n, 0.0);
    let mut p = 0.0f32;
    while p <= 1.0 {
        let cur = frame_index(n, p);
        assert!(cur >= prev, "frame_index must be nondecreasing in p: p={p}");
        prev = cur;
        p += 0.03;
    }
}
