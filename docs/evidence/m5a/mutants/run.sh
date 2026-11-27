#!/usr/bin/env bash
# Reproduce the M5-A wrong-value REDs. Run from the repo root on a clean checkout of the
# commit the patches were cut against (e7d2896 or a descendant with an unchanged
# src/stage/motion.rs). Each run must exit non-zero, and every panic site must be in
# tests/, never in src/. The tree is restored after each run.
# This builds and tests code; on a shared machine, run it under your build lock.
set -u
dir=docs/evidence/m5a/mutants
for p in "$dir"/*.patch; do
  m=$(basename "$p" .patch)
  git apply "$p" || { echo "$m: patch does not apply"; exit 2; }
  out=$(cargo +1.98.1 test --test stage_motion 2>&1); rc=$?
  git checkout -- src/stage/motion.rs
  echo "== $m exit=$rc"
  grep -E "^test result" <<<"$out"
  echo "panic sites: tests/=$(grep -c 'panicked at tests/' <<<"$out") src/=$(grep -c 'panicked at src/' <<<"$out")"
done
