#!/usr/bin/env python3
"""
Python example demonstrating LibGibson usage through ctypes.
"""

from gibson import Context, Node, ColorType, BorderType, GibsonColor, GibsonStyle

def main():
    print("--- Running Python ctypes Example with LibGibson ---")

    with Context() as ctx:
        # 1. Commit initial line
        ctx.commit("[Python ctypes] Initialized LibGibson context.")

        # 2. Build declarative UI tree
        header_style = GibsonStyle.make(
            fg=GibsonColor.named(ColorType.BRIGHT_CYAN),
            bold=True
        )

        border_style = GibsonStyle.make(
            fg=GibsonColor.named(ColorType.BRIGHT_GREEN)
        )

        root = Node.col().width(70.0).gap(1.0)
        root.add_child(Node.text("Python Interface via ctypes FFI", header_style))

        box = Node.border_box(BorderType.ROUNDED, border_style).width(60.0).height(3.0)
        box.add_child(Node.text("Dynamic language bindings calling Rust engine via C ABI"))
        root.add_child(box)

        # 3. Render
        ctx.set_root(root)
        ctx.render()

        # 4. Commit output
        ctx.commit("[Python ctypes] Frame rendered and committed successfully.")

        # 5. Query stats
        stats = ctx.stats()
        print(f"[Python ctypes] Stats: frames = {stats.frames}, bytes emitted = {stats.bytes_emitted}")

    print("[Python ctypes] Test completed successfully.")

if __name__ == "__main__":
    main()
