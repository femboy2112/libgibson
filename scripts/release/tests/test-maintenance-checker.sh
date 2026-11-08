#!/usr/bin/env bash
# shellcheck disable=SC2016 # the sed scripts below are literal ('$a' appends), not expansions
#
# test-maintenance-checker.sh — calibration of check-maintenance-candidate.sh.
#
# The checker is a structural instrument; this proves it tells a valid maintenance candidate from
# invalid ones, and that each invalid one fails for its intended reason and no other. Every
# negative candidate except the development one and the baseline itself is the valid candidate
# plus one perturbation, built with `git commit-tree` in a throwaway shared clone; the real
# repository gains no commit, ref or tag (every GIT_* variable is cleared first, and the clone's
# git directory is verified before any ref in it is touched).
#
#   P1 valid candidate (+ allow-list, + built library)                 -> PASS
#   P2 + a comment-only change to include/gibson.h                     -> PASS
#   P3 both release tags on the candidate                              -> PASS (both tag PASS lines)
#   P4 + a doc-comment-only change to src/ffi.rs                       -> PASS
#   P5 the allow-list with CRLF line ends and two-column entries       -> PASS
#   P6 a library exporting exactly the baseline's names (synthetic)    -> PASS
#   N1 a development candidate (0.7 ABI additions, 0.7.0-dev)          -> FAIL version, C ABI
#   N2 abi/gibson-abi-v1.symbols changed                               -> FAIL abi/ only
#   N3 the candidate's tree on a parent that does not descend from the baseline -> FAIL ancestry
#   N4 allow-list: a commit missing / an extra commit / a bad class    -> FAIL commits only
#   N5 built library from another tree / Cargo.toml version / --release-version mismatch
#   N6 v0.6.0 or bindings/go/v0.6.0 tag on another commit             -> FAIL tag only
#   N7 include/gibson.h declaration change                             -> FAIL include/ only
#   N8 MSRV; N9 frozen PCM producer; N10 a new gibson_* export; N11 no authorized branch
#   N12 a macro redefinition hidden in a comment by line splices       -> FAIL include/ only
#   N13 whitespace changed inside a string literal in gibson.hpp       -> FAIL include/ only
#   N14 a comment spliced by a trigraph                                -> FAIL include/ only
#   N15 two #[repr(C)] fields swapped; N16 a parameter type changed;
#   N21 a GIBSON_* const value changed                                 -> FAIL Rust surface only
#   N17 a new export whose `fn` is on the line after `extern "C"`;
#   N18 a new exported static                                          -> FAIL exports + surface
#   N19 the baseline itself as the candidate                           -> FAIL ancestry only
#   N20 the candidate only on feature/release/v0.6.0-rc                -> FAIL branch only
#   N22 a library that also exports a gibson_* data symbol (synthetic) -> FAIL built library only
#   N23 a src/ file whose blob is missing from the object store        -> FAIL closed (surface, exports)
#   N24 a splice followed by 6000 padding comments (a SIGPIPE probe)   -> FAIL include/ only
#   N25 #[cfg] on an export; N26 generics on an export; N28 a type alias in the ABI module
#                                                                      -> FAIL Rust surface only
#   N27 an export through #[export_name]                               -> FAIL exports + surface
#   N29 the layout oracle (tests/ffi_abi_layout_freeze.rs) edited      -> FAIL layout oracle only
#   P7 a new non-exporting #[cfg(unix)] module (a legitimate backport)  -> PASS
#   P8 a #[cfg(test)] module with imports in src/ffi.rs                -> PASS
#   P9 a Cargo.toml with 20000 comment lines (no reader exits early)    -> PASS
#   F1-F4 a tool dies after (part of) its output: the header read (both revisions truncated to
#   one common prefix), header enumeration, nm, branch enumeration    -> FAIL tools (+ that check)
#
# Usage:
#   scripts/release/tests/test-maintenance-checker.sh --baseline <rev> --candidate <rev> \
#       --allow-commits-file <list> --so <libgibson.so built from the candidate> \
#       --foreign-so <libgibson.so built from another tree> --dev-candidate <rev>
# Exit 0 iff every case has its expected exit status, exactly its expected FAIL lines, and any
# PASS lines it requires; 2 on a setup error.

set -uo pipefail
while IFS= read -r v; do unset "$v"; done < <(compgen -e | grep '^GIT_' || true)

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
CHECKER="$REPO_ROOT/scripts/release/check-maintenance-candidate.sh"
SURFACE="$REPO_ROOT/scripts/release/maintenance_surface.py"
die() { echo "test-maintenance-checker: $*" >&2; exit 2; }

baseline="" candidate="" allow="" so="" foreign_so="" dev=""
while [ $# -gt 0 ]; do
    case "$1" in
        --baseline) baseline="$2"; shift 2 ;;
        --candidate) candidate="$2"; shift 2 ;;
        --allow-commits-file) allow="$2"; shift 2 ;;
        --so) so="$2"; shift 2 ;;
        --foreign-so) foreign_so="$2"; shift 2 ;;
        --dev-candidate) dev="$2"; shift 2 ;;
        *) die "unknown argument: $1" ;;
    esac
done
for v in baseline candidate allow so foreign_so dev; do
    [ -n "${!v}" ] || die "--${v//_/-} is required"
done
abspath() { (cd "$(dirname "$1")" && printf '%s/%s\n' "$(pwd)" "$(basename "$1")"); }
allow="$(abspath "$allow")" so="$(abspath "$so")" foreign_so="$(abspath "$foreign_so")"
command -v cc >/dev/null || die "cc is required"

B="$(git -C "$REPO_ROOT" rev-parse --verify "$baseline^{commit}")" || die "bad --baseline"
C="$(git -C "$REPO_ROOT" rev-parse --verify "$candidate^{commit}")" || die "bad --candidate"
D="$(git -C "$REPO_ROOT" rev-parse --verify "$dev^{commit}")" || die "bad --dev-candidate"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/libgibson-maint-checker-test.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
R="$WORK/repo"
git clone -q --shared --no-checkout "$REPO_ROOT" "$R" || die "clone failed"
[ "$(git -C "$R" rev-parse --absolute-git-dir)" = "$R/.git" ] || die "the clone's git directory is not $R/.git; refusing to touch refs"
mkdir -p "$R/scripts/release"
cp "$CHECKER" "$SURFACE" "$R/scripts/release/" || die "copy failed"
g() { git -C "$R" "$@"; }
# Nothing from the source repository's refs may decide a case: drop every ref, then add per case.
g for-each-ref --format='%(refname)' | while read -r ref; do g update-ref -d "$ref" || exit 1; done || die "ref cleanup failed"
g remote remove origin 2>/dev/null || true
[ -z "$(g for-each-ref)" ] || die "refs remain in the clone"

# commit_with <base> <path> <blob> <message>: <base> with <path> replaced by <blob>.
commit_with() {
    local base="$1" path="$2" blob="$3" msg="$4" tree idx="$WORK/index.$RANDOM"
    GIT_INDEX_FILE="$idx" g read-tree "$base" || return 1
    GIT_INDEX_FILE="$idx" g update-index --cacheinfo "100644,$blob,$path" || return 1
    tree="$(GIT_INDEX_FILE="$idx" g write-tree)" || return 1
    rm -f "$idx"
    if [ "$(g rev-parse "$base^{tree}")" = "$tree" ]; then echo "perturbation did not change $path" >&2; return 1; fi
    g commit-tree "$tree" -p "$base" -m "$msg"
}
# perturb <base> <path> <sed-expression> <message>
perturb() {
    local blob
    blob="$(g show "$1:$2" | sed -e "$3" | g hash-object -w --stdin)" || return 1
    commit_with "$1" "$2" "$blob" "$4"
}
# replace <base> <path> <old> <new> <message>: <old> must occur exactly once in <path>.
replace() {
    local blob
    blob="$(g show "$1:$2" | python3 -I -c '
import sys
old, new = sys.argv[1], sys.argv[2]
s = sys.stdin.read()
if s.count(old) != 1:
    sys.exit("replace: %d occurrences" % s.count(old))
sys.stdout.write(s.replace(old, new))' "$3" "$4" | g hash-object -w --stdin)" || return 1
    commit_with "$1" "$2" "$blob" "$5"
}
with_extra() { cp "$allow" "$WORK/allow.$1" && echo "$2 release-engineering calibration perturbation" >>"$WORK/allow.$1" && echo "$WORK/allow.$1"; }

failures=0 REQUIRE_PASS="" CASE_ENV=()
# case_ <name> <expected-exit> <expected-FAIL-regexes, '|'-joined or '-' for none> -- <checker args>
# The patterns are split on '|', so a single pattern must not use alternation. REQUIRE_PASS, if
# set, is a '|'-joined list of PASS-line regexes the output must also contain.
case_() {
    local name="$1" want_exit="$2" want_fails="$3"; shift 4
    local out got_exit fails n_fail ok=1 re line matched
    out="$(cd "$R" && env "${CASE_ENV[@]}" scripts/release/check-maintenance-candidate.sh "$@" 2>&1)"; got_exit=$?
    fails="$(printf '%s\n' "$out" | grep '^FAIL' || true)"
    n_fail="$(printf '%s\n' "$fails" | grep -c . || true)"
    [ "$got_exit" = "$want_exit" ] || ok=0
    if [ "$want_fails" = "-" ]; then
        [ "$n_fail" -eq 0 ] || ok=0
    else
        IFS='|' read -ra res <<<"$want_fails"
        for re in "${res[@]}"; do
            printf '%s\n' "$fails" | grep -qE "$re" || { ok=0; echo "   missing expected FAIL /$re/"; }
        done
        while IFS= read -r line; do
            [ -n "$line" ] || continue
            matched=0
            for re in "${res[@]}"; do printf '%s\n' "$line" | grep -qE "$re" && matched=1; done
            [ "$matched" = 1 ] || { ok=0; echo "   unexpected: $line"; }
        done <<<"$fails"
    fi
    if [ -n "$REQUIRE_PASS" ]; then
        IFS='|' read -ra res <<<"$REQUIRE_PASS"
        for re in "${res[@]}"; do
            printf '%s\n' "$out" | grep -qE "^PASS  $re" || { ok=0; echo "   missing required PASS /$re/"; }
        done
    fi
    REQUIRE_PASS="" CASE_ENV=()
    if [ "$ok" = 1 ]; then echo "ok    $name (exit $got_exit, $n_fail FAIL line(s))"; else
        echo "WRONG $name (exit $got_exit, want $want_exit)"; failures=$((failures + 1)); fi
    printf '%s\n' "$out" | sed 's/^/      | /'
}
on_branch() {
    g for-each-ref --format='%(refname)' refs/heads | while read -r ref; do g update-ref -d "$ref"; done
    g update-ref refs/heads/release/v0.6.0-rc "$1" || die "cannot create the branch"
}
no_tags() { g tag -d v0.6.0 bindings/go/v0.6.0 >/dev/null 2>&1 || true; }
# fake_so <out> [extra C]: a shared library defining a function for each baseline ABI name.
fake_so() {
    { g show "$B:abi/gibson-abi-v1.symbols" | grep -v '^#' | grep . | sed 's/.*/void &(void) {}/'; printf '%s\n' "${2:-}"; } >"$WORK/fake.c"
    cc -shared -fPIC -o "$1" "$WORK/fake.c" || die "cannot build $1"
}

# Fault-injection shims: when the command line matches SHIM_*_MATCH, the real tool's output is
# passed through (truncated to SHIM_KEEP_BYTES, if set) and the shim then fails, the way a tool
# that dies mid-output looks to the checker.
REAL_GIT="$(command -v git)" REAL_NM="$(command -v nm)"
mkdir -p "$WORK/shim"
for tool in git nm; do
    real="$REAL_GIT"; [ "$tool" = nm ] && real="$REAL_NM"
    var="SHIM_${tool^^}_MATCH"
    cat >"$WORK/shim/$tool" <<SHIM
#!/usr/bin/env bash
if [ -n "\${$var:-}" ] && [[ "\$*" =~ \$$var ]]; then
    if [ -n "\${SHIM_KEEP_BYTES:-}" ]; then "$real" "\$@" | head -c "\$SHIM_KEEP_BYTES"; else "$real" "\$@"; fi
    exit 128
fi
exec "$real" "\$@"
SHIM
    chmod +x "$WORK/shim/$tool"
done
SHIMPATH="$WORK/shim:$PATH"

echo "calibration: baseline $B, candidate $C, dev $D"
echo "calibration: --so $so (sha256 $(sha256sum "$so" | cut -d' ' -f1))"
echo "calibration: --foreign-so $foreign_so (sha256 $(sha256sum "$foreign_so" | cut -d' ' -f1))"
echo "calibration: checker sha256 $(sha256sum "$CHECKER" | cut -d' ' -f1), surface sha256 $(sha256sum "$SURFACE" | cut -d' ' -f1)"
A=(--baseline "$B")
HDR_FAIL='^FAIL  C ABI: include/ declarations differ'
SURF_FAIL='^FAIL  C ABI: the Rust C-ABI surface of src/ differs'
EXP_FAIL='^FAIL  C ABI: src/ at the candidate exports'

on_branch "$C"; no_tags
case_ "P1 valid candidate" 0 - -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow" --so "$so"

X="$(perturb "$C" include/gibson.h '0,/^\/\*/s//\/* calibration: a comment-only edit. *\/\n\/*/' 'calibration: comment-only header change')" || die "P2 setup"
on_branch "$X"; a="$(with_extra p2 "$X")" || die "P2 allow"
REQUIRE_PASS='C ABI: include/ declarations are identical'
case_ "P2 comment-only header change" 0 - -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"

on_branch "$C"; g tag -a v0.6.0 -m calibration "$C" || die "P3 tag"
g tag -a bindings/go/v0.6.0 -m calibration "$C" || die "P3 tag"
REQUIRE_PASS='tag: v0\.6\.0 peels to the candidate|tag: bindings/go/v0\.6\.0 peels to the candidate'
case_ "P3 both tags on the candidate" 0 - -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow" --so "$so"
no_tags

X="$(replace "$C" src/ffi.rs '    /// Also used as the ANSI-256 index.' '    /// Also used as the ANSI-256 index (calibration: a doc-only edit).' 'calibration: doc-only ffi change')" || die "P4 setup"
on_branch "$X"; a="$(with_extra p4 "$X")" || die "P4 allow"
REQUIRE_PASS='C ABI: the Rust C-ABI surface of src/ is identical'
case_ "P4 doc-comment-only change in src/ffi.rs" 0 - -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"

on_branch "$C"
grep -v '^#' "$allow" | grep . | awk '{printf "%s %s\r\n", $1, $2}' >"$WORK/allow.crlf"
case_ "P5 CRLF two-column allow-list" 0 - -- "${A[@]}" --candidate "$C" --allow-commits-file "$WORK/allow.crlf"

fake_so "$WORK/fake59.so"
REQUIRE_PASS="C ABI: the built library exports the baseline's"
case_ "P6 synthetic library with exactly the baseline's names" 0 - -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow" --so "$WORK/fake59.so"

on_branch "$D"
case_ "N1 development candidate" 1 "^FAIL  version: |$EXP_FAIL|$SURF_FAIL|^FAIL  C ABI: abi/gibson-abi-v1.symbols lists|^FAIL  C ABI: abi/ differs|$HDR_FAIL|^FAIL  layout oracle: " -- "${A[@]}" --candidate "$D"

X="$(perturb "$C" abi/gibson-abi-v1.symbols '$a gibson_calibration_extra' 'calibration: abi baseline change')" || die "N2 setup"
on_branch "$X"; a="$(with_extra n2 "$X")" || die "N2 allow"
case_ "N2 changed ABI symbol baseline" 1 '^FAIL  C ABI: abi/gibson-abi-v1.symbols lists|^FAIL  C ABI: abi/ differs' -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"

X="$(g commit-tree "$(g rev-parse "$C^{tree}")" -p "$(g rev-parse "$B^1")" -m 'calibration: the candidate tree, not descended from the baseline')" || die "N3 setup"
on_branch "$X"
case_ "N3 not descended from the baseline" 1 '^FAIL  ancestry: ' -- "${A[@]}" --candidate "$X"

on_branch "$C"
grep -v '^#' "$allow" | grep . | sed '$d' >"$WORK/allow.missing"
case_ "N4a allow-list missing a commit" 1 '^FAIL  commits: [0-9a-f]{7} is not in the allow-list' -- "${A[@]}" --candidate "$C" --allow-commits-file "$WORK/allow.missing"
{ cat "$allow"; echo "$D backport not in the range"; } >"$WORK/allow.extra"
case_ "N4b allow-list with an extra commit" 1 '^FAIL  commits: allow-list entry [0-9a-f]{7} is not in' -- "${A[@]}" --candidate "$C" --allow-commits-file "$WORK/allow.extra"
sed '0,/ backport / s// feature /' "$allow" >"$WORK/allow.class"
case_ "N4c allow-list with a bad class" 1 "^FAIL  commits: allow-list entry [0-9a-f]{7} has class 'feature'" -- "${A[@]}" --candidate "$C" --allow-commits-file "$WORK/allow.class"

case_ "N5a built library from another tree" 1 '^FAIL  C ABI: the built library exports' -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow" --so "$foreign_so"
X="$(perturb "$C" Cargo.toml '0,/^version = "0.6.0"$/s//version = "0.6.1"/' 'calibration: manifest version mismatch')" || die "N5b setup"
on_branch "$X"; a="$(with_extra n5 "$X")" || die "N5b allow"
case_ "N5b Cargo.toml version mismatch" 1 '^FAIL  version: candidate Cargo.toml' -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
on_branch "$C"; g update-ref refs/heads/release/v0.6.1-rc "$C" || die "N5c branch"
case_ "N5c release version mismatch" 1 '^FAIL  version: candidate [^=]+ = .0\.6\.0., expected 0\.6\.1' -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow" --release-version 0.6.1

on_branch "$C"; g tag -a v0.6.0 -m calibration "$B" || die "N6a tag"
case_ "N6a v0.6.0 tag on another commit" 1 '^FAIL  tag: v0\.6\.0 exists and peels to' -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow"
no_tags; g tag -a bindings/go/v0.6.0 -m calibration "$B" || die "N6b tag"
case_ "N6b bindings/go/v0.6.0 tag on another commit" 1 '^FAIL  tag: bindings/go/v0\.6\.0 exists and peels to' -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow"
no_tags

X="$(perturb "$C" include/gibson.h 's/uint8_t \*buf, size_t cap, size_t \*out_len);$/uint8_t *buf, uint32_t cap, size_t *out_len);/' 'calibration: header declaration change')" || die "N7 setup"
on_branch "$X"; a="$(with_extra n7 "$X")" || die "N7 allow"
case_ "N7 header declaration change" 1 "$HDR_FAIL" -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
X="$(perturb "$C" Cargo.toml 's/^rust-version = "1\.85"$/rust-version = "1.86"/' 'calibration: MSRV change')" || die "N8 setup"
on_branch "$X"; a="$(with_extra n8 "$X")" || die "N8 allow"
case_ "N8 MSRV change" 1 '^FAIL  MSRV: ' -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
pcm_file="$(g ls-tree -r --name-only "$C" -- src/audio | grep '\.rs$' | head -1)"
X="$(perturb "$C" "$pcm_file" '$a // calibration' 'calibration: PCM producer change')" || die "N9 setup"
on_branch "$X"; a="$(with_extra n9 "$X")" || die "N9 allow"
case_ "N9 frozen PCM producer change" 1 '^FAIL  PCM golden: ' -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
X="$(perturb "$C" src/ffi.rs '$a #[no_mangle]\npub extern "C" fn gibson_calibration_extra() {}' 'calibration: new export')" || die "N10 setup"
on_branch "$X"; a="$(with_extra n10 "$X")" || die "N10 allow"
case_ "N10 a new gibson_* export in src/" 1 "$EXP_FAIL|$SURF_FAIL" -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
g for-each-ref --format='%(refname)' refs/heads | while read -r ref; do g update-ref -d "$ref"; done
case_ "N11 candidate on no authorized branch" 1 '^FAIL  branch: ' -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow"

X="$(replace "$C" include/gibson.h '/* Status codes */' $'/* Status codes *\\\n/\n#undef GIBSON_ABI_VERSION\n#define GIBSON_ABI_VERSION 2u\n/\\\n* hidden */' 'calibration: a redefinition hidden by line splices')" || die "N12 setup"
on_branch "$X"; a="$(with_extra n12 "$X")" || die "N12 allow"
case_ "N12 macro hidden in a comment by line splices" 1 "$HDR_FAIL" -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
X="$(replace "$C" include/gibson.hpp '"Gibson render failed"' '"Gibson  render failed"' 'calibration: whitespace inside a string literal')" || die "N13 setup"
on_branch "$X"; a="$(with_extra n13 "$X")" || die "N13 allow"
case_ "N13 whitespace inside a string literal" 1 "$HDR_FAIL" -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
X="$(replace "$C" include/gibson.h '/* Status codes */' $'/* Status codes *?\x3f/\n/\n#define GIBSON_CALIBRATION 1\n/?\x3f/\n* hidden */' 'calibration: a comment spliced by trigraphs')" || die "N14 setup"
on_branch "$X"; a="$(with_extra n14 "$X")" || die "N14 allow"
case_ "N14 comment spliced by a trigraph" 1 "$HDR_FAIL" -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"

X="$(replace "$C" src/ffi.rs $'    pub r: u8,\n    pub g: u8,\n    pub b: u8,' $'    pub b: u8,\n    pub g: u8,\n    pub r: u8,' 'calibration: repr(C) fields swapped')" || die "N15 setup"
on_branch "$X"; a="$(with_extra n15 "$X")" || die "N15 allow"
case_ "N15 two #[repr(C)] fields swapped" 1 "$SURF_FAIL" -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
X="$(replace "$C" src/ffi.rs $'fn gibson_set_max_fps(\n    ctx: *mut GibsonContextOpaque,\n    fps: u32,' $'fn gibson_set_max_fps(\n    ctx: *mut GibsonContextOpaque,\n    fps: u64,' 'calibration: parameter type change')" || die "N16 setup"
on_branch "$X"; a="$(with_extra n16 "$X")" || die "N16 allow"
case_ "N16 a parameter type changed" 1 "$SURF_FAIL" -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
X="$(perturb "$C" src/ffi.rs '$a #[no_mangle]\npub extern "C"\nfn gibson_calibration_multiline() {}' 'calibration: multi-line export')" || die "N17 setup"
on_branch "$X"; a="$(with_extra n17 "$X")" || die "N17 allow"
case_ "N17 an export whose fn is on the next line" 1 "$EXP_FAIL|$SURF_FAIL" -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
X="$(perturb "$C" src/ffi.rs '$a #[no_mangle]\npub static gibson_calibration_data: u32 = 1;' 'calibration: exported static')" || die "N18 setup"
on_branch "$X"; a="$(with_extra n18 "$X")" || die "N18 allow"
case_ "N18 a new exported static" 1 "$EXP_FAIL|$SURF_FAIL" -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
on_branch "$B"
case_ "N19 the baseline itself as the candidate" 1 '^FAIL  ancestry: the candidate is the baseline' -- "${A[@]}" --candidate "$B"
g for-each-ref --format='%(refname)' refs/heads | while read -r ref; do g update-ref -d "$ref"; done
g update-ref refs/heads/feature/release/v0.6.0-rc "$C" || die "N20 branch"
case_ "N20 candidate only on feature/release/v0.6.0-rc" 1 '^FAIL  branch: ' -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow"
g update-ref -d refs/heads/feature/release/v0.6.0-rc
X="$(replace "$C" src/ffi.rs 'pub const GIBSON_CELL_GLYPH_CAP: usize = 16;' 'pub const GIBSON_CELL_GLYPH_CAP: usize = 17;' 'calibration: const value change')" || die "N21 setup"
on_branch "$X"; a="$(with_extra n21 "$X")" || die "N21 allow"
case_ "N21 a GIBSON_* const value changed" 1 "$SURF_FAIL" -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
on_branch "$C"; fake_so "$WORK/fake59d.so" 'int gibson_calibration_data = 1;'
case_ "N22 a library that also exports a gibson_* data symbol" 1 '^FAIL  C ABI: the built library exports 60' -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow" --so "$WORK/fake59d.so"

X="$(replace "$C" include/gibson.h '/* Status codes */' "$(printf '/* Status codes *\\\n/\n#undef GIBSON_ABI_VERSION\n#define GIBSON_ABI_VERSION 2u\n/\\\n* hidden */'; for _ in $(seq 6000); do printf '\n/* padding */'; done)" 'calibration: a splice followed by padding (SIGPIPE probe)')" || die "N24 setup"
on_branch "$X"; a="$(with_extra n24 "$X")" || die "N24 allow"
case_ "N24 a splice followed by 6000 padding comments (84 KB, past a 64 KB pipe buffer)" 1 "$HDR_FAIL" -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
X="$(replace "$C" src/ffi.rs $'#[no_mangle]\npub extern "C" fn gibson_abi_version()' $'#[no_mangle]\n#[cfg(any())]\npub extern "C" fn gibson_abi_version()' 'calibration: cfg on an export')" || die "N25 setup"
on_branch "$X"; a="$(with_extra n25 "$X")" || die "N25 allow"
case_ "N25 a cfg attribute on an export" 1 "$SURF_FAIL" -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
X="$(replace "$C" src/ffi.rs 'pub extern "C" fn gibson_abi_version()' 'pub extern "C" fn gibson_abi_version<T>()' 'calibration: generic export')" || die "N26 setup"
on_branch "$X"; a="$(with_extra n26 "$X")" || die "N26 allow"
case_ "N26 generic parameters on an export" 1 "$SURF_FAIL" -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
X="$(perturb "$C" src/ffi.rs '$a #[export_name = "gibson_calibration_named"]\npub extern "C" fn calibration_named() {}' 'calibration: export_name')" || die "N27 setup"
on_branch "$X"; a="$(with_extra n27 "$X")" || die "N27 allow"
case_ "N27 an export through #[export_name]" 1 "$EXP_FAIL|$SURF_FAIL" -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
X="$(perturb "$C" src/ffi.rs '$a pub type GibsonCalibrationAlias = u32;' 'calibration: type alias in the ABI module')" || die "N28 setup"
on_branch "$X"; a="$(with_extra n28 "$X")" || die "N28 allow"
case_ "N28 a type alias added to the ABI module" 1 "$SURF_FAIL" -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
X="$(perturb "$C" tests/ffi_abi_layout_freeze.rs '$a // calibration' 'calibration: layout oracle edit')" || die "N29 setup"
on_branch "$X"; a="$(with_extra n29 "$X")" || die "N29 allow"
case_ "N29 the layout oracle edited" 1 '^FAIL  layout oracle: ' -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
X="$(perturb "$C" src/lib.rs '$a #[cfg(unix)]\nmod calibration_internal;' 'calibration: a non-exporting module (a legitimate backport shape)')" || die "P7 setup"
on_branch "$X"; a="$(with_extra p7 "$X")" || die "P7 allow"
REQUIRE_PASS='C ABI: the Rust C-ABI surface of src/ is identical'
case_ "P7 a new non-exporting #[cfg(unix)] module" 0 - -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"

X="$(perturb "$C" src/ffi.rs '$a #[cfg(test)]\nmod calibration_test_only {\n    use std::mem::size_of;\n    #[test]\n    fn size_check() { assert_eq!(size_of::<u32>(), 4); }\n}' 'calibration: a test-only module in the ABI file')" || die "P8 setup"
on_branch "$X"; a="$(with_extra p8 "$X")" || die "P8 allow"
REQUIRE_PASS='C ABI: the Rust C-ABI surface of src/ is identical'
case_ "P8 a #[cfg(test)] module with imports in src/ffi.rs" 0 - -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
for _ in $(seq 20000); do echo '# calibration padding'; done >"$WORK/padding.toml"
# The padding goes at the END, after [package]'s version line: a reader that exits once it has the
# version then abandons a git show that still has ~440 KB to write (the hazard P9 guards).
X="$(perturb "$C" Cargo.toml "\$r $WORK/padding.toml" 'calibration: 20000 trailing comment lines in Cargo.toml')" || die "P9 setup"
on_branch "$X"; a="$(with_extra p9 "$X")" || die "P9 allow"
REQUIRE_PASS='version: candidate Cargo.toml = 0\.6\.0'
case_ "P9 a Cargo.toml with 20000 comment lines (no early-exit SIGPIPE)" 0 - -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"

# Fault injection: each tool dies after (part of) its output; the checker must fail, not pass.
X="$(perturb "$C" include/gibson.h 's/uint8_t \*buf, size_t cap, size_t \*out_len);$/uint8_t *buf, uint32_t cap, size_t *out_len);/' 'calibration: header declaration change (fault injection)')" || die "F1 setup"
on_branch "$X"; a="$(with_extra f1 "$X")" || die "F1 allow"
CASE_ENV=(PATH="$SHIMPATH" SHIM_GIT_MATCH='^show [0-9a-f]{40}:include/gibson\.h$' SHIM_KEEP_BYTES=200)
case_ "F1 the header read dies after a common prefix" 1 '^FAIL  tools: |'"$HDR_FAIL" -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
X="$(perturb "$C" include/gibson.h '0,/^\/\*/s//\/* calibration: a comment-only edit. *\/\n\/*/' 'calibration: comment-only header change (fault injection)')" || die "F2 setup"
on_branch "$X"; a="$(with_extra f2 "$X")" || die "F2 allow"
CASE_ENV=(PATH="$SHIMPATH" SHIM_GIT_MATCH='^diff --name-status')
case_ "F2 header enumeration fails" 1 '^FAIL  tools: |'"$HDR_FAIL" -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"
on_branch "$C"
CASE_ENV=(PATH="$SHIMPATH" SHIM_NM_MATCH='^-D --defined-only')
case_ "F3 nm dies after printing every symbol" 1 '^FAIL  tools: |^FAIL  C ABI: the built library exports' -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow" --so "$so"
CASE_ENV=(PATH="$SHIMPATH" SHIM_GIT_MATCH='^for-each-ref --contains')
case_ "F4 branch enumeration dies after printing" 1 '^FAIL  tools: |^FAIL  branch: ' -- "${A[@]}" --candidate "$C" --allow-commits-file "$allow"

# N23: src/ffi.rs's blob is absent from the object store (git mktree --missing), so reading it fails.
src_tree="$(g ls-tree "$C" -- src | awk '{print $3}')" || die "N23 setup"
missing="$(printf 'e%.0s' $(seq 40))"
new_src="$(g ls-tree "$src_tree" | sed -E "s/^(100644 blob )[0-9a-f]{40}(\tffi\.rs)$/\1$missing\2/" | g mktree --missing)" || die "N23 setup"
[ "$new_src" != "$src_tree" ] || die "N23 setup: ffi.rs not replaced"
new_root="$(g ls-tree "$C" | sed -E "s/^(040000 tree )[0-9a-f]{40}(\tsrc)$/\1$new_src\2/" | g mktree --missing)" || die "N23 setup"
X="$(g commit-tree "$new_root" -p "$C" -m 'calibration: unreadable src blob')" || die "N23 setup"
on_branch "$X"; a="$(with_extra n23 "$X")" || die "N23 allow"
case_ "N23 an unreadable src/ file fails closed" 1 '^FAIL  C ABI: cannot parse the Rust C-ABI surface at the candidate|^FAIL  C ABI: src/ at the candidate exports 0|^FAIL  tools: ' -- "${A[@]}" --candidate "$X" --allow-commits-file "$a"

echo
total="$(grep -c '^case_ "' "${BASH_SOURCE[0]}")"
if [ "$failures" -eq 0 ]; then echo "CALIBRATION: PASS — $total of $total cases met their expected verdict for their intended reason"; exit 0; fi
echo "CALIBRATION: FAIL — $failures of $total case(s) wrong" >&2
exit 1
