import re

with open("src/context.rs", "r") as f:
    text = f.read()

text = text.replace("Renderer::new(mode, sync)", "Renderer::new(mode)")
text = text.replace("self.renderer.set_sync_updates(self.session.sync_updates());\n", "")

text = re.sub(r"self\.renderer\.commit\(text, &mut self\.session\)", "self.renderer.commit(text, &mut self.session, &mut std::io::stdout())", text)
text = re.sub(r"self\.renderer\.clear_live_region\(&mut self\.session\)", "self.renderer.clear_live_region(&mut self.session, &mut std::io::stdout())", text)

with open("src/context.rs", "w") as f:
    f.write(text)
