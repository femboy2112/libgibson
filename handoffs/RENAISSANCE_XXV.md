# Renaissance XXV — terminal ownership, math M0 and release evidence

Snapshot while work is in flight; fetch live refs, read the PR discussions and
recheck every candidate before acting. This file is an engineering handoff,
not independent certification or authority to bypass a gate.

## Main and release

- Main: `0bd31616f77313e5dd3e78460601b7a776f77eaa`, source version `0.7.0-dev`.
  Exact-main development CI run `42745447592` completed successfully. #335
  merged at this commit and added `docs/MATH_TYPESETTING_ROADMAP.md`.
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
  cause. A fresh, same-host A/B and a clean external witness are required.
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
| #333 | `290f7089c43aab21d7403277da300cffa0e8523b` | Mouse retry and stale mouse/screen guards. Exact-head CI run `42738708564` passed. Does not alone establish terminal sovereignty. |
| #338 | `71a382ebcaf95e30690f61badbf954dbdfc9c5cc` | Draft based on #333, terminal ownership follow-up. **HOLD:** a deliberately ignored direct-renderer PTY counterexample fails when explicitly run; identity gates do not establish renderer-state ownership. |
| #329 | `f1dd2b3ad2bc21dbcafb5f1a8176498c2eccace0` | Caught-owner-panic raw input and bracketed-paste re-entry. Input-only; #328 remains open for wider lifecycle concerns. |
| #327 | `22d6d78aac28b4098e43d8cba355a6753923b8ab` | Focus retry and cleanup obligations. Reconcile with the other terminal changes and review the combined state machine. |
| #325 | `0b922c9043c393f65d877701cb107d314b164167` | C++/Python/Go focus/geometry parity; new absolute geometry oracle and mutation controls. Rebuild from the final runtime candidate and rerun all consumers. |
| #331 | `d1cd6016d9828000f21066bc76488ea4dbdd26a8` | Unicode lowercase-expansion pairing counterexample. Decoder-model evidence, not a physical keyboard-layout test. |
| #332 | `704c91bdd3fcfad8e3da4903e65877c6b9ec06fd` | Ready for review, not draft: released-artifact first-test guide. Fresh download, native checksum, wheel virtual environment and rendered-cell acceptance reported successful; wrong-glyph control failed as intended. Exact-head CI pending at snapshot. |
| #336 | `49482bf6b63cb831dec2f3aade9fb722fc358885` | Fixed-fixture math M0 research. Partial; not a public API, security sandbox or LibGibson terminal graphics path. |
| #337 | `4e147b8bfacde2658c8cdf3e41cfec62b9e18f7d` | Candidate ABI export-name provenance, with red-first fake-tool controls. An independently reproduced `assume-unchanged` preflight bypass is being addressed; recheck the live head and review. |
| #323 | Review the current live head | Handoff draft; update after integration and do not merge obsolete status. |

The #325, #332, #336, #337 and #338 checks were still in progress at this snapshot. A
later green result applies only to the head actually tested.

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
Do not promote these results to green full-suite evidence. Public
`Context.session` and `Context.renderer` replacement
can carry stale physical-screen assumptions; the experiment guards managed
output after replacement and requires compatibility review. Public unmanaged
input helpers and direct `Renderer` APIs with caller-provided writers remain
explicit escape hatches, including headless rendering to a physical terminal.
Do not claim a universal library-wide lease invariant while they remain.

At #338's recorded head, a separate same-family reviewer proposed a direct
renderer counterexample. A real PTY experiment matched the virtual session's
color capabilities to the physical one, rendered through the same public
renderer into a virtual buffer, then requested that frame through the context.
The old physical frame remained. An explicit ignored test fails on that
scenario; different color capabilities triggered a repaint and were a
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
After that witness, an independent same-family review reproduced a preflight
cleanliness bypass involving Git's `assume-unchanged` flag; a fix is in flight.
The earlier build result does not validate the revised head or the full
preflight. Verify that witness and rerun on the final release candidate. The latter
command checks exported symbol names only, not signatures or structure
layout. Confirm C ABI v1 compatibility separately and keep the 67-export
baseline unless a reviewed additive change is explicitly adopted.

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
