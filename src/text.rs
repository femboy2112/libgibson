//! Terminal display-width measurement: the number of columns the painter advances
//! when it lays text on one row, exposed so callers can budget columns exactly.
//!
//! The measure is per grapheme cluster, as painted: a cluster is as wide as the
//! Unicode East Asian Width of its text (`unicode-width` 0.2), capped at two
//! columns; combining marks and other zero-width clusters add nothing; clusters
//! containing a control character (tab, ESC, BEL, ...) are never painted and add
//! nothing; and a line break (`\r` or `\n`) ends the row, so nothing after it is
//! counted. Measure the whole text you will paint with [`display_width`]; use
//! [`char_width`] only for a character painted on its own.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// The number of terminal columns `s` occupies when painted on one row.
///
/// ```
/// use gibson::text::display_width;
/// assert_eq!(display_width("abc"), 3);
/// assert_eq!(display_width("日本"), 4);
/// assert_eq!(display_width("e\u{301}"), 1); // e + combining acute
/// assert_eq!(display_width("a\tb"), 2); // controls are never painted
/// assert_eq!(display_width("ab\ncd"), 2); // a line break ends the row
/// ```
pub fn display_width(s: &str) -> usize {
    let mut width = 0;
    for grapheme in s.graphemes(true) {
        if grapheme == "\r" || grapheme == "\n" {
            break;
        }
        if grapheme.chars().any(char::is_control) {
            continue;
        }
        width += UnicodeWidthStr::width(grapheme).min(2);
    }
    width
}

/// The number of terminal columns `c` occupies when painted on its own:
/// `display_width` of the one-character string.
///
/// ```
/// use gibson::text::char_width;
/// assert_eq!(char_width('a'), 1);
/// assert_eq!(char_width('日'), 2);
/// assert_eq!(char_width('\u{301}'), 0);
/// assert_eq!(char_width('\x1b'), 0);
/// ```
pub fn char_width(c: char) -> usize {
    display_width(c.encode_utf8(&mut [0; 4]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Style;
    use crate::Surface;

    fn painted(s: &str) -> usize {
        let mut surface = Surface::new(200, 1);
        surface.print_str(0, 0, s, Style::default(), None) as usize
    }

    #[test]
    fn display_width_is_the_painted_column_count() {
        for s in [
            "",
            "abc",
            "日本語",
            "e\u{301}x",
            "👩\u{200d}💻",
            "a\u{fe0f}",
            "│ code",
            "\t",
            "\x1b",
            "\x07",
            "\r\n",
            "a\tb",
            "\u{644}\u{627}",
            "\u{5D0}\u{200D}\u{5DC}",
            "\u{17D8}",
            "ab\ncd",
            "x\ry",
        ] {
            assert_eq!(display_width(s), painted(s), "{s:?}");
        }
    }

    #[test]
    fn char_width_is_the_painted_width_of_one_char() {
        for c in ['a', '日', '\u{301}', '\x1b', '\t', '\n', '\u{17D8}', '😀'] {
            assert_eq!(char_width(c), painted(&c.to_string()), "{c:?}");
        }
    }
}
