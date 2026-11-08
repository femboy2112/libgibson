/* SPDX-License-Identifier: MIT OR Apache-2.0
 *
 * fb_adapter: a generic software-framebuffer adapter over the LibGibson public C ABI
 * (v0.7 node J, in-repo prototype).
 *
 * A producer -- any program that draws a W x H software framebuffer, e.g. a 320x200 engine port --
 * submits frames as 8-bit palette indices plus a 256-entry RGB palette, or as 32-bit 0xAARRGGBB
 * pixels. The adapter owns everything between that framebuffer and a terminal:
 *
 *   - scaling to the current cell grid (aspect-preserving letterbox, the rule below), through a
 *     sampling map that is rebuilt only when the grid changes;
 *   - lowering to U+2580 half-block cells (one cell = two vertical pixels: fg = top, bg = bottom)
 *     as styled spans through the public ABI (gibson_line_add_span / gibson_node_rich_text);
 *   - input: gibson_poll_event_kind -> producer key codes, with a press and a SYNTHESIZED release
 *     after a hold grace counted in tics (a press-only terminal never reports key-up);
 *   - an optional fixed-tic loop (default 35 Hz) that advances the producer by elapsed wall time,
 *     independently of how often frames are presented, with bounded catch-up and dropped-tic
 *     accounting;
 *   - resize (rebuild the map; never touch producer state) and lifecycle (gibson_destroy_context
 *     restores the terminal; GIBSON_ERR_IO from a poll = terminal gone, exit without retry;
 *     SIGINT/SIGTERM set a flag the loop honours).
 *
 * It uses no game-specific LibGibson API: only the installed SDK's public header.
 *
 * ## The scaling rule (declared, so a test can recompute it independently)
 *
 * Let the grid be C x R cells and S = status_rows (0 or 1). The image area is C x (R - S) cells,
 * i.e. VW = C by VH = 2 (R - S) "virtual pixels" (half cells). With source W x H, source pixel
 * aspect a = par_num/par_den (width/height of one source pixel) and cell aspect
 * k = cell_hw_num/cell_hw_den (cell height/width):
 *
 *     P = W * par_num * cell_hw_num,   Q = H * par_den * 2 * cell_hw_den   (target dw/dh = P/Q)
 *     if VW * Q <= VH * P:  dw = VW;  dh = round_half_up(VW * Q / P), clamped to [.., VH],
 *                           then rounded DOWN to even, and at least 2
 *     else:                 dh = VH;  dw = round_half_up(VH * P / Q), clamped to [1, VW]
 *     image cells: img_cols = dw, img_rows = dh / 2
 *     origin:      img_x = (C - img_cols) / 2,  img_y = ((R - S) - img_rows) / 2   (floor)
 *     sampling (nearest neighbour, pixel centres):
 *         sx(i) = floor((2 i + 1) W / (2 dw)),  sy(j) = floor((2 j + 1) H / (2 dh))
 *     cell (img_x + i, img_y + r):  glyph U+2580, fg = pixel(sx(i), sy(2 r)), bg = pixel(sx(i), sy(2 r + 1))
 *     every other cell of the image area: glyph ' ' with fg = bg = the bar colour;
 *     the status row (if any) is row R - 1.
 *
 * Nothing about the rule depends on frame content, so it is computed once per grid size.
 */
#ifndef FB_ADAPTER_H
#define FB_ADAPTER_H

#include <gibson.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ---- status ---- */
typedef enum {
    FB_OK = 0,
    FB_ERR_PARAM = -1,      /* a bad argument or configuration */
    FB_ERR_GIBSON = -2,     /* a LibGibson call failed (see fb_adapter_last_error) */
    FB_ERR_NOMEM = -3,
    FB_TERMINAL_GONE = -4   /* gibson_poll_event_kind returned GIBSON_ERR_IO: the terminal hung up */
} fb_status;

/* ---- producer key codes ----
 * Typed characters are delivered as their Unicode scalar value (ASCII letters lowercased by the
 * default key map). Special keys use codes ABOVE Unicode (>= 0x110000), so they can never collide
 * with a typed character (cf. GIBSON_KEY_* vs U+1001-U+100D, issue #274). FB_KEY_x = FB_KEY_BASE +
 * (GIBSON_KEY_x - 0x1000). */
#define FB_KEY_BASE      0x110000u
#define FB_KEY_ENTER     (FB_KEY_BASE + 0x01u)
#define FB_KEY_LEFT      (FB_KEY_BASE + 0x02u)
#define FB_KEY_RIGHT     (FB_KEY_BASE + 0x03u)
#define FB_KEY_UP        (FB_KEY_BASE + 0x04u)
#define FB_KEY_DOWN      (FB_KEY_BASE + 0x05u)
#define FB_KEY_HOME      (FB_KEY_BASE + 0x06u)
#define FB_KEY_END       (FB_KEY_BASE + 0x07u)
#define FB_KEY_BACKSPACE (FB_KEY_BASE + 0x08u)
#define FB_KEY_DELETE    (FB_KEY_BASE + 0x09u)
#define FB_KEY_ESC       (FB_KEY_BASE + 0x0Au)
#define FB_KEY_TAB       (FB_KEY_BASE + 0x0Bu)
#define FB_KEY_PAGE_UP   (FB_KEY_BASE + 0x0Cu)
#define FB_KEY_PAGE_DOWN (FB_KEY_BASE + 0x0Du)

/* Maps an adapter key code (above) to the producer's own code; return 0 to drop the key.
   `mods` is GIBSON_MOD_* of the terminal event. */
typedef uint32_t (*fb_key_map_fn)(void *user, uint32_t fb_code, uint8_t mods);

/* Observes every key event the adapter polls, before mapping (kind = GIBSON_KEY_KIND_*), stamped
   with the tic it is observed at. Used to record replayable input traces. */
typedef void (*fb_input_observer_fn)(void *user, uint64_t tic, uint32_t key_kind, uint32_t key_code,
                                     uint8_t mods);

typedef struct fb_adapter_config {
    uint16_t src_w, src_h;           /* framebuffer size the producer submits (1..4096 each) */
    uint8_t par_num, par_den;        /* source pixel aspect w:h; 1:1 default (320x200 at 4:3 is 5:6) */
    uint8_t cell_hw_num, cell_hw_den;/* terminal cell height:width; 2:1 default (half-block square) */
    uint8_t status_rows;             /* 0 or 1: a text status row at the bottom of the grid */
    uint8_t bar_rgb[3];              /* letterbox colour (default 0,0,0) */
    uint32_t tic_hz;                 /* fixed simulation rate (default 35) */
    uint32_t max_catchup_tics;       /* most tics one loop iteration runs (default 10); the rest are dropped */
    uint32_t hold_grace_tics;        /* a first press is held this many tics (default 25, issue #275) */
    uint32_t hold_repeat_tics;       /* an autorepeat re-arms the hold this many tics (default 4) */
    uint32_t quit_char;              /* typed char that requests quit (default 'q'; 0 = none). Ctrl-C always does. */
    int32_t color_depth;             /* GIBSON_COLOR_DEPTH_*, or -1 (default) to keep the context's own */
    size_t headless_capture_limit;   /* headless only: replace the context once its ANSI capture exceeds
                                        this many bytes (default 4 MiB; 0 = never). See README, finding F1. */
    int install_signal_handlers;     /* terminal only: SIGINT/SIGTERM set the quit flag (default 1) */
    fb_key_map_fn map_key;           /* NULL = default map (ASCII letters lowercased, everything else as is) */
    void *map_key_user;
    fb_input_observer_fn observer;   /* optional */
    void *observer_user;
} fb_adapter_config;

/* Fills `cfg` with the defaults above for a src_w x src_h producer. */
void fb_adapter_config_defaults(fb_adapter_config *cfg, uint16_t src_w, uint16_t src_h);

typedef struct fb_adapter fb_adapter;

/* A live terminal: gibson_create_context(FULLSCREEN). stdout must be a terminal. Raw mode is
   entered by the first poll; fb_adapter_close restores the terminal. */
fb_status fb_adapter_open_terminal(const fb_adapter_config *cfg, fb_adapter **out);
/* A headless context of cols x rows: deterministic, input only through gibson_push_event[_kind]
   on fb_adapter_context(ad). */
fb_status fb_adapter_open_headless(const fb_adapter_config *cfg, uint16_t cols, uint16_t rows,
                                   fb_adapter **out);
/* gibson_destroy_context (which restores a terminal), uninstalls the signal handlers, frees. */
void fb_adapter_close(fb_adapter *ad);
/* The context the adapter currently renders into (headless: may be replaced on present, see
   headless_capture_limit -- fetch it again after each present). */
gibson_context_t *fb_adapter_context(const fb_adapter *ad);
/* Thread-local-free last error text (a static buffer; diagnostic only). */
const char *fb_adapter_last_error(void);

/* ---- frame submission ---- */
/* Copies a src_w x src_h frame of palette indices (row pitch in bytes) and its 256 x RGB palette. */
fb_status fb_adapter_submit_indexed(fb_adapter *ad, const uint8_t *pixels, size_t pitch,
                                    const uint8_t palette_rgb[768]);
/* Copies a src_w x src_h frame of 0xAARRGGBB pixels (row pitch in pixels; alpha ignored). This is
   the layout of doomgeneric's DG_ScreenBuffer. */
fb_status fb_adapter_submit_argb32(fb_adapter *ad, const uint32_t *pixels, size_t pitch_px);
/* Text for the status row (printable ASCII; other bytes become '?'). Ignored if status_rows == 0. */
fb_status fb_adapter_set_status(fb_adapter *ad, const char *ascii);
/* Lowers the last submitted frame at the current grid and renders it (gibson_render). A failed
   render is FB_ERR_GIBSON (the renderer re-establishes the screen on the next frame); a terminal
   that hung up is reported by the next fb_adapter_pump as FB_TERMINAL_GONE. On a terminal the
   rendered frame size is read back (gibson_get_frame_size) and, if it differs from the grid the
   map was built for, the map is rebuilt for the next frame. On a headless context whose capture
   exceeds headless_capture_limit, the context is replaced first (undelivered events carried over). */
fb_status fb_adapter_present(fb_adapter *ad);

typedef struct fb_geometry {
    uint16_t cols, rows;              /* grid the map was built for */
    uint16_t img_x, img_y;            /* image origin, cells */
    uint16_t img_cols, img_rows;      /* image size, cells */
    uint32_t dw, dh;                  /* image size, virtual pixels (dh = 2 img_rows) */
    uint64_t map_builds;              /* how many times the sampling map has been (re)built */
} fb_geometry;
void fb_adapter_geometry(const fb_adapter *ad, fb_geometry *out);

/* Wall time of the last present: lowering (spans + node + set_root) and gibson_render, in ns. */
void fb_adapter_last_present_ns(const fb_adapter *ad, uint64_t *lower_ns, uint64_t *render_ns);

/* ---- input ---- */
typedef struct fb_key_event {
    uint32_t code;        /* producer key code (after map_key) */
    uint8_t pressed;      /* 1 = press, 0 = release */
    uint8_t synthesized;  /* 1 for a release inferred by the hold grace (always, today) */
    uint64_t tic;         /* tic the event takes effect at */
} fb_key_event;

/* Polls the terminal: waits up to timeout_ms for the first event, then drains what is ready.
   Key presses are observed at `tic` (a new press starts a hold that ends hold_grace_tics later;
   a repeat of a held key re-arms it hold_repeat_tics later). RESIZE rebuilds the sampling map.
   Returns FB_TERMINAL_GONE on GIBSON_ERR_IO -- the caller must stop polling. */
fb_status fb_adapter_pump(fb_adapter *ad, uint64_t tic, uint32_t timeout_ms);
/* Pops the next key event due at or before `tic`: synthesized releases of holds that ended at or
   before `tic` (in end order) are queued ahead of presses observed later. 1 = written, 0 = none. */
int fb_adapter_next_key(fb_adapter *ad, uint64_t tic, fb_key_event *out);
/* Quit requested by quit_char, Ctrl-C, or a SIGINT/SIGTERM received since this adapter opened. */
int fb_adapter_quit_requested(const fb_adapter *ad);
/* The SIGINT/SIGTERM received since `ad` was opened (0 if none). A signal that ended an earlier
   adapter does not end a later one. */
int fb_adapter_signal(const fb_adapter *ad);
/* The last SIGINT/SIGTERM the process received while handlers were installed (0 if none): what a
   program re-raises after restoring the terminal. */
int fb_adapter_signal_received(void);
/* Monotonic milliseconds since the adapter opened (for an engine's own clock, e.g. DG_GetTicksMs). */
uint32_t fb_adapter_ticks_ms(const fb_adapter *ad);
/* The adapter's wall-clock tic (elapsed time x tic_hz), for engines that run their own loop. */
uint64_t fb_adapter_wall_tic(const fb_adapter *ad);

/* ---- the fixed-tic loop (optional) ---- */
typedef struct fb_producer {
    void *user;
    void (*key)(void *user, uint32_t code, int pressed);   /* delivered before the tic it is due at */
    void (*tic)(void *user);                               /* advance exactly one tic */
    fb_status (*draw)(void *user, fb_adapter *ad);         /* submit the current frame (+ status) */
} fb_producer;

typedef uint64_t (*fb_clock_fn)(void *user); /* monotonic nanoseconds */

typedef struct fb_run_options {
    uint64_t max_tics;            /* stop once this many tics ran (0 = until quit/hangup/signal) */
    uint64_t max_iterations;      /* stop after this many loop iterations (0 = no limit; tests) */
    uint32_t present_divisor;     /* present on every k-th iteration that ran a tic (default 1; 0 = never) */
    int block_on_input;           /* wait for input until the next tic is due (default 1) */
    fb_clock_fn clock;            /* NULL = CLOCK_MONOTONIC; called exactly once per iteration */
    void *clock_user;
    void (*pre_tic)(void *user, fb_adapter *ad, uint64_t tic);  /* before each tic's input pump */
    void (*on_present)(void *user, fb_adapter *ad, uint64_t tics_run);
    void *hook_user;
} fb_run_options;
void fb_run_options_defaults(fb_run_options *o);

typedef enum {
    FB_EXIT_NONE = 0,
    FB_EXIT_QUIT,        /* quit_char / Ctrl-C */
    FB_EXIT_SIGNAL,      /* SIGINT / SIGTERM (fb_adapter_signal_received says which) */
    FB_EXIT_HANGUP,      /* the terminal went away (GIBSON_ERR_IO) */
    FB_EXIT_TIC_LIMIT,   /* max_tics reached */
    FB_EXIT_ERROR
} fb_exit_reason;

typedef struct fb_run_stats {
    uint64_t tics;               /* producer tics run */
    uint64_t tics_dropped;       /* tics owed by wall time but skipped by the catch-up cap */
    uint64_t iterations;
    uint64_t frames_presented;
    uint64_t keys_delivered;
    uint64_t max_tics_in_iteration;
    uint64_t render_errors;
    fb_exit_reason exit_reason;
} fb_run_stats;

fb_status fb_adapter_run(fb_adapter *ad, const fb_producer *p, const fb_run_options *opts,
                         fb_run_stats *stats);

/* ---- counters ---- */
typedef struct fb_adapter_counters {
    uint64_t keys_observed, keys_dropped, resizes, capture_rotations, presents;
} fb_adapter_counters;
void fb_adapter_counters_get(const fb_adapter *ad, fb_adapter_counters *out);

/* ---- test-only mutants (negative controls). Never set outside a test. ---- */
#define FB_FAULT_STALE_MAP_ON_RESIZE   0x01u /* a resize updates the grid but keeps the old map */
#define FB_FAULT_RESIZE_STEPS_PRODUCER 0x02u /* a resize runs one extra producer tic */
#define FB_FAULT_FRAME_LOCKED_SIM      0x04u /* the loop runs exactly one tic per presented frame */
#define FB_FAULT_LEAK_PER_FRAME        0x08u /* each present leaks 64 KiB */
#define FB_FAULT_RETRY_ON_IO           0x10u /* GIBSON_ERR_IO from a poll is ignored (spins on hangup) */
#define FB_FAULT_STICKY_SIGNAL         0x20u /* any signal ever received ends every later adapter too */
#define FB_FAULT_SLOW_PRESENT          0x40u /* every 35th present stalls 400 ms (inside the timing; longer than
                                                the 10-tic catch-up cap, so a real-clock loop drops tics) */
void fb_adapter_set_faults(fb_adapter *ad, unsigned faults);

#ifdef __cplusplus
}
#endif

#endif /* FB_ADAPTER_H */
