use crate::cell::{Cell, Glyph, Style, TextAlign};
use crate::layout::{wrap_rich_text, wrap_text};
use crate::node::{Node, NodeKind};
use crate::surface::{Rect, Surface};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Context maintained during painting traversal.
#[derive(Debug, Default)]
pub struct PaintContext {
    /// If a widget requested focused cursor visibility, its (x, y) coordinates.
    pub cursor_position: Option<(u16, u16)>,

    pub text_scroll_offset: Option<usize>,
}

/// Paints a laid-out UI node tree onto the target surface.
pub fn paint(root: &Node, surface: &mut Surface) -> PaintContext {
    let mut ctx = PaintContext::default();
    paint_node(root, surface, surface.area(), 0, 0, &mut ctx);
    ctx
}

/// Intersects a possibly-negative, offset rectangle with `clip`.
///
/// Positioned layers may sit partly or fully off-screen (including negative
/// origins); this computes the visible sub-rectangle.
fn intersect_signed(x: i32, y: i32, w: u16, h: u16, clip: Rect) -> Rect {
    if w == 0 || h == 0 {
        return Rect::new(0, 0, 0, 0);
    }
    let x1 = x.saturating_add(w as i32);
    let y1 = y.saturating_add(h as i32);
    let cx1 = clip.x as i32 + clip.width as i32;
    let cy1 = clip.y as i32 + clip.height as i32;
    let ix0 = x.max(clip.x as i32);
    let iy0 = y.max(clip.y as i32);
    let ix1 = x1.min(cx1);
    let iy1 = y1.min(cy1);
    if ix1 <= ix0 || iy1 <= iy0 {
        Rect::new(0, 0, 0, 0)
    } else {
        Rect::new(
            ix0 as u16,
            iy0 as u16,
            (ix1 - ix0) as u16,
            (iy1 - iy0) as u16,
        )
    }
}

/// Accumulated signed offset for a child. Absolutely-positioned children add
/// their own offset; others inherit the parent offset unchanged.
fn child_offset(child: &Node, ox: i32, oy: i32) -> (i32, i32) {
    if child.layout_style.absolute {
        (
            ox + child.layout_style.offset_x.round() as i32,
            oy + child.layout_style.offset_y.round() as i32,
        )
    } else {
        (ox, oy)
    }
}

fn paint_node(
    node: &Node,
    surface: &mut Surface,
    clip: Rect,
    ox: i32,
    oy: i32,
    ctx: &mut PaintContext,
) {
    if node.surface_fx.is_empty() {
        paint_node_contents(node, surface, clip, ox, oy, ctx);
        return;
    }
    let cr = node.computed_rect;
    let x = (cr.x as i32).saturating_add(ox);
    let y = (cr.y as i32).saturating_add(oy);
    let visible = intersect_signed(x, y, cr.width, cr.height, clip);
    if visible.is_empty() {
        return;
    }
    // Realize before destination clipping, preserving natural layout and stable
    // entity-local masks. Only this entity's extent needs the extra allocation.
    let mut local = Surface::new_transparent(cr.width, cr.height);
    let mut local_ctx = PaintContext::default();
    let area = local.area();
    paint_node_contents(
        node,
        &mut local,
        area,
        -(cr.x as i32),
        -(cr.y as i32),
        &mut local_ctx,
    );
    crate::surface_fx::SurfaceFx::apply_chain(&node.surface_fx, &mut local);
    surface.blit_transparent_clipped(&local, x, y, visible);
    // Effects may displace/erase an input insertion point. Do not publish its
    // stale hardware cursor: retain the previous unaffected focused cursor.
}

fn paint_node_contents(
    node: &Node,
    surface: &mut Surface,
    clip: Rect,
    ox: i32,
    oy: i32,
    ctx: &mut PaintContext,
) {
    let cr = node.computed_rect;
    let origin_x = cr.x as i32 + ox;
    let origin_y = cr.y as i32 + oy;
    let rect = intersect_signed(origin_x, origin_y, cr.width, cr.height, clip);
    if rect.is_empty() {
        return;
    }

    // Paint at natural extent before clipping on ANY edge. Leading-edge clips
    // otherwise lose the origin; trailing-edge clips rewrap text and move the
    // border inward. Identity post-processing must produce the same geometry.
    if origin_x < clip.x as i32
        || origin_y < clip.y as i32
        || (!matches!(
            node.kind,
            NodeKind::TextInput { .. }
                | NodeKind::Box { border: None, .. }
                | NodeKind::Stack
                | NodeKind::Viewport { .. }
        ) && (rect.width < cr.width || rect.height < cr.height))
    {
        let tx = ox - origin_x; // maps this node's origin to 0
        let ty = oy - origin_y;
        // The scratch must fit the node's natural extent (it may be larger than
        // the target surface, e.g. a viewport world wider than the camera).
        let sw = surface.width.max(cr.width);
        let sh = surface.height.max(cr.height);
        let mut scratch = Surface::new_transparent(sw, sh);
        let prev_cursor = ctx.cursor_position.take();
        let scratch_area = scratch.area();
        paint_node(node, &mut scratch, scratch_area, tx, ty, ctx);
        surface.blit_transparent_clipped(&scratch, origin_x, origin_y, rect);
        ctx.cursor_position = match ctx.cursor_position.take() {
            Some((cx, cy)) => {
                let mx = cx as i32 + origin_x;
                let my = cy as i32 + origin_y;
                if mx >= rect.x as i32
                    && my >= rect.y as i32
                    && mx < (rect.x + rect.width) as i32
                    && my < (rect.y + rect.height) as i32
                {
                    Some((mx as u16, my as u16))
                } else {
                    prev_cursor
                }
            }
            None => prev_cursor,
        };
        return;
    }

    match &node.kind {
        NodeKind::Box {
            border,
            border_style,
            background,
        } => {
            if let Some(bg) = background {
                let cell = Cell::space(Style::new().bg(*bg));
                surface.fill_rect(rect, cell);
            }
            if let Some(bt) = border {
                surface.draw_border(rect, *bt, *border_style);
            }
        }
        NodeKind::Text { text, style, wrap } => {
            let lines = wrap_text(text, *wrap, rect.width);
            for (i, line) in lines.iter().enumerate() {
                let line_y = rect.y + (i as u16);
                if line_y >= rect.y + rect.height {
                    break;
                }
                surface.print_str(rect.x, line_y, line, *style, Some(rect.width));
            }
        }
        NodeKind::RichText { text, wrap } => {
            let lines = wrap_rich_text(text, *wrap, rect.width);
            for (i, line) in lines.iter().enumerate() {
                let line_y = rect.y + (i as u16);
                if line_y >= rect.y + rect.height {
                    break;
                }
                let line_w = line.display_width() as u16;
                let offset_x = match line.align {
                    TextAlign::Left => 0,
                    TextAlign::Center => (rect.width.saturating_sub(line_w)) / 2,
                    TextAlign::Right => rect.width.saturating_sub(line_w),
                };
                let mut cur_x = rect.x + offset_x;
                let max_x = rect.x + rect.width;
                for span in &line.spans {
                    if cur_x >= max_x {
                        break;
                    }
                    let written = surface.print_str(
                        cur_x,
                        line_y,
                        span.text.as_str(),
                        span.style,
                        Some(max_x.saturating_sub(cur_x)),
                    );
                    cur_x += written;
                }
            }
        }
        NodeKind::Rule {
            title,
            style,
            title_style,
        } => {
            if rect.height > 0 && rect.width > 0 {
                let y = rect.y;
                if let Some(t) = title {
                    let title_w = UnicodeWidthStr::width(t.as_str()) as u16;
                    if rect.width >= title_w + 6 {
                        for x in rect.x..(rect.x + 3) {
                            surface.set_cell(x, y, Cell::new(Glyph::new("─"), *style));
                        }
                        surface.print_str(rect.x + 3, y, " ", *style, None);
                        surface.print_str(rect.x + 4, y, t.as_str(), *title_style, None);
                        surface.print_str(rect.x + 4 + title_w, y, " ", *style, None);
                        for x in (rect.x + 5 + title_w)..(rect.x + rect.width) {
                            surface.set_cell(x, y, Cell::new(Glyph::new("─"), *style));
                        }
                    } else {
                        for x in rect.x..(rect.x + rect.width) {
                            surface.set_cell(x, y, Cell::new(Glyph::new("─"), *style));
                        }
                    }
                } else {
                    for x in rect.x..(rect.x + rect.width) {
                        surface.set_cell(x, y, Cell::new(Glyph::new("─"), *style));
                    }
                }
            }
        }
        NodeKind::Rail { style } => {
            for y in rect.y..(rect.y + rect.height) {
                surface.set_cell(rect.x, y, Cell::new(Glyph::new("│"), *style));
            }
        }
        NodeKind::Border {
            border_type,
            style,
            title,
            title_style,
            background,
        } => {
            if let Some(bg) = background {
                surface.fill_rect(rect, Cell::space(Style::new().bg(*bg)));
            }
            surface.draw_border(rect, *border_type, *style);
            if let Some(t) = title {
                if rect.width > 4 {
                    let title_text = format!(" {} ", t);
                    surface.print_str(
                        rect.x + 2,
                        rect.y,
                        &title_text,
                        *title_style,
                        Some(rect.width.saturating_sub(4)),
                    );
                }
            }
        }
        NodeKind::Spinner {
            frames,
            frame_index,
            style,
            label,
            label_style,
        } => {
            let frame = if !frames.is_empty() {
                &frames[*frame_index % frames.len()]
            } else {
                " "
            };
            let fw = surface.print_str(rect.x, rect.y, frame, *style, Some(rect.width));
            if let Some(lbl) = label {
                if rect.width > fw + 1 {
                    surface.print_str(
                        rect.x + fw + 1,
                        rect.y,
                        lbl,
                        *label_style,
                        Some(rect.width.saturating_sub(fw + 1)),
                    );
                }
            }
        }
        NodeKind::TextInput {
            value,
            cursor_grapheme,
            placeholder,
            style,
            placeholder_style,
            cursor_style,
            scroll_offset,
        } => {
            paint_text_input(
                rect,
                value,
                *cursor_grapheme,
                placeholder.as_deref(),
                *style,
                *placeholder_style,
                *cursor_style,
                *scroll_offset,
                surface,
                ctx,
            );
        }
        NodeKind::Dim => {
            surface.apply_dim_rect(rect);
        }
        NodeKind::Stack => {}
        NodeKind::Viewport { .. } => {}
        NodeKind::Raster { surface: raster } => {
            // Draw the raster aligned to the node origin, clipped to the visible
            // rectangle (supports partly/fully off-screen placement).
            surface.blit_transparent_clipped(raster.as_ref(), origin_x, origin_y, rect);
        }
    }

    // Bordered containers clip children to the *inside* of the border so content
    // can never overwrite the frame, regardless of how the flex layout sizes the
    // panel.
    let child_clip = match &node.kind {
        NodeKind::Box {
            border: Some(_), ..
        } => rect.shrink(1),
        NodeKind::Border { .. } => rect.shrink(1),
        _ => rect,
    };

    if matches!(node.kind, NodeKind::Stack) {
        // Composite children in z-order through transparent scratch layers.
        // A child only covers the cells it actually paints.
        for child in &node.children {
            let (cx, cy) = child_offset(child, ox, oy);
            let mut layer = Surface::new_transparent(surface.width, surface.height);
            paint_node(child, &mut layer, child_clip, cx, cy, ctx);
            surface.blit_transparent(&layer);
        }
        return;
    }

    // A camera viewport translates its world by the negative camera offset.
    let (cam_x, cam_y) = match &node.kind {
        NodeKind::Viewport { offset_x, offset_y } => (-*offset_x, -*offset_y),
        _ => (0, 0),
    };

    for child in &node.children {
        let (cx, cy) = if cam_x != 0 || cam_y != 0 {
            (ox + cam_x, oy + cam_y)
        } else {
            child_offset(child, ox, oy)
        };
        paint_node(child, surface, child_clip, cx, cy, ctx);
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_text_input(
    rect: Rect,
    value: &str,
    cursor_grapheme: usize,
    placeholder: Option<&str>,
    style: Style,
    placeholder_style: Style,
    cursor_style: Style,
    scroll_offset: usize,
    surface: &mut Surface,
    ctx: &mut PaintContext,
) {
    if rect.height == 0 || rect.width == 0 {
        return;
    }

    let y = rect.y;
    let graphemes: Vec<&str> = value.graphemes(true).collect();
    let total_graphemes = graphemes.len();

    // Cursor policy (single source of truth):
    //
    // * The hardware cursor always marks the insertion point via
    //   `ctx.cursor_position`.
    // * The software cursor is only ever a *highlight applied to the glyph that
    //   already occupies the cursor cell*. It never replaces a glyph with a
    //   blank, so the placeholder stays fully intact.
    // * When there is no glyph at the insertion point (empty buffer, cursor past
    //   the last grapheme) nothing is fabricated; the hardware cursor alone marks
    //   the point. There is no second reverse-video "cursor block" to conflict.
    if total_graphemes == 0 {
        if let Some(ph) = placeholder {
            surface.print_str(rect.x, y, ph, placeholder_style, Some(rect.width));
            // Highlight the first placeholder grapheme in place; never blank it.
            highlight_cursor(&mut *surface, rect.x, y, cursor_style);
            ctx.cursor_position = Some((rect.x, y));
            return;
        } else {
            ctx.cursor_position = Some((rect.x, y));
            return;
        }
    }

    // Precompute display widths and cumulative column positions for each grapheme
    let mut col_positions = Vec::with_capacity(total_graphemes + 1);
    let mut widths = Vec::with_capacity(total_graphemes);
    let mut acc_col: usize = 0;
    for &g in &graphemes {
        col_positions.push(acc_col);
        let w = UnicodeWidthStr::width(g).max(1);
        widths.push(w);
        acc_col += w;
    }
    col_positions.push(acc_col); // for cursor at end

    let cursor_idx = cursor_grapheme.min(total_graphemes);
    let cursor_col = col_positions[cursor_idx];
    let visible_cols = rect.width as usize;

    let mut scroll_col = scroll_offset;
    if cursor_col < scroll_col {
        scroll_col = cursor_col;
    } else if cursor_col >= scroll_col + visible_cols {
        scroll_col = cursor_col.saturating_sub(visible_cols) + 1;
    }
    ctx.text_scroll_offset = Some(scroll_col);

    let max_x = rect.x + rect.width;

    for (idx, &g) in graphemes.iter().enumerate() {
        let g_col = col_positions[idx];
        let g_w = widths[idx];

        // Check if fully left of viewport
        if g_col + g_w <= scroll_col {
            continue;
        }
        // Check if at or beyond right of viewport
        if g_col >= scroll_col + visible_cols {
            break;
        }

        if g_col < scroll_col {
            let cell_x = rect.x;
            surface.set_cell(cell_x, y, Cell::space(style));
            continue;
        }

        let cell_x = rect.x + (g_col - scroll_col) as u16;
        if cell_x + (g_w as u16) > max_x {
            break;
        }

        let is_cursor = idx == cursor_idx;
        let cell_style = if is_cursor {
            ctx.cursor_position = Some((cell_x, y));
            style.overlay(cursor_style)
        } else {
            style
        };

        let glyph = Glyph::new(g);
        surface.set_cell(cell_x, y, Cell::new(glyph, cell_style));
    }

    // Cursor at the end of the input (after the last grapheme): the hardware
    // cursor marks the insertion point and no glyph is fabricated.
    if cursor_idx >= total_graphemes
        && cursor_col >= scroll_col
        && cursor_col < scroll_col + visible_cols
    {
        let end_cursor_x = rect.x + (cursor_col - scroll_col) as u16;
        if end_cursor_x < max_x {
            ctx.cursor_position = Some((end_cursor_x, y));
        }
    }
}

/// Applies `cursor` as an overlay style to the glyph already at `(x, y)`.
///
/// Used by the text input's software cursor: it *decorates* an existing glyph
/// (and its wide-glyph continuation) rather than replacing anything, so a
/// placeholder can never lose its first character to the cursor.
fn highlight_cursor(surface: &mut Surface, x: u16, y: u16, cursor: Style) {
    let (is_wide_lead, transparent) = match surface.get(x, y) {
        Some(c) => (
            c.glyph.display_width == 2 && !c.is_continuation,
            c.transparent,
        ),
        None => return,
    };
    if transparent {
        return;
    }
    if let Some(c) = surface.get_mut(x, y) {
        c.style = c.style.overlay(cursor);
    }
    if is_wide_lead {
        if let Some(cont) = surface.get_mut(x.saturating_add(1), y) {
            if cont.is_continuation {
                cont.style = cont.style.overlay(cursor);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::compute_layout;
    use crate::surface::BorderType;

    #[test]
    fn test_paint_box_with_border_and_text() {
        let mut root = Node::border_box(BorderType::Rounded, Style::default())
            .width(10.0)
            .height(3.0)
            .child(Node::text("Hi", Style::default()));

        compute_layout(&mut root, 80, 24).unwrap();

        let mut surface = Surface::new(15, 5);
        paint(&root, &mut surface);

        // Check rounded corners
        assert_eq!(surface.get(0, 0).unwrap().glyph.grapheme.as_str(), "╭");
        assert_eq!(surface.get(9, 0).unwrap().glyph.grapheme.as_str(), "╮");
        assert_eq!(surface.get(0, 2).unwrap().glyph.grapheme.as_str(), "╰");
        assert_eq!(surface.get(9, 2).unwrap().glyph.grapheme.as_str(), "╯");

        // Check child text
        assert_eq!(surface.get(1, 1).unwrap().glyph.grapheme.as_str(), "H");
        assert_eq!(surface.get(2, 1).unwrap().glyph.grapheme.as_str(), "i");
    }

    #[test]
    fn test_paint_spinner() {
        let mut spinner = Node::spinner(2, Style::default(), Some("Loading..."))
            .width(20.0)
            .height(1.0);

        compute_layout(&mut spinner, 80, 24).unwrap();

        let mut surface = Surface::new(20, 1);
        paint(&spinner, &mut surface);

        // Frame index 2 of braille is "⠹"
        assert_eq!(surface.get(0, 0).unwrap().glyph.grapheme.as_str(), "⠹");
        // Space at 1
        assert_eq!(surface.get(1, 0).unwrap().glyph.grapheme.as_str(), " ");
        // "L" at 2
        assert_eq!(surface.get(2, 0).unwrap().glyph.grapheme.as_str(), "L");
    }

    #[test]
    fn test_paint_rule() {
        let mut rule = Node::rule(Some("Section"), Style::default()).width(20.0);
        compute_layout(&mut rule, 20, 1).unwrap();

        let mut surface = Surface::new(20, 1);
        paint(&rule, &mut surface);

        // First 3 should be ─
        assert_eq!(surface.get(0, 0).unwrap().glyph.grapheme.as_str(), "─");
        assert_eq!(surface.get(1, 0).unwrap().glyph.grapheme.as_str(), "─");
        assert_eq!(surface.get(2, 0).unwrap().glyph.grapheme.as_str(), "─");
        // Space at 3
        assert_eq!(surface.get(3, 0).unwrap().glyph.grapheme.as_str(), " ");
        // Title starts at 4
        assert_eq!(surface.get(4, 0).unwrap().glyph.grapheme.as_str(), "S");
    }

    #[test]
    fn test_paint_rail() {
        let mut rail = Node::rail(Style::default())
            .width(20.0)
            .height(2.0)
            .child(Node::text("Content", Style::default()));
        compute_layout(&mut rail, 20, 2).unwrap();

        let mut surface = Surface::new(20, 2);
        paint(&rail, &mut surface);

        // Rail vertical line at x=0
        assert_eq!(surface.get(0, 0).unwrap().glyph.grapheme.as_str(), "│");
        assert_eq!(surface.get(0, 1).unwrap().glyph.grapheme.as_str(), "│");
        // Padding at x=1
        assert_eq!(surface.get(1, 0).unwrap().glyph.grapheme.as_str(), " ");
        // Content starts at x=2
        assert_eq!(surface.get(2, 0).unwrap().glyph.grapheme.as_str(), "C");
    }

    #[test]
    fn test_paint_text_input_cjk_emoji_scrolling() {
        use crate::cell::Color;
        let mut input = Node::text_input(
            "🦀你好世界",
            3, // cursor at index 3 (after 你好)
            None,
            Style::new().fg(Color::White),
        )
        .width(10.0)
        .height(1.0);

        compute_layout(&mut input, 10, 1).unwrap();

        let mut surface = Surface::new(10, 1);
        let ctx = paint(&input, &mut surface);

        // 🦀 is 2 cols (0,1), 你 is 2 cols (2,3), 好 is 2 cols (4,5)
        // Cursor after 3 graphemes is at col 6
        assert_eq!(ctx.cursor_position, Some((6, 0)));
        assert_eq!(surface.get(0, 0).unwrap().glyph.grapheme.as_str(), "🦀");
        assert_eq!(surface.get(2, 0).unwrap().glyph.grapheme.as_str(), "你");
        assert_eq!(surface.get(4, 0).unwrap().glyph.grapheme.as_str(), "好");
    }

    // -----------------------------------------------------------------------
    // Cursor / placeholder semantics.
    //
    // Regression: the empty-input placeholder used to be painted and then its
    // first cell overwritten by a reverse-styled space, showing "ype a…".
    // -----------------------------------------------------------------------

    fn paint_input(
        value: &str,
        cursor: usize,
        placeholder: Option<&str>,
    ) -> (Surface, PaintContext) {
        let mut node = Node::text_input(value, cursor, placeholder, Style::new())
            .width(20.0)
            .height(1.0);
        compute_layout(&mut node, 20, 1).unwrap();
        let mut surface = Surface::new(20, 1);
        let ctx = paint(&node, &mut surface);
        (surface, ctx)
    }

    fn line_text(s: &Surface) -> String {
        (0..s.width)
            .map(|x| s.get(x, 0).unwrap().glyph.grapheme.to_string())
            .collect()
    }

    #[test]
    fn placeholder_is_fully_intact_with_cursor() {
        let (surface, ctx) = paint_input("", 0, Some("type a follow-up…"));
        assert_eq!(surface.get(0, 0).unwrap().glyph.grapheme.as_str(), "t");
        assert!(
            line_text(&surface).starts_with("type a follow-up"),
            "placeholder lost a character: {:?}",
            line_text(&surface)
        );
        assert_eq!(ctx.cursor_position, Some((0, 0)));
        // The cursor decorates the existing glyph, it does not fabricate a blank.
        assert!(surface.get(0, 0).unwrap().style.reverse);
        assert!(!surface.get(1, 0).unwrap().style.reverse);
    }

    #[test]
    fn empty_input_without_placeholder_cursor_only() {
        let (surface, ctx) = paint_input("", 0, None);
        assert_eq!(ctx.cursor_position, Some((0, 0)));
        // Nothing fabricated: the cell is an ordinary space.
        assert_eq!(surface.get(0, 0).unwrap().glyph.grapheme.as_str(), " ");
        assert!(!surface.get(0, 0).unwrap().style.reverse);
    }

    #[test]
    fn cursor_at_start_highlights_first_glyph() {
        let (surface, ctx) = paint_input("hello", 0, None);
        assert_eq!(ctx.cursor_position, Some((0, 0)));
        assert_eq!(surface.get(0, 0).unwrap().glyph.grapheme.as_str(), "h");
        assert!(surface.get(0, 0).unwrap().style.reverse);
        assert_eq!(surface.get(4, 0).unwrap().glyph.grapheme.as_str(), "o");
    }

    #[test]
    fn cursor_in_the_middle_highlights_existing_glyph() {
        let (surface, ctx) = paint_input("hello", 2, None);
        assert_eq!(ctx.cursor_position, Some((2, 0)));
        assert_eq!(surface.get(2, 0).unwrap().glyph.grapheme.as_str(), "l");
        assert!(surface.get(2, 0).unwrap().style.reverse);
        assert_eq!(surface.get(1, 0).unwrap().glyph.grapheme.as_str(), "e");
    }

    #[test]
    fn cursor_at_end_does_not_fabricate_a_glyph() {
        let (surface, ctx) = paint_input("hello", 5, None);
        assert_eq!(ctx.cursor_position, Some((5, 0)));
        assert_eq!(surface.get(5, 0).unwrap().glyph.grapheme.as_str(), " ");
        assert!(!surface.get(5, 0).unwrap().style.reverse);
        assert_eq!(surface.get(4, 0).unwrap().glyph.grapheme.as_str(), "o");
    }

    #[test]
    fn wide_glyph_cursor_highlights_both_halves_without_splitting() {
        let (surface, ctx) = paint_input("你好", 1, None);
        // Cursor after 你 (2 cols) is at column 2, on 好.
        assert_eq!(ctx.cursor_position, Some((2, 0)));
        assert_eq!(surface.get(2, 0).unwrap().glyph.grapheme.as_str(), "好");
        assert!(surface.get(2, 0).unwrap().style.reverse);
        assert!(surface.get(3, 0).unwrap().is_continuation);
        assert!(surface.get(3, 0).unwrap().style.reverse);
    }

    #[test]
    fn combining_and_emoji_cursor_positions_are_cluster_aligned() {
        // e + combining acute (1 cluster, 1 col), then a crab (2 cols).
        let value = "e\u{0301}🦀";
        let (surface, ctx) = paint_input(value, 1, None);
        // Cursor after the combined cluster is at column 1, on the crab lead.
        assert_eq!(ctx.cursor_position, Some((1, 0)));
        assert_eq!(
            surface.get(0, 0).unwrap().glyph.grapheme.as_str(),
            "e\u{0301}"
        );
        assert_eq!(surface.get(1, 0).unwrap().glyph.grapheme.as_str(), "🦀");
    }

    #[test]
    fn horizontal_scroll_keeps_cursor_visible_and_placeholder_intact() {
        let long = "the quick brown fox jumps over the lazy dog";
        let (surface, ctx) = paint_input(long, long.chars().count(), None);
        let (cx, _cy) = ctx.cursor_position.expect("cursor visible");
        assert!(cx < 20, "cursor must stay inside the field, got {cx}");
        // No fabricated glyph at the cursor cell.
        assert_eq!(surface.get(cx, 0).unwrap().glyph.grapheme.as_str(), " ");
    }
}

#[cfg(test)]
mod border_clip_tests {
    use super::*;
    use crate::cell::{RichText, Style};
    use crate::layout::compute_layout;
    use crate::node::{Node, WrapMode};
    use crate::surface::{BorderType, Surface};

    #[test]
    fn panel_children_never_overwrite_the_border() {
        // Panel is only 3 rows tall but has 5 lines of content; the extra lines
        // must be clipped, not painted over the frame.
        let mut root = Node::panel("T", BorderType::Rounded, Style::default())
            .width(12.0)
            .height(3.0)
            .child(Node::rich_text_wrapped(
                RichText::raw("a\nb\nc\nd\ne"),
                WrapMode::NoWrap,
            ));
        compute_layout(&mut root, 12, 3).unwrap();
        let mut s = Surface::new(12, 3);
        paint(&root, &mut s);
        assert_eq!(s.get(0, 0).unwrap().glyph.grapheme.as_str(), "╭");
        assert_eq!(s.get(0, 2).unwrap().glyph.grapheme.as_str(), "╰");
        assert_eq!(s.get(11, 2).unwrap().glyph.grapheme.as_str(), "╯");
    }
}
