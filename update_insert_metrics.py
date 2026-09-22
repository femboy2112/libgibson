import re

with open("src/renderer.rs", "r") as f:
    text = f.read()

text = text.replace(
    "pub fn insert_before_live(\n        &mut self,\n        lines: &[&str],\n        session: &mut TerminalSession,\n        writer: &mut dyn Write,\n    ) -> io::Result<()> {",
    "pub fn insert_before_live(\n        &mut self,\n        lines: &[&str],\n        session: &mut TerminalSession,\n        writer: &mut dyn Write,\n    ) -> io::Result<usize> {"
)
text = text.replace(
    "            writer.flush()?;\n            return Ok(());\n        }\n\n        if lines.is_empty() {\n            return Ok(());\n        }",
    "            writer.flush()?;\n            return Ok(0);\n        }\n\n        if lines.is_empty() {\n            return Ok(0);\n        }"
)
text = text.replace(
    "        tx.commit()?;\n        Ok(())\n    }",
    "        let bytes = tx.buffer.len();\n        tx.commit()?;\n        Ok(bytes)\n    }"
)

# And insert_node_before_live:
text = text.replace(
    "pub fn insert_node_before_live(\n        &mut self,\n        node: &mut Node,\n        session: &mut TerminalSession,\n        writer: &mut dyn Write,\n    ) -> io::Result<()> {",
    "pub fn insert_node_before_live(\n        &mut self,\n        node: &mut Node,\n        session: &mut TerminalSession,\n        writer: &mut dyn Write,\n    ) -> io::Result<usize> {"
)

with open("src/renderer.rs", "w") as f:
    f.write(text)

with open("src/context.rs", "r") as f:
    text = f.read()

text = re.sub(
    r"pub fn insert_before_live\(&mut self, lines: &\[&str\]\) -> io::Result<\(\)> \{\n\s*self\.renderer\.insert_before_live\(lines, &mut self\.session, &mut std::io::stdout\(\)\)\n\s*\}",
    r"pub fn insert_before_live(&mut self, lines: &[&str]) -> io::Result<()> {\n        let bytes = self.renderer.insert_before_live(lines, &mut self.session, &mut std::io::stdout())?;\n        self.stats.history_insertions += 1;\n        self.stats.insertion_bytes += bytes;\n        Ok(())\n    }",
    text
)
text = re.sub(
    r"pub fn insert_node_before_live\(&mut self, node: &mut Node\) -> io::Result<\(\)> \{\n\s*self\.renderer\.insert_node_before_live\(node, &mut self\.session, &mut std::io::stdout\(\)\)\n\s*\}",
    r"pub fn insert_node_before_live(&mut self, node: &mut Node) -> io::Result<()> {\n        let bytes = self.renderer.insert_node_before_live(node, &mut self.session, &mut std::io::stdout())?;\n        self.stats.history_insertions += 1;\n        self.stats.insertion_bytes += bytes;\n        Ok(())\n    }",
    text
)

with open("src/context.rs", "w") as f:
    f.write(text)
