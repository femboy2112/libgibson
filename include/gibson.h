#ifndef GIBSON_H
#define GIBSON_H

/**
 * LibGibson / Termframe
 *
 * A language-neutral terminal UI engine with a real cell framebuffer,
 * differential renderer, and immutable-scrollback / mutable-live-region semantics.
 *
 * ## ABI rules
 *
 *  - No internal struct is exposed directly; `gibson_stats_t` is a dedicated,
 *    versioned, `#[repr(C)]` layout.
 *  - Enum-like inputs cross the boundary as `int32_t` and are validated.
 *  - Callers initialize `gibson_stats_t` with `gibson_stats_init()` (or set
 *    `struct_size = sizeof(gibson_stats_t)` and `abi_version = GIBSON_ABI_VERSION`).
 */

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** Runtime ABI version. Bump on any breaking change. */
#define GIBSON_ABI_VERSION 1u

/* Status codes */
typedef enum {
    GIBSON_OK = 0,
    GIBSON_ERR_INVALID_PARAM = -1,
    GIBSON_ERR_IO = -2,
    GIBSON_ERR_PANIC = -3,
    GIBSON_ERR_NOT_INITIALIZED = -4,
    GIBSON_ERR_QUEUE_FULL = -5     /* gibson_push_event / gibson_push_event_kind only (ABI v1, additive) */
} gibson_status_t;

/* Render modes (pass as int32_t) */
typedef enum {
    GIBSON_MODE_INLINE = 0,
    GIBSON_MODE_FULLSCREEN = 1
} gibson_render_mode_t;

/* Color depth (pass to gibson_set_color_depth as int32_t).
   Downgrade ladder: TRUECOLOR -> ANSI256 -> ANSI16 -> MONO. */
typedef enum {
    GIBSON_COLOR_DEPTH_MONO = 0,
    GIBSON_COLOR_DEPTH_ANSI16 = 1,
    GIBSON_COLOR_DEPTH_ANSI256 = 2,
    GIBSON_COLOR_DEPTH_TRUECOLOR = 3
} gibson_color_depth_t;

/* Colors (pass as int32_t) */
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

/* Max UTF-8 bytes of one cell's grapheme cluster carried inline by gibson_cell_t.glyph. Most
   graphemes fit, but some do NOT (a ZWJ emoji sequence, e.g. a 4-person family, is 25 bytes; a
   long combining "Zalgo" stack is more): a longer cluster is truncated at a codepoint boundary,
   the glyph_truncated flag (_reserved[0]) is set, and the full bytes are available from
   gibson_get_frame_cell_glyph. */
#define GIBSON_CELL_GLYPH_CAP 16

/* A resolved frame cell (see gibson_get_frame_cell). glyph holds the grapheme's UTF-8 bytes
   (NOT NUL-terminated); glyph_len is how many are valid. style is quantized to the context's
   color depth, so it matches the emitted ANSI wire. */
typedef struct {
    uint8_t glyph[GIBSON_CELL_GLYPH_CAP];
    uint8_t glyph_len;
    uint8_t width;          /* display columns: 0 (continuation/empty), 1, or 2 */
    uint8_t is_continuation;
    uint8_t transparent;
    uint8_t _reserved[4];   /* [0] = glyph_truncated: 1 iff glyph was cut at the cap
                               (fetch full bytes via gibson_get_frame_cell_glyph); [1..3]=0 */
    gibson_style_t style;
} gibson_cell_t;

/* Border styles (pass as int32_t) */
typedef enum {
    GIBSON_BORDER_SINGLE = 0,
    GIBSON_BORDER_DOUBLE = 1,
    GIBSON_BORDER_ROUNDED = 2,
    GIBSON_BORDER_THICK = 3,
    GIBSON_BORDER_ASCII = 4
} gibson_border_type_t;

/* Text wrap modes (pass as int32_t) */
typedef enum {
    GIBSON_WRAP_NONE = 0,
    GIBSON_WRAP_CHAR = 1,
    GIBSON_WRAP_WORD = 2
} gibson_wrap_mode_t;

/* Text alignment (pass as int32_t) */
typedef enum {
    GIBSON_ALIGN_LEFT = 0,
    GIBSON_ALIGN_CENTER = 1,
    GIBSON_ALIGN_RIGHT = 2
} gibson_align_t;

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
#define GIBSON_KEY_PAGE_UP   0x100C
#define GIBSON_KEY_PAGE_DOWN 0x100D

/* Key kinds (gibson_poll_event_kind / gibson_push_event_kind; ABI v1, additive). GIBSON_KEY_*
   occupy 0x1001-0x100D, which are also Unicode scalar values (U+1001-U+100D, Myanmar letters);
   key_code alone cannot tell them apart, the kind can. */
#define GIBSON_KEY_KIND_NONE    0u /* not a key event */
#define GIBSON_KEY_KIND_CHAR    1u /* key_code is a typed Unicode scalar value */
#define GIBSON_KEY_KIND_SPECIAL 2u /* key_code is a GIBSON_KEY_* constant */

/* Key Modifiers */
#define GIBSON_MOD_SHIFT   0x01
#define GIBSON_MOD_CONTROL 0x02
#define GIBSON_MOD_ALT     0x04

/* Event Types */
typedef enum {
    GIBSON_EVENT_NONE = 0,
    GIBSON_EVENT_KEY = 1,
    GIBSON_EVENT_PASTE = 2,
    GIBSON_EVENT_RESIZE = 3,
    /* Key-release reporting (ABI v1, additive, #275): delivered only after
       gibson_set_key_release_reporting. KEY_RELEASE: a key went up; key_code, modifiers and the
       key kind are exactly those its press was delivered with. KEY_RELEASES_ACTIVE: the terminal
       confirmed the request; every key that goes up after this event is a KEY_RELEASE.
       KEY_RELEASES_UNSUPPORTED: it answered without confirming; keys stay press-only. */
    GIBSON_EVENT_KEY_RELEASE = 4,
    GIBSON_EVENT_KEY_RELEASES_ACTIVE = 5,
    GIBSON_EVENT_KEY_RELEASES_UNSUPPORTED = 6
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

/**
 * Versioned statistics snapshot.
 *
 * Initialize with gibson_stats_init() before calling gibson_get_stats().
 *
 * ABI contract (exact-version): `abi_version` must equal GIBSON_ABI_VERSION,
 * and `struct_size` must be at least `sizeof(gibson_stats_t)`. The engine writes
 * exactly `min(struct_size, sizeof(gibson_stats_t))` bytes. A caller with a
 * larger buffer is safe (trailing bytes untouched); a caller built against an
 * older, smaller struct is rejected with GIBSON_ERR_INVALID_PARAM rather than
 * truncated. GIBSON_ABI_VERSION is bumped whenever this layout changes.
 */
typedef struct {
    uint32_t struct_size;   /* caller sets to sizeof(gibson_stats_t) */
    uint32_t abi_version;   /* engine validates against GIBSON_ABI_VERSION */
    uint64_t frames;
    uint64_t skipped_frames;
    uint64_t dirty_cells;
    uint64_t total_cells;
    uint64_t frame_bytes;   /* live frame bytes (control sequences included) */
    uint64_t full_repaints;
    uint64_t last_render_duration_micros;
    uint64_t history_insertions;
    uint64_t insertion_repaints; /* insertions that repainted the live framebuffer */
    uint64_t fast_insertions;    /* insertions that used the InsertLineFastPath */
    uint64_t insertion_bytes;
    uint64_t anchor_resyncs;     /* physical anchor invalidations/re-establishments */
    uint64_t commit_bytes;
    uint64_t control_bytes;
} gibson_stats_t;

/* Opaque handles */
typedef struct GibsonContextOpaque gibson_context_t;
typedef struct GibsonNodeOpaque gibson_node_t;
typedef struct GibsonLineOpaque gibson_line_t;
typedef struct GibsonRichTextOpaque gibson_rich_text_t;

/* ABI */
uint32_t gibson_abi_version(void);
void     gibson_stats_init(gibson_stats_t *out_stats);

/* Context Lifecycle */
gibson_status_t gibson_create_context(int32_t mode, gibson_context_t **out);
/* Headless: renders into an in-memory buffer with deterministic, TTY-independent semantics
   (fixed cols x rows, truecolor, no terminal lease). Read the ANSI frame via
   gibson_get_rendered_output. cols and rows must be non-zero. (ABI v1, additive) */
gibson_status_t gibson_create_headless_context(int32_t mode, uint16_t cols, uint16_t rows, gibson_context_t **out);
void            gibson_destroy_context(gibson_context_t *ctx);
/* Synchronized-update framing (CSI ?2026 h/l) around each frame on a TTY; on by default. A request
   that the terminal present each update as a unit -- NOT atomic I/O: a failed write can leave a
   partial frame, which the renderer re-establishes on the next frame. */
gibson_status_t gibson_set_sync_updates(gibson_context_t *ctx, int32_t enabled);
gibson_status_t gibson_set_max_fps(gibson_context_t *ctx, uint32_t fps);
/* Asks the terminal to report key releases through the kitty keyboard protocol (enabled != 0),
   or stops asking (0). Off by default. On a terminal context this enters raw mode, pushes the
   protocol's keyboard flags on the current screen and asks whether the terminal took them; the
   answer is polled as GIBSON_EVENT_KEY_RELEASES_ACTIVE or GIBSON_EVENT_KEY_RELEASES_UNSUPPORTED, in
   input order, and once active each key that goes up is polled as GIBSON_EVENT_KEY_RELEASE. A
   terminal without the protocol may answer UNSUPPORTED or nothing: keys then stay press-only.
   gibson_destroy_context (and the panic hook) pop the flags. Tested in kitty 0.32.2. While enabled,
   modifier keys alone are reported by the terminal but not delivered, and text typed through an
   input method may not arrive: this is for direct key control, not text entry. A no-op on a
   headless context (push the events instead) and off Unix. GIBSON_ERR_IO if the write fails.
   (ABI v1, additive, #275) */
gibson_status_t gibson_set_key_release_reporting(gibson_context_t *ctx, int32_t enabled);
/* Overrides the color depth (one of gibson_color_depth_t); unknown values return
   GIBSON_ERR_INVALID_PARAM. Call before the first render (see note at gibson_get_rendered_output).
   (ABI v1, additive) */
gibson_status_t gibson_set_color_depth(gibson_context_t *ctx, int32_t depth);

/* UI & Rendering */
gibson_status_t gibson_set_root_node(gibson_context_t *ctx, gibson_node_t *node);
gibson_status_t gibson_render(gibson_context_t *ctx);
gibson_status_t gibson_render_if_due(gibson_context_t *ctx, int32_t *out_rendered);
gibson_status_t gibson_request_render(gibson_context_t *ctx);
gibson_status_t gibson_commit_text(gibson_context_t *ctx, const char *utf8_text);
gibson_status_t gibson_commit(gibson_context_t *ctx, const char *utf8_text); /* alias of commit_text */
gibson_status_t gibson_commit_raw_ansi_unchecked(gibson_context_t *ctx, const char *utf8_ansi);
/* Safe: width-aware plain text; terminal controls neutralized. */
gibson_status_t gibson_insert_text_before_live(gibson_context_t *ctx, const char *utf8_text);
/* Raw escape hatch: text is a terminal byte stream, NOT sanitized. */
gibson_status_t gibson_insert_raw_lines_before_live_unchecked(gibson_context_t *ctx, const char *utf8_text);
gibson_status_t gibson_commit_node(gibson_context_t *ctx, gibson_node_t *node);
gibson_status_t gibson_insert_node_before_live(gibson_context_t *ctx, gibson_node_t *node);
gibson_status_t gibson_commit_rich_text(gibson_context_t *ctx, const gibson_rich_text_t *rich);
gibson_status_t gibson_insert_rich_text_before_live(gibson_context_t *ctx, const gibson_rich_text_t *rich);
gibson_status_t gibson_clear_live_region(gibson_context_t *ctx);

/* Input */
/* On a terminal context, reads the terminal (waiting up to timeout_ms). On a HEADLESS context,
   delivers the events queued by gibson_push_event, oldest first, each exactly once, without
   waiting; GIBSON_EVENT_NONE when the queue is empty. A delivered RESIZE also sets the headless
   terminal size and schedules a render, as a real resize does: a FULLSCREEN frame is then
   cols x rows; an INLINE frame is cols wide and, as on a terminal, as tall as its content, at
   least 1 and at most max(rows - 1, 1) rows. Every field of out_event is written, so a polled event can be pushed back as is. */
gibson_status_t gibson_poll_event(gibson_context_t *ctx, uint32_t timeout_ms, gibson_event_t *out_event);

/* Like gibson_poll_event, and also writes *out_key_kind (non-NULL): GIBSON_KEY_KIND_CHAR or
   GIBSON_KEY_KIND_SPECIAL for a KEY or KEY_RELEASE event, GIBSON_KEY_KIND_NONE otherwise. *out_event is written
   exactly as gibson_poll_event writes it. Use this to tell a typed U+1001-U+100D from the
   GIBSON_KEY_* constant with the same value, which gibson_poll_event reports identically.
   (ABI v1, additive) */
gibson_status_t gibson_poll_event_kind(gibson_context_t *ctx, uint32_t timeout_ms,
                                       gibson_event_t *out_event, uint32_t *out_key_kind);

/* Most undelivered events one headless context holds (see gibson_push_event). */
#define GIBSON_EVENT_QUEUE_CAPACITY 1024

/* Queues an input event on a HEADLESS context, for gibson_poll_event to deliver: the encoding is
   exactly what gibson_poll_event writes, so a pushed event polls back unchanged.
     KEY:    key_code is a GIBSON_KEY_* constant or a Unicode scalar value; modifiers uses only
             GIBSON_MOD_* bits. Because GIBSON_KEY_* occupy 0x1001-0x100D, those codepoints are
             always the special keys here; gibson_push_event_kind can push them as characters.
     PASTE:  paste_str is a non-NULL, NUL-terminated UTF-8 string; it is copied.
     RESIZE: cols and rows are both non-zero.
     KEY_RELEASE: as KEY.  KEY_RELEASES_ACTIVE, KEY_RELEASES_UNSUPPORTED: no fields.
   Fields the event type does not use are ignored and may be left uninitialized. Returns GIBSON_ERR_INVALID_PARAM for a NULL
   argument, a terminal (non-headless) context or an unrepresentable event, and
   GIBSON_ERR_QUEUE_FULL when GIBSON_EVENT_QUEUE_CAPACITY events are undelivered. A refused event
   is not queued, and nothing already queued is displaced. (ABI v1, additive) */
gibson_status_t gibson_push_event(gibson_context_t *ctx, const gibson_event_t *event);

/* Like gibson_push_event, with key_kind saying how to read a KEY or KEY_RELEASE event's key_code:
   GIBSON_KEY_KIND_CHAR - any Unicode scalar value, typed (U+1001-U+100D included);
   GIBSON_KEY_KIND_SPECIAL - must be a GIBSON_KEY_* constant.
   Any other key_kind, or a key_code that does not fit it, is GIBSON_ERR_INVALID_PARAM. key_kind is
   ignored for other event types. gibson_poll_event_kind reports the same kind back.
   (ABI v1, additive) */
gibson_status_t gibson_push_event_kind(gibson_context_t *ctx, const gibson_event_t *event,
                                       uint32_t key_kind);

/* Metrics & Diagnostics */
gibson_status_t gibson_get_stats(gibson_context_t *ctx, gibson_stats_t *out_stats);
int32_t         gibson_last_error_message(char *buf, size_t buf_len);
/* Copies the headless ANSI frame stream into buf (binary-safe, NOT NUL-terminated).
   out_len always receives the available size; pass buf=NULL or cap=0 to size-query first. */
gibson_status_t gibson_get_rendered_output(gibson_context_t *ctx, uint8_t *buf, size_t cap, size_t *out_len);
/* Resolved styled cell grid of the most recently rendered frame. frame_size writes (0,0) if there
   is no frame yet; frame_cell returns GIBSON_ERR_INVALID_PARAM for a null out, no frame, or an
   out-of-bounds (x,y). Style is quantized to the context's color depth. (ABI v1, additive) */
gibson_status_t gibson_get_frame_size(gibson_context_t *ctx, uint16_t *out_cols, uint16_t *out_rows);
gibson_status_t gibson_get_frame_cell(gibson_context_t *ctx, uint16_t x, uint16_t y, gibson_cell_t *out);
/* Bulk readback: up to max_cells resolved cells row-major from (x,y) across row ends to the end of
   the frame; *out_written = count. Same per-cell encoding as gibson_get_frame_cell. (ABI v1, additive) */
gibson_status_t gibson_get_frame_cells(gibson_context_t *ctx, uint16_t x, uint16_t y,
                                       gibson_cell_t *out, size_t max_cells, size_t *out_written);
/* The cell's full (untruncated) grapheme-cluster UTF-8 bytes into buf. *out_len always = the full
   length; bytes are copied only if buf != NULL && cap >= *out_len (so *out_len > cap => not
   written). The escape hatch for glyph_truncated cells. (ABI v1, additive) */
gibson_status_t gibson_get_frame_cell_glyph(gibson_context_t *ctx, uint16_t x, uint16_t y,
                                            uint8_t *buf, size_t cap, size_t *out_len);

/* Node Construction */
gibson_status_t gibson_node_box_col(gibson_node_t **out);
gibson_status_t gibson_node_box_row(gibson_node_t **out);
/* Overlay container: children are absolutely positioned and composited in order. */
gibson_status_t gibson_node_stack(gibson_node_t **out);
/* Dim veil (style-only overlay when composited in a stack). */
gibson_status_t gibson_node_dim(gibson_node_t **out);
gibson_status_t gibson_node_text(const char *utf8_text, const gibson_style_t *style, int32_t wrap_mode, gibson_node_t **out);
gibson_status_t gibson_node_spinner(uint32_t frame_index, const gibson_style_t *style, const char *label, gibson_node_t **out);
/* Single-line text input. `value` may be arbitrary UTF-8: a grapheme that holds a control
   character (C0, DEL, C1 -- e.g. the ESC of a pasted escape sequence) or a line/paragraph
   separator, or that has zero display width standing alone, is painted as ONE U+FFFD cell, and a
   grapheme wider than two cells is laid codepoint by codepoint, so no control from content reaches
   the terminal and the cursor column matches the painted cells (issue #252). To store pasted text
   the way the Rust TextInputState does, flatten line breaks and TAB to spaces and drop other
   controls before passing it here. */
gibson_status_t gibson_node_text_input(const char *value, uint32_t cursor_grapheme, const char *placeholder, const gibson_style_t *style, gibson_node_t **out);
gibson_status_t gibson_node_border_box(int32_t border_type, const gibson_style_t *style, gibson_node_t **out);
gibson_status_t gibson_node_rule(const char *title, const gibson_style_t *style, gibson_node_t **out);
gibson_status_t gibson_node_rail(const gibson_style_t *style, gibson_node_t **out);
gibson_status_t gibson_node_rich_text(const gibson_rich_text_t *rich, int32_t wrap_mode, gibson_node_t **out);
gibson_status_t gibson_node_add_child(gibson_node_t *parent, gibson_node_t *child);

/* Language-neutral structured rich text */
gibson_status_t gibson_line_new(gibson_line_t **out);
gibson_status_t gibson_line_add_span(gibson_line_t *line, const char *utf8_text, const gibson_style_t *style);
gibson_status_t gibson_line_set_align(gibson_line_t *line, int32_t align);
void            gibson_line_free(gibson_line_t *line);
gibson_status_t gibson_rich_text_new(gibson_rich_text_t **out);
gibson_status_t gibson_rich_text_add_line(gibson_rich_text_t *rich, const gibson_line_t *line);
void            gibson_rich_text_free(gibson_rich_text_t *rich);

/* Node Layout Styling */
gibson_status_t gibson_node_set_width(gibson_node_t *node, float width);
gibson_status_t gibson_node_set_height(gibson_node_t *node, float height);
gibson_status_t gibson_node_set_percent_width(gibson_node_t *node, float percent);
gibson_status_t gibson_node_set_min_width(gibson_node_t *node, float width);
gibson_status_t gibson_node_set_max_width(gibson_node_t *node, float width);
gibson_status_t gibson_node_set_flex_grow(gibson_node_t *node, float grow);
gibson_status_t gibson_node_set_gap(gibson_node_t *node, float gap);
gibson_status_t gibson_node_set_padding(gibson_node_t *node, float padding);
gibson_status_t gibson_node_set_padding_sides(gibson_node_t *node, float left, float right, float top, float bottom);
void            gibson_node_free(gibson_node_t *node);

#ifdef __cplusplus
}
#endif

#endif /* GIBSON_H */
