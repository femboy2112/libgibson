//! Inspectable lowering to existing substrate objects.
use super::element::{Element, ElementKind, Key};
use super::interaction::{Interaction, InteractionMap, ModalScope, UiError};
use super::motion::MotionRole;
use super::skin::{Chrome, ControlRole, ControlState, ResolvedSkin, Skin, UiEnvironment};
use super::style::{Density, Elevation, Emphasis, Tone};
use crate::{AlignItems, Dimension, JustifyContent, Node, SurfaceFx, WrapMode};
use std::collections::BTreeMap;
use std::time::Duration;

/// Environment for constructing a semantic tree. All clocks are explicit.
///
/// Focus and motion are intentionally absent: they are reconciled *after* the
/// complete tree exists. A [`super::element::Component`] must express semantic
/// intent without inspecting a previous frame's presentation state. Use
/// [`super::element::presented`] for a custom ordinary node that needs the
/// current frame's reconciled [`PresentationCx`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildCx {
    pub skin: ResolvedSkin,
    pub environment: UiEnvironment,
    pub time: Duration,
}
impl BuildCx {
    pub fn new(skin: Skin, environment: UiEnvironment) -> Self {
        Self {
            skin: skin.resolve(&environment),
            environment,
            time: Duration::ZERO,
        }
    }
}

/// Reconciled context used only when lowering a complete semantic tree.
///
/// [`super::runtime::UiRuntime::frame`] prepares this after validating keys,
/// restoring modal focus, and resolving motion for the current frame. Explicit
/// callers can construct one for [`compile_presented`]. Custom [`super::element::presented`]
/// leaves see it during lowering and cannot change semantic metadata.
#[derive(Clone)]
pub struct PresentationCx {
    pub build: BuildCx,
    pub focused: Option<Key>,
    pub motions: BTreeMap<Key, Vec<SurfaceFx>>,
}

impl PresentationCx {
    /// A settled, unfocused presentation for an environment.
    pub fn new(build: BuildCx) -> Self {
        Self {
            build,
            focused: None,
            motions: BTreeMap::new(),
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

#[cfg(test)]
std::thread_local! {
    static METADATA_PASSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn metadata_passes() -> usize {
    METADATA_PASSES.with(std::cell::Cell::get)
}

pub(crate) fn collect_metadata<A: Clone>(root: &Element<A>) -> Result<UiMetadata<A>, UiError> {
    #[cfg(test)]
    METADATA_PASSES.with(|passes| passes.set(passes.get() + 1));
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
                | ElementKind::Tabs
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
    if matches!(el.kind, ElementKind::Input { .. })
        || el.on_press.is_some()
        || el.on_edit.is_some()
        || el.on_event.is_some()
    {
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
/// This environment-only path is settled and unfocused. Use [`compile_presented`]
/// for explicit presentation state or [`super::runtime::UiRuntime::frame`] for
/// current-tree reconciliation.
/// Duplicate keys and excessive semantic-tree depth/size are rejected before lowering.
pub fn compile<A: Clone>(root: &Element<A>, cx: &BuildCx) -> Result<Compiled<A>, UiError> {
    compile_presented(root, &PresentationCx::new(*cx))
}

/// Pure lowering with explicit presentation state. Unlike a runtime frame, this
/// does not reconcile or validate that `focused` belongs to an enabled control;
/// the caller supplies that state intentionally. Structural validation still
/// runs before any custom presentation callback is invoked.
pub fn compile_presented<A: Clone>(
    root: &Element<A>,
    cx: &PresentationCx,
) -> Result<Compiled<A>, UiError> {
    let metadata = collect_metadata(root)?;
    Ok(lower_with_metadata(root, cx, metadata))
}

/// Both entry paths lower only after collecting and validating metadata once.
pub(crate) fn lower_with_metadata<A>(
    root: &Element<A>,
    cx: &PresentationCx,
    metadata: UiMetadata<A>,
) -> Compiled<A> {
    let mut section = 0;
    let node = lower(root, cx, &mut vec![], &mut section, false, None);
    Compiled {
        node,
        interactions: metadata.interactions,
        keys: metadata.keys,
    }
}

fn rule(cx: &PresentationCx, width: u16) -> Node {
    Node::text(
        cx.build.skin.glyphs.horizontal.repeat(width as usize),
        cx.build.skin.border,
    )
    .height(1.0)
    .min_width(0.0)
    .flex_shrink(0.0)
}
fn title(cx: &PresentationCx, value: &str, section: usize) -> String {
    let value = if cx.build.skin.typography.uppercase_titles {
        value.to_uppercase()
    } else {
        value.to_owned()
    };
    if cx.build.skin.typography.numbered_sections {
        format!("{section:02} / {value}")
    } else {
        value
    }
}

// Edges are ordinary positioned text, kept out of intrinsic layout. Their
// percent bounds are solved by Taffy against the actual local frame, including
// nested panels; no terminal-sized Surface or persistent post-process is used.
fn trailing_edges(
    cx: &PresentationCx,
    vertical: &str,
    bottom: Node,
    style: crate::Style,
) -> Vec<Node> {
    let right = Node::col()
        .percent_width(100.0)
        .percent_height(100.0)
        .padding_top(1.0)
        .align_items(AlignItems::End)
        .offset(0.0, 0.0)
        .child(
            Node::text_wrapped(
                vertical.repeat(cx.build.environment.height as usize),
                style,
                WrapMode::CharWrap,
            )
            .width(1.0)
            .percent_height(100.0)
            .min_height(0.0),
        );
    let bottom = Node::col()
        .percent_width(100.0)
        .percent_height(100.0)
        .justify_content(JustifyContent::End)
        .offset(0.0, 0.0)
        .child(bottom.height(1.0).percent_width(100.0).flex_shrink(0.0));
    vec![right, bottom]
}

fn beveled_window(cx: &PresentationCx, skin: &ResolvedSkin, window: Node) -> Node {
    let (_, _, left, right, horizontal, vertical) = skin.border_type.chars();
    let bottom = Node::row()
        .child(Node::text(left, skin.lowlight).width(1.0).flex_shrink(0.0))
        .child(
            Node::text(
                horizontal.repeat(cx.build.environment.width as usize),
                skin.lowlight,
            )
            .width(0.0)
            .flex_grow(1.0),
        )
        .child(Node::text(right, skin.lowlight).width(1.0).flex_shrink(0.0));
    let mut frame = Node::col().child(window.flex_grow(1.0));
    for edge in trailing_edges(cx, vertical, bottom, skin.lowlight) {
        frame.add_child(edge);
    }
    // The wrapper keeps the natural window size; the shadow is a separate
    // bounded silhouette, omitted when the application requests Compact.
    if skin.density == Density::Compact {
        return frame;
    }
    let mut shadow = Node::col()
        .padding_right(1.0)
        .padding_bottom(1.0)
        .child(frame.flex_grow(1.0));
    let bottom = Node::text(" ".repeat(cx.build.environment.width as usize), skin.shadow);
    for edge in trailing_edges(cx, " ", bottom, skin.shadow) {
        shadow.add_child(edge);
    }
    shadow
}
fn container(
    cx: &PresentationCx,
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
            beveled_window(cx, skin, window)
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
            .child(rule(cx, cx.build.environment.width))
            .child(Node::text(title(cx, value, section), skin.title).height(1.0))
            .child(body),
    }
}
fn lower<A>(
    el: &Element<A>,
    cx: &PresentationCx,
    path: &mut Vec<usize>,
    section: &mut usize,
    disabled: bool,
    inherited_density: Option<Density>,
) -> Node {
    lower_role(el, cx, path, section, disabled, inherited_density, None)
}

fn lower_role<A>(
    el: &Element<A>,
    cx: &PresentationCx,
    path: &mut Vec<usize>,
    section: &mut usize,
    disabled: bool,
    inherited_density: Option<Density>,
    control_role: Option<ControlRole>,
) -> Node {
    let id = key(el, path);
    let disabled = disabled || el.disabled;
    let density = el
        .density
        .or(inherited_density)
        .unwrap_or(cx.build.skin.density);
    let (skin_gap, skin_padding) = cx.build.skin.spacing.resolve(density);
    let gap = el.layout.gap.unwrap_or(skin_gap);
    let padding = el.layout.padding.unwrap_or(skin_padding);
    let focused = !disabled && cx.focused.as_ref() == Some(&id);
    let style = if disabled {
        cx.build.skin.style(el.tone, Emphasis::Faint)
    } else {
        cx.build.skin.style(el.tone, el.emphasis)
    };
    let mut chrome_skin = cx.build.skin;
    chrome_skin.density = density;
    // Tone changes the semantic chrome while retaining its title-bar treatment.
    if el.tone != Tone::Neutral {
        if chrome_skin.chrome == Chrome::Window {
            // The title remains light on a tone-colored title bar. Applying a
            // dark semantic foreground to the existing plum bar loses contrast.
            if cx.build.environment.color_depth != crate::ColorDepth::Mono {
                chrome_skin.title.bg = style.fg.or(chrome_skin.title.bg);
            }
        } else {
            chrome_skin.title.fg = style.fg;
            chrome_skin.highlight = style.bold();
        }
        chrome_skin.border = style;
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
        el.number.map(usize::from).unwrap_or(*section)
    } else {
        0
    };
    let mut children: Vec<Node> = el
        .children
        .iter()
        .enumerate()
        .map(|(i, child)| {
            path.push(i);
            let n = lower_role(
                child,
                cx,
                path,
                section,
                disabled,
                Some(density),
                matches!(el.kind, ElementKind::Tabs).then_some(ControlRole::Tab),
            );
            path.pop();
            n
        })
        .collect();
    // A horizontal semantic row allocates unspecified bases by grow weight.
    // Explicit widths and responsive columns retain ordinary dimensions.
    if matches!(el.kind, ElementKind::Row | ElementKind::Tabs)
        && !el
            .layout
            .breakpoint
            .is_some_and(|b| cx.build.environment.width < b)
    {
        for (child, semantic) in children.iter_mut().zip(&el.children) {
            if semantic.layout.width.is_none() && semantic.layout.grow.is_some_and(|g| g > 0.0) {
                child.layout_style.width = Dimension::Length(0.0);
            }
        }
    }
    let available = cx
        .build
        .environment
        .width
        .saturating_sub(2 * padding.min(cx.build.environment.width / 2));
    let width = el.layout.width.unwrap_or(24).min(available);
    let mut node = match &el.kind {
        ElementKind::Screen => Node::col()
            .background(cx.build.skin.background)
            .width(cx.build.environment.width as f32)
            .padding(padding as f32)
            .gap(gap as f32)
            .children(children),
        ElementKind::Row | ElementKind::Tabs => if el
            .layout
            .breakpoint
            .is_some_and(|b| cx.build.environment.width < b)
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
                style.overlay(cx.build.skin.title).bold()
            } else {
                style.bold()
            },
        )
        .height(1.0),
        ElementKind::Code(s) => Node::text(
            s,
            if el.tone == Tone::Neutral {
                style.overlay(cx.build.skin.styles.code)
            } else {
                style
            },
        ),
        ElementKind::Divider => rule(cx, cx.build.environment.width),
        ElementKind::Panel(s) | ElementKind::Card(s) | ElementKind::Section(s) => container(
            cx,
            &chrome_skin,
            s,
            children,
            item_section,
            (padding, gap),
            el.elevation != Elevation::Flat,
        ),
        ElementKind::Status(s) => Node::text(
            format!("{} {s}", cx.build.skin.status_marker(el.tone)),
            style,
        )
        .height(1.0),
        ElementKind::Badge(s) => Node::text(format!(" {s} "), style.reverse()).height(1.0),
        ElementKind::Button(s) | ElementKind::Choice(s) => cx.build.skin.control(
            s,
            control_role.unwrap_or(if matches!(el.kind, ElementKind::Choice(_)) {
                ControlRole::Choice
            } else {
                ControlRole::Button
            }),
            ControlState {
                focused,
                selected: el.selected,
                disabled,
            },
            style,
        ),
        ElementKind::Input { state, placeholder } => {
            let input_style = if cx.build.skin.chrome == Chrome::Window {
                style.bg(cx.build.skin.well)
            } else {
                style
            };
            let input = if focused {
                {
                    let mut input = Node::text_input(
                        &state.text,
                        state.cursor_grapheme,
                        placeholder.as_deref(),
                        input_style,
                    )
                    .scroll_offset(state.scroll_offset)
                    .height(1.0);
                    if let crate::NodeKind::TextInput {
                        placeholder_style,
                        cursor_style,
                        ..
                    } = &mut input.kind
                    {
                        *placeholder_style = if cx.build.skin.chrome == Chrome::Window {
                            cx.build.skin.styles.muted.bg(cx.build.skin.well)
                        } else {
                            cx.build.skin.styles.muted
                        };
                        *cursor_style = cx.build.skin.selection;
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
                        if cx.build.skin.chrome == Chrome::Window {
                            cx.build.skin.styles.muted.bg(cx.build.skin.well)
                        } else {
                            cx.build.skin.styles.muted
                        }
                    } else {
                        input_style
                    },
                )
                .height(1.0)
            };
            if cx.build.skin.chrome == Chrome::Window {
                cx.build.skin.instrument_well(input, focused)
            } else {
                input
            }
        }
        ElementKind::Progress { label, fraction } => {
            cx.build.skin.progress(label, *fraction, width)
        }
        ElementKind::Sparkline(values) => cx.build.skin.sparkline(values, width),
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
                            if head { cx.build.skin.title } else { style },
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
                .child(rule(cx, cx.build.environment.width));
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
            if let Some(effects) = cx.motions.get(&id) {
                panel = panel.post_process(effects.clone());
            }
            panel = panel.post_process(el.effects.clone());
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
            let skin = &cx.build.skin;
            let content = format!("{} {s}", skin.status_marker(el.tone));
            let mut panel = match skin.chrome {
                Chrome::Window => {
                    let mut compact = *skin;
                    compact.density = Density::Compact;
                    beveled_window(
                        cx,
                        &compact,
                        Node::border_box(skin.border_type, skin.highlight)
                            .background(skin.surface)
                            .child(Node::text(content, style)),
                    )
                }
                Chrome::Rail => Node::row()
                    .background(skin.well)
                    .padding_axes(1.0, 0.0)
                    .align_items(AlignItems::Center)
                    .child(
                        Node::text(skin.glyphs.rail, style.bg(skin.well).bold())
                            .width(1.0)
                            .height(1.0),
                    )
                    .child(
                        Node::text_wrapped(content, style.bg(skin.well), WrapMode::WordWrap)
                            .min_width(0.0)
                            .flex_grow(1.0),
                    ),
                Chrome::Editorial => Node::col()
                    .background(skin.surface)
                    .child(rule(cx, w))
                    .child(Node::text_wrapped(content, style, WrapMode::WordWrap).min_width(0.0)),
            }
            .percent_width(100.0)
            .max_width(w as f32)
            .height(h as f32);
            panel.layout_style.max_height = Dimension::Percent(100.0);
            if let Some(effects) = cx.motions.get(&id) {
                panel = panel.post_process(effects.clone());
            }
            panel = panel.post_process(el.effects.clone());
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
        ElementKind::Presented(build) => build(cx),
    };
    if matches!(el.kind, ElementKind::Row | ElementKind::Tabs)
        && !el
            .layout
            .breakpoint
            .is_some_and(|b| cx.build.environment.width < b)
        && el
            .children
            .iter()
            .any(|c| c.layout.grow.is_some_and(|g| g > 0.0))
    {
        node = node.percent_width(100.0);
    }
    // Raw nodes preserve their own layout until a builder explicitly overrides it.
    if !matches!(el.kind, ElementKind::Raw(_) | ElementKind::Presented(_)) {
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
    if !matches!(el.kind, ElementKind::Modal(_) | ElementKind::Toast(_)) {
        if let Some(effects) = cx.motions.get(&id) {
            node = node.post_process(effects.clone());
        }
        node = node.post_process(el.effects.clone());
    }
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
            layers = layers.height(cx.build.environment.height as f32);
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
            let mut floating = lower(overlay, cx, path, section, disabled, Some(density));
            // Mount out of flow without erasing an expert Node's local offset.
            floating.layout_style.absolute = true;
            layers = layers.child(floating);
            path.pop();
            path.pop();
        }
        node = layers;
    }
    node
}

#[cfg(test)]
mod visual_tests {
    use super::*;
    use crate::ui::{label, panel, row, skins, text_input, MotionPreference};
    use crate::{compute_layout, paint, ColorDepth, Surface, TextInputState};

    #[test]
    fn vapor_bevel_keeps_corners_local_and_inset_editor_keeps_cursor() {
        for width in [17, 38, 39] {
            for color_depth in [ColorDepth::Mono, ColorDepth::TrueColor] {
                let environment = UiEnvironment {
                    width,
                    height: 10,
                    color_depth,
                    motion: MotionPreference::None,
                    ..UiEnvironment::default()
                };
                let mut cx = PresentationCx::new(BuildCx::new(skins::VAPOR95, environment));
                cx.focused = Some(Key::from("editor"));
                let tree = panel("Window")
                    .density(Density::Compact)
                    .width(width)
                    .child(
                        text_input(&TextInputState::with_text("READY"))
                            .key("editor")
                            .on_edit(|_| ()),
                    );
                let mut node = compile_presented(&tree, &cx).unwrap().node;
                compute_layout(&mut node, width, 10).unwrap();
                let mut surface = Surface::new(width, 10);
                let cursor = paint(&node, &mut surface)
                    .cursor_position
                    .expect("well lost editor cursor");
                assert_eq!(surface.get(0, 0).unwrap().glyph.grapheme.as_str(), "╔");
                assert_eq!(
                    surface.get(width - 1, 0).unwrap().glyph.grapheme.as_str(),
                    "╗"
                );
                assert_eq!(
                    surface.get(width - 1, 3).unwrap().glyph.grapheme.as_str(),
                    "╝"
                );
                assert_ne!(
                    surface.get(0, 1).unwrap().style,
                    surface.get(width - 1, 1).unwrap().style
                );
                assert!(cursor.0 > 1 && cursor.0 < width - 1);
                assert_eq!(cursor.1, 2);
            }
        }
    }

    #[test]
    fn stretched_vapor_window_fills_its_frame_instead_of_exposing_screen_background() {
        let environment = UiEnvironment {
            width: 60,
            height: 20,
            motion: MotionPreference::None,
            ..UiEnvironment::default()
        };
        let cx = BuildCx::new(skins::VAPOR95, environment);
        let tree: Element<()> = row()
            .height(12)
            .gap(1)
            .child(
                panel("Short")
                    .density(Density::Normal)
                    .grow(1.0)
                    .child(label("Ready")),
            )
            .child(
                panel("Long")
                    .density(Density::Normal)
                    .grow(1.0)
                    .child(label("Work")),
            );
        let mut node = compile(&tree, &cx).unwrap().node;
        compute_layout(&mut node, 60, 20).unwrap();
        let mut surface = Surface::new(60, 20);
        paint(&node, &mut surface);
        assert_eq!(surface.get(3, 8).unwrap().style.bg, Some(cx.skin.surface));
        assert_eq!(surface.get(35, 8).unwrap().style.bg, Some(cx.skin.surface));
    }
}
