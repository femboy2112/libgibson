# v0.6 Qualification Ledger (Renaissance XIV — final causal audit; XV — the glyph boundary; XVI — the terminal-write contract; XVII — merge and source version)

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
- Branch: `feat/v0.6-RXIV-causal-audit-191` (linear on top of `c55d277`). RXIV ended at `c5741f5`
  (CI 41987570234, Release Preflight 41987570279); RXV (#252) continues the same branch and ended at
  `4e33631` (CI 42020629007, Release Preflight 42020629037 / 42020687874); RXVI (#254) follows it.
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

## Renaissance XV — the text-input glyph boundary (#252) · commit `f404389`

An independent follow-up audit filed issue #252 against `c5741f5`: the pre-existing single-line
editor path disagreed with itself and with the terminal.

| | |
|---|---|
| Defect | `paint_text_input` placed graphemes with `Surface::set_cell`, bypassing the control guard `Surface::print_str` applies, and the ANSI compiler emitted cell glyphs verbatim. `normalize_paste` flattened CR/LF but kept TAB and every other control. Five width rules disagreed: cursor (`width.max(1)` per grapheme), total (`width` of the whole string), painter (`width.max(1)`, uncapped), compiler (the glyph's `display_width`, 0 for a control, capped at 2) and layout (`width` of the string). |
| RED (untouched `c5741f5`; the first-cut test files) | `tests/text_input_glyph_boundary.rs` **9/11 red** (the 2 green are the positive controls). The issue reproduction: buffer `"foo\tbar"`, cursor 7 vs total 6, raw `0x09` emitted. Hostile matrix: **19/19** payloads fail through `Node::text_input` and **18/18** representable ones through `gibson_node_text_input` (NUL cannot cross a C string) — raw `ESC ]` (OSC title, OSC 52 clipboard), `ESC P` (DCS), lone ESC, BEL, BS, DEL, NUL and C1 CSI/OSC/NEL bytes in the output; CSI erase/SGR consumed the visible text; DSR and private-mode CSIs reached the terminal. `tests/pty_text_input_paste.rs` **red on a real PTY**: the hostile bracketed paste left the screen blank except `6nz` — the pasted `ESC[2J` executed. Plus: a wide grapheme under the cursor at the right edge was neither drawn nor given a hardware cursor; layout sized the box from the string width, one cell short per zero-width grapheme. **REFUTED** (the "content cannot inject" property). |
| Repair | (1) Insertion — `handle_event` (keys, paste), `insert_str`, `insert_char`, `with_text` — is single-line and control-free: a run of line breaks (CR, LF, VT, FF, NEL, U+2028, U+2029) and each TAB → one space, other `char::is_control` dropped. (2) The painter is total over arbitrary strings: a grapheme holding a control or a line/paragraph separator, or a standalone zero-width one, is painted as one `U+FFFD` cell (kept, not rejected: a base typed before a lone mark merges with it; the cell stays a visible cursor stop Backspace removes). (3) One placement rule (`editor_cells`): a 1–2-wide grapheme is one cell; a grapheme wider than two cells is laid codepoint by codepoint over consecutive cells (same bytes, the advance `unicode-width` and a per-codepoint terminal make); a codepoint no cell can hold (U+17D8, 3 wide) → `U+FFFD`. `cursor_display_column`, `total_display_width`, `update_scroll`, the painter and the layout's intrinsic input width all derive from it. (4) Scrolling shows every cell of the cursor grapheme; a grapheme is painted whole or not at all; the hardware cursor is always placed. (5) `NumericInput::set_text` keeps its text as given, so invalid stays `Unparseable`. No public API change; the normalization, the per-grapheme `total_display_width` and `handle_event` → `false` for a no-op are documented behaviour changes. |
| Review round | An independent adversarial review of the first repair (read-only, traced) confirmed no C0/DEL/C1 path survived and found: **F1** a grapheme wider than two cells was capped at 2 — `ᄀᄀxyz` put the cursor inside the cluster, and one paste of 40 leading jamo (one grapheme, 80 columns) **erased the neighbouring widget** on the vt100 screen (both probed); **F3** `with_text` normalization turned `NumericInput::set_text("5\u{0}0")` into a valid 50; **F4** VT/FF/NEL fused words and U+2028/2029 stayed in a "single-line" buffer; **F2** `Node::text` (the unfocused-input path) emits zero-width Cf characters such as bidi overrides — not terminal controls; stated as a limit, not changed; **F5** two doc sentences overclaimed. F1, F3, F4, F5 repaired; the new tests are **7/13 red on the first repair** and green after. |
| GREEN | `text_input_glyph_boundary` (13): the issue reproduction; positive controls (ASCII, newline paste, grapheme backspace, wide emoji, CJK, combining marks, a ZWJ family); 21 payloads × 6 entry paths (direct node, paste, `insert_str`, typed keys, `with_text`, the C ABI) under three oracles — a strict renderer byte grammar (CSI only; CR/LF the only raw controls), a vt100 screen + hardware-cursor check against an independent spec model, and a vt100 callback recorder (no title, clipboard, bell, OSC or foreign CSI); agreement of cursor, total, painter, terminal and per-cell readback at **every** cursor position of a width corpus (ZWSP, a baseless mark, BOM + bidi override, a lone variation selector, Devanagari `क्षि` / `भोः` / `स्त्री`, a Hangul jamo cluster, U+17D8, U+2028) at mono and truecolour; an 80-column grapheme and Devanagari beside a neighbouring widget that must survive; scrolling a narrow input over every position; deletion from both ends and Ctrl+U/K; 40×15 mono; numeric text. `pty_text_input_paste` (1): on a real PTY (`resize_test_app`, raw mode, crossterm bracketed paste, `NO_COLOR`, 40×15) a hostile paste leaves the same input row and the same terminal side-effect profile as pasting its sanitized form, the stream carries no OSC, BEL, TAB, C1 or `ESC[2J`, and the editor exits 0 by OS wait status. Unit: wide-cursor scrolling, insertion normalization and no-op return values. |
| Mutations (final code) | **M1** painter guard removed (raw `Glyph::new(grapheme)`) → 8/13 red; 21/21 direct and 20/21 C-ABI payloads leak again (NUL is not representable in C). **M2** insertion sanitizer disabled → 5/13 red on exactly the state-path assertions while every painter-only oracle stays green. Two independent guards. |
| Gate B (local, final source) | Full `cargo test --no-fail-fast` + the CI example tests + the PTY job set run serially: **2388 passed, 1 failed, 24 ignored** over 188 result sets. The one failure is `wire_instrument_fullduplex`'s *throughput witness* (`max_tick > 3`: the Python producer must emit more than three 2 MiB fields during the window — its ACK round-trip assertion passed). That test links only `gibson::wire` / `snapshot` and the example transport, none of which RXV touches. Across 17 runs on the final source (1 in-suite, 16 isolated) it passed 15 — both failures during or right after the full suite, load ≈ 4 — and 10/10 on the untouched source: classified a **load-sensitive flake**, OBSERVED, not caused by this change; CI on the exact head is the judge. `clippy --all-targets --all-features -D warnings` (1.98.1), `RUSTDOCFLAGS=-D warnings cargo doc`, `fmt --check` and `cargo +1.85 check --locked --lib` clean. The earlier full run on the first repair: 2371 passed, 0 failed. |
| Oracle note | vt100 0.16.2 never draws `U+FFFD` (it reserves it for vte's invalid-UTF-8 marker), while real emulators draw it in one cell; the *screen* oracle therefore reads the stream through a byte-exact lens (`EF BF BD` → `¤`, both one cell). The *byte* oracle always sees the unmodified stream, and per-cell readback checks the substitute directly. |
| Limits | The width model is `unicode-width`'s: ambiguous = narrow; an emoji ZWJ or VS16 sequence is one 2-cell cluster, which per-codepoint terminals (vt100 — measured in the suite) draw wider; a grapheme-cluster terminal may size a >2-cell cluster differently from the editor's per-codepoint layout. `Node::text` still caps a cluster at two cells and emits Cf format characters (unfocused inputs use it). `Surface::set_cell`, `Glyph::new` and the raw-ANSI entry points trust their caller by design. A pathological cluster (e.g. one base plus thousands of combining marks) is one cell of unbounded byte length, re-segmented per keystroke — a cost, not a correctness break. |

## Renaissance XVI — the terminal-write documentation contract (#254) · commit `fbe86e5`

The independent merge-gate review of `4e33631` held the candidate on issue #254: the code and
Release Contract §9 say a terminal write is **not** atomic, but current documentation still
promised it. Documentation-contract defect; no runtime counterexample was involved.

| | |
|---|---|
| Defect | README ("a single atomic write per frame", "the renderer always knows where its live region actually is", "one atomic `TerminalTransaction`"), DESIGN §8 ("a single atomic write", "atomic from the terminal's perspective", the pre-RXIV `TerminalTransaction::commit`), `docs/FEATURES.md` ("Atomic `TerminalTransaction`"), Release Contract §6 ("`?2026` guarantees only a wire-atomic write") and ROADMAP's Phase 1.75 label — against `src/transaction.rs` (`commit_counted`: counted write loop + flush) and the Gate G3 cut-point recovery. |
| Static probe | `git grep -n -i -E 'atomic\|always knows'` over tracked files, minus std atomics and fixtures: **71** occurrences at `4e33631`, each classified — 24 other domains (in-memory compound rollback, channel admit-or-neither, decoder chunk rejection, lock-free atomics); 25 genuine filesystem atomicity (capture/trace/media files, `atomic_write`); 7 accurate negations of I/O atomicity; 2 accurate synchronized-update presentation statements; 3 superseded historical wording; **10 unsupported current promises**. Beyond the five sites #254 named, the probe found Release Contract §2 ("the atomic wire transaction"), the Go binding (`SetSyncUpdates` "enables or disables atomic terminal updates") and a `src/lib.rs` comment; a second probe for obsolete mechanism names (`TerminalTransaction::commit`, `write_all + flush`) found ROADMAP Phase 1.85, STATE_OF_LIBGIBSON, a renderer comment, the partial-write test header and the cinema envelope doc. |
| RED | `tests/doc_terminal_write_contract.rs` against the `4e33631` documents: **2/3 red** — 14 overclaim hits across README, DESIGN, ROADMAP, FEATURES, RELEASE_CONTRACT, the Go binding and `src/lib.rs`, and the positive control (README never stated the contract). The third test (the guard flags each sentence #254 quoted and spares true statements, including filesystem atomicity) is the instrument's own calibration. |
| Repair | One contract stated in every current document: batching, **not** atomic I/O; a zero-byte failure is a certain no-op retried in place; a torn prefix or a failed flush voids the physical belief (`Renderer::physical_state_uncertain()`), fullscreen recovery repaints exactly, inline recovery abandons the torn region to scrollback; synchronized-update framing is a presentation request. The anchor is described as a belief. Doc comments added to `Context::set_sync_updates` and `gibson_set_sync_updates`. Historical entries kept and marked superseded in place. Code changes are comment-only (the `+`/`-` lines of the `.rs`/`.go`/`.h` diff are all comment lines, checked mechanically). |
| GREEN | `doc_terminal_write_contract` 3/3. The probe after the repair: 75 occurrences, category "unsupported current promise" empty. |
| Mutations | **M1** the README sentence restored → red (1 hit, `README.md:67`). **M2** "Terminal writes are not atomic" deleted from Release Contract §9 → the positive control red. |
| Preservation (local, `fbe86e5`) | `fmt --check`, `clippy --all-targets --all-features -D warnings` (1.98.1), `RUSTDOCFLAGS=-D warnings cargo doc --no-deps`, `cargo +1.85 check --locked --lib`, `gofmt -l` clean. Targeted, serial: `text_input_glyph_boundary` 13/13, `pty_text_input_paste` 1/1 (real PTY), `renderer_partial_write` 8/8, `field_cinema_effect_contract` 25/25, `field_cinema_causal_oracle` 9/9, `field_cinema_interactive_oracle` 16/16, `field_cinema_session_provenance` 31/31, `field_cinema_live_wire` 6/6, `wire_instrument_source_lifecycle` 3/3, `wire_instrument_channel` 8/8, `wire_instrument_trace` 7/7, `ffi_abi_layout_freeze` 4/4, `audio_pocket_voice` 4/4 (frozen PCM golden; `tests/audio_pocket_voice.rs` and `docs/fixtures/` unchanged since `4e33631`). Exported `gibson_*` symbols of the built library = the 59-symbol baseline. The full suite, CI and Release Preflight run on the exact head (recorded on the PR). |
| Limits | A textual guard, not a semantic proof: it catches the phrases #254 found and close paraphrases in the scoped files; a novel paraphrase, or an overclaim in a file outside its scope, is not caught. Lines mentioning files are skipped so filesystem atomicity stays expressible, which also hides a terminal overclaim written on such a line. Historical documents outside the scope are annotated, not policed. |

## Renaissance XVII — the merge, the harness race and the source version (#253, #255, #257)

| | |
|---|---|
| Merge | The independent audit approved exactly `eaa0d1c` (GO). Revalidated before acting: PR head `eaa0d1c`, base `main` `2ffdd92`, 133 ahead / 0 behind, mergeable CLEAN, all 8 CI jobs + both preflights green on that head, 0 unresolved threads, every stacked v0.6 PR head (#200–#251) an ancestor of it. Merged with a merge commit and a head-SHA guard: **`f6cd8c7`**, parents `2ffdd92` (first) + `eaa0d1c`; its tree is byte-identical to `eaa0d1c`. Exact-main CI [42129557477](https://github.com/femboy2112/libgibson/actions/runs/42129557477) 8/8: Rust job 2376 passed / 0 failed / 23 ignored; ABI 59 = 59; hangup gate attempt 1/3; installed-SDK forcing consumer `OK frames=240 grid=64x8 cells_read=122880 … checksum=15734824182601092804`. #252 and #254 closed on merge. The 39 stacked PRs whose bases were feature branches (#207–#251) were closed as *integrated via #253* with per-PR ancestry evidence; none was merged on its own. |
| #255 (harness race) | `event_pressure_lab` trial directories were named `pid` + CLOCK_MONOTONIC µs, so parallel test threads collided with `EEXIST`. RED: the legacy name reproduced verbatim under a frozen clock and barrier-synchronized starts — 1 of 16 creators wins, deterministically. Repair: a process-wide sequence, exclusive creation, a bounded skip of existing names, other errors propagated. GREEN: `event_pressure_trial_dir` 6/6; full local suite 2363 passed / 0 failed. Mutants M0–M3 killed. A blind adversarial review found nothing material (3 nits recorded on the PR). Tier A; merged as #258 (**`7f57047`**, parents `f6cd8c7` + `37dc9f8`) after exact-head CI [42132595230](https://github.com/femboy2112/libgibson/actions/runs/42132595230) + preflight [42132595076](https://github.com/femboy2112/libgibson/actions/runs/42132595076). |
| #257 (release states) | Release Contract §1 still called v0.5.0 "a prepared release candidate, not yet tagged"; `v0.5.0` → `d8c7b99` is a published GitHub prerelease (2026-10-24). §1 now defines three states (prepared in source / tagged and released on GitHub / published through registries) and labels each milestone. |
| Source version | `0.6.0-dev` → `0.6.0` in `Cargo.toml`, `Cargo.lock` (the crate's entry only), `pyproject.toml` and the regenerated notices. `cargo package --list` (183 files) and the resolved dependency tree are unchanged apart from the crate's own version. **v0.6.0 is release state 1 only: not tagged, not released, not published**; tagging and release need separate authorization. |
| Limits | The #255 tests exercise `create_trial_dir`; re-inlining the legacy name in `Trial::start` would be caught only probabilistically by the parallel PTY suite. The merge proves integration on `main`, not new runtime capability. |

## R6.1–R6.6 verdicts

| Workstream | Verdict |
|---|---|
| R6.1 instrument-driven cinematic grammar | **Completed and verified** (merged, `f6cd8c7`) — limits under Gate A/C |
| R6.2 consumer ergonomics & interaction depth | **Completed and verified** (merged, `f6cd8c7`); #176/#177 superseded; the text-input glyph boundary (#252) closed in RXV |
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
publication is part of this candidate. **Outcome (RXVII):** approved head `eaa0d1c` merged as
`f6cd8c7`; exact-main CI 8/8 (see the Renaissance XVII table).
