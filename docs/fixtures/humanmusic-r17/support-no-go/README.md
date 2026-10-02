# Round XVII SWISS low-root octave-only search

Observed human localization at b028: muting G5 leaves the unwanted sound, while muting C4 removes it. The isolated-static-G5 explanation does not survive that intervention at b028. Low-root involvement is supported; a C4/B2 interaction versus patch timbre or masking remains unresolved. No new listening verdict is inferred at b036/b044.

The ignored source-unit probe enumerates the existing solver's complete finite same-pitch-class octave alternatives at b028/b036/b044. It reconstructs the frozen R16 source path, checks that source emission reproduces every pad pitch, onset, gate, and velocity, then realizes candidate source paths. It never changes a finished production Score. These probes and guards share implementation provenance; they are not independent listening evidence.

The unchanged constraints include the original register range and upper-layer obligation, no internal minor seconds/ninths, no newly introduced high-seventh witness, identity retention, the existing band-contact guard (including its frozen resolving-bass exception), new tail contacts at most 300 ms, and strictly improved adjacent motion. Every candidate is recorded in `candidates.tsv`; `contacts.txt` preserves the actual tested semitone/ninth contacts.

There are 58 alternatives per site; 32 remove C4. Of these, 24 survive the high-seventh and identity checks. Eight survive the band/tail guards at b028, zero at b036/b044; zero pass all unchanged source guards at any site. Every b028 band-admitted alternative raises the top to at least B5 and changes at least three voices. In particular, C5 E5 G5 B5 makes the old high-seventh witness false because its root is now the bottom pad voice, despite restoring the rejected high B5 register. That diagnostic blind spot is not acceptance evidence.

Verified finite NO-GO: no common C4-removing octave-only alternative survives the unchanged source gates. `support_top_voice` has no implemented treatment; its factorial duplicates are an unimplemented-factor boundary, not evidence of no perceptual effect. The C4 G4 B4 E5 control voicing is not promoted: it retains the human-implicated C4 and does not meet the existing G5 upper-layer obligation. No threshold, solver law, pitch-class set, or other player's notes were changed by this search.

Reproduce from the repository root:

```sh
HUMANMUSIC_SUPPORT_RECEIPT_DIR=target/humanmusic-r17/support-no-go \
  cargo +1.98.1 test --lib pocket_low_root_octave_only_no_go -- --ignored --nocapture
diff -u docs/fixtures/humanmusic-r17/support-no-go/candidates.tsv target/humanmusic-r17/support-no-go/candidates.tsv
diff -u docs/fixtures/humanmusic-r17/support-no-go/summary.txt target/humanmusic-r17/support-no-go/summary.txt
```

Boundary: this finite refusal does not rule out different articulation, timbre, voicing cardinality, register boundaries, or other newly authorized interventions. It does not establish that any generated candidate sounds good.

## Storage

Large text dumps in this directory, and exact duplicates of dumps kept elsewhere, are stored
compacted; no byte was discarded. Every original path, byte count and sha256 is in
`docs/fixtures/humanmusic-archive/MANIFEST.tsv`; the unique bytes are in the solid archive
`docs/fixtures/humanmusic-archive/blobs.tar.xz` (member name = sha256). See
[`HUMANMUSIC_FIXTURE_MANIFEST.md`](../../HUMANMUSIC_FIXTURE_MANIFEST.md) for the policy.

- List what was compacted here: `grep '^docs/fixtures/humanmusic-r17/support-no-go/' docs/fixtures/humanmusic-archive/MANIFEST.tsv | cut -f1,4,5`
- Restore every compacted file to its original path and bytes:
  `python3 scripts/fixtures/expand-humanmusic-archives.py expand` (or add `--dest /tmp/x` to
  grep a copy instead of touching the tree; expanded files are working copies, do not commit them).
- Verify all manifest hashes: `python3 scripts/fixtures/expand-humanmusic-archives.py check --expanded`
  (after `expand`, any older `sha256sum -c` manifest in this directory passes again).
- Without Python: `mkdir /tmp/blobs && tar -xJf docs/fixtures/humanmusic-archive/blobs.tar.xz -C /tmp/blobs`, then look up the
  member named by the sha256 in the manifest.
