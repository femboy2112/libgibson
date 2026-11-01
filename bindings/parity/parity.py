#!/usr/bin/env python3
"""FFI behavioral parity harness (Python side) — #142, v0.6 Gate H.

Drives the canonical non-TTY plain-scrollback op-script through the Python ctypes binding,
reads ctx.stats(), and writes a canonical stats report to argv[1]. fd 1 is redirected to
argv[2] BEFORE the context is created, so the engine runs its non-TTY plain path (matching the
Rust golden's session.is_tty=false) and the committed byte stream is captured as the wire
output. Nothing is printed to stdout once fd 1 is redirected — this output is `cmp`'d.

The CI step `cmp`s this against bindings/parity/parity.c's output and the committed golden
(bindings/parity/golden_{stats,wire}.txt), which the Rust job verifies against the native API.
See tests/ffi_stats_parity.rs for the full contract.
"""
import os
import sys

from gibson import Context, Node

# Canonical order, matching tests/ffi_stats_parity.rs and parity.c; the timing field
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
        sys.stderr.write("parity_py: usage: parity.py STATS_OUT WIRE_OUT\n")
        return 1
    stats_out, wire_out = sys.argv[1], sys.argv[2]

    # Redirect fd 1 to the wire file BEFORE creating the context: non-TTY plain path + wire
    # capture. gibson.py's loaded .so writes to fd 1, so this must precede Context().
    fd = os.open(wire_out, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o644)
    os.dup2(fd, 1)
    os.close(fd)

    with Context() as ctx:  # RenderMode.INLINE default
        ctx.commit("alpha")
        ctx.commit("beta gamma")
        ctx.insert_text_before_live("delta")
        root = Node.col()
        root.add_child(Node.text("live"))
        ctx.set_root(root)
        ctx.render()
        ctx.commit("omega")
        s = ctx.stats()

    with open(stats_out, "w") as o:
        for f in FIELDS:
            o.write(f"{f}={getattr(s, f)}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
