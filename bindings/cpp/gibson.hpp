// LibGibson C++ wrapper — forwarding shim.
//
// The canonical, installable C++ header now lives at include/gibson.hpp and is
// staged into <prefix>/include/gibson.hpp by the native SDK (scripts/release/stage-sdk.sh).
// This shim keeps in-repository consumers that #include "gibson.hpp" from
// bindings/cpp working unchanged. New or installed consumers should instead use
//     #include <gibson.hpp>
// compiled against the SDK prefix (e.g. via `pkg-config --cflags libgibson`).
#include "../../include/gibson.hpp"
