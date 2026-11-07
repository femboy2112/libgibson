# fb_adapter — a software-framebuffer adapter over the installed SDK (v0.7 node J, in-repo part)

**What this is.** A generic C11 adapter that puts any program which draws a `W x H` software
framebuffer (a Doom-shaped 320x200 palette-indexed frame, or 32-bit pixels) onto a terminal through
LibGibson's **public C ABI only**, plus a rights-safe synthetic producer, an interactive demo, and
the evidence that the adapter keeps its contract. It names no game: nothing here is
Doom-specific, and nothing in LibGibson changed (no `src/`, ABI or build change; ABI v1 stays 62
symbols). It is MIT OR Apache-2.0, like the repository. A GPL engine port lives **outside** this
repository and links this adapter in (see "The engine-port contract").

| File | Role |
|---|---|
| `fb_adapter.h` / `fb_adapter.c` | the adapter: scaling + half-block lowering, input + hold grace, fixed-tic loop, resize, lifecycle |
| `synthetic_producer.h` / `.c` | own procedural art: a steerable tunnel, star dust and a ship marker, 320x200 indexed, deterministic state + FNV-1a 64 hash |
| `fb_adapter_demo.c` | interactive demo, headless replay, and `--selfcheck` (7 properties, 12 negative controls) |
| `pty_harness.py` | an independent real-PTY harness: P2 on the wire (needs pyte), P3, P5, P6, P7 through a pty; 7 negative controls |
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
| P6 | the terminal is restored | real PTY at 80x24, termios read with `tcgetattr` on the slave the harness holds; ~1 s of steady play (36 tics) before each action | `q`: termios after = before, exit 0 in 0.01 s. SIGTERM: termios after = before, died by signal 15 (re-raised after restore) in 0.02 s. Hangup (master closed, SIGHUP ignored): the poll returned `GIBSON_ERR_IO`, report `exit=hangup`, exit 0 in 0.01 s, child CPU 0.07 s in total (no spin). Same on the PTY harness route |
| P7 | memory and frame cost stay bounded | 2000 frames per size, fresh process per size, numbers after a 200-frame warm-up; VmHWM growth must stay within 1024 KiB and the last 500 frames' mean within 1.5x of frames 200-699 | see the tables below; headless growth +456 / +0 / +4 KiB |

### Negative controls (each must FAIL; `fb_adapter_demo --selfcheck --list-controls`)

| Control | Property | Defect injected | Observed |
|---|---|---|---|
| `p1-shifted-ref` | P1 | expected pattern shifted one pixel | 0/32,000 cells match, both formats |
| `p2-edge-sampling` | P2 | reference samples pixel edges, not centres | 384,294 mismatching cells in 64 of 72 cases |
| `p2-no-letterbox` | P2 | reference stretches to the whole grid | 728,776 mismatching cells in 58 cases |
| `p3-ignore-input` | P3 | producer ignores keys | keyed hash = unkeyed hash (headless and PTY) |
| `p4-frame-locked` | P4 | mutant adapter: one tic per presented frame | 421 / 105 / 0 tics, three different hashes |
| `p5-stale-map` | P5 | mutant adapter: map not rebuilt on resize | map builds 1; 360,342 bad cells after the resize (PTY: map grid stays 80x24) |
| `p5-resize-steps` | P5 | mutant adapter: a resize runs an extra producer tic | hash differs from the no-resize run (headless and PTY) |
| `p6-no-restore` | P6 | exit without `gibson_destroy_context` | termios after != before |
| `p6-no-signals` | P6 | no SIGTERM handler | killed by signal 15 with no report; termios after != before |
| `p6-retry-io` | P6 | mutant adapter: `GIBSON_ERR_IO` from a poll ignored | kept looping 1.45 s after the hangup (1.51 s CPU), then exited `error` through the loop's render-error limit, never `hangup` |
| `p7-leak` | P7 | mutant adapter: 64 KiB leaked per present | VmHWM +129,216 KiB at 80x24 |
| `p7-no-rotation` | P7 | headless capture never rotated (finding F1) | VmHWM +69,152 KiB at 80x24, capture 78,683,261 B |
| `p2-wire-wrong-tic` (PTY) | P2 wire | reference is the replay 5 tics early | 1,366 mismatching cells at 80x24 |

### Frame cost and memory (P7)

Per presented frame: *lower* = building the styled spans, the rich-text node and `set_root_node`;
*render* = `gibson_render` (headless: into the capture; PTY: including the write to the pty). Mean
and p99 in microseconds over frames 200-1999. The 35 Hz budget is 28,571 us.

Headless (`--selfcheck`, fresh process per size, capture rotated at 4 MiB):

| Grid | lower mean / p99 | render mean / p99 | total mean / p99 | VmHWM after warm-up -> end | capture rotations |
|---|---|---|---|---|---|
| 80x24 | 69 / 112 | 616 / 852 | 685 / 966 | 8,864 -> 9,320 KiB (+456) | 18 |
| 160x50 | 242 / 413 | 2,174 / 3,070 | 2,416 / 3,440 | 15,560 -> 15,560 KiB (+0) | 60 |
| 320x100 | 874 / 1,510 | 7,453 / 11,397 | 8,327 / 12,986 | 19,240 -> 19,244 KiB (+4) | 166 |

Real PTY (`pty_harness.py`; `--bench-frames 2000` presents one frame per tic with no waiting; the
real-clock run is the normal 35 Hz loop for 175 tics):

| Grid | bytes / frame | lower mean / p99 | render + pty write mean / p99 | total mean / p99 | VmHWM growth | real clock, 175 tics |
|---|---|---|---|---|---|---|
| 80x24 | 39 KB | 68.6 / 111.9 | 799.0 / 1,045.2 | 867.6 / 1,135.8 | +40 KiB | 175 frames in 5,002 ms (35.0 fps), 0 dropped |
| 160x50 | 128 KB | 264.4 / 447.7 | 3,088.2 / 3,926.7 | 3,352.6 / 4,290.7 | +100 KiB | 175 in 5,004 ms (35.0 fps), 0 dropped |
| 200x60 | 169 KB | 369.8 / 604.0 | 4,197.4 / 5,762.7 | 4,567.2 / 6,295.1 | +0 KiB | 175 in 5,006 ms (35.0 fps), 0 dropped |
| 320x100 | 355 KB | 886.5 / 1,439.8 | 9,239.5 / 12,488.3 | 10,126.0 / 13,910.4 | +16 KiB | 175 in 5,012 ms (34.9 fps), 0 dropped |

So with this producer, a 320x200 frame lowered to half-blocks fits the 35 Hz budget at every size
measured, the worst p99 (13.9 ms at 320x100 on the PTY) at under half of it. The cost is in
`gibson_render` (about 90%), not in the adapter's lowering.

### Findings about LibGibson

**F1 (defect class: unbounded memory for any long-running headless C consumer).** A headless
context appends every rendered frame's ANSI to an in-memory capture, and the C ABI has no call
that drains it: `gibson_get_rendered_output` only copies (Rust has `Context::take_output`; C has
no equivalent). Memory therefore grows by every frame's bytes, without bound. Measured with this
adapter's rotation turned off (`p7-no-rotation`), 2000 frames: 80x24 +69 MB VmHWM (capture 79 MB);
in an earlier run of the same check over all three sizes, 160x50 +227 MB and 320x100 +636 MB
(capture 724 MB). The adapter works around it by replacing the headless
context once its capture passes 4 MiB (`headless_capture_limit`), carrying undelivered injected
events over; a terminal context is not affected (its output goes to stdout). Exact repro against
the staged SDK:

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

A domain-neutral fix would be one additive symbol that drains (or caps) the capture, e.g. a
`gibson_take_rendered_output`; nothing here needs it beyond the workaround.

**F2 (gap, worked around through the ABI).** C ABI v1 has no terminal-size query; a `RESIZE`
event arrives only on a change. The adapter learns the starting grid by rendering an empty frame
and reading `gibson_get_frame_size`, and re-reads the frame size after every present (which also
heals a grid that a missed `RESIZE` would leave stale).

**F3 (gap, recorded).** C ABI v1 reports focus events as `GIBSON_EVENT_NONE` (issue #126), so a C
adapter cannot release held keys on focus loss the way DEEP SIGNAL does in Rust (#276); a hold
there ends only by its grace timeout. The kitty release events (#285) are not used either.

### Boundary

**Proven** (on this box, at this SHA, with the synthetic producer): the seven properties above,
each with a control that fails; that a 320x200 frame lowered to half-blocks fits a 35 Hz budget
headless at 80x24, 160x50 and 320x100, and under a PTY at 80x24, 160x50, 200x60 and 320x100,
including a real-clock 35 Hz loop with no dropped tics at each of those sizes; and that the bytes
on the wire carry the same cells as the readback (pyte, 80x24 and 40x15).

**Not proven:** no real Doom engine is measured in this repository (it cannot be: GPL); the PTY
reader here is a Python thread that discards bytes, so a real terminal emulator's parse and paint
throughput (355 KB per frame at 320x100 truecolor, about 12 MB/s at 35 Hz) is not measured;
lower colour depths are not measured; a terminal that stops reading stalls the loop inside
`gibson_render` (the loop is synchronous; node F's observation still applies); Linux only; a
single key press moves for the 25-tic grace (#275's trade-off).
