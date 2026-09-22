import re

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
text = text.replace(
    "None => return Ok(()),",
    "None => return Ok(crate::painter::PaintContext::default()),"
)

with open("src/context.rs", "w") as f:
    f.write(text)
