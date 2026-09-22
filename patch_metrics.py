import re

with open("src/context.rs", "r") as f:
    text = f.read()

text = re.sub(
    r"pub struct RenderStats \{([\s\S]*?)\}",
    r"pub struct RenderStats {\1\n    pub history_insertions: u32,\n    pub insertion_repaints: u32,\n    pub insertion_bytes: usize,\n    pub anchor_resyncs: u32,\n}",
    text
)

with open("src/context.rs", "w") as f:
    f.write(text)
