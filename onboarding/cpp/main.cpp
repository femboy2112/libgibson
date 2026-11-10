#include <gibson.hpp>
#include <iostream>
#include <utility>

int main() {
    try {
        auto ctx = gibson::Context::headless(GIBSON_MODE_FULLSCREEN, 40, 8);
        auto root = gibson::Node::col();
        root.add_child(gibson::Node::text("Hello, LibGibson", nullptr, GIBSON_WRAP_NONE));
        ctx.set_root(std::move(root));
        ctx.render();
        const auto size = ctx.frame_size();
        if (size.first != 40 || size.second != 8 || ctx.frame_cell_glyph(0, 0) != "H") return 1;
        std::cout << "C++ headless frame " << size.first << "x" << size.second << " OK\n";
    } catch (const std::exception& e) {
        std::cerr << e.what() << '\n';
        return 1;
    }
}
