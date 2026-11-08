/* SPDX-License-Identifier: MIT OR Apache-2.0
 *
 * fb_adapter_demo: the synthetic producer on a terminal through fb_adapter, plus the property
 * checks that prove the adapter's contract.
 *
 *   fb_adapter_demo [--play] [options]      interactive (keyboard: arrows/WASD steer, space pulse, q quit)
 *   fb_adapter_demo --selfcheck [--only N] [--control NAME]
 *   fb_adapter_demo --replay TRACE --tics N [--grid CxR] [--dump-cells FILE]
 *
 * play options: --exit-after-tics N  --report FILE  --trace FILE  --square-pixels  --seed N
 *               --hold-grace TICS  --hold-repeat TICS  --tic-hz N  --color-depth mono|16|256|true
 *               --bench-frames N (one tic + one present per loop, as fast as the terminal takes it)
 *               negative controls: --control-ignore-input  --control-no-restore  --control-no-signals
 *               --fault stale-map|resize-steps|frame-locked|leak|retry-io|sticky-signal|slow-present
 *               --reopen-after-signal TICS (after a handled SIGINT/SIGTERM, open a second adapter and run TICS)
 *
 * --selfcheck prints one line per property, `PROP <n> <name>: PASS|FAIL <evidence>`, then
 * `fb_adapter: OK` and exits 0 only if every property passed. --control runs one property with a
 * deliberate defect (a wrong reference, a mutant adapter or producer) and must FAIL; the controls
 * are listed by --selfcheck --list-controls.
 *
 * Built only against the installed SDK: <gibson.h> via pkg-config, never repository paths.
 */
#define _GNU_SOURCE
#include "fb_adapter.h"
#include "synthetic_producer.h"

#include <errno.h>
#include <fcntl.h>
#include <math.h>
#include <poll.h>
#include <signal.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <termios.h>
#include <time.h>
#include <unistd.h>

#define U2580 "\xe2\x96\x80"
#define DOOM_W 320
#define DOOM_H 200

/* ================================================================ small utilities */

static uint64_t now_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
}

static long proc_status_kb(const char *key) {
    FILE *f = fopen("/proc/self/status", "r");
    if (!f) return -1;
    char line[256];
    long v = -1;
    size_t kl = strlen(key);
    while (fgets(line, sizeof line, f))
        if (strncmp(line, key, kl) == 0 && line[kl] == ':') {
            v = strtol(line + kl + 1, NULL, 10);
            break;
        }
    fclose(f);
    return v;
}

static long maxrss_kb(void) {
    struct rusage ru;
    getrusage(RUSAGE_SELF, &ru);
    return ru.ru_maxrss;
}

static int cmp_u64(const void *a, const void *b) {
    uint64_t x = *(const uint64_t *)a, y = *(const uint64_t *)b;
    return x < y ? -1 : x > y;
}

/* mean and p99 (nearest rank) in microseconds of v[from, to) */
static void stats_us(const uint64_t *v, size_t from, size_t to, double *mean, double *p99) {
    *mean = *p99 = 0;
    if (to <= from) return;
    size_t n = to - from;
    uint64_t *c = malloc(n * sizeof *c);
    if (!c) return;
    double sum = 0;
    for (size_t i = 0; i < n; i++) {
        c[i] = v[from + i];
        sum += (double)c[i];
    }
    qsort(c, n, sizeof *c, cmp_u64);
    size_t rank = (size_t)ceil(0.99 * (double)n);
    if (rank < 1) rank = 1;
    *mean = sum / (double)n / 1000.0;
    *p99 = (double)c[rank - 1] / 1000.0;
    free(c);
}

/* A scripted clock for fb_adapter_run: call 0 returns 0; call k advances the simulated wall time
   by steps[(k-1) % nsteps] tics (1 if no steps), or by stall_tics on call stall_call. 35 Hz only. */
typedef struct fclock {
    uint64_t calls, cum;
    const uint8_t *steps;
    size_t nsteps;
    uint64_t stall_call, stall_tics;
} fclock;

static uint64_t fclock_now(void *u) {
    fclock *c = u;
    if (c->calls > 0) {
        uint64_t step = c->steps ? c->steps[(c->calls - 1) % c->nsteps] : 1;
        if (c->stall_call && c->calls == c->stall_call) step = c->stall_tics;
        c->cum += step;
    }
    c->calls++;
    return (c->cum * 1000000000ull + 34) / 35; /* ceil(cum / 35 s) in ns */
}

/* ================================================================ play / bench / replay */

typedef struct play_opts {
    uint64_t exit_after_tics, bench_frames, seed, reopen_tics;
    const char *report, *trace, *replay, *dump_cells;
    uint64_t replay_tics;
    uint16_t grid_cols, grid_rows;
    int square, ignore_input, no_restore, no_signals;
    unsigned faults;
    uint32_t grace, repeat, hz;
    int depth;
} play_opts;

typedef struct demo {
    synth_producer *prod;
    fb_producer inner;
    const fb_run_stats *stats;
    uint64_t fps_t0, fps_n;
    double fps;
    FILE *trace;
    int bench;
    uint64_t *lower, *render;
    size_t n, cap, warm_at, mid_at;
    long hwm_warm, maxrss_warm, rss_warm, hwm_mid;
} demo;

static fb_status demo_draw(void *u, fb_adapter *ad) {
    demo *d = u;
    fb_status st = d->inner.draw(d->inner.user, ad);
    if (st != FB_OK) return st;
    fb_geometry g;
    fb_adapter_geometry(ad, &g);
    char s[256];
    unsigned long long tic = d->prod->s.tic, drop = d->stats ? d->stats->tics_dropped : 0;
    /* first, so a narrow status row never cuts it off */
    const char *exact = fb_adapter_releases_reported(ad) ? " | keys exact" : "";
    if (g.cols >= 100)
        snprintf(s, sizeof s, " LIBGIBSON fb_adapter%s | tic %llu | %.1f fps | dropped %llu | %ux%u | arrows/WASD steer  space pulse  q quit",
                 exact, tic, d->fps, drop, (unsigned)g.cols, (unsigned)g.rows);
    else if (g.cols >= 60)
        snprintf(s, sizeof s, " fb_adapter%s | tic %llu | %.1f fps | drop %llu | arrows/WASD  space  q", exact, tic, d->fps, drop);
    else
        snprintf(s, sizeof s, " tic %llu %.0f fps  q quit", tic, d->fps);
    return fb_adapter_set_status(ad, s);
}

static void demo_key(void *u, uint32_t code, int pressed) {
    demo *d = u;
    d->inner.key(d->inner.user, code, pressed);
}

static void demo_tic(void *u) {
    demo *d = u;
    d->inner.tic(d->inner.user);
}

static void demo_on_present(void *u, fb_adapter *ad, uint64_t tics) {
    (void)tics;
    demo *d = u;
    const uint64_t t = now_ns();
    if (!d->fps_t0) d->fps_t0 = t;
    d->fps_n++;
    if (t - d->fps_t0 >= 500000000ull) {
        d->fps = (double)d->fps_n * 1e9 / (double)(t - d->fps_t0);
        d->fps_t0 = t;
        d->fps_n = 0;
    }
    if (d->bench && d->n < d->cap) {
        fb_adapter_last_present_ns(ad, &d->lower[d->n], &d->render[d->n]);
        d->n++;
        if (d->n == d->warm_at) {
            d->hwm_warm = proc_status_kb("VmHWM");
            d->maxrss_warm = maxrss_kb();
            d->rss_warm = proc_status_kb("VmRSS");
        }
        if (d->n == d->mid_at) d->hwm_mid = proc_status_kb("VmHWM");
    }
}

static void demo_observer(void *u, uint64_t tic, uint32_t kind, uint32_t code, uint8_t mods) {
    demo *d = u;
    if (d->trace) {
        fprintf(d->trace, "%llu %u %u %u\n", (unsigned long long)tic, kind, code, (unsigned)mods);
        fflush(d->trace);
    }
}

static const char *exit_name(fb_exit_reason r) {
    switch (r) {
    case FB_EXIT_QUIT: return "quit";
    case FB_EXIT_SIGNAL: return "signal";
    case FB_EXIT_HANGUP: return "hangup";
    case FB_EXIT_TIC_LIMIT: return "tic_limit";
    case FB_EXIT_ERROR: return "error";
    default: return "none";
    }
}

static void config_for_play(fb_adapter_config *cfg, const play_opts *o) {
    fb_adapter_config_defaults(cfg, DOOM_W, DOOM_H);
    if (!o->square) {
        cfg->par_num = 5; /* 320x200 shown at 4:3 */
        cfg->par_den = 6;
    }
    cfg->status_rows = 1;
    cfg->bar_rgb[0] = 6;
    cfg->bar_rgb[1] = 4;
    cfg->bar_rgb[2] = 14;
    if (o->grace) cfg->hold_grace_tics = o->grace;
    if (o->repeat) cfg->hold_repeat_tics = o->repeat;
    if (o->hz) cfg->tic_hz = o->hz;
    cfg->color_depth = o->depth;
}

static void bench_summary(FILE *f, const demo *d) {
    double lm, lp, rm, rp, tm, tp;
    uint64_t *tot = malloc((d->n ? d->n : 1) * sizeof *tot);
    if (!tot) return;
    for (size_t i = 0; i < d->n; i++) tot[i] = d->lower[i] + d->render[i];
    size_t from = d->warm_at < d->n ? d->warm_at : 0;
    stats_us(d->lower, from, d->n, &lm, &lp);
    stats_us(d->render, from, d->n, &rm, &rp);
    stats_us(tot, from, d->n, &tm, &tp);
    fprintf(f, "bench_frames=%zu warm=%zu lower_mean_us=%.1f lower_p99_us=%.1f render_mean_us=%.1f render_p99_us=%.1f "
               "total_mean_us=%.1f total_p99_us=%.1f hwm_warm_kb=%ld hwm_mid_kb=%ld hwm_end_kb=%ld maxrss_warm_kb=%ld "
               "maxrss_end_kb=%ld rss_warm_kb=%ld rss_end_kb=%ld\n",
            d->n, d->warm_at, lm, lp, rm, rp, tm, tp, d->hwm_warm, d->hwm_mid, proc_status_kb("VmHWM"), d->maxrss_warm,
            maxrss_kb(), d->rss_warm, proc_status_kb("VmRSS"));
    free(tot);
}

static int run_play(const play_opts *o) {
    fb_adapter_config cfg;
    config_for_play(&cfg, o);
    cfg.install_signal_handlers = !o->no_signals;
    synth_producer *prod = calloc(1, sizeof *prod);
    if (!prod || synth_init(prod, o->seed) != 0) {
        fprintf(stderr, "fb_adapter_demo: out of memory\n");
        return 1;
    }
    prod->ignore_input = o->ignore_input;
    demo d;
    memset(&d, 0, sizeof d);
    d.prod = prod;
    d.inner = synth_producer_iface(prod);
    if (o->trace && !(d.trace = fopen(o->trace, "w"))) {
        fprintf(stderr, "fb_adapter_demo: cannot write %s\n", o->trace);
        return 1;
    }
    cfg.observer = demo_observer;
    cfg.observer_user = &d;
    fb_adapter *ad = NULL;
    fb_status st = fb_adapter_open_terminal(&cfg, &ad);
    if (st != FB_OK) {
        fprintf(stderr, "fb_adapter_demo: %s (interactive mode needs a terminal; try --selfcheck)\n", fb_adapter_last_error());
        return 2;
    }
    fb_adapter_set_faults(ad, o->faults);
    fb_producer p = {&d, demo_key, demo_tic, demo_draw};
    fb_run_options ro;
    fb_run_options_defaults(&ro);
    ro.max_tics = o->exit_after_tics;
    ro.on_present = demo_on_present;
    ro.hook_user = &d;
    fclock fc;
    memset(&fc, 0, sizeof fc);
    if (o->bench_frames) {
        ro.clock = fclock_now; /* one tic per loop iteration, no waiting: measure the present path */
        ro.clock_user = &fc;
        ro.block_on_input = 0;
        ro.max_tics = o->bench_frames;
        d.bench = 1;
        d.cap = (size_t)o->bench_frames;
        d.warm_at = d.cap >= 1000 ? 200 : d.cap / 5;
        d.mid_at = d.warm_at + (d.cap - d.warm_at) / 2;
        d.lower = calloc(d.cap, sizeof *d.lower);
        d.render = calloc(d.cap, sizeof *d.render);
        if (!d.lower || !d.render) return 1;
    }
    fb_run_stats rs;
    memset(&rs, 0, sizeof rs);
    d.stats = &rs;
    const uint64_t t0 = now_ns();
    st = fb_adapter_run(ad, &p, &ro, &rs);
    const uint64_t wall = now_ns() - t0;

    /* the report is written while the context still exists (frame size readback) */
    if (o->report) {
        FILE *f = fopen(o->report, "w");
        if (f) {
            uint16_t fc_ = 0, fr_ = 0;
            (void)gibson_get_frame_size(fb_adapter_context(ad), &fc_, &fr_);
            fb_geometry g;
            fb_adapter_geometry(ad, &g);
            fb_adapter_counters c;
            fb_adapter_counters_get(ad, &c);
            struct rusage ru;
            getrusage(RUSAGE_SELF, &ru);
            fprintf(f, "exit=%s tics=%llu dropped=%llu frames=%llu iterations=%llu hash=%016llx keys_observed=%llu "
                       "keys_delivered=%llu resizes=%llu map_builds=%llu grid=%ux%u frame=%ux%u render_errors=%llu "
                       "max_tics_in_iteration=%llu wall_ms=%.1f cpu_ms=%.1f maxrss_kb=%ld\n",
                    exit_name(rs.exit_reason), (unsigned long long)rs.tics, (unsigned long long)rs.tics_dropped,
                    (unsigned long long)rs.frames_presented, (unsigned long long)rs.iterations,
                    (unsigned long long)synth_hash(prod), (unsigned long long)c.keys_observed,
                    (unsigned long long)rs.keys_delivered, (unsigned long long)c.resizes,
                    (unsigned long long)g.map_builds, (unsigned)g.cols, (unsigned)g.rows, (unsigned)fc_, (unsigned)fr_,
                    (unsigned long long)rs.render_errors, (unsigned long long)rs.max_tics_in_iteration, (double)wall / 1e6,
                    (double)(ru.ru_utime.tv_sec + ru.ru_stime.tv_sec) * 1e3 + (double)(ru.ru_utime.tv_usec + ru.ru_stime.tv_usec) / 1e3,
                    ru.ru_maxrss);
            if (d.bench) bench_summary(f, &d);
            fclose(f);
        }
    }
    if (o->no_restore) _exit(0); /* negative control: leave the terminal as it is */
    fb_adapter_close(ad);
    if (d.trace) fclose(d.trace);
    d.trace = NULL; /* the second session below must not write to the closed stream (#299 review) */
    if (o->reopen_tics && rs.exit_reason == FB_EXIT_SIGNAL) {
        /* --reopen-after-signal: the signal was handled and the terminal restored; a second adapter
           opened in this same process must run normally, not inherit the old signal. */
        fb_adapter *ad2 = NULL;
        fb_run_stats rs2;
        memset(&rs2, 0, sizeof rs2);
        fb_status st2 = fb_adapter_open_terminal(&cfg, &ad2);
        if (st2 == FB_OK) {
            fb_adapter_set_faults(ad2, o->faults);
            d.stats = &rs2;
            fb_run_options ro2;
            fb_run_options_defaults(&ro2);
            ro2.max_tics = o->reopen_tics;
            ro2.on_present = demo_on_present;
            ro2.hook_user = &d;
            st2 = fb_adapter_run(ad2, &p, &ro2, &rs2);
            const int sig2 = fb_adapter_signal(ad2);
            fb_adapter_close(ad2);
            if (o->report) {
                FILE *f = fopen(o->report, "a");
                if (f) {
                    fprintf(f, "second_exit=%s second_tics=%llu second_frames=%llu second_signal=%d\n",
                            exit_name(rs2.exit_reason), (unsigned long long)rs2.tics,
                            (unsigned long long)rs2.frames_presented, sig2);
                    fclose(f);
                }
            }
        }
        fprintf(stderr, "fb_adapter_demo: second adapter: %s after %llu tics\n", exit_name(rs2.exit_reason),
                (unsigned long long)rs2.tics);
        synth_free(prod);
        free(prod);
        return (st2 == FB_OK && rs2.exit_reason == FB_EXIT_TIC_LIMIT) ? 0 : 1;
    }
    fprintf(stderr, "fb_adapter_demo: %s after %llu tics, %llu frames, %llu dropped\n", exit_name(rs.exit_reason),
            (unsigned long long)rs.tics, (unsigned long long)rs.frames_presented, (unsigned long long)rs.tics_dropped);
    if (rs.exit_reason == FB_EXIT_SIGNAL) {
        int sig = fb_adapter_signal_received();
        signal(sig, SIG_DFL); /* die by it, so the parent sees the real cause */
        raise(sig);
    }
    synth_free(prod);
    free(prod);
    return (rs.exit_reason == FB_EXIT_ERROR || st != FB_OK) ? 1 : 0;
}

/* ---- tic-stamped input injected into a headless adapter ---- */
typedef struct tev {
    uint64_t tic;
    uint32_t kind, code;
    uint8_t mods;
    uint16_t cols, rows; /* kind 0 = RESIZE */
} tev;

typedef struct inject {
    const tev *ev;
    size_t n, next;
    int pushed_fail;
} inject;

static void inject_pre_tic(void *u, fb_adapter *ad, uint64_t tic) {
    inject *in = u;
    while (in->next < in->n && in->ev[in->next].tic <= tic) {
        const tev *t = &in->ev[in->next++];
        if (t->tic < tic) continue;
        gibson_event_t e;
        memset(&e, 0, sizeof e);
        if (t->kind == 0) {
            e.event_type = GIBSON_EVENT_RESIZE;
            e.cols = t->cols;
            e.rows = t->rows;
            if (gibson_push_event(fb_adapter_context(ad), &e) != GIBSON_OK) in->pushed_fail++;
        } else if (t->kind == FB_OBSERVED_RELEASES_ACTIVE || t->kind == FB_OBSERVED_RELEASES_UNSUPPORTED) {
            /* the terminal's answer to the key-release request (#275) */
            e.event_type = t->kind == FB_OBSERVED_RELEASES_ACTIVE ? GIBSON_EVENT_KEY_RELEASES_ACTIVE
                                                                   : GIBSON_EVENT_KEY_RELEASES_UNSUPPORTED;
            if (gibson_push_event(fb_adapter_context(ad), &e) != GIBSON_OK) in->pushed_fail++;
        } else {
            e.event_type = (t->kind & FB_OBSERVED_RELEASE) ? GIBSON_EVENT_KEY_RELEASE : GIBSON_EVENT_KEY;
            e.key_code = t->code;
            e.modifiers = t->mods;
            if (gibson_push_event_kind(fb_adapter_context(ad), &e, t->kind & 0xFFu) != GIBSON_OK) in->pushed_fail++;
        }
    }
}

/* Reads back the whole last frame (also used by the selfcheck as read_frame). */
static int read_frame_cells(gibson_context_t *ctx, gibson_cell_t **cells, uint16_t *cols, uint16_t *rows) {
    if (gibson_get_frame_size(ctx, cols, rows) != GIBSON_OK || !*cols || !*rows) return 0;
    size_t n = (size_t)*cols * *rows;
    gibson_cell_t *c = realloc(*cells, n * sizeof *c);
    if (!c) return 0;
    *cells = c;
    size_t w = 0;
    return gibson_get_frame_cells(ctx, 0, 0, c, n, &w) == GIBSON_OK && w == n;
}

static int run_replay(const play_opts *o) {
    FILE *f = fopen(o->replay, "r");
    if (!f) {
        fprintf(stderr, "fb_adapter_demo: cannot read %s\n", o->replay);
        return 1;
    }
    size_t cap = 64, n = 0;
    tev *ev = malloc(cap * sizeof *ev);
    unsigned long long t;
    unsigned kind, code, mods;
    while (ev && fscanf(f, "%llu %u %u %u", &t, &kind, &code, &mods) == 4) {
        if (n == cap) {
            tev *ne = realloc(ev, (cap *= 2) * sizeof *ev);
            if (!ne) break;
            ev = ne;
        }
        ev[n].tic = t;
        ev[n].kind = kind;
        ev[n].code = code;
        ev[n].mods = (uint8_t)mods;
        ev[n].cols = ev[n].rows = 0;
        n++;
    }
    fclose(f);
    fb_adapter_config cfg;
    config_for_play(&cfg, o);
    synth_producer *prod = calloc(1, sizeof *prod);
    if (!ev || !prod || synth_init(prod, o->seed) != 0) return 1;
    fb_adapter *ad = NULL;
    if (fb_adapter_open_headless(&cfg, o->grid_cols ? o->grid_cols : 80, o->grid_rows ? o->grid_rows : 24, &ad) != FB_OK) {
        fprintf(stderr, "fb_adapter_demo: %s\n", fb_adapter_last_error());
        return 1;
    }
    inject in = {ev, n, 0, 0};
    fb_producer p = synth_producer_iface(prod);
    fb_run_options ro;
    fb_run_options_defaults(&ro);
    fclock fc;
    memset(&fc, 0, sizeof fc);
    ro.clock = fclock_now;
    ro.clock_user = &fc;
    ro.block_on_input = 0;
    ro.present_divisor = 8;
    ro.max_tics = o->replay_tics;
    ro.pre_tic = inject_pre_tic;
    ro.hook_user = &in;
    fb_run_stats rs;
    fb_status st = fb_adapter_run(ad, &p, &ro, &rs);
    if (st == FB_OK && o->dump_cells) {
        /* the frame of the final state, as the live loop presents it after its last tic */
        if (p.draw(p.user, ad) != FB_OK || fb_adapter_present(ad) != FB_OK) st = FB_ERR_GIBSON;
        uint16_t c = 0, r = 0;
        gibson_cell_t *cells = NULL;
        FILE *df = fopen(o->dump_cells, "w");
        if (st == FB_OK && df && read_frame_cells(fb_adapter_context(ad), &cells, &c, &r)) {
            fprintf(df, "%u %u\n", c, r);
            for (size_t i = 0; i < (size_t)c * r; i++) {
                const gibson_cell_t *x = &cells[i];
                const char kind = (x->glyph_len == 3 && memcmp(x->glyph, U2580, 3) == 0) ? 'H'
                                  : (x->glyph_len == 1 && x->glyph[0] == ' ') ? 'S' : 'O';
                fprintf(df, "%c %02x%02x%02x %02x%02x%02x\n", kind, x->style.fg.r, x->style.fg.g, x->style.fg.b,
                        x->style.bg.r, x->style.bg.g, x->style.bg.b);
            }
        } else {
            st = FB_ERR_GIBSON;
        }
        if (df) fclose(df);
        free(cells);
    }
    printf("REPLAY tics=%llu hash=%016llx keys_delivered=%llu exit=%s events=%zu push_failures=%d\n",
           (unsigned long long)rs.tics, (unsigned long long)synth_hash(prod), (unsigned long long)rs.keys_delivered,
           exit_name(rs.exit_reason), n, in.pushed_fail);
    fb_adapter_close(ad);
    synth_free(prod);
    free(prod);
    free(ev);
    return st == FB_OK ? 0 : 1;
}

/* ================================================================ selfcheck */

enum {
    C_NONE = 0,
    C_P1_SHIFTED_REF,
    C_P2_EDGE_SAMPLING,
    C_P2_NO_LETTERBOX,
    C_P3_IGNORE_INPUT,
    C_P3_IGNORE_RELEASES,
    C_P3_REARM_LOSES_RELEASE,
    C_P4_FRAME_LOCKED,
    C_P5_STALE_MAP,
    C_P5_RESIZE_STEPS,
    C_P6_NO_RESTORE,
    C_P6_NO_SIGNALS,
    C_P6_RETRY_IO,
    C_P6_STICKY_SIGNAL,
    C_P7_LEAK,
    C_P7_NO_ROTATION,
    C_P7_SLOW_PRESENT,
    C_COUNT
};

static const struct {
    const char *name;
    int prop;
    const char *what;
} CONTROLS[C_COUNT] = {
    {"none", 0, ""},
    {"p1-shifted-ref", 1, "the expected pattern is shifted one pixel right"},
    {"p2-edge-sampling", 2, "the reference samples pixel edges, not centres"},
    {"p2-no-letterbox", 2, "the reference stretches to the whole grid (no bars)"},
    {"p3-ignore-input", 3, "the producer ignores every key"},
    {"p3-ignore-releases", 3, "mutant adapter: reported key releases are ignored (holds keep the grace)"},
    {"p3-rearm-loses-release", 3, "mutant adapter: a re-press re-arms a hold whose release is waiting for a full queue"},
    {"p4-frame-locked", 4, "mutant adapter: one tic per presented frame"},
    {"p5-stale-map", 5, "mutant adapter: the map is not rebuilt on resize"},
    {"p5-resize-steps", 5, "mutant adapter: a resize runs an extra producer tic"},
    {"p6-no-restore", 6, "the program exits without gibson_destroy_context"},
    {"p6-no-signals", 6, "no SIGTERM handler: the default action kills it"},
    {"p6-retry-io", 6, "mutant adapter: GIBSON_ERR_IO from a poll is ignored"},
    {"p6-sticky-signal", 6, "mutant adapter: a handled signal also ends every later adapter"},
    {"p7-leak", 7, "mutant adapter: each present leaks 64 KiB"},
    {"p7-no-rotation", 7, "headless capture never rotated (the library's capture grows: finding F1)"},
    {"p7-slow-present", 7, "mutant adapter: every 35th present stalls 400 ms"},
};

static int g_control = C_NONE;
static char g_exe[4096];

static int ctl(int c) { return g_control == c; }

#define EVID 2048
static void ev_append(char *ev, const char *fmt, ...) {
    size_t n = strlen(ev);
    if (n + 1 >= EVID) return;
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(ev + n, EVID - n, fmt, ap);
    va_end(ap);
}

static int read_frame(gibson_context_t *ctx, gibson_cell_t **cells, uint16_t *cols, uint16_t *rows) {
    return read_frame_cells(ctx, cells, cols, rows);
}

static uint32_t cell_rgb(const gibson_color_t *c) {
    if (c->color_type != GIBSON_COLOR_RGB) return 0xFF000000u; /* never equals an RGB value */
    return ((uint32_t)c->r << 16) | ((uint32_t)c->g << 8) | c->b;
}

/* ---- the independent nearest-neighbour reference (double arithmetic, written from the
        declared rule in fb_adapter.h, sharing no code with the adapter's map) ---- */
typedef struct refsrc {
    int fmt; /* 1 indexed, 2 argb32 */
    const uint8_t *idx;
    const uint8_t *pal;
    const uint32_t *argb;
    int w, h;
} refsrc;

static uint32_t ref_px(const refsrc *s, int x, int y) {
    if (s->fmt == 1) {
        const uint8_t *c = s->pal + 3 * s->idx[(size_t)y * s->w + x];
        return ((uint32_t)c[0] << 16) | ((uint32_t)c[1] << 8) | c[2];
    }
    return s->argb[(size_t)y * s->w + x] & 0xFFFFFFu;
}

typedef struct refgeo {
    int area_rows, dw, dh, img_x, img_y, img_cols, img_rows;
} refgeo;

typedef struct refcfg {
    int pn, pd, cn, cd, status;
    uint32_t bar;
    const char *status_text;
} refcfg;

static refgeo ref_geometry(int cols, int rows, int W, int H, const refcfg *rc, int stretch) {
    refgeo g;
    memset(&g, 0, sizeof g);
    g.area_rows = rows - rc->status;
    if (cols <= 0 || g.area_rows <= 0) return g;
    const double VW = cols, VH = 2.0 * g.area_rows;
    const double Pd = (double)W * rc->pn * rc->cn, Qd = (double)H * rc->pd * 2.0 * rc->cd;
    double dw, dh;
    if (stretch) {
        dw = VW;
        dh = VH;
    } else if (VW * Qd <= VH * Pd) {
        dw = VW;
        dh = floor(VW * Qd / Pd + 0.5);
        if (dh > VH) dh = VH;
        dh = 2.0 * floor(dh / 2.0);
        if (dh < 2) dh = 2;
    } else {
        dh = VH;
        dw = floor(VH * Pd / Qd + 0.5);
        if (dw > VW) dw = VW;
        if (dw < 1) dw = 1;
    }
    g.dw = (int)dw;
    g.dh = (int)dh;
    g.img_cols = g.dw;
    g.img_rows = g.dh / 2;
    g.img_x = (cols - g.img_cols) / 2;
    g.img_y = (g.area_rows - g.img_rows) / 2;
    return g;
}

/* Compares every cell; returns the number of mismatches and counts image / bar / status cells. */
static long ref_compare(const gibson_cell_t *cells, int cols, int rows, const refsrc *src, const refcfg *rc,
                        int edge, int stretch, long *n_img, long *n_bar, char *first, size_t firstlen) {
    const refgeo g = ref_geometry(cols, rows, src->w, src->h, rc, stretch);
    long bad = 0;
    const size_t tl = rc->status_text ? strlen(rc->status_text) : 0;
    for (int y = 0; y < rows; y++)
        for (int x = 0; x < cols; x++) {
            const gibson_cell_t *c = &cells[(size_t)y * cols + x];
            int ok;
            char why[160];
            if (rc->status && y == rows - 1) {
                const char want = (size_t)x < tl ? rc->status_text[x] : ' ';
                ok = c->glyph_len == 1 && c->glyph[0] == (uint8_t)want;
                snprintf(why, sizeof why, "status cell (%d,%d) glyph %02x want '%c'", x, y, c->glyph[0], want);
            } else if (y >= g.img_y && y < g.img_y + g.img_rows && x >= g.img_x && x < g.img_x + g.img_cols) {
                const int i = x - g.img_x, r = y - g.img_y;
                const int sx = edge ? (int)floor((double)i * src->w / g.dw) : (int)floor((i + 0.5) * src->w / g.dw);
                const int yt = (int)floor((2 * r + 0.5) * src->h / g.dh);
                const int yb = (int)floor((2 * r + 1.5) * src->h / g.dh);
                const uint32_t ft = ref_px(src, sx, yt), fb = ref_px(src, sx, yb);
                ok = c->glyph_len == 3 && memcmp(c->glyph, U2580, 3) == 0 && cell_rgb(&c->style.fg) == ft &&
                     cell_rgb(&c->style.bg) == fb;
                snprintf(why, sizeof why, "image cell (%d,%d) fg %06x bg %06x want %06x/%06x (src %d,%d/%d)", x, y,
                         cell_rgb(&c->style.fg), cell_rgb(&c->style.bg), ft, fb, sx, yt, yb);
                (*n_img)++;
            } else {
                ok = c->glyph_len == 1 && c->glyph[0] == ' ' && cell_rgb(&c->style.bg) == rc->bar;
                snprintf(why, sizeof why, "bar cell (%d,%d) glyph %02x bg %06x want ' '/%06x", x, y, c->glyph[0],
                         cell_rgb(&c->style.bg), rc->bar);
                (*n_bar)++;
            }
            if (!ok) {
                if (!bad && first) snprintf(first, firstlen, "%s", why);
                bad++;
            }
        }
    return bad;
}

/* ---- test patterns ---- */
static uint8_t T_IDX[DOOM_H][DOOM_W];
static uint8_t T_PAL[768];
static uint32_t T_ARGB[DOOM_H][DOOM_W];

static void make_patterns(void) {
    for (int i = 0; i < 256; i++) { /* 256 distinct colours (red channel = index) */
        T_PAL[3 * i] = (uint8_t)i;
        T_PAL[3 * i + 1] = (uint8_t)(255 - i);
        T_PAL[3 * i + 2] = (uint8_t)((i * 37) & 255);
    }
    for (int y = 0; y < DOOM_H; y++)
        for (int x = 0; x < DOOM_W; x++) {
            T_IDX[y][x] = (uint8_t)((x * 7 + y * 13) & 255);
            T_ARGB[y][x] = 0xFF000000u | ((uint32_t)(x & 255) << 16) | ((uint32_t)y << 8) | (uint32_t)((x >> 8) * 0x80 + 0x11);
        }
}

/* ---------------------------------------------------------------- P1 */
static int prop1(char *ev) {
    fb_adapter_config cfg;
    fb_adapter_config_defaults(&cfg, DOOM_W, DOOM_H); /* 1:1 pixels, no status: 320x100 cells = exact */
    int pass = 1;
    gibson_cell_t *cells = NULL;
    for (int fmt = 1; fmt <= 2; fmt++) {
        fb_adapter *ad = NULL;
        if (fb_adapter_open_headless(&cfg, 320, 100, &ad) != FB_OK) {
            ev_append(ev, "open failed: %s", fb_adapter_last_error());
            return 0;
        }
        fb_status st = fmt == 1 ? fb_adapter_submit_indexed(ad, &T_IDX[0][0], DOOM_W, T_PAL)
                                : fb_adapter_submit_argb32(ad, &T_ARGB[0][0], DOOM_W);
        if (st == FB_OK) st = fb_adapter_present(ad);
        uint16_t cols = 0, rows = 0;
        if (st != FB_OK || !read_frame(fb_adapter_context(ad), &cells, &cols, &rows) || cols != 320 || rows != 100) {
            ev_append(ev, "present/readback failed (%s) ", fb_adapter_last_error());
            pass = 0;
            fb_adapter_close(ad);
            continue;
        }
        const int shift = ctl(C_P1_SHIFTED_REF) ? 1 : 0;
        long bad = 0, n = 0;
        char first[200] = "";
        for (int y = 0; y < 100; y++)
            for (int x = 0; x < 320; x++) {
                const int sx = (x + shift) % DOOM_W;
                uint32_t want_t, want_b;
                if (fmt == 1) {
                    const uint8_t *a = T_PAL + 3 * T_IDX[2 * y][sx], *b = T_PAL + 3 * T_IDX[2 * y + 1][sx];
                    want_t = ((uint32_t)a[0] << 16) | ((uint32_t)a[1] << 8) | a[2];
                    want_b = ((uint32_t)b[0] << 16) | ((uint32_t)b[1] << 8) | b[2];
                } else {
                    want_t = T_ARGB[2 * y][sx] & 0xFFFFFF;
                    want_b = T_ARGB[2 * y + 1][sx] & 0xFFFFFF;
                }
                const gibson_cell_t *c = &cells[(size_t)y * 320 + x];
                n++;
                if (!(c->glyph_len == 3 && memcmp(c->glyph, U2580, 3) == 0 && cell_rgb(&c->style.fg) == want_t &&
                      cell_rgb(&c->style.bg) == want_b)) {
                    if (!bad) snprintf(first, sizeof first, "first (%d,%d) got %06x/%06x want %06x/%06x", x, y,
                                       cell_rgb(&c->style.fg), cell_rgb(&c->style.bg), want_t, want_b);
                    bad++;
                }
            }
        fb_geometry g;
        fb_adapter_geometry(ad, &g);
        ev_append(ev, "%s: %ld/%ld cells exact%s%s (map %ux%u at %u,%u); ", fmt == 1 ? "indexed+palette" : "argb32",
                  n - bad, n, bad ? ", " : "", first, g.img_cols, g.img_rows, g.img_x, g.img_y);
        if (bad || g.img_cols != 320 || g.img_rows != 100) pass = 0;
        fb_adapter_close(ad);
    }
    free(cells);
    ev_append(ev, "grid 320x100 = 320x200 px 1:1");
    return pass;
}

/* ---------------------------------------------------------------- P2 */
static int prop2(char *ev) {
    static const uint16_t grids[][2] = {{80, 24}, {120, 40}, {40, 15}, {160, 50}, {200, 60}, {33, 7}, {1, 1}, {320, 100}, {400, 120}};
    static const int pars[][2] = {{5, 6}, {1, 1}};
    /* hand-derived from the rule (5:6 pixels, 2:1 cells, no status row): img_x, img_cols, img_rows */
    static const struct { uint16_t c, r; int x, w, h; } hand[] = {
        {80, 24, 8, 64, 24}, {120, 40, 6, 107, 40}, {40, 15, 0, 40, 15}, {160, 50, 13, 133, 50}, {200, 60, 20, 160, 60}};
    const int edge = ctl(C_P2_EDGE_SAMPLING), stretch = ctl(C_P2_NO_LETTERBOX);
    long cases = 0, bad_cases = 0, cells_total = 0, bad_cells = 0, img_total = 0, bar_total = 0, geo_bad = 0;
    int hand_ok = 0;
    char first[240] = "";
    gibson_cell_t *cells = NULL;
    const char *status = "P2 status row: the quick brown fox 0123456789";
    for (size_t gi = 0; gi < sizeof grids / sizeof grids[0]; gi++)
        for (int pi = 0; pi < 2; pi++)
            for (int status_rows = 0; status_rows <= 1; status_rows++)
                for (int fmt = 1; fmt <= 2; fmt++) {
                    const uint16_t C = grids[gi][0], R = grids[gi][1];
                    fb_adapter_config cfg;
                    fb_adapter_config_defaults(&cfg, DOOM_W, DOOM_H);
                    cfg.par_num = (uint8_t)pars[pi][0];
                    cfg.par_den = (uint8_t)pars[pi][1];
                    cfg.status_rows = (uint8_t)status_rows;
                    cfg.bar_rgb[0] = 3;
                    cfg.bar_rgb[1] = 5;
                    cfg.bar_rgb[2] = 7;
                    fb_adapter *ad = NULL;
                    if (fb_adapter_open_headless(&cfg, C, R, &ad) != FB_OK) {
                        bad_cases++;
                        continue;
                    }
                    fb_status st = fmt == 1 ? fb_adapter_submit_indexed(ad, &T_IDX[0][0], DOOM_W, T_PAL)
                                            : fb_adapter_submit_argb32(ad, &T_ARGB[0][0], DOOM_W);
                    if (st == FB_OK) st = fb_adapter_set_status(ad, status);
                    if (st == FB_OK) st = fb_adapter_present(ad);
                    uint16_t cols = 0, rows = 0;
                    cases++;
                    if (st != FB_OK || !read_frame(fb_adapter_context(ad), &cells, &cols, &rows) || cols != C || rows != R) {
                        bad_cases++;
                        if (!first[0]) snprintf(first, sizeof first, "grid %ux%u: present/readback failed", C, R);
                        fb_adapter_close(ad);
                        continue;
                    }
                    refsrc src = {fmt, &T_IDX[0][0], T_PAL, &T_ARGB[0][0], DOOM_W, DOOM_H};
                    refcfg rc = {pars[pi][0], pars[pi][1], 2, 1, status_rows, 0x030507u, status};
                    char f1[200] = "";
                    long ni = 0, nb = 0;
                    long bad = ref_compare(cells, C, R, &src, &rc, edge, stretch, &ni, &nb, f1, sizeof f1);
                    cells_total += (long)C * R;
                    img_total += ni;
                    bar_total += nb;
                    if (bad) {
                        bad_cases++;
                        bad_cells += bad;
                        if (!first[0]) snprintf(first, sizeof first, "grid %ux%u par %d:%d status %d %s: %ld bad, %.150s", C, R,
                                                pars[pi][0], pars[pi][1], status_rows, fmt == 1 ? "idx" : "argb", bad, f1);
                    }
                    /* the adapter's reported geometry agrees with the reference's */
                    const refgeo rg = ref_geometry(C, R, DOOM_W, DOOM_H, &rc, 0);
                    fb_geometry g;
                    fb_adapter_geometry(ad, &g);
                    if (g.img_x != rg.img_x || g.img_y != rg.img_y || g.img_cols != rg.img_cols || g.img_rows != rg.img_rows ||
                        g.map_builds != 1)
                        geo_bad++;
                    if (pi == 0 && status_rows == 0 && fmt == 1)
                        for (size_t h = 0; h < sizeof hand / sizeof hand[0]; h++)
                            if (hand[h].c == C && hand[h].r == R && g.img_x == hand[h].x && g.img_cols == hand[h].w &&
                                g.img_rows == hand[h].h && rg.img_x == hand[h].x && rg.img_cols == hand[h].w && rg.img_rows == hand[h].h)
                                hand_ok++;
                    fb_adapter_close(ad);
                }
    free(cells);
    const int pass = bad_cases == 0 && geo_bad == 0 && hand_ok == 5 && cases == 72;
    ev_append(ev, "%ld cases (9 grids x par{5:6,1:1} x status{0,1} x {indexed,argb32}), %ld cells vs independent NN reference: "
                  "%ld mismatching cells in %ld cases; %ld image + %ld letterbox cells; adapter geometry == reference in %ld/%ld; "
                  "hand-derived geometry %d/5 (80x24->64x24@8, 120x40->107x40@6, 40x15->40x15@0, 160x50->133x50@13, 200x60->160x60@20); "
                  "map built once per grid",
              cases, cells_total, bad_cells, bad_cases, img_total, bar_total, cases - geo_bad, cases, hand_ok);
    if (first[0]) ev_append(ev, "; first: %s", first);
    return pass;
}

/* ---------------------------------------------------------------- shared headless runner */
typedef struct runres {
    uint64_t hash, tics, frames, dropped, max_in_iter, keys;
    fb_run_stats rs;
    synth_key_log log[SYNTH_LOG];
    unsigned nlog;
    uint64_t map_builds;
    uint16_t sizes[8][2];
    int nsizes;
    long ref_bad, ref_frames;
    char first[240];
    int ok;
} runres;

typedef struct runctx {
    inject in;
    synth_producer *prod;
    runres *res;
    int check_ref; /* compare each presented frame after the first resize to the reference */
    int seen_resize;
    refcfg rc;
} runctx;

static void run_pre_tic(void *u, fb_adapter *ad, uint64_t tic) {
    runctx *r = u;
    inject_pre_tic(&r->in, ad, tic);
}

static void run_on_present(void *u, fb_adapter *ad, uint64_t tics) {
    (void)tics;
    runctx *r = u;
    uint16_t c = 0, w = 0;
    gibson_get_frame_size(fb_adapter_context(ad), &c, &w);
    runres *res = r->res;
    if (res->nsizes == 0 || res->sizes[res->nsizes - 1][0] != c || res->sizes[res->nsizes - 1][1] != w) {
        if (res->nsizes > 0) r->seen_resize = 1;
        if (res->nsizes < 8) {
            res->sizes[res->nsizes][0] = c;
            res->sizes[res->nsizes][1] = w;
            res->nsizes++;
        }
    }
    if (r->check_ref && r->seen_resize) {
        static gibson_cell_t *cells;
        uint16_t cols = 0, rows = 0;
        if (!read_frame(fb_adapter_context(ad), &cells, &cols, &rows)) {
            res->ref_bad++;
            return;
        }
        refsrc src = {1, &r->prod->pixels[0][0], r->prod->palette, NULL, SYNTH_W, SYNTH_H};
        long ni = 0, nb = 0;
        char f1[200] = "";
        long bad = ref_compare(cells, cols, rows, &src, &r->rc, 0, 0, &ni, &nb, f1, sizeof f1);
        res->ref_frames++;
        if (bad) {
            if (!res->ref_bad) snprintf(res->first, sizeof res->first, "frame %ux%u: %ld bad, %s", cols, rows, bad, f1);
            res->ref_bad += bad;
        }
    }
}

/* Runs the synthetic producer headless through fb_adapter_run. */
static int headless_run(runres *res, const tev *script, size_t n, uint16_t cols, uint16_t rows, int status_rows,
                        fclock *clk, uint32_t divisor, uint64_t max_tics, uint64_t max_iters, unsigned faults,
                        int ignore_input, int check_ref) {
    memset(res, 0, sizeof *res);
    synth_producer *prod = calloc(1, sizeof *prod);
    if (!prod || synth_init(prod, 2112) != 0) return 0;
    prod->ignore_input = ignore_input;
    fb_adapter_config cfg;
    fb_adapter_config_defaults(&cfg, SYNTH_W, SYNTH_H);
    cfg.par_num = 5;
    cfg.par_den = 6;
    cfg.status_rows = (uint8_t)status_rows;
    fb_adapter *ad = NULL;
    if (fb_adapter_open_headless(&cfg, cols, rows, &ad) != FB_OK) {
        synth_free(prod);
        free(prod);
        return 0;
    }
    fb_adapter_set_faults(ad, faults);
    runctx rc;
    memset(&rc, 0, sizeof rc);
    rc.in.ev = script;
    rc.in.n = n;
    rc.prod = prod;
    rc.res = res;
    rc.check_ref = check_ref;
    rc.rc.pn = 5;
    rc.rc.pd = 6;
    rc.rc.cn = 2;
    rc.rc.cd = 1;
    rc.rc.status = status_rows;
    rc.rc.bar = 0;
    rc.rc.status_text = "";
    fb_producer p = synth_producer_iface(prod);
    fb_run_options ro;
    fb_run_options_defaults(&ro);
    ro.clock = fclock_now;
    ro.clock_user = clk;
    ro.block_on_input = 0;
    ro.present_divisor = divisor;
    ro.max_tics = max_tics;
    ro.max_iterations = max_iters;
    ro.pre_tic = run_pre_tic;
    ro.on_present = run_on_present;
    ro.hook_user = &rc;
    fb_status st = fb_adapter_run(ad, &p, &ro, &res->rs);
    res->hash = synth_hash(prod);
    res->tics = res->rs.tics;
    res->frames = res->rs.frames_presented;
    res->dropped = res->rs.tics_dropped;
    res->max_in_iter = res->rs.max_tics_in_iteration;
    res->keys = res->rs.keys_delivered;
    memcpy(res->log, prod->log, sizeof prod->log);
    res->nlog = prod->nlog;
    fb_geometry g;
    fb_adapter_geometry(ad, &g);
    res->map_builds = g.map_builds;
    res->ok = st == FB_OK && rc.in.pushed_fail == 0;
    fb_adapter_close(ad);
    synth_free(prod);
    free(prod);
    return res->ok;
}

#define SP GIBSON_KEY_KIND_SPECIAL
#define CH GIBSON_KEY_KIND_CHAR

/* ---------------------------------------------------------------- P3 */
typedef struct logent {
    uint64_t tic;
    uint32_t code;
    uint8_t pressed;
} logent;

static int cmp_log(const void *a, const void *b) {
    const logent *x = a, *y = b;
    if (x->tic != y->tic) return x->tic < y->tic ? -1 : 1;
    if (x->code != y->code) return x->code < y->code ? -1 : 1;
    return (int)x->pressed - (int)y->pressed;
}

static uint32_t expect_code(uint32_t kind, uint32_t code) {
    if (kind == SP) return FB_KEY_BASE + (code - 0x1000u);
    return (code >= 'A' && code <= 'Z') ? code + 32 : code;
}

/* An independent model of the hold rule: first observation L starts a hold ending L + grace; an
   observation while held re-arms the end to L + repeat; a hold is released at its end tic. */
typedef struct holdk {
    uint32_t code;
    int held;
    uint64_t ends;
} holdk;

static size_t model_holds(const tev *s, size_t n, uint64_t tics, uint64_t grace, uint64_t repeat, logent *out, size_t cap) {
    holdk k[16];
    size_t nk = 0, no = 0;
    for (uint64_t t = 0; t < tics; t++) {
        for (size_t i = 0; i < nk; i++)
            if (k[i].held && k[i].ends <= t && no < cap) {
                out[no++] = (logent){k[i].ends, k[i].code, 0};
                k[i].held = 0;
            }
        for (size_t e = 0; e < n; e++) {
            if (s[e].tic != t || s[e].kind == 0) continue;
            const uint32_t code = expect_code(s[e].kind, s[e].code);
            size_t i = 0;
            while (i < nk && k[i].code != code) i++;
            if (i == nk) {
                if (nk == 16) continue;
                k[nk++] = (holdk){code, 0, 0};
            }
            if (k[i].held) k[i].ends = t + repeat;
            else {
                k[i].held = 1;
                k[i].ends = t + grace;
                if (no < cap) out[no++] = (logent){t, code, 1};
            }
        }
    }
    return no;
}

static const tev S3[] = {
    {10, SP, GIBSON_KEY_RIGHT, 0, 0, 0}, {28, SP, GIBSON_KEY_RIGHT, 0, 0, 0}, {30, SP, GIBSON_KEY_RIGHT, 0, 0, 0},
    {32, SP, GIBSON_KEY_RIGHT, 0, 0, 0}, {60, CH, 'W', 0, 0, 0},              {90, CH, ' ', 0, 0, 0},
    {100, CH, 'a', 0, 0, 0},             {101, CH, 'a', 0, 0, 0},
};

/* Reported releases (#275): the terminal confirms, then w is held 10..100 while d is pressed at 20,
   autorepeats and is released at 40 (the chord an autorepeat-only terminal cannot hold); a tap of
   a pressed and released inside tic 120; Right held 150..155. */
#define REL FB_OBSERVED_RELEASE
static const tev S3R[] = {
    {0, FB_OBSERVED_RELEASES_ACTIVE, 0, 0, 0, 0},
    {10, CH, 'w', 0, 0, 0},       {20, CH, 'd', 0, 0, 0},        {34, CH, 'd', 0, 0, 0},
    {35, CH, 'd', 0, 0, 0},       {40, CH | REL, 'd', 0, 0, 0},  {100, CH | REL, 'w', 0, 0, 0},
    {120, CH, 'a', 0, 0, 0},      {120, CH | REL, 'a', 0, 0, 0}, {150, SP, GIBSON_KEY_RIGHT, 0, 0, 0},
    {155, SP | REL, GIBSON_KEY_RIGHT, 0, 0, 0},
};
/* The producer key log the rule gives, written out by hand. */
static const logent S3R_WANT[] = {
    {10, 'w', 1}, {20, 'd', 1}, {40, 'd', 0}, {100, 'w', 0}, {120, 'a', 1}, {121, 'a', 0},
    {150, FB_KEY_BASE + (GIBSON_KEY_RIGHT - 0x1000u), 1}, {155, FB_KEY_BASE + (GIBSON_KEY_RIGHT - 0x1000u), 0},
};

/* Queue pressure with reported releases (#275 review): the caller pumps without draining until the
   256-entry key queue is full; `a` is released then (its release cannot be queued) and pressed
   again. The release must still reach the producer once there is room, and the second press is
   the one dropped (counted), never the release. Returns 1 when that holds; *dropped gets the count. */
static int queue_pressure_keeps_releases(unsigned long long *dropped, unsigned faults) {
    fb_adapter_config cfg;
    fb_adapter_config_defaults(&cfg, SYNTH_W, SYNTH_H);
    fb_adapter *ad = NULL;
    if (fb_adapter_open_headless(&cfg, 80, 24, &ad) != FB_OK) return 0;
    fb_adapter_set_faults(ad, faults);
    gibson_context_t *ctx = fb_adapter_context(ad);
    gibson_event_t e;
    uint64_t tic = 1;
#define PUSH(type, code)                                                         \
    do {                                                                         \
        memset(&e, 0, sizeof e);                                                 \
        e.event_type = (type);                                                   \
        e.key_code = (code);                                                     \
        if ((type) == GIBSON_EVENT_KEY_RELEASES_ACTIVE) gibson_push_event(ctx, &e); \
        else gibson_push_event_kind(ctx, &e, CH);                                \
        fb_adapter_pump(ad, tic, 0);                                             \
        tic++;                                                                   \
    } while (0)
    PUSH(GIBSON_EVENT_KEY_RELEASES_ACTIVE, 0);
    PUSH(GIBSON_EVENT_KEY, 'a');                   /* 1 queued */
    for (int i = 0; i < 127; i++) {                /* 254 more: b taps, never drained */
        PUSH(GIBSON_EVENT_KEY, 'b');
        PUSH(GIBSON_EVENT_KEY_RELEASE, 'b');
    }
    PUSH(GIBSON_EVENT_KEY, 'c');                   /* 256: full */
    PUSH(GIBSON_EVENT_KEY_RELEASE, 'a');           /* its release cannot be queued */
    PUSH(GIBSON_EVENT_KEY, 'a');                   /* pressed again */
#undef PUSH
    int a_press = 0, a_release = 0, order_ok = 1;
    fb_key_event k;
    for (uint64_t t = tic; t < tic + 600; t++)
        while (fb_adapter_next_key(ad, t, &k))
            if (k.code == 'a') {
                if (k.pressed) a_press++;
                else {
                    a_release++;
                    if (a_press != 1) order_ok = 0;
                }
            }
    fb_adapter_counters c;
    fb_adapter_counters_get(ad, &c);
    *dropped = c.keys_dropped;
    fb_adapter_close(ad);
    return a_press == 1 && a_release == 1 && order_ok && c.keys_dropped == 1;
}

/* Withdrawal (#275 review): grace 25, repeat 100. `a` pressed at 1 and repeated at 2 (inferred
   deadline 102); the terminal confirms at 3; `b` pressed at 3 (held until its release); the
   terminal withdraws at 4. As in gibson::hold, only `b` (waiting for a release) gets a new
   deadline, 4 + 25 = 29; `a` keeps 102. Returns 1 when the releases come at exactly those tics. */
static int withdrawal_keeps_inferred_deadlines(uint64_t *a_end, uint64_t *b_end) {
    fb_adapter_config cfg;
    fb_adapter_config_defaults(&cfg, SYNTH_W, SYNTH_H);
    cfg.hold_repeat_tics = 100;
    fb_adapter *ad = NULL;
    if (fb_adapter_open_headless(&cfg, 80, 24, &ad) != FB_OK) return 0;
    gibson_context_t *ctx = fb_adapter_context(ad);
    static const struct { uint64_t tic; int32_t type; uint32_t code; } S[] = {
        {1, GIBSON_EVENT_KEY, 'a'}, {2, GIBSON_EVENT_KEY, 'a'}, {3, GIBSON_EVENT_KEY_RELEASES_ACTIVE, 0},
        {3, GIBSON_EVENT_KEY, 'b'}, {4, GIBSON_EVENT_KEY_RELEASES_UNSUPPORTED, 0},
    };
    for (size_t i = 0; i < sizeof S / sizeof S[0]; i++) {
        gibson_event_t e;
        memset(&e, 0, sizeof e);
        e.event_type = S[i].type;
        e.key_code = S[i].code;
        if (S[i].type == GIBSON_EVENT_KEY) gibson_push_event_kind(ctx, &e, CH);
        else gibson_push_event(ctx, &e);
        fb_adapter_pump(ad, S[i].tic, 0);
    }
    *a_end = *b_end = 0;
    fb_key_event k;
    for (uint64_t t = 4; t < 200; t++)
        while (fb_adapter_next_key(ad, t, &k))
            if (!k.pressed) {
                if (k.code == 'a') *a_end = k.tic;
                if (k.code == 'b') *b_end = k.tic;
            }
    fb_adapter_close(ad);
    return *a_end == 102 && *b_end == 29;
}

static int prop3(char *ev) {
    fclock a, b;
    memset(&a, 0, sizeof a);
    memset(&b, 0, sizeof b);
    static runres keyed, quiet;
    const int ign = ctl(C_P3_IGNORE_INPUT);
    int ok = headless_run(&keyed, S3, sizeof S3 / sizeof S3[0], 80, 24, 1, &a, 1, 200, 0, 0, ign, 0);
    ok = headless_run(&quiet, NULL, 0, 80, 24, 1, &b, 1, 200, 0, 0, ign, 0) && ok;
    logent want[32], got[SYNTH_LOG];
    const size_t nw = model_holds(S3, sizeof S3 / sizeof S3[0], 200, 25, 4, want, 32);
    for (unsigned i = 0; i < keyed.nlog; i++) got[i] = (logent){keyed.log[i].tic, keyed.log[i].code, keyed.log[i].pressed};
    qsort(want, nw, sizeof *want, cmp_log);
    qsort(got, keyed.nlog, sizeof *got, cmp_log);
    int log_ok = nw == keyed.nlog;
    for (size_t i = 0; log_ok && i < nw; i++) log_ok = cmp_log(&want[i], &got[i]) == 0;
    /* Reported releases, pushed as the terminal's answer and GIBSON_EVENT_KEY_RELEASE events. */
    static runres rel;
    fclock c;
    memset(&c, 0, sizeof c);
    const unsigned rfault = ctl(C_P3_IGNORE_RELEASES) ? FB_FAULT_IGNORE_RELEASES : 0;
    ok = headless_run(&rel, S3R, sizeof S3R / sizeof S3R[0], 80, 24, 1, &c, 1, 200, 0, rfault, 0, 0) && ok;
    const size_t nr = sizeof S3R_WANT / sizeof S3R_WANT[0];
    static logent rgot[SYNTH_LOG];
    for (unsigned i = 0; i < rel.nlog; i++) rgot[i] = (logent){rel.log[i].tic, rel.log[i].code, rel.log[i].pressed};
    qsort(rgot, rel.nlog, sizeof *rgot, cmp_log);
    int rel_ok = rel.nlog == nr;
    for (size_t i = 0; rel_ok && i < nr; i++) rel_ok = cmp_log(&S3R_WANT[i], &rgot[i]) == 0;
    unsigned long long qdropped = 0;
    const int qok = queue_pressure_keeps_releases(&qdropped, ctl(C_P3_REARM_LOSES_RELEASE) ? FB_FAULT_REARM_PENDING : 0);
    uint64_t wa = 0, wb = 0;
    const int wok = withdrawal_keeps_inferred_deadlines(&wa, &wb);
    const int pass = ok && keyed.hash != quiet.hash && log_ok && keyed.tics == 200 && quiet.tics == 200 && rel_ok && qok && wok;
    ev_append(ev, "keys pushed with gibson_push_event_kind, polled by the adapter (gibson_poll_event_kind): keyed hash=%016llx %s "
                  "unkeyed hash=%016llx (200 tics each); producer key log %u events %s independent hold model (grace 25, repeat 4): ",
              (unsigned long long)keyed.hash, keyed.hash != quiet.hash ? "!=" : "==", (unsigned long long)quiet.hash,
              keyed.nlog, log_ok ? "==" : "!=");
    for (size_t i = 0; i < nw && i < 8; i++)
        ev_append(ev, "%s%s@%llu", i ? " " : "", want[i].pressed ? "press" : "release", (unsigned long long)want[i].tic);
    ev_append(ev, "; reported releases (KEY_RELEASES_ACTIVE, then KEY_RELEASE events): producer key log %u events %s "
                  "the rule (w held 10..100 across a d chord 20..40, a tap at 120 held one tic, Right 150..155):",
              rel.nlog, rel_ok ? "==" : "!=");
    for (unsigned i = 0; i < rel.nlog && i < 10; i++)
        ev_append(ev, " %s@%llu", rgot[i].pressed ? "press" : "release", (unsigned long long)rgot[i].tic);
    ev_append(ev, "; a full key queue keeps a reported release (a's release delivered once there is room, its re-press dropped: "
                  "%s, keys_dropped=%llu); a withdrawal re-times only holds waiting for a release (a ends at %llu, want 102; "
                  "b at %llu, want 29)", qok ? "yes" : "NO", qdropped, (unsigned long long)wa, (unsigned long long)wb);
    if (ign) ev_append(ev, " [control: producer ignores input]");
    if (rfault) ev_append(ev, " [control: reported releases ignored]");
    return pass;
}

/* ---------------------------------------------------------------- P4 */
static const tev S4[] = {
    {5, SP, GIBSON_KEY_RIGHT, 0, 0, 0},   {23, SP, GIBSON_KEY_RIGHT, 0, 0, 0}, {25, SP, GIBSON_KEY_RIGHT, 0, 0, 0},
    {27, SP, GIBSON_KEY_RIGHT, 0, 0, 0},  {29, SP, GIBSON_KEY_RIGHT, 0, 0, 0}, {31, SP, GIBSON_KEY_RIGHT, 0, 0, 0},
    {33, SP, GIBSON_KEY_RIGHT, 0, 0, 0},  {70, CH, 'w', 0, 0, 0},              {88, CH, 'w', 0, 0, 0},
    {90, CH, 'w', 0, 0, 0},               {120, CH, ' ', 0, 0, 0},             {150, SP, GIBSON_KEY_LEFT, 0, 0, 0},
    {151, CH, 's', 0, 0, 0},              {168, SP, GIBSON_KEY_LEFT, 0, 0, 0}, {170, SP, GIBSON_KEY_LEFT, 0, 0, 0},
    {200, CH, 'd', 0, 0, 0},              {260, SP, GIBSON_KEY_UP, 0, 0, 0},   {300, CH, ' ', 0, 0, 0},
    {330, CH, 'A', 0, 0, 0},              {331, CH, 'a', 0, 0, 0},             {332, CH, 'a', 0, 0, 0},
    {400, CH, 'x', 0, 0, 0},
};
#define N4 (sizeof S4 / sizeof S4[0])

static int prop4(char *ev) {
    const unsigned fault = ctl(C_P4_FRAME_LOCKED) ? FB_FAULT_FRAME_LOCKED_SIM : 0;
    static runres r1, r4, r0, rj, rs;
    fclock c1, c4, c0, cj, cs;
    memset(&c1, 0, sizeof c1);
    memset(&c4, 0, sizeof c4);
    memset(&c0, 0, sizeof c0);
    memset(&cj, 0, sizeof cj);
    memset(&cs, 0, sizeof cs);
    /* Same simulated wall time (421 loop iterations at one tic each = 420 tics owed), three cadences. */
    int ok = headless_run(&r1, S4, N4, 80, 24, 1, &c1, 1, 0, 421, fault, 0, 0);
    ok = headless_run(&r4, S4, N4, 80, 24, 1, &c4, 4, 0, 421, fault, 0, 0) && ok;
    ok = headless_run(&r0, S4, N4, 80, 24, 1, &c0, 0, 0, 421, fault, 0, 0) && ok;
    /* A jittery clock (0..10 tics per iteration) and a 2 s stall, to 420 tics. */
    static const uint8_t jitter[] = {1, 0, 3, 1, 0, 0, 7, 2, 10, 1, 4};
    cj.steps = jitter;
    cj.nsteps = sizeof jitter;
    ok = headless_run(&rj, S4, N4, 80, 24, 1, &cj, 1, 420, 100000, fault, 0, 0) && ok;
    cs.stall_call = 100;
    cs.stall_tics = 70;
    ok = headless_run(&rs, S4, N4, 80, 24, 1, &cs, 2, 420, 100000, fault, 0, 0) && ok;
    const int same = r1.hash == r4.hash && r1.hash == r0.hash && r1.hash == rj.hash && r1.hash == rs.hash;
    const int tics_ok = r1.tics == 420 && r4.tics == 420 && r0.tics == 420 && rj.tics == 420 && rs.tics == 420;
    const int frames_ok = r1.frames == 420 && r4.frames == 105 && r0.frames == 0;
    const int catchup_ok = rj.max_in_iter == 10 && rj.dropped == 0 && rs.dropped == 60 && rs.max_in_iter == 10;
    const int pass = ok && same && tics_ok && frames_ok && catchup_ok;
    ev_append(ev, "same 22-event tic-stamped script, same wall time: present every tic hash=%016llx (%llu tics, %llu frames), "
                  "every 4th hash=%016llx (%llu tics, %llu frames), never hash=%016llx (%llu tics, %llu frames); "
                  "jitter clock (0-10 tics/iteration) hash=%016llx max %llu tics/iteration dropped %llu; "
                  "2.0 s stall hash=%016llx ran %llu in that iteration, dropped %llu (cap 10)",
              (unsigned long long)r1.hash, (unsigned long long)r1.tics, (unsigned long long)r1.frames,
              (unsigned long long)r4.hash, (unsigned long long)r4.tics, (unsigned long long)r4.frames,
              (unsigned long long)r0.hash, (unsigned long long)r0.tics, (unsigned long long)r0.frames,
              (unsigned long long)rj.hash, (unsigned long long)rj.max_in_iter, (unsigned long long)rj.dropped,
              (unsigned long long)rs.hash, (unsigned long long)rs.max_in_iter, (unsigned long long)rs.dropped);
    if (fault) ev_append(ev, " [control: mutant frame-locked simulation]");
    return pass;
}

/* ---------------------------------------------------------------- P5 */
static int prop5(char *ev) {
    unsigned fault = 0;
    if (ctl(C_P5_STALE_MAP)) fault = FB_FAULT_STALE_MAP_ON_RESIZE;
    if (ctl(C_P5_RESIZE_STEPS)) fault = FB_FAULT_RESIZE_STEPS_PRODUCER;
    tev script[N4 + 2];
    size_t n = 0;
    for (size_t i = 0; i < N4; i++) {
        if (S4[i].tic >= 300) break;
        if (S4[i].tic > 150 && n == (size_t)(&S4[i] - S4)) script[n++] = (tev){150, 0, 0, 0, 120, 40};
        if (S4[i].tic > 220 && script[n - 1].tic < 220) script[n++] = (tev){220, 0, 0, 0, 40, 15};
        script[n++] = S4[i];
    }
    static runres rr, rn;
    fclock a, b;
    memset(&a, 0, sizeof a);
    memset(&b, 0, sizeof b);
    int ok = headless_run(&rr, script, n, 80, 24, 1, &a, 1, 300, 0, fault, 0, 1);
    tev noresize[N4];
    size_t m = 0;
    for (size_t i = 0; i < n; i++)
        if (script[i].kind != 0) noresize[m++] = script[i];
    ok = headless_run(&rn, noresize, m, 80, 24, 1, &b, 1, 300, 0, fault, 0, 0) && ok;
    int sizes_ok = rr.nsizes == 3 && rr.sizes[0][0] == 80 && rr.sizes[0][1] == 24 && rr.sizes[1][0] == 120 &&
                   rr.sizes[1][1] == 40 && rr.sizes[2][0] == 40 && rr.sizes[2][1] == 15;
    const int pass = ok && rr.hash == rn.hash && rr.tics == 300 && sizes_ok && rr.map_builds == 3 && rr.ref_bad == 0 &&
                     rr.ref_frames > 100;
    ev_append(ev, "RESIZE pushed at tic 150 (120x40) and 220 (40x15): hash=%016llx %s no-resize hash=%016llx (300 tics); "
                  "frame size readback ",
              (unsigned long long)rr.hash, rr.hash == rn.hash ? "==" : "!=", (unsigned long long)rn.hash);
    for (int i = 0; i < rr.nsizes; i++) ev_append(ev, "%s%ux%u", i ? "->" : "", rr.sizes[i][0], rr.sizes[i][1]);
    ev_append(ev, "; map builds %llu (want 3); %ld post-resize frames vs independent reference: %ld bad cells",
              (unsigned long long)rr.map_builds, rr.ref_frames, rr.ref_bad);
    if (rr.first[0]) ev_append(ev, " (first: %s)", rr.first);
    if (fault) ev_append(ev, " [control: %s]", CONTROLS[g_control].what);
    return pass;
}

/* ---------------------------------------------------------------- P6: a real PTY, driven from C */
typedef struct ptyc {
    int master, slave;
    pid_t pid;
    long bytes;
    struct termios before; /* the slave's termios before the child existed */
} ptyc;

static int pty_spawn(ptyc *pc, char *const argv[], int ignore_hup) {
    memset(pc, 0, sizeof *pc);
    pc->master = pc->slave = -1;
    int m = posix_openpt(O_RDWR | O_NOCTTY);
    if (m < 0 || grantpt(m) != 0 || unlockpt(m) != 0) return 0;
    char name[128];
    if (ptsname_r(m, name, sizeof name) != 0) return 0;
    int s = open(name, O_RDWR | O_NOCTTY);
    if (s < 0) return 0;
    struct winsize ws = {24, 80, 0, 0};
    ioctl(m, TIOCSWINSZ, &ws);
    /* The baseline is taken before the child exists, so a child that enters raw mode at once
       cannot contaminate it. */
    if (tcgetattr(s, &pc->before) != 0) return 0;
    pid_t pid = fork();
    if (pid < 0) return 0;
    if (pid == 0) {
        setsid();
        ioctl(s, TIOCSCTTY, 0);
        dup2(s, 0);
        dup2(s, 1);
        dup2(s, 2);
        if (s > 2) close(s);
        close(m);
        signal(SIGHUP, ignore_hup ? SIG_IGN : SIG_DFL);
        signal(SIGTERM, SIG_DFL);
        signal(SIGINT, SIG_DFL);
        setenv("TERM", "xterm-256color", 1);
        setenv("COLORTERM", "truecolor", 1);
        unsetenv("DISPLAY"); /* never reach a desktop from a test child */
        unsetenv("WAYLAND_DISPLAY");
        execv(g_exe, argv);
        _exit(127);
    }
    fcntl(m, F_SETFL, fcntl(m, F_GETFL) | O_NONBLOCK);
    pc->master = m;
    pc->slave = s;
    pc->pid = pid;
    return 1;
}

static void pty_drain(ptyc *pc, int ms) {
    if (pc->master < 0) {
        if (ms > 0) usleep((useconds_t)ms * 1000);
        return;
    }
    struct pollfd p = {pc->master, POLLIN, 0};
    if (poll(&p, 1, ms) > 0) {
        char buf[65536];
        ssize_t r;
        while ((r = read(pc->master, buf, sizeof buf)) > 0) pc->bytes += r;
    }
}

static int termios_same(const struct termios *a, const struct termios *b) {
    return a->c_iflag == b->c_iflag && a->c_oflag == b->c_oflag && a->c_cflag == b->c_cflag && a->c_lflag == b->c_lflag &&
           memcmp(a->c_cc, b->c_cc, sizeof a->c_cc) == 0 && cfgetispeed(a) == cfgetispeed(b) && cfgetospeed(a) == cfgetospeed(b);
}

/* Waits until the child has painted and put the terminal in raw mode (ICANON off). */
static int pty_wait_raw(ptyc *pc, int timeout_ms) {
    const uint64_t end = now_ns() + (uint64_t)timeout_ms * 1000000ull;
    while (now_ns() < end) {
        pty_drain(pc, 20);
        struct termios t;
        if (pc->bytes > 2000 && tcgetattr(pc->slave, &t) == 0 && !(t.c_lflag & ICANON)) return 1;
    }
    return 0;
}

static int pty_wait_exit(ptyc *pc, int timeout_ms, int *status, struct rusage *ru, double *secs) {
    const uint64_t t0 = now_ns(), end = t0 + (uint64_t)timeout_ms * 1000000ull;
    while (now_ns() < end) {
        pty_drain(pc, 10);
        pid_t r = wait4(pc->pid, status, WNOHANG, ru);
        if (r == pc->pid) {
            *secs = (double)(now_ns() - t0) / 1e9;
            return 1;
        }
    }
    kill(pc->pid, SIGKILL);
    wait4(pc->pid, status, 0, ru);
    *secs = (double)(now_ns() - t0) / 1e9;
    return 0;
}

/* The whole report, its lines joined by spaces. */
static void read_report(const char *path, char *out, size_t cap) {
    out[0] = '\0';
    FILE *f = fopen(path, "r");
    if (!f) return;
    size_t n = fread(out, 1, cap - 1, f);
    fclose(f);
    out[n] = '\0';
    for (size_t i = 0; i < n; i++)
        if (out[i] == '\n') out[i] = ' ';
}

static const char *report_field(const char *rep, const char *key, char *val, size_t cap) {
    char k[64];
    snprintf(k, sizeof k, "%s=", key);
    const char *p = rep;
    val[0] = '\0';
    for (;;) { /* a whole token: at the start or after a space */
        p = strstr(p, k);
        if (!p) return val;
        if (p == rep || p[-1] == ' ') break;
        p++;
    }
    p += strlen(k);
    size_t i = 0;
    while (p[i] && p[i] != ' ' && i + 1 < cap) {
        val[i] = p[i];
        i++;
    }
    val[i] = '\0';
    return val;
}

static double cpu_s(const struct rusage *ru) {
    return (double)(ru->ru_utime.tv_sec + ru->ru_stime.tv_sec) + (double)(ru->ru_utime.tv_usec + ru->ru_stime.tv_usec) / 1e6;
}

/* One P6 scenario: 0 = 'q', 1 = SIGTERM, 2 = hangup (master closed, child ignores SIGHUP). */
static int p6_case(int which, const char *dir, char *ev) {
    char report[512];
    snprintf(report, sizeof report, "%s/p6_%d.report", dir, which);
    unlink(report);
    char *argv[16];
    int a = 0;
    argv[a++] = g_exe;
    argv[a++] = "--play";
    argv[a++] = "--report";
    argv[a++] = report;
    argv[a++] = "--exit-after-tics";
    argv[a++] = "3500";
    if (which == 0 && ctl(C_P6_NO_RESTORE)) argv[a++] = "--control-no-restore";
    if (which == 1 && ctl(C_P6_NO_SIGNALS)) argv[a++] = "--control-no-signals";
    if (which == 2 && ctl(C_P6_RETRY_IO)) {
        argv[a++] = "--fault";
        argv[a++] = "retry-io";
    }
    if (which == 3) {
        argv[a++] = "--reopen-after-signal";
        argv[a++] = "35";
        if (ctl(C_P6_STICKY_SIGNAL)) {
            argv[a++] = "--fault";
            argv[a++] = "sticky-signal";
        }
    }
    argv[a] = NULL;
    static const char *names[] = {"quit 'q'", "SIGTERM", "hangup", "SIGTERM then a second adapter"};
    ptyc pc;
    if (!pty_spawn(&pc, argv, which == 2)) {
        ev_append(ev, "%s: cannot open a pty (%s); ", names[which], strerror(errno));
        return 0;
    }
    struct termios during, after;
    const struct termios before = pc.before; /* taken before fork */
    const int raw = pty_wait_raw(&pc, 8000);
    /* let the loop run in steady state (~35 tics) before acting, so the exit is from a live loop */
    for (const uint64_t settle = now_ns() + 1000000000ull; now_ns() < settle;) pty_drain(&pc, 20);
    tcgetattr(pc.slave, &during);
    const int changed = !termios_same(&before, &during);
    if (which == 0) {
        if (write(pc.master, "q", 1) != 1) ev_append(ev, "(write q failed) ");
    } else if (which == 1 || which == 3) {
        kill(pc.pid, SIGTERM);
    } else {
        close(pc.master);
        pc.master = -1;
    }
    int status = 0;
    struct rusage ru;
    memset(&ru, 0, sizeof ru);
    double secs = 0;
    const int exited = pty_wait_exit(&pc, 5000, &status, &ru, &secs);
    char rep[2048], ex[32], tics[32];
    read_report(report, rep, sizeof rep);
    report_field(rep, "exit", ex, sizeof ex);
    report_field(rep, "tics", tics, sizeof tics);
    int ok = raw && changed && exited;
    if (which == 3) {
        char ex2[32], t2[32], s2[16];
        report_field(rep, "second_exit", ex2, sizeof ex2);
        report_field(rep, "second_tics", t2, sizeof t2);
        report_field(rep, "second_signal", s2, sizeof s2);
        const int r = tcgetattr(pc.slave, &after);
        const int restored = r == 0 && termios_same(&before, &after);
        ok = ok && restored && strcmp(ex, "signal") == 0 && strcmp(ex2, "tic_limit") == 0 && strcmp(t2, "35") == 0 &&
             strcmp(s2, "0") == 0 && WIFEXITED(status) && WEXITSTATUS(status) == 0;
        ev_append(ev, "%s: first adapter exit=%s after %s tics, second adapter (same process, after the handled "
                      "signal) exit=%s after %s tics (want tic_limit after 35), termios after %s before, status %s %d; ",
                  names[which], ex[0] ? ex : "(no report)", tics[0] ? tics : "?", ex2[0] ? ex2 : "(none)", t2[0] ? t2 : "?",
                  restored ? "==" : "!=", WIFSIGNALED(status) ? "signal" : "code",
                  WIFSIGNALED(status) ? WTERMSIG(status) : WEXITSTATUS(status));
    } else if (which == 2) {
        errno = 0;
        const int r = tcgetattr(pc.slave, &after);
        const int eio = r != 0 && errno == EIO;
        ok = ok && strcmp(ex, "hangup") == 0 && WIFEXITED(status) && WEXITSTATUS(status) == 0 && secs < 2.0 && cpu_s(&ru) < 3.0;
        ev_append(ev, "%s: raw during run=%s, exit=%s (status %s %d) %.2fs after the master closed, child cpu %.2fs, "
                      "termios after: %s; ",
                  names[which], raw && changed ? "yes" : "no", ex[0] ? ex : "(no report)",
                  WIFSIGNALED(status) ? "signal" : "code", WIFSIGNALED(status) ? WTERMSIG(status) : WEXITSTATUS(status), secs,
                  cpu_s(&ru), eio ? "unreadable (EIO: the kernel hangs up every slave fd)" : (r == 0 ? "readable" : strerror(errno)));
    } else {
        const int r = tcgetattr(pc.slave, &after);
        const int restored = r == 0 && termios_same(&before, &after);
        const int exit_ok = which == 0 ? (strcmp(ex, "quit") == 0 && WIFEXITED(status) && WEXITSTATUS(status) == 0)
                                       : (strcmp(ex, "signal") == 0 && WIFSIGNALED(status) && WTERMSIG(status) == SIGTERM);
        ok = ok && restored && exit_ok;
        ev_append(ev, "%s: raw during run=%s, termios after %s before, exit=%s (status %s %d) in %.2fs after %s tics; ",
                  names[which], raw && changed ? "yes" : "no", restored ? "==" : "!=", ex[0] ? ex : "(no report)",
                  WIFSIGNALED(status) ? "signal" : "code", WIFSIGNALED(status) ? WTERMSIG(status) : WEXITSTATUS(status), secs,
                  tics[0] ? tics : "?");
    }
    if (pc.master >= 0) close(pc.master);
    close(pc.slave);
    unlink(report);
    return ok;
}

static int prop6(char *ev, const char *dir) {
    int pass = 1;
    if (g_control == C_NONE || ctl(C_P6_NO_RESTORE)) pass = p6_case(0, dir, ev) && pass;
    if (g_control == C_NONE || ctl(C_P6_NO_SIGNALS)) pass = p6_case(1, dir, ev) && pass;
    if (g_control == C_NONE || ctl(C_P6_RETRY_IO)) pass = p6_case(2, dir, ev) && pass;
    if (g_control == C_NONE || ctl(C_P6_STICKY_SIGNAL)) pass = p6_case(3, dir, ev) && pass;
    ev_append(ev, "(pty 80x24, termios via tcgetattr on the slave, baseline taken before fork)");
    if (g_control != C_NONE) ev_append(ev, " [control: %s]", CONTROLS[g_control].what);
    return pass;
}

/* ---------------------------------------------------------------- P7 */
/* Child: a fresh process image per size, so VmHWM is this run's own high-water mark. */
#define P7_WARM 200
#define P7_WINDOW 500
#define P7_MIN_FRAMES (P7_WARM + 2 * P7_WINDOW) /* warm-up, then an early and a late window */

static int run_p7_child(uint16_t cols, uint16_t rows, uint64_t frames, const char *control) {
    if (cols == 0 || rows == 0 || frames < P7_MIN_FRAMES || frames > 1000000) {
        fprintf(stderr, "fb_adapter_demo: --p7-child needs a non-zero grid and %d..1000000 frames "
                        "(%d warm-up + two %d-frame windows), got %ux%u and %llu\n",
                P7_MIN_FRAMES, P7_WARM, P7_WINDOW, cols, rows, (unsigned long long)frames);
        return 2;
    }
    fb_adapter_config cfg;
    fb_adapter_config_defaults(&cfg, SYNTH_W, SYNTH_H);
    cfg.par_num = 5;
    cfg.par_den = 6;
    cfg.status_rows = 1;
    if (strcmp(control, "no-rotation") == 0) cfg.headless_capture_limit = 0;
    synth_producer *prod = calloc(1, sizeof *prod);
    if (!prod || synth_init(prod, 7) != 0) return 1;
    fb_adapter *ad = NULL;
    if (fb_adapter_open_headless(&cfg, cols, rows, &ad) != FB_OK) return 1;
    if (strcmp(control, "leak") == 0) fb_adapter_set_faults(ad, FB_FAULT_LEAK_PER_FRAME);
    if (strcmp(control, "slow-present") == 0) fb_adapter_set_faults(ad, FB_FAULT_SLOW_PRESENT);
    demo d;
    memset(&d, 0, sizeof d);
    d.prod = prod;
    d.inner = synth_producer_iface(prod);
    d.bench = 1;
    d.cap = (size_t)frames;
    d.warm_at = P7_WARM;
    d.mid_at = P7_WARM + (d.cap - P7_WARM) / 2; /* the leak gate compares the second half only */
    d.lower = calloc(d.cap, sizeof *d.lower);
    d.render = calloc(d.cap, sizeof *d.render);
    if (!d.lower || !d.render) return 1;
    fb_producer p = {&d, demo_key, demo_tic, demo_draw};
    fb_run_options ro;
    fb_run_options_defaults(&ro);
    fclock fc;
    memset(&fc, 0, sizeof fc);
    ro.clock = fclock_now;
    ro.clock_user = &fc;
    ro.block_on_input = 0;
    ro.max_tics = frames;
    ro.on_present = demo_on_present;
    ro.hook_user = &d;
    fb_run_stats rs;
    d.stats = &rs;
    if (fb_adapter_run(ad, &p, &ro, &rs) != FB_OK || d.n != d.cap) return 1;
    uint64_t *tot = malloc(d.n * sizeof *tot);
    if (!tot) return 1;
    for (size_t i = 0; i < d.n; i++) tot[i] = d.lower[i] + d.render[i];
    double lm, lp, rm, rp, tm, tp, early, late, dummy;
    /* d.n == frames >= P7_MIN_FRAMES (checked above, and the run presented every frame), so every
       window below lies inside the d.n samples. */
    stats_us(d.lower, P7_WARM, d.n, &lm, &lp);
    stats_us(d.render, P7_WARM, d.n, &rm, &rp);
    stats_us(tot, P7_WARM, d.n, &tm, &tp);
    stats_us(tot, P7_WARM, P7_WARM + P7_WINDOW, &early, &dummy);
    stats_us(tot, d.n - P7_WINDOW, d.n, &late, &dummy);
    fb_adapter_counters c;
    fb_adapter_counters_get(ad, &c);
    size_t cap_len = 0;
    gibson_get_rendered_output(fb_adapter_context(ad), NULL, 0, &cap_len);
    printf("P7CHILD %u %u %zu %.1f %.1f %.1f %.1f %.1f %.1f %.1f %.1f %ld %ld %ld %ld %ld %ld %llu %zu %ld\n", cols, rows, d.n,
           lm, lp, rm, rp, tm, tp, early, late, d.hwm_warm, proc_status_kb("VmHWM"), d.maxrss_warm, maxrss_kb(), d.rss_warm,
           proc_status_kb("VmRSS"), (unsigned long long)c.capture_rotations, cap_len, d.hwm_mid);
    free(tot);
    fb_adapter_close(ad);
    return 0;
}

static int prop7(char *ev) {
    static const uint16_t sizes[][2] = {{80, 24}, {160, 50}, {320, 100}};
    const char *control = ctl(C_P7_LEAK) ? "leak" : ctl(C_P7_NO_ROTATION) ? "no-rotation"
                        : ctl(C_P7_SLOW_PRESENT) ? "slow-present" : "none";
    const long tol_kb = 1024;
    const double budget_us = 1e6 / 35.0; /* one 35 Hz tic: 28571 us */
    /* A control needs one size to show the check failing (and no-rotation at 320x100 would grow a
       child to ~700 MB on a shared box). */
    const int nsizes = g_control == C_NONE ? 3 : 1;
    int pass = 1;
    for (int i = 0; i < nsizes; i++) {
        int pfd[2];
        if (pipe(pfd) != 0) return 0;
        pid_t pid = fork();
        if (pid == 0) {
            dup2(pfd[1], 1);
            close(pfd[0]);
            close(pfd[1]);
            unsetenv("DISPLAY");
            unsetenv("WAYLAND_DISPLAY");
            char c[16], r[16];
            snprintf(c, sizeof c, "%u", sizes[i][0]);
            snprintf(r, sizeof r, "%u", sizes[i][1]);
            execl(g_exe, g_exe, "--p7-child", c, r, "2000", control, (char *)NULL);
            _exit(127);
        }
        close(pfd[1]);
        char line[1024] = "";
        FILE *f = fdopen(pfd[0], "r");
        if (f) {
            if (!fgets(line, sizeof line, f)) line[0] = '\0';
            fclose(f);
        }
        int status = 0;
        waitpid(pid, &status, 0);
        unsigned cc, rr;
        size_t n, caplen;
        double lm, lp, rm, rp, tm, tp, early, late;
        long hw0, hw1, mr0, mr1, rs0, rs1, hwm;
        unsigned long long rot;
        if (sscanf(line, "P7CHILD %u %u %zu %lf %lf %lf %lf %lf %lf %lf %lf %ld %ld %ld %ld %ld %ld %llu %zu %ld", &cc, &rr, &n,
                   &lm, &lp, &rm, &rp, &tm, &tp, &early, &late, &hw0, &hw1, &mr0, &mr1, &rs0, &rs1, &rot, &caplen, &hwm) != 20 ||
            !WIFEXITED(status) || WEXITSTATUS(status) != 0) {
            ev_append(ev, "%ux%u: child failed; ", sizes[i][0], sizes[i][1]);
            pass = 0;
            continue;
        }
        /* A leak grows in proportion to frames; the allocator settles once (measured: 0 to 2.8 MiB
           at 320x100 between frames 200 and ~1000, then flat to frame 6000). So the gate is the
           second half's growth; the first half's is reported as settling. */
        const long settle = hwm - hw0, growth = hw1 - hwm;
        const int mem_ok = growth <= tol_kb;
        const int creep_ok = late <= 1.5 * early;
        const int budget_ok = tp <= budget_us;
        if (!mem_ok || !creep_ok || !budget_ok) pass = 0;
        ev_append(ev, "%ux%u: %zu frames, VmHWM %ld->%ld->%ld KiB at frames 200/%zu/%zu (settling %+ld, not gated; "
                      "second half %+ld, gate <= %ld: %s) ru_maxrss %ld->%ld KiB, lower mean %.0f p99 %.0f us, "
                      "render mean %.0f p99 %.0f us, total mean %.0f p99 %.0f us (gate p99 <= %.0f us: %s), "
                      "late/early mean %.2f (gate <= 1.50: %s), capture rotations %llu, capture now %zu B; ",
                  cc, rr, n, hw0, hwm, hw1, (size_t)(P7_WARM + (n - P7_WARM) / 2), n, settle, growth, tol_kb,
                  mem_ok ? "pass" : "FAIL", mr0, mr1, lm, lp, rm, rp, tm, tp, budget_us, budget_ok ? "pass" : "FAIL",
                  early > 0 ? late / early : 0, creep_ok ? "pass" : "FAIL", rot, caplen);
    }
    ev_append(ev, "(headless, frames after a 200-frame warm-up, fresh process per size)");
    if (g_control != C_NONE) ev_append(ev, " [control: %s]", CONTROLS[g_control].what);
    return pass;
}

static int selfcheck(int only) {
    make_patterns();
    char dir[] = "/tmp/fbadapter-selfcheck.XXXXXX";
    const char *tmp = getenv("TMPDIR");
    char dirbuf[1024];
    snprintf(dirbuf, sizeof dirbuf, "%s/fbadapter-selfcheck.XXXXXX", tmp && *tmp ? tmp : "/tmp");
    char *d = mkdtemp(dirbuf);
    if (!d) d = mkdtemp(dir);
    if (!d) {
        printf("fb_adapter: FAIL (cannot create a temp dir)\n");
        return 1;
    }
    static const char *names[] = {"", "framebuffer_submission", "scaling_halfblock_geometry", "input_to_simulation",
                                  "tics_independent_of_presentation", "resize_preserves_state", "terminal_restoration",
                                  "bounded_memory_and_frame_cost"};
    int all = 1, ran = 0;
    for (int p = 1; p <= 7; p++) {
        if (only && p != only) continue;
        if (g_control != C_NONE && CONTROLS[g_control].prop != p) continue;
        char *ev = calloc(1, EVID);
        if (!ev) return 1;
        int ok = 0;
        switch (p) {
        case 1: ok = prop1(ev); break;
        case 2: ok = prop2(ev); break;
        case 3: ok = prop3(ev); break;
        case 4: ok = prop4(ev); break;
        case 5: ok = prop5(ev); break;
        case 6: ok = prop6(ev, d); break;
        case 7: ok = prop7(ev); break;
        }
        printf("PROP %d %s: %s %s\n", p, names[p], ok ? "PASS" : "FAIL", ev);
        fflush(stdout);
        free(ev);
        all = all && ok;
        ran++;
    }
    rmdir(d);
    if (g_control != C_NONE) {
        printf("fb_adapter: %s (control %s: %s)\n", all ? "OK" : "FAIL", CONTROLS[g_control].name, CONTROLS[g_control].what);
        return all ? 0 : 1;
    }
    printf("fb_adapter: %s\n", all && ran ? "OK" : "FAIL");
    return all && ran ? 0 : 1;
}

/* ================================================================ main */

static void usage(void) {
    fprintf(stderr,
            "usage: fb_adapter_demo [--play] [--exit-after-tics N] [--report FILE] [--trace FILE] [--square-pixels]\n"
            "                       [--seed N] [--hold-grace TICS] [--hold-repeat TICS] [--tic-hz N]\n"
            "                       [--color-depth mono|16|256|true] [--bench-frames N]\n"
            "       fb_adapter_demo --selfcheck [--only N] [--control NAME] [--list-controls]\n"
            "       fb_adapter_demo --replay TRACE --tics N [--grid CxR] [--dump-cells FILE]\n");
}

int main(int argc, char **argv) {
    ssize_t n = readlink("/proc/self/exe", g_exe, sizeof g_exe - 1);
    if (n > 0) g_exe[n] = '\0';
    else snprintf(g_exe, sizeof g_exe, "%s", argv[0]);

    play_opts o;
    memset(&o, 0, sizeof o);
    o.depth = -1;
    int mode = 0, only = 0; /* 0 play, 1 selfcheck, 2 replay */
    for (int i = 1; i < argc; i++) {
        const char *a = argv[i];
        const char *v = i + 1 < argc ? argv[i + 1] : NULL;
#define NEEDV() do { if (!v) { usage(); return 2; } i++; } while (0)
        if (!strcmp(a, "--play")) mode = 0;
        else if (!strcmp(a, "--selfcheck")) mode = 1;
        else if (!strcmp(a, "--list-controls")) {
            for (int c = 1; c < C_COUNT; c++) printf("%s\tP%d\t%s\n", CONTROLS[c].name, CONTROLS[c].prop, CONTROLS[c].what);
            return 0;
        } else if (!strcmp(a, "--p7-child")) {
            if (i + 4 >= argc) return 2;
            return run_p7_child((uint16_t)atoi(argv[i + 1]), (uint16_t)atoi(argv[i + 2]), strtoull(argv[i + 3], NULL, 10), argv[i + 4]);
        } else if (!strcmp(a, "--only")) { NEEDV(); only = atoi(v); }
        else if (!strcmp(a, "--control")) {
            NEEDV();
            g_control = -1;
            for (int c = 1; c < C_COUNT; c++)
                if (!strcmp(v, CONTROLS[c].name)) g_control = c;
            if (g_control < 0) { fprintf(stderr, "unknown control %s (see --list-controls)\n", v); return 2; }
        } else if (!strcmp(a, "--replay")) { NEEDV(); o.replay = v; mode = 2; }
        else if (!strcmp(a, "--tics")) { NEEDV(); o.replay_tics = strtoull(v, NULL, 10); }
        else if (!strcmp(a, "--grid")) {
            NEEDV();
            unsigned c = 0, r = 0;
            if (sscanf(v, "%ux%u", &c, &r) != 2 || !c || !r) { usage(); return 2; }
            o.grid_cols = (uint16_t)c;
            o.grid_rows = (uint16_t)r;
        } else if (!strcmp(a, "--dump-cells")) { NEEDV(); o.dump_cells = v; }
        else if (!strcmp(a, "--reopen-after-signal")) { NEEDV(); o.reopen_tics = strtoull(v, NULL, 10); }
        else if (!strcmp(a, "--exit-after-tics")) { NEEDV(); o.exit_after_tics = strtoull(v, NULL, 10); }
        else if (!strcmp(a, "--bench-frames")) { NEEDV(); o.bench_frames = strtoull(v, NULL, 10); }
        else if (!strcmp(a, "--seed")) { NEEDV(); o.seed = strtoull(v, NULL, 10); }
        else if (!strcmp(a, "--report")) { NEEDV(); o.report = v; }
        else if (!strcmp(a, "--trace")) { NEEDV(); o.trace = v; }
        else if (!strcmp(a, "--hold-grace")) { NEEDV(); o.grace = (uint32_t)atoi(v); }
        else if (!strcmp(a, "--hold-repeat")) { NEEDV(); o.repeat = (uint32_t)atoi(v); }
        else if (!strcmp(a, "--tic-hz")) { NEEDV(); o.hz = (uint32_t)atoi(v); }
        else if (!strcmp(a, "--square-pixels")) o.square = 1;
        else if (!strcmp(a, "--color-depth")) {
            NEEDV();
            o.depth = !strcmp(v, "mono") ? GIBSON_COLOR_DEPTH_MONO : !strcmp(v, "16") ? GIBSON_COLOR_DEPTH_ANSI16
                    : !strcmp(v, "256") ? GIBSON_COLOR_DEPTH_ANSI256 : GIBSON_COLOR_DEPTH_TRUECOLOR;
        } else if (!strcmp(a, "--control-ignore-input")) o.ignore_input = 1;
        else if (!strcmp(a, "--control-no-restore")) o.no_restore = 1;
        else if (!strcmp(a, "--control-no-signals")) o.no_signals = 1;
        else if (!strcmp(a, "--fault")) {
            NEEDV();
            if (!strcmp(v, "stale-map")) o.faults |= FB_FAULT_STALE_MAP_ON_RESIZE;
            else if (!strcmp(v, "resize-steps")) o.faults |= FB_FAULT_RESIZE_STEPS_PRODUCER;
            else if (!strcmp(v, "frame-locked")) o.faults |= FB_FAULT_FRAME_LOCKED_SIM;
            else if (!strcmp(v, "leak")) o.faults |= FB_FAULT_LEAK_PER_FRAME;
            else if (!strcmp(v, "retry-io")) o.faults |= FB_FAULT_RETRY_ON_IO;
            else if (!strcmp(v, "sticky-signal")) o.faults |= FB_FAULT_STICKY_SIGNAL;
            else if (!strcmp(v, "slow-present")) o.faults |= FB_FAULT_SLOW_PRESENT;
            else { usage(); return 2; }
        } else if (!strcmp(a, "-h") || !strcmp(a, "--help")) { usage(); return 0; }
        else { usage(); return 2; }
#undef NEEDV
    }
    if (mode == 1) return selfcheck(only);
    if (mode == 2) {
        if (!o.replay_tics) { usage(); return 2; }
        return run_replay(&o);
    }
    return run_play(&o);
}
