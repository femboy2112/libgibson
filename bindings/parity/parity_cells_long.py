#!/usr/bin/env python3
"""FFI long-glyph cell-grid parity harness (Python side) — #142, v0.6 Gate K (K2).

Renders "a" + a 25-byte ZWJ family emoji (width 2) + "Z", styled, in a headless 24x4 Inline context
and serializes the whole grid via the NEW bulk Context.frame_cells, the GibsonCell.glyph_truncated
flag, and Context.frame_cell_glyph for each cell's full (untruncated) bytes. The line format is
identical to parity_cells_long.c and tests/ffi_cell_readback.rs:
    size <cols> <rows>
    <x> <y> <glyph_hex|-> <glyph_len> <trunc> <full_hex|-> <w> <cont> <transp> <fgct> <fgr> <fgg> <fgb> <bgct> <bgr> <bgg> <bgb> <bold> <dim> <ital> <under> <rev>

The CI step cmp's this == parity_cells_long.c == golden_cells_long.txt.
"""
import sys

from gibson import Context, GibsonColor, GibsonStyle, Node, RenderMode, WrapMode

# "a" + U+1F468 ZWJ U+1F469 ZWJ U+1F467 ZWJ U+1F466 + "Z": one 25-byte ZWJ cluster, width 2.
TEXT = "a\U0001f468‍\U0001f469‍\U0001f467‍\U0001f466Z"


def emit_cell(ctx, x, y, c):
    glyph = bytes(c.glyph[: c.glyph_len])
    ghex = glyph.hex() if c.glyph_len else "-"
    full = ctx.frame_cell_glyph(x, y)
    fhex = full.hex() if full else "-"
    trunc = 1 if c.glyph_truncated else 0
    fg, bg = c.style.fg, c.style.bg
    return (
        f"{x} {y} {ghex} {c.glyph_len} {trunc} {fhex} {c.width} {c.is_continuation} {c.transparent} "
        f"{fg.color_type} {fg.r} {fg.g} {fg.b} {bg.color_type} {bg.r} {bg.g} {bg.b} "
        f"{c.style.bold} {c.style.dim} {c.style.italic} {c.style.underline} {c.style.reverse}\n"
    )


def main():
    if len(sys.argv) < 2:
        sys.stderr.write("parity_cells_long_py: usage: parity_cells_long.py GRID_OUT\n")
        return 1
    grid_out = sys.argv[1]

    ctx = Context.headless(RenderMode.INLINE, 24, 4)
    try:
        style = GibsonStyle.make(fg=GibsonColor.rgb(200, 50, 50), bold=True)
        root = Node.col()
        root.add_child(Node.text(TEXT, style=style, wrap=WrapMode.NONE))
        ctx.set_root(root)
        ctx.render()
        cols, rows = ctx.frame_size()
        cells = ctx.frame_cells(0, 0)  # whole frame, row-major, one ABI call
        lines = [f"size {cols} {rows}\n"]
        for i, c in enumerate(cells):
            x, y = i % cols, i // cols
            lines.append(emit_cell(ctx, x, y, c))
    finally:
        ctx.close()

    with open(grid_out, "w") as o:
        o.write("".join(lines))
    return 0


if __name__ == "__main__":
    sys.exit(main())
