use compact_str::CompactString;
use unicode_width::UnicodeWidthStr;

/// Terminal color representations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Color {
    #[default]
    Reset,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    Ansi256(u8),
    Rgb(u8, u8, u8),
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color::Rgb(r, g, b)
    }

    pub const fn ansi(n: u8) -> Self {
        Color::Ansi256(n)
    }

    /// Approximate RGB triple for this color. 16-color and 256-color values are
    /// mapped onto the xterm palette so gradients work across color depths.
    ///
    /// `Color::Reset` has **no** defined RGB value; it is approximated as
    /// `(204, 204, 204)` for display/legacy interpolation. Use
    /// [`Color::resolve_rgb`] when that approximation would be a lie.
    pub fn to_rgb(self) -> (u8, u8, u8) {
        match self {
            Color::Reset => (204, 204, 204),
            Color::Black => (0, 0, 0),
            Color::Red => (205, 49, 49),
            Color::Green => (13, 188, 121),
            Color::Yellow => (229, 229, 16),
            Color::Blue => (36, 114, 200),
            Color::Magenta => (188, 63, 188),
            Color::Cyan => (17, 168, 205),
            Color::White => (229, 229, 229),
            Color::BrightBlack => (102, 102, 102),
            Color::BrightRed => (241, 76, 76),
            Color::BrightGreen => (35, 209, 139),
            Color::BrightYellow => (245, 245, 67),
            Color::BrightBlue => (59, 142, 234),
            Color::BrightMagenta => (214, 112, 214),
            Color::BrightCyan => (41, 184, 219),
            Color::BrightWhite => (255, 255, 255),
            Color::Ansi256(n) => xterm_rgb(n),
            Color::Rgb(r, g, b) => (r, g, b),
        }
    }

    /// Linear interpolation toward `other` by `t` in `[0, 1]`, returning truecolor.
    ///
    /// Useful for gradients, heat maps and animated color sweeps.
    ///
    /// **Note:** if either endpoint is [`Color::Reset`] this interpolates through
    /// the arbitrary `(204, 204, 204)` approximation. If that is not what you
    /// want, use [`Color::lerp_resolved`] and handle `None`.
    pub fn lerp(self, other: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let (r1, g1, b1) = self.to_rgb();
        let (r2, g2, b2) = other.to_rgb();
        let mix = |a: u8, b: u8| ((a as f32) + (b as f32 - a as f32) * t).round() as u8;
        Color::Rgb(mix(r1, r2), mix(g1, g2), mix(b1, b2))
    }

    /// RGB triple, or `None` for [`Color::Reset`] ("terminal default"), which has
    /// no defined numeric value.
    pub fn resolve_rgb(self) -> Option<(u8, u8, u8)> {
        if self == Color::Reset {
            None
        } else {
            Some(self.to_rgb())
        }
    }

    /// Like [`Color::lerp`], but returns `None` when either endpoint is
    /// [`Color::Reset`], so callers do not silently interpolate a fake gray.
    pub fn lerp_resolved(self, other: Color, t: f32) -> Option<Color> {
        let a = self.resolve_rgb()?;
        let b = other.resolve_rgb()?;
        let t = t.clamp(0.0, 1.0);
        let mix = |x: u8, y: u8| ((x as f32) + (y as f32 - x as f32) * t).round() as u8;
        Some(Color::Rgb(mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2)))
    }
}

/// Maps an xterm-256 index to an approximate RGB triple.
pub(crate) fn xterm_rgb(n: u8) -> (u8, u8, u8) {
    match n {
        0..=15 => {
            // Reuse the 16 base colors.
            let base = [
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
            // Avoid infinite recursion: map base colors directly.
            match base[(n & 0x0f) as usize] {
                Color::Ansi256(_) => (204, 204, 204),
                c => {
                    let (r, g, b) = c.to_rgb();
                    (r, g, b)
                }
            }
        }
        16..=231 => {
            let n = n - 16;
            let r = n / 36;
            let g = (n % 36) / 6;
            let b = n % 6;
            let axis = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            (axis(r), axis(g), axis(b))
        }
        _ => {
            let v = 8 + 10 * (n - 232);
            (v, v, v)
        }
    }
}

/// Design tokens defining an application-wide or component-level visual palette.
/// Default theme preserves transparent terminal backgrounds (Color::Reset)
/// to respect dark, light, and semi-transparent user terminals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub text: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub border: Color,
    pub rail: Color,
    pub code: Color,
    pub bg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            text: Color::Reset,
            text_muted: Color::BrightBlack,
            accent: Color::BrightCyan,
            success: Color::BrightGreen,
            warning: Color::BrightYellow,
            error: Color::BrightRed,
            border: Color::BrightBlack,
            rail: Color::BrightCyan,
            code: Color::BrightYellow,
            bg: Color::Reset,
        }
    }
}

impl Theme {
    pub const fn dark() -> Self {
        Self {
            text: Color::White,
            text_muted: Color::BrightBlack,
            accent: Color::BrightCyan,
            success: Color::BrightGreen,
            warning: Color::BrightYellow,
            error: Color::BrightRed,
            border: Color::BrightBlack,
            rail: Color::BrightCyan,
            code: Color::BrightYellow,
            bg: Color::Reset,
        }
    }

    pub const fn light() -> Self {
        Self {
            text: Color::Black,
            text_muted: Color::BrightBlack,
            accent: Color::Blue,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            border: Color::BrightBlack,
            rail: Color::Blue,
            code: Color::Magenta,
            bg: Color::Reset,
        }
    }

    /// A theme that emits no color attributes at all, only structural/default
    /// foreground. Use for `--no-color` and log capture.
    pub const fn no_color() -> Self {
        Self {
            text: Color::Reset,
            text_muted: Color::Reset,
            accent: Color::Reset,
            success: Color::Reset,
            warning: Color::Reset,
            error: Color::Reset,
            border: Color::Reset,
            rail: Color::Reset,
            code: Color::Reset,
            bg: Color::Reset,
        }
    }

    /// Resolves the palette into complete semantic [`Style`] values.
    ///
    /// This is the preferred surface for demos and components: callers should
    /// use *roles* (`accent`, `muted`, `success`, ...) rather than assuming a
    /// specific ANSI color is readable on every terminal theme.
    pub fn styles(&self) -> ThemeStyles {
        // `Color::Reset` means "inherit the terminal's default", which is
        // represented as *no* foreground so no SGR code is emitted at all.
        fn fg(c: Color) -> Style {
            if c == Color::Reset {
                Style::new()
            } else {
                Style::new().fg(c)
            }
        }
        ThemeStyles {
            text: fg(self.text),
            // `muted` deliberately means "default foreground + dim" rather than a
            // hardcoded BrightBlack, which is unreadable on some light terminals.
            muted: Style::new().dim(),
            faint: Style::new().dim(),
            accent: fg(self.accent).bold(),
            success: fg(self.success),
            warning: fg(self.warning),
            error: fg(self.error),
            border: fg(self.border),
            rail: fg(self.rail),
            code: fg(self.code),
            link: fg(self.accent).underline(),
            selection: Style::new().reverse(),
        }
    }
}

/// Complete semantic [`Style`] roles resolved from a [`Theme`].
///
/// See [`Theme::styles`]. Components should prefer these over raw colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeStyles {
    pub text: Style,
    pub muted: Style,
    pub faint: Style,
    pub accent: Style,
    pub success: Style,
    pub warning: Style,
    pub error: Style,
    pub border: Style,
    pub rail: Style,
    pub code: Style,
    pub link: Style,
    pub selection: Style,
}

/// Text styling attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub reverse: bool,
}

impl Style {
    pub const fn new() -> Self {
        Self {
            fg: None,
            bg: None,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            reverse: false,
        }
    }

    pub const fn fg(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    pub const fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    pub const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub const fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    pub const fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    pub const fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    pub const fn reverse(mut self) -> Self {
        self.reverse = true;
        self
    }

    pub fn is_default(&self) -> bool {
        self.fg.is_none()
            && self.bg.is_none()
            && !self.bold
            && !self.dim
            && !self.italic
            && !self.underline
            && !self.reverse
    }

    /// Combines a style-only overlay on top of this base style.
    ///
    /// Explicit `fg`/`bg` in the overlay win; boolean attributes are OR-ed, so a
    /// dim veil on an already-bold cell stays bold and dim.
    pub fn overlay(self, over: Style) -> Style {
        Style {
            fg: over.fg.or(self.fg),
            bg: over.bg.or(self.bg),
            bold: self.bold || over.bold,
            dim: self.dim || over.dim,
            italic: self.italic || over.italic,
            underline: self.underline || over.underline,
            reverse: self.reverse || over.reverse,
        }
    }

    pub fn to_sgr(&self) -> String {
        let mut s = String::new();
        self.write_sgr(&mut s);
        s
    }

    pub fn write_sgr(&self, out: &mut String) {
        if self.is_default() {
            out.push_str("\x1b[0m");
            return;
        }

        let mut codes: Vec<String> = Vec::new();
        if self.bold {
            codes.push("1".to_string());
        }
        if self.dim {
            codes.push("2".to_string());
        }
        if self.italic {
            codes.push("3".to_string());
        }
        if self.underline {
            codes.push("4".to_string());
        }
        if self.reverse {
            codes.push("7".to_string());
        }

        if let Some(fg) = self.fg {
            match fg {
                Color::Reset => codes.push("39".to_string()),
                Color::Black => codes.push("30".to_string()),
                Color::Red => codes.push("31".to_string()),
                Color::Green => codes.push("32".to_string()),
                Color::Yellow => codes.push("33".to_string()),
                Color::Blue => codes.push("34".to_string()),
                Color::Magenta => codes.push("35".to_string()),
                Color::Cyan => codes.push("36".to_string()),
                Color::White => codes.push("37".to_string()),
                Color::BrightBlack => codes.push("90".to_string()),
                Color::BrightRed => codes.push("91".to_string()),
                Color::BrightGreen => codes.push("92".to_string()),
                Color::BrightYellow => codes.push("93".to_string()),
                Color::BrightBlue => codes.push("94".to_string()),
                Color::BrightMagenta => codes.push("95".to_string()),
                Color::BrightCyan => codes.push("96".to_string()),
                Color::BrightWhite => codes.push("97".to_string()),
                Color::Ansi256(n) => codes.push(format!("38;5;{}", n)),
                Color::Rgb(r, g, b) => codes.push(format!("38;2;{};{};{}", r, g, b)),
            }
        }

        if let Some(bg) = self.bg {
            match bg {
                Color::Reset => codes.push("49".to_string()),
                Color::Black => codes.push("40".to_string()),
                Color::Red => codes.push("41".to_string()),
                Color::Green => codes.push("42".to_string()),
                Color::Yellow => codes.push("43".to_string()),
                Color::Blue => codes.push("44".to_string()),
                Color::Magenta => codes.push("45".to_string()),
                Color::Cyan => codes.push("46".to_string()),
                Color::White => codes.push("47".to_string()),
                Color::BrightBlack => codes.push("100".to_string()),
                Color::BrightRed => codes.push("101".to_string()),
                Color::BrightGreen => codes.push("102".to_string()),
                Color::BrightYellow => codes.push("103".to_string()),
                Color::BrightBlue => codes.push("104".to_string()),
                Color::BrightMagenta => codes.push("105".to_string()),
                Color::BrightCyan => codes.push("106".to_string()),
                Color::BrightWhite => codes.push("107".to_string()),
                Color::Ansi256(n) => codes.push(format!("48;5;{}", n)),
                Color::Rgb(r, g, b) => codes.push(format!("48;2;{};{};{}", r, g, b)),
            }
        }

        if !codes.is_empty() {
            out.push_str("\x1b[");
            out.push_str(&codes.join(";"));
            out.push('m');
        }
    }
}

/// A contiguous slice of text sharing a single style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: CompactString,
    pub style: Style,
}

impl Span {
    pub fn raw(text: impl AsRef<str>) -> Self {
        Self {
            text: CompactString::new(text.as_ref()),
            style: Style::default(),
        }
    }

    pub fn styled(text: impl AsRef<str>, style: Style) -> Self {
        Self {
            text: CompactString::new(text.as_ref()),
            style,
        }
    }

    pub fn display_width(&self) -> usize {
        UnicodeWidthStr::width(self.text.as_str())
    }
}

impl From<&str> for Span {
    fn from(s: &str) -> Self {
        Span::raw(s)
    }
}

impl From<String> for Span {
    fn from(s: String) -> Self {
        Span::raw(s)
    }
}

/// Text alignment within a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// A single line composed of styled spans.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Line {
    pub spans: Vec<Span>,
    pub align: TextAlign,
}

impl Line {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn raw(text: impl AsRef<str>) -> Self {
        Self {
            spans: vec![Span::raw(text)],
            align: TextAlign::Left,
        }
    }

    pub fn styled(text: impl AsRef<str>, style: Style) -> Self {
        Self {
            spans: vec![Span::styled(text, style)],
            align: TextAlign::Left,
        }
    }

    pub fn from_spans(spans: Vec<Span>) -> Self {
        Self {
            spans,
            align: TextAlign::Left,
        }
    }

    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    pub fn push(&mut self, span: Span) {
        self.spans.push(span);
    }

    pub fn span(mut self, span: Span) -> Self {
        self.spans.push(span);
        self
    }

    pub fn display_width(&self) -> usize {
        self.spans.iter().map(|s| s.display_width()).sum()
    }

    /// Returns plain UTF-8 text with ZERO escape codes.
    pub fn plain_text(&self) -> String {
        let mut s = String::new();
        for span in &self.spans {
            s.push_str(span.text.as_str());
        }
        s
    }

    /// Formats the line with ANSI escape sequences for TTY display.
    pub fn to_ansi(&self) -> String {
        let mut out = String::new();
        let mut active_style = Style::default();

        for span in &self.spans {
            if span.style != active_style {
                if !active_style.is_default() {
                    out.push_str("\x1b[0m");
                }
                if !span.style.is_default() {
                    span.style.write_sgr(&mut out);
                }
                active_style = span.style;
            }
            out.push_str(span.text.as_str());
        }

        if !active_style.is_default() {
            out.push_str("\x1b[0m");
        }

        out
    }
}

impl From<&str> for Line {
    fn from(s: &str) -> Self {
        Line::raw(s)
    }
}

impl From<String> for Line {
    fn from(s: String) -> Self {
        Line::raw(s)
    }
}

impl From<Vec<Span>> for Line {
    fn from(spans: Vec<Span>) -> Self {
        Line::from_spans(spans)
    }
}

impl From<Span> for Line {
    fn from(span: Span) -> Self {
        Line::from_spans(vec![span])
    }
}

/// A structured multi-line block of styled text.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RichText {
    pub lines: Vec<Line>,
}

impl RichText {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn raw(text: impl AsRef<str>) -> Self {
        let lines = text.as_ref().split('\n').map(Line::raw).collect();
        Self { lines }
    }

    pub fn styled(text: impl AsRef<str>, style: Style) -> Self {
        let lines = text
            .as_ref()
            .split('\n')
            .map(|l| Line::styled(l, style))
            .collect();
        Self { lines }
    }

    pub fn from_lines(lines: Vec<Line>) -> Self {
        Self { lines }
    }

    pub fn push_line(&mut self, line: Line) {
        self.lines.push(line);
    }

    pub fn line(mut self, line: Line) -> Self {
        self.lines.push(line);
        self
    }

    /// Returns plain UTF-8 text with ZERO escape codes.
    pub fn plain_text(&self) -> String {
        self.lines
            .iter()
            .map(|l| l.plain_text())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Formats the rich text with ANSI escape sequences for TTY display.
    pub fn to_ansi(&self) -> String {
        self.lines
            .iter()
            .map(|l| l.to_ansi())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl From<&str> for RichText {
    fn from(s: &str) -> Self {
        RichText::raw(s)
    }
}

impl From<String> for RichText {
    fn from(s: String) -> Self {
        RichText::raw(s)
    }
}

impl From<Line> for RichText {
    fn from(line: Line) -> Self {
        RichText::from_lines(vec![line])
    }
}

impl From<Vec<Line>> for RichText {
    fn from(lines: Vec<Line>) -> Self {
        RichText::from_lines(lines)
    }
}

/// A single grapheme cluster and its measured cell width.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Glyph {
    pub grapheme: CompactString,
    pub display_width: u8,
}

impl Glyph {
    pub fn new(s: &str) -> Self {
        let width = UnicodeWidthStr::width(s).min(2) as u8;
        Self {
            grapheme: CompactString::new(s),
            display_width: width,
        }
    }

    pub fn space() -> Self {
        Self {
            grapheme: CompactString::new(" "),
            display_width: 1,
        }
    }

    pub fn empty() -> Self {
        Self {
            grapheme: CompactString::new(""),
            display_width: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.grapheme.is_empty()
    }
}

impl Default for Glyph {
    fn default() -> Self {
        Self::space()
    }
}

/// A logical terminal cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub glyph: Glyph,
    pub style: Style,
    /// True if this cell is the trailing continuation half of a wide (display_width == 2) glyph.
    pub is_continuation: bool,
    /// Explicit transparency. A transparent cell is *not* painted by the
    /// compositor and leaves the lower layer untouched. Ordinary blank spaces
    /// are opaque; transparency must be requested deliberately.
    pub transparent: bool,
    /// Style-only overlay. When true (and not transparent), the cell contributes
    /// its style to the cell beneath it without replacing its glyph.
    pub style_only: bool,
}

impl Cell {
    pub fn new(glyph: Glyph, style: Style) -> Self {
        Self {
            glyph,
            style,
            is_continuation: false,
            transparent: false,
            style_only: false,
        }
    }

    pub fn space(style: Style) -> Self {
        Self {
            glyph: Glyph::space(),
            style,
            is_continuation: false,
            transparent: false,
            style_only: false,
        }
    }

    pub fn continuation(style: Style) -> Self {
        Self {
            glyph: Glyph::empty(),
            style,
            is_continuation: true,
            transparent: false,
            style_only: false,
        }
    }

    /// A fully transparent cell: contributes nothing when composited.
    pub fn transparent() -> Self {
        Self {
            glyph: Glyph::space(),
            style: Style::default(),
            is_continuation: false,
            transparent: true,
            style_only: false,
        }
    }

    /// A style-only cell: contributes `style` to the cell beneath it without
    /// replacing its glyph (used for dimming/veils).
    pub fn style_overlay(style: Style) -> Self {
        Self {
            glyph: Glyph::space(),
            style,
            is_continuation: false,
            transparent: false,
            style_only: true,
        }
    }

    pub fn reset(&mut self) {
        self.glyph = Glyph::space();
        self.style = Style::default();
        self.is_continuation = false;
        self.transparent = false;
        self.style_only = false;
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self::space(Style::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glyph_ascii() {
        let g = Glyph::new("a");
        assert_eq!(g.display_width, 1);
        assert_eq!(g.grapheme.as_str(), "a");
    }

    #[test]
    fn test_glyph_wide_cjk() {
        let g = Glyph::new("你");
        assert_eq!(g.display_width, 2);
        assert_eq!(g.grapheme.as_str(), "你");
    }

    #[test]
    fn test_glyph_emoji() {
        let g = Glyph::new("🦀");
        assert_eq!(g.display_width, 2);
    }

    #[test]
    fn test_glyph_combining() {
        // e + combining acute accent
        let g = Glyph::new("e\u{0301}");
        assert_eq!(g.display_width, 1);
        assert_eq!(g.grapheme.as_str(), "é");
    }

    #[test]
    fn test_color_lerp_and_xterm_mapping() {
        assert_eq!(
            Color::Rgb(0, 0, 0).lerp(Color::Rgb(100, 200, 50), 0.0),
            Color::Rgb(0, 0, 0)
        );
        assert_eq!(
            Color::Rgb(0, 0, 0).lerp(Color::Rgb(100, 200, 50), 1.0),
            Color::Rgb(100, 200, 50)
        );
        assert_eq!(
            Color::Rgb(0, 0, 0).lerp(Color::Rgb(100, 200, 50), 0.5),
            Color::Rgb(50, 100, 25)
        );
        // 256-color indices resolve to concrete RGB (cube + grayscale).
        assert_eq!(Color::Ansi256(16).to_rgb(), (0, 0, 0));
        assert_eq!(Color::Ansi256(231).to_rgb(), (255, 255, 255));
        assert_eq!(Color::Ansi256(232).to_rgb(), (8, 8, 8));
    }

    #[test]
    fn test_style_builder() {
        let s = Style::new()
            .fg(Color::Red)
            .bg(Color::Blue)
            .bold()
            .underline();
        assert_eq!(s.fg, Some(Color::Red));
        assert_eq!(s.bg, Some(Color::Blue));
        assert!(s.bold);
        assert!(s.underline);
        assert!(!s.italic);
    }

    #[test]
    fn test_theme_tokens() {
        let theme = Theme::default();
        assert_eq!(theme.text, Color::Reset);
        assert_eq!(theme.bg, Color::Reset);
        assert_eq!(theme.accent, Color::BrightCyan);
    }

    #[test]
    fn test_theme_styles_are_semantic_roles() {
        let styles = Theme::default().styles();
        // `muted` must be "default foreground + dim", not a hardcoded BrightBlack.
        assert!(styles.muted.dim);
        assert_eq!(styles.muted.fg, None);
        // accent carries emphasis.
        assert!(styles.accent.bold);
        // No style should paint a background by default (native canvas preserved).
        assert_eq!(styles.text.bg, None);
        assert_eq!(styles.accent.bg, None);
    }

    #[test]
    fn test_no_color_theme_adds_no_color() {
        let styles = Theme::no_color().styles();
        // No role may introduce a foreground or background color.
        for (name, s) in [
            ("text", styles.text),
            ("accent", styles.accent),
            ("success", styles.success),
            ("warning", styles.warning),
            ("error", styles.error),
            ("border", styles.border),
            ("rail", styles.rail),
            ("code", styles.code),
        ] {
            assert_eq!(s.fg, None, "{name} leaked a foreground in no-color mode");
            assert_eq!(s.bg, None, "{name} leaked a background in no-color mode");
        }
    }

    #[test]
    fn test_span_and_line_plain_and_ansi() {
        let line = Line::new()
            .span(Span::styled("Status: ", Style::new().bold()))
            .span(Span::styled("Online", Style::new().fg(Color::Green)));

        // Plain text must contain zero ANSI escapes
        assert_eq!(line.plain_text(), "Status: Online");
        assert!(!line.plain_text().contains('\x1b'));

        // ANSI text must contain SGR escapes
        let ansi = line.to_ansi();
        assert!(ansi.contains("\x1b[1m"));
        assert!(ansi.contains("\x1b[32m"));
        assert!(ansi.ends_with("\x1b[0m"));
    }

    #[test]
    fn test_rich_text_multi_line() {
        let mut rich = RichText::new();
        rich.push_line(Line::styled("Header", Style::new().bold()));
        rich.push_line(Line::raw("Content line 1"));
        rich.push_line(Line::raw("Content line 2"));

        assert_eq!(rich.plain_text(), "Header\nContent line 1\nContent line 2");
        assert!(!rich.plain_text().contains('\x1b'));
        assert!(rich.to_ansi().contains("\x1b[1m"));
    }
}
