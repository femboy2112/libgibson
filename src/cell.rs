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
}

impl Cell {
    pub fn new(glyph: Glyph, style: Style) -> Self {
        Self {
            glyph,
            style,
            is_continuation: false,
        }
    }

    pub fn space(style: Style) -> Self {
        Self {
            glyph: Glyph::space(),
            style,
            is_continuation: false,
        }
    }

    pub fn continuation(style: Style) -> Self {
        Self {
            glyph: Glyph::empty(),
            style,
            is_continuation: true,
        }
    }

    pub fn reset(&mut self) {
        self.glyph = Glyph::space();
        self.style = Style::default();
        self.is_continuation = false;
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
