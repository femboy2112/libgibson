#include "gibson.hpp"
#include <iostream>
#include <string>
#include <fcntl.h>
#include <unistd.h>
#include <dirent.h>
#include <sys/stat.h>
#include <cstdlib>

// Since #371 a session writes its own duplicate of the terminal it acquired, not fd 1 as it is
// later. To make that write fail, point every open descriptor for the same terminal device as
// fd 1 (fd 0 excepted) at /dev/full. Returns false if nothing could be redirected.
static bool redirect_terminal_writes_to_full() {
    struct stat out {};
    if (::fstat(STDOUT_FILENO, &out) != 0 || !S_ISCHR(out.st_mode)) return false;
    int full = ::open("/dev/full", O_WRONLY | O_CLOEXEC);
    if (full < 0) return false;
    int fds[256]; int n = 0;
    if (DIR* d = ::opendir("/proc/self/fd")) {
        while (dirent* e = ::readdir(d)) {
            int fd = std::atoi(e->d_name);
            if (fd > 0 && fd != full && n < 256) fds[n++] = fd;
        }
        ::closedir(d);
    }
    int moved = 0;
    for (int i = 0; i < n; ++i) {
        struct stat st {};
        if (::fstat(fds[i], &st) == 0 && S_ISCHR(st.st_mode) && st.st_rdev == out.st_rdev) {
            int flags = ::fcntl(fds[i], F_GETFD);
            if (::dup3(full, fds[i], (flags & FD_CLOEXEC) ? O_CLOEXEC : 0) >= 0) ++moved;
        }
    }
    ::close(full);
    return moved > 1;  // fd 1 and the session's own descriptor
}

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
            if (!redirect_terminal_writes_to_full()) return 2;
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
