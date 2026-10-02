# HumanMusic compacted-fixture archive

Storage for the HumanMusic evidence that used to be ~1,000 loose text dumps. Nothing here is a
new claim; it is the same bytes, stored once and compressed.

| file | what |
|---|---|
| `MANIFEST.tsv` | one row per compacted file: `path  bytes  sha256  storage  ref  class`. `storage=archive`: the bytes are the member named `sha256` in `blobs.tar.xz`. `storage=inplace`: identical bytes still live in the tree at `ref`. |
| `blobs.tar.xz` | solid, deterministic tar.xz (GNU format, mtime 0, root owner, `xz -9e`) with one member per distinct sha256, named by that sha256. |

Commands (stdlib Python; `xz -d`/`tar` also work):

```
python3 scripts/fixtures/expand-humanmusic-archives.py check --expanded   # verify all hashes (after expand)
python3 scripts/fixtures/expand-humanmusic-archives.py check              # verify archive + in-place refs only
python3 scripts/fixtures/expand-humanmusic-archives.py expand             # restore in place
python3 scripts/fixtures/expand-humanmusic-archives.py expand --dest /tmp/hm   # restore a copy
python3 scripts/fixtures/compact-humanmusic-fixtures.py build             # rebuild (needs an expanded tree)
python3 scripts/fixtures/compact-humanmusic-fixtures.py report            # per-directory before/after table
```

Expanded files are working copies: do not commit them. Full policy, classes and per-directory
accounting: [`../HUMANMUSIC_FIXTURE_MANIFEST.md`](../HUMANMUSIC_FIXTURE_MANIFEST.md).
