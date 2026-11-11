# Resize endpoint research — XXXII

This is a deliberately incomplete, non-mergeable experiment on top of the
#338 → #339 → #343 output stack. It is not a compatibility or release verdict.
The parent research commit is `f65f31291ed70c1465616535ab5807fe2f60d95e`.

On Linux, a managed session acquired PTY A at 40×15, then stdout was redirected
to independent PTY B at 120×40. After A changed to 50×20, the parent experiment's
getter returned 50×20 while its resize event returned 120×40. A controlling-A
control returned 50×20 for both. The test explicitly verifies that the other
case has no controlling terminal.

This experiment binds the resize reader to the managed output endpoint for each
Unix input poll, without resetting the pre-existing input source. Four cases pass:
controlling A, no controlling terminal, redirect before first poll, and a free
poll that created the source before session acquisition. The latter case failed
when the reader was bound only at source creation. A weak endpoint reference does
not itself keep a relinquished session open; it also does not prove all in-flight
operations are retired.

On this research tree, `cargo +1.98.1 test --locked --test
endpoint_geometry_pty resize_event_matches_the_acquired_output_geometry --
--exact --nocapture --test-threads=1` passed all four cases. Mutating away the
per-poll reader assignment made the no-controlling-terminal case fail again
(exit 101). The Unix input-source unit selection passed 59 tests, and five
focused renderer, endpoint, sovereignty and ANSI test targets passed 47 test
entries including child helpers. These tests do not qualify the Rust API or the
full release candidate.

A fifth, deliberately stronger control starts a free poll before the managed
session and observes the worker blocked in the Linux `poll` syscall before the
session is acquired. After stdout is redirected, that older unmanaged poll still
returns B's 120×40 resize while the managed getter reports A's 50×20. Its proposed
acceptance asks for A's geometry. Whether the supported contract should instead
cancel or reject that earlier unmanaged poll needs an explicit decision. The
control remains ordinary and failing; do not describe it as an established
managed-session guarantee. A source generation reset does not cancel a poll
already in progress.

The raw-input control also remains ordinary and failing: after acquiring raw on
A, replacing stdin with B causes restore to leave A raw and modify B. This is a
hostile descriptor replacement, not evidence that fixed-descriptor restoration
is broken.

On main `823d606b81b820c9b0f8374861f13ee16304a98e`, a separate no-retarget PTY
probe enabled Crossterm raw mode in the host before creating a session. With no
session, both the Crossterm flag and physical raw settings stayed on. With a
session, its normal restore turned both off. The fixture cleans up its own PTY.
The current restore documentation says it disables raw mode, so host-owned
Crossterm raw preservation is a proposed integration requirement, not a claim
that an explicitly documented existing guarantee was violated. CR/LF/Ctrl+J
and host raw mode acquired through other APIs remain to be tested.

A sound successor must address source acquisition, pre-existing queued events,
in-flight poll cancellation or ownership, raw-mode setup and restore, owner
panic, handoff, hangup, CR/LF/Ctrl+J decoding, error paths and FFI together.
The per-poll geometry reader is only a diagnostic candidate. Its process-global
lease observation is not an atomic ownership transfer; do not promote it as a
complete solution. It also allocates a boxed reader on every Unix poll; idle and
high-rate input costs have not been measured. Keep the original renderer A/B/A and immutable ANSI golden
controls, inline/history behavior, old installed clients and Rust 1.85 gates.

## Input selection timing — XXXIII follow-up

An additional Linux PTY probe uses distinct, independently raw terminals A and B,
with no controlling terminal. It enters managed raw input on A, then tests stdin
replacement with B before or after the first poll. Different markers are sent to
both PTY masters. In the unchanged-stdin control and when the source is opened
before replacement, the poll reads A's `x`. When stdin is replaced before source
creation, it reads B's `y`. In a separate queued-input case, `FIONREAD` confirms
four bytes on A before retargeting and all four still pending after the poll has
returned B's events. The fixture restores stdin to A before session cleanup and
restores B's separate termios, so this isolates input selection from #344's raw
restore defect. The same two-input-control result and the A-queue observation were reproduced
using the fixture against main `823d606b81b820c9b0f8374861f13ee16304a98e`.
This adversarial descriptor replacement does not establish a failure for
applications that keep stdin stable.

The same stream checks CR as Enter, and two LF bytes as two Ctrl+J events, with
no extra event observed by a zero-timeout poll after the stream. At this Unix
byte interface LF and Ctrl+J are the same byte; the probe does not claim to
recover physical keystroke provenance. The branch retains both requested
A-identity controls as ordinary failing tests. Pinning the descriptor at
acquisition could address this timing difference, but needs coordinated review
of unmanaged in-flight polls, queued input, host raw ownership, panic recovery,
hangup, raw restoration, FFI and failure behavior before it is a fix.

Commands (each failing acceptance is intentional, exit 101):

```sh
cargo +1.98.1 test --locked --test endpoint_geometry_pty input -- --nocapture --test-threads=1
cargo +1.98.1 test --locked --test endpoint_geometry_pty queued_input_survives_stdin_retarget_before_first_poll -- --exact --nocapture --test-threads=1
```
