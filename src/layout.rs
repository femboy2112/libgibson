use crate::cell::{Line, RichText, Span};
use crate::node::{
    AlignItems, Dimension, FlexDirection, JustifyContent, LayoutStyle, Node, NodeKind, WrapMode,
};
use crate::surface::Rect;
use taffy::geometry::Rect as TaffyRect;
use taffy::prelude::*;
use taffy::style::{
    AlignItems as TaffyAlignItems, Dimension as TaffyDimension,
    FlexDirection as TaffyFlexDirection, JustifyContent as TaffyJustifyContent, LengthPercentage,
    LengthPercentageAuto, Position,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Wraps text into lines according to the specified wrap mode and maximum width.
pub fn wrap_text(text: &str, wrap: WrapMode, max_width: u16) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }

    let mut lines = Vec::new();

    for raw_line in text.split('\n') {
        if raw_line.is_empty() {
            lines.push(String::new());
            continue;
        }

        match wrap {
            WrapMode::NoWrap => {
                lines.push(raw_line.to_string());
            }
            WrapMode::CharWrap => {
                let mut current = String::new();
                let mut current_w = 0;

                for g in raw_line.graphemes(true) {
                    let gw = UnicodeWidthStr::width(g) as u16;
                    if current_w + gw > max_width && current_w > 0 {
                        lines.push(current);
                        current = String::new();
                        current_w = 0;
                    }
                    current.push_str(g);
                    current_w += gw;
                }
                if !current.is_empty() || lines.is_empty() {
                    lines.push(current);
                }
            }
            WrapMode::WordWrap => {
                let mut current = String::new();
                let mut current_w = 0;

                let words = raw_line.split_inclusive(' ');
                for word in words {
                    let word_w = UnicodeWidthStr::width(word) as u16;
                    if current_w + word_w > max_width && current_w > 0 {
                        lines.push(current.trim_end_matches(' ').to_string());
                        current = String::new();
                        current_w = 0;
                    }

                    // If a single word is wider than max_width, break it by characters
                    if word_w > max_width {
                        for g in word.graphemes(true) {
                            let gw = UnicodeWidthStr::width(g) as u16;
                            if current_w + gw > max_width && current_w > 0 {
                                lines.push(current);
                                current = String::new();
                                current_w = 0;
                            }
                            current.push_str(g);
                            current_w += gw;
                        }
                    } else {
                        current.push_str(word);
                        current_w += word_w;
                    }
                }
                if !current.is_empty() {
                    lines.push(current.trim_end_matches(' ').to_string());
                }
            }
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

/// Wraps rich text into lines according to the specified wrap mode and maximum width,
/// preserving span styles across word wraps.
pub fn wrap_rich_text(rich: &RichText, wrap: WrapMode, max_width: u16) -> Vec<Line> {
    if rich.lines.is_empty() {
        return vec![Line::new()];
    }

    let mut result = Vec::new();

    for line in &rich.lines {
        if line.spans.is_empty() {
            result.push(Line::new().align(line.align));
            continue;
        }

        match wrap {
            WrapMode::NoWrap => {
                result.push(line.clone());
            }
            WrapMode::CharWrap => {
                let mut current = Line::new().align(line.align);
                let mut current_w: u16 = 0;

                for span in &line.spans {
                    for g in span.text.graphemes(true) {
                        let gw = UnicodeWidthStr::width(g) as u16;
                        if current_w + gw > max_width && current_w > 0 {
                            result.push(current);
                            current = Line::new().align(line.align);
                            current_w = 0;
                        }
                        current.push(Span::styled(g, span.style));
                        current_w += gw;
                    }
                }
                if !current.spans.is_empty() || result.is_empty() {
                    result.push(current);
                }
            }
            WrapMode::WordWrap => {
                let mut current = Line::new().align(line.align);
                let mut current_w: u16 = 0;

                for span in &line.spans {
                    let words = span.text.split_inclusive(' ');
                    for word in words {
                        let word_w = UnicodeWidthStr::width(word) as u16;

                        if current_w + word_w > max_width && current_w > 0 {
                            result.push(current);
                            current = Line::new().align(line.align);
                            current_w = 0;
                        }

                        if word == " " && current_w == 0 {
                            continue;
                        }

                        if word_w > max_width {
                            for g in word.graphemes(true) {
                                let gw = UnicodeWidthStr::width(g) as u16;
                                if current_w + gw > max_width && current_w > 0 {
                                    result.push(current);
                                    current = Line::new().align(line.align);
                                    current_w = 0;
                                }
                                current.push(Span::styled(g, span.style));
                                current_w += gw;
                            }
                        } else {
                            current.push(Span::styled(word, span.style));
                            current_w += word_w;
                        }
                    }
                }
                if !current.spans.is_empty() {
                    result.push(current);
                }
            }
        }
    }

    if result.is_empty() {
        result.push(Line::new());
    }

    result
}

fn convert_dimension(dim: Dimension) -> TaffyDimension {
    match dim {
        Dimension::Auto => TaffyDimension::auto(),
        Dimension::Length(l) => TaffyDimension::length(l),
        Dimension::Percent(p) => TaffyDimension::percent(p / 100.0),
    }
}

fn convert_length_percentage_auto(dim: Dimension) -> LengthPercentageAuto {
    match dim {
        Dimension::Auto => LengthPercentageAuto::auto(),
        Dimension::Length(l) => LengthPercentageAuto::length(l),
        Dimension::Percent(p) => LengthPercentageAuto::percent(p / 100.0),
    }
}

#[allow(clippy::field_reassign_with_default)]
fn convert_style(ls: &LayoutStyle) -> taffy::style::Style {
    let mut style = taffy::style::Style::default();
    style.display = Display::Flex;

    style.flex_direction = match ls.direction {
        FlexDirection::Row => TaffyFlexDirection::Row,
        FlexDirection::Column => TaffyFlexDirection::Column,
    };

    style.size.width = convert_dimension(ls.width);
    style.size.height = convert_dimension(ls.height);
    style.min_size.width = convert_length_percentage_auto(ls.min_width);
    style.min_size.height = convert_length_percentage_auto(ls.min_height);
    style.max_size.width = convert_length_percentage_auto(ls.max_width);
    style.max_size.height = convert_length_percentage_auto(ls.max_height);

    style.flex_grow = ls.flex_grow;
    style.flex_shrink = ls.flex_shrink;

    style.gap.width = LengthPercentage::length(ls.gap_col);
    style.gap.height = LengthPercentage::length(ls.gap_row);

    style.padding = TaffyRect {
        left: LengthPercentage::length(ls.padding_left),
        right: LengthPercentage::length(ls.padding_right),
        top: LengthPercentage::length(ls.padding_top),
        bottom: LengthPercentage::length(ls.padding_bottom),
    };

    if let Some(align) = ls.align_items {
        style.align_items = Some(match align {
            AlignItems::Start => TaffyAlignItems::START,
            AlignItems::End => TaffyAlignItems::END,
            AlignItems::Center => TaffyAlignItems::CENTER,
            AlignItems::Stretch => TaffyAlignItems::STRETCH,
        });
    }

    if let Some(justify) = ls.justify_content {
        style.justify_content = Some(match justify {
            JustifyContent::Start => TaffyJustifyContent::START,
            JustifyContent::End => TaffyJustifyContent::END,
            JustifyContent::Center => TaffyJustifyContent::CENTER,
            JustifyContent::SpaceBetween => TaffyJustifyContent::SPACE_BETWEEN,
            JustifyContent::SpaceAround => TaffyJustifyContent::SPACE_AROUND,
            JustifyContent::SpaceEvenly => TaffyJustifyContent::SPACE_EVENLY,
        });
    }

    style
}

/// Measures leaf intrinsic content dimensions.
fn measure_leaf(
    kind: &NodeKind,
    known_dimensions: Size<Option<f32>>,
    available_space: Size<AvailableSpace>,
) -> Size<f32> {
    if let (Some(w), Some(h)) = (known_dimensions.width, known_dimensions.height) {
        return Size {
            width: w,
            height: h,
        };
    }

    match kind {
        NodeKind::Text { text, wrap, .. } => {
            let max_w = match available_space.width {
                AvailableSpace::Definite(w) => w.max(1.0) as u16,
                _ => 1000,
            };

            let lines = wrap_text(text, *wrap, max_w);
            let mut width: f32 = 0.0;
            for line in &lines {
                let w = UnicodeWidthStr::width(&line[..]) as f32;
                if w > width {
                    width = w;
                }
            }
            let height = lines.len() as f32;

            Size {
                width: known_dimensions.width.unwrap_or(width),
                height: known_dimensions.height.unwrap_or(height),
            }
        }
        NodeKind::RichText { text, wrap } => {
            let max_w = match available_space.width {
                AvailableSpace::Definite(w) => w.max(1.0) as u16,
                _ => 1000,
            };

            let lines = wrap_rich_text(text, *wrap, max_w);
            let mut width: f32 = 0.0;
            for line in &lines {
                let w = line.display_width() as f32;
                if w > width {
                    width = w;
                }
            }
            let height = lines.len() as f32;

            Size {
                width: known_dimensions.width.unwrap_or(width),
                height: known_dimensions.height.unwrap_or(height),
            }
        }
        NodeKind::Rule { title, .. } => {
            let title_w = title
                .as_ref()
                .map(|t| UnicodeWidthStr::width(t.as_str()) as f32 + 6.0)
                .unwrap_or(4.0);
            Size {
                width: known_dimensions.width.unwrap_or(title_w),
                height: known_dimensions.height.unwrap_or(1.0),
            }
        }
        NodeKind::Rail { .. } => Size {
            width: known_dimensions.width.unwrap_or(2.0),
            height: known_dimensions.height.unwrap_or(1.0),
        },
        NodeKind::Spinner {
            frames,
            frame_index,
            label,
            ..
        } => {
            let frame = if !frames.is_empty() {
                &frames[*frame_index % frames.len()]
            } else {
                " "
            };
            let frame_w = UnicodeWidthStr::width(frame) as f32;
            let total_w = match label {
                Some(lbl) => frame_w + 1.0 + (UnicodeWidthStr::width(&lbl[..]) as f32),
                None => frame_w,
            };
            Size {
                width: known_dimensions.width.unwrap_or(total_w),
                height: known_dimensions.height.unwrap_or(1.0),
            }
        }
        NodeKind::TextInput {
            value, placeholder, ..
        } => {
            let val_w = UnicodeWidthStr::width(&value[..]) as f32;
            let ph_w = placeholder
                .as_ref()
                .map(|p| UnicodeWidthStr::width(&p[..]) as f32)
                .unwrap_or(0.0);
            let content_w = val_w.max(ph_w) + 1.0; // cursor extra cell
            Size {
                width: known_dimensions.width.unwrap_or(content_w.max(10.0)),
                height: known_dimensions.height.unwrap_or(1.0),
            }
        }
        NodeKind::Border { .. } => Size {
            width: known_dimensions.width.unwrap_or(2.0),
            height: known_dimensions.height.unwrap_or(2.0),
        },
        NodeKind::Stack | NodeKind::Dim => Size {
            width: known_dimensions.width.unwrap_or(0.0),
            height: known_dimensions.height.unwrap_or(0.0),
        },
        NodeKind::Viewport { .. } => Size {
            width: known_dimensions.width.unwrap_or(0.0),
            height: known_dimensions.height.unwrap_or(0.0),
        },
        NodeKind::Raster { surface } => Size {
            width: known_dimensions.width.unwrap_or(surface.width as f32),
            height: known_dimensions.height.unwrap_or(surface.height as f32),
        },
        NodeKind::Box { border, .. } => {
            let min_s = if border.is_some() { 2.0 } else { 0.0 };
            Size {
                width: known_dimensions.width.unwrap_or(min_s),
                height: known_dimensions.height.unwrap_or(min_s),
            }
        }
    }
}

/// Recursively registers nodes into the Taffy tree.
fn build_taffy_tree(
    taffy: &mut TaffyTree<NodeKind>,
    node: &Node,
) -> Result<NodeId, taffy::TaffyError> {
    let mut child_ids = Vec::with_capacity(node.children.len());
    for child in &node.children {
        let cid = build_taffy_tree(taffy, child)?;
        child_ids.push(cid);
    }

    let is_stack = matches!(node.kind, NodeKind::Stack);
    let is_viewport = matches!(node.kind, NodeKind::Viewport { .. });
    let style = convert_style(&node.layout_style);
    let id = if child_ids.is_empty() {
        taffy.new_leaf_with_context(style, node.kind.clone())?
    } else {
        taffy.new_with_children(style, &child_ids)?
    };

    for (i, cid) in child_ids.iter().enumerate() {
        let child_abs = node
            .children
            .get(i)
            .map(|c| c.layout_style.absolute)
            .unwrap_or(false);

        if is_stack || child_abs {
            // Stack children fill the content box; explicitly positioned layers
            // are out of flow at the content origin (the painter applies their
            // signed offset, so off-screen/negative placement still clips).
            let mut cs = taffy.style(*cid)?.clone();
            cs.position = Position::Absolute;
            cs.inset = TaffyRect {
                left: LengthPercentageAuto::length(0.0),
                right: LengthPercentageAuto::length(0.0),
                top: LengthPercentageAuto::length(0.0),
                bottom: LengthPercentageAuto::length(0.0),
            };
            taffy.set_style(*cid, cs)?;
        }

        if is_viewport {
            // The world may be larger than the camera; it must not shrink.
            let mut cs = taffy.style(*cid)?.clone();
            cs.flex_shrink = 0.0;
            taffy.set_style(*cid, cs)?;
        }
    }

    Ok(id)
}

/// Recursively copies computed layout coordinates from Taffy to Node tree.
fn copy_layout_results(
    taffy: &TaffyTree<NodeKind>,
    node_id: NodeId,
    node: &mut Node,
    parent_x: u16,
    parent_y: u16,
) -> Result<(), taffy::TaffyError> {
    let layout = taffy.layout(node_id)?;

    let x = parent_x.saturating_add(layout.location.x.round().max(0.0) as u16);
    let y = parent_y.saturating_add(layout.location.y.round().max(0.0) as u16);
    let width = layout.size.width.round().max(0.0) as u16;
    let height = layout.size.height.round().max(0.0) as u16;

    node.computed_rect = Rect::new(x, y, width, height);

    let child_ids = taffy.children(node_id)?;
    for (i, child) in node.children.iter_mut().enumerate() {
        if i < child_ids.len() {
            copy_layout_results(taffy, child_ids[i], child, x, y)?;
        }
    }

    Ok(())
}

/// Computes integer cell layouts for the entire UI tree given available width and height.
pub fn compute_layout(
    root: &mut Node,
    available_width: u16,
    available_height: u16,
) -> Result<Rect, String> {
    let mut taffy = TaffyTree::new();
    let root_id = build_taffy_tree(&mut taffy, root).map_err(|e| format!("{:?}", e))?;

    let available_space = Size {
        width: AvailableSpace::Definite(available_width as f32),
        height: if available_height > 0 {
            AvailableSpace::Definite(available_height as f32)
        } else {
            AvailableSpace::MinContent
        },
    };

    taffy
        .compute_layout_with_measure(
            root_id,
            available_space,
            |inputs, _node_id, context, style| {
                taffy::compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known_dimensions, available_space| {
                        if let Some(kind) = context {
                            measure_leaf(kind, known_dimensions, available_space)
                        } else {
                            Size::ZERO
                        }
                    },
                )
            },
        )
        .map_err(|e| format!("{:?}", e))?;

    copy_layout_results(&taffy, root_id, root, 0, 0).map_err(|e| format!("{:?}", e))?;

    Ok(root.computed_rect)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Style;

    #[test]
    fn test_text_wrapping() {
        let text = "The quick brown fox jumps over the lazy dog";
        let wrapped = wrap_text(text, WrapMode::WordWrap, 15);
        assert!(wrapped.len() >= 3);
        for line in &wrapped {
            assert!(UnicodeWidthStr::width(&line[..]) <= 15);
        }
    }

    #[test]
    fn test_row_layout_flex() {
        let mut row = Node::row()
            .width(80.0)
            .height(1.0)
            .child(Node::text("Left", Style::default()).flex_grow(1.0))
            .child(Node::text("Right", Style::default()).flex_grow(1.0));

        let rect = compute_layout(&mut row, 80, 24).unwrap();
        assert_eq!(rect.width, 80);
        assert_eq!(rect.height, 1);

        assert_eq!(row.children[0].computed_rect.width, 40);
        assert_eq!(row.children[1].computed_rect.width, 40);
        assert_eq!(row.children[1].computed_rect.x, 40);
    }

    #[test]
    fn test_col_layout_with_gap() {
        let mut col = Node::col()
            .width(40.0)
            .gap(1.0)
            .child(Node::text("Line 1", Style::default()).height(1.0))
            .child(Node::text("Line 2", Style::default()).height(1.0));

        compute_layout(&mut col, 80, 24).unwrap();

        assert_eq!(col.children[0].computed_rect.y, 0);
        assert_eq!(col.children[0].computed_rect.height, 1);
        // Line 1 height is 1, gap is 1 -> Line 2 starts at y = 2
        assert_eq!(col.children[1].computed_rect.y, 2);
    }
}

#[cfg(test)]
mod additional_tests {
    use super::*;
    use crate::node::{Dimension, Node};

    #[test]
    fn test_percentage_semantics() {
        let mut n1 = Node::col();
        n1.layout_style.width = Dimension::Length(80.0);
        n1.layout_style.height = Dimension::Length(100.0);

        let mut child1 = Node::col();
        child1.layout_style.width = Dimension::Percent(50.0);
        child1.layout_style.height = Dimension::Percent(100.0);
        child1.layout_style.flex_shrink = 0.0;
        n1.add_child(child1.clone());

        let mut child2 = Node::col();
        child2.layout_style.width = Dimension::Percent(25.0);
        child2.layout_style.height = Dimension::Percent(50.0);
        child2.layout_style.flex_shrink = 0.0;
        n1.add_child(child2.clone());

        let _ = compute_layout(&mut n1, 80, 100);

        let c1_rect = n1.children[0].computed_rect;
        assert_eq!(c1_rect.width, 40);
        assert_eq!(c1_rect.height, 100);

        let c2_rect = n1.children[1].computed_rect;
        assert_eq!(c2_rect.width, 20);
        assert_eq!(c2_rect.height, 50);
    }
}
