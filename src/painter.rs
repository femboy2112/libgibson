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
}

/// Paints a laid-out UI node tree onto the target surface.
pub fn paint(root: &Node, surface: &mut Surface) -> PaintContext {
    let mut ctx = PaintContext::default();
    paint_node(root, surface, surface.area(), &mut ctx);
    ctx
}

fn paint_node(node: &Node, surface: &mut Surface, clip: Rect, ctx: &mut PaintContext) {
    let rect = node.computed_rect.intersection(&clip);
    if rect.is_empty() {
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
        } => {
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
    }

    // Paint children
    for child in &node.children {
        paint_node(child, surface, rect, ctx);
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
