import re

def rewrite():
    # 1. ansi.rs
    with open("src/ansi.rs", "r") as f: text = f.read()
    text = re.sub(r"pub sync_updates: bool,\n\s*", "", text)
    text = text.replace("pub fn new(sync_updates: bool)", "pub fn new()")
    text = re.sub(r"if self\.sync_updates \{\n\s*// Begin synchronized update[^\n]*\n\s*out\.extend_from_slice\(b\"\\x1b\[\?2026h\"\);\n\s*\}\n", "", text)
    text = re.sub(r"if self\.sync_updates \{\n\s*out\.extend_from_slice\(b\"\\x1b\[\?2026l\"\);\n\s*\}\n", "", text)
    with open("src/ansi.rs", "w") as f: f.write(text)

    # 2. session.rs
    with open("src/session.rs", "r") as f: text = f.read()
    text = text.replace('b"\\x1b[0m\\x1b[?2026l\\r\\n"', 'b"\\x1b[0m\\x1b[?2026l\\x1b[?7h\\x1b[?25h\\r\\n"')
    text = text.replace('b"\\x1b[0m\\x1b[?2026l"', 'b"\\x1b[0m\\x1b[?2026l\\x1b[?7h\\x1b[?25h"')
    
    text = re.sub(r"pub fn hide_cursor.*?\n    }", lambda _: "pub fn hide_cursor(&mut self) -> Option<&'static [u8]> {\n        if !self.is_tty || self.cursor_hidden { return None; }\n        self.cursor_hidden = true;\n        Some(b\"\\x1b[?25l\")\n    }", text, flags=re.DOTALL)
    text = re.sub(r"pub fn show_cursor.*?\n    }", lambda _: "pub fn show_cursor(&mut self) -> Option<&'static [u8]> {\n        if !self.is_tty || !self.cursor_hidden { return None; }\n        self.cursor_hidden = false;\n        Some(b\"\\x1b[?25h\")\n    }", text, flags=re.DOTALL)
    with open("src/session.rs", "w") as f: f.write(text)

    # 3. renderer.rs
    with open("src/renderer.rs", "r") as f: text = f.read()
    if "use crate::transaction::TerminalTransaction;" not in text:
        text = text.replace("use std::io::{self, stdout, Write};", "use std::io::{self, stdout, Write};\nuse crate::transaction::TerminalTransaction;")
    
    text = text.replace("pub fn new(mode: RenderMode, sync_updates: bool)", "pub fn new(mode: RenderMode)")
    text = text.replace("compiler: AnsiCompiler::new(sync_updates)", "compiler: AnsiCompiler::new()")
    text = re.sub(r"pub fn set_sync_updates[^\}]+}", "", text) # remove set_sync_updates

    for fn_name in ["render", "commit", "insert_before_live", "clear_live_region", "commit_node", "insert_node_before_live"]:
        text = re.sub(r"(pub fn " + fn_name + r"\([\s\S]*?session: &mut TerminalSession)(?:\s*,\s*writer: &mut dyn Write)?\s*\)", r"\1, writer: &mut dyn Write)", text)

    text = re.sub(r"let mut stdout_handle = stdout\(\);\s*", "", text)
    text = re.sub(r"stdout_handle\.write_all\(&out\)\?;", "writer.write_all(&out)?;", text)
    text = re.sub(r"stdout_handle\.flush\(\)\?;", "writer.flush()?;", text)

    def replacer(m):
        body = m.group(1)
        body = body.replace("let mut out = Vec::new();", "let mut tx = TerminalTransaction::new(writer, session.sync_updates());\ntx.begin();")
        body = body.replace("out.extend_from_slice(", "tx.push(")
        body = body.replace("out.push(b'\\r')", "tx.push(b\"\\r\")")
        body = body.replace("write!(out,", "write!(tx.buffer,")
        body = body.replace("&mut out", "&mut tx.buffer")
        body = body.replace("let _ = session.show_cursor();", "if let Some(cmd) = session.show_cursor() { tx.push(cmd); }")
        body = body.replace("let _ = session.hide_cursor();", "if let Some(cmd) = session.hide_cursor() { tx.push(cmd); }")
        body = body.replace("writer.write_all(&tx.buffer)?;\n        writer.flush()?;", "tx.commit()?;")
        return body

    text = re.sub(r"(let mut out = Vec::new\(\);[\s\S]*?writer\.flush\(\)\?;)", replacer, text)
    
    text = text.replace("self.commit(&joined, session)", "self.commit(&joined, session, writer)")
    text = text.replace("self.insert_before_live(&line_refs, session)", "self.insert_before_live(&line_refs, session, writer)")

    with open("src/renderer.rs", "w") as f: f.write(text)

    # 4. context.rs
    with open("src/context.rs", "r") as f: text = f.read()
    text = text.replace("Renderer::new(mode, sync)", "Renderer::new(mode)")
    text = text.replace("self.renderer.set_sync_updates(self.session.sync_updates());", "")

    for method in ["render", "commit", "commit_node", "insert_before_live", "insert_node_before_live", "clear_live_region"]:
        text = re.sub(r"self\.renderer\." + method + r"\((.*?),\s*&mut self\.session\)", r"self.renderer." + method + r"(\1, &mut self.session, &mut std::io::stdout())", text)
    
    with open("src/context.rs", "w") as f: f.write(text)

rewrite()
