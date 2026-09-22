import re

with open("src/renderer.rs", "r") as f:
    text = f.read()

text = text.replace(
    ") -> io::Result<(usize, usize, usize, bool)> {",
    ") -> io::Result<(usize, usize, usize, bool, crate::painter::PaintContext)> {"
)

text = re.sub(
    r"Ok\(\(dirty_cells, total_cells, bytes, is_full_repaint\)\)",
    r"Ok((dirty_cells, total_cells, bytes, is_full_repaint, paint_ctx))",
    text
)

with open("src/renderer.rs", "w") as f:
    f.write(text)

with open("src/context.rs", "r") as f:
    text = f.read()

text = re.sub(
    r"let \(dirty_cells, total_cells, bytes, is_full_repaint\) = result\?",
    r"let (dirty_cells, total_cells, bytes, is_full_repaint, paint_ctx) = result?",
    text
)

text = re.sub(
    r"Ok\(result\)",
    r"Ok(paint_ctx)",
    text
)

with open("src/context.rs", "w") as f:
    f.write(text)

