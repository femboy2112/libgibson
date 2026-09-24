# Crossterm #1126 input starvation — diagnosis and fix analysis

**Status: diagnosis CORROBORATED; no production fix implemented in LibGibson.**

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
