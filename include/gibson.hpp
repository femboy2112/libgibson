#ifndef GIBSON_HPP
#define GIBSON_HPP

#include "gibson.h"
#include <cstddef>
#include <cstdint>
#include <string>
#include <stdexcept>
#include <utility>
#include <vector>

namespace gibson {

class Node {
public:
    explicit Node(gibson_node_t* raw) : raw_(raw) {}

    ~Node() {
        if (raw_) {
            gibson_node_free(raw_);
            raw_ = nullptr;
        }
    }

    Node(const Node&) = delete;
    Node& operator=(const Node&) = delete;

    Node(Node&& other) noexcept : raw_(other.raw_) {
        other.raw_ = nullptr;
    }

    Node& operator=(Node&& other) noexcept {
        if (this != &other) {
            if (raw_) {
                gibson_node_free(raw_);
            }
            raw_ = other.raw_;
            other.raw_ = nullptr;
        }
        return *this;
    }

    static Node col() {
        gibson_node_t* n = nullptr;
        gibson_node_box_col(&n);
        return Node(n);
    }

    static Node row() {
        gibson_node_t* n = nullptr;
        gibson_node_box_row(&n);
        return Node(n);
    }

    static Node stack() {
        gibson_node_t* n = nullptr;
        gibson_node_stack(&n);
        return Node(n);
    }

    static Node dim() {
        gibson_node_t* n = nullptr;
        gibson_node_dim(&n);
        return Node(n);
    }

    static Node text(const std::string& str, const gibson_style_t* style = nullptr, gibson_wrap_mode_t wrap = GIBSON_WRAP_WORD) {
        gibson_node_t* n = nullptr;
        gibson_node_text(str.c_str(), style, wrap, &n);
        return Node(n);
    }

    static Node border_box(gibson_border_type_t type = GIBSON_BORDER_ROUNDED, const gibson_style_t* style = nullptr) {
        gibson_node_t* n = nullptr;
        gibson_node_border_box(type, style, &n);
        return Node(n);
    }

    static Node rule(const std::string& title = "", const gibson_style_t* style = nullptr) {
        gibson_node_t* n = nullptr;
        const char* title_ptr = title.empty() ? nullptr : title.c_str();
        gibson_node_rule(title_ptr, style, &n);
        return Node(n);
    }

    static Node rail(const gibson_style_t* style = nullptr) {
        gibson_node_t* n = nullptr;
        gibson_node_rail(style, &n);
        return Node(n);
    }

    static Node spinner(uint32_t frame_index = 0, const gibson_style_t* style = nullptr, const std::string& label = "") {
        gibson_node_t* n = nullptr;
        const char* label_ptr = label.empty() ? nullptr : label.c_str();
        gibson_node_spinner(frame_index, style, label_ptr, &n);
        return Node(n);
    }

    static Node text_input(const std::string& value, uint32_t cursor_grapheme = 0, const std::string& placeholder = "", const gibson_style_t* style = nullptr) {
        gibson_node_t* n = nullptr;
        const char* placeholder_ptr = placeholder.empty() ? nullptr : placeholder.c_str();
        gibson_node_text_input(value.c_str(), cursor_grapheme, placeholder_ptr, style, &n);
        return Node(n);
    }

    Node& width(float w) {
        if (raw_) gibson_node_set_width(raw_, w);
        return *this;
    }

    Node& height(float h) {
        if (raw_) gibson_node_set_height(raw_, h);
        return *this;
    }

    Node& percent_width(float pw) {
        if (raw_) gibson_node_set_percent_width(raw_, pw);
        return *this;
    }

    Node& min_width(float mw) {
        if (raw_) gibson_node_set_min_width(raw_, mw);
        return *this;
    }

    Node& max_width(float mw) {
        if (raw_) gibson_node_set_max_width(raw_, mw);
        return *this;
    }

    Node& flex_grow(float grow) {
        if (raw_) gibson_node_set_flex_grow(raw_, grow);
        return *this;
    }

    Node& gap(float g) {
        if (raw_) gibson_node_set_gap(raw_, g);
        return *this;
    }

    Node& padding(float p) {
        if (raw_) gibson_node_set_padding(raw_, p);
        return *this;
    }

    Node& padding_sides(float left, float right, float top, float bottom) {
        if (raw_) gibson_node_set_padding_sides(raw_, left, right, top, bottom);
        return *this;
    }

    Node& add_child(Node child) {
        if (raw_ && child.raw_) {
            gibson_node_add_child(raw_, child.release());
        }
        return *this;
    }

    gibson_node_t* release() {
        gibson_node_t* tmp = raw_;
        raw_ = nullptr;
        return tmp;
    }

    gibson_node_t* raw() const { return raw_; }

private:
    gibson_node_t* raw_{nullptr};
};

// Language-neutral structured rich text RAII wrappers.
class Line {
public:
    explicit Line(gibson_line_t* raw) : raw_(raw) {}
    ~Line() { if (raw_) { gibson_line_free(raw_); raw_ = nullptr; } }
    Line(const Line&) = delete;
    Line& operator=(const Line&) = delete;
    Line(Line&& other) noexcept : raw_(other.raw_) { other.raw_ = nullptr; }
    Line& operator=(Line&& other) noexcept {
        if (this != &other) { if (raw_) gibson_line_free(raw_); raw_ = other.raw_; other.raw_ = nullptr; }
        return *this;
    }

    static Line create() {
        gibson_line_t* l = nullptr;
        gibson_line_new(&l);
        return Line(l);
    }

    Line& add_span(const std::string& text, const gibson_style_t* style = nullptr) {
        if (raw_) gibson_line_add_span(raw_, text.c_str(), style);
        return *this;
    }

    Line& align(gibson_align_t a) {
        if (raw_) gibson_line_set_align(raw_, a);
        return *this;
    }

    gibson_line_t* raw() const { return raw_; }

private:
    gibson_line_t* raw_{nullptr};
};

class RichText {
public:
    explicit RichText(gibson_rich_text_t* raw) : raw_(raw) {}
    ~RichText() { if (raw_) { gibson_rich_text_free(raw_); raw_ = nullptr; } }
    RichText(const RichText&) = delete;
    RichText& operator=(const RichText&) = delete;
    RichText(RichText&& other) noexcept : raw_(other.raw_) { other.raw_ = nullptr; }
    RichText& operator=(RichText&& other) noexcept {
        if (this != &other) { if (raw_) gibson_rich_text_free(raw_); raw_ = other.raw_; other.raw_ = nullptr; }
        return *this;
    }

    static RichText create() {
        gibson_rich_text_t* r = nullptr;
        gibson_rich_text_new(&r);
        return RichText(r);
    }

    // Borrows `line`; the caller retains ownership of it.
    RichText& add_line(const Line& line) {
        if (raw_ && line.raw()) gibson_rich_text_add_line(raw_, line.raw());
        return *this;
    }

    gibson_rich_text_t* raw() const { return raw_; }

private:
    gibson_rich_text_t* raw_{nullptr};
};

// Error convention of the headless / event / readback API below: exceptions. A non-OK status throws
// gibson::Error (a std::runtime_error, so the existing `catch (const std::runtime_error&)` keeps
// working), carrying the status and the C ABI's last-error text. The last error is thread-local, so
// it is read in the constructor of the Error, i.e. right after the failing call on the same thread.
// (The older Context methods above keep throwing a plain std::runtime_error.)
class Error : public std::runtime_error {
public:
    Error(const char* what, gibson_status_t status) : Error(what, status, last_error_text()) {}
    gibson_status_t status() const noexcept { return status_; }
    // The C ABI's last-error text at the time of the failure ("" if it set none).
    const std::string& last_error() const noexcept { return last_error_; }

private:
    Error(const char* what, gibson_status_t status, std::string err)
        : std::runtime_error(compose(what, status, err)), status_(status), last_error_(std::move(err)) {}
    static std::string last_error_text() {
        char buf[512] = {0};
        gibson_last_error_message(buf, sizeof(buf));
        return std::string(buf);
    }
    static std::string compose(const char* what, gibson_status_t status, const std::string& err) {
        std::string m = std::string("Gibson ") + what + " failed: status " + std::to_string(static_cast<int>(status));
        return err.empty() ? m : m + ": " + err;
    }
    gibson_status_t status_;
    std::string last_error_;
};

// push_event on a headless context already holding GIBSON_EVENT_QUEUE_CAPACITY events.
class QueueFullError : public Error {
public:
    explicit QueueFullError(const char* what) : Error(what, GIBSON_ERR_QUEUE_FULL) {}
};

// One input event, as pushed to or polled from a headless context. Fields the type does not use are
// 0 (poll writes them that way). `key_kind` is meaningful only when `has_kind`: a KEY event polled
// with poll_event_kind carries GIBSON_KEY_KIND_CHAR / _SPECIAL (a typed U+1001 stays distinct from
// GIBSON_KEY_ENTER); other types carry GIBSON_KEY_KIND_NONE. A kind-less poll (poll_event(timeout))
// leaves has_kind false. push_event uses gibson_push_event_kind iff has_kind, else gibson_push_event
// (U+1001..U+100D are then always the special keys). `paste` is only read when `has_paste`; an
// event with has_paste false and type PASTE is pushed with a NULL paste_str (the C ABI refuses it).
// A polled Event can be pushed back unchanged.
struct Event {
    int32_t type = GIBSON_EVENT_NONE;
    uint32_t key_code = 0;
    uint8_t modifiers = 0;
    uint16_t cols = 0;
    uint16_t rows = 0;
    bool has_paste = false;
    std::string paste;  // raw bytes; polled pastes are valid UTF-8
    bool has_kind = false;
    uint32_t key_kind = GIBSON_KEY_KIND_NONE;

    static Event char_key(uint32_t scalar, uint8_t mods = 0) {
        Event e; e.type = GIBSON_EVENT_KEY; e.key_code = scalar; e.modifiers = mods;
        e.has_kind = true; e.key_kind = GIBSON_KEY_KIND_CHAR; return e;
    }
    static Event special_key(uint32_t key, uint8_t mods = 0) {
        Event e; e.type = GIBSON_EVENT_KEY; e.key_code = key; e.modifiers = mods;
        e.has_kind = true; e.key_kind = GIBSON_KEY_KIND_SPECIAL; return e;
    }
    // Kind-less key: gibson_push_event semantics.
    static Event legacy_key(uint32_t code, uint8_t mods = 0) {
        Event e; e.type = GIBSON_EVENT_KEY; e.key_code = code; e.modifiers = mods; return e;
    }
    static Event paste_text(std::string bytes) {
        Event e; e.type = GIBSON_EVENT_PASTE; e.has_paste = true; e.paste = std::move(bytes); return e;
    }
    static Event resize(uint16_t c, uint16_t r) {
        Event e; e.type = GIBSON_EVENT_RESIZE; e.cols = c; e.rows = r; return e;
    }
    friend bool operator==(const Event& a, const Event& b) {
        return a.type == b.type && a.key_code == b.key_code && a.modifiers == b.modifiers && a.cols == b.cols &&
               a.rows == b.rows && a.has_paste == b.has_paste && a.paste == b.paste && a.has_kind == b.has_kind &&
               a.key_kind == b.key_kind;
    }
    friend bool operator!=(const Event& a, const Event& b) { return !(a == b); }
};

class Context {
public:
    explicit Context(gibson_render_mode_t mode = GIBSON_MODE_INLINE) {
        // ABI gate: refuse to run against a native library whose ABI differs from
        // the one this header was compiled against, BEFORE any other ABI call is
        // made. Mirrors the load-time checks in the Python and Go wrappers; see
        // docs/RELEASE_CONTRACT.md §4. There is no negotiation — a mismatch is a
        // hard, clear failure, not a segfault-first-explain-later.
        if (!abi_compatible()) {
            throw std::runtime_error(
                std::string("LibGibson ABI mismatch: this header was built against ABI ") +
                std::to_string(static_cast<unsigned>(GIBSON_ABI_VERSION)) +
                ", but the loaded native library reports ABI " +
                std::to_string(static_cast<unsigned>(gibson_abi_version())) +
                "; install a compatible libgibson");
        }
        gibson_status_t status = gibson_create_context(mode, &ctx_);
        if (status != GIBSON_OK) {
            char buf[256] = {0};
            gibson_last_error_message(buf, sizeof(buf));
            throw std::runtime_error(std::string("Failed to create Gibson context: ") + buf);
        }
    }

    // A HEADLESS context: renders into an in-memory buffer with deterministic, TTY-independent
    // semantics (fixed cols x rows, truecolor, no terminal lease); events are injected with
    // push_event and the frame is read back with frame_*/rendered_output. Throws Error on a failed
    // create (non-zero cols and rows required) and std::runtime_error on an ABI mismatch.
    static Context headless(gibson_render_mode_t mode, uint16_t cols, uint16_t rows) {
        if (!abi_compatible()) {
            throw std::runtime_error("LibGibson ABI mismatch: install a compatible libgibson");
        }
        gibson_context_t* h = nullptr;
        gibson_status_t st = gibson_create_headless_context(mode, cols, rows, &h);
        if (st != GIBSON_OK) throw Error("create_headless_context", st);
        return Context(h, AdoptTag());
    }

    // Destroys the native context now; later calls on this object are undefined, but a second
    // close() (or the destructor after close()) is a no-op: the handle is cleared, never destroyed twice.
    void close() noexcept {
        if (ctx_) {
            gibson_destroy_context(ctx_);
            ctx_ = nullptr;
        }
    }

    // The underlying handle, for C ABI calls the wrapper does not cover. Still owned by this object.
    gibson_context_t* raw() const { return ctx_; }

    ~Context() {
        if (ctx_) {
            gibson_destroy_context(ctx_);
            ctx_ = nullptr;
        }
    }

    Context(const Context&) = delete;
    Context& operator=(const Context&) = delete;

    Context(Context&& other) noexcept : ctx_(other.ctx_) {
        other.ctx_ = nullptr;
    }

    Context& operator=(Context&& other) noexcept {
        if (this != &other) {
            if (ctx_) gibson_destroy_context(ctx_);
            ctx_ = other.ctx_;
            other.ctx_ = nullptr;
        }
        return *this;
    }

    void set_sync_updates(bool enabled) {
        gibson_status_t st = gibson_set_sync_updates(ctx_, enabled ? 1 : 0);
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson set_sync_updates failed");
        }
    }

    void set_max_fps(uint32_t fps) {
        gibson_status_t st = gibson_set_max_fps(ctx_, fps);
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson set_max_fps failed");
        }
    }

    void set_root(Node root) {
        gibson_status_t st = gibson_set_root_node(ctx_, root.release());
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson set_root failed");
        }
    }

    void render() {
        gibson_status_t st = gibson_render(ctx_);
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson render failed");
        }
    }

    bool render_if_due() {
        int32_t rendered = 0;
        gibson_status_t st = gibson_render_if_due(ctx_, &rendered);
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson render_if_due failed");
        }
        return rendered != 0;
    }

    void request_render() {
        gibson_status_t st = gibson_request_render(ctx_);
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson request_render failed");
        }
    }

    void commit(const std::string& text) {
        gibson_status_t st = gibson_commit_text(ctx_, text.c_str());
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson commit failed");
        }
    }

    void commit_text(const std::string& text) {
        gibson_status_t st = gibson_commit_text(ctx_, text.c_str());
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson commit_text failed");
        }
    }

    void commit_raw_ansi_unchecked(const std::string& text) {
        gibson_status_t st = gibson_commit_raw_ansi_unchecked(ctx_, text.c_str());
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson commit_raw_ansi_unchecked failed");
        }
    }

    void commit_rich_text(const RichText& rich) {
        gibson_status_t st = gibson_commit_rich_text(ctx_, rich.raw());
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson commit_rich_text failed");
        }
    }

    void insert_rich_text_before_live(const RichText& rich) {
        gibson_status_t st = gibson_insert_rich_text_before_live(ctx_, rich.raw());
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson insert_rich_text_before_live failed");
        }
    }

    /// Safe: width-aware plain text with terminal controls neutralized.
    void insert_text_before_live(const std::string& text) {
        gibson_status_t st = gibson_insert_text_before_live(ctx_, text.c_str());
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson insert_text_before_live failed");
        }
    }

    /// Raw escape hatch: `text` is a terminal byte stream and is NOT sanitized.
    void insert_raw_lines_before_live_unchecked(const std::string& text) {
        gibson_status_t st = gibson_insert_raw_lines_before_live_unchecked(ctx_, text.c_str());
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson insert_raw_lines_before_live_unchecked failed");
        }
    }

    void commit_node(Node node) {
        gibson_status_t st = gibson_commit_node(ctx_, node.release());
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson commit_node failed");
        }
    }

    void insert_node_before_live(Node node) {
        gibson_status_t st = gibson_insert_node_before_live(ctx_, node.release());
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson insert_node_before_live failed");
        }
    }

    void clear_live_region() {
        gibson_status_t st = gibson_clear_live_region(ctx_);
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson clear_live_region failed");
        }
    }

    bool poll_event(uint32_t timeout_ms, gibson_event_t& out_event) {
        gibson_status_t st = gibson_poll_event(ctx_, timeout_ms, &out_event);
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson poll_event failed");
        }
        return out_event.event_type != GIBSON_EVENT_NONE;
    }

    // ---- headless events (see the Error convention above) -----------------------------------

    // Queues `ev` on a HEADLESS context for poll_event / poll_event_kind (at most
    // GIBSON_EVENT_QUEUE_CAPACITY undelivered). Throws QueueFullError (nothing displaced) or Error
    // (status GIBSON_ERR_INVALID_PARAM for a terminal context or an unrepresentable event), both with the
    // last-error text; std::invalid_argument if `paste` holds a NUL byte (the C string would be cut
    // short). Nothing else is validated here: the C ABI decides.
    void push_event(const Event& ev) {
        gibson_event_t c{};
        c.event_type = ev.type;
        c.key_code = ev.key_code;
        c.modifiers = ev.modifiers;
        c.cols = ev.cols;
        c.rows = ev.rows;
        if (ev.has_paste) {
            if (ev.paste.find('\0') != std::string::npos) {
                throw std::invalid_argument("Gibson Event.paste contains a NUL byte");
            }
            c.paste_str = ev.paste.c_str();
        }
        gibson_status_t st = ev.has_kind ? gibson_push_event_kind(ctx_, &c, ev.key_kind) : gibson_push_event(ctx_, &c);
        if (st == GIBSON_ERR_QUEUE_FULL) throw QueueFullError("push_event");
        if (st != GIBSON_OK) throw Error("push_event", st);
    }

    // Next event with its key kind (gibson_poll_event_kind): has_kind is true. On a headless context it
    // never waits and returns type GIBSON_EVENT_NONE on an empty queue; a polled RESIZE has already set
    // the headless size. Throws Error on a non-OK status.
    Event poll_event_kind(uint32_t timeout_ms = 0) {
        gibson_event_t c{};
        uint32_t kind = GIBSON_KEY_KIND_NONE;
        gibson_status_t st = gibson_poll_event_kind(ctx_, timeout_ms, &c, &kind);
        if (st != GIBSON_OK) throw Error("poll_event_kind", st);
        return from_c(c, true, kind);
    }

    // Kind-less poll (gibson_poll_event): a typed U+1001 and GIBSON_KEY_ENTER look the same; has_kind
    // is false. (The older poll_event(timeout, gibson_event_t&) overload is unchanged.)
    Event poll_event(uint32_t timeout_ms) {
        gibson_event_t c{};
        gibson_status_t st = gibson_poll_event(ctx_, timeout_ms, &c);
        if (st != GIBSON_OK) throw Error("poll_event", st);
        return from_c(c, false, GIBSON_KEY_KIND_NONE);
    }

    // ---- frame readback (headless or terminal; throws Error on a non-OK status) ---------------

    // (cols, rows) of the most recently rendered frame, (0, 0) if there is none yet.
    std::pair<uint16_t, uint16_t> frame_size() const {
        uint16_t c = 0, r = 0;
        gibson_status_t st = gibson_get_frame_size(ctx_, &c, &r);
        if (st != GIBSON_OK) throw Error("frame_size", st);
        return std::make_pair(c, r);
    }

    // The resolved styled cell at (x, y). Throws Error if there is no frame or (x, y) is out of bounds.
    gibson_cell_t frame_cell(uint16_t x, uint16_t y) const {
        gibson_cell_t cell{};
        gibson_status_t st = gibson_get_frame_cell(ctx_, x, y, &cell);
        if (st != GIBSON_OK) throw Error("frame_cell", st);
        return cell;
    }

    // Bulk readback, row-major from (x, y) to the end of the frame (at most max_cells). One ABI call.
    std::vector<gibson_cell_t> frame_cells(uint16_t x = 0, uint16_t y = 0, size_t max_cells = static_cast<size_t>(-1)) const {
        std::pair<uint16_t, uint16_t> sz = frame_size();
        size_t total = static_cast<size_t>(sz.first) * sz.second;
        size_t start = static_cast<size_t>(y) * sz.first + x;
        size_t avail = total > start ? total - start : 0;
        size_t want = max_cells < avail ? max_cells : avail;
        if (want == 0) return std::vector<gibson_cell_t>();
        std::vector<gibson_cell_t> cells(want);
        size_t written = 0;
        gibson_status_t st = gibson_get_frame_cells(ctx_, x, y, cells.data(), want, &written);
        if (st != GIBSON_OK) throw Error("frame_cells", st);
        cells.resize(written);
        return cells;
    }

    // The full (untruncated) grapheme-cluster UTF-8 bytes at (x, y): the escape hatch for cells whose
    // glyph_truncated flag (_reserved[0]) is set.
    std::string frame_cell_glyph(uint16_t x, uint16_t y) const {
        size_t len = 0;
        gibson_status_t st = gibson_get_frame_cell_glyph(ctx_, x, y, nullptr, 0, &len);
        if (st != GIBSON_OK) throw Error("frame_cell_glyph", st);
        std::string g(len, '\0');
        if (len == 0) return g;
        size_t got = 0;
        st = gibson_get_frame_cell_glyph(ctx_, x, y, reinterpret_cast<uint8_t*>(&g[0]), len, &got);
        if (st != GIBSON_OK) throw Error("frame_cell_glyph", st);
        g.resize(got);
        return g;
    }

    // The cell's complete glyph bytes: the inline bytes, or frame_cell_glyph(x, y) if they were truncated.
    std::string full_glyph(const gibson_cell_t& cell, uint16_t x, uint16_t y) const {
        if (cell._reserved[0]) return frame_cell_glyph(x, y);
        return std::string(reinterpret_cast<const char*>(cell.glyph), cell.glyph_len);
    }

    // ---- headless capture ----------------------------------------------------------------------

    // The ANSI frame bytes captured since creation or the last take/clear. Copies; never drains.
    std::string rendered_output() const {
        for (;;) {
            size_t need = 0;
            gibson_status_t st = gibson_get_rendered_output(ctx_, nullptr, 0, &need);
            if (st != GIBSON_OK) throw Error("rendered_output", st);
            if (need == 0) return std::string();
            std::string out(need, '\0');
            size_t got = 0;
            st = gibson_get_rendered_output(ctx_, reinterpret_cast<uint8_t*>(&out[0]), need, &got);
            if (st != GIBSON_OK) throw Error("rendered_output", st);
            if (got > need) continue;  // grew between the two calls; nothing was copied
            out.resize(got);
            return out;
        }
    }

    // Drains the capture: returns its bytes and empties it (gibson_take_rendered_output, which is
    // all-or-nothing, so a result never splits an escape sequence).
    std::string take_rendered_output() {
        for (;;) {
            size_t need = 0;
            gibson_status_t st = gibson_take_rendered_output(ctx_, nullptr, 0, &need);
            if (st != GIBSON_OK) throw Error("take_rendered_output", st);
            if (need == 0) return std::string();
            std::string out(need, '\0');
            size_t avail = 0;
            st = gibson_take_rendered_output(ctx_, reinterpret_cast<uint8_t*>(&out[0]), need, &avail);
            if (st != GIBSON_OK) throw Error("take_rendered_output", st);
            if (avail > need) continue;  // grew between the two calls; nothing was removed
            out.resize(avail);
            return out;
        }
    }

    // Discards the capture without copying it; returns the number of bytes removed.
    size_t clear_rendered_output() {
        size_t n = 0;
        gibson_status_t st = gibson_clear_rendered_output(ctx_, &n);
        if (st != GIBSON_OK) throw Error("clear_rendered_output", st);
        return n;
    }

    gibson_stats_t stats() const {
        gibson_stats_t s{};
        gibson_stats_init(&s);
        gibson_status_t st = gibson_get_stats(ctx_, &s);
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson get_stats failed");
        }
        return s;
    }

    static uint32_t abi_version() { return gibson_abi_version(); }

    // True iff the loaded native library's ABI matches the ABI this header was
    // compiled against (GIBSON_ABI_VERSION). The Context constructor enforces
    // this and throws on mismatch; callers may also check it explicitly.
    static bool abi_compatible() { return gibson_abi_version() == GIBSON_ABI_VERSION; }

private:
    static Event from_c(const gibson_event_t& c, bool has_kind, uint32_t kind) {
        Event e;
        e.type = c.event_type;
        e.key_code = c.key_code;
        e.modifiers = c.modifiers;
        e.cols = c.cols;
        e.rows = c.rows;
        // paste_str is only valid until the next poll: copy it out now.
        if (c.event_type == GIBSON_EVENT_PASTE && c.paste_str) {
            e.has_paste = true;
            e.paste = c.paste_str;
        }
        e.has_kind = has_kind;
        e.key_kind = kind;
        return e;
    }
    struct AdoptTag {};
    Context(gibson_context_t* adopted, AdoptTag) : ctx_(adopted) {}
    gibson_context_t* ctx_{nullptr};
};

} // namespace gibson

#endif /* GIBSON_HPP */
