# C137 semantic exchange — exact-head source audit receipts

**Inspected GitHub source:** feat/humanmusic-c137-phase-change at commit f170f9e07c4a52b9fe7a9beb0df538b82a211296 (draft PR #108). Exact-head GitHub Actions workflow 38338109472 succeeded. This research branch is stacked on that exact SHA. No new Rust tests, WAV render or golden7 listening were performed in this audit.

A JavaScript probe ran over six full, connector-returned exact-head source files. It evaluated **24/24 source-text structural assertions TRUE**. TRUE includes statements identifying LIMITATIONS; it is not a 24/24 musical performance score. The deeper interpretation is in [HUMANMUSIC_C137_SEMANTIC_EXCHANGE.md](../../docs/research/HUMANMUSIC_C137_SEMANTIC_EXCHANGE.md).

## Pinned Git blob SHAs

- src/audio/human_music/kernel.rs — 40bfd01720c9d3dbfc0f39d469d19a69412ee9bb
- examples/c137_song/source_bridge.rs — 0cd159e4a924dc6af42560b39c1629ab46ec428a
- src/audio/human_music/argument.rs — 49c0ba50370d74a1d6cbbdc4f1b23aebb5d4d5a6
- src/audio/human_music/material.rs — 58ca54c4b6f6b183c00a2aaf9dd14e8cf06ba836
- src/audio/human_music/theme_family.rs — edf1307cf44056f2273883d5712ab41b7bc1abd8
- tests/audio_c137_song.rs — cf5369aa599c26c7920ce9b52116356f7f47fced

## Structural assertions, all evaluated TRUE

| ID | Source fact | Implication |
|---|---|---|
| 1 | source_bridge declares a four-section, 64-beat source with exactly Verse/Hook/Bridge referents | Real 16-bar zoom-out |
| 2 | Return uses the SAME hook ReferentId(1) and default (identity) transform | Final lead headline repeats original hook |
| 3 | Source progression appends the SAME HOOK_PROG in first Hook and Return | Same chart on both choruses |
| 4 | Return depends_on also lists the bridge step | Logical provenance of journey is recorded (positive) |
| 5 | BandMemory remains open_call Boolean plus prev_answer_events assigned but unread | Very narrow long-horizon musical memory |
| 6 | Keys response is chosen from tonic-closed, rising contour and stage status, or one carried prior-cell override | Coarse responder policy |
| 7 | Keys answer window fixed to the second bar of each 16-beat episode | Same exchange shape every section |
| 8 | Material transform Quote/Invert extracts at most FOUR source-tail events | Source information bottleneck |
| 9 | Keys' melodic response is projected to chord tones | Another possible distinguishing-feature bottleneck |
| 10 | Four-snare generic fill scheduled by modulo at every 16-beat episode seam | Automatic transition virtuosity |
| 11 | Drum's heard-keys response echoes the LAST TWO keys attacks | Thin but real role response |
| 12 | Hat subdivision switches on whether harmonic state is Home | Functional groove mechanism (positive) |
| 13 | Bass non-Home figure chooses a beat-four chromatic approach from next root | Competent but largely chart-driven floor |
| 14 | Pad, when whole-song stage admits it, revoices EVERY bar's full chord pitch classes | Sustained blanket layer, not source-dialogue controlled |
| 15 | Bass, drums and pad all multiply by harmonic-drive and modulo-episode swell | Shared-energy correlation remains |
| 16 | groove_swell is a deterministic 4-bar modulo reset | Identical dynamic ramp per episode |
| 17 | Drum pulse includes an automatic 4-hit fill at episode boundary | Possible show-off when no gesture demands it |
| 18 | Required lead source is realized through argument_transport | Source legal identity preserved (positive) |
| 19 | Exact required source/harmony validated BEFORE realization | Final source authority preserved (positive) |
| 20 | PR #107 repair credits carried Answer to the TRUE originating call | Genuine causal provenance fix (positive) |
| 21 | Interaction rows now registered for performed keys answers | No dangling keys InteractionId in admitted route (positive) |
| 22 | Current tests reject an unrelated bridge or wrong return referent | Useful necessary source rejection, not audible earned-return proof |
| 23 | Generated bridge intentionally thinner than verse/hook | Texture contrast is planned (positive) |
| 24 | The existing 8-bar control stays separately selectable and long-form Score carries section metadata | Regretted prior control/metadata issue repaired (positive) |

## Precise deduction, bounded by source

Given the fixed song built by source_bridge:

- Step 1 / first hook and Step 3 / Return reference EXACT SAME hook material.
- Both transformations are identity.
- Both receive identical four-bar HOOK_PROG charts.
- Therefore at the composition/source layer, the returning **required thematic event sequence** is the first hook's sequence under a shift of 32 beats, except separately declared relation-dependent velocities/accents and registration when realized. The bridge motif's specific musical content is NOT an input to the compilation of this required Return material.

This is a rigorous statement about source/placement construction, NOT a theorem about identical final PCM: keys' history, section gain, pad/drum/bass, synthesis, and other performance parameters can still make the heard return differ. Whether the bridge changes the return's actual audible consequences is a *test question*.

The named strong next falsifier should hold the hook and return SOURCE unchanged, mutate only the lawful bridge material within fixed chart and look for a **specific, witnessed, audible changed return interpretation**. The current tests cover wrong/unrelated motifs, NOT this pair of lawful alternatives. Distinguish failure of such a requirement (for the musical target) from failure of the basic preexisting source validity laws.

Likewise, pad/kit/bass are not proven inherently bad because they read harmony rather than literal motifs: preserving a groove/foundation often DEMANDS this. The problem is calling all of their generic activity "semantic dialogue" without actual source-contrast evidence and without separately comparing full mix versus role stems.

## Expected next test commands (NOT executed in this research)

In a real checkout at the latest head:

    cargo test --test audio_c137_song -- --nocapture
    cargo run --release --example c137_song -- --out=/tmp/c137-source-control --seeds=2112,7,99 --tempo=144 --bars=8
    cargo run --release --example c137_song -- --out=/tmp/c137-full-arc-control --seeds=2112,7,99 --tempo=144 --bars=16

Use different EMPTY output paths for all arms; record actual revision, world config, source/Score fingerprints, Score TSVs, individual buses, PCM SHA-256, and acoustic level/timbre consistency. Use source-level RE-PERFORMANCE for bridge/keys/cue mutations; after-the-fact muted exports must be labeled UNCOMMITTED controls.

**Suggested new Rust RED test names:** lawful_bridge_variants_have_witnessed_return_consequences; source_different_equal_density_keys_response_is_distinguishable; generic_modulo_fill_is_not_an_earned_transition; full_pad_and_selective_pad_share_same_source_but_different_masking; no_extra_bass_voice_but_motif_destination_changes_bass_duty; return_preserves_full_hook_identity_with_real_bridge_memory; a_formal_only_dependency_does_not_create_audible_semantics.

The acceptance gates remain: source correctness, meaningful role correspondence, render fidelity, and USER listening are DIFFERENT. No current source diagnostics certify that the band actually communicates ideas like the seven accepted handwritten renders.
