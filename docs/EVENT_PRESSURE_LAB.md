# Event Pressure Lab

Investigation of [issue #15](https://github.com/femboy2112/libgibson/issues/15),
started on 2026-09-24 from main
`0f4a3dfe153ec1ae95bc96834779f6ed0a22b8e3`, on
`codex/event-pressure-lab`.

**Current finding: reproduced and isolated upstream; no production fix is
claimed.** A quiet standalone Crossterm program can strand one already-readable
key after reporting a resize. A renderer, LibGibson Context, Acid-vs-Crash,
terminal emulator, and vt100 reconstruction are unnecessary for that failure.
The controlled syscall trace and a narrow scratch intervention corroborate loss
of the remainder of a readiness batch in Crossterm's Unix Mio source. Issue #15
remains open until a supported fix or workaround is merged and its delivery
regression passes. Issues #10 and #11 remain separate.

This is diagnostic telemetry, not a throughput benchmark or a universal terminal
latency guarantee. The live visualizer and replay are projections of receipts;
they must leave unobserved checkpoints dark rather than invent backend events.

## Baseline and environment

| Item | Observed value |
|---|---|
| Main before changes | `0f4a3dfe153ec1ae95bc96834779f6ed0a22b8e3` |
| Rust | `rustc 1.98.1 (48a229cea 2026-09-01)` |
| Cargo | `cargo 1.98.1 (797e8a9bc 2026-08-05)` |
| Linux | `7.0.0-28-generic`, x86_64 |
| Kernel build | `#28~24.04.1-Ubuntu SMP PREEMPT_DYNAMIC` |
| Crossterm | `0.29.0`, default Mio event source |
| Mio | `1.2.3` |
| signal-hook-mio | `0.2.5` |
| portable-pty | `0.9.0` |
| vt100 | `0.16.2` |
| Baseline suite | **574 passed: 226 unit + 348 integration** |

All five pre-edit commands exited zero:

```sh
cargo +1.98.1 fmt --check
cargo +1.98.1 clippy --all-targets --all-features -- -D warnings
cargo +1.98.1 test
cargo +1.98.1 build --release
cargo +1.98.1 build --examples
```

The baseline worktree was clean. The full test run took 97.8 seconds and the
release build 18.5 seconds on this host; these are observations, not thresholds.
The pre-investigation exact-main public run was
[35965119408](https://github.com/femboy2112/libgibson/actions/runs/35965119408).
Final branch validation and exact-tip CI are recorded separately below.

## Question and competing explanations

The historical graphical PTY sequence combined a resize, a paused output reader,
and a lowercase key. Sometimes a later diagnostic key released the earlier one.
That behavior suggested queued delivery, but did not locate the responsible
layer.

| Candidate | Discriminating observation |
|---|---|
| Parent failed to write input | Check successful PTY-master write, then child queue/read observations. |
| Kernel/PTY did not make input readable | Compare raw libc poll/read with the same resize/key sequence. |
| Backend lost readiness | Observe kernel readiness batch and subsequent read syscalls; compare token order. |
| Crossterm parser failed | Distinguish missing stdin read from a read whose byte fails to decode. |
| LibGibson scheduling/dispatch failed | Compare standalone Crossterm with Context, without graphical output. |
| Output write delayed application receipt | Observe frame begin/commit and drain state separately from input decode. |
| Harness/display reconstruction hid delivery | Record application receipts through a separate diagnostic channel. |

Repeated runs of the same harness are repetitions, not independent evidence
families. Raw reading changes the readiness/decoding implementation; strace adds a
separate syscall observation; reversing order changes the readiness batch;
retaining that batch changes the proposed mechanism. They share the same host,
PTY setup, and input script. No cross-platform generality follows from them.

## Smallest reproduced failure

The initial standalone child enables raw mode and initializes Crossterm's event
source with `event::poll(Duration::ZERO)`. It records readiness through an
independent file, then suspends polling for 150 ms while signal handlers remain
active. The parent waits for that readiness receipt, waits 20 ms, performs
`TIOCSWINSZ(80×24)`, waits 5 ms, and writes exactly one ASCII `r`.

The child resumes polling with a 20 ms timeout for a three-second observation
window. It samples `FIONREAD` without consuming bytes, then records decoded
events. The parent sends **nothing else**, waits no more than five seconds,
reaps its child, and checks original termios restoration. Startup logs have
unique names to prevent an old readiness receipt from passing a new trial.

The raw control uses libc `poll`/`read` instead of Crossterm events, while keeping
the same Crossterm raw-mode setup. The reverse-order control writes `r` before
resizing. The polling suspension is a deliberate coincidence barrier, not a
claim about normal application timing. It exposes the failure without output
pressure; a blocked graphical write can create an analogous polling gap.

### Results from initial isolation

| Variant | Trials | Actual result |
|---|---:|---|
| Original Crossterm, resize then key, no second key | 10 untraced | **0/10** delivered `r` within three seconds; all delivered Resize and retained one readable byte. |
| Scratch variant preserving remaining readiness tokens | 10 untraced | **10/10** delivered Resize then exactly one `r`; queue occupancy returned to zero. |
| Original Crossterm under strace | 1 | Signal-first batch contained both tokens; no stdin read followed; key remained queued. |
| Raw libc poll/read under strace | 1 | `read(0,"r",128)=1`; exact key delivered. |
| Original Crossterm, key then resize, under strace | 1 | TTY-first batch delivered `r` but stranded Resize during the window. |
| Original Crossterm with a deliberately later `z` | 1 | A fresh readiness edge caused `read(0,"rz",1024)=2`; both keys appeared. **Diagnostic only, not acceptance.** |

All these children exited zero and restored original PTY termios. Baseline and
scratch-intervention repeat parents ran concurrently in pairs, so the timing is
not a controlled hardware benchmark. The scratch variant's post-barrier key
receipt had median 211.5 μs and nearest-rank p95/max 266 μs across ten trials.
Those values exclude the intentional polling suspension and are **not**
PTY-write-to-application latencies.

The initial evidence, small parent/child sources, syscall logs, and intervention
diff were retained in local scratch space. Final portable commands, retained
fixtures, and broader repetition results are pending integration into the lab;
the scratch paths are not installation dependencies.

## Earliest failing layer and source path

The normalized original syscall witness is:

```text
ioctl(stdin, FIONREAD, [1]) = 0
epoll_wait(..., [SIGNAL_TOKEN=1, TTY_TOKEN=0], ...) = 2
    application receives Resize(80,24)
ioctl(stdin, FIONREAD, [1]) = 0
epoll_wait(..., [], ..., 20ms) = 0
    repeats until deadline; no read(stdin, ...) occurs
```

Kernel readiness was delivered. The failure begins in **Crossterm's consumption
of an already-returned readiness batch**, before reading/decoding the input
byte. The syscall trace does not support blaming Mio for failing to notify
Crossterm in that observation.

In Crossterm 0.29.0,
[`src/event/source/unix/mio.rs`](https://github.com/crossterm-rs/crossterm/blob/36d95b26a26e64b0f8c12edfe11f410a6d56a812/src/event/source/unix/mio.rs#L75-L146)
iterates a batch of ready tokens. Handling SIGWINCH immediately returns a Resize
event. Handling TTY can also return on the first decoded event. No saved cursor
or queue preserves later tokens in that batch. Mio's Linux selector clears the
events buffer before its next `epoll_wait`, and the TTY is registered with
`EPOLLET`. Already-readable input need not create another readiness edge.

The narrow scratch intervention retained unhandled tokens and asked the kernel
for another batch only after exhausting that queue. The same signal-first batch
then produced Resize followed by `read(stdin,"r",1024)=1` and Key('r'). This is a
causal discriminator, **not a shipped or reviewed backend patch**. It does not
establish complete handling of large reads, EOF/errors, continuous signals, or
render fairness.

**OBSERVED:** both readiness notifications arrive, the byte remains readable,
and Crossterm makes no stdin read after its Resize return. **CORROBORATED:** loss
of the remainder of the readiness batch explains the isolated stall. This
conclusion combines source control flow, syscall ordering, the raw control,
reversed ordering, and the narrow intervention. It does not prove that every
historical delayed event has this cause.

## A separate output-pressure delay

[`Context::run_once`](../src/context.rs) polls for an event and then performs any
due synchronous render before returning that event to the application.
[`input::poll_event`](../src/input.rs) maps one Crossterm event; the renderer
writes and flushes through ordinary blocking output. Consequently an event can
already be decoded while application receipt waits for a blocked frame write.

That is a distinct source-level scheduling prediction. A trace must separate
decode, Context return, application receipt, frame commit, and parent drain
instead of labeling every late application event “lost readiness.” If output
resumes and that event arrives without new input, it differs from the quiet
backend stall above. No input-priority or bounded-draining core change is
justified merely by observing a blocked synchronous writer.

## Measurement contracts

- Event sequence and elapsed timestamps use monotonic time. Parent/child clocks
  require an explicit shared epoch or alignment; independent `Instant` origins
  cannot be subtracted as if they were one clock. strace timestamps are
  supplementary syscall-order evidence, not the latency clock.
- Successful master writes identify the bytes sent. `FIONREAD` measures queued
  byte count; it does **not** identify a particular key or prove parsing. A visual
  key pulse must not be advanced through TTY or backend stages solely from that
  occupancy sample.
- Unsupported backend checkpoints remain visibly unobserved. A later app event
  does not retroactively manufacture an internal poll/read receipt.
- Latency summaries name their endpoints and sample count. Percentiles over
  successful deliveries omit censored failures, so report failure counts beside
  them. A deadline exceedance is not a very large measured latency.
- Output generated, output committed, and output drained are distinct counters.
  A backlog estimate must state which counters it subtracts; in-progress blocked
  writes can make frame-commit-only estimates incomplete.
- Rendering a trace is pure: visual time is supplied, bounded recent history is
  retained, and paint cannot consume input or advance a measurement clock.
- The acceptance path sends one key and no rescue key. A later-key release demo
  is a separately labeled diagnostic scenario.

## Upstream status and repair boundary

Upstream review on 2026-09-24 found related open work:

- [Crossterm PR #1057](https://github.com/crossterm-rs/crossterm/pull/1057)
  addresses incomplete draining of the TTY after a decoded event, including
  large-paste stalls. Its current change does not retain an unprocessed TTY
  token after a signal-first return. The one-byte batch failure is distinct.
- [Issue #839](https://github.com/crossterm-rs/crossterm/issues/839),
  [PR #840](https://github.com/crossterm-rs/crossterm/pull/840), and
  [PR #1047](https://github.com/crossterm-rs/crossterm/pull/1047) concern
  zero-timeout polling in the alternate `use-dev-tty` backend. Selecting that
  level-triggered backend alone is not established as a safe replacement for
  callers that use `Duration::ZERO`.

An upstream report has been prepared; submission/link is pending. No dependency
fork, alternate backend, fake input, repeated synthetic SIGWINCH, unbounded
drain loop, or core scheduling modification is adopted by this evidence record.
The supported repair still needs correctness, zero-timeout compatibility,
bounded fairness, portability, and integration evidence.

## Lab integration and final gates

**Pending finalization:** exact executable controls and replay commands; committed
small failure fixture; raw/Crossterm/Context scenario matrix; repeated graphical
pressure counts and end-to-end percentiles; final local suite count; exact-tip
public CI; issue #15 update/upstream report link. These are deliberately not
reported as passing while implementation continues.

The required scenario protocol includes normal `ABC`, resize-then-key,
key-then-resize, a controlled coincident batch, silent application, paused output
drain at 20/60/150 ms, bounded burst output, and resize storm. Finite trials must
assert exact key order and multiplicity, retain bounded failure traces, terminate
and reap every child, and verify terminal restoration. No standard acceptance
trial may inject a second key to release the first.

No input-draining fix has been introduced, so no new 10,000-event fairness
guarantee is claimed. The lab's retention/resource measurements may inform #10,
and restoration observations may inform #11; neither issue is resolved by a
finite PTY diagnostic campaign.
