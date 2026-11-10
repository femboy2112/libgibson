# Renaissance XXVI — renderer destination and qualification (working handoff)

This is a point-in-time engineering record, not release approval. Fetch the live
refs and inspect actual CI checkout commits before acting. Same-model-family
agent reviews are useful additional source passes, not independent human or
independent-model certification.

## Main and release

At the recorded snapshot, main is `e64402526562ce0d7b6d88c0b5f7bb31152574d6`,
version `0.7.0-dev`; exact-main development run `42748526447` passed. GitHub
prerelease `v0.6.0` and `bindings/go/v0.6.0` peel to
`492d0fec7727658d50b9a82025c784b7e427d6b0`. No v0.7 candidate has qualified.
Do not publish until the terminal, ABI, installed consumers, MSRV, preflight,
resource, downstream and independent-review gates pass at one release SHA.
Node J requires a separately published GPL engine adapter; the separate
engine port remains a local prototype and is not the in-tree framebuffer
adapter. A documented milestone decision or a separately authorized and
qualified external deliverable is still required.
Windows remains unverified.

## Destination identity

On #338 old head `7156134c98ce20dfe8c96b7f62bbb2c06c8ad817`, execute:

```
cargo +1.98.1 test --locked --test terminal_sovereignty_pty \
  direct_renderer_use_must_not_poison_context_output_state -- --exact --ignored --test-threads=1
```

It failed with FRAME_A still on the real PTY after a headless FRAME_B render
poisoned the renderer cache. A candidate Context guard marks direct renderer
output attempts, checks physical-state fields and detects used renderer/session
swaps. Its accepted prototype contract is fail-closed: it rejects the later
Context output and does not claim FRAME_B was delivered. This is a behavioral
compatibility change requiring review, not complete destination isolation.

A separate standalone Renderer A/B/A writer test still fails: the first
writer retains FRAME_A while the renderer cache says FRAME_B. It compares all
80x24 VT100 cells and cursor state against a fresh reference. The shared
`&mut dyn Write` API supplies no stable destination identity. #338 remains
HOLD, including for this explicitly ignored release-critical test. Do not
promote its green ordinary suite or a same-model-family source pass to release
qualification. Direct unmanaged writer and input paths are not subject to a
universal terminal lease guarantee.

## Release verification

#337 initially allowed a missing Go tool to end in `ALL CONSUMERS OK`; an
extracted-block missing-Go negative control reproduced the path. The draft
now makes Go mandatory and adds fake-tool presence, absence and build-failure
controls. A separate fixture mutated selected libraries during the subsequent
metadata query; the staging update snapshots and checks the selected .so/.a.
These controls are not a real installed SDK, an exact release-tag build, or a
sandbox against a hostile compiler or concurrent actor. Check the candidate
symbol set against an independently retained historical baseline and check C
signatures and struct layouts separately; symbol names alone are insufficient.
The latest PR head and its checkout/tree/CI must be verified live.

## Mathematical M0

#336 contains four fixed trusted TeX sources. Caller-controlled strings are not
fed to an unsandboxed typesetter. A prior full check incorrectly accepted an
integral substituted for the matrix and a terminal-state oracle ignored an
alternate-screen control. The draft adds PDF bounding-box row/column checks
and rejects unmodelled terminal controls. These tests do not establish complete
integral geometry, paper-quality math in a LibGibson-controlled real terminal,
font qualification, a safe arbitrary-input renderer, or a stable public API.
A Kitty graphics probe was separate from LibGibson math presentation.

## Integration

#333, #329, #327 and #338 overlap terminal ownership and must be qualified as a
combined state machine. #330 and #328 remain open; raw input recovery alone is
not full terminal recovery. #312 remains open: shifted legacy `!` and a Kitty
base-key `1` release can leave a hold, and public event histories need not
identify physical origin. Do not impose arbitrary timeouts on genuine holds.

Rebuild #325 against the eventually qualified runtime in an external installed
prefix and verify complete five-language transcripts, controls and PTY focus.
Keep #331's Unicode test as a decoder-model example, not a physical keyboard
claim. Do not merge this status handoff while it is stale.

## XXVI additional observations and controls

- A real PTY regression at #329 old head
  `f1dd2b3ad2bc21dbcafb5f1a8176498c2eccace0` reproduced a second raw-mode
  cleanup failure. Enable Kitty key-release reporting, catch an owner panic,
  call `enter_interactive`, reconcile ended Kitty reporting, then restore: the
  terminal remains raw. Direct panic/re-entry/restore is the positive control.
  Draft #329 head `974bc1264008fc76a2f984025f9f6ca99a34d596` stops late Kitty
  reconciliation from clearing a newly acquired raw-mode obligation; its
  isolated PTY and fault tests passed. It still needs
  exact-head CI, integration with #333/#327, and a combined independent review.
- #337 at `ff8f6c6fdeec0fb838c3c044cd34f96a41684eec` includes the Go and
  selected-library controls described above. A real staging build on prior
  head `49d5ba2` failed because the ordinary `cargo rustc` metadata query
  changed the selected archive. The new head stages the pre-query snapshots;
  real staging, checksums, and external C/C++ starters with a spaced prefix
  passed. It reused the previous build target, and the intervening commit
  changed only release scripts/docs: this is a staging witness, not a fresh
  exact-commit release artifact. Fake-Go controls do not qualify a Go consumer.
- #336 at `84e57e80d6ff19a72096e30e4791e6a04ed50aa2` passed its freshly
  built fixed-fixture check; matrix/aligned substitutions and an alternate
  screen capture were rejected. Hosted run `42924982464` passed 8/8 on
  synthetic merge `b49afd85b494f1683651260759e26f1395a1b0e2`, whose tree
  equals the PR head tree; it did not check out the head commit itself.
- #338 at `8c027a5fe98229bef7d6aed1da699f63ba9f24cd` contains the draft
  fail-closed Context contract and the still-failing standalone writer control;
  it merges #333's stronger stale-mouse test without rewriting history.
  The combined mouse and Context PTY run passed 25 tests with the standalone
  test ignored; running that test explicitly returned 101. A guard-removal
  mutant failed the PTY Context control. The Rust API behavior change and all
  relevant writer/session orderings require review before promotion.
- #333 at `2bd3dcdad3cdb524812d5c85f70933b352b131e5` adds a stale-call
  PTY oracle that detects individual mouse requests and torn escape prefixes.
  A planted partial escape passed the previous complete-sequence oracle and
  failed the new check; 11 focused tests passed with the mutation removed.

Useful commands, from isolated checkouts of the specified heads:

```
# #338: 8c027a5fe98229bef7d6aed1da699f63ba9f24cd
cargo +1.98.1 test --locked --test terminal_sovereignty_pty -- --test-threads=1
cargo +1.98.1 test --locked --test terminal_sovereignty_pty \
  standalone_renderer_must_not_share_cache_between_writers -- --exact --ignored --test-threads=1
# #329: 974bc1264008fc76a2f984025f9f6ca99a34d596
cargo +1.98.1 test --locked --test caught_panic_input_pty \
  key_reporting_reconciliation_after_reentry_preserves_raw_cleanup -- --exact --test-threads=1
# #337: ff8f6c6fdeec0fb838c3c044cd34f96a41684eec
scripts/release/tests/test-cleanroom-go.sh
scripts/release/tests/test-stage-sdk-target.sh
# #336: 84e57e80d6ff19a72096e30e4791e6a04ed50aa2
PYTHONNOUSERSITE=1 /usr/bin/python3 experiments/math_m0/check.py
```

Other recorded open heads, all needing live verification and reconciliation:
#333 `2bd3dcdad3cdb524812d5c85f70933b352b131e5`; #327
`22d6d78aac28b4098e43d8cba355a6753923b8ab`; #325
`0b922c9043c393f65d877701cb107d314b164167`; #331
`d1cd6016d9828000f21066bc76488ea4dbdd26a8`.

The explicit standalone command is expected to fail until a destination
contract is implemented. Fake-writer, fake-Go and PDF-geometry controls do not
replace real terminal capability evidence or exact release artifacts.

## Late CI and integration review

- Head-associated #329 development run `42925559795` passed 8/8, but
  checked out synthetic merge `637ee665243a729f3e8096f80d28c9edd05542d9`.
  Its tree differs from head `974bc1264008fc76a2f984025f9f6ca99a34d596`;
  it is not an exact-head execution. Reconcile against updated main and rerun.
- Read-only merge analysis found actual `session.rs` conflicts between #333
  and #329, and between #333 and #327. Keep the distinct panic mouse and input
  notifications, independent mouse/focus confirmation and cleanup flags,
  and both sets of fault tests. A textual merge does not qualify the combined
  lifecycle.
- Unverified inherited concern: #327 can request focus before entering raw
  input, including after a caught panic. A PTY experiment should inject focus
  sequences before polling, check echo/canonical flags and event order, and
  compare explicit raw entry and no-panic controls. Do not characterize this
  hypothesis as a reproduced regression or terminal support.
