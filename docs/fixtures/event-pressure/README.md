# Readiness-batch witness

Historical mechanism evidence for [issue #15](https://github.com/femboy2112/libgibson/issues/15),
collected on 2026-09-24. These small records are not application replay files or a
production dependency patch.

`readiness-witness.json` preserves selected real syscall records and ten paired
untraced trial results. PIDs, epoch timestamps, and file-descriptor decorations
were removed from the syscall excerpts; source line numbers and original trace
SHA-256 hashes preserve provenance. The large scratch logs are intentionally not
repository assets. Parent/child application timing used monotonic clocks;
strace supplied an independent observation of kernel calls and token order.

The original standalone child used a 150 ms polling suspension after readiness
registration, allowing signal handlers to run. In the failing ordering, the
parent resized before writing a single `r`. There was no renderer, LibGibson
dependency, or second key during the three-second delivery window.

| Control | Observation |
|---|---|
| Original Crossterm, resize then key | 0/10 delivered; unread byte remained present |
| Scratch preserved-batch variant | 10/10 delivered Resize then exactly one `r` |
| Raw libc reader, syscall-instrumented | Received `r` |
| Original, reversed ordering, syscall-instrumented | Received `r`; stranded Resize |
| Separate later-key diagnostic | New readiness caused one `read(...,"rz",...)` |

Every child exited zero and restored the original PTY termios. The scratch patch
only retained unprocessed readiness tokens across calls. It did not modify
parsing, terminal mode, or the input sequence. All dependency versions matched
LibGibson's baseline lockfile: Crossterm 0.29.0, Mio 1.2.3, libc 0.2.189.
The Linux x86_64 host ran kernel 7.0.0-28-generic and Rust 1.98.1.

`scratch-batch-control.patch.txt` is the exact experimental intervention against
Crossterm 0.29's `src/event/source/unix/mio.rs`, with line endings normalized for
review. **It is not a production fix or vendored dependency.** It does not repair
the separate large-paste read-draining issue, establish fairness, or validate
other platforms. The upstream crate is MIT-licensed; its
[source and license](https://github.com/crossterm-rs/crossterm/tree/36d95b26a26e64b0f8c12edfe11f410a6d56a812)
remain authoritative.

## Repeat independently

The maintained reproducer uses a gate file instead of the historical fixed
suspension. It builds an isolated temporary Cargo project, has no LibGibson
dependency, and always sends exactly one key per child:

```sh
python3 scripts/dev/crossterm_resize_repro.py --repeat 2 --strace --out-dir /tmp/crossterm-evidence
```

It can be invoked by absolute path from any working directory. Python's standard
library, Linux PTYs, Cargo/Rust 1.98.1, and the pinned dependencies are required;
`strace` is optional. `--repeat` accepts 1–100. Every child has finite startup/run
deadlines; Cargo has a 180-second deadline. Child sessions are terminated/reaped
and the scratch build directory is removed. `--out-dir` retains the resolved
lockfile, bounded child logs, parent traces, and optional syscall traces.

Exit **1 is the expected reproduced upstream failure**, not a passing LibGibson
regression. Exit 0 means every trial delivered the one key; exit 2 means the
instrument/build/restoration failed. This intentionally failing probe is not a
standard CI test. The gate-based command above was run from `/tmp`: raw controls
passed 4/4, original Crossterm delivered 0/2 resize-first and 2/2 key-first; all
eight children restored termios and exited zero. The script exited 1.

This evidence isolates readiness consumption; it does not by itself establish
every graphical workload's behavior or close issue #15's integration gates.

## Visual playback receipts

`crossterm-gate.tsv` and `raw-gate.tsv` are real release-lab traces from the same
controlled signal-first scenario on this host. They preserve all semantic
receipts and only the first/last two repetitive poll/empty/queue samples per
category. Sequence numbers were renumbered after thinning; monotonic elapsed
microseconds and values are unchanged. No interpolated backend receipts were
added. These are separate measured trials, not a before/after production fix.

```sh
cargo run --release --example event_pressure_lab -- --replay-trace docs/fixtures/event-pressure/crossterm-gate.tsv
cargo run --release --example event_pressure_lab -- --replay-trace docs/fixtures/event-pressure/raw-gate.tsv
```

The first stops at an observed readable TTY; the second shows raw control delivery.
Backend-internal readiness stays unprobed in these application traces. Consult
the independent syscall witness for that layer. The viewer retains the recorded
mode/scenario/pause instead of accepting unrelated command-line labels.

## Repeated matrix

`pressure-matrix.csv` records 2,100 finite release trials at implementation commit
`a913dd5a36ed0a659bbd540d9f1d233b675a9e06` on the same Linux host. Five invocations
ran sequentially: silent/0 ms, graphics/0 ms, graphics/20 ms, graphics/60 ms,
graphics/150 ms. Each invoked all 3 controls × 7 scenarios × 20 repetitions with
`--deadline-ms 500`. Pause values affect only the slow-drain scenario; repeated
normal rows are not independent factorial interventions. No seed or RNG is used;
real OS scheduling is intentionally observed, not claimed deterministic.

All five matrix commands exited zero (measurement completion). The file preserves
**269 failed-delivery trials**; this is not a green acceptance matrix. All 2,100
children restored termios. Key percentiles use nearest rank and only completed
exact-sequence trials; `key_samples=0` has no latency estimate (numeric CSV zero
is a sentinel, not instantaneous delivery). The 500 ms deadline starts after the
nominal final operation; the intentional gate/pause contributes to latency.

Command template:

```sh
target/release/examples/event_pressure_lab --matrix --repeat 20 --workload graphics --pause-ms 60 --deadline-ms 500
```

GNU time observations (no CI thresholds):

| Workload / pause | Elapsed s | User s | System s | Maximum RSS KiB |
|---|---:|---:|---:|---:|
| Silent / 0 | 73.05 | 1.34 | 4.16 | 3460 |
| Graphical / 0 | 69.81 | 6.48 | 5.88 | 4828 |
| Graphical / 20 | 80.85 | 7.17 | 6.28 | 4780 |
| Graphical / 60 | 83.76 | 7.26 | 6.38 | 4760 |
| Graphical / 150 | 88.77 | 7.20 | 6.60 | 4784 |

These include the supervisor and child process work; RSS is the tool's maximum,
not simultaneous process-tree heap accounting. The reader discarded about
1.46 GB of terminal output across 15,114 committed frames instead of retaining
captures. The matrix does not establish long-session stability, a CPU SLA, or
cross-machine latency. Other bounded development activity occurred on the host.
