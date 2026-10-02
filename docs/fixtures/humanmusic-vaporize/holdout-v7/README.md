# HumanMusic merge holdout v7 (VAPORIZE, final head)

Status: **configuration and harness committed after the final library source was frozen
(`d175090`, the VAPOR95 revision) and after the post-freeze broad BAND search; no case executed at
commit time.** First-contact results are recorded separately under `results/`. Holdouts v1–v6 are
known evidence and stay immutable (v6: `../../humanmusic-consolidation2/holdout-v6/`, 28/28 at
`04c5c26`, source `abc3f9f`); nothing in this round was fitted to them.

**Why v7 exists.** The library source changed after v6's contact: the VAPOR95 world was deliberately
revised (palette, the typed `WorldProduction` law, 84 BPM). Patch envelopes are part of the musical
model (sounding tails feed the BAND identity and expression laws), so the revised world re-performs
every VAPOR95 take, and the final implementation needs its own untouched contact.

Configuration SHA256: `3eb9b3a2e072e2aead14d5807ac8327e18d6f1ab8474eeb92f71a7fb2941ba34`
(`config.tsv`, 28 rows, 20 tab-separated fields). Harness: `tests/audio_holdout_v7.rs`
(`merge_holdout_v7`, ignored; release only; refuses a dirty tree and records the executed commit;
the output directory, `HUMANMUSIC_HOLDOUT_V7_OUT`, must be new). It is v6's harness with only the
version strings and the configuration path changed. `holdout_v7_configuration_is_well_formed`
checks the configuration without executing a row.

## Design

v6's reviewed 28-row design (see `../../humanmusic-consolidation2/holdout-v6/README.md` for every
law a row is judged by, the declared grammar rule for StablePropulsion, the lawful refusal family and
the cross-swing Groove quotient), with:

- every seed moved to the unused `984xxxxx` range (`984001xx` sources, `984101xx` targets, `984102xx`
  Ode, `984103xx` Swing) — none used by any earlier holdout, sweep, the style lab (`98000xxx`) or this
  round's searches (`9820xxxx`, `9830xxxx`);
- three rows biased toward the revised VAPOR95, all `lawful` (no new `lift` prediction):
  **V02** SWISS_SIGNAL → VAPOR95 (StablePropulsion, PropulsiveReturn), **V06** the one-bar
  bass-figure stress as VAPOR95 → VAPOR95, **V17** VAPOR95 (swung) → SWISS_SIGNAL straight
  (groove-only, `nothing`).

VAPOR95 is source or target in 15 of 28 rows: VAPOR95 → VAPOR95 (V03, V04, V06, V12, V14, V18,
V19), BLACK/SWISS → VAPOR95 (V02, V10, V20), VAPOR95 → BLACK/SWISS (V09, V16, V17), Ode → VAPOR95
(O04), Swing partial → VAPOR95 (S03); straight ↔ swung groove transport (V09, V10; V19 the
same-swing control — V17's groove-only selection stops at nothing-to-cover, no transport); partial and
very short forms; every fidelity preset; identity under overlap; the vocabulary refusal (V16, V21,
O02). `lift` stays on 6 rows (V01, V11, V12, V13, V15, O01).

"VAPOR95" is `MusicWorld::vapor95()` at the executed source; the archival v1 world is never used.

## Pre-contact declaration review (no execution)

An adversarial review of every declaration (fork C) ran v7's own `run_case` on four variants of every
row (112 cases) with probe seeds `9850xxxx`/`9851xxxx` at `d175090` — never a `984xxxxx` seed — in a
standalone scratch build, and required **no row change**: **Observed** 112/112 cases pass. Every
`lift` shape (V01, V11 at 104 BPM, V12 VAPOR95 → VAPOR95, V13, V15, O01) lifted 4/4 with receipt and
conformance; the changed rows V02 and V06 lifted 4/4 and V17 ended 4/4 at nothing-to-cover
(`spec=[Groove]`); V16, V21 and O02 refused 4/4 inside the declared family; V08 refused 2/4 inside the
family and lifted 2/4; the three StablePropulsion rows declare PropulsiveReturn. No seed is reused.
Boundary: the probe build was not LTO; equivalence with the release build is inferred (no fast-math).

## Execution (once)

```sh
cargo +1.98.1 test --release --test audio_holdout_v7 holdout_v7_configuration_is_well_formed
HUMANMUSIC_HOLDOUT_V7_OUT=docs/fixtures/humanmusic-vaporize/holdout-v7/results \
  cargo +1.98.1 test --release --test audio_holdout_v7 merge_holdout_v7 -- --ignored --nocapture
```
