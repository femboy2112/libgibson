//! Inspectable lowering to existing substrate objects.
use super::element::{Element, ElementKind, Key};
use super::interaction::{Interaction, InteractionMap, ModalScope, UiError};
use super::motion::MotionRole;
use super::skin::{Chrome, ResolvedSkin, Skin, UiEnvironment};
use super::style::{Density, Elevation, Emphasis, Tone};
use crate::{AlignItems, Dimension, JustifyContent, Node, SurfaceFx, WrapMode};
use std::collections::BTreeMap;
use std::time::Duration;

/// The complete context of a pure lowering operation. All clocks are explicit.
#[derive(Clone)]
pub struct BuildCx {
    pub skin: ResolvedSkin,
    pub environment: UiEnvironment,
    pub focused: Option<Key>,
    pub motions: BTreeMap<Key, Vec<SurfaceFx>>,
    pub time: Duration,
}
impl BuildCx {
    pub fn new(skin: Skin, environment: UiEnvironment) -> Self {
        Self {
            skin: skin.resolve(&environment),
            environment,
            focused: None,
            motions: BTreeMap::new(),
            time: Duration::ZERO,
        }
    }
}

/// Only presentation-relevant semantic changes, never application data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementState {
    pub revision: u64,
    pub selected: bool,
    pub disabled: bool,
    pub modal: bool,
    pub motion: Option<MotionRole>,
}
/// The boundary is public: inspect or transform the node, or put it in a Scene.
pub struct Compiled<A> {
    pub node: Node,
    pub interactions: InteractionMap<A>,
    pub keys: BTreeMap<Key, ElementState>,
}
pub(crate) struct UiMetadata<A> {
    pub interactions: InteractionMap<A>,
    pub keys: BTreeMap<Key, ElementState>,
}

pub(crate) fn collect_metadata<A: Clone>(root: &Element<A>) -> Result<UiMetadata<A>, UiError> {
    let mut out = UiMetadata {
        interactions: InteractionMap::default(),
        keys: BTreeMap::new(),
    };
    collect(root, &mut vec![], None, false, &mut out)?;
    Ok(out)
}
fn key<A>(el: &Element<A>, path: &[usize]) -> Key {
    el.key.clone().unwrap_or_else(|| Key::Path(path.to_vec()))
}
fn collect<A: Clone>(
    el: &Element<A>,
    path: &mut Vec<usize>,
    modal: Option<Key>,
    disabled: bool,
    out: &mut UiMetadata<A>,
) -> Result<(), UiError> {
    if path.len() > 128 {
        return Err(UiError::TooDeep { limit: 128 });
    }
    if out.keys.len() >= 16384 {
        return Err(UiError::TooManyElements { limit: 16384 });
    }
    let id = key(el, path);
    if !el.children.is_empty()
        && !matches!(
            el.kind,
            ElementKind::Screen
                | ElementKind::Row
                | ElementKind::Column
                | ElementKind::Stack
                | ElementKind::Panel(_)
                | ElementKind::Card(_)
                | ElementKind::Section(_)
                | ElementKind::List
                | ElementKind::Modal(_)
                | ElementKind::Viewport(..)
        )
    {
        return Err(UiError::InvalidChildren(id));
    }
    if matches!(el.kind, ElementKind::Viewport(..)) && el.children.len() > 1 {
        return Err(UiError::InvalidViewport(id));
    }
    let disabled = disabled || el.disabled;
    let is_modal = matches!(el.kind, ElementKind::Modal(_));
    if out
        .keys
        .insert(
            id.clone(),
            ElementState {
                revision: el.revision,
                selected: el.selected,
                disabled,
                modal: is_modal,
                motion: el.motion,
            },
        )
        .is_some()
    {
        return Err(UiError::DuplicateKey(id));
    }
    let scope = if is_modal {
        out.interactions.modals.push(ModalScope {
            key: id.clone(),
            parent: modal.clone(),
            on_dismiss: el.on_dismiss.clone(),
        });
        Some(id.clone())
    } else {
        modal
    };
    if el.on_press.is_some() || el.on_edit.is_some() || el.on_event.is_some() {
        out.interactions.entries.push(Interaction {
            key: id,
            disabled,
            modal: scope.clone(),
            on_press: el.on_press.clone(),
            input: match &el.kind {
                ElementKind::Input { state, .. } => Some(state.clone()),
                _ => None,
            },
            on_edit: el.on_edit.clone(),
            on_event: el.on_event.clone(),
        });
    }
    for (i, child) in el.children.iter().enumerate() {
        path.push(i);
        collect(child, path, scope.clone(), disabled, out)?;
        path.pop();
    }
    for (i, child) in el.overlays.iter().enumerate() {
        path.extend([usize::MAX, i]);
        collect(child, path, scope.clone(), disabled, out)?;
        path.pop();
        path.pop();
    }
    Ok(())
}

/// Compile without owning any terminal or mutating any application state.
/// Duplicate keys and excessive semantic-tree depth/size are rejected before lowering.
pub fn compile<A: Clone>(root: &Element<A>, cx: &BuildCx) -> Result<Compiled<A>, UiError> {
    let metadata = collect_metadata(root)?;
    let mut section = 0;
    let node = lower(root, cx, &mut vec![], &mut section, false, None);
    Ok(Compiled {
        node,
        interactions: metadata.interactions,
        keys: metadata.keys,
    })
}

fn rule(cx: &BuildCx, width: u16) -> Node {
    Node::text(
        cx.skin.glyphs.horizontal.repeat(width as usize),
        cx.skin.border,
    )
    .height(1.0)
    .min_width(0.0)
    .flex_shrink(0.0)
}
fn title(cx: &BuildCx, value: &str, section: usize) -> String {
    let value = if cx.skin.typography.uppercase_titles {
        value.to_uppercase()
    } else {
        value.to_owned()
    };
    if cx.skin.typography.numbered_sections {
        format!("{section:02} / {value}")
    } else {
        value
    }
}
fn container(
    cx: &BuildCx,
    skin: &ResolvedSkin,
    value: &str,
    children: Vec<Node>,
    section: usize,
    spacing: (u16, u16),
    elevated: bool,
) -> Node {
    let (padding, gap) = spacing;
    let body = Node::col()
        .gap(gap as f32)
        .padding_axes(padding as f32, 0.0)
        .min_width(0.0)
        .children(children);
    match skin.chrome {
        Chrome::Window => {
            let window = Node::border_box(skin.border_type, skin.highlight)
                .background(skin.surface)
                .child(
                    Node::col()
                        .background(skin.title.bg.unwrap_or(skin.surface))
                        .child(
                            Node::text(
                                format!(" {}  {}", title(cx, value, section), skin.glyphs.close),
                                skin.title,
                            )
                            .height(1.0),
                        ),
                )
                .child(body);
            // Chrome is ordinary sibling composition: a permanent effect on the
            // entire window would suppress the focused editor hardware cursor.
            if skin.density == Density::Compact {
                window
            } else {
                Node::col().child(window).child(
                    Node::text(
                        skin.glyphs.horizontal.repeat(cx.environment.width as usize),
                        skin.shadow,
                    )
                    .height(1.0)
                    .min_width(0.0),
                )
            }
        }
        Chrome::Rail => {
            let content = Node::col()
                .child(
                    Node::text(
                        format!("{} {}", skin.glyphs.rail, title(cx, value, section)),
                        skin.title,
                    )
                    .height(1.0),
                )
                .child(body);
            if elevated {
                Node::border_box(skin.border_type, skin.border)
                    .background(skin.surface)
                    .child(content)
            } else {
                Node::col().background(skin.surface).child(content)
            }
        }
        Chrome::Editorial => Node::col()
            .background(skin.surface)
            .child(rule(cx, cx.environment.width))
            .child(Node::text(title(cx, value, section), skin.title).height(1.0))
            .child(body),
    }
}
fn lower<A>(
    el: &Element<A>,
    cx: &BuildCx,
    path: &mut Vec<usize>,
    section: &mut usize,
    disabled: bool,
    inherited_density: Option<Density>,
) -> Node {
    let id = key(el, path);
    let disabled = disabled || el.disabled;
    let density = el.density.or(inherited_density).unwrap_or(cx.skin.density);
    let (skin_gap, skin_padding) = cx.skin.spacing.resolve(density);
    let gap = el.layout.gap.unwrap_or(skin_gap);
    let padding = el.layout.padding.unwrap_or(skin_padding);
    let focused = !disabled && cx.focused.as_ref() == Some(&id);
    let style = if disabled {
        cx.skin.style(el.tone, Emphasis::Faint)
    } else {
        cx.skin.style(el.tone, el.emphasis)
    };
    let mut chrome_skin = cx.skin;
    chrome_skin.density = density;
    // Tone changes the semantic chrome while retaining its title-bar treatment.
    if el.tone != Tone::Neutral {
        chrome_skin.title.fg = style.fg;
        chrome_skin.border = style;
        chrome_skin.highlight = style.bold();
    }
    if matches!(el.emphasis, Emphasis::Faint | Emphasis::Muted) {
        chrome_skin.title = chrome_skin.title.dim();
    }
    let item_section = if matches!(
        el.kind,
        ElementKind::Panel(_)
            | ElementKind::Card(_)
            | ElementKind::Section(_)
            | ElementKind::Modal(_)
    ) {
        *section += 1;
        *section
    } else {
        0
    };
    let mut children: Vec<Node> = el
        .children
        .iter()
        .enumerate()
        .map(|(i, child)| {
            path.push(i);
            let n = lower(child, cx, path, section, disabled, Some(density));
            path.pop();
            n
        })
        .collect();
    // A horizontal semantic row allocates unspecified bases by grow weight.
    // Explicit widths and responsive columns retain ordinary dimensions.
    if matches!(el.kind, ElementKind::Row)
        && !el
            .layout
            .breakpoint
            .is_some_and(|b| cx.environment.width < b)
    {
        for (child, semantic) in children.iter_mut().zip(&el.children) {
            if semantic.layout.width.is_none() && semantic.layout.grow.is_some_and(|g| g > 0.0) {
                child.layout_style.width = Dimension::Length(0.0);
            }
        }
    }
    let available = cx
        .environment
        .width
        .saturating_sub(2 * padding.min(cx.environment.width / 2));
    let width = el.layout.width.unwrap_or(24).min(available);
    let mut node = match &el.kind {
        ElementKind::Screen => Node::col()
            .background(cx.skin.background)
            .width(cx.environment.width as f32)
            .padding(padding as f32)
            .gap(gap as f32)
            .children(children),
        ElementKind::Row => if el
            .layout
            .breakpoint
            .is_some_and(|b| cx.environment.width < b)
        {
            Node::col()
        } else {
            Node::row()
        }
        .gap(gap as f32)
        .children(children),
        ElementKind::Column | ElementKind::List => Node::col().gap(gap as f32).children(children),
        ElementKind::Stack => Node::stack().children(children),
        ElementKind::Spacer => Node::col(),
        ElementKind::Text(s) => Node::text_wrapped(s, style, WrapMode::WordWrap),
        ElementKind::Label(s) => Node::text(s, style).height(1.0),
        ElementKind::Heading(s) => Node::text(
            s,
            if el.tone == Tone::Neutral {
                style.overlay(cx.skin.title).bold()
            } else {
                style.bold()
            },
        )
        .height(1.0),
        ElementKind::Code(s) => Node::text(
            s,
            if el.tone == Tone::Neutral {
                style.overlay(cx.skin.styles.code)
            } else {
                style
            },
        ),
        ElementKind::Divider => rule(cx, cx.environment.width),
        ElementKind::Panel(s) | ElementKind::Card(s) | ElementKind::Section(s) => container(
            cx,
            &chrome_skin,
            s,
            children,
            item_section,
            (padding, gap),
            el.elevation != Elevation::Flat,
        ),
        ElementKind::Status(s) => {
            Node::text(format!("{} {s}", cx.skin.status_marker(el.tone)), style).height(1.0)
        }
        ElementKind::Badge(s) => Node::text(format!(" {s} "), style.reverse()).height(1.0),
        ElementKind::Button(s) | ElementKind::Choice(s) => Node::text(
            cx.skin.button_label(s, focused, el.selected),
            if focused || el.selected {
                style.overlay(cx.skin.selection)
            } else {
                style
            },
        )
        .height(1.0),
        ElementKind::Input { state, placeholder } => {
            if focused {
                {
                    let mut input = Node::text_input(
                        &state.text,
                        state.cursor_grapheme,
                        placeholder.as_deref(),
                        style,
                    )
                    .scroll_offset(state.scroll_offset)
                    .height(1.0);
                    if let crate::NodeKind::TextInput {
                        placeholder_style,
                        cursor_style,
                        ..
                    } = &mut input.kind
                    {
                        *placeholder_style = cx.skin.styles.muted;
                        *cursor_style = cx.skin.selection;
                    }
                    input
                }
            } else {
                // An inactive editor lowers to ordinary text, so it cannot steal
                // the substrate's single hardware cursor from the focused one.
                Node::text(
                    if state.text.is_empty() {
                        placeholder.as_deref().unwrap_or("")
                    } else {
                        &state.text
                    },
                    if state.text.is_empty() {
                        cx.skin.styles.muted
                    } else {
                        style
                    },
                )
                .height(1.0)
            }
        }
        ElementKind::Progress { label, fraction } => cx.skin.progress(label, *fraction, width),
        ElementKind::Sparkline(values) => cx.skin.sparkline(values, width),
        ElementKind::Table { headers, rows } => {
            let cols = headers
                .len()
                .max(rows.iter().map(Vec::len).max().unwrap_or(0));
            let make_row = |values: &[String], head: bool| {
                let mut row = Node::row().gap(2.0);
                for i in 0..cols {
                    row = row.child(
                        Node::text(
                            values.get(i).map(String::as_str).unwrap_or(""),
                            if head { cx.skin.title } else { style },
                        )
                        .percent_width(100.0 / cols.max(1) as f32)
                        .min_width(0.0)
                        .height(1.0),
                    );
                }
                row.height(1.0)
            };
            let mut table = Node::col()
                .child(make_row(headers, true))
                .child(rule(cx, cx.environment.width));
            for row in rows {
                table = table.child(make_row(row, false));
            }
            table
        }
        ElementKind::Modal(s) => {
            let w = el.layout.width.unwrap_or(52);
            let h = el.layout.height.unwrap_or(12);
            let mut panel = container(
                cx,
                &chrome_skin,
                s,
                children,
                item_section,
                (padding, gap),
                true,
            )
            .percent_width(90.0)
            .max_width(w as f32)
            .height(h as f32);
            panel.layout_style.max_height = Dimension::Percent(90.0);
            // Percent constraints are resolved by ordinary Taffy against the
            // containing overlay, including local panels and nested modals.
            Node::col()
                .percent_width(100.0)
                .percent_height(100.0)
                .align_items(AlignItems::Center)
                .justify_content(JustifyContent::Center)
                .child(
                    Node::dim()
                        .percent_width(100.0)
                        .percent_height(100.0)
                        .offset(0.0, 0.0),
                )
                .child(panel)
        }
        ElementKind::Toast(s) => {
            let w = el.layout.width.unwrap_or(36);
            let h = el.layout.height.unwrap_or(3);
            let mut panel = Node::border_box(cx.skin.border_type, cx.skin.border)
                .background(cx.skin.surface)
                .child(Node::text(
                    format!("{} {s}", cx.skin.status_marker(el.tone)),
                    style,
                ))
                .percent_width(100.0)
                .max_width(w as f32)
                .height(h as f32);
            panel.layout_style.max_height = Dimension::Percent(100.0);
            Node::col()
                .percent_width(100.0)
                .percent_height(100.0)
                .align_items(AlignItems::End)
                .justify_content(JustifyContent::End)
                .child(panel)
        }
        ElementKind::Viewport(x, y) => Node::viewport(*x, *y).children(children),
        ElementKind::Raw(raw) => {
            let mut n = raw.clone();
            n.children.extend(children);
            n
        }
    };
    if matches!(el.kind, ElementKind::Row)
        && !el
            .layout
            .breakpoint
            .is_some_and(|b| cx.environment.width < b)
        && el
            .children
            .iter()
            .any(|c| c.layout.grow.is_some_and(|g| g > 0.0))
    {
        node = node.percent_width(100.0);
    }
    // Raw nodes preserve their own layout until a builder explicitly overrides it.
    if !matches!(el.kind, ElementKind::Raw(_)) {
        node = node.min_width(0.0).flex_shrink(0.0);
    }
    if !matches!(el.kind, ElementKind::Modal(_) | ElementKind::Toast(_)) {
        if let Some(w) = el.layout.width {
            node = node.width(w as f32);
        }
        if let Some(h) = el.layout.height {
            node = node.height(h as f32);
        }
    }
    if let Some(grow) = el.layout.grow {
        node = node.flex_grow(grow).flex_shrink(1.0);
    }
    if let Some(gap) = el.layout.gap {
        node = node.gap(gap as f32);
    }
    if let Some(pad) = el.layout.padding {
        if !matches!(
            el.kind,
            ElementKind::Panel(_)
                | ElementKind::Card(_)
                | ElementKind::Section(_)
                | ElementKind::Modal(_)
        ) {
            node = node.padding(pad as f32);
        }
    }
    if let Some(effects) = cx.motions.get(&id) {
        node = node.post_process(effects.clone());
    }
    node = node.post_process(el.effects.clone());
    if !el.overlays.is_empty() {
        // An in-flow base plus absolute overlay children keeps intrinsic sizing.
        // A Stack makes *all* children absolute, so using it here would discard
        // the base's natural size. Transfer outer constraints to the wrapper.
        let original = node.layout_style.clone();
        let mut layers = Node::col();
        layers.layout_style.width = original.width;
        layers.layout_style.height = original.height;
        layers.layout_style.min_width = original.min_width;
        layers.layout_style.max_width = original.max_width;
        layers.layout_style.min_height = original.min_height;
        layers.layout_style.max_height = original.max_height;
        layers.layout_style.flex_grow = original.flex_grow;
        layers.layout_style.flex_shrink = original.flex_shrink;
        layers.layout_style.absolute = original.absolute;
        layers.layout_style.offset_x = original.offset_x;
        layers.layout_style.offset_y = original.offset_y;
        if matches!(el.kind, ElementKind::Screen) && el.layout.height.is_none() {
            layers = layers.height(cx.environment.height as f32);
        }
        node.layout_style.absolute = false;
        node.layout_style.offset_x = 0.0;
        node.layout_style.offset_y = 0.0;
        node.layout_style.flex_grow = 0.0;
        if original.width != Dimension::Auto {
            node.layout_style.width = Dimension::Percent(100.0);
        }
        if original.height != Dimension::Auto {
            node.layout_style.height = Dimension::Percent(100.0);
        }
        layers = layers.child(node);
        for (i, overlay) in el.overlays.iter().enumerate() {
            path.extend([usize::MAX, i]);
            layers = layers
                .child(lower(overlay, cx, path, section, disabled, Some(density)).offset(0.0, 0.0));
            path.pop();
            path.pop();
        }
        node = layers;
    }
    node
}
