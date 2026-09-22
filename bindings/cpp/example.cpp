#include "gibson.hpp"
#include <iostream>

int main() {
    std::cout << "--- Running C++ RAII Example with LibGibson ---" << std::endl;

    try {
        gibson::Context ctx(GIBSON_MODE_INLINE);

        // 1. Commit initial scrollback line
        ctx.commit("[C++ RAII] Initialized LibGibson context.");

        // 2. Build declarative UI tree using modern C++ RAII
        gibson_style_t header_style{};
        header_style.fg.color_type = GIBSON_COLOR_BRIGHT_MAGENTA;
        header_style.bold = 1;

        gibson_style_t border_style{};
        border_style.fg.color_type = GIBSON_COLOR_BRIGHT_YELLOW;

        gibson_style_t item_style{};
        item_style.fg.color_type = GIBSON_COLOR_WHITE;

        auto root = gibson::Node::col();
        root.width(70.0f);
        root.gap(1.0f);

        root.add_child(gibson::Node::text("C++ Modern RAII Interface", &header_style));

        auto box = gibson::Node::border_box(GIBSON_BORDER_ROUNDED, &border_style);
        box.width(60.0f);
        box.height(3.0f);
        box.add_child(gibson::Node::text("Clean RAII semantics with automatic resource management", &item_style));

        root.add_child(std::move(box));

        // 3. Render
        ctx.set_root(std::move(root));
        ctx.render();

        // 4. Commit output
        ctx.commit("[C++ RAII] Tree rendered and committed successfully.");

        // 5. Query stats
        auto stats = ctx.stats();
        std::cout << "[C++ RAII] Stats: frames = " << stats.frames
                  << ", bytes emitted = " << stats.bytes_emitted << std::endl;

        std::cout << "[C++ RAII] Test completed successfully." << std::endl;
    } catch (const std::exception& ex) {
        std::cerr << "C++ error: " << ex.what() << std::endl;
        return 1;
    }

    return 0;
}
