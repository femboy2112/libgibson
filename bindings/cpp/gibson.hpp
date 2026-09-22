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

    Node& width(float w) {
        if (raw_) gibson_node_set_width(raw_, w);
        return *this;
    }

    Node& height(float h) {
        if (raw_) gibson_node_set_height(raw_, h);
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

    void set_root(Node root) {
        gibson_set_root_node(ctx_, root.release());
    }

    void render() {
        gibson_status_t st = gibson_render(ctx_);
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson render failed");
        }
    }

    void commit(const std::string& text) {
        gibson_status_t st = gibson_commit(ctx_, text.c_str());
        if (st != GIBSON_OK) {
            throw std::runtime_error("Gibson commit failed");
        }
    }

    gibson_stats_t stats() const {
        gibson_stats_t s{};
        gibson_get_stats(ctx_, &s);
        return s;
    }

private:
    gibson_context_t* ctx_{nullptr};
};

} // namespace gibson

#endif /* GIBSON_HPP */
