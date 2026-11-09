# Renaissance XXII — current integration handoff

Baseline inspected: 2026-11-09, `main` `4573d0d6e571159b376e08fb2761b4e8eaffe6f3`.
CI run 42578229665 reports success at that SHA. Verify live state before acting.
The published `v0.6.0` and `bindings/go/v0.6.0` tags peel to
`492d0fec7727658d50b9a82025c784b7e427d6b0`; v0.7.0 remains **HOLD**:
no exact release candidate, full preflight, external consumer qualification, or
independent final audit has been established in this round.

## Correctness boundaries

- #312 remains open. A legacy `!` press during a screen transition can be followed by a
  Kitty base-key `1` release. With reporting active, no intervening repeat and no later
  recovery event, the hold has no finite deadline. The existing test advances to tic 1000
  and still sees the hold. Focus loss or disabling reporting can clear it, but neither is
  guaranteed. A general timeout would also end a genuinely held exact key; the input event
  currently does not carry the legacy/Kitty press provenance needed for a selective policy.
- #305: the successful-pop/failed-switch double-pop was addressed by #316. A failed write
  or flush can leave the terminal's keyboard stack unknown. Restoration is best effort in
  that case; do not claim atomic terminal I/O or guaranteed stack ownership.
- #292 remains open; Windows runtime behavior was not verified here. Do not indiscriminately
  drop release events, because committed characters may be affected.
- #320 implements pause/resume/step, not time scaling or a proof that the full UI loop is
  independent of output stalls. #130 remains open.
- The ABI baseline file lists 67 symbols. This is a source count, not an installed-binary
  compatibility test.

## Draft terminal audit PR

Draft #322, `audit/xxii-terminal-contract`, head
`3a277df307c4226944f4320711250db9ac3519bd`, corrects the claims above and adds
an input-to-hold witness through 1,000,000 tics, an exact Kitty control and a flush-failure
bookkeeping control. On that head, `cargo test --lib issue_312 -- --nocapture` passed 4/4
and `cargo test --lib key_release_switch_tests -- --nocapture` passed 6/6 with one Cargo
job on rustc 1.97.0-nightly; these are local results, not CI or an independent final audit.
No runtime fix is included. Review the draft and exact-head CI before any merge.

## Draft onboarding slice

Draft #324, `codex/xxii-onboarding`, head
`2142da84835e2aa8f968df818a8a1e54c2521279`, adds five headless starters and an
external-artifact smoke. A local run passed C, C++, Python, Go and Rust using a freshly
staged native SDK built from baseline main, an installed wheel, an external Go module copy
and an unpacked crate; missing SDK and deliberately incorrect ABI controls failed as
expected. This was a nightly-toolchain headless run, not a release gate. Interactive and
application examples, full diagnostic coverage, independent review and exact-head CI remain.

## PR debt

#167 was closed as already integrated by ancestry, not merged via GitHub: its head
`24d2a13a7b6308f2d01e0be6700ad9ca3b805e20` is an ancestor of baseline main, its selection
implementation and test files are unchanged, and its public module remains exported.
Its remote branch was retained. The open heads of #153, #154, #156–#164 (excluding the
already closed numbers), #166 are ancestors of baseline main, but later edits to files
require individual acceptance-test and documentation review before closure. #173 and #177
are divergent and require review of their unique changes. Dependabot PRs #168 and #169
are also divergent; do not apply the integrated-by-ancestry reasoning to them.

## Resume safely

The XXI merge train #316, #318, #319, #317, #320 and #321 is already merged; do not replay it.
Inspect live PRs, branches and CI before choosing an integration candidate. Wrapper focus
and geometry parity, installed five-language quickstarts, full terminal fault controls,
clock-loop integration and the external v0.7 consumer canary remain to be qualified.
Use the main-line release procedure in `docs/RELEASING.md` §§0–6, not the maintenance-line
§7; do not publish registries or modify old tags. Keep source changes, user data, and
other projects; no force push or remote-branch deletion. Do not treat the prior handoff's
“bounded” wording as a finite #312 deadline.

First commands:

```sh
git status --short
git worktree list --porcelain
git fetch origin
git rev-parse origin/main
gh pr list --repo femboy2112/libgibson --state open --limit 100
gh run list --repo femboy2112/libgibson --commit "$(git rev-parse origin/main)"
cargo +1.98.1 test --lib issue_312 -- --nocapture
```

Run compiler-heavy work serially with a verified writable target; stage a fresh SDK before
running `scripts/dev/parity_v08.sh <prefix>`. The above test demonstrates the documented
state and conditional recovery; it does not establish a deadline or a terminal-level fix.
