/* SPDX-License-Identifier: MIT OR Apache-2.0
 *
 * fb_adapter: a generic software-framebuffer adapter over the LibGibson public C ABI.
 * See fb_adapter.h for the contract and the declared scaling rule. Built only against the
 * installed SDK (<gibson.h> via pkg-config); no repository paths, no private API.
 */
#define _POSIX_C_SOURCE 200809L
#include "fb_adapter.h"

#include <signal.h>
#include <stdatomic.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#define FB_HOLD_SLOTS 64
#define FB_KEYQ 256
#define FB_STATUS_CAP 512
#define FB_LEAK_BYTES (64u * 1024u)
#define U2580 "\xe2\x96\x80"

typedef struct hold_slot {
    uint32_t code;
    uint8_t used;
    uint8_t released; /* the terminal reported the release: the end is not inferred */
    uint64_t started; /* tic of the press that started the hold */
    uint64_t ends_at; /* first tic at which the key is no longer held (UINT64_MAX: at its release) */
} hold_slot;

struct fb_adapter {
    fb_adapter_config cfg;
    gibson_context_t *ctx;
    int headless;
    uint16_t cols, rows; /* the grid as last reported (resize / frame size) */

    /* the last submitted frame (adapter-owned copy) */
    int format; /* 0 none, 1 indexed, 2 argb32 */
    uint8_t *idx;
    uint32_t *argb;
    uint32_t pal32[256];
    char status[FB_STATUS_CAP];

    /* the sampling map for geo.cols x geo.rows */
    fb_geometry geo;
    uint32_t *map_x;            /* img_cols source x */
    uint32_t *map_top, *map_bot;/* img_rows source row offsets (sy * src_w) */
    size_t cap_x, cap_y;

    /* lowering scratch */
    char *runbuf;
    size_t runcap;
    char *spaces;
    size_t spacecap;

    /* input */
    hold_slot holds[FB_HOLD_SLOTS];
    fb_key_event q[FB_KEYQ];
    unsigned qhead, qlen;
    int quit, gone, resized_flag;
    int releases; /* the terminal confirmed key releases (#275) */
    unsigned sig_gen_at_open; /* g_signal_gen when this adapter opened */

    uint64_t t_open_ns;
    uint64_t last_lower_ns, last_render_ns;
    fb_adapter_counters ctr;
    unsigned faults;
    uint64_t fault_extra_tics; /* FB_FAULT_RESIZE_STEPS_PRODUCER: producer tics owed to resizes */
    void **leaks;
    size_t nleaks, leakcap;
    int owns_signals;
};

/* ---------------------------------------------------------------- errors / time / signals */

static char g_err[512];

static void set_err(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(g_err, sizeof g_err, fmt, ap);
    va_end(ap);
}

static void set_gibson_err(const char *what, gibson_status_t st) {
    char lib[256];
    lib[0] = '\0';
    if (gibson_last_error_message(lib, sizeof lib) < 0) lib[0] = '\0';
    set_err("%s failed (status %d): %s", what, (int)st, lib);
}

const char *fb_adapter_last_error(void) { return g_err; }

static uint64_t mono_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
}

static uint64_t mono_clock(void *user) {
    (void)user;
    return mono_ns();
}

/* floor(ns * hz / 1e9), exact for any ns */
static uint64_t tics_for_ns(uint64_t ns, uint32_t hz) {
    return (ns / 1000000000ull) * hz + ((ns % 1000000000ull) * hz) / 1000000000ull;
}

/* ceil(n * 1e9 / hz), exact */
static uint64_t ns_for_tics(uint64_t n, uint32_t hz) {
    return (n / hz) * 1000000000ull + (((n % hz) * 1000000000ull) + hz - 1) / hz;
}

/* The handler stores the signal and bumps a lock-free C11 atomic generation counter, nothing else.
   sa_mask stops it nesting on one thread, but handlers can run concurrently on different threads,
   so the counter is incremented with an atomic fetch-add (a plain read-then-write could lose a
   fresh signal across an adapter reopen; #299 review). Lock-free atomics are async-signal-safe.
   An adapter is signalled when the generation moved past the value it saw at open, so a signal
   that ended one adapter never ends a later one. */
_Static_assert(ATOMIC_INT_LOCK_FREE == 2, "the signal generation counter must be lock-free");
static volatile sig_atomic_t g_signal; /* the last stop signal received */
static atomic_uint g_signal_gen;       /* stop signals received so far */
static int g_signals_owned;
static struct sigaction g_old_int, g_old_term;

static void on_stop_signal(int sig) {
    g_signal = sig;
    atomic_fetch_add_explicit(&g_signal_gen, 1u, memory_order_relaxed);
}

static void install_signals(void) {
    struct sigaction sa;
    memset(&sa, 0, sizeof sa);
    sa.sa_handler = on_stop_signal;
    sigemptyset(&sa.sa_mask);
    sigaddset(&sa.sa_mask, SIGINT);
    sigaddset(&sa.sa_mask, SIGTERM);
    sa.sa_flags = SA_RESTART;
    sigaction(SIGINT, &sa, &g_old_int);
    sigaction(SIGTERM, &sa, &g_old_term);
    g_signals_owned = 1;
}

static void uninstall_signals(void) {
    if (!g_signals_owned) return;
    sigaction(SIGINT, &g_old_int, NULL);
    sigaction(SIGTERM, &g_old_term, NULL);
    g_signals_owned = 0;
}

int fb_adapter_signal_received(void) { return (int)g_signal; }

int fb_adapter_signal(const fb_adapter *ad) {
    if (ad->faults & FB_FAULT_STICKY_SIGNAL) return (int)g_signal; /* mutant: the old global flag */
    return atomic_load_explicit(&g_signal_gen, memory_order_relaxed) != ad->sig_gen_at_open ? (int)g_signal : 0;
}

/* ---------------------------------------------------------------- config */

void fb_adapter_config_defaults(fb_adapter_config *cfg, uint16_t src_w, uint16_t src_h) {
    memset(cfg, 0, sizeof *cfg);
    cfg->src_w = src_w;
    cfg->src_h = src_h;
    cfg->par_num = 1;
    cfg->par_den = 1;
    cfg->cell_hw_num = 2;
    cfg->cell_hw_den = 1;
    cfg->status_rows = 0;
    cfg->tic_hz = 35;
    cfg->max_catchup_tics = 10;
    cfg->hold_grace_tics = 25;
    cfg->hold_repeat_tics = 4;
    cfg->quit_char = 'q';
    cfg->color_depth = -1;
    cfg->headless_capture_limit = 4u * 1024u * 1024u;
    cfg->install_signal_handlers = 1;
    cfg->key_releases = 1;
}

void fb_run_options_defaults(fb_run_options *o) {
    memset(o, 0, sizeof *o);
    o->present_divisor = 1;
    o->block_on_input = 1;
}

static int config_ok(const fb_adapter_config *c) {
    if (!c) return 0;
    if (c->src_w == 0 || c->src_h == 0 || c->src_w > 4096 || c->src_h > 4096) return 0;
    if (!c->par_num || !c->par_den || !c->cell_hw_num || !c->cell_hw_den) return 0;
    if (c->status_rows > 1) return 0;
    if (c->tic_hz == 0 || c->tic_hz > 1000) return 0;
    if (c->max_catchup_tics == 0 || c->hold_grace_tics == 0 || c->hold_repeat_tics == 0) return 0;
    return 1;
}

/* ---------------------------------------------------------------- the sampling map */

static int grow_u32(uint32_t **p, size_t *cap, size_t need) {
    if (need <= *cap) return 1;
    uint32_t *n = realloc(*p, need * sizeof **p);
    if (!n) return 0;
    *p = n;
    *cap = need;
    return 1;
}

/* Builds geometry + map for cols x rows by the rule in fb_adapter.h. */
static fb_status build_map(fb_adapter *ad, uint16_t cols, uint16_t rows) {
    const fb_adapter_config *c = &ad->cfg;
    fb_geometry g;
    memset(&g, 0, sizeof g);
    g.cols = cols;
    g.rows = rows;
    g.map_builds = ad->geo.map_builds + 1;
    uint32_t area_rows = rows > c->status_rows ? (uint32_t)rows - c->status_rows : 0;
    if (cols > 0 && area_rows > 0) {
        const uint64_t VW = cols, VH = 2ull * area_rows;
        const uint64_t P = (uint64_t)c->src_w * c->par_num * c->cell_hw_num;
        const uint64_t Q = (uint64_t)c->src_h * c->par_den * 2u * c->cell_hw_den;
        uint64_t dw, dh;
        if (VW * Q <= VH * P) {
            dw = VW;
            dh = (2 * VW * Q + P) / (2 * P);
            if (dh > VH) dh = VH;
            dh &= ~(uint64_t)1;
            if (dh < 2) dh = 2;
        } else {
            dh = VH;
            dw = (2 * VH * P + Q) / (2 * Q);
            if (dw > VW) dw = VW;
            if (dw < 1) dw = 1;
        }
        g.dw = (uint32_t)dw;
        g.dh = (uint32_t)dh;
        g.img_cols = (uint16_t)dw;
        g.img_rows = (uint16_t)(dh / 2);
        g.img_x = (uint16_t)((cols - g.img_cols) / 2);
        g.img_y = (uint16_t)((area_rows - g.img_rows) / 2);
        if (!grow_u32(&ad->map_x, &ad->cap_x, g.img_cols) || !grow_u32(&ad->map_top, &ad->cap_y, g.img_rows)) {
            set_err("out of memory building the sampling map");
            return FB_ERR_NOMEM;
        }
        size_t capb = ad->cap_y > g.img_rows ? ad->cap_y : g.img_rows;
        uint32_t *nb = realloc(ad->map_bot, capb * sizeof *nb);
        if (!nb) {
            set_err("out of memory building the sampling map");
            return FB_ERR_NOMEM;
        }
        ad->map_bot = nb;
        for (uint32_t i = 0; i < g.img_cols; i++)
            ad->map_x[i] = (uint32_t)(((2ull * i + 1) * c->src_w) / (2ull * dw));
        for (uint32_t r = 0; r < g.img_rows; r++) {
            uint64_t yt = ((2ull * (2 * r) + 1) * c->src_h) / (2ull * dh);
            uint64_t yb = ((2ull * (2 * r + 1) + 1) * c->src_h) / (2ull * dh);
            ad->map_top[r] = (uint32_t)(yt * c->src_w);
            ad->map_bot[r] = (uint32_t)(yb * c->src_w);
        }
    }
    /* lowering scratch for this width */
    size_t need_run = (size_t)cols * 3 + 1, need_sp = (size_t)cols + 1;
    if (need_run > ad->runcap) {
        char *n = realloc(ad->runbuf, need_run);
        if (!n) return FB_ERR_NOMEM;
        ad->runbuf = n;
        ad->runcap = need_run;
    }
    if (need_sp > ad->spacecap) {
        char *n = realloc(ad->spaces, need_sp);
        if (!n) return FB_ERR_NOMEM;
        ad->spaces = n;
        ad->spacecap = need_sp;
    }
    memset(ad->spaces, ' ', need_sp - 1);
    ad->spaces[need_sp - 1] = '\0';
    ad->geo = g;
    return FB_OK;
}

static fb_status set_grid(fb_adapter *ad, uint16_t cols, uint16_t rows) {
    if ((ad->faults & FB_FAULT_RESIZE_STEPS_PRODUCER) && ad->geo.map_builds > 0) ad->fault_extra_tics++;
    ad->cols = cols;
    ad->rows = rows;
    if (ad->faults & FB_FAULT_STALE_MAP_ON_RESIZE) return FB_OK; /* mutant: keep the old map */
    return build_map(ad, cols, rows);
}

void fb_adapter_geometry(const fb_adapter *ad, fb_geometry *out) { *out = ad->geo; }

/* ---------------------------------------------------------------- open / close */

static fb_adapter *alloc_adapter(const fb_adapter_config *cfg) {
    fb_adapter *ad = calloc(1, sizeof *ad);
    if (!ad) return NULL;
    ad->cfg = *cfg;
    size_t n = (size_t)cfg->src_w * cfg->src_h;
    ad->idx = calloc(n, 1);
    ad->argb = calloc(n, sizeof *ad->argb);
    if (!ad->idx || !ad->argb) {
        free(ad->idx);
        free(ad->argb);
        free(ad);
        return NULL;
    }
    ad->t_open_ns = mono_ns();
    ad->sig_gen_at_open = atomic_load_explicit(&g_signal_gen, memory_order_relaxed);
    return ad;
}

static void free_adapter(fb_adapter *ad) {
    free(ad->leaks); /* the blocks it pointed to stay leaked: that is the mutant */
    free(ad->idx);
    free(ad->argb);
    free(ad->map_x);
    free(ad->map_top);
    free(ad->map_bot);
    free(ad->runbuf);
    free(ad->spaces);
    free(ad);
}

fb_status fb_adapter_open_headless(const fb_adapter_config *cfg, uint16_t cols, uint16_t rows, fb_adapter **out) {
    if (!out || !config_ok(cfg) || cols == 0 || rows == 0) {
        set_err("fb_adapter_open_headless: invalid configuration or geometry");
        return FB_ERR_PARAM;
    }
    *out = NULL;
    fb_adapter *ad = alloc_adapter(cfg);
    if (!ad) {
        set_err("out of memory");
        return FB_ERR_NOMEM;
    }
    ad->headless = 1;
    gibson_status_t st = gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, cols, rows, &ad->ctx);
    if (st != GIBSON_OK) {
        set_gibson_err("gibson_create_headless_context", st);
        free_adapter(ad);
        return FB_ERR_GIBSON;
    }
    if (cfg->color_depth >= 0 && (st = gibson_set_color_depth(ad->ctx, cfg->color_depth)) != GIBSON_OK) {
        set_gibson_err("gibson_set_color_depth", st);
        gibson_destroy_context(ad->ctx);
        free_adapter(ad);
        return FB_ERR_GIBSON;
    }
    fb_status fs = set_grid(ad, cols, rows);
    if (fs != FB_OK) {
        gibson_destroy_context(ad->ctx);
        free_adapter(ad);
        return fs;
    }
    *out = ad;
    return FB_OK;
}

fb_status fb_adapter_open_terminal(const fb_adapter_config *cfg, fb_adapter **out) {
    if (!out || !config_ok(cfg)) {
        set_err("fb_adapter_open_terminal: invalid configuration");
        return FB_ERR_PARAM;
    }
    *out = NULL;
    if (!isatty(STDOUT_FILENO)) {
        set_err("stdout is not a terminal");
        return FB_ERR_PARAM;
    }
    fb_adapter *ad = alloc_adapter(cfg);
    if (!ad) {
        set_err("out of memory");
        return FB_ERR_NOMEM;
    }
    /* Handlers first, so a SIGTERM that arrives while the terminal is being taken over is
       recorded and honoured by the loop rather than killing the process mid-change. */
    if (cfg->install_signal_handlers && !g_signals_owned) {
        install_signals();
        ad->owns_signals = 1;
    }
    gibson_status_t st = gibson_create_context(GIBSON_MODE_FULLSCREEN, &ad->ctx);
    if (st != GIBSON_OK) {
        set_gibson_err("gibson_create_context", st);
        if (ad->owns_signals) uninstall_signals();
        free_adapter(ad);
        return FB_ERR_GIBSON;
    }
    if (cfg->color_depth >= 0) (void)gibson_set_color_depth(ad->ctx, cfg->color_depth);
    /* Key releases (#275): the answer arrives through the pump; until then holds are inferred.
       A failed write leaves the adapter press-only, as on a terminal without the protocol. */
    if (cfg->key_releases) (void)gibson_set_key_release_reporting(ad->ctx, 1);
    /* The grid: render an empty frame and read its size back (pure ABI). */
    uint16_t cols = 0, rows = 0;
    gibson_node_t *empty = NULL;
    if (gibson_node_box_col(&empty) == GIBSON_OK && gibson_set_root_node(ad->ctx, empty) == GIBSON_OK &&
        gibson_render(ad->ctx) == GIBSON_OK)
        (void)gibson_get_frame_size(ad->ctx, &cols, &rows);
    if (cols == 0 || rows == 0) {
        cols = 80;
        rows = 24;
    }
    fb_status fs = set_grid(ad, cols, rows);
    if (fs != FB_OK) {
        gibson_destroy_context(ad->ctx);
        if (ad->owns_signals) uninstall_signals();
        free_adapter(ad);
        return fs;
    }
    *out = ad;
    return FB_OK;
}

void fb_adapter_close(fb_adapter *ad) {
    if (!ad) return;
    if (ad->ctx) gibson_destroy_context(ad->ctx);
    ad->ctx = NULL;
    if (ad->owns_signals) uninstall_signals();
    free_adapter(ad);
}

gibson_context_t *fb_adapter_context(const fb_adapter *ad) { return ad->ctx; }

void fb_adapter_set_faults(fb_adapter *ad, unsigned faults) { ad->faults = faults; }

void fb_adapter_counters_get(const fb_adapter *ad, fb_adapter_counters *out) { *out = ad->ctr; }

void fb_adapter_last_present_ns(const fb_adapter *ad, uint64_t *lower_ns, uint64_t *render_ns) {
    if (lower_ns) *lower_ns = ad->last_lower_ns;
    if (render_ns) *render_ns = ad->last_render_ns;
}

uint32_t fb_adapter_ticks_ms(const fb_adapter *ad) { return (uint32_t)((mono_ns() - ad->t_open_ns) / 1000000ull); }

uint64_t fb_adapter_wall_tic(const fb_adapter *ad) { return tics_for_ns(mono_ns() - ad->t_open_ns, ad->cfg.tic_hz); }

int fb_adapter_quit_requested(const fb_adapter *ad) { return ad->quit || fb_adapter_signal(ad) != 0; }

/* ---------------------------------------------------------------- submission */

fb_status fb_adapter_submit_indexed(fb_adapter *ad, const uint8_t *pixels, size_t pitch, const uint8_t palette_rgb[768]) {
    if (!ad || !pixels || !palette_rgb || pitch < ad->cfg.src_w) {
        set_err("fb_adapter_submit_indexed: invalid argument");
        return FB_ERR_PARAM;
    }
    const size_t w = ad->cfg.src_w;
    for (size_t y = 0; y < ad->cfg.src_h; y++) memcpy(ad->idx + y * w, pixels + y * pitch, w);
    for (int i = 0; i < 256; i++)
        ad->pal32[i] = ((uint32_t)palette_rgb[3 * i] << 16) | ((uint32_t)palette_rgb[3 * i + 1] << 8) | palette_rgb[3 * i + 2];
    ad->format = 1;
    return FB_OK;
}

fb_status fb_adapter_submit_argb32(fb_adapter *ad, const uint32_t *pixels, size_t pitch_px) {
    if (!ad || !pixels || pitch_px < ad->cfg.src_w) {
        set_err("fb_adapter_submit_argb32: invalid argument");
        return FB_ERR_PARAM;
    }
    const size_t w = ad->cfg.src_w;
    for (size_t y = 0; y < ad->cfg.src_h; y++)
        for (size_t x = 0; x < w; x++) ad->argb[y * w + x] = pixels[y * pitch_px + x] & 0x00FFFFFFu;
    ad->format = 2;
    return FB_OK;
}

fb_status fb_adapter_set_status(fb_adapter *ad, const char *ascii) {
    if (!ad || !ascii) return FB_ERR_PARAM;
    size_t i = 0;
    for (; ascii[i] && i + 1 < FB_STATUS_CAP; i++) {
        unsigned char ch = (unsigned char)ascii[i];
        ad->status[i] = (ch >= 0x20 && ch < 0x7f) ? (char)ch : '?';
    }
    ad->status[i] = '\0';
    return FB_OK;
}

/* ---------------------------------------------------------------- lowering + present */

static gibson_style_t style_rgb(uint32_t fg, uint32_t bg) {
    gibson_style_t s;
    memset(&s, 0, sizeof s);
    s.fg.color_type = GIBSON_COLOR_RGB;
    s.fg.r = (uint8_t)(fg >> 16);
    s.fg.g = (uint8_t)(fg >> 8);
    s.fg.b = (uint8_t)fg;
    s.bg.color_type = GIBSON_COLOR_RGB;
    s.bg.r = (uint8_t)(bg >> 16);
    s.bg.g = (uint8_t)(bg >> 8);
    s.bg.b = (uint8_t)bg;
    return s;
}

static int add_span(gibson_line_t *line, const char *text, uint32_t fg, uint32_t bg) {
    gibson_style_t s = style_rgb(fg, bg);
    return gibson_line_add_span(line, text, &s) == GIBSON_OK;
}

/* Adds n bar cells (spaces) to the line. */
static int add_bar(fb_adapter *ad, gibson_line_t *line, size_t n, uint32_t bar) {
    if (n == 0) return 1;
    const char saved = ad->spaces[n]; /* n <= cols < spacecap */
    ad->spaces[n] = '\0';
    int ok = add_span(line, ad->spaces, bar, bar);
    ad->spaces[n] = saved;
    return ok;
}

static fb_status lower(fb_adapter *ad, gibson_node_t **out) {
    const fb_geometry *g = &ad->geo;
    const uint32_t bar = ((uint32_t)ad->cfg.bar_rgb[0] << 16) | ((uint32_t)ad->cfg.bar_rgb[1] << 8) | ad->cfg.bar_rgb[2];
    gibson_rich_text_t *rich = NULL;
    if (gibson_rich_text_new(&rich) != GIBSON_OK) {
        set_err("gibson_rich_text_new failed");
        return FB_ERR_GIBSON;
    }
    const int have = ad->format != 0;
    for (uint32_t row = 0; row < g->rows; row++) {
        gibson_line_t *line = NULL;
        if (gibson_line_new(&line) != GIBSON_OK) goto fail_rich;
        int ok = 1;
        if (ad->cfg.status_rows && row == (uint32_t)g->rows - 1u) {
            size_t n = strlen(ad->status);
            if (n > g->cols) n = g->cols;
            char *buf = ad->runbuf; /* cols*3+1 >= cols+1 */
            memcpy(buf, ad->status, n);
            memset(buf + n, ' ', (size_t)g->cols - n);
            buf[g->cols] = '\0';
            ok = add_span(line, buf, 0xE6E6F0u, 0x181828u);
        } else if (have && row >= g->img_y && row < (uint32_t)g->img_y + g->img_rows) {
            const uint32_t r = row - g->img_y;
            const uint32_t top = ad->map_top[r], bot = ad->map_bot[r];
            ok = add_bar(ad, line, g->img_x, bar);
            size_t n = 0;
            uint64_t cur = 0;
            for (uint32_t i = 0; ok && i < g->img_cols; i++) {
                const uint32_t sx = ad->map_x[i];
                uint32_t t, b;
                if (ad->format == 1) {
                    t = ad->pal32[ad->idx[top + sx]];
                    b = ad->pal32[ad->idx[bot + sx]];
                } else {
                    t = ad->argb[top + sx];
                    b = ad->argb[bot + sx];
                }
                const uint64_t key = ((uint64_t)t << 32) | b;
                if (n && key != cur) {
                    ad->runbuf[n * 3] = '\0';
                    ok = add_span(line, ad->runbuf, (uint32_t)(cur >> 32), (uint32_t)cur);
                    n = 0;
                }
                cur = key;
                memcpy(ad->runbuf + n * 3, U2580, 3);
                n++;
            }
            if (ok && n) {
                ad->runbuf[n * 3] = '\0';
                ok = add_span(line, ad->runbuf, (uint32_t)(cur >> 32), (uint32_t)cur);
            }
            if (ok) ok = add_bar(ad, line, (size_t)g->cols - g->img_x - g->img_cols, bar);
        } else {
            ok = add_bar(ad, line, g->cols, bar);
        }
        gibson_status_t st = ok ? gibson_rich_text_add_line(rich, line) : GIBSON_ERR_INVALID_PARAM;
        gibson_line_free(line);
        if (st != GIBSON_OK) goto fail_rich;
    }
    gibson_node_t *node = NULL;
    gibson_status_t st = gibson_node_rich_text(rich, GIBSON_WRAP_NONE, &node);
    gibson_rich_text_free(rich);
    if (st != GIBSON_OK) {
        set_gibson_err("gibson_node_rich_text", st);
        return FB_ERR_GIBSON;
    }
    *out = node;
    return FB_OK;
fail_rich:
    set_err("building the styled spans failed");
    gibson_rich_text_free(rich);
    return FB_ERR_GIBSON;
}

/* Headless only: replaces the context once its capture exceeds the limit (finding F1: the C ABI
   cannot drain a headless context's capture, so it grows by every frame's bytes). */
static fb_status rotate_capture_if_needed(fb_adapter *ad) {
    if (!ad->headless || ad->cfg.headless_capture_limit == 0) return FB_OK;
    size_t len = 0;
    if (gibson_get_rendered_output(ad->ctx, NULL, 0, &len) != GIBSON_OK || len <= ad->cfg.headless_capture_limit)
        return FB_OK;
    gibson_context_t *n = NULL;
    gibson_status_t st = gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, ad->cols, ad->rows, &n);
    if (st != GIBSON_OK) {
        set_gibson_err("gibson_create_headless_context (capture rotation)", st);
        return FB_ERR_GIBSON;
    }
    if (ad->cfg.color_depth >= 0) (void)gibson_set_color_depth(n, ad->cfg.color_depth);
    /* carry undelivered injected events over, in order */
    for (int i = 0; i < GIBSON_EVENT_QUEUE_CAPACITY; i++) {
        gibson_event_t ev;
        uint32_t kind = 0;
        if (gibson_poll_event_kind(ad->ctx, 0, &ev, &kind) != GIBSON_OK || ev.event_type == GIBSON_EVENT_NONE) break;
        (void)gibson_push_event_kind(n, &ev, kind);
    }
    gibson_destroy_context(ad->ctx);
    ad->ctx = n;
    ad->ctr.capture_rotations++;
    return FB_OK;
}

fb_status fb_adapter_present(fb_adapter *ad) {
    if (!ad) return FB_ERR_PARAM;
    fb_status fs = rotate_capture_if_needed(ad);
    if (fs != FB_OK) return fs;
    const uint64_t t0 = mono_ns();
    gibson_node_t *node = NULL;
    fs = lower(ad, &node);
    if (fs != FB_OK) return fs;
    if ((ad->faults & FB_FAULT_SLOW_PRESENT) && ad->ctr.presents % 35 == 34) {
        struct timespec slow = {0, 400000000L}; /* mutant: 400 ms inside the measured window */
        nanosleep(&slow, NULL);
    }
    gibson_status_t st = gibson_set_root_node(ad->ctx, node);
    if (st != GIBSON_OK) {
        gibson_node_free(node);
        set_gibson_err("gibson_set_root_node", st);
        return FB_ERR_GIBSON;
    }
    const uint64_t t1 = mono_ns();
    st = gibson_render(ad->ctx);
    const uint64_t t2 = mono_ns();
    ad->last_lower_ns = t1 - t0;
    ad->last_render_ns = t2 - t1;
    ad->ctr.presents++;
    if (ad->faults & FB_FAULT_LEAK_PER_FRAME) {
        if (ad->nleaks == ad->leakcap) {
            size_t nc = ad->leakcap ? ad->leakcap * 2 : 64;
            void **nl = realloc(ad->leaks, nc * sizeof *nl);
            if (nl) {
                ad->leaks = nl;
                ad->leakcap = nc;
            }
        }
        void *p = malloc(FB_LEAK_BYTES);
        if (p) {
            memset(p, 0xA5, FB_LEAK_BYTES);
            if (ad->nleaks < ad->leakcap) ad->leaks[ad->nleaks++] = p;
        }
    }
    if (st != GIBSON_OK) {
        set_gibson_err("gibson_render", st);
        return FB_ERR_GIBSON;
    }
    if (!ad->headless && !(ad->faults & FB_FAULT_STALE_MAP_ON_RESIZE)) {
        uint16_t c = 0, r = 0;
        if (gibson_get_frame_size(ad->ctx, &c, &r) == GIBSON_OK && c && r && (c != ad->geo.cols || r != ad->geo.rows)) {
            fs = set_grid(ad, c, r);
            if (fs != FB_OK) return fs;
            ad->resized_flag = 1;
        }
    }
    return FB_OK;
}

/* ---------------------------------------------------------------- input */

static int q_push(fb_adapter *ad, const fb_key_event *e) {
    if (ad->qlen == FB_KEYQ) return 0;
    ad->q[(ad->qhead + ad->qlen) % FB_KEYQ] = *e;
    ad->qlen++;
    return 1;
}

/* Releases every hold that ended at or before `tic`, earliest end first. */
static void expire_holds(fb_adapter *ad, uint64_t tic) {
    for (;;) {
        int best = -1;
        for (int i = 0; i < FB_HOLD_SLOTS; i++)
            if (ad->holds[i].used && ad->holds[i].ends_at <= tic &&
                (best < 0 || ad->holds[i].ends_at < ad->holds[best].ends_at))
                best = i;
        if (best < 0) return;
        fb_key_event e = {ad->holds[best].code, 0, ad->holds[best].released ? 0 : 1, ad->holds[best].ends_at};
        if (!q_push(ad, &e)) return; /* queue full: keep the hold; release it on a later call */
        ad->holds[best].used = 0;
    }
}

static int releases_on(const fb_adapter *ad) {
    return ad->releases && !(ad->faults & FB_FAULT_IGNORE_RELEASES);
}

static void observe_key(fb_adapter *ad, uint64_t tic, uint32_t code) {
    expire_holds(ad, tic);
    const int exact = releases_on(ad);
    int free_slot = -1;
    for (int i = 0; i < FB_HOLD_SLOTS; i++) {
        if (ad->holds[i].used && ad->holds[i].code == code) {
            /* an autorepeat re-arms; with reported releases the key is held until its release
               (pressed again in the tic it was released: down again) */
            ad->holds[i].released = 0;
            ad->holds[i].ends_at = exact ? UINT64_MAX : tic + ad->cfg.hold_repeat_tics;
            return;
        }
        if (!ad->holds[i].used && free_slot < 0) free_slot = i;
    }
    fb_key_event e = {code, 1, 0, tic};
    if (free_slot < 0 || !q_push(ad, &e)) {
        ad->ctr.keys_dropped++;
        return;
    }
    ad->holds[free_slot].used = 1;
    ad->holds[free_slot].released = 0;
    ad->holds[free_slot].code = code;
    ad->holds[free_slot].started = tic;
    ad->holds[free_slot].ends_at = exact ? UINT64_MAX : tic + ad->cfg.hold_grace_tics;
}

/* The terminal reported `code` going up at `tic` (#275): its hold ends at `tic`, or at tic + 1 if
   it started at `tic`, so every press is held at least one tic. Not held: nothing. */
static void release_key(fb_adapter *ad, uint64_t tic, uint32_t code) {
    expire_holds(ad, tic);
    if (ad->faults & FB_FAULT_IGNORE_RELEASES) return;
    for (int i = 0; i < FB_HOLD_SLOTS; i++) {
        if (ad->holds[i].used && ad->holds[i].code == code) {
            ad->holds[i].released = 1;
            ad->holds[i].ends_at = ad->holds[i].started < tic ? tic : tic + 1;
            return;
        }
    }
}

/* The terminal stopped reporting releases: no hold may wait for one. */
static void releases_withdrawn(fb_adapter *ad, uint64_t tic) {
    for (int i = 0; i < FB_HOLD_SLOTS; i++) {
        hold_slot *h = &ad->holds[i];
        if (h->used && !h->released && h->ends_at > tic + ad->cfg.hold_grace_tics)
            h->ends_at = tic + ad->cfg.hold_grace_tics;
    }
}

int fb_adapter_releases_reported(const fb_adapter *ad) { return ad && releases_on(ad); }

static uint32_t default_map(uint32_t code) {
    if (code >= 'A' && code <= 'Z') return code + ('a' - 'A');
    return code;
}

fb_status fb_adapter_pump(fb_adapter *ad, uint64_t tic, uint32_t timeout_ms) {
    if (!ad) return FB_ERR_PARAM;
    if (ad->gone) return FB_TERMINAL_GONE;
    uint32_t wait = timeout_ms;
    for (int n = 0; n < GIBSON_EVENT_QUEUE_CAPACITY; n++) {
        gibson_event_t ev;
        uint32_t kind = 0;
        gibson_status_t st = gibson_poll_event_kind(ad->ctx, wait, &ev, &kind);
        wait = 0;
        if (st == GIBSON_ERR_IO) {
            if (ad->faults & FB_FAULT_RETRY_ON_IO) return FB_OK; /* mutant: pretend nothing happened */
            set_gibson_err("gibson_poll_event_kind (terminal gone)", st);
            ad->gone = 1;
            return FB_TERMINAL_GONE;
        }
        if (st != GIBSON_OK) {
            set_gibson_err("gibson_poll_event_kind", st);
            return FB_ERR_GIBSON;
        }
        if (ev.event_type == GIBSON_EVENT_NONE) break;
        if (ev.event_type == GIBSON_EVENT_RESIZE) {
            if (ev.cols && ev.rows && (ev.cols != ad->cols || ev.rows != ad->rows)) {
                fb_status fs = set_grid(ad, ev.cols, ev.rows);
                if (fs != FB_OK) return fs;
                ad->ctr.resizes++;
                ad->resized_flag = 1;
            }
            continue;
        }
        if (ev.event_type == GIBSON_EVENT_KEY_RELEASES_ACTIVE ||
            ev.event_type == GIBSON_EVENT_KEY_RELEASES_UNSUPPORTED) {
            const int active = ev.event_type == GIBSON_EVENT_KEY_RELEASES_ACTIVE;
            if (ad->cfg.observer)
                ad->cfg.observer(ad->cfg.observer_user, tic,
                                 active ? FB_OBSERVED_RELEASES_ACTIVE : FB_OBSERVED_RELEASES_UNSUPPORTED, 0, 0);
            if (!active && ad->releases) releases_withdrawn(ad, tic);
            ad->releases = active;
            continue;
        }
        if (ev.event_type == GIBSON_EVENT_KEY_RELEASE) {
            if (ad->cfg.observer)
                ad->cfg.observer(ad->cfg.observer_user, tic, kind | FB_OBSERVED_RELEASE, ev.key_code, ev.modifiers);
            uint32_t fb_code = (kind == GIBSON_KEY_KIND_SPECIAL) ? FB_KEY_BASE + (ev.key_code - 0x1000u) : ev.key_code;
            uint32_t code = ad->cfg.map_key ? ad->cfg.map_key(ad->cfg.map_key_user, fb_code, ev.modifiers)
                                            : default_map(fb_code);
            if (code) release_key(ad, tic, code);
            continue;
        }
        if (ev.event_type != GIBSON_EVENT_KEY) continue; /* paste: not a key */
        ad->ctr.keys_observed++;
        if (ad->cfg.observer) ad->cfg.observer(ad->cfg.observer_user, tic, kind, ev.key_code, ev.modifiers);
        if (kind == GIBSON_KEY_KIND_CHAR) {
            if ((ev.modifiers & GIBSON_MOD_CONTROL) && (ev.key_code == 'c' || ev.key_code == 'C' || ev.key_code == 3)) {
                ad->quit = 1;
                continue;
            }
            if (ad->cfg.quit_char && ev.key_code == ad->cfg.quit_char && !(ev.modifiers & GIBSON_MOD_CONTROL)) {
                ad->quit = 1;
                continue;
            }
        }
        uint32_t fb_code = (kind == GIBSON_KEY_KIND_SPECIAL) ? FB_KEY_BASE + (ev.key_code - 0x1000u) : ev.key_code;
        uint32_t code = ad->cfg.map_key ? ad->cfg.map_key(ad->cfg.map_key_user, fb_code, ev.modifiers) : default_map(fb_code);
        if (code) observe_key(ad, tic, code);
    }
    return FB_OK;
}

int fb_adapter_next_key(fb_adapter *ad, uint64_t tic, fb_key_event *out) {
    expire_holds(ad, tic);
    if (ad->qlen == 0 || ad->q[ad->qhead].tic > tic) return 0;
    *out = ad->q[ad->qhead];
    ad->qhead = (ad->qhead + 1) % FB_KEYQ;
    ad->qlen--;
    return 1;
}

/* ---------------------------------------------------------------- the fixed-tic loop */

fb_status fb_adapter_run(fb_adapter *ad, const fb_producer *p, const fb_run_options *opts_in, fb_run_stats *stats) {
    if (!ad || !p || !p->tic || !stats) return FB_ERR_PARAM;
    fb_run_options o;
    if (opts_in) o = *opts_in;
    else fb_run_options_defaults(&o);
    memset(stats, 0, sizeof *stats);
    fb_clock_fn clock = o.clock ? o.clock : mono_clock;
    const uint32_t hz = ad->cfg.tic_hz;
    const uint64_t t0 = clock(o.clock_user);
    uint64_t accounted = 0, ticked_iters = 0, consecutive_render_errors = 0;
    fb_status rc = FB_OK;

    for (;;) {
        stats->iterations++;
        if (fb_adapter_signal(ad)) { stats->exit_reason = FB_EXIT_SIGNAL; break; }
        if (ad->quit) { stats->exit_reason = FB_EXIT_QUIT; break; }
        const uint64_t now = stats->iterations == 1 ? t0 : clock(o.clock_user);
        uint64_t due;
        int locked_present = 0;
        if (ad->faults & FB_FAULT_FRAME_LOCKED_SIM) {
            /* mutant: simulation advances once per presented frame, whatever the wall time */
            locked_present = o.present_divisor && (stats->iterations % o.present_divisor == 0);
            due = locked_present ? 1 : 0;
        } else {
            const uint64_t owed = tics_for_ns(now >= t0 ? now - t0 : 0, hz);
            due = owed > accounted ? owed - accounted : 0;
            uint64_t dropped = 0;
            if (due > ad->cfg.max_catchup_tics) {
                dropped = due - ad->cfg.max_catchup_tics;
                due = ad->cfg.max_catchup_tics;
            }
            stats->tics_dropped += dropped;
            accounted += due + dropped;
        }
        if (o.max_tics && stats->tics + due > o.max_tics) due = o.max_tics - stats->tics;

        uint64_t ran = 0;
        for (uint64_t k = 0; k < due; k++) {
            const uint64_t tic = stats->tics;
            if (o.pre_tic) o.pre_tic(o.hook_user, ad, tic);
            fb_status fs = fb_adapter_pump(ad, tic, 0);
            if (fs == FB_TERMINAL_GONE) { stats->exit_reason = FB_EXIT_HANGUP; goto out; }
            if (fs != FB_OK) { stats->exit_reason = FB_EXIT_ERROR; rc = fs; goto out; }
            if (ad->quit || fb_adapter_signal(ad)) break;
            for (; ad->fault_extra_tics; ad->fault_extra_tics--) p->tic(p->user); /* mutant only */
            fb_key_event ev;
            while (fb_adapter_next_key(ad, tic, &ev)) {
                if (p->key) p->key(p->user, ev.code, ev.pressed);
                stats->keys_delivered++;
            }
            p->tic(p->user);
            stats->tics++;
            ran++;
        }
        if (ran > stats->max_tics_in_iteration) stats->max_tics_in_iteration = ran;
        if (ad->quit || fb_adapter_signal(ad)) continue; /* the top of the loop records why */

        int present = 0;
        if (ad->faults & FB_FAULT_FRAME_LOCKED_SIM) {
            present = locked_present;
        } else if (ran) {
            ticked_iters++;
            present = o.present_divisor && (ticked_iters % o.present_divisor == 0);
        } else if (ad->resized_flag && o.present_divisor) {
            present = 1; /* repaint promptly at the new size */
        }
        if (present) {
            ad->resized_flag = 0;
            if (p->draw) {
                fb_status fs = p->draw(p->user, ad);
                if (fs != FB_OK) { stats->exit_reason = FB_EXIT_ERROR; rc = fs; goto out; }
            }
            fb_status fs = fb_adapter_present(ad);
            if (fs == FB_OK) {
                stats->frames_presented++;
                consecutive_render_errors = 0;
                if (o.on_present) o.on_present(o.hook_user, ad, stats->tics);
            } else {
                stats->render_errors++;
                if (++consecutive_render_errors > 50) { stats->exit_reason = FB_EXIT_ERROR; rc = fs; goto out; }
            }
        }
        if (o.max_tics && stats->tics >= o.max_tics) { stats->exit_reason = FB_EXIT_TIC_LIMIT; break; }
        if (o.max_iterations && stats->iterations >= o.max_iterations) { stats->exit_reason = FB_EXIT_NONE; break; }

        /* Wait for input until the next tic is due (only on the real clock). */
        uint32_t timeout_ms = 0;
        if (o.block_on_input && !o.clock) {
            const uint64_t next = t0 + ns_for_tics(accounted + 1, hz);
            const uint64_t n2 = mono_ns();
            if (next > n2) {
                uint64_t ms = (next - n2 + 999999ull) / 1000000ull;
                timeout_ms = ms > 1000 ? 1000 : (uint32_t)ms;
            }
        }
        fb_status fs = fb_adapter_pump(ad, stats->tics, timeout_ms);
        if (fs == FB_TERMINAL_GONE) { stats->exit_reason = FB_EXIT_HANGUP; break; }
        if (fs != FB_OK) { stats->exit_reason = FB_EXIT_ERROR; rc = fs; break; }
    }
out:
    return rc;
}
