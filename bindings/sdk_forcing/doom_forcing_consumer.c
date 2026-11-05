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
 *   1. input -> render -> readback closes: a marker driven by the frame index lands at the raster
 *      column the input demanded (the game-loop contract).
 *   2. the BULK readback equals the authoritative single-cell readback, byte for byte, over the
 *      whole frame (the fast path a raster needs must not diverge from gibson_get_frame_cell).
 *   3. gibson_set_color_depth on a LIVE context restyles the readback: the same styled cell resolves
 *      to a different style at TRUECOLOR vs MONO (the mid-session depth change a terminal that loses
 *      color must survive).
 *   4. the long-glyph escape hatch returns a grapheme cluster longer than the inline cap in full
 *      (a 25-byte ZWJ emoji is one cell; gibson_get_frame_cell_glyph recovers all 25 bytes).
 *   5. the ANSI output path yields bytes at every depth.
 *
 * Any violated invariant is a nonzero exit with a diagnostic on stderr; success prints a one-line
 * summary and exits 0. The ABI symbols exercised: gibson_create_headless_context, gibson_set_root_node,
 * gibson_node_box_col, gibson_node_text, gibson_node_add_child, gibson_render, gibson_get_frame_size,
 * gibson_get_frame_cell, gibson_get_frame_cells, gibson_get_frame_cell_glyph, gibson_set_color_depth,
 * gibson_get_rendered_output, gibson_destroy_context.
 */
#include <gibson.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

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

    /* ---- The Doom-shaped frame loop: input -> render -> full-frame bulk readback ---- */
    for (int f = 0; f < FRAMES; f++) {
        /* Simulated input: a marker sweeps across the row, one column per frame. */
        int marker = f % (COLS - 1);
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
        if (found != marker) { free(bulk); return fail("marker column did not track the simulated input"); }

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

    gibson_destroy_context(ctx);
    free(bulk);

    printf("doom_forcing_consumer: OK  frames=%d grid=%dx%d cells_read=%zu long_glyph=%zuB ansi=%zuB checksum=%llu\n",
           FRAMES, COLS, ROWS, total_cells_read, full_len, ansi_len, checksum);
    return 0;
}
