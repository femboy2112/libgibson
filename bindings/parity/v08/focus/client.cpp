#include "gibson.hpp"
#include <iostream>
#include <string>
#include <fcntl.h>
#include <unistd.h>

int main(int argc, char** argv) {
    const std::string mode = argc > 1 ? argv[1] : "normal";
    try {
        gibson::Context ctx;
        if (mode == "events") {
            if (ctx.poll_event_kind(0).type != GIBSON_EVENT_NONE) return 6;
            ctx.set_focus_reporting(true);  // The PTY peer sends two input sequences after this.
            const auto first = ctx.poll_event_kind(2000);
            const auto second = ctx.poll_event_kind(2000);
            const auto extra = ctx.poll_event_kind(0);
            if (first.type != GIBSON_EVENT_FOCUS_GAINED || second.type != GIBSON_EVENT_FOCUS_LOST ||
                extra.type != GIBSON_EVENT_NONE) {
                std::cerr << "FOCUS_ORDER_MISMATCH\n";
                return 5;
            }
            ctx.close();
            std::cerr << "FOCUS_INPUT_OK\n";
            return 0;
        }
        if (mode == "io_error") {
            int fd = ::open("/dev/full", O_WRONLY);
            if (fd < 0 || ::dup2(fd, STDOUT_FILENO) < 0) return 2;
            ::close(fd);
            try {
                ctx.set_focus_reporting(true);
            } catch (const gibson::Error& e) {
                if (e.status() == GIBSON_ERR_IO && e.last_error().find("os error 28") != std::string::npos) {
                    std::cerr << "EXPECTED_IO_ERROR\n";
                    return 0;
                }
            }
            return 3;
        }
        auto enable = [&] { if (mode != "noop") ctx.set_focus_reporting(true); };
        enable(); enable();
        ctx.set_focus_reporting(false); ctx.set_focus_reporting(false);
        enable();
        ctx.close();
        std::cerr << "COMPLETE\n";
        return 0;
    } catch (const std::exception& e) {
        std::cerr << e.what() << '\n';
        return 4;
    }
}
