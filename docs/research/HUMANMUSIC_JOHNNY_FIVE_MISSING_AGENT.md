# OPERATION JOHNNY FIVE — THE MISSING MUSICAL AGENT
## Why the C137 band is extremely polished but still performs like a script

**STATUS:** research / hostile source audit and next-round implementation handoff. **Not a Rust patch, not an actual PCM audition, not a proof of musical quality.** Pinned source is draft PR #111, \`feat/humanmusic-c137-expressive-transfer@f68c9bfca179f9f248dd5cd601f0c4d3987a1658\`, stacked on #110, #108, #106 and Codex's #104. Exact-head GitHub Actions 38492405201: **SUCCESS**. Released \`main\` still stands apart. This research branch is based directly on that head and changes **docs only**.

**Maintainer hearing:** ~90% of the way to the seven accepted handcrafted \`teacher_golden7\` renders. The generated music has strong structure, dynamics, harmony, performance articulation and pocket, yet its underlying flow lacks "smartness/aliveness" (Number 5 → Johnny Five). Individual instrumental upgrades (expressive lead, keys comp, bass V→I, drum lead-echo) were accepted by ear in #111. Do not regress those accomplishments. The accepted golden7 WAVs were NOT obtained or listened to by this auditor; compare actual media only when locally hash-verified. No empirical claim here that the proposed cure sounds better.

## EXECUTIVE DIAGNOSIS

The frontier's main problem is **not that its musicians lack humanized note timing or velocity**. They lack enough **decision-relevant local musical state and authorized alternatives**. The current C137 architecture largely maps an already authored lead/harmony/episode label to hardcoded accompaniment, then adds excellent articulation/voice-leading. It has proven causal interactions, but a large part of its behavior is a response to a **very coarse quotient of the actual music**.

This can be summarized as:

\`\`\`
                   GOOD TODAY                          MISSING 10%
  complete static verse/hook/bridge chart      source-born initiative and selective proposals
  repeating full lead source into every arc     motif custody moving through the band
  fixed player rules by chord/bar/relation      history-dependent alternatives (including silence)
  finite memory: one unpaid call               actual multi-debt / local performer memory
  keys response by closed/rising/tail          attended, source-relevant heard gestures + true anticipation
  bass destination and drum rhythm echo        action chosen because of other musician's prior action
  shared beat + velocity/gate shaping          maintained relational pocket with meaningful repairs
  impeccable end-result source receipts        truthful plan-vs-heard causality and candidate/choice receipts
\`\`\`

**Distinguish two propositions:** (a) source/Score can produce musical-sounding responses (demonstrated), from (b) a performer uses a history-sensitive policy to **choose among legal replies**, carrying a shared story forward (mostly NOT demonstrated on the current kernel). This is musical simulation, not artificial sentience; "Johnny Five" is an artistic metaphor.

## 1. POSITIVE HISTORY: THE MISSING FEATURE HAS AN EAR-ACCEPTED PRECEDENT

**Critical:** \`docs/HUMAN_MUSIC_BAND_STORY.md\` and \`src/audio/human_music/narrative.rs\` ALREADY prove by controlled, density/loudness-matched listening that a thematic identity passed among players is audible and beneficial. Earlier \`band_story_probe\` compared STATIC, NARRATIVE, SCRAMBLED carrier ordering and FLAT density-matched versions:
- The user said meaningful **handoff was load-bearing**; FLAT matched density but did not breathe like NARRATIVE.
- NARRATIVE survived a lead-mute (keys/bass still carried the germ); STATIC became backing without the melody.
- Reversing the same carrier sequence made dynamics **semantically unjustified**.
- A lead WITHHOLD where keys carried the "dark" read as **intentional silence**, not simply no notes.

The ear-accepted trajectory was **lead teaches → keys reinforce → bass develops → lead withholds and keys carry a harmonic miss → ensemble payoff**. Exact literal pitches and tempos were NOT a universal law; **carrier transfer + ordered relation** was the proved ingredient. Cite the archival human acceptance from that doc correctly; it came from a *hand-authored* probe, not a general C137 generator.

**Yet the CURRENT C137 kernel's \`longform_domain\` requires \`p.carriers == [Agent::Lead]\`, \`perform_song\` calls \`argument_transport::replace_required_role(..., Role::Lead,...)\`, and \`kernel.rs\` never references \`NarrativePlan\`.** \`PerformancePlan\` has an opt-in \`NarrativePolicy::Ensemble\` behind \`AdmissionInputs\`, but \`kernel::perform_song\` passes \`AdmissionInputs { argument, harmony, ..Default::default() }\`: the narrative handoff feature is NOT active on the current C137 path. Original full-song sources explicitly assign \`carriers: vec![Agent::Lead]\` for every episode. Keys can quote a tail and bass can harmonically anticipate; neither is assigned ownership of an entire thematic obligation as the ear-proved story required.

**MOST IMPORTANT NEW MOONSHOT:** recover this ALREADY EAR-VALIDATED type of interdependent, source-carrying agency in the SOURCE-FIRST kernel, with the correct source and role admissions rather than tacking another low bass countermelody onto a finished bass. Start with a bounded lead→keys handoff and an actual lead-withhold source, then a feasible bass role transfer within the mono foundation; prove the thematic identity survives both solo-stem and lead-mute tests. Don't re-enable rejected \`BASS_WEAVE\`.

## 2. CHECKABLE SOURCE-LEVEL OBSTRUCTIONS

### J1 — A 32-bar typed song is still three referents played repeatedly, not eight decisions

\`examples/c137_song/source_song.rs\` generates a single full verse and hook via \`generate_developed_pair_phrased\` and one bridge; the eight-section non-intro skeleton is \`VERSE→HOOK→VERSE→HOOK→BRIDGE→HOOK→HOOK→HOOK\`. Every post-bridge chorus is a \`Return\` of the SAME hook referent and cites the SAME bridge as a dependency. Exact identity of a refrain is valuable, but the last three choruses presently lack source-justified change in each player's duty merely because that player's own history moved on. The separate intro variant swaps one tail chorus for an actual sparse late lead pickup; the intro's source and rhythm-section build are genuine steps forward and must be preserved.

The 32-bar source is still optional (\`--form=song\` or \`--form=song-intro\`) until the ear accepts it. The default remains 16 bars, exactly as intended by the current acceptance gate. Do NOT silently promote a repetitive 32-bar template to C137 default merely for length.

**Red experiment:** compare normalized time-relative lead/keys/bass/drum subsequences in post-bridge Return episodes 5/6/7, holding the same source and harmonic chart. Record exact repeating role windows and any genuine differences. Then mutate a real prior band interaction while keeping hook/chords invariant: the next two returns should change *where the source narrative gives a reason*, not because of randomization. Any new 3rd-chorus variation must be source- and performer-memory-owned.

### J2 — The current observation quotient destroys much of the source

\`kernel::perform_song\` selects the keys base policy from essentially:
- lead event window in the FIRST four beats of a 16-beat episode;
- whether the last heard lead pitch closes on tonic;
- whether the last heard pitch is above the first;
- stage availability;
- one pending-call Boolean and the prior source material;
- for Return, a declared bridge index.

The base response is \`Silence\`, \`Invert\` or \`Quote\`, placed at the SAME \`episode_start + 4.0\` in an inflexible [4,8) answer window.

Outside that reply, keys comp is a fixed pair of offbeat chord voicings in each on-stage home bar; bass is root/fifth/third/next-root; drums follow regular kick/snare/hat slots with lead-onset quote and basic harmony density. The effective policy has *very few observational and action distinctions*. Two genuinely different prior musical situations may collapse to the same chord/root/relation/rising/end-note summary and therefore select the SAME musical action. More expressive envelopes cannot restore musical distinctions the policy never read.

**Rigorous finite information-loss lemma:** let \`O_i:H_i→Q_i\` be a musician's actual observation summary, and \`π_i = f_i∘O_i\`. If \`O_i(h)=O_i(h')\`, then \`π_i(h)=π_i(h')\` for ANY deterministic implementation of \`f_i\`. Distinct source states that deserve different replies but collapse under \`O_i\` are **mathematically unanswerable by that policy**. This is not a proof that any particular test source should sound different; the counterfactual musical obligations must be PREDECLARED before probing. Remedy: add only the **specific source and prior-performance observations needed to distinguish a legitimate next decision**, not a large artificial intelligence score.

**Red experiment:** find two lawful 16-bar sources with the same chart, form, loudness/onset count, first four lead beats and summary predicates, but different later meaningful motif/debt. Test whether the present backing is (largely) identical. Attribute each difference by dependency. Choose an intervention that demonstrably ought to change a named keys/drum/bass answer, and see whether the quotient blocks it. Mutating debug labels alone must have NO audible consequence.

### J3 — We may be labeling *rehearsed future knowledge* as *heard improvisation*

This is the most consequential epistemic/causal seam to falsify:

- Keys \`EffectiveHearing::capture(Agent::Keys, Role::Lead, ws, ws+4, &score)\` records only the first FOUR beats of the lead episode.
- Keys begin their response at \`keys_at=ws+4\`.
- Yet \`cell = perf.materials[e].clone()\` is the authored material for the WHOLE 16-beat lead episode.
- \`material::transform_material\` always uses the LAST at-most-four events of \`src.events\` for \`Quote\` and \`Invert\`, i.e., typically the **future cadence tail** not heard by the keys as of \`ws+4\`.
- \`Response\` then records \`call.end_beat = origin.start_beat + origin.length()\` (almost the whole episode), \`latency = keys_at - call.end_beat\` (NEGATIVE for an answer during the call), but explicitly stores \`overlap:false\`. Source and action provenance may still link correctly while the auditory interpretation is wrong.

It is **not invalid for a rehearsed musician to know the future chart and future written lead phrase**. The existing \`SongMap\` is such a shared score. It is invalid to claim the keys *heard an actual future performed phrase* or to label the negative latency as non-overlap. **Unify the semantics, don't forcibly delay good musical answers:** distinguish:
1. \`HeardActual(prefix, until_beat)\` — a true performed-history observation.
2. \`KnownAuthoredFuture(source_id, future_window)\` — anticipated from the common score.
3. \`PredictedOther(action, confidence/contract)\` — optional bounded forecast of another player's not-yet-committed behavior, which must be verified or invalidated when the event happens.

Reuse \`EffectiveHearing\`, \`Call\`, \`Response\`, \`InteractionMaterial\`, and \`Action\`; do NOT invent an epistemology framework with a hundred types. Existing properties \`Response.overlap\` and \`latency\` should correctly record a planned overlapping anticipation when that is what occurs. If a response is presented as adaptive *listening*, its musical material must be a function of actually heard notes, not unreleased future material. Write two red-first tests that distinguish both lawful regimes.

**Causal horizon law:** for interventions to future *unannounced* performances, committed past responses must be unchanged. Future AUTHORED source changes may influence explicitly preplanned anticipations; label that dependence as score knowledge, not hearing. A claimed adaptive answer whose deciding information comes from a future tail is a false observation receipt, even if the song sounds good.

### J4 — A single Boolean is not a musician's history

\`BandMemory {open_call: bool, prev_answer_events: usize}\` plus \`carried_cell: Option<InteractionMaterial>\` is the whole long-form persistent memory. \`prev_answer_events\` has only a field declaration and an assignment (no actual reader). Every episode overwrites \`carried_cell\`. Three different pending calls cannot survive two silent opportunities as individually named obligations. No bass-specific and drummer-specific phrase-memory state evolves in the C137 kernel.

The older \`interaction.rs::InteractionMemory\` ALREADY exists, with remembered initiator, responder, latency, placement and transformation and a finite novelty/penalty policy. The modern source-first kernel **does not use it**. The generic version's novelty biases are not automatically musically correct, but the needed capability is already conceptualized in-repo.

**Red experiment:** open source-distinct obligations A and B; have the next keys opportunity discharge B; A must still be open by ID and later be recovered or explicitly left unresolved. Then alter an actually performed bass/drum phrase while the future authored chord chart remains identical; an authorized later response must account for what was actually played if the musician has heard it. Mere same-onset hashes are not enough.

### J5 — The remembered role remains "lead soloist, with accompaniment"

Even with true keys response receipts, the C137 program does not enact the previous proven **lead withhold / thematic migration / ensemble payoff** from \`NarrativePlan\`. \`longform_domain\` explicitly refuses non-lead carriers, and the long-form composer clears a single required source into the LEAD lane. Keys and bass may add source-derived decoration, but the lead is never allowed to yield its thematic duty to another player within the modern kernel.

**First vertical slice:** one already source-admissible line whose lead deliberately withholds at a prepared harmonic Miss (not at a protected new thesis), keys carry its thematic identity, bass keeps monophonic floor and MAY briefly develop its rhythmic cell as part of its OWN line, drums make-room instead of obligatory full fill, and lead returns after hearing the keys. Score must have truthful source IDs, timing, effective hearer receipts, narrative role/custody trace, and no made-up note or retroactive instrument rewrite. Actual lead-mute test: meaning still has a musical carrier in at least one other instrument.

The old NARRATIVE/STATIC/SCRAMBLED/FLAT A/B tests are the gold experiment. Adapt them to a **NEW generative C137 source** rather than reusing literal teacher pitches or claiming the older hand-authored evidence is sufficient for the generator.

### J6 — Ramps and section roles are still largely fixed, not self-modifying musical choices

The shared \`pocket_offset\` is one deterministic bar-position table for lead/bass/drums; \`groove_swell\` is the same \`0.94 + .12 * (bar % 4)/4\` per episode. The new intro-build \`SKELETON=[.80,.90,.96,1.0]\` and \`SWELL=[.30,.46,.72,1.0]\` are GOOD improvements—separate structural pocket from textural emergence—but remain prewritten curves, not a decision that responds to an actual interaction. The 32-bar form's middle and final choruses reuse the same referents and chord routes. This makes it likely that repeated choruses share nearly identical backing trajectories even when the band's experiences differ.

**Improve using small, role-specific state:** a remembered last answer, a pending handoff, an unfinished anticipated harmonic turn, and an explicit \`Withhold/Prepare/Answer/Support\` duty can change the NEXT section's articulation, comp location, drum preparation and bass anticipatory cell, while keeping the chord+lead refrain stable. A well-justified identically played refrain is ALSO legal; the model should distinguish purposeful sameness from lack of choice.

**No random timing jitter, no independent clocks, no magic energy optimizer, no new bus compressor trick.** Pocket is still the common reference; local phase is bounded and source-justified. Preserve the accepted R17 pocket and \`CoupledR8\` ear-rejected control.

### J7 — The kernel has no observable candidate-selection/initiative boundary

Current music is determined by one source referent followed by hardcoded rules: keys \`Silence/Invert/Quote\`, fixed keys comp slots, bass chord duty, drum hats/ghost/fills, and sections chosen in \`source_song.rs\`. There is no general short-horizon decision where a performer chooses between, e.g., **answer vs listen**, **quote vs withhold**, **bass approach vs anchor**, **drum echo vs spare the lead** because of something the OTHER musician ACTUALLY did and a named source goal.

The legacy \`InteractionMemory\` and \`Stage\` already model some alternatives; port the *smallest necessary part*, not the entire old \`perform_with_profile\` with its post-hoc repairs. A sufficient next seam is a **single four-bar bounded choice**:
1. Shared source/actual committed notes/final harmony determine a legitimate musical need.
2. Generate 2–4 lawfully admissible role-specific alternatives.
3. Select by named purpose and prior performer state: answer this source, preserve a foundation, create or keep a rest, prepare the next arrival. Seeded creativity may tie-break equally justified options at the PHRASE level, never inject per-note jitter.
4. Commit the chosen action (or named silence); downstream performers hear the actual result.
5. Record a compact witness: what was observed/known, which source IDs mattered, feasible alternatives, selected musical reason, and sound-bearing consequences. No false claim that a compiled deterministic program is a conscious agent.

**Counterfactual criterion:** with the same global chord chart and motif labels, changing an actual PRIOR authorized gesture changes only correctly dependent later choices (not everything); different irrelevant metadata does nothing. Scrambling the selected actions while holding count+RMS approximately fixed should be audibly worse. A "more notes" control that fills meaningful silence must fail the independent purpose/silence contract.

## 3. MATHEMATICAL MODEL — AN INDEXED CATEGORY OF LAWFUL PERFORMER DECISIONS

Let \`B\` be the fixed song-time/harmonic/pocket base. The state at boundary \`t\` is:

\`\`\`
X_t = (AuthoredSong_t, CommittedPast_t, Obligations_t,
       InstrumentLocalMemory_t, RoleDuties_t, ActiveVoiceTails_t).
\`\`\`

A role's actual observations are **typed**:
\`\`\`
O_i^heard : CommittedPast_[0,t] -> HeardFacts_i(t)
O_i^score : AuthoredSong -> KnownFutureSource_i(t)
\`\`\`

A lawful musical decision is a PARTIAL transition
\`\`\`
a_i : X_t ⇀ X_{t+Δ}
\`\`\`
with an emitted Score stream, updated performer state, and a source/meaning witness. It composes with another role or the next section only where its source IDs, time, voice capacity, authorized knowledge, phrase debts and actual hearing claims are compatible. A shared pulse is a compatibility BASE, not the diagonal \`e_bass(t)=e_drums(t)=e_lead(t)\`.

The two load-bearing invariants:

**Observer sufficiency / expressibility.** An action policy cannot distinguish source histories its actual observation map identifies. To get richer *purposeful* behavior, add a distinguishing source- or heard-history observable, not cosmetic dynamics or noise.

**No false foreknowledge.** A committed decision can depend on performed history up to its decision time and on DECLARED authored source/intent at future times; it cannot truthfully cite an *unheard actual future performance*. Verify all planned-vs-heard dependence edges, and choose legal action time/latency accordingly.

Categorical laws worth writing executable Rust tests for (not abstract poetry):
- Identity/no-op source metadata does not change music.
- Disjoint administrative repartition of a section does not re-roll the source or reset player memory.
- Gluing two performed episodes passes carried, ID-addressable debts and explicit active voice continuation correctly.
- Transposition of a lawful source/region transports Score-level source relations and pocket coordinates when licensed (not identical PCM).
- Rehearsed anticipation and truly reactive answers have DIFFERENT observation provenance.
- A neutral stage/score change outside the authorized causal cone cannot alter past independent events.
- A scheduled thematic handoff genuinely moves the music to another voice and survives lead-mute.

**Musical selection evidence** is a vector of independent source/Score/PCM observations and a human listening verdict, NEVER a scalar "Johnny intelligence" score, Shannon bits invented without a probability model, or a claim that mathematical proof certifies listening quality.

## 4. SCIENTIFIC TRIANGULATION (MOTIVATION, NOT A CLAIM ABOUT SOFTWARE)

Empirical joint-performance research distinguishes **shared goals** from the abilities to **anticipate, attend to and ADAPT** to another musician's actual actions. It suggests exactly which behavioral distinction the current source-first kernel must be tested for:

- Keller, Novembre & Hove (2014), *Rhythm in joint action*, https://pmc.ncbi.nlm.nih.gov/articles/PMC4240961/ — coordinated joint action uses anticipation, attention, adaptation.
- Keller (2014), *Ensemble Performance: Interpersonal Alignment of Musical Expression*, DOI 10.1093/acprof:oso/9780199659647.003.0015 — expressive coordination depends on specific temporal/intensity/articulation cues and performer intentions.
- *Coordination and Consonance Between Interacting, Improvising Musicians*, https://pmc.ncbi.nlm.nih.gov/articles/PMC8412203/ — in a controlled jazz duo study, mutually adaptive improvisation differs from overdubbed parts, with listener preference for the coupled condition.

These empirical results MOTIVATE a **mutual-adaptation vs overdub/fixed-plan** negative control. They do not prove Johnny Five is mathematically necessary, that the software is conscious, or that any specific code patch improves the ear.

## 5. EXECUTION PLAN — ONE HYPOTHESIS AT A TIME, NOT ANOTHER BUREAUCRACY

### R0 — Freeze, identify the *missing source*, then falsify
- Refresh latest SHA and PR. Freeze the accepted current 16-bar Full, the existing 8-bar controls, the 32-bar song/no-intro and intro/build branches, plus \`--source-answer\` exploratory source response; preserve WAV/stems/Score/manifest/SHA-256 and exact CLI. New 32-bar and source-answer remain ear-gated, NOT auto-promoted C137 Full.
- Reproduce source-level characterization in \`research/johnny_five/STATIC_EVIDENCE.md\`. Run REAL Rust/PCM tests for J2/J3 "heard first four beats, material derived from future cadence", including the negative-latency/overlap discrepancy. Do not just change \`overlap\` without fixing the contract.
- Recover verified teacher_golden7 media only if physically available; reconstructing one teacher is a clearly labeled control. Confirm exact baseline playback and acceptance provenance.
- Draw one dependency/knowledge graph of C137 source→plans→observations→performer decisions→commitments→mix. Mark each actual decision's knowledge (heard past? announced score future? generic style heuristic?), alternatives, and source purpose.

### R1 — Repair epistemic truth / action causality WITHOUT changing accepted notes unless necessary
- Introduce or reuse a small declared-future vs actual-heard distinction at the existing \`EffectiveHearing\` + \`Interaction\` seam.
- True reactive keys answer: derive from an authorized performed prefix available at response time; authored-anticipatory keys answer: may use future authored tail, but receipt must say score-known, overlapping and anticipated.
- Reconstruct the correct \`Call.end_beat\`, response overlap/latency and source provenance. Do not retrospectively change the entire future melody because an already-heard note was modified.
- Pin allowed hearing/Score knowledge via red/green temporal intervention tests. Preserve old acoustic reference as A/B; if the semantics-only repair leaves PCM identical, that's a valid win, not an audible Johnny Five claim.

### R2 — Recover the EAR-PROVEN carrier handoff inside the modern source kernel
- Trace the existing \`NarrativePlan::from_observation(MeaningPlan::observe(song))\` and \`NarrativePolicy::Ensemble\`. This is earlier EAR-PROVEN, but the current C137 route does not enact it.
- Design the smallest source-first carriage admission that can move one protected thematic obligation from lead to keys at a valid non-protected transition. The lead genuinely withholds or recedes at that site; keys fulfill the source in their instrument/register/timing, drums respond to the actual keys cue, bass protects floor.
- Follow with ONE safe bass development in its OWN monophonic line; reject any candidate that violates floor, source ID, voice lifetime or low-frequency masking. NEVER revive late \`BASS_WEAVE\` as a second low voice.
- Run STATIC, NARRATIVE, SCRAMBLED and FLAT controls with matched density/context; test lead-mute narrative survival and source/argument witness. Blind listening is mandatory; only the user decides if handoff has recovered the missing feel.
- If the original source requires lead carriage, implement a small explicitly licensed source/role handoff rather than circumventing the witness.

### R3 — A small local-choice protocol ("why this player played NOW")
- Build ONE 4-bar exchange with two or three legal keyboard/bass/drum alternatives and a meaningful intentional silence. Use existing \`Stage\`, \`InteractionMaterial\`, \`ActionPlan\`, \`InteractionMemory\`/debts and immutable \`CommittedSong\`, not a new musical actor graph.
- Make the choice depend on what an instrument actually heard and its declared own duty; add a minimal test-only decision trace. Reject unmatched, false or out-of-domain actions. Prevent the "same bar template every time" failure.
- Controlled experiment: mutually adaptive arrangement versus overdub/fixed choices on SAME source, pitch count and loudness, plus scrambled-choices arm. Maintain pulse time and harmonic destination; meaningful relative phase and silence are allowed.
- Demonstrate ONE payoff: source mutation in an early exchange produces a musically justified later variation while unrelated earlier bars remain invariant. Ear acceptance, not a score, decides promotion.

### R4 — Make repeated 32-bar refrains evolve WITH MEMORY, without erasing the refrain
- Keep the recognizable hook source, but let its custody, anticipation, register, rhythmic comp, bass approach, drum arrangement and dynamic response change only where the band's previous performed experience/obligations justify it.
- Test the triple post-bridge Return currently referencing the SAME bridge source. Propose an ordered progression (recognize → develop → final ensemble payoff) using existing typed source operations or narrow new legal relation. Do not pretend three identical \`Return\` edges generate development.
- Give musicians' independent memories a finite ID-addressable obligation state; one bool and an unused event count do not implement history.
- Preserve distinct sections, seamless source-prepared phase changes, one tempo base, non-clairvoyant actual hearings, and the first intro's skeleton-vs-texture buildup. Add a true intro→verse handoff only when supported by source.
- Deliver two-to-three genuinely different **lawful** full-song seeds and role stems. A correct unchanged refrain is allowed where the source intentionally repeats; all compulsory variation must have purpose.

### R5 — Acoustic and historical hardening
- Test 32-bar PCM and role-stem causal differences. Check drums' single snare voice and same-sample echo collisions (previous PR #112 research), keys six-voice pool, bass mono/voice occupancy, source/Score harmonics and mastering ablations.
- Run \`cargo fmt --check\`, all-targets/all-features Clippy warnings denied, rustdoc denied warnings, targeted/full Cargo/tests/examples/MSRV/ABI/PTY/Go, historical R17 48 WAV/757 diagnostics, cover/fingerprint/PCM controls and exact-head GitHub CI.
- Preserve frozen 16-bar user-accepted Full and 4/8-bar controls and Jerry switches. No promotion of experimental \`--source-answer\`, 32-bar form, intro-build or Johnny choice protocol to the default until each earns listening acceptance.
- One non-main code branch based on refreshed PR #111 head, coherent commits and a draft PR. No main merge/tag/release/ABI rewrite/default alteration without explicit user approval.
- Build versioned audition bundles with correct program SHA, song/world/seed, full mix, solo stems, true listened references, source/decision traces, negative-control mixes, source/Score fingerprints and audio SHA-256. Do not claim quality from tests or generated provenance labels alone.

## 6. STRONG FALSIFIERS AND STOP CONDITIONS

1. **Future-tail / false-hearing:** same heard lead prefix and same declared available source, but changed not-yet-known actual future phrase → earlier keys reaction must be unchanged; if explicitly authored-anticipatory, mark knowledge and adjust overlap.
2. **Coarse-observer collision:** equal first/last pitch, harmony and note count, different source-relevant question → named keys answer changes or refutes its claimed source purpose.
3. **Meaningful handoff:** when lead yields, keys or bass independently carries source motif with recognizable identity, lawful voice registration, and lead-muted audibility.
4. **SCRAMBLED negative:** same timings, dynamics and total notes, carrier/response order permuted ⇒ legal local notes but broken global argument; should NOT receive same semantic-completion witness.
5. **Overdub negative:** each player realizes its role from final chart but ignores earlier committed cues ⇒ fewer/different authorizing choices; old metrics may pass, but actual source exchange must fail/decline.
6. **Three open debts:** A,B opened, B answered, A remains identifiable; no silent overwrite or relabel.
7. **Static triple chorus:** change only prior performed call in an authorized section and demonstrate later role variation (or explicitly justify stable recurrence) without changing unrelated bars or the preserved hook's identity.
8. **Silence anti-saturation:** source-declared Listen/Withhold wins over a legal busy fill at least once; density gate cannot force that silence to disappear.
9. **Physical player:** same-sample snare retriggers, active keys-voice steals, bass note lifetime and intentional legato/detach are measured in actual PCM before declaring musical acceptance.
10. **No artificial novelty:** change irrelevant metadata, reorder harmless evaluation or vary seed without changing source decisions: no spurious audible variation. If a proposal engine emits noise to differentiate hashes, reject.
11. **Rehearsed vs improvised:** shared score may legitimately predict a future chord/phrase; actual performer history may legitimately change a later response. The receipt must tell these apart.
12. **Ear gate:** if NARRATIVE, adaptivity or longer form loses to frozen 16-bar Full in matched blind listening, preserve the baseline and characterize the first bad change rather than declaring victory.

## 7. RECOMMENDED AGENT PARALLELISM

- Scout A: source and knowledge provenance (J3), red-first true-hearing vs authored-future tests.
- Scout B: old \`NarrativePlan\`, handoff source/witness and lead-mute / scrambled controls (J5).
- Scout C: independent observable-quotient and actual local-choice micro-model, adversarial source pairs (J2/J7).
- Scout D: multi-debt/return memory and 32-bar refrain source identity (J1/J4).
- Scout E: true teacher and PCM/stem/acoustic checks, snare collisions and end-to-end blind packet.

One principal integrator owns \`kernel.rs\`, \`argument.rs\`, \`performance.rs\`, \`theme_family.rs\` and source fixture reconciliation; agents should not independently race those files. Push reviewed checkpoints, not unverified moonshot code.

## FINAL PRINCIPLE

**Number 5 already possesses motor control, a score, and an increasingly perfect performance style. Johnny Five needs conditional musical initiative:** what I have heard, what I know from the chart, what *I* am trying to accomplish, what my bandmates need, and which admissible action best serves the evolving song **right now**.

We do not need synthetic consciousness; we need a **truthful, finite musical decision process**. The most important proof we can run is not "the velocity changes." It is: **one musician's source-grounded musical decision changes what another musician chooses to do later, and the resulting band is audibly more coherent than either static backing or unrelated improvisation**.

Make that happen WITHOUT losing the successful notes, historical sound, common pocket, accepted pianist/bass/drum upgrades or the source's actual identity.

**Only the maintainer's ear can move Johnny Five from a hypothesis to C137 acceptance.**
