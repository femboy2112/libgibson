/*
 * LibGibson v0.8 parity slice -- C consumer (the C ABI reference).
 *
 * Interprets bindings/parity/v08/script.txt against the INSTALLED SDK (#include <gibson.h> via
 * pkg-config) and writes the canonical transcript (one JSON object per line, sorted keys, no
 * floats) to stdout. Format and record schema: docs/V08_PARITY_SLICE.md.
 *
 *   parity_v08_c SCRIPT [REPS]
 *
 * REPS > 1 re-runs the whole script in-process (create .. destroy each time) and prints one
 * resource record to stderr; only rep 1's transcript is printed, and every later rep's transcript
 * must be byte-identical to it (iter_identical).
 *
 * Negative controls (never on by default): PARITY_V08_PERTURB=cell flips the last byte of step
 * s06's text; PARITY_V08_PERTURB=events swaps steps e01 and e02; PARITY_V08_PERTURB=leak makes
 * step d01 drop the context WITHOUT destroying it (a planted per-rep leak for the resource gate).
 */
#define _POSIX_C_SOURCE 200809L
#include <gibson.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <time.h>

/* ------------------------------------------------------------------ output buffer */
typedef struct {
    char *p;
    size_t len, cap;
} Buf;

static void buf_putn(Buf *b, const char *s, size_t n) {
    if (b->len + n + 1 > b->cap) {
        size_t nc = b->cap ? b->cap : 4096;
        while (b->len + n + 1 > nc) nc *= 2;
        b->p = realloc(b->p, nc);
        if (!b->p) { fprintf(stderr, "parity_v08_c: out of memory\n"); exit(2); }
        b->cap = nc;
    }
    memcpy(b->p + b->len, s, n);
    b->len += n;
    b->p[b->len] = 0;
}
static void buf_puts(Buf *b, const char *s) { buf_putn(b, s, strlen(s)); }

/* ------------------------------------------------------------------ FNV-1a 64 */
#define FNV_OFFSET 0xcbf29ce484222325ULL
#define FNV_PRIME 0x100000001b3ULL
static uint64_t fnv(uint64_t h, const void *data, size_t n) {
    const unsigned char *p = data;
    for (size_t i = 0; i < n; i++) { h ^= p[i]; h *= FNV_PRIME; }
    return h;
}

static void hex_of(const unsigned char *p, size_t n, char *out) {
    static const char d[] = "0123456789abcdef";
    for (size_t i = 0; i < n; i++) { out[2 * i] = d[p[i] >> 4]; out[2 * i + 1] = d[p[i] & 15]; }
    out[2 * n] = 0;
}
static size_t unhex(const char *s, unsigned char *out, size_t cap) {
    size_t n = strlen(s) / 2;
    if (n > cap) n = cap;
    for (size_t i = 0; i < n; i++) {
        unsigned v = 0;
        sscanf(s + 2 * i, "%2x", &v);
        out[i] = (unsigned char)v;
    }
    return n;
}

/* ------------------------------------------------------------------ records (sorted keys) */
enum { V_INT, V_STR, V_ARR4 };
typedef struct {
    const char *key;
    int type;
    long long i;
    char s[1100];
    int a[4];
} KV;
typedef struct {
    KV kv[16];
    int n;
} Rec;

static KV *rec_slot(Rec *r, const char *k) { KV *kv = &r->kv[r->n++]; kv->key = k; return kv; }
static void rec_int(Rec *r, const char *k, long long v) { KV *kv = rec_slot(r, k); kv->type = V_INT; kv->i = v; }
/* Strings are emitted verbatim only if they need no JSON escaping; otherwise as "hex:<utf8 hex>". */
static void rec_str(Rec *r, const char *k, const char *v) {
    KV *kv = rec_slot(r, k);
    kv->type = V_STR;
    int safe = 1;
    for (const unsigned char *p = (const unsigned char *)v; *p; p++)
        if (*p < 0x20 || *p > 0x7e || *p == '"' || *p == '\\') { safe = 0; break; }
    if (safe) {
        snprintf(kv->s, sizeof kv->s, "%s", v);
    } else {
        strcpy(kv->s, "hex:");
        size_t n = strlen(v);
        if (n > 500) n = 500;
        hex_of((const unsigned char *)v, n, kv->s + 4);
    }
}
static void rec_arr4(Rec *r, const char *k, int a, int b, int c, int d) {
    KV *kv = rec_slot(r, k);
    kv->type = V_ARR4;
    kv->a[0] = a; kv->a[1] = b; kv->a[2] = c; kv->a[3] = d;
}
static int kv_cmp(const void *x, const void *y) { return strcmp(((const KV *)x)->key, ((const KV *)y)->key); }
static void rec_emit(Buf *out, Rec *r) {
    char tmp[1300];
    qsort(r->kv, (size_t)r->n, sizeof(KV), kv_cmp);
    buf_puts(out, "{");
    for (int i = 0; i < r->n; i++) {
        KV *kv = &r->kv[i];
        if (kv->type == V_INT) snprintf(tmp, sizeof tmp, "%s\"%s\":%lld", i ? "," : "", kv->key, kv->i);
        else if (kv->type == V_STR) snprintf(tmp, sizeof tmp, "%s\"%s\":\"%s\"", i ? "," : "", kv->key, kv->s);
        else snprintf(tmp, sizeof tmp, "%s\"%s\":[%d,%d,%d,%d]", i ? "," : "", kv->key, kv->a[0], kv->a[1], kv->a[2], kv->a[3]);
        buf_puts(out, tmp);
    }
    buf_puts(out, "}\n");
    r->n = 0;
}

static const char *st_name(int s) {
    switch (s) {
    case GIBSON_OK: return "OK";
    case GIBSON_ERR_INVALID_PARAM: return "INVALID_PARAM";
    case GIBSON_ERR_IO: return "IO";
    case GIBSON_ERR_PANIC: return "PANIC";
    case GIBSON_ERR_NOT_INITIALIZED: return "NOT_INITIALIZED";
    case GIBSON_ERR_QUEUE_FULL: return "QUEUE_FULL";
    default: return "UNKNOWN";
    }
}
static const char *kind_name(uint32_t k) {
    return k == GIBSON_KEY_KIND_CHAR ? "CHAR" : k == GIBSON_KEY_KIND_SPECIAL ? "SPECIAL" : "NONE";
}
static const char *type_name(int32_t t) {
    switch (t) {
    case GIBSON_EVENT_KEY: return "KEY";
    case GIBSON_EVENT_PASTE: return "PASTE";
    case GIBSON_EVENT_RESIZE: return "RESIZE";
    case GIBSON_EVENT_FOCUS_GAINED: return "FOCUS_GAINED";
    case GIBSON_EVENT_FOCUS_LOST: return "FOCUS_LOST";
    default: return "NONE";
    }
}
static void last_err(char *buf, size_t n) {
    buf[0] = 0;
    gibson_last_error_message(buf, n);
}

/* ------------------------------------------------------------------ script */
typedef struct {
    char raw[1024];  /* the script line as read */
    char line[1024]; /* tokenized copy of raw */
    char *tok[12];
    int ntok; /* tok[0]=id tok[1]=dim tok[2]=op */
} Step;

static void tokenize(Step *st) {
    memcpy(st->line, st->raw, sizeof st->line);
    st->ntok = 0;
    char *save = NULL;
    for (char *t = strtok_r(st->line, " ", &save); t && st->ntok < 12; t = strtok_r(NULL, " ", &save))
        st->tok[st->ntok++] = t;
}

static int load_script(const char *path, Step **out) {
    FILE *f = fopen(path, "r");
    if (!f) return -1;
    int cap = 256, n = 0;
    Step *s = calloc((size_t)cap, sizeof(Step));
    char line[1024];
    while (fgets(line, sizeof line, f)) {
        line[strcspn(line, "\r\n")] = 0;
        if (line[0] == 0 || line[0] == '#') continue;
        if (n == cap) { cap *= 2; s = realloc(s, (size_t)cap * sizeof(Step)); }
        Step *st = &s[n++];
        memset(st, 0, sizeof *st);
        snprintf(st->raw, sizeof st->raw, "%s", line);
        tokenize(st);
    }
    fclose(f);
    *out = s;
    return n;
}

static void apply_perturbation(Step *s, int n) {
    const char *p = getenv("PARITY_V08_PERTURB");
    if (!p || !*p) return;
    if (strcmp(p, "cell") == 0) {
        for (int i = 0; i < n; i++)
            if (strcmp(s[i].tok[0], "s06") == 0) {
                size_t L = strlen(s[i].raw); /* the line ends in the hex text; last byte '!' -> '?' */
                s[i].raw[L - 2] = '3'; s[i].raw[L - 1] = 'f';
                tokenize(&s[i]);
            }
    } else if (strcmp(p, "events") == 0) {
        int a = -1, b = -1;
        for (int i = 0; i < n; i++) {
            if (strcmp(s[i].tok[0], "e01") == 0) a = i;
            if (strcmp(s[i].tok[0], "e02") == 0) b = i;
        }
        if (a >= 0 && b >= 0) {
            char tmp[1024];
            memcpy(tmp, s[a].raw, sizeof tmp);
            memcpy(s[a].raw, s[b].raw, sizeof tmp);
            memcpy(s[b].raw, tmp, sizeof tmp);
            tokenize(&s[a]);
            tokenize(&s[b]);
        }
    }
}

/* ------------------------------------------------------------------ interpreter */
typedef struct {
    char name[32];
    gibson_style_t style;
} NamedStyle;

typedef struct {
    gibson_context_t *ctx;
    gibson_node_t *root;
    NamedStyle styles[8];
    int nstyles;
    Buf *out;
} State;

static void parse_color(const char *spec, gibson_color_t *c) {
    memset(c, 0, sizeof *c);
    unsigned r = 0, g = 0, b = 0, n = 0;
    if (strcmp(spec, "reset") == 0) c->color_type = GIBSON_COLOR_RESET;
    else if (sscanf(spec, "rgb:%u,%u,%u", &r, &g, &b) == 3) { c->color_type = GIBSON_COLOR_RGB; c->r = (uint8_t)r; c->g = (uint8_t)g; c->b = (uint8_t)b; }
    else if (sscanf(spec, "ansi:%u", &n) == 1) { c->color_type = GIBSON_COLOR_ANSI256; c->r = (uint8_t)n; }
    else if (sscanf(spec, "named:%u", &n) == 1) c->color_type = (int32_t)n;
}

static const gibson_style_t *find_style(State *S, const char *name) {
    for (int i = 0; i < S->nstyles; i++)
        if (strcmp(S->styles[i].name, name) == 0) return &S->styles[i].style;
    return NULL;
}

static int32_t wrap_of(const char *w) { return strcmp(w, "CHAR") == 0 ? GIBSON_WRAP_CHAR : strcmp(w, "WORD") == 0 ? GIBSON_WRAP_WORD : GIBSON_WRAP_NONE; }
static uint32_t cp_of(const char *u) { return (uint32_t)strtoul(u + 2, NULL, 16); }

static uint32_t special_of(const char *name) {
    static const struct { const char *n; uint32_t v; } t[] = {
        {"ENTER", GIBSON_KEY_ENTER}, {"LEFT", GIBSON_KEY_LEFT}, {"RIGHT", GIBSON_KEY_RIGHT},
        {"UP", GIBSON_KEY_UP}, {"DOWN", GIBSON_KEY_DOWN}, {"HOME", GIBSON_KEY_HOME},
        {"END", GIBSON_KEY_END}, {"BACKSPACE", GIBSON_KEY_BACKSPACE}, {"DELETE", GIBSON_KEY_DELETE},
        {"ESC", GIBSON_KEY_ESC}, {"TAB", GIBSON_KEY_TAB}, {"PAGE_UP", GIBSON_KEY_PAGE_UP},
        {"PAGE_DOWN", GIBSON_KEY_PAGE_DOWN}};
    for (size_t i = 0; i < sizeof t / sizeof t[0]; i++)
        if (strcmp(t[i].n, name) == 0) return t[i].v;
    return 0;
}

/* Full (untruncated) glyph bytes of a cell: inline bytes, or the escape hatch when truncated. */
/* Longest grapheme (bytes) this consumer reads back; its hex fits a record field (1100). */
#define GLYPH_MAX 512

/* The cell's full glyph into buf (cap >= GLYPH_MAX). The length is queried first; a longer
 * grapheme is reported and the consumer exits 2, so it never reads past what the library copied. */
static size_t full_glyph(gibson_context_t *ctx, const gibson_cell_t *c, uint16_t x, uint16_t y, unsigned char *buf, size_t cap) {
    if (!c->_reserved[0]) {
        size_t n = c->glyph_len < sizeof c->glyph ? c->glyph_len : sizeof c->glyph;
        memcpy(buf, c->glyph, n);
        return n;
    }
    size_t len = 0, got = 0;
    gibson_get_frame_cell_glyph(ctx, x, y, NULL, 0, &len);
    if (len > cap) {
        fprintf(stderr, "parity_v08: the glyph at %u,%u is %zu bytes, more than this consumer reads (%zu)\n",
                (unsigned)x, (unsigned)y, len, cap);
        exit(2);
    }
    if (gibson_get_frame_cell_glyph(ctx, x, y, buf, cap, &got) != GIBSON_OK || got != len) {
        fprintf(stderr, "parity_v08: glyph readback at %u,%u returned %zu of %zu bytes\n", (unsigned)x, (unsigned)y, got, len);
        exit(2);
    }
    return got;
}

/* The canonical per-cell line hashed into the frame digest (docs/V08_PARITY_SLICE.md). */
static size_t cell_line(gibson_context_t *ctx, const gibson_cell_t *c, uint16_t x, uint16_t y, char *line) {
    unsigned char g[GLYPH_MAX];
    char gh[2 * GLYPH_MAX + 1];
    size_t gl = full_glyph(ctx, c, x, y, g, sizeof g);
    hex_of(g, gl, gh);
    const gibson_style_t *s = &c->style;
    return (size_t)sprintf(line, "%s;%u;%u;%u;%d,%u,%u,%u;%d,%u,%u,%u;%u%u%u%u%u\n", gh, c->width, c->is_continuation,
                           c->transparent, s->fg.color_type, s->fg.r, s->fg.g, s->fg.b, s->bg.color_type, s->bg.r,
                           s->bg.g, s->bg.b, s->bold ? 1 : 0, s->dim ? 1 : 0, s->italic ? 1 : 0, s->underline ? 1 : 0,
                           s->reverse ? 1 : 0);
}

static void emit_event(State *S, const char *id, int st, const gibson_event_t *ev, const char *kind) {
    Rec r = {0};
    char pid[64], ph[1024] = "";
    snprintf(pid, sizeof pid, "%s", id);
    rec_str(&r, "id", pid);
    rec_str(&r, "k", "event");
    rec_str(&r, "st", st_name(st));
    rec_str(&r, "type", type_name(ev->event_type));
    rec_str(&r, "kind", kind);
    rec_int(&r, "key_code", ev->event_type == GIBSON_EVENT_KEY ? (long long)ev->key_code : 0);
    rec_int(&r, "mods", ev->event_type == GIBSON_EVENT_KEY ? ev->modifiers : 0);
    rec_int(&r, "cols", ev->event_type == GIBSON_EVENT_RESIZE ? ev->cols : 0);
    rec_int(&r, "rows", ev->event_type == GIBSON_EVENT_RESIZE ? ev->rows : 0);
    if (ev->event_type == GIBSON_EVENT_PASTE && ev->paste_str) {
        size_t n = strlen(ev->paste_str);
        if (n > 500) n = 500;
        hex_of((const unsigned char *)ev->paste_str, n, ph);
    }
    rec_str(&r, "paste", ph);
    rec_emit(S->out, &r);
}

static void probe(State *S, const char *id, const char *name) {
    Rec r = {0};
    char err[512];
    int st = 0, with_err = 0;
    gibson_context_t *c2 = NULL;
    gibson_cell_t cell;
    gibson_event_t ev;
    memset(&ev, 0, sizeof ev);
    uint16_t cols = 0, rows = 0;
    gibson_get_frame_size(S->ctx, &cols, &rows);
    rec_str(&r, "id", id);
    rec_str(&r, "k", "probe");
    rec_str(&r, "name", name);

    if (!strcmp(name, "create_bad_mode")) {
        st = gibson_create_headless_context(7, 40, 12, &c2); with_err = 1;
    } else if (!strcmp(name, "create_zero_cols")) {
        st = gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, 0, 12, &c2); with_err = 1;
    } else if (!strcmp(name, "create_null_out")) {
        st = gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, 40, 12, NULL);
    } else if (!strcmp(name, "render_null_ctx")) {
        st = gibson_render(NULL);
    } else if (!strcmp(name, "cell_oob_x")) {
        st = gibson_get_frame_cell(S->ctx, cols, 0, &cell);
    } else if (!strcmp(name, "cell_oob_y")) {
        st = gibson_get_frame_cell(S->ctx, 0, rows, &cell);
    } else if (!strcmp(name, "cells_oob")) {
        size_t written = 99;
        st = gibson_get_frame_cells(S->ctx, 0, rows, &cell, 1, &written);
        rec_int(&r, "written", (long long)written);
    } else if (!strcmp(name, "cell_null_out")) {
        st = gibson_get_frame_cell(S->ctx, 0, 0, NULL);
    } else if (!strcmp(name, "text_bad_utf8")) {
        gibson_node_t *n = NULL;
        st = gibson_node_text("\xff\xfe", NULL, GIBSON_WRAP_NONE, &n); with_err = 1;
        if (st == GIBSON_OK) gibson_node_free(n);
    } else if (!strcmp(name, "push_null_event")) {
        st = gibson_push_event(S->ctx, NULL);
    } else if (!strcmp(name, "push_surrogate")) {
        ev.event_type = GIBSON_EVENT_KEY; ev.key_code = 0xD800;
        st = gibson_push_event_kind(S->ctx, &ev, GIBSON_KEY_KIND_CHAR); with_err = 1;
    } else if (!strcmp(name, "push_special_not_key")) {
        ev.event_type = GIBSON_EVENT_KEY; ev.key_code = 'a';
        st = gibson_push_event_kind(S->ctx, &ev, GIBSON_KEY_KIND_SPECIAL);
    } else if (!strcmp(name, "push_bad_kind")) {
        ev.event_type = GIBSON_EVENT_KEY; ev.key_code = 'a';
        st = gibson_push_event_kind(S->ctx, &ev, 9);
    } else if (!strcmp(name, "push_bad_mods")) {
        ev.event_type = GIBSON_EVENT_KEY; ev.key_code = 'a'; ev.modifiers = 0x80;
        st = gibson_push_event_kind(S->ctx, &ev, GIBSON_KEY_KIND_CHAR);
    } else if (!strcmp(name, "push_resize_zero")) {
        ev.event_type = GIBSON_EVENT_RESIZE; ev.cols = 0; ev.rows = 12;
        st = gibson_push_event(S->ctx, &ev);
    } else if (!strcmp(name, "paste_bad_utf8")) {
        ev.event_type = GIBSON_EVENT_PASTE; ev.paste_str = "\xc3\x28";
        st = gibson_push_event(S->ctx, &ev); with_err = 1;
    } else if (!strcmp(name, "paste_null")) {
        ev.event_type = GIBSON_EVENT_PASTE; ev.paste_str = NULL;
        st = gibson_push_event(S->ctx, &ev);
    } else if (!strcmp(name, "poll_null_out")) {
        st = gibson_poll_event(S->ctx, 0, NULL);
    } else if (!strcmp(name, "poll_kind_null")) {
        st = gibson_poll_event_kind(S->ctx, 0, &ev, NULL);
    } else if (!strcmp(name, "color_depth_bad")) {
        st = gibson_set_color_depth(S->ctx, 9);
    } else if (!strcmp(name, "queue_full")) {
        long long accepted = 0, drained = 0;
        ev.event_type = GIBSON_EVENT_KEY; ev.key_code = 'q';
        for (int i = 0; i < GIBSON_EVENT_QUEUE_CAPACITY; i++)
            if (gibson_push_event_kind(S->ctx, &ev, GIBSON_KEY_KIND_CHAR) == GIBSON_OK) accepted++;
        st = gibson_push_event_kind(S->ctx, &ev, GIBSON_KEY_KIND_CHAR); with_err = 1;
        last_err(err, sizeof err);
        for (int i = 0; i < 2 * GIBSON_EVENT_QUEUE_CAPACITY; i++) {
            gibson_event_t got; uint32_t kind = 0;
            if (gibson_poll_event_kind(S->ctx, 0, &got, &kind) != GIBSON_OK || got.event_type == GIBSON_EVENT_NONE) break;
            drained++;
        }
        rec_int(&r, "accepted", accepted);
        rec_int(&r, "drained", drained);
        rec_str(&r, "err", err);
        with_err = 0;
    } else if (!strcmp(name, "frame_before_render")) {
        uint16_t c = 7, rr = 7;
        gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, 8, 2, &c2);
        int sst = gibson_get_frame_size(c2, &c, &rr);
        st = gibson_get_frame_cell(c2, 0, 0, &cell);
        gibson_destroy_context(c2);
        c2 = NULL;
        rec_int(&r, "cols", c);
        rec_int(&r, "rows", rr);
        rec_str(&r, "size_st", st_name(sst));
    } else {
        st = 999;
    }
    if (with_err) {
        last_err(err, sizeof err);
        rec_str(&r, "err", err);
    }
    if (c2) gibson_destroy_context(c2);
    rec_str(&r, "st", st == 999 ? "UNKNOWN_PROBE" : st_name(st));
    rec_emit(S->out, &r);
}

static void run_script(Step *steps, int nsteps, Buf *out) {
    State S;
    memset(&S, 0, sizeof S);
    S.out = out;
    Rec r = {0};
    for (int i = 0; i < nsteps; i++) {
        Step *s = &steps[i];
        const char *id = s->tok[0], *op = s->tok[2];
        char **a = &s->tok[3];
        r.n = 0;
        if (!strcmp(op, "create")) {
            int32_t mode = strcmp(a[0], "INLINE") == 0 ? GIBSON_MODE_INLINE : GIBSON_MODE_FULLSCREEN;
            int st = gibson_create_headless_context(mode, (uint16_t)atoi(a[1]), (uint16_t)atoi(a[2]), &S.ctx);
            rec_int(&r, "cols", atoi(a[1])); rec_str(&r, "id", id); rec_str(&r, "k", "create");
            rec_str(&r, "mode", a[0]); rec_int(&r, "rows", atoi(a[2])); rec_str(&r, "st", st_name(st));
            rec_emit(out, &r);
        } else if (!strcmp(op, "style")) {
            NamedStyle *ns = &S.styles[S.nstyles++];
            memset(ns, 0, sizeof *ns);
            snprintf(ns->name, sizeof ns->name, "%s", a[0]);
            parse_color(a[1], &ns->style.fg);
            parse_color(a[2], &ns->style.bg);
            ns->style.bold = a[3][0] == '1'; ns->style.dim = a[3][1] == '1'; ns->style.italic = a[3][2] == '1';
            ns->style.underline = a[3][3] == '1'; ns->style.reverse = a[3][4] == '1';
        } else if (!strcmp(op, "col")) {
            int st = gibson_node_box_col(&S.root);
            rec_str(&r, "id", id); rec_str(&r, "k", "node"); rec_str(&r, "st", st_name(st));
            rec_emit(out, &r);
        } else if (!strcmp(op, "text")) {
            unsigned char txt[512];
            size_t n = unhex(a[2], txt, sizeof txt - 1);
            txt[n] = 0;
            gibson_node_t *node = NULL;
            int st = gibson_node_text((const char *)txt, find_style(&S, a[0]), wrap_of(a[1]), &node);
            if (st == GIBSON_OK) st = gibson_node_add_child(S.root, node);
            rec_str(&r, "id", id); rec_str(&r, "k", "node"); rec_str(&r, "st", st_name(st));
            rec_emit(out, &r);
        } else if (!strcmp(op, "mount")) {
            int st = gibson_set_root_node(S.ctx, S.root); /* consumes the tree */
            S.root = NULL;
            rec_str(&r, "id", id); rec_str(&r, "k", "mount"); rec_str(&r, "st", st_name(st));
            rec_emit(out, &r);
        } else if (!strcmp(op, "render")) {
            int st = gibson_render(S.ctx);
            rec_str(&r, "id", id); rec_str(&r, "k", "render"); rec_str(&r, "st", st_name(st));
            rec_emit(out, &r);
        } else if (!strcmp(op, "terminal_size")) {
            uint16_t c = 0, rr = 0;
            int st = gibson_get_terminal_size(S.ctx, &c, &rr);
            rec_int(&r, "cols", c); rec_str(&r, "id", id); rec_str(&r, "k", "terminal_size"); rec_int(&r, "rows", rr);
            rec_str(&r, "st", st_name(st)); rec_emit(out, &r);
        } else if (!strcmp(op, "focus_reporting")) {
            int st = gibson_set_focus_reporting(S.ctx, atoi(a[0]));
            rec_str(&r, "id", id); rec_str(&r, "k", "focus_reporting"); rec_str(&r, "st", st_name(st)); rec_emit(out, &r);
        } else if (!strcmp(op, "geom")) {
            uint16_t c = 0, rr = 0;
            int st = gibson_get_frame_size(S.ctx, &c, &rr);
            rec_int(&r, "cols", c); rec_str(&r, "id", id); rec_str(&r, "k", "geom"); rec_int(&r, "rows", rr);
            rec_str(&r, "st", st_name(st));
            rec_emit(out, &r);
        } else if (!strcmp(op, "frame")) {
            uint16_t c = 0, rr = 0;
            int st = gibson_get_frame_size(S.ctx, &c, &rr);
            size_t total = (size_t)c * rr, written = 0;
            gibson_cell_t *cells = calloc(total ? total : 1, sizeof *cells);
            if (st == GIBSON_OK) st = gibson_get_frame_cells(S.ctx, 0, 0, cells, total, &written);
            uint64_t h = FNV_OFFSET;
            char line[2 * GLYPH_MAX + 256];
            for (size_t k = 0; k < written; k++) {
                size_t L = cell_line(S.ctx, &cells[k], (uint16_t)(k % c), (uint16_t)(k / c), line);
                h = fnv(h, line, L);
            }
            free(cells);
            size_t wl = 0;
            gibson_get_rendered_output(S.ctx, NULL, 0, &wl);
            unsigned char *wire = malloc(wl ? wl : 1);
            size_t got = 0;
            gibson_get_rendered_output(S.ctx, wire, wl, &got);
            uint64_t wh = fnv(FNV_OFFSET, wire, got);
            free(wire);
            char dh[17], wd[17];
            snprintf(dh, sizeof dh, "%016llx", (unsigned long long)h);
            snprintf(wd, sizeof wd, "%016llx", (unsigned long long)wh);
            rec_int(&r, "cells", (long long)written); rec_str(&r, "digest", dh); rec_str(&r, "id", id);
            rec_str(&r, "k", "frame"); rec_str(&r, "st", st_name(st)); rec_str(&r, "wire_digest", wd);
            rec_int(&r, "wire_len", (long long)got);
            rec_emit(out, &r);
        } else if (!strcmp(op, "cell")) {
            uint16_t x = (uint16_t)atoi(a[0]), y = (uint16_t)atoi(a[1]);
            gibson_cell_t c;
            memset(&c, 0, sizeof c);
            int st = gibson_get_frame_cell(S.ctx, x, y, &c);
            unsigned char g[GLYPH_MAX];
            char gh[2 * GLYPH_MAX + 1];
            size_t gl = st == GIBSON_OK ? full_glyph(S.ctx, &c, x, y, g, sizeof g) : 0;
            hex_of(g, gl, gh);
            char attrs[6];
            snprintf(attrs, sizeof attrs, "%u%u%u%u%u", c.style.bold ? 1 : 0, c.style.dim ? 1 : 0, c.style.italic ? 1 : 0,
                     c.style.underline ? 1 : 0, c.style.reverse ? 1 : 0);
            rec_str(&r, "attrs", attrs);
            rec_arr4(&r, "bg", c.style.bg.color_type, c.style.bg.r, c.style.bg.g, c.style.bg.b);
            rec_int(&r, "cont", c.is_continuation);
            rec_arr4(&r, "fg", c.style.fg.color_type, c.style.fg.r, c.style.fg.g, c.style.fg.b);
            rec_str(&r, "glyph", gh); rec_str(&r, "id", id); rec_str(&r, "k", "cell"); rec_str(&r, "st", st_name(st));
            rec_int(&r, "transp", c.transparent); rec_int(&r, "w", c.width); rec_int(&r, "x", x); rec_int(&r, "y", y);
            rec_emit(out, &r);
        } else if (!strcmp(op, "glyph")) {
            uint16_t x = (uint16_t)atoi(a[0]), y = (uint16_t)atoi(a[1]);
            size_t len = 0;
            int st = gibson_get_frame_cell_glyph(S.ctx, x, y, NULL, 0, &len);
            unsigned char *g = malloc(len ? len : 1);
            size_t len2 = 0;
            if (st == GIBSON_OK) st = gibson_get_frame_cell_glyph(S.ctx, x, y, g, len, &len2);
            char *gh = malloc(2 * len2 + 1);
            hex_of(g, len2, gh);
            rec_str(&r, "full_hex", gh); rec_int(&r, "full_len", (long long)len2); rec_str(&r, "id", id);
            rec_str(&r, "k", "glyph"); rec_str(&r, "st", st_name(st)); rec_int(&r, "x", x); rec_int(&r, "y", y);
            rec_emit(out, &r);
            free(g); free(gh);
        } else if (!strcmp(op, "glyphcap")) {
            uint16_t x = (uint16_t)atoi(a[0]), y = (uint16_t)atoi(a[1]);
            gibson_cell_t c;
            memset(&c, 0, sizeof c);
            int st = gibson_get_frame_cell(S.ctx, x, y, &c);
            rec_str(&r, "id", id); rec_int(&r, "inline_len", c.glyph_len); rec_str(&r, "k", "glyphcap");
            rec_str(&r, "st", st_name(st)); rec_int(&r, "trunc", c._reserved[0]); rec_int(&r, "x", x); rec_int(&r, "y", y);
            rec_emit(out, &r);
        } else if (!strncmp(op, "push_", 5)) {
            gibson_event_t ev;
            memset(&ev, 0, sizeof ev);
            unsigned char paste[512];
            int st;
            if (!strcmp(op, "push_char")) {
                ev.event_type = GIBSON_EVENT_KEY; ev.key_code = cp_of(a[0]); ev.modifiers = (uint8_t)atoi(a[1]);
                st = gibson_push_event_kind(S.ctx, &ev, GIBSON_KEY_KIND_CHAR);
            } else if (!strcmp(op, "push_special")) {
                ev.event_type = GIBSON_EVENT_KEY; ev.key_code = special_of(a[0]); ev.modifiers = (uint8_t)atoi(a[1]);
                st = gibson_push_event_kind(S.ctx, &ev, GIBSON_KEY_KIND_SPECIAL);
            } else if (!strcmp(op, "push_legacy")) {
                ev.event_type = GIBSON_EVENT_KEY; ev.key_code = cp_of(a[0]); ev.modifiers = (uint8_t)atoi(a[1]);
                st = gibson_push_event(S.ctx, &ev);
            } else if (!strcmp(op, "push_paste")) {
                size_t n = unhex(a[0], paste, sizeof paste - 1);
                paste[n] = 0;
                ev.event_type = GIBSON_EVENT_PASTE; ev.paste_str = (const char *)paste;
                st = gibson_push_event(S.ctx, &ev);
            } else if (!strcmp(op, "push_focus")) {
                ev.event_type = !strcmp(a[0], "GAINED") ? GIBSON_EVENT_FOCUS_GAINED : GIBSON_EVENT_FOCUS_LOST;
                st = gibson_push_event(S.ctx, &ev);
            } else { /* push_resize */
                ev.event_type = GIBSON_EVENT_RESIZE; ev.cols = (uint16_t)atoi(a[0]); ev.rows = (uint16_t)atoi(a[1]);
                st = gibson_push_event(S.ctx, &ev);
            }
            rec_str(&r, "id", id); rec_str(&r, "k", "push"); rec_str(&r, "st", st_name(st));
            rec_emit(out, &r);
        } else if (!strcmp(op, "poll")) {
            int n = atoi(a[0]);
            for (int k = 1; k <= n; k++) {
                gibson_event_t ev;
                uint32_t kind = 0;
                int st = gibson_poll_event_kind(S.ctx, 0, &ev, &kind);
                char pid[64];
                snprintf(pid, sizeof pid, "%s.%d", id, k);
                emit_event(&S, pid, st, &ev, kind_name(kind));
            }
        } else if (!strcmp(op, "poll_legacy")) {
            gibson_event_t ev;
            int st = gibson_poll_event(S.ctx, 0, &ev);
            emit_event(&S, id, st, &ev, "ABSENT");
        } else if (!strcmp(op, "probe")) {
            probe(&S, id, a[0]);
        } else if (!strcmp(op, "lifecycle")) {
            int st = GIBSON_OK;
            if (!strcmp(a[0], "node_free_unattached")) {
                gibson_node_t *n = NULL;
                st = gibson_node_text("unattached", NULL, GIBSON_WRAP_NONE, &n);
                gibson_node_free(n);
            } else if (!strcmp(a[0], "destroy_null")) {
                gibson_destroy_context(NULL);
            }
            rec_str(&r, "id", id); rec_str(&r, "k", "lifecycle"); rec_str(&r, "name", a[0]); rec_str(&r, "st", st_name(st));
            rec_emit(out, &r);
        } else if (!strcmp(op, "destroy")) {
            const char *pt = getenv("PARITY_V08_PERTURB");
            if (!pt || strcmp(pt, "leak") != 0) gibson_destroy_context(S.ctx); /* leak control: skip */
            S.ctx = NULL;
            rec_str(&r, "id", id); rec_str(&r, "k", "destroy"); rec_str(&r, "st", "OK");
            rec_emit(out, &r);
        } else {
            rec_str(&r, "id", id); rec_str(&r, "k", "unknown_op"); rec_str(&r, "st", "UNSUPPORTED");
            rec_emit(out, &r);
        }
    }
    if (S.root) gibson_node_free(S.root);
    if (S.ctx) gibson_destroy_context(S.ctx);
}

static long maxrss_kb(void) {
    struct rusage ru;
    getrusage(RUSAGE_SELF, &ru);
    return ru.ru_maxrss;
}
static long long now_us(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (long long)ts.tv_sec * 1000000LL + ts.tv_nsec / 1000;
}

int main(int argc, char **argv) {
    if (argc < 2) { fprintf(stderr, "usage: parity_v08_c SCRIPT [REPS]\n"); return 2; }
    if (gibson_abi_version() != GIBSON_ABI_VERSION) { fprintf(stderr, "ABI mismatch\n"); return 2; }
    int reps = argc > 2 ? atoi(argv[2]) : 1;
    if (reps < 1) reps = 1;
    Step *steps = NULL;
    int n = load_script(argv[1], &steps);
    if (n <= 0) { fprintf(stderr, "parity_v08_c: cannot read %s\n", argv[1]); return 2; }
    apply_perturbation(steps, n);

    long long t0 = now_us();
    Buf first = {0};
    run_script(steps, n, &first);
    long rss1 = maxrss_kb();
    uint64_t h1 = fnv(FNV_OFFSET, first.p, first.len);
    long rssmid = rss1;
    int identical = 1;
    for (int rep = 2; rep <= reps; rep++) {
        Buf b = {0};
        run_script(steps, n, &b);
        if (b.len != first.len || fnv(FNV_OFFSET, b.p, b.len) != h1) identical = 0;
        free(b.p);
        if (rep == (reps + 1) / 2) rssmid = maxrss_kb();
    }
    long long wall = now_us() - t0;
    fwrite(first.p, 1, first.len, stdout);
    if (reps > 1)
        fprintf(stderr, "{\"iter_identical\":%d,\"k\":\"resource\",\"lang\":\"c\",\"maxrss_kb_1\":%ld,\"maxrss_kb_mid\":%ld,"
                        "\"maxrss_kb_n\":%ld,\"per_rep_us\":%lld,\"reps\":%d,\"wall_ms\":%lld}\n",
                identical, rss1, rssmid, maxrss_kb(), wall / reps, reps, wall / 1000);
    free(first.p);
    free(steps);
    return 0;
}
