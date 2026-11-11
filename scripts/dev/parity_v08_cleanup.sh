#!/usr/bin/env bash
# Report temporary paths without deleting through a pathname that could have been replaced.
parity_cleanup() {
    local status=$?
    trap - EXIT
    if [ -n "${GO_CACHE:-}" ]; then
        printf 'parity_v08: retained temporary Go cache path for audited cleanup: %s\n' "$GO_CACHE" >&2
    fi
    if [ -n "${SDK_ALIAS_DIR:-}" ]; then
        printf 'parity_v08: retained temporary SDK alias path for audited cleanup: %s\n' "$SDK_ALIAS_DIR" >&2
    fi
    exit "$status"
}
