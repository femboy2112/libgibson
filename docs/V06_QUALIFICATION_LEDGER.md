# v0.6 Qualification Ledger (Renaissance XIV — final causal audit)

The release-qualification record for the v0.6 integration candidate. It records, per gate, the
defect an independent audit named, the **red** evidence executed against the unmodified code, the
repair, the **green** evidence, the mutation controls that prove each test has teeth, and what
remains **limited** or **unproven**. A green CI run is not treated as qualification while a known
load-bearing counterexample survives outside its tests; every counterexample below was executed,
not argued.

Status vocabulary: **DISCLOSED** (stated, not independently checked) · **OBSERVED** (one clean
measurement on the real path) · **CORROBORATED** (two independent routes agree) · **CONJECTURED** ·
**UNVERIFIED** · **REFUTED** (a counterexample was executed).

- Starting point: `main` = `2ffdd92`; development head `c55d277` (8/8 CI run 41972784603;
  Release Preflight 41974038659 green). Stack `#241 ← … ← #249 ← #250 ← #251`, all draft.
- Branch: `feat/v0.6-RXIV-causal-audit-191` (linear on top of `c55d277`).
- Toolchains: tests on the local default (`rustc 1.97.0-nightly 2026-04-14`), `fmt`/`clippy`
  pinned `+1.98.1` (the CI pin); CI runs `ubuntu-24.04` + `1.98.1` for the Rust job.

## Gate A — the cinematic causal false positive · commit `8e29c22`

| | |
|---|---|
| Defect | TRANSFORM was gated on `observed_peak >= 0.8·target`: a threshold on the target alone. |
| RED (untouched arc, `c55d277`) | 3/3: baseline 2.0 / target 2.0 / ACK / observed 2.0 → TRANSFORM; baseline 2.0 / target 0.5 / observed 2.0 → TRANSFORM; **real producer** no-op `set-amplitude 1.0` (peak 0.99941 → 0.99863, Acknowledged) → TRANSFORM. **REFUTED.** |
| Repair | Typed `EffectContract` / `Evidence`: ACK, a newer same-epoch observation, a change beyond a declared tolerance, and consistency with the request are separate; TRANSFORM only on `Consistent`. Unevidenceable requests (`Indiscriminable`, `Uncalibrated`, `NonStationary`, `UnmeasurableBaseline`) are classified at INTERVENE; a bounded evidence window closes suppressed cycles as `Unevidenced`. |
| GREEN | `field_cinema_effect_contract` (22): the A1 cases permanently, the full A3 hostile set, a second non-Gaussian geometry, and on the real producer: the no-op edit, MEASURED natural drift (< 1% vs the 5% band), a decrease-after-increase second transform, refused/suppressed, and the **matched-counterfactual attribution witness** (edited field == 2 × no-edit field cell-for-cell at identical model ticks). RXIII arc/oracle/PTY claims ported unchanged. |
| Mutations | Target-only rule reinjected into `classify` → 5 red. Arm-time `Indiscriminable` guard removed → 2 red (label assertions) while `classify`'s `Unchanged` branch still blocks the transform: two independent guards. |
| Limits | Causal **attribution** is claimed only where a matched counterfactual exists (the deterministic reference producer); the arc claims consistency, not mechanism. The contract measures the **peak sample** against the requested amplitude: a source whose grid misses the analytic peak (a test bump sampled at 0.94 × amplitude) is correctly `Inconsistent` — the requested quantity must be the measured quantity, declared per source. Stationarity is a declared source property; observed calibration can refute it, not prove it. |

## Gate B — command provenance and process lifetime · commit `df424f8`

| | |
|---|---|
| Defect | `live_link::Link` was an independent raw channel: `recv_ack` took the next ACK without id/source checks; FIELDs were staged under a fixed `source 1 / epoch 1`; irregular frames were untyped panics over a blocking read. |
| RED (fake producer via `GIBSON_PYTHON`) | 9/9: wrong-command-id ACK, wrong-source ACK, duplicate ACK and stale-epoch FIELD silently **accepted**; a foreign FIELD relabelled as source 1; early ACK, RETIRE, malformed frame and restart-with-unresolved-command were untyped **panics**. |
| Repair | `LiveSession` over the instrument's certified `Reader` / `ProducerOwner` / `CommandChannel` / `classify_source` (reused by `#[path]`, no new framework, no public API); typed fail-closed `LiveError`; bounded stderr tail; wall-clock deadlines; truncated-mid-frame provenance and bounded graceful shutdown added to the shared transport. |
| GREEN | `field_cinema_session_provenance` (29) over a deterministic in-memory lockstep responder + real processes measured through `/proc`; PTY: an inner failure reports typed and reaps the grandchild. |
| Mutations | Ledger bypass → 4 red; foreign source admitted → 2 red; session leaked on the failure path → PTY red. The PTY reaping test first passed **for the wrong reason** (the PTY's own hangup SIGHUPs the process group) and was hardened with a SIGHUP-ignoring fake before it could discriminate. |
| Limits | A SIGKILLed presenter runs no destructor: reaping then relies on the producer honouring stdin EOF (the reference producer does — pinned in the interactive PTY suite). |

## Gate C — the interactive live film · commit `54b8d81`

| | |
|---|---|
| Defect | `--live` was a scripted demonstration: programmatic probe, fixed target, printed text. |
| Repair | `field_cinema --live --interactive` (`film.rs`): real field → probe (arrows) → target (`[ ]`) → command through the tracked channel → correlated verdict → next authoritative field → composition responds to the VERIFIED difference; `gibson::scene` composition (eased camera between data-derived shots, narration slides, transform jolt), reduced motion = cuts, compact 40×15 layout, typed failure kept on screen; scripted `--live` stays the deterministic CI film. |
| GREEN | Real PTY (8, synchronized on painted text): accepted (+ producer-free replay equality), 40×15 monochrome (no colour SGR), refused, suppressed, masked sample on the non-Gaussian ring holdout, producer disconnect, reduced motion, SIGKILLed film. Headless oracle (11): on **rendered surfaces** at matched ticks, accepted ≠ no-edit while suppressed ≡ no-edit and refused ≡ no-edit **byte-for-byte**, on the Gaussian and the ring holdout. |
| Mutations | Displayed field frozen at frame 1 → 6 red. |
| Limits | Auto-advance timing is not under test (all tests step explicitly); motion is verified as "animates vs cuts", not judged aesthetically. |

## Gate D — capture / trace round-trip and resource debt · commits `4e86a3c`, `0588e15`, `54b8d81`

| | |
|---|---|
| RED (RXIII capture) | 5/5: writer emitted zero- and NaN-cell-width frames the reader rejects; default limits admitted `65536 × 2^20 × 8` = **512 GiB**; trailing garbage read back as valid; `File::create` + write destroyed a previous good capture on failure (117-byte truncated file). |
| Repair | One writer/reader contract (`validate_capture`), aggregate budget (`max_total_bytes`, 256 MiB, charged before allocation with checked arithmetic), strict readers + explicitly tolerant `*_prefix_with`, `UnsupportedVersion`, atomic commit (`atomic_write`). Trace format **v2** records command arguments (v1 could not distinguish "set 99, refused" from "set 50, refused"), `Failed` verdicts and `TransportFailed` admission; `wire_instrument --trace` and the film's `--record` / `--replay-film` use it. |
| GREEN | `wire_instrument_capture_integrity` (9) incl. a seeded 4000-capture closure sweep (486 accepted all read back identically and rewrite byte-identically; 3514 refused wrote nothing); a real-PTY instrument session commits a strictly readable trace with exactly its history; the film oracle replays every history exactly, separates equal-observation histories by causal digest, refuses tampered records, and is colour-depth independent. |
| Limits | Checksums are FNV-1a deterministic receipts — not authentication, not collision-free identifiers. Replay reproduces decisions from recorded observations; it does not re-execute the producer. |

## Gate E — the scene-algebra law ledger · commit `eb1b431`

RED: the RXIII "interchange under duration-aligned tiles" claim on four 1 s operands writing one
entity: left `(3,0)`, right `(10,0)` at `t = 1.5 s` with equal 2 s durations (**REFUTED**). The
law that holds is interchange under **S ∧ C** (`dur f = dur g`; `g`, `h` writes commute), derived
from `eval` and pinned on finite cases; the second tile need not be aligned. See
[`SCENE_ALGEBRA_LAWS.md`](SCENE_ALGEBRA_LAWS.md) (five categories; strict monoidal / duoidal /
functor readings **not claimed**).

## Gate F — physical viewport and grapheme realization · commit `e56cc46`

RED: no probe marker was painted at all, and on an 80-column PTY with a 300-column field the window
read `[0..240)` with the probe at column 100 off the physical screen; `clip("ab" + ZWJ family + "cd",
5)` produced `"ab👨\u{200d}…"` and the renderer painted the partial cluster. Repair: one geometry
contract bounded by the physical terminal; the probe is painted (`X`, outside the density ramp);
`clip` walks grapheme clusters. Mutation (ignore the physical bound) → 3 red. Limit: there is no
ASCII/Unicode glyph-capability mode in `TerminalCapabilities` to vary — the renderer was verified at
Mono vs TrueColor.

## Gate G — SDK, performance, partial writes, #15 · commits `060ffd0`, `0eee880`, `c97958b`, `9ca3ed4`, `249f1b4`

**G3 — partial terminal writes (a LIBRARY defect, `src/renderer.rs`).** RED: for every cut point of
a frame's byte stream (fail-after-N writer; received bytes replayed into a vt100 emulator; glyphs and
attributes compared with a clean render), the screen failed to converge after a torn frame for
**119/143** (fullscreen, reverting frame), **119/143** (fullscreen, new frame) and **120/157**
(inline) cut points — the renderer kept diffing against a belief the torn prefix had falsified.
Repair: commits count delivered bytes; zero bytes ⇒ certain (emission state restored, retry in
place); a prefix or a failed flush ⇒ uncertain (fullscreen re-establishes exactly; inline abandons
the torn region). GREEN at every cut point. The first cut poisoned on every failure and regressed
the RXI Gate F1 in-place inline restyle (a zero-byte failure) — caught by the 37-binary
render-adjacent run (and independently by CI run 41983109655 on `c97958b`), fixed by the
certain/uncertain split (`0eee880`); render-adjacent suites then 37 binaries, 290 passed, 0 failed.

**G1 — SDK.** The installed-SDK forcing consumer runs in the CI Bindings job (first green: run
41983651435 on `0eee880`, log `doom_forcing_consumer: OK frames=240 … cells_read=122880`) and in the
release cleanroom. A real-TTY input→state→render loop runs entirely through ABI v1
(`abi_real_tty_input_drives_state_that_is_rendered`, verified on the painted vt100 frame; mutation
"advance state without re-rendering" → red). ABI stays 59 symbols, MSRV 1.85.

**G2 — envelope.** Measured at `9ca3ed4`, two release runs, host and toolchain named:
[`V06_PERFORMANCE_ENVELOPE.md`](V06_PERFORMANCE_ENVELOPE.md). The round trips (≈1.06 ms) are upper
bounds quantized by `LiveSession`'s 1 ms reader poll — stated, not hidden. A streaming producer
ignores stdin EOF, so its cleanup takes the 2 s bounded kill backstop (OBSERVED).

**G4 — #15.** A real-PTY A/B isolated from LibGibson: stock crossterm 0.29.0 delivered **0/30**
resize-then-key collisions (interleaved re-run 0/30); the #1128 fork `2a195fb4` **30/30**; the
key-then-resize control 30/30 on both. #1128 still open, crates.io still 0.29.0 (checked
2026-11-05): **blocked upstream**, not claimed fixed
([`research/issue15_crossterm_ab`](research/issue15_crossterm_ab/README.md)).

## Independent adversarial review round · commit `b060093`

An independent, read-only reviewer was given the RXIV code (not these conclusions) and asked to
break five load-bearing pieces. It found **eight real defects — none a false TRANSFORM** — each of
which was then reproduced RED on the unmodified head and fixed:

| # | Defect | RED on `b70aa5c` | Repair |
|---|---|---|---|
| 1 (high) | An uncalibrated edit the producer applied was folded into calibration as "natural drift" → every later intervention `NonStationary` for the session | `NonStationary` where `Pending` was required (2 tests) | a cycle closing without TRANSFORM re-seeds calibration from the post-command regime |
| 2 | An unmeasured frame counted as calibration while hiding the drift across it | 1.0, None, 9.0 armed `Pending` | calibration chains measured observations only |
| 3 | A zero-byte-failed resize frame dropped its canvas reset | retry emitted no `2J` | `owed_reanchor` until a frame lands |
| 4 | A fullscreen scrollback commit cleared the uncertainty without re-establishing the canvas | flag cleared; no `2J` next frame | only inline clears it |
| 5 | A torn frame could leave auto-wrap off; recovery did not re-enable it | committed text before any `?7h` | every recovery prefix starts `?7h` |
| 6 | No verdict deadline: a mute producer froze the film in INTERVENE | real-PTY film hung > 20 s | `VERDICT_DEADLINE` → typed failure, command recorded pending |
| 7 | Frames after RETIRE were admitted | the post-RETIRE frame was returned | nothing admitted after RETIRE (`after_retire`) |
| 8 | A producer descendant holding the pipes stalled teardown past any deadline | cleanup blocked 5.99 s | process-group kill + bounded joins |

The review also confirmed what it could **not** break: no TRANSFORM on an ACK alone, an unchanged or
too-high value, a stale cursor, an epoch break, NaN/±inf, refused-then-retried or repeat-after-
TRANSFORM; wrong-id / wrong-source / duplicate / pre-decoded early ACKs fail closed; the capture
closure, charge-before-allocate and strict reads; `atomic_write` never touching the target on
failure; the counted-commit certain/uncertain split. Residuals kept as **stated limits** (not
fixed): an ACK that guesses the next command id and is decoded *during* `submit` is accepted (an
inherent wire race without a delivery handshake); the film's two record files are committed by two
atomic writes, so a failure between them leaves a mismatched pair that replay detects
(`ChecksumMismatch`) but cannot repair; extreme legal geometry (origin ~1e13, cell 1e-3) can paint
the probe marker up to tens of columns from its cell through floating-point rounding (it stays
inside the area).

## R6.1–R6.6 verdicts

| Workstream | Verdict |
|---|---|
| R6.1 instrument-driven cinematic grammar | **Completed and verified** (candidate) — limits under Gate A/C |
| R6.2 consumer ergonomics & interaction depth | **Completed and verified** (candidate); #176/#177 superseded |
| R6.3 cross-language interaction contracts | **Completed with documented limitation** — headless output-only, no pixel raster, no claimed C++/Go/Python interactive parity |
| R6.4 extended graphics / media | **Explicitly deferred by design** (#186/#188) |
| R6.5 performance & resource envelopes | **Completed with documented limitation** — one-host envelope + a truthful, tested partial-write policy |
| R6.6 long-standing terminal issues (#15) | **Blocked upstream; documented limitation** — next probe: rerun the A/B on the first crossterm release containing #1128, then un-ignore the delivery acceptance |

## Gate H — exact-head qualification

The candidate is the head of `feat/v0.6-RXIV-causal-audit-191`, proposed to `main` as one
integration PR that preserves the full Renaissance ancestry. Its exact-head development CI and
publication-free Release Preflight run IDs — on that same SHA, never an earlier green head — are
recorded on the PR (recording them here would change the SHA they certify). Merge requires the
independent audit's approval of that SHA, then the owner's action; no tag, release or registry
publication is part of this candidate.
