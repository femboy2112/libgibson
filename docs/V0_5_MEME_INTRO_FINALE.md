# v0.5 Finale — The Upgraded Meme Intro (Filthy Frank Reaction Cut)

> **Owner directive (2026-10-09): v0.5.0 ends with an EPIC NEW LIBGIBSON MEME INTRO, not merely a set of green feature branches.** It must genuinely demonstrate the new v0.5 features as a directed, coherent short film, with the **Filthy Frank keyed greenscreen reaction subject composited on top of the cinematic content as in the previous meme intro**. This is the named **v0.5 final flagship and maintainer visual+listening acceptance gate**.
>
> **Status: REQUIRED RELEASE-ENDING DELIVERABLE / PLAN ONLY. NOT IMPLEMENTED OR APPROVED HERE.** This is an editorial/art-direction and engineering acceptance skeleton, not a frozen edit decision or a release announcement. The maintainer can add new v0.5 features and revise the shot list as work lands. No feature is depicted as working until it is actually present on the integration head.
>
> Plan branch: \`docs/v0.5-observable-instruments\`. Source baselines sampled 2026-10-09: \`main@80706c91...\` (released v0.4.0); draft PRs #81 (plot), #83 (spatial UI), #84 (auditory observer scaffold—partly superseded by work on #87), #86 (cinematic film/authoring kit, layered on spatial branch), #87 (Rick-C137 HumanMusic). **Heads and PR topology must be refreshed at actual integration.**

## 1. The movie's non-negotiable premise

**This is not a demo reel of screenshots.** It is a fresh, upgraded version of the existing LibGibson introduction/reaction film: the engine visibly transforms the *same information* across increasingly wild but truthful representations, with intelligible cause/effect, new cinematic/spatial/musical capabilities used for real, and the keyed Filthy Frank reaction integrated into the picture as its comedic foil.

Two things must be simultaneously true:

1. An audience unfamiliar with the library watches a brilliant, funny, visually legible "what the hell is this?" terminal film that escalates and then lands a final payoff.
2. A skeptical engineer can point at each shot and name the **actual v0.5 source state, public primitive and observable transformation** that generated it, not a prerendered fake or a conveniently hard-coded title.

An ambitious visual language is welcome; a fake claim is not. **Maximum absurdism, minimum epistemic cheating.** The joke lands because it is genuinely happening.

Preserve and **surpass** the earlier examples:
- \`examples/libgibson_intro.rs\` and its director/source-to-frame contract;
- \`examples/libgibson_intro_reaction.rs\` plus \`examples/libgibson_intro_reaction/{director,transform,compositor}.rs\`;
- \`examples/reaction_soundtrack.rs\`, the audio-aware reaction edit with real local dialogue clips and procedural underscore, when legally available;
- \`docs/FRANK_REACTION_CUT_PLAN.md\` (source windows, edit-comedy grammar, continuous source-film spine, local-media/caching boundaries).

The existing Frank reaction cut already belongs to v0.3.1/v0.4 heritage. The **v0.5 finale re-directs and upgrades it to react to the NEW film/features**; merely re-running the old v0.3.1 clip does **not** satisfy the gate.

## 2. Source and time authority: one edit, independent projections

The latest cinematic branch introduces a general reusable \`gibson::timeline::Timeline<T>\` and a **single authoritative edit** for the cinematic show (\`examples/cinematic_showcase/reel.rs\`). Do not replace it with independent clocks for camera, shot identity, Frank, sound and temporal effects.

~~~text
                                   ONE author-owned edit clock t
                                            |
                              transparent cue identity/provenance
                           ________________|________________
                          /                |                \
                  visual director    semantic music M     reaction director
                f(t) -> Surface      Timeline -> Trace     t -> keyed clip frame
                    |                     |                  |
        real plot/spatial/scene        C137 generated        transform + matte
        graphic act and HUD           Score -> PCM          source audio windows
                    |                     |                  |
                    +----- guarded surface-local overlay ----+
                                            |
                                   ONE LibGibson Surface
                                   ONE renderer, diff + PTY
                                            |
                              reproducible seek/capture stream

   A/V time alignment derives from the same edit clock; source media time is a
   declared map from edit to local clip, not a competing master.
~~~

- Preserve musical ownership: the film chooses semantic *dramatic intent* through a show-side mapping \`M : Timeline<Shot> -> SemanticTrace\`; the HumanMusic planner owns its notes/harmony/ensemble. Don't hardcode named notes or special "Frank" branches into generic \`gibson::audio\` or \`gibson::timeline\`.
- The video reaction has its own *source-media* time mapping, subordinate to the film's edit clock. It may punch in, squash, freeze, smash cut, hold, and use deadpan pauses, **only where it improves the joke**; not randomly everywhere.
- Retain the **continuous underlying film/story spine** when the reaction appears. The old \`Continue/Hold/Slow\` per-cue time policy caused jerky underlying cinema and was superseded; don't accidentally resurrect it. Any deliberate full-film hold must be explicit and not violate source identity.
- The old keyed-video compositor preserves the base \`Surface\` **byte-exact outside the union of previous/current reaction bounding boxes**. Keep that strong invariant with edge/resize/pan/cut controls; never reproject the entire terminal-native frame and degrade crisp text merely to show Frank.
- Temporal subcell/color modulation is optional and gated by actual terminal presentation conditions. A flickering face or unreadable video is not proof of "higher resolution"; static half-block/verified Braille fallback is always valid.
- Use one clean asset-and-media pipeline; no per-frame ffmpeg, repeated full 302s decode, unbounded PCM cache or side-effectful re-generation of the entire song at each visual tick.

## 3. The evolving act map (all claims must be truthful)

This is **art direction, not a sealed timeline**. The chosen acts should feel like one escalating coherent narrative rather than seven disjoint tech advertising panels.

| Beat / truth the audience learns | Source-backed v0.5 instrument | Comic/cinematic move |
|---|---|---|
| **Cold-open: "You thought it was just text."** | The original intro's information/harness → temporal/scene/renderer identity | Unreasonably confident demonstration; Frank reacts with guarded disdain. |
| **Geometry becomes measurable.** | \`gibson::plot\`: real sample/transform, axis, annotations and preserved data under pan/zoom | The camera "discovers" an observable that the film itself is producing; Frank disbelieves a valid diagnostic, not a fabricated chart. |
| **One object becomes six interfaces.** | \`ExperienceRuntime\` / spatial UI grammars and Album Flow/textured planes | Preserve one concrete item/selection identity as the scene reinterprets it spatially. Frank suddenly gets much bigger or is shoved aside by the interface. |
| **Space, film and time cohere.** | v0.5 perspective-correct raster3d textured quad + cinematic authoring timeline/camera path | The same source object turns through a spatial basis change, camera crosses the cut without two edit clocks. |
| **The band actually tells the story.** | HumanMusic C137 accepted generated \`SemanticTrace -> Score\`; harmonic/tension-withhold-carrier semantics, not a hand-staged fake | Stage a clearly listenable call/response, a deliberate lead withholding with bass/keys carrying the germ; cut Frank at the silence or payoff. Show only claims backed by source-aware/per-carrier witnesses. |
| **Audio is observed, not self-certified.** | \`gibson::audio::perception\` + \`gibson::plot\`, where merged/tested | Show an actual measured waveform, auditory excitation/onset witness or honest audible/visual relationship; no "sounds human" quality score and no fake absolute SPL without calibration. |
| **Impossible but real final reveal.** | The integrated timeline + scene + graphics + music + keyed reaction -> one renderer | Every prior invariant returns, then an unmistakably comedic "IT'S TIME TO STOP"-style reaction/sting and LibGibson final title/payoff. Don't depend on a copyrighted catchphrase clip being distributed. |

**Scope law:** a feature listed here but not actually accepted/merged into the integration head must not be shown as a finished engine capability. If the maintainer adds a new v0.5 feature, it earns a cameo or replaces a weaker act **only if it strengthens the film's argument and remains observable**. MeatsackProjection is currently a boundary/research program, and GSPU-FM / Doom / cross-language parity are future work, not features that can be falsely claimed as shipped in v0.5.

## 4. The reaction: Filthy Frank must be IN the new film

The user specifically wants the **keyed Filthy Frank subject reacting over the visuals like the old meme intro demo**, not a reaction video in a separate window and not a generic cartoon replacement.

- Reuse the local-source discovery / \`--clip=PATH\` contract and existing cue windows from \`docs/FRANK_REACTION_CUT_PLAN.md\`. The original reference WebM may reside in the maintainer's \`~/Downloads\`; **do not assume it is present in an automated environment**. Reinspect actual media, find the best in/out cuts, and choose genuinely funny reaction-beat alignment rather than blindly preserving old cue timestamps.
- Use **chroma key / premultiplied soft alpha / aspect-correct inverse sampling** and bounded punch-zoom, freeze/stutter and squash effects. The matte and subject must survive source windows; no obvious green fringe or corrupted face.
- Minimize reaction obstruction of the real new demonstrations. Frank reacts *to* the content; viewers must be able to see the plot, interface and band's carrier handoff. Stage a deliberate escalation from cameo to full-frame interruption and back.
- No Frank clip, source soundtrack, proprietary fonts/album covers, external MIDI or media cache is committed or redistributed. The demo can run deterministically in **\`--no-reaction\` mode**, with an **original/synthetic test-key source** for CI composite validation. A fully Frank-keyed render remains a **separate maintainer-supplied-media + visual-acceptance gate**: absence of the local clip must not be renamed "passed".
- Keep both a source-media-extracted dialogue option and a procedural-score-only option. If original dialogue is present, duck/crossfade score at intentional cues and preserve intelligibility. Audio licenses and optional ffmpeg/device backends remain scoped to the media demo, not mandatory on library builds.

## 5. The cinematic score and acoustics

Read \`docs/CINEMATIC_SCORE_PLAN.md\` on the cinematic branch and \`docs/HUMAN_MUSIC_C137_HARDENING_LEDGER.md\` on the active music branch before construction.

- A movie score is **not** an extra fixed sine pad masquerading as HumanMusic. Build or reuse the small show-side *meaning* projection \`M\`, then let the accepted C137 generator compose/perform the music. Respect \`mu(song)\` vs target \`F(trace)\` and the accepted narrative/physical bounds.
- We must **not** infer ensemble storytelling from note density or overall band note count. At the latest inspected C137 head, read-only \`germ_carry_shortfalls\` and \`carrier_shortfalls\` identify source-specific motif carry and missing named carriers. If the "tutti" isn't actually plural, do not label it one; repair opt-in and re-ear if needed.
- Retain the score/written historical defaults, canonical source IDs, performance fingerprint, and ear-accepted sound unless a deliberately opt-in change is auditioned.
- Fix one master A/V time origin, exact integer sample↔edit mapping and soundtrack length, including frame cadence boundaries, source dialogue windows, audio tails and final-sting hold. If video and PCM use different rates, explicitly resample/round and measure boundary mismatch; no drift just because both print elapsed seconds.
- At least three audio reference variants: original accepted generator, new cinematic score, and no-lead/no-reaction ablations; plus independent stems if already exposed. Use human listening as the musical gate; no automated acoustic witness overrules what the ear actually hears.

## 6. Deliverable and release-closure gate

The final artifact is a **runnable interactive terminal intro/demo** based on the accepted integrated libgibson code, not a static MP4 with fake ANSI. A reproducible offline capture (ANSI frame sequence + audio WAV + optional authored review video) is a **secondary artifact** for review; it cannot substitute for the executable. All media-dependent output remains local to the user, outside source control.

Required:

1. **Integration proof:** exact SHAs/branch lineage for plot, spatial UI, cinematic authoring, auditory observation and C137. No main merge/release without owner signoff. All combined code tested on its **actual integration commit**; independent branch greenness doesn't transfer automatically.
2. **Truth ledger:** for each major v0.5 act, identify the actual enabled module, input source, transform, expected observable receipt and a falsifier. No placeholder screenshot of a capability that does not exist.
3. **Provenance/geometry:** data, selection identity, song source motifs and cinematic shot identity survive their respective transformations. Underlying film outside reaction union remains byte-identical.
4. **Temporal precision:** single deterministic edit timeline; seek-at-time CLI (incl. hold/freeze), stable dumps at every act \`b-ε,b,b+ε\`; no ticking second "reaction clock". Seek/clip/audio boundaries checked.
5. **Visual/capability matrix:** real terminal and headless; compact and wide sizes; truecolor, ANSI256, mono/static fallback; no unwarranted 60fps/perceptual resolution claims. Measure terminal output bytes, dirty regions, frametime, memory and media cache boundedness.
6. **A/V/audio honesty:** actual WAV + source audio if present, clean deliberate dialogue sidechain; no clipping, unexplained audible jumps, fabricated auditory metrics or symbolic "pass" substituted for a human ear verdict.
7. **Local Frank gate:** with a maintainer-provided lawful source, render the keyed Frank cut and inspect multiple matte windows, reveal/transition corridors, actual comedic timing and layer obstruction. Report \`UNVERIFIED (media absent)\` if absent; NEVER auto-approve the full release finale from the no-reaction control alone.
8. **Maintainer-directed art gate:** final film must be **epic, coherent and funny**. It is accepted only after the maintainer watches/listens and says so; automated tests can only reject. Bring a short preview and a clean branch/PR for review, not a fake green release badge.

Keep release metadata/version/tag off-limits until the visual+audio flagship is accepted and the maintainer explicitly authorizes publication. Film art direction may be deranged; ABI/ownership/time/source semantics remain clinically correct.

## 7. Build order and safe parallelism

**Prerequisite:** verify fresh branch heads, reconcile the current 0.5 feature dependency graph and ensure exact-head CI. In particular PR #86 (cinematic) is stacked on #83 (spatial). C137 PR #87 is independent and actively changing. The older v0.5 three-axis research plan predates the dramatic score/narrative additions; treat these documents as evolving.

**Suggested order:**
- F0: feature/branch integration census + exact-head red/green tests and freeze the API seams without freezing artistry.
- F1: prove the integrated **silent** film accurately demonstrates all selected v0.5 visual features and remains seekable/one-clock.
- F2: score it using a show-side semantic trace → existing C137 generator, offline WAV; run no-lead and other A/B listening gates.
- F3: stage the keyed Frank compositor over F1 + soundtrack source windows, preserving the underlying surface invariant and source custody; add authored comedic cuts.
- F4: one-clock end-to-end runtime and captured review packet + performance/fallback matrix; fix failures *at their true owning layer*.
- F5: final maintainer watch/listen and explicit merge/release decision (not an agent's decision).

Fan-out to independent source audits, red-first probes, media/compositor review, musical tests and integration graph analysis **when agents are available**. **One principal integrator** owns the combined branch and cherry-picks reviewed children; other agents get separate worktrees/branches and file ownership, no competing writes on the same head or core modules. Agent count is not proof independence; use distinct analytical methods and hostile fixtures.

## 8. Durable references

- Existing v0.3.1/0.4 Frank reactions: \`examples/libgibson_intro_reaction.rs\`, \`examples/libgibson_intro_reaction/{director,compositor,transform}.rs\`, \`examples/reaction_soundtrack.rs\`, \`docs/FRANK_REACTION_CUT_PLAN.md\`.
- v0.5 spatial/cinematic: https://github.com/femboy2112/libgibson/pull/83 and https://github.com/femboy2112/libgibson/pull/86; \`examples/cinematic_showcase.rs\`, \`examples/cinematic_showcase/reel.rs\`, \`docs/CINEMATIC_AUTHORING.md\`, \`docs/CINEMATIC_SCORE_PLAN.md\`.
- v0.5 observable plotting: https://github.com/femboy2112/libgibson/pull/81; \`docs/PLOT_OBSERVABLE_GEOMETRY.md\`.
- v0.5 auditory observer research/implementation: https://github.com/femboy2112/libgibson/pull/84 and current C137 work https://github.com/femboy2112/libgibson/pull/87.
- Rick-C137 bridge and ear requirements: \`docs/HUMAN_MUSIC_C137_HARDENING_LEDGER.md\` and \`docs/HUMAN_MUSIC_MEATSACK_PROJECTION_BOUNDARY.md\` on PR #87.
- Later reserved programs, **not 0.5**: https://github.com/femboy2112/libgibson/pull/90 (GSPU-FM) and https://github.com/femboy2112/libgibson/pull/91 (DOOM + 1.0 language ABI parity).

**The last thing v0.5 does is make an utterly absurd movie that is mathematically, acoustically, visually, and architecturally true.** The maintainer watches Frank react to the impossible terminal and approves the actual film; only then is v0.5 ready to be called done.
