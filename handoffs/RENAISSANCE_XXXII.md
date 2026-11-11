# Renaissance XXXII — one terminal, two pipes: the input half

This is a checkpoint, not a substitute for fetching live refs. Baseline main was
`823d606b81b820c9b0f8374861f13ee16304a98e` (push CI 43082323084 success).
The latest published Engineering Alpha remains v0.6.0 at
`492d0fec7727658d50b9a82025c784b7e427d6b0`. Source version 0.7.0-dev, C ABI v1
(unchanged this round), MSRV 1.85, CI toolchain 1.98.1. v0.7.0 remains HOLD.

#349 (head `bdc0f1032379262e606455dd76329dd80676ac34`) merged normally with an
exact-head match after CI 43111972760 (8/8 jobs) and publication-free Release
Preflight 43111972876 passed on that head. Main is now
`b3428dbd9793e0e530b4ca12e8e6c22ca68ebd58`; its push CI 43113961200 was in progress
at this checkpoint — fetch its result before relying on it. #345 closed via
`Fixes`; #344 closed as superseded (its test is on main unchanged). (An earlier
CI/preflight pair on the same head stuck in `queued` with no jobs for 1h42m and
refused cancel/rerun; a PR close/reopen produced the runs above.)

## What was fixed: #345 (raw restore hit the replacement terminal)

Mechanism (source-verified): crossterm 0.29.0 `disable_raw_mode`
(`src/terminal/sys/unix.rs:148-157`) keeps one process-global saved termios and
re-resolves `tty_fd()` (stdin if a tty, else `/dev/tty`) on every call. After
`dup2(B, 0)` it applied A's baseline to B and returned `Ok` while A stayed raw.

Repair (#349): a private `src/input_endpoint.rs` lease half. The first
`enter_interactive` acquires a close-on-exec descriptor for the terminal (dup of
stdin when it is a tty, else its own open of `/dev/tty`); raw entry, restore, the
panic hook, the input source and the parser's raw-state question all bind to that
descriptor. crossterm is still used for entry/restore when its target is provably
the acquired terminal (`TIOCGDEV` on Linux), so hosts that also call crossterm's
raw API (e.g. `examples/event_pressure_lab`) stay correct. Acquisition and restore
bump the input generation; a poll already blocked on a retired source returns
`Ok(None)` and its bytes are discarded. Host crossterm raw + replaced stdin is a
safe refusal: `restore()` returns `Err` and touches neither terminal.

The full decision record, per-case SUPPORTED / SAFE REFUSAL / EXPLICITLY EXCLUDED /
UNVERIFIED table, crossterm residuals 1–5 and mutation table live in
`docs/TERMINAL_LEASE_CONTRACT.md`.

Evidence (local, rustc 1.98.1, Linux):

- #344's unchanged oracle `raw_input_retarget_pty`: red on 823d606
  (`A_RESTORED=false B_UNCHANGED=false`), green on the fix.
- `tests/acquired_input_endpoint_pty.rs`: 14/14, each case in its own child; termios
  compared on iflag/oflag/cflag/lflag, line discipline, c_cc and speeds. Blocked-poll
  case uses `/proc/self/task/<tid>/syscall` syscall-entry evidence, no sleep races.
- Mutations: restore-on-stdin kills 6; blind crossterm delegate kills 6; pure bypass
  kills 1 (the crossterm-host case); source/generation mutations each killed. The
  parser-ignores-lease mutant survives as an equivalent mutant on Linux (documented).
- Full `cargo +1.98.1 test --locked --no-fail-fast`: 2742 passed, 0 failed, 19 ignored.
  Clippy `-D warnings` clean; `cargo +1.85 check` clean.
- Independent adversarial review (source-only, same model family): 4 findings fixed,
  4 documented as residuals or an owner decision point (contract §residuals).

Not separately tested: poll blocked under an old generation at *restore* time (same
code path as the acquisition case); non-Linux Unix `st_rdev` identity; Windows.

## What was not done (still HOLD)

| Lane | State | Note |
|---|---|---|
| #345 raw restore target | MERGED | #349 |
| #344 reproducer | CLOSED (superseded) | test carried unchanged into #349 |
| Resize geometry from the output endpoint | BLOCKED | #348 red `resize_event_matches_the_acquired_output_geometry`; `size()` falls back to stdout without ctty |
| Renderer stack #338 → #339 → #343 | IN REVIEW / red | #339 ordinary acceptance still red; do not merge a red parent |
| Output endpoint identity (writes, restore bytes) | UNVERIFIED | main writes to process stdout at each write |
| MINERVA substitutions + canary | WIP (read-only audit only) | ProjectGHOST `6ed77a1` pins v0.6.0; its early poll is `ctx.poll_event` (managed), so retire-on-acquire does not drop its type-ahead. No downstream patch built |
| Streaming viewport primitive / protected regions | not started | #346 stays example-level |
| v0.7 release | HOLD | lifecycle output half, geometry, canary, GPL adapter node J, SDK parity |
| Math Kitty image element (#334) | not started | M0 #336 only |

## Decisive probes for the next round

1. **Geometry source.** Rivals: (a) resize events read `size()` lazily from whatever
   stdout is; (b) SIGWINCH is delivered for the ctty only. Test: #348's
   `resize_event_matches_the_acquired_output_geometry` with A 40×15, B 120×40,
   resize A to 50×20, no ctty and with ctty. If (a), bind `TIOCGWINSZ` to the
   acquired output descriptor; if (b), the no-ctty case needs a polled geometry check.
2. **Output acquisition reusing the input pattern.** Does dup-at-acquire of stdout plus
   a shared generation make #343's lease-output tests green without #339's API
   redesign? Run #343's tests on a branch that ports only the descriptor capture.
3. **#339 red cause.** Contractual change or regression? Diff its inline/history output
   against the immutable 77-byte golden on main before touching the golden.
4. **Restore-time in-flight poll.** Add the restore-side twin of the blocked-poll test;
   expected `Ok(None)` and no delivery to the next owner.
5. **MINERVA canary.** Build ProjectGHOST in a disposable copy against main with
   substitutions A (`Context::run_once`) and B (`enter_interactive`) separately;
   idle-CPU witness, one Enter, no echo, clean restore at 40×15/80×24/120×40.

## Bootstrap

```sh
git status --short --branch
git worktree list --porcelain
git fetch origin
git rev-parse origin/main
gh pr list --repo femboy2112/libgibson --state open --limit 100
cargo +1.98.1 test --locked --test acquired_input_endpoint_pty -- --test-threads=1
cargo +1.98.1 test --locked --test raw_input_retarget_pty -- --test-threads=1
```

Boundaries unchanged: preserve tags, ABI v1, Rust 1.85, frozen audio; no force-push,
protection bypass, registry publication or remote-branch deletion of unreconciled
work; never modify ProjectGHOST main or publish its source. `gh pr edit` is broken —
use `gh api -X PATCH`. At most two heavy Cargo builds at once on the 7 GB host.
