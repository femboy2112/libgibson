# Renaissance XXX — renderer truth and qualified integration

Checkpoint dated 2026-11-11. This record supersedes the live-state instructions
in Renaissance XXII–XXVI; those files remain historical evidence. Fetch live
refs and inspect actual CI checkout commits before acting. Later heads require
fresh qualification. Private consumer identity, source, grants, paths and raw
receipts are deliberately excluded.

This is the engineering snapshot **before the #323 documentation/model-test
merge**. The post-merge main SHA must be fetched; it is not self-certified by
this pre-merge file. #323's final-head CI/review remains pending at this checkpoint.

```text
MAIN: 3358f6679ad7fc53623b9757b9c1d2485e7e3b9f | CI: PASS (43008412700, eight jobs)
RELEASE: v0.6.0 / 492d0fec7727658d50b9a82025c784b7e427d6b0 | PUBLISHED: YES
MERGED: #340, #337, #333, #329, #327, #325, #336 (seven normal merges; exact commits below)
RENDERER: original red reproduced; #343 passes scoped endpoint controls; full compatibility remains HOLD
DOWNSTREAM: v0.6 and development b459 each passed 451 tests, zero failed, three ignored; production shims removed: 0
MATH: four trusted fixtures and cell fallback; integrated high-fidelity terminal math UNVERIFIED
```

Source remains `0.7.0-dev`, Rust floor 1.85, ABI v1 with 67 baseline exports.
The published Go tag `bindings/go/v0.6.0` names the same release commit.
**v0.7 remains HOLD: no RC, version bump, new tag or new release.**

## Flash table

| Workstream | State | PR / SHA | Verified evidence | Remaining blocker | Next action |
|---|---|---|---|---|---|
| Streaming forcing example | MERGED | #340 / `da40325c782a6610222830a5f1581899b8d9e06b` | Three synthetic viewport sizes; stale-cache mutation rejected | Not a stable text-flow or protected-focus API | Measure long streams, retention and real output pressure |
| Release tooling | MERGED | #337 / `de5db3c5b063307a9b4b139ee27b3053242924d5` | Captured source/artifact selection, safe cleanup, argument and native-archive controls; CI/preflight | No final release artifacts qualified | Use the hardened gate on the eventual frozen candidate |
| Mouse/screen lease | MERGED | #333 / `b41bccca4f4d000bbc52602f47df60869238030c` | Red baseline, torn-prefix controls, PTY/writer faults and gates | Does not finish all terminal effects | Finish combined output/lifecycle qualification |
| Caught-panic input | MERGED | #329 / `bf4cc2d3b405807bb4bd7e4f0190a8d14cc21165` | Exact-head CI/preflight and merged-main CI | Input-mode repair is not full session recovery | Retain the wider #328 acceptance boundary |
| Focus retry | MERGED | #327 / `b45906290045fa0f33e01a067b928bf08e21eeca` | Final head CI/preflight passed; HUP precondition race calibrated and repaired | Future output endpoint changes need requalification | Preserve distinct notices, confirmation and cleanup flags |
| Wrapper parity | MERGED | #325 / `631c5aa3156f44b131b74a654f85431e3c60669a` | Exact-head rebuilt SDK; five languages × 84 records × 500 reps; controls, PTYs and historical clients passed | This is a qualified slice, not complete Rust parity or a release | Requalify after experimental runtime integration |
| Renderer targets | HOLD | #339 / `7b839e944f152920598484bbe8e95f1ab9e61509` | Main reconciled; standalone A/B/A controls pass; managed control remains red before #343 | Public compatibility and physical-output lifecycle | Review #343 and the combined state machine |
| Managed output endpoint | WIP / HOLD | #343 / `0f8014ece2045353e31256ae0cbb6724fb1dd64b` | Eight-job CI 43008631429 passed; earlier endpoint/unit controls and migrated seven-target matrix: 70 passed, zero failed/ignored; keep-master mutant failed | Parent API/geometry/input, installed wrapper fault clients and full current-main reconciliation remain | Requalify the complete runtime and clients before promotion |
| Original Context draft | HOLD | #338 / `da9322811e4280bb486317126a42312af3c8d2d4` | Original complete-grid counterexamples retained | Superseded implementation direction must be reconciled explicitly | Avoid independently merging conflicting output state machines |
| Trusted math M0 | MERGED | #336 / `3358f6679ad7fc53623b9757b9c1d2485e7e3b9f` | Exact-head Rust 1.85 four-fixture checker and eight-job CI passed; merged-main CI passed | No owned pixel transport, untrusted-input support or stable math API | Advance independent geometry and owned display research |
| Unicode pairing boundary | IN REVIEW through #323 | #331 / `b01c7e6132f526ecae791aff43ba67d3a8694fe1` | Normally incorporated into the handoff branch; one exact Rust model test and Python mapping control passed | #323 final-head CI/review/merge pending | After #323 merges, close #331 as integrated by ancestry, not GitHub-merged |
| Downstream canary | PARTIAL | Public source `b45906290045fa0f33e01a067b928bf08e21eeca` versus v0.6 | Both first-attempt suites: 451 passed, zero failed, three ignored across 34 targets | Not a future release witness or complete pressure/terminal matrix | Repeat after final runtime and candidate freeze |

## Merged evidence and limits

The seven merge commits are listed in the table. Do not replay them. #342
`cf29b902023f007686c4b9f087d84c0b4ddfaa36` was closed as already
incorporated by ancestry after serial integration; it was not a GitHub merge.
Remote branches were retained.

#340 adds `examples/streaming_viewport.rs` and
`docs/STREAMING_VIEWPORT_EXPERIMENT.md`. Application-owned block IDs, a width
cache, an explicit held anchor and 32-block retention keep a historical view
stable while new text arrives. Synthetic status/input rows remain visible at
40×15, 80×24 and 120×40. The library does not own conversation retention or
approval policy. Retention bounds block count, not source bytes; selected rows
are still cloned, and the rebuild reference shares the wrapping implementation.
Measured 24-turn debug layout workloads were roughly 2.5–3.2 ms and 380–454 KB.
These are not 1,000-turn, terminal-latency, authority/focus or pressure claims.

#337 final head `6799092566340765446f62621dbd7f12842ca50a` passed development
CI and publication-free preflight 42985832794. Merged main
`de5db3c5b063307a9b4b139ee27b3053242924d5` passed CI 42987686868 and
exact-main preflight 42987847747. Repairs include:

- Failed `mktemp` plus `cd ""` could select the source directory for recursive
  cleanup. Intercepted-deletion red controls reproduced the regression safely;
  allocation/canonicalization and cleanup now fail closed.
- C/C++ pkg-config flags pass through `shlex`, NUL records and a Bash array,
  never `eval`. Real native clients and malformed/empty/tool-failure controls
  constrain argument handling.
- Cargo-reported libraries are selected and snapshotted before metadata queries
  can rebuild them. Candidate symbol checks build committed source explicitly.
- Native SDK installed static-archive copies remove compiler IR/options sections
  rejected by Go 1.27.4, retaining their members and native machine code. Issue #341 was closed after the source repair and controls.

None of this sandboxes a hostile toolchain or proves ABI signatures from names.
Retained preflight scratch still needs local ownership/activity checks before removal.

#333 final head `635da58ee5f53a3d40de2002cf9d9facbb83edde` passed CI
42987828362 and preflight 42987819638. #329 final head
`55087c918ef619cbc9635a8ac0d418b6a2a2c94a` passed eight-job CI 42990315543
and actual-head preflight 42990308529; merged main passed CI 42992260834.
Its bounded repair restores raw input/bracketed paste after a caught owner panic.
A late Kitty reconciliation must not erase a newly reacquired raw cleanup debt.
Issue #328 is not a claim that renderer, screen, mouse and cursor recover fully.

#327 final head `7f0ee8f77dd5b6a5c1617983084cd7c235c5e15d` passed CI
43001210185 and preflight 43001200536 before normal merge. Merged main
`b45906290045fa0f33e01a067b928bf08e21eeca` passed eight-job CI 43003830159.
Input, mouse and focus panic notices remain distinct; a completed request is
separate from cleanup responsibility and actual terminal acknowledgment.

The earlier #336 stack's `no_panic_hangup_control` failure was retained and
investigated, not dismissed as a flake. Repeated execution reproduced a precondition failure at repetition 206,
before the tested recovery call: closing the
parent's master did not establish that the kernel already reported HUP. The
repaired test observes kernel HUP within a bounded interval before measuring
recovery. A controlled 3,000-case observer probe passed, and the deliberately
stalled post-HUP recovery mutant failed with exit 101. This is calibration of
the harness precondition, not repeated library calls until success. Runtime
source equality was checked while reconciling the relevant branches.

## Exact installed-SDK and comparator evidence

#325's merge head was `24fb246b95f7cb94ea927638f36cdc07d3d719cf`, tree
`b491f58206131e689859509acaf9a2443da424ac`. CI 43004110351, actual-head
dispatch preflight 43004102509 and PR preflight 43004110641 passed. The earlier
locally tested `00d497648d93baa7fcf177d45c9bb09a3855db10` had the same tree,
but was not substituted for the final-head rebuild.

The final candidate's own release library artifacts were removed with
package-scoped Cargo cleanup; dependencies were retained. Rust 1.98.1 rebuilt
and freshly staged that exact clean head into a spaced external SDK prefix.
The current Rust parity example was rebuilt, and official Go 1.27.4 was used.
Native checksums passed; exactly 67 baseline exports were observed. Shared-library
SHA-256: `1cb41b36c023dfd03098aeb3599b16af3ec92a7fd11dff34a174434372ccbbee`.

The unmodified full harness exited 0 with all five languages emitting exactly
84 unique records in committed order, 500 repetitions and A/B/in-process replay
agreement. C/C++/Python/Go were byte-identical; Rust retained exactly 28 documented
differences. Every language had zero second-half RSS growth. The planted leak
grew 18,176 KB against the 4,096 KB gate and was caught. Comparator 12, cleanup
six and qualification-summary four controls passed. All cell/event/focus,
ABI-discriminant, common-geometry, resize and leak mutations were caught.
Serial C++/Go/Python PTYs passed request/cleanup, peer-emulated focus order,
reversed-order and ENOSPC controls. These do not prove terminal focus support.

A source review reproduced a real oracle hole: changing the C reference to
Rust's still-pinned known-gap value silently erased the expected difference.
The repaired comparator rejects disappeared gaps on both comparison legs.
An unchanged declared-gap matrix still passes. All 84 record IDs and pinned
literals were preserved. A further positive plus eight mutations of the actual
complete transcripts rejected missing, extra, duplicate, common reordered/focus/
geometry, changed pinned-gap and common healed-gap records while preserving
replay agreement. This prevents determinism alone from catching the mutation.

As an independent historical contract, the actual v0.6 C header/client and Python
wrapper/client ran against the freshly staged final-head library and reproduced
the immutable release's 77-byte ANSI frame and 13 stats. No golden was regenerated.
Earlier same-tree installed C++ testing passed 102 checks, and Python event/error
and copied external Go-module suites passed. Symbol names and these clients are
bounded ABI evidence, not every signature/layout or full engine correctness.

## Renderer destination and endpoint frontier

The original same-renderer A/B/A tests use matched color capabilities and complete
80×24 VT100 cell/attribute/cursor comparisons. A second retargets one writer object
internally. Both fail at cell (0,6) on the older #338 implementation and pass as
ordinary tests in #339. Writer addresses, session IDs, fd numbers and `isatty`
are not operational proof of persistent destination identity.

#339 compares per-target state, lifetime binding and stateless fullscreen fallback
in `docs/RENDER_TARGET_CONTRACT.md`. A bound generic writer still requires its
caller not to retarget or independently write to the destination. The first
Context-owned virtual display is explicitly known blank; arbitrary targets are
not. Draining a capture does not renew that baseline. This preserves the existing
77-byte ANSI golden and its six dirty cells. Scene/surface migrations retain 204
assertion macros and the old damage, cursor, fault and performance oracles.

The 1,260 local passes and seven historical ignored tests at
`991b1b6c64e0145b1e012ca904116b0ce0e7472d` are historical evidence only.
Later #339 `727cd3ab36747717708eb62a3048a2f338055497` added an ordinary
managed-output gate, not an engine fix: CI 42996100299 failed that gate, and the
scoped run returned 21 passes, one failure, zero ignored. Current reconciled
#339 `7b839e944f152920598484bbe8e95f1ab9e61509` retained that red runtime
behavior before the #343 successor. Old green results never overrode it.

The real Context reproducer paints A to a PTY, redirects stdout to a pipe for B,
restores stdout and requests B again. Its complete intermediate/final frame and
cursor acceptance allows correct pinned delivery, destination-aware output or an
explicit refusal without effects before `NotConnected`/`InvalidData`. It keeps
pipe descriptors alive and drains nonblockingly. Synthetic pinned/refusal positives
pass; dropping the first successful frame is rejected. Same-model reviewer findings
closed two oracle holes before promotion.

New #343, now `0f8014ece2045353e31256ae0cbb6724fb1dd64b`, is based on #339 and
experiments with a session-owned Unix endpoint, global owner access and a Weak
Context reference. Managed rendering, session mode requests, panic cleanup and
restoration use the acquired endpoint. Public unmanaged writer/input helpers
remain outside a universal lease-enforcement claim. At earlier runtime head
`ee74df9bde794c8b0c6d957b97e1cd0a1f69cd72`, twelve ordinary endpoint tests,
16 units, Rust 1.85 check and targeted Clippy passed. Buffered-output ordering
and non-TTY-constructor regressions found by review have red/green repairs.
The successor changes tests/probe/docs, not production library source. These
scopes may overlap and are not summed into a complete-suite or merge claim.

The first legacy matrix returned 68 passes and two failures because redirecting
fd 1 to `/dev/full` no longer faults a captured endpoint. The committed migration
preserves both original error/cleanup oracles while faulting the actual captured
output. The seven-target matrix now passed 70 tests, zero failed and zero ignored,
with real kernel-HUP and termios checks. Keeping the actual master alive produced
`RESTORE_RESULT=OK` and failed the intended expected-error assertion, exit 101;
the unmutated positive then passed once. Targeted migration Clippy passed, and
a separate same-model source pass found no additional defect in the helper.
CI 43008631429 completed successfully for this exact head; all eight jobs passed.
That green run does not waive the remaining source/compatibility gates below.

This does not qualify the complete parent Rust API, geometry/input endpoint
correspondence or all installed clients. In particular, #325's installed focus
client still redirects fd 1 to `/dev/full`; that technique must be migrated to
an actual captured-endpoint fault with equivalent status/detail/cleanup controls.
Current-main reconciliation, full compatibility and exact-head gates remain HOLD.

The Rust behavior cost remains explicit: unbound fullscreen calls repaint;
unbound inline/history-relative calls fail clearly. Bound session controls,
rebinding and compatibility need review for the next pre-1.0 minor. C ABI v1 is
not authorized to break. No one may obtain green by weakening failure/cleanup
oracles, deleting an ordinary red gate or merging inconsistent output designs.

## Sanitized downstream canary and math

The same isolated external consumer and workloads were compared with published
v0.6 and public development source
`b45906290045fa0f33e01a067b928bf08e21eeca`. Both first-attempt full suites
passed 451 tests, zero failed, three ignored across 34 targets, without retries.
Normal headless selfcheck exited 0. The observed idle call rate was about 25 per
second in both lanes; this is not an idle CPU percentage or all pacing regimes.
Strong startup controls passed twice; an ECHO mutation failed with exit 101.

Earlier v0.6/exact `e64402526562ce0d7b6d88c0b5f7bb31152574d6`/isolated
public-API substitution variants each passed 17 scoped PTY tests; thirteen
headless rendering tests and seven unit cases passed. These support further
workaround retirement in isolated copies, but **zero production shims were
removed** and no external main branch was modified. The suite is not the full
size/color/reduced-motion, slow-backend, reconnect, memory-pressure and output-
backpressure matrix. A future runtime/release SHA needs its own canary.

#336 merged normally at `3358f6679ad7fc53623b9757b9c1d2485e7e3b9f` from
head `df6ab6162c95c39220d22d78f4d60ab4e9cf61a8`. Fresh Rust 1.85 ran the
full four-fixture checker on that exact head, exit 0; eight-job CI 43006713951
passed, followed by merged-main eight-job CI 43008412700. Bounded trusted-source
review found no additional blocker within that scope. The checker honors external/
spaced Cargo targets. Six experiment files formerly entered the crate;
`/experiments` now excludes them, and the post-change package inspection found zero.

The fixed integral, nested fraction, matrix and aligned sources go through
pdfTeX/Poppler, `RgbRaster`, `Node::raster` and headless capture. Mathtext is a
separate subset comparison. Source choices do not accept arbitrary model/user
TeX. No filesystem/network sandbox, untrusted-input interface, default heavy
dependency, stable math API or new ABI is qualified. No fonts are vendored.
Full integral/delimiter metrics and clipping remain open. The separate Kitty
probe is not a LibGibson-owned image transport or paper-quality equation display.
The current cell oracle rejects CSI `J`; calibrate legitimate clear-screen
handling when moving this experiment onto the renderer branch.

#331 `b01c7e6132f526ecae791aff43ba67d3a8694fe1` was normally incorporated
into the #323 handoff branch as a 35-line documentation/model-test correction,
without runtime logic changes. One exact Rust Unicode test passed; an earlier
short filter selected zero tests and is retained but not counted as a pass.
A separate Python mapping control passed. Final #323 head CI/review/merge is
still required before closing #331 as integrated by ancestry, not as a GitHub
merge. Its lowercase-expansion example is a decoder model, not a physical
keyboard experiment. #312 remains open: legacy shifted `!`
and Kitty base-key `1` release can leave an indefinite hold. Do not invent
physical provenance or time out a genuine long hold merely to hide ambiguity.

## Merge queue and release gates

| Order | PR / checkpoint head | Next executable decision | Verdict |
|---|---|---|---|
| Tier A documentation/model test | #323 incorporating #331 `b01c7e6132f526ecae791aff43ba67d3a8694fe1` | Review the final handoff/roadmap/state/model-test diff and exact-head CI; after normal merge close #331 by ancestry | HOLD pending final-head gates |
| Runtime stack | #338 `da9322811e4280bb486317126a42312af3c8d2d4` → #339 `7b839e944f152920598484bbe8e95f1ab9e61509` → #343 `0f8014ece2045353e31256ae0cbb6724fb1dd64b` | Current eight-job CI passed; finish parent API/geometry/input/client fault qualification and current-main reconciliation; choose coherent integration | HOLD |

The runtime arrows describe source dependencies, not permission to land a red
intermediate parent. Choose a coherent successor or incorporate the endpoint
repair into the parent branch and requalify that combined head; do not merge the
current red #339 merely to reach #343.

Use full expected head SHAs for normal merges, preserve protections and do not
merge a stale stack merely because it is mergeable. #342 is already closed by
ancestry, not another merge candidate. Unrelated divergent/frozen research stays
out of this queue; no remote branches are deleted.

| v0.7 predicate | Current evidence | Remaining gate |
|---|---|---|
| Managed destination/lifecycle | Scoped #343 endpoint controls plus 70-test actual-fault legacy matrix pass | Full parent API/geometry/input/client compatibility, independent assessment and combined integration |
| Ownership and caught panic | #333/#329/#327 merged; notices and HUP preconditions calibrated | Requalify all relevant operations after endpoint redesign; #330/#328 claims remain bounded |
| Critical ordinary controls | Managed gate ordinary; focused successor passes | No ignored or surviving release-critical red on final candidate |
| Exact-candidate CI/preflight | Wrapper head passed all gates; latest main eight-job CI passed | One frozen release SHA with fresh complete CI/preflight |
| ABI v1 | 67 names and genuine v0.6 header/client witnesses | Exact historical signatures/layouts and final installed clients |
| Rust 1.85/frozen audio | Floor retained; math 1.85 and preflight checks passed | Final locked/fresh-resolution and unchanged PCM/HumanMusic receipts |
| Five installed languages | Fresh exact #325 SDK, 84×500 and real focus PTYs passed | Repeat for final combined runtime and release artifacts; full parity remains broader |
| Terminal/hangup | Scoped red/green/negative matrices and kernel observation | First-attempt final combined fault/teardown matrix |
| External consumer | v0.6/b459 first-attempt 451/0/3 suites and startup controls | Final source plus remaining pressure/size/color/accessibility matrix |
| Resources/performance | Scoped RSS and retained-text evidence | Long-session memory, slow output, pacing and graphics envelopes |
| GPL node J | In-tree adapter and separate local prototype remain distinct | Authorized published external port or explicit milestone scope decision |
| Release artifacts/review | No version PR or tag; release tooling improved | Changelog/notices/licenses, independent final review, installed/downloaded assets |

Linux x86_64 Engineering Alpha is the intended v0.7 scope. Windows remains
UNVERIFIED. Use `docs/RELEASING.md` §§0–6; the maintenance-line exception does
not apply. Do not publish registries or rewrite existing tags.

| Version | Implemented/verified scope | Released | Blocking requirement |
|---|---|---|---|
| v0.7 | Input/focus/lease repairs, installed wrapper slice and release tooling merged | NO | Renderer/lifecycle integration and complete exact-candidate gates |
| v0.8 | Focus/geometry projection qualified; streaming forcing example and bounded trusted math research merged | NO | Broader native language, semantic, lifecycle and accessibility parity |
| v0.9 | Portability/pressure and math transport remain research fronts | NO | Long sessions, capability probes, graphics lifecycle, SSH/tmux and accessibility |
| v1.0 | Existing release contract; stable CORE still prospective | NO | Frozen target/ownership/error/ABI contracts, reproducible artifacts and independent downstream acceptance |

## Five verdict-changing probes

1. **Endpoint truth beyond the first green.** On #343, combine physical/headless
   output, owner handoffs, buffered host bytes, non-TTY construction, redirects,
   panic and restore. Require all intermediate/final VT cells and cursor state,
   geometry/input correspondence, no effects before refusal, and closure of the
   session-owned endpoint. A fresh
   pinned frame alone does not establish every lifecycle path.
2. **Actual faults versus obsolete injection.** Prove the captured endpoint has
   HUP before expecting I/O failure; a retained master must make that precondition
   fail. Exercise partial writes/flushes, restore and late Kitty reconciliation
   without dropping cleanup debt. Preserve first failures and bounded observers;
   never replace a failing endpoint test with a successful write to another fd.
3. **Source/artifact and shared-oracle integrity.** Rebuild the final source in
   an audited prefix, poison stale libraries/crates/targets and fail allocation.
   Require no source deletion, exact provenance, unchanged known-gap literals,
   genuine historical clients and signature/layout checks beyond symbol names.
4. **Operator-held long streams.** Stream 1,000 synthetic turns while held 200
   rows above tail, resize and evict. Require stable logical content/explicit
   eviction, semantic text preservation and bounded source/layout cost. Test
   actual output pressure and keyboard/focus precedence rather than cache timing alone.
5. **Mathematics in the real display.** Independently constrain fraction,
   integral, matrix and alignment geometry, then observe an owned image's create/
   replace/clip/resize/delete lifecycle on a qualified terminal. Retain semantic
   source and honest half-block/Braille fallback; do not feed untrusted TeX to
   the current trusted toolchain experiment.

## Commands, provenance and local cleanup

Select development CI by workflow name, event, exact source SHA and expected job
set. A successful Dependabot workflow is not the eight-job development gate.
The initial draft mistakenly cited run 43008614279 for main; it was a one-job
Dependabot run. The corrected exact-main CI is 43008412700, verified as a push
at `3358f6679ad7fc53623b9757b9c1d2485e7e3b9f` with all eight jobs successful.

CI/preflight IDs above are public receipts. Detailed stdout, artifact manifests,
process inventories, private consumer receipts and cleanup paths stay local.
Reproduction commands are instructions, not claims that every listed command ran
on current main. Use the specified branch/head, a writable absolute target and
serialized ownership/signal/PTY tests. Set the actual official Go toolchain on
PATH before parity; this round used Go 1.27.4 and Rust 1.98.1. The dedicated
Cargo caches were cleaned after qualification: retained SDKs/manifests/transcripts
remain, but future Rust example/test commands must rebuild in a writable target.

```sh
# Current installed wrapper slice: freshly staged SDK, no prebuilt Rust shortcut.
unset PARITY_V08_RUST_BIN
RUSTUP_TOOLCHAIN=1.98.1 CARGO_BUILD_JOBS=1 scripts/release/stage-sdk.sh --prefix "$SDK_PREFIX"
PARITY_V08_REPS=500 PARITY_V08_SKIP_CONTROLS=0 CARGO_BUILD_JOBS=1 GOMAXPROCS=1 \
  scripts/dev/parity_v08.sh "$SDK_PREFIX"
python3 bindings/parity/v08/test_compare.py
python3 bindings/parity/v08/test_cleanup.py

# Merged release-tool controls; intercepted cleanup, tiny native fixtures, no Cargo build.
scripts/release/tests/test-cleanroom-scratch.sh
scripts/release/tests/test-cleanroom-pkgconfig.sh
scripts/release/tests/test-native-sdk-archive.sh
python3 scripts/release/tests/test-isolated-preflight.py

# Combined terminal acceptance, using the matching integrated branch.
cargo +1.98.1 test --locked --test caught_panic_input_pty \
  --test panic_recovery_hangup_pty --test panic_focus_order_pty \
  --test mouse_lease_pty --test key_release_reporting_pty -- --test-threads=1
# Renderer/successor ordinary acceptance; do not add --ignored or waive a red case.
cargo +1.98.1 test --locked --test context_output_identity_pty -- --test-threads=1

# Trusted M0 experiment, from its exact qualifying branch.
cargo +1.85 build --locked --example math_m0_capture -j 1
PYTHONNOUSERSITE=1 /usr/bin/python3 experiments/math_m0/check.py

# Publication-free candidate gate; this does not create a release.
STABLE_TOOLCHAIN=+1.98.1 MSRV_TOOLCHAIN=+1.85 scripts/release/preflight.sh
```

Review passes were by separate agents in the same model family, plus distinct
compiler/emulator/backend executions and published historical contracts. These
are not independent human or different-model certification. Final independent
release review remains absent. Successful write/flush is not physically atomic
I/O or terminal-support acknowledgment.

Audited scoped cleanup removed **14.29 GiB of allocated storage, gross**,
including the earlier disposable downstream copies/caches. This is one scoped
total, not an additional amount to sum with older partial receipts, and not a
net round-wide free-space gain. Detailed host measurements remain local.

Only the primary LibGibson worktree remains registered. All three temporary
LibGibson checkouts created this round were removed by normal Git procedures,
without force, after checking source preservation, pushed ancestry, locks and
activity. All source blobs in the relevant audited archive were verified before
explicit disposable-file cleanup. Branch refs were unchanged, connectivity
checking passed and the final worktree-prune dry run was empty; no metadata
prune or Git garbage collection was used.

Dedicated compilation caches were cleaned through normal Cargo commands.
Both newly staged SDK prefixes, their unchanged native checksums, manifests,
transcripts and local raw receipts remain accessible. Protected lab/bundles,
the original SDK and prior receipts were untouched. Future commands must rebuild
the removed Cargo target; retained SDKs must not be relabelled as artifacts of a
later source tree. No public host-path inventory belongs here.

## NEXT CODEX 6.1 SOL XHIGH ROUND — BOOTSTRAP FROM LIVE STATE

Repository `github.com/femboy2112/libgibson`; fetch before acting. Checkpoint main
`3358f6679ad7fc53623b9757b9c1d2485e7e3b9f`, `0.7.0-dev`, Rust 1.85,
ABI v1/67 baseline exports. Main CI 43008412700 passed all eight jobs. This is
the snapshot before the pending #323 documentation/model-test merge; fetch its
actual post-merge main SHA before resuming.
Published `v0.6.0` and Go tag remain
`492d0fec7727658d50b9a82025c784b7e427d6b0`; v0.7 is HOLD with no RC/tag.

Already merged this round: #340
`da40325c782a6610222830a5f1581899b8d9e06b`; #337
`de5db3c5b063307a9b4b139ee27b3053242924d5`; #333
`b41bccca4f4d000bbc52602f47df60869238030c`; #329
`bf4cc2d3b405807bb4bd7e4f0190a8d14cc21165`; #327
`b45906290045fa0f33e01a067b928bf08e21eeca`; #325
`631c5aa3156f44b131b74a654f85431e3c60669a`; #336
`3358f6679ad7fc53623b9757b9c1d2485e7e3b9f`. #342 was closed as already
incorporated by ancestry, not merged. Do not duplicate those repairs.

Queue: #323 normally incorporates #331
`b01c7e6132f526ecae791aff43ba67d3a8694fe1` and the historical handoffs,
roadmap/state corrections and this file. Qualify the final documentation/model-
test head with CI/review, merge normally, then close #331 as integrated by
ancestry, not GitHub-merged. Its one exact Rust test and Python mapping control
passed; an initial zero-test filter is not counted. The claim is a Unicode
decoder model, not physical keyboard evidence. #336 is already merged after
exact-head CI 43006713951 and a fresh Rust 1.85 four-fixture checker, exit 0.

Runtime DAG: #338 `da9322811e4280bb486317126a42312af3c8d2d4` → #339
`7b839e944f152920598484bbe8e95f1ab9e61509` → #343
`0f8014ece2045353e31256ae0cbb6724fb1dd64b`. #339's original managed-output
control is red before the successor. #343's session-owned Unix endpoint makes
12 ordinary endpoint tests and 16 units pass, with Rust 1.85/Clippy green,
but full API/compatibility is HOLD. Buffered-output/non-TTY regression controls
were repaired red-first. The committed two-fixture migration preserves the old
error/cleanup oracles and faults the captured endpoint: seven legacy targets
pass 70/0/0, with kernel HUP/termios checks. A retained-master mutation produced
RESTORE_RESULT=OK and correctly failed the expected-error assertion, exit 101;
the restored positive passed. Current CI 43008631429 passed all eight jobs. The DAG does not authorize
merging red intermediate parents: choose one coherent combined head and requalify.
Qualify parent Rust API, geometry/input correspondence, current-main reconciliation and the
installed wrapper clients; their old fd-1 `/dev/full` fault technique still needs
actual-endpoint migration. Preserve the unchanged ANSI golden, original full-
grid/cursor oracles and known Rust behavior costs. Scoped green is not release GO.

#325 final head `24fb246b95f7cb94ea927638f36cdc07d3d719cf` passed all hosted
and actual-head preflight gates plus an exact-head rebuilt external SDK, all
84×500 relative records, mutations and serial focus PTYs. Genuine v0.6 header/
wrapper clients retained the 77-byte historical frame. Those results do not
qualify future endpoint/renderer code; repeat relevant clients afterward.

Downstream forcing consumer: published v0.6 and public development b459 each
passed the full first-attempt 451/0/3 suite across 34 targets, with headless
selfcheck, limited idle/startup controls and a rejected ECHO mutation. Earlier
isolated public-API substitution had 17 scoped PTY passes; production shims
removed: zero. No external main or private capabilities were changed. A final
candidate still needs the remaining pressure/color/size/accessibility matrix.
Private source, identity, hashes, payloads and raw receipts stay private.

Remaining release predicates: complete managed endpoint/lifecycle and public API
assessment; exact-candidate CI/preflight, ABI/layout/signatures, Rust 1.85,
frozen PCM/HumanMusic, installed artifacts, exact downstream canary, resources,
GPL node J decision, accurate notices/changelog and independent final review.
Math is trusted fixed-source cell fallback, not an owned pixel protocol or
untrusted-input interface. Large streaming/protected focus, graphics transport
and scientific fronts remain scoped independent work.

Local cleanup removed 14.29 GiB allocated, gross across the audited scopes;
this is not net round-wide free-space recovery. Only the primary LibGibson
worktree remains registered; three temporary checkouts were removed normally
without force. Branch refs/source were preserved and connectivity checks passed.
Dedicated Cargo caches were cleaned, so future Rust consumers/tests must rebuild.
Both new staged SDKs, manifests/transcripts, protected lab/bundles, original SDK
and local receipts remain intact. Re-inventory current activity before new builds
or cleanup. Preserve unique branches/unpushed work, and use normal Git removal
only after safety gates. At most two heavy builds, with modest per-build jobs,
and serial PTY/signal tests.

Authorization continues for scoped LibGibson branches, commits, pushes, issues,
PRs and qualified normal merges with full expected head SHAs. GitHub Engineering
Alpha release remains conditional on every exact-candidate gate. No force-push,
protection bypass, tag rewrite, registry publication, ABI-v1 break, frozen-audio
change, remote-branch deletion, private-capability activation or other-project
main modification.

First commands from the LibGibson checkout:

```sh
git rev-parse --show-toplevel
git status --short
git worktree list --porcelain
git fetch origin
git rev-parse origin/main
gh pr list --repo femboy2112/libgibson --state open --limit 100
gh pr checks 343 --repo femboy2112/libgibson
gh pr checks 339 --repo femboy2112/libgibson
gh pr checks 323 --repo femboy2112/libgibson
gh run list --repo femboy2112/libgibson --workflow ci.yml --branch main --limit 3
gh run list --repo femboy2112/libgibson --commit "$(git rev-parse origin/main)" --limit 5
df -h .
```

Read this file, historical handoffs with archival labels, the renderer contract
on #339, `docs/V07_EXECUTION_GRAPH.md`, `docs/V08_PARITY_SLICE.md`,
`docs/MATH_TYPESETTING_ROADMAP.md`, `docs/RELEASE_CONTRACT.md`,
`docs/RELEASING.md` and live discussions. Resume from actual failed predicates;
never promote a historical green run into qualification of a later tree.
