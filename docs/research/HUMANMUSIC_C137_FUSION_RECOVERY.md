# C137 Fusion Recovery — from a valid argument to a living, dense, completed song

> **Research / next-round handoff, 2026-10-09 local (2026-10-10 UTC).**
> Audit base **c6a69dc57e75cd4063adbeb813f0429141ea00ca**, branch \`integrate/humanmusic-semantic-resurrection\`, draft PR **#96**, on top of Sol’s implementation **37dcb964bc3451875c3adb7a475dc1e5d51bbad6** (PR **#95**), on top of C137 **fbdbffcee3f77dab3cef0234666192cadc7da370** (PR **#87**).
> **Status: DESIGN + SOURCE AUDIT ONLY. No Rust implementation, audio render, or ear acceptance was performed in this research branch.** Refresh moving refs, repository instructions, actual WAVs and CI before implementing.
>
> **Human verdict on #96:** better pocket, better vocal lead, more breathing band, but **still not the handcrafted “golden seven” C137 quality in coherence, density and semantic completion**. **Do not manufacture a claim of acceptance from tests.**
>
> **Critical scope:** retain Sol's referent-bound composition, complete timed source carriage, independent raw-Score witness and historical opt-in defaults. Repair the **actual musical content and ensemble architecture**. Don't divert to Meatsack, GSPU-FM, DOOM, v1.0 ABI or the meme-film edit; they remain future milestones.

## 1. Audited moving frontier

Sol #95 correctly adds \`MusicalArgument\`, \`MusicalReferent\`, source-linked \`ArgumentStep\`, typed rational/octet transforms, \`CompiledArgument\`, \`prepare_performance\`, \`replace_required_role\`, and \`observe_argument\` with independent timing/pitch/chord checks. Its baseline admits **A/B only**; conflict/reconciliation C is explicitly unsupported. The new Score witness is not a listening witness; historical \`Commutation::check\` and \`PerformanceReceipt\` do not fully accept the new opt-in route (A/B source-path coarse divergences reported as 5/7 at Sol's checkpoint). Preserve that honest boundary; do not forge old action/material IDs.

Claude #96 adds a valid pocket rescue (**removed harmful 3/2 time scaling**), keeps the lead on thematic developments, lengthens the calibration germ, and applies section dynamics to non-source Score velocities in the *audition example*. This is appreciated by ear but not yet a compositional cure. **PR #96's description says `lead_life ALIVE`, but the executed `examples/humanmusic_argument.rs` arm uses `development:true, spacing:false, dynamics:true`; log and test the effective values, and audition spacing separately instead of mislabeling it.** The PR's locally reported \`fmt/clippy/rustdoc\` and targeted tests are not the same as verified exact-HEAD hosted CI; GitHub run **38018403672** was in progress when inspected. Sol's head CI **38010747703** was green.

### 1.1 Source-count comparison: a structural chasm, NOT a psychoacoustic theorem

From the pinned source code (\`examples/rick_probe.rs\`, \`src/audio/human_music/argument.rs\`, \`examples/humanmusic_argument.rs\`):

| Dimension | Hand-authored \`rick_probe\` | Integrated new argument audition |
|---|---|---|
| Form | 56 bars, 224 beats, 138 BPM default | 128 beats, 108 BPM default |
| Thematic corpus | VERSE **24** notes, HOOK **25**, BRIDGE1 **22**, BRIDGE2 **22**, explicitly distinct | **one** ten-note \`MusicalReferent\`, peak varied slightly by seed, plus one-note optional completion |
| Statements | 12 genuine 4-bar verse/hook/bridge statements, plus outro note | six \`ArgumentStep\` placements across eight 4-bar phrases |
| Computed lead/source density | **291** lead notes in 224 beats ≈ **2.988/sec** at 138 BPM (4×VERSE + 6×HOOK + BRIDGE1 + BRIDGE2 + outro) | family A: **63** source notes in 128 beats ≈ **0.886/sec** at 108 BPM (6×10 + three completions); B has 64 unique source notes but fewer lead-carried statements |
| Harmonic construction | Recurrent per-bar sections including ii7–V7–Imaj7–V7/vi, IV–V–ii–i and A7→D-minor bridge pivots | essentially one explicit V7 / denied-vi / tonic chord per *whole argument relation*; \`prepare_performance\` pins the same chord to all spans under that relation |
| Accompaniment | \`bass_dense\`: 8 pitched approaches/arpeggio notes/bar, \`keys_dense\`: downbeat guide-tone shell + offbeat stabs, 16th hats & fills | default production players, but \`examples/humanmusic_argument.rs\` sets \`actions:false\` (no dynamic ActionPlan and **no planned interactive responses**) |
| Carrier reservation | complete 16-beat phrase, many authored pitched onsets and intentional small holes | 10-note source occupies **5 beats** (or 6 with completion) of each 16-beat phrase; the entire phrase window is reserved |

The ratio **2.988/0.886 ≈ 3.37** is **lead thematic event rate at the code-defined default tempos**, not a claim that doubling notes alone makes better music or that the full mix has the same ratio. Source event content has only one line vs four phrase functions in the teacher. The seven actual golden WAV filenames/hashes were **not independently located** in this audit: the integrator must pin them, not guess.

Under \`DeflectedLift\` the \`CoherenceContract\` sets \`phrase_bars=4\`. At 128 beats there are eight 16-beat phrases; \`MusicalArgument::compile\` places six steps at indices **0,1,2,4,5,7**, i.e. beats **0,16,32,64,80,112**. Four-bar semantic statements therefore have 10–11 beats of *unused melodic source time* after the first 5–6 beats. The 32–64 beat region includes a whole phrase with no required argument event.

### 1.2 Four code-level choke points

**D1: no genuine development.** The current A/B \`calibration\` uses one referent on every step. \`Develop\` is usually \`ArgumentTransform::default()\` (no pitch/rhythm variation), so the note source is the same as \`Question\` but the carrier and chord category change. After Claude removed 3/2 augmentation to restore pocket, **B's “transformed return” is also effectively a restatement plus terminal pitch**, not a developed phrase; update any stale prose that claims actual augmentation. \`same_head\` in \`argument.rs\` demands equality of every original source event modulo octave/time-scale for all source-related steps and is too rigid to represent a genuinely related but distinct VERSE→HOOK transformation. \`bank.hook\` is a three-note half-time fragment and is not separately scheduled as a taught hook. More layers cannot create a second independent musical phrase that this source schema refuses.

**D2: silence by over-reservation.** \`argument_transport::replace_required_role\` removes the role's entire backing from \`[phrase.start, phrase.end)\`, then inserts source events in its first 5–6 beats. This is mathematically consistent with a full-phrase source reservation, but it suppresses ordinary bass/keys service for the remaining ~10–11 beats, *even when the semantic intention was just an early statement and later accompaniment*. Compare to the teacher: it also suppresses incompatible backing when a role carries, but its **24–25-note melody occupies the whole four-bar phrase**. The new compiler must not simply fill every rest indiscriminately. Distinguish:
- **required note/gesture occupancy**,
- **musical breath** where other roles may answer,
- **hard silence** that must remain empty,
- **accompaniment-service windows** where the carrier resumes complementary comp/bass.
Reserve a *subphrase* source window with explicit silence scope, or generate a genuinely longer 4-bar source. If a carrier also comps, give source and accompaniment disjoint owned lanes and prove no overbooking. Required source must not disappear or be masked by its own backing.

**D3: disabling actions silences the band conversation.** \`examples/humanmusic_argument.rs\` performs the argument using \`PerformanceOptions { actions:false, ... }\`. In \`performance.rs\`, this selects \`ActionPlan::none()\`. In \`interaction.rs\`, \`interact = opts.actions\`, and \`if !interact\` returns with no planned calls/responses. It therefore suppresses exactly the interlocking phrase conversation needed for Casiopea/Takanaka-level ensemble density; it also removes action-driven fills, kinetic changes and other intent moves. **Do not blindly set \`actions:true\`**: Sol disabled those because unrestricted harmonic edits/modulations conflicted with the pinned functional chart. Introduce an **argument-compatible action admissibility interface**: referent/phrase-bound Call/Answer, Pickup, Fill, ReEntry, Push etc may be admitted only after semantic/harmonic planning proves them compatible; harmonic mutation types require source re-planning or typed refusal. Keep opts public layouts/default behavior intact where required. The new action ledger may not count discretionary ornaments as proof that an obligated thematic answer occurred.

**D4: late dynamics and stale planning views.** Claude's \`section_energy\` multiplies non-source \`Score.notes\` velocities and drum levels **after** \`perform_argument\` and its independent source witness have returned. It can improve listening, but the final sounding part differs from the part other performers heard, and the global performance receipt/planner did not govern this dynamic. Move a typed \`PhraseDynamics\`/ensemble energy-terrace input **upstream** into \`CompositionPlan\` or \`PerformancePlan\` before voice choice, expression, masking and hearings; add a corresponding read-only source/Score witness. Preserve the current example as a controlled A/B until accepted, but don't turn the late pass into core production.

Related architecture issue: \`MusicalArgument::compile\` rewrites selected \`song.plan.discourse.goals\`, ledger/backbone/harmonic map while preserving other derived fields from the original \`CompositionPlan\` (e.g. form/arrangement and scalar goals) rather than building a completely coherent argument-aware plan. \`prepare_performance\` pins chords/contexts **after** \`PerformancePlan::from_song_admitted\` has already built actions, accent grid, interaction statements, ensemble modes and budgets. With \`actions:false\` this is constrained but still a partly two-stage authority split; turning independent actions back on before relocating harmonic commitment would risk stale decisions. Reorganize the opt-in route so the source harmony and argument obligations are committed before dependent planning. Do not recalc everything after realization and call that causal repair.

### 1.3 Additional load-bearing music defects

- \`ArgumentRelation::Answer\` currently adds one tonic completion to an otherwise repeated ten-note source; a semantically valid terminal event need not feel like a **long-horizon melodic and harmonic resolution**. Make completion over a multi-bar phrase by carrying prior named intervals, specific guide-tone arrivals, and the band's changed relationship to the theme; certify it without claiming universal perceptual meaning.
- A/B source case variation is narrow: the ten-note source differs mainly in its peak (10 vs 12 semitones). Six placements and two denied-vs-open choices cannot represent 12 independently related verse/hook/bridge statements, a turnaround, or section-level chromatic development.
- All named carriers currently copy the complete phrase in octave register. This is valid for an intentional tutti, but not enough for **complementary counterpoint**. Existing \`EnsembleBar\`, \`AccentGrid\`, \`KeysMode\`, \`BassMode\`, \`DrumsMode\` and \`budget::allocate\` are already present: use them under stronger source bindings, rather than creating an unrelated band generator.
- \`SongMap.plan.contract.grammar\` remains DeflectedLift (loop-shaped) while the new argument wants directed question/denial/answer. Sol reports \`Commutation::check\` divergences A=5/B=7 and ordinary \`PerformanceReceipt\` not green. Preserve those failures as distinct **legacy coarse-law scope** until an honest optional source-aware read-side projection can name exactly which obligations are satisfied. Never force target from observation.
- \`argument_witness\` correctly verifies mandatory raw-source events. Its \`observe\` sorts/deduplicates by statement content and currently does not certify the *non-source bass/keys/drum composition*, multivoice interlock, actual sounding guide-tones, masking, or human semantic comprehension. Add separate non-source evidence; never weaken raw source checks to make busy backing falsely count as a thematic carrier.

## 2. New musical architecture — a small, typed compositional compiler, NOT another engine

Use the existing \`MusicalArgument\`/source graph as the top-level source, extended through small sidecars. The requested upgrade is a **joint source/harmony/form/ensemble composition pass**, *before* the existing performance realizers.

### 2.1 \`ThemeFamily\`: related, non-identical compositional material

Define a type or narrow structure that contains:
- \`verse\`: a **four-bar generative timed phrase**, with distinctive full-length interval/rhythm profile, rests, accents, cadence tendency and a salient source head;
- \`hook\`: another **four-bar generative timed phrase**, with demonstrably shared defining DNA (e.g. opening cell, rhythm or interval transform) but a **genuinely different consequent and recognizable arrival**;
- \`bridge\`: a defensible thematic transformation (pivot/modal reinterpretation, contrary motion, fragmentation/reconstruction or reversal) under a **different functional harmonic context**;
- \`return\`: a transformed/reinterpreted hook whose new closure pays a named earlier harmonic or motif obligation.

Do not hardcode the teacher's absolute MIDI arrays or Casiopea/Takanaka copyrighted melodies. Extract **generation laws** and compatibility constraints from the teacher as calibration and retain fresh seed/world holdouts. The present \`ArgumentTransform\` (octave and rational time) remains a valid transport subgroup, but add a **separate partial compositional-development algebra**: bounded contour variation, rhythmic rearrangement with preserved landmark cell, sequence, motivated chromatic approach, fragmented/answered consequent, register shift and harmony-aware target. Source identity is preserved under an explicitly declared invariant, not arbitrary reuse of \`ReferentId\`. Different motif families must remain distinguishable. Prefix similarity alone is not an independent ear certificate.

The current \`same_head\` full-event equality should remain exact for literal source carriage, but **must not be the criterion for all rhetorical kinship**. Give each transformation its domain, preserved landmark(s), changed musical property, expected destination and typed refusal. Do not silently squeeze four-bar material into a too-short phrase.

### 2.2 \`HarmonicRoute\`: source-owned functional travel

Add a source-level route of time-stamped **functional harmonic events** (chord family/quality, allowed extensions, pivot region, cadence destination, deadlines), rendered through the **existing harmony/context/voicing** infrastructure. A four-bar verse may travel through pre-dominant→dominant→tonic→secondary dominant; a hook may reinterpret IV→V→ii→i; a bridge needs an earned pivot/temporary region and a return home. This is **a generative relationship grammar**, not a frozen Am–F–C–G lookup. Confirm the source/pitch basis and vocabulary in each world or explicitly reject incompatibility.

Pin harmony BEFORE building interactions, accent grids, ensemble/budget plans or voice realizations. Use the actual \`HarmonicContext\`, leading-tone resolutions, 3rd/7th voice-leading and bass root motion as an independent Score witness. \`Score.chords\` alone is a **written claim**; collect actual note content in the relevant time spans and distinguish simultaneous guide tones from a mere temporal union or inaudible meter. Do not rely on a single I/V7/vi chord held over 4–8 bars and claim teacher-level harmonic travel.

### 2.3 \`EnsembleDutyPlan\`: complementary high-information density under one groove

Compile each 4-bar phrase from shared source, harmony and \`AccentGrid\` into role-specific **timed responsibilities** *before* emitting notes:

- Lead carries expressive verse/hook speech, with 8th/16th inflection, leaps, meaningful long notes, controlled pickups and **actual** call/answer contours.
- Bass is a melodic voice: play chord roots/3rds/7ths, coherent arpeggiation and **destination-relative chromatic approaches**, lock downbeats with the kick, counter-melody in lead holes. Do not overtake the lead or surrender the root through entire phrases.
- Keys comp guide tones/7ths/9ths/13ths, staccato rhythmic stabs and short countermotifs in the *available gaps*; supports changing harmonic functions without opaque pad sludge.
- Drums carry a shared pocket (e.g. 16th hat grid, ghost snare, bass-linked kick), section-level fills and measured collective push/pull. Structured group timing, NOT independent stochastic note jitter.
- Pad occupies only licensed sustaining support space and may disappear entirely where it masks motif/counterpoint.
- Each role has \`RequiredStatement | Counterline | HarmonicFoundation | Comp | Fill | WrittenBreath | HardSilence\` duty windows (illustrative type names). The authority of motif source events, rhythmic rests and stage silences is explicit. Complementary density fills only spaces licensed by those duties.

Use \`plan_ensemble\`, \`AccentGrid\`, \`budget::allocate\`, source continuities and rehearsal/admission as the starting point. Implement opt-in **argument-compatible action sets** that keep Calls, Answers, Fill, ReEntry and kinetic variation while refusing uncontrolled harmonic edits. Test against \`actions:false\` and full \`actions:true\` negative control. Every source obligatory event must be reserved before optional ornament.

Define density as a *vector*, not an objective to maximize: note onsets by role per bar; onset phase and syncopation; guide-tone movement; interlock vs unisons; source coverage and intentional silence; per-section dynamic contrast; unresolved/paid source debts. Compare teacher's distributions under matched tempo/form when possible; avoid “3× more random notes” as a fix.

### 2.4 Place source and performance with one clock/authority

Build typed \`SectionProgram\` as an arrow from musical context (tonal region, groove phase, source identity, open debts, roles) to next context, with source/arrival/duty witnesses. Section composition is allowed only when endpoints and obligation semantics match; typed failure otherwise. \`ThemeFamily\` and \`HarmonicRoute\` are not competing global authors; they are joint constraints resolved into **one** existing \`SongMap\` / \`CompositionPlan\` with an argument sidecar, before \`PerformancePlan\`.

Musical ensemble combination can be modeled as a constrained join of role performances over the **same** harmony/metric/source context. Call it a categorical fiber product only if the maps and matching constraints are explicitly implemented/tested; do not use category jargon as decoration.

Tests for genuinely applicable laws:
- identity/source neutrality and v0.4 OFF byte identity;
- composability of legal section transitions and associative gluing where defined;
- lawful transposition/world change preserves declared thematic identities, but not necessarily PCM;
- source-carriage and semantic-obligation diagrams commute under a **strong independent Score projection**;
- false cross-role “same count” or modal/chord-label compensation cannot satisfy a missing source or wrong functional arrival.

This is a **partial, typed algebra** with refusals: no need to pretend every musical operation is invertible, total, or equivalent.

## 3. Decisive execution rounds (red-first, each independently committable)

**R0 — freeze golden and baseline; run cheap discriminators.** Locate/record the actual seven golden WAVs, their exact source SHA, invocation args, hash and listener annotations. If files unavailable, recreate only what source permits and label them \`RECONSTRUCTED\`, not the actual accepted seven. Capture the #96 A/B \`humanmusic_argument\` and C137 source baseline under native and matched conditions; no loudness/tempo/production confounds. Execute exact-head regression/CI. Report \`SourceGraph\`, per-role onset/gate events, chord changes, role occupancy, active voice count, meters and stems. First falsifiers: \`actions:false\` kills interactions; six short phrase placements; over-reserved silent windows; \`Develop\` is literal, not an actual source variation; section energy is after planning.

**R1 — fix source phrase occupancy and dialogue.** Keep current 10-note germ as a control; implement a narrow \`CarrierDutyWindow\` distinction between \`RequiredStatement\`, source-owned silence and freely complementable gaps. Show that the same material can be placed without erasing the bass/keys for eleven subsequent beats, *and* that hard intended rests remain empty. Make compatible short Call/Answer and Fill functions available without mutating the committed chart. Red tests for unauthorized filling, duplicated source, wrong carrier and harmonic mutation after a dependent has planned. Do not increase source-semantic invariants by counting extra backing notes.

**R2 — actual 4-bar verse/hook pair.** Generate a recognizable long verse and a distinct but source-related hook, with at least two motives/contrasting consequences; each phrase should extend meaningfully across its 16-beat slot. Preserve separate note onset, gate, accent, rest and phrase span. Build source relationship evidence that catches unrelated melodies but permits a purposeful answer. User listens to original generator vs candidate vs teacher in a controlled packet. If the long source is better but full mix still flat, the remaining limiter is arrangement/harmony.

**R3 — joint functional chart and bridge.** Introduce per-bar time-varying harmonic routes and targeted temporary tonicization/pivot, with actual leading-tone/guide-tone resolution. Exercise modes/world vocabularies and incompatible source refusal. The bridge must take the theme somewhere *different* and alter how the return is heard—not just repeat V7 until a tonic. Test denied vs answered arrivals in Score and sounding-note context; no unsupported claim of harmonic certainty from a written chart.

**R4 — fusion orchestration; earned density.** Activate and constrain the existing band's interactive/kinetic machinery under the source argument. Plan melodic bass approaches, guide-tone/counter-comp keys, 16th hats/ghosts/fills with coordinated pocket and complementary entry/rest windows. Reuse existing realization functions and tune bounds; avoid second score generator or fake density from unisons. Put the example's energy-terrace policy into the upstream opt-in plan so the score players have truthful hearings. Measure per-bar/section onset activity, offbeat distribution, independent counterpoint and masking, preserve intentional quiet.

**R5 — semantic completion + third family only if earned.** Distinguish real \`Develop\`/EarnedAnswer/Return from a repeated head plus one terminal note; require a source-related, harmonically/ensemble-supported multi-phrase closure. Then build C (conflict→reconciliation) only after related A/B musical cases are ear-legible, with an actual two-source compatibility operation and counterexample against mere simultaneous playing.

**R6 — production hardening and release handoff.** Honest optional read-side projection to general \`PerformanceReceipt\` and legacy \`Commutation\` without changing their scopes or forged IDs; support reference corpus and blind packet; many fresh seeds/tempi/lengths/worlds and negative controls. \`fmt\`, strict clippy, denied rustdoc, targeted/full tests, R17/covers, legacy PCM/fingerprint, ABI MSRV/CI exact SHA. No main merge, release, C ABI change or v0.5 finale acceptance without maintainer listening. Document what 1.0 language parity will later need.

## 4. Predeclared rival explanations — choose experiments that fail differently

| Hypothesis | Matched test | What changes the conclusion |
|---|---|---|
| **H1: sparse motif/weak distinct hook** | current 10-note source vs same harmony/production with licensed 4-bar source family | Theme becomes compelling in solo as well as full mix |
| **H2: role occupancy suppresses density** | whole-phrase hard reservation vs typed subphrase source+legal complementary service | Bass/keys keep their functional service without destroying or masking source |
| **H3: interaction engine is disconnected** | actions OFF vs source-allowed rhythmic/conversational actions ON, harmonic pinned in advance | Calls/answers/fills materialize and stay source-legal; old generic harmonic mutation still refused |
| **H4: harmonic travel is missing** | static tonic/V7/vi vs functional per-bar route, same theme/tempo/patches | Audible bridge/return improves without source identity loss |
| **H5: only performance/mix masks meaning** | source-only, role stems, dry/full mix, ablated source, equal level; see written guide tones too | Solo clear but full mix ambiguous localizes arrangement/masking |
| **H6: argument proof is too permissive** | mark literal \`Develop\` vs an explicitly nontrivial licensed transform; wrong bridge/cadence/hook negative mutants | New receipt rejects trivial/restated cases claiming development; legacy witness remains a control |

Stop tuning lower-priority mechanics once one test falsifies its corresponding cause. Preserve failing traces/negative routes with provenance; never fabricate an ear verdict.

## 5. Acceptance and integration handoff

**Engineering evidence**: each state has exact commit, source config, argument/meaning/discourse/Score fingerprints, role-level timed MIDI-like event export, harmonic route, occupancy/groove receipt, actual PCM WAV/stems, runnable commands and tests. A new Rust source-facing composer route must be opt-in and bounded; no hidden auto-enabling.

**Musical evidence**: at least one new generated piece exhibits an intelligible **distinct verse/hook, developed bridge and earned multi-voice return**; advanced rhythmic/bass/keys/drum motion serves the same argument. Compare against golden teacher with honest native/matched context distinctions, randomized blind IDs and independent user verdict. Jazz-fusion energy is a functional-arrangement target, not imitation of copyrighted Casiopea or Masayoshi Takanaka melodies.

**Honesty**: tests can demonstrate source identity, harmonic/temporal obligations and acoustic contribution, not Casiopea quality or human semantic recognition. PR #96 current listening remains **not golden**. Documentation in Sol #95 calling B's return “augmented” must be updated to describe Claude's current identity 1/1 return until a genuine transform lands. Fix false progress summaries whenever code or ears disagree.

**Next action:** give Claude the full research plan, ask it to fan out independent tests/source authoring/interaction-budget repair/harmonic compiler, and integrate via a new non-main code branch from the **fresh actual PR #96 head**, not this research branch. Avoid two writers on \`argument.rs\`, \`functor.rs\` or \`PerformancePlan\`. Full integration can be sequential even when independent probes are parallel.

**Guiding invariant:** every note is there because it represents a source, contributes to another player's gesture, moves a harmonic obligation, or creates an intentional silence. The new song should not merely repeat “I exist”; it should develop an identifiable question, explore real harmonic territory, let the band debate it, and conclude in a way its beginning made meaningful.
