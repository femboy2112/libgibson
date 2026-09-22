import re

with open("src/renderer.rs", "r") as f:
    text = f.read()

text = text.replace(
    "pub fn insert_before_live(\n        &mut self,\n        lines: &[&str],\n        session: &mut TerminalSession,\n        writer: &mut dyn Write,\n    ) -> io::Result<usize> {",
    "pub fn insert_before_live(\n        &mut self,\n        lines: &[&str],\n        session: &mut TerminalSession,\n        writer: &mut dyn Write,\n    ) -> io::Result<(usize, bool)> {"
)
text = text.replace(
    "            writer.flush()?;\n            return Ok(0);\n        }",
    "            writer.flush()?;\n            return Ok((0, false));\n        }"
)
text = text.replace(
    "        if lines.is_empty() {\n            return Ok(0);\n        }",
    "        if lines.is_empty() {\n            return Ok((0, false));\n        }"
)

# Replace Ok(bytes)
text = re.sub(
    r"        let bytes = tx\.buffer\.len\(\);\n        tx\.commit\(\)\?;\n        Ok\(bytes\)\n    \}",
    r"        let bytes = tx.buffer.len();\n        tx.commit()?;\n        let used_fallback = combined_height > term_rows;\n        Ok((bytes, used_fallback))\n    }",
    text
)

# Fix insert_node_before_live
text = text.replace(
    "pub fn insert_node_before_live(\n        &mut self,\n        node: &mut Node,\n        session: &mut TerminalSession,\n        writer: &mut dyn Write,\n    ) -> io::Result<usize> {",
    "pub fn insert_node_before_live(\n        &mut self,\n        node: &mut Node,\n        session: &mut TerminalSession,\n        writer: &mut dyn Write,\n    ) -> io::Result<(usize, bool)> {"
)

with open("src/renderer.rs", "w") as f:
    f.write(text)

with open("src/context.rs", "r") as f:
    text = f.read()

text = re.sub(
    r"let bytes = self\.renderer\.insert_before_live\(lines, &mut self\.session, &mut std::io::stdout\(\)\)\?;",
    r"let (bytes, used_fallback) = self.renderer.insert_before_live(lines, &mut self.session, &mut std::io::stdout())?;\n        if used_fallback { self.scheduler.stats.insertion_repaints += 1; }",
    text
)
text = re.sub(
    r"let bytes = self\.renderer\.insert_node_before_live\(node, &mut self\.session, &mut std::io::stdout\(\)\)\?;",
    r"let (bytes, used_fallback) = self.renderer.insert_node_before_live(node, &mut self.session, &mut std::io::stdout())?;\n        if used_fallback { self.scheduler.stats.insertion_repaints += 1; }",
    text
)

with open("src/context.rs", "w") as f:
    f.write(text)
