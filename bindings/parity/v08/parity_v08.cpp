// LibGibson v0.8 parity slice -- C++ consumer.
//
// Interprets bindings/parity/v08/script.txt against the INSTALLED SDK (#include <gibson.hpp> via
// pkg-config) and writes the canonical transcript to stdout. Format: docs/V08_PARITY_SLICE.md.
//
//   parity_v08_cpp SCRIPT [REPS]
//
// Surface used: gibson.hpp's RAII gibson::Node (col / text / add_child / release) builds the scene
// and gibson::Context::abi_compatible() gates the run. gibson.hpp has NO headless Context, no
// push/poll_kind, and no frame readback, so those go through the raw C ABI it includes -- that
// split is itself a measured result (docs/V08_PARITY_SLICE.md, "C++").
//
// Negative controls (never on by default): PARITY_V08_PERTURB=cell | events (see parity_v08.c).
#include <gibson.hpp>

#include <sys/resource.h>

#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <iostream>
#include <map>
#include <sstream>
#include <string>
#include <type_traits>
#include <utility>
#include <vector>

namespace {

constexpr uint64_t kFnvOffset = 0xcbf29ce484222325ULL;
constexpr uint64_t kFnvPrime = 0x100000001b3ULL;

// Compile-time probes of the gibson.hpp surface: does gibson::Context have member NAME?
#define PARITY_HAS_MEMBER(name)                                                        \
    template <typename T, typename = void>                                             \
    struct has_##name : std::false_type {};                                            \
    template <typename T>                                                              \
    struct has_##name<T, std::void_t<decltype(&T::name)>> : std::true_type {};
PARITY_HAS_MEMBER(headless)
PARITY_HAS_MEMBER(poll_event)
PARITY_HAS_MEMBER(poll_event_kind)
PARITY_HAS_MEMBER(push_event)
PARITY_HAS_MEMBER(push_event_kind)
PARITY_HAS_MEMBER(frame_size)
PARITY_HAS_MEMBER(frame_cells)
PARITY_HAS_MEMBER(rendered_output)

uint64_t fnv(uint64_t h, const void* data, size_t n) {
    auto p = static_cast<const unsigned char*>(data);
    for (size_t i = 0; i < n; ++i) { h ^= p[i]; h *= kFnvPrime; }
    return h;
}

std::string hex(const unsigned char* p, size_t n) {
    static const char d[] = "0123456789abcdef";
    std::string s;
    s.reserve(2 * n);
    for (size_t i = 0; i < n; ++i) { s += d[p[i] >> 4]; s += d[p[i] & 15]; }
    return s;
}
std::string hex(const std::string& b) { return hex(reinterpret_cast<const unsigned char*>(b.data()), b.size()); }

std::string unhex(const std::string& h) {
    std::string out;
    for (size_t i = 0; i + 1 < h.size(); i += 2) out += static_cast<char>(std::stoi(h.substr(i, 2), nullptr, 16));
    return out;
}

std::string hex16(uint64_t v) {
    char b[17];
    std::snprintf(b, sizeof b, "%016llx", static_cast<unsigned long long>(v));
    return b;
}

// A record: key -> already-rendered JSON value. std::map keeps keys sorted (bytewise).
class Rec {
public:
    Rec& i(const std::string& k, long long v) { m_[k] = std::to_string(v); return *this; }
    Rec& s(const std::string& k, const std::string& v) {
        bool safe = true;
        for (unsigned char c : v) if (c < 0x20 || c > 0x7e || c == '"' || c == '\\') { safe = false; break; }
        m_[k] = "\"" + (safe ? v : "hex:" + hex(v)) + "\"";
        return *this;
    }
    Rec& a4(const std::string& k, int a, int b, int c, int d) {
        m_[k] = "[" + std::to_string(a) + "," + std::to_string(b) + "," + std::to_string(c) + "," + std::to_string(d) + "]";
        return *this;
    }
    void emit(std::string& out) const {
        out += "{";
        bool first = true;
        for (const auto& kv : m_) {
            if (!first) out += ",";
            first = false;
            out += "\"" + kv.first + "\":" + kv.second;
        }
        out += "}\n";
    }

private:
    std::map<std::string, std::string> m_;
};

std::string st_name(int s) {
    switch (s) {
    case GIBSON_OK: return "OK";
    case GIBSON_ERR_INVALID_PARAM: return "INVALID_PARAM";
    case GIBSON_ERR_IO: return "IO";
    case GIBSON_ERR_PANIC: return "PANIC";
    case GIBSON_ERR_NOT_INITIALIZED: return "NOT_INITIALIZED";
    case GIBSON_ERR_QUEUE_FULL: return "QUEUE_FULL";
    default: return "UNKNOWN";
    }
}
std::string kind_name(uint32_t k) {
    return k == GIBSON_KEY_KIND_CHAR ? "CHAR" : k == GIBSON_KEY_KIND_SPECIAL ? "SPECIAL" : "NONE";
}
std::string type_name(int32_t t) {
    switch (t) {
    case GIBSON_EVENT_KEY: return "KEY";
    case GIBSON_EVENT_PASTE: return "PASTE";
    case GIBSON_EVENT_RESIZE: return "RESIZE";
    default: return "NONE";
    }
}
std::string last_err() {
    char b[512] = {0};
    gibson_last_error_message(b, sizeof b);
    return b;
}

struct Step {
    std::vector<std::string> tok;  // tok[0]=id tok[1]=dim tok[2]=op
};

std::vector<Step> load_script(const std::string& path) {
    std::ifstream f(path);
    std::vector<Step> steps;
    std::string line;
    while (std::getline(f, line)) {
        if (!line.empty() && line.back() == '\r') line.pop_back();
        if (line.empty() || line[0] == '#') continue;
        Step st;
        std::istringstream is(line);
        std::string t;
        while (is >> t) st.tok.push_back(t);
        steps.push_back(st);
    }
    return steps;
}

void apply_perturbation(std::vector<Step>& steps) {
    const char* p = std::getenv("PARITY_V08_PERTURB");
    if (!p) return;
    std::string mode = p;
    if (mode == "cell") {
        for (auto& s : steps)
            if (s.tok[0] == "s06") { auto& h = s.tok.back(); h.replace(h.size() - 2, 2, "3f"); }
    } else if (mode == "events") {
        size_t a = steps.size(), b = steps.size();
        for (size_t i = 0; i < steps.size(); ++i) {
            if (steps[i].tok[0] == "e01") a = i;
            if (steps[i].tok[0] == "e02") b = i;
        }
        if (a < steps.size() && b < steps.size()) std::swap(steps[a], steps[b]);
    }
}

gibson_color_t parse_color(const std::string& spec) {
    gibson_color_t c{};
    unsigned r = 0, g = 0, b = 0, n = 0;
    if (spec == "reset") c.color_type = GIBSON_COLOR_RESET;
    else if (std::sscanf(spec.c_str(), "rgb:%u,%u,%u", &r, &g, &b) == 3) {
        c.color_type = GIBSON_COLOR_RGB; c.r = static_cast<uint8_t>(r); c.g = static_cast<uint8_t>(g); c.b = static_cast<uint8_t>(b);
    } else if (std::sscanf(spec.c_str(), "ansi:%u", &n) == 1) { c.color_type = GIBSON_COLOR_ANSI256; c.r = static_cast<uint8_t>(n); }
    else if (std::sscanf(spec.c_str(), "named:%u", &n) == 1) c.color_type = static_cast<int32_t>(n);
    return c;
}

gibson_wrap_mode_t wrap_of(const std::string& w) {
    return w == "CHAR" ? GIBSON_WRAP_CHAR : w == "WORD" ? GIBSON_WRAP_WORD : GIBSON_WRAP_NONE;
}
uint32_t cp_of(const std::string& u) { return static_cast<uint32_t>(std::stoul(u.substr(2), nullptr, 16)); }
uint32_t special_of(const std::string& n) {
    static const std::map<std::string, uint32_t> t = {
        {"ENTER", GIBSON_KEY_ENTER}, {"LEFT", GIBSON_KEY_LEFT}, {"RIGHT", GIBSON_KEY_RIGHT}, {"UP", GIBSON_KEY_UP},
        {"DOWN", GIBSON_KEY_DOWN}, {"HOME", GIBSON_KEY_HOME}, {"END", GIBSON_KEY_END},
        {"BACKSPACE", GIBSON_KEY_BACKSPACE}, {"DELETE", GIBSON_KEY_DELETE}, {"ESC", GIBSON_KEY_ESC},
        {"TAB", GIBSON_KEY_TAB}, {"PAGE_UP", GIBSON_KEY_PAGE_UP}, {"PAGE_DOWN", GIBSON_KEY_PAGE_DOWN}};
    auto it = t.find(n);
    return it == t.end() ? 0 : it->second;
}

class Interp {
public:
    explicit Interp(std::string& out) : out_(out) {}
    ~Interp() {
        if (ctx_) gibson_destroy_context(ctx_);
    }

    void run(const std::vector<Step>& steps) {
        for (const auto& s : steps) step(s);
    }

private:
    std::string& out_;
    gibson_context_t* ctx_ = nullptr;
    gibson::Node root_{nullptr};
    std::map<std::string, gibson_style_t> styles_;

    std::string full_glyph(const gibson_cell_t& c, uint16_t x, uint16_t y) {
        if (!c._reserved[0]) return std::string(reinterpret_cast<const char*>(c.glyph), c.glyph_len);
        size_t len = 0;
        gibson_get_frame_cell_glyph(ctx_, x, y, nullptr, 0, &len);
        std::string g(len, '\0');
        gibson_get_frame_cell_glyph(ctx_, x, y, reinterpret_cast<uint8_t*>(&g[0]), len, &len);
        return g;
    }

    static std::string attrs(const gibson_style_t& s) {
        std::string a;
        for (uint8_t v : {s.bold, s.dim, s.italic, s.underline, s.reverse}) a += v ? '1' : '0';
        return a;
    }

    std::string cell_line(const gibson_cell_t& c, uint16_t x, uint16_t y) {
        const auto& s = c.style;
        std::ostringstream o;
        o << hex(full_glyph(c, x, y)) << ';' << +c.width << ';' << +c.is_continuation << ';' << +c.transparent << ';'
          << s.fg.color_type << ',' << +s.fg.r << ',' << +s.fg.g << ',' << +s.fg.b << ';' << s.bg.color_type << ','
          << +s.bg.r << ',' << +s.bg.g << ',' << +s.bg.b << ';' << attrs(s) << '\n';
        return o.str();
    }

    void emit_event(const std::string& id, int st, const gibson_event_t& ev, const std::string& kind) {
        bool key = ev.event_type == GIBSON_EVENT_KEY, rs = ev.event_type == GIBSON_EVENT_RESIZE;
        std::string paste;
        if (ev.event_type == GIBSON_EVENT_PASTE && ev.paste_str) paste = hex(std::string(ev.paste_str));
        Rec()
            .i("cols", rs ? ev.cols : 0)
            .s("id", id)
            .s("k", "event")
            .i("key_code", key ? static_cast<long long>(ev.key_code) : 0)
            .s("kind", kind)
            .i("mods", key ? ev.modifiers : 0)
            .s("paste", paste)
            .i("rows", rs ? ev.rows : 0)
            .s("st", st_name(st))
            .s("type", type_name(ev.event_type))
            .emit(out_);
    }

    void probe(const std::string& id, const std::string& name) {
        Rec r;
        r.s("id", id).s("k", "probe").s("name", name);
        uint16_t cols = 0, rows = 0;
        gibson_get_frame_size(ctx_, &cols, &rows);
        gibson_context_t* c2 = nullptr;
        gibson_cell_t cell{};
        gibson_event_t ev{};
        int st = 999;
        bool with_err = false;
        if (name == "create_bad_mode") { st = gibson_create_headless_context(7, 40, 12, &c2); with_err = true; }
        else if (name == "create_zero_cols") { st = gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, 0, 12, &c2); with_err = true; }
        else if (name == "create_null_out") st = gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, 40, 12, nullptr);
        else if (name == "render_null_ctx") st = gibson_render(nullptr);
        else if (name == "cell_oob_x") st = gibson_get_frame_cell(ctx_, cols, 0, &cell);
        else if (name == "cell_oob_y") st = gibson_get_frame_cell(ctx_, 0, rows, &cell);
        else if (name == "cells_oob") {
            size_t written = 99;
            st = gibson_get_frame_cells(ctx_, 0, rows, &cell, 1, &written);
            r.i("written", static_cast<long long>(written));
        } else if (name == "cell_null_out") st = gibson_get_frame_cell(ctx_, 0, 0, nullptr);
        else if (name == "text_bad_utf8") {
            gibson_node_t* n = nullptr;
            st = gibson_node_text("\xff\xfe", nullptr, GIBSON_WRAP_NONE, &n);
            with_err = true;
            if (st == GIBSON_OK) gibson_node_free(n);
        } else if (name == "push_null_event") st = gibson_push_event(ctx_, nullptr);
        else if (name == "push_surrogate") {
            ev.event_type = GIBSON_EVENT_KEY; ev.key_code = 0xD800;
            st = gibson_push_event_kind(ctx_, &ev, GIBSON_KEY_KIND_CHAR);
            with_err = true;
        } else if (name == "push_special_not_key") {
            ev.event_type = GIBSON_EVENT_KEY; ev.key_code = 'a';
            st = gibson_push_event_kind(ctx_, &ev, GIBSON_KEY_KIND_SPECIAL);
        } else if (name == "push_bad_kind") {
            ev.event_type = GIBSON_EVENT_KEY; ev.key_code = 'a';
            st = gibson_push_event_kind(ctx_, &ev, 9);
        } else if (name == "push_bad_mods") {
            ev.event_type = GIBSON_EVENT_KEY; ev.key_code = 'a'; ev.modifiers = 0x80;
            st = gibson_push_event_kind(ctx_, &ev, GIBSON_KEY_KIND_CHAR);
        } else if (name == "push_resize_zero") {
            ev.event_type = GIBSON_EVENT_RESIZE; ev.cols = 0; ev.rows = 12;
            st = gibson_push_event(ctx_, &ev);
        } else if (name == "paste_bad_utf8") {
            ev.event_type = GIBSON_EVENT_PASTE; ev.paste_str = "\xc3\x28";
            st = gibson_push_event(ctx_, &ev);
            with_err = true;
        } else if (name == "paste_null") {
            ev.event_type = GIBSON_EVENT_PASTE; ev.paste_str = nullptr;
            st = gibson_push_event(ctx_, &ev);
        } else if (name == "poll_null_out") st = gibson_poll_event(ctx_, 0, nullptr);
        else if (name == "poll_kind_null") st = gibson_poll_event_kind(ctx_, 0, &ev, nullptr);
        else if (name == "color_depth_bad") st = gibson_set_color_depth(ctx_, 9);
        else if (name == "queue_full") {
            long long accepted = 0, drained = 0;
            ev.event_type = GIBSON_EVENT_KEY; ev.key_code = 'q';
            for (int i = 0; i < GIBSON_EVENT_QUEUE_CAPACITY; ++i)
                if (gibson_push_event_kind(ctx_, &ev, GIBSON_KEY_KIND_CHAR) == GIBSON_OK) ++accepted;
            st = gibson_push_event_kind(ctx_, &ev, GIBSON_KEY_KIND_CHAR);
            std::string e = last_err();
            for (int i = 0; i < 2 * GIBSON_EVENT_QUEUE_CAPACITY; ++i) {
                gibson_event_t got{};
                uint32_t kind = 0;
                if (gibson_poll_event_kind(ctx_, 0, &got, &kind) != GIBSON_OK || got.event_type == GIBSON_EVENT_NONE) break;
                ++drained;
            }
            r.i("accepted", accepted).i("drained", drained).s("err", e);
        } else if (name == "frame_before_render") {
            uint16_t c = 7, rr = 7;
            gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, 8, 2, &c2);
            int sst = gibson_get_frame_size(c2, &c, &rr);
            st = gibson_get_frame_cell(c2, 0, 0, &cell);
            gibson_destroy_context(c2);
            c2 = nullptr;
            r.i("cols", c).i("rows", rr).s("size_st", st_name(sst));
        }
        if (with_err) r.s("err", last_err());
        if (c2) gibson_destroy_context(c2);
        r.s("st", st == 999 ? "UNKNOWN_PROBE" : st_name(st));
        r.emit(out_);
    }

    void step(const Step& s) {
        const std::string& id = s.tok[0];
        const std::string& op = s.tok[2];
        auto arg = [&](size_t i) -> const std::string& { return s.tok[3 + i]; };
        auto num = [&](size_t i) { return std::stoi(arg(i)); };
        if (op == "create") {
            int32_t mode = arg(0) == "INLINE" ? GIBSON_MODE_INLINE : GIBSON_MODE_FULLSCREEN;
            int st = gibson_create_headless_context(mode, static_cast<uint16_t>(num(1)), static_cast<uint16_t>(num(2)), &ctx_);
            Rec().i("cols", num(1)).s("id", id).s("k", "create").s("mode", arg(0)).i("rows", num(2)).s("st", st_name(st)).emit(out_);
        } else if (op == "style") {
            gibson_style_t st{};
            st.fg = parse_color(arg(1));
            st.bg = parse_color(arg(2));
            const std::string& a = arg(3);
            st.bold = a[0] == '1'; st.dim = a[1] == '1'; st.italic = a[2] == '1'; st.underline = a[3] == '1'; st.reverse = a[4] == '1';
            styles_[arg(0)] = st;
        } else if (op == "col") {
            root_ = gibson::Node::col();
            Rec().s("id", id).s("k", "node").s("st", root_.raw() ? "OK" : "WRAPPER_NULL").emit(out_);
        } else if (op == "text") {
            auto it = styles_.find(arg(0));
            // gibson::Node::text drops the C status: a null raw() is the only failure signal.
            gibson::Node n = gibson::Node::text(unhex(arg(2)), it == styles_.end() ? nullptr : &it->second, wrap_of(arg(1)));
            bool ok = n.raw() != nullptr;
            root_.add_child(std::move(n));
            Rec().s("id", id).s("k", "node").s("st", ok ? "OK" : "WRAPPER_NULL").emit(out_);
        } else if (op == "mount") {
            int st = gibson_set_root_node(ctx_, root_.release());  // consumes the tree
            Rec().s("id", id).s("k", "mount").s("st", st_name(st)).emit(out_);
        } else if (op == "render") {
            Rec().s("id", id).s("k", "render").s("st", st_name(gibson_render(ctx_))).emit(out_);
        } else if (op == "geom") {
            uint16_t c = 0, r = 0;
            int st = gibson_get_frame_size(ctx_, &c, &r);
            Rec().i("cols", c).s("id", id).s("k", "geom").i("rows", r).s("st", st_name(st)).emit(out_);
        } else if (op == "frame") {
            uint16_t c = 0, r = 0;
            int st = gibson_get_frame_size(ctx_, &c, &r);
            std::vector<gibson_cell_t> cells(static_cast<size_t>(c) * r + 1);
            size_t written = 0;
            if (st == GIBSON_OK) st = gibson_get_frame_cells(ctx_, 0, 0, cells.data(), static_cast<size_t>(c) * r, &written);
            uint64_t h = kFnvOffset;
            for (size_t k = 0; k < written; ++k) {
                std::string line = cell_line(cells[k], static_cast<uint16_t>(k % c), static_cast<uint16_t>(k / c));
                h = fnv(h, line.data(), line.size());
            }
            size_t wl = 0;
            gibson_get_rendered_output(ctx_, nullptr, 0, &wl);
            std::vector<uint8_t> wire(wl + 1);
            size_t got = 0;
            gibson_get_rendered_output(ctx_, wire.data(), wl, &got);
            Rec()
                .i("cells", static_cast<long long>(written))
                .s("digest", hex16(h))
                .s("id", id)
                .s("k", "frame")
                .s("st", st_name(st))
                .s("wire_digest", hex16(fnv(kFnvOffset, wire.data(), got)))
                .i("wire_len", static_cast<long long>(got))
                .emit(out_);
        } else if (op == "cell") {
            uint16_t x = static_cast<uint16_t>(num(0)), y = static_cast<uint16_t>(num(1));
            gibson_cell_t c{};
            int st = gibson_get_frame_cell(ctx_, x, y, &c);
            std::string g = st == GIBSON_OK ? full_glyph(c, x, y) : "";
            const auto& sy = c.style;
            Rec()
                .s("attrs", attrs(sy))
                .a4("bg", sy.bg.color_type, sy.bg.r, sy.bg.g, sy.bg.b)
                .i("cont", c.is_continuation)
                .a4("fg", sy.fg.color_type, sy.fg.r, sy.fg.g, sy.fg.b)
                .s("glyph", hex(g))
                .s("id", id)
                .s("k", "cell")
                .s("st", st_name(st))
                .i("transp", c.transparent)
                .i("w", c.width)
                .i("x", x)
                .i("y", y)
                .emit(out_);
        } else if (op == "glyph") {
            uint16_t x = static_cast<uint16_t>(num(0)), y = static_cast<uint16_t>(num(1));
            size_t len = 0;
            int st = gibson_get_frame_cell_glyph(ctx_, x, y, nullptr, 0, &len);
            std::string g(len, '\0');
            size_t len2 = 0;
            if (st == GIBSON_OK) st = gibson_get_frame_cell_glyph(ctx_, x, y, reinterpret_cast<uint8_t*>(&g[0]), len, &len2);
            g.resize(len2);
            Rec().s("full_hex", hex(g)).i("full_len", static_cast<long long>(len2)).s("id", id).s("k", "glyph")
                .s("st", st_name(st)).i("x", x).i("y", y).emit(out_);
        } else if (op == "glyphcap") {
            uint16_t x = static_cast<uint16_t>(num(0)), y = static_cast<uint16_t>(num(1));
            gibson_cell_t c{};
            int st = gibson_get_frame_cell(ctx_, x, y, &c);
            Rec().s("id", id).i("inline_len", c.glyph_len).s("k", "glyphcap").s("st", st_name(st))
                .i("trunc", c._reserved[0]).i("x", x).i("y", y).emit(out_);
        } else if (op.rfind("push_", 0) == 0) {
            gibson_event_t ev{};
            std::string paste;
            int st;
            if (op == "push_char") {
                ev.event_type = GIBSON_EVENT_KEY; ev.key_code = cp_of(arg(0)); ev.modifiers = static_cast<uint8_t>(num(1));
                st = gibson_push_event_kind(ctx_, &ev, GIBSON_KEY_KIND_CHAR);
            } else if (op == "push_special") {
                ev.event_type = GIBSON_EVENT_KEY; ev.key_code = special_of(arg(0)); ev.modifiers = static_cast<uint8_t>(num(1));
                st = gibson_push_event_kind(ctx_, &ev, GIBSON_KEY_KIND_SPECIAL);
            } else if (op == "push_legacy") {
                ev.event_type = GIBSON_EVENT_KEY; ev.key_code = cp_of(arg(0)); ev.modifiers = static_cast<uint8_t>(num(1));
                st = gibson_push_event(ctx_, &ev);
            } else if (op == "push_paste") {
                paste = unhex(arg(0));
                ev.event_type = GIBSON_EVENT_PASTE; ev.paste_str = paste.c_str();
                st = gibson_push_event(ctx_, &ev);
            } else {
                ev.event_type = GIBSON_EVENT_RESIZE; ev.cols = static_cast<uint16_t>(num(0)); ev.rows = static_cast<uint16_t>(num(1));
                st = gibson_push_event(ctx_, &ev);
            }
            Rec().s("id", id).s("k", "push").s("st", st_name(st)).emit(out_);
        } else if (op == "poll") {
            for (int k = 1; k <= num(0); ++k) {
                gibson_event_t ev{};
                uint32_t kind = 0;
                int st = gibson_poll_event_kind(ctx_, 0, &ev, &kind);
                emit_event(id + "." + std::to_string(k), st, ev, kind_name(kind));
            }
        } else if (op == "poll_legacy") {
            gibson_event_t ev{};
            int st = gibson_poll_event(ctx_, 0, &ev);
            emit_event(id, st, ev, "ABSENT");
        } else if (op == "probe") {
            probe(id, arg(0));
        } else if (op == "lifecycle") {
            std::string st = "OK";
            if (arg(0) == "node_free_unattached") {
                gibson::Node n = gibson::Node::text("unattached", nullptr, GIBSON_WRAP_NONE);
                if (!n.raw()) st = "WRAPPER_NULL";
            }  // ~Node frees it
            else if (arg(0) == "destroy_null") gibson_destroy_context(nullptr);
            Rec().s("id", id).s("k", "lifecycle").s("name", arg(0)).s("st", st).emit(out_);
        } else if (op == "destroy") {
            gibson_destroy_context(ctx_);
            ctx_ = nullptr;
            Rec().s("id", id).s("k", "destroy").s("st", "OK").emit(out_);
        } else {
            Rec().s("id", id).s("k", "unknown_op").s("st", "UNSUPPORTED").emit(out_);
        }
    }
};

long maxrss_kb() {
    rusage ru{};
    getrusage(RUSAGE_SELF, &ru);
    return ru.ru_maxrss;
}

}  // namespace

int main(int argc, char** argv) {
    if (argc < 2) { std::cerr << "usage: parity_v08_cpp SCRIPT [REPS]\n"; return 2; }
    if (!gibson::Context::abi_compatible()) { std::cerr << "ABI mismatch\n"; return 2; }
    int reps = argc > 2 ? std::max(1, std::atoi(argv[2])) : 1;
    auto steps = load_script(argv[1]);
    if (steps.empty()) { std::cerr << "parity_v08_cpp: cannot read " << argv[1] << "\n"; return 2; }
    apply_perturbation(steps);

    // Wrapper-surface observation (stderr, not part of the comparable transcript).
    {
        gibson::Node bad = gibson::Node::text("\xff\xfe", nullptr, GIBSON_WRAP_NONE);
        using Ctx = gibson::Context;
        std::cerr << "{\"ctx_has_frame_cells\":" << has_frame_cells<Ctx>::value
                  << ",\"ctx_has_frame_size\":" << has_frame_size<Ctx>::value
                  << ",\"ctx_has_headless\":" << has_headless<Ctx>::value
                  << ",\"ctx_has_poll_event\":" << has_poll_event<Ctx>::value
                  << ",\"ctx_has_poll_event_kind\":" << has_poll_event_kind<Ctx>::value
                  << ",\"ctx_has_push_event\":" << has_push_event<Ctx>::value
                  << ",\"ctx_has_push_event_kind\":" << has_push_event_kind<Ctx>::value
                  << ",\"ctx_has_rendered_output\":" << has_rendered_output<Ctx>::value
                  << ",\"k\":\"wrapper\",\"lang\":\"cpp\",\"node_text_invalid_utf8_raw_is_null\":" << (bad.raw() ? 0 : 1)
                  << ",\"node_text_status_observable\":0}\n";
    }

    auto t0 = std::chrono::steady_clock::now();
    std::string first;
    { Interp in(first); in.run(steps); }
    long rss1 = maxrss_kb();
    uint64_t h1 = fnv(kFnvOffset, first.data(), first.size());
    long rssmid = rss1;
    int identical = 1;
    for (int rep = 2; rep <= reps; ++rep) {
        std::string b;
        { Interp in(b); in.run(steps); }
        if (b.size() != first.size() || fnv(kFnvOffset, b.data(), b.size()) != h1) identical = 0;
        if (rep == (reps + 1) / 2) rssmid = maxrss_kb();
    }
    long long wall = std::chrono::duration_cast<std::chrono::microseconds>(std::chrono::steady_clock::now() - t0).count();
    std::fwrite(first.data(), 1, first.size(), stdout);
    if (reps > 1)
        std::cerr << "{\"iter_identical\":" << identical << ",\"k\":\"resource\",\"lang\":\"cpp\",\"maxrss_kb_1\":" << rss1
                  << ",\"maxrss_kb_mid\":" << rssmid << ",\"maxrss_kb_n\":" << maxrss_kb() << ",\"per_rep_us\":" << wall / reps << ",\"reps\":" << reps
                  << ",\"wall_ms\":" << wall / 1000 << "}\n";
    return 0;
}
