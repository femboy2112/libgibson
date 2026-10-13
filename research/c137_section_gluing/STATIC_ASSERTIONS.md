# C137 section-gluing — exact-head source audit receipts

**Audit base:** PR #106 head 4cbd60eed58ea5a8ddea08fe6cbd921528ad7d48.
**Method:** GitHub connector returned complete source text at this exact head. JavaScript source-level assertions over that text were evaluated in this chat. These are STATIC CHARACTERIZATIONS, NOT actual Rust execution, not generated WAVs, not golden7 listening. All 16 assertions evaluated TRUE. That does not mean 16 good properties; many characterize a current failure or scalability limit.

Pinned Git blobs (source sha1):
- src/audio/human_music/kernel.rs: 58fb5275b9891194f4929a5317848ffb97d9336c
- examples/c137_song/source.rs: 2d45e98dfc654d342493f78a3e4fa5b467cf2904
- src/audio/human_music/score.rs: 1cf29e7cfe2195670d975193eccb5c0a098897b9
- src/audio/human_music/argument.rs: 49c0ba50370d74a1d6cbbdc4f1b23aebb5d4d5a6

## 16 evaluated exact-head source assertions

| ID | Grounded condition checked | Result | Interpretation |
|---|---|---|---|
| 01 | longform_domain rejects totals != two 16-beat episodes and placement count != 2 | TRUE | Bounded 8-bar demonstration, not N-episode compiler |
| 02 | BandMemory.prev_answer_events appears only in declaration and assignment | TRUE | Dead state variable; not functioning memory |
| 03 | If open_call, transform_material uses carried_cell from previous episode | TRUE | Real source-based prior-material response exists (positive) |
| 04 | The ActionKind::Answer uses perf.statements[e].call (CURRENT episode) even in that branch | TRUE | Actual carried reply's source/action attribution mismatch when first call was unpaid |
| 05 | source_plan initializes Interactions to empty | TRUE | Existing planner constructs no Interaction rows |
| 06 | perform_song never inserts/extends perf.interactions | TRUE | Response ledger still empty after score generation |
| 07 | Generated keys notes stamp InteractionId(e) | TRUE | References can point to absent Interaction records |
| 08 | validate_song does not check those Interaction foreign keys | TRUE | Current independent Score source witness has this blind spot |
| 09 | kernel::provenance defaults SectionKind::A and discards perf/beat | TRUE | Generated keys/bass/drum section metadata is not hook-aware |
| 10 | Kernel never assigns Score.sections | TRUE | Score section metadata empty on this path |
| 11 | groove_swell resets based on bar % bars_per_episode | TRUE | Same macro dynamic ramp reinitializes at every episode boundary |
| 12 | harmonic_drive checks chord root == tonic BEFORE Dom7 quality | TRUE | Future tonic-root secondary dominant treated as Home |
| 13 | perform_song unconditionally constructs cross-episode lead continuation edge | TRUE | Legitimate detached/rest/legato articulation not yet source-selectable |
| 14 | Current 8-bar source uses only verse + hook referents (no generated bridge) | TRUE | Missing full phase-change example |
| 15 | Current verse chart's last bar is tonic minor after V7 | TRUE | Harmonic arrival occurs inside verse even if melody stays open; evaluate musical expectation, not comments |
| 16 | perform_song builds one 32-beat Score over one global timeline | TRUE | A genuinely important positive achievement: NOT stitched WAV fragments |

**Exact assertions were executed on connector-supplied source text, not on mutable README prose or inferred filenames.** This is enough to establish concrete source ownership and ordering facts. It is NOT enough to establish that a new implementation can pass cargo, satisfy source topology, or improve listening.

## The highest-priority Rust RED tests before refactor

- Use existing first-episode-keys-muted control: the returned second-episode keys response quotes earlier MaterialId/ActionId. Require its ActionCause::Interaction to point to that EARLIER call and require every note's provenance.interaction to index an actual Interaction with that call/response/material. Predicted current failure, not an executed Rust result.
- Give two distinct unanswered calls before a later opportunity. Demand that neither source obligation disappears or gets retargeted. Current bool+single carried_cell cannot pass as a general source ledger.
- Check new key/bass/kit events against actual SectionKind and Score.sections on the hook side of beat 16; compare with source-side argument/arc. Predicted metadata failure.
- Compare Am tonic versus A7 with the SAME tonic root in harmonic_drive; demand different function-aware behavior when chart purpose/destination differs. Current rule classifies both Home.
- Separate lead-source with explicit detached, legato and silence seam. Unconditional new VoiceContinuation is not sufficient.
- Compare neutral administrative split of an otherwise identical source event timeline against unsplit performance. An identity repartition must preserve Score; a declared musical transition is allowed to change it.
- Change only the future source after a transition; effects may reach a LICENSED advance-preparation window, but not unrelated earlier notes. One global synth Score must preserve the pulse and correctly linked role voice state.

## Evidence boundaries

Verified through source and green hosted CI: current 8-bar code exists and passes its current build/tests. Verified through STATIC CODE assertion: the limitations above are present in that source. **Not verified here:** that every missing metadata reference survives an actual runtime path (write the red tests), the spectral/psychoacoustic consequences, the 16/32/56-bar adaptation, or a golden7 sonic verdict. Never represent a new green source assertion as musical acceptance.

Implementation roadmap and categorical law: [HUMANMUSIC_C137_SECTION_GLUING.md](../../docs/research/HUMANMUSIC_C137_SECTION_GLUING.md).
