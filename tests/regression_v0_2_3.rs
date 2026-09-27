//! Regression coverage for the v0.2.3 layout + custom-graphics-seam patch.
//!   #36 — Element `percent_width`/`percent_height` lower to Taffy `Dimension::Percent`.
//!   #43 — `Glyph::from_char` (no `String` alloc); `BrailleCanvas::blit_to_surface`
//!         (overlay preserving background); `Node::canvas` (size-aware deferred surface).
//!   #48 E-06 — `Surface::clip` clipped, translated sub-surface view.

use gibson::node::Dimension;
use gibson::ui::prelude::*;
use gibson::{
    compute_layout, paint, BrailleCanvas, Cell, Color, Glyph, Node, Rect, Style, Surface,
};
use std::sync::{Arc, Mutex};

// ---- #36: percent sizing on Element -------------------------------------

#[test]
fn issue_36_element_percent_lowers_to_taffy_percent() {
    let tree: Element<()> = row()
        .child(status("A").percent_width(60.0).percent_height(50.0))
        .child(status("B").percent_width(40.0));
    let env = UiEnvironment {
        width: 100,
        height: 20,
        ..UiEnvironment::default()
    };
    let node = compile(&tree, &BuildCx::new(skins::SWISS_SIGNAL, env))
        .unwrap()
        .node;
    let a = &node.children[0];
    assert!(
        matches!(a.layout_style.width, Dimension::Percent(p) if (p - 60.0).abs() < 0.01),
        "percent_width did not lower to Dimension::Percent: {:?}",
        a.layout_style.width
    );
    assert!(
        matches!(a.layout_style.height, Dimension::Percent(p) if (p - 50.0).abs() < 0.01),
        "percent_height did not lower: {:?}",
        a.layout_style.height
    );
}

// ---- #48 E-06: clipped sub-surface view ---------------------------------

#[test]
fn issue_48_e06_surface_clip_is_local_and_bounded() {
    let mut s = Surface::new(10, 5);
    {
        let mut c = s.clip(Rect::new(2, 1, 3, 2));
        assert_eq!(c.width(), 3);
        assert_eq!(c.height(), 2);
        assert!(c.set_cell(0, 0, Cell::new(Glyph::from_char('X'), Style::default())));
        assert!(
            !c.set_cell(3, 0, Cell::new(Glyph::from_char('Y'), Style::default())),
            "x >= clip width must be dropped"
        );
        assert!(
            !c.set_cell(0, 2, Cell::new(Glyph::from_char('Z'), Style::default())),
            "y >= clip height must be dropped"
        );
        c.print_str(0, 1, "ab", Style::default());
    }
    assert_eq!(
        s.get(2, 1).unwrap().glyph.grapheme.as_str(),
        "X",
        "local (0,0) maps to absolute (2,1)"
    );
    assert_eq!(s.get(2, 2).unwrap().glyph.grapheme.as_str(), "a");
    assert_eq!(s.get(3, 2).unwrap().glyph.grapheme.as_str(), "b");
    assert_eq!(
        s.get(5, 0).unwrap().glyph.grapheme.as_str(),
        " ",
        "an out-of-clip write must not touch the surface"
    );
}

// ---- #43: Glyph::from_char ----------------------------------------------

#[test]
fn issue_43_glyph_from_char_matches_new() {
    assert_eq!(Glyph::from_char('A').grapheme.as_str(), "A");
    assert_eq!(Glyph::from_char('A').display_width, 1);
    let wide = Glyph::from_char('界');
    assert_eq!(wide.grapheme.as_str(), "界");
    assert_eq!(wide.display_width, 2);
    assert_eq!(
        Glyph::from_char('Z').grapheme.as_str(),
        Glyph::new("Z").grapheme.as_str()
    );
    assert_eq!(
        Glyph::from_char('界').display_width,
        Glyph::new("界").display_width
    );
}

// ---- #43: BrailleCanvas::blit_to_surface --------------------------------

#[test]
fn issue_43_braille_blit_overlays_and_preserves_bg() {
    let mut bc = BrailleCanvas::new(3, 2);
    bc.set(0, 0); // pixel (0,0) -> a dot in cell (0,0)

    let mut s = Surface::new(3, 2);
    let base = Style::default().bg(Color::Rgb(10, 20, 30));
    for y in 0..2 {
        for x in 0..3 {
            s.set_cell(x, y, Cell::new(Glyph::from_char('.'), base));
        }
    }
    bc.blit_to_surface(&mut s, Style::default().fg(Color::Rgb(200, 200, 200)));

    let dot = s.get(0, 0).unwrap();
    assert_ne!(
        dot.glyph.grapheme.as_str(),
        ".",
        "the braille glyph should overlay the base cell"
    );
    assert_eq!(
        dot.style.bg,
        Some(Color::Rgb(10, 20, 30)),
        "the underlying background must be preserved"
    );
    let untouched = s.get(2, 1).unwrap();
    assert_eq!(
        untouched.glyph.grapheme.as_str(),
        ".",
        "a cell with no dot must be left untouched"
    );
}

// ---- #43: size-aware deferred canvas ------------------------------------

#[test]
fn issue_43_canvas_callback_receives_layout_resolved_rect() {
    let seen: Arc<Mutex<Option<(u16, u16)>>> = Arc::new(Mutex::new(None));
    let seen2 = seen.clone();
    let canvas = Node::canvas(move |rect: Rect| {
        *seen2.lock().unwrap() = Some((rect.width, rect.height));
        let mut s = Surface::new(rect.width.max(1), rect.height.max(1));
        s.print_str(0, 0, "CV", Style::default(), None);
        s
    })
    .flex_grow(1.0);

    let mut root = Node::row()
        .width(20.0)
        .height(2.0)
        .gap(0.0)
        .child(
            Node::text("SIDE", Style::default())
                .width(5.0)
                .flex_shrink(0.0),
        )
        .child(canvas);
    compute_layout(&mut root, 20, 2).unwrap();
    let mut surf = Surface::new(20, 2);
    paint(&root, &mut surf);

    let (w, h) = seen
        .lock()
        .unwrap()
        .expect("canvas callback must run during paint");
    assert_eq!(
        w, 15,
        "canvas must receive the layout-resolved width (20 total - 5 fixed side), not a guess"
    );
    assert!(h >= 1);
    let lines = surf.to_visible_lines();
    assert!(lines.iter().any(|l| l.contains("SIDE")), "{lines:?}");
    assert!(
        lines.iter().any(|l| l.contains("CV")),
        "canvas surface was not composited: {lines:?}"
    );
}
