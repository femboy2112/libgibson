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

        # 2. Build declarative UI tree using modern chrome primitives (rule, rail)
        rule_style = GibsonStyle.make(
            fg=GibsonColor.named(ColorType.BRIGHT_MAGENTA),
            bold=True
        )

        rail_style = GibsonStyle.make(
            fg=GibsonColor.named(ColorType.BRIGHT_CYAN)
        )

        border_style = GibsonStyle.make(
            fg=GibsonColor.named(ColorType.BRIGHT_GREEN)
        )

        root = Node.col().percent_width(100.0).max_width(74.0).gap(1.0)
        root.add_child(Node.rule("LibGibson Python ctypes Demo", rule_style))

        rail = Node.rail(rail_style)
        rail.add_child(Node.text("Python Interface via ctypes FFI calling native Rust kernel."))
        root.add_child(rail)

        box = Node.border_box(BorderType.ROUNDED, border_style).width(60.0).height(3.0)
        box.add_child(Node.text("Dynamic language bindings calling Rust engine via C ABI"))
        root.add_child(box)

        # 3. Render
        ctx.set_root(root)
        ctx.render()

        # 4. Insert before live
        ctx.insert_before_live("[Python ctypes] Async background update inserted above active prompt.")

        # 5. Commit output
        ctx.commit("[Python ctypes] Frame rendered and committed successfully.")

        # 6. Query stats
        stats = ctx.stats()
        print(f"[Python ctypes] Stats: frames = {stats.frames}, bytes emitted = {stats.bytes_emitted}")

    print("[Python ctypes] Test completed successfully.")

if __name__ == "__main__":
    main()
