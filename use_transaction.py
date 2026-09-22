import re

with open("src/renderer.rs", "r") as f:
    text = f.read()

# Add transaction import
if "use crate::transaction::TerminalTransaction;" not in text:
    text = text.replace("use std::io::{self, stdout, Write};", "use std::io::{self, stdout, Write};\nuse crate::transaction::TerminalTransaction;")

# Rewrite render() body logic
# Search for: let mut out = Vec::new();
# Replace with: let mut tx = TerminalTransaction::new(writer, session.sync_updates()); tx.begin();
text = re.sub(
    r"let mut out = Vec::new\(\);",
    r"let mut tx = TerminalTransaction::new(writer, session.sync_updates());\n        tx.begin();",
    text
)

# Fix cursor visibility
text = re.sub(
    r"let _ = session\.show_cursor\(\);",
    r"if let Some(cmd) = session.show_cursor() { tx.push(cmd); }",
    text
)
text = re.sub(
    r"let _ = session\.hide_cursor\(\);",
    r"if let Some(cmd) = session.hide_cursor() { tx.push(cmd); }",
    text
)

# Fix out.extend_from_slice
text = text.replace("out.extend_from_slice(", "tx.push(")
text = text.replace("out.push(", "tx.push(&[")
text = text.replace("])", "])") # Wait, out.push(b'\r') -> tx.push(&[b'\r'])
# Let's use regex for out.push
text = re.sub(r"out\.push\((.*?)\);", r"tx.push(&[\1]);", text)

# Fix write!
text = text.replace("write!(out,", "write!(tx.buffer,")

# Replace stdout_handle writing
text = re.sub(
    r"let mut stdout_handle = stdout\(\);\n\s*stdout_handle\.write_all\(&out\)\?;\n\s*stdout_handle\.flush\(\)\?;",
    r"tx.commit()?;",
    text
)
# And some places don't have let mut stdout_handle
text = re.sub(
    r"stdout_handle\.write_all\(&out\)\?;\n\s*stdout_handle\.flush\(\)\?;",
    r"tx.commit()?;",
    text
)

# Fix move_to
text = text.replace("&mut out", "&mut tx.buffer")

with open("src/renderer.rs", "w") as f:
    f.write(text)
