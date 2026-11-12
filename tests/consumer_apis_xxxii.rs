//! Acceptance for three consumer-requested APIs (ProjectGHOST/MINERVA, Renaissance
//! XXXII): `ui::rich_label`, headless `Context::resize`, and `gibson::text` widths.
//! Oracles read painted cells and frame geometry, not implementation state.
use gibson::context::{Context, RenderMode};
use gibson::ui::prelude::*;
use gibson::{compute_layout, paint, Color, ColorDepth, Node, Span, Style, Surface};
use std::io::ErrorKind;

fn env(width: u16, height: u16) -> UiEnvironment {
    UiEnvironment {
        width,
        height,
        color_depth: ColorDepth::TrueColor,
        motion: MotionPreference::None,
        ..UiEnvironment::default()
    }
}

fn paint_element(el: &Element<()>, e: UiEnvironment) -> Surface {
    let mut node = compile(el, &BuildCx::new(skins::SWISS_SIGNAL, e))
        .unwrap()
        .node;
    compute_layout(&mut node, e.width, e.height).unwrap();
    let mut s = Surface::new(e.width, e.height);
    paint(&node, &mut s);
    s
}

fn row_text(s: &Surface, y: u16) -> String {
    (0..s.width)
        .filter_map(|x| {
            let c = s.get(x, y).unwrap();
            (!c.is_continuation).then_some(c.glyph.grapheme.as_str().to_string())
        })
        .collect()
}

/// Locates `needle` on any row; returns (x, y) of its first cell.
fn find(s: &Surface, needle: &str) -> Option<(u16, u16)> {
    let first = needle.chars().next()?.to_string();
    for y in 0..s.height {
        for x in 0..s.width {
            if s.get(x, y).unwrap().glyph.grapheme.as_str() == first {
                let rest: String = (x..s.width)
                    .map(|cx| s.get(cx, y).unwrap().glyph.grapheme.as_str().to_string())
                    .collect();
                if rest.starts_with(needle) {
                    return Some((x, y));
                }
            }
        }
    }
    None
}

fn occupied_rows(s: &Surface) -> Vec<u16> {
    (0..s.height)
        .filter(|&y| !row_text(s, y).trim().is_empty())
        .collect()
}

const GREEN: Color = Color::Rgb(0, 200, 0);
const RED: Color = Color::Rgb(220, 0, 0);

#[test]
fn rich_label_paints_each_span_in_its_own_style_on_one_row() {
    let el: Element<()> = column().child(rich_label(vec![
        Span::styled("12 │ ", Style::new().fg(RED)),
        Span::styled("let x = 1;", Style::new().fg(GREEN)),
    ]));
    let s = paint_element(&el, env(40, 6));
    let (gx, gy) = find(&s, "12 │ ").expect("gutter painted");
    let (cx, cy) = find(&s, "let x = 1;").expect("code painted");
    assert_eq!(gy, cy, "one row");
    assert_eq!(cx, gx + 5, "spans are contiguous");
    assert_eq!(s.get(gx, gy).unwrap().style.fg, Some(RED));
    assert_eq!(s.get(cx, cy).unwrap().style.fg, Some(GREEN));
    assert_eq!(occupied_rows(&s), vec![gy]);
}

#[test]
fn rich_label_never_wraps_and_clips_to_width() {
    // A word crossing the edge discriminates NoWrap (clip mid-word) from word wrap
    // (which would move the whole word off the one visible row).
    let el: Element<()> = column()
        .child(rich_label(vec![
            Span::styled("ab ", Style::new().fg(GREEN)),
            Span::styled("verylongwordcrossing", Style::new().fg(RED)),
        ]))
        .child(label("NEXT"));
    let s = paint_element(&el, env(10, 6));
    let (x, y) = find(&s, "ab verylon").expect("clipped mid-word on one row");
    assert_eq!(x, 0);
    assert_eq!(s.get(9, y).unwrap().style.fg, Some(RED));
    let (_, ny) = find(&s, "NEXT").expect("following row painted");
    assert_eq!(ny, y + 1, "the next element sits on the very next row");
    assert_eq!(occupied_rows(&s), vec![y, ny]);
}

#[test]
fn a_cut_span_ends_the_row_so_later_spans_never_fill_its_columns() {
    // The wide glyph does not fit in the last column; ")" must not take its place.
    let el: Element<()> = column().child(rich_label(vec![Span::raw("foo(日"), Span::raw(")")]));
    let s = paint_element(&el, env(5, 3));
    let (x, y) = find(&s, "foo(").expect("prefix painted");
    assert_eq!(x, 0);
    assert_eq!(row_text(&s, y).trim_end(), "foo(");
    assert!(
        find(&s, ")").is_none(),
        "a later span leaked into the cut column"
    );
}

#[test]
fn a_line_break_ends_a_rich_label_like_a_label() {
    let e = env(30, 4);
    let s = paint_element(
        &column().child(rich_label(vec![Span::raw("a\nb"), Span::raw("c")])),
        e,
    );
    let (_, y) = find(&s, "a").unwrap();
    assert_eq!(row_text(&s, y).trim_end(), "a");
    // Siblings are laid out against what is painted, exactly as for `label`.
    let x_after = |first: Element<()>| {
        let s = paint_element(&row().child(first).child(label("X")), e);
        find(&s, "X").unwrap()
    };
    assert_eq!(
        x_after(rich_label(vec![Span::raw("ab\ncd")])),
        x_after(label("ab\ncd"))
    );
    assert_eq!(
        x_after(rich_label(vec![Span::raw("a\tb\x1bc")])),
        x_after(label("abc"))
    );
}

#[test]
fn rich_label_with_plain_spans_matches_label_cells() {
    // Control: an unstyled rich label is indistinguishable from `label`.
    let e = env(30, 4);
    let plain = paint_element(&column().child(label("status: ok")), e);
    let rich = paint_element(
        &column().child(rich_label(vec![Span::raw("status: "), Span::raw("ok")])),
        e,
    );
    for y in 0..e.height {
        for x in 0..e.width {
            assert_eq!(plain.get(x, y), rich.get(x, y), "cell ({x},{y})");
        }
    }
}

#[test]
fn rich_label_span_colour_overrides_element_tone_but_inherits_it_when_unset() {
    let e = env(40, 4);
    let toned = paint_element(&column().child(label("base").tone(Tone::Danger)), e);
    let (bx, by) = find(&toned, "base").unwrap();
    let tone_fg = toned.get(bx, by).unwrap().style.fg;
    assert!(
        tone_fg.is_some(),
        "tone gives the label a colour in this skin"
    );

    let el: Element<()> = column().child(
        rich_label(vec![
            Span::raw("base"),
            Span::styled("own", Style::new().fg(GREEN)),
        ])
        .tone(Tone::Danger),
    );
    let s = paint_element(&el, e);
    let (x, y) = find(&s, "baseown").unwrap();
    assert_eq!(
        s.get(x, y).unwrap().style.fg,
        tone_fg,
        "unset span inherits tone"
    );
    assert_eq!(
        s.get(x + 4, y).unwrap().style.fg,
        Some(GREEN),
        "span colour wins"
    );
}

#[test]
fn headless_resize_keeps_one_context_and_changes_frame_geometry() {
    let mut ctx = Context::headless(RenderMode::Fullscreen, 80, 24);
    ctx.set_root(Node::text("probe", Style::default()));
    ctx.render().unwrap();
    assert_eq!(ctx.frame_size(), Some((80, 24)));
    for (c, r) in [(40, 15), (120, 40), (100, 32), (80, 24)] {
        ctx.resize(c, r).unwrap();
        ctx.render().unwrap();
        assert_eq!(ctx.frame_size(), Some((c, r)));
        assert!(ctx.last_frame_lines().iter().any(|l| l.contains("probe")));
    }
}

#[test]
fn headless_resize_drives_ui_runtime_environment() {
    // UiRuntime builds from the session geometry each frame; resize must reach it.
    let mut ctx = Context::headless(RenderMode::Fullscreen, 80, 24);
    for (c, r) in [(40u16, 15u16), (120, 40)] {
        ctx.resize(c, r).unwrap();
        let mut seen = None;
        App::fullscreen()
            .run_with_context(
                &mut ctx,
                (),
                |_, _, _| Ok(Control::Continue),
                |_, cx: &BuildCx| {
                    seen = Some((cx.environment.width, cx.environment.height));
                    column::<()>().child(label("x"))
                },
            )
            .unwrap();
        assert_eq!(seen, Some((c, r)));
        assert_eq!(ctx.frame_size(), Some((c, r)));
    }
}

#[test]
fn headless_resize_rejects_zero_without_changing_geometry() {
    let mut ctx = Context::headless(RenderMode::Fullscreen, 50, 20);
    ctx.set_root(Node::text("z", Style::default()));
    for (c, r) in [(0, 10), (10, 0), (0, 0)] {
        let err = ctx.resize(c, r).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }
    ctx.render().unwrap();
    assert_eq!(ctx.frame_size(), Some((50, 20)));
}

#[test]
fn resize_on_a_terminal_context_is_unsupported() {
    // A stdout-backed context in a test process: geometry belongs to the terminal.
    let mut ctx = Context::inline().expect("stdout context");
    let err = ctx.resize(40, 15).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Unsupported);
}

#[test]
fn text_widths_agree_with_painted_columns() {
    let e = env(40, 3);
    for s in [
        "abc",
        "日本語",
        "e\u{301}x",
        "mixed 日本",
        "a\tb",
        "\u{17D8}",
        // Not here: Arabic lam-alef and ZWJ-joined Hebrew. Layout under-measures them
        // (whole-string width) for `label` and `rich_label` alike, so a following
        // sibling is clipped; `display_width` itself matches the painter for them
        // (unit test in src/text.rs). Tracked as a separate layout issue.
    ] {
        let el: Element<()> = column().child(rich_label(vec![Span::raw(s), Span::raw("|")]));
        let surf = paint_element(&el, e);
        let (x, y) = find(&surf, "|").unwrap();
        let start = (0..e.width)
            .find(|&cx| {
                !surf
                    .get(cx, y)
                    .unwrap()
                    .glyph
                    .grapheme
                    .as_str()
                    .trim()
                    .is_empty()
            })
            .unwrap();
        assert_eq!(
            (x - start) as usize,
            gibson::text::display_width(s),
            "{s:?}"
        );
    }
    assert_eq!(gibson::text::char_width('日'), 2);
    assert_eq!(gibson::text::char_width('\x07'), 0);
}

#[test]
fn headless_resize_repaints_fully_at_the_new_size() {
    let mut ctx = Context::headless(RenderMode::Fullscreen, 80, 24);
    ctx.set_root(Node::text("probe", Style::default()));
    ctx.render().unwrap();
    ctx.take_rendered_bytes();
    ctx.render().unwrap();
    let steady = ctx.take_rendered_bytes();
    assert!(
        !String::from_utf8_lossy(&steady).contains("\x1b[2J"),
        "control: no clear"
    );
    ctx.resize(40, 15).unwrap();
    ctx.render().unwrap();
    let after = String::from_utf8_lossy(&ctx.take_rendered_bytes()).into_owned();
    assert!(after.contains("\x1b[2J"), "a shrink must clear stale cells");
    assert!(after.contains("probe"));
}
