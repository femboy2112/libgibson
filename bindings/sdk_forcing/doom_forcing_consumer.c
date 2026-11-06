/*
 * LibGibson installed-SDK forcing consumer (RXIII Checkpoint C / Gate E).
 *
 * This is NOT a repo-relative smoke. It is compiled and linked against the SDK as it is actually
 * SHIPPED — the headers and the shared object STAGED into a prefix OUTSIDE the checkout (see
 * scripts/dev/sdk_forcing_consumer.sh, which runs scripts/release/stage-sdk.sh and then builds this
 * file from a temp dir with `cc $(pkg-config --cflags --libs libgibson)`). The include below is the
 * ANGLE-BRACKET <gibson.h> resolved from the staged prefix, never "../../include/gibson.h" — if this
 * links and runs, a third party who installed the SDK can do the same.
 *
 * It FORCES the heavyweight Gate I/J/K readback surface the way a Doom-style bulk-raster renderer
 * would: a frame loop that reacts to (simulated) input, renders a headless "framebuffer", and pulls
 * the WHOLE resolved cell grid out each frame via the BULK path — then self-checks invariants a
 * naive smoke never would:
 *
 *   1. input -> render -> readback closes: a marker moved only by INJECTED input (one
 *      GIBSON_KEY_RIGHT per frame, pushed with gibson_push_event and delivered by
 *      gibson_poll_event) lands at the raster column the input demanded (the game-loop contract).
 *   2. the BULK readback equals the authoritative single-cell readback, byte for byte, over the
 *      whole frame (the fast path a raster needs must not diverge from gibson_get_frame_cell).
 *   3. gibson_set_color_depth on a LIVE context restyles the readback: the same styled cell resolves
 *      to a different style at TRUECOLOR vs MONO (the mid-session depth change a terminal that loses
 *      color must survive).
 *   4. the long-glyph escape hatch returns a grapheme cluster longer than the inline cap in full
 *      (a 25-byte ZWJ emoji is one cell; gibson_get_frame_cell_glyph recovers all 25 bytes).
 *   5. the ANSI output path yields bytes at every depth.
 *   6. injected input is bounded and exact: the queue accepts exactly GIBSON_EVENT_QUEUE_CAPACITY
 *      events, refuses the next with GIBSON_ERR_QUEUE_FULL, delivers each once in order; a pushed
 *      RESIZE, once polled, sets the headless frame geometry.
 *   7. (v0.7 node I) a C-side RGB raster, moved only by injected keys, is lowered BY THE CONSUMER to
 *      half-block cells through the styled-span ABI (one U+2580 span per cell: fg = top pixel,
 *      bg = bottom pixel) and read back exactly: every cell, every frame, at TRUECOLOR. No raster
 *      entry point is needed for this; the per-frame cost is reported.
 *
 * Any violated invariant is a nonzero exit with a diagnostic on stderr; success prints a one-line
 * summary and exits 0. The ABI symbols exercised: gibson_create_headless_context, gibson_set_root_node,
 * gibson_node_box_col, gibson_node_text, gibson_node_add_child, gibson_render, gibson_get_frame_size,
 * gibson_get_frame_cell, gibson_get_frame_cells, gibson_get_frame_cell_glyph, gibson_set_color_depth,
 * gibson_get_rendered_output, gibson_push_event, gibson_poll_event, gibson_destroy_context,
 * gibson_line_new, gibson_line_add_span, gibson_line_free, gibson_rich_text_new,
 * gibson_rich_text_add_line, gibson_rich_text_free, gibson_node_rich_text.
 */
#define _POSIX_C_SOURCE 199309L
#include <gibson.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static int fail(const char *m) {
    fprintf(stderr, "doom_forcing_consumer: FAIL: %s\n", m);
    return 1;
}

#define CHECK(cond, msg) \
    do { if (!(cond)) return fail(msg); } while (0)
#define OK(expr) \
    do { if ((expr) != GIBSON_OK) return fail(#expr); } while (0)

/* Build a fresh root whose single text row is `line` with `style`, and install+render it. */
static int stage_line(gibson_context_t *ctx, const char *line, const gibson_style_t *style) {
    gibson_node_t *root = NULL, *txt = NULL;
    OK(gibson_node_box_col(&root));
    OK(gibson_node_text(line, style, GIBSON_WRAP_NONE, &txt));
    OK(gibson_node_add_child(root, txt));
    OK(gibson_set_root_node(ctx, root));
    OK(gibson_render(ctx));
    return GIBSON_OK;
}

/* ---- (7) Node I: a consumer-side raster through the existing ABI ---- */
enum { RW = 48, RH = 16, RFRAMES = 60 }; /* pixels; RH/2 cell rows of half-blocks */

static void paint(uint8_t px[RH][RW][3], int phase) {
    for (int y = 0; y < RH; y++)
        for (int x = 0; x < RW; x++) {
            px[y][x][0] = (uint8_t)(x * 5 + phase * 7);
            px[y][x][1] = (uint8_t)(y * 16 + phase * 3);
            px[y][x][2] = (uint8_t)((x ^ y) * 9 + phase);
        }
}

static gibson_color_t rgb(const uint8_t p[3]) {
    gibson_color_t c;
    memset(&c, 0, sizeof c);
    c.color_type = GIBSON_COLOR_RGB;
    c.r = p[0];
    c.g = p[1];
    c.b = p[2];
    return c;
}

static int same_rgb(const gibson_color_t *c, const uint8_t p[3]) {
    return c->color_type == GIBSON_COLOR_RGB && c->r == p[0] && c->g == p[1] && c->b == p[2];
}

/* Lowers the raster to one rich-text node of U+2580 cells and installs it as the root. */
static int stage_raster(gibson_context_t *ctx, uint8_t px[RH][RW][3]) {
    gibson_rich_text_t *rich = NULL;
    OK(gibson_rich_text_new(&rich));
    for (int cy = 0; cy < RH / 2; cy++) {
        gibson_line_t *line = NULL;
        if (gibson_line_new(&line) != GIBSON_OK) { gibson_rich_text_free(rich); return fail("line_new"); }
        for (int cx = 0; cx < RW; cx++) {
            gibson_style_t st;
            memset(&st, 0, sizeof st);
            st.fg = rgb(px[2 * cy][cx]);
            st.bg = rgb(px[2 * cy + 1][cx]);
            if (gibson_line_add_span(line, "\xe2\x96\x80", &st) != GIBSON_OK) {
                gibson_line_free(line);
                gibson_rich_text_free(rich);
                return fail("line_add_span");
            }
        }
        gibson_status_t st = gibson_rich_text_add_line(rich, line);
        gibson_line_free(line);
        if (st != GIBSON_OK) { gibson_rich_text_free(rich); return fail("rich_text_add_line"); }
    }
    gibson_node_t *node = NULL;
    gibson_status_t st = gibson_node_rich_text(rich, GIBSON_WRAP_NONE, &node);
    gibson_rich_text_free(rich);
    if (st != GIBSON_OK) return fail("node_rich_text");
    OK(gibson_set_root_node(ctx, node));
    return GIBSON_OK;
}

/* Runs the raster leg; returns 0 on success, writing what it checked. */
static int raster_leg(int *phase_out, size_t *cells_checked, double *us_per_frame) {
    static uint8_t px[RH][RW][3];
    gibson_context_t *ctx = NULL;
    OK(gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, RW, RH / 2, &ctx));
    if (gibson_set_color_depth(ctx, GIBSON_COLOR_DEPTH_TRUECOLOR) != GIBSON_OK) {
        gibson_destroy_context(ctx);
        return fail("raster: set_color_depth");
    }
    gibson_cell_t *cells = calloc((size_t)RW * (RH / 2), sizeof *cells);
    if (!cells) { gibson_destroy_context(ctx); return fail("allocate raster readback"); }
    int phase = 0, expected = 0, rc = 0;
    struct timespec t0, t1;
    if (clock_gettime(CLOCK_MONOTONIC, &t0) != 0) {
        free(cells);
        gibson_destroy_context(ctx);
        return fail("raster: clock_gettime");
    }
    for (int f = 0; f < RFRAMES && rc == 0; f++) {
        /* Input: 'a' every third frame, else 'd'. The phase moves only on a polled key. */
        gibson_event_t k;
        memset(&k, 0, sizeof k);
        k.event_type = GIBSON_EVENT_KEY;
        k.key_code = (f % 3 == 0) ? 'a' : 'd';
        expected += (f % 3 == 0) ? -1 : 1;
        if (gibson_push_event(ctx, &k) != GIBSON_OK) { rc = fail("raster: push"); break; }
        int delivered = 0;
        uint32_t got = 0;
        for (;;) {
            gibson_event_t ev;
            if (gibson_poll_event(ctx, 0, &ev) != GIBSON_OK) { rc = fail("raster: poll"); break; }
            if (ev.event_type == GIBSON_EVENT_NONE) break;
            delivered++;
            got = ev.key_code;
            if (ev.event_type == GIBSON_EVENT_KEY && ev.key_code == 'd') phase++;
            if (ev.event_type == GIBSON_EVENT_KEY && ev.key_code == 'a') phase--;
        }
        if (rc) break;
        /* Every frame: exactly the one key pushed, delivered, and the phase it implies. */
        if (delivered != 1 || got != k.key_code || phase != expected) {
            rc = fail("raster: this frame's injected key was not delivered exactly once");
            break;
        }
        paint(px, phase);
        if (stage_raster(ctx, px) != GIBSON_OK || gibson_render(ctx) != GIBSON_OK) { rc = fail("raster: render"); break; }
        size_t written = 0;
        if (gibson_get_frame_cells(ctx, 0, 0, cells, (size_t)RW * (RH / 2), &written) != GIBSON_OK ||
            written != (size_t)RW * (RH / 2)) { rc = fail("raster: bulk readback"); break; }
        for (int cy = 0; cy < RH / 2 && rc == 0; cy++)
            for (int cx = 0; cx < RW; cx++) {
                const gibson_cell_t *c = &cells[(size_t)cy * RW + cx];
                if (c->glyph_len != 3 || memcmp(c->glyph, "\xe2\x96\x80", 3) != 0 ||
                    !same_rgb(&c->style.fg, px[2 * cy][cx]) || !same_rgb(&c->style.bg, px[2 * cy + 1][cx])) {
                    rc = fail("raster: a read-back cell differs from its two source pixels");
                    break;
                }
                (*cells_checked)++;
            }
    }
    if (clock_gettime(CLOCK_MONOTONIC, &t1) != 0 && rc == 0) rc = fail("raster: clock_gettime");
    *us_per_frame = ((double)(t1.tv_sec - t0.tv_sec) * 1e6 + (double)(t1.tv_nsec - t0.tv_nsec) / 1e3) / RFRAMES;
    *phase_out = phase;
    free(cells);
    gibson_destroy_context(ctx);
    if (rc) return rc;
    CHECK(phase == expected && phase != 0, "raster: the phase did not follow the injected keys");
    return 0;
}

int main(void) {
    enum { COLS = 64, ROWS = 8, FRAMES = 240 };

    /* FULLSCREEN headless = a FIXED cols x rows framebuffer (INLINE is content-sized, so its frame
       height tracks the content, not the requested grid — the wrong shape for a raster target). */
    gibson_context_t *ctx = NULL;
    if (gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, COLS, ROWS, &ctx) != GIBSON_OK)
        return fail("create headless context from the installed SDK");

    gibson_style_t plain;
    memset(&plain, 0, sizeof plain);

    const size_t grid = (size_t)COLS * (size_t)ROWS;
    gibson_cell_t *bulk = calloc(grid, sizeof *bulk);
    CHECK(bulk != NULL, "allocate bulk cell buffer");

    unsigned long long checksum = 0;
    size_t total_cells_read = 0;
    size_t events_delivered = 0;
    int marker = 0;

    /* ---- The Doom-shaped frame loop: input -> render -> full-frame bulk readback ---- */
    for (int f = 0; f < FRAMES; f++) {
        /* Injected input: after the first frame, one RIGHT key per frame moves the marker one
           column (wrapping before the last column). The marker moves only on a polled event. */
        if (f > 0) {
            gibson_event_t right;
            memset(&right, 0, sizeof right);
            right.event_type = GIBSON_EVENT_KEY;
            right.key_code = GIBSON_KEY_RIGHT;
            if (gibson_push_event(ctx, &right) != GIBSON_OK) { free(bulk); return fail("push RIGHT"); }
        }
        for (;;) {
            gibson_event_t ev;
            if (gibson_poll_event(ctx, 0, &ev) != GIBSON_OK) { free(bulk); return fail("poll"); }
            if (ev.event_type == GIBSON_EVENT_NONE) break;
            if (ev.event_type != GIBSON_EVENT_KEY || ev.key_code != GIBSON_KEY_RIGHT || ev.modifiers != 0) {
                free(bulk);
                return fail("polled event differs from the pushed RIGHT key");
            }
            marker = (marker + 1) % (COLS - 1);
            events_delivered++;
        }
        if (events_delivered != (size_t)f) { free(bulk); return fail("each pushed event must be delivered exactly once"); }
        char line[COLS + 1];
        for (int i = 0; i < COLS; i++) line[i] = (i == marker) ? '#' : '.';
        line[COLS] = '\0';
        if (stage_line(ctx, line, &plain) != GIBSON_OK) { free(bulk); return 1; }

        uint16_t cols = 0, rows = 0;
        if (gibson_get_frame_size(ctx, &cols, &rows) != GIBSON_OK) { free(bulk); return fail("frame size"); }
        if (cols != COLS || rows != ROWS) { free(bulk); return fail("frame size != requested"); }

        /* (2) ONE bulk call pulls the whole framebuffer. */
        size_t written = 0;
        if (gibson_get_frame_cells(ctx, 0, 0, bulk, grid, &written) != GIBSON_OK) { free(bulk); return fail("bulk readback"); }
        if (written != grid) { free(bulk); return fail("bulk readback did not fill the whole frame"); }
        total_cells_read += written;

        /* (1) input closed the loop: the '#' is at the demanded column on row 0. */
        int found = -1;
        for (int x = 0; x < COLS; x++) {
            const gibson_cell_t *c = &bulk[x];
            if (c->glyph_len == 1 && c->glyph[0] == '#') { found = x; break; }
        }
        if (found != marker) { free(bulk); return fail("marker column did not track the injected input"); }

        /* (2) bulk == single, byte for byte, on a stride sample across the frame. */
        for (size_t i = 0; i < grid; i += 7) {
            uint16_t x = (uint16_t)(i % COLS), y = (uint16_t)(i / COLS);
            gibson_cell_t single;
            memset(&single, 0, sizeof single);
            if (gibson_get_frame_cell(ctx, x, y, &single) != GIBSON_OK) { free(bulk); return fail("single-cell readback"); }
            if (memcmp(&single, &bulk[i], sizeof single) != 0) { free(bulk); return fail("bulk and single-cell readback diverged"); }
        }

        /* A cheap raster hash so the loop provably consumed every frame's pixels. */
        for (size_t i = 0; i < grid; i++) {
            const gibson_cell_t *c = &bulk[i];
            for (unsigned b = 0; b < c->glyph_len; b++) checksum = checksum * 1099511628211ULL + c->glyph[b];
            checksum += (unsigned)c->style.fg.r + c->style.bg.b;
        }
    }

    /* The frames are pinned: the same 240 frames the v0.6 consumer rendered from simulated input
       (docs/V07_BASELINE.md). A different value means the frames changed. */
    if (checksum != 15734824182601092804ULL) { free(bulk); return fail("frame checksum != the pinned 15734824182601092804"); }

    /* ---- (3) A live mid-session color-depth change restyles the readback ---- */
    gibson_style_t styled;
    memset(&styled, 0, sizeof styled);
    styled.fg.color_type = GIBSON_COLOR_RGB;
    styled.fg.r = 255; styled.fg.g = 128; styled.fg.b = 0;
    styled.bold = 1;

    OK(gibson_set_color_depth(ctx, GIBSON_COLOR_DEPTH_TRUECOLOR));
    if (stage_line(ctx, "Gate-E", &styled) != GIBSON_OK) { free(bulk); return 1; }
    gibson_cell_t tc;
    memset(&tc, 0, sizeof tc);
    if (gibson_get_frame_cell(ctx, 0, 0, &tc) != GIBSON_OK) { free(bulk); return fail("truecolor cell"); }
    CHECK(tc.style.fg.color_type == GIBSON_COLOR_RGB && tc.style.fg.r == 255,
          "truecolor styled cell did not resolve to RGB(255,128,0)");

    OK(gibson_set_color_depth(ctx, GIBSON_COLOR_DEPTH_MONO));
    if (stage_line(ctx, "Gate-E", &styled) != GIBSON_OK) { free(bulk); return 1; }
    gibson_cell_t mono;
    memset(&mono, 0, sizeof mono);
    if (gibson_get_frame_cell(ctx, 0, 0, &mono) != GIBSON_OK) { free(bulk); return fail("mono cell"); }
    CHECK(memcmp(&tc.style, &mono.style, sizeof tc.style) != 0,
          "set_color_depth(MONO) did not restyle the readback (depth change ignored)");

    /* ---- (4) The long-glyph escape hatch recovers a cluster longer than the inline cap ---- */
    /* 👨‍👩‍👧‍👦 — one grapheme cluster, 25 UTF-8 bytes, > GIBSON_CELL_GLYPH_CAP (16). */
    const char *family =
        "\xf0\x9f\x91\xa8\xe2\x80\x8d\xf0\x9f\x91\xa9\xe2\x80\x8d\xf0\x9f\x91\xa7\xe2\x80\x8d\xf0\x9f\x91\xa6";
    OK(gibson_set_color_depth(ctx, GIBSON_COLOR_DEPTH_TRUECOLOR));
    if (stage_line(ctx, family, &plain) != GIBSON_OK) { free(bulk); return 1; }
    gibson_cell_t emoji;
    memset(&emoji, 0, sizeof emoji);
    if (gibson_get_frame_cell(ctx, 0, 0, &emoji) != GIBSON_OK) { free(bulk); return fail("emoji cell"); }
    size_t full_len = 0;
    if (gibson_get_frame_cell_glyph(ctx, 0, 0, NULL, 0, &full_len) != GIBSON_OK) { free(bulk); return fail("glyph size query"); }
    CHECK(full_len == strlen(family), "long glyph full length != the 25-byte cluster");
    CHECK(full_len > GIBSON_CELL_GLYPH_CAP, "the cluster is not actually longer than the inline cap");
    CHECK(emoji.glyph_len <= GIBSON_CELL_GLYPH_CAP, "inline glyph buffer overran its cap");
    uint8_t fbuf[64];
    CHECK(full_len <= sizeof fbuf, "harness buffer too small for the cluster");
    size_t copied = full_len;
    if (gibson_get_frame_cell_glyph(ctx, 0, 0, fbuf, copied, &copied) != GIBSON_OK) { free(bulk); return fail("glyph copy"); }
    CHECK(copied == strlen(family) && memcmp(fbuf, family, copied) == 0,
          "the recovered cluster bytes did not match the source emoji");

    /* ---- (5) The ANSI output path yields bytes ---- */
    uint8_t ansi[4096];
    size_t ansi_len = 0;
    if (gibson_get_rendered_output(ctx, ansi, sizeof ansi, &ansi_len) != GIBSON_OK) { free(bulk); return fail("ansi output"); }
    CHECK(ansi_len > 0, "rendered ANSI output was empty");

    /* ---- (6) Injected input is bounded and exact ---- */
    size_t accepted = 0;
    for (;;) {
        gibson_event_t k;
        memset(&k, 0, sizeof k);
        k.event_type = GIBSON_EVENT_KEY;
        k.key_code = 0x4E00u + (uint32_t)(accepted % 1000u); /* CJK ideographs */
        gibson_status_t st = gibson_push_event(ctx, &k);
        if (st == GIBSON_ERR_QUEUE_FULL) break;
        if (st != GIBSON_OK) { free(bulk); return fail("push into a non-full queue"); }
        if (++accepted > GIBSON_EVENT_QUEUE_CAPACITY) { free(bulk); return fail("queue accepted more than its capacity"); }
    }
    CHECK(accepted == GIBSON_EVENT_QUEUE_CAPACITY, "queue refused before reaching its capacity");
    for (size_t i = 0; i < accepted; i++) {
        gibson_event_t ev;
        if (gibson_poll_event(ctx, 0, &ev) != GIBSON_OK) { free(bulk); return fail("drain poll"); }
        if (ev.event_type != GIBSON_EVENT_KEY || ev.key_code != 0x4E00u + (uint32_t)(i % 1000u)) {
            free(bulk);
            return fail("queued events were not delivered in push order");
        }
    }
    gibson_event_t none;
    OK(gibson_poll_event(ctx, 0, &none));
    CHECK(none.event_type == GIBSON_EVENT_NONE, "an event was delivered twice or a refused one was queued");

    gibson_event_t rs;
    memset(&rs, 0, sizeof rs);
    rs.event_type = GIBSON_EVENT_RESIZE;
    rs.cols = 40;
    rs.rows = 6;
    OK(gibson_push_event(ctx, &rs));
    gibson_event_t got;
    OK(gibson_poll_event(ctx, 0, &got));
    CHECK(got.event_type == GIBSON_EVENT_RESIZE && got.cols == 40 && got.rows == 6, "resize did not poll back");
    if (stage_line(ctx, "resized", &plain) != GIBSON_OK) { free(bulk); return 1; }
    uint16_t rc = 0, rr = 0;
    OK(gibson_get_frame_size(ctx, &rc, &rr));
    CHECK(rc == 40 && rr == 6, "a polled RESIZE did not set the headless frame geometry");

    gibson_destroy_context(ctx);
    free(bulk);

    int raster_phase = 0;
    size_t raster_cells = 0;
    double raster_us = 0;
    if (raster_leg(&raster_phase, &raster_cells, &raster_us) != 0) return 1;

    printf("doom_forcing_consumer: OK  frames=%d grid=%dx%d cells_read=%zu long_glyph=%zuB ansi=%zuB events=%zu queue=%zu checksum=%llu raster=%dx%dpx frames=%d phase=%d cells_checked=%zu us_per_frame=%.0f\n",
           FRAMES, COLS, ROWS, total_cells_read, full_len, ansi_len, events_delivered, accepted, checksum,
           RW, RH, RFRAMES, raster_phase, raster_cells, raster_us);
    return 0;
}
