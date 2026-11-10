/* A headless 4x4 software-framebuffer consumer of fb_adapter and the C SDK. */
#include "fb_adapter.h"
#include <stdio.h>
#include <string.h>

#define WIDTH 4u
#define HEIGHT 4u

/* The alpha byte is deliberately varied: fb_adapter_submit_argb32 ignores it. */
static const uint32_t pattern[WIDTH * HEIGHT] = {
    0x00112233u, 0x40445566u, 0x80778899u, 0xffaabbccu,
    0xff314253u, 0x00647586u, 0x4097a8b9u, 0x80cadbecu,
    0x80010203u, 0xff102030u, 0x00204060u, 0x4080a0c0u,
    0x40fedcbau, 0x80cba987u, 0xff987654u, 0x00654321u,
};

static int rgb_equals(gibson_color_t c, uint32_t value) {
    return c.color_type == GIBSON_COLOR_RGB && c.r == ((value >> 16) & 255u) &&
           c.g == ((value >> 8) & 255u) && c.b == (value & 255u);
}

int main(int argc, char **argv) {
    int wrong_oracle = 0;
    if (argc == 2 && strcmp(argv[1], "--wrong-oracle") == 0) wrong_oracle = 1;
    else if (argc != 1) {
        fprintf(stderr, "usage: %s [--wrong-oracle]\n", argv[0]);
        return 2;
    }
    if (gibson_abi_version() != GIBSON_ABI_VERSION) {
        fprintf(stderr, "header/library ABI mismatch\n");
        return 1;
    }

    fb_adapter_config cfg;
    fb_adapter_config_defaults(&cfg, WIDTH, HEIGHT);
    cfg.color_depth = GIBSON_COLOR_DEPTH_TRUECOLOR;
    fb_adapter *ad = NULL;
    if (fb_adapter_open_headless(&cfg, WIDTH, HEIGHT / 2, &ad) != FB_OK) {
        fprintf(stderr, "adapter open: %s\n", fb_adapter_last_error());
        return 1;
    }
    int result = 1;
    uint32_t pixels[WIDTH * HEIGHT];
    memcpy(pixels, pattern, sizeof pixels);
    /* A pitch shorter than the declared width must be rejected. */
    if (fb_adapter_submit_argb32(ad, pixels, WIDTH - 1) != FB_ERR_PARAM) {
        fprintf(stderr, "short pitch was not rejected\n");
        goto done;
    }
    if (fb_adapter_submit_argb32(ad, pixels, WIDTH) != FB_OK) {
        fprintf(stderr, "frame submission: %s\n", fb_adapter_last_error());
        goto done;
    }
    /* The adapter documents copying caller pixels; prove this frame is owned. */
    memset(pixels, 0, sizeof pixels);
    if (fb_adapter_present(ad) != FB_OK) {
        fprintf(stderr, "present: %s\n", fb_adapter_last_error());
        goto done;
    }
    gibson_context_t *ctx = fb_adapter_context(ad);
    uint16_t cols = 0, rows = 0;
    if (gibson_get_frame_size(ctx, &cols, &rows) != GIBSON_OK || cols != WIDTH || rows != HEIGHT / 2) {
        fprintf(stderr, "unexpected rendered geometry\n");
        goto done;
    }
    /* Equal source and virtual-pixel dimensions give a 1:1 nearest-neighbour map.
       Each U+2580 cell must carry its top source pixel as fg, bottom as bg. */
    for (uint16_t y = 0; y < rows; ++y) {
        for (uint16_t x = 0; x < cols; ++x) {
            gibson_cell_t cell;
            uint32_t top = pattern[(2u * y) * WIDTH + x];
            uint32_t bottom = pattern[(2u * y + 1u) * WIDTH + x];
            if (wrong_oracle && x == 0 && y == 0) top ^= 1u;
            if (gibson_get_frame_cell(ctx, x, y, &cell) != GIBSON_OK ||
                cell.glyph_len != 3 || memcmp(cell.glyph, "\xe2\x96\x80", 3) != 0 ||
                !rgb_equals(cell.style.fg, top) || !rgb_equals(cell.style.bg, bottom)) {
                fprintf(stderr, "cell mismatch at %u,%u\n", x, y);
                goto done;
            }
        }
    }
    puts("framebuffer: 4x4 pixels rendered to 4x2 cells OK");
    result = 0;
done:
    fb_adapter_close(ad);
    return result;
}
