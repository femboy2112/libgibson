# Renaissance XXV — terminal ownership, math M0 and release evidence

> Historical archive: statements, queues and CI below describe the recorded snapshot.
> Superseded by [Renaissance XXX](RENAISSANCE_XXX.md); fetch live state before acting.

Snapshot while work is in flight; fetch live refs, read the PR discussions and
recheck every candidate before acting. This file is an engineering handoff,
not independent certification or authority to bypass a gate.

## Main and release

- Main: `e64402526562ce0d7b6d88c0b5f7bb31152574d6`, source version `0.7.0-dev`.
  Exact-main development CI run `42748526447` completed successfully.
  Earlier main `0bd31616f77313e5dd3e78460601b7a776f77eaa` passed development
  CI run `42745447592`; #335 merged there and added
  `docs/MATH_TYPESETTING_ROADMAP.md`.
- #332 merged at current main after development CI run `42747338050` passed
  for head `704c91bdd3fcfad8e3da4903e65877c6b9ec06fd`. CI checked out
  synthetic merge `2c40a48521fd543e2927b6745e1d58f61158ce1d`, whose tree
  `8ad1a4bd8428d8a4899684bd68696372b20b558b` equals the head tree. Its released
  artifact headless guide was exercised with download, native checksum, wheel
  virtual environment and rendered-cell acceptance; a wrong-glyph control
  failed as intended. It is not candidate artifact or terminal qualification.
- Latest published GitHub engineering prerelease: `v0.6.0` and the Go module
  tag `bindings/go/v0.6.0` peel to
  `492d0fec7727658d50b9a82025c784b7e427d6b0`.
- **v0.7.0 HOLD**: no immutable release candidate has earned all gates. The
  terminal lease and recovery changes are not integrated; ABI signatures and
  layout need separate validation; exact-candidate installed consumers, MSRV,
  fresh preflight, resource/performance and independent release review remain.
  An earlier downstream v0.6-versus-development comparison had a first-run
  fault-injection test failure in the candidate. Later targeted runs on both
  versions passed, which does not erase the first failure or establish its
  cause. A fresh same-host A/B on this development commit passed as detailed
  below; a final-candidate witness and remaining release gates are still required.
- Node J's external GPL engine integration is recorded as a local prototype;
  the in-tree adapter does not prove that an external port was published or
  independently qualified. Obtain a defensible documented milestone decision
  or satisfy the deliverable within the authorized scope. Windows remains
  unverified.

## Queue snapshot

Entries are drafts unless noted, at the recorded heads. CI can change while a
review is in progress; query exact heads again. Do not force-push or delete
remote branches. Same-model review passes are useful but are not independent
human or distinct-model certification.

| PR | Head | Scope and remaining decision |
|---|---|---|
| #333 | `e9ba3e6f34b08a022ea933c0842c50a4c237649a` | Mouse retry and stale mouse/screen guards; head-associated development CI run `42748559384` passed 8/8 on a same-tree synthetic merge. Does not alone establish terminal sovereignty. |
| #338 | `7156134c98ce20dfe8c96b7f62bbb2c06c8ad817` | Draft based on #333; head-associated development CI run `42748580954` passed 8/8 on a same-tree synthetic merge. **HOLD:** a deliberately ignored direct-renderer PTY counterexample was explicitly rerun and failed on this head. Identity gates do not establish renderer-state ownership. |
| #329 | `f1dd2b3ad2bc21dbcafb5f1a8176498c2eccace0` | Caught-owner-panic raw input and bracketed-paste re-entry. Input-only; #328 remains open for wider lifecycle concerns. |
| #327 | `22d6d78aac28b4098e43d8cba355a6753923b8ab` | Focus retry and cleanup obligations. Reconcile with the other terminal changes and review the combined state machine. |
| #325 | `0b922c9043c393f65d877701cb107d314b164167` | C++/Python/Go focus/geometry parity; head-associated CI and preflight passed on an older synthetic merge with a different tree. Rebuild from the final runtime candidate and rerun all consumers. |
| #331 | `d1cd6016d9828000f21066bc76488ea4dbdd26a8` | Unicode lowercase-expansion pairing counterexample. Decoder-model evidence, not a physical keyboard-layout test. |
| #336 | `e64834f2cc7fb5af63062e5aae4c3c3615d295c0` | Fixed-fixture math M0 research, reconciled with main; head-associated development CI run `42748877100` passed on a same-tree synthetic merge. Partial; not a public API, security sandbox or LibGibson terminal graphics path. |
| #337 | `ca8f3cb80557ced6e07abf473bafb12a05ed9139` | Candidate ABI provenance, alternate-target SDK staging and isolated captured-commit preflight. Development CI and publication-free preflight passed; the latter tested a synthetic merge commit with the same tree, not the head commit. Final candidate qualification and independent review remain. |
| #323 | Review the current live head | Handoff draft; update after integration and do not merge obsolete status. |

PR checks can execute a
synthetic merge rather than the head commit. Verify the actual checkout SHA and
tree, not only the GitHub head association, when assessing every check.

For #333, CI checked out synthetic merge
`e7f258935f96e8b9a84fb4d7134bbd74a5128bd5`; it and its PR head share tree
`99b77e07107eb9daf1a02a0bfdab7aad1fe50307`. For #338, CI checked out
`32c0d51a2fd3a7685c7d82fe4027d454d4f74dd2`; it and its PR head share tree
`756e76ad933bc758ad1191f4b4faee253c053e39`. For #336, CI checked out
`8faf4cdd48def29b8393b128ee76ad745cabaacc`; it and its PR head share tree
`39ed352e73783e2571636448010c4ad9391d6cfc`. These are same-tree witnesses,
not executions of the corresponding head commits.

For #325, head-associated CI run `42746034136` checked out synthetic merge
`459047ce86f5ee170174380897d311e050722ebc` against previous main `0bd316...`.
Its tree `e2de4d4d7864256b5788b7fe05e71a5e22219ed5` differs from the PR
head tree `ae7e2ec72ab82369f4f2b28b5fe6285aa1fb453e`; main has since advanced.
The green run is therefore neither a head-tree nor current-base witness.

## Terminal lease: observed failures and boundaries

Issue #330 remains open. At #333 `290f708`, Linux PTY probes reproduced three
separate failures: a restored context rendered and committed into its
successor's terminal; changing an owner's public `is_tty` to false caused
restore to retain the lease and raw input; and changing a never-owner's flag to
true allowed it to emit a reset into a successor's terminal. Positive controls
exercise the active owner and terminal state. These results establish the
specific paths, not all terminal behavior.

Draft #338, based on #333, adds private lease checks for cleanup and managed
output, cursor checks, an operation matrix and focused PTY controls. On its
earlier head `c7b288327d2c94b0046f1271efc0b4e86b19ed33`, the author reported
30 focused PTY tests, locked Rust 1.85 library check and stable 1.98.1 library
Clippy with warnings denied passing. A nightly Clippy run failed on three
warnings in unchanged audio code; the full library suite was stopped early.
Exact-head CI at `71a382ebcaf95e30690f61badbf954dbdfc9c5cc` then failed on
four Clippy warnings in the new PTY tests. The later `8610ff7b...` commit fixes
those warnings; the author reports local Rust 1.98.1 test-target Clippy and
the PTY suite pass. After merging updated #333, development CI associated with
head `7156134c...` passed 8/8 on the same-tree synthetic merge described above;
the separate ignored failure below remains. Do not promote these results to green
full-suite evidence. Public
`Context.session` and `Context.renderer` replacement
can carry stale physical-screen assumptions; the experiment guards managed
output after replacement and requires compatibility review. Public unmanaged
input helpers and direct `Renderer` APIs with caller-provided writers remain
explicit escape hatches, including headless rendering to a physical terminal.
Do not claim a universal library-wide lease invariant while they remain.

At #338's earlier head `8610ff7b...`, a separate same-family reviewer proposed a direct
renderer counterexample. A real PTY experiment matched the virtual session's
color capabilities to the physical one, rendered through the same public
renderer into a virtual buffer, then requested that frame through the context.
The old physical frame remained. An explicit ignored test fails on that
scenario; the same test was rerun and failed at `7156134c...`. Different color
capabilities triggered a repaint and were a
misleading positive control. The updated swap tests compare all 1,920 cells
and cursor state and detect a planted extra-cell mutation. These are reported
tests at the stated head, not proof of general terminal capability. Run:

```sh
cargo test --locked --test terminal_sovereignty_pty -- --test-threads=1
cargo test --locked --test terminal_sovereignty_pty direct_renderer_use_must_not_poison_context_output_state -- --ignored --exact --test-threads=1
```

The second command is expected to fail until the renderer-cache issue is
fixed; do not mistake its `#[ignore]` status for a passing test. Red
baseline and positive-owner controls, terminal screen-state checks, forged
flags in both directions and headless/non-TTY controls must be retained.
Integrate #333, #329 and #327 without rewriting history, resolve overlapping
`src/session.rs` changes, and test the combined input/paste, mouse, focus and
Kitty transitions, panic notices, failures, hangup and repeated lease handoffs.
A failed write or flush leaves physical state unknown. Successful output is a
request, not proof of terminal support.

Issue #312 remains open: a legacy shifted `!` press followed by Kitty's base
`1` release may remain held indefinitely. Two distinct physical histories can
produce the same current public events. A `KeyId::Code` or a CSI-u sequence by
itself does not prove the missing physical provenance. Do not add a timeout
that silently truncates a genuine exact long hold.

## Wrapper parity and ABI provenance

At #325's recorded head, the comparator pins initial, pre-render and delivered
resize geometry to the committed expectation rather than trusting agreement
among clients. Its controls mutate geometry for all clients, as well as focus
identity, order, missing/duplicate records and known-gap records. Run the full
existing harness with 84 records per language and at least 500 repetitions
against a freshly staged SDK built from the qualifying source:

At that head, a local comparator run reported eight tests passing and twelve
additional common-mode mutations rejected, using previously generated
transcripts. This is comparator evidence, not a fresh installed-SDK run.

```sh
CARGO_BUILD_JOBS=1 scripts/dev/parity_v08.sh "$SDK_PREFIX"
```

Keep controls enabled; a prebuilt Rust example or unverified SDK provenance
makes the result partial. PTY focus bytes and peer-injected focus events do not
prove real terminal focus support. Exact-head #325 results and a post-terminal
integration installed-SDK run are still required.

The previous no-argument ABI checker could prefer a stale release library over
the candidate build. #337 adds an explicit library argument and a fresh archived
candidate build; fake-tool negative controls test bad explicit output, stale
good output, missing library, `nm` failure, dirty source, an advancing HEAD and
failed build.
These controls do not replace a real compiler witness:

```sh
bash scripts/release/tests/test-check-abi.sh
bash scripts/release/check-candidate-abi.sh
```

At #337 head `4e147b8bfacde2658c8cdf3e41cfec62b9e18f7d`, a fresh archived
candidate release build using Rust/Cargo 1.98.1 and explicit symbol check was
reported exit 0 with 67 of 67 baseline names and shared-library SHA-256
`a776781efeaf93bbee06fcf3c22fed8206dc5f62aa303c43b28e2de1688f8423`.
After that witness, an separate same-model-family review (not independent human or different-model certification) reproduced a preflight
cleanliness bypass involving Git's `assume-unchanged` flag; #337's `26a59b2...`
head adds flag controls. A further same-family review reproduced a false-clean
tracked file without hidden flags: with `core.trustctime=false` and
`core.checkstat=minimal`, a same-size content change with restored mtime passed
both `git status` and the index checker. A separate source audit found that
`stage-sdk.sh` can build into an alternate `CARGO_TARGET_DIR` yet copy ignored
stale libraries from the default `target/release`, so cleanroom consumers may
test the wrong SDK. #337 head `0d97720...` changes staging to use artifacts
reported by Cargo; fake-tool controls cover stale defaults and ambiguous or
missing artifact records. A real Rust 1.98.1 release staging run at that head
reported exit 0 with both custom target and external prefix paths containing
spaces; staged `.so` and `.a` matched Cargo's target artifacts byte-for-byte,
the SDK checksum manifest passed, and the C/C++ onboarding path test passed
with its missing-package and legacy-expansion negative controls. The `.so`
SHA-256 was `a776781efeaf93bbee06fcf3c22fed8206dc5f62aa303c43b28e2de1688f8423`.
This is not five-language release-artifact or full-preflight qualification.
At #337 `ca8f3cb...`, preflight instead runs against a detached isolated clone
of the captured commit with a fresh target; reported mock controls cover the
reproduced stat-cache false-clean source, hidden index flags, ignored files,
foreign target, stale crate and staging records. Development CI run
`42749977128` and publication-free preflight run `42749977158` passed for
#337 head `ca8f3cb...`; both checked out the synthetic PR merge
`b1077efff46f718bee7557260bab24b36a5bb53b`, with parents current main and
the PR head. Both synthetic merge and PR head have tree
`4ad917f74c111df31939a5da9e7940286318efb6`; this is same-tree evidence,
not a run on the exact head commit or final release candidate. The preflight
log reports passing Clippy with warnings denied, locked and fresh-resolution
Rust 1.85 checks, the candidate symbol baseline and `cleanroom.sh`. A writable
clone, transient source modification,
toolchain substitution and global-cache contamination remain unqualified;
verify exact-head CI and independently review before integration.

An independent local comparison reported 59 names in the v0.6.0 tag's ABI
manifest, 67 in current main's manifest and 67 in the #337 `4e147b8...` fresh
library: no released names were missing. Candidate #338's FFI, C header,
manifest and layout-test source matched current main, and its Rust 1.98.1
`ffi_abi_layout_freeze` run reportedly passed 7/7. These are bounded earlier
witnesses, not qualification of a final candidate or a complete signature and
layout proof. Rerun against the final source and installed artifacts. The
candidate ABI command checks exported symbol names only.

## External consumer comparison

A fresh isolated same-host A/B compared the same external-consumer source with
the published v0.6.0 dependency and a clean archive of development main
`e64402526562ce0d7b6d88c0b5f7bb31152574d6`. Using Rust 1.98.1, separate build
targets, offline dependency resolution, two compiler jobs and serial tests,
both first-attempt `cargo test --locked --offline --all-targets -- --test-threads=1`
runs exited 0: 451 passed, 0 failed and 3 ignored across 34
test binaries each. The earlier timing-sensitive fault-injection case passed
on this first attempt in both lanes. Both normal headless selfchecks exited 0;
both also exited 0 under a 512 MiB virtual-address-space limit, and the
corresponding output matched between lanes. This is synthetic headless
evidence, not sustained memory pressure, universal terminal behavior or a
future release-candidate witness. Candidate lockfile resolution changed the
library source and Windows-only transitive selections; the compared source
trees otherwise matched. The earlier first-run failure remains historical
evidence with unresolved cause, not an established library regression or a
discarded flake. Raw receipts are retained privately.

## Math M0 and graphical boundary

Issue #334 remains open. Draft #336 uses four trusted fixed fixtures: integral,
nested fraction, matrix and aligned equations. Local pdfTeX/Poppler output
passes through `RgbRaster`, `Node::raster` and headless LibGibson capture;
Matplotlib mathtext is a distinct comparison implementation for the fraction
and integral but rejected the tested matrix and aligned syntax. The cell
fallback loses fine detail. The fixture runner is not a sandbox and must not
be used for untrusted math source. Typst and MathJax were unavailable in the
recorded experiment.

The draft reports tests of repeated hashes, glyph/rule ordering, invalid fixture
and raw buffer inputs, deliberate renderer failure, an all-white output
mutation and a terminal-cell oracle. Run from that PR's checked-out head with
its declared dependencies:

```sh
cargo build --locked --example math_m0_capture -j 1
PYTHONNOUSERSITE=1 /usr/bin/python3 experiments/math_m0/check.py
```

A separate direct Kitty/X11 protocol probe reported an RGB query response and
visible colored pixels, with an invalid-format control. It bypassed LibGibson
and did not display the formula. It does **not** establish LibGibson graphics
support or paper-quality terminal output. Untrusted input isolation, full
metrics/clipping, image lifecycle, font/license review, Rust 1.85 and other
terminals remain unverified. Keep semantic source accessible and the component
experimental until its own gates pass.

## Bootstrap

```sh
git status --short
git worktree list --porcelain
git fetch origin
git rev-parse origin/main
gh pr list --repo femboy2112/libgibson --state open --limit 100
gh pr checks 333 --repo femboy2112/libgibson
gh pr checks 338 --repo femboy2112/libgibson
gh pr checks 329 --repo femboy2112/libgibson
gh pr checks 327 --repo femboy2112/libgibson
gh pr checks 325 --repo femboy2112/libgibson
gh pr checks 332 --repo femboy2112/libgibson
gh pr checks 336 --repo femboy2112/libgibson
gh pr checks 337 --repo femboy2112/libgibson
```

Read `docs/RELEASING.md` §§0–6 for a main-line release; §7 is for maintenance
lines. Only qualify and merge exact heads with the required red controls, PTY
and compatibility checks, independent review and green CI. Preserve ABI v1,
Rust 1.85 and frozen HumanMusic/PCM contracts. No force-push, old-tag rewrite,
remote-branch deletion, crates.io/PyPI publication, unrelated repository main
changes, private capability activation or external project creation. A GitHub
Engineering Alpha release is authorized only after all applicable gates pass.
