import re

with open("src/scheduler.rs", "r") as f:
    text = f.read()

text = re.sub(
    r"pub struct RenderStats \{([\s\S]*?)\}",
    r"pub struct RenderStats {\1\n    pub history_insertions: u64,\n    pub insertion_repaints: u64,\n    pub insertion_bytes: u64,\n    pub anchor_resyncs: u64,\n}",
    text
)

with open("src/scheduler.rs", "w") as f:
    f.write(text)
