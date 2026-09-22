import re

with open("src/painter.rs", "r") as f:
    text = f.read()

text = re.sub(
    r"pub struct PaintContext \{\n([\s\S]*?)\}",
    r"pub struct PaintContext {\n\1\n    pub text_scroll_offset: Option<usize>,\n}",
    text
)

text = text.replace(
    "let mut ctx = PaintContext::default();",
    "let mut ctx = PaintContext::default();"
)

text = text.replace(
    "let mut scroll_col = scroll_offset;\n    if cursor_col < scroll_col {\n        scroll_col = cursor_col;\n    } else if cursor_col >= scroll_col + visible_cols {\n        scroll_col = cursor_col.saturating_sub(visible_cols) + 1;\n    }",
    "let mut scroll_col = scroll_offset;\n    if cursor_col < scroll_col {\n        scroll_col = cursor_col;\n    } else if cursor_col >= scroll_col + visible_cols {\n        scroll_col = cursor_col.saturating_sub(visible_cols) + 1;\n    }\n    ctx.text_scroll_offset = Some(scroll_col);"
)

with open("src/painter.rs", "w") as f:
    f.write(text)

with open("src/context.rs", "r") as f:
    text = f.read()

text = text.replace(
    "pub fn render_now(&mut self) -> io::Result<()> {",
    "pub fn render_now(&mut self) -> io::Result<crate::painter::PaintContext> {"
)
text = text.replace(
    "pub fn render(&mut self) -> io::Result<()> {",
    "pub fn render(&mut self) -> io::Result<crate::painter::PaintContext> {"
)
text = text.replace(
    "fn render_internal(&mut self) -> io::Result<()> {",
    "fn render_internal(&mut self) -> io::Result<crate::painter::PaintContext> {"
)

text = re.sub(
    r"let mut root = match self\.root\.take\(\) \{\n\s*Some\(r\) => r,\n\s*None => return Ok\(\(\)\),\n\s*\};",
    r"let mut root = match self.root.take() {\n            Some(r) => r,\n            None => return Ok(crate::painter::PaintContext::default()),\n        };",
    text
)

text = re.sub(
    r"self\.root = Some\(root\);\n\n\s*Ok\(\(\)\)",
    r"self.root = Some(root);\n\n        Ok(result)",
    text
)

# wait! self.renderer.render now returns PaintContext?
# No, self.renderer.render returns io::Result<(usize, usize, usize, bool)>.
# Where does PaintContext come from? 
# In Context::render, we don't have PaintContext! It's returned by `paint`, which is called by `Renderer::render`.
