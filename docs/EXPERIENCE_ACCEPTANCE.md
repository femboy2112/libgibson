# Experience tier: consumer acceptance (E1–E8)

These cases were specified by an external application, ProjectGHOST/MINERVA, which plans to use
`ui::experience` for its six-category "Citadel" strip:
- **Blades** is the primary grammar;
- **Standard** is the fallback at ≤40 columns, in Mono, and with reduced motion.

The tests live in `tests/experience_minerva.rs`. Each guarded behaviour was checked by a
mutation: removing it makes the named test fail.

| Case | Contract | Test |
|---|---|---|
| E1 | Switching between Standard and Blades keeps the selected id, the required action set, and the unavailable reasons. Both receipts pass `check`. | `e1_…` |
| E2 | An unavailable destination is never a live target in any of the six grammars, or in `Fallback`: no items, no actions, and activation yields nothing. Its reason is painted. | `e2_…` |
| E3 | After the collection is reordered, the selection follows the item id (`a-<action_id>`), and Enter activates that same action. | `e3_…` |
| E4 | `Fallback(Blades)` presents Standard at 40 columns, in Mono, and with Reduced or None motion. Status words (AWAITING DECISION, UNDERWAY, VERIFIED) are painted whole. Blades itself is presented at 120×40 truecolour with full motion. | `e4_…` |
| E5 | With reduced motion, the first frame after switching destination is already the settled frame (`FrameDemand::OnChange`). A full-motion control still glides. | `e5_…` |
| E6 | The same scene, size and time paint identical surfaces. | `e6_…` |
| E7 | Camera-rig poses reproduce the consumer's own camera within 1e-6. | `gibson::rig` unit tests (#361) |
| E8 | Keys the experience does not own (Esc, Backspace, `q`, Tab, Delete) come back as `KeyOutcome::Unhandled` and change nothing. Navigation keys are `Consumed`. | `e8_…` |

## The API these cases rely on

- **`Destination::unavailable(reason)`** works in the model:
  - the content becomes the single line `Unavailable: <reason>`;
  - the title gains ` (unavailable)`;
  - the reason is reported in `RequiredSemantics::unavailable`.
  Because it works in the model, it holds however the experience is presented.
- **`handle_key_outcome` / `ExperienceRuntime::handle_key_outcome` → `KeyOutcome::{Action, Consumed,
  Unhandled}`**. `Intent::Back` is reported as `Unhandled`, because the experience keeps no
  history: leaving belongs to the application, which keeps the key loop. The existing
  `handle_key` is unchanged.
- **`PresentationState::select(key)`** selects by the caller's stable id.
- **`Fallback::new(Box<dyn Grammar>)`** presents Standard at or below 40 columns
  (`fallback_at_or_below`), in Mono, or with any motion preference other than `Full`. Keys are
  interpreted by whichever grammar was presented last.

Actions remain labels: a receipt lists the action ids a grammar surfaced, never whether an action
ran or is permitted. Dispatch is the application's.
