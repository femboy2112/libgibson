import re

with open("src/context.rs", "r") as f:
    text = f.read()

text = text.replace("Renderer::new(mode, sync)", "Renderer::new(mode)")
text = text.replace("self.renderer.set_sync_updates(self.session.sync_updates());", "")

for method in ["render", "commit", "commit_node", "insert_before_live", "insert_node_before_live", "clear_live_region"]:
    text = re.sub(
        r"self\.renderer\." + method + r"\((.*?),\s*&mut self\.session\)",
        r"self.renderer." + method + r"(\1, &mut self.session, &mut std::io::stdout())",
        text
    )

with open("src/context.rs", "w") as f:
    f.write(text)
