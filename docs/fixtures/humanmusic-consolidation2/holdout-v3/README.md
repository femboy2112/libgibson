# Consolidation holdout v3

Status: **configuration and harness committed after every consolidation-round source change was
frozen in commits (library source frozen at `362a4ddf9e99389db7a40d97a4e81ddf1b33c33d`); no case executed at commit time.** First-contact results are recorded separately under `results/`. Holdouts v1
(`../../humanmusic-consolidation/fresh/`) and v2 (`../../humanmusic-hardening/holdout-v2/`) are
known evidence and stay immutable; nothing in this round was fitted to them. v3 judges whether the
consolidation's laws hold on songs, worlds and covers nobody has looked at.

Configuration SHA256: `cf96dbe1fcd7414172e322fe0b2a5a960e8e806fd321229315ae4188bfccde90` (`config.tsv`,
48 rows, 20 tab-separated fields). Harness: `tests/audio_holdout_v3.rs` (`consolidation_holdout_v3`, ignored; release only; refuses a
dirty tree and records the executed commit; the output directory, `HUMANMUSIC_HOLDOUT_V3_OUT`, must
be new). `holdout_v3_configuration_is_well_formed` checks the configuration
without executing a row.

## What every row is judged by

- **The source's own contract.** Every generated source plays BAND at a declared drum restraint
  (rehearsed admission, arbitrated percussion, the harmonic vocabulary as a source law, earned
  support functions) and must pass the consolidated `PerformanceReceipt` under its profile — what
  `perform_checked` admits: song conformance, score domains, pitch functions present and true in
  time, no held-identity flip, causal hearings, lawful continuations, occupancy, every verb
  performed, nobody off stage, the performance domain, every declared anchor established, the
  vocabulary. A source that fails still proceeds to extraction and lift so its evidence is kept.
- **Recorded provenance.** The song's recorded composer and grammar are the declared ones.
- **Identity under overlap.** Wherever the map pins Motif, Riff or BassFigure, the source's natural
  overlaps (non-identity events at identity onsets) are reported, and a non-identity event added at
  an identity onset of each pinned role must leave the extracted map unchanged. The added event is
  that instrument's own kind of note - a lead statement note (`melody`) of a material that is not
  the song's theme, a bass `approach` - so only the identity projection can tell it apart.
- **Lawful refusal, nothing to cover, or admitted lift.** A refusal must be in the declared family
  (configuration header), at the stage that may make it. A selection that pins no song identity
  (the form scaffold alone) leaves nothing to cover. A lift is admitted only as `CoverAdmission =
  PerformanceReceipt + CoverConformance` under the target profile, with every pinned axis receipted,
  and must not replay its source.

## Coverage declared before contact

- 36 generated sources: BLACK_ICE, SWISS_SIGNAL and VAPOR95 (straight and swung) 12 each; all six
  grammars including WorldSwitch (5-7 each); all three composers, including StructuralR9 on the
  PropulsiveReturn grammar (V31, a provenance check: it must record StructuralR9); Simple and
  Fusion; every drum restraint. Lengths 2.5-64 beats: shorter than a bar (2.5, 3.0), exactly one
  bar (4.0), partial final bars in 30 of 36 rows.
- SWISS_SIGNAL PropulsiveReturn sources: V02, V20, V29 (the H05 family).
- Swung groove: VAPOR95 groove pinned into straight worlds (V03) and swung into swung (V15), straight
  into swung (V29), established VAPOR95 covers (V06, V21, V33).
- Selections: established (13), groove-free-established (2), partial covers motif-only (3),
  bass-only (2), groove-only (3), and every fidelity preset (Strict 3, Faithful 4, Interpretive 3,
  Loose 3).
- 8 Ode targets (every preset; with and without the labelled derived harmony; the v1 motif-only
  spec) and 4 Swing partial-chart rows (Loose/Interpretive declared to leave nothing; Faithful/Strict
  the skeleton).
- 84 seeds (`964xxxxx`: 36 sources, 48 targets), all distinct; a word-bounded search found none in `src`, `tests`,
  `examples`, `docs`, `scripts` or the compacted fixture archive before this configuration existed.

## Declared expectations

`lift` (12 rows) is a prediction, not a law: a generated source of at least a few bars covered into
its own world, language and grammar under its established spec (V02, V06, V13, V16, V21, V23, V28,
V31), the Ode under Loose and its v1 motif spec, and the Swing skeleton under Faithful/Strict. A
pinned line is re-judged in the target's own contexts (region, palette, gates), so even these may
meet a lawful refusal; a miss is recorded as a failed prediction. `nothing` is declared for Swing
Loose/Interpretive. Every other row is `lawful`: a refusal in the family, nothing to cover, or an
admitted lift. A refusal outside the family, at the wrong stage, or against a `lift` declaration
fails; so does any lift that is not admitted.

## Pre-freeze review

An adversarial read-only review (no execution) found, and root corrected before this commit:

- the overlap guard's lead event was tagged `quote`, which the projection rejects on its tag alone,
  so the material check was never exercised - now a `melody` note of a non-theme material, and the
  bass event a real `approach`;
- a map that pins only the form scaffold failed an "identity present" check even where `lawful`
  was declared (holdout v2's H22 shape) - now the declared `nothing to cover` outcome;
- two refusals the lift planner cannot reach by construction ('a pinned kit cannot carry a groove
  debt', 'a pinned stroke has no drum seat') were in the lawful family, so a broken planner
  contract would have passed - removed; MissingAxis and ConflictingPins are lawful only at
  extraction;
- four `lift` declarations (V01 3.0 and V33 2.5 beats: possibly no song identity; V11, V35:
  WorldSwitch/RiffDrive pin lines but no harmony, and SWISS re-derives harmony from a restricted
  chord set) were unjustified - now `lawful`;
- the run recorded no source provenance - it now refuses a dirty tree and records the commit;
- a library inconsistency visible before contact: a Groove anchor in a piece with no full bar was
  "realized" by any recorded pocket stroke yet could never conform - repaired at source
  (`a_groove_with_no_full_bar_is_inapplicable_not_a_deviation`) before this freeze;
- v2's derived-harmony provenance guard is restored.
