# Streaming viewport forcing example

Run `cargo run --locked --example streaming_viewport`. This uses public LibGibson
interfaces and the repository's declared dependencies. It requires no network,
terminal, model provider or private consumer. All content is synthetic.

The executable checks an application-owned line cache with stable content IDs,
replacement invalidation, one cached width, a held block/row anchor, explicit
32-block retention and eviction receipts. Appending a streaming block leaves a
held historical view unchanged. Resize preserves the anchored block and clamps
its visual row; it does not promise an exact source-byte anchor after reflow.
The application owns source retention and all status/approval decisions.

At 40×15, 80×24 and 120×40, a headless scene reserves status and input rows. Cell
readback asserts those rows remain visible while the middle viewport contains
synthetic content. This is a layout example, not a protected-overlay, keyboard
focus or authority-enforcement API. Those interactions still need PTY acceptance.
Terminal control characters are refused before modifying the retained source.

`cargo run --locked --example streaming_viewport -- --mutate-stale-cache` must
fail: it deliberately reuses the prior streaming block's wrapped lines. The
reference independently rebuilds the retained blocks but shares LibGibson's
wrapping implementation, so agreement verifies invalidation/anchoring, not an
independent proof of Unicode wrapping.

This experiment introduces no stable library API. It still clones cached rows
while selecting the viewport. Retention bounds the number of blocks, not their
source byte lengths. Output backpressure, precise source anchors after resize,
focus, screen-reader use and live-terminal behavior are unqualified. A future
core primitive needs end-to-end layout/allocation and terminal measurements.
