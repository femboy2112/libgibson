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
    let x1 = x + w as i32;
    let y1 = y + h as i32;
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
    let cr = node.computed_rect;
    let origin_x = cr.x as i32 + ox;
    let origin_y = cr.y as i32 + oy;
    let rect = intersect_signed(origin_x, origin_y, cr.width, cr.height, clip);
    if rect.is_empty() {
        return;
    }

    // A node clipped on its left/top edge (negative offset, camera pan, or a
    // layer crossing the boundary) cannot be drawn in place: surface primitives
    // would start at the *visible* origin and lose the off-screen part. Render
    // the subtree translated so its own origin maps to 0, then blit clipped.
    if origin_x < clip.x as i32 || origin_y < clip.y as i32 {
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

    if total_graphemes == 0 {
        if let Some(ph) = placeholder {
            surface.print_str(rect.x, y, ph, placeholder_style, Some(rect.width));

            // Cursor at first cell
            let cursor_x = rect.x;
            surface.set_cell(cursor_x, y, Cell::space(cursor_style));
            ctx.cursor_position = Some((cursor_x, y));
            return;
        } else {
            let cursor_x = rect.x;
            surface.set_cell(cursor_x, y, Cell::space(cursor_style));
            ctx.cursor_position = Some((cursor_x, y));
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
            cursor_style
        } else {
            style
        };

        let glyph = Glyph::new(g);
        surface.set_cell(cell_x, y, Cell::new(glyph, cell_style));
    }

    // If cursor is at the end of the input (after last grapheme)
    if cursor_idx >= total_graphemes
        && cursor_col >= scroll_col
        && cursor_col < scroll_col + visible_cols
    {
        let end_cursor_x = rect.x + (cursor_col - scroll_col) as u16;
        if end_cursor_x < max_x {
            surface.set_cell(end_cursor_x, y, Cell::space(cursor_style));
            ctx.cursor_position = Some((end_cursor_x, y));
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
