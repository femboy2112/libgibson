/*
 * FFI pixel/ANSI *frame* parity harness (C side) — #142, v0.6 Gate I.
 *
 * Drives the canonical headless frame op-script through the new C ABI
 * (gibson_create_headless_context + gibson_get_rendered_output): a single RGB-styled, bold text
 * node in a column, rendered once. It writes the painted stats report to argv[1] and the raw
 * painted ANSI frame bytes to argv[2]. Unlike the Gate H stats harness there is NO fd redirection:
 * a headless context captures into an in-memory buffer with deterministic TTY semantics, so the
 * frame is read back explicitly rather than scraped off fd 1.
 *
 * The CI step `cmp`s this output against bindings/parity/parity_ansi.py and against the committed
 * golden (bindings/parity/golden_ansi_stats.txt, golden_ansi.bin), which the Rust job verifies
 * against the native API. See tests/ffi_ansi_parity.rs for the full contract.
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../../include/gibson.h"

static int die(const char *m) {
    fprintf(stderr, "parity_ansi_c: %s\n", m);
    return 1;
}

int main(int argc, char **argv) {
    if (argc < 3) return die("usage: parity_ansi_c STATS_OUT FRAME_OUT");

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
    OK(gibson_set_root_node(ctx, root)); /* takes ownership of the tree */
    OK(gibson_render(ctx));

    /* Read back the painted ANSI frame: size-query, allocate, copy. */
    size_t need = 0;
    OK(gibson_get_rendered_output(ctx, NULL, 0, &need));
    uint8_t *buf = (uint8_t *)malloc(need ? need : 1);
    if (!buf) return die("malloc");
    size_t got = 0;
    OK(gibson_get_rendered_output(ctx, buf, need, &got));

    gibson_stats_t s;
    memset(&s, 0, sizeof s);
    gibson_stats_init(&s);
    OK(gibson_get_stats(ctx, &s));
#undef OK

    FILE *fo = fopen(argv[2], "wb");
    if (!fo) { free(buf); return die("open frame out"); }
    if (fwrite(buf, 1, got, fo) != got) { fclose(fo); free(buf); return die("write frame"); }
    fclose(fo);
    free(buf);

    FILE *o = fopen(argv[1], "w");
    if (!o) return die("open stats out");
    /* Canonical order, matching tests/ffi_ansi_parity.rs and parity_ansi.py; the timing field
     * last_render_duration_micros is deliberately excluded. */
#define P(f) fprintf(o, #f "=%llu\n", (unsigned long long)s.f)
    P(frames);
    P(skipped_frames);
    P(dirty_cells);
    P(total_cells);
    P(frame_bytes);
    P(full_repaints);
    P(history_insertions);
    P(insertion_repaints);
    P(fast_insertions);
    P(insertion_bytes);
    P(anchor_resyncs);
    P(commit_bytes);
    P(control_bytes);
#undef P
    fclose(o);

    gibson_destroy_context(ctx);
    return 0;
}
