# LibGibson Renaissance XXI — Operation Total Overclock — handoff

Durable round record. Date: 2026-11-09. Baseline `main` at round start:
**`4bff68d00d981b4184a5e8c768138cf09a0c6747`** (`0.7.0-dev`, C ABI v1 = 65 symbols). The body below
(sections A–G) is the plan **as it stood at round start**, with all work in open PRs; section 0 records
the **actual outcome** — the merge train ran to completion. Where the two disagree, section 0 wins.

## 0. COMPLETION STATUS — merge train FINISHED (2026-11-09)

All five development PRs merged to `main`, in this order, each with exact-head CI green and (for Tier B)
an owner-condition record comment + distinct-model (Codex) review:

| # | PR | what landed | final head | notes |
|---|---|---|---|---|
| 1 | #316 (#305) | screen-switch kitty-flag-stack restore | merged | Tier B; CI fixed a clippy `io_other_error` (lint runs `--tests`) |
| 2 | #318 (#312) | shifted-symbol limit disclosed; recovery is conditional, not time-bounded | merged | Tier A |
| 3 | #319 (#292) | Windows key-doubling disclosed | merged | Tier A |
| 4 | #317 (#298) | C `gibson_get_terminal_size` + focus events, **65→67** | `051828e` | Tier B; CI caught a real defect — #298 surfaces focus in the C ABI, and two pre-#298 C-ABI tests in `key_release_kind_pty.rs` still encoded the focus→`NONE` drop; fixed (oracle preserved), Codex CONFIRM |
| 5 | #320 (#130) | `FixedStepClock` pause/resume/step + falsifiers | `0339263` | Tier B; `time_scale` **cut** (partition-invariance/NaN-carry); Codex FLAGged a P3 `step_frame` ceiling doc/contract mismatch → documented + mutation-proven boundary test |

`main` after the five merges: **`de979ee40b517b46e7c8382057d2507b0e993e34`** (this handoff PR, #321, is the
final Tier A merge on top). C ABI v1 = **67 symbols**. `GIBSON_ABI_VERSION` still 1. No force-push, no
ABI break, no registry publish, no frozen-audio change. This PR also reconciles the framebuffer-adapter
README F2/F3 gap notes (terminal-size query and focus events are now in the ABI as of #298; adapter
adoption is tracked follow-up — the adapter behavior is unchanged).

**Still open after this round** (unchanged from the plan): v0.7.0 stays on **HOLD** (needs MINERVA
full-suite requalification against the merged `main` + the release gate); zombie PRs #153–#167 remain for
the owner to close; Front M, wrapper parity (#298 mirrors), #129 and #134/#135/#136 are next-round work.
Section G's next-round prompt is updated for this post-merge reality.

## A. Flash status

```
MAIN: de979ee40b517b46e7c8382057d2507b0e993e34 (after the 5 merges; #321 handoff on top) | C ABI = 67
RELEASE: v0.6.0 (tag → 492d0fe) | published: YES (prerelease) ; v0.7.0: NO (HOLD)
MERGED THIS ROUND: 5 (#316 #318 #319 #317 #320) + this handoff (#321) | open dev PRs: 0
BLOCKERS: 0 hard | merge train COMPLETE (serialized, each exact-head CI green)
NEXT MOVE: v0.7.0 requalification (MINERVA full suite) + release gate, OR harden (Front M / wrappers / #129)
```
(The line above is the post-round outcome; the table that follows is the round-start plan — see section 0.)

## B. Workstream table

| Lane | Status | Deliverable | PR / head | Gate | Next action |
|---|---|---|---|---|---|
| A #305 | READY | failed screen-switch no longer foreign-pops the kitty flag stack | #316 / `5688ec0` | Tier B: red-first + 2 mutation controls; Codex ×3 (core FIXED; residual §9-disclosed) | merge on green CI + record |
| C #298 | READY | C `gibson_get_terminal_size` + focus events, **65→67** symbols | #317 / `b154bb8` | Tier B: check-abi 67/67; Codex (P2/P3 fixed; test non-vacuous) | merge on green CI + record |
| B #312 | READY | shifted-symbol screen-switch limit disclosed; recovery is conditional (3 tests) | #318 / head | Tier A: tests + docs, no runtime change | merge on green CI |
| P #292 | READY | Windows key-doubling **disclosed** (no Windows env) | #319 / `6d00ad6` | Tier A: docs + 1 host-indep test, no runtime change | merge on green CI |
| D #130 | IN REVIEW | clock pause/resume/step + time-scale + 2 falsifiers | #320 / `5cd887d` | Tier B: Codex found time_scale partition-invariance edge → fix in progress | land fix, re-verify, re-Codex, then merge |
| N MINERVA | VERIFIED | external consumer (ProjectGHOST) canary vs v0.7-dev `main` | — | builds (exit 0); 1 suite flake is NOT a v0.7 regression (A/B) | requalify full suite before a v0.7.0 tag |
| Front M onboarding | NOT STARTED | zero-to-working examples + smoke-check | — | — | next round (see prompt below) |
| Fronts E/F/G/H/J | REFUTED (mostly done) | instrument epic #126–#141 | landed on `main` | #153–#167 are zombie PRs (work already in `main`) | close zombies; real residual = #129 dirty-rect, #134/#135/#136 controls/fields/probes |

## C. Version gate table

| Version | State | Qualified SHA | Blocking gate | Next action |
|---|---|---|---|---|
| v0.6.0 | RELEASED | `492d0fe` | — | preserve tags |
| v0.7.0 | DEV / **HOLD** | `4bff68d` (+ the 5 PRs) | MINERVA full-suite requalification + run the release gate (version bump, artifacts, preflight, installed-SDK consumers) | merge the 5 PRs; then execute `RELEASING.md` §7 for a 0.7.0 candidate |
| v0.8.0 | DEV | — | five-language wrapper parity (focus/size mirrors from #298 not yet in C++/Go/Python) | mirror the two new #298 symbols + constants into the wrappers |
| v0.9.0 | PLANNED | — | platform/stress/perf envelopes | — |
| v1.0.0 | PLANNED | — | frozen CORE contract + independent downstream acceptance | — |

**Release status is NOT inferred from the Cargo version.** `main` is `0.7.0-dev`; no v0.7.0 tag exists.

## D. Merge queue — the launchpad

All five branches are off `4bff68d` (= current `main`), so the FIRST merge is up-to-date; each later
one needs a branch update (merge `main` in) + CI re-run before it merges (strict up-to-date). CHANGELOG
is touched by #317/#318/#319 (trivial append conflicts); `tty_source.rs` by #316 & #318 (different
regions). `main` is protected: 6 required ci.yml checks, strict, **merge commits only**, and the merge
needs the FULL 40-char head via `--match-head-commit` (a short SHA is rejected).

| Order | PR | head | dep | CI | review | GO/HOLD | command |
|---|---|---|---|---|---|---|---|
| 1 | #316 | `5688ec0…` | — | pending | Codex ×3 ✓ | GO on green | post owner-condition record, then `gh pr merge 316 --repo femboy2112/libgibson --merge --match-head-commit <FULL-40>` |
| 2 | #318 | head | rebase after 1 | partial green | Tier A | GO on green | update branch, `gh pr merge 318 --merge --match-head-commit <FULL-40>` |
| 3 | #319 | `6d00ad6…` | rebase after 2 (CHANGELOG) | pending | Tier A | GO on green | update branch, merge |
| 4 | #317 | `b154bb8…` | rebase after 3 (CHANGELOG) | pending | Codex ✓ | GO on green | update branch, merge |
| 5 | #320 | `5cd887d…`→fix | rebase after 4 | re-run after fix | Codex re-review pending | HOLD until time_scale fix lands + re-Codex | land fix, then merge |

Owner-condition record (post as a PR comment before each Tier B merge): exact head SHA; CI run id (8/8);
Release Preflight run id (PASS); the distinct-model (Codex) review summary; the red-first + mutation
controls; "no unacknowledged support-contract regression".

## E. Top five next probes

1. **Run the full ProjectGHOST suite against v0.7-dev with `--no-fail-fast`, A/B every failure vs v0.6.0.**
   Rival it distinguishes: "v0.7-dev regresses MINERVA" vs "GHOST-side parallel-test flakes". The one
   failure seen this round (`d2b_second_crash…`) already A/B'd as a flake (passes 3/3 alone on both
   v0.6.0 and v0.7-dev). Pass/fail: any v0.7-only failure is a real regression → file it; all-flakes →
   MINERVA requalified. Deliverable: the compatibility matrix + a requalification note.
2. **time_scale partition-invariance (#130/#320).** Hypothesis: carrying the fractional scaled-nanosecond
   remainder across `advance()` restores fps/partition-invariance at non-unit scales. Rival: a documented
   "approximate at non-unit scale" limitation. Test: same total real time, coarse vs fine partition, at a
   fractional scale → identical ticks. Deliverable: the fix + test (in progress) or the disclosure.
3. **Mirror #298 focus/size into C++/Go/Python wrappers (v0.8 parity).** The two new symbols +
   `FOCUS_GAINED=7`/`FOCUS_LOST=8` under-expose in the wrappers (additive gap, not a break). Test: the
   76-record parity harness gains focus/size rows SUPPORTED. Deliverable: wrapper PRs.
4. **Close the zombie PRs #153–#167** (their work is already in `main`; heads are ancestors of `main`).
   Deliverable: closed PRs with a "superseded — landed in main" note; keep #173 (the hit-map unification
   design) and re-evaluate #177 (wave3 editor, dead base) on its merits.
5. **#129 dirty-rectangle / region-limited RGB ingest** — the one genuinely-missing instrument-epic
   primitive (frame + bounds landed; `grep dirty src/` = 0). Measure the existing full-frame path first
   (the issue's grids), add a bulk/dirty path only if an installed consumer proves the need.

## F. Commands & receipts (this round)

- Baseline: `main` `4bff68d`, `0.7.0-dev`, ABI 65 (`grep -cE '^[a-zA-Z]' abi/gibson-abi-v1.symbols`).
- #305: `cargo +1.98.1 test --lib` 1051✓; `--test key_release_reporting_pty` 21✓, `key_release_kind_pty`
  7✓, `ffi_key_release` 5✓, `input_hold` 19✓, `input_hold_released` 12✓; `clippy --tests` + `fmt` clean.
  Red-first: mutation M1 (drop claim-after-pop) → double-pop test RED; M2 (drop reconciliation) →
  raw-mode test RED; M(item1) (claim before pop) → pop-failure test RED; M(P2) (set bit after re-push)
  → re-push test RED. Codex 3 rounds (01a1be12…, 01a1be26…, 01a1be39…).
- #298: `bash scripts/release/check-abi.sh` → "baseline 67 symbols; built 67 symbols … OK". `ffi_focus`
  6✓ (incl. a polled-RESIZE-updates-size test); `ffi_abi_layout_freeze` 7✓ (event discriminants 0–8
  pinned); ffi.rs unit test `get_terminal_size_clamps_a_zero_dimension_to_80x24` mutation-proven. Codex
  (01a1be1e…, 01a1be31…).
- #312: `issue_312_*` 3✓ (limit pinned; conditional focus-loss + reporting-disable recovery). No runtime change. A hold may persist indefinitely without those events.
- #292: `cargo +1.98.1 test --lib input` 47✓; predicate unchanged; `windows_delivers_…_two_key_events_292`
  host-independent. Disclosure only.
- #130: `cargo +1.98.1 test --lib clock` 23✓; `--lib` 1052✓; falsifier #2 = 62 ticks + identical digests
  across 15/30/60/unlimited fps + slow/blocked consumers (frames 15/30/60/1000 differ). Codex (01a1be4d…)
  found the time_scale partition edge → fix in progress.
- MINERVA canary (isolated throwaway clone; ProjectGHOST real repo untouched): `projectghost v0.3.0`
  built against `libgibson …?rev=4bff68d` → `minerva`+`ghostd`, exit 0, 2m15s; its tests compile
  (exit 0); A/B of the one full-suite failure → passes 3/3 alone on BOTH v0.6.0 and v0.7-dev ⇒ GHOST-side
  parallel flake, NOT a v0.7 regression. (ProjectGHOST PR #66, the operator-gated v0.6.0 upgrade, MERGED.)

Not done, honestly: no v0.7.0 tag/release; MINERVA full suite not run green end-to-end; Front M not started.

## Scope & authorization boundaries honored

No force-push; no ABI break (additive 65→67 only, `GIBSON_ABI_VERSION` stays 1); no registry publish;
no frozen-audio change; ProjectGHOST's real repo and `main` untouched (isolated clone only); no GPL code
in the repo; private consumer specifics kept out of this file and the public tracker. Worktrees cleaned
(`.krc`/`.r20rc` removed; branch `feat/v0.7-key-release-c-abi` deleted; ~12G reclaimed, disk 27→77G).

---

## G. NEXT CLAUDE MEGA-ROUND — BOOTSTRAP FROM CURRENT STATE

Copy-paste into a fresh session. Assumes zero prior context.

```
You are the integration commander for LibGibson (github.com/femboy2112/libgibson), a Rust TUI
library with a C ABI v1. VERIFY ALL LIVE STATE FIRST — the SHAs below are from 2026-11-09 and
may have moved.

LIVE STATE (verify):
- main = de979ee40b517b46e7c8382057d2507b0e993e34 (+ the #321 handoff merge on top), version 0.7.0-dev,
  C ABI v1 = 67 symbols on main. (Renaissance XXI's 5-PR merge train is DONE — see below.)
- v0.6.0 RELEASED (tag v0.6.0 and bindings/go/v0.6.0 → 492d0fe, maintenance line). DO NOT touch its tags.
- Toolchain: CI pins rustc 1.98.1 — gate with `RUSTUP_TOOLCHAIN=1.98.1 cargo +1.98.1 …`. MSRV 1.85.
- main protected: 6 required ci.yml checks, strict up-to-date, merge commits only; merge with
  `gh pr merge N --merge --match-head-commit <FULL-40-char-SHA>` after an owner-condition record comment.
- STALE-ARTIFACT TRAP: rebuild before trusting scripts/release/check-abi.sh (the on-disk .so can lag).
- CI lints `--tests`; `cargo clippy --tests --all-targets` before any push (a `--lib`-only clippy misses it).

RENAISSANCE XXI MERGE TRAIN — COMPLETE (verify, do not re-merge): #316 (#305), #318 (#312), #319 (#292),
#317 (#298, 65→67 symbols), #320 (#130 clock) all merged; #321 (this handoff) is the final Tier A merge.
No open development PRs remain from this round.

COMPLETED — DO NOT REDO:
- The SmartChem instrument epic (#126/#127-core/#128/#129-frame/#132-core/#134-numeric/#137/#141) is
  already on main. #153–#167 are ZOMBIE PRs (work merged); CLOSE them, don't resurrect their bases.
- #130 u32-overflow is already fixed (boundary test in src/clock.rs). #305/#312 handled (fix / disclosed
  boundary). #298 adds C focus+size. #292 disclosed. MINERVA builds against v0.7-dev with no regression.

UNFINISHED — with acceptance gates:
- Front M "zero-to-working": minimal Rust/C/C++/Python/Go examples + headless/animated/resize examples +
  an agent-console template + a smoke-check script, each building from the INSTALLED SDK (stage-sdk +
  pkg-config name `libgibson`, not `gibson`), with expected output. Acceptance: each builds clean from a
  staged prefix; the smoke-check reports exactly what's missing.
- Wrapper parity (v0.8): mirror #298's gibson_get_terminal_size / gibson_set_focus_reporting and
  GIBSON_EVENT_FOCUS_GAINED=7 / FOCUS_LOST=8 into C++/Go/Python; add focus/size rows to the 76-record
  parity harness. Acceptance: SUPPORTED rows, installed-SDK consumers.
- #129 dirty-rectangle RGB ingest (the one missing instrument primitive). #134/#135/#136 controls/fields/
  probes (largest open front). #130 engine-loop wiring (ui::App::run still emits a wall-clock Tick).
- A publishable v0.7.0: requalify MINERVA against the merged main (full ProjectGHOST suite, A/B each
  failure vs v0.6.0 — one flake, d2b_second_crash, is already cleared), then run RELEASING.md §7
  (version bump PR, exact-head CI + preflight, native SDK + wheel from the tag, installed-artifact smoke,
  download-verify, GitHub Engineering Alpha prerelease). Linux x86_64 only; Windows stays UNVERIFIED (#292).

NEW BLOCKERS: none hard. The Renaissance XXI merge train is complete; the only standing HOLD is v0.7.0
itself (needs MINERVA full-suite requalification against the merged main + the release gate).

AUTHORIZATION: conditional Tier B merge + v0.7.0 Engineering-Alpha tag/release authority under the gates
(reproduced baseline, red-first regression, distinct-model review, exact-head CI, owner-condition record,
no unacknowledged support-contract regression; a failing gate = HOLD). NOT authorized: force-push, ABI
break, registry publish (crates.io/PyPI), frozen-audio change, touching ProjectGHOST's real repo/main or
enabling its disabled tools, GPL code in the repo, disclosing private consumer details in the public tracker.

FIRST COMMANDS:
  gh pr list --repo femboy2112/libgibson --state open     # expect: no Renaissance-XXI dev PRs
  git fetch origin && git rev-parse origin/main           # expect: de979ee + the #321 merge on top
  bash scripts/release/check-abi.sh                        # expect: 67/67 (rebuild first)
  # the 5-PR train is merged; start from the decisions below, not a merge.

RELEASE/MERGE DECISIONS PENDING: the 5 PRs are merged. Decide whether to open a v0.7.0 candidate
(needs MINERVA full-suite requalification against the merged main + the release gate) or continue
hardening (Front M, #129, #134/#135/#136, wrapper parity) first. Also: close zombie PRs #153–#167.

FAN OUT on independent lanes; keep the build tree coherent; merge what earns merging; HOLD anything whose
gate fails with the exact failing predicate. Deliver flash-tables + a fresh next-round prompt at the end.
```
