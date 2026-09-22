with open("src/renderer.rs", "r") as f:
    text = f.read()

text = text.replace("let bytes_emitted = out.len();", "let bytes_emitted = tx.buffer.len();")

# `writer` in renderer.rs:91
# pub fn render(..., writer: &mut dyn Write)
text = text.replace(
    "pub fn render(\n        &mut self,\n        root: &mut Node,\n        session: &mut TerminalSession,\n    )",
    "pub fn render(\n        &mut self,\n        root: &mut Node,\n        session: &mut TerminalSession,\n        writer: &mut dyn Write,\n    )"
)

# `writer` in renderer.rs:265
# insert_before_live
text = text.replace(
    "pub fn insert_before_live(\n        &mut self,\n        lines: &[&str],\n        session: &mut TerminalSession,\n    )",
    "pub fn insert_before_live(\n        &mut self,\n        lines: &[&str],\n        session: &mut TerminalSession,\n        writer: &mut dyn Write,\n    )"
)

with open("src/renderer.rs", "w") as f:
    f.write(text)
