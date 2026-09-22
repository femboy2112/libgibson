import re

with open("src/renderer.rs", "r") as f:
    text = f.read()

# Make methods take writer
for fn_name in ["render", "commit", "insert_before_live", "clear_live_region", "commit_node", "insert_node_before_live"]:
    text = re.sub(
        r"(pub fn " + fn_name + r"\([^)]*?session: &mut TerminalSession)(?:\s*,\s*writer: &mut dyn Write)?\s*\)",
        r"\1, writer: &mut dyn Write)",
        text,
        flags=re.DOTALL
    )

# Ensure transaction import
if "use crate::transaction::TerminalTransaction;" not in text:
    text = text.replace("use std::io::{self, stdout, Write};", "use std::io::{self, stdout, Write};\nuse crate::transaction::TerminalTransaction;")

# Rewrite render logic
# Find:
#         let mut out = Vec::new();
#         match self.mode {
# And up to:
#         stdout_handle.write_all(&out)?;
#         stdout_handle.flush()?;

def replace_body(m):
    body = m.group(1)
    
    body = body.replace("let mut out = Vec::new();", "let mut tx = TerminalTransaction::new(writer, session.sync_updates());\ntx.begin();")
    body = body.replace("&mut out", "&mut tx.buffer")
    body = body.replace("out.extend_from_slice(", "tx.push(")
    body = body.replace("out.push(b'\\r')", "tx.push(b\"\\r\")")
    body = body.replace("write!(out,", "write!(tx.buffer,")
    
    body = body.replace("let _ = session.show_cursor();", "if let Some(cmd) = session.show_cursor() { tx.push(cmd); }")
    body = body.replace("let _ = session.hide_cursor();", "if let Some(cmd) = session.hide_cursor() { tx.push(cmd); }")
    
    body = re.sub(
        r"let mut stdout_handle = stdout\(\);\n\s*stdout_handle\.write_all\(&out\)\?;\n\s*stdout_handle\.flush\(\)\?;",
        "tx.commit()?;",
        body
    )
    body = re.sub(
        r"stdout_handle\.write_all\(&out\)\?;\n\s*stdout_handle\.flush\(\)\?;",
        "tx.commit()?;",
        body
    )
    return body

text = re.sub(r"(let mut out = Vec::new\(\);.*?Ok\(\(dirty_cells, total_cells, bytes, is_full_repaint\)\))", replace_body, text, flags=re.DOTALL)

# Same for commit, clear_live_region, insert_before_live
def replace_simple(m):
    body = m.group(1)
    body = body.replace("let mut out = Vec::new();", "let mut tx = TerminalTransaction::new(writer, session.sync_updates());\ntx.begin();")
    body = body.replace("&mut out", "&mut tx.buffer")
    body = body.replace("out.extend_from_slice(", "tx.push(")
    body = body.replace("out.push(b'\\r')", "tx.push(b\"\\r\")")
    body = body.replace("write!(out,", "write!(tx.buffer,")
    
    body = re.sub(
        r"let mut stdout_handle = stdout\(\);\n\s*stdout_handle\.write_all\(&out\)\?;\n\s*stdout_handle\.flush\(\)\?;",
        "tx.commit()?;",
        body
    )
    return body

text = re.sub(r"(pub fn commit\([\s\S]*?\{[\s\S]*?let mut out = Vec::new\(\);[\s\S]*?Ok\(\(\)\)\n    \})", replace_simple, text)
text = re.sub(r"(pub fn insert_before_live\([\s\S]*?\{[\s\S]*?let mut out = Vec::new\(\);[\s\S]*?Ok\(\(\)\)\n    \})", replace_simple, text)
text = re.sub(r"(pub fn clear_live_region\([\s\S]*?\{[\s\S]*?let mut out = Vec::new\(\);[\s\S]*?Ok\(\(\)\)\n    \})", replace_simple, text)

# Remove sync_updates parameter in new
text = text.replace("pub fn new(mode: RenderMode, sync_updates: bool)", "pub fn new(mode: RenderMode)")
text = text.replace("compiler: AnsiCompiler::new(sync_updates),", "compiler: AnsiCompiler::new(),")


with open("src/renderer.rs", "w") as f:
    f.write(text)
