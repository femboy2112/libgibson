#!/usr/bin/env bash
# Summarize what this run actually established; success is not release qualification.
parity_qualification() {
    local controls=$1 rust=$2 pty=$3
    if [ "$controls" != PASS ] || [ "$rust" != WORKTREE_BUILD ] || [ "$pty" != PASS ]; then
        echo 'parity_v08: PARTIAL (skipped controls, unverified prebuilt Rust, or unavailable PTY)'
    else
        echo 'parity_v08: OK (relative parity only; SDK provenance and release gates unverified)'
    fi
}
