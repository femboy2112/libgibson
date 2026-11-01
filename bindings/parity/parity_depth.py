#!/usr/bin/env python3
"""FFI color-depth *downgrade* parity harness (Python side) — #142, v0.6 Gate J.

Drives the canonical headless frame op-script (a single RGB-styled, bold text node in a column)
through the Python ctypes binding at a chosen color depth: Context.headless, then
ctx.set_color_depth(DEPTH), then render. Writes the raw painted ANSI frame bytes to argv[2].
DEPTH (argv[1]) is a ColorDepth int (0=MONO, 1=ANSI16, 2=ANSI256, 3=TRUECOLOR).

The CI step `cmp`s this against bindings/parity/parity_depth.c and the committed golden for that
depth (golden_depth_{mono,ansi16,ansi256}.bin), which the Rust job verifies against the native API.
See tests/ffi_color_depth_parity.rs for the full contract.
"""
import sys

from gibson import Context, GibsonColor, GibsonStyle, Node, RenderMode, WrapMode


def main():
    if len(sys.argv) < 3:
        sys.stderr.write("parity_depth_py: usage: parity_depth.py DEPTH FRAME_OUT\n")
        return 1
    depth, frame_out = int(sys.argv[1]), sys.argv[2]

    ctx = Context.headless(RenderMode.INLINE, 24, 4)
    try:
        ctx.set_color_depth(depth)
        style = GibsonStyle.make(
            fg=GibsonColor.rgb(255, 128, 0),
            bg=GibsonColor.rgb(16, 32, 64),
            bold=True,
        )
        root = Node.col()
        root.add_child(Node.text("Gate-I", style=style, wrap=WrapMode.NONE))
        ctx.set_root(root)
        ctx.render()
        frame = ctx.rendered_output()
    finally:
        ctx.close()

    with open(frame_out, "wb") as fo:
        fo.write(frame)
    return 0


if __name__ == "__main__":
    sys.exit(main())
