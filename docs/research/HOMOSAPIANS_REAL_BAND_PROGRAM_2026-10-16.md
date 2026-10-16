# Operation THE HOMOSAPIANS: Make a Real Band
## Source-integrity, colloquial pop theory, physical instruments, and ensemble timbre

**October 16, 2026 — research implementation plan, not an accepted musical change.**

**Source of truth:** femboy2112/libgibson, active experimental PR #118, impl/humanmusic-johnny-five-soul-2026-10-14 @ **4b3306257af8d75beb61d117bb4b32d244c36818**. It stacks on PR #116 @ **f3725fdb750a62de69124f69800fc2eeec1ac66b**, itself stacked on Johnny Five #114 @ **b536699ed97c6db4320874d4cf31fb229e90cf25**. Exact-head CI #38823342157 passed. **main remains v0.4 @ 80706c91**, not the frontier. All experimental PRs remain draft.

**Regression foundation:** historical accepted 16-bar C137 Full on PR #111 @ **f68c9bfca179f9f248dd5cd601f0c4d3987a1658**; the independently pinned **C137_ACCEPTED_V1** verifies 51 WAV/Score items across seven scopes and three seeds. Nothing in this program authorizes changing historical PCM, un-drafting, merging, releasing, or making a sonic default.

**Previous relevant research designs (research-only, not runtime):** docs/HUMAN_MUSIC_GSPU_FM.md and HUMAN_MUSIC_GSPU_FM_HANDOFF.md at research/humanmusic-gspu-fm-2026-10-09; docs/HUMAN_MUSIC_MEATSACK_PROJECTION.md at research/humanmusic-meatsack-projection; docs/HUMAN_MUSIC_MEATSACK_PROJECTION_BOUNDARY.md on recent C137 feature descendants. Those designs were written on older parents; port the architecture to the real PR #118 head, not commits from main.

**Evidence status:** source inspection, the actual chord-quality interval table, source-event extraction and programmatic note/chord comparison, current CI and pre-existing listening ledger. The accompanying Rust integration tests are a newly committed REAL-Score diagnostic surface; their successful execution must be confirmed by CI. No remote audio was independently listened to in this audit.

---

## 1. The founding hypothesis: one thing, made of several things

The listener's Foo Fighters / Pearl Jam / Feels / Charlie Puth observation translates into two independently necessary kinds of coherence.

**Musical:** backing guitar/keys, bass, and drums have a complete instrumental statement even WITHOUT lead. They share a genuine pulse, a riff/loop identity, harmony, meaningful rhythmic relationships, a section journey, dynamic intent and an intelligible destination. **The band is the lock.**

**Narrative insertion:** a recognizable lead/topline enters the band as an independently meaningful but musically compatible statement, reinforcing or transforming the existing backing without destroying its own song identity. **The lead is the key; performance turns it.**

**Physical-acoustic:** guitar, bass, kit and keys maintain distinct timbres while their time-frequency envelopes, harmonics, attack/resonance, room perspective and shared nonlinear production cause the total to be experienced as a single sonic object. The band has BOTH constituent sources and collective gestalt.

Do not mistake stable electrical phase coherence for the objective; distinct independently played instruments normally do not have locked relative phases. Actual waveform superposition, amplitude envelopes, rhythmic synchrony, frequency occupancy and common room/bus response are different phenomena. Maximal overlap is mud; maximal isolation is unrelated tracks.

**Foo Fighters is a structural/production inspiration, not a claim that every recording is composed as a complete backing piece and only then has vocals overlaid.** Wasting Light producer Butch Vig discusses Grohl's rhythm guitar locking to drums, separate guitar roles, live/tape performance and the garage room. Reference: https://www.soundonsound.com/people/foo-fighters-recording-wasting-light.

---

## 2. ACTUAL harmonic audit: what Claude fixed, and what remains false/underspecified

### A. Chord construction is not categorically broken

The live PR #118 pop SONG Circle progression is **Am9 | Em7 | Fmaj9 | E7**; Lift is **Am9 | Fmaj9 | Dm9 | E7**. Source: examples/c137_song/source_pop.rs:82-105. The older **pop GROOVE** Circle is **Am9 | Dm9 | G9 | Cmaj9**. Do not conflate those forms or compare their pitches against the wrong chart.

The tones computed from src/audio/human_music/theory.rs:167-239 are conventional: Am9={A,C,E,G,B}, Em7={E,G,B,D}, Fmaj9={F,A,C,E,G}, E7={E,G♯,B,D}. The applied SourceDomain::SmoothPop chord sequence is real, not merely labels in a manifest.

**The R12 bass-direction repair is materially valid:** source_pop.rs:476-529 uses a newly typed BassDegree::Seventh at local bar-3 beat 1 over E7. kernel.rs's chord-derived seventh resolves D (pitch class 2), and this note is voiced in-range BELOW the bar's E-root. Our new tests characterize this on the real Score. That is an improvement, not an error to revert blindly.

### B. There is a specific note-level color conflict Claude did not eliminate

In latest Circle pop SONG hook, its second 16-beat episode starts at global beat 16:
- At **global beat 29.5**, the lead plays **G natural** (step 10 above tonic A). E7 sounds from global beat 28 to 32, and E7 contains G♯, its major third.
- The Consequent section seats Pad, which voices the E7 chord tones for the full final bar. Thus at **beat 29.75** there is an actually scheduled G-natural Lead against a sustained G♯ Pad. G natural over E7 is its sharp-nine/minor-third blues color: **possibly artistically intentional**, but it has NOT been removed by deleting only G♯ events from the lead.
- The current comments at source_pop.rs:451-459 overstate the fix (they call the remaining G natural non-blues because melody G♯ disappeared, ignoring chord/pad G♯). Source-to-Score consistency must inspect actual vertical sound, not a single line or the source comment.

Other non-chord tones in Circle hook include A=11th over Em7, B=#11 over Fmaj9, A=11/sus over E7. None are per se forbidden. A real observer should distinguish structural note, anticipation, neighbor, suspension, chord tension, contextually licensed color, and genuinely incompatible simultaneity; the latter is context/ear-dependent.

### C. "Resolved" is conflated with tonic-melody and groove-cycle closure

The final hook lead ends on A (tonic pitch) from **beat 30.5**, while the last actual chord until the song stops at beat 32 is **E7**, not Am. The source labels its ending ArgumentEnding::Resolved, and a melodic Kinship law treats that tonic-pitch ending as Closed.

This might be a perfectly artistic suspended ending or an implicit resolution on the NEXT loop iteration; that next Am is **not actually heard after the end of this 32-beat rendering**.

src/audio/human_music/argument_witness.rs:98-161 only checks a Consequent's cadential chord when transform.completion.is_some(); with this pop source it is absent, so it does not independently establish a sounding tonic-chord resolution. The pop_route_law checks exact chord-array recurrence, not a completed narrative payoff; with loop_bars equal to the route length, it can become vacuous.

**Repair the TRUTHFUL CLAIM before editing the NOTES.** Define typed independent closure axes: MelodicHome, HarmonicHome, GrooveCycle, NarrativeDebt, AcousticTailEnd, with true/false/intentionally deferred states. A tonic A over E7 is melodic home with suspended dominant harmony, NOT a proved full harmonic resolution. Only source-declared tension/resolution laws justify sound-changing fixes; never automatically snap all chromatics to chord tones.

### D. Last-round notes were overfitted by a narrow search

The PR ledger R8 was explicitly rejected and reverted: Claude changed an overall cadence while the owner was criticizing CONNECTIVE notes. R9 changed some connectors. R10 pulled blues-flavored lead chromatics but did not eliminate E7-vs-G color. R11/R12 corrected local bass direction near 7 seconds. Several high-level descriptions still claim generic closure or no remaining blues friction despite the actual chord and pad.

The current 9/9 smooth-pop tests verify source kinship, some active parts, and bass/kick synchronization; they do not universally guarantee consonant sounding harmony, desirable voice leading, expressive guitar dynamics or mix clarity. The user ear correctly detected things the validator could not observe.

**Test module committed alongside this plan:** tests/audio_homosapians_harmony_audit.rs. It examines committed pitches, real chord membership, actual simultaneous Pad/Lead notes, source-mutation refusal and the descending bass D. These are CHARACTERIZATIONS, not a mandate to freeze an unwanted clash. New repairs need a lawful independent observer, controlled A/B and deliberate update of expectations, not a blind golden refresh.

Commands (run, log result and exact SHA):
    cargo +1.99.0 test --test audio_homosapians_harmony_audit -- --nocapture
    cargo +1.99.0 test --test audio_smooth_pop
    cargo fmt --check
    cargo clippy --all-targets --all-features -- -D warnings
    scripts/verify-c137-accepted-v1.sh

Also run both flavors at multiple keys/tempos, and evaluate actual Note.function provenance and sound after physical envelope masking. No mandatory wholesale simplification of song harmony.

---

## 3. Stop pretending FM equals a physical instrument

### Current production substrate, accurately described

src/audio/human_music/instrument.rs holds a compact oscillator Patch (Shape or simple **two-operator phase modulation Fm{ratio,index}**, a global ADSR/filter envelope, sub/unison, gain/pan). src/audio/human_music/synth.rs:26-197 creates independent voice triggers, pitches, sample-gated ADSRs and releases, with Role-keyed voice pools. Its legacy FM voice also emits another ordinary oscillator sine; it is not a pure two-operator FM graph.

**Shared group processing ALREADY EXISTS** at synth.rs:822-983: per-role bus levels, a pad/keys/lead chorus stage, drum bus, stereo soft saturation AFTER summing, shared reverb, master comp and limiter. Do NOT introduce an additional generic "shared reverb" and call it a new system. Diagnose the current field by ablation, then implement more relevant interactions.

**Missing** are:
- per-string/hand attack & damping and physically plausible guitar chords;
- hammer/key/damper/sustain pedal and sustained, timbrally evolving piano states;
- distinct FM operators and per-operator envelope/modulation/feedback; real timbral dynamics during a held note;
- opt-in room/cabinet/body coupling, source aware bus/voice effects, and measured balance between spectral differentiation and ensemble binding;
- a proper declared relation between a singular scored Strum gesture and its multiple correlated excitations;
- honest coupling between the symbolic audible-lifetime heuristic and the new instrument's physical tails (voice.rs still estimates from historical Patch ADSR).

### Four separate authorities

1. **Song/source:** notes, harmony, narrative motif, macro groove, role duty; never rendered post-hoc to hide bad theory.
2. **Physical gesture execution / Meatsack:** typed intended Strum, Pluck, Mute, LetRing, PianoKey, SustainPedal, Hit with rig/resources; partial feasibility and event/gesture identity.
3. **Instrument voice program:** Legacy (byte-exact off), new pure 2/4-op FM, simplified string-waveguide/physical guitar, hybrid piano/multi-partial attack; evolving spectrum and ring/damper state.
4. **Ensemble Sonic Field:** live shared-state acoustic realization: band-subgroup stereo image, coupling/room response, shared distortion/compression, masking/competition/clarity, separate lead space, source-identical pre-bus stems and controlled post-bus effects.

Physical gesture model can be a typed partial map:
    P_K : (IntendedGesture, RigState) ⇀ (ExcitationEvents, ContinuousControls, RigState', Receipt)
One six-string strum ≠ six independently authored notes. Each string attack inherits ONE gesture and source owner; contact order, pressure and initial energy are correlated. Not every intended gesture is feasible. The renderer never invents a source-unsupported musical event just to make a sound more human.

First practical non-sample guitar candidate: one procedural Karplus–Strong / digital-waveguide string per contact, tunable damping/body bridge coupling and pickup/amp/cabinet tone. Add string stops/palm mutes, chord positions and directional stroke. For clean/electric pop, test actual string contact and ring BEFORE a distortion wall; with distortion, manage gain and speaker bandwidth. Research basis: https://xenon.stanford.edu/~blynn/sound/karplusstrong.html.

First practical piano candidate: per-key hammer transient + inharmonic multi-partial/filtered body, release/damper control, simple pedal/soundboard resonator. Hybrid FM is acceptable; a ten-operator monster is NOT required to prototype a believable sostenuto.

**GSPU-FM** is a valuable parallel orthogonal program: typed pure 2/4-op compiled operator graphs, independent operator envelopes, feedback, finite energy/alias budgets, eventual OPN2-inspired and OPL2/OPL3-inspired dialects (hardware fidelity must be separately proven). But **FM alone cannot construct a guitar hand/string gesture or resonant piano pedal**; do not delay the first physical prototype until the whole Yamaha stack is finished. No mandatory samples, no unreviewed copyrighted timbres.

---

## 4. Sonic coherence mathematically and perceptually

A relevant group-field illustration, not a claimed implemented theorem:

    x_i[n] = Instrument_i(Gesture_i, State_i, n)
    q[n+1] = A q[n] + Σ_i B_i x_i[n]
    v[n] = C q[n] + Σ_i D_i x_i[n]
    y[n] = NonlinearBus(v[n], group_history[n])

Here q is common room/cabinet/group acoustic state and NonlinearBus represents bounded nonlinear mix/comp/saturation. Shared-room tails and group saturation can alter timbre even with fixed sources. The instruments are not required to have phase-aligned carrier frequencies.

Importantly, for nonlinear production:
    Process(band + lead) != Process(band) + Process(lead)
Therefore **band-only PCM** need not be sample-identical to a lead-muted version of the already-compressed full mix. Keep distinct observable layers: source-invariant pre-bus band; physically responsive post-bus mix; direct stem, wet stem and group mix.

Test three CONDITIONS at matched perceived loudness:
- A = independent instrument buses into clean sum;
- B = EXISTING world chorus/saturation/reverb/bus compression;
- C = new opt-in Instrument+Ensemble field with declared shared resonance, instrument envelopes and subgroup bus geometry.

Do not treat increased frequency overlap, loudness or distortion as proof of a unified wall. Also test solo stem intelligibility, envelope attack alignment/causality, masking, phase/color alias artifacts, clipping, and the human judgment that it sounds like ONE BAND while individual instruments remain distinguishable.

---

## 5. Colloquial music theory: a small grammar, deep execution

**The central owner insight:** popular music's familiar, rule-of-thumb global structure is a HARD creative constraint—like a simple instruction set, not a low-ceiling theory. Great pop maximizes meaningful distinction WITHIN recognizable form. Do not solve "baby music theory" by endlessly stacking chromatics, lush ninths, tritone substitutions or instrumental virtuosity.

Introduce a typed **ColloquialMusicIR** layered over existing real music theory and actual score/gesture/room:
- Traditional layer: mode, chord/root/degree, functional tension, voice-leading, section shape, rhythmic meter, source referents, obligations.
- Colloquial layer: **lock in, sit behind, push, lift, hold, chug, strum, let ring, palm mute, leave space, answer, pickup, hold the backbeat, darken, open up, turn the corner, land the hook, drop the kit**.
- Every operator has preconditions/observable realizations/refusals; e.g. Strum = correlated contacts under one GestureId; LetRing = instrument body state persists without reattack; Darken = explicit register/voicing/timbre change, not just lower one note arbitrarily; Push = source-licensed anticipatory gesture against future chart; LayOut = no new attacks in a legal window but tail need not vanish.

Simple macro grammar should be typed (short forms, hooks, repeated riff, chord loop, 4/8-bar sections), while local generation retains control over melody, rhythmic phrasing, physical energy, spectral strategy, and style-specific dressing. A lexicon cannot certify "sounds like Feels" by labels alone.

**Distinct Homosapians dialects, one source/causality architecture:**
- Rock / Foo Fighters + Pearl Jam: strummed/muted electric guitar body, bass/drums form the backbone, dynamics & amp/room field, exposed lead over a self-sufficient rhythm-section song.
- Lean funk-pop / Feels + Sabrina: plucked/muted rhythmic guitar/keys, bass as melodic groove carrier, selective pads, tightly interlocked rhythm with intentional holes and lively lead.
- Piano/bass pop-soul / Charlie Puth: piano/EP chord duration/voicings, rich yet restrained bass and percussion conversation, no rock distortion imperative.

Generated hooks and parts should be ORIGINAL. References define relational targets, not recipes for duplicating recordings.

---

## 6. Roadmap: source-first, physical-first vertical integration

**R0 — INTEGRITY + NOTE FORENSICS (FIRST, blocking).** Verify all known C137 hashes and the new Rust audit; external held-out Score harmony checker over both Circle/Lift, all roles and actual chord spans. Annotate tension/deferred closure honestly; investigate specific G/G♯ interference, at least one valid intentional #9 positive control and an accidental clash negative. Add the existing acoustic script as a required pinned GitHub CI job; note cross-platform determinism scope. Stop on any historical drift.

**R1 — THE BACKING LOCK, band-only.** Build an ORIGINAL 8-bar band-only mini-song whose riff/harmony, rise/turnaround and timing work without any lead (not a four-bar vamp repeated twice). Preserve source identity and a complete BackingContract. Bass, drums, one guitar/keyboard line and selective harmonic support have their own mutual musical obligations. Add Lead later by source-compatible insertion. A/B against current lead-free groove and PR #118 song.

**R2 — MINIMAL MEATSACK GUITAR + PIANO GESTURES.** New opt-in isolated Rust modules, no global Rewrite. One Strum event -> several bounded ordered string contacts; per-string state + ring/damper/pick attack; instrument body/amp. Positive/negative tests on shared identity, infeasible fingering, ghost note prevention, string tail and pick order. A small piano key-on/key-off/sustain pedal experiment demonstrates that a held ringing chord differs from six independent Note gates.

**R3 — FM2/4 AS A PARALLEL DSP WORKSTREAM.** Characterize current FM+extra-sine legacy; pure 2/4-op graph with per-operator ADSR, routed modulation and robust bounded feedback. Introduce original modern electric-piano/bass/lead timbral programs. Test spectra, audible evolution, edge-trigger/click behavior, sample-rate/blocks. Do not claim exact Genesis/OPL emulation until chip semantics are tested.

**R4 — SHARED ENSEMBLE FIELD, UNDER ABOLITION CONTROL.** Compare no group effects, existing group chain, and new source-/instrument-aware band bus. Shared room and resonant body, appropriate amp/cab, group saturation, dynamic coupling of rhythmic envelopes, stereo/masking constraints. Keep band group and lead group observably separate. Measure post-bus interaction without mistaking severe masking for quality.

**R5 — COLLOQUIAL GRAMMAR + THREE DIALECTS.** Compile typed performance verbs into actual source/gesture/timbre/field actions; each needs audible witness. Demonstrate rock, funk-pop and piano pop-soul with one safe structural band/lead source and distinct legal rig treatments. Macro simplicity is PRESERVED.

**R6 — 16/32-BAR REAL SONG + HOLDOUTS.** Build from the ear-approved 8-bar band/lead vertical slice, then full dynamic verse/hook/bridge/return, original riffs, source memory, timbral coalescence. Hostile new seeds, modes, tempos, instrument staffing, MIDI-like density, timbral saturation/aliasing limits, stem masks, new sample-rate and CPU/latency regimes. Stable song/meaning and score across palette changes where declared; acoustic difference expected. Deliver full WAV+stems, Score, gesture/source receipts, FM program IDs, scene-field settings and blind loudness-matched ear packet.

**Stop immediately** on accepted C137 drift, notes being secretly snapped to avoid dissonance, incorrectly claimed harmonic closure, physically impossible gestures silently emitted, active flags with zero witness, memory/CPU blowups, unstable noise feedback, lost string tails, masking destroying instrumental distinction, or multiple workers racing kernel/synth source writes. No merge, default switch, public ABI update, release or sample-library dependency without owner permission.

---

## 7. Fan-out contract (children independent, one principal integrator)

**Principal:** owns exact source head, integration branch, shared kernel/synth collision points, CI + ear ledger. Fan out only unrelated proofs/assets into isolated branches and merge only after review.

1. **HARMONIC PROSECUTOR**: fresh Circle/Lift × both sections × sounding Lead/Keys/Pad/Bass chord witness, distinguish warranted tension and harmonic/narrative resolution; note mutation tests. No DSP.
2. **MEATSACK/STRING ENGINEER**: minimal strum-to-correlated-contacts, string waveguide, damper/let-ring, simple guitar/piano sustain, physical infeasibility and ear A/B. Own new physical modules, not kernel.
3. **FM INSTRUMENT ENGINEER**: legacy FM truth, per-op envelopes, compiled 2/4 operator programs, feedback/alias/lifetime controls, original patches and separate program IDs. Own DSP FM module, not whole MusicWorld.
4. **ENSEMBLE ACOUSTICS / MIX ENGINEER**: instrument/field shared-mix diagnostics with three-way negative controls, masking, room/pan/group dynamics, real stems, no new whole-composition changes.
5. **COLLOQUIAL COMPOSER**: expressive vocabulary with typed meaning, band-only 8-bar original mini-song, optional lead insertion, macro pop grammar + rock/funk/soul styles. Own examples, source/domain tests, not synth.
6. **HOSTILE QA**: add deterministic CI acoustic gate, negative mutations, CPU/polyphony bounds, post-bus and solo-stem audibility tests, documentation/claim audits. Never update goldens.

**Every child must return:** pinned parent SHA, hypothesis, exact files owned, independent tests (red-first controls), commands/output/CI links, artifacts/hashes, commit SHA, limitation, surprise/counterexample, source/physical/PCM claim status and pending ear verdict. Shared agent assumptions are not independent evidence.

### First non-negotiable acoustic demonstration

**The Homosapians — Band Lock/Lead Key A/B:**
- An 8-bar fully authored original **band-only** riff/groove/harmony mini-song.
- One actual procedural strummed guitar-like part with string contacts and reasonable sustain, plus bass + drums sharing a space.
- The same backing source with an independent source-licensed lead inserted.
- Complete controlled 2x2: lead absent/present × legacy physical/band realization, with at least one comparison versus new guitar/ensemble realization. Extend into both rock and light funk-pop if budget permits.
- Document how much improvement came from notes, gesture, timbre or group field, not from loudness.

**The goal is not more notes or simply four FM operators. It is one living, musically intelligent ensemble with independently recognizable members, expressing unusual meaning inside familiar pop constraints.**
