# HumanMusic merge holdout v6 (closeout, final head)

Status: **configuration and harness committed after the final library source was re-frozen
(`abc3f9f`); no case executed at commit time.** First-contact results are recorded separately under `results/`.
Holdouts v1 (`../../humanmusic-consolidation/fresh/`), v2 (`../../humanmusic-hardening/holdout-v2/`),
v3 (`../holdout-v3/`), v4 (`../holdout-v4/`) and v5 (`../holdout-v5/`) are known evidence and stay
immutable; nothing in this round was fitted to them.

**Why v6 exists.** Holdout v5 passed 28/28 at its first contact (`7110cee`, source `3146438`) and is
preserved as recorded. AFTER that contact, the closeout listening diff found an accidental change in
`938ab3e`: the pad-tail law cut the final harmony's consonant tail before the piece's end in 360/450
BAND performances, a change no law states (every receipt still passed, so no holdout or search could
see it). It was repaired red-first (`c875d0d` → `abc3f9f`). The final implementation therefore needs
its own untouched contact: v6 repeats v5's reviewed 28-row design unchanged, except that every seed
moves to the unused `978xxxxx` range.

Configuration SHA256: `3a5c29e16f0ec72840283633cdad152d499c38b76264c0035a87c05055950870`
(`config.tsv`, 28 rows, 20 tab-separated fields). Harness: `tests/audio_holdout_v6.rs`
(`merge_holdout_v6`, ignored; release only; refuses a dirty tree and records the executed commit;
the output directory, `HUMANMUSIC_HOLDOUT_V6_OUT`, must be new). `holdout_v6_configuration_is_well_formed`
checks the configuration without executing a row.

## What every row is judged by

Unchanged from v4 (see `../holdout-v4/README.md`): every generated BAND source must pass its own
`PerformanceReceipt` (what `perform_checked` admits) — now including this round's laws (the sounding
support answers for harmonic identity; relational pitch functions name a destination that sounds and
resolves; a declared bass figure keeps the downbeat its own line needs); recorded provenance;
identity under overlap; a lift only as `CoverAdmission = PerformanceReceipt + CoverConformance`, not a
replay; the cross-swing Groove quotient where Groove is pinned with another identity axis.

## What changed from v4 (declarations only; decided before v5's contact, unchanged in v6)

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
- Seeds: `978001xx`, `978101xx`, `978102xx`, `978103xx` — none used by any earlier holdout (incl. v5's
  `977xxxxx`), sweep or this round's exploratory searches (`968xxxxx`-`969xxxxx`).

## Pre-contact declaration review (no execution; done for v5, the same design)

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
cargo +1.98.1 test --release --test audio_holdout_v6 holdout_v6_configuration_is_well_formed
HUMANMUSIC_HOLDOUT_V6_OUT=docs/fixtures/humanmusic-consolidation2/holdout-v6/results \
  cargo +1.98.1 test --release --test audio_holdout_v6 merge_holdout_v6 -- --ignored --nocapture
```
