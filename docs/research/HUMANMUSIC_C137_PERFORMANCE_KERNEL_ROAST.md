# HUMANMUSIC C137 — THE PERFORMANCE KERNEL ROAST
## Why a green, complicated model can sound worse than the teacher

**Scope:** source-grounded audit and implementation handoff, not a coded Rust repair, not an accepted render. Pinned at feat/humanmusic-c137-ensemble-agency commit 29527a891ed7bfed9657a99046884a61670897e3, draft PR #102. Historical main remains on v0.4. Final hosted CI on this exact pinned head subsequently completed **successfully**: GitHub Actions run 38197706708. Earlier parent 2c945b5 was also green. These are tests of the existing code, not evidence that the music sounds golden. Recheck exact-head CI before implementation.

**User's finding:** Almost C137 but not teacher_golden7; the musicians are individually too aligned, dynamics statistically averaged, and a recent bass countermelody had to be withdrawn after stutter and mud. The real question is whether source, plan, playing, observing and reporting are the CORRECT categories of objects, or whether mutually inconsistent layers now certify different snapshots. The original seven accepted golden WAVs were not verified as GitHub-tracked media. NEVER invent their hashes or claim to have listened to them here.

**Thesis:** The old composition/synth can already make sophisticated sound. What has grown cumbersome is the construction of a causal performance. Multiple layers mutate emitted note lists AFTER a dependent musician has already heard them, and some mathematical witnesses observe only coarse note projections that omit the very dynamics under debate. More post-hoc enforcement cannot reliably produce musicianship. Strip back to a single frozen musical context and a bounded directed exchange of role-local performances, retaining historical controls and independent source witnesses.

## 1. Source facts, hypotheses and distinctions

### C1 — A dynamic-blind Hearing witness: verified source fact

In score.rs, Hearing owns a Vec<HeardNote>. The heard_note projection is EXACTLY:

    (start_beat, dur_beats, pitch, function)

It omits velocity, accent, effective intensity, source/debt provenance and, of course, downstream production. Score::stale_hearings compares this narrow tuple to final tuples. Consequently the same note with a different velocity produces an IDENTICAL HeardNote, although acoustic intensity changed. This is a mathematically exact finite counterexample to treating that projection as a verification of dynamic fidelity. It is not an assertion that the PCM is unchanged.

Do NOT silently break legacy HeardNote public/API behavior or old hashes. Add an opt-in final-effective-observation witness of exactly those parameters used by a dependent policy, and clarify which properties are OBSERVED versus just DECLARED. Pair it with an independent real Rust red test, not a fake claim from this paper.

### C2 — Late counterpoint and gain violate actual observation order: verified source order

In functor.rs::realize_policy_with_argument:

1. Realize the lead and create Hearing observations for keys/bass/drums.
2. Realize keys, bass, pad; collect drummer Hearing snapshots of bass and keys.
3. Realize drums from those events and earlier source plan.
4. ONLY THEN insert countersubject notes into keys/bass on the optional counterpoint arm. It removes earlier notes, inserts new ones, and (if bass counter is enabled) reties/reclips notes and reconstitutes voice links.
5. ONLY THEN multiply each non-argument pad/keys/bass note and drum hit by ONE shared section-energy scalar. A thematic note uses its own section gain.
6. Further tension repair may edit final notes afterward on its own narrower witness.

The drums thus cannot actually have heard the newly inserted counterline and did not decide their own playing in response to its final intensity. The old note-position Hearing can detect a missing or extra note when explicitly asked, but NOT the gain changes.

Current admit_take checks stage/orchestration violations; no comprehensive final-dynamic or final-hearing check was found in that admission path. perform_argument checks independent source-argument commitments but does not prove responsive ensemble dynamics. A live Rust run MUST determine whether latest counterpoint ON yields nonempty stale_hearings; no such run was performed in this audit.

The required mathematical square is straightforward: if role j consumes observable H_j(N_i) of role i, and i is later edited to N'_i, then EITHER H_j(N'_i) must equal what j consumed on ALL relevant fields OR j's plan must be invalidated and recomputed. Merely making the final Score satisfy another witness does not close the square.

### C3 — Source harmony and drive are installed AFTER dependent plans: verified source order

PerformancePlan::from_song_admitted constructs chords, regions, actions, stage, accent grid, interaction calls/responses, ensemble bar modes and budget. The argument route then calls argument::prepare_performance, replacing chords, contexts and deflection evidence. Only after budget allocation it flips perf.drive=true.

That is a split authority: the composer declares source harmony, the generic planner chooses its own intermediate harmony and decisions, then the argument changes harmonic state late. Whether a SPECIFIC dependent is wrong in a particular seed must be tested; the ordering defect is indisputable.

Astra: freeze FINAL source harmony and musical obligations BEFORE dependent planning, or explicitly rebuild every affected dependent. Prefer the former. Reuse the existing constrained-plan/cover preflight seams rather than inventing a second music planner. The final source cannot be an afterthought.

### C4 — Bass failed because its ROLE is wrong, not just its note scheduler: source plus maintainer listening

Commit sequence on PR #102:
- 31511bf: inserts a low bass germ-counter in lead gaps, deletes foundation notes from those windows and rebuilds mono voice links.
- 2cff879: clips notes to next onset to stop overlapping mono attacks.
- 2c945b5: ties fast equal-pitch neighbors to prevent repeated triggers.
- 29527a8: holds BASS_WEAVE=false after the low melody remained muddy and weakened the downbeat.

The woven bass line uses COUNTER_OCTAVE_BASS 2, with a MIDI 35–50 register band. The original bassist already owns a MONO low-frequency harmonic foundation. This is not two available independent music channels. Deleting note onsets does not plan a legitimate transfer of foundation responsibility; low notes may be correct chord tones but mask melodic movement and the bass's downbeat.

Do not re-enable BASS_WEAVE, raise every note by one octave, add gain, or tweak tail clipping as the next cure. First create an explicitly source-planned bassist responsibility in each short window: Foundation, Approach, BriefAnswer, Withhold, Rest. The bassist can become melodic with coherent root/3rd/5th/7th and anticipatory approaches, but must not silently lose the harmonic floor unless the song transfers or suspends that duty. Physical mono/lifetime/voice capacity, register and acoustic masking are HARD feasibility constraints. Compare actual PCM before deciding a melody is viable.

PR #102's BASS_WEAVE=false is the correct containment control; treat stutter and mud as separately observed failure mechanisms, not evidence that all bass countermelody is impossible.

### C5 — Complexity budget measures attacks, not reasons

budget.rs defines a weighted-onset-per-bar allowance. This is a valid texture/safety cap. It does NOT measure semantic density, guide-tone necessity, masking, motif retention or musical agency. Similarly PR #102 keys Listen estimates when to recede by lead onset COUNT thresholds (4/5/6), not by the contents of the actual musical Call or the statement's source obligation. The bass Withhold uses a real upcoming harmonic root change, which is useful, but it is still a local density patch.

Reuse interaction.rs instead: it ALREADY defines Call, Response, MaterialId, InteractionMemory, source-bound response transforms and Transform::Silence. An additional independent AgencyPlan/Cue ontology that restates these would worsen the bureaucracy. Make source-linked reasons choose whether the keys answer; allow simple onset count to guide HOW densely, not WHETHER the reply is warranted.

Distinguish HARD admissibility (voice capacity, source identity, stage, final harmony, intentional silence and hearing chronology) from SOFT texture allowance (optional extra attacks). Do not replace a scalar with a seven-dimensional optimizer pretending to measure human quality.

### C6 — We already have a negative-control proof that maximal coupling hurts

performance.rs::EnsembleCoupling says Round VIII CoupledR8 was rejected by ear: joint harmonic coercion revoiced roughly three quarters of the pad, reduced common tones and increased movement. The original R7b Independent baseline was preferred; Surgical was retained only for locally verified hard defects. This is crucial precedent: a five-musician global consistency solver risks destroying independent personality.

Current shared backing_energy_at(t) applied to every non-source role is another form of over-coupling. PR #102 reported raw per-role envelope correlations around r=0.81–0.89 and zero all-band silent Score bins. These reported measurements were not independently rerun here. A good band need not maximize zero-lag correlation. Different role energies may be positively, negatively or delayed-correlated because one player hands space to another.

### C7 — DSP may obscure actual contrasts: competing hypothesis, not first excuse

synth.rs uses saturation, reverb/space, common bus compression and limiting. PR #102 reported roughly 4dB dynamic-range difference normal vs reference production in its controlled arm. One can alter a correct source and hear it differently through effects. Compare the EXACT SAME Score under normal, no compression and dry/harmonic reference with matched gain. Do not change synthesis defaults to compensate for a source/plan defect.

### C8 — Our handmade teacher is the SIMPLICITY benchmark

examples/rick_probe.rs directly authors a 56-bar Score with discrete VERSE, HOOK, BRIDGE, fill and outro decisions. It writes specific bass arpeggios/approaches, keys guide-tone shells/offbeat stabs, drum ghost notes/fills and phrase dynamics by ROLE. It does not run five autonomous general agents or solve an NSE field. This is the most practical clue: finite local musician policies, jointly constrained by a shared song, can achieve this level. The seven accepted teacher_golden7 media files remain unavailable here; never present a reconstruction as the same golden file.

## 2. What our categorical and pocket conversations demand

The WRONG effective object is a collection of locally lawful Note lists with a single gain function and enough retrospective repairs to make the test dashboard green.

The RIGHT object is a BOUNDED, TIME-ORDERED MUSICAL PROCESS:

    S(t) = immutable song source, theme identity, final chords, pulse,
           named open debts and legitimate silence.
    x_i(t) = player-local duty, recent motif, breath/recovery, occupied
             monophonic/polyphonic capacity, anticipated harmonic target.
    m_{j→i}(t) = a specific causal, source-linked call/response/withhold
                 with prior performed evidence or explicit future source cue.
    x_i(t+Δ) = Φ_i(x_i(t), S(t), admissible prior messages).
    committed_i = Π_i(x_i,S)  // immutable observed performance events

Every musician projects from the SAME source and metric base, yet retains private state. Partial composition of role streams is allowed only when time, harmony, role capacity, source identity and silence obligations match. The joint object is NOT a diagonal forcing equal instantaneous phase, energy, attack count, melodic shape or voice leading. If the diagrams don't correspond to concrete maps and mutations, don't claim category-theoretic proof.

Pocket specifically: common pulse phase φ(t), musician relative phase δ_i(t)=wrap(θ_i(t)-φ(t)), intentional bounded phase offsets and delayed response. Stable RELATIONS to pulse and other players do not require identical notes or timing. Do not turn this into independent random phase jitter. Preservation of note order, source-relevant inter-onset contour and musician-specific push/layback are higher priority than decorative microtiming.

The Navier–Stokes/filtering analogy reveals a sufficient-statistic issue, NOT an instruction to solve PDEs: global current energy+chord+tempo is NOT enough to determine each musician's next gesture. Two histories with identical global aggregates but different lead source calls may justify DIFFERENT keys replies. Current lead-note-count Listen cannot distinguish such histories. That is a cheap distinguishing experiment. Use the same lesson from Observational SUCC: a coarse observation or receipt is not the complete causal source. Keep provenance, hidden local memory and reporting separate.

## 3. The smallest viable architecture, not a fourth engine

**Principal architecture test:** first draw the complete current dependency DAG and MUTATION ledger. For each fact (harmonic source, timing, actual event, role duty, silence, expected relation, dynamics, final hearing) name ONE authoritative owner. Mark every later writer and every already-frozen consumer.

Try to create an optional C137 performance kernel through existing SongMap, CompiledArgument, PerformancePlan, InteractionMaterial and Score:

1. Finalize/freeze ONE source/harmonic timeline and argument obligations BEFORE ActionPlan, interactions, accent, ensemble modes, source capacities and budget.
2. Derive ONE sequence of small timed role-duty episodes. Use existing Stage and Interaction types as the source-bound cue language; add only missing duty/state fields at the narrowest seam.
3. Commit the actual bass/keys/lead/drum decisions in a finite CAUSAL ORDER. A drummer can anticipate a declared future chord; it cannot claim to have responded to a keys improvisation which was generated after drums. If two-way response is essential, bounded propose/acknowledge and deterministic revise; no circular unlimited rehearsal.
4. Only after all dependent intents are settled, materialize immutable final notes/kit and voice continuity. Preserve the current generator and synthesizer as much as possible.
5. Build an observer that reads FINALLY PERFORMED relevant features, including dynamics if a musician depended on them. Rejection is typed; no source repair by lying, clipping or doubling after hearing.
6. Delete or retire redundant opt-in post-processing branches as the kernel takes over responsibility, keeping old adapters and golden regressions intact.

Conceptual internal contracts, NOT mandatory new public structs:
- FrozenSongContext — final source, chord, pulse, hard silences, valid capacities.
- TimedPlayerDuty — source-bearing call/answer/comp/floor/listen with role and occupancy.
- CommittedPerformance — immutable timed events plus effective hearing edges.

If existing types express these, USE THEM. Two tested types and fewer writers beat a new hierarchy of 15 types. New generative content still uses the existing ThemeFamily and MusicalArgument; this is a PERFORMANCE rewrite, not a different composer.

## 4. Red tests before optimizing

Test A — hearing blind spot: construct two notes identical in (time,duration,pitch,function), differing only velocity; legacy HeardNote collapses them. New opt-in dynamic witness must notice when dependent strategy used velocity. No legacy API break.

Test B — late counterpoint: on current --counterpoint ON (BASS_WEAVE=false), inspect Score::stale_hearings. If final keys notes differ from drummer's consumed notes, produce a failing causal receipt. New path must have ZERO stale required hearings after final source/counter commit.

Test C — harmony authority: alter a feasible source chord while keeping seed/form fixed; compare all pre- versus post-argument planning fields that depend on harmony. In new kernel, final chord commits BEFORE consumers; context/hazards/readouts consistent.

Test D — bass capacity: provoke mono overlaps, too-close equal-pitch retriggers, low-register melodic blur, lost foundation on next downbeat and unauthorized deletions of source carrier notes. Correct refusal/choosing foundation is better than a nominally feasible but muddy edit. Never re-enable the held bass weave to fake a positive test.

Test E — semantic response: lead emits two different source-related calls with identical onset count and global energy. Keys must distinguish them if the source contract warrants different answers; irrelevant metadata must change NOTHING. A keys Listen must sometimes be the true best choice.

Test F — causally warranted kit: remove one legitimate keys answer and recompute. Drums' specifically dependent later response must change OR be refused; unrelated rhythmic anchors remain stable. No false retroactive Hearing.

Test G — pocket: vary permitted phase offsets/delay preserving source contour and drum beat; forbid off-grid arbitrary timing or phase drift. Allow coupled lagged interactions, not forced zero-lag.

Test H — simplify vs bureaucratize: every new acceptance predicate must reject an independent hostile mutation old rules let through; don't add a feature that only reproduces an old receipt.

Test I — full negative controls: coupled R8, uniform energy fader, random independent energy, note-count-padded busy ensemble, compulsive keys counter on every lead rest, bass low-frequency counter; preserve teacher/r17/covers and old PR #102 held-bass sound.

Test J — acoustic: Score-stable NORMAL vs nocomp/dry/reference, role stems, full mix, masker/limiter comparison, named source events and local silences. PCM quietness is distinct from Score lack of written notes due release/reverb tails.

## 5. Do actual work in short, reversible rounds

**Round 0 / forensic debt map.** Refresh source and test CI. Draw current owner/writer/reader DAG and identify every post-commit mutation, harmonic overwrite and duplicate gate. Run baseline production/teacher-golden control where media exists. Add red-first Rust tests for Hearing-velocity and counterpoint/drummer stale hearing. Keep exact SHAs and artifacts.

**Round 1 / one complete 4-bar band episode.** Choose one short musical story: lead states a recognizable theme, breaths; keys chooses ANSWER or LISTEN based on actual source and duty; bass maintains meaningful floor or executes a brief licensed approach, preserving mono voice; drums makes an explicitly causal choice after keys answer or a planned withheld event, ending on prepared harmony. Players own their own decisions. No post hoc bass/keys note list surgery. Must render, be groove-correct, and defeat old shallow controls.

**Round 2 / source and final-hearing unification.** Precommit source harmony and optional drive before budgets/actions, bring necessary counterpoint source proposals before dependent hearings, replace shared final-stage gain sweep with optional role-specific authored/duty dynamics determined upstream; add actual final-effective dynamic Hearing. Keep legacy paths byte-identical and BASS_WEAVE=false.

**Round 3 / generalize as finite transducers.** Extend small role rules across verse/hook/bridge/return; role-specific memory/recovery, meaningful silence, anticipatory goal with shared pulse, restricted cue graph and stable local voice ownership. Resist inventing separate conductor/musicology engine.

**Round 4 / subtract features.** A/B the new kernel against old for physical source defects, low-end artifacts, per-role phrasing and output; delete redundant branch-local wrappers if superseded. The source dependency graph should have FEWER authoritative writers and source-altering postpasses than before. Report code LOC/owner DAG complexity and actual sustained regression evidence, not subjective declarations of simplicity.

**Round 5 / listening and hardening.** Actual seven accepted golden WAVs only if accessible, else reconstructed teacher labeled distinctly; current PR #102 bass-held render, new agent-like part-writing, and intentionally defective controls with matched sound/tempo/mix. User listens to stems/mix; ear ACCEPT/REJECT/PENDING. Full fmt/clippy-deny/rustdoc-deny, test suite, real render, source/voice/rehearsal gates, historical PCM fingerprint/R17/cover regressions, exact-head CI and isolated draft PR. No main merge/release/tag without owner approval.

Fan-out: separate scouts (1) causal DAG/Hearing, (2) bass mono+spectral problem, (3) source/harmony authority refactor design, (4) teacher/neutral PCM and package, (5) adversarial categorical/pocket tests. One principal writes and integrates the core; exclusive worktrees, commit provenance, no racing changes to functor.rs/performance.rs/argument.rs/comp.rs.

## 6. Do not mistake proof for art

**Source-verified:** C1–C6 structural facts, R8 negative-control historical choice, teacher direct-part authorship, production chain, beta bass weaving disabled. **PR/user reported, not independently played here:** bass stutter and mud, r=0.81–0.89 role-energy correlations, ~4 dB production difference, teacher_golden7 superiority. **Still hypotheses:** how much of missing quality is from phase vs role intent vs register vs mixing, which kernel is minimal in actual Rust, whether dynamic Hearing alone will make audible improvement.

A true band is not five copies of the conductor's global scalar and not five independently noisy processes. It is a shared musical truth realized through locally different, history-respecting musicians whose cues and silences are causally connected. The teacher proves a relatively simple Score can embody that. The lesson is to make our generative decisions as clear as that Score, not to make our theory taller than the sound.
