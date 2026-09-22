with open("src/renderer.rs", "r") as f:
    text = f.read()

text = text.replace(
    "pub fn commit_node(\n        &mut self,\n        node: &mut Node,\n        session: &mut TerminalSession,\n    )",
    "pub fn commit_node(\n        &mut self,\n        node: &mut Node,\n        session: &mut TerminalSession,\n        writer: &mut dyn Write,\n    )"
)
text = text.replace(
    "pub fn insert_node_before_live(\n        &mut self,\n        node: &mut Node,\n        session: &mut TerminalSession,\n    )",
    "pub fn insert_node_before_live(\n        &mut self,\n        node: &mut Node,\n        session: &mut TerminalSession,\n        writer: &mut dyn Write,\n    )"
)
with open("src/renderer.rs", "w") as f:
    f.write(text)

with open("src/context.rs", "r") as f:
    text = f.read()
text = text.replace("let sync = session.sync_updates();\n", "")
import re
text = re.sub(
    r"self\.renderer\.insert_node_before_live\(node, &mut self\.session\)",
    r"self.renderer.insert_node_before_live(node, &mut self.session, &mut std::io::stdout())",
    text
)
with open("src/context.rs", "w") as f:
    f.write(text)
