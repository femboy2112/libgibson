#!/usr/bin/env python3
"""FFI styled cell-grid parity harness (Python side) — #142, v0.6 Gate J.

Renders the canonical headless op-script (an RGB-styled, bold "Gate-I" node, truecolor) and
serializes the whole resolved cell grid to argv[1] via Context.frame_size + Context.frame_cell.
The line format is identical to parity_cells.c and tests/ffi_cell_grid_parity.rs:
    size <cols> <rows>
    <x> <y> <glyph_hex|-> <w> <cont> <transp> <fgct> <fgr> <fgg> <fgb> <bgct> <bgr> <bgg> <bgb> <bold> <dim> <ital> <under> <rev>

The CI step `cmp`s this against parity_cells.c and the committed golden_cells.txt, which the Rust
job verifies against the native API. See tests/ffi_cell_grid_parity.rs for the full contract.
"""
import sys

from gibson import Context, GibsonColor, GibsonStyle, Node, RenderMode, WrapMode


def emit_cell(x, y, c):
    glyph_bytes = bytes(c.glyph[: c.glyph_len])
    hexs = glyph_bytes.hex() if c.glyph_len else "-"
    fg, bg = c.style.fg, c.style.bg
    return (
        f"{x} {y} {hexs} {c.width} {c.is_continuation} {c.transparent} "
        f"{fg.color_type} {fg.r} {fg.g} {fg.b} {bg.color_type} {bg.r} {bg.g} {bg.b} "
        f"{c.style.bold} {c.style.dim} {c.style.italic} {c.style.underline} {c.style.reverse}\n"
    )


def main():
    if len(sys.argv) < 2:
        sys.stderr.write("parity_cells_py: usage: parity_cells.py GRID_OUT\n")
        return 1
    grid_out = sys.argv[1]

    ctx = Context.headless(RenderMode.INLINE, 24, 4)
    try:
        style = GibsonStyle.make(
            fg=GibsonColor.rgb(255, 128, 0),
            bg=GibsonColor.rgb(16, 32, 64),
            bold=True,
        )
        root = Node.col()
        root.add_child(Node.text("Gate-I", style=style, wrap=WrapMode.NONE))
        ctx.set_root(root)
        ctx.render()
        cols, rows = ctx.frame_size()
        lines = [f"size {cols} {rows}\n"]
        for y in range(rows):
            for x in range(cols):
                lines.append(emit_cell(x, y, ctx.frame_cell(x, y)))
    finally:
        ctx.close()

    with open(grid_out, "w") as o:
        o.write("".join(lines))
    return 0


if __name__ == "__main__":
    sys.exit(main())
