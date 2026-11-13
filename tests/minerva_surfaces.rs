//! Acceptance for consumer surfaces requested by ProjectGHOST/MINERVA after
//! v0.7.0-alpha.2: `progress_steps`, `toned_label` and `image`. Oracles read
//! painted cells and geometry, never implementation state.
use gibson::raster::RgbRaster;
use gibson::ui::prelude::*;
use gibson::{compute_layout, paint, Color, ColorDepth, Surface};

fn env(width: u16, height: u16, depth: ColorDepth) -> UiEnvironment {
    UiEnvironment {
        width,
        height,
        color_depth: depth,
        motion: MotionPreference::None,
        ..UiEnvironment::default()
    }
}

fn paint_with(el: &Element<()>, skin: Skin, e: UiEnvironment) -> Surface {
    let mut node = compile(el, &BuildCx::new(skin, e)).unwrap().node;
    compute_layout(&mut node, e.width, e.height).unwrap();
    let mut s = Surface::new(e.width, e.height);
    paint(&node, &mut s);
    s
}

fn paint_element(el: &Element<()>, e: UiEnvironment) -> Surface {
    paint_with(el, skins::SWISS_SIGNAL, e)
}

fn row_text(s: &Surface, y: u16) -> String {
    (0..s.width)
        .filter_map(|x| {
            let c = s.get(x, y).unwrap();
            (!c.is_continuation).then_some(c.glyph.grapheme.as_str().to_string())
        })
        .collect()
}

fn find(s: &Surface, needle: &str) -> Option<(u16, u16)> {
    (0..s.height).find_map(|y| {
        let row = row_text(s, y);
        row.find(needle)
            .map(|byte| (row[..byte].chars().count() as u16, y))
    })
}

fn occupied_rows(s: &Surface) -> Vec<u16> {
    (0..s.height)
        .filter(|&y| !row_text(s, y).trim().is_empty())
        .collect()
}

fn count_glyph(s: &Surface, glyph: &str) -> usize {
    (0..s.height)
        .flat_map(|y| (0..s.width).map(move |x| (x, y)))
        .filter(|&(x, y)| s.get(x, y).unwrap().glyph.grapheme.as_str() == glyph)
        .count()
}

// ---------------------------------------------------------------- progress_steps

const FULL: &[&str] = &["█", "▓", "━", "#", "="];
const EMPTY: &[&str] = &["─", "░", "·", ".", "-"];

/// (full cells, partial cells, meter width) on row `y`.
fn meter(s: &Surface, y: u16) -> (usize, usize, usize) {
    let (mut full, mut partial, mut empty) = (0, 0, 0);
    for x in 0..s.width {
        let g = s.get(x, y).unwrap().glyph.grapheme.as_str().to_string();
        if FULL.contains(&g.as_str()) {
            full += 1;
        } else if EMPTY.contains(&g.as_str()) {
            empty += 1;
        } else if ["▒", "╸", "+", ">"].contains(&g.as_str()) {
            partial += 1;
        }
    }
    (full, partial, full + partial + empty)
}

#[test]
fn steps_print_the_count_not_a_percentage() {
    let el: Element<()> = column().child(progress_steps("step", 7, 30).width(48));
    let s = paint_element(&el, env(60, 6, ColorDepth::TrueColor));
    assert!(find(&s, "step 7/30").is_some(), "{:?}", row_text(&s, 0));
    assert!(find(&s, "%").is_none());
    assert_eq!(occupied_rows(&s).len(), 2, "count row + meter row");
}

#[test]
fn steps_meter_never_runs_ahead_of_done_over_total() {
    for skin in [skins::SWISS_SIGNAL, skins::BLACK_ICE, skins::VAPOR95] {
        for total in [1u32, 3, 7, 30, 97] {
            for done in 0..=total {
                let el: Element<()> = column().child(progress_steps("s", done, total).width(48));
                let s = paint_with(&el, skin, env(60, 8, ColorDepth::TrueColor));
                let y = *occupied_rows(&s)
                    .iter()
                    .find(|&&y| meter(&s, y).2 > 0)
                    .unwrap();
                let (full, partial, inner) = meter(&s, y);
                let exact = u64::from(done) * inner as u64 * 8 / u64::from(total);
                assert_eq!(full as u64, exact / 8, "{done}/{total} full cells");
                assert_eq!(partial == 1, exact % 8 > 0, "{done}/{total} partial cell");
                assert!((full * 8) as u64 <= u64::from(done) * inner as u64 * 8 / u64::from(total));
            }
        }
    }
}

#[test]
fn steps_are_text_only_in_mono_and_at_forty_columns() {
    let el: Element<()> = column().child(progress_steps("step", 7, 30).width(48));
    let mono = paint_element(&el, env(60, 6, ColorDepth::Mono));
    assert_eq!(occupied_rows(&mono).len(), 1);
    assert!(find(&mono, "step 7/30").is_some());

    // The element's own width decides, not the terminal's: 40 columns wide in a
    // 100-column terminal is text-only, 41 has a meter, and so does the
    // default-width element only if it is wider than 40 (it is 24).
    for (width, rows) in [(Some(40), 1), (Some(41), 2), (None, 1)] {
        let el = progress_steps::<()>("step", 7, 30);
        let el = match width {
            Some(w) => el.width(w),
            None => el,
        };
        let s = paint_element(&column().child(el), env(100, 6, ColorDepth::TrueColor));
        assert_eq!(occupied_rows(&s).len(), rows, "{width:?}");
        assert!(find(&s, "step 7/30").is_some());
    }
}

#[test]
fn steps_clamp_and_handle_zero_total() {
    let el: Element<()> = column()
        .child(progress_steps("a", 9, 4).width(48))
        .child(progress_steps("b", 0, 0).width(48));
    let s = paint_element(&el, env(60, 8, ColorDepth::TrueColor));
    assert!(find(&s, "a 4/4").is_some());
    let (_, b) = find(&s, "b 0/0").unwrap();
    let (_, a) = find(&s, "a 4/4").unwrap();
    // 4/4 is a full meter; 0/0 is an empty one.
    let (full_a, partial_a, inner_a) = meter(&s, a + 1);
    assert_eq!((full_a, partial_a), (inner_a, 0));
    let (full_b, partial_b, inner_b) = meter(&s, b + 1);
    assert!(inner_b > 0);
    assert_eq!((full_b, partial_b), (0, 0));
}

#[test]
fn settled_steps_frame_is_identical_with_motion_on_and_off() {
    let el: Element<()> = column().child(progress_steps("step", 7, 30).width(48));
    for depth in [ColorDepth::TrueColor, ColorDepth::Ansi16, ColorDepth::Mono] {
        let off = paint_element(&el, env(60, 6, depth));
        let on = paint_element(
            &el,
            UiEnvironment {
                motion: MotionPreference::Full,
                ..env(60, 6, depth)
            },
        );
        assert!(off == on, "{depth:?}");
    }
}

#[test]
fn an_unfinished_fraction_never_reads_one_hundred_percent() {
    let el: Element<()> = column().child(progress("job", 0.996).width(48));
    let s = paint_element(&el, env(60, 6, ColorDepth::TrueColor));
    assert!(find(&s, "99%").is_some(), "{:?}", row_text(&s, 0));
    let done: Element<()> = column().child(progress("job", 1.0).width(48));
    assert!(find(
        &paint_element(&done, env(60, 6, ColorDepth::TrueColor)),
        "100%"
    )
    .is_some());
}

// ---------------------------------------------------------------- toned_label

fn provenance() -> Element<()> {
    column().child(toned_label([
        tone_span("SYNTHETIC").tone(Tone::Warning).strong(),
        tone_span(" · image · seed 42 · sha 9dc3").muted(),
    ]))
}

#[test]
fn toned_label_colours_one_word_from_the_skin() {
    let e = env(60, 4, ColorDepth::TrueColor);
    let s = paint_element(&provenance(), e);
    let (x, y) = find(&s, "SYNTHETIC").unwrap();
    let rs = skins::SWISS_SIGNAL.resolve(&e);
    let warn = rs.style(Tone::Warning, Emphasis::Strong);
    let muted = rs.style(Tone::Neutral, Emphasis::Muted);
    assert_eq!(s.get(x, y).unwrap().style.fg, warn.fg);
    assert_eq!(s.get(x + 12, y).unwrap().style.fg, muted.fg);
    assert_ne!(warn.fg, muted.fg);
    assert_eq!(occupied_rows(&s), vec![y]);
}

#[test]
fn a_narrow_toned_label_still_starts_with_synthetic() {
    for depth in [ColorDepth::TrueColor, ColorDepth::Ansi16, ColorDepth::Mono] {
        let s = paint_element(&provenance(), env(20, 4, depth));
        let y = occupied_rows(&s)[0];
        assert!(
            row_text(&s, y).trim_start().starts_with("SYNTHETIC"),
            "{depth:?}"
        );
        assert_eq!(occupied_rows(&s).len(), 1);
    }
}

#[test]
fn toned_label_strips_line_breaks_and_controls() {
    let el: Element<()> = column().child(toned_label([
        tone_span("ok\x1b[31m"),
        tone_span("x\nhidden"),
    ]));
    let s = paint_element(&el, env(30, 4, ColorDepth::TrueColor));
    assert_eq!(occupied_rows(&s).len(), 1);
    assert!(find(&s, "hidden").is_none());
    assert!(find(&s, "\x1b").is_none());
}

// ---------------------------------------------------------------- image

fn halves(w: u16, h: u16) -> RgbRaster {
    let mut r = RgbRaster::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let c = if x < w / 2 { (220, 0, 0) } else { (0, 0, 220) };
            r.set(x.into(), y.into(), c);
        }
    }
    r
}

const ALT: &str = "image 640×480 · sha256 3fa1 · SYNTHETIC";

fn picture(r: &RgbRaster) -> Element<()> {
    // gap(0): the label sits directly under the reserved box.
    column()
        .gap(0)
        .child(image(r, (40, 12), ALT))
        .child(label("BELOW"))
}

#[test]
fn mono_shows_only_the_alt_line() {
    let s = paint_element(&picture(&halves(640, 480)), env(40, 15, ColorDepth::Mono));
    assert_eq!(count_glyph(&s, "▀"), 0);
    assert!(find(&s, "image 640×480").is_some());
    assert_eq!(occupied_rows(&s).len(), 2, "alt + BELOW");
}

#[test]
fn truecolor_fits_640x480_within_40x12_preserving_aspect() {
    let s = paint_element(
        &picture(&halves(640, 480)),
        env(80, 24, ColorDepth::TrueColor),
    );
    let rows: Vec<u16> = (0..s.height)
        .filter(|&y| (0..s.width).any(|x| s.get(x, y).unwrap().glyph.grapheme.as_str() == "▀"))
        .collect();
    let cols = (0..s.width)
        .filter(|&x| s.get(x, rows[0]).unwrap().glyph.grapheme.as_str() == "▀")
        .count() as f64;
    let h = rows.len() as f64;
    assert!(cols <= 40.0 && h <= 12.0, "{cols}×{h}");
    // Half-block pixels are square: cols / (2·rows) should be 640/480.
    let ideal_cols = 2.0 * h * 640.0 / 480.0;
    assert!((cols - ideal_cols).abs() <= 1.0, "{cols} vs {ideal_cols}");
    // Left half red, right half blue.
    let y = rows[0];
    let x0 = (0..s.width)
        .find(|&x| s.get(x, y).unwrap().glyph.grapheme.as_str() == "▀")
        .unwrap();
    assert_eq!(s.get(x0, y).unwrap().style.fg, Some(Color::Rgb(220, 0, 0)));
    assert_eq!(
        s.get(x0 + cols as u16 - 1, y).unwrap().style.fg,
        Some(Color::Rgb(0, 0, 220))
    );
}

#[test]
fn the_box_height_is_reserved_whatever_is_painted() {
    let below = |el: &Element<()>, depth| {
        find(&paint_element(el, env(80, 24, depth)), "BELOW")
            .unwrap()
            .1
    };
    let wide = below(&picture(&halves(640, 480)), ColorDepth::TrueColor);
    assert_eq!(wide, 12);
    assert_eq!(below(&picture(&halves(640, 480)), ColorDepth::Mono), wide);
    assert_eq!(
        below(&picture(&halves(10, 10)), ColorDepth::TrueColor),
        wide
    );
    assert_eq!(
        below(&picture(&RgbRaster::new(0, 0)), ColorDepth::TrueColor),
        wide
    );
}

#[test]
fn small_boxes_render_alt_text_only() {
    for max in [(19, 12), (40, 5)] {
        let el: Element<()> = column().child(image(&halves(640, 480), max, ALT));
        let s = paint_element(&el, env(80, 24, ColorDepth::TrueColor));
        assert_eq!(count_glyph(&s, "▀"), 0, "{max:?}");
        assert!(find(&s, "image").is_some());
    }
}

#[test]
fn never_upscales_a_small_raster() {
    let el: Element<()> = column().child(image(&halves(8, 6), (40, 12), ALT));
    let s = paint_element(&el, env(80, 24, ColorDepth::TrueColor));
    assert_eq!(count_glyph(&s, "▀"), 8 * 3);
}

#[test]
fn a_narrower_space_shrinks_the_picture_not_the_height() {
    let el = picture(&halves(640, 480));
    let s = paint_element(&el, env(30, 24, ColorDepth::TrueColor));
    let (cols, rows) = picture_extent(&s);
    // Shrunk, not clipped: still 4:3 in square half-block pixels, and so fewer
    // than the 12 rows a clipped 32-column picture would keep.
    assert!((20..=30).contains(&cols), "{cols}");
    assert!(rows < 12, "{rows}");
    let ideal_cols = 2.0 * f64::from(rows) * 640.0 / 480.0;
    assert!((f64::from(cols) - ideal_cols).abs() <= 1.0, "{cols}×{rows}");
    assert_eq!(find(&s, "BELOW").unwrap().1, 12);
}

fn picture_extent(s: &Surface) -> (u16, u16) {
    let rows: Vec<u16> = (0..s.height)
        .filter(|&y| (0..s.width).any(|x| s.get(x, y).unwrap().glyph.grapheme.as_str() == "▀"))
        .collect();
    let cols = (0..s.width)
        .filter(|&x| s.get(x, rows[0]).unwrap().glyph.grapheme.as_str() == "▀")
        .count() as u16;
    (cols, rows.len() as u16)
}

#[test]
fn a_maximum_size_raster_is_fitted_from_a_bounded_sample() {
    // RgbRaster caps each side at MAX_RASTER_DIMENSION (2048); a 4096 request is clamped.
    let big = RgbRaster::new(4096, 4096);
    assert_eq!((big.width(), big.height()), (2048, 2048));
    let el: Element<()> = column().child(image(&big, (40, 12), ALT));
    let s = paint_element(&el, env(80, 24, ColorDepth::TrueColor));
    assert_eq!(count_glyph(&s, "▀"), 24 * 12, "24×24 px in 24×12 cells");
}

#[test]
fn alt_text_is_one_sanitised_row() {
    let el: Element<()> = column().child(image(&halves(640, 480), (40, 12), "a\x1b[2Jb\nsecond"));
    let s = paint_element(&el, env(40, 15, ColorDepth::Mono));
    assert_eq!(occupied_rows(&s).len(), 1);
    assert!(find(&s, "second").is_none());
    assert!(find(&s, "\x1b").is_none());
}
