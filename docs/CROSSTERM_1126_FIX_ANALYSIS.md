# Crossterm #1126 input starvation — diagnosis and fix analysis

**Status: diagnosis CORROBORATED. Shape A fix IMPLEMENTED + independently VERIFIED
in an isolated upstream clone (staged, NOT filed upstream, NOT vendored into
LibGibson). Issue #15 remains OPEN — LibGibson still builds on unpatched
crossterm 0.29.0.** See [Round II](#round-ii--shape-a-implemented-and-verified-staged-not-filed).

This document is the staged outcome of an independent investigation into the
single-key input stall tracked in
[issue #15](https://github.com/femboy2112/libgibson/issues/15). It records what
was proven, why the *obvious* fix is wrong, and the fix shapes that could work —
so the fix is ready to reason about (or submit upstream) later, **without any
change to LibGibson's runtime, no vendored crossterm fork, and no
`[patch.crates-io]`**. It complements the [Event Pressure Lab](EVENT_PRESSURE_LAB.md),
which reproduces the failure; this document analyses the *fix*.

## Environment (the exact tested stack — not a universal claim)

crossterm `0.29.0` (default Mio event source), mio `1.2.3`, signal-hook-mio
`0.2.5`, Rust `1.98.1`, Linux `7.0.0-28-generic` x86_64, over a `/dev/pts` PTY.
crossterm `master` (`f6cb0751`) carries byte-identical event-loop logic, and
`0.29.0` is still the latest published release.

## The defect (CORROBORATED)

`UnixInternalEventSource::try_read` (`src/event/source/unix/mio.rs`) iterates one
Mio readiness batch and `return`s on the **first** decoded event, abandoning any
other ready token in the same batch. When a resize (`SIGWINCH`) and a keypress
arrive close enough to share one `poll()` batch, whichever token iterates first
wins and the other's readiness is discarded. Because the tty and signal fds are
registered edge-triggered (`EPOLLET`), the abandoned readiness never re-fires, so
the pending byte (or resize) is stranded until an *unrelated* later edge — e.g.
the next keypress — arrives.

Corroborated across four independent bearings:

1. **Source** — the early `return` in the `for token in self.events.iter()` loop
   (TTY and SIGNAL arms), with no retention of the batch remainder.
2. **Source** — mio registers the fds `EPOLLIN | EPOLLRDHUP | EPOLLET`.
3. **Runtime counts** — under a forced signal-first collision: raw libc
   poll/read delivered the key every trial; crossterm delivered **only** the
   `Resize`, the byte sat readable (`FIONREAD = 1`) for the full window, never
   read. The reverse order stranded the `Resize` symmetrically.
4. **strace** — `epoll_wait(...) = [{SIGNAL}, {TTY}]`, application receives
   `Resize`, then repeated `epoll_wait(...) = 0` with **no `read(stdin)`** until
   the deadline.

The one sub-claim left `UNVERIFIED` (untested, plausible): the "`rz`"
double-delivery on a subsequent rescue key.

## Why the obvious fix is WRONG (OBSERVED)

The intuitive fix — and the one attempted here — is: drain each ready fd to
`WouldBlock` and queue every decoded event, then return the first, so no batch
remainder is lost. **On this stack that hangs the event loop.**

crossterm's tty fd is **blocking**: `tty_fd()` returns fd 0 or opens `/dev/tty`
with no `O_NONBLOCK`, and raw mode uses `cfmakeraw` → `VMIN=1, VTIME=0`. mio's
`SourceFd` registers the fd for epoll but does **not** make it non-blocking. On a
blocking `VMIN=1` fd, `WouldBlock` and `Ok(0)` are unreachable: after the one
ready byte is consumed, the *next* read blocks waiting for `VMIN` more bytes.

strace of the drain-fix attempt, signal-first collision:

```text
epoll_wait(...) = [{SIGNAL}, {TTY}]
ioctl(/dev/tty, TIOCGWINSZ, 24x80)      # Resize computed (SIGNAL arm)
read(0, "r", 1024) = 1                   # the intended fix works: tty read after Resize
read(0, ..., 1024) = ? ERESTARTSYS       # the trailing drain read BLOCKS
<SIGTERM at +3.8s>                        # killed while blocked; no event ever returned
```

The pristine loop dodged this by returning immediately after the first parsed
event — one read per readiness edge; its inner re-read only fired to reassemble a
split escape sequence, where more bytes were genuinely coming.

**This matters beyond LibGibson:** "drain to `WouldBlock`" is exactly the approach
of upstream [PR #1057](https://github.com/crossterm-rs/crossterm/pull/1057), which
has remained open and unmerged with a reviewer requesting a real-PTY test harness.
This investigation supplies the missing evidence for *why*: on the blocking tty
fd, that approach hangs unless the fd is also made non-blocking.

## Correct fix shapes (PROPOSAL — not implemented here)

### Shape A — single-read discipline (fixes #1126, minimal, no fd change)

Process **every** ready token in the batch, but read the tty **exactly once** per
readiness (epoll already guaranteed data is present, so one read returns
immediately and cannot block), enqueue the `Resize` instead of early-returning,
then return the first queued event after the batch loop. Behaviourally equivalent
to the pristine tty path for the ≤1024-byte case (it does not fix the >1024-byte
`#1057` stall), removes the signal/tty cross-drop, and introduces no blocking
trailing read. Lowest risk; touches no fd flags.

### Shape B — non-blocking fd + drain (fixes #1126 **and** #1057, wider blast radius)

Set `O_NONBLOCK` on the tty fd (restored on drop), after which drain-to-`WouldBlock`
becomes sound and also fixes the >1024-byte partial-read stall (#1057). The cost:
`O_NONBLOCK` on the shared tty/stdin file description affects every other reader of
that fd in the process, so it needs a real correctness argument, restore-on-drop,
and interaction review with escape-sequence disambiguation. This is arguably a
change the crossterm maintainers should own.

## LibGibson's position

No crossterm fork is vendored, no `[patch.crates-io]` override is added, and the
input path is unchanged. **Issue #15 remains OPEN.** The upstream issue
[#1126](https://github.com/crossterm-rs/crossterm/issues/1126) exists but was filed
by this project's own account and has no third-party corroboration or maintainer
response yet; it is the team's own ticket, not community validation.

**Boundary:** everything above is for the exact Linux/pts stack named; macOS
(kqueue), Windows, a real hardware tty, and crossterm's async `event-stream` /
`WAKE_TOKEN` path were not exercised. The `read`-after-`Resize` line in the strace
is a success signal for the *intended* mechanism only — it coexists with the fatal
hang on the next line, and must not be read in isolation as "fixed."

## Round II — Shape A implemented and verified (staged, NOT filed)

Round II took Shape A from proposal to a **tested, independently verified** patch,
built entirely in an **isolated clone** of crossterm — never a LibGibson dependency,
never a vendored fork, never pushed anywhere.

- **Clone:** `crossterm-rs/crossterm` tag `0.29` = commit
  `36d95b26a26e64b0f8c12edfe11f410a6d56a812` (the latest published release), in a
  scratch worktree outside the LibGibson tree.
- **What changed:** only `src/event/source/unix/mio.rs` (34 insertions, 5 deletions),
  plus a scratch-only reproducer (`tests/repro_1126.rs` + two example fixtures) and
  the two `[dev-dependencies]` it needs. No fd blocking-mode change, no touched
  `WAKE_TOKEN`/`event-stream` path.

### The Shape A patch

Both the TTY and SIGNAL arms stop returning the instant they produce an event; they
enqueue into the parser's existing `internal_events` backlog and let the batch loop
finish, then one queued event is returned (any remainder is delivered on the next
call via the pre-existing top-of-function check — no extra `poll()`). The TTY inner
loop keeps its **identical** bounded read discipline (it detects a decode via a
length-diff on the backlog and `break`s — it does **not** drain to `WouldBlock`), so
it adds no read the pristine code didn't already do, and therefore cannot reintroduce
the blocking-fd hang that killed Shape B.

```diff
             for token in self.events.iter().map(|x| x.token()) {
                 match token {
                     TTY_TOKEN => {
+                        let queued_before = self.parser.internal_events.len();
                         loop {
                             match self.tty_fd.read(&mut self.tty_buffer) {
                                 Ok(read_count) => { /* advance parser as before */ }
                                 Err(e) => { /* WouldBlock -> break; Interrupted -> continue */ }
                             };
-                            if let Some(event) = self.parser.next() {
-                                return Ok(Some(event));
+                            // A full event decoded off this readiness: stop reading
+                            // (never a further drain) and move on to the rest of the batch.
+                            if self.parser.internal_events.len() > queued_before {
+                                break;
                             }
                         }
                     }
                     SIGNAL_TOKEN => {
                         if self.signals.pending().next() == Some(SIGWINCH) {
                             let new_size = crate::terminal::size()?;
-                            return Ok(Some(InternalEvent::Event(Event::Resize(new_size.0, new_size.1))));
+                            // Queue instead of early return — see TTY_TOKEN above.
+                            self.parser.internal_events.push_back(
+                                InternalEvent::Event(Event::Resize(new_size.0, new_size.1)));
                         }
                     }
                 }
             }
+            // Whole batch accounted for: hand back the oldest queued event, if any.
+            if let Some(event) = self.parser.next() {
+                return Ok(Some(event));
+            }
```

### The reproducer (the evidence #1057 lacked)

A real PTY, a real `SIGWINCH`, a real session leader. The child enables raw mode,
forces mio/epoll registration, then parks on a sentinel **without polling**; the
harness pre-arms *both* a resize and a keypress while the child is parked, so the
child's very first `epoll_wait` after release necessarily returns both tokens in one
batch — exactly the `epoll_wait(...) = [{SIGNAL}, {TTY}]` shape from the strace above.
Every wait is bounded, so a hang is a test *failure*, never an actual stall.

### Independently verified (re-run here, not taken on faith)

Reverting `mio.rs` to stock and re-running, then re-applying the patch:

| build | `resize_and_key` (collision) | controls (lone key, lone resize, zero-poll) | wall time |
|---|---|---|---|
| **stock 0.29.0** | **FAILED** — `got resize=true key=false` (key stranded) | pass | 2.69 s (bounded, no hang) |
| **Shape A** | **ok** — both delivered from one batch | pass | 2.27 s |

Zero-duration poll returned in ~103 µs (nowhere near a hang). Full crossterm suite
with Shape A: 105 unit + 53 doc-tests green, 7 pre-existing unrelated ignores.

### Scope, boundaries, and the honest bits

- Fixes the **#1126** batch-cross-drop. Does **not** fix **#1057** (the >1024-byte
  partial-drain stall) — that still needs Shape B (non-blocking fd). Not claimed.
- `WAKE_TOKEN` (`event-stream` feature) is deliberately untouched: feature-gated,
  unexercised by the rig, a materially larger change than Shape A's minimal footprint.
- The reproducer pulls `portable-pty` into the clone's dev-deps for safe
  session-leader setup; a real upstream submission might prefer a lighter PTY helper.
- Verified on the exact Linux/`pts` stack only; macOS (kqueue), Windows, and a real
  hardware tty are unexercised.

### Proposed upstream PR (NOT FILED — awaiting an explicit decision)

This is drafted and ready; it has **not** been posted to `crossterm-rs/crossterm`,
because filing on a third party's repository is an outward action reserved for an
explicit go-ahead. If filed, it would read:

> **Title:** Fix #1126: deliver every event in a single readiness batch (Unix Mio source)
>
> **Body:** On Unix, `UnixInternalEventSource::try_read` returns on the first decoded
> event in a `poll()` batch and abandons the other ready tokens. Because the tty and
> signal fds are edge-triggered, the dropped readiness never re-fires, so a keypress
> that shares one batch with a `SIGWINCH` is stranded until unrelated later I/O
> (issue #1126). This queues each ready token's event into the parser's existing
> backlog and returns after the whole batch, keeping the tty read discipline byte-for-
> byte (one read per readiness; no drain-to-`WouldBlock`), so it does **not** change any
> fd's blocking mode and cannot introduce the hang that a naive drain would on the
> blocking `VMIN=1` tty. Distinct from #1057 (>1024-byte partial drain), which this
> does not address. Includes a real-PTY reproducer that fails on `main` (resize
> delivered, key stranded) and passes with the fix; controls (lone key, lone resize,
> zero-duration poll) and the full existing suite stay green.

## LibGibson adoption status (issue #15)

**#15 stays OPEN.** An upstream patch existing — even a verified one — is not adoption.
LibGibson still depends on unpatched crossterm `0.29.0`; its input path is unchanged;
no fork is vendored and no `[patch.crates-io]` is added. #15 closes only when a
*supported* fixed crossterm (a release or an upstream-merged commit) exists, LibGibson
adopts it, the known-red single-key acceptance turns green, and the fairness/regression
matrix passes on that adopted path.
