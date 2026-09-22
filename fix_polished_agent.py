import re

with open("examples/polished_agent.rs", "r") as f:
    text = f.read()

text = text.replace(
    "ctx.render()?;",
    """let paint_ctx = ctx.render()?;
        if let Some(offset) = paint_ctx.text_scroll_offset {
            input_state.scroll_offset = offset;
        }"""
)

with open("examples/polished_agent.rs", "w") as f:
    f.write(text)
