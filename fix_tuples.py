with open("src/renderer.rs", "r") as f:
    text = f.read()

text = text.replace("return Ok((0, 0, 0, false));", "return Ok((0, 0, 0, false, crate::painter::PaintContext::default()));")
text = text.replace("return Ok((0, total_cells, 0, false));", "return Ok((0, total_cells, 0, false, paint_ctx));")
text = text.replace("Ok((dirty_cells, total_cells, bytes_emitted, is_full_repaint))", "Ok((dirty_cells, total_cells, bytes_emitted, is_full_repaint, paint_ctx))")

with open("src/renderer.rs", "w") as f:
    f.write(text)

with open("src/context.rs", "r") as f:
    text = f.read()

text = text.replace("Ok((dirty, total, bytes, full)) => {", "Ok((dirty, total, bytes, full, paint_ctx)) => {")
text = text.replace("Ok((dirty_cells, total_cells, bytes, is_full_repaint, paint_ctx, paint_ctx))", "Ok((dirty_cells, total_cells, bytes, is_full_repaint, paint_ctx))")

with open("src/context.rs", "w") as f:
    f.write(text)
