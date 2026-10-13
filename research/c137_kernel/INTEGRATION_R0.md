# C137 long-form integration — R0 checkpoint

Branch `feat/humanmusic-c137-longform`, base `cfd5aea3243da2156681590e98c482c61a67018e`.
This is the single integration of Codex's proven causal kernel (PR #104) plus the
start of the long-form composer (plan: [HUMANMUSIC_C137_LONGFORM_INTEGRATION.md](../../docs/research/HUMANMUSIC_C137_LONGFORM_INTEGRATION.md)).

## Ancestry — integrated ONCE, no duplicated commits

Verified against git, not the plan text:

- PR #102 `feat/humanmusic-c137-ensemble-agency@29527a8` (keys weave on, **bass weave held**) **is a
  direct ancestor** of PR #104 `feat/humanmusic-c137-causal-kernel@cfd5aea`
  (`git merge-base --is-ancestor 29527a8 cfd5aea` → true).
- So PR #104 is a clean linear stack of 13 additive commits on top of my ensemble-agency work:
  new `kernel.rs` (1021 lines), surgical edits to `argument.rs`/`performance.rs`/`functor.rs`/
  `material.rs`, 5 `tests/audio_c137_*` files, `examples/c137_conversation{.rs,/source.rs}`, and the
  `research/c137_kernel/` evidence dir.
- Integration is therefore a **branch-off of `cfd5aea`**, not a merge-reconcile. No divergence, no
  cherry-pick of earlier PRs. The only thing added from PR #105 is its one docs-only plan commit.
- `BASS_WEAVE` stays `false`. The frozen `perform*` paths and the 4-bar `examples/c137_conversation`
  baseline are kept as permanent regression/listening controls.

## Reproduced locally on this branch (not taken on faith)

- Exact-head CI for `cfd5aea` = run **38202899251**, conclusion **success** (verified via `gh`).
- Focused kernel suite: `audio_c137_conversation` (9), `audio_c137_kernel_hostile` (19),
  `audio_c137_kernel_baseline` (4, 2 ignored red-oracles), `audio_c137_source_authority` (3),
  `audio_c137_bass_capacity` (5, 1 ignored) → **40 passed, 0 failed**, 3 intentionally-ignored oracles.
- The **causal law** holds: `muted_cue_changes_only_its_later_drum_response` (silence the keys
  arrangement, re-perform → the `heard-keys` drum fill disappears while pulse/lead/bass stay
  byte-identical) and `consumed_velocity_changes_a_real_downstream_fill` both green.
- One real render: `c137_conversation --seeds=2112 --tempo=144` → Score fingerprint
  `468ddeeb851c6101`, matching the HANDOFF table. The example runs end-to-end here.

## Why the samples are sparse and seeds collapse — diagnosis (informs R1)

Measured by a read-only probe of the generator (a Python port of `Rng`/`mix`/`snap_aeolian`,
cross-checked against the shipped `acoustic_candidate.md` MIDI). The sparseness and
seed-collapse are **a narrow source plus coarse observation**, not a kernel-wiring bug:

1. The seed reaches the band through one ~1.6-bit channel: `pair.verse[..4]`
   (`examples/c137_conversation/source.rs:61`). `snap_aeolian(4) == 3` merges two of four cells, so
   seeds 2112 and 7 produce the identical cell `[0,3,5,7]`; seed 99 lands on `[3,5,7,8]`.
2. The route is a hardcoded constant (`source.rs:125`, `compiled.route = verse.to_vec()`), so bass
   (route-only) and drums (keys-only) are **exactly seed-invariant**. Pad is always silent.
3. Keys always pick `Invert` (the cell always rises and never ends on the tonic), and
   `transform_material` rebases to intervals so the absolute-pitch difference is snapped away.
4. **`pair.hook` is generated and then discarded** (`source.rs:60`) — dead computation. The hook
   (an `AlteredConsequent`, proven by `Kinship`) and the hook's own progression
   `[bVI, bVII, iv, i]` are exactly the distinct-but-related material the long form needs.

The repair is **source material and source-conditioned decisions** — use the whole generated phrase
and the hook, let the route travel — *not* random texture to force unique hashes.

## Plan from here (smallest honest steps, each with a listening gate)

- **R1** — feed the kernel a genuine generated phrase (not a 4-event prefix), and one distinct-but-
  related answer whose rhythm/pitch depend on the source, keeping `Silence` valid.
- **R2 / Stage A–D** — compose causally-connected episodes into ONE continuous Score (never
  concatenated WAVs): `(Context, Memory, Obligations) → (CommittedMusic, Context', Memory',
  Obligations')`. 8 bars → 16 → 32 → 48–56, each a real render, the second episode audibly
  remembering the first. Parameterize the kernel's hardcoded windows; do not just delete the
  length guard.

Human acceptance against `teacher_golden7` remains **PENDING**. Metrics only reject; the ear decides.
