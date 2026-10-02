# HumanMusic merge holdout v4

Status: **configuration and harness committed after the final pre-main source was frozen
(`006bacfda5d7c79f2e3c9266f78095d4bf070363`); no case executed at commit time.** First-contact
results are recorded separately under `results/`. Holdouts v1
(`../../humanmusic-consolidation/fresh/`), v2 (`../../humanmusic-hardening/holdout-v2/`) and v3
(`../holdout-v3/`) are known evidence and stay immutable; nothing in this round was fitted to them.

Configuration SHA256: `27be8e4a31e8a7c871e9606282139318f59f26d0ad27017dc526881baaed57ba` (`config.tsv`,
28 rows, 20 tab-separated fields). Harness: `tests/audio_holdout_v4.rs` (`merge_holdout_v4`, ignored;
release only; refuses a dirty tree and records the executed commit; the output directory,
`HUMANMUSIC_HOLDOUT_V4_OUT`, must be new). `holdout_v4_configuration_is_well_formed` checks the
configuration without executing a row.

## What every row is judged by

- **The source's own contract.** Every generated source plays BAND at a declared drum restraint
  (rehearsed admission, arbitrated percussion, the harmonic vocabulary as a source law, earned
  support functions) and must pass the consolidated `PerformanceReceipt` under its profile — what
  `perform_checked` admits: song conformance, score domains, pitch functions present and true in
  time, no held-identity flip, causal hearings, lawful continuations, occupancy, every verb
  performed, nobody off stage, the performance domain, every declared anchor established, the
  vocabulary. A red source still proceeds to extraction and lift so its evidence is kept.
- **Recorded provenance.** The song's recorded composer and grammar are the declared ones.
- **Identity under overlap.** Wherever the map pins Motif, Riff or BassFigure, the source's natural
  overlaps are reported, and a non-identity event added at an identity onset of each pinned role
  must leave the extracted map unchanged.
- **Lawful refusal, nothing to cover, or admitted lift.** A refusal must be in the declared family,
  at the stage that may make it. A selection that pins no song identity (the form scaffold alone)
  leaves nothing to cover. A lift is admitted only as `CoverAdmission = PerformanceReceipt +
  CoverConformance` under the target profile, with every pinned axis receipted, and must not replay
  its source.
- **Cross-swing Groove quotient.** When the map pins `CoverAxis::Groove` together with another
  song-identity axis, `cross_swing_groove_quotient` restates the law independently of
  `CoverConformance`: every pinned stroke's canonical METRIC position, carried through the target's
  declared swing transport, must sound as a realized pinned stroke of the same voice, and the
  transport must preserve the metric source. The performed float onsets are **not** required to
  match across worlds; the check reports how many the transport moved and never asserts equality of
  the floats.

## Coverage declared before contact

- 20 generated sources: BLACK_ICE, SWISS_SIGNAL and VAPOR95, all six grammars (HookArc,
  LoopEvolution, RiffDrive, WorldSwitch, DeflectedLift, PropulsiveReturn), all three composers,
  Simple and Fusion, every drum restraint, lengths 2.5-64 beats including very short (2.5, 4.0),
  one bar (via the shortest forms) and partial final bars (7.25, 13.25, 33.25).
- This round's declared gap, absent from v3: **a swung Groove crossing into a target with a
  different swing law while another actual song-identity axis is pinned** — V06 (SWISS straight →
  VAPOR95 swung, groove+motif), V07 (VAPOR95 swung → BLACK_ICE straight, groove+motif, declared
  `lift`), V08 (VAPOR95 → SWISS, groove+harmonic), V09 (BLACK_ICE → VAPOR95, groove+bass), V20
  (VAPOR95 → VAPOR95, groove+motif, same-swing control, declared `lift`). V17 is groove-only and
  must leave nothing to cover.
- Identity overlap is exercised wherever a map pins Motif/Riff/BassFigure; Motif, Riff and
  BassFigure selections are all present.
- 8 Ode targets (v1 motif-only; every fidelity preset; with and without the labelled derived
  harmony) and 4 Swing partial-chart rows (Loose/Interpretive declared to leave nothing;
  Faithful/Strict the skeleton).
- Seeds (`974xxxxx` and `9741xxxx`): 48 distinct, none used by any earlier holdout or sweep.

## Declared expectations

`lift` (14 rows) is a prediction, not a law. `nothing` is declared for Swing Loose/Interpretive and
groove-only. Every other row is `lawful`: a refusal in the family, nothing to cover, or an admitted
lift. A refusal outside the family, at the wrong stage, or against a `lift` declaration fails; so
does any lift that is not admitted.

## Pre-freeze review

A pre-freeze adversarial review (no holdout execution) found that two candidate rows (V06, V12)
combined an intent trace with an incompatible grammar at a partial final bar. Those combinations
are **kept** in the declaration: the underlying truncated-form conformance bugs they exposed were
repaired at source (`006bacf`, with falsifiers `audio_band_truncated_form`), so both rows now hold
their own receipt and are exercised rather than avoided.
