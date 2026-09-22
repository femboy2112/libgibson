import re

with open("src/ansi.rs", "r") as f:
    text = f.read()

text = text.replace("pub sync_updates: bool,\n", "")
text = text.replace("pub fn new(sync_updates: bool)", "pub fn new()")
text = text.replace("sync_updates,\n", "")

# Remove sync_updates logic
text = re.sub(
    r"        if self\.sync_updates \{\n            // Begin synchronized update[^\n]*\n            out\.extend_from_slice\(b\"\\x1b\[\?2026h\"\);\n        \}\n",
    "",
    text
)
text = re.sub(
    r"        if self\.sync_updates \{\n            // End synchronized update[^\n]*\n            out\.extend_from_slice\(b\"\\x1b\[\?2026l\"\);\n        \}\n",
    "",
    text
)

# Fix tests
text = text.replace("AnsiCompiler::new(false)", "AnsiCompiler::new()")
text = text.replace("AnsiCompiler::new(true)", "AnsiCompiler::new()")

# Delete test_compiler_synchronized_update_wrap
text = re.sub(
    r"    #\[test\]\n    fn test_compiler_synchronized_update_wrap\(\) \{\n.*?\n    \}\n",
    "",
    text,
    flags=re.DOTALL
)

with open("src/ansi.rs", "w") as f:
    f.write(text)
