/*
 * LibGibson installed-SDK consumer for #297: a headless context's capture stays bounded when the
 * consumer drains it.
 *
 * A headless context appends every rendered frame's ANSI bytes to an in-memory capture, and
 * gibson_get_rendered_output only copies. gibson_take_rendered_output (copy-and-clear) and
 * gibson_clear_rendered_output (discard) bound it with ONE context for the whole run, no
 * replacement.
 *
 * For each grid (40x15, 80x24, 240x80) and workload (FIXED: identical frames; CHANGING: every cell
 * changes every frame):
 *   CONTROL  one context, never drained, N frames; records each frame's capture growth and the
 *            frame at which the capture reached 1 MiB. N = 20 x that frame (CHANGING), or
 *            FIXED_FRAMES when a FIXED workload never reaches 1 MiB.
 *   TAKE     a fresh context, the same N frames; after each render: get (copy) and take must
 *            return identical bytes, equal in length to the control's growth for that frame, and
 *            the capture is 0 afterwards.
 *   CLEAR    the same with gibson_clear_rendered_output; the discarded count must equal the
 *            control's growth and the capture is 0 afterwards.
 * Memory: VmRSS after N frames minus VmRSS after the warm-up must stay under
 *   RSS_SLACK_KIB + 4 x (largest frame) / 1024.
 * The capture after a drain is 0, so the library holds at most one frame between drains; the
 * consumer's own take buffer holds one more. 4 x the largest frame covers both and the allocator's
 * growth headroom; RSS_SLACK_KIB (2 MiB) covers allocator noise. For CHANGING workloads the
 * control's capture over the same frames is >= 19 MiB (asserted), so an undrained run cannot pass.
 * A FIXED workload emits one full frame and then 0 bytes per identical frame, so it never reaches
 * 1 MiB; it runs FIXED_FRAMES frames and exercises the zero-byte drain.
 *
 * The control and each drained run execute in a separate child process (fork), so VmRSS of one run
 * is not masked by memory an earlier run freed but the allocator kept resident.
 *
 * Prints one line per case and ends with `headless_capture_drain: OK` only if every assertion
 * held (otherwise the failing assertions and exit 1). No wall-clock assertions. Linux only
 * (VmRSS from /proc/self/status).
 *
 * Test knob (environment):
 *   GIBSON_DRAIN_NO_DRAIN=1  NEGATIVE CONTROL ONLY: the TAKE and CLEAR runs skip the drain, so
 *                            their assertions must fail.
 */
#define _POSIX_C_SOURCE 200809L
#include <gibson.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#define MIB ((size_t)1 << 20)
#define FRAMES_FACTOR 20u
#define FIXED_FRAMES 3000u
#define RSS_SLACK_KIB 2048L

static int g_failed = 0;
static int g_no_drain = 0;

static void check(int ok, const char *what, const char *tag) {
    if (!ok) {
        fprintf(stderr, "headless_capture_drain: FAIL [%s]: %s\n", tag, what);
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
} workload_t;

/* Cell i of frame f is 'a' + (i + f) % 26; the FIXED workload pins f to 0. */
static char cell_char(const workload_t *w, size_t i, unsigned long f) {
    return (char)('a' + (i + (w->changing ? f : 0)) % 26);
}

static void fill_text(workload_t *w, unsigned long f) {
    size_t p = 0;
    for (uint16_t y = 0; y < w->rows; y++) {
        for (uint16_t x = 0; x < w->cols; x++) w->text[p++] = cell_char(w, (size_t)y * w->cols + x, f);
        if (y + 1 < w->rows) w->text[p++] = '\n';
    }
    w->text[p] = '\0';
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

/* Runs one drained variant (use_clear = 0: take; 1: clear) over the control's frames. */
static void run_drained(workload_t *w, const char *tag, int use_clear, const size_t *ctl_delta,
                        unsigned long n, size_t max_frame) {
    gibson_context_t *ctx = NULL;
    uint8_t *copy = NULL, *taken = NULL;
    size_t buf_cap = 0, peak = 0, after_max = 0, mismatched = 0, total = 0;
    unsigned long warm = n / 10 < 200 ? n / 10 : 200, f;
    long rss_warm = -1, rss_end, bound_kib;
    char vtag[96];
    snprintf(vtag, sizeof vtag, "%s %s", tag, use_clear ? "clear" : "take");
    if (warm < 1) warm = 1;

    check(open_ctx(w, &ctx) == GIBSON_OK, "create context", vtag);
    for (f = 0; ctx && f < n; f++) {
        size_t len, got = (size_t)-1, after;
        if (draw(ctx, w, f) != GIBSON_OK) { check(0, "draw", vtag); break; }
        len = cap_len(ctx);
        if (len > peak) peak = len;
        if (len != ctl_delta[f]) mismatched++;
        if (!g_no_drain) {
            if (use_clear) {
                if (gibson_clear_rendered_output(ctx, &got) != GIBSON_OK) { check(0, "clear status", vtag); break; }
                if (got != len) mismatched++;
            } else {
                if (len > buf_cap) {
                    uint8_t *a = realloc(copy, len), *b;
                    if (!a) { check(0, "oom", vtag); break; }
                    copy = a;
                    b = realloc(taken, len);
                    if (!b) { check(0, "oom", vtag); break; }
                    taken = b;
                    buf_cap = len;
                }
                if (gibson_get_rendered_output(ctx, copy, buf_cap, &got) != GIBSON_OK || got != len) {
                    check(0, "get before take", vtag);
                    break;
                }
                got = (size_t)-1;
                if (gibson_take_rendered_output(ctx, taken, buf_cap, &got) != GIBSON_OK) { check(0, "take status", vtag); break; }
                if (got != len || (len && memcmp(copy, taken, len) != 0)) mismatched++;
            }
            total += got;
        }
        after = cap_len(ctx);
        if (after > after_max) after_max = after;
        if (f + 1 == warm) rss_warm = proc_status_kib("VmRSS");
    }
    if (ctx) check(readback_ok(ctx, w, n - 1), "final frame size and cell (0,0)", vtag);
    rss_end = proc_status_kib("VmRSS");
    bound_kib = RSS_SLACK_KIB + (long)(4 * max_frame / 1024);

    check(f == n, "every frame rendered", vtag);
    check(after_max == 0, "capture is 0 after every drain", vtag);
    check(peak <= max_frame, "capture never exceeds one frame's bytes", vtag);
    check(mismatched == 0, "drained bytes per frame equal the control's per-frame growth", vtag);
    check(rss_warm >= 0 && rss_end >= 0, "VmRSS readable from /proc/self/status", vtag);
    if (rss_warm >= 0 && rss_end >= 0)
        check(rss_end - rss_warm < bound_kib, "VmRSS after N frames - VmRSS after warm-up < bound", vtag);
    printf("%-9s %-14s frames=%lu warmup=%lu drained_bytes=%zu peak_capture=%zu max_after_drain=%zu "
           "mismatched_frames=%zu rss_warm_kib=%ld rss_end_kib=%ld rss_delta_kib=%ld bound_kib=%ld\n",
           use_clear ? "CLEAR" : "TAKE", tag, n, warm, total, peak, after_max, mismatched, rss_warm, rss_end,
           rss_warm >= 0 ? rss_end - rss_warm : 0L, bound_kib);
    gibson_destroy_context(ctx);
    free(copy);
    free(taken);
}

/* What a CONTROL child reports to the parent, followed by `n` per-frame growth values. */
typedef struct {
    unsigned long n, f_to_mib;
    size_t len, max_frame;
    int failed;
} control_t;

static int write_all(int fd, const void *p, size_t n) {
    const char *c = p;
    while (n) {
        ssize_t k = write(fd, c, n);
        if (k <= 0) return 0;
        c += k;
        n -= (size_t)k;
    }
    return 1;
}

static int read_all(int fd, void *p, size_t n) {
    char *c = p;
    while (n) {
        ssize_t k = read(fd, c, n);
        if (k <= 0) return 0;
        c += k;
        n -= (size_t)k;
    }
    return 1;
}

/* CONTROL (child): one context, never drained. Probe for the 1 MiB frame, then run on to N. */
static void run_control(workload_t *w, const char *tag, int fd) {
    gibson_context_t *ctx = NULL;
    control_t c = {0, 0, 0, 0, 0};
    size_t *delta = NULL, dcap = 0, prev = 0;
    unsigned long f;
    long rss0, rss1;
    check(open_ctx(w, &ctx) == GIBSON_OK, "create control context", tag);
    rss0 = proc_status_kib("VmRSS");
    c.n = w->changing ? 10000000ul : FIXED_FRAMES;
    for (f = 0; ctx && f < c.n; f++) {
        if (draw(ctx, w, f) != GIBSON_OK) { check(0, "control draw", tag); break; }
        c.len = cap_len(ctx);
        if (f == dcap) {
            size_t *d = realloc(delta, sizeof *delta * (dcap ? dcap * 2 : 1024));
            if (!d) { check(0, "oom", tag); break; }
            delta = d;
            dcap = dcap ? dcap * 2 : 1024;
        }
        delta[f] = c.len - prev;
        if (delta[f] > c.max_frame) c.max_frame = delta[f];
        prev = c.len;
        if (!c.f_to_mib && c.len >= MIB) {
            c.f_to_mib = f + 1;
            c.n = c.f_to_mib * FRAMES_FACTOR;
        }
    }
    rss1 = proc_status_kib("VmRSS");
    check(f == c.n, "control rendered every frame", tag);
    if (w->changing) {
        check(c.f_to_mib > 0, "changing control reaches 1 MiB", tag);
        /* N = 20 x the frames to 1 MiB, but the first frame is larger than the rest, so the
           undrained capture lands just under 20 MiB; 19 MiB is still ~9x every bound. */
        check(c.len >= (size_t)(FRAMES_FACTOR - 1) * MIB, "control capture >= 19 MiB over N frames", tag);
    }
    printf("CONTROL   %-14s frames=%lu frames_to_1mib=%lu capture_bytes=%zu first_frame_bytes=%zu "
           "max_frame_bytes=%zu rss_growth_kib=%ld\n",
           tag, c.n, c.f_to_mib, c.len, delta ? delta[0] : 0, c.max_frame, rss1 - rss0);
    gibson_destroy_context(ctx);
    c.failed = g_failed || f != c.n;
    if (!write_all(fd, &c, sizeof c) || (!c.failed && !write_all(fd, delta, sizeof *delta * c.n)))
        g_failed = 1;
    free(delta);
}

/* Each phase runs in its own child, so no phase's resident memory is inherited by, or recycled
   into, the next: freed capture memory left in this process's heap would hide a later run's
   growth from VmRSS. */
static pid_t spawn(int *rfd) {
    int pfd[2] = {-1, -1};
    pid_t pid;
    fflush(stdout);
    fflush(stderr);
    if (rfd && pipe(pfd) != 0) return -1;
    pid = fork();
    if (pid == 0) {
        g_failed = 0; /* a child reports only its own phase */
        if (rfd) close(pfd[0]);
        if (rfd) *rfd = pfd[1];
        return 0;
    }
    if (rfd) {
        close(pfd[1]);
        *rfd = pid > 0 ? pfd[0] : -1;
        if (pid < 0) close(pfd[0]);
    }
    return pid;
}

static int reap(pid_t pid) {
    int status = 0;
    if (pid < 0 || waitpid(pid, &status, 0) != pid) return 0;
    return WIFEXITED(status) && WEXITSTATUS(status) == 0;
}

int main(void) {
    static const struct { uint16_t cols, rows; } grids[] = {{40, 15}, {80, 24}, {240, 80}};
    const char *e;
    if ((e = getenv("GIBSON_DRAIN_NO_DRAIN")) && atoi(e) == 1) g_no_drain = 1;

    if (gibson_abi_version() != GIBSON_ABI_VERSION) {
        fprintf(stderr, "headless_capture_drain: FAIL: ABI mismatch\n");
        return 1;
    }
    printf("config: frames_factor=%u fixed_frames=%u rss_slack_kib=%ld no_drain=%d start_rss_kib=%ld\n",
           FRAMES_FACTOR, FIXED_FRAMES, RSS_SLACK_KIB, g_no_drain, proc_status_kib("VmRSS"));

    for (size_t gi = 0; gi < sizeof grids / sizeof grids[0]; gi++) {
        for (int changing = 0; changing <= 1; changing++) {
            workload_t w;
            char tag[64];
            size_t cells = (size_t)grids[gi].cols * grids[gi].rows;
            size_t *delta = NULL;
            control_t c;
            int fd = -1, got;
            pid_t pid;
            w.cols = grids[gi].cols;
            w.rows = grids[gi].rows;
            w.changing = changing;
            w.text = malloc(cells + w.rows + 1);
            if (!w.text) { fprintf(stderr, "oom\n"); return 1; }
            snprintf(tag, sizeof tag, "%ux%u %s", w.cols, w.rows, changing ? "changing" : "fixed");

            pid = spawn(&fd);
            if (pid == 0) {
                run_control(&w, tag, fd);
                close(fd);
                fflush(stdout);
                _exit(g_failed ? 1 : 0);
            }
            got = fd >= 0 && read_all(fd, &c, sizeof c) && !c.failed && c.n > 0 &&
                  (delta = malloc(sizeof *delta * c.n)) != NULL && read_all(fd, delta, sizeof *delta * c.n);
            if (fd >= 0) close(fd);
            if (!reap(pid) || !got) {
                check(0, "control run", tag);
                free(delta);
                free(w.text);
                continue;
            }

            for (int use_clear = 0; use_clear <= 1; use_clear++) {
                pid = spawn(NULL);
                if (pid == 0) {
                    run_drained(&w, tag, use_clear, delta, c.n, c.max_frame);
                    fflush(stdout);
                    _exit(g_failed ? 1 : 0);
                }
                if (!reap(pid)) g_failed = 1;
            }
            free(delta);
            free(w.text);
        }
    }

    if (g_failed) return 1;
    printf("headless_capture_drain: OK\n");
    return 0;
}
