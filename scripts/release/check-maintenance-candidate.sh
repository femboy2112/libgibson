#!/usr/bin/env bash
#
# check-maintenance-candidate.sh — read-only gate for a maintenance-line release candidate.
#
# Checks a candidate commit against the maintenance-line exception to
# docs/RELEASE_CONTRACT.md §1 (PROPOSED, owner decision #287; procedure in
# docs/RELEASING.md "Maintenance-line release"). A maintenance release may carry
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
#   1. ancestry   the baseline is an ancestor of the candidate
#   2. version    the release version is a plain x.y.z; the candidate's Cargo.toml,
#                 Cargo.lock (libgibson entry) and bindings/python/pyproject.toml carry it
#   3. C ABI      abi/ and include/ are byte-identical to the baseline; the gibson_*
#                 exports defined in src/ equal the baseline's abi/gibson-abi-v1.symbols
#                 (the extractor is first calibrated against that file at the baseline)
#   4. MSRV       Cargo.toml rust-version is unchanged
#   5. PCM golden src/audio/ and tests/audio_pocket_voice.rs (the frozen PCM
#                 fingerprint) are unchanged
#   6. branch     a local or remote-tracking ref named release/v<x.y.z>-rc or
#                 release/<x.y> contains the candidate
#   7. tags       v<x.y.z> and bindings/go/v<x.y.z>, if they exist locally, peel to the
#                 candidate
#   8. commits    every commit in baseline..candidate is listed (and checked against
#                 --allow-commits-file when given)
#
# Everything is read from git objects, so the working tree is irrelevant and nothing is
# written. No network access: refs and tags are whatever is local (fetch first).
# Exit status: 0 PASS, 1 one or more violations, 2 usage or input error.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

ABI_FILE="abi/gibson-abi-v1.symbols"
PCM_GOLDEN_TEST="tests/audio_pocket_voice.rs"
PCM_GOLDEN_PATHS=(src/audio "$PCM_GOLDEN_TEST")

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

violations=0
pass() { printf 'PASS  %s\n' "$*"; }
fail() { printf 'FAIL  %s\n' "$*"; violations=$((violations + 1)); }
info() { printf 'INFO  %s\n' "$*"; }

# show <rev> <path>: a file at a revision, or nothing if it does not exist there.
show() { git show "$1:$2" 2>/dev/null || true; }
# package_field <rev> <key>: a key of Cargo.toml's [package] table.
package_field() {
    show "$1" Cargo.toml | awk -v k="$2" '
        /^\[/ { in_pkg = ($0 == "[package]"); next }
        in_pkg && $1 == k { sub(/^[^=]*=[[:space:]]*"/, ""); sub(/".*/, ""); print; exit }'
}
lock_version() { show "$1" Cargo.lock | sed -n '/^name = "libgibson"$/{n;s/^version = "\(.*\)"/\1/p;}' | head -1; }
pyproject_version() { show "$1" bindings/python/pyproject.toml | sed -n 's/^version = "\(.*\)"/\1/p' | head -1; }
declared_symbols() { show "$1" "$ABI_FILE" | grep -v '^#' | grep -v '^[[:space:]]*$' | sort -u || true; }
count() { grep -c . || true; }

# src_exports <rev>: the gibson_* names src/ exports — each `extern "C" fn gibson_*`
# definition, plus the gibson_* first argument of every invocation of a macro whose body
# defines an `extern "C" fn $name` (src/ffi.rs's node_dim_setter!). A source heuristic,
# not a link map, so it is calibrated against the committed symbol file at the baseline
# before its candidate reading counts; --so is the built-library bearing.
src_exports() {
    local rev="$1" macros m
    {
        git grep -h -o -E 'extern "C" fn gibson_[A-Za-z0-9_]+' "$rev" -- src | sed 's/.*fn //' || true
        macros="$(git grep -E 'macro_rules![[:space:]]*[A-Za-z0-9_]+|extern "C" fn \$' "$rev" -- src \
            | awk '{
                f = $0; sub(/:.*/, "", f); rest = substr($0, length(f) + 2)
                p = rest; sub(/:.*/, "", p); line = substr(rest, length(p) + 2)
                if (p != cur) { cur = p; m = "" }
                if (line ~ /macro_rules!/) {
                    m = line; sub(/.*macro_rules![[:space:]]*/, "", m); sub(/[^A-Za-z0-9_].*/, "", m); next
                }
                if (m != "") print m
              }' | sort -u || true)"
        for m in $macros; do
            git grep -h -o -E "(^|[^A-Za-z0-9_])$m![[:space:]]*\([[:space:]]*gibson_[A-Za-z0-9_]+" "$rev" -- src \
                | sed 's/.*([[:space:]]*//' || true
        done
    } | sort -u
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
if git merge-base --is-ancestor "$B" "$C"; then
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
b_src="$(src_exports "$B")" c_src="$(src_exports "$C")"
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
for path in abi include; do
    if git diff --quiet "$B" "$C" -- "$path/"; then
        pass "C ABI: $path/ is byte-identical to the baseline"
    else
        fail "C ABI: $path/ differs from the baseline:"
        git diff --stat "$B" "$C" -- "$path/" | sed 's/^/       /'
    fi
done
if [ -n "$so" ]; then
    built="$(nm -D --defined-only "$so" 2>/dev/null | awk '$2=="T"{print $3}' | grep '^gibson_' | sort -u || true)"
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

# 6. branch
mm="${release_version%.*}"
branch_re="(^|/)release/(v${release_version//./\\.}-rc|${mm//./\\.})$"
branches="$(git for-each-ref --contains "$C" --format='%(refname:short)' refs/heads refs/remotes \
    | grep -E "$branch_re" | tr '\n' ' ' || true)"
if [ -n "$branches" ]; then
    pass "branch: candidate is on ${branches% }"
else
    fail "branch: no local or remote-tracking ref named release/v$release_version-rc or release/$mm contains the candidate"
fi

# 7. tags (local refs only)
for tag in "v$release_version" "bindings/go/v$release_version"; do
    if git rev-parse --verify --quiet "refs/tags/$tag" >/dev/null; then
        target="$(git rev-parse "refs/tags/$tag^{commit}")"
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
range="$(git rev-list --reverse "$B..$C")"
n_range="$(printf '%s\n' "$range" | count)"
echo "INFO  commits: $n_range in ${B:0:7}..${C:0:7}:"
# shellcheck disable=SC2086 # one commit id per word
[ -z "$range" ] || git log --no-walk=unsorted --format='        %h %s' $range
if [ -n "$allow_file" ]; then
    allowed=""
    while IFS= read -r raw || [ -n "$raw" ]; do
        line="${raw%%#*}"
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

if [ "$violations" -eq 0 ]; then
    echo "RESULT: PASS — ${C:0:12} meets the maintenance-line checks against baseline ${B:0:12}"
    exit 0
fi
echo "RESULT: FAIL — $violations violation(s); ${C:0:12} is not a valid maintenance candidate for baseline ${B:0:12}" >&2
exit 1
