# M5-B Slice 1 evidence

**`golden_rev_id.py` is a REGRESSION PIN (not independent).** It is a numpy float32 replication
of the implementation, for the RICH synthetic fixture and recipe in
`tests/figure_mechanics.rs` (`rich_family`, `RICH_CTL`). It mirrors the implementation's float
evaluation order on purpose and shares its formulas, so it is not an independent bearing on
correctness. It pins the documented canonical byte layout and the f32 arithmetic order. The hash
is pinned in `tests/figure_mechanics.rs::golden_rev_id`.

    python3 -I docs/evidence/m5b/golden_rev_id.py
    # 0042bc1826da28bce4f31b222b6b86382f60da56e0c762076d8e2edb98063ed8

If the Rust test disagrees, diff `rev.canonical_bytes()` against this script's bytes. Never copy
the Rust output into the test. Bit-stability is claimed on x86_64 only. The script converts every
fixture literal from its decimal text with exact rounding, so it agrees with how rustc rounds a
float literal.

The independent bearings are the f64 oracle (`tests/figure_oracle/mod.rs`) and the hand-derived
canonical bytes test (`tests/figure_canonical_bytes.rs`).

`predict_kills.py` and `KILL_MATRIX_PREDICTED.md` are a PREDICTION of which tests kill which
planted mutant, from a python model. They are not a measurement.
