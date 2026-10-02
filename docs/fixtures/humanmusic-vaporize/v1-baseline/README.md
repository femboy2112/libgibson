# Legacy VAPOR95 (v1) palette baseline

**Observed** at library source `abc3f9f` (head `ef6a7e0`), before any VAPOR95 change: the style
record of the archival VAPOR95 v1 world (`tests/common/vapor95_v1.rs`, field-for-field equal to
`MusicWorld::vapor95()` at this source — `tests/audio_vapor95_palette.rs`).

Five representative BAND songs, each performed with `perform_checked` under `PerformanceProfile::BAND`
and admitted (fixed seeds; the SongMap is world-independent):

| take | trace | beats | seed | grammar | composer | language |
| --- | --- | ---: | ---: | --- | --- | --- |
| `hookarc` | demo | 64 | 98000101 | HookArc | StructuralR9 | fusion |
| `deflected` | deflected-lift | 64 | 98000202 | DeflectedLift | StructuralR9 | fusion |
| `propulsive` | demo | 64 | 98000303 | (PropulsiveReturn, selected by the composer) | StablePropulsion | simple |
| `riffdrive` | demo | 64 | 98000404 | RiffDrive | MeaningDirected | fusion |
| `ode_faithful` | Ode to Joy import, Faithful preset | — | 904 | HookArc | cover | default |

`record.txt` holds the world parameters, every SongMap / Score / PerformancePlan canonical
fingerprint, full-mix measurements (peak, RMS, ceiling contact, below-150 Hz side energy, tail level)
and per-bus RMS/peak. `WAV_SHA256SUMS` covers the 30 WAVs (full mix + keys/pad/bass/lead/drums stems
per take), which stay local under `target/humanmusic-vaporize/legacy-baseline/`.

```sh
cargo run --release --example vapor95_style_lab -- --record=target/humanmusic-vaporize/legacy-baseline --world=v1
```

After the revision the same command (`--world=v1`) must reproduce these bytes: the archival world is
the reproducibility contract for historical VAPOR95 audio.
