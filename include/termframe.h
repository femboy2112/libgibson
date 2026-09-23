#ifndef TERMFRAME_H
#define TERMFRAME_H

/**
 * Termframe compatibility header.
 * Aliases the LibGibson core C ABI to termframe_* / tf_* naming.
 */

#include "gibson.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef gibson_status_t      tf_status_t;
typedef gibson_status_t      tf_status;
typedef gibson_context_t     tf_context;
typedef gibson_node_t        tf_node;
typedef gibson_event_t       tf_event;
typedef gibson_stats_t       tf_stats;
typedef gibson_style_t       tf_style;
typedef gibson_color_t       tf_color;

#define tf_create            gibson_create_context
#define tf_destroy           gibson_destroy_context
#define tf_render            gibson_render
#define tf_render_if_due     gibson_render_if_due
#define tf_request_render    gibson_request_render
#define tf_commit            gibson_commit
#define tf_insert_before_live gibson_insert_raw_lines_before_live_unchecked
#define tf_poll_event        gibson_poll_event

#define tf_box_col           gibson_node_box_col
#define tf_box_row           gibson_node_box_row
#define tf_text              gibson_node_text
#define tf_spinner           gibson_node_spinner
#define tf_text_input        gibson_node_text_input
#define tf_border_box        gibson_node_border_box
#define tf_rule              gibson_node_rule
#define tf_rail              gibson_node_rail
#define tf_add_child         gibson_node_add_child
#define tf_node_free         gibson_node_free

#ifdef __cplusplus
}
#endif

#endif /* TERMFRAME_H */
