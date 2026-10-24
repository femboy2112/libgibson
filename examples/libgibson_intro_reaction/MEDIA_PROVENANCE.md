# Reaction-media provenance

The `libgibson_intro_reaction` finale can composite a keyed reaction performer over
the LibGibson intro film. For convenience, a baked copy of that reaction media is
bundled in `assets/` so the demo runs self-contained from a fresh checkout:

| File | Bytes | What it is |
|---|---:|---|
| `00_welcome.gmed` | 1,806,710 | keyed reaction frames (GMED container) |
| `01_prank_smash.gmed` | 365,325 | keyed reaction frames |
| `02_rant.gmed` | 3,041,138 | keyed reaction frames |
| `03_stop1.gmed` | 1,453,344 | keyed reaction frames |
| `04_stop2.gmed` | 871,928 | keyed reaction frames |
| `05_stop3.gmed` | 1,266,536 | keyed reaction frames |
| `06_wtf_quiet.gmed` | 1,086,214 | keyed reaction frames |
| `07_nice_prank.gmed` | 719,425 | keyed reaction frames |
| `08_idiot.gmed` | 1,334,435 | keyed reaction frames |
| `dialogue.m4a` | 3,051,943 | extracted dialogue audio |

These are **derived** artifacts (chroma-keyed frames + extracted audio), not the
original video. The original source clip is **not** committed.

## Source and license status

The reaction media is derived from the green-screen footage that the *Filthy Frank*
(TVFilthyFrank / TooDamnFilthy) channel released publicly for free use on
2015-12-24 — a compilation of several green-screen bits (this cut's cue source
times span roughly 0–301 s, so it draws from the multi-bit greenscreen set, not any
single short clip). That footage is publicly mirrored on the Internet Archive, where
anyone can download it and reproduce the bake themselves:

- Internet Archive, TooDamnFilthy uploads (green-screen releases):
  <https://archive.org/details/itstimetostop_202001> and
  <https://archive.org/details/ItsTimeToStop>
- Full channel archive (contains the green-screen uploads):
  <https://archive.org/details/filthy-frank-youtube-channel-1080p>

> **Redistribution rights are NOT yet concretely verified.** "Free to use" from a
> 2015-era fan upload does not by itself establish a clear, durable redistribution
> license today, and the exact source item used for this bake has not been pinned to
> a specific archived file. **TODO (maintainer):** confirm the precise source item
> and that its stated terms permit redistributing these derived frames/audio, and
> record the finding here. Until then these bundled binaries are present for local
> demo convenience only and are **not** asserted to be cleared for redistribution.

## How to supply the media yourself

You do not need the bundled copy. Download the free-use source from the Internet
Archive (above) and point the example at it:

```
# bake + play from your own local source clip
cargo run --example libgibson_intro_reaction --features embedded-media -- --clip=/path/to/your/greenscreen.mp4 --live
```

`--clip=PATH` bakes from a local source; `--live` forces a fresh bake instead of the
bundled embed. With no flags (and `--features embedded-media`) the example plays the
bundled embed.

## Distribution boundary

- **The published crate is unaffected.** `Cargo.toml` excludes `/examples` from the
  packaged crate, so none of this media ships in the library.
- **The release artifact keeps third-party media out of the release path.** Pending
  verified rights, the v0.5.0 release should ship an original/synthetic chroma-key
  fixture for CI and local review, with real media loaded only via `--clip=PATH` on
  the maintainer's machine — not the bundled Frank-derived binaries.
