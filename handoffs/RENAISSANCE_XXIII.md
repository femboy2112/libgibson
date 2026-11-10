# Renaissance XXIII — live-state bootstrap

Round baseline: `main` `4573d0d6e571159b376e08fb2761b4e8eaffe6f3`
(`0.7.0-dev`); development CI run 42578229665 succeeded at that SHA. #322 then
merged as `b04e9c42d6484506373355d0a3538254d87d16ba`; verify its main CI live.
The latest published prerelease is v0.6.0; both `v0.6.0` and
`bindings/go/v0.6.0` peel to `492d0fec7727658d50b9a82025c784b7e427d6b0`.
No v0.7.0 release candidate was qualified. Fetch and verify live state.

## Findings and review boundaries

- #312 remains open. The parser can emit the same public observations for a legacy
  shifted `!` whose physical key was released under Kitty base key `1`, and a still-held
  exact `!` accompanied by an unrelated `1` release. A timeout driven solely by those
  observations cannot distinguish them. Exact Kitty special keys can also use internal
  `KeyId::Code`, so that identity alone is not a legacy-format marker. Draft #326 pins
  these constructed protocol traces; it does not implement a fix or prove hardware-layout
  behavior. The existing indefinitely-held condition is conditional on no recovery event.
- The #305 failed-flush boundary is best effort, not atomic restoration. Merged #322
  corrects historical claims and models the #312 failure; its hold test is not an actual
  screen-transition test. The release procedure for v0.7.0 follows the main-line
  `docs/RELEASING.md` §§0–6, not the maintenance-line §7.
- A focus enable can fail during output and leave cleanup ownership set. On baseline,
  retrying can return success without sending an enable request. A PTY/C-ABI experiment
  injected `/dev/full`, saw the first call fail, then saw a success return with no PTY
  output. The experiment used the previously staged baseline binary; the source-level
  writer-seam regression was red on baseline. Draft #327 (`e080e2fe585aa1cdd1e35acbfc97a3890e6c404b`
  at initial submission) adds retry bookkeeping; focused writer, keyboard-switch and FFI
  tests passed locally. A later C-ABI PTY retest of the fix passed: after an injected
  failed enable, the retry wrote an enable request. Exact-head CI remains needed. Torn terminal writes remain physically uncertain.
- Windows #292 is not verified. Clock #130 still needs loop/blocked-output evidence.
  HumanMusic/PCM and ABI v1 are frozen. Never infer terminal behavior from headless parity.

## Integration and verification

The XXI train #316, #318, #319, #317, #320, #321 is merged; do not repeat it. Drafts #325 (`32bd96fd0483444ec5d1cfb9bd89a06647dadaed`), #324
(`eecdd341465d4268f6dc99b13ef0fd1aa25a0caf`), #326
(`0d7ee4f12a10b4b220d5370eb28ff310683b30cd`) and #327
(`ceb375e54c27622581162f6e31ea6224181accaf`, based before #322) need
current-head and dependency review, CI and appropriate independent review before merge. #324 contains five headless starters and two interactive public-Python-API applications:
an offline echo console and a simulated dashboard. Scientific, framebuffer, and other
requested application examples remain outstanding. Its SDK path with
spaces regression is fixed on the current branch and has real C/C++ and negative controls.
A new Linux PTY control passed for C++, Go and Python enable/disable/restore bytes, a
no-op negative control and an injected output error; it does not prove terminal support.
The full parity harness has not yet been rerun on the updated head.
PRs #153, #154, #156–#164 and #166 were closed as already integrated by ancestry after
checking each exact head and surviving tests/docs against the round baseline; they were not GitHub merges. #173 and
#177 have divergent history and remain open. #173 contains a design handoff absent from
main; #177 contains a scene-editor example and PTY tests absent from main. Remote branches
were retained.

For v0.7.0 retain HOLD until the complete exact-candidate release gates, five installed
consumers, genuine external v0.6-versus-candidate canary, PTY/hangup checks, performance,
ABI compatibility, MSRV, audio goldens and independent final review all pass. Do not
publish crates.io/PyPI, alter old tags or delete remote branches. Do not edit another
project's main or enable private capabilities.

## First commands

```sh
git status --short
git worktree list --porcelain
git fetch origin
git rev-parse origin/main
gh pr list --repo femboy2112/libgibson --state open --limit 100
gh pr checks 322 --repo femboy2112/libgibson
gh pr checks 325 --repo femboy2112/libgibson
df -h .
```

Recheck every PR's exact head, its base and all check runs. Serialize heavyweight builds,
use isolated writable target directories and preserve test output and actual exit status.
Keep private consumer paths, environment details and receipts out of public issues and files.

The downstream v0.6 baseline test run passed 451 tests with 3 ignored and a separate
headless self-check passed. Its corresponding development run uses the round-baseline
runtime `4573d0d6e571159b376e08fb2761b4e8eaffe6f3`, not the later main SHA; consult
the local private receipts for completion and comparison. Do not treat this as an
exact-current-main release candidate gate.
