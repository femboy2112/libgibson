#!/usr/bin/env bash
# Regenerate deterministic visual goldens. Review the diff before committing.
set -euo pipefail
cd "$(dirname "$0")/../.."
UPDATE_GOLDENS=1 cargo test --test visual_goldens -- --nocapture
echo
echo "Goldens updated under tests/goldens/. Review with: git diff -- tests/goldens"
