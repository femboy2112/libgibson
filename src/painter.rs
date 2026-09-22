use crate::cell::{Cell, Glyph, Style};
use crate::layout::wrap_text;
use crate::node::{Node, NodeKind};
use crate::surface::{Rect, Surface};
use unicode_segmentation::UnicodeSegmentation;

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
        }
    }

    // Determine viewport slice
    let start_idx = scroll_offset.min(total_graphemes);
    let mut current_x = rect.x;
    let max_x = rect.x + rect.width;

    for (idx, &g) in graphemes.iter().enumerate().skip(start_idx) {
        if current_x >= max_x {
            break;
        }

        let glyph = Glyph::new(g);
        let gw = glyph.display_width as u16;
        if current_x + gw > max_x {
            break;
        }

        let is_cursor = idx == cursor_grapheme;
        let cell_style = if is_cursor {
            ctx.cursor_position = Some((current_x, y));
            cursor_style
        } else {
            style
        };

        let cell = Cell::new(glyph, cell_style);
        surface.set_cell(current_x, y, cell);
        current_x += gw;
    }

    // If cursor is at the end of the input (after last grapheme)
    if cursor_grapheme >= total_graphemes && current_x < max_x {
        surface.set_cell(current_x, y, Cell::space(cursor_style));
        ctx.cursor_position = Some((current_x, y));
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
}
