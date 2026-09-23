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

        # 4. Language-neutral structured rich text
        from gibson import Line, RichText, ColorType as CT
        label = GibsonStyle.make(fg=GibsonColor.named(CT.BRIGHT_YELLOW), dim=True)
        value = GibsonStyle.make(fg=GibsonColor.named(CT.WHITE), bold=True)
        line = Line().add_span("RichText: ", label).add_span("one line, many styles", value)
        rich = RichText().add_line(line)
        ctx.insert_rich_text_before_live(rich)
        ctx.commit_rich_text(rich)

        # 5. Safe insertion: controls in the text are neutralized by the engine.
        ctx.insert_text_before_live("[Python ctypes] Async background update inserted above active prompt.")

        # 5b. Raw escape hatch: explicitly unchecked and unsanitized.
        ctx.insert_raw_lines_before_live_unchecked("[Python ctypes] Raw notice (unchecked path).")

        # 6. Commit output
        ctx.commit("[Python ctypes] Frame rendered and committed successfully.")

        # 7. Query stats (versioned struct)
        stats = ctx.stats()
        print(f"[Python ctypes] ABI v{stats.abi_version}: frames = {stats.frames}, "
              f"frame_bytes = {stats.frame_bytes}, anchor_resyncs = {stats.anchor_resyncs}")

    print("[Python ctypes] Test completed successfully.")

if __name__ == "__main__":
    main()
