#!/usr/bin/env bash
# shellcheck disable=SC2016 # the sed scripts below are literal ('$a' appends), not expansions
#
# test-maintenance-checker.sh — calibration of check-maintenance-candidate.sh.
#
# The checker is a structural instrument; this proves it can tell a valid maintenance candidate
# from invalid ones, and that each invalid one fails for its intended reason and no other.
# Every negative candidate except the development one is the valid candidate plus one
# perturbation, built with `git commit-tree` in a throwaway shared clone, so the real
# repository gains no commit, ref or tag.
#
#   P1 valid candidate (+ allow-list, + built library)                   -> PASS
#   P2 valid candidate + a comment-only change to include/gibson.h       -> PASS (comments)
#   N1 a development candidate (0.7 ABI additions, 0.7.0-dev)            -> FAIL version, C ABI
#   N2 abi/gibson-abi-v1.symbols changed                                 -> FAIL abi/ only
#   N3 the candidate's tree on a parent that does not descend from the baseline -> FAIL ancestry
#   N4 allow-list: a commit missing / an extra commit / a bad class      -> FAIL commits only
#   N5 built library from another tree / Cargo.toml version / --release-version mismatch
#   N6 v0.6.0 (and bindings/go/v0.6.0) tag pointing at another commit    -> FAIL tag only
#   N7 include/gibson.h declaration change                               -> FAIL include/ only
#   N8 MSRV change; N9 frozen PCM producer change; N10 a new gibson_* export in src/;
#   N11 the candidate on no authorized branch                            -> FAIL that check only
#
# Usage:
#   scripts/release/tests/test-maintenance-checker.sh --baseline <rev> --candidate <rev> \
#       --allow-commits-file <list> --so <libgibson.so built from the candidate> \
#       --foreign-so <libgibson.so built from another tree> --dev-candidate <rev>
# Exit 0 iff every case has its expected exit status and exactly its expected FAIL lines.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
CHECKER="$REPO_ROOT/scripts/release/check-maintenance-candidate.sh"

baseline="" candidate="" allow="" so="" foreign_so="" dev=""
while [ $# -gt 0 ]; do
    case "$1" in
        --baseline) baseline="$2"; shift 2 ;;
        --candidate) candidate="$2"; shift 2 ;;
        --allow-commits-file) allow="$2"; shift 2 ;;
        --so) so="$2"; shift 2 ;;
        --foreign-so) foreign_so="$2"; shift 2 ;;
        --dev-candidate) dev="$2"; shift 2 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done
for v in baseline candidate allow so foreign_so dev; do
    [ -n "${!v}" ] || { echo "--${v//_/-} is required" >&2; exit 2; }
done
allow="$(cd "$(dirname "$allow")" && pwd)/$(basename "$allow")"
so="$(cd "$(dirname "$so")" && pwd)/$(basename "$so")"
foreign_so="$(cd "$(dirname "$foreign_so")" && pwd)/$(basename "$foreign_so")"

B="$(git -C "$REPO_ROOT" rev-parse --verify "$baseline^{commit}")" || exit 2
C="$(git -C "$REPO_ROOT" rev-parse --verify "$candidate^{commit}")" || exit 2
D="$(git -C "$REPO_ROOT" rev-parse --verify "$dev^{commit}")" || exit 2

WORK="$(mktemp -d "${TMPDIR:-/tmp}/libgibson-maint-checker-test.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
git clone -q --shared --no-checkout "$REPO_ROOT" "$WORK/repo"
mkdir -p "$WORK/repo/scripts/release"
cp "$CHECKER" "$WORK/repo/scripts/release/"
R="$WORK/repo"
g() { git -C "$R" "$@"; }
# Nothing from the source repository's refs may decide a case: drop every ref, then add per case.
g for-each-ref --format='%(refname)' | while read -r ref; do g update-ref -d "$ref"; done

# perturb <base> <path> <sed-expression> <message>: a commit whose tree is <base>'s with <path>
# edited by sed, parented on <base>. Prints the new commit id.
perturb() {
    local base="$1" path="$2" expr="$3" msg="$4" blob tree idx="$WORK/index.$RANDOM"
    blob="$(g show "$base:$path" | sed -e "$expr" | g hash-object -w --stdin)"
    GIT_INDEX_FILE="$idx" g read-tree "$base"
    GIT_INDEX_FILE="$idx" g update-index --cacheinfo "100644,$blob,$path"
    tree="$(GIT_INDEX_FILE="$idx" g write-tree)"
    rm -f "$idx"
    if [ "$(g rev-parse "$base^{tree}")" = "$tree" ]; then echo "perturb: '$expr' did not change $path" >&2; exit 2; fi
    g commit-tree "$tree" -p "$base" -m "$msg"
}
with_extra() { cp "$allow" "$WORK/allow.$1"; echo "$2 release-engineering calibration perturbation" >>"$WORK/allow.$1"; echo "$WORK/allow.$1"; }

failures=0
# case <name> <expected-exit> <expected-FAIL-regexes, '|'-joined or '-' for none> -- <checker args>
# (the patterns are split on '|', so a single pattern must not use alternation)
case_() {
    local name="$1" want_exit="$2" want_fails="$3"; shift 4
    local out got_exit fails n_fail n_want ok=1
    out="$(cd "$R" && scripts/release/check-maintenance-candidate.sh "$@" 2>&1)"; got_exit=$?
    fails="$(printf '%s\n' "$out" | grep '^FAIL' || true)"
    n_fail="$(printf '%s\n' "$fails" | grep -c . || true)"
    [ "$got_exit" = "$want_exit" ] || ok=0
    if [ "$want_fails" = "-" ]; then
        [ "$n_fail" -eq 0 ] || ok=0
    else
        n_want=0
        IFS='|' read -ra res <<<"$want_fails"
        for re in "${res[@]}"; do
            n_want=$((n_want + 1))
            printf '%s\n' "$fails" | grep -qE "$re" || { ok=0; echo "   missing expected FAIL /$re/"; }
        done
        # exactly the intended failures: every FAIL line matches one of the expected patterns
        while IFS= read -r line; do
            [ -n "$line" ] || continue
            local matched=0
            for re in "${res[@]}"; do printf '%s\n' "$line" | grep -qE "$re" && matched=1; done
            [ "$matched" = 1 ] || { ok=0; echo "   unexpected: $line"; }
        done <<<"$fails"
    fi
    if [ "$ok" = 1 ]; then echo "ok    $name (exit $got_exit, $n_fail FAIL line(s))"; else
        echo "WRONG $name (exit $got_exit, want $want_exit)"; failures=$((failures + 1)); fi
    printf '%s\n' "$out" | sed 's/^/      | /'
}
on_branch() { g update-ref -d refs/heads/release/v0.6.0-rc 2>/dev/null || true; g update-ref refs/heads/release/v0.6.0-rc "$1"; }
no_tags() { g tag -d v0.6.0 bindings/go/v0.6.0 >/dev/null 2>&1 || true; }

echo "calibration: baseline $B, candidate $C, dev $D"
echo "calibration: --so $so (sha256 $(sha256sum "$so" | cut -d' ' -f1))"
echo "calibration: --foreign-so $foreign_so (sha256 $(sha256sum "$foreign_so" | cut -d' ' -f1))"
echo "calibration: checker sha256 $(sha256sum "$CHECKER" | cut -d' ' -f1)"
A=(--baseline "$B")

on_branch "$C"; no_tags
case_ "P1 valid candidate" 0 - -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow" --so "$so"

P2="$(perturb "$C" include/gibson.h '0,/^\/\*/s//\/* calibration: a comment-only edit. *\/\n\/*/' 'calibration: comment-only header change')"
on_branch "$P2"
case_ "P2 comment-only header change" 0 - -- "${A[@]}" --candidate "$P2" --allow-commits-file "$(with_extra p2 "$P2")"

on_branch "$D"
case_ "N1 development candidate" 1 '^FAIL  version: |^FAIL  C ABI: src/ at the candidate exports|^FAIL  C ABI: abi/gibson-abi-v1.symbols lists|^FAIL  C ABI: abi/ differs|^FAIL  C ABI: include/ declarations differ' -- "${A[@]}" --candidate "$D"

N2="$(perturb "$C" abi/gibson-abi-v1.symbols '$a gibson_calibration_extra' 'calibration: abi baseline change')"
on_branch "$N2"
case_ "N2 changed ABI symbol baseline" 1 '^FAIL  C ABI: abi/gibson-abi-v1.symbols lists|^FAIL  C ABI: abi/ differs' -- "${A[@]}" --candidate "$N2" --allow-commits-file "$(with_extra n2 "$N2")"

N3="$(g commit-tree "$(g rev-parse "$C^{tree}")" -p "$(g rev-parse "$B^1")" -m 'calibration: the candidate tree, not descended from the baseline')"
on_branch "$N3"
case_ "N3 not descended from the baseline" 1 '^FAIL  ancestry: ' -- "${A[@]}" --candidate "$N3"

on_branch "$C"
grep -v '^#' "$allow" | grep . | sed '$d' >"$WORK/allow.missing"
case_ "N4a allow-list missing a commit" 1 '^FAIL  commits: [0-9a-f]{7} is not in the allow-list' -- "${A[@]}" --candidate "$C" --allow-commits-file "$WORK/allow.missing"
{ cat "$allow"; echo "$D backport not in the range"; } >"$WORK/allow.extra"
case_ "N4b allow-list with an extra commit" 1 '^FAIL  commits: allow-list entry [0-9a-f]{7} is not in' -- "${A[@]}" --candidate "$C" --allow-commits-file "$WORK/allow.extra"
sed '0,/ backport / s// feature /' "$allow" >"$WORK/allow.class"
case_ "N4c allow-list with a bad class" 1 "^FAIL  commits: allow-list entry [0-9a-f]{7} has class 'feature'" -- "${A[@]}" --candidate "$C" --allow-commits-file "$WORK/allow.class"

case_ "N5a built library from another tree" 1 '^FAIL  C ABI: the built library exports' -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow" --so "$foreign_so"
N5="$(perturb "$C" Cargo.toml '0,/^version = "0.6.0"$/s//version = "0.6.1"/' 'calibration: manifest version mismatch')"
on_branch "$N5"
case_ "N5b Cargo.toml version mismatch" 1 '^FAIL  version: candidate Cargo.toml' -- "${A[@]}" --candidate "$N5" --allow-commits-file "$(with_extra n5 "$N5")"
g update-ref refs/heads/release/v0.6.1-rc "$C"
case_ "N5c release version mismatch" 1 '^FAIL  version: candidate [^=]+ = .0\.6\.0., expected 0\.6\.1' -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow" --release-version 0.6.1
g update-ref -d refs/heads/release/v0.6.1-rc

on_branch "$C"; g tag -a v0.6.0 -m calibration "$B"
case_ "N6a v0.6.0 tag on another commit" 1 '^FAIL  tag: v0\.6\.0 exists and peels to' -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow"
no_tags; g tag -a bindings/go/v0.6.0 -m calibration "$B"
case_ "N6b bindings/go/v0.6.0 tag on another commit" 1 '^FAIL  tag: bindings/go/v0\.6\.0 exists and peels to' -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow"
no_tags; g tag -a v0.6.0 -m calibration "$C"; g tag -a bindings/go/v0.6.0 -m calibration "$C"
case_ "P3 both tags on the candidate" 0 - -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow" --so "$so"
no_tags

N7="$(perturb "$C" include/gibson.h 's/uint8_t \*buf, size_t cap, size_t \*out_len);$/uint8_t *buf, uint32_t cap, size_t *out_len);/' 'calibration: header declaration change')"
on_branch "$N7"
case_ "N7 header declaration change" 1 '^FAIL  C ABI: include/ declarations differ' -- "${A[@]}" --candidate "$N7" --allow-commits-file "$(with_extra n7 "$N7")"
N8="$(perturb "$C" Cargo.toml 's/^rust-version = "1\.85"$/rust-version = "1.86"/' 'calibration: MSRV change')"
on_branch "$N8"
case_ "N8 MSRV change" 1 '^FAIL  MSRV: ' -- "${A[@]}" --candidate "$N8" --allow-commits-file "$(with_extra n8 "$N8")"
pcm_file="$(g ls-tree -r --name-only "$C" -- src/audio | grep '\.rs$' | head -1)"
N9="$(perturb "$C" "$pcm_file" '$a // calibration' 'calibration: PCM producer change')"
on_branch "$N9"
case_ "N9 frozen PCM producer change" 1 '^FAIL  PCM golden: ' -- "${A[@]}" --candidate "$N9" --allow-commits-file "$(with_extra n9 "$N9")"
N10="$(perturb "$C" src/ffi.rs '$a #[no_mangle]\npub extern "C" fn gibson_calibration_extra() {}' 'calibration: new export')"
on_branch "$N10"
case_ "N10 a new gibson_* export in src/" 1 '^FAIL  C ABI: src/ at the candidate exports' -- "${A[@]}" --candidate "$N10" --allow-commits-file "$(with_extra n10 "$N10")"
g update-ref -d refs/heads/release/v0.6.0-rc
case_ "N11 candidate on no authorized branch" 1 '^FAIL  branch: ' -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow"

echo
if [ "$failures" -eq 0 ]; then echo "CALIBRATION: PASS — every case met its expected verdict for its intended reason"; exit 0; fi
echo "CALIBRATION: FAIL — $failures case(s) wrong" >&2
exit 1
