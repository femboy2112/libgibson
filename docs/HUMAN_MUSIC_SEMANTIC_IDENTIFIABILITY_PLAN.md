# HumanMusic: semantic identifiability and Rick-C137 composition upgrade

> **2026-10-09, RESEARCH / IMPLEMENTATION HANDOFF — NOT A CODED SOLUTION, NOT RELEASE-READY.**
> Base for this independent documentation branch: C137 feature head \`fbdbffcee3f77dab3cef0234666192cadc7da370\` (PR #87, draft). The head is **moving**. Refresh all refs, CI, tests, and accepted listening feedback before coding.
>
> **Maintainer verdict:** all accumulated changes genuinely improve musical coherence, but the song is still *speaking a coherent language whose meaning the listener cannot grasp*. The unresolved obstruction feels upstream, near seed/semantic identity, not merely instrument playback. Take that perceptual report seriously; do not mistake formal \`MeaningPlan\` commutation for intelligible music.
>
> Companion: [the earlier source-preservation audit](https://github.com/femboy2112/libgibson/pull/93) (\`research/c137-quality-transfer/README.md\`, Python probes). That audit is necessary but not sufficient: lossless transfer of a musically ambiguous source cannot manufacture a meaningful composition.
>
> **Boundary:** this is a program to improve **musically audible relational meaning** (themes, recurrence, replies, expectations, denials, transformation, closure), **not** a promise that instrumental music transmits unique literal narratives, words, or identical emotional interpretations to every listener. Human listening is the acceptance gate; tests certify necessary mechanisms, not aesthetic success.

## I. Source-level root diagnosis: the meaning map has a large kernel

### 1. The incoming semantic type names presentation state, not a musical argument

\`semantic.rs\` defines \`SemanticState {tone, emphasis, density, elevation}\` and events such as \`FocusAcquired\`, \`ModalEntered\`, \`Impact\`, \`Confirmation\`. That is valid as **UI/story-affect control**. The trace does NOT identify a thesis's particular musical referent, question, distinct actors, a relationship that changes, or what specific earlier gesture a later answer resolves. The technical problem is NOT that the enum needs still more emotions; those four dimensions are mostly *style and energy*.

\`intent.rs\` maps into \`MusicIntent { energy, tension, density, register, function, motif: (id, depth, transpose), expectation }\`; the important ongoing developmental state consists of small counters/scalars, not a source-linked musical question with a discriminating future answer. The underlying system is much richer farther downstream, but it is not being given a sufficiently specific *piece-level thing to communicate*.

### 2. The explicit F(trace) meaning law is low-rank in the semantic input

**Direct reading of \`meaning.rs\`, not a listening verdict:** for a FIXED \`CompositionPlan\`, \`MeaningPlan::target(trace, plan)\` invokes \`story_arc(trace)\`, which extracts ONLY \`(Level arc from maximum pressure, Close from whether that peak is later confirmed/resolved)\`. It then walks **lead seats / prechosen discourse roles and harmonic backbone slots** to emit \`MeaningEvent\` labels (\`Learn\`, \`Reinforce\`, \`Payoff\`, \`Answer\`, \`Develop\`, \`Prepare\`, \`Miss\`, \`Open\`, \`Reset\`). It does not read the individual semantic-event identities/times/referents when constructing those events, except through the fixed plan and that pair.

Hence the following finite result follows by inspection:

\`\`\`text
For fixed form/plan P, define A(t) = story_arc(t) = (arc(t), resolution(t)).
If A(t1) = A(t2), then MeaningPlan::target(t1, P) = MeaningPlan::target(t2, P).
\`\`\`

The induced map factors through the coarse quotient \`A\`: \`F_P = f_P ◦ A\`. Its kernel includes all pairs of materially different traces sharing this coarse pair. **This does NOT prove entire compositions are identical**: the earlier \`IntentTimeline\` may still produce different forms/arrangements/performances when the trace varies. It proves the supposed *meaning certification target* for the fixed plan cannot distinguish them. Build a Rust test with that exact scope and a negative mutation: two distinct fixed-plan story traces with identical peak/release but opposed intermediate developments must produce the same \`MeaningPlan\` today.

### 3. μ(song) is a symbolic signature, not an independent ear or full musical argument

\`MeaningPlan::observe\` reads site motifs and chart gestures and bins them into \`MeaningKind\`: thesis reach Low/Mid/High, relation Literal/Variant/Derived/Foreign, landing Home/Open, chord-family/pull classes, etc. It does not analyze generated PCM, true per-note phrase articulation or what a listener actually recognizes. \`Commutation::check\` compares these coarse classes at fixed site indices.

Two songs with very different melodies, harmonic paths within a family, phrase pacing or expressive question-answer structure may pass the same categorical labels; \`μ(song)=F(trace)\` is **true only in the deliberately limited event vocabulary**. Do not call it proof of intelligible meaning. Preserve the original commutation law as a useful **coarse control** and add a strictly stronger optional semantic/realization contract that can reject its false equivalences.

### 4. The song's “thesis” is not yet a bound musical referent

\`discourse.rs::MusicalThesis\` holds home energy/tension/register/density, established phrase and a descriptive copy of anchors — no concrete MotifId / interval-rhythm gesture / specific answer target. \`Obligation\` remembers \`kind\`, source phrase, deadline, strength, settlement but not necessarily the source's actual motif/material referent. In \`discourse.rs::discharges\`, \`MotifQuestion\` accepts an \`ActionKind::Answer\` in the settling phrase (or a related generic Call/Answer) without checking that answer's **specific source-derived musical material answers the original question**. This is correct as a coarse action-family witness but not sufficient as a semantic discharge proof. A specifically unrelated answer should be a red control even when the ledger's current generic kind says Paid.

\`DiscoursePlan\` already implements useful long-horizon roles/goals/ledger and even a scrambled-role negative control. KEEP these; fix the missing **referent binding** rather than writing yet another disconnected discourse layer.

### 5. The composer searches for lawfulness and short-term fit, not a communicative melodic thesis

\`composer.rs\`: the meaning-directed theme grammar spans 2 pickup × 2 rhythmic cell × 3 launches × 2 sequences × 3 reaches = **72 schematic pattern parameters**; it chooses among lawful/typical motifs and chart-tone fit, then uses seeded tie-breaking. The chart is chosen before the motif. \`CompositionalPrior::HOOKY_FUSION\` is a small stylistic prior, not a full plot/meaning model. The hard semantic law only checks the coarse \`MeaningPlan\` events.

\`schedule\` sets \`bank.identity = thesis\` and **\`bank.hook = thesis\`**, assigning the thesis at Payoff, and answer only at Answer. Distinct verse/hook/bridge relations are underrepresented; rhetorical roles often name predeclared structures rather than constrain transformations of a particular musical proposition.

\`MotifBank::generate\` on the legacy path largely chooses seed_a/seed_b by major-mode brightness and a small transpose; it is not an intentional semantic seed. \`ThemeSeed\` improves supplying recognizable DNA but still carries **only thesis/answer Motifs**, not an executable musical argument, compatible section chart, phrase-level timings, nor what exact relationship the final version must demonstrate.

### 6. The “narrative” performer often recasts labels, not a complete idea

\`narrative.rs\` maps observed classes to fixed role assignments (\`Reinforce→Keys\`, \`Develop→Bass\`, \`Payoff→Keys+Bass\`, etc.), rather than originating a concrete **who-is-answering-which-musical-question** identity. At the prior source audit, \`interaction.rs\` allowed a single discretionary response and 4-event tail \`Quote\`, and \`bass.rs\` changed pitches over existing backing rhythm. These lose material and do not guarantee full per-carrier thematic fulfillment. PR #93 contains executable counterexamples. **A musical argument cannot survive a transport that changes what was actually said.**

The user is reporting that accumulated work sounds better; therefore **do not revert these real performance improvements or blame FM/DSP as the root problem**. Repair upstream content and conditional, source-faithful lowering.

## II. Proposed minimum new semantic object: one musical argument

Create ONE optional, versionable **\`MusicalPremise\` / \`MusicalArgument\`** (final naming should fit existing types, not proliferate ontologies) as a source-level interface separate from UI \`SemanticState\`. It should express *audible, conditional commitments*, not literary metadata:

- Stable **musical referents** (source motifs/gestures, their declared interval-rhythm identity and realization licenses; one may stand for “speaker A”, but the identity is musical).
- A **question/expectation**: what recognizable motif/harmonic/groove event has been stated, what unresolved aspect remains, what event would count as an actual reply.
- A few **typed transformations** over those referents: statement, repetition, variation/sequence, inversion with limits, fragmentation with preserved head, contradiction/denial, delayed answer, reconciliation/earned return, purposeful silence. Reuse existing \`Motif\`, \`MaterialEvent\`, \`MaterialSource\`, \`DiscourseRole\`, \`ObligationLedger\`, \`InteractionMaterial\`; do NOT duplicate them as a new generic language.
- A **section/phrase order and obligation dependency graph**, with source IDs, deadlines and explicit permitted partial/unresolved outcomes. Caller declares story objectives; composer chooses HOW to satisfy them. Requirements must have actual witnessable consequences in SongMap and Score.
- **Expressive parameters** (tone/energy/density/elevation/world) are separate modulation/style controls: they color and shape a proposition but do not supply the missing proposition itself.

Start with a tiny **3-program calibration suite**, not infinite freeform text-to-music:
1. **Call → earned answer**: identifiable question, delayed specific response, return; one strong motif.
2. **Promise → denied arrival → transformed return**: establish a reference arrival before denying it; the end acknowledges the same source under licensed transformation.
3. **Conflict → reconciliation**: two distinct, independently recognizable germs exchange roles; the final phrase combines them using an audibly specific relationship, not mere simultaneity.

Critically: choose/match their **coarse pressure and final resolution classes** so the old \`MeaningPlan::target\` cannot trivially discriminate them. These are controlled musical hypotheses, not universally guaranteed emotions or story labels.

The user must be able to hear at least what “question/answer”, “denial” or “reconciliation” mean IN THE MUSICAL RELATIONS, without seeing a file name. An instrumented experiment can determine if that is achieved; the new model cannot assume it.

## III. Categorical architecture: specify the quotient, then prove separation on selected contrasts

Existing arrows:

\`\`\`text
UI SemanticTrace T ------> IntentTimeline --> CompositionPlan
       |                                       |
       | F (coarse)                            |
       v                                       v
 MeaningPlan  <------- μ(SongMap) <------ SongMap
                                                |
                                  PerformancePlan -> Score -> PCM
\`\`\`

Refined opt-in program (coarse path remains a control):

\`\`\`text
  SemanticTrace (affect, timing)    MusicalArgument (actual referents/commitments)
                  \                      /
                    CompositionBrief (validated)
                              |
                   Discourse+obligation graph
                              |
            concrete motif phrases + song form + harmonic goals
                              |
                       SongMap source IR
                              |
             required material/carrier assignments
                              |
              PerformancePlan + voice realization
                              |
                   Score -> PCM -> audited listening
\`\`\`

The correct target is not global injectivity: many valid musical realizations of the same argument should remain in one intentional equivalence class. Define a **small declared contrast set \`D\`** (e.g. answered vs unanswered, same question vs unrelated answer, repeated vs transformed return), a family of **source-level musical observables \`O\`**, and a partial inverse/witness \`R\`. Require an experimental **separation law**:

\`\`\`text
For each predeclared contrasting pair (a,b) in D,
  if a ≉ b by their musical commitments,
  the corresponding scored witnesses must differ in at least one
  load-bearing dimension: O(compose(a)) != O(compose(b)).
\`\`\`

This is **restricted faithfulness / identifiability over declared contrasts**, not a category-theoretic proof that all sound has unique meaning. Declare the equivalences and observational resolution **before testing**. Legacy \`μ=F\` remains useful as coarse relative correctness; the stronger new \`ArgumentReceipt\` reads actual source/material events, harmonic relations, instrument carriage and consequent/answer identity WITHOUT trusting the composer’s own declaration. An **independent auditory observer** and blind human listening probe ask if distinctions are exposed and perceived after PCM.

The diagram should commute AND be discriminating on the test's chosen claims. A merely commuting diagram with a very large kernel is inadequate for this use case.

**Composition law:** local phrases can glue into a whole only when their boundary source IDs, unresolved obligations, expectation states and harmonic/motif relations agree; a shuffle/mutation can be locally valid and globally false. Validate and test associativity/source identity of the specific implemented gluing operations where applicable; don't cite category theory to conceal an unimplemented composition law.

## IV. Implementable staged research-to-code program

**S0 / 0-day evidence:** current exact head, existing tests/CI, real listening references and prior PR #93. Add non-default, reproducible **counterfactual meaning-collision probes**: under a fixed CompositionPlan two different traces with equal peak/release have identical \`MeaningPlan::target\`; with distinct generated phrases, coarse μ equality does not guarantee source-equivalent musical gestures; an unrelated Answer action can satisfy generic MotifQuestion's action-kind discharge. Include normal positive controls and adversarial negative controls. Do not silently strengthen historic 0.4 behavior.

**S1 / bridge from PR #93:** ensure true timed phrase events (onset/gate/accent/pitch basis/span) survive source-level lowering into the existing Score/PCM path; preserve the user's accepted handcrafted reference and source IDs. If the source cannot survive the carrier transport, do not claim new semantic goals survive it.

**S2 / smallest meaningful vertical slice:** implement one optional \`MusicalArgument\` with one concrete question ID, themed source motif, an answer required to be a licensed transformation of THAT motif, and a mapped form/harmonic plan. Compose a 24–48-bar generative example end-to-end (not hardcoded final notes), existing runtime unchanged by default. Prepare note/role/phrase charts and independent source receipts.

**S3 / genuine score-level intentionality:** make motif seed and its transformations depend on the argument; joint enough motif/chart/form planning that a phrase survives its destination harmony; distinct memorable hook vs verse/answer when required; long-range recurrence and structurally earned departures/arrivals; space and accompaniment explicitly planned around the carrying identity. Reuse the existing meaning/discourse/obligation machinery, replacing coarse proof points with referent-bound gates instead of a brand-new private composer. Keep discretionary fills distinct from required arguments.

**S4 / counterfactual listening suite:** three musical arguments, matched world/tempo/loudness/seed families and lengths, with 2–3 fresh holdout seeds each. Generate altered arguments with identical overall emotional envelope; blind label-free AB/forced-choice listening packets (genuine and mutated, randomized IDs); mutated after-the-fact score narration must fail appropriate receipt; unfamiliar listeners optional. **Human verdict** may ACCEPT / REJECT / AMBIGUOUS and a single maintainer listen cannot certify universal decoding. No new magical numerical \`human_quality\` scalar.

**S5 / compatibility and integration:** run v0.4 exact PCM/fingerprint/R17/cover regression controls, full fmt/clippy/rustdoc/tests/CI, bound CPU/memory, no new mandatory audio/device/sample dependencies; make new composition API opt-in and auditable; show 1.0-era API/ABI/ownership considerations for the eventual language-parity roadmap but **do not** prematurely bump the C ABI. Leave MeatsackProjection and GSPU-FM under their independent research holds.

**S6 / release target:** after C137 music is ear-accepted and the v0.5 visual integration head exists, use the source-true meaningful composer to score the upgraded Filthy Frank meme-intro finale required by draft PR #92. That movie must demonstrate actual new features and the audible score, not a separately imposed soundtrack that happens to change loudness at cuts. No release/tag without maintainer authorization.

## V. Fresh falsifiers BEFORE optimizing

At least the following hostile controls must be implemented and produce specific failures:

- **Same coarse F, opposed premise**: exact \`MeaningPlan\` equivalence is established, stronger optional argument receipt distinguishes them.
- **Same emotional trace, changed proposition**: change ONLY the question/answer relationship; keep tempo/production/dynamic trajectory fixed; source and performance should vary in identifiable events.
- **Wrong answer**: same nominal \`Answer\` action at the same time, but use material unrelated to the opened question; generic action-kind checks may pass, referent-bound check MUST fail.
- **Moved answer**: same notes in the wrong place (before the question or after the deadline) must fail despite identical local note legality.
- **Quoted wrong germ**: a generic chord-tone or modulo-index imitation must not count as source identity; verify declared contour/rhythm/time relationships.
- **Intentionally unresolved**: not every open debt is a defect; check explicit legitimate terminal unresolved contract vs accidentally abandoned obligation.
- **Story swap and null perturbation**: changing a real musical argument should change the corresponding source/content; changing irrelevant formatting metadata should NOT alter source IDs, budget, stochastic streams or PCM.
- **Source preserving world/transposition**: intentional realizations preserve typed interval/relative rhythmic identity where licensed, while timbre changes are free. Don't demand waveform identity from different instruments.
- **True multi-carrier reprise**: keys/bass/lead each satisfy their named obligation even with same-count accompaniment replacement and lead silence; do not substitute one player's surplus for another.
- **Ablation**: compare content-bearing motif lines vs backing/groove-only under same world, ensuring recognizability does not come only from production.
- **Humans disagree**: maintain honest subjective outcomes, no fabricated semantic certainty.

## VI. Engineering, fan-out and future handoff

When a user authorizes implementation in ChatGPT Work, use **a NEW non-main implementation branch forked from current accepted C137 work**, not this docs-only research parent; inspect the exact GitHub refs and source first. Read both #93 and this design. Parallelize only independent workstreams with isolated worktrees and no file collisions:

1. **Semantic/quotient adversary**: minimal invariant theorem, red test cases, independent source-witness verification, specifically try to fool μ and generic action discharge.
2. **Phrase transport**: lossless audio source/material timing from #93, proof against generated and teacher phrase comparison; guarded by existing PCM and sample-clock contracts.
3. **Argument compiler**: choose one small typed \`MusicalArgument\`, source identity and referent-bound obligations, test end-to-end generator under one archetype.
4. **Independent listening/holdouts**: fresh seeds/semantic counterfactuals, matched WAV+stems and user listening packet, provenance and run commands.
5. **Release/integration**: existing branch ancestry, main/ABI/R17/covers/CI, roadmap #92/#90/#91 boundaries; no accidental cross-feature integrations.

Principal integration engineer owns the single new branch; merge only reviewed children after validating their claims. Source commit checks, exact-head CI, per-test result and changed file manifest mandatory. If a route fails, record the falsifier and repair the smallest actual cause. Do not claim the full Rick-C137 ear verdict until the maintainer actually listens.

**The necessary conceptual advance:** a musical statement is not the sequence of adjectives describing its mood. It is the concrete thematic/harmonic/temporal relationship the piece introduces, develops, obligates, contradicts and ultimately answers—or deliberately leaves open. The story is what survives through these causally linked transformations, not the fact that all locally grammatical transformations happened in a plausible order.
