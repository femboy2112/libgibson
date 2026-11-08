// Headless context, event push/poll, frame readback and capture drain through gibson.hpp
// (v0.8 parity slice, finding F5). The C++ twin of bindings/python/test_events.py.
//
//   g++ -std=c++17 -Wall -Wextra -Iinclude bindings/cpp/test_headless.cpp -Ltarget/release -lgibson -o t
//
// Exit 0 iff every check holds. Needs no terminal.
#include "gibson.hpp"

#include <cstdio>
#include <string>
#include <type_traits>
#include <utility>
#include <vector>

static int g_checks = 0;
static int g_failed = 0;

#define CHECK(...)                                                                  \
    do {                                                                            \
        ++g_checks;                                                                 \
        if (!(__VA_ARGS__)) { ++g_failed; std::printf("FAIL %s:%d  %s\n", __FILE__, __LINE__, #__VA_ARGS__); } \
    } while (0)

// Runs `stmt` (parenthesize it if it has top-level commas), requires it to throw `Exc`, and runs the
// trailing statements with the caught object bound to `var`.
#define EXPECT_THROW(stmt, Exc, var, ...)                                           \
    do {                                                                            \
        bool threw_ = false;                                                        \
        try { stmt; } catch (const Exc& var) { threw_ = true; __VA_ARGS__; }        \
        ++g_checks;                                                                 \
        if (!threw_) { ++g_failed; std::printf("FAIL %s:%d  %s did not throw %s\n", __FILE__, __LINE__, #stmt, #Exc); } \
    } while (0)

using gibson::Context;
using gibson::Error;
using gibson::Event;
using gibson::QueueFullError;

static Context headless(uint16_t cols = 40, uint16_t rows = 12) {
    return Context::headless(GIBSON_MODE_FULLSCREEN, cols, rows);
}

static void layout_and_constants() {
    static_assert(sizeof(gibson_event_t) == (sizeof(void*) == 8 ? 24 : 20), "gibson_event_t layout");
    CHECK(GIBSON_KEY_ENTER == 0x1001 && GIBSON_KEY_PAGE_DOWN == 0x100D);
    CHECK(GIBSON_EVENT_QUEUE_CAPACITY == 1024);
    // Existing API stays source-compatible: the two-argument poll_event still returns bool.
    static_assert(std::is_same<decltype(std::declval<Context&>().poll_event(0u, std::declval<gibson_event_t&>())), bool>::value,
                  "legacy poll_event(timeout, gibson_event_t&) -> bool");
}

static void empty_queue() {
    Context ctx = headless();
    Event e = ctx.poll_event_kind();
    CHECK(e.type == GIBSON_EVENT_NONE && e.has_kind && e.key_kind == GIBSON_KEY_KIND_NONE);
    CHECK(e.key_code == 0 && e.modifiers == 0 && e.cols == 0 && e.rows == 0 && !e.has_paste);
    Event k = ctx.poll_event(0);
    CHECK(k.type == GIBSON_EVENT_NONE && !k.has_kind);
    gibson_event_t legacy{};
    CHECK(!ctx.poll_event(0, legacy) && legacy.event_type == GIBSON_EVENT_NONE);
}

static void keys() {
    Context ctx = headless();
    ctx.push_event(Event::char_key('a', GIBSON_MOD_SHIFT | GIBSON_MOD_CONTROL));
    ctx.push_event(Event::char_key(0x1001));                 // a typed U+1001 (Myanmar letter)
    ctx.push_event(Event::special_key(GIBSON_KEY_ENTER));    // the constant with the same value
    ctx.push_event(Event::char_key(0x1F600, GIBSON_MOD_ALT));
    ctx.push_event(Event::legacy_key(0x1001));               // kind-less: always the special key
    ctx.push_event(Event::legacy_key('z'));
    Event g[6];
    for (auto& e : g) e = ctx.poll_event_kind();
    CHECK(g[0].type == GIBSON_EVENT_KEY && g[0].key_code == 'a' && g[0].key_kind == GIBSON_KEY_KIND_CHAR &&
          g[0].modifiers == (GIBSON_MOD_SHIFT | GIBSON_MOD_CONTROL) && g[0].has_kind);
    CHECK(g[1].key_code == 0x1001 && g[1].key_kind == GIBSON_KEY_KIND_CHAR);
    CHECK(g[2].key_code == 0x1001 && g[2].key_kind == GIBSON_KEY_KIND_SPECIAL);
    CHECK(g[1].key_code == g[2].key_code && g[1].key_kind != g[2].key_kind);
    CHECK(g[3].key_code == 0x1F600 && g[3].modifiers == GIBSON_MOD_ALT);
    CHECK(g[4].key_kind == GIBSON_KEY_KIND_SPECIAL && g[4].key_code == GIBSON_KEY_ENTER);
    CHECK(g[5].key_kind == GIBSON_KEY_KIND_CHAR);
    CHECK(ctx.poll_event_kind().type == GIBSON_EVENT_NONE);

    ctx.push_event(Event::char_key(0x1001));
    ctx.push_event(Event::special_key(GIBSON_KEY_ENTER));
    Event k1 = ctx.poll_event(0), k2 = ctx.poll_event(0);
    CHECK(k1.key_code == 0x1001 && k2.key_code == 0x1001 && !k1.has_kind && !k2.has_kind);  // conflated, per gibson.h
}

static void paste() {
    Context ctx = headless();
    const std::string text = "h\xc3\xa9llo \xe4\xb8\x96\xe7\x95\x8c\n\xf0\x9f\x98\x80";
    ctx.push_event(Event::paste_text(text));
    Event e = ctx.poll_event_kind();
    CHECK(e.type == GIBSON_EVENT_PASTE && e.has_paste && e.paste == text && e.key_kind == GIBSON_KEY_KIND_NONE);
    ctx.push_event(Event::paste_text(""));
    e = ctx.poll_event_kind();
    CHECK(e.has_paste && e.paste.empty());  // empty, not absent
    EXPECT_THROW(ctx.push_event(Event::paste_text("\xc3\x28")), Error, err,
                 CHECK(err.status() == GIBSON_ERR_INVALID_PARAM); CHECK(!err.last_error().empty()));
    Event none; none.type = GIBSON_EVENT_PASTE;  // has_paste false -> NULL paste_str
    EXPECT_THROW(ctx.push_event(none), Error, err2, CHECK(err2.status() == GIBSON_ERR_INVALID_PARAM));
    EXPECT_THROW((ctx.push_event(Event::paste_text(std::string("a\0b", 3)))), std::invalid_argument, ia, (void)ia);
    CHECK(ctx.poll_event_kind().type == GIBSON_EVENT_NONE);  // nothing refused was queued
}

static void resize() {
    Context ctx = headless(40, 12);
    gibson::Node root = gibson::Node::col();
    root.add_child(gibson::Node::text("x", nullptr, GIBSON_WRAP_NONE));
    ctx.set_root(std::move(root));
    ctx.render();
    CHECK(ctx.frame_size() == std::pair<uint16_t, uint16_t>(40, 12));
    ctx.push_event(Event::resize(60, 16));
    CHECK(ctx.frame_size() == std::pair<uint16_t, uint16_t>(40, 12));  // queued only
    Event e = ctx.poll_event_kind();
    CHECK(e.type == GIBSON_EVENT_RESIZE && e.cols == 60 && e.rows == 16 && e.key_kind == GIBSON_KEY_KIND_NONE);
    ctx.render();
    CHECK(ctx.frame_size() == std::pair<uint16_t, uint16_t>(60, 16));
    const uint16_t zeros[3][2] = {{0, 12}, {12, 0}, {0, 0}};
    for (auto& z : zeros) {
        EXPECT_THROW(ctx.push_event(Event::resize(z[0], z[1])), Error, err,
                     CHECK(err.status() == GIBSON_ERR_INVALID_PARAM));
    }
    CHECK(ctx.poll_event_kind().type == GIBSON_EVENT_NONE);
}

static void queue_full() {
    Context ctx = headless();
    for (int i = 0; i < GIBSON_EVENT_QUEUE_CAPACITY; ++i) ctx.push_event(Event::char_key('q'));
    EXPECT_THROW(ctx.push_event(Event::char_key('q')), QueueFullError, err,
                 CHECK(err.status() == GIBSON_ERR_QUEUE_FULL); CHECK(err.last_error().find("full") != std::string::npos));
    // QueueFullError is an Error is a std::runtime_error.
    EXPECT_THROW(ctx.push_event(Event::char_key('q')), std::runtime_error, re, (void)re);
    int n = 0;
    while (ctx.poll_event_kind().type != GIBSON_EVENT_NONE) ++n;
    CHECK(n == GIBSON_EVENT_QUEUE_CAPACITY);  // the refusals displaced nothing
    ctx.push_event(Event::char_key('r'));
    CHECK(ctx.poll_event_kind().key_code == 'r');
}

static void invalid_params() {
    Context ctx = headless();
    Event cases[4] = {Event::char_key(0xD800), Event::special_key('a'), Event::char_key('a'), Event::char_key('a', 0x80)};
    cases[2].key_kind = 9;  // an unknown kind
    for (const Event& ev : cases) {
        EXPECT_THROW(ctx.push_event(ev), Error, err,
                     CHECK(err.status() == GIBSON_ERR_INVALID_PARAM); CHECK(dynamic_cast<const QueueFullError*>(&err) == nullptr));
    }
    CHECK(ctx.poll_event_kind().type == GIBSON_EVENT_NONE);
    EXPECT_THROW((Context::headless(GIBSON_MODE_FULLSCREEN, 0, 12)), Error, e1, CHECK(e1.status() == GIBSON_ERR_INVALID_PARAM));
    EXPECT_THROW((Context::headless(static_cast<gibson_render_mode_t>(7), 40, 12)), Error, e2, CHECK(!e2.last_error().empty()));
    EXPECT_THROW(ctx.frame_cell(40, 0), Error, e3, CHECK(e3.status() == GIBSON_ERR_INVALID_PARAM));
}

static void push_back_unchanged() {
    Context ctx = headless();
    std::vector<Event> in = {Event::char_key('a', GIBSON_MOD_SHIFT), Event::char_key(0x1001),
                             Event::special_key(GIBSON_KEY_TAB, GIBSON_MOD_CONTROL), Event::paste_text("round\ntrip \xe4\xb8\x96"),
                             Event::resize(33, 9)};
    for (const Event& e : in) ctx.push_event(e);
    std::vector<Event> first, second;
    for (size_t i = 0; i < in.size(); ++i) first.push_back(ctx.poll_event_kind());
    for (const Event& e : first) ctx.push_event(e);  // as polled, nothing rebuilt
    for (size_t i = 0; i < in.size(); ++i) second.push_back(ctx.poll_event_kind());
    CHECK(first == second);
    CHECK(first[1].key_kind == GIBSON_KEY_KIND_CHAR && first[2].key_kind == GIBSON_KEY_KIND_SPECIAL);
    ctx.push_event(Event::char_key(0x1001));
    Event a = ctx.poll_event(0);
    ctx.push_event(a);
    CHECK(ctx.poll_event(0) == a);  // the kind-less loop too
}

static void readback_and_capture() {
    Context ctx = headless(20, 4);
    CHECK(ctx.frame_size() == std::pair<uint16_t, uint16_t>(0, 0));  // no frame yet
    CHECK(ctx.frame_cells().empty());
    gibson_style_t st{};
    st.fg.color_type = GIBSON_COLOR_RGB; st.fg.r = 10; st.fg.g = 20; st.fg.b = 30; st.bold = 1;
    // a 14-byte-capped inline glyph needs a long grapheme: ZWJ family (25 bytes)
    const std::string zwj = "\xf0\x9f\x91\xa8\xe2\x80\x8d\xf0\x9f\x91\xa9\xe2\x80\x8d\xf0\x9f\x91\xa7\xe2\x80\x8d\xf0\x9f\x91\xa6";
    gibson::Node root = gibson::Node::col();
    root.add_child(gibson::Node::text("ab", &st, GIBSON_WRAP_NONE));
    root.add_child(gibson::Node::text(zwj, nullptr, GIBSON_WRAP_NONE));
    ctx.set_root(std::move(root));
    ctx.render();
    auto sz = ctx.frame_size();
    CHECK(sz.first == 20 && sz.second == 4);
    gibson_cell_t c = ctx.frame_cell(0, 0);
    CHECK(c.glyph_len == 1 && c.glyph[0] == 'a' && c.style.bold && c.style.fg.color_type == GIBSON_COLOR_RGB && c.style.fg.g == 20);
    std::vector<gibson_cell_t> all = ctx.frame_cells();
    CHECK(all.size() == 80);
    CHECK(ctx.frame_cells(0, 3).size() == 20 && ctx.frame_cells(0, 0, 5).size() == 5);
    CHECK(ctx.frame_cells(0, 0, 5)[1].glyph[0] == 'b');
    gibson_cell_t z = ctx.frame_cell(0, 1);
    CHECK(z._reserved[0] == 1 && z.glyph_len < zwj.size());  // truncated inline
    CHECK(ctx.frame_cell_glyph(0, 1) == zwj && ctx.full_glyph(z, 0, 1) == zwj);
    CHECK(ctx.full_glyph(c, 0, 0) == "a");

    const std::string wire = ctx.rendered_output();
    CHECK(!wire.empty() && ctx.rendered_output() == wire);  // copy, not drain
    CHECK(ctx.take_rendered_output() == wire);
    CHECK(ctx.rendered_output().empty() && ctx.take_rendered_output().empty());
    ctx.request_render();
    ctx.render();
    size_t n = ctx.rendered_output().size();
    CHECK(ctx.clear_rendered_output() == n && ctx.clear_rendered_output() == 0);
}

static void lifecycle() {
    gibson_context_t* raw = nullptr;
    {
        Context a = headless();
        raw = a.raw();
        CHECK(raw != nullptr);
        Context b = std::move(a);  // ownership moves; a must not destroy
        CHECK(a.raw() == nullptr && b.raw() == raw);
        b.close();
        b.close();  // second close is a no-op
        CHECK(b.raw() == nullptr);
        Context c = headless();
        Context d = headless();
        d = std::move(c);  // move-assign destroys d's old context once
        CHECK(c.raw() == nullptr && d.raw() != nullptr);
    }  // destructors: a (empty), b (closed), c (empty), d (live): no double destroy (ASan/UBSan job)
}

int main() {
    layout_and_constants();
    empty_queue();
    keys();
    paste();
    resize();
    queue_full();
    invalid_params();
    push_back_unchanged();
    readback_and_capture();
    lifecycle();
    if (g_failed) {
        std::printf("%d of %d checks failed\n", g_failed, g_checks);
        return 1;
    }
    std::printf("test_headless: all %d checks passed\n", g_checks);
    return 0;
}
