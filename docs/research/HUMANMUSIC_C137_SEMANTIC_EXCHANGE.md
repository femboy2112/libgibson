# C137 SEMANTIC EXCHANGE — THE BAND HAS TECHNIQUE; MAKE IT CONVEY IDEAS
## Source-grounded research handoff, not a runtime change

**Audited snapshot:** femboy2112/libgibson, draft PR #108, branch feat/humanmusic-c137-phase-change, exact HEAD f170f9e07c4a52b9fe7a9beb0df538b82a211296. It is three commits ahead of PR #106 and stacked on the existing causal kernel. Exact-head GitHub Actions run 38338109472 **succeeded**. Research branch only; no source/runtime/ABI/default/main/remote-PR merge, no audio generation in this audit.

**Maintainer's direct listening:** "a coherent professional band trying to show off." More fluency and coherent pocket than earlier, but insufficient transfer of intelligible musical ideas. Required: **the semantics and instrumental exchanges of teacher_golden7 without sacrificing coherent information density, detail, pocket, or natural ensemble identity; no pretentious ornament for ornament's sake**.

The actual accepted seven golden WAV files and listener annotations are NOT available through this audited GitHub source. Their music should not be invented from a reconstructed example. The original rick_probe.rs / narrative teacher sources are available as structural calibration, and the full accepted golden audio should be recovered/hash-verified on the maintainer's local machine if possible. The user verdict is evidence of the perceived symptom; the specific causes below are code hypotheses / source findings, not freshly measured PCM.

## 1. Preserve the REAL progress

PR #108 includes:
- A single continuous **64-beat / 16-bar**, approximately 26.67 seconds of notated music at 144 BPM (plus render tail), four-section verse → hook → bridge → return. No independently concatenated WAVs.
- A developed/embellished full ThemeFamily verse and hook, a deliberately thinner generated bridge and source-verified relatedness.
- Frozen source harmonies before dependent planning, a source-preserving performer with effective hearings, an explicit action+interaction ledger and bridge/hook provenance from PR #107's red work.
- Shared meter and bounded pocket, functional A7 dominant correctly classified Driving (not Home), monophonic safe bass floor, keys answers, actual keys→drum fill dependence, source/mute negatives, a pad bed.
- Old 4-bar and 8-bar opt-in controls remain, as do historical v0.4/R17/covers. Exact-head PR #108 CI is green. Preserve these instead of abandoning them for yet another composer.

These are structural advances. Passing a source/kinship/argument witness establishes source legality; it does NOT show that the audience can hear the intended meaning.

## 2. Root observation: most notes are arranged, few ideas are EXCHANGED

**S1: logical dependency is not sufficient audible path dependence.** In examples/c137_song/source_bridge.rs, the final Return step reuses ReferentId(1) = the hook, with ArgumentTransform::default. The Route appends the same HOOK_PROG for both the first Hook and final Return. The Return step names the Bridge in depends_on, which ensures logical precedence, but no mechanism requires the returning SOUND to encode a specific fact from the Bridge. Repetition can absolutely be the correct ending; the problem is **unproven recontextualization**, not reused melody. Current source tests reject an UNRELATED bridge or WRONG return referent, but not two distinct LAWFUL bridges whose alleged return is effectively identical.

**Decisive falsifier:** Create two different legal bridge motifs B1/B2 with the same verse, hook, length and harmonic route. Render both. Compare the last 16 beats' pitch/rhythm/guide-tone/role duty/expressive dynamics, excluding mere metadata. Predeclare which source features from B1/B2 the return should recognize. If the final performance is unchanged except trivial score/provenance fields, its dependency edge is formal, not musically causal. This could be solved by keys carrying a bridge cell, bass guide-tone reinterpretation, a withheld cue, altered hook articulation or drum phrase preparation; the actual hook lead notes need not change.

**S2: keys can answer, but only through a severely compressed interaction channel.** In kernel.rs::perform_song, every 16-beat episode's keys window is fixed at [e*16+4,e*16+8); the decision uses whether the FIRST BAR's lead is tonic-closed, overall rising, and the stage; otherwise it may quote the previous unpaid call. In material.rs::transform_material, Quote and Invert take at most the LAST FOUR events. Then project_pitched places those events on the current chord's stable tones. All lawful musical questions that map to the same (closed, rising, source tail after projection, stage, debt bit) are indistinguishable to this responder. Not every distinct source must differ, but distinguishing relevant QUESTIONS should matter. The bass is never given that melodic reply as a named duty; it mostly receives the harmonic chart.

**Falsifier:** Equal beat-count, energy and harmony with two SOURCE-distinct calls that differ in a meaningful salient motive or expected answer. Show the keys/drums either change lawfully or intentionally withhold for a source-grounded reason; show transformations remain audible *after* chord snapping. Keep note-count and patch matched. Preserve exact source identities and origin IDs.

**S3: drummer technique is generous but its semantic inputs are narrow.** Fixed backbeat anchors, hat subdivision on Home/Building/Driving, lead-gap ghost/kick comp selected until 0/2/3-per-bar budget, a four-hit snare fill at EVERY episode end (modulo index), plus an optional two-snare rhythm echo of the LAST TWO keys attacks. These are all legitimate ingredients of a talented kit. But repeating a fixed boundary fill under every kind of transition rewards elapsed time rather than a named obligation; the last-two-notes echo compresses the actual keyboard idea. The drummer currently cannot take a longer keys/lead rhythmic question and develop its meaningful answer into a later section.

**Falsifier:** Same bar grid/chords/pitch counts, different actual transition intents (continued phrase, preparatory hold, bridge pivot, earned tutti, false arrival). Demand different source-justified kit patterns; generic end-of-every-episode flourish should fail as a substitute for the specified preparation. A drummer may decide to do LESS, even silence, while the kick/hat pocket remains anchored where the source requests it.

**S4: bass playing is coherent but not participating much in the conversation.** The 16-bar kernel generates root/fifth/third/walking figure from current chord HarmonicDrive, plus a recurring chromatic approach toward next roots. It does not meaningfully read the actual keys material, lead question identity or earlier drum answer. This creates competent, repetitive motion. Re-enable neither the broken low BASS_WEAVE nor global R8 collision solver. A bassist can translate ONE source interval/rhythmic head/answer into a short legal destination-bearing approach or contrary movement as part of the SINGLE monophonic line, without abandoning the foundation.

**Falsifier:** Same chords and groove, but alter a named lead/keys source cue. A bound bass response changes only in an authorized window, while downbeat foundation, midi 36–55 register, direct audible mono capacity and source-stage law remain. A source without bass duty may leave the baseline bass unchanged; no forced novelty for every mutation.

**S5: sustained pad makes density feel thicker without proportionate meaning.** PR #108 enables Pad for ALL sixteen bars; the kernel voices each per-bar chord's pitch classes starting from MIDI ~50 as a bar-long blanket, with the same HarmonicDrive/groove_swell scalar used by other backing parts. Pad does not read a specific lead/keys/bass cue, and the conditional is all-song stage admission. This may bridge texture and can be useful, but it's also a likely competitor in the keys/lead mid-register. One long tonic rug may be good; 16 bars of continuously re-voiced chord sustain risks masking answers. This is a spectral/attention hypothesis, NOT measured masking evidence.

**Falsifier:** Same source+Score role phrases in matched mix with pad OFF / selectively SECTIONED / original full bed, equal-level, solo buses and pre-master contribution. Keep harmonic identity and reverb/limiter under control. Let pad silence be a deliberate contribution; no obligation to fill every bar.

**S6: a common energetic gesture can become another fader.** harmonic_drive and groove_swell drive the bass, kit and pad similarly; groove_swell's modulo-4-bar ramp resets each episode. This is source-aware at chord level, but not individual SOURCE-MATERIAL awareness. Phrase A in a new section is not necessarily entitled to the same dynamic ramp as phrase A in every other section. The earlier rejected CoupledR8 and common fader are warnings against artificially increasing collective similarity.

**S7: BandMemory is still just open_call + previous material.** The new provenance fix correctly associates paid answer Actions with the true originating source, and registers Interaction rows. Preserve it. But a one-bit debt and a single cell cannot retain several live ideas or remember which motif the drums or bass quoted; prev_answer_events is written but not used. The kernel currently has a mostly one-way lead→keys→drum causality, not an evolving many-role conversational history. Do not add a second disconnected "agent framework": reuse existing MaterialId, Interaction, ActionCause, ArgumentStep, Stage and obligations. Give player-specific bounded memory only where demonstrated by counterfactual evidence.

## 2b. The teacher's practical secret: distributed RHYTHMIC/HARMONIC JOBS

The handwritten examples/rick_probe.rs is an especially instructive architectural CONTROL. It must not be mislabeled as the complete accepted golden7 audio set.

From exact source:
- bass_dense writes a full functional line: root at 0, third at 1, fifth at 1.5, seventh at 2, fifth at 2.5, root at 3, followed by two destination-root chromatic approaches at 3.5 and 3.75 (eight pitched events on ordinary bars). This expresses harmony and NEXT-HARMONY expectation, with one monophonic line. It does NOT add a second low melody over an occupied foundation.
- keys_dense writes a downbeat guide-tone/upper-color chord (third, seventh, upper extension) plus four syncopated single-note stabs at beat offsets 1.25, 1.75, 2.75 and 3.25. Crucially those stabs occupy deliberate holes BETWEEN the bass's scheduled events, making two independently coherent parts lock together without having to mute either whole voice.
- drums_dense supplies 16th hats, downbeat/backbeat and ghosts, with chorus-specific pushes and DIFFERENT announced fill treatments. It is not identical 4-snare crescendo at every four-bar boundary.
- pad only enters on the chorus and selected outro home, not sixteen consecutive bars of mandatory harmony.
- lead_phrase delivers complete, rhythmically differentiated four-bar verse, hook, and bridge phrases with their own per-note expression; the 56-bar chart and section fill rules carry shared direction.

This is a highly concrete counterexample to the mistaken assumption that semantic band dialogue requires every backing musician to literally invert/quote the lead on every bar. The teacher can sound conversational because instrumental roles are **complementary over time and harmonic destination**. Some of the information exchange is realized as "your offbeat stab makes my bass approach intelligible" or "your silence lets my next phrase mean something" — not as a tagged answer Action.

Compared with that calibration, the newest kernel dedicates keys to one source-answer window per four-bar episode, relies on a largely root/third/fifth bass figure of 2 or 4 notes/bar, uses frequent formulaic kit fills and enables full-bar pad harmonies across the whole sixteen-bar arc. The roles aren't simply too active; **the detail is distributed into the wrong functional responsibilities**. A denser, idiomatic source-connected bass + harmonic counter-comp may actually sound CLEARER than a sparse keyboard plus continuous pad and hyperactive kit.

Implementation lesson: BEFORE writing a bigger inter-agent cue graph, make the role-specific part writers share source/harmony/metric obligations with deliberate complementary placements. Then weave a FEW identifiable thematic cues and long-horizon debts THROUGH that living arrangement, not on top as additional solo voices. Keep a shared rhythmic field and independent melodic schedules. Protect written silence, lead foreground, bass mono capacity and a single synthesis clock. Compare teacher as structural rhythmic/harmonic evidence, not as copied MIDI arrays or a claim of reproducing accepted audio.

## 3. The corrected mathematical target — relational musical information, not maximal entropy

The right object is a **source-conditioned, causally committed ensemble passage**. A phrase may contain:
- one recognizable **foreground proposition** (a contour/rhythmic signature, expectation, return, denied arrival);
- complementary roles playing harmonic, rhythmic, or textural BACKGROUND duties;
- named opportunities for an answering role to TRANSFORM and convey an identifiable feature of an earlier proposition;
- consciously reserved silence and space;
- open musical questions with individual source IDs and actual audible resolution.

This isn't literal literary semantics. In instrumental music the "proposition" means a musician-recognizable musical relationship/expectation, not an imposed verbal story.

For a musical gesture m, each instrument implements a **partial idiom-sensitive projection**:
- keys: rhythmic/melodic inversion or changed harmonic realization;
- bass: guide-tone/approach/root anticipation retaining the necessary relationship;
- drums: contour of accent/spacing/anticipation, with no literal pitches;
- pad: stable harmonic support or meaningful deliberate absence;
- lead: source-preserving thematic reprise or licensed expressive development.

The transformation must declare which invariants it preserves (rhythmic cell, accent envelope, melodic head, harmonic expectation, interval direction, temporal relation) and what it changes; not every pair of projections must commute. When a desired source invariant is erased by chord snapping or timbral/masking effects, refuse or select a different licensed interpretation. The rendering must be faithful on a small, PREDECLARED contrast set, never globally injective.

### Correct categorical law

Let S be a frozen source-argument/harmonic/pulse context and H_i a role's committed, prior heard events. An instrument's response is a partial map R_i(S,H_i, duty_i) that emits admissible gestures. A cross-role musical arrow j→i exists only if a concrete source ID/material/beat window from j changes i's decision within a licensed causal horizon.

For a given source question m, the auditory rendering and relation witness must agree on the meaningful source-contrast under the chosen style/domain. An old gate accepting the planned label is not enough.

Phrases/sections partially compose only when their outgoing source debts, carrier duties, harmonic function, beat/pocket, voice capacity, event continuity and performer memories match the incoming state. The listener may hear the same foreground melody over a new harmonic counterline; the path changes its interpretation even when the pitches of that melody are unchanged.

**Attention/semantic channel capacity:** each listener can only track some simultaneous salient streams. But do NOT hardcap the band at one active instrument; teacher_golden7 demonstrates high concurrent information. Instead assign a phrase-level foreground or exchange among roles, let others provide idiomatic foundation/counter-rhythm, and keep source-specific signal audible in time/register/dynamics. A low-valued fill is dispensable; a true source response is not.

Keep the existing ComplexityBudget as an optional attack-texture cap. It is not a semantic-value score. Do not claim "bits of meaning" without a probability model and blinded human responses.

## 4. The most efficient intervention: PRESERVE DENSITY, CHANGE ITS SOURCE OF AUTHORITY

Do not "fix pretentiousness" with a global note-density multiplier or a mute-all-background switch. Run TWO parallel density-controlled treatments:

- **Surface-dense control:** existing same seed, 16 bars, same instruments/patch/tempo/charts, current generic fills, walking bass, full pad, keys answer.
- **Relationally dense treatment:** similar total audible attack activity and groove propulsion but transfer select phrase-level embellishment budget from automatic drum fills, blanket pad and indiscriminate walking into actual themed keys response, bass destination communication, drummer anticipation/withhold and return accompaniment that depends on bridge history.

Also test a **simplified treatment** which only removes unjustified strokes (same source and pocket) and a **shuffle/no-cue** mutant which preserves note count and average energy but breaks the named source relationship.

Keep note counts and full-mix loudness measured as covariates, not success criteria. If the simpler version wins by ear, use it. If the denser relational treatment sounds strong, retain the density. If a good fixed band already handles a particular bar better, do not make it "smarter" there.

## 5. Predeclared red/green adversarial controls

1. **Two lawful bridges, same hook and final chart.** Modify ONLY bridge source material while retaining legitimate hook kinship/departure, beat count and route. The return is declared earned: its lead may repeat unchanged, but some specified RETURN accompaniment/phrase decision must depend on bridge evidence, or the strong earned-interpretation claim fails. Keep earlier verse/hook invariant.
2. **Formal dependency without performance use.** Add/remove an unused ArgumentStep.depends_on edge. The output cannot claim a new audible semantic exchange merely by changed metadata.
3. **Equal count, different question.** Same beat/onset count and energy, different meaningful contour/cadence anticipation; keys/musician should react when duty warrants. If different source questions project to identical chord tones, classify as a real information-loss collision, not a passing source answer.
4. **Actual typed interaction chain.** Source lead → keys response → drum rhythm and/or bass anticipation. Every edge cites actual source event/action IDs, correct time and effective Hearing; mutation of a required upstream event changes/rejects its intended downstream response.
5. **Musician-agency ablation.** Keep global chart and pulse, disable named message transfers. A supposed inter-player conversation whose output doesn't change was decorative. Delayed/negative coordination is valid; raw energy correlation is not a quality score.
6. **No gratuitous fill.** Same transition length but no semantic prep obligation. The default four-hit transition fill and ghost comp should not automatically count as an earned cue.
7. **Padding-with-notes mutant.** Add unrelated pitch-correct fills to equal/exceed target note density without actual source binding. Old R gates can pass, but new relational observer and blind listener should separate it from the source-bound arrangement.
8. **Silence as evidence.** Withhold keys or kit in a particular source window and recompute dependent players. Must distinguish intentional silence from missing/stage-inactive notes. Allow an unanswered idea to remain open when declared.
9. **Bass foundation and mono negatives.** No extra low countervoice, lost root, rapid same-note retrigger, unsupported chord crossing, unauthorized stage or capacity violation. Preserve held BASS_WEAVE=false as control.
10. **Pad occupancy/masking.** Full rug vs selective support vs no pad; measure score voice space, bus spectra, lead/keys audibility and compare matched blind takes. PCM spectral occupancy isn't a psychoacoustic masking proof.
11. **Return role meaning.** Same melodic hook but source-specific changed bass/keys/pad/drum disposition after different bridge is a valid, independently audible reinterpretation; a mere return label or uniformly louder band is not.
12. **Musical pocket preservation.** Same shared pulse, stable source-relative phase offsets, no 3/2 drift, source-preserving note identity, unchanged historical R17. Distinguish musical layback from random jitter.
13. **Long-form boundary law.** Neutral repartition of a section changes no performance events; actual musical transition permits only licensed preparation and later response windows.
14. **Live audio honest result.** User blind listening determines whether idea exchange is FOLLOWABLE and COMPENSATES for ornament, not static gate PASS or the engineer's description of source types.

Use mutated, re-PERFORMED source inputs, not exported after-the-fact wav mutes, when testing actual causal decisions. Keep mutes as explicitly uncommitted acoustic diagnostic controls.

## 6. Implement in bounded, honest rounds

**R0 — Actual listener/reference and source observability baseline.** Capture exact-head PR #108 SHA/CI and 8-/16-bar source, Score, WAVs/stems on user machine; pin actual accepted teacher_golden7 files/hashes if available, else reconstruct only under that label. Inventory current per-role activity/source actions; report ratio of source-bearing causal exchanges to generic texture WITHOUT inventing a semantic quality number. Set up masked, same-Score and source-mutant controls.

**R1 — Prove/disprove earned-return causality.** Write real Rust and Score-level tests for two lawful bridges, source-conditioned hook-returns, and a formal-only dependency edge. Preserve exact raw hook and current PCM as the control. Name the missing cross-bridge memory/role response. Do not scatter modifications to every player yet.

**R2 — One source-bound inter-player conversation with useful DENSITY.** Reuse existing InteractionMaterial/Action/Stage/EffectiveHearing, add the smallest necessary per-role phrase-duty or transition state, not another generalized ontology. A specific source question passes across at least TWO distinct roles and changes a later development through a distinct idiomatic projection, with a legible silence/hand-off. Preserve all prior callers' true causal references. Show note-count-matched decoration fails.

**R3 — Restore idiomatic distributed density.** Give keys more than one fixed 4-note window when needed: lawful harmonic comp and source counterline, sometimes silence. Bass longer interesting but mono-safe, tonic-oriented, anticipatory motion only when warranted. Drums steady pocket plus source-prepared fills and rest instead of every episode's fixed showcase fill. Pad selective texture/harmonic support, not an always-on blanket. Keep high rhythmic information but allocate it to musical responsibilities rather than autonomous virtuoso ornaments. Use source-dependent motives/cell memory, not randomness or a global volume knob.

**R4 — Genuine 16→32-bar consequence propagation.** One global Score and synth, real verse/hook/depart/return, at least one changed bridge meaning visibly alters return interpretation. Tempo/mode/world constraints and source-kinship laws remain. Build an 8–16 bar conversation A/B first, then 32 bars if successful, then 48–56 once human listening judges it viable. No mere repeated four-episode loop.

**R5 — Relational listening study.** Randomize blind labels for baseline PR #108, decorated busy control, relational treatment, simplified treatment, wrong-bridge/no-message control, recovered teacher and role stems. Level/tempo harmonize matched experimental arms; native teacher context separately. Ask what idea was stated/repeated, which instrument answered, what the bridge caused, whether the hook return felt EARNED, whether any section was trying too hard, and whether the pocket/band feel remained. Record user's verdict and exact failures; no claim of universal comprehension.

**R6 — Consolidation, safety, handoff.** New non-main branch from latest actual PR #108, small reviewable commits, draft PR, source/Score/PCM and SHA custody, strict fmt/clippy/doc/tests/CI, archived R17 48 WAV hashes/757 receipts, v0.4 defaults, covers, source/action/Interaction foreign keys, pitch-basis, mono bass, stage and final-hearing invariants. Do not touch main, releases, other v0.5 cinematic obligations, Meatsack, FM, or language parity. Preserve worktrees and audio archives as in PR #105.

**Safe independent scouts:** (A) bridge/return counterfactual, (B) keys response + pitch projection fidelity, (C) ensemble density/attention/masking, (D) per-player role-state/voice/pocket, (E) tests/teacher/acoustic controls. One principal owns overlapping kernel.rs/material.rs/performance.rs. First agree the minimal role-duty interface; no five parallel composers.

## 7. What counts as C137 progress

- The intro introduces a **memorable** musical object with a clear rhythmic/harmonic identity.
- The hook isn't just louder: it answers, redirects or reframes that object.
- The bassist keeps the pocket and communicates the next destination.
- The keys add a meaningful related idea or know when to step back.
- The drummer plays one musician's impulse into another's future rather than filling every mathematically available hole.
- The bridge changes the band's shared expectations.
- The return incorporates a SPECIFIC musical consequence of the bridge, even if its recognizable headline melody remains exactly the hook.
- A busy passage is intelligible because its busy voices serve different aspects of ONE situation, not because all players are demonstrating skills.
- Silence is not missing data but a musician's chosen gesture.
- When optional flourishes are subtracted, only their actual musical value should justify keeping them.
- Full-song articulation, bass mono capacity, one pocket and source/interaction/witness truth remain intact.

The exact simple teacher reveals why this can work without massive bureaucracy: it has melodic/rhythmic/harmonic ROLE RELATIONSHIPS planned together. It does not need five independent cleverness engines. Our existing categorical/causal source machinery is enough if we make the relationships LOAD-BEARING in the performed music.

**Bottom line:** Stop teaching five virtuosos to be independently impressive. Let a few good musical ideas travel through five idiomatic musical languages, maintaining recognizability, harmonic destination, rhythmic pocket and cause-and-effect from statement through return. The result should be rich and alive while feeling effortless to hear.
