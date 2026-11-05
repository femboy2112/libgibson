/*
 * FFI long-glyph cell-grid parity harness (C side) — #142, v0.6 Gate K (K2).
 *
 * Renders "a" + a 25-byte ZWJ family emoji (width 2) + "Z", styled, in a headless 24x4 Inline
 * context, and serializes the whole grid via the NEW bulk gibson_get_frame_cells, the
 * glyph_truncated flag (gibson_cell_t._reserved[0]), and gibson_get_frame_cell_glyph for each
 * cell's full (untruncated) bytes. The line format is identical to parity_cells_long.py and
 * tests/ffi_cell_readback.rs:
 *   size <cols> <rows>
 *   <x> <y> <glyph_hex|-> <glyph_len> <trunc> <full_hex|-> <w> <cont> <transp> <fgct> <fgr> <fgg> <fgb> <bgct> <bgr> <bgg> <bgb> <bold> <dim> <ital> <under> <rev>
 *
 * CI cmp's this == parity_cells_long.py == golden_cells_long.txt.
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../../include/gibson.h"

static int die(const char *m) {
    fprintf(stderr, "parity_cells_long_c: %s\n", m);
    return 1;
}

/* Lowercase hex of n bytes into out (sized >= 2n+1), or "-" when empty. */
static void hexcat(char *out, const uint8_t *b, size_t n) {
    if (n == 0) {
        strcpy(out, "-");
        return;
    }
    for (size_t i = 0; i < n; i++) sprintf(out + 2 * i, "%02x", b[i]);
}

int main(int argc, char **argv) {
    if (argc < 2) return die("usage: parity_cells_long_c GRID_OUT");

    gibson_context_t *ctx = NULL;
    if (gibson_create_headless_context(GIBSON_MODE_INLINE, 24, 4, &ctx) != GIBSON_OK)
        return die("create headless context");

    gibson_style_t style;
    memset(&style, 0, sizeof style);
    style.fg.color_type = GIBSON_COLOR_RGB;
    style.fg.r = 200; style.fg.g = 50; style.fg.b = 50;
    style.bold = 1;

    /* "a" + U+1F468 ZWJ U+1F469 ZWJ U+1F467 ZWJ U+1F466 + "Z" (explicit UTF-8 bytes). */
    const char *text =
        "a"
        "\xf0\x9f\x91\xa8\xe2\x80\x8d\xf0\x9f\x91\xa9\xe2\x80\x8d\xf0\x9f\x91\xa7\xe2\x80\x8d\xf0\x9f\x91\xa6"
        "Z";

#define OK(x) do { if ((x) != GIBSON_OK) return die(#x); } while (0)
    gibson_node_t *root = NULL, *txt = NULL;
    OK(gibson_node_box_col(&root));
    OK(gibson_node_text(text, &style, GIBSON_WRAP_NONE, &txt));
    OK(gibson_node_add_child(root, txt));
    OK(gibson_set_root_node(ctx, root));
    OK(gibson_render(ctx));

    uint16_t cols = 0, rows = 0;
    OK(gibson_get_frame_size(ctx, &cols, &rows));

    size_t n = (size_t)cols * (size_t)rows;
    gibson_cell_t *cells = calloc(n ? n : 1, sizeof *cells);
    if (!cells) return die("alloc cells");
    size_t written = 0;
    OK(gibson_get_frame_cells(ctx, 0, 0, cells, n, &written));
    if (written != n) { free(cells); return die("short bulk read"); }
#undef OK

    FILE *o = fopen(argv[1], "w");
    if (!o) { free(cells); return die("open grid out"); }
    fprintf(o, "size %u %u\n", cols, rows);
    for (size_t i = 0; i < n; i++) {
        unsigned x = (unsigned)(i % cols), y = (unsigned)(i / cols);
        const gibson_cell_t *c = &cells[i];

        char ghex[2 * GIBSON_CELL_GLYPH_CAP + 2];
        hexcat(ghex, c->glyph, c->glyph_len);

        uint8_t fbuf[256];
        size_t flen = 0;
        if (gibson_get_frame_cell_glyph(ctx, (uint16_t)x, (uint16_t)y, NULL, 0, &flen) != GIBSON_OK) {
            fclose(o); free(cells); return die("glyph size query");
        }
        if (flen > sizeof fbuf) { fclose(o); free(cells); return die("glyph too long for harness buffer"); }
        if (flen > 0 &&
            gibson_get_frame_cell_glyph(ctx, (uint16_t)x, (uint16_t)y, fbuf, flen, &flen) != GIBSON_OK) {
            fclose(o); free(cells); return die("glyph copy");
        }
        char fhex[2 * 256 + 2];
        hexcat(fhex, fbuf, flen);

        fprintf(o, "%u %u %s %u %u %s %u %u %u %d %u %u %u %d %u %u %u %u %u %u %u %u\n",
                x, y, ghex, c->glyph_len, c->_reserved[0], fhex,
                c->width, c->is_continuation, c->transparent,
                c->style.fg.color_type, c->style.fg.r, c->style.fg.g, c->style.fg.b,
                c->style.bg.color_type, c->style.bg.r, c->style.bg.g, c->style.bg.b,
                c->style.bold, c->style.dim, c->style.italic, c->style.underline, c->style.reverse);
    }
    fclose(o);
    free(cells);

    gibson_destroy_context(ctx);
    return 0;
}
