import os
import glob
import re

for file in glob.glob("tests/*.rs") + glob.glob("examples/*.rs"):
    with open(file, "r") as f:
        text = f.read()

    text = re.sub(r"Renderer::new\(([^,]*?),\s*false\)", r"Renderer::new(\1)", text)
    text = re.sub(r"Renderer::new\(([^,]*?),\s*true\)", r"Renderer::new(\1)", text)
    text = re.sub(r"AnsiCompiler::new\(false\)", r"AnsiCompiler::new()", text)
    text = re.sub(r"AnsiCompiler::new\(true\)", r"AnsiCompiler::new()", text)

    text = re.sub(r"\.render\((.*?),\s*&mut session\)", r".render(\1, &mut session, &mut std::io::stdout())", text)
    text = re.sub(r"\.commit\((.*?),\s*&mut session\)", r".commit(\1, &mut session, &mut std::io::stdout())", text)
    text = re.sub(r"\.insert_before_live\((.*?),\s*&mut session\)", r".insert_before_live(\1, &mut session, &mut std::io::stdout())", text)

    with open(file, "w") as f:
        f.write(text)
