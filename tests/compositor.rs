//! Layer-compositor tests: explicit transparency, style-only overlays, z-order,
//! overlay removal (no ghost cells) and no-reflow behaviour.

use gibson::cell::{Cell, Glyph, Style};
use gibson::layout::compute_layout;
use gibson::node::{Node, NodeKind};
use gibson::painter::paint;
use gibson::surface::{BorderType, Surface};

#[test]
fn transparent_cells_leave_lower_layer_untouched() {
    let mut base = Surface::new(6, 1);
    base.print_str(0, 0, "HELLO", Style::default(), None);

    let mut layer = Surface::new_transparent(6, 1);
    layer.print_str(2, 0, "X", Style::default(), None);

    base.blit_transparent(&layer);

    assert_eq!(base.get(0, 0).unwrap().glyph.grapheme.as_str(), "H");
    assert_eq!(base.get(1, 0).unwrap().glyph.grapheme.as_str(), "E");
    assert_eq!(base.get(2, 0).unwrap().glyph.grapheme.as_str(), "X");
    assert_eq!(base.get(3, 0).unwrap().glyph.grapheme.as_str(), "L");
    assert_eq!(base.get(4, 0).unwrap().glyph.grapheme.as_str(), "O");
}

#[test]
fn style_only_overlay_keeps_glyph_and_merges_style() {
    let mut base = Surface::new(3, 1);
    base.set_cell(0, 0, Cell::new(Glyph::new("A"), Style::new().bold()));

    let mut layer = Surface::new_transparent(3, 1);
    layer.set_cell(0, 0, Cell::style_overlay(Style::new().dim()));
    base.blit_transparent(&layer);

    let c = base.get(0, 0).unwrap();
    assert_eq!(c.glyph.grapheme.as_str(), "A");
    assert!(c.style.bold, "existing bold must survive a dim veil");
    assert!(c.style.dim, "veil must add dim");
}

#[test]
fn stack_children_share_the_same_rectangle() {
    let mut stack = Node::stack()
        .width(20.0)
        .height(5.0)
        .child(Node::text("base", Style::default()))
        .child(Node::text("overlay", Style::default()));
    compute_layout(&mut stack, 40, 10).unwrap();
    assert_eq!(stack.children.len(), 2);
    assert_eq!(
        stack.children[0].computed_rect,
        stack.children[1].computed_rect
    );
    assert_eq!(stack.children[0].computed_rect.width, 20);
    assert_eq!(stack.children[0].computed_rect.height, 5);
}

#[test]
fn opaque_overlay_replaces_and_transparent_does_not() {
    let mut base = Surface::new(8, 1);
    base.print_str(0, 0, "ABCDEFGH", Style::default(), None);

    let mut layer = Surface::new_transparent(8, 1);
    // paint an opaque box border only in the middle
    layer.set_cell(3, 0, Cell::new(Glyph::new("Z"), Style::default()));
    base.blit_transparent(&layer);

    assert_eq!(base.get(2, 0).unwrap().glyph.grapheme.as_str(), "C");
    assert_eq!(base.get(3, 0).unwrap().glyph.grapheme.as_str(), "Z");
    assert_eq!(base.get(4, 0).unwrap().glyph.grapheme.as_str(), "E");
}

#[test]
fn wide_glyph_is_cleared_when_overlay_writes_its_continuation() {
    let mut base = Surface::new(8, 1);
    base.print_str(0, 0, "你", Style::default(), None);
    assert!(base.get(1, 0).unwrap().is_continuation);

    let mut layer = Surface::new_transparent(8, 1);
    layer.set_cell(1, 0, Cell::new(Glyph::new("x"), Style::default()));
    base.blit_transparent(&layer);

    // Lead must have been cleared to a space, continuation replaced by x.
    assert_eq!(base.get(0, 0).unwrap().glyph.grapheme.as_str(), " ");
    assert_eq!(base.get(1, 0).unwrap().glyph.grapheme.as_str(), "x");
    assert!(!base.get(1, 0).unwrap().is_continuation);
}

#[test]
fn multiple_z_layers_composite_in_order() {
    let mut base = Surface::new(4, 1);
    base.print_str(0, 0, "aaaa", Style::default(), None);

    let mut mid = Surface::new_transparent(4, 1);
    mid.print_str(0, 0, "bb", Style::default(), None);
    base.blit_transparent(&mid);

    let mut top = Surface::new_transparent(4, 1);
    top.print_str(0, 0, "c", Style::default(), None);
    base.blit_transparent(&top);

    let s: String = (0..4)
        .map(|x| base.get(x, 0).unwrap().glyph.grapheme.to_string())
        .collect();
    assert_eq!(s, "cbaa");
}

#[test]
fn stack_paints_overlay_without_destroying_base_interior() {
    let mut root = Node::stack()
        .width(20.0)
        .height(5.0)
        .child(Node::border_box(BorderType::Ascii, Style::default()))
        .child(
            Node::row()
                .padding_left(3.0)
                .child(Node::text("modal", Style::default())),
        );
    compute_layout(&mut root, 20, 5).unwrap();
    let mut s = Surface::new(20, 5);
    paint(&root, &mut s);

    // Base border corners remain (overlay only wrote its own cells).
    assert_eq!(s.get(0, 0).unwrap().glyph.grapheme.as_str(), "+");
    assert_eq!(s.get(19, 4).unwrap().glyph.grapheme.as_str(), "+");
    // Overlay text present, offset by its container padding.
    assert_eq!(s.get(3, 0).unwrap().glyph.grapheme.as_str(), "m");
    assert_eq!(s.get(0, 1).unwrap().glyph.grapheme.as_str(), "|");
}

#[test]
fn dim_node_marks_region_style_only_in_a_layer() {
    let mut base = Surface::new(4, 1);
    base.print_str(0, 0, "text", Style::default(), None);

    let mut layer = Surface::new_transparent(4, 1);
    layer.apply_dim_rect(gibson::surface::Rect::new(0, 0, 4, 1));
    base.blit_transparent(&layer);

    for x in 0..4 {
        assert!(
            base.get(x, 0).unwrap().style.dim,
            "cell {x} should be dimmed"
        );
    }
    assert_eq!(base.get(0, 0).unwrap().glyph.grapheme.as_str(), "t");
}

#[test]
fn node_kind_stack_is_public_and_matchable() {
    let n = Node::stack();
    assert!(matches!(n.kind, NodeKind::Stack));
    let d = Node::dim();
    assert!(matches!(d.kind, NodeKind::Dim));
}
