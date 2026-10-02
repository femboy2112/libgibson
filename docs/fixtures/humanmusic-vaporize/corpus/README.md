# VAPOR95 revision listening corpus

**Observed** with `vapor95_style_lab` at the revision source (see `docs/HUMAN_MUSIC_VAPOR95.md` §4).
WAVs stay local under `target/humanmusic-vaporize/corpus/`; `WAV_SHA256SUMS` covers all 110. No file
here claims a musical result — the maintainer listens; aesthetic success is **UNVERIFIED**.

## Listen first (8 files)

One song (`hookarc`: HookArc, demo trace, 64 beats, seed 98000101, StructuralR9, fusion) through every
step, then a familiar tune:

| # | file | what it isolates |
| --- | --- | --- |
| 1 | `v1/hookarc.full.wav` | the old VAPOR95 (71 BPM) |
| 2 | `palette/hookarc.full.wav` | **same notes as 1**, new palette (timbre, mix, chorus, echo, room) |
| 3 | `current/hookarc.full.wav` | the new world: new palette at 84 BPM, light swing (the landed default) |
| 4 | `candidates/hookarc.t78_s16.full.wav` | candidate: slower, 78 BPM |
| 5 | `candidates/hookarc.t84_s32.full.wav` | candidate: 84 BPM, more swing (58 %) |
| 6 | `candidates/hookarc.t90_s00.full.wav` | candidate: 90 BPM, straight |
| 7 | `v1/ode_faithful.full.wav` | Ode to Joy (Faithful cover), old world |
| 8 | `current/ode_faithful.full.wav` | the same cover, new world |

If something sounds wrong, the stems beside each full mix (`*.keys|pad|bass|lead|drums.wav`) and the
single-factor ablations (`ablate/hookarc.{nochorus,noecho,fullband,nosat,dry}.full.wav`) localize it.

## Layout

- `v1/` — the five takes under the archival v1 world (byte-identical to `../v1-baseline/`).
- `palette/` — A: each v1 Score through the new palette (`palette-report.txt`: same `score=`).
- `current/` — B: the same SongMaps performed under the new world (`current-record.txt`).
- `ablate/` — hookarc and deflected, one production factor removed per file.
- `candidates/` — hookarc and deflected at four tempo/swing cells (84/0.16 is the default).

```sh
L=./target/release/examples/vapor95_style_lab C=target/humanmusic-vaporize/corpus
$L --record=$C/v1 --world=v1; $L --record=$C/current --world=current
$L --palette=$C/palette; $L --ablate=$C/ablate; $L --candidates=$C/candidates
```
