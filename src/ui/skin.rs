//! Complete, editable design grammars layered above the existing Theme palette.
//!
//! Resolution is pure and explicit: no environment variables or global terminal
//! state are read here. The compiler interprets Chrome structurally; it does not
//! merely paint different colors onto the same boxes.

use std::time::Duration;

use crate::{
    capability::{quantize_color, quantize_style, ColorDepth},
    glyph::SubcellGlyphMode,
    show, BorderType, Color, Line, Node, RichText, Span, Style, Theme, ThemeStyles,
};

use super::{
    motion::{MotionLanguage, MotionPlan, MotionPreference, MotionRole, MotionTokens},
    style::{Density, Emphasis, Tone},
};

/// An explicit snapshot of the environment used to build a semantic view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiEnvironment {
    pub width: u16,
    pub height: u16,
    pub color_depth: ColorDepth,
    pub glyph_mode: SubcellGlyphMode,
    pub motion: MotionPreference,
}

impl Default for UiEnvironment {
    fn default() -> Self {
        Self {
            width: 80,
            height: 24,
            color_depth: ColorDepth::TrueColor,
            glyph_mode: SubcellGlyphMode::Braille2x4,
            motion: MotionPreference::Full,
        }
    }
}

/// Structural container language, discharged into ordinary Node composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chrome {
    /// Title bars, raised frames, inset controls, and block shadows.
    Window,
    /// Sparse panels with instrumentation titles and vertical rails.
    Rail,
    /// Strong rules, open sections, and typographic hierarchy.
    Editorial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpacingTokens {
    pub compact_gap: u16,
    pub normal_gap: u16,
    pub spacious_gap: u16,
    pub compact_padding: u16,
    pub normal_padding: u16,
    pub spacious_padding: u16,
}

impl SpacingTokens {
    pub const fn resolve(self, density: Density) -> (u16, u16) {
        match density {
            Density::Compact => (self.compact_gap, self.compact_padding),
            Density::Normal => (self.normal_gap, self.normal_padding),
            Density::Spacious => (self.spacious_gap, self.spacious_padding),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypographyTokens {
    pub uppercase_titles: bool,
    pub numbered_sections: bool,
}

/// One-cell chrome markers. Application text is never transliterated.
/// Custom skin authors should keep each marker to one displayed cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphTokens {
    pub horizontal: &'static str,
    pub vertical: &'static str,
    pub rail: &'static str,
    pub focus: &'static str,
    pub selected: &'static str,
    pub bullet: &'static str,
    pub close: &'static str,
    pub neutral: &'static str,
    pub accent: &'static str,
    pub info: &'static str,
    pub success: &'static str,
    pub warning: &'static str,
    pub danger: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressTreatment {
    Segmented,
    Instrument,
    Rule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphTreatment {
    Blocks,
    Trace,
    Editorial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VizTokens {
    pub progress: ProgressTreatment,
    pub graph: GraphTreatment,
}

/// Presentation facts, independent of the action or selected domain object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ControlState {
    pub focused: bool,
    pub selected: bool,
    pub disabled: bool,
}

/// Related controls share interaction routing but retain distinct chrome roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlRole {
    Button,
    Choice,
    Tab,
}

/// Explicit colours a [`Skin`] uses at one reduced colour depth instead of the
/// nearest-colour mapping of its truecolor palette.
///
/// Nearest-colour mapping can merge colours a skin needs kept apart: a gold accent
/// and an amber warning both land on yellow at [`ColorDepth::Ansi16`]. A
/// `DepthPalette` lets the skin choose, for example, `BrightYellow` for accent and
/// `Red` for warning at that depth while keeping its RGB palette on truecolor
/// terminals. Colours already representable at the depth (base-16 colours at
/// Ansi16, base-16 or `Color::Ansi256` at Ansi256) are used verbatim; any other colour is
/// still mapped to its nearest representable colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DepthPalette {
    pub palette: Theme,
    pub surface: Color,
    /// Recessed instruments and editor surface.
    pub well: Color,
}

/// A complete design grammar. All fields are public for local customization.
/// Theme remains a palette; it gains no new rendering or application semantics.
///
/// To define a skin locally, start from a built-in one so new fields keep their
/// defaults: `Skin { name: "mine", palette, ..skins::BLACK_ICE }`. That copies the
/// base skin's `ansi16`/`ansi256` too; built-in skins carry `None` today, but a skin
/// with its own palette should set both explicitly (to `None` or its own
/// [`DepthPalette`]) so a later built-in override can never replace its colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Skin {
    pub name: &'static str,
    pub palette: Theme,
    pub surface: Color,
    /// Recessed instruments and editor surface.
    pub well: Color,
    /// Palette used at [`ColorDepth::Ansi16`] instead of nearest-colour mapping of
    /// `palette`/`surface`/`well`; `None` keeps the mapping. See [`DepthPalette`].
    pub ansi16: Option<DepthPalette>,
    /// Palette used at [`ColorDepth::Ansi256`]; `None` keeps nearest-colour mapping.
    pub ansi256: Option<DepthPalette>,
    pub chrome: Chrome,
    pub density: Density,
    pub spacing: SpacingTokens,
    pub typography: TypographyTokens,
    pub glyphs: GlyphTokens,
    pub ascii_glyphs: GlyphTokens,
    pub border_type: BorderType,
    pub title: Style,
    pub selection: Style,
    pub focus: Style,
    pub disabled: Style,
    pub shadow: Style,
    pub highlight: Style,
    pub lowlight: Style,
    pub motion: MotionTokens,
    pub viz: VizTokens,
}

impl Default for Skin {
    fn default() -> Self {
        skins::BLACK_ICE
    }
}

impl Skin {
    /// The palette, surface and well this skin starts from at `depth`, before
    /// quantization: the explicit [`DepthPalette`] for Ansi16/Ansi256 when one is
    /// set, otherwise the skin's own colours.
    pub fn palette_for(&self, depth: ColorDepth) -> DepthPalette {
        let explicit = match depth {
            ColorDepth::Ansi16 => self.ansi16,
            ColorDepth::Ansi256 => self.ansi256,
            _ => None,
        };
        explicit.unwrap_or(DepthPalette {
            palette: self.palette,
            surface: self.surface,
            well: self.well,
        })
    }

    /// Resolve palette, geometry, glyph vocabulary, and motion policy once for
    /// an environment. Resolved styles cooperate with renderer quantization.
    pub fn resolve(&self, environment: &UiEnvironment) -> ResolvedSkin {
        let depth = environment.color_depth;
        let color = |c| quantize_color(c, depth).unwrap_or(Color::Reset);
        let source = self.palette_for(depth);
        let palette = Theme {
            text: color(source.palette.text),
            text_muted: color(source.palette.text_muted),
            accent: color(source.palette.accent),
            success: color(source.palette.success),
            warning: color(source.palette.warning),
            error: color(source.palette.error),
            border: color(source.palette.border),
            rail: color(source.palette.rail),
            code: color(source.palette.code),
            bg: color(source.palette.bg),
        };
        let mut styles = palette.styles();
        let surface = color(source.surface);
        // A skin selects its background explicitly, so its muted text must also
        // select the palette's foreground rather than inherit terminal defaults.
        styles.muted = quantize_style(Style::new().fg(palette.text_muted), depth);
        if depth == ColorDepth::Mono {
            styles.muted = styles.muted.dim();
        }
        styles.faint = styles.muted.dim();
        // Text cells replace their destination cells in the substrate painter;
        // carry the skin surface explicitly instead of relying on parent fill.
        for style in [
            &mut styles.text,
            &mut styles.muted,
            &mut styles.faint,
            &mut styles.accent,
            &mut styles.success,
            &mut styles.warning,
            &mut styles.error,
            &mut styles.border,
            &mut styles.rail,
            &mut styles.code,
            &mut styles.link,
        ] {
            *style = quantize_style(style.bg(surface), depth);
        }
        let mono = depth == ColorDepth::Mono;
        let title = if mono {
            match self.chrome {
                Chrome::Window => Style::new().reverse().bold(),
                Chrome::Rail => Style::new().bold(),
                Chrome::Editorial => Style::new().bold().underline(),
            }
        } else {
            quantize_style(
                Style {
                    bg: self.title.bg.or(Some(surface)),
                    ..self.title
                },
                depth,
            )
        };
        let selection = if mono {
            match self.chrome {
                Chrome::Window | Chrome::Rail => Style::new().reverse().bold(),
                Chrome::Editorial => Style::new().bold(),
            }
        } else {
            quantize_style(
                Style {
                    bg: self.selection.bg.or(Some(surface)),
                    ..self.selection
                },
                depth,
            )
        };
        styles.selection = selection;
        let focus = if mono {
            Style::new().underline()
        } else {
            quantize_style(self.focus, depth)
        };
        let disabled = if mono {
            Style::new().dim()
        } else {
            quantize_style(self.disabled, depth)
        };
        let density = if environment.width < 48 || environment.height < 18 {
            Density::Compact
        } else {
            self.density
        };
        let (gap, padding) = self.spacing.resolve(density);
        let ascii = environment.glyph_mode == SubcellGlyphMode::Ascii;
        ResolvedSkin {
            name: self.name,
            palette,
            styles,
            chrome: self.chrome,
            background: palette.bg,
            surface,
            well: color(source.well),
            title,
            border: if mono && self.chrome == Chrome::Window {
                Style::new().bold()
            } else {
                styles.border
            },
            selection,
            focus,
            disabled,
            shadow: if mono {
                Style::new().reverse()
            } else {
                quantize_style(self.shadow, depth)
            },
            highlight: if mono {
                Style::new().bold()
            } else {
                quantize_style(self.highlight.bg(surface), depth)
            },
            lowlight: if mono {
                Style::new().dim()
            } else {
                quantize_style(self.lowlight.bg(surface), depth)
            },
            density,
            gap,
            padding,
            spacing: self.spacing,
            typography: self.typography,
            glyphs: if ascii {
                self.ascii_glyphs
            } else {
                self.glyphs
            },
            border_type: if ascii {
                BorderType::Ascii
            } else {
                self.border_type
            },
            motion: self.motion,
            viz: self.viz,
            environment: *environment,
        }
    }
}

/// Whether two resolved colours select the same terminal colour: equal, or a base-16
/// colour and the `Ansi256` index of the same palette slot.
fn same_colour(a: Option<Color>, b: Option<Color>) -> bool {
    const BASE16: [Color; 16] = [
        Color::Black,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::White,
        Color::BrightBlack,
        Color::BrightRed,
        Color::BrightGreen,
        Color::BrightYellow,
        Color::BrightBlue,
        Color::BrightMagenta,
        Color::BrightCyan,
        Color::BrightWhite,
    ];
    let slot = |c: Color| match c {
        Color::Ansi256(n) if n < 16 => BASE16[n as usize],
        other => other,
    };
    a.map(slot) == b.map(slot)
}

/// Inspectable, capability-resolved tokens consumed by semantic lowering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedSkin {
    pub name: &'static str,
    pub palette: Theme,
    pub styles: ThemeStyles,
    pub chrome: Chrome,
    pub background: Color,
    pub surface: Color,
    pub well: Color,
    pub title: Style,
    pub border: Style,
    pub selection: Style,
    pub focus: Style,
    pub disabled: Style,
    pub shadow: Style,
    pub highlight: Style,
    pub lowlight: Style,
    pub density: Density,
    pub gap: u16,
    pub padding: u16,
    pub spacing: SpacingTokens,
    pub typography: TypographyTokens,
    pub glyphs: GlyphTokens,
    pub border_type: BorderType,
    pub motion: MotionTokens,
    pub viz: VizTokens,
    pub environment: UiEnvironment,
}

impl ResolvedSkin {
    /// Pairs of tones whose resolved foreground **and** background colours are
    /// identical at this skin's resolved colour depth (at [`Emphasis::Normal`]), in
    /// declaration order. Attributes are deliberately ignored: a bold accent and a
    /// plain warning in the same yellow still read as the same colour. A base-16
    /// colour and the 256-colour index naming the same palette slot (`Color::Red` and
    /// `Color::Ansi256(1)`) count as the same colour. A skin test can assert that tones a user
    /// must tell apart, such as `Warning` and `Accent`, never collide:
    ///
    /// ```
    /// use gibson::capability::ColorDepth;
    /// use gibson::ui::prelude::*;
    /// for depth in [ColorDepth::Ansi16, ColorDepth::Ansi256, ColorDepth::TrueColor] {
    ///     let env = UiEnvironment { color_depth: depth, ..UiEnvironment::default() };
    ///     let resolved = skins::BLACK_ICE.resolve(&env);
    ///     assert!(!resolved.tone_collisions().contains(&(Tone::Warning, Tone::Danger)));
    /// }
    /// ```
    ///
    /// At [`ColorDepth::Mono`] no colour is emitted, so colour tones collide by
    /// construction; tone is carried there by status markers and attributes.
    pub fn tone_collisions(&self) -> Vec<(Tone, Tone)> {
        const TONES: [Tone; 6] = [
            Tone::Neutral,
            Tone::Accent,
            Tone::Info,
            Tone::Success,
            Tone::Warning,
            Tone::Danger,
        ];
        let mut out = Vec::new();
        for (i, &a) in TONES.iter().enumerate() {
            for &b in &TONES[i + 1..] {
                let (sa, sb) = (
                    self.style(a, Emphasis::Normal),
                    self.style(b, Emphasis::Normal),
                );
                if same_colour(sa.fg, sb.fg) && same_colour(sa.bg, sb.bg) {
                    out.push((a, b));
                }
            }
        }
        out
    }

    pub fn style(&self, tone: Tone, emphasis: Emphasis) -> Style {
        let style = match tone {
            Tone::Neutral => match emphasis {
                Emphasis::Muted => self.styles.muted,
                Emphasis::Faint => self.styles.faint,
                _ => self.styles.text,
            },
            Tone::Accent => self.styles.accent,
            Tone::Info => self.styles.rail,
            Tone::Success => self.styles.success,
            Tone::Warning => self.styles.warning,
            Tone::Danger => self.styles.error,
        };
        match emphasis {
            Emphasis::Faint => style.dim(),
            Emphasis::Muted if self.environment.color_depth == ColorDepth::Mono => style.dim(),
            Emphasis::Muted => style,
            Emphasis::Normal => style,
            Emphasis::Strong => style.bold(),
        }
    }

    pub fn status_marker(&self, tone: Tone) -> &'static str {
        match tone {
            Tone::Neutral => self.glyphs.neutral,
            Tone::Accent => self.glyphs.accent,
            Tone::Info => self.glyphs.info,
            Tone::Success => self.glyphs.success,
            Tone::Warning => self.glyphs.warning,
            Tone::Danger => self.glyphs.danger,
        }
    }

    pub fn button_label(&self, label: &str, focused: bool, selected: bool) -> String {
        self.control_line(
            label,
            ControlRole::Button,
            ControlState {
                focused,
                selected,
                disabled: false,
            },
            self.styles.text,
        )
        .plain_text()
    }

    /// Focus and selection are independent attributes. Disabled chrome has
    /// precedence without erasing the application's selected marker.
    pub fn control_style(&self, base: Style, state: ControlState) -> Style {
        if state.disabled {
            return Style {
                fg: self.disabled.fg.or(base.fg),
                bg: self.disabled.bg.or(base.bg),
                dim: true,
                ..self.disabled
            };
        }
        let selected = if state.selected {
            base.overlay(self.selection)
        } else {
            base
        };
        if state.focused {
            selected.overlay(self.focus)
        } else {
            selected
        }
    }

    /// An inspectable ordinary rich line, also useful inside custom components.
    /// The focus slot and selected slot are separate even in monochrome.
    pub fn control_line(
        &self,
        label: &str,
        role: ControlRole,
        state: ControlState,
        base: Style,
    ) -> Line {
        let style = self.control_style(base, state);
        let focus = if state.disabled {
            "x"
        } else if state.focused {
            self.glyphs.focus
        } else {
            " "
        };
        let selected = if state.selected {
            self.glyphs.selected
        } else {
            " "
        };
        let marker_style = if state.disabled {
            style
        } else if state.focused {
            base.overlay(self.focus).bold()
        } else {
            base
        };
        let (open, close) = match (self.chrome, role) {
            (Chrome::Window, ControlRole::Tab)
                if self.environment.glyph_mode == SubcellGlyphMode::Ascii =>
            {
                if state.selected {
                    ("[", "]")
                } else {
                    ("/", "\\")
                }
            }
            (Chrome::Window, ControlRole::Tab) => {
                if state.selected {
                    ("▏", "▕")
                } else {
                    ("╭", "╮")
                }
            }
            (Chrome::Window, _) | (Chrome::Rail, ControlRole::Button) => ("[", "]"),
            _ => ("", ""),
        };
        let (light, dark) = if state.disabled {
            (style, style)
        } else if state.selected && self.chrome == Chrome::Window {
            (self.lowlight, self.highlight)
        } else {
            (self.highlight, self.lowlight)
        };
        Line::from_spans(vec![
            Span::styled(focus, marker_style),
            Span::styled(open, light),
            Span::styled(
                selected,
                if state.disabled {
                    style
                } else if state.selected {
                    base.overlay(self.selection)
                } else {
                    base
                },
            ),
            Span::styled(format!("{label} "), style),
            Span::styled(close, dark),
        ])
    }

    /// Single-row control chrome; layout and painting remain ordinary Node work.
    pub fn control(
        &self,
        label: &str,
        role: ControlRole,
        state: ControlState,
        style: Style,
    ) -> Node {
        Node::rich_text_wrapped(
            RichText::from_lines(vec![self.control_line(label, role, state, style)]),
            crate::WrapMode::NoWrap,
        )
        .height(1.0)
    }

    /// Inset rails around one editor/instrument row. This never post-processes
    /// the child, so a focused TextInput retains its hardware cursor.
    pub(crate) fn instrument_well(&self, mut content: Node, focused: bool) -> Node {
        let edge = if self.environment.glyph_mode == SubcellGlyphMode::Ascii {
            "|"
        } else {
            "▏"
        };
        let far = if self.environment.glyph_mode == SubcellGlyphMode::Ascii {
            "|"
        } else {
            "▕"
        };
        content.layout_style.width = crate::Dimension::Length(0.0);
        Node::row()
            .background(self.well)
            .height(1.0)
            .child(
                Node::text(edge, self.lowlight)
                    .width(1.0)
                    .height(1.0)
                    .flex_shrink(0.0),
            )
            .child(content.flex_grow(1.0).min_width(0.0))
            .child(
                Node::text(
                    far,
                    if focused {
                        self.highlight.overlay(self.focus)
                    } else {
                        self.highlight
                    },
                )
                .width(1.0)
                .height(1.0)
                .flex_shrink(0.0),
            )
    }

    pub fn motion(&self, role: MotionRole) -> MotionPlan {
        let tone = match role {
            MotionRole::Success => Tone::Success,
            MotionRole::Warning => Tone::Warning,
            MotionRole::Error => Tone::Danger,
            _ => Tone::Accent,
        };
        self.motion.resolve(
            role,
            self.environment.motion,
            self.style(tone, Emphasis::Strong),
        )
    }

    /// Two rows: label/percentage, then a skin-specific meter. Sampling and
    /// clamping are delegated to `show::progress`; this only realizes its spans.
    pub fn progress(&self, label: &str, fraction: f32, width: u16) -> Node {
        let fraction = unit(fraction);
        // Formatted exactly as before (`{:>3.0}` rounds half to even), except that
        // an unfinished job never reads 100%.
        let mut readout = format!("{:>3.0}%", fraction * 100.0);
        if fraction < 1.0 && readout == "100%" {
            readout = " 99%".to_string();
        }
        self.progress_node(
            label,
            readout,
            |inner| {
                show::progress_spans_eighths(
                    (fraction * f32::from(inner) * 8.0).round() as usize,
                    usize::from(inner),
                    self.palette.accent,
                    self.palette.success,
                    self.palette.border,
                )
            },
            width,
            true,
        )
    }

    /// Discrete progress: `label done/total`, a meter rounded *down* to the
    /// nearest eighth of a cell (never ahead of `done / total`), and no meter at
    /// all when monochrome or 40 columns or narrower. See [`super::progress_steps`].
    pub fn progress_steps(&self, label: &str, done: u32, total: u32, width: u16) -> Node {
        let done = done.min(total);
        let meter = self.environment.color_depth != ColorDepth::Mono
            && width.min(self.environment.width) > 40;
        self.progress_node(
            label,
            format!("{done}/{total}"),
            |inner| {
                let eighths = if total == 0 {
                    0
                } else {
                    (u64::from(done) * u64::from(inner) * 8 / u64::from(total)) as usize
                };
                show::progress_spans_eighths(
                    eighths,
                    usize::from(inner),
                    self.palette.accent,
                    self.palette.success,
                    self.palette.border,
                )
            },
            width,
            meter,
        )
    }

    fn progress_node(
        &self,
        label: &str,
        readout: String,
        spans: impl FnOnce(u16) -> Vec<Span>,
        width: u16,
        with_meter: bool,
    ) -> Node {
        let width = width.min(self.environment.width);
        if width == 0 {
            return Node::col().width(0.0).height(0.0);
        }
        let inset = self.chrome == Chrome::Window && width > 2;
        let inner_width = if inset { width - 2 } else { width };
        let mut meter = Line::from_spans(spans(inner_width));
        let ascii = self.environment.glyph_mode == SubcellGlyphMode::Ascii;
        for span in &mut meter.spans {
            let glyph = span.text.as_str();
            let full = glyph == "█";
            let empty = glyph == "─";
            let replacement = match (self.viz.progress, ascii, full, empty) {
                (ProgressTreatment::Segmented, true, true, _) => "#",
                (ProgressTreatment::Segmented, true, _, true) => ".",
                (ProgressTreatment::Segmented, true, _, _) => "+",
                (ProgressTreatment::Segmented, false, true, _) => "▓",
                (ProgressTreatment::Segmented, false, _, true) => "░",
                (ProgressTreatment::Segmented, false, _, _) => "▒",
                (ProgressTreatment::Instrument, true, true, _) => "=",
                (ProgressTreatment::Instrument, true, _, true) => ".",
                (ProgressTreatment::Instrument, true, _, _) => ">",
                (ProgressTreatment::Instrument, false, true, _) => "━",
                (ProgressTreatment::Instrument, false, _, true) => "·",
                (ProgressTreatment::Instrument, false, _, _) => "╸",
                (ProgressTreatment::Rule, true, true, _) => "=",
                (ProgressTreatment::Rule, true, _, true) => "-",
                (ProgressTreatment::Rule, true, _, _) => ">",
                (ProgressTreatment::Rule, false, true, _) => "━",
                (ProgressTreatment::Rule, false, _, true) => "─",
                (ProgressTreatment::Rule, false, _, _) => "╸",
            };
            span.text = replacement.into();
            span.style = quantize_style(
                span.style.bg(if inset { self.well } else { self.surface }),
                self.environment.color_depth,
            );
        }
        let heading = Line::from_spans(vec![
            Span::styled(
                format!("{label} "),
                self.style(Tone::Neutral, Emphasis::Muted),
            ),
            Span::styled(readout, self.styles.accent),
        ]);
        if !with_meter {
            return Node::rich_text_wrapped(
                RichText::from_lines(vec![heading]),
                crate::WrapMode::NoWrap,
            )
            .width(f32::from(width))
            .height(1.0);
        }
        let meter =
            Node::rich_text_wrapped(RichText::from_lines(vec![meter]), crate::WrapMode::NoWrap)
                .height(1.0);
        Node::col()
            .width(f32::from(width))
            .height(2.0)
            .child(
                Node::rich_text_wrapped(
                    RichText::from_lines(vec![heading]),
                    crate::WrapMode::NoWrap,
                )
                .height(1.0),
            )
            .child(if inset {
                self.instrument_well(meter, false)
            } else {
                meter
            })
    }

    /// Half-block realization of fitted pixels in a box exactly `rows` tall; the
    /// alt text instead when monochrome, the box is under 20 × 6 or there are no
    /// pixels. See [`super::image`].
    pub(crate) fn image(
        &self,
        fitted: &super::image::Fitted,
        rows: u16,
        alt: &str,
        width: u16,
    ) -> Node {
        let width = width.min(self.environment.width);
        let frame = Node::col().width(f32::from(width)).height(f32::from(rows));
        let pixels = fitted.refit(width, rows.saturating_mul(2));
        if self.environment.color_depth == ColorDepth::Mono
            || width < 20
            || rows < 6
            || pixels.width == 0
        {
            if rows == 0 {
                return frame;
            }
            let alt: String = alt
                .split(['\r', '\n'])
                .next()
                .unwrap_or("")
                .chars()
                .filter(|c| !c.is_control())
                .collect();
            let line = Line::from_spans(vec![Span::styled(
                alt,
                self.style(Tone::Neutral, Emphasis::Muted),
            )]);
            return frame.child(
                Node::rich_text_wrapped(RichText::from_lines(vec![line]), crate::WrapMode::NoWrap)
                    .height(1.0),
            );
        }
        let depth = self.environment.color_depth;
        let w = usize::from(pixels.width);
        let lines: Vec<Line> = pixels
            .pixels
            .chunks(w * 2)
            .map(|pair| {
                let (top, bottom) = pair.split_at(w.min(pair.len()));
                Line::from_spans(
                    top.iter()
                        .enumerate()
                        .map(|(x, &(r, g, b))| {
                            let mut style = Style::new().fg(Color::Rgb(r, g, b));
                            style = match bottom.get(x) {
                                Some(&(r, g, b)) => style.bg(Color::Rgb(r, g, b)),
                                None => style.bg(self.background),
                            };
                            Span::styled("▀", quantize_style(style, depth))
                        })
                        .collect(),
                )
            })
            .collect();
        frame.child(
            Node::rich_text_wrapped(RichText::from_lines(lines), crate::WrapMode::NoWrap)
                .height(f32::from(pixels.height.div_ceil(2))),
        )
    }

    /// One row of `show::sparkline` samples with capability-safe realization.
    pub fn sparkline(&self, values: &[f32], width: u16) -> Node {
        let width = width.min(self.environment.width);
        if width == 0 {
            return Node::col().width(0.0).height(0.0);
        }
        let clean: Vec<f32> = values
            .iter()
            .map(|v| if v.is_finite() { v.max(0.0) } else { 0.0 })
            .collect();
        let inset = self.chrome == Chrome::Window && width > 2;
        let inner_width = if inset { width - 2 } else { width };
        let mut line = show::sparkline(
            &clean,
            usize::from(inner_width),
            self.palette.accent,
            self.palette.success,
        );
        let ascii = self.environment.glyph_mode == SubcellGlyphMode::Ascii;
        const LEVELS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
        const BLOCKS: [&str; 8] = [".", ":", "-", "=", "+", "*", "#", "@"];
        const TRACE: [&str; 8] = ["_", ".", ":", "-", "=", "+", "*", "#"];
        const EDITORIAL: [&str; 8] = [".", ".", ":", ":", "|", "|", "#", "#"];
        for span in &mut line.spans {
            let level = LEVELS
                .iter()
                .position(|s| *s == span.text.as_str())
                .unwrap_or(0);
            if ascii {
                span.text = match self.viz.graph {
                    GraphTreatment::Blocks => BLOCKS[level],
                    GraphTreatment::Trace => TRACE[level],
                    GraphTreatment::Editorial => EDITORIAL[level],
                }
                .into();
            }
            if self.viz.graph == GraphTreatment::Editorial {
                span.style = self.styles.text;
            }
            span.style = quantize_style(
                span.style.bg(if inset { self.well } else { self.surface }),
                self.environment.color_depth,
            );
        }
        let graph =
            Node::rich_text_wrapped(RichText::from_lines(vec![line]), crate::WrapMode::NoWrap)
                .height(1.0);
        (if inset {
            self.instrument_well(graph, false)
        } else {
            graph
        })
        .width(f32::from(width))
    }
}

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Built-in starting points. Copy a constant and edit its public tokens to
/// author a variant; existing Theme consumers are unaffected.
pub mod skins {
    use super::*;

    const SPACING: SpacingTokens = SpacingTokens {
        compact_gap: 0,
        normal_gap: 1,
        spacious_gap: 1,
        compact_padding: 0,
        normal_padding: 1,
        spacious_padding: 2,
    };
    const ASCII: GlyphTokens = GlyphTokens {
        horizontal: "-",
        vertical: "|",
        rail: "|",
        focus: ">",
        selected: "*",
        bullet: "-",
        close: "x",
        neutral: "-",
        accent: ">",
        info: "i",
        success: "+",
        warning: "!",
        danger: "x",
    };

    /// Raised diagnostic windows, plum title bars, segmented meters.
    pub const VAPOR95: Skin = Skin {
        name: "Vapor95",
        ansi16: None,
        ansi256: None,
        palette: Theme {
            text: Color::Rgb(38, 29, 49),
            text_muted: Color::Rgb(91, 71, 105),
            accent: Color::Rgb(115, 38, 163),
            success: Color::Rgb(0, 103, 96),
            warning: Color::Rgb(122, 75, 0),
            error: Color::Rgb(171, 20, 69),
            border: Color::Rgb(100, 78, 122),
            rail: Color::Rgb(42, 152, 161),
            code: Color::Rgb(100, 31, 132),
            bg: Color::Rgb(61, 35, 79),
        },
        surface: Color::Rgb(207, 196, 217),
        well: Color::Rgb(236, 227, 242),
        chrome: Chrome::Window,
        density: Density::Normal,
        spacing: SPACING,
        typography: TypographyTokens {
            uppercase_titles: false,
            numbered_sections: false,
        },
        glyphs: GlyphTokens {
            horizontal: "═",
            vertical: "║",
            rail: "▌",
            focus: "▪",
            selected: "■",
            bullet: "▪",
            close: "×",
            neutral: "□",
            accent: "◇",
            info: "i",
            success: "+",
            warning: "!",
            danger: "×",
        },
        ascii_glyphs: GlyphTokens {
            horizontal: "=",
            rail: "#",
            focus: "=",
            selected: "#",
            ..ASCII
        },
        border_type: BorderType::Double,
        title: Style::new()
            .fg(Color::Rgb(241, 242, 255))
            .bg(Color::Rgb(92, 36, 137))
            .bold(),
        selection: Style::new()
            .fg(Color::Rgb(24, 40, 65))
            .bg(Color::Rgb(94, 222, 226))
            .bold(),
        focus: Style::new().underline(),
        disabled: Style::new().fg(Color::Rgb(118, 108, 127)).dim(),
        shadow: Style::new()
            .fg(Color::Rgb(31, 17, 45))
            .bg(Color::Rgb(31, 17, 45)),
        highlight: Style::new().fg(Color::Rgb(248, 236, 255)),
        lowlight: Style::new().fg(Color::Rgb(83, 63, 103)),
        motion: MotionTokens {
            language: MotionLanguage::Physical,
            enter: Duration::from_millis(150),
            exit: Duration::from_millis(110),
            focus: Duration::from_millis(80),
            activate: Duration::from_millis(90),
            signal: Duration::from_millis(160),
            modal: Duration::from_millis(190),
        },
        viz: VizTokens {
            progress: ProgressTreatment::Segmented,
            graph: GraphTreatment::Blocks,
        },
    };

    /// Near-black instrumentation, sparse rails, acquisition motion.
    pub const BLACK_ICE: Skin = Skin {
        name: "Black Ice",
        ansi16: None,
        ansi256: None,
        palette: Theme {
            text: Color::Rgb(222, 240, 242),
            text_muted: Color::Rgb(109, 139, 145),
            accent: Color::Rgb(61, 221, 231),
            success: Color::Rgb(91, 225, 145),
            warning: Color::Rgb(246, 190, 89),
            error: Color::Rgb(255, 94, 141),
            border: Color::Rgb(49, 93, 103),
            rail: Color::Rgb(61, 221, 231),
            code: Color::Rgb(175, 156, 230),
            bg: Color::Rgb(8, 14, 19),
        },
        surface: Color::Rgb(12, 22, 29),
        well: Color::Rgb(7, 15, 20),
        chrome: Chrome::Rail,
        density: Density::Compact,
        spacing: SPACING,
        typography: TypographyTokens {
            uppercase_titles: true,
            numbered_sections: false,
        },
        glyphs: GlyphTokens {
            horizontal: "─",
            vertical: "│",
            rail: "┃",
            focus: "▸",
            selected: "◆",
            bullet: "·",
            close: "×",
            neutral: "·",
            accent: "▸",
            info: "◇",
            success: "+",
            warning: "!",
            danger: "×",
        },
        ascii_glyphs: ASCII,
        border_type: BorderType::Single,
        title: Style::new().fg(Color::Rgb(61, 221, 231)).bold(),
        selection: Style::new()
            .fg(Color::Rgb(224, 252, 249))
            .bg(Color::Rgb(22, 72, 81))
            .bold(),
        focus: Style::new().underline(),
        disabled: Style::new().fg(Color::Rgb(91, 113, 120)).dim(),
        shadow: Style::new().fg(Color::Black).bg(Color::Black),
        highlight: Style::new().fg(Color::Rgb(91, 225, 145)),
        lowlight: Style::new().fg(Color::Rgb(49, 93, 103)),
        motion: MotionTokens {
            language: MotionLanguage::Acquisition,
            enter: Duration::from_millis(160),
            exit: Duration::from_millis(100),
            focus: Duration::from_millis(100),
            activate: Duration::from_millis(90),
            signal: Duration::from_millis(140),
            modal: Duration::from_millis(210),
        },
        viz: VizTokens {
            progress: ProgressTreatment::Instrument,
            graph: GraphTreatment::Trace,
        },
    };

    /// Open editorial sections, ivory surfaces, cobalt rules, restrained motion.
    pub const SWISS_SIGNAL: Skin = Skin {
        name: "Swiss Signal",
        ansi16: None,
        ansi256: None,
        palette: Theme {
            text: Color::Rgb(30, 32, 35),
            text_muted: Color::Rgb(99, 102, 105),
            accent: Color::Rgb(26, 63, 188),
            success: Color::Rgb(24, 104, 70),
            warning: Color::Rgb(163, 79, 0),
            error: Color::Rgb(198, 45, 29),
            border: Color::Rgb(48, 50, 53),
            rail: Color::Rgb(26, 63, 188),
            code: Color::Rgb(77, 57, 94),
            bg: Color::Rgb(245, 243, 234),
        },
        surface: Color::Rgb(245, 243, 234),
        well: Color::Rgb(245, 243, 234),
        chrome: Chrome::Editorial,
        density: Density::Spacious,
        spacing: SPACING,
        typography: TypographyTokens {
            uppercase_titles: true,
            numbered_sections: true,
        },
        glyphs: GlyphTokens {
            horizontal: "━",
            vertical: "│",
            rail: "│",
            focus: "→",
            selected: "●",
            bullet: "—",
            close: "×",
            neutral: "—",
            accent: "→",
            info: "i",
            success: "+",
            warning: "!",
            danger: "×",
        },
        ascii_glyphs: GlyphTokens {
            horizontal: "=",
            selected: "+",
            ..ASCII
        },
        border_type: BorderType::Thick,
        title: Style::new().fg(Color::Rgb(30, 32, 35)).bold(),
        selection: Style::new().fg(Color::Rgb(26, 63, 188)).bold(),
        focus: Style::new().underline(),
        disabled: Style::new().fg(Color::Rgb(133, 133, 126)).dim(),
        shadow: Style::new()
            .fg(Color::Rgb(191, 190, 181))
            .bg(Color::Rgb(191, 190, 181)),
        highlight: Style::new().fg(Color::Rgb(26, 63, 188)).bold(),
        lowlight: Style::new().fg(Color::Rgb(99, 102, 105)),
        motion: MotionTokens {
            language: MotionLanguage::Editorial,
            enter: Duration::from_millis(140),
            exit: Duration::from_millis(90),
            focus: Duration::from_millis(90),
            activate: Duration::from_millis(60),
            signal: Duration::from_millis(100),
            modal: Duration::from_millis(180),
        },
        viz: VizTokens {
            progress: ProgressTreatment::Rule,
            graph: GraphTreatment::Editorial,
        },
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn muted_color_is_not_dimmed_twice_and_empty_selection_slots_are_neutral() {
        for depth in [
            ColorDepth::TrueColor,
            ColorDepth::Ansi256,
            ColorDepth::Ansi16,
        ] {
            let env = UiEnvironment {
                color_depth: depth,
                ..UiEnvironment::default()
            };
            for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
                let resolved = skin.resolve(&env);
                assert!(!resolved.style(Tone::Neutral, Emphasis::Muted).dim);
                assert!(resolved.style(Tone::Neutral, Emphasis::Faint).dim);
                if resolved.selection.bg != resolved.styles.text.bg {
                    let line = resolved.control_line(
                        "Run",
                        ControlRole::Button,
                        ControlState::default(),
                        resolved.styles.text,
                    );
                    assert!(
                        line.spans
                            .iter()
                            .all(|span| span.style.bg != resolved.selection.bg),
                        "inactive {} control painted selection into its blank marker",
                        skin.name
                    );
                }
            }
        }
    }

    #[test]
    fn mono_retains_structural_and_attribute_identity() {
        let env = UiEnvironment {
            color_depth: ColorDepth::Mono,
            ..UiEnvironment::default()
        };
        let vapor = skins::VAPOR95.resolve(&env);
        let ice = skins::BLACK_ICE.resolve(&env);
        let swiss = skins::SWISS_SIGNAL.resolve(&env);
        assert_eq!(vapor.palette, Theme::no_color());
        assert_eq!(ice.palette, Theme::no_color());
        assert_eq!(swiss.palette, Theme::no_color());
        assert_ne!(vapor.chrome, ice.chrome);
        assert_ne!(ice.chrome, swiss.chrome);
        assert!(vapor.title.reverse);
        assert!(swiss.title.underline);
        assert_ne!(
            vapor.button_label("Run", true, false),
            ice.button_label("Run", true, false)
        );
        assert_ne!(
            ice.button_label("Run", true, false),
            swiss.button_label("Run", true, false)
        );
    }

    #[test]
    fn ascii_resolution_and_narrow_spacing_are_explicit() {
        let env = UiEnvironment {
            width: 28,
            height: 10,
            glyph_mode: SubcellGlyphMode::Ascii,
            ..UiEnvironment::default()
        };
        for skin in [skins::VAPOR95, skins::BLACK_ICE, skins::SWISS_SIGNAL] {
            let resolved = skin.resolve(&env);
            assert_eq!(resolved.density, Density::Compact);
            assert_eq!(resolved.border_type, BorderType::Ascii);
            for tone in [
                Tone::Neutral,
                Tone::Accent,
                Tone::Info,
                Tone::Success,
                Tone::Warning,
                Tone::Danger,
            ] {
                assert!(resolved.status_marker(tone).is_ascii());
            }
            assert!(resolved.button_label("Run", true, true).is_ascii());
        }
    }

    #[test]
    fn low_color_styles_contain_no_rgb_and_tokens_are_editable() {
        let env = UiEnvironment {
            color_depth: ColorDepth::Ansi16,
            ..UiEnvironment::default()
        };
        let mut custom = skins::SWISS_SIGNAL;
        custom.name = "Custom editorial";
        custom.spacing.spacious_gap = 3;
        let resolved = custom.resolve(&env);
        assert_eq!(resolved.gap, 3);
        for tone in [Tone::Neutral, Tone::Accent, Tone::Success, Tone::Danger] {
            let style = resolved.style(tone, Emphasis::Strong);
            assert!(!matches!(
                style.fg,
                Some(Color::Rgb(..) | Color::Ansi256(_))
            ));
            assert!(style.bold);
        }
    }
}
