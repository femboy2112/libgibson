//! Positioned layers, camera viewports, raster embedding, and their damage
//! behaviour through the whole renderer.

use gibson::cell::{Cell, Glyph, Style};
use gibson::node::Node;
use gibson::renderer::Renderer;
use gibson::session::TerminalSession;
use gibson::surface::Surface;
use gibson::{paint, RenderMode};

fn layout_paint(root: &mut Node, w: u16, h: u16) -> Surface {
    gibson::layout::compute_layout(root, w, h).unwrap();
    let mut s = Surface::new(w, h);
    paint(root, &mut s);
    s
}

fn assert_no_dangling_wide(s: &Surface) {
    for y in 0..s.height {
        for x in 0..s.width {
            let c = s.get(x, y).unwrap();
            if c.is_continuation {
                let prev_ok = x > 0
                    && s.get(x - 1, y)
                        .map(|p| p.glyph.display_width == 2 && !p.is_continuation)
                        .unwrap_or(false);
                assert!(
                    !c.is_continuation || prev_ok,
                    "dangling continuation at {x},{y}"
                );
            }
            if c.glyph.display_width == 2 && !c.is_continuation {
                let next_ok = s.get(x + 1, y).map(|n| n.is_continuation).unwrap_or(false);
                assert!(next_ok, "wide lead without continuation at {x},{y}");
            }
        }
    }
}

#[test]
fn positioned_layer_sits_at_offset_without_reflowing_base() {
    let mut root = Node::stack()
        .width(20.0)
        .height(6.0)
        .child(Node::text("BASE", Style::default()))
        .child(
            Node::text("SPRITE", Style::default())
                .width(6.0)
                .height(1.0)
                .offset(10.0, 2.0),
        );
    let s = layout_paint(&mut root, 20, 6);
    assert_eq!(s.get(10, 2).unwrap().glyph.grapheme.as_str(), "S");
    assert_eq!(s.get(11, 2).unwrap().glyph.grapheme.as_str(), "P");
    // Base is untouched elsewhere, and just past the sprite is blank.
    assert_eq!(s.get(0, 0).unwrap().glyph.grapheme.as_str(), "B");
    assert_eq!(s.get(16, 2).unwrap().glyph.grapheme.as_str(), " ");
}

#[test]
fn negative_offset_is_clipped_at_the_left_edge() {
    let mut root = Node::stack().width(10.0).height(2.0).child(
        Node::text("ABCDE", Style::default())
            .width(5.0)
            .height(1.0)
            .offset(-3.0, 0.0),
    );
    let s = layout_paint(&mut root, 10, 2);
    // Origin at -3: visible columns 0..=1 are 'D','E'.
    assert_eq!(s.get(0, 0).unwrap().glyph.grapheme.as_str(), "D");
    assert_eq!(s.get(1, 0).unwrap().glyph.grapheme.as_str(), "E");
    assert_eq!(s.get(2, 0).unwrap().glyph.grapheme.as_str(), " ");
}

#[test]
fn fully_offscreen_layer_paints_nothing() {
    let mut root = Node::stack()
        .width(10.0)
        .height(2.0)
        .child(Node::text("BASE", Style::default()))
        .child(
            Node::text("GHOST", Style::default())
                .width(5.0)
                .height(1.0)
                .offset(-20.0, 0.0),
        );
    let s = layout_paint(&mut root, 10, 2);
    assert_eq!(s.get(0, 0).unwrap().glyph.grapheme.as_str(), "B");
    let rendered: String = (0..10)
        .map(|x| s.get(x, 0).unwrap().glyph.grapheme.to_string())
        .collect();
    assert!(
        !rendered.contains('G'),
        "offscreen sprite leaked: {rendered:?}"
    );
}

#[test]
fn later_children_win_z_order() {
    let mut root = Node::stack()
        .width(8.0)
        .height(1.0)
        .child(
            Node::text("AAAA", Style::default())
                .width(4.0)
                .height(1.0)
                .offset(1.0, 0.0),
        )
        .child(
            Node::text("BBBB", Style::default())
                .width(4.0)
                .height(1.0)
                .offset(1.0, 0.0),
        );
    let s = layout_paint(&mut root, 8, 1);
    assert_eq!(s.get(1, 0).unwrap().glyph.grapheme.as_str(), "B");
}

#[test]
fn raster_node_composites_shared_surface() {
    let mut raster = Surface::new_transparent(3, 1);
    raster.print_str(0, 0, "XYZ", Style::default(), None);
    let mut root = Node::stack()
        .width(10.0)
        .height(2.0)
        .child(Node::text(".........", Style::default()))
        .child(Node::raster(raster).width(3.0).height(1.0).offset(4.0, 1.0));
    let s = layout_paint(&mut root, 10, 2);
    assert_eq!(s.get(4, 1).unwrap().glyph.grapheme.as_str(), "X");
    assert_eq!(s.get(6, 1).unwrap().glyph.grapheme.as_str(), "Z");
    assert_eq!(s.get(0, 0).unwrap().glyph.grapheme.as_str(), ".");
}

#[test]
fn raster_larger_than_viewport_is_clipped() {
    let mut raster = Surface::new_transparent(20, 3);
    for x in 0..20u16 {
        raster.set_cell(x, 0, Cell::new(Glyph::new("#"), Style::default()));
    }
    let mut root = Node::stack().width(6.0).height(2.0).child(
        Node::raster(raster)
            .width(20.0)
            .height(3.0)
            .offset(2.0, 0.0),
    );
    let s = layout_paint(&mut root, 6, 2);
    // Only columns 2..=5 are visible.
    assert_eq!(s.get(0, 0).unwrap().glyph.grapheme.as_str(), " ");
    assert_eq!(s.get(2, 0).unwrap().glyph.grapheme.as_str(), "#");
    assert_eq!(s.get(5, 0).unwrap().glyph.grapheme.as_str(), "#");
}

#[test]
fn wide_glyph_at_layer_boundary_never_dangles() {
    let mut root = Node::stack().width(4.0).height(1.0).child(
        Node::text("你", Style::default())
            .width(2.0)
            .height(1.0)
            .offset(3.0, 0.0),
    );
    let s = layout_paint(&mut root, 4, 1);
    assert_no_dangling_wide(&s);
}

#[test]
fn positioned_layer_wide_glyph_left_clipped_leaves_no_half_glyph() {
    // Negative origin forces the scratch + clipped-blit path. The wide glyph's
    // lead scrolls off the left edge, so the whole glyph must be suppressed and
    // the base beneath must survive untouched.
    let mut root = Node::stack()
        .width(6.0)
        .height(1.0)
        .child(Node::text("abcdef", Style::default()))
        .child(
            Node::text("你AB", Style::default())
                .width(3.0)
                .height(1.0)
                .offset(-1.0, 0.0),
        );
    let s = layout_paint(&mut root, 6, 1);
    assert_no_dangling_wide(&s);
    // Column 0 was the continuation of 你 in the layer; it must not appear.
    assert_eq!(s.get(0, 0).unwrap().glyph.grapheme.as_str(), "a");
    assert!(!s.get(0, 0).unwrap().is_continuation);
    // 'A' from the layer lands on column 1.
    assert_eq!(s.get(1, 0).unwrap().glyph.grapheme.as_str(), "A");
}

#[test]
fn raster_node_wide_glyph_cannot_leak_past_its_rectangle() {
    // Raster surface is 6 wide but the node is only 5 wide. A wide glyph whose
    // lead lands on the node's final column (4) must be suppressed so its
    // continuation (column 5) never escapes the node rectangle.
    let mut raster = Surface::new_transparent(6, 1);
    raster.print_str(4, 0, "你", Style::default(), None);
    let mut root = Node::stack()
        .width(8.0)
        .height(1.0)
        .child(Node::text("abcdefgh", Style::default()))
        .child(Node::raster(raster).width(5.0).height(1.0).offset(0.0, 0.0));
    let s = layout_paint(&mut root, 8, 1);
    assert_no_dangling_wide(&s);
    assert_eq!(s.get(4, 0).unwrap().glyph.grapheme.as_str(), "e");
    assert_eq!(s.get(5, 0).unwrap().glyph.grapheme.as_str(), "f");
    assert!(!s.get(5, 0).unwrap().is_continuation);
}

#[test]
fn viewport_wide_glyph_scrolled_off_left_is_suppressed() {
    // Camera pans one column right; the world's leading wide glyph scrolls off
    // the left. It must vanish whole, never leaving a dangling continuation.
    let mut root = Node::stack()
        .width(6.0)
        .height(1.0)
        .child(Node::text("abcdef", Style::default()))
        .child(
            Node::viewport(1, 0)
                .width(4.0)
                .height(1.0)
                .child(Node::text("你XY", Style::default()).width(4.0).height(1.0)),
        );
    let s = layout_paint(&mut root, 6, 1);
    assert_no_dangling_wide(&s);
    // Visible world after panning one column: X at viewport col 1.
    assert_eq!(s.get(1, 0).unwrap().glyph.grapheme.as_str(), "X");
    assert_eq!(s.get(2, 0).unwrap().glyph.grapheme.as_str(), "Y");
    // Base content outside the viewport must be unchanged.
    assert_eq!(s.get(4, 0).unwrap().glyph.grapheme.as_str(), "e");
}

#[test]
fn viewport_pans_horizontally_and_clips() {
    let world = Node::text("0123456789ABCDEFGHIJ", Style::default())
        .width(20.0)
        .height(1.0);
    let mut root = Node::viewport(5, 0).width(10.0).height(1.0).child(world);
    let s = layout_paint(&mut root, 10, 1);
    assert_eq!(s.get(0, 0).unwrap().glyph.grapheme.as_str(), "5");
    assert_eq!(s.get(9, 0).unwrap().glyph.grapheme.as_str(), "E");
}

#[test]
fn viewport_pans_vertically() {
    let mut world = Node::col().width(6.0).height(4.0);
    for i in 0..4 {
        world = world.child(
            Node::text(format!("L{i}xxx"), Style::default())
                .width(6.0)
                .height(1.0),
        );
    }
    let mut root = Node::viewport(0, 2).width(6.0).height(2.0).child(world);
    let s = layout_paint(&mut root, 6, 2);
    assert_eq!(s.get(0, 0).unwrap().glyph.grapheme.as_str(), "L");
    assert_eq!(s.get(1, 0).unwrap().glyph.grapheme.as_str(), "2");
    assert_eq!(s.get(1, 1).unwrap().glyph.grapheme.as_str(), "3");
}

#[test]
fn viewport_hides_world_outside_its_rectangle() {
    let world = Node::text("0123456789", Style::default())
        .width(10.0)
        .height(1.0);
    // Viewport only 4 wide, world 10 wide: columns beyond 3 must not leak.
    let mut root = Node::stack()
        .width(10.0)
        .height(1.0)
        .child(Node::text("..........", Style::default()))
        .child(
            Node::viewport(0, 0)
                .width(4.0)
                .height(1.0)
                .child(world)
                .offset(0.0, 0.0),
        );
    let s = layout_paint(&mut root, 10, 1);
    assert_eq!(s.get(0, 0).unwrap().glyph.grapheme.as_str(), "0");
    assert_eq!(s.get(3, 0).unwrap().glyph.grapheme.as_str(), "3");
    assert_eq!(
        s.get(4, 0).unwrap().glyph.grapheme.as_str(),
        ".",
        "world leaked past viewport"
    );
}

#[test]
fn viewport_inside_stack_is_positioned_and_clipped() {
    let world = Node::text("ABCDEFGH", Style::default())
        .width(8.0)
        .height(1.0);
    let mut root = Node::stack()
        .width(20.0)
        .height(3.0)
        .child(Node::text("BASE", Style::default()))
        .child(
            Node::viewport(3, 0)
                .width(4.0)
                .height(1.0)
                .child(world)
                .offset(10.0, 1.0),
        );
    let s = layout_paint(&mut root, 20, 3);
    assert_eq!(s.get(10, 1).unwrap().glyph.grapheme.as_str(), "D");
    assert_eq!(s.get(13, 1).unwrap().glyph.grapheme.as_str(), "G");
    assert_eq!(s.get(9, 1).unwrap().glyph.grapheme.as_str(), " ");
}

// ---------------------------------------------------------------------------
// Damage behaviour through the whole renderer
// ---------------------------------------------------------------------------

struct Fs {
    renderer: Renderer,
    session: TerminalSession,
    parser: vt100::Parser,
}

impl Fs {
    fn new(cols: u16, rows: u16) -> Self {
        Self {
            renderer: Renderer::new(RenderMode::Fullscreen),
            session: TerminalSession::headless(cols, rows),
            parser: vt100::Parser::new(rows, cols, 0),
        }
    }

    fn render(&mut self, root: &mut Node) -> (usize, usize, usize) {
        let mut out = Vec::new();
        let (dirty, total, bytes, _, _) = self
            .renderer
            .render(root, &mut self.session, &mut out)
            .unwrap();
        self.parser.process(&out);
        (dirty, total, bytes)
    }

    fn cell(&self, x: u16, y: u16) -> String {
        self.parser.screen().cell(y, x).unwrap().contents()
    }
}

fn sprite_at(x: f32) -> Node {
    Node::stack()
        .percent_width(100.0)
        .percent_height(100.0)
        .child(Node::text("BASE....", Style::default()))
        .child(
            Node::text("X", Style::default())
                .width(1.0)
                .height(1.0)
                .offset(x, 5.0),
        )
}

#[test]
fn moving_one_cell_sprite_has_bounded_damage_and_no_ghost() {
    let mut fs = Fs::new(40, 12);
    let (_, total, _) = fs.render(&mut sprite_at(10.0));
    assert_eq!(fs.cell(10, 5), "X");

    let (dirty, total2, bytes) = fs.render(&mut sprite_at(11.0));
    assert_eq!(fs.cell(10, 5), " ", "old sprite location must clear");
    assert_eq!(fs.cell(11, 5), "X");
    assert!(bytes > 0);
    assert!(
        dirty * 4 <= total2,
        "1-cell sprite move dirtied too much: {dirty}/{total2} (first full frame total {total})"
    );

    // Re-rendering the identical frame must emit nothing.
    let (dirty_same, _, bytes_same) = fs.render(&mut sprite_at(11.0));
    assert_eq!(dirty_same, 0, "identical frame must be clean");
    assert_eq!(bytes_same, 0, "identical frame must emit zero bytes");
}

#[test]
fn viewport_pan_damage_is_viewport_local() {
    let world = |off: i32| {
        Node::stack()
            .percent_width(100.0)
            .percent_height(100.0)
            .child(Node::text("SIDEBAR-CONTENT", Style::default()))
            .child(
                Node::viewport(off, 0)
                    .width(10.0)
                    .height(2.0)
                    .child(
                        Node::text("0123456789ABCDEFGHIJKLMNOP", Style::default())
                            .width(26.0)
                            .height(2.0),
                    )
                    .offset(20.0, 4.0),
            )
    };
    let mut fs = Fs::new(40, 12);
    fs.render(&mut world(0));
    let (dirty, total, _) = fs.render(&mut world(1));
    assert!(
        dirty * 4 <= total,
        "viewport pan dirtied too much: {dirty}/{total}"
    );
    assert_eq!(fs.cell(20, 4), "1");
}
