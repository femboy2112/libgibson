#include <gibson.h>
#include <stdio.h>

#define CHECK(call) do { if ((call) != GIBSON_OK) { fprintf(stderr, "%s failed\n", #call); goto fail; } } while (0)
int main(void) {
    gibson_context_t *ctx = NULL;
    gibson_node_t *root = NULL, *text = NULL;
    uint16_t cols = 0, rows = 0;
    uint8_t glyph[8]; size_t glyph_len = 0;
    if (gibson_abi_version() != GIBSON_ABI_VERSION) {
        fprintf(stderr, "ABI mismatch: header=%u library=%u\n", GIBSON_ABI_VERSION, gibson_abi_version());
        return 1;
    }
    CHECK(gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, 40, 8, &ctx));
    CHECK(gibson_node_box_col(&root));
    CHECK(gibson_node_text("Hello, LibGibson", NULL, GIBSON_WRAP_NONE, &text));
    CHECK(gibson_node_add_child(root, text));
    text = NULL; /* ownership moved into root */
    CHECK(gibson_set_root_node(ctx, root));
    root = NULL; /* ownership moved into context */
    CHECK(gibson_render(ctx));
    CHECK(gibson_get_frame_size(ctx, &cols, &rows));
    if (cols != 40 || rows != 8) { fprintf(stderr, "unexpected frame size\n"); goto fail; }
    CHECK(gibson_get_frame_cell_glyph(ctx, 0, 0, glyph, sizeof glyph, &glyph_len));
    if (glyph_len != 1 || glyph[0] != 'H') { fprintf(stderr, "unexpected frame content\n"); goto fail; }
    printf("C headless frame %ux%u OK\n", cols, rows);
    gibson_destroy_context(ctx);
    return 0;
fail:
    /* Exit immediately on error: a panicking ownership transfer may have consumed a node. */
    if (ctx) gibson_destroy_context(ctx);
    return 1;
}
