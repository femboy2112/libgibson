with open("src/context.rs", "r") as f:
    text = f.read()

replacement = """pub fn insert_node_before_live(&mut self, node: &mut Node) -> io::Result<()> {
        let (bytes, used_fallback) = self.renderer.insert_node_before_live(node, &mut self.session, &mut std::io::stdout())?;
        if used_fallback { self.scheduler.stats.insertion_repaints += 1; }
        self.scheduler.stats.history_insertions += 1;
        self.scheduler.stats.insertion_bytes += bytes as u64;
        Ok(())
    }"""

import re
text = re.sub(
    r"pub fn insert_node_before_live\(&mut self, node: &mut Node\) -> io::Result<\(\)> \{\n\s*self\.renderer\n\s*\.insert_node_before_live\(node, &mut self\.session, &mut std::io::stdout\(\)\)\n\s*\}",
    replacement,
    text
)

# wait it's multi line
text = re.sub(
    r"pub fn insert_node_before_live\(&mut self, node: &mut Node\) -> io::Result<\(\)> \{\n.*?\}\n\n",
    replacement + "\n\n",
    text,
    flags=re.DOTALL
)

with open("src/context.rs", "w") as f:
    f.write(text)
