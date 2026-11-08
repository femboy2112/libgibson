/*
 * LibGibson installed-SDK consumer for #297: bounding a headless context's capture.
 *
 * A headless context appends every rendered frame's ANSI bytes to an in-memory capture.
 * gibson_get_rendered_output only copies (a size query never drains), and the 0.6 C ABI has no
 * drain. A C program that renders forever therefore grows memory linearly. The consumer procedure
 * that bounds it without a new API:
 *
 *   1. after each gibson_render, size-query the capture (buf=NULL, cap=0);
 *   2. once it reaches a byte budget B, gibson_destroy_context and create a fresh headless
 *      context, re-apply settings (gibson_set_color_depth) and the root node, and keep rendering.
 *
 * gibson_set_root_node takes ownership of the node and frees the previous root; the context owns
 * and frees its root on destroy. This program never gibson_node_free's a node it handed over.
 *
 * For each grid (40x15, 80x24, 240x80) and workload (FIXED: identical frames; CHANGING: every cell
 * changes every frame) it runs a CONTROL (one context, no replacement: shows the hazard) and a
 * PROCEDURE (replacement at B = 1 MiB), prints one line per case and ends with
 * `headless_capture_lifetime: OK` only if every assertion held (otherwise the failing assertion
 * and exit 1). No wall-clock assertions. Linux only (VmRSS/VmHWM from /proc/self/status).
 *
 * Test knobs (environment):
 *   GIBSON_LIFETIME_SCALE=N     divide budget and frame counts by N (valgrind runs); RSS-bound
 *                               assertions are skipped when N != 1.
 *   GIBSON_LIFETIME_NO_REPLACE=1  NEGATIVE CONTROL ONLY: disable the replacement so the
 *                               PROCEDURE assertions must fail.
 */
#define _POSIX_C_SOURCE 200809L
#include <gibson.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define BUDGET_BYTES ((size_t)1 << 20)
#define RSS_SLACK_KIB (8 * 1024)
#define FIXED_PROBE_CAP 2000u

static int g_failed = 0;
static unsigned g_scale = 1;
static int g_no_replace = 0;

static void check(int ok, const char *what, const char *tag) {
    if (!ok) {
        fprintf(stderr, "headless_capture_lifetime: FAIL [%s]: %s\n", tag, what);
        g_failed = 1;
    }
}

static long proc_status_kib(const char *key) {
    FILE *f = fopen("/proc/self/status", "r");
    char line[256];
    long v = -1;
    size_t kl = strlen(key);
    if (!f) return -1;
    while (fgets(line, sizeof line, f)) {
        if (strncmp(line, key, kl) == 0 && line[kl] == ':') {
            v = strtol(line + kl + 1, NULL, 10);
            break;
        }
    }
    fclose(f);
    return v;
}

typedef struct {
    uint16_t cols, rows;
    int changing;
    char *text; /* rows lines of cols chars, '\n'-separated */
    size_t text_len;
} workload_t;

/* Cell i of frame f is 'a' + (i + f) % 26; the FIXED workload pins f to 0. */
static char cell_char(const workload_t *w, size_t i, unsigned long f) {
    return (char)('a' + (i + (w->changing ? f : 0)) % 26);
}

static void fill_text(workload_t *w, unsigned long f) {
    size_t p = 0;
    for (uint16_t y = 0; y < w->rows; y++) {
        for (uint16_t x = 0; x < w->cols; x++)
            w->text[p++] = cell_char(w, (size_t)y * w->cols + x, f);
        if (y + 1 < w->rows) w->text[p++] = '\n';
    }
    w->text[p] = '\0';
    w->text_len = p;
}

static gibson_status_t open_ctx(const workload_t *w, gibson_context_t **out) {
    gibson_status_t st = gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, w->cols, w->rows, out);
    if (st != GIBSON_OK) return st;
    return gibson_set_color_depth(*out, GIBSON_COLOR_DEPTH_TRUECOLOR);
}

/* Build this frame's tree, hand it to the context (which frees the previous root), render. */
static gibson_status_t draw(gibson_context_t *ctx, workload_t *w, unsigned long f) {
    gibson_node_t *n = NULL;
    gibson_status_t st;
    fill_text(w, f);
    st = gibson_node_text(w->text, NULL, GIBSON_WRAP_NONE, &n);
    if (st != GIBSON_OK) return st;
    st = gibson_set_root_node(ctx, n); /* takes ownership of n */
    if (st != GIBSON_OK) return st;
    return gibson_render(ctx);
}

static size_t cap_len(gibson_context_t *ctx) {
    size_t len = (size_t)-1;
    if (gibson_get_rendered_output(ctx, NULL, 0, &len) != GIBSON_OK) return (size_t)-1;
    return len;
}

/* Frame size is cols x rows and cell (0,0) holds the expected glyph for frame f. */
static int readback_ok(gibson_context_t *ctx, const workload_t *w, unsigned long f) {
    uint16_t c = 0, r = 0;
    gibson_cell_t cell;
    if (gibson_get_frame_size(ctx, &c, &r) != GIBSON_OK) return 0;
    if (c != w->cols || r != w->rows) return 0;
    if (gibson_get_frame_cell(ctx, 0, 0, &cell) != GIBSON_OK) return 0;
    return cell.glyph_len == 1 && cell.glyph[0] == (uint8_t)cell_char(w, 0, f);
}

int main(void) {
    static const struct { uint16_t cols, rows; unsigned n_ctl; } grids[] = {
        {40, 15, 3000}, {80, 24, 2000}, {240, 80, 400},
    };
    const char *e;
    size_t budget;
    if ((e = getenv("GIBSON_LIFETIME_SCALE")) && atoi(e) > 0) g_scale = (unsigned)atoi(e);
    if ((e = getenv("GIBSON_LIFETIME_NO_REPLACE")) && atoi(e) == 1) g_no_replace = 1;
    budget = BUDGET_BYTES / g_scale;

    if (gibson_abi_version() != GIBSON_ABI_VERSION) {
        fprintf(stderr, "headless_capture_lifetime: FAIL: ABI mismatch\n");
        return 1;
    }
    printf("config: budget=%zu scale=%u no_replace=%d start_rss_kib=%ld\n", budget, g_scale,
           g_no_replace, proc_status_kib("VmRSS"));

    for (size_t gi = 0; gi < sizeof grids / sizeof grids[0]; gi++) {
        for (int changing = 0; changing <= 1; changing++) {
            workload_t w;
            char tag[64];
            size_t cells = (size_t)grids[gi].cols * grids[gi].rows;
            unsigned n_ctl = grids[gi].n_ctl / g_scale;
            if (n_ctl < 2) n_ctl = 2;
            w.cols = grids[gi].cols; w.rows = grids[gi].rows; w.changing = changing;
            w.text = malloc(cells + w.rows + 1);
            if (!w.text) { fprintf(stderr, "oom\n"); return 1; }
            snprintf(tag, sizeof tag, "%ux%u %s", w.cols, w.rows, changing ? "changing" : "fixed");

            /* ---- CONTROL: one context, no replacement ---- */
            {
                gibson_context_t *ctx = NULL;
                long rss0, rss1;
                size_t len, first = 0;
                char ctag[80];
                snprintf(ctag, sizeof ctag, "%s control", tag);
                check(open_ctx(&w, &ctx) == GIBSON_OK, "create control context", ctag);
                rss0 = proc_status_kib("VmRSS");
                for (unsigned i = 0; i < n_ctl && ctx; i++)
                {
                    if (draw(ctx, &w, i) != GIBSON_OK) { check(0, "control draw", ctag); break; }
                    if (i == 0) first = cap_len(ctx);
                }
                len = ctx ? cap_len(ctx) : 0;
                rss1 = proc_status_kib("VmRSS");
                if (changing)
                    check(len >= (size_t)n_ctl * (cells / 2), "capture >= N_ctl * cells/2", ctag);
                else
                    check(len > 0 && len != (size_t)-1, "capture non-empty", ctag);
                check(len < ((size_t)64 << 20), "control stays under 64 MiB", ctag);
                printf("CONTROL   %-14s frames=%u capture_bytes=%zu first_frame_bytes=%zu steady_bytes_per_frame=%.1f rss_growth_kib=%ld\n",
                       tag, n_ctl, len, first, n_ctl > 1 ? (double)(len - first) / (n_ctl - 1) : 0.0, rss1 - rss0);
                gibson_destroy_context(ctx);
            }

            /* ---- PROCEDURE: replace at the budget ---- */
            {
                gibson_context_t *ctx = NULL;
                unsigned long f_to_b = 0, n_proc, f;
                size_t len, peak = 0, max_delta = 0, prev = 0, nrepl = 0, bytes_total = 0;
                long rss_first = -1, rss_end, hwm;
                int reached = 0;
                char ptag[80];
                snprintf(ptag, sizeof ptag, "%s procedure", tag);

                /* probe: frames for one fresh context to reach the budget */
                check(open_ctx(&w, &ctx) == GIBSON_OK, "create probe context", ptag);
                for (f = 0; ctx && f < (changing ? 10000000ul : FIXED_PROBE_CAP); f++) {
                    if (draw(ctx, &w, f) != GIBSON_OK) { check(0, "probe draw", ptag); break; }
                    if (cap_len(ctx) >= budget) { f_to_b = f + 1; reached = 1; break; }
                }
                len = ctx ? cap_len(ctx) : 0;
                gibson_destroy_context(ctx);
                ctx = NULL;
                if (!reached) {
                    printf("PROBE     %-14s budget %zu not reached in %lu frames; capture=%zu bytes_per_frame=%.2f\n",
                           tag, budget, (unsigned long)FIXED_PROBE_CAP, len, (double)len / FIXED_PROBE_CAP);
                    check(!changing, "changing workload must reach the budget", ptag);
                    n_proc = FIXED_PROBE_CAP;
                } else {
                    n_proc = f_to_b * 10;
                }

                check(open_ctx(&w, &ctx) == GIBSON_OK, "create procedure context", ptag);
                for (f = 0; ctx && f < n_proc; f++) {
                    if (draw(ctx, &w, f) != GIBSON_OK) { check(0, "procedure draw", ptag); break; }
                    len = cap_len(ctx);
                    if (len > peak) peak = len;
                    if (len >= prev && len - prev > max_delta) max_delta = len - prev;
                    if (len >= budget && !g_no_replace) {
                        bytes_total += len;
                        gibson_destroy_context(ctx);
                        ctx = NULL;
                        {
                            gibson_status_t st = open_ctx(&w, &ctx);
                            check(st == GIBSON_OK, "replacement create returned GIBSON_OK", ptag);
                            if (st != GIBSON_OK) break;
                        }
                        /* the new context renders the next frame and reads back correctly */
                        if (draw(ctx, &w, f + 1) != GIBSON_OK) { check(0, "post-replacement draw", ptag); break; }
                        f++;
                        check(readback_ok(ctx, &w, f), "post-replacement frame size and cell (0,0)", ptag);
                        len = cap_len(ctx);
                        nrepl++;
                        if (nrepl == 1) rss_first = proc_status_kib("VmRSS");
                    }
                    prev = len;
                }
                if (ctx) check(readback_ok(ctx, &w, f ? f - 1 : 0), "final frame size and cell (0,0)", ptag);
                rss_end = proc_status_kib("VmRSS");
                hwm = proc_status_kib("VmHWM");
                bytes_total += len;

                check(peak <= budget + max_delta, "capture never exceeds budget + one frame", ptag);
                if (reached && changing) check(nrepl > 0, "replacements > 0", ptag);
                if (reached && !changing) check(nrepl > 0, "replacements > 0 (budget was reachable)", ptag);
                if (g_scale == 1 && reached && changing && !g_no_replace) {
                    /* The resident-memory claim needs both readings; a failed read fails closed. */
                    check(rss_first >= 0 && rss_end >= 0, "VmRSS readable from /proc/self/status", ptag);
                    if (rss_first >= 0 && rss_end >= 0)
                        check(rss_end - rss_first < (long)(2 * budget / 1024) + RSS_SLACK_KIB,
                              "RSS after run - RSS after first cycle < 2*B + 8 MiB", ptag);
                }
                printf("PROCEDURE %-14s frames=%lu frames_to_budget=%lu replacements=%zu peak_capture=%zu max_frame_bytes=%zu "
                       "total_bytes_rendered=%zu rss_after_first_cycle_kib=%ld rss_end_kib=%ld rss_delta_kib=%ld bound_kib=%ld vmhwm_kib=%ld\n",
                       tag, n_proc, f_to_b, nrepl, peak, max_delta, bytes_total, rss_first, rss_end,
                       rss_first >= 0 ? rss_end - rss_first : 0L, (long)(2 * budget / 1024) + RSS_SLACK_KIB, hwm);
                gibson_destroy_context(ctx);
            }
            free(w.text);
        }
    }

    if (g_failed) return 1;
    printf("headless_capture_lifetime: OK\n");
    return 0;
}
