# Acid vs Crash: validation record

Starting authority: local and `git ls-remote origin refs/heads/main` both returned
`1b4358e91df108cdf0e0dbb92e886f010a8b7eb9`. Working tree was clean.
Development branch: `codex/acid-vs-crash-cinematic`; no merge authorized.

## Baseline, 2026-09-23

| Command | Result |
| --- | --- |
| `cargo fmt --check` | exit 0 |
| `cargo clippy --all-targets --all-features -- -D warnings` | exit 0 |
| `cargo test` | exit 0; 226 unit + 136 integration = 362 tests |
| `cargo build --release` | exit 0 |

Local raw logs for this run: `/tmp/libgibson-acid-audit/baseline-*.log`.
These paths are run-local receipts, not portable repository artifacts.
Remote Actions were reported billing-blocked at task intake; no remote success
is inferred from these local checks.

## Evidence boundaries

The encounter is entirely fictional and offline. Scene and story APIs remain
experimental and Rust-only. Automated cell/PTY evidence does not certify
subjective cinematic impact or every terminal emulator.
