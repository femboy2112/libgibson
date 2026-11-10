#!/usr/bin/env bash
# Helpers for removing only directories created by the parity harness itself.

parity_directory_token() {
    python3 - "$1" <<'PY'
import os, stat, sys
s = os.lstat(sys.argv[1])
if not stat.S_ISDIR(s.st_mode):
    raise SystemExit("not a directory")
print(f"{s.st_dev}:{s.st_ino}")
PY
}

# EXIT trap: retain the original exit status unless cleanup itself fails after success.
parity_cleanup() {
    local status=$? actual
    trap - EXIT
    if [ -n "${GO_CACHE:-}" ]; then
        actual=$(parity_directory_token "$GO_CACHE" 2>/dev/null || true)
        if [ -n "${GO_CACHE_TOKEN:-}" ] && [ "$actual" = "$GO_CACHE_TOKEN" ]; then
            # The root's device/inode is the one returned by our mktemp; find does not follow links.
            if ! find -P "$GO_CACHE" -depth -delete; then
                echo "parity_v08: could not remove its temporary Go cache: $GO_CACHE" >&2
                [ "$status" -ne 0 ] || status=1
            fi
        elif [ -e "$GO_CACHE" ] || [ -L "$GO_CACHE" ]; then
            echo "parity_v08: temporary Go cache identity changed; preserved: $GO_CACHE" >&2
            [ "$status" -ne 0 ] || status=1
        fi
    fi
    if [ -n "${SDK_ALIAS_DIR:-}" ]; then
        actual=$(parity_directory_token "$SDK_ALIAS_DIR" 2>/dev/null || true)
        if [ -n "${SDK_ALIAS_TOKEN:-}" ] && [ "$actual" = "$SDK_ALIAS_TOKEN" ]; then
            if [ -L "$SDK_ALIAS_DIR/sdk" ]; then
                rm -- "$SDK_ALIAS_DIR/sdk" || { [ "$status" -ne 0 ] || status=1; }
            fi
            if ! rmdir -- "$SDK_ALIAS_DIR"; then
                echo "parity_v08: could not remove its SDK alias: $SDK_ALIAS_DIR" >&2
                [ "$status" -ne 0 ] || status=1
            fi
        elif [ -e "$SDK_ALIAS_DIR" ] || [ -L "$SDK_ALIAS_DIR" ]; then
            echo "parity_v08: SDK alias identity changed; preserved: $SDK_ALIAS_DIR" >&2
            [ "$status" -ne 0 ] || status=1
        fi
    fi
    exit "$status"
}
