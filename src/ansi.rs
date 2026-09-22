use crate::cell::{Color, Style};
use crate::diff::{RowPatch, SurfaceDiff};
use std::io::Write;

/// An ANSI escape code compiler that converts surface diffs into compact byte streams.
pub struct AnsiCompiler {
    pub cursor_x: u16,
    pub cursor_y: u16,
    pub current_style: Style,
    pub sync_updates: bool,
}

impl AnsiCompiler {
    pub fn new(sync_updates: bool) -> Self {
        Self {
            cursor_x: 0,
            cursor_y: 0,
            current_style: Style::default(),
            sync_updates,
        }
    }

    pub fn reset_cursor(&mut self, x: u16, y: u16) {
        self.cursor_x = x;
        self.cursor_y = y;
    }

    /// Compiles a surface diff into a vector of ANSI bytes.
    pub fn compile(&mut self, diff: &SurfaceDiff) -> Vec<u8> {
        let mut out = Vec::new();
        if diff.is_empty() {
            return out;
        }

        if self.sync_updates {
            // Begin synchronized update (CSI ? 2026 h)
            out.extend_from_slice(b"\x1b[?2026h");
        }

        // Disable auto-wrap mode (DECAWM) to protect against right-margin wrap glitch
        out.extend_from_slice(b"\x1b[?7l");

        for patch in &diff.patches {
            self.compile_row_patch(patch, &mut out);
        }

        // If previous surface had more rows than next, clear the orphaned rows
        if diff.rows_to_clear > 0 {
            for row in diff.next_height..(diff.next_height + diff.rows_to_clear) {
                self.move_to(0, row, &mut out);
                self.apply_style(Style::default(), &mut out);
                out.extend_from_slice(b"\x1b[K"); // CSI K: erase to end of line
            }
        }

        // Always reset styling at end of frame
        if !self.current_style.is_default() {
            out.extend_from_slice(b"\x1b[0m");
            self.current_style = Style::default();
        }

        // Re-enable auto-wrap mode (DECAWM)
        out.extend_from_slice(b"\x1b[?7h");

        if self.sync_updates {
            // End synchronized update (CSI ? 2026 l)
            out.extend_from_slice(b"\x1b[?2026l");
        }

        out
    }

    fn compile_row_patch(&mut self, patch: &RowPatch, out: &mut Vec<u8>) {
        let y = patch.y;

        for run in &patch.runs {
            self.move_to(run.x, y, out);

            for cell in &run.cells {
                // Continuation cells are skipped because terminal hardware cursor already advanced
                if cell.is_continuation {
                    continue;
                }

                if cell.style != self.current_style {
                    self.apply_style(cell.style, out);
                }

                if cell.glyph.is_empty() {
                    out.push(b' ');
                    self.cursor_x += 1;
                } else {
                    out.extend_from_slice(cell.glyph.grapheme.as_bytes());
                    self.cursor_x += cell.glyph.display_width as u16;
                }
            }
        }

        if let Some(erase_x) = patch.erase_eol_from {
            self.move_to(erase_x, y, out);
            self.apply_style(Style::default(), out);
            out.extend_from_slice(b"\x1b[K"); // Erase to EOL
        }
    }

    /// Moves cursor to target (x, y) with minimal escape sequence bytes.
    pub fn move_to(&mut self, x: u16, y: u16, out: &mut Vec<u8>) {
        if self.cursor_x == x && self.cursor_y == y {
            return;
        }

        // Vertical movement
        if y != self.cursor_y {
            if y > self.cursor_y {
                let dy = y - self.cursor_y;
                if dy == 1 && x == 0 {
                    // Moving down 1 line and to start of line: newline is shorter
                    out.extend_from_slice(b"\r\n");
                    self.cursor_x = 0;
                    self.cursor_y = y;
                    return;
                } else {
                    write!(out, "\x1b[{}B", dy).ok();
                }
            } else {
                let dy = self.cursor_y - y;
                write!(out, "\x1b[{}A", dy).ok();
            }
            self.cursor_y = y;
        }

        // Horizontal movement
        if x != self.cursor_x {
            if x == 0 {
                out.push(b'\r');
            } else {
                // Absolute column position: CSI <col> G (1-indexed)
                write!(out, "\x1b[{}G", x + 1).ok();
            }
            self.cursor_x = x;
        }
    }

    /// Emits SGR escape codes to transition to `target_style`.
    fn apply_style(&mut self, target: Style, out: &mut Vec<u8>) {
        if self.current_style == target {
            return;
        }

        // If styles were previously active and need disabling, reset first
        if self.needs_reset(&target) {
            out.extend_from_slice(b"\x1b[0m");
            self.current_style = Style::default();
        }

        let mut codes: Vec<String> = Vec::new();

        if target.bold && !self.current_style.bold {
            codes.push("1".to_string());
        }
        if target.dim && !self.current_style.dim {
            codes.push("2".to_string());
        }
        if target.italic && !self.current_style.italic {
            codes.push("3".to_string());
        }
        if target.underline && !self.current_style.underline {
            codes.push("4".to_string());
        }
        if target.reverse && !self.current_style.reverse {
            codes.push("7".to_string());
        }

        // Foreground color
        if target.fg != self.current_style.fg {
            if let Some(color) = target.fg {
                match color {
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
            } else if self.current_style.fg.is_some() {
                codes.push("39".to_string());
            }
        }

        // Background color
        if target.bg != self.current_style.bg {
            if let Some(color) = target.bg {
                match color {
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
            } else if self.current_style.bg.is_some() {
                codes.push("49".to_string());
            }
        }

        if !codes.is_empty() {
            write!(out, "\x1b[{}m", codes.join(";")).ok();
        }

        self.current_style = target;
    }

    fn needs_reset(&self, target: &Style) -> bool {
        (self.current_style.bold && !target.bold)
            || (self.current_style.dim && !target.dim)
            || (self.current_style.italic && !target.italic)
            || (self.current_style.underline && !target.underline)
            || (self.current_style.reverse && !target.reverse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::compute_diff;
    use crate::surface::Surface;

    #[test]
    fn test_compiler_empty_diff_emits_zero_bytes() {
        let mut compiler = AnsiCompiler::new(false);
        let diff = SurfaceDiff::default();
        let bytes = compiler.compile(&diff);
        assert_eq!(bytes.len(), 0);
    }

    #[test]
    fn test_compiler_patch_emits_text() {
        let s1 = Surface::new(10, 1);
        let mut s2 = Surface::new(10, 1);
        s2.print_str(0, 0, "hi", Style::default(), None);

        let diff = compute_diff(Some(&s1), &s2);
        let mut compiler = AnsiCompiler::new(false);
        let bytes = compiler.compile(&diff);
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains("hi"));
    }

    #[test]
    fn test_compiler_synchronized_update_wrap() {
        let s1 = Surface::new(10, 1);
        let mut s2 = Surface::new(10, 1);
        s2.print_str(0, 0, "a", Style::default(), None);

        let diff = compute_diff(Some(&s1), &s2);
        let mut compiler = AnsiCompiler::new(true);
        let bytes = compiler.compile(&diff);
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.starts_with("\x1b[?2026h"));
        assert!(s.ends_with("\x1b[?2026l"));
    }

    #[test]
    fn test_compiler_erase_to_eol() {
        let mut s1 = Surface::new(20, 1);
        s1.print_str(0, 0, "abcdefghij", Style::default(), None);

        let mut s2 = Surface::new(20, 1);
        s2.print_str(0, 0, "abc", Style::default(), None);

        let diff = compute_diff(Some(&s1), &s2);
        let mut compiler = AnsiCompiler::new(false);
        let bytes = compiler.compile(&diff);
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains("\x1b[K")); // Contains CSI K
    }

    #[test]
    fn test_compiler_autowrap_protection() {
        let s1 = Surface::new(10, 1);
        let mut s2 = Surface::new(10, 1);
        s2.print_str(0, 0, "test", Style::default(), None);

        let diff = compute_diff(Some(&s1), &s2);
        let mut compiler = AnsiCompiler::new(false);
        let bytes = compiler.compile(&diff);
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains("\x1b[?7l")); // Disables autowrap
        assert!(s.contains("\x1b[?7h")); // Re-enables autowrap
    }
}
