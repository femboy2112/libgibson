# The Homosapians — Rehearsal-Driven Composition
## Seed -> guitar riff -> bass response -> drum response -> guitar revision -> song

**October 16, 2026. Research bootstrap, not a finished rehearsal composer or a new listening verdict.**

**Audited HEAD**: femboy2112/libgibson, PR #121, impl/homosapians-real-band-2026-10-16 at 56f2391ec0d1324878918404fb76ea56143c8375; exact-head CI 38963564645 successful. It stacks on research PR #120 at 3cf10b0f0083063a2e194f4d0312168d3d9168b7, itself on pop PR #118. The accepted C137 51-artifact PCM/Score corpus, legacy covers/R17/Jerry and historical controls are NOT to be changed. The branch containing this note is a separate research successor to #121, not main. No main/default/release/ABI changes are authorized.

Read the actual latest code in examples/c137_song/source_pop.rs, src/audio/human_music/{song.rs,composer.rs,argument.rs,argument_transport.rs,argument_witness.rs,kernel.rs}, and docs/research/HOMOSAPIANS_EVIDENCE_LEDGER.md, as well as the source-independent gap tests committed beside this plan.

## 1. Preserve the real progress

- R0 confirmed the exact Score-level G natural Lead against E7/Pad G sharp color, the separate melodic-vs-harmonic closure, and the R12 bass D below E turnaround. A color is not intrinsically an error; don't blindly snap notes.
- R1 created a distinct 8-bar backing with verse/chorus arrangement. R2 changed the keyboard from a single-line pretend lead to chordal comp; owner heard a large improvement. R3 added a soft pad bed and a two-chord verse camp versus brighter chorus reharmonization; owner reported further improvement. R4 slowed the harmonic ground relative to the surface, repeated both verses and choruses and expanded to 16 bars. This is genuine progress.
- FM/GSPU and MeatsackProjection remain separately researched physical-rendering directions. Richer physical strum/ringing and shared band timbre are useful, but fixing the songwriting process does not have to await fully implemented physical instruments.
- Last reported R4 16-bar Circle/Lift render and 51/51 accepted C137 lock remained green; the research tests and roadmap added here have separate CI gates.

## 2. Current construction does not yet resemble a band composing together

**Exact source-level findings, scoped to #121:**

1. SongMap was designed to be constructed before performers; its stated law has performers projecting a known, complete song. composer.rs searches theme/chart candidates, a useful existing candidate-selection substrate, but it also chooses finished song-level material rather than running iterative negotiations between different musicians. The pop-band example builds a SongMap, then explicitly overrides its referents, 16-bar route and bass cells with fixed arrays.
2. The seed argument in source_homosapians_backing does NOT choose the authored musical content: backing_lead and homosapians_backing_bass are fixed per flavor, and the chord route is built from fixed verse_camp4 and chorus_conjugate4. Multiple seeds can affect other SongMap state, but the specific riff/reference/bass cells are unchanged. This is a hand-authored working demo, not a proven riff generator.
3. The drummer's pop-funk algorithm locks kick contacts onto predeclared Root and end-push events from BassDegree cells, not the musical contour or timbral accent decisions of an independently chosen guitarist's riff. The interlock is real, but the guitar -> bass -> drums -> guitar feedback cycle is absent.
4. The "band-only" score contains a quietly performed Lead carrier around velocity 0.06. This exists to keep polyphonic Keys outside strict monophonic required-source carrier windows. It is not genuinely zero-lead, and quiet Lead attacks can still influence the keys room-availability gate and lead occupancy decisions. This is a type/ownership problem, not primarily a gain problem.
5. Four separate 16-beat sections are all ArgumentRelation::Establish, with literal verse-source repetition and literal chorus-source repetition, rather than a source-witnessed musician-generated consequent/transition/return.
6. The current pop full-song backing uses PopSourceGrammar.loop_bars equal to 16, exactly the route length. The pop_route_law test of route[i] == route[i modulo 16] therefore tests nothing beyond the fact that a chord list was supplied. It does NOT certify a valid two-chord vamp, meaningful sectional conjugation or musical resolution; Claude's ledger explicitly calls it a loophole.
7. No guitarist authoring agent or guitar physical rig yet exists in Score::Role (Pad, Keys, Bass, Lead). Introduce guitar as a pre-source composing role/rig sidecar, not a breaking global Role enum replacement until tests justify it.

The characterization suite tests/audio_homosapians_rehearsal_gap.rs pins these precise current facts, so a future implementation can replace them deliberately. These are NOT aspirational passing requirements for the new composer.

## 3. Two clocks: the key mathematical abstraction

**Rehearsal revision time k and performed music time t are DIFFERENT causal systems.**

Music-time t: the accepted take is immutable, the drummer cannot have heard a future performed note, and a performer must respect existing source/Score/observation laws.

Rehearsal-time k: the guitarist can audition a trial, hear the bassist and drummer's proposed changes, and REVISE an earlier bar in a new draft. That is not retrocausality: the performance from rehearsal revision k remains a frozen historical trial, while the new candidate at k+1 has a different source.

    RehearsalDraft[k] = (Seed, style, motif bank, active riff,
                         chart hypotheses, bass/drum responses,
                         physical rig constraints, outstanding musical debts,
                         trial Score/PCM receipts, revision history)

    Proposal : RehearsalDraft[k] ⇀ RehearsalDraft[k+1]

Each proposal declares author_role, reads_revision, read_source_ids, changed_part_ids, licensed transformations, musician reason, and measurable candidate/trial consequences. An obsolete proposal is refused. A rejected suggestion does not contaminate the accepted draft. All auditions are immutable reference takes.

When a draft has a coherent band-only riff/transition, the composing engine FREEZES it into the existing SongMap/MusicalArgument/PerformancePlan/Score, under strict source and causal checks. Never unlock previously committed performances to make a new composition appear legal.

**The critical new back-edge is guitar -> bass -> drums -> GUITAR REVISES.** A one-time feed-forward sequence produces a predetermined arrangement, not a rehearsal.

## 4. Minimum viable executable jam

**Stage A — a seed, not a finished song.** Supply a key/mode policy, tempo pocket, two-bar size, a rhythmic/melodic germ (e.g. a few pitch intervals and salient placements), rough stylistic direction, and some space for discovering the harmonization. Do not encode the entire prewritten 16-bar chord route in the seed.

**Stage B — guitarist proposes a riff.** Generate 4-12 feasible ORIGINAL 2-bar riffs with note/pause/accent and optionally strum/palm-mute/let-ring gestures. The user can choose a promising riff by ear; machine checks test bounded register, source identity, recognizability and potential for harmonic development, not universal good-music probability. Keep independent candidate fingerprints and receipts. Existing ThemeSeed/CompositionalPrior and theme_family logic may be reusable.

**Stage C — bassist responds to THAT riff.** Produce several legal bass candidates: synchronized root groove, anticipatory/octave riff, counterline, pedal or deliberate space. Each cites specific riff onset/held-note/source evidence and the current harmonic hypothesis. Equal-note-count riff mutations must sometimes induce different, reasoned bass decisions. Do not merely select BassDegree arrays by a seed RNG.

**Stage D — drummer responds to both.** Choose kick/backbeat/hats/ghosts/fills from actual committed rehearsal bass+guitar preview, preserving a steady pulse but not every exact accent. A change in bassist contour can alter later discretionary drum behavior. Distinguish written/known future during rehearsal from heard actual performance during the take.

**Stage E — guitarist responds AGAIN.** With bass+drums now present, propose an altered pickup, a one- or two-bar transition, a chord voicing/sustain change, or an intentional rest. Choose the transition because it emerges from how the whole band currently grooves, not merely because the global chord schedule says bar eight is a chorus. The guitarist must sometimes simplify the original riff. The band may reject a flashy idea if it destroys the organism.

**Stage F — form evolves.** Reuse/transform a chosen two-bar riff to make an original 8-bar mini-song, then grow to 16/32 bars after listening. Chorus/bridge are lawful transformations and answers of actual prior material; independent bar-local harmonic closure and transition conditions replace the current full-song loop-period loophole. Add a lead AFTER the band-only piece already carries a song.

**Stage G — freeze and perform.** Export auditable source, role proposals, chosen/rejected alternatives, band Score, solo/stem/full WAV, and listening contrasts. Reuse the existing warm palette at first to isolate *composing process* from FM/Meatsack/field production improvements.

## 5. Suggested narrow types and responsibilities

    RehearsalSeed { seed, meter, pocket, tonic_policy, style, germ }
    RiffCandidate { source_id, authored_2bar_events, implied_harmony,
                    physical_gestures, identity_receipt }
    DraftPart { author_role, events, source_dependencies, response_obligations }
    RehearsalDraft { revision, riff_candidates, selected_riff, parts,
                     harmonic_hypotheses, unfulfilled_duties }
    Proposal { author, read_revision, causal_inputs, changes, explanation }
    TrialTake { revision, source_hash, score, pcm_hash, measured_audibility }
    Decision { accept, reject, revise, evidence }
    FrozenSong { SongMap, MusicalArgument, source_receipts }

A real chordal support part must be distinct from a monophonic thematic carrier. Do not fabricate a 0.06 velocity Lead to free Keys for chords. Keep physical guitarist authoring role distinct from generic Score::Role on the initial isolated branch to avoid breaking existing exhaustive matches.

## 6. Falsification matrix

| Gate | Perturbation / negative control | Pass criterion |
| --- | --- | --- |
| G0 | Existing live Circle backing, seeds 2112 vs 7 | Characterize literal stable referents/route/bass; don't claim riff generativity prematurely |
| G1 | Riff A vs B, SAME harmony and count, altered rhythmic accents | A musician's reasoned bass response differs in at least one legal counterfactual; fixed bass control demonstrates what "deaf" sounds like |
| G2 | Bass A vs B over SAME riff | Drummer makes some dependent choices while fixed backbone remains; response cites bass content, not a generic timed preset |
| G3 | Two drummer options, same riff/bass | Guitarist's transition or withheld note can change after a later rehearsal revision with an explicit log |
| G4 | Revision k+1 changes bar 0 | Earlier audition Take_k remains immutable and all source IDs/receipts truthful; no silent post-commit edit |
| G5 | Band-only draft | A literal zero Lead Score with playable chordal support, not quiet singer scaffold |
| G6 | Section-local 2/4-bar vamps | Witnesses reconstruct repeated local law, distinct chorus transition, source debt, and honest melodic/harmonic completion; route.len == period cannot fake this |
| G7 | Different riffs with same note count and dynamic sum | Structural identity changes remain audible and linked to actual chosen source |
| G8 | Lead insertion | Upstream backing identity stays unchanged; later physical nonlinear group mix is permitted to respond |
| G9 | Held-out seeds, rhythm styles, instrument templates | Multiple lawful new riff identities and true causal role differences; no stock riff cloned across all seeds |
| G10 | Accepted C137, R17, covers and Jerry | Historical PCM unchanged, 51 hash check passes with new path OFF |
| G11 | Old warm mix vs any future FM/Meatsack/room changes | Musical improvement attributable to rehearsal method separately from timbre changes |

Choose candidates under hard constraints first, then source identity, meaningful alternatives, peer musical response, feasible instrumentation, and ear acceptance. No pseudo-psychophysical universal "meaning score" should be invented to make selections sound objective.

## 7. Implementation sequence and safe fan-out

**R0**: execute new characterization tests on latest source, reconcile #121 and freeze accepted C137. Log exact proof and run outputs.

**R1**: isolated RehearsalDraft / RehearsalSeed / two-bar riff-candidate search in a NEW opt-in module or research example. Determinism, distinct seeds/candidates, legal transformed musical source. No edits to old performer.

**R2**: riff-conditioned BassAgent proposals and negative controls; then DrumsAgent that consumes committed draft parts; test counterfactual cause, not just synchronized beats.

**R3**: Guitarist revision round on the band preview, accepted/rejected proposal log, lawful 2bar→8bar thematic transition and chorus alternative. Fix real accompaniment/source ownership when needed, without weakening witness.

**R4**: freeze into existing MusicalArgument and perform through accepted warm synth. 8-bar full mix and separate bass/kit/keys/guitar-ish stem controls, against hand-authored #121. Ear decides whether it has more band-writing intelligence.

**R5**: integrate physical strum/ring, multi-operator FM and ensemble-sonic-field work as SEPARATE rendering axes, preserving source invariants; 2x2 old/new composing process x old/new timbral realization.

One principal integrator owns kernel.rs and synth.rs; fork children into independent worktrees for Riff search, Bass/Drums response laws, adversarial source tests, and physical/acoustic audition. One heavy Cargo build lane on the constrained machine.

**Stop conditions:** historical PCM drift; empty mutation witness; source provenance forged; fake lead retained and described as "band only"; loop-length degeneracy passed off as a rigorous style law; changing already committed score during "rehearsal"; all seeds same riff; musician proposal ignores cited earlier riff/bass; no audible improvement under matched owner listening; multiple agents racing hot files.

### Primary-source inspiration (context, not a universal recipe)

In a Butch Vig interview about Foo Fighters' Wasting Light, rehearsals provided several drum options and the band even recut an over-arranged track to recover the raw collective groove: https://www.fooarchive.com/features/butchvig11.htm . Dream Theater's John Petrucci described the band jamming, collecting snippets, developing song sections and arranging them later: https://blabbermouth.net/news/dream-theater-members-discuss-songwriting-process-being-on-the-road . Both are real examples of draft revision and collective input; not proof guitar always starts.

**North star:** stop making five musicians read parts of a completed page. Make the page emerge from those musicians constructing, refusing and revising ideas together. Keep the final page as strictly witnessed and reproducible as ever.
