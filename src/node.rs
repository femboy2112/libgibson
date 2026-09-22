use crate::cell::{Color, Style};
use crate::surface::{BorderType, Rect};

/// Flex direction for layout containers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDirection {
    #[default]
    Column,
    Row,
}

/// Alignment along the cross axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignItems {
    Start,
    End,
    Center,
    Stretch,
}

/// Justification along the main axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JustifyContent {
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Layout dimension constraints.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Dimension {
    #[default]
    Auto,
    Length(f32),
    Percent(f32),
}

/// Flexbox layout attributes for a node.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutStyle {
    pub direction: FlexDirection,
    pub width: Dimension,
    pub height: Dimension,
    pub min_width: Dimension,
    pub min_height: Dimension,
    pub max_width: Dimension,
    pub max_height: Dimension,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub gap_row: f32,
    pub gap_col: f32,
    pub padding_top: f32,
    pub padding_bottom: f32,
    pub padding_left: f32,
    pub padding_right: f32,
    pub align_items: Option<AlignItems>,
    pub justify_content: Option<JustifyContent>,
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            direction: FlexDirection::Column,
            width: Dimension::Auto,
            height: Dimension::Auto,
            min_width: Dimension::Auto,
            min_height: Dimension::Auto,
            max_width: Dimension::Auto,
            max_height: Dimension::Auto,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            gap_row: 0.0,
            gap_col: 0.0,
            padding_top: 0.0,
            padding_bottom: 0.0,
            padding_left: 0.0,
            padding_right: 0.0,
            align_items: None,
            justify_content: None,
        }
    }
}

/// How text wraps when exceeding container bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WrapMode {
    #[default]
    NoWrap,
    CharWrap,
    WordWrap,
}

/// Default standard spinner frame animation sets.
pub const SPINNER_BRAILLE: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
pub const SPINNER_DOTS: &[&str] = &[".  ", ".. ", "...", " ..", "  .", "   "];

/// Specific component visual and behavioral variant.
#[derive(Debug, Clone)]
pub enum NodeKind {
    Box {
        border: Option<BorderType>,
        border_style: Style,
        background: Option<Color>,
    },
    Text {
        text: String,
        style: Style,
        wrap: WrapMode,
    },
    Border {
        border_type: BorderType,
        style: Style,
        title: Option<String>,
        title_style: Style,
    },
    Spinner {
        frames: Vec<String>,
        frame_index: usize,
        style: Style,
        label: Option<String>,
        label_style: Style,
    },
    TextInput {
        value: String,
        cursor_grapheme: usize,
        placeholder: Option<String>,
        style: Style,
        placeholder_style: Style,
        cursor_style: Style,
        scroll_offset: usize,
    },
}

/// A node in the declarative UI render tree.
#[derive(Debug, Clone)]
pub struct Node {
    pub kind: NodeKind,
    pub layout_style: LayoutStyle,
    pub children: Vec<Node>,
    /// Computed layout rectangle after layout pass.
    pub computed_rect: Rect,
}

impl Node {
    pub fn new(kind: NodeKind) -> Self {
        Self {
            kind,
            layout_style: LayoutStyle::default(),
            children: Vec::new(),
            computed_rect: Rect::default(),
        }
    }

    /// Creates a column flex box.
    pub fn col() -> Self {
        let mut n = Self::new(NodeKind::Box {
            border: None,
            border_style: Style::default(),
            background: None,
        });
        n.layout_style.direction = FlexDirection::Column;
        n
    }

    /// Creates a row flex box.
    pub fn row() -> Self {
        let mut n = Self::new(NodeKind::Box {
            border: None,
            border_style: Style::default(),
            background: None,
        });
        n.layout_style.direction = FlexDirection::Row;
        n
    }

    /// Creates a text node.
    pub fn text(content: impl Into<String>, style: Style) -> Self {
        Self::new(NodeKind::Text {
            text: content.into(),
            style,
            wrap: WrapMode::NoWrap,
        })
    }

    /// Creates a wrapped text node.
    pub fn text_wrapped(content: impl Into<String>, style: Style, wrap: WrapMode) -> Self {
        Self::new(NodeKind::Text {
            text: content.into(),
            style,
            wrap,
        })
    }

    /// Creates a spinner node.
    pub fn spinner(frame_index: usize, style: Style, label: Option<&str>) -> Self {
        let frames = SPINNER_BRAILLE.iter().map(|&s| s.to_string()).collect();
        Self::new(NodeKind::Spinner {
            frames,
            frame_index,
            style,
            label: label.map(|s| s.to_string()),
            label_style: Style::default(),
        })
    }

    /// Creates a text input node.
    pub fn text_input(
        value: impl Into<String>,
        cursor_grapheme: usize,
        placeholder: Option<&str>,
        style: Style,
    ) -> Self {
        Self::new(NodeKind::TextInput {
            value: value.into(),
            cursor_grapheme,
            placeholder: placeholder.map(|s| s.to_string()),
            style,
            placeholder_style: Style::new().dim(),
            cursor_style: Style::new().reverse(),
            scroll_offset: 0,
        })
    }

    /// Creates a bordered container node.
    pub fn border_box(border_type: BorderType, style: Style) -> Self {
        let mut n = Self::new(NodeKind::Box {
            border: Some(border_type),
            border_style: style,
            background: None,
        });
        n.layout_style.padding_top = 1.0;
        n.layout_style.padding_bottom = 1.0;
        n.layout_style.padding_left = 1.0;
        n.layout_style.padding_right = 1.0;
        n
    }

    // Builder chaining methods

    pub fn width(mut self, width: f32) -> Self {
        self.layout_style.width = Dimension::Length(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.layout_style.height = Dimension::Length(height);
        self
    }

    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.layout_style.flex_grow = grow;
        self
    }

    pub fn flex_shrink(mut self, shrink: f32) -> Self {
        self.layout_style.flex_shrink = shrink;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.layout_style.gap_row = gap;
        self.layout_style.gap_col = gap;
        self
    }

    pub fn padding(mut self, p: f32) -> Self {
        self.layout_style.padding_top = p;
        self.layout_style.padding_bottom = p;
        self.layout_style.padding_left = p;
        self.layout_style.padding_right = p;
        self
    }

    pub fn padding_axes(mut self, horizontal: f32, vertical: f32) -> Self {
        self.layout_style.padding_left = horizontal;
        self.layout_style.padding_right = horizontal;
        self.layout_style.padding_top = vertical;
        self.layout_style.padding_bottom = vertical;
        self
    }

    pub fn align_items(mut self, align: AlignItems) -> Self {
        self.layout_style.align_items = Some(align);
        self
    }

    pub fn justify_content(mut self, justify: JustifyContent) -> Self {
        self.layout_style.justify_content = Some(justify);
        self
    }

    pub fn background(mut self, bg: Color) -> Self {
        if let NodeKind::Box {
            ref mut background, ..
        } = self.kind
        {
            *background = Some(bg);
        }
        self
    }

    pub fn add_child(&mut self, child: Node) {
        self.children.push(child);
    }

    pub fn child(mut self, child: Node) -> Self {
        self.children.push(child);
        self
    }

    pub fn children(mut self, children: Vec<Node>) -> Self {
        self.children = children;
        self
    }
}
