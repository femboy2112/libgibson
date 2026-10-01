# HumanMusic merge holdout v5 (closeout)

Status: **configuration and harness committed after the final library source was frozen
(`3146438c77f540b32668cc1b33743ec3b5fcae50`; library source last changed at `938ab3e`); no case
executed at commit time.** First-contact results are recorded separately under `results/`.
Holdouts v1 (`../../humanmusic-consolidation/fresh/`), v2 (`../../humanmusic-hardening/holdout-v2/`),
v3 (`../holdout-v3/`) and v4 (`../holdout-v4/`) are known evidence and stay immutable; nothing in
this round was fitted to them.

Configuration SHA256: `c52d04217bcc9193452daf4fc4732463fd5d5ef852c8f1fd5ef8262027c1301f`
(`config.tsv`, 28 rows, 20 tab-separated fields). Harness: `tests/audio_holdout_v5.rs`
(`merge_holdout_v5`, ignored; release only; refuses a dirty tree and records the executed commit;
the output directory, `HUMANMUSIC_HOLDOUT_V5_OUT`, must be new). `holdout_v5_configuration_is_well_formed`
checks the configuration without executing a row.

## What every row is judged by

Unchanged from v4 (see `../holdout-v4/README.md`): every generated BAND source must pass its own
`PerformanceReceipt` (what `perform_checked` admits) — now including this round's laws (the sounding
support answers for harmonic identity; relational pitch functions name a destination that sounds and
resolves; a declared bass figure keeps the downbeat its own line needs); recorded provenance;
identity under overlap; a lift only as `CoverAdmission = PerformanceReceipt + CoverConformance`, not a
replay; the cross-swing Groove quotient where Groove is pinned with another identity axis.

## What changed from v4 (declarations only; decided before contact)

- **Declared grammar.** Every `StablePropulsion` row declares `PropulsiveReturn`, the grammar that
  composer selects by design (v4's V09/V14/V20 declaration error is not repeated).
- **Ode → SWISS derived harmony** (O02) is `lawful`, not `lift` (v4's O02 failed prediction).
- **`lift` is used sparingly** (6 rows: V01, V11, V12, V13, V15, O01).
- **Lawful refusal family** gains the library's documented lawful refusal *"a pinned theme has no
  seated phrase to be stated in the target form"*. `UnprojectableTiming` and *"a pinned stroke has no
  drum seat"* stay OUTSIDE the family: either would be a defect.
- **Swing nothing-to-cover** under a `lawful` row now passes, consistent with the declared meaning of
  `lawful` (v4 accepted it only under `nothing`; no v4 row was affected).
- **`riff-only`** selection added (`CoverSpec::new([Riff])`), exercised by V20.

## Coverage declared before contact

- 21 generated sources: BLACK_ICE, SWISS_SIGNAL, VAPOR95; HookArc, LoopEvolution, RiffDrive,
  WorldSwitch, DeflectedLift, PropulsiveReturn; StructuralR9, MeaningDirected, StablePropulsion;
  Simple and Fusion; every drum restraint; lengths 2.5-64 beats including very short (2.5, 4.0) and
  partial final bars (7.25, 13.25, 33.25); source tempo 132 (V18), target tempo 104 (V11).
- This round's stress shapes: rootless-support identity (V03 VAPOR95 fusion deflected 33.25, V04
  demo 7.25, V19 demo 64); truncated relational function (V05 BLACK_ICE fusion deflected 33.25);
  bass-figure room at one-bar forms (V06 RiffDrive 2.5 `bass-only`, V07 WorldSwitch 4.0).
- Cross-swing Groove with another identity axis: V09 (VAPOR95 swung → BLACK_ICE straight,
  groove+motif), V10 (BLACK_ICE → VAPOR95, groove+bass), V19 (VAPOR95 → VAPOR95, same-swing control).
- Motif / Riff / BassFigure / Groove selections; every fidelity preset; Ode (v1 motif-only, every
  preset, with and without the labelled derived harmony); Swing partial (Faithful, Strict, Loose).
- Refusal-bearing rows: V16 and V21 (a fusion source at preset-strict lifted into SWISS simple) and
  O02 are expected to exercise the vocabulary refusal; V08 may refuse its pinned attacks. O03 (Ode
  Strict → BLACK_ICE fusion) is `lawful`, **not** a predicted refusal.
- Seeds: `977001xx`, `977101xx`, `977102xx`, `977103xx` — none used by any earlier holdout, sweep or
  this round's exploratory searches (`968xxxxx`-`969xxxxx`).

## Pre-contact declaration review (no execution)

An adversarial review of every declaration (code reading, plus probes on seeds `9830xxxx` only — never
a v5 seed; run at `239da10`, before the final pad extension `938ab3e`) required **no row change**. It
confirmed the StablePropulsion grammar declarations, that each `lift` shape lifted with receipt +
conformance on 12 non-holdout seeds, that O02/V16 are refusal shapes and V17/S03 nothing-to-cover
shapes, and it produced the harness corrections listed above. V21 implements its recommendation of one
more refusal-bearing row; V21 itself was declared after that review, `lawful`.

**Boundary (Disclosed):** each stress row is ONE seed of a shape whose mechanism fired in roughly
1-8% of matching fresh cases before this round's laws; a row exercises the shape, it does not
guarantee the mechanism triggers. The exploratory searches carry the mechanism evidence.

## Declared expectations

`lift` (6 rows) is a prediction, not a law. `nothing` is declared for groove-only (V17) and Swing
Loose (S03). Every other row is `lawful`: a refusal in the family, nothing to cover, or an admitted
lift. A refusal outside the family, at the wrong stage, or against a `lift` declaration fails; so
does any lift that is not admitted, and any generated source whose own receipt fails.

## Execution (once)

```sh
cargo +1.98.1 test --release --test audio_holdout_v5 holdout_v5_configuration_is_well_formed
HUMANMUSIC_HOLDOUT_V5_OUT=docs/fixtures/humanmusic-consolidation2/holdout-v5/results \
  cargo +1.98.1 test --release --test audio_holdout_v5 merge_holdout_v5 -- --ignored --nocapture
```
