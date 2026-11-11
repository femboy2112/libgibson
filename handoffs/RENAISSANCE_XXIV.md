# Renaissance XXIV — terminal recovery and SDK audit

> Historical archive: statements, queues and CI below describe the recorded snapshot.
> Superseded by [Renaissance XXX](RENAISSANCE_XXX.md); fetch live state before acting.

Snapshot: `main` `43bae9a6e50336e9a8e0d64e313a0e2d3a16e654`, version
`0.7.0-dev`. Development CI run 42731417582 passed all eight jobs at that SHA.
The latest published GitHub prerelease is `v0.6.0`; both its tag and
`bindings/go/v0.6.0` peel to `492d0fec7727658d50b9a82025c784b7e427d6b0`.
No v0.7.0 candidate has been qualified. Fetch and verify all state before acting.

## Terminal recovery

Issue #328 remains open. On a Linux PTY, pre-#329 main failed the caught
owner-panic input test with canonical/echo mode still active and the un-newlined
key not delivered; no-panic and non-owner-panic controls passed. Draft #329
repairs raw input and bracketed-paste re-entry only. It does not establish recovery
of screen, cursor, renderer, focus or mouse state after a caught panic.

A separate sequential-owner PTY test found that a restored context could poll and
re-enter raw mode after a second session acquired the lease. #329 head
`f1dd2b3ad2bc21dbcafb5f1a8176498c2eccace0` guards both context polling and direct
input entry, including a test that the old session cannot consume a pending notice
belonging to the new owner. A partial-write paste test complements the failed-flush
control. On the same source before commit, Rust 1.98.1 Linux tests passed: 3 paste
fault controls; 7 caught-panic/input tests; 9 interactive-entry PTY tests; and
22 Kitty reporting PTY tests. The four explicit hangup tests passed on the committed
head. I/O recovery remains best effort and cannot promise atomic terminal writes.
Exact-head CI run 42733860032 passed all eight jobs, including unsuppressed Clippy
and the Rust 1.85 library job. Full requested fault coverage and independent
adversarial review remain required. A same-model-family review is useful, but is
not distinct-model evidence.

A related pre-existing ownership defect is tracked in #330: on a Linux PTY, after
session A restored and session B acquired the lease, A could enable mouse or focus
reporting, and B's restore did not emit the corresponding disable. Positive controls
using B's own setters emitted both enable and disable. Other mode methods and panic
interactions remain to be checked. Draft #333 guards stale mouse and alternate-screen
operations: three targeted PTY cases failed on main. A separate review then found
that an uncertain mouse enable/disable or a caught panic could cause a false-success
retry. Those three writer-fault cases and the panic retry failed before the repair.
At draft head `290f7089c43aab21d7403277da300cffa0e8523b`, six writer-fault and ten
PTY cases passed, as did screen-switch fault, Kitty and ownership regressions. Partial
disable writes, hangup and real teardown failure remain untested. The draft does not
repair focus; #330 remains open. Draft #327 handles focus retry and must
be reviewed in combination with #329 after the latter is qualified; do not collapse
cleanup obligation and successful request, or imply terminal capability support.
An exploratory `git merge-tree --write-tree` of those draft heads reported a content
conflict in `src/session.rs`, in their appended test modules. Resolve and review the
full combined state after #329 integration, then rerun the fault and PTY matrices;
the dry run is not an integration or a qualification.
An equivalent merge-tree preview of #329 and #333 also reports a `src/session.rs`
content conflict. #333 alone does not include #329's stale input-entry guard; the
eventual combined implementation needs review and fresh tests as one state machine.

Historical draft queue (all HOLD; fetch and check exact heads before acting):

1. #329: `f1dd2b3ad2bc21dbcafb5f1a8176498c2eccace0` — input recovery and
   stale-session entry guard; independent review and remaining fault controls.
2. #327: `22d6d78aac28b4098e43d8cba355a6753923b8ab` — focus retry; integrate
   against qualified #329 and review the combined terminal state machine.
3. #333: `290f7089c43aab21d7403277da300cffa0e8523b` — stale mouse/screen
   guards and mouse retry; independent review, current-head CI and reconciliation
   with #329/#327.
4. #325: `bf812d51f8f6af4976c5c07a34407435fe8122eb` — wrapper projection;
   refresh against runtime fixes and qualify full installed parity.
5. #331: `d1cd6016d9828000f21066bc76488ea4dbdd26a8` — Unicode decoder-model
   boundary; exact-head development CI and publication-free preflight passed;
   review before integration.
6. #332: `db583a9663d09942941c6457f06a8d880233f5ad` — released-SDK first
   five minutes; exact-head development CI passed, and a same-model-family
   review found and confirmed correction of the shell failure-flow issue.
7. #323: this handoff branch; recheck its live head and reconcile after integration.

## Key provenance and parity

Issue #312 was reopened because its stated acceptance requires a matching release
or finite bound, neither of which has been demonstrated. Merged #322/#326 establish
that an unmatched legacy-origin hold can persist and that two physical histories
can have the same public event observations. Parser wire origin may provide an
additional observation, but neither internal `KeyId::Code` nor a CSI-u sequence
alone proves physical identity or exact release coverage. A timeout for an
uncertain-origin hold is a policy tradeoff that may truncate a real long hold.
Draft #331 adds a Unicode lowercase-expansion decoder-model case with a `W`/`w`
control; it is not a physical keyboard-layout result.

Draft #325 projects focus and geometry into the existing wrappers. Its comparator
previously accepted common-mode focus-identity and ordering mutations, and a changed
known-gap value that matched the live C reference. Head
`bf812d51f8f6af4976c5c07a34407435fe8122eb` adds absolute focus records, committed
order checking, pinned-gap checking, tests for these mutations, PTY focus-input
controls for the C++, Go and Python wrappers, and preservation controls for temporary
Go caches; verify its live head before using this snapshot. At that head, the complete
harness passed against a fresh external SDK built from source identical to main's
Rust/C ABI tree, with the draft's C++ header and worktree-built Rust example: 84 records per
language, 500 repetitions, negative controls and emulated PTY focus checks. This does
not qualify the later combined runtime state. A same-model-family reviewer identified
a race between the temporary cache identity check and recursive path deletion; a
controlled replacement fixture was deleted, with no user data lost. The new head
retains temporary paths for audited
cleanup rather than recursively deleting them, labels partial runs, and adds C static
assertions for event values 7 and 8. The full run, local negative controls, exact-head
development CI run 42737631932 and publication-free preflight run 42737631935 passed
on that head; independent review and post-integration testing remain necessary.
PTY focus output requests and injected input do not prove terminal capability;
real terminal-generated focus input, pre-render and resized geometry, ownership
and error paths require further checks.

## Installed onboarding

From exact main `43bae9a6e50336e9a8e0d64e313a0e2d3a16e654`, a freshly built
native SDK, a freshly built and installed Python wheel, an external Go module copy,
and an unpacked Rust crate passed the five headless starters with Rust 1.98.1.
The three Python apps passed Linux PTY startup, input, resize-liveness and restore
smokes against that SDK and wheel. Their first-poll event-preservation check uses
a synthetic event; their response/input checks use the PTY. These checks do not
establish a physical terminal's capabilities or an exact release candidate. The SDK native library's
SHA-256 was `a776781efeaf93bbee06fcf3c22fed8206dc5f62aa303c43b28e2de1688f8423`;
the wheel's was `42b3a86f82b9c7afb101fde8093f2f1c15124899eb3427442b8fddc30f452127`.
The release build's `scripts/release/check-abi.sh` reported all 67 baseline symbols
present and 67 built exports; that is a symbol-name check, not a layout or signature proof.
`git diff --quiet v0.6.0^{commit} origin/main -- src/audio tests/audio_pocket_voice.rs`
exited 0 at this snapshot. This source equality is not a fresh PCM execution witness.

The published v0.6.0 native archive passed its published SHA-256 check; the C and
Python headless starters ran with release artifacts. The newer C++ headless
constructor is absent from the v0.6.0 wrapper. Draft #332 provides a short,
executable Python path using the published SDK and wheel. Its shell block was run
end-to-end against downloaded assets; a wrong-cell control failed, and separate
controls confirmed it stops on an existing directory and a failed archive checksum.
The wheel has no separately published checksum asset in that release.

An external-consumer A/B was started on the same host: the v0.6.0 baseline full
suite passed 451 tests with 3 ignored, while the development dependency's first
full suite returned 445 passed, 1 failed, 2 ignored and stopped on a fault-injection
timing test because zero of ten attempts landed in its intended window. That result
is ambiguous and is not classified as a
library regression or a harmless flake. Retain the first failure; compare both
dependencies under the same targeted workload and run tests skipped after failure.
A subsequent same-host targeted run passed for both: one of ten attempts landed in
the intended window for the baseline, three of ten for development. This demonstrates
timing sensitivity but does not replace or conclusively explain the failed first run.
The candidate's separately run skipped PTY binary passed 5 tests with 1 ignored.
The first offline fetch of the new dependency missed the local cache; a subsequent
public dependency refresh allowed the locked, offline test run. The two dependency
lockfiles also differ in Windows-target resolution. The baseline headless selfcheck
passed normally and with a 512 MiB virtual-address cap; that is not a sustained
memory-pressure test; the candidate passed the same two selfchecks. A real PTY
hangup probe observed raw mode and output before closing the master; both versions
exited on SIGHUP. A post-hangup termios query returned EIO, so restoration was not
observable. The first probe mishandled that EIO; the corrected repeat retained that
limit. Verify current canary results before promoting this evidence.

## Release and next steps

No feature or release merge is authorized merely by this note. For Tier B, obtain
red baselines, discriminating controls, the combined-state review and exact-head CI.
Update #327 against the qualified #329 result, then #325 against the resulting main.
Qualify installed C, C++, Python, Go and Rust consumers on one exact candidate, plus
an external consumer A/B canary, ABI/layout and MSRV, first-attempt PTY/hangup,
frozen PCM, publication-free preflight, resource/graphics evidence and independent
release review. Node J's external GPL engine integration remains a local prototype,
not a published or CI-verified deliverable; obtain a properly authorized scope
adjustment or complete it. Windows remains unverified. Follow main-line
`docs/RELEASING.md` §§0–6, do not publish registries, rewrite old tags, force-push,
change frozen audio, enable private capabilities, or disclose private consumer data.

Start with `git status --short`, `git worktree list --porcelain`, `git fetch origin`,
`git rev-parse origin/main`, `gh pr list --state open --limit 100`, and exact-head
`gh pr checks` for #329, #327, #333, #325, #331 and #332. Re-read PR discussions; this is a
historical snapshot, not a substitute for current state.
