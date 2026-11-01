/*
 * FFI color-depth *downgrade* parity harness (C side) — #142, v0.6 Gate J.
 *
 * Drives the canonical headless frame op-script (a single RGB-styled, bold text node in a column)
 * through the C ABI at a chosen color depth: gibson_create_headless_context, then
 * gibson_set_color_depth(DEPTH), then render. Writes the raw painted ANSI frame bytes to argv[2].
 * DEPTH (argv[1]) is a gibson_color_depth_t int (0=MONO, 1=ANSI16, 2=ANSI256, 3=TRUECOLOR).
 *
 * The CI step `cmp`s this output against bindings/parity/parity_depth.py and the committed golden
 * for that depth (golden_depth_{mono,ansi16,ansi256}.bin), which the Rust job verifies against the
 * native API. See tests/ffi_color_depth_parity.rs for the full contract.
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../../include/gibson.h"

static int die(const char *m) {
    fprintf(stderr, "parity_depth_c: %s\n", m);
    return 1;
}

int main(int argc, char **argv) {
    if (argc < 3) return die("usage: parity_depth_c DEPTH FRAME_OUT");
    int depth = atoi(argv[1]);

    gibson_context_t *ctx = NULL;
    if (gibson_create_headless_context(GIBSON_MODE_INLINE, 24, 4, &ctx) != GIBSON_OK)
        return die("create headless context");
    if (gibson_set_color_depth(ctx, depth) != GIBSON_OK)
        return die("set color depth");

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
    OK(gibson_set_root_node(ctx, root)); /* takes ownership of the tree */
    OK(gibson_render(ctx));

    size_t need = 0;
    OK(gibson_get_rendered_output(ctx, NULL, 0, &need));
    uint8_t *buf = (uint8_t *)malloc(need ? need : 1);
    if (!buf) return die("malloc");
    size_t got = 0;
    OK(gibson_get_rendered_output(ctx, buf, need, &got));
#undef OK

    FILE *fo = fopen(argv[2], "wb");
    if (!fo) { free(buf); return die("open frame out"); }
    if (fwrite(buf, 1, got, fo) != got) { fclose(fo); free(buf); return die("write frame"); }
    fclose(fo);
    free(buf);

    gibson_destroy_context(ctx);
    return 0;
}
