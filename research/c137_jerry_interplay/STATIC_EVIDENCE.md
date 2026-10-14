# C137 Jerry/Interplay audit — exact-source receipts

Audited \`feat/humanmusic-c137-expressive-transfer@125d759f10842b759ef6c1279de63b425a64e1fe\` (PR #111). GitHub-hosted CI at this exact SHA completed SUCCESS, [run 38368023355](https://github.com/femboy2112/libgibson/actions/runs/38368023355). Refresh head if Claude pushes later.

**Procedure actually executed here:** fetched TEN full files at this ref using GitHub connector; verified their Git blob SHAs; applied TWENTY-SIX targeted JavaScript static source predicates; **26/26 matched**. That means identified **facts and risks are present in source**, not that all are virtues. No Rust toolchain/audio audition or real seed's drum event enumeration was executed by this research pass.

Git blob pins:
\`\`\`text
examples/c137_song.rs                 cc328926350d2bdbbe5c5f5dc06dda9029e6d7e9
examples/c137_song/preset.rs          ea53c7274d6aebbbc0380579effca74adc5d99b4
examples/c137_song/source_bridge.rs   7ca7fb4ea109bf6ca067fcb7f9457750e0ccb091
src/audio/human_music/kernel.rs       5c367f9986084fd94839af860d59e6c7bb6a2189
src/audio/human_music/argument.rs     78b694e7f5576553cef40aa9d1f9975bdf32950f
src/audio/human_music/theme_family.rs 80ed392bbf9e5994be22877acb1b676c15aabdfb
src/audio/human_music/synth.rs        9337d4667f65b99283c94146d3aa64eac3d7f2dc
src/audio/dsp/drums.rs                076264b398b13c800e8b96c8e8e3c3fdbef18703
tests/audio_c137_song.rs              bc66c57fb7501a1ce500c940479b5b428068d4bb
src/audio/human_music/score.rs        1cf29e7cfe2195670d975193eccb5c0a098897b9
\`\`\`

## Source predicates (ALL TRUE)

| ID | Matched direct code/source relationship | Nature |
|---|---|---|
| J01 | CLI no-flags selects C137Preset::full, 16 bars, Phrasing::Ensemble | positive |
| J02 | Preset implementation is under examples/ rather than C137 library entry | API gap |
| J03 | Jerry::Plain sets Phrasing::Plain and automatically disables comp_keys, anticipate_bass, translate_lead_rhythm | confounded A/B |
| J04 | CompiledArgument::compile defaults all four CP3/ensemble flags false | archived default, C137 constructor gap |
| J05 | manifest has no independently witnessed rows for keys comp, bass anticipation, drum translation | observability gap |
| J06 | Phrasing enum's generic default remains Plain | historical compatibility, avoid changing |
| D01 | Breath licensed by non-home harmonic drive | meaningful but coarse cue |
| D02 | New drum answer constructed through generic push role-note tag | incomplete response provenance |
| D03 | lead-echo fill computes time = lead onset + 2.0, targets Snare | source-time dependency |
| D04 | pulse backbeat includes bar+3.0+pocket(3) Snare | kit fact |
| D05 | regular and bridge cadence grids include beat +1.0 | call fact |
| D06 | pocket_offset(+1)=pocket_offset(+3)=+0.030 beats | pocket fact |
| D07 | synth owns one \`snare: Snare\` and dispatches every scheduled Snare via trigger() | render capacity |
| D08 | Snare::trigger re-triggers envelope and resets oscillator phase | audible consequence mechanism |
| D09 | Score::validate verifies drum range/time/velocity, not per-voice same-sample collisions | observer gap |
| D10 | echo test verifies call offset, not conflict/voice capacity | test gap |
| K01 | Keys comp rootless voicings not admitted through synth actual voice-pool physics | source/physics gap |
| K02 | synth has six keys voices | finite capacity |
| S01 | source fixtures set new CP3 flags as a function of lead_is_expressive | confounding |
| S02 | groove_swell still has modulo per-episode amplitude reset | macroform risk |
| S03 | lead cross-section continuation edge still unconditional | articulation risk |
| S04 | keys-derived drum Fill creates ActionKind::Fill with call=None and phrase=0 | incomplete provenance |
| S05 | validator checks pitched-note interaction references, not drum reaction references | missing witness |
| H01 | example source hash may show an explicit “not embedded” fallback | custody risk |
| H02 | default example renders full mix only (no solo stems) | audition gap |
| H03 | default example does not itself compute file SHA256 | custody gap |

## Independently calculated deterministic drum scheduling counterexample

The cadence grid guarantees a source attack at beat 1.0 of its last bar. The performed lead attacks on the global bar-relative beat 1.030 because the current pocket offset is +0.030. The new \`lead-echo\` rule translates this to a snare at beat 3.030 of that bar. The existing backbeat snare is ALSO at beat 3.030:

\`\`\`text
lead local attack      = 1.000 + 0.030 = 1.030 beat
echo snare time        = 1.030 + 2.000 = 3.030 beat
regular backbeat snare = 3.000 + 0.030 = 3.030 beat
\`\`\`

At 48kHz and 144 BPM: \`samples_per_beat = 20000\`. For the cadence bar starting at global beat 12:

\`\`\`text
lead source attack global beat: 13.030
echo and pulse snare global beat: 15.030
sample index of both: 300600
\`\`\`

**This arithmetic was executed in JavaScript and returned \`same_sample: true\`**. It demonstrates a schedulable collision for the source lattice and playback pipeline; it does NOT constitute an executed Rust/WAV rendering of seed 2112. Next step: run an actual source-indexed sample-space DrumHit count on each current Full seed; confirm triggers and quantized PCM. Do not rely on a floating timestamp-only comparison: group after the real \`Tempo::beat_to_sample\` conversion.

Given one \`Snare\` instance, two triggers on the same sample *restart the same synth voice*, not two independent layered snare sounds. Arbitrary near-coincident hits within the active snare envelope should be independently diagnosed too, though musical ghost/retrigger patterns may be intentional. Fix at author/commit-time rather than mutating final events after other musicians heard them.

## Remaining evidence boundary

The shipped branch is not proven bad by this audit. The accepted expressive lead, piano and bass are valuable. The drum fill's user A/B is PENDING; avoid declaring it better or worse. No golden7 WAV collection was available to this tool. All 26 source checks and the deterministic collision equation are a **prioritized roadmap for actual Rust/PCM verification**, not a claim to have completed it.

See [HUMANMUSIC_C137_JERRY_INTERPLAY_AUDIT.md](../../docs/research/HUMANMUSIC_C137_JERRY_INTERPLAY_AUDIT.md) for R0–R6, independent control design and preservation rules.
