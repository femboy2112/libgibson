with open("src/context.rs", "r") as f:
    text = f.read()

text = text.replace("                Ok(())", "                Ok(paint_ctx)")
with open("src/context.rs", "w") as f:
    f.write(text)

with open("src/ffi.rs", "r") as f:
    text = f.read()

text = text.replace("Ok(()) => GibsonStatus::Ok,", "Ok(_) => GibsonStatus::Ok,")
with open("src/ffi.rs", "w") as f:
    f.write(text)
