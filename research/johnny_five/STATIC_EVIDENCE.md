# Johnny Five — exact-head code evidence and adversarial test seeds

**Audited source head:** \`feat/humanmusic-c137-expressive-transfer@f68c9bfca179f9f248dd5cd601f0c4d3987a1658\`. Hosted GitHub Actions [38492405201](https://github.com/femboy2112/libgibson/actions/runs/38492405201) **SUCCESS**. This is SOURCE inspection, NOT an actual Rust test, fresh render, or hearing the accepted golden7 audio.

Git blob SHA-1s pinned through the current GitHub connector:
- \`src/audio/human_music/kernel.rs\` — \`86967863a6a86eb6d3ddd82d7379e14da5367ca3\`
- \`examples/c137_song/source_song.rs\` — \`6f4858e5eeca72f72afbfa0fcdbd9fbc9a84d04f\`
- \`src/audio/human_music/material.rs\` — \`58ca54c4b6f6b183c00a2aaf9dd14e8cf06ba836\`
- \`src/audio/human_music/interaction.rs\` — \`9568ed2fdad50516b68a4f4ca3f3929ae5cf242c\`
- \`src/audio/human_music/narrative.rs\` — \`a9cdfd68c6fd455c4aaaa189eacdd1a3b55b02bc\`
- \`src/audio/human_music/performance.rs\` — \`383237e2509c2dfdcb33f2f32bac786480e9a7f6\`
- \`src/audio/human_music/policy.rs\` — \`5530cff65e555c7b8716ff582d913002b1991e7d\`
- \`examples/c137_song.rs\` — \`952f4ecdedd06f5fed5817fe91f9b0ccdbe17461\`
- \`examples/c137_song/preset.rs\` — \`932bc7136bd4ebb3c679285ef110892e2248c5db\`
- \`docs/HUMAN_MUSIC_BAND_STORY.md\` — \`e0b2fea9a2e6daa6a4008dcf4b46af573d318f78\`

A JS source-characterization batch was **actually evaluated** over the fetched exact GitHub file text. It contained 30 direct text/pattern conditions. **29/30 returned TRUE.** The one false was an **over-specific textual matcher**, \`"NarrativePolicy::Ensemble"\` in \`policy.rs\`, while that file declares \`pub enum NarrativePolicy { Archived, Ensemble }\` using the **unqualified** variant declaration. Direct GitHub fetch of \`policy.rs:203–229\` confirmed that \`NarrativePolicy::Ensemble\` does exist. Thus 29 literal checks succeeded and one was independently resolved by direct source examination; do not convert this into a made-up Rust test pass. The critical 29 include all J1–J7 observations below.

## Source conditions verified

### Positive controls

1. Earlier \`docs/HUMAN_MUSIC_BAND_STORY.md\` records **human acceptance NARRATIVE over FLAT**, actual handoff/silence and lead-mute survival.
2. \`narrative.rs::NarrativePlan\` exists, \`from_observation\` uses \`MeaningPlan::observe(song)\` rather than the target.
3. \`interaction.rs::InteractionMemory\` exists and stores signatures and history penalties.
4. Current 32-bar song contains one global Score and **eight 16-beat episodes**.
5. Intro is a real source-authored late pickup: local lead onsets 9,12,14,15 over already-active bass/drums.
6. Intro-build now distinguishes a relatively steady skeleton from a textural arrival using separate four-step gain curves.
7. New \`--source-answer\` path emits an ActionKind::Answer with initiating lead-source call; it is explicit experimental audition, not default.
8. Main no-flag C137 still uses accepted 16-bar Full; 32-bar forms are opt-in pending ear acceptance.

### Missing or thin capabilities (structural evidence, not acoustic diagnoses)

9. The C137 kernel's longform domain requires \`p.carriers == [Agent::Lead]\`.
10. The full-song fixture gives every section only \`Agent::Lead\` as a thematic carrier.
11. \`kernel.rs\` never accesses \`NarrativePlan\` / \`NarrativePolicy\`.
12. \`kernel.rs\` never accesses \`InteractionMemory\`, although the older planner contains it.
13. \`kernel::perform_song\` uses \`AdmissionInputs {argument, harmony, ..Default::default()}\`; the older narrative feature is not selected.
14. The 32-bar song generator creates ONE verse, ONE hook and ONE bridge and reuses those sources.
15. In the no-intro form the last three \`Return\` steps reference the same hook and depend on the same bridge; all three are admissible, but distinct performer developments are not thereby guaranteed.
16. The semantic trace for the ordinary 32-bar fixture contains events at only 0 and 124 beats; other section meanings are authored through the argument/relation arc, not fresh trace events.
17. \`BandMemory\` persists one \`open_call: bool\` and a previous-material cell; not an ID-addressable multi-debt ledger.
18. \`prev_answer_events\` appears exactly twice in \`kernel.rs\` (field declaration and assignment) — no read.
19. The keys' formal \`EffectiveHearing\` of the lead covers \`[episode_start, episode_start+4)\`.
20. The keys answer always starts at \`episode_start+4\`.
21. The keys transform the full \`perf.materials[e]\` source for the current episode, which is typically 16 beats long.
22. \`material::transform_material\` chooses the LAST four source events for \`Quote\` and \`Invert\`, often later than what the keys' recorded hearing observed.
23. The \`Call.end_beat\` for that response is set to \`origin.start_beat+origin.length()\`; often after the response at +4.
24. \`Response.latency\` is computed from those timestamps, but \`Response.overlap\` is hardcoded FALSE — potential negative-latency contradiction. Requires a real source fixture/Rust falsifier before claiming a live failing instance.
25. Immediate keys base selection distinguishes three coarse modes (\`Silence\`, \`Invert\`, \`Quote\`) using closed/rising first-bar summary and stage.
26. Keys comp uses fixed \`bar_start+2.5\` and \`bar_start+3.5\` slots (with common pocket).
27. Bass uses small prewritten root/fifth/third/next-root duty templates (the accepted V→I anticipation is genuinely destination-aware).
28. Per-episode dynamics still use a fixed modulo-four \`groove_swell\` curve.
29. The C137 full song does not yet have a true repeated-return adaptation policy that chooses from an independent inventory of legal performer acts based on its *own* prior realized playing.
30. No direct caller in the kernel invokes the old general \`plan_interactions\` / \`InteractionMemory\` selection path.

The direct observation that FULL 32-bar choruses share identical source is NOT a prediction of byte-exact full-mix audio; their role responses and drum production may differ. Measure those windows in real Score+PCM before claiming repeated acoustic fingerprints.

## Exact red-first Rust experiments to write next

**H1 — future knowledge vs performed hearing.** Produce two legal instances where the lead's performed first four beats are identical and its later actual notes differ, with the allowed shared authored-knowledge inputs controlled separately. The keys' [4,8) response can only change if it **knew** the later source through the declared score or legitimately heard a different prefix. Label both regimes. Check Call/Response latency, overlap, material/ID and EffectiveHearing. Do not promote the wrong observer into the right one merely by changing an enum label.

**H2 — handoff vs dead lead.** Use the existing ear-proved NARRATIVE/STATIC/SCRAMBLED/FLAT matrix on one modern generated 16-bar C137 source with matched note counts/loudness where practical. Lead-mute a designated withhold scene: the same motif must remain in keys/bass. Prove full carrier ownership, not just one keys ornamental fragment; keep bass monophonic floor.

**H3 — choice vs branch formula.** Two lawful source histories with identical chord, bar position, first/last note summary and onset counts but different named, heard prior gestures must justify distinct next acts. An opt-in new role-choice policy should choose differently on declared source reasons; the old formula is the negative control. Unrelated metadata must not cause audible changes.

**H4 — multi-debt source integrity.** Pending A and B, one paid, the other intentionally open or later paid by its own ID; never overwrite with \`carried_cell\`. Preserving source identity while still selecting a coherent next phrase is the gate.

**H5 — repeated-return development.** Freeze one recognizable hook across 32 bars; three post-bridge choruses may repeat the melody, but a source-earned new handoff/answer/development must occur across them, and when an earlier interaction is ablated that later change is correspondingly absent/altered. No mandatory cosmetic differences.

**H6 — pulse and physical readiness.** Use actual Score/PCM, role stems, source/plan fingerprints and production controls. Check snare single-voice retriggers, keys-voice steals, bass mono occupancy and role phase; preserve accepted lead/bass/keys/drum sound until listening proves a change helps.

**H7 — source consistency.** The earlier \`NarrativePlan\` cannot be copied into C137 if it violates \`ArgumentPlacement\` source/role contracts. Define the necessary licensed handoff at the source level and independently verify it. Blind acceptance remains PENDING.

## A mathematical warning about "Johnny Five"

The formal observer-quotient lemma is **trivial but consequential**:

\`\`\`
a_i = f_i(O_i(H_i))
O_i(H) = O_i(H')  =>  a_i(H) = a_i(H')
\`\`\`

It tells us why a model can never make a semantically responsive choice when its inputs erase exactly the difference that would justify the response. It does **not** prove how to score musical meaning, nor that arbitrary random variations are expressive. Strong tests must prescribe **which source distinction matters musically** and use a human listening arm. That is where this program could make progress.

**Preservation:** no C137 source edits on this research branch, no change to main, no unapproved default promotion, no fabricated audio auditions. Implementation belongs on a new non-main branch based on the REFRESHED accepted C137 head.
