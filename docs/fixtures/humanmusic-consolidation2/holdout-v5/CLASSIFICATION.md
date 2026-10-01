# Holdout v5 — first contact

**One contact, no fitting.** Executed once, release profile (`debug_assertions=false`), on a clean
tree at the declaration commit `7110cee2ea2366a52a626f2240955071dd79104f` (library source frozen at
`3146438`, last changed at `938ab3e`; config SHA256
`c52d04217bcc9193452daf4fc4732463fd5d5ef852c8f1fd5ef8262027c1301f`). Raw receipts are under
`results/` exactly as written by the harness; nothing here was tuned to them.

## Result (Observed)

**28 / 28 cases pass; 503 checks, 0 failing.**

- **Generated sources:** all 21 BAND sources pass their own `PerformanceReceipt` (what
  `perform_checked` admits), including the round's stress shapes — rootless-support identity (V03, V04,
  V19), the truncated relational function (V05) and the one-bar bass-figure room (V06, V07).
- **Predicted lifts:** 6/6 admitted (V01, V11, V12, V13, V15, O01).
- **Admitted covers:** every one passes `PerformanceReceipt + CoverConformance` under its target
  profile, with every pinned axis receipted; none replays its source.
- **Lawful refusals (4), each in the declared family at the lift stage:**

  | Case | Refusal |
  | --- | --- |
  | V08 | `Invalid("no lawful harmony contains the pinned simultaneous attacks")` |
  | V16 | `Invalid("pinned harmony outside target vocabulary")` |
  | V21 | `Invalid("pinned harmony outside target vocabulary")` |
  | O02 | `Invalid("pinned harmony outside target vocabulary")` (Ode derived harmony → SWISS) |

- **Nothing to cover (2):** V17 (groove-only: spec `[Groove]` carries no song identity) and S03
  (Swing Loose: no observed axis survives).
- **O03** (Ode Strict → BLACK_ICE fusion) lifted and conformed — `lawful`, as declared; it is not a
  refusal at this source.
- **Cross-swing Groove quotient:** PASS on all 12 admitted Groove-pinning covers; the canonical metric
  quotient matched everywhere, including V03 (1 stroke) and V18 (3 strokes) carried to another
  performed float by the target's swing transport.

**No internally invalid BAND performance and no undeclared internal red appeared.**

## Boundary

One contact on 28 rows. Each stress row is one seed of its shape; the exploratory searches (340,416
admitted / 0 rejected on fresh seeds at `938ab3e`) carry the mechanism evidence. A holdout pass is
machine evidence about declared laws, not a listening result.
