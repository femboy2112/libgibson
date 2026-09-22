import re

with open("examples/polished_agent.rs", "r") as f:
    text = f.read()

old = """                    Node::text_input(
                        &input.text,
                        input.cursor_grapheme,
                        Some("Type instruction (Esc to exit)..."),
                        Style::new().fg(Color::White),
                    )
                    .percent_width(95.0),"""

new = """                    Node::text_input(
                        &input.text,
                        input.cursor_grapheme,
                        Some("Type instruction (Esc to exit)..."),
                        Style::new().fg(Color::White),
                    )
                    .scroll_offset(input.scroll_offset)
                    .percent_width(95.0),"""

text = text.replace(old, new)
# Wait, I previously changed percent_width(95.0) to flex_grow(1.0) !!
# So the old string is flex_grow(1.0)
old2 = """                    Node::text_input(
                        &input.text,
                        input.cursor_grapheme,
                        Some("Type instruction (Esc to exit)..."),
                        Style::new().fg(Color::White),
                    )
                    .flex_grow(1.0),"""

new2 = """                    Node::text_input(
                        &input.text,
                        input.cursor_grapheme,
                        Some("Type instruction (Esc to exit)..."),
                        Style::new().fg(Color::White),
                    )
                    .scroll_offset(input.scroll_offset)
                    .flex_grow(1.0),"""

text = text.replace(old2, new2)

with open("examples/polished_agent.rs", "w") as f:
    f.write(text)
