#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../../include/gibson.h"

int main(void) {
    printf("--- Running C FFI Example with LibGibson ---\n");
    printf("[C FFI] ABI version = %u\n", gibson_abi_version());

    gibson_context_t *ctx = NULL;
    gibson_status_t status = gibson_create_context(GIBSON_MODE_INLINE, &ctx);
    if (status != GIBSON_OK) {
        char err[256] = {0};
        gibson_last_error_message(err, sizeof(err));
        fprintf(stderr, "Failed to create context: %s\n", err);
        return 1;
    }

    // 1. Commit initial scrollback line
    gibson_commit_text(ctx, "[C FFI] LibGibson context initialized successfully.");

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
    gibson_node_text("Zero-flicker native terminal rendering with C ABI bindings.",
                     &text_style, GIBSON_WRAP_WORD, &text_node);
    gibson_node_add_child(rail_node, text_node);

    gibson_node_add_child(root, rail_node);

    // 3. Language-neutral structured rich text (multiple spans per line)
    {
        gibson_style_t label_style = {0};
        label_style.fg.color_type = GIBSON_COLOR_BRIGHT_YELLOW;
        label_style.dim = 1;

        gibson_style_t value_style = {0};
        value_style.fg.color_type = GIBSON_COLOR_WHITE;
        value_style.bold = 1;

        gibson_line_t *line = NULL;
        gibson_line_new(&line);
        gibson_line_add_span(line, "RichText spans: ", &label_style);
        gibson_line_add_span(line, "one line, many styles", &value_style);
        gibson_line_set_align(line, GIBSON_ALIGN_LEFT);

        gibson_rich_text_t *rich = NULL;
        gibson_rich_text_new(&rich);
        gibson_rich_text_add_line(rich, line);
        gibson_line_free(line);

        gibson_node_t *rich_node = NULL;
        gibson_node_rich_text(rich, GIBSON_WRAP_WORD, &rich_node);
        gibson_node_add_child(rail_node, rich_node);

        // Commit the same structured rich text directly to scrollback.
        gibson_commit_rich_text(ctx, rich);
        gibson_rich_text_free(rich);
    }

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

    // 4. Set root node and render
    gibson_set_root_node(ctx, root);
    status = gibson_render(ctx);
    if (status != GIBSON_OK) {
        fprintf(stderr, "Render failed with status: %d\n", status);
    }

    // 5. Safe insertion: controls in the text are neutralized by the engine.
    gibson_insert_text_before_live(ctx, "[C FFI] Notice: Live background event inserted above active region.");

    // 5b. Raw escape hatch: explicitly unchecked, text is a terminal byte
    //     stream (not sanitized).
    gibson_insert_raw_lines_before_live_unchecked(ctx, "[C FFI] Raw notice (unchecked path).");

    // 6. Commit completion line to finalize
    gibson_commit_text(ctx, "[C FFI] Render executed and output committed to scrollback.");

    // 7. Query stats using the versioned, overflow-safe struct
    gibson_stats_t stats;
    memset(&stats, 0, sizeof(stats));
    gibson_stats_init(&stats);
    status = gibson_get_stats(ctx, &stats);
    if (status != GIBSON_OK) {
        fprintf(stderr, "get_stats failed: %d\n", status);
        gibson_destroy_context(ctx);
        return 1;
    }
    printf("[C FFI] Stats: abi=%u frames=%llu frame_bytes=%llu anchor_resyncs=%llu\n",
           stats.abi_version,
           (unsigned long long)stats.frames,
           (unsigned long long)stats.frame_bytes,
           (unsigned long long)stats.anchor_resyncs);

    // 8. Hostile input must be rejected, not crash.
    gibson_context_t *bad = NULL;
    if (gibson_create_context(999, &bad) != GIBSON_ERR_INVALID_PARAM) {
        fprintf(stderr, "Expected invalid render mode to be rejected!\n");
        return 1;
    }

    // 9. Cleanup
    gibson_destroy_context(ctx);
    printf("[C FFI] Test completed successfully.\n");
    return 0;
}
