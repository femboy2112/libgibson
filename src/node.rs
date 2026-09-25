use crate::cell::{Color, Line, RichText, Style};
use crate::surface::{BorderType, Rect, Surface};
use std::sync::Arc;

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
    /// When true this node is taken out of flow and positioned relative to its
    /// parent's content origin by `(offset_x, offset_y)`. Signed offsets allow
    /// off-screen placement; painting clips to the parent. Siblings never
    /// reflow when the offset changes.
    pub absolute: bool,
    pub offset_x: f32,
    pub offset_y: f32,
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
            absolute: false,
            offset_x: 0.0,
            offset_y: 0.0,
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
///
/// `#[non_exhaustive]`: this is the primary extension point for new component
/// kinds, so downstream `match`es must include a `_` arm; adding a variant here is
/// then a non-breaking `0.1.z` change rather than a `0.y` bump.
#[derive(Debug, Clone)]
#[non_exhaustive]
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
    RichText {
        text: RichText,
        wrap: WrapMode,
    },
    Rule {
        title: Option<String>,
        style: Style,
        title_style: Style,
    },
    Rail {
        style: Style,
    },
    Border {
        border_type: BorderType,
        style: Style,
        title: Option<String>,
        title_style: Style,
        /// Optional background fill for the whole panel rect (used by floating
        /// panels/modals so the layer beneath is hidden). `Color::Reset` fills
        /// with the terminal default background.
        background: Option<Color>,
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
    /// Overlapping layer container. Children are laid out into the same content
    /// box (z-order = child order) and composited with explicit transparency.
    Stack,
    /// A dim veil over its rectangle. Applied in-place on an opaque surface, or
    /// as a style-only layer when painted into a transparent scratch surface.
    Dim,
    /// A clipped camera: its single child is translated by `(offset_x, offset_y)`
    /// (in cells) and clipped to this node's rectangle. Pair with
    /// [`crate::ViewportState`] for scrolling.
    Viewport {
        offset_x: i32,
        offset_y: i32,
    },
    /// An already-rendered raster surface composited at this node's rectangle.
    /// Held behind an `Arc` so cloning a node never duplicates the buffer.
    Raster {
        surface: Arc<Surface>,
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
    /// Already-realized, ordered surface operations. Empty uses the ordinary
    /// paint path. No clocks, application state, or layout changes live here.
    pub surface_fx: Vec<crate::surface_fx::SurfaceFx>,
}

impl Node {
    pub fn new(kind: NodeKind) -> Self {
        Self {
            kind,
            layout_style: LayoutStyle::default(),
            children: Vec::new(),
            computed_rect: Rect::default(),
            surface_fx: Vec::new(),
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

    /// Appends post-processing to this node's ordinary painted subtree.
    ///
    /// Layout is identical. Only nodes with a nonempty chain allocate an entity
    /// scratch surface. Removing the chain reveals the original rendering.
    pub fn post_process(
        mut self,
        effects: impl IntoIterator<Item = crate::surface_fx::SurfaceFx>,
    ) -> Self {
        self.surface_fx.extend(effects);
        self
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

    /// Creates an overlapping-layer container.
    ///
    /// Children occupy the same content box; z-order is child order (later
    /// children paint on top). Composite transparency means a child only covers
    /// the cells it actually paints. Give the stack an explicit size, percent
    /// size, or flex sizing — it sizes to its largest child otherwise.
    pub fn stack() -> Self {
        Self::new(NodeKind::Stack)
    }

    /// Creates a dim veil over the node's rectangle.
    pub fn dim() -> Self {
        Self::new(NodeKind::Dim)
    }

    /// Creates a clipped camera viewport. Its single child is translated by
    /// `(offset_x, offset_y)` cells and clipped to the viewport rectangle.
    pub fn viewport(offset_x: i32, offset_y: i32) -> Self {
        Self::new(NodeKind::Viewport { offset_x, offset_y })
    }

    /// Creates a raster node from a shared surface (cheap to clone).
    ///
    /// The node defaults to the surface's own cell dimensions so a raster is
    /// visible without the caller having to repeat its size; callers may still
    /// override width/height for scaling or clipping.
    pub fn surface(surface: Arc<Surface>) -> Self {
        let (w, h) = (surface.width as f32, surface.height as f32);
        let mut node = Self::new(NodeKind::Raster { surface });
        node.layout_style.width = Dimension::Length(w);
        node.layout_style.height = Dimension::Length(h);
        node
    }

    /// Creates a raster node owning a surface.
    pub fn raster(surface: Surface) -> Self {
        Self::surface(Arc::new(surface))
    }

    /// Positions this node absolutely inside its parent by `(x, y)` cells.
    ///
    /// The node is removed from flow, so moving it never reflows siblings.
    /// Negative offsets (partly/fully off-screen) clip at the parent bounds.
    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.layout_style.absolute = true;
        self.layout_style.offset_x = x;
        self.layout_style.offset_y = y;
        self
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

    /// Creates a rich text node.
    pub fn scroll_offset(mut self, offset: usize) -> Self {
        if let NodeKind::TextInput { scroll_offset, .. } = &mut self.kind {
            *scroll_offset = offset;
        }
        self
    }

    pub fn rich_text(text: impl Into<RichText>) -> Self {
        Self::new(NodeKind::RichText {
            text: text.into(),
            wrap: WrapMode::WordWrap,
        })
    }

    /// Creates a rich text node with explicit wrap mode.
    pub fn rich_text_wrapped(text: impl Into<RichText>, wrap: WrapMode) -> Self {
        Self::new(NodeKind::RichText {
            text: text.into(),
            wrap,
        })
    }

    /// Creates a single-line rich text node.
    pub fn line(line: impl Into<Line>) -> Self {
        Self::rich_text(RichText::from(line.into()))
    }

    /// Creates a horizontal rule separator.
    pub fn rule(title: Option<impl Into<String>>, style: Style) -> Self {
        let mut n = Self::new(NodeKind::Rule {
            title: title.map(|t| t.into()),
            style,
            title_style: style.bold(),
        });
        n.layout_style.height = Dimension::Length(1.0);
        n
    }

    /// Creates a callout rail with a vertical line on the left margin.
    pub fn rail(style: Style) -> Self {
        let mut n = Self::new(NodeKind::Rail { style });
        n.layout_style.direction = FlexDirection::Column;
        n.layout_style.padding_left = 2.0; // 1 col for '│' + 1 col space
        n
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

    /// Creates a titled bordered panel — the primary container for dashboards.
    ///
    /// Unlike [`Node::border_box`], the title is drawn into the top border with a
    /// bold emphasis, and children are inset by one cell of padding.
    pub fn panel(title: impl Into<String>, border_type: BorderType, style: Style) -> Self {
        let mut n = Self::new(NodeKind::Border {
            border_type,
            style,
            title: Some(title.into()),
            title_style: style.bold(),
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

    pub fn percent_width(mut self, percent: f32) -> Self {
        self.layout_style.width = Dimension::Percent(percent);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.layout_style.height = Dimension::Length(height);
        self
    }

    pub fn percent_height(mut self, percent: f32) -> Self {
        self.layout_style.height = Dimension::Percent(percent);
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.layout_style.min_width = Dimension::Length(width);
        self
    }

    pub fn max_width(mut self, width: f32) -> Self {
        self.layout_style.max_width = Dimension::Length(width);
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.layout_style.min_height = Dimension::Length(height);
        self
    }

    pub fn max_height(mut self, height: f32) -> Self {
        self.layout_style.max_height = Dimension::Length(height);
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

    pub fn padding_left(mut self, p: f32) -> Self {
        self.layout_style.padding_left = p;
        self
    }

    pub fn padding_right(mut self, p: f32) -> Self {
        self.layout_style.padding_right = p;
        self
    }

    pub fn padding_top(mut self, p: f32) -> Self {
        self.layout_style.padding_top = p;
        self
    }

    pub fn padding_bottom(mut self, p: f32) -> Self {
        self.layout_style.padding_bottom = p;
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
        match &mut self.kind {
            NodeKind::Box {
                ref mut background, ..
            } => *background = Some(bg),
            NodeKind::Border {
                ref mut background, ..
            } => *background = Some(bg),
            _ => {}
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
