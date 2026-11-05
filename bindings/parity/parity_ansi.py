#!/usr/bin/env python3
"""FFI pixel/ANSI *frame* parity harness (Python side) — #142, v0.6 Gate I.

Drives the canonical headless frame op-script through the Python ctypes binding
(Context.headless + ctx.rendered_output): a single RGB-styled, bold text node in a column,
rendered once. Writes the painted stats report to argv[1] and the raw painted ANSI frame bytes to
argv[2]. Unlike the Gate H stats harness there is NO fd redirection — a headless context captures
into an in-memory buffer, read back explicitly.

The CI step `cmp`s this against bindings/parity/parity_ansi.c and the committed golden
(bindings/parity/golden_ansi_stats.txt, golden_ansi.bin), which the Rust job verifies against the
native API. See tests/ffi_ansi_parity.rs for the full contract.
"""
import sys

from gibson import Context, GibsonColor, GibsonStyle, Node, RenderMode, WrapMode

# Canonical order, matching tests/ffi_ansi_parity.rs and parity_ansi.c; the timing field
# last_render_duration_micros is deliberately excluded.
FIELDS = [
    "frames",
    "skipped_frames",
    "dirty_cells",
    "total_cells",
    "frame_bytes",
    "full_repaints",
    "history_insertions",
    "insertion_repaints",
    "fast_insertions",
    "insertion_bytes",
    "anchor_resyncs",
    "commit_bytes",
    "control_bytes",
]


def main():
    if len(sys.argv) < 3:
        sys.stderr.write("parity_ansi_py: usage: parity_ansi.py STATS_OUT FRAME_OUT\n")
        return 1
    stats_out, frame_out = sys.argv[1], sys.argv[2]

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
        frame = ctx.rendered_output()
        s = ctx.stats()
    finally:
        ctx.close()

    with open(frame_out, "wb") as fo:
        fo.write(frame)
    with open(stats_out, "w") as o:
        for f in FIELDS:
            o.write(f"{f}={getattr(s, f)}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
