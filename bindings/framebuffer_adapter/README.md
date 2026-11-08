# fb_adapter — a software-framebuffer adapter over the installed SDK (v0.7 node J, in-repo part)

**What this is.** A generic C11 adapter that puts any program which draws a `W x H` software
framebuffer (a Doom-shaped 320x200 palette-indexed frame, or 32-bit pixels) onto a terminal through
LibGibson's **public C ABI only**, plus a rights-safe synthetic producer, an interactive demo, and
the evidence that the adapter keeps its contract. It names no game: nothing here is
Doom-specific, and the slice that added it changed nothing in LibGibson (no `src/`, ABI or build
change; ABI v1 was 62 symbols). Since #297 it bounds its headless capture with
`gibson_clear_rendered_output` (ABI v1, 64 symbols). It is MIT OR Apache-2.0, like the repository. A GPL engine port lives **outside** this
repository and links this adapter in (see "The engine-port contract").

| File | Role |
|---|---|
| `fb_adapter.h` / `fb_adapter.c` | the adapter: scaling + half-block lowering, input + hold grace, fixed-tic loop, resize, lifecycle |
| `synthetic_producer.h` / `.c` | own procedural art: a steerable tunnel, star dust and a ship marker, 320x200 indexed, deterministic state + FNV-1a 64 hash |
| `fb_adapter_demo.c` | interactive demo, headless replay, and `--selfcheck` (7 properties, 14 negative controls) |
| `pty_harness.py` | an independent real-PTY harness: P2 on the wire (needs pyte), P3, P5, P6, P7 through a pty; 9 negative controls |
| `../../scripts/dev/fb_adapter_consumer.sh` | builds all of it from a temp dir against a staged SDK and runs everything |

## Build and run

Against an installed (staged) SDK, never the checkout's `include/` or `target/`:

```sh
scripts/release/stage-sdk.sh --prefix /tmp/gibson-sdk --force       # or use an SDK you already staged
export PKG_CONFIG_PATH=/tmp/gibson-sdk/lib/pkgconfig
cc -std=c11 -Wall -Wextra -Werror -O2 fb_adapter.c synthetic_producer.c fb_adapter_demo.c \
   $(pkg-config --cflags --libs libgibson) -lm -Wl,-rpath,$(pkg-config --variable=libdir libgibson) \
   -o fb_adapter_demo
./fb_adapter_demo                 # play: arrows/WASD steer, space pulses, q (or Ctrl-C) quits
./fb_adapter_demo --selfcheck     # PROP 1..7, then "fb_adapter: OK"
python3 pty_harness.py --demo ./fb_adapter_demo
```

Or all of it, plus every negative control: `scripts/dev/fb_adapter_consumer.sh [SDK_PREFIX]`
(`FB_ADAPTER_SDK_PREFIX` also works; without either it stages one with
`scripts/release/stage-sdk.sh --prefix <tmp> --force`, which runs a release cargo build).

The demo is keyboard-only and works down to a 40x15 terminal (status row included). At
`--play` it shows a 4:3 view (320x200 with 5:6 pixels) letterboxed in the grid, with a status row:
tic, presented fps, dropped tics, and the keys. A tap holds a direction for the 25-tic grace (see
"Input"), so a single key press moves the ship for 0.71 s.

## The adapter contract

### Frames

```c
fb_adapter_config cfg;
fb_adapter_config_defaults(&cfg, 320, 200);   /* 35 Hz, catch-up 10, grace 25, repeat 4, 1:1 pixels */
cfg.par_num = 5; cfg.par_den = 6;              /* 320x200 shown at 4:3 */
cfg.status_rows = 1;
fb_adapter_open_terminal(&cfg, &ad);           /* or fb_adapter_open_headless(&cfg, cols, rows, &ad) */
fb_adapter_submit_indexed(ad, pixels, 320, palette_rgb);   /* or fb_adapter_submit_argb32(ad, px, 320) */
fb_adapter_present(ad);                        /* lower to U+2580 spans + gibson_render */
fb_adapter_close(ad);                          /* gibson_destroy_context: restores the terminal */
```

The scaling rule is **declared** in `fb_adapter.h` so a test can recompute it independently:
aspect-preserving fit of `W*par` by `H` into `C` by `2(R - status)` half-cell pixels (a cell is
`cell_hw` = 2:1 tall by default), the non-limiting side rounded half up, the image height kept even
so every image cell is a full half-block pair, centred (floor), nearest-neighbour sampling at pixel
centres (`sx(i) = floor((2i+1)W / 2dw)`). Image cells are `U+2580` with fg = top pixel, bg = bottom
pixel; letterbox cells are spaces in the bar colour; the status row is the last row. The map
(`sx`, and the two source rows per cell row) is computed only when the grid changes
(`fb_geometry.map_builds` counts it); each frame is a table walk plus styled spans, with runs of
identical cells merged into one span (`gibson_line_add_span` per run, one `gibson_node_rich_text`,
`gibson_set_root_node`, `gibson_render`).

### Input

`fb_adapter_pump(ad, tic, timeout_ms)` polls `gibson_poll_event_kind`. A typed character becomes
its Unicode value (ASCII letters lowercased by the default map); a `GIBSON_KEY_*` special becomes
`FB_KEY_* = 0x110000 + (code - 0x1000)`, above Unicode, so the U+1001-U+100D collision of #274
cannot happen; `cfg.map_key` maps both to the producer's own codes. A terminal reports presses
only, so the adapter infers the hold, exactly the rule of `gibson::hold` (#265, #275): the first
press starts a hold that ends `hold_grace_tics` (25) later, an autorepeat while held re-arms it
`hold_repeat_tics` (4) later, and the producer receives a **press**, then a **synthesized release**
at the end tic (`fb_adapter_next_key`). `quit_char` (default `q`) and Ctrl-C request quit. A
`RESIZE` rebuilds the map; it never reaches the producer.

### Time

`fb_adapter_run(ad, &producer, &opts, &stats)` owns a fixed-tic loop: tics owed =
`floor(elapsed * tic_hz)`, at most `max_catchup_tics` (10) run per iteration and the rest are
counted in `tics_dropped`; a frame is presented after an iteration that ran a tic (every
`present_divisor`-th one; 0 = never); between iterations it waits for input until the next tic is
due. The producer only sees `key()` and `tic()` in tic order, and `draw()` when a frame is
presented, so its state cannot depend on presentation or wall time.

### Lifecycle

`gibson_destroy_context` (inside `fb_adapter_close`) restores the terminal. `GIBSON_ERR_IO` from a
poll means the terminal is gone: `fb_adapter_pump` returns `FB_TERMINAL_GONE`, the loop exits with
`FB_EXIT_HANGUP`, and nothing polls again. SIGINT and SIGTERM (handlers installed by
`fb_adapter_open_terminal` unless `install_signal_handlers = 0`) set a flag the loop honours within
one tic; the demo then restores the terminal and re-raises the signal so its parent sees it.
SIGHUP keeps its default action (the terminal cannot be restored anyway).

### The engine-port contract (for a GPL engine outside this repository)

An engine port implements its platform hooks on top of the adapter and keeps its own loop and
clock. For a doomgeneric-style port:

| Engine hook | fb_adapter call |
|---|---|
| init | `fb_adapter_config_defaults(&c, RESX, RESY)`; `c.par_num = 5; c.par_den = 6; c.quit_char = 0; c.map_key = <engine key map>`; `fb_adapter_open_terminal(&c, &ad)`; `atexit(close)` so the engine's own `exit()` restores the terminal |
| draw frame | `fb_adapter_submit_argb32(ad, DG_ScreenBuffer, RESX)` (or `_indexed` with the palette); `fb_adapter_present(ad)` |
| read key | `fb_adapter_pump(ad, fb_adapter_wall_tic(ad), 0)`, then `fb_adapter_next_key(ad, fb_adapter_wall_tic(ad), &ev)` -> `(ev.pressed, ev.code)` |
| ticks | `fb_adapter_ticks_ms(ad)` |
| sleep | `fb_adapter_pump(ad, fb_adapter_wall_tic(ad), ms)` (waits for input or the timeout) |
| resize | nothing: the pump rebuilds the map; the engine never hears of it |
| exit | after each engine tick: `FB_TERMINAL_GONE` from a pump, or `fb_adapter_quit_requested(ad)` -> `fb_adapter_close(ad)` and exit |

Game data is never bundled; the port must refuse to start without a user-supplied IWAD. Linking
this MIT OR Apache-2.0 code into a GPL-2.0 program is compatible in that direction and leaves
LibGibson's license unaffected.

## Evidence

Measured 2026-11-07 on the development box (8 cores, ~7 GB RAM shared with other work, Linux),
against an SDK staged from `main` 5790f1a (`libgibson` 0.7.0-dev, ABI v1 = 62 symbols, release
library), built and run by `scripts/dev/fb_adapter_consumer.sh` from a temp dir. Timings are
load-dependent: they are measurements, not a benchmark.

### Properties

`--selfcheck` checks all seven; P6 runs the demo on a real PTY that the check opens itself
(`posix_openpt`). `pty_harness.py` is a second, independent route (Python, observing only the pty
and the demo's report file) for P3, P5, P6 and P7, and adds a wire check for P2.

| # | Property | How it is checked | Result |
|---|---|---|---|
| P1 | a producer can submit a framebuffer | 320x200 test pattern (indexed + palette, and argb32) at a 320x100 grid (1:1); every cell read back with `gibson_get_frame_cells` | 32,000/32,000 cells exact, both formats |
| P2 | scaling + half-block lowering keep the declared geometry | 72 cases: 9 grids (80x24, 120x40, 40x15, 160x50, 200x60, 33x7, 1x1, 320x100, 400x120) x pixel aspect {5:6, 1:1} x status row {0, 1} x {indexed, argb32}; every cell against a nearest-neighbour reference written separately in double arithmetic (no adapter code); the five 5:6 geometries also against values derived by hand | 860,416 cells, 0 mismatches (760,678 image + 94,322 letterbox); adapter geometry = reference 72/72; hand geometry 5/5; one map build per grid. Wire (PTY + pyte): the last frame's ANSI equals the headless readback of the same recorded input, 0 mismatches at 80x24 (1,403 image + 437 bar cells) and 40x15 (518 + 42), pyte 0.8.2 |
| P3 | input flows terminal -> simulation | headless: keys pushed with `gibson_push_event_kind`, polled by the adapter; the producer's key log against an independent model of the hold rule. PTY: keys typed into the master | headless keyed `0f49011c9bbbb009` != unkeyed `9280b905d57cd933`; key log = model (press@10 release@36, press@60 release@85, press@90 release@115, press@100 release@105). PTY (3x Right, w, space, 2x a typed): keyed `a73b0e133b1da5fd` != unkeyed `f2042341bb257298` at 140 tics; each run's recorded trace, replayed headless, reproduces its hash |
| P4 | tics are independent of presentation | the same 22-event tic-stamped script for the same simulated wall time, presenting every tic / every 4th / never; plus a jittery clock (0-10 tics per iteration) and a 2.0 s stall | all five runs `611f8add2a08ead1` at 420 tics (420 / 105 / 0 frames); jitter max 10 tics/iteration, 0 dropped; the stall ran 10 tics and dropped 60 (cap 10) |
| P5 | resize changes geometry, not state | headless: RESIZE pushed at tics 150 (120x40) and 220 (40x15); every post-resize frame against the reference. PTY: `TIOCSWINSZ` mid-run | headless `5be5245583eb9b2f` = no-resize hash; frame size 80x24 -> 120x40 -> 40x15; 3 map builds; 150 post-resize frames, 0 bad cells. PTY (80x24 -> 120x40 -> 40x15, no keys): `fe9e9dd14b3590a5` = the no-input headless hash at 175 tics; final frame 40x15, 3 map builds |
| P6 | the terminal is restored; a handled signal ends only its own adapter | real PTY at 80x24, termios read with `tcgetattr` on the slave the check holds, the baseline taken before fork; ~1 s of steady play (36 tics) before each action | `q`: termios after = before, exit 0 in 0.01 s. SIGTERM: termios after = before, died by signal 15 (re-raised after restore) in 0.04 s. Hangup (master closed, SIGHUP ignored): the poll returned `GIBSON_ERR_IO`, report `exit=hangup`, exit 0 in 0.01 s, child CPU 0.09 s in total (no spin). SIGTERM, then a second adapter in the same process (`--reopen-after-signal 35`): the first ended `signal` after 36 tics, the second ran its 35 tics to `tic_limit`, termios after = before, exit 0. Same on the PTY harness route |
| P7 | memory and frame cost stay bounded; a frame fits a 35 Hz tic | 2000 frames per size, fresh process per size, after a 200-frame warm-up. Gates: present p99 <= 28,571 us (one 35 Hz tic); VmHWM growth over the second half (frames 1100-2000) <= 1024 KiB; mean of the last 500 frames <= 1.5x that of frames 200-699. The PTY route adds a real-clock gate (below) | every gate passes at 80x24 / 160x50 / 320x100: p99 1,185 / 2,559 / 8,840 us; second-half growth +156 / +0 / +28 KiB; late/early 0.74 / 1.03 / 1.05 |

### Negative controls (each must FAIL)

`fb_adapter_demo --selfcheck --list-controls` lists the 14 selfcheck controls; `pty_harness.py
--control NAME` runs the 9 PTY ones. The consumer script runs all 23 and requires each to fail.

| Control | Property | Defect injected | Observed |
|---|---|---|---|
| `p1-shifted-ref` | P1 | expected pattern shifted one pixel | 0/32,000 cells match, both formats |
| `p2-edge-sampling` | P2 | reference samples pixel edges, not centres | 384,294 mismatching cells in 64 of 72 cases |
| `p2-no-letterbox` | P2 | reference stretches to the whole grid | 728,776 mismatching cells in 58 cases |
| `p2-wire-shifted-cells` (PTY) | P2 wire | same tic and state hash; each wire cell compared with its right-hand neighbour | `mismatches=1294` (80x24) and `507` (40x15) with `hash=match`: it fails on cells alone (the script checks both) |
| `p3-ignore-input` | P3 | producer ignores keys | keyed hash = unkeyed hash (headless and PTY) |
| `p4-frame-locked` | P4 | mutant adapter: one tic per presented frame | 421 / 105 / 0 tics, three different hashes |
| `p5-stale-map` | P5 | mutant adapter: map not rebuilt on resize | map builds 1; 360,342 bad cells after the resize (PTY: map grid stays 80x24) |
| `p5-resize-steps` | P5 | mutant adapter: a resize runs an extra producer tic | hash differs from the no-resize run (headless and PTY) |
| `p6-no-restore` | P6 | exit without `gibson_destroy_context` | termios after != before (headless check and PTY) |
| `p6-no-signals` | P6 | no SIGTERM handler | killed by signal 15 with no report; termios after != before |
| `p6-retry-io` | P6 | mutant adapter: `GIBSON_ERR_IO` from a poll ignored | kept looping 1.44 s after the hangup (1.51 s CPU), then exited `error` through the loop's render-error limit, never `hangup` |
| `p6-sticky-signal` | P6 | mutant adapter: a handled signal also ends every later adapter (the old global flag) | the second adapter exits `signal` after 0 tics, exit code 1 (headless check and PTY) |
| `p7-leak` | P7 | mutant adapter: 64 KiB leaked per present | second-half VmHWM +67,384 KiB at 80x24 |
| `p7-no-rotation` | P7 | headless capture never cleared (`headless_capture_limit = 0`; finding F1) | second-half VmHWM +34,572 KiB at 80x24 |
| `p7-slow-present` | P7 | mutant adapter: every 35th present stalls 400 ms | headless p99 401,700 us; PTY bench p99 401,991 us, and real clock 141 frames in 5,519 ms = 25.5 fps with 16 dropped tics: the p99, fps and dropped-tic gates fail (the creep and memory gates can pass: every 500-frame window holds the same 15 stalls) |

### Frame cost and memory (P7)

Per presented frame: *lower* = building the styled spans, the rich-text node and `set_root_node`;
*render* = `gibson_render` (headless: into the capture; PTY: including the write to the pty). Mean
and p99 in microseconds over frames 200-1999.

**The gate, exactly** (both routes unless marked):
- the p99 of lower + render over frames 200-1999 is at most one 35 Hz tic, 28,571 us;
- VmHWM grows at most 1024 KiB from frame 1100 to frame 2000. Growth from frame 200 to 1100 is
  allocator settling and is reported, not gated: at 320x100 it measured 0 to 2.8 MiB across runs,
  and a 6000-frame run stayed flat from frame 200 on, so it does not scale with frames as a leak
  would;
- headless: the mean of the last 500 frames is at most 1.5x that of frames 200-699 (no creep);
- PTY only, at 80x24, 160x50 and 200x60: a real-clock run of the normal 35 Hz loop for 175 tics
  ends by its tic limit after 175 tics, drops no tic, and presents at least 34.0 frames per
  wall-clock second. At 320x100 the real-clock run is measured and reported, **not gated**: its
  result depends on host load (an independent re-run on the development box measured 34.9 fps with
  0 dropped tics three times, and 22.1 fps with 29 dropped tics once, inside a full script run).

The thresholds were set from margins measured before the gate existed: p99 at most 13.9 ms,
34.9-35.0 fps, 0 dropped tics, second-half growth at most 100 KiB.

Headless (`--selfcheck`, fresh process per size, capture bounded at 4 MiB). This table was measured
when the adapter bounded the capture by replacing the context. Since #297 it clears the capture
instead (`gibson_clear_rendered_output`), and the counter, still named `capture_rotations`, counts
clears: a re-run after the change cleared 18 / 60 / 166 times, as many as the rotations below, and
every P7 gate passed (second-half VmHWM +16 / +0 / +0 KiB).

| Grid | lower mean / p99 | render mean / p99 | total mean / p99 | VmHWM at frames 200 / 1100 / 2000 (second half) | capture rotations (context replacements) |
|---|---|---|---|---|---|
| 80x24 | 72 / 124 | 624 / 1,063 | 695 / 1,185 | 8,712 / 9,216 / 9,372 KiB (+156) | 18 |
| 160x50 | 234 / 320 | 2,032 / 2,281 | 2,266 / 2,559 | 15,340 / 15,340 / 15,340 KiB (+0) | 60 |
| 320x100 | 831 / 1,053 | 6,920 / 7,944 | 7,751 / 8,840 | 17,056 / 17,192 / 17,220 KiB (+28) | 166 |

Real PTY (`pty_harness.py`; `--bench-frames 2000` presents one frame per tic with no waiting; the
real-clock run is the normal 35 Hz loop for 175 tics):

| Grid | bytes / frame | lower mean / p99 | render + pty write mean / p99 | total mean / p99 | VmHWM second half | real clock, 175 tics |
|---|---|---|---|---|---|---|
| 80x24 | 39 KB | 74.3 / 128.9 | 829.6 / 1,431.5 | 903.9 / 1,545.8 | +4 KiB | 175 frames in 5,003 ms (35.0 fps), 0 dropped |
| 160x50 | 128 KB | 242.8 / 389.1 | 2,823.3 / 3,260.9 | 3,066.1 / 3,528.9 | +8 KiB | 175 in 5,008 ms (34.9 fps), 0 dropped |
| 200x60 | 169 KB | 328.2 / 539.9 | 3,821.4 / 4,515.0 | 4,149.6 / 4,998.9 | +0 KiB | 175 in 5,009 ms (34.9 fps), 0 dropped |
| 320x100 | 355 KB | 820.6 / 1,237.0 | 9,267.5 / 11,579.0 | 10,088.1 / 12,754.5 | +0 KiB | 175 in 5,016 ms (34.9 fps), 0 dropped |

So with this producer every gate passes at every size measured; the worst p99 (12.8 ms at 320x100
on the PTY) is under half a tic. The cost is in `gibson_render` (about 90%), not in the adapter's
lowering. The `p7-slow-present` control (every 35th present stalls 400 ms) fails the p99, fps and
dropped-tic gates (not the creep gate, whose two windows hold the same number of stalls):
see the controls table.

### Findings about LibGibson

**F1 (defect class: unbounded memory for any long-running headless C consumer).** A headless
context appends every rendered frame's ANSI to an in-memory capture, and the C ABI has no call
that drains it: `gibson_get_rendered_output` only copies (Rust has `Context::take_output`; C has
no equivalent). Memory therefore grows by every frame's bytes, without bound. Measured with this
adapter's rotation turned off (`p7-no-rotation`), 2000 frames: 80x24 +69 MB VmHWM from frame 200 to 2000
(capture 79 MB);
in an earlier run of the same check over all three sizes, 160x50 +227 MB and 320x100 +636 MB
(capture 724 MB). The adapter first worked around it by replacing the headless context once its
capture passed 4 MiB (`headless_capture_limit`), carrying undelivered injected events over. #297
added `gibson_take_rendered_output` / `gibson_clear_rendered_output`; the adapter reads cells, not
the ANSI, so it now calls `gibson_clear_rendered_output` at the same limit and keeps its context.
A terminal context is not affected (its output goes to stdout). Exact repro against the staged SDK
(before the fix; with it, a `gibson_take_rendered_output` after each render keeps the capture at
one frame):

```c
/* f1_capture_growth.c */
#include <gibson.h>
#include <stdio.h>
#include <string.h>
int main(void) {
    gibson_context_t *ctx = NULL;
    if (gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, 80, 24, &ctx) != GIBSON_OK) return 1;
    for (int f = 1; f <= 1000; f++) {
        char line[81];
        for (int i = 0; i < 80; i++) line[i] = (char)('a' + (f + i) % 26); /* every cell changes */
        line[80] = '\0';
        gibson_style_t st;
        memset(&st, 0, sizeof st);
        gibson_node_t *root = NULL, *txt = NULL;
        gibson_node_box_col(&root);
        for (int r = 0; r < 24; r++) {
            gibson_node_text(line, &st, GIBSON_WRAP_NONE, &txt);
            gibson_node_add_child(root, txt);
        }
        gibson_set_root_node(ctx, root);
        gibson_render(ctx);
        size_t len = 0;
        gibson_get_rendered_output(ctx, NULL, 0, &len);
        if (f == 1 || f == 10 || f == 100 || f == 1000) printf("frame %4d: capture %zu bytes\n", f, len);
    }
    gibson_destroy_context(ctx);
    return 0;
}
```

```
$ cc -std=c11 -Wall -Wextra -Werror f1_capture_growth.c $(pkg-config --cflags --libs libgibson) \
     -Wl,-rpath,$(pkg-config --variable=libdir libgibson) -o f1 && ./f1
frame    1: capture 2001 bytes
frame   10: capture 19956 bytes
frame  100: capture 199506 bytes
frame 1000: capture 1995006 bytes
```

The fix is the additive pair above (#297); `bindings/sdk_forcing/headless_capture_drain.c` checks
that one drained context stays bounded.

**F2 (gap, worked around through the ABI).** C ABI v1 has no terminal-size query; a `RESIZE`
event arrives only on a change. The adapter learns the starting grid by rendering an empty frame
and reading `gibson_get_frame_size`, and re-reads the frame size after every present (which also
heals a grid that a missed `RESIZE` would leave stale).

**F3 (gap, recorded).** C ABI v1 reports focus events as `GIBSON_EVENT_NONE` (issue #126), so a C
adapter cannot release held keys on focus loss the way DEEP SIGNAL does in Rust (#276); a hold
there ends only by its grace timeout. The kitty release events (#285) are not used either.

### Boundary

**Proven, by gates that fail when what they check is broken** (on this box; branch head with an
SDK staged from `main` 5790f1a; with the synthetic producer): the seven properties above, each
with at least one control that fails it. For the 35 Hz question that means exactly what P7 gates,
and no more: (a) the p99 of one present (lowering + `gibson_render`; on the PTY including the
write to it) over 1,800 frames is within one 35 Hz tic, headless at 80x24, 160x50 and 320x100 and
on a PTY at 80x24, 160x50, 200x60 and 320x100; (b) on that PTY, at 80x24, 160x50 and 200x60, a
175-tic (5 s) real-clock run of the 35 Hz loop presented at least 34.0 frames per second and
dropped no tic (at 320x100 this is load-dependent and not claimed); (c) high-water memory grew at most 1024 KiB over the second 900 frames at each
size. Separately, the bytes on the wire carry the same cells as the readback (pyte, 80x24 and
40x15).

**Not proven, or not measured:** a real terminal emulator's parse and paint cost (the PTY reader
here is a Python thread that discards bytes; 355 KB per frame at 320x100 truecolor is about
12 MB/s at 35 Hz); a real Doom engine (it cannot be in this repository: GPL); sizes other than
those listed; sessions longer than 5 s of real clock or 2000 frames; colour depths below
truecolor; a terminal that stops reading (the loop is synchronous and stalls in `gibson_render`;
node F's observation still applies); other machines or load levels; non-Linux; and the 25-tic
grace means a single key press moves for 0.71 s (#275's trade-off).
