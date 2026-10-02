# Post-freeze broad BAND search (VAPORIZE)

**Observed** at library source `d175090` (the frozen VAPOR95 revision), with
`tests/audio_band_search.rs` (`perform_checked` under `PerformanceProfile::BAND.with_drum_restraint`,
every grammar × composer × trace × both languages × all four drum restraints, lengths
1.75 / 2.5 / 4 / 7.25 / 13.25 / 16 / 33.25 / 64 beats incl. partial final bars). Seeds unused by every
holdout, earlier search and the style lab. Full TSVs stay local; `not-admitted.tsv` lists every row
that was not admitted.

| set | seeds | worlds | target tempi | performances | admitted | refused | rejected |
| --- | --- | --- | --- | ---: | ---: | ---: | ---: |
| F | 98200000:24 | BLACK_ICE, SWISS_SIGNAL, VAPOR95 | nominal | 165,888 | 165,888 | 0 | 0 |
| G | 98300000:48 | VAPOR95 | nominal, 66, 96, 120, 150 | 552,960 | 552,948 | 0 | **12** |
| H | 98310000:48 | VAPOR95, VAPOR95_V1 (archival) | 96, 108, 120, 150 | 884,736 | 884,736 | 0 | 0 |
| I | 98320000:24 | BLACK_ICE, SWISS_SIGNAL | 132, 150 | 221,184 | 221,184 | 0 | 0 |
| **total** | | | | **1,824,768** | **1,824,756** | **0** | **12** |

No panic. Revised VAPOR95 alone: 1,050,624 performances, 12 rejected.

## The 12 rejections — one pre-existing family, preserved and classified (not repaired)

All 12 are ONE song — seed `98300007`, deflected trace, 33.25 beats, DeflectedLift, StructuralR9,
fusion, VAPOR95 — at target tempi 96, 120 and 150 BPM, under all four drum restraints (one held-identity
flip each). At 66 and the nominal 84 BPM it is admitted.

- **Not introduced by the palette revision (Observed):** the archival VAPOR95 v1 world rejects the same
  song at the same tempi (96, 108, 120, 150 BPM; admitted at 66, 71, 84, 90) — the family predates this
  round (it is present at the closeout source `abc3f9f`). Set H (the same fast tempi, 48 further seeds,
  both worlds side by side) admits 884,736 / 884,736.
- **Mechanism (Observed, instrumented probe; no source change kept):** Dm7 over beats 8-12, a Thin bar
  (pad silent); the keys hold a rootless F4-C5-E5; the bass leaves D for A2 at 9.75 → A minor holds
  1.4 s (the closeout's "K" shape). The keys' identity law offers the right answer (E5 → D5, as it does
  at 84 BPM, removing the flip completely), but its hold-law acceptance (`keeps_held_identity`) vetoes
  it: with the keys' D5 release tail now crossing 0.72 beats into the next harmony (Dm9), the pad's
  answer there changes and four instants of Dm9 go from Rooted to a SUB-HOLD Passing F major (longest
  run 0.498 s, just under the 0.5 s hold). The acceptance permits only Implied → Passing; "a root lost
  anywhere" vetoes — a deliberate closeout rule. Faster tempi stretch the seconds-long tail over more
  beats, which is why the family appears only at ≥ 96 BPM.
- **Disposition:** a repair would widen a deliberate closeout acceptance rule (allow trading a held
  flip for sub-hold passing rivals in a neighbouring harmony) — a semantic change this aesthetic round
  is not licensed to make. Reported as a remaining internal red in
  `docs/HUMAN_MUSIC_MERGE_READINESS.md`.

```sh
BAND_SEARCH_SEEDS=98300000:48 BAND_SEARCH_WORLDS=VAPOR95 BAND_SEARCH_TEMPI=nominal,66,96,120,150 \
BAND_SEARCH_LENGTHS=1.75,2.5,4,7.25,13.25,16,33.25,64 BAND_SEARCH_THREADS=7 BAND_SEARCH_OUT=G.tsv \
  cargo +1.98.1 test --release --test audio_band_search -- --ignored --nocapture
```
