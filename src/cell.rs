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
}
