//! Semantic intent and ordinary Rust builders. No renderer or terminal ownership.
use super::compile::{BuildCx, PresentationCx};
use super::motion::MotionRole;
use super::style::{Density, Elevation, Emphasis, Tone};
use crate::{Event, Node, Surface, SurfaceFx, TextInputState};
use std::fmt;
use std::sync::Arc;

/// Explicit keys survive reordering. Unkeyed elements receive positional paths.
/// Named and positional identities have separate namespaces.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    Named(String),
    Path(Vec<usize>),
}
impl Key {
    pub fn named(value: impl ToString) -> Self {
        Self::Named(value.to_string())
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
    Input {
        state: TextInputState,
        placeholder: Option<String>,
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
    pub(crate) on_event: Option<Arc<dyn Fn(Event) -> A>>,
    pub(crate) on_dismiss: Option<A>,
    pub(crate) selected: bool,
    pub(crate) disabled: bool,
    pub(crate) revision: u64,
    pub(crate) motion: Option<MotionRole>,
    pub(crate) tone: Tone,
    pub(crate) emphasis: Emphasis,
    pub(crate) density: Option<Density>,
    pub(crate) elevation: Elevation,
    pub(crate) number: Option<u16>,
    pub(crate) layout: Layout,
    pub(crate) effects: Vec<SurfaceFx>,
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
            on_event: None,
            on_dismiss: None,
            selected: false,
            disabled: false,
            revision: 0,
            motion: None,
            tone: Tone::Neutral,
            emphasis: Emphasis::Normal,
            density: None,
            elevation: Elevation::Flat,
            number: None,
            layout: Layout::default(),
            effects: vec![],
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
    })
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
pub fn viewport<A>(x: i32, y: i32) -> Element<A> {
    Element::new(ElementKind::Viewport(x, y))
}
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
