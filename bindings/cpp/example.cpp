#include "gibson.hpp"
#include <iostream>

int main() {
    std::cout << "--- Running C++ RAII Example with LibGibson ---" << std::endl;

    try {
        gibson::Context ctx(GIBSON_MODE_INLINE);

        // 1. Commit initial scrollback line
        ctx.commit("[C++ RAII] Initialized LibGibson context.");

        // 2. Build declarative UI tree using modern C++ RAII with rule & rail primitives
        gibson_style_t rule_style{};
        rule_style.fg.color_type = GIBSON_COLOR_BRIGHT_MAGENTA;
        rule_style.bold = 1;

        gibson_style_t rail_style{};
        rail_style.fg.color_type = GIBSON_COLOR_CYAN;

        gibson_style_t item_style{};
        item_style.fg.color_type = GIBSON_COLOR_WHITE;

        auto root = gibson::Node::col();
        root.percent_width(100.0f);
        root.max_width(76.0f);
        root.gap(1.0f);

        root.add_child(gibson::Node::rule("LibGibson C++ RAII Header", &rule_style));

        auto rail = gibson::Node::rail(&rail_style);
        rail.add_child(gibson::Node::text("C++ Modern RAII Interface with zero-cost abstractions & move semantics.", &item_style));
        root.add_child(std::move(rail));

        gibson_style_t border_style{};
        border_style.fg.color_type = GIBSON_COLOR_BRIGHT_YELLOW;

        auto box = gibson::Node::border_box(GIBSON_BORDER_ROUNDED, &border_style);
        box.width(64.0f);
        box.height(3.0f);
        box.add_child(gibson::Node::text("Clean RAII semantics with automatic resource management", &item_style));

        root.add_child(std::move(box));

        // 3. Render
        ctx.set_root(std::move(root));
        ctx.render();

        // 4. Language-neutral structured rich text
        {
            gibson_style_t label{};
            label.fg.color_type = GIBSON_COLOR_BRIGHT_YELLOW;
            label.dim = 1;
            gibson_style_t value{};
            value.fg.color_type = GIBSON_COLOR_WHITE;
            value.bold = 1;

            auto line = gibson::Line::create();
            line.add_span("RichText: ", &label);
            line.add_span("one line, many styles", &value);

            auto rich = gibson::RichText::create();
            rich.add_line(line);
            ctx.insert_rich_text_before_live(rich);
            ctx.commit_rich_text(rich);
        }

        // 5. Test insert_before_live
        ctx.insert_raw_lines_before_live_unchecked("[C++ RAII] Async notice inserted before active live region.");

        // 6. Commit output
        ctx.commit("[C++ RAII] Tree rendered and committed successfully.");

        // 7. Query stats (versioned struct)
        auto stats = ctx.stats();
        std::cout << "[C++ RAII] ABI v" << gibson::Context::abi_version()
                  << ", frames = " << stats.frames
                  << ", frame_bytes = " << stats.frame_bytes
                  << ", anchor_resyncs = " << stats.anchor_resyncs << std::endl;

        std::cout << "[C++ RAII] Test completed successfully." << std::endl;
    } catch (const std::exception& ex) {
        std::cerr << "C++ error: " << ex.what() << std::endl;
        return 1;
    }

    return 0;
}
