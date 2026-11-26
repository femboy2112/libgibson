//! Semantic intent and ordinary Rust builders. No renderer or terminal ownership.
use super::compile::{BuildCx, PresentationCx};
use super::motion::MotionRole;
use super::style::{Density, Elevation, Emphasis, Tone};
use crate::{Event, Node, Surface, SurfaceFx, TextAreaState, TextInputState};
use std::fmt;
use std::sync::Arc;

/// Explicit keys survive reordering. Unkeyed elements receive positional paths.
/// Named, positional and scoped identities have separate namespaces.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Key {
    Named(String),
    Path(Vec<usize>),
    /// An explicit key placed under a scope by [`Element::scoped_keys`]. The scope is a
    /// structural part of the identity, not text joined onto the key, so scope `a` + key
    /// `b` never equals the user key `a.b`. Scopes nest: scoping an already scoped subtree
    /// wraps the existing key again.
    Scoped {
        scope: String,
        key: Box<Key>,
    },
}
impl Key {
    pub fn named(value: impl ToString) -> Self {
        Self::Named(value.to_string())
    }
    /// `key` placed under `scope`, as [`Element::scoped_keys`] produces it.
    pub fn scoped(scope: impl ToString, key: impl Into<Key>) -> Self {
        Self::Scoped {
            scope: scope.to_string(),
            key: Box::new(key.into()),
        }
    }
}
impl From<&str> for Key {
    fn from(value: &str) -> Self {
        Self::named(value)
    }
}
impl From<String> for Key {
    fn from(value: String) -> Self {
        Self::Named(value)
    }
}
impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Named(s) => write!(f, "{s}"),
            Self::Path(p) => write!(f, "path:{p:?}"),
            // Display is for people; identity is structural (see `Key::Scoped`).
            Self::Scoped { scope, key } => write!(f, "{scope}::{key}"),
        }
    }
}

#[derive(Clone)]
pub(crate) enum ElementKind {
    Screen,
    Row,
    Tabs,
    Column,
    Stack,
    Spacer,
    Text(String),
    Label(String),
    RichLabel(crate::cell::Line),
    TonedLabel(Vec<ToneSpan>),
    Image {
        fitted: super::image::Fitted,
        max: (u16, u16),
        alt: String,
        /// How `fitted` was sampled and is realized. `HalfBlock` is [`image`] /
        /// [`image_area`]: one sample per cell column, two per cell row.
        encoder: crate::glyph::CellEncoder,
    },
    Heading(String),
    Code(String),
    Panel(String),
    Card(String),
    Section(String),
    Divider,
    Status(String),
    Badge(String),
    Button(String),
    Choice(String),
    /// [`toggle`]: `[x] label` / `[ ] label`; `selected` carries the on state.
    Toggle(String),
    /// [`form_row`]: the label; the control is the single child.
    FormRow(String),
    Input {
        state: TextInputState,
        placeholder: Option<String>,
    },
    /// EXPERIMENTAL v0.9.1 multi-line editor ([`text_area`]).
    Area {
        state: TextAreaState,
        rows: u16,
    },
    Progress {
        label: String,
        fraction: f32,
    },
    ProgressSteps {
        label: String,
        done: u32,
        total: u32,
    },
    Sparkline(Vec<f32>),
    List,
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Modal(String),
    Toast(String),
    ToastLane,
    Viewport(i32, i32),
    Raw(Node),
    Presented(Arc<dyn Fn(&PresentationCx) -> Node>),
}

#[derive(Clone, Default)]
pub(crate) struct Layout {
    pub width: Option<u16>,
    pub height: Option<u16>,
    pub percent_width: Option<f32>,
    pub percent_height: Option<f32>,
    pub grow: Option<f32>,
    pub gap: Option<u16>,
    pub padding: Option<u16>,
    pub breakpoint: Option<u16>,
}

/// A semantic component tree. Business state is supplied anew by the application.
/// Lower it with [`super::compile::compile()`] or [`super::runtime::UiRuntime::frame`].
#[derive(Clone)]
pub struct Element<A> {
    pub(crate) kind: ElementKind,
    pub(crate) children: Vec<Self>,
    pub(crate) overlays: Vec<Self>,
    pub(crate) key: Option<Key>,
    pub(crate) on_press: Option<A>,
    pub(crate) on_edit: Option<Arc<dyn Fn(TextInputState) -> A>>,
    pub(crate) on_area_edit: Option<Arc<dyn Fn(TextAreaState) -> A>>,
    pub(crate) on_event: Option<Arc<dyn Fn(Event) -> A>>,
    pub(crate) on_dismiss: Option<A>,
    pub(crate) selected: bool,
    pub(crate) disabled: bool,
    pub(crate) revision: u64,
    pub(crate) motion: Option<MotionRole>,
    pub(crate) tone: Tone,
    /// [`Element::error`]: one `Tone::Danger` row under the control.
    pub(crate) error: Option<String>,
    /// [`Element::hint`]: a dim trailing cell on a [`form_row`].
    pub(crate) hint: Option<String>,
    pub(crate) emphasis: Emphasis,
    pub(crate) density: Option<Density>,
    pub(crate) elevation: Elevation,
    pub(crate) number: Option<u16>,
    pub(crate) layout: Layout,
    pub(crate) effects: Vec<SurfaceFx>,
    /// Full-resolution source kept by [`pixel_image`]; `None` for every other element.
    pub(crate) source: Option<Arc<crate::raster::RgbaRaster>>,
}
impl<A> Element<A> {
    fn new(kind: ElementKind) -> Self {
        Self {
            kind,
            children: vec![],
            overlays: vec![],
            key: None,
            on_press: None,
            on_edit: None,
            on_area_edit: None,
            on_event: None,
            on_dismiss: None,
            selected: false,
            disabled: false,
            revision: 0,
            motion: None,
            tone: Tone::Neutral,
            error: None,
            hint: None,
            emphasis: Emphasis::Normal,
            density: None,
            elevation: Elevation::Flat,
            number: None,
            layout: Layout::default(),
            effects: vec![],
            source: None,
        }
    }
    pub fn child(mut self, child: Self) -> Self {
        self.children.push(child);
        self
    }
    pub fn children(mut self, children: impl IntoIterator<Item = Self>) -> Self {
        self.children.extend(children);
        self
    }
    /// Mount a floating layer without changing the layout of the base content.
    ///
    /// A [`screen`] with an overlay and no explicit `.height(...)` takes the
    /// environment height. In inline mode that grows the live region to nearly
    /// the full terminal the first time a popup opens. Give an inline screen a
    /// constant `.height(...)` so popups open and close with no reflow (see
    /// `docs/UI_LAYER.md`).
    pub fn overlay(mut self, overlay: Self) -> Self {
        self.overlays.push(overlay);
        self
    }
    pub fn key(mut self, key: impl ToString) -> Self {
        self.key = Some(Key::named(key));
        self
    }
    pub fn on_press(mut self, action: A) -> Self {
        self.on_press = Some(action);
        self
    }
    /// The application receives the edited state; the UI does not store its own draft.
    pub fn on_edit(mut self, action: impl Fn(TextInputState) -> A + 'static) -> Self {
        self.on_edit = Some(Arc::new(action));
        self
    }
    /// EXPERIMENTAL (v0.9.1): the [`text_area`] counterpart of [`on_edit`](Self::on_edit).
    /// The application receives the edited [`TextAreaState`] (its `scroll_row` already
    /// updated for the visible rows); the UI does not store its own draft. This is the
    /// only edit signal: a text area never emits a commit. See
    /// `docs/design/v0.9.1-text-area.md`.
    pub fn on_area_edit(mut self, action: impl Fn(TextAreaState) -> A + 'static) -> Self {
        self.on_area_edit = Some(Arc::new(action));
        self
    }
    /// Visible rows of a [`text_area`] (at least 1; default 4). Other elements ignore it.
    /// Set the box with `rows` and `.width(..)`, not `.height(..)`.
    pub fn rows(mut self, rows: u16) -> Self {
        if let ElementKind::Area { rows: r, .. } = &mut self.kind {
            *r = rows.max(1);
        }
        self
    }
    /// Route unhandled events to a focused custom control, e.g. a viewport.
    pub fn on_event(mut self, action: impl Fn(Event) -> A + 'static) -> Self {
        self.on_event = Some(Arc::new(action));
        self
    }
    pub fn on_dismiss(mut self, action: A) -> Self {
        self.on_dismiss = Some(action);
        self
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    /// Explicit semantic change counter for motion; changing text alone does not restart effects.
    pub fn revision(mut self, revision: u64) -> Self {
        self.revision = revision;
        self
    }
    pub fn motion(mut self, role: MotionRole) -> Self {
        self.motion = Some(role);
        self
    }
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
    /// Attach a validation message shown on a [`Tone::Danger`] line under the
    /// control. `None` renders nothing and takes no rows. The element stays
    /// focusable either way, and the message row is never a focus stop.
    ///
    /// On a [`form_row`] the line is indented to the control column; on any
    /// other element it sits at the element's own left edge. It is one row,
    /// clipped on the right, never wrapped.
    ///
    /// The parameter is `Option<String>` (not `Option<impl Into<String>>`) so a
    /// bare `.error(None)` infers without annotation.
    pub fn error(mut self, message: Option<String>) -> Self {
        self.error = message;
        self
    }
    /// Attach a secondary note to a [`form_row`]: a dim, single-row cell after
    /// the control on the same row, clipped on the right, never wrapped and never
    /// a focus stop. `None` renders nothing, byte-identical to omitting the call.
    /// Other elements ignore it.
    ///
    /// The parameter is `Option<String>` for the same reason as [`Element::error`].
    pub fn hint(mut self, hint: Option<String>) -> Self {
        self.hint = hint;
        self
    }
    pub fn emphasis(mut self, emphasis: Emphasis) -> Self {
        self.emphasis = emphasis;
        self
    }
    pub fn density(mut self, density: Density) -> Self {
        self.density = Some(density);
        self
    }
    pub fn elevation(mut self, elevation: Elevation) -> Self {
        self.elevation = elevation;
        self
    }
    /// An authored section number for skins with numbered titles. Unlike the
    /// automatic traversal ordinal, it stays meaningful through reflow/reorder.
    /// Applies to panel/card/section/modal titles; other roles ignore it.
    pub fn number(mut self, number: u16) -> Self {
        self.number = Some(number);
        self
    }
    /// Sets the width in terminal columns (the flex basis when the element grows).
    ///
    /// Note: this does not change the element's flex growth. A [`spacer`] grows
    /// by default (`grow(1.0)`), so `spacer().width(n)` is only a preferred
    /// basis and may receive a different width when free space is distributed.
    /// For a fixed-width spacer, also clear its growth:
    /// `spacer().width(n).grow(0.)`.
    pub fn width(mut self, width: u16) -> Self {
        self.layout.width = Some(width);
        self
    }
    pub fn height(mut self, height: u16) -> Self {
        self.layout.height = Some(height);
        self
    }
    /// Proportional width as a percentage of the parent's content box (issue #36),
    /// lowering to Taffy's `Dimension::Percent` — the semantic counterpart of
    /// `Node::percent_width`. Use for split panes (e.g. `.percent_width(60.0)`)
    /// where a fixed cell width or `grow` share does not express the intent.
    pub fn percent_width(mut self, percent: f32) -> Self {
        self.layout.percent_width = Some(if percent.is_finite() {
            percent.max(0.0)
        } else {
            0.0
        });
        self
    }
    /// Proportional height as a percentage of the parent's content box (issue #36),
    /// lowering to Taffy's `Dimension::Percent` — the semantic counterpart of
    /// `Node::percent_height`. Use for vertical splits (e.g. 70% stage / 30% log).
    pub fn percent_height(mut self, percent: f32) -> Self {
        self.layout.percent_height = Some(if percent.is_finite() {
            percent.max(0.0)
        } else {
            0.0
        });
        self
    }
    /// Relative width share inside a horizontal row; ordinary flex growth elsewhere.
    /// An explicit width supplies the basis instead. Responsive columns keep intrinsic width.
    pub fn grow(mut self, grow: f32) -> Self {
        self.layout.grow = Some(if grow.is_finite() { grow.max(0.0) } else { 0.0 });
        self
    }
    pub fn gap(mut self, gap: u16) -> Self {
        self.layout.gap = Some(gap);
        self
    }
    pub fn padding(mut self, padding: u16) -> Self {
        self.layout.padding = Some(padding);
        self
    }
    /// Rows become columns below this terminal-width breakpoint; identity is preserved.
    pub fn responsive(mut self, below: u16) -> Self {
        self.layout.breakpoint = Some(below);
        self
    }
    pub fn placeholder(mut self, value: impl Into<String>) -> Self {
        if let ElementKind::Input { placeholder, .. } = &mut self.kind {
            *placeholder = Some(value.into());
        }
        self
    }
    /// Append ordinary local substrate post-processing after semantic motion.
    pub fn post_process(mut self, effects: impl IntoIterator<Item = SurfaceFx>) -> Self {
        self.effects.extend(effects);
        self
    }
    /// Place every explicit key in this subtree (this element, children and overlays) under
    /// the scope `prefix`: key `k` becomes [`Key::Scoped`] `{ scope: prefix, key: k }`
    /// (see [`Key::scoped`]), so a second copy of the same view can coexist in one tree
    /// without `UiError::DuplicateKey`. The scope is structural, so it can never alias a user
    /// key that happens to contain a separator. Unkeyed elements keep their position-derived
    /// keys, which are already unique. Used for the outgoing view of a transition; see
    /// `docs/design/v0.9-view-transitions.md`.
    pub fn scoped_keys(mut self, prefix: &str) -> Self {
        self.scope_keys_in_place(prefix);
        self
    }
    fn scope_keys_in_place(&mut self, prefix: &str) {
        if let Some(key) = self.key.take() {
            self.key = Some(Key::scoped(prefix, key));
        }
        for child in self.children.iter_mut().chain(self.overlays.iter_mut()) {
            child.scope_keys_in_place(prefix);
        }
    }
}

/// Optional semantic extension route. Ordinary functions returning `Element<A>`
/// work too. The environment-only context deliberately has no focus or motion:
/// those are reconciled after this component has returned its tree. A custom
/// node needing current presentation state can be returned with [`presented`].
pub trait Component<A> {
    fn build(self, cx: &BuildCx) -> Element<A>;
}
pub fn component<A>(value: impl Component<A>, cx: &BuildCx) -> Element<A> {
    value.build(cx)
}
pub fn screen<A>() -> Element<A> {
    Element::new(ElementKind::Screen)
}
pub fn row<A>() -> Element<A> {
    Element::new(ElementKind::Row)
}
pub fn column<A>() -> Element<A> {
    Element::new(ElementKind::Column)
}
pub fn stack<A>() -> Element<A> {
    Element::new(ElementKind::Stack)
}
/// A flexible gap that absorbs free space along its parent's main axis.
///
/// A spacer starts with `grow(1.0)`, and [`Element::width`] / [`Element::height`]
/// leave that growth in place, so `spacer().width(n)` does not reserve exactly
/// `n` columns when free space is distributed. Fixed-width spacer: clear the
/// growth explicitly with `spacer().width(n).grow(0.)`. This is documented
/// behavior; `spacer()` growth is unchanged.
pub fn spacer<A>() -> Element<A> {
    Element::new(ElementKind::Spacer).grow(1.0)
}
pub fn text<A>(value: impl Into<String>) -> Element<A> {
    Element::new(ElementKind::Text(value.into()))
}
pub fn label<A>(value: impl Into<String>) -> Element<A> {
    Element::new(ElementKind::Label(value.into()))
}
/// A single-row label whose spans each carry their own style: a gutter that
/// differs from its code, a heading with one coloured word.
///
/// Like [`label`] it is exactly one row tall and never wraps; content wider than
/// the space it is given is clipped, so a caller's row budget stays exact. The
/// element's own style (tone, emphasis) is the base each span's style is laid
/// over, so a span's explicit colour wins and an unstyled span looks like a
/// plain [`label`].
///
/// ```
/// use gibson::cell::{Color, Span, Style};
/// use gibson::ui::prelude::*;
///
/// let gutter: Element<()> = rich_label(vec![
///     Span::styled("12 │ ", Style::new().dim()),
///     Span::styled("let x = 1;", Style::new().fg(Color::Green)),
/// ]);
/// # let _ = gutter;
/// ```
pub fn rich_label<A>(line: impl Into<crate::cell::Line>) -> Element<A> {
    Element::new(ElementKind::RichLabel(line.into()))
}
/// One run of text in a [`toned_label`], styled by the skin's semantic tones
/// rather than by explicit colours, so it follows the active skin and colour depth.
///
/// Built with [`tone_span`]; defaults to [`Tone::Neutral`] and [`Emphasis::Normal`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToneSpan {
    pub(crate) text: String,
    pub(crate) tone: Tone,
    pub(crate) emphasis: Emphasis,
}
impl ToneSpan {
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
    pub fn emphasis(mut self, emphasis: Emphasis) -> Self {
        self.emphasis = emphasis;
        self
    }
    pub fn strong(self) -> Self {
        self.emphasis(Emphasis::Strong)
    }
    pub fn muted(self) -> Self {
        self.emphasis(Emphasis::Muted)
    }
    pub fn faint(self) -> Self {
        self.emphasis(Emphasis::Faint)
    }
    pub fn text(&self) -> &str {
        &self.text
    }
}
/// A [`ToneSpan`] with neutral tone and normal emphasis.
pub fn tone_span(text: impl Into<String>) -> ToneSpan {
    ToneSpan {
        text: text.into(),
        tone: Tone::Neutral,
        emphasis: Emphasis::Normal,
    }
}
/// A single-row label whose spans each take a semantic [`Tone`] and [`Emphasis`]
/// from the active skin: one warning-coloured word inside a muted row.
///
/// Row, clipping and sanitising behave exactly as in [`rich_label`]: one row,
/// never wraps, cut on the right, nothing after a line break, no control
/// characters. Text that comes first is therefore the last to be cut, so put the
/// word that must survive a narrow terminal first. In monochrome the tones fall
/// back to the skin's mono treatment (bold, dim) and the words remain.
///
/// ```
/// use gibson::ui::prelude::*;
///
/// let provenance: Element<()> = toned_label([
///     tone_span("SYNTHETIC").tone(Tone::Warning).strong(),
///     tone_span(" · image · seed 42").muted(),
/// ]);
/// # let _ = provenance;
/// ```
pub fn toned_label<A>(spans: impl IntoIterator<Item = ToneSpan>) -> Element<A> {
    Element::new(ElementKind::TonedLabel(spans.into_iter().collect()))
}
/// A static picture painted with half-block cells (two pixels per cell, one
/// above the other) inside a box of at most `max = (columns, rows)` cells.
///
/// The raster is resampled once, here, to fit the box with its aspect ratio
/// preserved and never enlarged beyond one pixel per half-cell. Only the fitted
/// pixels are kept: construction reads at most `2 × columns × rows` source
/// pixels and allocates only that many, whatever the raster's size (an
/// [`RgbRaster`](crate::raster::RgbRaster) is itself capped at [`crate::raster::MAX_RASTER_DIMENSION`]
/// per side). Sampling is nearest-pixel, so fine detail can alias when shrinking.
///
/// The element is always exactly `rows` tall, so swapping one picture for
/// another, or for its text, never moves the layout below it. It is as wide as
/// the box, or narrower if the terminal (less the screen's padding) is narrower
/// or an explicit `.width(..)` is smaller; the picture is then shrunk again,
/// still within the same pixel budget. Like [`progress`], it does not measure
/// its parent container, so inside a narrower panel it is clipped, not shrunk:
/// choose `max` (or `.width(..)`) for the space you place it in.
///
/// `alt` is painted instead of any pixels (muted, one row, clipped on the right)
/// when the colour depth is monochrome, when the box is smaller than 20 × 6
/// cells, or when the raster is empty. The picture is never animated.
///
/// ```
/// use gibson::ui::prelude::*;
/// use gibson::raster::RgbRaster;
///
/// let decoded = RgbRaster::new(640, 480);
/// let picture: Element<()> = image(&decoded, (40, 12), "image 640×480 · SYNTHETIC");
/// # let _ = picture;
/// ```
pub fn image<A>(
    raster: &crate::raster::RgbRaster,
    max: (u16, u16),
    alt: impl Into<String>,
) -> Element<A> {
    Element::new(ElementKind::Image {
        fitted: super::image::Fitted::from_raster(raster, max.0, max.1.saturating_mul(2)),
        max,
        alt: alt.into(),
        encoder: crate::glyph::CellEncoder::HalfBlock,
    })
}
/// [`image`] with an exact area (box) filter instead of nearest-pixel
/// sampling, for painted or photographic art that would otherwise alias. An
/// explicit opt-in: it reads every source pixel once at construction (work
/// proportional to the raster, not the box) and still stores only the fitted
/// pixels. Everything else is as for [`image`].
pub fn image_area<A>(
    raster: &crate::raster::RgbRaster,
    max: (u16, u16),
    alt: impl Into<String>,
) -> Element<A> {
    Element::new(ElementKind::Image {
        fitted: super::image::Fitted::from_raster_area(raster, max.0, max.1.saturating_mul(2)),
        max,
        alt: alt.into(),
        encoder: crate::glyph::CellEncoder::HalfBlock,
    })
}
/// Options for [`image_with`]. The default is the half-block encoder, so
/// `image_with(.., ImageOptions::default())` draws what [`image`] draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct ImageOptions {
    /// How the picture is realized as cells. The box, its reserved height and
    /// the alt-text rule do not depend on it.
    pub encoder: crate::glyph::CellEncoder,
}

impl ImageOptions {
    /// The default options: half-block, as [`image`].
    pub const fn new() -> Self {
        Self {
            encoder: crate::glyph::CellEncoder::HalfBlock,
        }
    }

    /// These options with `encoder`.
    pub fn with_encoder(mut self, encoder: crate::glyph::CellEncoder) -> Self {
        self.encoder = encoder;
        self
    }
}

/// [`image`] with an opt-in cell encoder (v0.9.0 design:
/// `docs/design/v0.9-cell-encoders.md`). The box, the reserved height and the
/// alt-text fallback are those of [`image`] for every encoder.
///
/// [`CellEncoder::HalfBlock`](crate::glyph::CellEncoder::HalfBlock) (the default
/// option) is exactly [`image`]. Any other encoder changes only the sampling
/// density: the footprint in cells is the half-block one, and it is sampled at the
/// encoder's grid (`sample_grid`) with the exact area filter of [`image_area`], so
/// one-pixel detail does not alias. Like `image` it stores only the fitted
/// samples. Colour depth is applied after encoding, at paint time. The encoder is
/// not gated here: degrade it first with [`crate::allowed_encoder`].
pub fn image_with<A>(
    raster: &crate::raster::RgbRaster,
    max: (u16, u16),
    alt: impl Into<String>,
    options: ImageOptions,
) -> Element<A> {
    let encoder = options.encoder;
    if encoder == crate::glyph::CellEncoder::HalfBlock {
        return image(raster, max, alt);
    }
    Element::new(ElementKind::Image {
        fitted: super::image::Fitted::from_raster_grid(
            raster,
            max.0,
            max.1.saturating_mul(2),
            encoder.sample_grid(),
        ),
        max,
        alt: alt.into(),
        encoder,
    })
}
/// A static picture that keeps its full-resolution straight-alpha source.
///
/// Unlike [`image`], the element holds the caller's `Arc<RgbaRaster>` itself
/// (the same shape as `stage::RasterPlane.source`), so alpha and the source
/// resolution survive lowering. The element carries no pixel route and no image
/// cache handle: whether a terminal paints it as pixels or as cells is decided
/// later, by the terminal adapter, on the lowered frame.
///
/// Today's lowering paints it as an ordinary [`image`]: the source is flattened onto black
/// and fitted to `max`. The kept source is what a later terminal adapter reads.
pub fn pixel_image<A>(source: Arc<crate::raster::RgbaRaster>, max: (u16, u16)) -> Element<A> {
    let flat = crate::raster::flatten(&source, (0, 0, 0));
    let mut element = image(&flat, max, String::new());
    element.source = Some(source);
    element
}
impl<A> Element<A> {
    /// The full-resolution source kept by [`pixel_image`]; `None` for every other element.
    pub fn image_source(&self) -> Option<&Arc<crate::raster::RgbaRaster>> {
        self.source.as_ref()
    }
}
pub fn heading<A>(value: impl Into<String>) -> Element<A> {
    Element::new(ElementKind::Heading(value.into()))
}
pub fn code<A>(value: impl Into<String>) -> Element<A> {
    Element::new(ElementKind::Code(value.into()))
}
pub fn panel<A>(title: impl Into<String>) -> Element<A> {
    Element::new(ElementKind::Panel(title.into()))
}
pub fn card<A>(title: impl Into<String>) -> Element<A> {
    Element::new(ElementKind::Card(title.into())).elevation(Elevation::Raised)
}
pub fn section<A>(title: impl Into<String>) -> Element<A> {
    Element::new(ElementKind::Section(title.into()))
}
pub fn divider<A>() -> Element<A> {
    Element::new(ElementKind::Divider)
}
pub fn status<A>(value: impl Into<String>) -> Element<A> {
    Element::new(ElementKind::Status(value.into()))
}
pub fn badge<A>(value: impl Into<String>) -> Element<A> {
    Element::new(ElementKind::Badge(value.into()))
}
pub fn button<A>(label: impl Into<String>) -> Element<A> {
    Element::new(ElementKind::Button(label.into()))
}
pub fn choice<A>(label: impl Into<String>, selected: bool) -> Element<A> {
    Element::new(ElementKind::Choice(label.into())).selected(selected)
}
/// A labelled on/off setting, rendered `[x] label` / `[ ] label`.
///
/// The marker is text, never colour alone. It is focusable like [`choice`]
/// (when it has an `on_press`), and Space and Enter fire `on_press`. State stays
/// application-owned: activation never mutates `on`. A disabled toggle behaves
/// like a disabled [`choice`]: dim skin, not a focus stop, no actions.
pub fn toggle<A>(label: impl Into<String>, on: bool) -> Element<A> {
    Element::new(ElementKind::Toggle(label.into())).selected(on)
}
/// One settings row: `label` on the left, `control` on the right, on one row at
/// 80 columns or wider. Below 80 columns the label stacks on its own row above
/// the control, which is indented two columns.
///
/// The label column is aligned across sibling rows in the same container. The
/// row is not a focus stop: the control keeps its own key, handlers, focus and
/// editing behaviour, and a non-interactive control is skipped by Tab. See
/// [`Element::error`] and [`Element::hint`].
pub fn form_row<A>(label: impl Into<String>, control: Element<A>) -> Element<A> {
    Element::new(ElementKind::FormRow(label.into())).child(control)
}
/// An application-owned single-line editor. Inputs participate in focus and own
/// editing keys/paste even without callbacks. Without `on_edit` they consume
/// these events without changing the supplied state; bind `on_edit` to receive
/// an edited copy, and rebuild before the next event. Up/Down are consumed
/// no-ops in this single-line editor. Unsupported shortcuts
/// remain available to `on_event` or the application's unconsumed-event path.
pub fn text_input<A>(state: &TextInputState) -> Element<A> {
    Element::new(ElementKind::Input {
        state: state.clone(),
        placeholder: None,
    })
}
/// Default wrap width, in cells, of a [`text_area`] without an explicit `.width(..)`.
pub(crate) const DEFAULT_AREA_COLS: u16 = 80;
/// Default visible rows of a [`text_area`].
pub(crate) const DEFAULT_AREA_ROWS: u16 = 4;
/// The wrap width, in cells, shared by editing (`TextAreaState::handle_event`) and
/// painting for a [`text_area`]: its explicit `.width(..)`, else
/// [`DEFAULT_AREA_COLS`]; never 0. It is not clamped by the parent (like
/// [`image`], the element does not measure its parent), so editing and painting
/// can never wrap at different widths.
pub(crate) fn area_cols<A>(el: &Element<A>) -> u16 {
    el.layout.width.unwrap_or(DEFAULT_AREA_COLS).max(1)
}
/// EXPERIMENTAL (v0.9.1): an application-owned multi-line editor.
///
/// Enter inserts a newline and never commits; there is no commit signal on the
/// element or the state. Provide an explicit Save control. Bind
/// [`Element::on_area_edit`] to receive an edited copy, and rebuild before the next
/// event. Set the width with `.width(n)` (the wrap width for editing and painting)
/// and the visible rows with [`Element::rows`]. Esc and cancel/restore belong to the
/// consumer: Esc is not consumed. See `docs/design/v0.9.1-text-area.md`.
pub fn text_area<A>(state: &TextAreaState) -> Element<A> {
    Element::new(ElementKind::Area {
        state: state.clone(),
        rows: DEFAULT_AREA_ROWS,
    })
}
pub fn progress<A>(label: impl Into<String>, fraction: f32) -> Element<A> {
    Element::new(ElementKind::Progress {
        label: label.into(),
        fraction,
    })
}
/// Progress counted in discrete steps: `label done/total` and a meter that never
/// shows more than `done / total` (it rounds down to the nearest eighth of a cell).
///
/// Unlike [`progress`] it prints the count instead of a percentage, and it has no
/// animation of its own, so a settled frame is the same with motion on or off.
/// When the colour depth is monochrome, or the element is 40 columns wide or
/// less, the meter is dropped and only the one text row is painted. `done` is
/// clamped to `total`; a `total` of zero paints `0/0` and an empty meter.
///
/// ```
/// use gibson::ui::prelude::*;
///
/// let job: Element<()> = progress_steps("step", 7, 30).width(48);
/// # let _ = job;
/// ```
pub fn progress_steps<A>(label: impl Into<String>, done: u32, total: u32) -> Element<A> {
    Element::new(ElementKind::ProgressSteps {
        label: label.into(),
        done: done.min(total),
        total,
    })
}
pub fn sparkline<A>(values: &[f32]) -> Element<A> {
    Element::new(ElementKind::Sparkline(values.to_vec()))
}
pub fn list<A>() -> Element<A> {
    Element::new(ElementKind::List)
}
pub fn table<A, H, R, C, S>(headers: H, rows: R) -> Element<A>
where
    H: IntoIterator<Item = S>,
    R: IntoIterator<Item = C>,
    C: IntoIterator<Item = S>,
    S: Into<String>,
{
    Element::new(ElementKind::Table {
        headers: headers.into_iter().map(Into::into).collect(),
        rows: rows
            .into_iter()
            .map(|r| r.into_iter().map(Into::into).collect())
            .collect(),
    })
}
/// Tabs are controlled choices: the application supplies selection and actions.
pub fn tabs<A>() -> Element<A> {
    Element::new(ElementKind::Tabs).gap(1).responsive(48)
}
pub fn modal<A>(title: impl Into<String>) -> Element<A> {
    Element::new(ElementKind::Modal(title.into())).elevation(Elevation::Overlay)
}
pub fn toast<A>(value: impl Into<String>) -> Element<A> {
    Element::new(ElementKind::Toast(value.into())).tone(Tone::Success)
}
/// EXPERIMENTAL (v0.9 design): a noninteractive lane that stacks toasts from the top-right of
/// the element it is mounted on. Mount it with [`Element::overlay`] on the element whose
/// rectangle the toasts must stay inside, so anything outside that host is never covered.
///
/// Pass the newest toast first. Each toast is drawn at its own height (3 rows by default, or
/// its `.height(...)`), so toasts never overlap. When they do not all fit, the newest ones are
/// drawn and a one-row "+N more" counts the rest; toasts are never dropped silently. The lane
/// is inert: it paints its toasts but adds no pointer or focus target of its own, and a toast
/// inside it is not a pointer target either. The application owns expiry.
/// See `docs/design/v0.9-toast-lane.md`. An ordinary [`toast`] is unchanged.
///
/// ```
/// use gibson::ui::prelude::*;
///
/// let lane: Element<()> = toast_lane([toast("saved"), toast("synced")]);
/// let host: Element<()> = column().overlay(lane);
/// # let _ = host;
/// ```
pub fn toast_lane<A>(toasts: impl IntoIterator<Item = Element<A>>) -> Element<A> {
    Element::new(ElementKind::ToastLane).children(toasts)
}
pub fn viewport<A>(x: i32, y: i32) -> Element<A> {
    Element::new(ElementKind::Viewport(x, y))
}
/// Imports a visual node subtree, preserving its layout and paint content.
///
/// Lowering clears every imported `Node::tag`, including hand-authored tags:
/// numeric target indices belong to their originating `Compiled`, not this host.
/// Bind actions on the semantic wrapper to obtain a fresh host target. To keep
/// an existing compiled tree interactive through a Scene transform, retain that
/// same `Compiled` and its sidecars rather than re-importing its node here.
pub fn raw<A>(node: Node) -> Element<A> {
    Element::new(ElementKind::Raw(node))
}
/// A custom ordinary node constructed during current-frame presentation.
///
/// Unlike [`Component::build`], this callback runs after complete-tree key and
/// focus reconciliation. It cannot add semantic children or action metadata;
/// bind those on the returned element. Its layout is preserved just like
/// [`raw`]. Semantic motion and `.post_process` are applied outside its node,
/// so do not apply the same `cx.motions` chain again inside the callback.
/// All hit tags on the returned node and its descendants are cleared before
/// assigning the semantic wrapper's own host tag, just as for [`raw`].
///
/// Determinism is the callback author's responsibility: use `cx.build.time` and
/// captured immutable data instead of reading a wall clock or mutable globals.
pub fn presented<A>(build: impl Fn(&PresentationCx) -> Node + 'static) -> Element<A> {
    Element::new(ElementKind::Presented(Arc::new(build)))
}
pub fn surface<A>(surface: Arc<Surface>) -> Element<A> {
    raw(Node::surface(surface))
}
pub fn raster<A>(surface: Surface) -> Element<A> {
    raw(Node::raster(surface))
}
