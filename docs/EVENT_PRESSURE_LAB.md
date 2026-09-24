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
diff were retained in local scratch space. The independent [control script](../scripts/dev/crossterm_resize_repro.py) and
[small normalized witness](fixtures/event-pressure/README.md) preserve the portable
recipe and provenance; scratch paths are not installation dependencies.

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

[Crossterm issue #1126](https://github.com/crossterm-rs/crossterm/issues/1126)
contains the submitted independent report, source links and reproduction command. No dependency
fork, alternate backend, fake input, repeated synthetic SIGWINCH, unbounded
drain loop, or core scheduling modification is adopted by this evidence record.
The supported repair still needs correctness, zero-timeout compatibility,
bounded fairness, portability, and integration evidence.

## Running the oscilloscope

```sh
# Measured normal graphical traffic, repeatedly; M switches input control.
cargo run --release --example event_pressure_lab -- --auto

# Actual separate child PTY with a deliberately paused output reader.
cargo run --release --example event_pressure_lab -- --scenario=slow-drain --pause-ms=150 --auto

# Interleaved size changes/keys with graphical output.
cargo run --release --example event_pressure_lab -- --scenario=storm --auto

# Quiet causal control: the input fails without rendering.
cargo run --release --example event_pressure_lab -- --scenario=coincident --workload=silent --mode=crossterm

# Same scenario, raw reader control; this is NOT a patched production backend.
cargo run --release --example event_pressure_lab -- --scenario=coincident --workload=silent --mode=raw

# Real historical receipts, replayed at quarter speed; Space holds the projection.
cargo run --release --example event_pressure_lab -- --replay-trace docs/fixtures/event-pressure/crossterm-gate.tsv
cargo run --release --example event_pressure_lab -- --replay-trace docs/fixtures/event-pressure/raw-gate.tsv

# Deterministic inspection at a supplied trace time, in microseconds.
cargo run --release --example event_pressure_lab -- --replay-trace docs/fixtures/event-pressure/crossterm-gate.tsv --freeze-at=700000 --deterministic --dump --width=120 --height=32 --color=mono

# Independent upstream reproduction, no LibGibson dependency. Expected exit 1.
python3 scripts/dev/crossterm_resize_repro.py --repeat 2 --strace --out-dir /tmp/crossterm-evidence

# Explicit real acceptance: currently FAILS (exit 101), intentionally ignored normally.
cargo +1.98.1 test --test event_pressure_pty context_coincident_lone_key_delivery_acceptance -- --ignored --exact --nocapture
```

Live controls: `1..7` select normal, settled resize-key, key-resize, controlled
coincident, slow-drain, burst, or storm. `M` rotates raw/Crossterm/Context. `[`/`]`
change the next trial's drain pause and rerun; `R` reruns; `A` toggles repeat;
Space freezes only the visual projection while supervision continues. Esc/Ctrl-C
exits. Lowercase ordinary ASCII keys are forwarded to a still-running child and
are recorded as additional real inputs; they are never used as automated rescue
keys. Finished trials require rerun. Manual injection naturally changes the script.

The burst workload requests full graphical frames at up to 240 Hz, versus 60 Hz
for normal graphical trials; actual commits depend on rendering/output. No FPS
promise follows. Silent controls build no raster; standalone raw/Crossterm silent
children construct no Context. The viewer owns a different terminal from the
supervised child. The raw/Crossterm graphical controls reuse Context only for the
same output workload, never for their input path.

`--headless --trace FILE` records one real trial without the viewer and exits 2
on failed delivery. `--matrix --repeat 20 --workload graphics --pause-ms 60
--deadline-ms 500` runs all three modes and seven scenarios, outputting CSV;
its exit 0 means the measurement completed, **not that every trial delivered**.
Failure columns remain explicit. Live physical timings are never deterministic;
`--deterministic` is for trace/illustrative projection. `--illustrative --dump`
is deliberately marked ILLUSTRATIVE and is not a historical witness.

## Bounds and telemetry

- Supervisor drains at most 64 KiB per turn, using nonblocking reads on its own
  thread. It does not retain terminal output. There is no detached reader.
- Child receipt file: 65,536 records, 8 MiB reader cap; combined finite trial:
  65,536 records. Budget exhaustion is an instrument error, not dropped evidence.
  Visual history uses at most 512 entries, eight pulses, and 512 latency samples.
- Trial startup deadline 2 s; supervisor lifetime 7 s; child self-limit 8 s;
  kill/reap cleanup bounded at 2 s. No arbitrary-descendant supervision claim.
- Input delivery deadline is configurable 100–3000 ms after the nominal final
  script operation. Tests use a generous 700 ms; recorded matrix uses 500 ms.
  A 60 ms duplicate/cleanup observation window cannot convert a missed deadline
  into success. Late, invalid, duplicate and reordered receipts fail.
- PTY write timestamp is captured **before the successful write syscall**. Child
  receipt can otherwise precede the parent's return. Latency includes intentional
  gate/drain delays and diagnostic overhead. Samples are conditional on complete
  exact-sequence delivery, not a distribution over missing/censored keys.
- Committed frame bytes exclude in-flight blocked writes and lifecycle controls.
  Committed minus drained is only an accounting estimate, not kernel queue depth;
  an unmatched frame begin explicitly says in-flight bytes are unmeasured.
- Backend readiness is unprobed in normal application traces. It remains `?`;
  the separately retained syscall witness establishes that layer. Context receipts
  mean run_once returned, not that an internal decode hook was observed.
- Monochrome changes presentation only. Frozen frames have zero exact delta,
  affected footprint and wire bytes; real viewer freeze/resize/restore is tested.

## Validation and repetition

The final local suite passes **591 tests: 226 unit + 365 integration**, plus
one deliberately ignored upstream delivery acceptance. Repetition results follow.
No input-draining fix was introduced, so no new 10,000-event fairness guarantee
is claimed. Such a test belongs to the eventual supported backend repair. This
finite lab does not close #10 or #11.

### Release repetition results

[Machine-readable matrix](fixtures/event-pressure/pressure-matrix.csv), with
[commands/resource provenance](fixtures/event-pressure/README.md#repeated-matrix):

| Input path | Trials | Exact delivery | Readable but undelivered | Other failures | Restored |
|---|---:|---:|---:|---:|---:|
| Raw poll/read | 700 | 700 | 0 | 0 | 700 |
| Crossterm poll/read | 700 | 539 | 161 | 0 | 700 |
| Context run_once | 700 | 592 | 108 | 0 | 700 |

All controlled signal-first collision trials failed through Crossterm and Context
(100/100 per path across silent and graphical configurations); raw delivered
100/100. Normal and settled resize-key controls delivered in every tested cell. One of
the 100 graphical Crossterm storm trials also stranded input (60 ms configuration
row, which does not itself pause during storm). This is not a
universal assertion that any overlap must strand a key.

The graphical slow-drain subset, **20 trials per cell**:

| Path | 0 ms | 20 ms | 60 ms | 150 ms |
|---|---:|---:|---:|---:|
| Raw | 20 delivered | 20 delivered | 20 delivered | 20 delivered |
| Crossterm | 20 delivered | 20 stalled | 20 stalled | 20 stalled |
| Context | 20 delivered | 20 delivered | 20 delivered | 20 delivered |

Context's final slow-drain cells passed even though an earlier graphical witness
stalled after draining resumed. Source ordering and render timing change which
ready token is consumed first; this variation is why the separate quiet barrier
and actual syscall witness matter. In the silent zero-pause slow-drain script,
Context stalled 8/20: adjacent resize/key operations can also coincide without
output. Do not cherry-pick a passing workload as a backend repair.

Representative successful-sequence latency, PTY write start → APP receipt:

| Path/scenario | Samples | p50 | p95 | max |
|---|---:|---:|---:|---:|
| Raw, graphical slow drain 150 ms | 20 | 78.422 ms | 79.470 ms | 80.444 ms |
| Context, graphical slow drain 150 ms | 20 | 76.579 ms | 77.926 ms | 78.220 ms |
| Crossterm, graphical slow drain 150 ms | 0 | censored | censored | deadline failures |

Keys are injected partway through the pause, so their delay need not equal its
full duration. The remaining per-cell percentiles and sample counts are in the
CSV; they are conditional diagnostics, not an SLA. Across all configurations,
3,629 keys belong to successful exact-sequence trials.

The matrix used bounded logging and discarded 1.46 GB of output across 15,114
frames. GNU time maximum RSS was 3.4–4.8 MiB; aggregate user+system time was
5.50 s over 73.05 s for the silent matrix and 12.36–13.80 s over 69.81–88.77 s for
graphical matrices. These are finite process measurements, not proof against all
busy-loop/resource failures or a resolution of #10.

### Final local gates

All of these exited **0** on Rust 1.98.1:

```sh
cargo +1.98.1 fmt --check
cargo +1.98.1 clippy --all-targets --all-features -- -D warnings
cargo +1.98.1 test
cargo +1.98.1 build --release
cargo +1.98.1 build --examples
cargo +1.98.1 build --release --examples
RUSTDOCFLAGS="-D warnings" cargo +1.98.1 doc --no-deps
cargo +1.98.1 test --example fx_lab
RUSTUP_TOOLCHAIN=1.98.1 bash scripts/dev/bindings_smoke.sh --asan
git diff --check
cargo +1.98.1 test --test event_pressure_pty --test event_pressure_trace --test event_pressure_visual --test pty_integration --test pty_resize_torture --test intro_pty --test resize_torture -- --test-threads=1
cargo +1.98.1 test --test pty_demos acid_ -- --test-threads=1
```

Full suite: **591 passed = 226 unit + 365 integration**, with **one explicitly
ignored known-failing delivery acceptance**. FX Lab separately passed three
example tests. Bindings smoke exercised C/C++/Python/Go and native ASan/UBSan.
The standard suite took 77.5 s on this host. The dedicated serial PTY/resize/intro
selection took 26.8 s; Acid PTYs 18.5 s. No timing is a CI assertion.

The ignored acceptance was also run explicitly and **failed, exit 101**, as it
must on the unresolved backend: exactly one `r` sent, zero application keys,
positive readable-byte receipts through deadline, restored termios and exit 0
from the supervised child. The standalone upstream Python probe independently
exits **1** for the same delivery failure. Neither result is reclassified green.

Diagnostic regressions additionally reject duplicate/reordered/malformed/late
receipts and inverted timestamps; replay cannot import a future verdict or hide
a nonzero child exit/restoration failure. The Mono viewer test checks input,
frozen zero wire, 56↔160↔120 reflow and restoration. Separate release PTYs at
56×24 Mono, 120×32 TrueColor and 160×40 TrueColor exited/restored with Esc or
Ctrl-C. Agent inspection of reconstructed measured frames covered those sizes
and Mono. This is agent visual inspection, not independent human UX acceptance.

First lab commit `a913dd5` passed all five public jobs in
[run 35968176421](https://github.com/femboy2112/libgibson/actions/runs/35968176421).
[PR #18](https://github.com/femboy2112/libgibson/pull/18) records final exact-tip
checks separately; the earlier run must not be substituted for a later SHA.
The existing PTY job now also invokes `event_pressure_pty`; the long repetition
matrix stays an explicit local command rather than multiplying normal CI time.

**Verdict:** runtime failure **reproduced**; earliest responsible layer
**isolated upstream**; batch-loss causal mechanism **CORROBORATED** by source,
syscalls and a narrow intervention. Production repair **NOT IMPLEMENTED**.
Issue #15 remains **OPEN**. Core code/dependencies/bindings/cinematic content are
unchanged. No merge is performed by this investigation.
