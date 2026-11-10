import gibson

with gibson.Context.headless(gibson.RenderMode.FULLSCREEN, 40, 8) as ctx:
    root = gibson.Node.col()
    root.add_child(gibson.Node.text("Hello, LibGibson"))
    ctx.set_root(root)
    ctx.render()
    assert ctx.frame_size() == (40, 8)
    assert ctx.frame_cell_glyph(0, 0) == b"H"
    print("Python headless frame 40x8 OK")
