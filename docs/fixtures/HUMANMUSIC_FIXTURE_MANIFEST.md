# HumanMusic fixture manifest (storage compaction)

Scope: `docs/fixtures/humanmusic-*`. Pre-compaction tree = revision `5ad153e`
(`fix(audio): a finite form only promises the anchors it can state (U2)`).
This is a storage change only: **no claim, number, verdict or receipt changed, and no byte is
unrecoverable** (round trip proven below). Baseline audit: 1,491 files / 95,220,730 B at `a84ca08`
(the four `humanmusic-hardening/` files added since are untouched).

## Totals

| | files | bytes |
|---|---:|---:|
| before (`5ad153e`, incl. 4 hardening files / 11,456 B) | 1,495 | 95,232,186 |
| after (working tree, incl. archive + manifest + `RECEIPTS_SHA256SUMS` + 4 new READMEs) | 508 | 6,356,208 |

- Compacted out of the tree: **994 files / 89,469,235 B**. 331 of them are exact duplicates whose
  identical bytes still sit in the tree (`storage=inplace`: 25,662,361 B); 663 live in the archive
  (63,806,874 B, which collapses to **246 unique blobs = 25,043,095 B, stored as a 285,524 B solid
  `tar.xz`**, about 1.1 %).
- New infrastructure: `humanmusic-archive/{MANIFEST.tsv 219,510 B, blobs.tar.xz 285,524 B, README.md}`,
  `humanmusic-r17/final/RECEIPTS_SHA256SUMS` (72,277 B), READMEs for r15/r16/r17.
- The repository history still contains the old blobs (no history rewrite was made or is implied);
  the saving is in the working tree and in every future checkout of this tree, not in `.git`.

## Classes and policy

| class | meaning here | treatment |
|---|---|---|
| golden regression fixture | read by Rust (`tests/audio_consolidation_characterization.rs` reads 22 `r17/final/{black_ice,swiss}_r17.*.txt`; `include_str!` of `cover/ode-import/reference.tsv` and `consolidation/fresh/config.tsv`) | stays in place, byte-identical |
| minimal falsifier | first-contact holdout failures (`consolidation/fresh/**`, r16 holdout sweeps), `baseline-failures.txt` (r13, r14), `r17/diagnostic-failures`, `r17/support-no-go` README + `candidates.tsv`, hardening baseline | stays in place, greppable; `consolidation/` and `hardening/` never touched |
| reproducible derived receipt | verifier/generator-backed, e.g. `r17/final/*.txt` (`scripts/verify-pocket-freeze.py`), `r17/baseline` (`scripts/dev/humanmusic_r17_witnesses.py`), every hash manifest | pinned by sha256 manifest; bulky ones archived |
| archival experiment output | test logs, fuzz/PTY/render logs, `hearings`/`temporal`/`audit`/`mass`/`notes` dumps of superseded rounds | text `>= 16 KiB` moved to the archive |
| exact duplicate / redundant output | same sha256 at several paths (factorial cells, r16/r17 re-dumps, cover post-repair copies) | one canonical copy (in tree if any member is protected, else one archive blob); every other path is a manifest row |

Rules actually applied (`scripts/fixtures/compact-humanmusic-fixtures.py`): never compact
`consolidation/`, `hardening/`, `cover/ode-import`, `cover/sources`, golden files, falsifier files,
any `*SHA256SUMS*`/`*manifest*`, `*.md`, `*.py`, `*.gz`, `*.meta`, non-text; otherwise compact
`.txt/.tsv/.json/.log/.jsonl` of `>= 16 KiB`, or any non-empty exact duplicate.

## Where each removed byte is recoverable from

Every compacted file has one row in `docs/fixtures/humanmusic-archive/MANIFEST.tsv`
(`path, bytes, sha256, storage, ref, class`):

- `storage=inplace`: identical bytes remain at `ref` (a protected/golden canonical copy).
- `storage=archive`: the bytes are the member named `sha256` in
  `docs/fixtures/humanmusic-archive/blobs.tar.xz` (deterministic: GNU tar, sorted, mtime 0, root
  owner, `xz -9e`; `tar -xJf` or Python `tarfile`+`lzma` read it).

Restore / check (stdlib Python):

```
python3 scripts/fixtures/expand-humanmusic-archives.py expand                # in place
python3 scripts/fixtures/expand-humanmusic-archives.py expand --dest /tmp/hm # or a copy
python3 scripts/fixtures/expand-humanmusic-archives.py check --expanded      # all manifest hashes
python3 scripts/fixtures/compact-humanmusic-fixtures.py report               # regenerate the table below
```

Expanded files are working copies; do not commit them.

## How each retained claim stays reproducible

- **R17 pocket freeze (`r17/final`)**: `docs/fixtures/humanmusic-r17/final/RECEIPTS_SHA256SUMS` pins
  every one of the 759 original `final/*.txt` receipts by original name and sha256;
  `python3 scripts/verify-pocket-freeze.py <regenerated dir>` now compares each regenerated file's sha256
  to that manifest (sha256 equality is the same byte-for-byte claim as the old `read_bytes()` compare)
  and, on a mismatch, recovers the frozen original from the tree/archive and prints a diff head.
  WAV hashes still come from the untouched `final/SHA256SUMS`.
- **Rust golden tests**: the 22 files `tests/audio_consolidation_characterization.rs` reads, and both
  `include_str!` TSVs, are unmoved and byte-identical (verified by the round trip below).
- **Dev generators** `scripts/dev/humanmusic_r17_witnesses.py` / `humanmusic_r17_receipts.py` (read
  `r16/final/*.txt`): they call `ensure_expanded` and restore missing files from the archive first.
  `scripts/dev/humanmusic_r17_knockouts.py` reads only `r16/final/SHA256SUMS` (unmoved).
- **Older `sha256sum -c` manifests** (`cover/listening/SHA256SUMS`, `r17/baseline/SOURCE_SHA256SUMS`,
  `r17/verification/*.sha256`, ...): unchanged; they pass again after `expand` (implied by the byte-identical round trip; `cover/listening/SHA256SUMS` and `r17/final/RECEIPTS_SHA256SUMS` were also run through `sha256sum -c` on the expanded copy).
- **Holdout v1 and hardening baseline**: byte-untouched.

## Proof

Round trip: the tree was copied to a scratch directory, `expand` run there, and **all 1,491 files
of the pre-change inventory** (path, size, sha256 from the audit `inventory.tsv`) compared:
1,483 byte-identical; the other 8 are existing READMEs that gained a 17-line `## Storage` section (numstat: +17 / -0 each, nothing else changed). `expand-humanmusic-archives.py check --expanded` reported 994 rows / 246 blobs /
0 problems. Re-running `compact-humanmusic-fixtures.py build` on the expanded copy reproduced
`blobs.tar.xz`, `MANIFEST.tsv` and `RECEIPTS_SHA256SUMS` byte-identically.
Verifier: old script on the old tree and new script on the new tree both report `pass`, 48 WAVs,
757 receipts against `target/humanmusic-hardening/r17-after-u1/`; a tampered receipt is rejected by the new one.

## Per-directory accounting

Regenerate: `python3 scripts/fixtures/compact-humanmusic-fixtures.py report --before-rev 5ad153e`
(files/bytes per leaf directory: `5ad153e` vs the working tree; class column is editorial).

| directory | class | files before | bytes before | files after | bytes after |
|---|---|---:|---:|---:|---:|
| `humanmusic-r17/final` | golden (22 files read by Rust) + reproducible derived receipts (713 compacted, ~330 exact duplicates) | 766 | 72853075 | 54 | 2738547 |
| `humanmusic-r16/final` | archival + exact duplicates; first-contact holdout sweeps and WAV manifests kept | 84 | 7937962 | 11 | 31980 |
| `humanmusic-r16/frozen` | archival experiment output (R14/R15 dumps; partly exact duplicates of `final/`) | 37 | 4981561 | 1 | 452 |
| `humanmusic-consolidation/fresh/results` | minimal falsifier (holdout v1 first contact) - IMMUTABLE, untouched | 41 | 1294458 | 41 | 1294458 |
| `humanmusic-r15/final` | archival experiment output + exact duplicates; `r14-control-sweep.txt` kept | 32 | 1186942 | 7 | 18872 |
| `humanmusic-cover/listening/swing-partial` | archival experiment output (hearings/temporal dumps); manifests/receipt kept where small | 27 | 1165778 | 12 | 28818 |
| `humanmusic-cover/listening/ode` | archival experiment output (hearings/temporal dumps); manifests/receipt kept where small | 32 | 628257 | 23 | 107383 |
| `humanmusic-r12/final` | archival experiment output; `final-manifest.json` kept | 26 | 609117 | 4 | 18086 |
| `humanmusic-r17/verification` | reproducible derived receipt / archival logs; summaries + hash manifest kept | 21 | 478058 | 16 | 46181 |
| `humanmusic-r12` | archival experiment output; `final-manifest.json` kept | 17 | 315306 | 13 | 48627 |
| `humanmusic-cover/listening/generated` | archival experiment output (hearings/temporal dumps); manifests/receipt kept where small | 17 | 291896 | 9 | 21897 |
| `humanmusic-cover/ode-import` | golden regression fixture (`reference.tsv` read by Rust) + derived receipts - untouched | 8 | 272300 | 8 | 272300 |
| `humanmusic-r11` | archival experiment output (test/PTY/render logs) | 15 | 269094 | 7 | 17107 |
| `humanmusic-r15` | archival experiment output + exact duplicates; `r14-control-sweep.txt` kept | 21 | 267678 | 18 | 29234 |
| `humanmusic-r12/baseline` | archival experiment output (R11 baseline audits) | 8 | 267628 | 0 | 0 |
| `humanmusic-consolidation/known-r17-replay` | reproducible derived receipt - untouched | 9 | 210337 | 9 | 210337 |
| `humanmusic-r16` | archival + exact duplicates; first-contact holdout sweeps and WAV manifests kept | 33 | 206896 | 32 | 92458 |
| `humanmusic-r13/final` | archival experiment output; `baseline-failures.txt` (falsifier) kept | 18 | 181675 | 12 | 87229 |
| `humanmusic-r14` | archival experiment output; `baseline-failures.txt` (falsifier) + manifest kept | 10 | 173783 | 8 | 28699 |
| `humanmusic-r13` | archival experiment output; `baseline-failures.txt` (falsifier) kept | 11 | 164716 | 9 | 34134 |
| `humanmusic-r17/support-no-go` | minimal falsifier (no-go README + `candidates.tsv` kept); `contacts.txt` archived | 5 | 160836 | 4 | 20622 |
| `humanmusic-r13b` | archival experiment output; `final-manifest.json` kept | 9 | 156186 | 7 | 23821 |
| `humanmusic-r13/baseline` | archival; `baseline-failures.txt` is the minimal falsifier and stays | 8 | 140099 | 3 | 36895 |
| `humanmusic-r13b/final` | archival experiment output; `final-manifest.json` kept | 28 | 129261 | 20 | 33712 |
| `humanmusic-cover/listening` | archival experiment output (hearings/temporal dumps); manifests/receipt kept where small | 6 | 116863 | 5 | 44673 |
| `humanmusic-r14/frozen` | archival experiment output; `baseline-failures.txt` (falsifier) + manifest kept | 13 | 110038 | 5 | 46705 |
| `humanmusic-r17/knockouts` | reproducible derived receipt - untouched | 4 | 98151 | 4 | 98151 |
| `humanmusic-r14/final` | archival experiment output; `baseline-failures.txt` (falsifier) + manifest kept | 29 | 91000 | 13 | 45809 |
| `humanmusic-consolidation/known-r17-replay-final` | reproducible derived receipt - untouched | 5 | 68652 | 5 | 68652 |
| `humanmusic-r17/baseline` | reproducible derived receipt (`scripts/dev/humanmusic_r17_witnesses.py`); hash manifests kept | 6 | 54241 | 3 | 25764 |
| `humanmusic-cover/listening-post-repair/ode` | archival experiment output (mostly exact duplicates of `listening/`) | 8 | 33391 | 4 | 544 |
| `humanmusic-cover/integration` | reproducible derived receipt (parser/integration/development receipts) - untouched | 10 | 28946 | 10 | 28946 |
| `humanmusic-cover/listening-post-repair/generated` | archival experiment output (mostly exact duplicates of `listening/`) | 5 | 28445 | 4 | 3924 |
| `humanmusic-cover/development-review` | reproducible derived receipt (parser/integration/development receipts) - untouched | 10 | 26609 | 10 | 26609 |
| `humanmusic-consolidation/fresh` | minimal falsifier (holdout v1 first contact) - IMMUTABLE, untouched | 3 | 21159 | 3 | 21159 |
| `humanmusic-r13b/independent` | reproducible derived receipt - untouched | 8 | 20365 | 8 | 20365 |
| `humanmusic-cover/sources/mutopia-528` | golden source (public-domain Mutopia #528) - untouched | 5 | 20033 | 5 | 20033 |
| `humanmusic-cover/listening/raw/swing-partial` | reproducible derived receipt (deterministic gz of changed originals) - untouched | 10 | 18219 | 10 | 18219 |
| `humanmusic-consolidation/baseline` | reproducible derived receipt - untouched | 5 | 16239 | 5 | 16239 |
| `humanmusic-cover/listening-post-repair/swing-partial` | archival experiment output (mostly exact duplicates of `listening/`) | 7 | 15419 | 5 | 680 |
| `humanmusic-cover/listening/diagnosis` | archival experiment output (hearings/temporal dumps); manifests/receipt kept where small | 8 | 15283 | 7 | 11740 |
| `humanmusic-cover/final-parser` | reproducible derived receipt (parser/integration/development receipts) - untouched | 9 | 14598 | 9 | 14598 |
| `humanmusic-cover/listening/logs` | archival experiment output - untouched (small) | 3 | 13534 | 3 | 13534 |
| `humanmusic-cover/listening/raw/ode` | reproducible derived receipt (deterministic gz of changed originals) - untouched | 12 | 13283 | 12 | 13283 |
| `humanmusic-hardening/baseline` | minimal falsifier / baseline (this round) - IMMUTABLE, untouched | 4 | 11456 | 4 | 11456 |
| `humanmusic-consolidation/verification` | reproducible derived receipt - untouched | 9 | 10821 | 9 | 10821 |
| `humanmusic-cover/listening-post-repair` | archival experiment output (mostly exact duplicates of `listening/`) | 4 | 8007 | 4 | 9228 |
| `humanmusic-consolidation/pocket-freeze-final` | reproducible derived receipt - untouched | 4 | 6448 | 4 | 6448 |
| `humanmusic-r17` | experiment contracts - untouched | 2 | 5482 | 3 | 7656 |
| `humanmusic-cover` | reproducible derived receipt (parser/integration/development receipts) - untouched | 1 | 5175 | 1 | 5175 |
| `humanmusic-cover/listening/raw/generated` | reproducible derived receipt (deterministic gz of changed originals) - untouched | 6 | 4815 | 6 | 4815 |
| `humanmusic-cover/integration/raw` | reproducible derived receipt (parser/integration/development receipts) - untouched | 5 | 4211 | 5 | 4211 |
| `humanmusic-r17/diagnostic-failures` | minimal falsifier - untouched | 1 | 2523 | 1 | 2523 |
| `humanmusic-cover/development-review/raw` | reproducible derived receipt (parser/integration/development receipts) - untouched | 4 | 2137 | 4 | 2137 |
| `humanmusic-cover/swing-partial` | reproducible derived receipt (parser/integration/development receipts) - untouched | 2 | 2071 | 2 | 2071 |
| `humanmusic-cover/development` | reproducible derived receipt (parser/integration/development receipts) - untouched | 1 | 1208 | 1 | 1208 |
| `humanmusic-cover/listening/raw/diagnosis` | reproducible derived receipt (deterministic gz of changed originals) - untouched | 3 | 465 | 3 | 465 |
| `humanmusic-archive` | infrastructure (new): archive + manifest | 0 | 0 | 3 | 506518 |
| **total** |  | 1495 | 95232186 | 508 | 6356208 |

Notable files: golden `humanmusic-r17/final/{black_ice,swiss}_r17.*.txt` (22, ~2.6 MB, largest kept
in place because Rust reads them from that path); `humanmusic-consolidation/fresh/results/run.log`
(643 KB) is holdout v1 and stays; `humanmusic-cover/ode-import/{observations,midi-observations}.json`
(symbolic-source observations) stay untouched.

## Correction after cherry-pick (hardening round)

`humanmusic-r17/verification/fresh-failures.json` and `fresh-sweep.txt` were compacted, but the
committed known-sweep classifier (`humanmusic-consolidation/known-r17-replay/classify.py`) reads them
directly; the compaction's reference search covered `src/ tests/ examples/ scripts/`, not tooling under
`docs/`. Both were restored in place, byte-identical to their manifest rows (their archive rows are kept,
so `expand-humanmusic-archives.py check` still reports 0 problems). Working-tree size grows by their bytes.
