import re

with open("src/renderer.rs", "r") as f:
    text = f.read()

# Replace self.compiler.compile_full(prev) with diff
text = re.sub(
    r"let bytes = self\.compiler\.compile_full\(prev\);",
    r"let diff = crate::diff::compute_diff(None, prev);\n                let bytes = self.compiler.compile(&diff);",
    text
)

# Fix insert_node_before_live return value in context.rs
with open("src/context.rs", "r") as f2:
    ctx_text = f2.read()

ctx_text = ctx_text.replace("+= bytes;", "+= bytes as u64;")

with open("src/renderer.rs", "w") as f:
    f.write(text)

with open("src/context.rs", "w") as f2:
    f2.write(ctx_text)
