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

    // 2. Build declarative UI tree using chrome primitives (rule, rail)
    gibson_node_t *root = NULL;
    gibson_node_box_col(&root);
    gibson_node_set_percent_width(root, 100.0f);
    gibson_node_set_max_width(root, 72.0f);
    gibson_node_set_gap(root, 1.0f);

    // Rule header with title
    gibson_style_t rule_style = {0};
    rule_style.fg.color_type = GIBSON_COLOR_BRIGHT_BLUE;
    rule_style.bold = 1;

    gibson_node_t *rule_node = NULL;
    gibson_node_rule("LibGibson Native C ABI Demo", &rule_style, &rule_node);
    gibson_node_add_child(root, rule_node);

    // Rail callout with structured text
    gibson_style_t rail_style = {0};
    rail_style.fg.color_type = GIBSON_COLOR_CYAN;

    gibson_node_t *rail_node = NULL;
    gibson_node_rail(&rail_style, &rail_node);

    gibson_style_t text_style = {0};
    text_style.fg.color_type = GIBSON_COLOR_BRIGHT_GREEN;

    gibson_node_t *text_node = NULL;
    gibson_node_text("Zero-flicker native terminal rendering with C ABI bindings.", &text_style, GIBSON_WRAP_WORD, &text_node);
    gibson_node_add_child(rail_node, text_node);

    gibson_node_add_child(root, rail_node);

    // Border box child
    gibson_style_t border_style = {0};
    border_style.fg.color_type = GIBSON_COLOR_BRIGHT_YELLOW;

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

    // 4. Test insert_before_live: insert asynchronous notice into scrollback above active region
    gibson_insert_before_live(ctx, "[C FFI] Notice: Live background event inserted above active region.");

    // 5. Commit completion line to finalize
    gibson_commit(ctx, "[C FFI] Render executed and output committed to scrollback.");

    // 6. Query stats
    gibson_stats_t stats = {0};
    gibson_get_stats(ctx, &stats);
    printf("[C FFI] Stats: frames rendered = %llu, total bytes emitted = %llu\n",
           (unsigned long long)stats.frames,
           (unsigned long long)stats.bytes_emitted);

    // 7. Cleanup
    gibson_destroy_context(ctx);
    printf("[C FFI] Test completed successfully.\n");
    return 0;
}
