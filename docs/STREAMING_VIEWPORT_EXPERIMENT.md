# Streaming viewport forcing example

Run `cargo run --locked --example streaming_viewport`. This uses public LibGibson
interfaces and the repository's declared dependencies. It requires no network,
terminal, model provider or private consumer. All content is synthetic.

The executable checks an application-owned line cache with stable content IDs,
replacement invalidation, one cached width, a held block/row anchor, explicit
32-block retention and eviction receipts. Appending a streaming block leaves a
held historical view unchanged. Resize preserves the anchored block and clamps
its visual row; it does not promise an exact source-byte anchor after reflow.
The clamped visual row is stored when a replacement or resize shortens the block,
so later growth does not jump back to an earlier row. The application owns source
retention and all status/approval decisions.

At 40×15, 80×24 and 120×40, a headless scene reserves status and input rows. Cell
readback asserts those rows remain visible while the middle viewport contains
synthetic content. This is a layout example, not a protected-overlay, keyboard
focus or authority-enforcement API. Those interactions still need PTY acceptance.
Terminal control characters, including tabs, are refused before modifying the
retained source. Callers must provide spaces if they want visible indentation;
the example never silently drops tabs or rewrites the stored source.

`cargo run --locked --example streaming_viewport -- --mutate-stale-cache` must
fail: it deliberately reuses the prior streaming block's wrapped lines. The
reference independently rebuilds the retained blocks but shares LibGibson's
wrapping implementation, so agreement verifies invalidation/anchoring, not an
independent proof of Unicode wrapping.

This experiment introduces no stable library API. Output backpressure, precise
source anchors after resize, focus, screen-reader use and live-terminal behavior
are unqualified. A future core primitive needs end-to-end layout/allocation and
terminal measurements.

The example applies three logical limits in addition to its block-count limit:
retained source UTF-8 bytes, retained cached visual rows, and individual block
source bytes (32 KiB, 4,096 rows, and 8 KiB per block in the ordinary demo).
An append validates its whole block; if it fits alone, it evicts oldest blocks
until all limits fit and returns their IDs. A replacement that would
exceed a limit is rejected without changing source, cache or anchor. A resize
that would exceed the row limit is likewise rejected without changing width or
content. Source text is never silently truncated. The caller must decide how to
handle a budget error or eviction receipt.

The budgets constrain logical source bytes and row cardinality, not process RSS,
allocator capacity, a caller-owned incoming String, temporary wrap allocations,
or row clones in viewport selection. Reflow preserves a block/visual-row anchor
with clamping, not an exact source-byte offset. `--workload` exercises synthetic
1, 10, 100, and 1,000-turn histories at two widths. Its timings compare the
cached experiment with a complete rebuild using the same wrapping function;
they are local CPU observations, not independent Unicode, PTY, or latency
validation. The tests include separately specified Unicode and ASCII row fixtures.
Run them with `cargo test --locked --example streaming_viewport budget_tests`.
