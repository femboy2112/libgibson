# THE HOMOSAPIANS — Phase, Pocket and Categorical Naturality
## Source-pinned audit, executable negative controls and the R0–R6 mega-round

**Status: audit and research tests, not an accepted audio change. October 19, 2026.**

### Provenance / evidence
- Exact audited parent: femboy2112/libgibson, active draft PR #123, impl/homosapians-rehearsal-composer-2026-10-16 @ **e6fbaffd015399931afb4f063a2b4a75ed17fd3b**. Its head contains a comp register-ceiling improvement accepted as better by the owner's ears, but the Keys still feel out of pocket. Head CI was in progress at initial audit; prior 2026-10-17 head b3547700 passed hosted CI run 38995541867. Refresh head and CI before implementation.
- Stacked ancestors: PR #122 research @ 0f416e08, #121 Homosapians band @ 56f2391, #120 research @ 3cf10b0, earlier Johnny Five and C137. Historical C137_ACCEPTED_V1 frozen 51 acoustic/Score reference artifacts must stay byte-exact with all experimental switches OFF.
- Audit methods: exact GitHub source inspection, algebraic timing analysis, existing evidence ledger and user listener verdicts. Separate test file audio_homosapians_pocket_naturality_audit.rs is a characterization of the REAL frozen Score; **tests need their own exact-head CI verification**. No independent WAV/PCM audition has occurred in this audit.

## 1. What is genuinely working

The rehearsal composer is more than a relabel: source-dependent two-bar riff candidates; riff→bass decisions; bass→DrumResponse proposals; a guitarist revision back-edge; immutable rehearsal trial snapshots; original chord camps; Keys guide-tone polyphonic accompaniment, actual zero-Lead, pad; less cliched harmonic shape and tasteful syncopation. The latest B4/MIDI71 comp ceiling lowered clashing upper chord tones and the user heard an improvement. Preserve the seed/notes/harmony/section form as an acoustic-control source while investigating timing. Do NOT debug pocket by changing pitch, comp width, bassline or chord chart simultaneously.

## 2. Source-verified timing mismatch

The current code has these actual maps:

- theme_family.rs:821: Shared pocket P(t)=t+pocket_offset(t mod 4). Offsets in beats: downbeat 0, beat3 0.012, beats2/4 0.030, eighth ands 0.034, other sixteenths 0.046.
- kernel.rs:906: KEYS_ATTACK_COMP=0.030 **beats**.
- kernel.rs:1652: riff-comp score onset K(t)=max(0,P(t)-0.03).
- kernel.rs:1940: Bass note B(t)=P(t).
- kernel.rs:2048+: Kick/snare at P(t) when using shared-pocket attacks. The funk kit's sixteenth hi-hats are ON GRID (no pocket delay), by explicit design.
- rehearsal.rs:1199+: required Keys source-carrier arpeggio takes four onsets 4.5,5.5,6.5,7.5 plus pocket_offset **without** the subtraction.
- synth.rs:625+: one Score TempoMap for all roles. Relative mismatches in Score are not repaired later by the synth.
- VAPOR95 patch: nominal Keys ADSR attack 0.003 seconds; warm chorus ~18ms ±3.5ms wet delay at ~40% mix, so the dry attack still exists. A fixed pretrigger of 0.03 beat is NOT a demonstrated real physical onset latency.

At 104 BPM, -0.03 beat equals **-17.31ms** relative to a bass/kick on the same pocketed slot. Compare nominal positions (NOT PCM measurements):

| Position modulo 4 beats | Shared pocket +beats | Keys-comp displacement from grid, beats | Keys vs grid-only hats at 104 BPM | Keys vs same-slot pocketed bass/kick |
| --- | ---: | ---: | ---: | ---: |
| 0.00 | 0.000 | -0.030 (song start clipped to 0) | -17.31ms | -17.31ms |
| 0.25 | +0.046 | +0.016 | +9.23ms | -17.31ms |
| 0.50 | +0.034 | +0.004 | +2.31ms | -17.31ms |
| 1.00 | +0.030 | 0.000 | 0ms | -17.31ms |
| 2.00 | +0.012 | -0.018 | -10.38ms | -17.31ms |
| 3.50 | +0.034 | +0.004 | +2.31ms | -17.31ms |

**Critical bar-crossing:** every riff includes local onset 4.0. Its comp attack is placed at 3.970, while the bass root/kick remain at 4.0. The comp obtains the NEXT bar's chord using at_grid=4.0 but emits it in the previous chord's time-span, with older pad harmony still sounding. If anticipation is deliberate, the source must authorize it; current subtraction is unconditioned by musical intention. Source/section provenance can therefore disagree with actual harmonic intent. The first onset at song time0 is CLIPPED to0, so the same riff downbeat has a different phase relationship at later repeats.

**Same-instrument flam:** required Keys carrier source notes are at 4.5,5.5,6.5,7.5 + pocket, and riff-comp notes at coincident riff onsets are at pocket-0.03. If selected riff contains 5.5, carrier is 5.534 and comp 5.504: two same-Keys-family attacks spaced 17.31ms in the SAME six-voice synth pool. Register ceiling does not repair temporal disagreement. Short note gates with 0.45s release and close chord clusters also make voice stealing an independent concern to measure.

**Tempo naturality:** the alleged "physical FM latency correction" scales in seconds with tempo because it is in beats: 80BPM=22.5ms; 104BPM=17.31ms; 140BPM=12.86ms. A DSP patch's attack and chorus delay are specified in seconds, so a fixed physical time latency does not obey this rule. Musical behind/ahead timing may legitimately scale in beats; musical phase and physical onset compensation must be typed independently.

## 3. Existing tests miss the failure

tests/audio_homosapians_rehearsal.rs:keys_comp_stabs_on_the_riff_rhythm checks ANY Keys note within **0.12 beat** of the riff onset (~69ms @104), ignoring whether it is actually a comp note, and without reference to bass/kick or acoustic onset. This can pass while chords consistently land 17ms early, cross a section boundary, or flam against the carrier.

Existing src/audio/human_music/pocket.rs already declares "physical latency has no role in construction" and provides typed LatticeSlot/MetricPosition and phase observation tools; riff-comp currently bypasses those richer obligations using a global numeric constant.

Current argument_witness.rs:in_window excludes all Keys events with the string provenance role_note="comp" from the exact carrier check. This valid narrow fix prevented a fake silent Lead, but the **separate CompGesture content/rhythm/phase law is not independently certified**. Add a typed CompIntent/CompGesture sidecar rather than loosening carrier preservation or accepting free-form labels as proof.

## 4. Another non-load-bearing arrow: drummer's discretionary composition

The pure rehearsal composer actually computes DrumResponse(reinforce_push,leave_gap,fill,reasons) from bass and the guitarist can react to it. But examples/c137_song/rehearsal.rs:freeze NEVER reads draft.drums when it builds the CompiledArgument; it passes the bass cells, enables pop_funk_kit, and the kernel makes a stock kit that reinforces root/push. Consequently altering only the recorded drummer decisions can leave the entire final Score's drums UNCHANGED. The bass/kick relationship is real; the claimed discretionary drummer→actual performed part relationship is not yet load-bearing.

Do not infer false work from a pretty category arrow. Supply a source-authorized DrumGesturePlan emitted and independently witnessed in the frozen Score; a genuine WITHHOLD decision may realize as justifiable silence while preserving the skeleton.

## 5. Categorical ethos as actual obligations: typed commutation

We already have three distinct times:
- Rehearsal revision k: editable, versioned drafts, past trial takes immutable.
- Musical metric t: source-intended onsets, rhythm, section identity; committed Score protected.
- Physical sample n: scheduled synthesis, patch envelopes, chorus, stems, observed attacks.

Introduce tiny EXPERIMENTAL sidecar types (avoid changing historical Score and public Patch):
- MetricAnchor(source_event_id, bar, subdivision, grid_beat)
- PocketIntent(anchor, owner, relation: Lock|Push|Behind|Withhold, reference_id)
- AcousticLatency(patch_id, render_chain_id, attack_delay_ms, measurement_error)
- PhaseReceipt(source_id, intended_time, pocketed_time, scheduled_note_time, measured_pcm_onset, signed_error, provenance/section)
- CompGestureId and DrumGestureId binding SOURCE to emitted notes/hits, distinct from decorative prose labels.

**Law C1: shared time is a RELATION, not a float.** For a declared Lock(source Keys hit, pocket reference kick/bass), the measured adjusted relative phase must fall in a calibrated bound. A deliberate Push/Behind may violate strict simultaneity only if licensed by SOURCE. No universal grid quantization or automatic note snapping.

**Law C2: naturality across tempo changes.** Musical pocket offsets measured in beats scale with tempo, while patch acoustic latency in seconds does not. Move compensation to one explicit physical realization boundary or parameterize it with measured milliseconds; test 80/104/140 BPM and 44.1/48kHz.

**Law C3: no unauthorized bar/section crossings.** A chord belonging to the next harmonic interval may sound before its bar ONLY with an explicit anticipatory gesture and independent source-to-sounded-chord witness. Physical instrument corrections must not silently change musical harmony or provenance.

**Law C4: source-to-stem comp projection.** A riff gesture can cause two or three chord notes, but all of them project to the SAME authorized CompGestureId; multiplicity, timing, role, harmonies and rests must agree with independent witnesses. Merely filtering a "comp" text tag from the required carrier check does not prove the generated accompaniment is lawful.

**Law C5: no inert musician decisions.** For a claimed audible DrumResponse action, counterfactual changes to a legal choice must alter authorized discretionary drum Score/PCM events (or produce explicitly licensed silence), without changing the immutable pulse skeleton or earlier rehearsal trial. Don't demand full mathematical faithfulness over all music, only discrimination on a finite set of declared audible causal effects.

**Commuting experiment (not an asserted theorem):**

    Authored riff gesture ----> authorized comp/bass/drum gestures
              |                           |
        intended pocket                scheduled Score
              |                           |
              v                           v
         target attack  <--- observed physical onset (latency-corrected)

A square with a label "Lock" but 17ms undeclared phase drift does not commute. A square with a documented deliberate anticipation may commute to a DIFFERENT target relation. The proof obligation must refer to the same source identity and actually emitted events.

## 6. Next mega-round, in dependency order

**R0: truth gate, no creative edits.** Run the new characterization suite P1–P4; prior rehearsal 19/19 and all pop, C137 suites; original 51-item lock. Dump actual Score note onsets, provenance, riff IDs, Bass root/Kick/hat, section/chord at each planned onset. Resolve whether any current test claims more than it measures. Pin 7/42/2112 plus 9/13 and holdouts. Label SOURCE/SCORE/PCM/EAR separately.

**R1: perceptual phase metrology.** Render SAME Score and voicings at 104BPM through dry Keys, Keys+chorus, bass+drums and full band, using existing ProductionControl. Analyze transient/envelope onset timing and source-relative phase; report method/error bars. A/B 2x2 of original 0.03beating compensation vs 0 compensation × chorus OFF/ON; optional intermediate values with explicit seconds/beat units; loudness-match without independently overnormalizing stems. Hold harmony, riff, note count, voicing and tempo fixed. Ear is sole aesthetic promotion gate.

**R2: small, lawfully typed timing fix.** Calibrate actual patch latency if measurable; avoid unlicensed whole-bar/key chord anticipations, make first/repeated bar treatment consistent. Let Keys comp and required Keys carrier use a common declared musical pocket relation unless an explicitly authorized source gesture demands otherwise. Use one tempo conversion; never globally change old C137, Bass/Kit or acceptable R17. Invalidate/rewrite characterization tests only after new positive/negative witness is installed.

**R3: categorical teeth for the comp.** Build typed CompGesture ownership with independent source-to-Score timing/chord membership, controlled polyphony, no phantom notes or omitted rests, auditable stage/section ownership, hostile phase mutants and erroneous next-chord-before-boundary cases. Protect the exact required Keys carrier and historical source witnesses. No "all comp role_notes exempt" as the ONLY proof.

**R4: make drummer decisions audible.** Add opt-in DrumGesturePlan downstream of actual rehearsed DrumResponse; preserve kick/backbeat backbone. Test counterfactual different legal drummer choices with same riff/bass/harmony producing different discretionary Score events; source and draft receipts immutable. An explicit SILENCE may be a real decision.

**R5: song form and lead key.** Fix the G6 source-section form witness, still-vacuous full-route loop period; certify 2-bar vamp, chorus transformation, rehearsal revision, and melodic/harmonic/narrative closure independently. Add a source-licensed foreground lead only after the rhythm band is ear-approved. Upstream backing should stay the same source under insertion; nonlinear full mix needn't be sample-additive.

**R6: resume physical/FM/ensemble after phase acceptance.** Multi-operator FM, Meatsack physical guitar/string ringing, piano pedal and shared acoustic field are separate legitimate workstreams, but don't use more timbre to hide an unresolved source-level timing problem.

## 7. Fan-out responsibilities and falsification protocol

Principal integrator: sole owner of kernel.rs/synth.rs and final PR; ONE heavy Cargo build lane on the limited-memory Linux machine.

- Scout A: phase metrology/DSP/attack onset analysis and matched dry/chorus A/B; no composing edits.
- Scout B: categorical typed times, MetricPosition/reused pocket diagnostics, commutative observers, tempo/samplerate invariance.
- Scout C: Keys carrier/comp mutual timing, exact Source→CompGesture projection/ownership and bar-crossing mutants; no generic rhythm rewriting.
- Scout D: drummer's current inert response and typed emission/witness; separate opt-in module until principal integrates.
- Scout E: hostile test author: source ghosts, stale provenance, false near-riff matches, voice stealing, 80/104/140 BPM, frozen C137 hashes; no rebasing accepted goldens.

Each child returns pinned SHA, actual code/tests, measurement, red control and green case, exact commands/CI, WAVs or an explicit NOT AUDITIONED label, interpretation limits and next falsifier. Keep new changes reversibly opt-in. No main merges, releases, default flips, public ABI modifications or unapproved sample assets.

**Goal:** the Homosapians' categorical foundation must guarantee that every perceived musical relationship comes from a declared musician/source relation, and that every musician's claimed decision has an observable legal consequence. A pocket diagram that commutes only in prose has no teeth.
