# v0.8 cross-language parity: first slice

**Status (2026-11-07):** first measured slice of the v0.8 milestone ("independent Rust/C/C++/Python/Go
clients built against installed artifacts and shared semantic fixtures", ROADMAP.md). One fixed
script, five consumers, one canonical transcript format, a comparator gated by a committed
expectation, and negative controls. **No library or C ABI change**: `src/` and `include/` are
untouched; the ABI is still `GIBSON_ABI_VERSION 1` with the 62 symbols on `main` `5790f1a`.

This slice MEASURES what exists. It does not claim full parity (see [Not claimed](#not-claimed)).

| Artifact | Path |
|---|---|
| The script (data) | `bindings/parity/v08/script.txt` |
| C consumer (the ABI reference) | `bindings/parity/v08/parity_v08.c` |
| C++ consumer | `bindings/parity/v08/parity_v08.cpp` |
| Python consumer | `bindings/parity/v08/parity_v08.py` |
| Go consumer | `bindings/parity/v08/go/` (`main.go`, `go.mod`) |
| Rust consumer | `examples/parity_v08.rs` |
| Comparator | `bindings/parity/v08/compare.py` |
| Committed expectation | `bindings/parity/v08/expected.json` |
| Harness | `scripts/dev/parity_v08.sh` |

## Reproduce

```sh
# a staged SDK built from this commit (any prefix; the run below used one staged from main 5790f1a,
# libgibson.so sha256 94041711b52c4d45a1b2759d1f0029718ed9412a8ba894064ede396d1a03f81d)
scripts/release/stage-sdk.sh --prefix /tmp/libgibson-sdk
CARGO_BUILD_JOBS=2 scripts/dev/parity_v08.sh /tmp/libgibson-sdk
```

Knobs: `PARITY_V08_REPS` (default 500), `PARITY_V08_WORK` (default: fresh `mktemp -d`),
`PARITY_V08_CARGO` (default `cargo +1.98.1`), `PARITY_V08_RUST_BIN` (skip the cargo build),
`PARITY_V08_SKIP_CONTROLS=1`. Exit 0 = no regression against `expected.json`; 1 = regression;
2 = usage/environment error.

On Linux the same harness also runs the C++, Go and Python interactive focus wrappers under a
PTY. It checks the enable/disable/restore bytes, catches a client that skips enable, and checks
the propagated error from an injected `/dev/full` write. This is a separate control and does not
change the 84 transcript records. Other systems report this control as UNVERIFIED; the PTY check
proves the requests written by the wrappers, not that a particular terminal implements them.

How each consumer reaches the installed SDK:

| Lang | Build / load | Surface actually used |
|---|---|---|
| C | `cc ... $(pkg-config --cflags --libs libgibson) -Wl,-rpath,<libdir>` | the C ABI (it IS the reference) |
| C++ | `c++ -std=c++17 ...` same pkg-config + rpath | `gibson.hpp`: `gibson::Node` RAII, `Context::abi_compatible()`, `Context::headless`, `set_root`, `render`, `push_event`, `poll_event_kind` / `poll_event_kindless(timeout)`, `frame_size`, `frame_cell(s)`, `frame_cell_glyph` / `full_glyph`, `rendered_output`, `close`, with `Error` / `QueueFullError`. Raw C only for what the wrapper cannot express (NULL pointers, `destroy(NULL)`, `set_color_depth`, `Node::text` of invalid UTF-8): 9 probes, `l02` |
| Python | a copy of `bindings/python/gibson.py` on `PYTHONPATH` (temp dir), `LIBGIBSON_LIBRARY=<sdk>/lib/libgibson.so` | `gibson.py`: `Context.headless`, `Node`, `GibsonStyle`, `set_root`, `render`, `frame_size`, `frame_cells`, `frame_cell`, `frame_cell_glyph`, `rendered_output`, `push_event`, `poll_event` (with `Event`, `Key`, `KeyKind`), `close`. Probes the wrapper cannot express (NULL pointers, foreign contexts, `destroy(NULL)`): raw ctypes on the same `.so` (`gibson._lib._name`) |
| Go | `go build` (cgo, `#cgo pkg-config: libgibson`), `GOFLAGS=-mod=mod GOPROXY=off`, rpath via `CGO_LDFLAGS`; `replace` to `bindings/go` | `bindings/go`: `CheckABI`, `NewHeadlessContext`, `NewColNode`, `NewStyledTextNode` + `gibson.Style`, `SetRootNode`, `Render`, `PushEvent`, `PollEventKind` / `PollEvent`, `FrameSize`, `FrameCell(s)`, `FrameCellGlyph` / `FullGlyph`, `RenderedOutput`, `Close`, `*gibson.Error`. Raw cgo only for what the wrapper cannot express (NULL pointers, `destroy(NULL)`, `set_color_depth`, the `cells_oob` out-param): 8 probes, `l02`, via `Context.UnsafeHandle()` |
| Rust | `cargo +1.98.1 build --release --example parity_v08` (this worktree's source = the SDK's source) | the native crate API: `Context::headless`, `Node`, `Style`, `InjectedQueue` + `Context::poll_from`, `Context::frame_size`/`frame_cell`/`rendered_bytes` (`TerminalSession::set_terminal_size` only in the `unapplied` resize control, since #293). No `gibson::ffi` call |

Toolchains of the recorded run: gcc/g++ 13.3.0, Python 3.12.3, Go `go1.18 gccgo` (GCC 14.2.0),
Rust 1.98.1, Linux x86_64.

## The script

`bindings/parity/v08/script.txt`, format v1. One step per line: `<id> <dim> <op> [arg ...]`,
single-space separated; `#` starts a comment line. Strings are lowercase hex of their UTF-8 bytes
(no token ever contains a space); code points are `U+XXXX`. `dim` assigns the step's records to a
matrix row: `EVT`, `GEO`, `CELL`, `TXT`, `ERR`, `LIFE` (RES and DET are measured by the harness).

What it does, in order:

1. **Create** a headless FULLSCREEN 40x12 context.
2. **Scene**: a column box with seven text nodes, `WRAP_NONE`: plain ASCII; RGB fg/bg + bold +
   underline; ANSI-256 fg on named-blue bg + italic; two wide CJK ideographs; two combining
   sequences (`e`+U+0301, `a`+U+0308); a 25-byte ZWJ family emoji (exceeds the 16-byte
   `GIBSON_CELL_GLYPH_CAP`); Myanmar U+1001 (the value of `GIBSON_KEY_ENTER`). Mount, render, read
   geometry, full-frame digest, nine explicit cells, the full ZWJ glyph, two inline-cap records.
3. **Events**: push `a`; U+1001 as CHAR; ENTER as SPECIAL; Ctrl+`x`; Shift+LEFT; a paste
   (`paste:` + CJK + U+1001 + TAB + `tab`); RESIZE 60x16; Alt+`é`. Then poll 9 times
   (8 events + the empty-queue NONE) with `gibson_poll_event_kind`.
4. **After the delivered resize**: render, geometry (expect 60x16), digest, two cells.
5. **Failure probes** `p01`-`p22` (table below).
6. **Lifecycle**: free an unattached node; `gibson_destroy_context(NULL)`.
7. **Legacy key path** (last, because it leaves the queue in a language-dependent state): push
   0x1001 with kind-less `gibson_push_event` (always ENTER), push U+1001 as CHAR, poll both with
   kind-less `gibson_poll_event`. Then destroy.

### Ops

| op | args | C ABI call(s) | record `k` |
|---|---|---|---|
| `create` | `MODE COLS ROWS` | `gibson_create_headless_context` | `create` |
| `style` | `NAME FG BG ATTRS` (`reset` / `rgb:R,G,B` / `ansi:N` / `named:N`; ATTRS = 5 bits bold,dim,italic,underline,reverse) | none (builds a `gibson_style_t`) | none |
| `col` | | `gibson_node_box_col` | `node` |
| `text` | `STYLE WRAP HEX` | `gibson_node_text` + `gibson_node_add_child` | `node` |
| `mount` | | `gibson_set_root_node` (consumes the tree) | `mount` |
| `render` | | `gibson_render` | `render` |
| `geom` | | `gibson_get_frame_size` | `geom` |
| `terminal_size` | | `gibson_get_terminal_size` | `terminal_size` |
| `focus_reporting` | `0` / `1` | `gibson_set_focus_reporting` (headless no-op; not evidence of terminal support) | `focus_reporting` |
| `frame` | | `gibson_get_frame_size`, `gibson_get_frame_cells` (all cells), `gibson_get_frame_cell_glyph` for truncated cells, `gibson_get_rendered_output` | `frame` |
| `cell` | `X Y` | `gibson_get_frame_cell` (+ glyph escape hatch) | `cell` |
| `glyph` | `X Y` | `gibson_get_frame_cell_glyph` (size query, copy) | `glyph` |
| `glyphcap` | `X Y` | `gibson_get_frame_cell`: `glyph_len`, `_reserved[0]` | `glyphcap` |
| `push_char` | `U+X MODS` | `gibson_push_event_kind(..., GIBSON_KEY_KIND_CHAR)` | `push` |
| `push_special` | `NAME MODS` | `gibson_push_event_kind(..., GIBSON_KEY_KIND_SPECIAL)` | `push` |
| `push_legacy` | `U+X MODS` | `gibson_push_event` | `push` |
| `push_paste` | `HEX` | `gibson_push_event` (PASTE) | `push` |
| `push_focus` | `GAINED` / `LOST` | `gibson_push_event` (focus) | `push` |
| `push_resize` | `COLS ROWS` | `gibson_push_event` (RESIZE) | `push` |
| `poll` | `N` | N x `gibson_poll_event_kind(ctx, 0, ...)` | `event` (ids `<id>.1`..`<id>.N`) |
| `poll_legacy` | | `gibson_poll_event` | `event` (`kind` = `ABSENT`) |
| `probe` | `NAME` | see the probe table | `probe` |
| `lifecycle` | `NAME` | `node_free_unattached`: create + `gibson_node_free`; `destroy_null`: `gibson_destroy_context(NULL)` | `lifecycle` |
| `destroy` | | `gibson_destroy_context` (never twice: double-destroy is UB) | `destroy` |

### Probes (C ABI result / Rust result)

| id | name | C ABI call | C | Rust |
|---|---|---|---|---|
| p01 | create_bad_mode | create_headless(mode 7) | INVALID_PARAM, err `Invalid render mode: 7` | UNSUPPORTED (closed enum) |
| p02 | create_zero_cols | create_headless(FULLSCREEN, 0, 12) | INVALID_PARAM, err `Headless geometry must be non-zero, got 0x12` | **RUST_ACCEPTED**: renders a 0x12 frame |
| p03 | create_null_out | create_headless(..., NULL) | INVALID_PARAM | UNSUPPORTED |
| p04 | render_null_ctx | render(NULL) | INVALID_PARAM | UNSUPPORTED |
| p05 | cell_oob_x | get_frame_cell(cols, 0) | INVALID_PARAM | RUST_NONE |
| p06 | cell_oob_y | get_frame_cell(0, rows) | INVALID_PARAM | RUST_NONE |
| p07 | cells_oob | get_frame_cells(0, rows) | INVALID_PARAM, written 0 | UNSUPPORTED (no bulk readback) |
| p08 | cell_null_out | get_frame_cell(..., NULL) | INVALID_PARAM | UNSUPPORTED |
| p09 | text_bad_utf8 | node_text("\xff\xfe") | INVALID_PARAM, err `Invalid UTF-8 in text` | UNSUPPORTED (`str`) |
| p10 | push_null_event | push_event(ctx, NULL) | INVALID_PARAM | UNSUPPORTED |
| p11 | push_surrogate | push_event_kind(KEY 0xD800, CHAR) | INVALID_PARAM, err `... key_code 0xd800 is not a Unicode scalar value` | RUST_NONE (`char::from_u32`) |
| p12 | push_special_not_key | push_event_kind(KEY 'a', SPECIAL) | INVALID_PARAM | UNSUPPORTED |
| p13 | push_bad_kind | push_event_kind(kind 9) | INVALID_PARAM | UNSUPPORTED |
| p14 | push_bad_mods | modifiers 0x80 | INVALID_PARAM | RUST_NONE (`KeyModifiers::from_bits`) |
| p15 | push_resize_zero | RESIZE 0x12 | INVALID_PARAM | **RUST_ACCEPTED** (`InjectedQueue` takes `Resize(0,12)`) |
| p16 | paste_bad_utf8 | PASTE "\xc3\x28" | INVALID_PARAM, err `... paste_str is not valid UTF-8` | RUST_ERR (`String::from_utf8`) |
| p17 | paste_null | PASTE NULL | INVALID_PARAM | UNSUPPORTED |
| p18 | poll_null_out | poll_event(ctx, 0, NULL) | INVALID_PARAM | UNSUPPORTED |
| p19 | poll_kind_null | poll_event_kind(..., NULL kind) | INVALID_PARAM | UNSUPPORTED |
| p20 | color_depth_bad | set_color_depth(9) | INVALID_PARAM | UNSUPPORTED |
| p21 | queue_full | 1024 pushes, then one more, then drain | QUEUE_FULL, accepted 1024, drained 1024, err `... injected-event queue is full (1024 events)` | RUST_ERR, accepted 1024, drained 1024 (capacity is the caller's choice in Rust) |
| p22 | frame_before_render | fresh 8x2 ctx: frame_size, frame_cell(0,0) | size OK (0,0), cell INVALID_PARAM | RUST_NONE / RUST_NONE (`Option`) |

## Transcript (canonical form)

One JSON object per line, keys sorted bytewise, `,`/`:` separators with no spaces, integers only
(no floats). Every string value is printable ASCII without `"` or `\`; anything else is emitted as
`"hex:<utf8 hex>"`. Every record has `id` (the step id; polls append `.n`) and `k` (record kind).
`compare.py` re-serializes each line and rejects any line that is not in this form.

| `k` | keys |
|---|---|
| `create` | cols, id, k, mode, rows, st |
| `node` / `mount` / `render` / `push` / `destroy` | id, k, st |
| `geom` | cols, id, k, rows, st |
| `frame` | cells, digest, id, k, st, wire_digest, wire_len |
| `cell` | attrs, bg, cont, fg, glyph, id, k, st, transp, w, x, y |
| `glyph` | full_hex, full_len, id, k, st, x, y |
| `glyphcap` | id, inline_len, k, st, trunc, x, y |
| `event` | cols, id, k, key_code, kind, mods, paste, rows, st, type |
| `probe` | id, k, name, st (+ accepted, cols, drained, err, render, rows, size_st, written where relevant) |
| `lifecycle` | id, k, name, st |

* **`st`** vocabulary: the C ABI status names `OK`, `INVALID_PARAM`, `IO`, `PANIC`,
  `NOT_INITIALIZED`, `QUEUE_FULL`; `UNSUPPORTED` (+ `why`) when the language has no equivalent;
  `RUST_NONE` / `RUST_ERR` for a Rust-native rejection (`Option::None` / `Err`); `RUST_ACCEPTED`
  when Rust accepts an input the C ABI rejects; `WRAPPER_NULL` / `WRAPPER_RAISED` when a binding's
  only failure signal is a null handle / an exception (none occurred in this run).
* **Colors** `fg`/`bg` are `[color_type, r, g, b]` in the C ABI numbering (`RESET`=0 ... `ANSI256`=17,
  `RGB`=18); Rust's `None` and `Color::Reset` both encode as `[0,0,0,0]`, as in `src/ffi.rs`.
* **Keys**: `key_code` is the scalar value for `kind:"CHAR"`, the `GIBSON_KEY_*` value for
  `kind:"SPECIAL"`; `kind:"ABSENT"` marks the kind-less legacy poll.
* **Glyphs** are always the FULL grapheme bytes: when `gibson_cell_t` flags truncation
  (`_reserved[0]`), ABI consumers fetch the bytes with `gibson_get_frame_cell_glyph`.
* **Frame digest** = FNV-1a 64 (offset `cbf29ce484222325`, prime `100000001b3`) over the
  concatenation, row-major, of one ASCII line per cell:
  `<glyph hex>;<width>;<is_continuation>;<transparent>;<fg ct>,<r>,<g>,<b>;<bg ct>,<r>,<g>,<b>;<bold><dim><italic><underline><reverse>\n`.
  `wire_digest` = FNV-1a 64 over the headless ANSI output accumulated so far.
* **Out of the comparable transcript** (stderr): one `{"k":"resource",...}` line per repetition
  run, and one `{"k":"wrapper",...}` line of binding-surface observations (C++, Python, Go).

### What the comparator requires (review fix)

An independent review (Codex) showed the first comparator could pass a transcript that omitted
records, duplicated them, or changed a record inside a known gap. It now requires, per language:

- **completeness**: exactly the 76 ids of `expected.json` `record_ids`, each once (MISSING,
  UNEXPECTED and DUPLICATE are regressions);
- **pinned gaps**: every allowlisted difference (the 28 Rust records) must still equal its exact
  line in `expected.json` `pinned`; a change inside a known gap (e.g. `p21` turning from a refusal
  into `st:"OK"`) is a regression (CHANGED).

Each was checked against the failing scenario: Rust omitting its 28 allowlisted records, every
language emitting only `s01`, a duplicated record, and `p21` altered inside the gap all now exit 1;
the recorded transcripts still pass. The C consumer also queries a cell's full glyph length before
copying and exits 2 on a grapheme longer than 512 bytes, instead of reading past a fixed buffer.

## Results of the recorded run

`scripts/dev/parity_v08.sh` against the staged SDK from `5790f1a`, `REPS=500`, exit **0** (the fifth
full run of the session, on exactly the committed files; runs 2-4 had the same transcript agreement and
control results).

### Transcript agreement

Each transcript has 76 records. The C reference transcript sha256 is
`31fb3a805f609f1a996919ff6f4cdb5aa56689b4c63768f037c4d4a309557347`.

| Pair | Result |
|---|---|
| C++ vs C | **byte-identical** (all 76 records, same order) |
| Python vs C | **byte-identical** |
| Go vs C | **byte-identical** |
| Rust vs C | 48 records byte-identical; **28 differ**, all expected: `s27,s28` (inline glyph cap), `p01`-`p22` (all failure probes), `l02` (destroy NULL), `x01,x03,x04` (legacy kind-less key path) |
| C, C++, Python, Go vs Rust | the same 28 ids, nothing else |

So every scene, frame-digest, wire-digest, explicit-cell, full-glyph, push, poll and geometry
record is identical across all five languages, including all nine polls (e09.1-e09.9: eight events,
then the empty-queue NONE) and the 60x16 re-render. In the recorded run Rust needed one extra explicit step to get there (finding F1); since #293 `poll_from` alone does it.

### Replay determinism

For each language: run A == run B (two processes) == rep 1 of the 500-rep process, byte for byte,
and all 500 in-process repetitions produced the identical transcript (`iter_identical=1`). Five of five.

### Resource bounds (500 in-process repetitions of the whole script, create through destroy)

| Lang | wall ms | us / rep | peak RSS KB: rep 1 / rep 250 / rep 500 | second-half growth |
|---|---|---|---|---|
| C | 517 | 1035 | 3260 / 3260 / 3260 | 0 |
| C++ | 574 | 1149 | 4844 / 4844 / 4844 | 0 |
| Python | 6784 | 13569 | 16152 / 16152 / 16152 | 0 |
| Go | 3036 | 6073 | 61672 / 68008 / 68008 | 0 |
| Rust | 453 | 906 | 2740 / 2740 / 2740 | 0 |

The script refuses fewer than 500 repetitions: the bound is fixed in KB, so its sensitivity is about
4096 / (reps / 2) KB per repetition, and an independent run at 100 repetitions let the planted leak
slip under it (3712 KB). The gate is peak-RSS growth over the **second half** (rep 250 to rep 500) at most 4096 KB, which flags
a per-rep leak of about 16 KB or more at N=500. The first run used rep-1-to-rep-N growth and failed Go
(+6128 KB). A probe showed this was Go GC pacing, not a leak. Go grew +5.5 MB at 100 reps and +6.2 MB at
2000 reps (flat in N), and the growth tracked `GOGC` (25: +3.7 MB, 400: +20.3 MB). Warm-up growth is now
reported only. Go's second-half growth across runs 2-5 was 116, 512, 68 and 0 KB. Wall times are single runs
on a shared, loaded host (load average about 7-10 during the session), so they are not a benchmark.
Python's per-rep cost includes a pure-Python FNV loop over all 1,440 cell lines and the 2,050 ctypes
push/poll calls of `p21` (not profiled).

### Negative controls (all CAUGHT, exactly the planted ids)

| Control | Mechanism | Found by the comparator |
|---|---|---|
| cell, x5 languages | `PARITY_V08_PERTURB=cell`: last byte of step s06's text `!` -> `?` | `r03,s16` (both frame digests) for C, C++, Python, Go, Rust |
| events, x5 languages | `PARITY_V08_PERTURB=events`: swap steps e01/e02 | `ORDER,e09.1,e09.2` for all five |
| Rust resize witness | `PARITY_V08_RUST_RESIZE=unapplied`: put back the geometry `poll_from` applied (the behaviour before #293). In the recorded run this was `=native`, which skipped the consumer's explicit `set_terminal_size`; #293 removed that step | `r02,r03,r04,t02` in the extended script; r02 = `{"cols":40,"id":"r02","k":"geom","rows":12,"st":"OK"}` |
| planted leak (resource gate) | `PARITY_V08_PERTURB=leak` (C): d01 drops the context without destroying it | second-half growth 18304 KB > 4096 -> EXCEEDED |

The perturbations are env-gated in the consumers and never on by default. The harness unsets them
for the main runs.

## Capability matrix

Labels, applied per language per dimension:

* **SUPPORTED**: every record of the dimension is byte-identical to the C reference, and every
  operation of the dimension goes through that language's own public binding (for C, the ABI; for
  Rust, the crate API). For RES/DET: measured and within the gate.
* **PARTIAL**: identical results reachable only by bypassing the binding (raw C ABI from C++, raw
  ctypes, the consumer's own cgo), or some records of the dimension differ / are UNSUPPORTED.
* **UNSUPPORTED**: no record of the dimension can be produced in that language.
* **UNVERIFIED**: not exercised by this slice.

| Dimension | C | C++ | Python | Go | Rust |
|---|---|---|---|---|---|
| Event identity & ordering | SUPPORTED: e09.1-e09.9 are the reference; `poll_event_kind` separates e09.2 (U+1001 CHAR) from e09.3 (ENTER SPECIAL) | SUPPORTED: every e01-e09/x01-x04 record through gibson.hpp (`Context::headless`, `push_event`, `poll_event_kind`; `Event::char_key` vs `Event::special_key` keep e09.2 and e09.3 apart; kind-less `poll_event_kindless(timeout)` / `Event::legacy_key` for the x-records; wrapper obs `ctx_has_push_event:1`, `ctx_has_poll_event_kind:1`) | SUPPORTED: every e01-e09/x01-x04 record through gibson.py (`Context.push_event` / `poll_event`; `Event.char` vs `Event.special` keep e09.2 and e09.3 apart; `poll_event(kind=False)` is the kind-less x-record path; `ctx_has_push_event:1`, `ctx_has_poll_event:1`, `module_has_event_struct:1`) | SUPPORTED: every e01-e09/x01-x04 record through bindings/go (`NewHeadlessContext`, `PushEvent`, `PollEventKind`; `CharKey` vs `SpecialKey` keep e09.2 and e09.3 apart; kind-less `PollEvent` / `LegacyKey` for the x-records; `ctx_has_PushEvent:1`, `ctx_has_PollEventKind:1`) | PARTIAL: e01-e09 byte-identical via `InjectedQueue` + `poll_from`; x01/x03/x04 UNSUPPORTED (no kind-less key path) |
| Geometry | SUPPORTED: s15 40x12, r02 60x16; a polled RESIZE is the only resize path (no direct resize call, none needed) | SUPPORTED: s15/r02 via `Context::frame_size`, with the RESIZE pushed and polled through `push_event` / `poll_event_kind` (was PARTIAL: no `frame_size`, raw C) | SUPPORTED: s15/r02 via `Context.frame_size`, with the RESIZE pushed and polled through `Context.push_event` / `poll_event` (was PARTIAL: the resize needed raw ctypes) | SUPPORTED: s15/r02 via `Context.FrameSize`, with the RESIZE pushed and polled through `PushEvent` / `PollEventKind` (was PARTIAL: no readback) | PARTIAL: in the recorded run r02 matched only with an explicit `ctx.session.set_terminal_size`; since #293 `poll_from` alone gives r02 60x16 (the label is unchanged in `expected.json`) |
| Rendered cells & styles | SUPPORTED: s16/r03 digests (cells + wire), s18 RGB+bold+underline, s19 ANSI256+named+italic, s25/r04 blanks | SUPPORTED: every CELL record through gibson.hpp (`Context::headless`, `Node` RAII + style, `set_root`, `render`, `frame_cells`, `frame_cell`, `rendered_output`) | SUPPORTED: every CELL record through gibson.py (`Context.headless`, `Node.text`+`GibsonStyle`, `frame_cells`, `frame_cell`, `rendered_output`) | SUPPORTED: every CELL record through bindings/go (`NewHeadlessContext`, `NewStyledTextNode` + `gibson.Style`, `SetRootNode`, `Render`, `FrameCells`, `FrameCell`, `RenderedOutput`) | SUPPORTED: s16/r03 and s18/s19/s25/r04 via `frame_cell` + `rendered_bytes` (r03/r04 rely on the GEO step) |
| Text & Unicode semantics | SUPPORTED: s20/s21 CJK width 2 + continuation, s22 `e`+U+0301 one cell, s23/s26 25-byte ZWJ width 2, s24/r05 U+1001, s27 inline cap = 14-byte prefix, `trunc:1` | SUPPORTED: `Node::text` carries the UTF-8; `Context::frame_cell_glyph` / `full_glyph` read the full grapheme | SUPPORTED: `Node.text(str)`, `frame_cell_glyph`, `GibsonCell.glyph_truncated` | SUPPORTED: `NewStyledTextNode` carries the UTF-8; `FrameCellGlyph` / `FullGlyph` read the full grapheme | PARTIAL: s17-s26/r05 byte-identical; s27/s28 UNSUPPORTED (Rust always has the full grapheme; the cap is a C artifact) |
| Failure codes | SUPPORTED: p01-p22 = 21 INVALID_PARAM + 1 QUEUE_FULL; last-error text for 6 | PARTIAL: p01-p22 byte-identical; 13 probes (p01, p02, p05, p06, p11-p17, p21, p22) via `gibson::Error` / `QueueFullError` with last-error text, 9 still raw C (NULL pointers, `set_color_depth`, the `cells_oob` out-param, `Node::text` of invalid UTF-8 drops the status: `node_text_invalid_utf8_raw_is_null:1`) | PARTIAL: byte-identical; the 8 event probes (p11-p17, p21) go through `Context.push_event` and `GibsonError` (status + last-error text), the other 14 still need raw ctypes (NULL pointers, foreign contexts) and `Node.*` ignore the status | PARTIAL: p01-p22 byte-identical; 14 probes (p01, p02, p05, p06, p09, p11-p17, p21, p22) via `*gibson.Error` with the last-error text read inside the failing cgo call, 8 still raw cgo (p03, p04, p07, p08, p10, p18, p19, p20); the older constructors (`NewTextNode`, `AddChild`, `SetRoot`) still drop statuses | PARTIAL: no status codes: 13 unrepresentable, 7 rejected natively, **2 accepted where C rejects** (p02, p15) |
| Lifecycle / ownership | SUPPORTED: s01 create, l01 free unattached node, l02 `destroy(NULL)`, d01 destroy; no double-destroy (UB) | PARTIAL: create / `close` / destroy via `Context::headless` and RAII (`close()` twice and moves are no-ops, no double destroy), l01 freed by `~Node`; l02 `destroy(NULL)` only via raw C | PARTIAL: create/`close`/`Node.__del__` via gibson.py (`close()` twice is a no-op, `close_twice_safe:1`); l02 only via raw ctypes | PARTIAL: create / `Close` (idempotent) / `Node.Free` via bindings/go; l02 `destroy(NULL)` only via raw cgo | PARTIAL: create/drop identical; l02 UNSUPPORTED (no null `Context`); ownership by move |
| Resource bounds | SUPPORTED: 0 KB second-half growth | SUPPORTED: 0 KB | SUPPORTED: 0 KB | SUPPORTED: 0 KB (68-512 KB in earlier runs; GC warm-up ~6 MB reported, not gated) | SUPPORTED: 0 KB |
| Replay determinism | SUPPORTED | SUPPORTED | SUPPORTED | SUPPORTED | SUPPORTED |

No cell is UNSUPPORTED or UNVERIFIED for the eight dimensions *as this script exercises them*;
dimensions the script does not exercise are listed under [Not claimed](#not-claimed).

## Findings

* **F1 (Rust, observed).** `Context::poll_from` delivers an injected `Resize` but does not resize a
  headless session. `src/context.rs:686-697` only marks the scheduler dirty, and the renderer then
  re-reads the unchanged `headless_size`. The C ABI's poll applies it (`src/ffi.rs:976-981`). The
  `poll_from` doc claim "keeping a scripted resize indistinguishable from a terminal one"
  (`src/context.rs:685`) therefore does not hold for a headless context. Witness r02 =
  `{"cols":40,...,"rows":12}` vs C `{"cols":60,...,"rows":16}`. **Fixed by #293:** `poll_from` applies a
  delivered resize to a headless context through the same `Context` implementation the C ABI's poll
  now calls; a zero-dimension resize is delivered but not applied. The consumer's explicit step is gone.
* **F2 (Rust, observed).** Input validation exists only on the C side. `Context::headless(_, 0, 12)`
  is accepted and renders a 0x12 frame (p02). `InjectedQueue` accepts `Resize(0, 12)` (p15), and a
  consumer that applies it as the C ABI does would set a zero-width headless size.
* **F3 (C ABI, observed, documented).** Kind-less polling conflates a legacy-pushed 0x1001 (ENTER)
  with a typed U+1001: x03 and x04 are identical records for different events. The `_kind` pair
  separates them (e09.2 vs e09.3). Rust cannot express the ambiguity.
* **F4 (C ABI, observed).** The 16-byte inline glyph cap cuts the 25-byte ZWJ family to a 14-byte
  codepoint-boundary prefix with `trunc:1` (s27). A consumer that hashes `gibson_cell_t.glyph` without
  the escape hatch would diverge from Rust. All four ABI consumers use `gibson_get_frame_cell_glyph`.
* **F5 (bindings, observed).** The binding surfaces cover far less than the C ABI does.
  - gibson.hpp now has (since this change) a headless `Context::headless`, `push_event`, `poll_event_kind`, `frame_size` / `frame_cell(s)` / `frame_cell_glyph`, `rendered_output` / `take_rendered_output` / `clear_rendered_output`, and `gibson::Error` carrying the status and last-error text. Still missing: node-constructor statuses, `set_color_depth`, NULL inputs (by design).
  - gibson.py has headless, readback, glyph, and (since this change) `GibsonEvent`, `Event`, `Context.push_event` / `poll_event` with the CHAR/SPECIAL key kind and `GibsonError` / `QueueFullError` carrying the last-error text. Still missing: node-constructor statuses, NULL/invalid-UTF-8 inputs (by design), destroy(NULL).
  - bindings/go now has (since this change) `NewHeadlessContext`, `Event` / `PushEvent` / `PollEventKind` / `PollEvent`, `FrameSize` / `FrameCell(s)` / `FrameCellGlyph`, `TakeRenderedOutput` / `ClearRenderedOutput`, a cgo-free `Style` with `NewStyledTextNode` / `AddStyledSpan`, and `*gibson.Error`. `Context.UnsafeHandle()` lets other cgo code reach the native context. The older `Line.AddSpan` (cgo-typed style), `NewTextNode`, `AddChild` and `SetRoot` are unchanged and still lose statuses.
  - All three bindings ignore or flatten the C status of node constructors.
* **F6 (Go binding, REPRODUCED on gccgo, fixed).** `NewContext` read the thread-local
  `gibson_last_error_message` in a second cgo call after `gibson_create_context` failed, so the
  message could come from another OS thread. Observed with `TestNewContextErrorIsFromItsOwnCall`
  (`bindings/go/gibson/last_error_test.go`). Each of 64 goroutines calls `NewContext` with its own
  invalid mode (100+i), which fails before touching a terminal with its own text, 2000 times
  (128,000 calls per run, GOMAXPROCS 8), while 4 unlocked polluter goroutines write a third text
  into whichever thread they run on.
  * Old wrapper (`c23bd72`), 12 runs of one compiled binary: **12 of 12 runs failed**, **4,601 of
    1,536,000 calls** returned a foreign message (another goroutine's mode, or the polluters' text).
  * Fixed wrapper, same 12 runs: **0 of 1,536,000**.
  * A first version of the test (one shared mode, polluters locked to their own threads) could only
    see an empty message. It caught 24 of 3,840,000 (15 of 30 runs; an independent re-run 14 of
    1,536,000, 8 of 12 runs), all empty. Review (Codex) noted that design could not see foreign
    text; the current test can.
  * Toolchain: local gccgo 1.18 only. gc (CI's `setup-go stable`) was **not** run locally, so its
    failure rate is UNVERIFIED; the test still passes or fails on gc by the same rule.
  * Detection is probabilistic: the broken wrapper is caught only when the scheduler moves a
    goroutine between the two calls; the fixed wrapper cannot fail this way.
  * Fix: create and read the error in one C helper in the cgo preamble, so both run in one cgo call
    on one thread. It was the only `gibson_last_error_message` call site in the Go binding; the
    exported API is unchanged.
* **F7 (header, source read).** `gibson.h` does not state that `gibson_destroy_context(NULL)` /
  `gibson_node_free(NULL)` are no-ops. The implementation null-checks (`src/ffi.rs:583`, `:1623`),
  which is what l02 relies on.

## Public API additions genuinely required

Only additions forced by an observed gap in this run, with the transcript line that shows it.

**C ABI (`include/gibson.h`): none.** Every step of the script produced its reference record through
the existing 62 symbols. In particular, resize needs no new call: a pushed RESIZE, once polled, resizes the
headless context (`{"cols":60,"id":"r02","k":"geom","rows":16,"st":"OK"}`).

**Rust crate API:**

1. **A delivered `Resize` must resize a headless session on the Rust injected-input path** (or a
   named method must do it), matching `gibson_poll_event`. Failing line (PARITY_V08_RUST_RESIZE=native):
   `{"cols":40,"id":"r02","k":"geom","rows":12,"st":"OK"}`. The shape (apply inside `poll_from`
   for headless sessions vs. a new method) is a decision, not made here. **Done in #293:** applied
   inside `poll_from`, no new method.

**Binding surfaces (not ABI). Each list is forced because the transcript lines could only be produced by
bypassing the binding:**

2. **gibson.hpp** (CLOSED for headless, events and readback): a headless `gibson::Context`
   constructor; `push_event` (kind carried by `Event`) / `poll_event_kind`; `frame_size` /
   `frame_cell(s)` / `frame_cell_glyph` / `rendered_output`. Before:
   `{"ctx_has_frame_cells":0,"ctx_has_frame_size":0,"ctx_has_headless":0,"ctx_has_poll_event":1,"ctx_has_poll_event_kind":0,"ctx_has_push_event":0,...}`;
   now: `{"ctx_has_frame_cells":1,"ctx_has_frame_size":1,"ctx_has_headless":1,"ctx_has_poll_event":1,"ctx_has_poll_event_kind":1,"ctx_has_push_event":1,...}`.
3. **gibson.py** (CLOSED for events): a `gibson_event_t` structure and `Context.push_event` /
   `poll_event` with the key kind, and `GIBSON_EVENT_QUEUE_CAPACITY`. Before: `{"ctx_has_poll_event":0,"ctx_has_poll_event_kind":0,"ctx_has_push_event":0,...,"module_has_event_struct":0,...}`;
   now: `{"ctx_has_poll_event":1,"ctx_has_push_event":1,...,"module_has_event_struct":1,...}`.
4. **bindings/go** (CLOSED for headless, events, readback and styled text): a headless context constructor; push / poll (with kind); frame size / cells /
   glyph / rendered output; a styled text node with a Go-constructible style type. Before:
   `{"ctx_has_FrameCells":0,"ctx_has_FrameSize":0,"ctx_has_PollEvent":0,"ctx_has_PollEventKind":0,"ctx_has_PushEvent":0,"ctx_has_PushEventKind":0,"ctx_has_RenderedOutput":0,...}`;
   now: `{"ctx_has_FrameCells":1,"ctx_has_FrameSize":1,"ctx_has_PollEvent":1,"ctx_has_PollEventKind":1,"ctx_has_PushEvent":1,"ctx_has_RenderedOutput":1,"ctx_has_TakeRenderedOutput":1}`.

**Surfaced as decisions, not required additions:**
* F2 validation: Rust may add fallible constructors, or document that the Rust API is infallible
  where the ABI validates. Lines: `{"cols":0,"id":"p02",...,"render":"OK","rows":12,"st":"RUST_ACCEPTED"}`
  and `{"id":"p15",...,"st":"RUST_ACCEPTED"}`.
* F5 status loss in wrapper node constructors: report the status (exception / error return) or
  document the null-handle signal. Line: `"node_text_invalid_utf8_raw_is_null":1`.
* F7: document the NULL no-op in `gibson.h`. This is a docs change, not an addition.

## Not claimed

* **Not full parity.** One headless FULLSCREEN scene in truecolor. The slice does not exercise:
  - the INLINE mode, scrollback, `commit_*` and `insert_*_before_live`;
  - rich text and Line nodes; spinner, text_input, border box, rule, rail, stack and dim nodes;
  - layout setters, wrapping, color-depth downgrade, stats parity;
  - general interactive TTY scenes and input timeouts (the request-byte control above is narrower);
  - mouse events (absent from ABI v1) and focus delivery by an interactive terminal;
  - `KeyCode::BackTab` (the ABI folds it into TAB);
  - threading;
  - `gibson_push_event` on a non-headless context.
  These are UNVERIFIED by this slice.

The extended headless script does inject and poll focus gained/lost events through ABI v1
and the wrappers. That verifies their identity and ordering, not terminal support.
* **Common mode.** C, C++, Python and Go all drive the same `libgibson.so`. Their agreement proves
  binding/ABI fidelity, not engine correctness. Rust-vs-C compares two API paths over the same engine
  source. The comparison is relative: an engine change that alters every language's output equally
  passes, because there is no committed golden transcript.
* **Not a packaging test.** Python imported a copied `gibson.py` from `PYTHONPATH` rather than a wheel in a venv
  (that path is `scripts/release/cleanroom.sh`). Go was gccgo 1.18, not the gc toolchain. The Rust
  consumer was built from this worktree's source, not a packaged crate. One host (Linux x86_64),
  one recorded run set.
* **Resources.** Peak RSS is a high-water mark, so it cannot show memory being freed. The gate detects leaks
  of about 16 KB per rep or more at N=500, not smaller. Wall times are not benchmarks.
* **Labels are as of `5790f1a`.** F7 is a source read, not an observed failure; F6 was observed (see its entry).
* **No claim** that the C ABI needs no additions beyond what this script exercises.
