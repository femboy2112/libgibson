/*
 * FFI behavioral parity harness (C side) — #142, v0.6 Gate H.
 *
 * Drives the canonical non-TTY plain-scrollback op-script through the C ABI, reads
 * gibson_get_stats, and writes a canonical stats report to argv[1]. fd 1 is redirected to
 * argv[2] BEFORE the context is created, so (a) the engine runs its non-TTY plain path (the
 * same path the Rust golden pins with session.is_tty=false) and (b) the committed byte stream
 * is captured as the wire output. This file is written to be `cmp`'d, so it prints NOTHING to
 * stdout once fd 1 is redirected — all diagnostics go to stderr.
 *
 * The CI step then `cmp`s this output against bindings/parity/parity.py's output and against
 * the committed golden (bindings/parity/golden_{stats,wire}.txt), which the Rust job verifies
 * against the native API. See tests/ffi_stats_parity.rs for the full contract.
 */
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include "../../include/gibson.h"

static int die(const char *m) {
    fprintf(stderr, "parity_c: %s\n", m);
    return 1;
}

int main(int argc, char **argv) {
    if (argc < 3) return die("usage: parity_c STATS_OUT WIRE_OUT");

    /* Redirect fd 1 to the wire file BEFORE creating the context: forces the non-TTY plain
     * path (stdout is no longer a terminal) and captures the committed byte stream. */
    int fd = open(argv[2], O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (fd < 0 || dup2(fd, 1) < 0) return die("redirect fd1");
    close(fd);

    gibson_context_t *ctx = NULL;
    if (gibson_create_context(GIBSON_MODE_INLINE, &ctx) != GIBSON_OK) return die("create context");

#define OK(x) do { if ((x) != GIBSON_OK) return die(#x); } while (0)
    OK(gibson_commit_text(ctx, "alpha"));
    OK(gibson_commit_text(ctx, "beta gamma"));
    OK(gibson_insert_text_before_live(ctx, "delta"));

    gibson_node_t *root = NULL, *live = NULL;
    gibson_style_t text_style;
    memset(&text_style, 0, sizeof text_style);
    OK(gibson_node_box_col(&root));
    OK(gibson_node_text("live", &text_style, GIBSON_WRAP_NONE, &live));
    OK(gibson_node_add_child(root, live));
    OK(gibson_set_root_node(ctx, root)); /* takes ownership of the tree */
    OK(gibson_render(ctx));
    OK(gibson_commit_text(ctx, "omega"));

    gibson_stats_t s;
    memset(&s, 0, sizeof s);
    gibson_stats_init(&s);
    OK(gibson_get_stats(ctx, &s));
#undef OK

    FILE *o = fopen(argv[1], "w");
    if (!o) return die("open stats out");
    /* Canonical order, matching tests/ffi_stats_parity.rs and parity.py; the timing field
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
