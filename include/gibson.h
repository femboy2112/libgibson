#ifndef GIBSON_H
#define GIBSON_H

/**
 * LibGibson / Termframe
 *
 * A language-neutral terminal UI engine with a real cell framebuffer,
 * differential renderer, and immutable-scrollback / mutable-live-region semantics.
 */

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Status codes */
typedef enum {
    GIBSON_OK = 0,
    GIBSON_ERR_INVALID_PARAM = -1,
    GIBSON_ERR_IO = -2,
    GIBSON_ERR_PANIC = -3,
    GIBSON_ERR_NOT_INITIALIZED = -4
} gibson_status_t;

/* Render modes */
typedef enum {
    GIBSON_MODE_INLINE = 0,
    GIBSON_MODE_FULLSCREEN = 1
} gibson_render_mode_t;

/* Colors */
typedef enum {
    GIBSON_COLOR_RESET = 0,
    GIBSON_COLOR_BLACK,
    GIBSON_COLOR_RED,
    GIBSON_COLOR_GREEN,
    GIBSON_COLOR_YELLOW,
    GIBSON_COLOR_BLUE,
    GIBSON_COLOR_MAGENTA,
    GIBSON_COLOR_CYAN,
    GIBSON_COLOR_WHITE,
    GIBSON_COLOR_BRIGHT_BLACK,
    GIBSON_COLOR_BRIGHT_RED,
    GIBSON_COLOR_BRIGHT_GREEN,
    GIBSON_COLOR_BRIGHT_YELLOW,
    GIBSON_COLOR_BRIGHT_BLUE,
    GIBSON_COLOR_BRIGHT_MAGENTA,
    GIBSON_COLOR_BRIGHT_CYAN,
    GIBSON_COLOR_BRIGHT_WHITE,
    GIBSON_COLOR_ANSI256,
    GIBSON_COLOR_RGB
} gibson_color_type_t;

typedef struct {
    int32_t color_type; /* gibson_color_type_t */
    uint8_t r;          /* Also used as ANSI 256 code */
    uint8_t g;
    uint8_t b;
    uint8_t _pad;
} gibson_color_t;

/* Text styles */
typedef struct {
    gibson_color_t fg;
    gibson_color_t bg;
    uint8_t bold;
    uint8_t dim;
    uint8_t italic;
    uint8_t underline;
    uint8_t reverse;
    uint8_t _reserved[3];
} gibson_style_t;

/* Border styles */
typedef enum {
    GIBSON_BORDER_SINGLE = 0,
    GIBSON_BORDER_DOUBLE = 1,
    GIBSON_BORDER_ROUNDED = 2,
    GIBSON_BORDER_THICK = 3,
    GIBSON_BORDER_ASCII = 4
} gibson_border_type_t;

/* Text wrap modes */
typedef enum {
    GIBSON_WRAP_NONE = 0,
    GIBSON_WRAP_CHAR = 1,
    GIBSON_WRAP_WORD = 2
} gibson_wrap_mode_t;

/* Special Key Constants */
#define GIBSON_KEY_ENTER     0x1001
#define GIBSON_KEY_LEFT      0x1002
#define GIBSON_KEY_RIGHT     0x1003
#define GIBSON_KEY_UP        0x1004
#define GIBSON_KEY_DOWN      0x1005
#define GIBSON_KEY_HOME      0x1006
#define GIBSON_KEY_END       0x1007
#define GIBSON_KEY_BACKSPACE 0x1008
#define GIBSON_KEY_DELETE    0x1009
#define GIBSON_KEY_ESC       0x100A
#define GIBSON_KEY_TAB       0x100B

/* Key Modifiers */
#define GIBSON_MOD_SHIFT   0x01
#define GIBSON_MOD_CONTROL 0x02
#define GIBSON_MOD_ALT     0x04

/* Event Types */
typedef enum {
    GIBSON_EVENT_NONE = 0,
    GIBSON_EVENT_KEY = 1,
    GIBSON_EVENT_PASTE = 2,
    GIBSON_EVENT_RESIZE = 3
} gibson_event_type_t;

typedef struct {
    int32_t event_type;       /* gibson_event_type_t */
    uint32_t key_code;        /* Unicode codepoint or GIBSON_KEY_* */
    uint8_t modifiers;        /* GIBSON_MOD_* */
    uint8_t _pad[3];
    uint16_t cols;            /* Resize columns */
    uint16_t rows;            /* Resize rows */
    const char *paste_str;    /* Pointer to pasted string (valid until next event poll) */
} gibson_event_t;

/* Statistics */
typedef struct {
    uint64_t frames;
    uint64_t skipped_frames;
    uint64_t dirty_cells;
    uint64_t total_cells;
    uint64_t bytes_emitted;
    uint64_t full_repaints;
    uint64_t last_render_duration_micros;
} gibson_stats_t;

/* Opaque handles */
typedef struct GibsonContextOpaque gibson_context_t;
typedef struct GibsonNodeOpaque gibson_node_t;

/* Context Lifecycle */
gibson_status_t gibson_create_context(int32_t mode, gibson_context_t **out);
void            gibson_destroy_context(gibson_context_t *ctx);
gibson_status_t gibson_set_sync_updates(gibson_context_t *ctx, int32_t enabled);
gibson_status_t gibson_set_max_fps(gibson_context_t *ctx, uint32_t fps);

/* UI & Rendering */
gibson_status_t gibson_set_root_node(gibson_context_t *ctx, gibson_node_t *node);
gibson_status_t gibson_render(gibson_context_t *ctx);
gibson_status_t gibson_commit(gibson_context_t *ctx, const char *utf8_text);

/* Input */
gibson_status_t gibson_poll_event(gibson_context_t *ctx, uint32_t timeout_ms, gibson_event_t *out_event);

/* Metrics & Diagnostics */
gibson_status_t gibson_get_stats(gibson_context_t *ctx, gibson_stats_t *out_stats);
int32_t         gibson_last_error_message(char *buf, size_t buf_len);

/* Node Construction */
gibson_status_t gibson_node_box_col(gibson_node_t **out);
gibson_status_t gibson_node_box_row(gibson_node_t **out);
gibson_status_t gibson_node_text(const char *utf8_text, const gibson_style_t *style, int32_t wrap_mode, gibson_node_t **out);
gibson_status_t gibson_node_spinner(uint32_t frame_index, const gibson_style_t *style, const char *label, gibson_node_t **out);
gibson_status_t gibson_node_text_input(const char *value, uint32_t cursor_grapheme, const char *placeholder, const gibson_style_t *style, gibson_node_t **out);
gibson_status_t gibson_node_border_box(int32_t border_type, const gibson_style_t *style, gibson_node_t **out);
gibson_status_t gibson_node_add_child(gibson_node_t *parent, gibson_node_t *child);

/* Node Layout Styling */
gibson_status_t gibson_node_set_width(gibson_node_t *node, float width);
gibson_status_t gibson_node_set_height(gibson_node_t *node, float height);
gibson_status_t gibson_node_set_flex_grow(gibson_node_t *node, float grow);
gibson_status_t gibson_node_set_gap(gibson_node_t *node, float gap);
gibson_status_t gibson_node_set_padding(gibson_node_t *node, float padding);
void            gibson_node_free(gibson_node_t *node);

#ifdef __cplusplus
}
#endif

#endif /* GIBSON_H */
