# Renaissance XXIV — terminal recovery and SDK audit

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

A related, source-inspected pre-existing ownership concern is tracked in #330:
mode methods on restored sessions can still affect a later owner. Its proposed
interleaving needs a real PTY reproduction. Draft #327 handles focus retry and must
be reviewed in combination with #329 after the latter is qualified; do not collapse
cleanup obligation and successful request, or imply terminal capability support.

Historical draft queue (all HOLD; fetch and check exact heads before acting):

1. #329: `f1dd2b3ad2bc21dbcafb5f1a8176498c2eccace0` — input recovery and
   stale-session entry guard; independent review and remaining fault controls.
2. #327: `22d6d78aac28b4098e43d8cba355a6753923b8ab` — focus retry; integrate
   against qualified #329 and review the combined terminal state machine.
3. #325: `25358ab98fce26d9e47e644e09c5834a23a859c8` — wrapper projection;
   refresh against runtime fixes and qualify full installed parity.
4. #331: `d1cd6016d9828000f21066bc76488ea4dbdd26a8` — Unicode decoder-model
   boundary; review and exact-head CI.
5. #332: `db583a9663d09942941c6457f06a8d880233f5ad` — released-SDK first
   five minutes; review and exact-head CI.
6. #323: this handoff branch; recheck its live head and reconcile after integration.

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
`25358ab98fce26d9e47e644e09c5834a23a859c8` adds absolute focus records, committed
order checking, pinned-gap checking, tests for these mutations, and PTY focus-input
controls for the C++, Go and Python wrappers; verify its live
head before using this snapshot. The prior five-language 84-record output remains
historical, not a fresh installed-SDK qualification of the updated branch.
PTY focus output requests do not prove terminal capability; actual decoded input,
pre-render and resized geometry, ownership and error paths require fresh checks.

## Installed onboarding

From exact main `43bae9a6e50336e9a8e0d64e313a0e2d3a16e654`, a freshly built
native SDK, a freshly built and installed Python wheel, an external Go module copy,
and an unpacked Rust crate passed the five headless starters with Rust 1.98.1.
The three Python apps passed Linux PTY startup, input, resize-liveness and restore
smokes against that SDK and wheel. These checks do not establish a physical
terminal's capabilities or an exact release candidate. The SDK native library's
SHA-256 was `a776781efeaf93bbee06fcf3c22fed8206dc5f62aa303c43b28e2de1688f8423`;
the wheel's was `42b3a86f82b9c7afb101fde8093f2f1c15124899eb3427442b8fddc30f452127`.

The published v0.6.0 native archive passed its published SHA-256 check; the C and
Python headless starters ran with release artifacts. The newer C++ headless
constructor is absent from the v0.6.0 wrapper. Draft #332 provides a short,
executable Python path using the published SDK and wheel. Its shell block was run
end-to-end against downloaded assets; a wrong-cell control failed, and separate
controls confirmed it stops on an existing directory and a failed archive checksum.
The wheel has no separately published checksum asset in that release.

## Release and next steps

No feature or release merge is authorized merely by this note. For Tier B, obtain
red baselines, discriminating controls, the combined-state review and exact-head CI.
Update #327 against the qualified #329 result, then #325 against the resulting main.
Qualify installed C, C++, Python, Go and Rust consumers on one exact candidate, plus
an external ProjectGHOST A/B canary, ABI/layout and MSRV, first-attempt PTY/hangup,
frozen PCM, publication-free preflight, resource/graphics evidence and independent
release review. Node J's external GPL engine integration remains a local prototype,
not a published or CI-verified deliverable; obtain a properly authorized scope
adjustment or complete it. Windows remains unverified. Follow main-line
`docs/RELEASING.md` §§0–6, do not publish registries, rewrite old tags, force-push,
change frozen audio, enable private capabilities, or disclose private consumer data.

Start with `git status --short`, `git worktree list --porcelain`, `git fetch origin`,
`git rev-parse origin/main`, `gh pr list --state open --limit 100`, and exact-head
`gh pr checks` for #329, #327, #325, #331 and #332. Re-read PR discussions; this is a
historical snapshot, not a substitute for current state.
