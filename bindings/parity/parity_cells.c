/*
 * FFI styled cell-grid parity harness (C side) — #142, v0.6 Gate J.
 *
 * Renders the canonical headless op-script (an RGB-styled, bold "Gate-I" node, truecolor) and
 * serializes the whole resolved cell grid to argv[1] via gibson_get_frame_size +
 * gibson_get_frame_cell. The line format is identical to parity_cells.py and to
 * tests/ffi_cell_grid_parity.rs:
 *   size <cols> <rows>
 *   <x> <y> <glyph_hex|-> <w> <cont> <transp> <fgct> <fgr> <fgg> <fgb> <bgct> <bgr> <bgg> <bgb> <bold> <dim> <ital> <under> <rev>
 *
 * The CI step `cmp`s this against parity_cells.py and the committed golden_cells.txt, which the
 * Rust job verifies against the native API. See tests/ffi_cell_grid_parity.rs for the full contract.
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../../include/gibson.h"

static int die(const char *m) {
    fprintf(stderr, "parity_cells_c: %s\n", m);
    return 1;
}

static void emit_cell(FILE *o, unsigned x, unsigned y, const gibson_cell_t *c) {
    fprintf(o, "%u %u ", x, y);
    if (c->glyph_len == 0) {
        fputc('-', o);
    } else {
        for (unsigned i = 0; i < c->glyph_len; i++) fprintf(o, "%02x", c->glyph[i]);
    }
    fprintf(o, " %u %u %u %d %u %u %u %d %u %u %u %u %u %u %u %u\n",
            c->width, c->is_continuation, c->transparent,
            c->style.fg.color_type, c->style.fg.r, c->style.fg.g, c->style.fg.b,
            c->style.bg.color_type, c->style.bg.r, c->style.bg.g, c->style.bg.b,
            c->style.bold, c->style.dim, c->style.italic, c->style.underline, c->style.reverse);
}

int main(int argc, char **argv) {
    if (argc < 2) return die("usage: parity_cells_c GRID_OUT");

    gibson_context_t *ctx = NULL;
    if (gibson_create_headless_context(GIBSON_MODE_INLINE, 24, 4, &ctx) != GIBSON_OK)
        return die("create headless context");

    gibson_style_t style;
    memset(&style, 0, sizeof style);
    style.fg.color_type = GIBSON_COLOR_RGB;
    style.fg.r = 255; style.fg.g = 128; style.fg.b = 0;
    style.bg.color_type = GIBSON_COLOR_RGB;
    style.bg.r = 16; style.bg.g = 32; style.bg.b = 64;
    style.bold = 1;

#define OK(x) do { if ((x) != GIBSON_OK) return die(#x); } while (0)
    gibson_node_t *root = NULL, *txt = NULL;
    OK(gibson_node_box_col(&root));
    OK(gibson_node_text("Gate-I", &style, GIBSON_WRAP_NONE, &txt));
    OK(gibson_node_add_child(root, txt));
    OK(gibson_set_root_node(ctx, root));
    OK(gibson_render(ctx));

    uint16_t cols = 0, rows = 0;
    OK(gibson_get_frame_size(ctx, &cols, &rows));
#undef OK

    FILE *o = fopen(argv[1], "w");
    if (!o) return die("open grid out");
    fprintf(o, "size %u %u\n", cols, rows);
    for (uint16_t y = 0; y < rows; y++) {
        for (uint16_t x = 0; x < cols; x++) {
            gibson_cell_t cell;
            memset(&cell, 0, sizeof cell);
            if (gibson_get_frame_cell(ctx, x, y, &cell) != GIBSON_OK) {
                fclose(o);
                return die("get_frame_cell");
            }
            emit_cell(o, x, y, &cell);
        }
    }
    fclose(o);

    gibson_destroy_context(ctx);
    return 0;
}
