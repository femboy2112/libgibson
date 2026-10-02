# Holdout v7 — first contact (preserved as recorded)

**Observed.** Executed once, release (`debug_assertions false`), clean tree, at the declaration commit
`6512cd311eb4bdc29bbb6e5ae7999721c14f770a` (library source `d175090`, the frozen VAPOR95 revision).
Configuration SHA256 `3eb9b3a2e072e2aead14d5807ac8327e18d6f1ab8474eeb92f71a7fb2941ba34`. Raw receipts:
`results/` (31 files, unedited).

**Result: 28/28 PASS — 510 checks, 0 failing.** No undeclared internal red.

| outcome | rows |
| --- | --- |
| generated BAND source passes its own `PerformanceReceipt` | 21/21 (V01–V21) |
| predicted `lift`, admitted (`PerformanceReceipt` + `CoverConformance`) | 6/6: V01, V11 (104 BPM), V12 (VAPOR95 → VAPOR95), V13, V15, O01 |
| `lawful`, lifted and admitted | V02–V10, V14, V18, V19, V20, O03, O04, S01, S02 |
| `lawful`, explicit refusal in the declared family | V16, V21, O02 — "pinned harmony outside target vocabulary" |
| `nothing` | V17 ("no song identity survives the selection: spec=[Groove]"), S03 (no observed axis survives Loose) |
| cross-swing Groove quotient | 12/12 PASS (V01, V02, V03, V05, V09, V10, V11, V12, V13, V14, V18, V19) |

VAPOR95 rows at the revised world (nominal 84 BPM): V02 (SWISS → VAPOR95), V03, V04, V06, V12, V14,
V18 (source at 132 BPM), V19, V20 and O04 lifted and conformed; V09 and V16 (VAPOR95 sources) passed
their receipts; V17's VAPOR95 source passed and its groove-only selection was nothing to cover; S03
nothing to cover. Compared with v6 (declarations, not evidence): V08 lifted here where v6 refused its
pinned attacks — both inside the declared `lawful` family.

Boundary: each stress row is one seed of its shape. The broad search (`../search/`) carries the
mechanism evidence, including the one preserved pre-existing red family at VAPOR95 ≥ 96 BPM, which
no v7 row exercised.
