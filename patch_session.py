import re

with open("src/session.rs", "r") as f:
    text = f.read()

text = text.replace(
    'execute!(out, Show, LeaveAlternateScreen, DisableBracketedPaste);\n                let _ = out.write_all(b"\\x1b[0m\\x1b[?2026l\\r\\n");',
    'execute!(out, Show, LeaveAlternateScreen, DisableBracketedPaste);\n                let _ = out.write_all(b"\\x1b[0m\\x1b[?2026l\\x1b[?7h\\x1b[?25h\\r\\n");'
)

text = text.replace(
    '// Reset styling and synchronized update state\n        let _ = out.write_all(b"\\x1b[0m\\x1b[?2026l");',
    '// Reset styling, synchronized update state, enable autowrap, show cursor\n        let _ = out.write_all(b"\\x1b[0m\\x1b[?2026l\\x1b[?7h\\x1b[?25h");'
)

text = re.sub(
    r"pub fn hide_cursor\(&mut self\) -> io::Result<\(\)> \{\n.*?Ok\(\(\)\)\n    \}",
    lambda m: "pub fn hide_cursor(&mut self) -> Option<&'static [u8]> {\n        if !self.is_tty || self.cursor_hidden {\n            return None;\n        }\n        self.cursor_hidden = true;\n        Some(b\"\\x1b[?25l\")\n    }",
    text,
    flags=re.DOTALL
)

text = re.sub(
    r"pub fn show_cursor\(&mut self\) -> io::Result<\(\)> \{\n.*?Ok\(\(\)\)\n    \}",
    lambda m: "pub fn show_cursor(&mut self) -> Option<&'static [u8]> {\n        if !self.is_tty || !self.cursor_hidden {\n            return None;\n        }\n        self.cursor_hidden = false;\n        Some(b\"\\x1b[?25h\")\n    }",
    text,
    flags=re.DOTALL
)

text = text.replace("cursor::{Hide, Show},", "cursor::{Show},")

with open("src/session.rs", "w") as f:
    f.write(text)
