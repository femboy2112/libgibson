#ifndef GIBSON_HPP
#define GIBSON_HPP

#include "../../include/gibson.h"
#include <string>
#include <stdexcept>
#include <utility>

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

class Context {
public:
    explicit Context(gibson_render_mode_t mode = GIBSON_MODE_INLINE) {
        gibson_status_t status = gibson_create_context(mode, &ctx_);
        if (status != GIBSON_OK) {
            char buf[256] = {0};
            gibson_last_error_message(buf, sizeof(buf));
            throw std::runtime_error(std::string("Failed to create Gibson context: ") + buf);
        }
    }

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

private:
    gibson_context_t* ctx_{nullptr};
};

} // namespace gibson

#endif /* GIBSON_HPP */
