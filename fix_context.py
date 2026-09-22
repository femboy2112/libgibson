import re

with open("src/context.rs", "r") as f:
    text = f.read()

# Replace self.renderer.render(...) with self.renderer.render(..., &mut std::io::stdout())
text = re.sub(
    r"self\.renderer\.render\(&mut root, &mut self\.session\)",
    r"self.renderer.render(&mut root, &mut self.session, &mut std::io::stdout())",
    text
)
text = re.sub(
    r"self\.renderer\.commit\(text, &mut self\.session\)",
    r"self.renderer.commit(text, &mut self.session, &mut std::io::stdout())",
    text
)
text = re.sub(
    r"self\.renderer\.commit_node\(node, &mut self\.session\)",
    r"self.renderer.commit_node(node, &mut self.session, &mut std::io::stdout())",
    text
)
text = re.sub(
    r"self\.renderer\.insert_before_live\(lines, &mut self\.session\)",
    r"self.renderer.insert_before_live(lines, &mut self.session, &mut std::io::stdout())",
    text
)
text = re.sub(
    r"self\.renderer\.insert_node_before_live\(node, &mut self\.session\)",
    r"self.renderer.insert_node_before_live(node, &mut self.session, &mut std::io::stdout())",
    text
)
text = re.sub(
    r"self\.renderer\.clear_live_region\(&mut self\.session\)",
    r"self.renderer.clear_live_region(&mut self.session, &mut std::io::stdout())",
    text
)

# Fix renderer.rs commit_node and insert_node_before_live
with open("src/renderer.rs", "r") as f2:
    r_text = f2.read()

r_text = re.sub(
    r"self\.commit\(&joined, session\)",
    r"self.commit(&joined, session, writer)",
    r_text
)
r_text = re.sub(
    r"self\.insert_before_live\(&line_refs, session\)",
    r"self.insert_before_live(&line_refs, session, writer)",
    r_text
)

with open("src/context.rs", "w") as f:
    f.write(text)
with open("src/renderer.rs", "w") as f2:
    f2.write(r_text)
