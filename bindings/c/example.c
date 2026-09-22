#include <stdio.h>
#include <stdlib.h>
#include "../../include/gibson.h"

int main(void) {
    printf("--- Running C FFI Example with LibGibson ---\n");

    gibson_context_t *ctx = NULL;
    gibson_status_t status = gibson_create_context(GIBSON_MODE_INLINE, &ctx);
    if (status != GIBSON_OK) {
        char err[256] = {0};
        gibson_last_error_message(err, sizeof(err));
        fprintf(stderr, "Failed to create context: %s\n", err);
        return 1;
    }

    // 1. Commit initial scrollback line
    gibson_commit(ctx, "[C FFI] LibGibson context initialized successfully.");

    // 2. Build declarative UI tree
    gibson_node_t *root = NULL;
    gibson_node_box_col(&root);
    gibson_node_set_width(root, 60.0f);
    gibson_node_set_gap(root, 1.0f);

    // Styled text child
    gibson_style_t text_style = {0};
    text_style.fg.color_type = GIBSON_COLOR_BRIGHT_GREEN;
    text_style.bold = 1;

    gibson_node_t *text_node = NULL;
    gibson_node_text("Hello from Native C via LibGibson ABI!", &text_style, GIBSON_WRAP_WORD, &text_node);
    gibson_node_add_child(root, text_node);

    // Border box child
    gibson_style_t border_style = {0};
    border_style.fg.color_type = GIBSON_COLOR_CYAN;

    gibson_node_t *box = NULL;
    gibson_node_border_box(GIBSON_BORDER_ROUNDED, &border_style, &box);
    gibson_node_set_width(box, 50.0f);
    gibson_node_set_height(box, 3.0f);

    gibson_style_t inner_style = {0};
    inner_style.fg.color_type = GIBSON_COLOR_WHITE;
    gibson_node_t *inner_text = NULL;
    gibson_node_text("Inside C-allocated rounded border box", &inner_style, GIBSON_WRAP_NONE, &inner_text);
    gibson_node_add_child(box, inner_text);

    gibson_node_add_child(root, box);

    // 3. Set root node and render
    gibson_set_root_node(ctx, root);
    status = gibson_render(ctx);
    if (status != GIBSON_OK) {
        fprintf(stderr, "Render failed with status: %d\n", status);
    }

    // 4. Commit a completion line
    gibson_commit(ctx, "[C FFI] Render executed and output committed to scrollback.");

    // 5. Query stats
    gibson_stats_t stats = {0};
    gibson_get_stats(ctx, &stats);
    printf("[C FFI] Stats: frames rendered = %llu, total bytes emitted = %llu\n",
           (unsigned long long)stats.frames,
           (unsigned long long)stats.bytes_emitted);

    // 6. Cleanup
    gibson_destroy_context(ctx);
    printf("[C FFI] Test completed successfully.\n");
    return 0;
}
