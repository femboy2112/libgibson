# Renaissance XXXI — output identity and consumer pressure

This is a checkpoint, not a substitute for fetching live refs. Baseline main was
`eb38fc3baa2cba05acd0b4c280b72b686c03ba36`; exact push CI 43014010195 had eight
successful jobs. The latest published Engineering Alpha remained v0.6.0 and its
Go tag, both peeled to `492d0fec7727658d50b9a82025c784b7e427d6b0`. The source
version was 0.7.0-dev; v0.7.0 remained HOLD without a release candidate.

All eight XXX main merges, including #323, are already present. Do not replay
#340, #337, #333, #329, #327, #325, #336 or #323. #331 was recognized by GitHub
as merged via ancestry through #323; #342 was closed as incorporated by ancestry.

## Destination and lifecycle evidence

The existing stack is draft research, not an independent merge train: #338
`da9322811e4280bb486317126a42312af3c8d2d4` → #339
`7b839e944f152920598484bbe8e95f1ab9e61509` → #343
`0f8014ece2045353e31256ae0cbb6724fb1dd64b`. #339 had failing Rust and release
preflight checks. #343's green checks do not settle inherited compatibility.
The two original standalone A/B/A controls were rerun at #338's exact head:
both exited 101 with A instead of B at VT100 cell (0,6), including the control
that retargets a single writer object internally.

The stacked research continuation `research/xxxi-endpoint-integration` at
`f65f31291ed70c1465616535ab5807fe2f60d95e` contains a normal merge of the
checkpoint main and intentionally failing ordinary acceptance tests. It is HOLD.
In a Linux two-PTY child without a controlling terminal, the original #343 getter
followed redirected stdout B (120×40), although its owned output endpoint was A
(40×15). The experimental captured-descriptor getter passed different-size,
matched-size and controlling-A query controls; nine existing endpoint PTY tests
passed. The getter change is not qualified for post-restore/error fallbacks,
FFI clients, or non-Linux Unix platforms.

At that branch, a focused four-target run passed 38 test entries (including
child-helper entries), zero failed or ignored. It covered the original distinct-
writer and internally-retargetable-writer A/B/A controls, full-grid managed
stdout redirection, bound/unbound renderer safety and the immutable native ANSI
golden. These positives do not remove the separate red resize and raw controls or
qualify the changed unbound inline/history Rust behavior.

A fresh SIGWINCH probe resized A to 50×20 after redirecting stdout to B. The
experimental getter reported 50×20 while the delivered event reported B's
120×40. The controlling-A control reported 50×20 for both. The ordinary
`resize_event_matches_the_acquired_output_geometry` test exits 101. Resize event
acquisition and endpoint lifetime need a coherent solution, not just a getter fix.

Draft #344 head `01177c3a9c9c84086483c6f18005d15e2d99a207` and issue #345 record
a separate current-main Linux repro: after raw input is acquired on PTY A,
replacing stdin with B and restoring returns success while A remains raw and B's
termios changes. The unchanged-stdin control restores A and preserves B. This is
a deliberately hostile descriptor-replacement scenario; its support boundary and
safe behavior need an explicit decision. It is not evidence of an ordinary
fixed-descriptor regression. The test resets its disposable PTYs after observation.
It remains ordinary and red. Do not merge #344 as a repair.
Hosted Linux CI run 43077430652 also reached that same failing test with the
same positive and retarget observations; this is a second host running the same
test implementation, not an independent oracle.

Source-only same-model-family review found that a complete input-identity repair
must consider lazy source opening, the decoder's Crossterm raw-state dependency,
owner-panic recovery, host-owned raw mode and in-flight polls at lease handoff.
Those points have not been independently runtime-qualified. No human or different-
model independent certification was obtained. #312, #328 and #330 remain open.
The shifted-symbol physical-key ambiguity in #312 must not be papered over by
arbitrary timeouts or fabricated identity.

## Streaming viewport example

PR #346, head `1f44546a791c3002122a296c88dedf791ab4d3d5`, adds logical source-byte,
per-block and cached-row budgets to the example, explicit eviction receipts and
atomic rejected replace/resize operations. Tabs are rejected because the cell
renderer would omit them; held anchors are persistently clamped on shrink or
reflow. The example is not a new public text-flow API or an RSS limit.

Old behavior failed controls for unbounded single-block growth, disappearing tabs
and jumping anchors. A separate same-model source review found the old large-
layout body oracle checked only the status/input rows. The revised tests validate
ten fixed body cells in the 40×15, 80×24 and 120×40 examples, plus a separate full
eight-cell row; correct and corrupted fixtures constrain those tests. Seventeen
focused tests, the example, synthetic 1/10/100/1,000-turn workload and Clippy
passed; stale-cache, byte-guard and body mutations were rejected. The timing
comparison shares a wrapping implementation and is not a terminal/backpressure or
independent Unicode proof. Check live exact-head CI and review before any merge.

## Consumer and math boundaries

An isolated downstream comparison at this checkpoint used the published v0.6
baseline and exact public development commit `eb38fc3baa2cba05acd0b4c280b72b686c03ba36`.
Baseline idle and attached-input PTYs passed 1/1 and 2/2. An isolated candidate
with direct `run_once` calls and eager input entry passed attached-input 3/3,
demo/resize/idle 3/3 and the explicit idle witness 1/1 at 25 calls/s. A line
queued after raw entry and before readiness was received once without echo; an
ECHO mutation failed. Input typed before raw entry, slow-inference latency and
the complete release canary remain unverified. The private candidate retained
separate production and test-instrumentation patches; the production patch alone
was not built, and no downstream main or capabilities were changed.

After the downstream main advanced by a documentation-only merge, the same
production-only substitutions were separately applied to that newer exact source
and built without the earlier test instrumentation. Its original attached-input
PTYs passed 2/2, idle console/demo PTY passed 1/1, and headless selfcheck passed
at four sizes/color settings. Those existing tests cover input at readiness, not
typing before readiness. This is still a scoped consumer witness, not the full
release canary.

Math M0 remains fixed-trusted-input, headless cell/raster research. A standalone
Kitty 0.32.2 RGB graphics probe on a freshly isolated Xvfb display received the
valid query response, rejected an invalid format, and a screenshot showed a
filled red rectangle with no exact-green control pixels. This used a virtual
framebuffer and standalone terminal transport: it is not a LibGibson-managed
pixel element, a physical-monitor observation, or evidence of paper-quality math.
The M0 fixture runner accepts only fixed trusted sources; its TeX and toolchain
environment is not sandboxed.

## Decisive next actions

1. Design one owned input endpoint and lifecycle across raw setup, polling, panic,
   restore and generation handoff; preserve CR/LF/Ctrl+J and host-owned raw mode.
   Re-run #344 on main and the integrated stack, with observable in-flight polls.
2. Make resize events and rendering derive geometry from the same qualified output
   endpoint; test no-controlling tty, controlling tty, HUP, post-restore and FFI.
3. Resolve #339's unbound inline/history and renderer/writer exclusivity behavior;
   retain original full-grid A/B/A, the 77-byte ANSI golden, scene/damage tests and
   old installed clients. Do not merge an intentionally red parent to reach a child.
4. Re-run fresh SDK parity and full release preflight only on a single candidate
   that passes the preceding gates; repeat the downstream canary on that exact SHA.
5. Resolve the roadmap's separately published GPL adapter requirement or obtain a
   documented approved scope decision. Windows remains unverified.

First commands:

```sh
git status --short --branch
git worktree list --porcelain
git fetch origin
git rev-parse origin/main
gh pr list --repo femboy2112/libgibson --state open --limit 100
gh pr view 344 --repo femboy2112/libgibson --json headRefOid,statusCheckRollup
git worktree add --detach ../libgibson-repro-344 origin/research/xxxi-raw-retarget-repro
cd ../libgibson-repro-344
cargo +1.98.1 test --locked --test raw_input_retarget_pty raw_cleanup_tracks_the_terminal_where_raw_was_enabled -- --exact --nocapture --test-threads=1
```

The test is expected to fail until the defect or its supported boundary is resolved.
Preserve existing tags,
ABI v1, Rust 1.85 and frozen audio; do not force-push, bypass protection, publish
registries, delete remote branches or alter a downstream main. Normal qualified
LibGibson merges and a fully gated GitHub Engineering Alpha remain authorized.
