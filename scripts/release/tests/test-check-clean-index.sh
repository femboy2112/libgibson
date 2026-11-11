#!/usr/bin/env bash
# No-compiler controls for flags that hide tracked changes from git status.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
work=$(mktemp -d "${TMPDIR:-/tmp}/libgibson-index-test.XXXXXXXX")
echo "index checker fixtures retained: $work"
mkdir -p "$work/repo/scripts/release"
cp "$root/scripts/release/check-clean-index.py" "$work/repo/scripts/release/"
cd "$work/repo"
git init -q
printf 'committed\n' > tracked
git add tracked scripts/release/check-clean-index.py
git -c user.name='ABI fixture' -c user.email='abi-fixture@example.invalid' \
    -c commit.gpgsign=false -c core.hooksPath=/dev/null commit -qm fixture
checker=scripts/release/check-clean-index.py
"$checker"
git update-index --assume-unchanged tracked
printf 'hidden assume-unchanged\n' > tracked
[[ -z $(git status --porcelain --untracked-files=all) ]]
if "$checker"; then echo 'FAIL: assume-unchanged accepted' >&2; exit 1; fi
git update-index --no-assume-unchanged tracked
git restore tracked
git update-index --skip-worktree tracked
printf 'hidden skip-worktree\n' > tracked
[[ -z $(git status --porcelain --untracked-files=all) ]]
if "$checker"; then echo 'FAIL: skip-worktree accepted' >&2; exit 1; fi
git update-index --no-skip-worktree tracked
git restore tracked
"$checker"
echo 'PASS: normal index accepted; assume-unchanged and skip-worktree rejected'
