import re

with open("src/renderer.rs", "r") as f:
    text = f.read()

text = text.replace("use std::io::{self, stdout, Write};", "use std::io::{self, stdout, Write};\nuse crate::transaction::TerminalTransaction;")
text = text.replace("pub fn new(mode: RenderMode, sync_updates: bool)", "pub fn new(mode: RenderMode)")
text = text.replace("compiler: AnsiCompiler::new(sync_updates)", "compiler: AnsiCompiler::new()")

# Delete set_sync_updates from Renderer
text = re.sub(r"    pub fn set_sync_updates\(&mut self, enabled: bool\) \{\n        self\.compiler\.sync_updates = enabled;\n    \}\n", "", text)

# Add writer to signatures
for fn in ["render", "commit", "commit_node", "insert_before_live", "insert_node_before_live", "clear_live_region"]:
    text = re.sub(
        r"(pub fn " + fn + r"\([^)]*?session: &mut TerminalSession)\s*\)",
        r"\1, writer: &mut dyn Write)",
        text,
        flags=re.DOTALL
    )

# Fix method bodies
def replace_body(m):
    body = m.group(1)
    body = body.replace("let mut out = Vec::new();", "let mut tx = TerminalTransaction::new(writer, session.sync_updates());\n        tx.begin();")
    body = body.replace("out.extend_from_slice(", "tx.push(")
    body = body.replace("out.push(b'\\r')", "tx.push(b\"\\r\")")
    body = body.replace("&mut out", "&mut tx.buffer")
    body = body.replace("write!(out,", "write!(tx.buffer,")
    
    body = body.replace("let _ = session.show_cursor();", "if let Some(cmd) = session.show_cursor() { tx.push(cmd); }")
    body = body.replace("let _ = session.hide_cursor();", "if let Some(cmd) = session.hide_cursor() { tx.push(cmd); }")
    
    body = re.sub(r"        let mut stdout_handle = stdout\(\);\n\s*stdout_handle\.write_all\(&out\)\?;\n\s*stdout_handle\.flush\(\)\?;", "        tx.commit()?;", body)
    body = re.sub(r"        stdout_handle\.write_all\(&out\)\?;\n\s*stdout_handle\.flush\(\)\?;", "        tx.commit()?;", body)
    
    return body

text = re.sub(r"(let mut out = Vec::new\(\);.*?Ok\(\(dirty_cells, total_cells, dirty_cells, is_full_repaint\)\))", replace_body, text, flags=re.DOTALL)
# wait, my regex failed last time because `bytes` was returned, not `dirty_cells`.
text = re.sub(r"(let mut out = Vec::new\(\);.*?Ok\(\(dirty_cells, total_cells, bytes, is_full_repaint\)\))", replace_body, text, flags=re.DOTALL)

def replace_simple(m):
    return replace_body(m)

text = re.sub(r"(let mut out = Vec::new\(\);[\s\S]*?Ok\(\(\)\))", replace_simple, text)

text = text.replace("self.commit(&joined, session)", "self.commit(&joined, session, writer)")
text = text.replace("self.insert_before_live(&line_refs, session)", "self.insert_before_live(&line_refs, session, writer)")

with open("src/renderer.rs", "w") as f:
    f.write(text)
