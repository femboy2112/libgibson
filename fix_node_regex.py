import re

with open("src/node.rs", "r") as f:
    text = f.read()

replacement = """pub fn scroll_offset(mut self, offset: usize) -> Self {
        if let NodeKind::TextInput { scroll_offset, .. } = &mut self.kind {
            *scroll_offset = offset;
        }
        self
    }"""

text = re.sub(
    r"(pub fn rich_text)",
    replacement + r"\n\n    \1",
    text,
    count=1
)

with open("src/node.rs", "w") as f:
    f.write(text)

# Also fix tests/non_tty_redirection.rs
with open("tests/non_tty_redirection.rs", "r") as f:
    test_text = f.read()

test_text = test_text.replace(
    "let (_dirty, _total, bytes, _full) = renderer.render(",
    "let (_dirty, _total, bytes, _full, _paint_ctx) = renderer.render("
)

with open("tests/non_tty_redirection.rs", "w") as f:
    f.write(test_text)
