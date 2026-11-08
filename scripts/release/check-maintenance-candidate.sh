#!/usr/bin/env bash
#
# check-maintenance-candidate.sh — read-only gate for a maintenance-line release candidate.
#
# Checks a candidate commit against the maintenance-line exception to
# docs/RELEASE_CONTRACT.md §1 (approved 2026-11-08, decision #287; procedure in
# docs/RELEASING.md §7 "Maintenance-line release"). A maintenance release may carry
# backported fixes; it must not quietly import later development.
#
# Usage:
#   scripts/release/check-maintenance-candidate.sh --baseline <rev> --candidate <rev>
#       [--allow-commits-file <path>] [--release-version <x.y.z>] [--so <libgibson.so>]
#
#   --baseline            the declared source-version baseline (e.g. 9a899e1 for 0.6.0)
#   --candidate           the exact commit proposed for the tag
#   --allow-commits-file  the qualification record's commit list: one commit per line,
#                         "<sha> <backport|release-engineering> [note]"; '#' starts a
#                         comment. Every commit in baseline..candidate must be listed,
#                         and every listed commit must be in that range.
#   --release-version     the version being released (default: the baseline's
#                         Cargo.toml version)
#   --so                  a libgibson.so built from the candidate; its exported gibson_*
#                         symbols must equal the baseline's ABI symbol set exactly
#
# Checks:
#   1. ancestry   the baseline is an ancestor of the candidate, and is not the candidate
#                 itself (the baseline is never tagged; at least one commit follows it)
#   2. version    the release version is a plain x.y.z; the candidate's Cargo.toml,
#                 Cargo.lock (libgibson entry) and bindings/python/pyproject.toml carry it
#   3. C ABI      abi/ is byte-identical to the baseline. include/ is byte-identical, or
#                 differs only in comments: comments are removed by the compiler's own lexer
#                 (`cc -fpreprocessed -dD -E -P`) and whitespace is collapsed outside string
#                 and character literals; a header that contains a line splice (a line ending
#                 in a backslash) or a trigraph introducer (`??`) in either revision, a raw
#                 string literal, an added/removed/renamed header, or no compiler fails closed.
#                 The Rust C-ABI surface of src/ (scripts/release/maintenance_surface.py
#                 rust-abi: every extern "C" fn gibson_* signature and its #[no_mangle], every
#                 static gibson_*, every #[repr] struct/enum/union body, every GIBSON_* const
#                 value, and the macros that define exported functions) equals the baseline's,
#                 and its exported names equal the baseline's abi/gibson-abi-v1.symbols (the
#                 extractor is first calibrated against that file at the baseline). With --so,
#                 every defined gibson_* symbol of the built library (any symbol type) must
#                 equal that set.
#   4. MSRV       Cargo.toml rust-version is unchanged
#   5. PCM golden src/audio/ and tests/audio_pocket_voice.rs (the frozen PCM
#                 fingerprint) are unchanged; tests/ffi_abi_layout_freeze.rs (the C ABI layout
#                 oracle CI runs) is unchanged
#   6. branch     a local branch refs/heads/release/v<x.y.z>-rc or refs/heads/release/<x.y>, or
#                 the same name under a configured remote (refs/remotes/<remote>/...), contains
#                 the candidate
#   7. tags       v<x.y.z> and bindings/go/v<x.y.z>, if they exist locally, peel to the
#                 candidate
#   8. commits    every commit in baseline..candidate is listed (and checked against
#                 --allow-commits-file when given)
#
# Everything is read from git objects, so the working tree is irrelevant and nothing is
# written. No network access: refs and tags are whatever is local (fetch first); a tag that
# exists only on the remote is invisible here, so RELEASING §7 checks the remote separately.
# The checker is structural: it does not judge whether a listed commit is a legitimate
# backport (the independent review does), and it does not see a behaviour change inside an
# unchanged signature (CI and the review do). Its Rust surface is a parser of the constructs this
# crate's C ABI uses; the export set is settled by --so (required to qualify), struct layouts by
# the frozen layout oracle in CI, and anything the parser cannot see by the independent review.
# Any tool failure feeding a check fails the run. Requires git, python3, and cc for include/.
# Exit status: 0 PASS, 1 one or more violations, 2 usage or input error.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

ABI_FILE="abi/gibson-abi-v1.symbols"
PCM_GOLDEN_TEST="tests/audio_pocket_voice.rs"
PCM_GOLDEN_PATHS=(src/audio "$PCM_GOLDEN_TEST")
LAYOUT_ORACLE="tests/ffi_abi_layout_freeze.rs"

die() { echo "check-maintenance-candidate: $*" >&2; exit 2; }
usage() { sed -n '3,/^$/s/^# \{0,1\}//p' "${BASH_SOURCE[0]}"; }

baseline="" candidate="" allow_file="" release_version="" so=""
while [ $# -gt 0 ]; do
    case "$1" in
        --baseline|--candidate|--allow-commits-file|--release-version|--so)
            [ $# -ge 2 ] || die "$1 needs a value"
            case "$1" in
                --baseline) baseline="$2" ;;
                --candidate) candidate="$2" ;;
                --allow-commits-file) allow_file="$2" ;;
                --release-version) release_version="$2" ;;
                --so) so="$2" ;;
            esac
            shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) die "unknown argument: $1 (see --help)" ;;
    esac
done
if [ -z "$baseline" ] || [ -z "$candidate" ]; then die "--baseline and --candidate are required (see --help)"; fi

B="$(git rev-parse --verify --quiet "$baseline^{commit}")" || die "baseline '$baseline' is not a commit"
C="$(git rev-parse --verify --quiet "$candidate^{commit}")" || die "candidate '$candidate' is not a commit"
if [ -n "$allow_file" ] && [ ! -f "$allow_file" ]; then die "allow-commits file not found: $allow_file"; fi
if [ -n "$so" ]; then
    [ -f "$so" ] || die "--so library not found: $so"
    command -v nm >/dev/null || die "nm is required for --so"
fi
command -v python3 >/dev/null || die "python3 is required (scripts/release/maintenance_surface.py)"
SURFACE="$REPO_ROOT/scripts/release/maintenance_surface.py"
[ -f "$SURFACE" ] || die "missing $SURFACE"

violations=0
# Tool failures inside command substitutions and pipelines cannot stop the script (errexit does
# not reach them), so every tool call that feeds a check records its failure here, and the run
# fails if anything was recorded: a check never passes on partial or missing tool output.
ERRLOG="$(mktemp "${TMPDIR:-/tmp}/check-maintenance-candidate.err.XXXXXX")"
trap 'rm -f "$ERRLOG"' EXIT
err() { printf '%s\n' "$*" >>"$ERRLOG"; }
pass() { printf 'PASS  %s\n' "$*"; }
fail() { printf 'FAIL  %s\n' "$*"; violations=$((violations + 1)); }
info() { printf 'INFO  %s\n' "$*"; }

# show <rev> <path>: a file at a revision, or nothing if it does not exist there. A path that
# exists but cannot be read is recorded as a tool failure.
show() {
    local rc=0
    git cat-file -e "$1:$2" 2>/dev/null || rc=$?
    case "$rc" in
        0) git show "$1:$2" || err "git show $1:$2 failed" ;;
        1|128) git rev-parse --verify --quiet "$1^{commit}" >/dev/null || err "cannot resolve $1" ;;
        *) err "git cat-file -e $1:$2 exited $rc" ;;
    esac
}
# package_field <rev> <key>: a key of Cargo.toml's [package] table.
package_field() {
    show "$1" Cargo.toml | awk -v k="$2" '
        /^\[/ { in_pkg = ($0 == "[package]"); next }
        in_pkg && $1 == k { sub(/^[^=]*=[[:space:]]*"/, ""); sub(/".*/, ""); print; exit }'
}
lock_version() { show "$1" Cargo.lock | sed -n '/^name = "libgibson"$/{n;s/^version = "\(.*\)"/\1/p;}' | head -1; }
pyproject_version() { show "$1" bindings/python/pyproject.toml | sed -n 's/^version = "\(.*\)"/\1/p' | head -1; }
declared_symbols() { show "$1" "$ABI_FILE" | sed -e '/^#/d' -e '/^[[:space:]]*$/d' | sort -u; }
count() { grep -c . || true; }

# rust_surface <rev>: the canonical Rust C-ABI surface of src/ at <rev> (see check 3), or a
# non-zero status if it cannot be parsed (the caller fails closed).
rust_surface() {
    local rev="$1" files f
    # Callers use `|| die`, which disables errexit here, so every step checks its own status.
    files="$(git ls-tree -r --name-only "$rev" -- src)" || { err "git ls-tree $rev src failed"; return 1; }
    {
        printf '%s\n' "$files" | grep '\.rs$' | while IFS= read -r f; do
            printf '//@file %s\n' "$f"
            git show "$rev:$f" || { err "git show $rev:$f failed"; exit 1; }
            printf '\n'
        done
    } | python3 -I "$SURFACE" rust-abi || return 1
}
# header_decls <rev> <path>: a public header's declarations: comments removed by the compiler's
# own lexer (no macro expansion; directives kept), whitespace collapsed outside literals. Fails
# (the caller then counts the header as changed) when the file is absent, no compiler is
# available, the text has a line splice or a trigraph introducer, or it cannot be normalized.
header_decls() {
    local lang=c raw out
    case "$2" in *.hpp|*.hh|*.hxx) lang="c++" ;; esac
    command -v "${CC:-cc}" >/dev/null 2>&1 || return 1
    git cat-file -e "$1:$2" 2>/dev/null || return 1
    raw="$(git show "$1:$2")" || { err "git show $1:$2 failed"; return 1; }
    raw="${raw//$'\r'/}"
    # Exit 1 (no splice, no trigraph) is the only way on; a match (0) or an error (2) fails closed.
    # A here-string, not a pipe: grep -q exits at the first match, and a writer into a pipe would
    # then die of SIGPIPE, whose status pipefail would report as the condition's.
    local rc=0
    grep -q -E '\\$|\?\?' <<<"$raw" || rc=$?
    [ "$rc" -eq 1 ] || return 1
    out="$(printf '%s\n' "$raw" | "${CC:-cc}" -fpreprocessed -dD -E -P -x "$lang" - 2>/dev/null)" || return 1
    [ -n "$out" ] || return 1
    printf '%s' "$out" | python3 -I "$SURFACE" header
}
# set_delta <expected> <actual>: the symbols missing from (-) and added to (+) <actual>.
set_delta() {
    comm -23 <(printf '%s\n' "$1") <(printf '%s\n' "$2") | sed '/^$/d; s/^/        - /'
    comm -13 <(printf '%s\n' "$1") <(printf '%s\n' "$2") | sed '/^$/d; s/^/        + /'
}

b_ver="$(package_field "$B" version)"
[ -n "$release_version" ] || release_version="$b_ver"
echo "check-maintenance-candidate: baseline  $B (Cargo.toml $b_ver)"
echo "check-maintenance-candidate: candidate $C (Cargo.toml $(package_field "$C" version))"
echo "check-maintenance-candidate: release version $release_version"

# 1. ancestry
if [ "$B" = "$C" ]; then
    fail "ancestry: the candidate is the baseline ${B:0:7} itself; the baseline is never tagged (decision 0001)"
elif git merge-base --is-ancestor "$B" "$C"; then
    pass "ancestry: baseline ${B:0:7} is an ancestor of candidate ${C:0:7}"
else
    fail "ancestry: baseline ${B:0:7} is NOT an ancestor of candidate ${C:0:7} (merge-base: $(git merge-base "$B" "$C" | cut -c1-7))"
fi

# 2. version
if [[ ! "$release_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    fail "version: release version '$release_version' is not a plain x.y.z release version"
fi
for pair in "Cargo.toml:$(package_field "$C" version)" "Cargo.lock libgibson:$(lock_version "$C")" \
            "bindings/python/pyproject.toml:$(pyproject_version "$C")"; do
    where="${pair%:*}" got="${pair##*:}"
    if [ "$got" = "$release_version" ]; then
        pass "version: candidate $where = $got"
    else
        fail "version: candidate $where = '${got:-<missing>}', expected $release_version"
    fi
done

# 3. C ABI
b_decl="$(declared_symbols "$B")" c_decl="$(declared_symbols "$C")"
b_surf="$(rust_surface "$B")" || die "cannot parse the Rust C-ABI surface at the baseline"
if ! c_surf="$(rust_surface "$C")"; then fail "C ABI: cannot parse the Rust C-ABI surface at the candidate (fail closed)"; c_surf=""; fi
b_src="$(printf '%s\n' "$b_surf" | sed -n 's/^export //p' | sort -u)"
c_src="$(printf '%s\n' "$c_surf" | sed -n 's/^export //p' | sort -u)"
n_base="$(printf '%s\n' "$b_decl" | count)"
if [ -z "$b_decl" ]; then
    fail "C ABI: $ABI_FILE is missing or empty at the baseline; nothing to compare against"
elif [ "$b_src" != "$b_decl" ]; then
    fail "C ABI: extractor calibration failed at the baseline (src/ $(printf '%s\n' "$b_src" | count) vs $ABI_FILE $n_base); fix src_exports before trusting it:"
    set_delta "$b_decl" "$b_src"
else
    if [ "$c_src" = "$b_decl" ]; then
        pass "C ABI: src/ at the candidate exports the baseline's $n_base gibson_* symbols exactly (extractor calibrated at the baseline: $n_base = $n_base)"
    else
        fail "C ABI: src/ at the candidate exports $(printf '%s\n' "$c_src" | count) gibson_* symbols, the baseline $n_base:"
        set_delta "$b_decl" "$c_src"
    fi
    if [ "$c_decl" != "$b_decl" ]; then
        fail "C ABI: $ABI_FILE lists $(printf '%s\n' "$c_decl" | count) symbols at the candidate, $n_base at the baseline:"
        set_delta "$b_decl" "$c_decl"
    fi
fi
if [ -n "$c_surf" ] && [ "$b_surf" = "$c_surf" ]; then
    pass "C ABI: the Rust C-ABI surface of src/ is identical to the baseline ($(printf '%s\n' "$c_surf" | count) items: signatures, #[repr] layouts, GIBSON_* consts, exporting macros)"
elif [ -n "$c_surf" ]; then
    fail "C ABI: the Rust C-ABI surface of src/ differs from the baseline:"
    diff <(printf '%s\n' "$b_surf") <(printf '%s\n' "$c_surf") | grep '^[<>]' | head -n 20 \
        | sed 's/^</        -/; s/^>/        +/' || true
fi
if git diff --quiet "$B" "$C" -- abi/; then
    pass "C ABI: abi/ is byte-identical to the baseline"
else
    fail "C ABI: abi/ differs from the baseline:"
    git diff --stat "$B" "$C" -- abi/ | sed 's/^/       /'
fi
# include/: byte-identical, or differing only in comments (a disclosed limitation in a doc
# comment is release engineering; a declaration, macro or layout change is not).
if git diff --quiet "$B" "$C" -- include/; then
    pass "C ABI: include/ is byte-identical to the baseline"
else
    comment_only=() decl_changed=()
    changed="$(git diff --name-status "$B" "$C" -- include/)" || { err "git diff --name-status include/ failed"; changed=""; }
    [ -n "$changed" ] || decl_changed+=("(no changed header could be listed)")
    while IFS=$'\t' read -r st p _; do
        [ -n "${st:-}" ] || continue
        if [ "$st" != "M" ]; then decl_changed+=("$p($st)"); continue; fi
        bd="$(header_decls "$B" "$p")" || { decl_changed+=("$p(comment stripper failed)"); continue; }
        cd_="$(header_decls "$C" "$p")" || { decl_changed+=("$p(comment stripper failed)"); continue; }
        if [ -n "$bd" ] && [ "$bd" = "$cd_" ]; then comment_only+=("$p"); else decl_changed+=("$p"); fi
    done <<<"$changed"
    if [ ${#decl_changed[@]} -eq 0 ]; then
        pass "C ABI: include/ declarations are identical to the baseline; only comments changed in ${comment_only[*]} (review the text)"
    else
        fail "C ABI: include/ declarations differ from the baseline in: ${decl_changed[*]}"
        git diff --stat "$B" "$C" -- include/ | sed 's/^/       /'
    fi
fi
if [ -n "$so" ]; then
    nm_out="$(nm -D --defined-only "$so")" || { err "nm -D --defined-only $so failed"; nm_out=""; }
    built="$(printf '%s\n' "$nm_out" | awk 'NF>=3{print $3}' | sed 's/@.*//' | sed -n '/^gibson_/p' | sort -u)"
    so_id="$so, sha256 $(sha256sum "$so" | cut -d' ' -f1)"
    if [ -n "$b_decl" ] && [ "$built" = "$b_decl" ]; then
        pass "C ABI: the built library exports the baseline's $n_base gibson_* symbols exactly ($so_id)"
    else
        fail "C ABI: the built library exports $(printf '%s\n' "$built" | count) gibson_* symbols, the baseline $n_base ($so_id):"
        set_delta "$b_decl" "$built"
    fi
fi

# 4. MSRV
b_msrv="$(package_field "$B" rust-version)" c_msrv="$(package_field "$C" rust-version)"
if [ -n "$b_msrv" ] && [ "$b_msrv" = "$c_msrv" ]; then
    pass "MSRV: rust-version $c_msrv, unchanged"
else
    fail "MSRV: rust-version '${b_msrv:-<missing>}' at the baseline, '${c_msrv:-<missing>}' at the candidate"
fi

# 5. PCM golden
fp="$(show "$C" "$PCM_GOLDEN_TEST" | grep -o -E 'fingerprint\(&historical\), 0x[0-9a-f]+' | sed 's/.*, //' | head -1 || true)"
if ! git cat-file -e "$B:$PCM_GOLDEN_TEST" 2>/dev/null; then
    fail "PCM golden: $PCM_GOLDEN_TEST does not exist at the baseline"
elif git diff --quiet "$B" "$C" -- "${PCM_GOLDEN_PATHS[@]}"; then
    pass "PCM golden: ${PCM_GOLDEN_PATHS[*]} unchanged (frozen fingerprint ${fp:-<not found>})"
else
    fail "PCM golden: the producer or the frozen golden changed since the baseline:"
    git diff --stat "$B" "$C" -- "${PCM_GOLDEN_PATHS[@]}" | sed 's/^/       /'
fi

# 5b. the C ABI layout oracle (the test that freezes struct sizes and field offsets) is unchanged,
# so CI keeps checking the baseline's layouts at the candidate.
if ! git cat-file -e "$B:$LAYOUT_ORACLE" 2>/dev/null; then
    info "layout oracle: $LAYOUT_ORACLE does not exist at the baseline"
elif git diff --quiet "$B" "$C" -- "$LAYOUT_ORACLE"; then
    pass "layout oracle: $LAYOUT_ORACLE unchanged (CI runs it at the candidate)"
else
    fail "layout oracle: $LAYOUT_ORACLE changed since the baseline"
fi

# 6. branch
mm="${release_version%.*}"
names_re="release/(v${release_version//./\\.}-rc|${mm//./\\.})"
remotes="$(git remote)" || { err "git remote failed"; remotes=""; }
# shellcheck disable=SC2016 # a literal sed script
remotes_re="$(printf '%s\n' "$remotes" | sed '/^$/d; s/[.[\*^$()+?{|]/\\&/g' | paste -sd'|' -)"
branch_re="^refs/heads/${names_re}\$"
[ -z "$remotes_re" ] || branch_re="${branch_re}|^refs/remotes/(${remotes_re})/${names_re}\$"
refs="$(git for-each-ref --contains "$C" --format='%(refname)' refs/heads refs/remotes)" \
    || { err "git for-each-ref --contains failed"; refs=""; }
branches="$(printf '%s\n' "$refs" | sed -n -E "/${branch_re//\//\\/}/p" | sed 's#^refs/heads/##; s#^refs/remotes/##' | tr '\n' ' ')"
if [ -n "$branches" ]; then
    pass "branch: candidate is on ${branches% }"
else
    fail "branch: no branch release/v$release_version-rc or release/$mm (local, or of a configured remote) contains the candidate"
fi

# 7. tags (local refs only)
for tag in "v$release_version" "bindings/go/v$release_version"; do
    rc=0
    git show-ref --verify --quiet "refs/tags/$tag" || rc=$?
    if [ "$rc" -ne 0 ] && [ "$rc" -ne 1 ]; then
        err "git show-ref refs/tags/$tag exited $rc"
    elif [ "$rc" -eq 0 ]; then
        target="$(git rev-parse --verify "refs/tags/$tag^{commit}")" || { err "cannot peel refs/tags/$tag"; target=""; }
        if [ "$target" = "$C" ]; then
            pass "tag: $tag peels to the candidate"
        else
            fail "tag: $tag exists and peels to ${target:0:12}, not the candidate"
        fi
    else
        info "tag: $tag does not exist locally"
    fi
done

# 8. commits
range="$(git rev-list --reverse "$B..$C")" || { err "git rev-list $B..$C failed"; range=""; }
n_range="$(printf '%s\n' "$range" | count)"
echo "INFO  commits: $n_range in ${B:0:7}..${C:0:7}:"
# shellcheck disable=SC2086 # one commit id per word
[ -z "$range" ] || git log --no-walk=unsorted --format='        %h %s' $range
if [ -n "$allow_file" ]; then
    allowed=""
    while IFS= read -r raw || [ -n "$raw" ]; do
        line="${raw%%#*}"; line="${line//$'\r'/}"
        read -r sha class _ <<<"$line" || true
        [ -n "${sha:-}" ] || continue
        full="$(git rev-parse --verify --quiet "$sha^{commit}")" || { fail "commits: allow-list entry '$sha' is not a commit"; continue; }
        case "${class:-}" in
            backport|release-engineering) ;;
            *) fail "commits: allow-list entry ${full:0:7} has class '${class:-<none>}' (expected backport or release-engineering)" ;;
        esac
        allowed+="$full"$'\n'
    done <"$allow_file"
    unlisted="$(comm -23 <(printf '%s\n' "$range" | sort -u) <(printf '%s' "$allowed" | sort -u) | sed '/^$/d')"
    outside="$(comm -13 <(printf '%s\n' "$range" | sort -u) <(printf '%s' "$allowed" | sort -u) | sed '/^$/d')"
    for c in $unlisted; do fail "commits: ${c:0:7} is not in the allow-list ($(git log -1 --format=%s "$c"))"; done
    for c in $outside; do fail "commits: allow-list entry ${c:0:7} is not in ${B:0:7}..${C:0:7}"; done
    if [ -z "$unlisted" ] && [ -z "$outside" ]; then
        pass "commits: all $n_range commits are in the allow-list $allow_file, and it lists nothing else"
    fi
else
    info "commits: not checked against a qualification record (pass --allow-commits-file; required to qualify a candidate)"
fi

if [ -s "$ERRLOG" ]; then
    fail "tools: a git, nm or other command failed, so a check above may rest on partial output:"
    sed 's/^/        /' "$ERRLOG"
fi
if [ "$violations" -eq 0 ]; then
    echo "RESULT: PASS — ${C:0:12} meets the maintenance-line checks against baseline ${B:0:12}"
    exit 0
fi
echo "RESULT: FAIL — $violations violation(s); ${C:0:12} is not a valid maintenance candidate for baseline ${B:0:12}" >&2
exit 1
