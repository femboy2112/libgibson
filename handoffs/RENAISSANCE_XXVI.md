# Renaissance XXVI — renderer destination and qualification (working handoff)

> Historical archive: statements, queues and CI below describe the recorded snapshot.
> Superseded by [Renaissance XXX](RENAISSANCE_XXX.md); fetch live state before acting.

This is a point-in-time engineering record, not release approval. Fetch the live
refs and inspect actual CI checkout commits before acting. Same-model-family
agent reviews are useful additional source passes, not independent human or
independent-model certification.

The later XXVII updates supersede older #333 and #338 heads and check status
in this historical record. At that recorded snapshot, this handoff was a draft and was not on main.

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
claim. The recorded snapshot instructed: do not merge the status handoff while stale.
This file is now historical evidence; use Renaissance XXX for the later snapshot.

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
- #333 head-associated run `42926618911` passed 8/8 on synthetic merge
  `3e7051775adc9cdb554fc96228960bc48c4bf677`; its tree equals the PR head tree.
- #338 head-associated run `42926746890` passed 8/8 on synthetic merge
  `61798aef2bac8738d61410204ff330b9416bb014`; its tree equals the PR head tree.
  The explicitly run standalone writer control still fails, overriding green CI.
- #337 head-associated development run `42925970286` passed 8/8 and
  publication-free preflight run `42925970169` passed on synthetic merge
  `cb1e6bfdfbccf0e45af05229d116d607610a448d`, whose tree equals the PR head
  tree. These are not the future tagged commit, and separate ABI history,
  signature/layout and final-candidate consumer checks still apply.

## XXVII update — stale screen control and renderer work in progress

At this snapshot, main is still `e64402526562ce0d7b6d88c0b5f7bb31152574d6`;
there have been no merges in this update. The latest published release remains
the `v0.6.0` GitHub prerelease; both that tag and the Go module tag peel to
`492d0fec7727658d50b9a82025c784b7e427d6b0`. v0.7 is HOLD, with no qualified
release candidate. Verify these refs and the checks again before acting.

- #333 now points to `d9b2c2a648c4408084e24359d0cf6226574006eb`. The
  stale-screen PTY test brackets the two stale operations and rejects any
  escape in that interval. Synthetic controls check complete
  alternate-screen requests and torn prefixes. An injected torn `ESC[?104`
  inside the real PTY interval passed the earlier full-request oracle (exit 0)
  and failed the strengthened oracle (exit 101); the normal focused suite
  passed 12/12 (exit 0). The local agent who implemented the test is from the
  same model family as the other reviews. This is not independent human or
  different-model certification. The new head requires finished CI and a
  review before any merge; earlier CI qualified only the earlier head.
- #338 remains at `8c027a5fe98229bef7d6aed1da699f63ba9f24cd`, based on the
  earlier #333 head. Its standalone A/B/A writer acceptance test is still
  ignored in ordinary CI and failed when explicitly run on that head. The
  Context fail-closed path does not present FRAME_B and does not resolve the
  standalone writer contract. A cursor-recovery investigation is uncommitted
  and in progress; it has no qualifying PR head or test verdict here. Do not
  assume those local changes are retained in the remote branch or treat them
  as a verified fix. Reconcile #333 without rewriting history and qualify the
  resulting exact #338 head only after the failing control and wider writer,
  cursor, partial-write and lifecycle matrix pass.
- #330 and #328 stay open, as does the #312 physical-key provenance issue.
  Continue #333, #329 and #327 serially, preserving independent panic notices,
  cleanup obligations and request confirmations. A same-model-family pass is
  not an external certification. No fresh external-consumer canary or new
  release qualification is claimed here.

Reproduce the #333 controls from its specified head:

```sh
cargo +1.98.1 test --locked --test mouse_lease_pty -- --test-threads=1
GIBSON_TEST_STALE_SCREEN_TORN=1 cargo +1.98.1 test --locked \
  --test mouse_lease_pty restored_session_cannot_switch_a_new_owners_screen \
  -- --exact --test-threads=1
```

The second command is an intentional mutation and must fail; remove the
environment variable to run the acceptance suite. For #338, run the explicit
standalone control shown earlier, with its `--ignored` flag while it remains
ignored. Do not confuse the planted torn-sequence control with proof that the
terminal applies a particular protocol or that a physically torn write is
atomic.

## XXVII follow-up — cursor recovery, destination contract still open

#338 is now `da9322811e4280bb486317126a42312af3c8d2d4`, a test-only change
after `be38740fe108a9dc1be04fc3eb6eadd1d34579d0`. The latter merge commit
includes #333's `d9b2c2a648c4408084e24359d0cf6226574006eb` and cursor work
at `6fef3ff6a127dfbe51ae7f20d2b8ba98bae38ae4`. The preceding statement that
the cursor work was uncommitted is historical. Main is still
`e64402526562ce0d7b6d88c0b5f7bb31152574d6`; no merge or release is recorded
in this follow-up. Check PR heads and final CI live.

A frame that failed before output could advance session cursor bookkeeping;
the previous renderer then skipped a hide/show request on retry. The draft
preserves cursor cleanup obligations on uncertain writes and tracks cursor
visibility uncertainty separately from inline-region recovery. The added
controls exercise both cursor-visibility directions, fullscreen and inline,
every tested frame write cut, flush failures with all or none of the bytes
delivered, and inline history operations before retry. The new control failed
on the old source, and a mutation discarding the uncertainty failed the new
control. A scoped local mouse/renderer/PTY run at `be38740` passed 63 tests
with one ignored. A separate source reviewer was in the same model family;
there is no independent human or different-model certification.

The scoped run and the still-failing control used:

```sh
cargo +1.98.1 test --locked --test mouse_lease_pty \
  --test terminal_sovereignty_pty --test renderer_partial_write \
  --test renderer_depth_repaint --test renderer_depth_repaint_failed_write \
  --test whole_renderer_vt100 -- --test-threads=1
cargo +1.98.1 test --locked --test terminal_sovereignty_pty \
  standalone_renderer_must_not_share_cache_between_writers \
  -- --exact --ignored --test-threads=1
```

At `be38740`, the first exited 0 (63 passed, one ignored); the second exited
101. The old-source red test was named
`cursor_hide_is_retried_after_zero_byte_frame_failure`;
the final expanded test is `cursor_visibility_is_retried_after_write_and_flush_failures`.
For the cursor mutation, local assignments setting
`cursor_visibility_uncertain = true` were removed, then this command exited
101; the source was restored and those assignments were subsequently
refactored into `poison_physical_state`:

```sh
cargo +1.98.1 test --locked --test terminal_sovereignty_pty \
  inline_history_does_not_forget_uncertain_cursor_visibility \
  -- --exact --test-threads=1
```

The ignored test is a release-critical standalone arbitrary-writer A/B/A
control; it remains unresolved, so #338 remains **HOLD**. At `da93228`, a
second deliberately ignored control retargets the *same writer object* to a
different destination and fails at cell (0,6), exit 101:

```sh
cargo +1.98.1 test --locked --test terminal_sovereignty_pty \
  standalone_renderer_must_not_assume_writer_address_identifies_target \
  -- --exact --ignored --test-threads=1
```

The ordinary `terminal_sovereignty_pty` suite at that head passed 17 tests
with both release-critical controls ignored, and targeted Clippy passed.
Rust 1.85 locked library checking passed at `be38740`; the later delta is
test-only. None of these outcomes resolves the two failures. The Context guard
still rejects ambiguous reuse rather than presenting FRAME_B. A proposed design
is to reset and emit the complete frame and cursor for each unmanaged
arbitrary-writer fullscreen call, while retaining differential updates on a
managed target with an explicit identity. Inline origin, commit, insert and
clear operations need an explicit target contract or a returned error. This
is a candidate design, not an implementation or a qualified API contract;
it needs compatibility review, exact-head negative controls and integration
with the terminal lifecycle. In particular, a session identity, writer
address or identical output bytes alone does not prove destination identity.

## XXVIII — Context stdout redirection and #333 CI

At this snapshot, `main` remains
`e64402526562ce0d7b6d88c0b5f7bb31152574d6`. No merge or release has occurred
in this update. v0.7 remains **HOLD** with no qualified release candidate.

#333 head `d9b2c2a648c4408084e24359d0cf6226574006eb` passed all eight jobs
in hosted development run `42930229874`. The Rust job checked out synthetic
merge `8e6a99f7b4f24157c9544956b246870019a3bcf3`, whose parents were the
then-current main and the PR head. Its tree
`24a3e3e1ffb5d120ee6492154c6bc8218741ed16` exactly matches the PR head's
tree. This is same-tree synthetic-merge CI, not a checkout of the head commit
itself. The PR remained a draft with no GitHub review at the time of audit;
the source review was same-model-family and was not independent human or
different-model certification. The review found no definite new defect in
its scoped mouse and screen guards, but the remaining unguarded session paths
and the combined panic, focus, input, restore and renderer lifecycle still
require qualification. #330 remains open.

A new real-PTY probe demonstrates that **Context output**, not only the public
standalone renderer, can share differential state across different physical
destinations. A Context rendered FRAME_A to a PTY while holding its lease;
stdout was redirected to a pipe, FRAME_B was rendered, stdout was restored,
and FRAME_B was requested again. On #338 head
`da9322811e4280bb486317126a42312af3c8d2d4`, the final physical PTY still
contained FRAME_A at cell (0,6) where the no-redirection control showed
FRAME_B. The test explicitly verifies the initial A frame and replays the exact
redirected bytes from that baseline through a VT100 model: all 1,920 cells and
the cursor then match the no-redirection B control, establishing that the
redirected call carried the B update. This is a source-specific Linux PTY
counterexample; the model does not establish all real-terminal behavior.

The unignored red test and its strengthened positive control are preserved in
research branch `research/xxviii-context-stdout-identity` at
`bfafd766d5ee1ac45a2526e03061d7a492a18a52`; no competing PR was opened. In a
checkout of that branch, this command exited 101 on the physical comparison,
after the A-frame, B-frame and redirected-byte full-grid controls passed:

```sh
cargo +1.98.1 test --locked --test terminal_sovereignty_pty \
  context_stdout_redirection_does_not_poison_physical_frame \
  -- --exact --test-threads=1
```

The test presently assumes the redirected render succeeds; an explicitly
fail-closed contract requires a revised acceptance oracle that proves either
safe delivery or an error without claiming an undelivered frame. This new
counterexample compounds the two standalone-writer failures. #338 remains
**HOLD**; stdout descriptor identity, actual terminal destination, session mode
writes and ownership cannot be inferred from writer addresses or from
`isatty` alone. A duplicate of the acquired terminal descriptor is one design
candidate, subject to a lifecycle and compatibility audit; it is not a proof
that all session operations use the same destination.

### Experimental renderer target branch — HOLD

Draft #339 is a separate experiment based on #338, head
`4dd1427cef39f4bf3b4817deaa8d7450f73de5c7`. It explores conservative
fullscreen output for unbound writers and a target-binding API for stateful
operations. It is not merged or qualified, and it does not repair the Context
stdout-redirection counterexample above. Its Rust public API and existing
standalone behavior need compatibility review before any promotion.

At that head, the focused `renderer_unbound_safety` suite passed 11/11,
including controls for first-target state, forged renderer state, writer
panic/forgotten binding and inline growth/restyle. The two previously failing
standalone A/B/A tests also passed as ordinary, unignored tests. Both had
failed with exit 101 on the pre-experiment source; on an earlier experimental
prototype, restoring unsafe fullscreen cache reuse made both fail. A focused
Clippy check passed. These controls are evidence for particular modeled cases,
not proof of general terminal or lifecycle safety. Review was by a separate
agent in the same model family, not an independent human or different model.

A six-target no-fail-fast compatibility run at the experimental head exited
101 with 27 failures across four targets:

| Test target | Result |
|---|---:|
| `renderer_depth_repaint` | 3 passed |
| `renderer_depth_repaint_failed_write` | 1 passed, 1 failed |
| `renderer_partial_write` | 4 passed, 4 failed |
| `renderer_unbound_safety` | 11 passed |
| `terminal_sovereignty_pty` | 16 passed, 3 failed |
| `whole_renderer_vt100` | 2 passed, 19 failed |

Some failures may reflect intentional rejection of unbound operations, but
that has not been established for each failure; do not rewrite or delete the
older tests merely to obtain a green suite. Reconcile each behavior against
public callers and target-binding semantics, review output cleanup and failure
paths, add the Context retargeting acceptance or a fail-closed equivalent, and
rerun the combined exact-head suite before considering #339 or #338 for merge.

```sh
cargo +1.98.1 test --locked --test renderer_unbound_safety -- --test-threads=1
cargo +1.98.1 test --locked --test terminal_sovereignty_pty \
  standalone_renderer_must_not -- --test-threads=1
cargo +1.98.1 test --locked --no-fail-fast \
  --test renderer_depth_repaint --test renderer_depth_repaint_failed_write \
  --test renderer_partial_write --test renderer_unbound_safety \
  --test terminal_sovereignty_pty --test whole_renderer_vt100 -- --test-threads=1
```

## XXIX — renderer compatibility and pinned Context output (working)

The verified main at this snapshot is still
`e64402526562ce0d7b6d88c0b5f7bb31152574d6`; no merge or release has been
performed in this round. Release `v0.7.0` remains **HOLD** with no qualified
candidate. Re-fetch live refs before acting.

#333 at `d9b2c2a648c4408084e24359d0cf6226574006eb` has 8/8 hosted CI
passing on synthetic merge `8e6a99f7b4f24157c9544956b246870019a3bcf3`;
its checkout tree equals the PR head tree. This is not an independent human
or different-model review and does not resolve #330 or the combined
#329/#327 lifecycle. No merge is claimed.

### Context stdout pin research — not a lifecycle fix

The isolated research branch `research/xxix-context-output-pin` at
`f0c268357f57faa6a0ce5462209f0de6350a25dc` is based on the prior real-PTY
stdout-retarget test and is **WIP / DO NOT MERGE**. On Unix its experiment
keeps renderer output on a duplicated terminal descriptor; a red-first real
PTY test showed that the old source diverted a frame to a pipe, while the
prototype preserved the no-redirection full-cell/cursor result and sent no
renderer bytes to the pipe. At the experimental head, the focused ordinary
suite passed 3 tests with 2 ignored; targeted Clippy passed. Running those two
ignored controls explicitly exited 101: `Context::restore()` while stdout is
redirected reports success while the physical terminal cursor remains hidden,
and `ctx.session.restore()` leaves the Context's pinned descriptor open while
the Context remains alive. These failures are not accepted release controls.

The pin does not cover session mode writes, panic hooks or restoration, all of
which still use process stdout. The acquisition check does not synchronize
against concurrent descriptor retargeting, including the alternate-screen
transition; direct public session methods, stdout buffering and interleaving
also remain part of the lifecycle and ownership review. Explicit
`Context::restore` closes the duplicate, but the public direct-session path
does not. The previously retained stdout-retarget test expects the diverted
pipe to receive FRAME_B; a changed pinned-output contract needs a reviewed
acceptance oracle, not deletion of the physical-delivery check. Windows and
other platform behavior are unverified. The evidence is from a same-model
agent, not independent human or different-model certification.

```sh
cargo +1.98.1 test --locked --test context_stdout_pinning_pty -- --test-threads=1
cargo +1.98.1 test --locked --test context_stdout_pinning_pty -- --ignored --test-threads=1
cargo +1.98.1 clippy --locked --test context_stdout_pinning_pty -- -D warnings
```

The ordinary test command exited 0; the explicit ignored command exited 101
with both named lifecycle failures. Full session ownership and terminal
cleanup require further design and independently falsifying tests.

### Renderer target-binding compatibility — under active migration

The prior #339 head `4dd1427cef39f4bf3b4817deaa8d7450f73de5c7` had 27
failures in the six-target renderer compatibility run. At intermediate head
`4af5f4225d5f3b6443365b2b241dedae782d40a6`, the same six targets passed
64/64 with no ignored tests after moving the depth-retry, cursor-fault,
torn-write and whole-frame VT harnesses to a stable bound writer. Existing
expected frames, write-cut matrices and cursor controls must remain in place.
This is evidence for that intermediate tree, not a qualification of a later
head or proof that all formerly failing behavior is acceptable.

Hostile controls were exercised on the migrated harnesses: prematurely
advancing color-depth bookkeeping made both depth retry controls fail;
discarding cursor uncertainty failed the cursor control; forcing an empty
write-cut range and corrupting the clean reference caused the respective
partial-write control to fail. Test writers now verify that cut points are
actually reached and that independently generated clean reference frames
contain their expected content before serving as an oracle. These mutations
are not substitutes for final exact-head CI or independent review.

A broader earlier run of 13 additional targets failed 38 tests in 12 targets;
these failures were under active investigation and must not all be assumed to
be harmless migrations. A separate four-target effects/cinematic migration,
using stable capture writers and preserving resize, cell, frozen-byte and
performance thresholds, passed 39 tests with no ignored tests on its isolated
test branch. Structured-output controls likewise passed 22 tests across four
focused targets on their isolated branch. These targeted runs do not represent
a complete integrated suite.

```sh
cargo +1.98.1 test --locked --no-fail-fast \
  --test renderer_depth_repaint --test renderer_depth_repaint_failed_write \
  --test renderer_partial_write --test renderer_unbound_safety \
  --test terminal_sovereignty_pty --test whole_renderer_vt100 -- --test-threads=1
cargo +1.98.1 test --locked --no-fail-fast \
  --test effects_perf --test acid_graphics --test acid_presentation \
  --test acid_render -- --test-threads=1
```

The next session must obtain the final #339 head and CI checkout tree live,
rerun the expanded integration and applicable consumers, audit remaining
public session controls and writer-lifetime obligations, and reconcile the
independent Context/session lifecycle counterexample. No green targeted suite
alone is a release or merge verdict.

### XXIX integration checkpoint

The #339 branch was subsequently pushed at
`551eb36ab60201a7eb21056c43003bbf92b4da53`. An integrated run on that tree
passed **125 tests across 14 targets**, zero ignored, exit 0: the six original
renderer targets, four effects/cinematic targets, and four structured-output
and safety targets. The previously described 39- and 22-test isolated runs
are included in that integrated evidence. A mutation that restored the unsafe
raw deprecated commit alias failed its retained control. The 38-failure broad
run preceded these integrations; **six prior failures in four scene/surface
targets remain unresolved** at this checkpoint. An isolated scene migration
is work in progress and is not included in the integrated 125-test claim.

Hosted CI for this new head was pending at the checkpoint. The Context
stdout-retarget and pinned-output restore controls remain unresolved, and the
binding's API/lifecycle contract, full development CI, installed consumers,
release preflight and independent human or different-model review have not
been qualified. #339, #338 and v0.7 remain **HOLD**. Confirm current remote
head, checkout tree and checks before any action.

```sh
cargo +1.98.1 test --locked --no-fail-fast \
  --test renderer_depth_repaint --test renderer_depth_repaint_failed_write \
  --test renderer_partial_write --test renderer_unbound_safety \
  --test terminal_sovereignty_pty --test whole_renderer_vt100 \
  --test effects_perf --test acid_graphics --test acid_presentation --test acid_render \
  --test capability_fallback --test commit_invariance --test safety_api \
  --test structured_output -- --test-threads=1
```
