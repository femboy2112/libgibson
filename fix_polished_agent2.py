with open("examples/polished_agent.rs", "r") as f:
    text = f.read()

# Only replace in the loop
old = """        loop {
            let prompt_node = build_prompt_ui(&theme, &input_state);
            ctx.set_root(prompt_node);
            ctx.render()?;"""

new = """        loop {
            let prompt_node = build_prompt_ui(&theme, &input_state);
            ctx.set_root(prompt_node);
            let paint_ctx = ctx.render()?;
            if let Some(offset) = paint_ctx.text_scroll_offset {
                input_state.scroll_offset = offset;
            }"""

text = text.replace(old, new)
with open("examples/polished_agent.rs", "w") as f:
    f.write(text)

with open("examples/resize_test_app.rs", "r") as f:
    text = f.read()

old_resize = """        ctx.set_root(root);
        ctx.render()?;"""

new_resize = """        ctx.set_root(root);
        let paint_ctx = ctx.render()?;
        """ # I'm not using input_state in resize_test_app, it uses String directly without scroll_offset? 
        # Actually in resize_test_app I just do `Node::text_input(&text, text.len(), ...)` so I don't care about scroll.
        # But wait, resize_test_app doesn't use `TextInputState`, so `ctx.render()?;` just works because we can drop `paint_ctx`.

text = text.replace(old_resize, new_resize)
with open("examples/resize_test_app.rs", "w") as f:
    f.write(text)
