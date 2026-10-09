# HumanMusic — MeatsackProjection

> **Status: RESEARCH DESIGN ONLY. Not implemented, not merged, not scheduled for automatic integration.**
>
> Branch: **research/humanmusic-meatsack-projection**; independent parent **main @ 80706c91b88e59495b131bfd8ab23aa3ed013414** (v0.4.0 baseline). Designed while Claude's C137 HumanMusic work was continuing separately; latest observed C137 head at initial inspection: **0c7314fc176f5338336c0a2c2e82051f6c1d23d7** (moving target). Refresh the remote before implementing.
>
> Read with [validation and falsifiers](MEATSACK_PROJECTION_VALIDATION.md) and [future Claude integration handoff](MEATSACK_PROJECTION_HANDOFF.md). **This branch must not modify the active HumanMusic PR or main.**

## 1. Thesis

Human expressivity is **not** a mathematically perfect score with independent onset/velocity noise appended. It is the observable effect of **musical intention passing through a particular, finite, asymmetric performer and instrument**. One musical gesture may produce several related acoustic excitations, and some requested gestures are physically impossible for a declared performer rig.

The goal is not a full anatomical simulator or an automatic musicality machine. The goal is the **smallest empirically falsifiable physical-gesture model** that produces audible, correlated, instrument-specific asymmetries while preserving song identity, shared pocket, coherent phrasing and C137's band narrative.

The score is an **authoritative musical coordinate**, not an exact physical excitation schedule. Some actual attacks can land exactly on its grid. Neither mandatory snapping nor mandatory off-grid displacement is permitted.

## 2. Authorities: preserve the entire upstream causal chain

Conceptual architecture (implementation boundary still to be earned):

```text
SemanticTrace / MeaningPlan / SongMap
          |
          v
Discourse, ensemble narrative, PerformancePlan
          |       who says what; theme, harmony, obligations, shared pocket
          v
Authored Score + explicit intended gesture groupings
          |       source musical identities and required/optional distinctions
          v
GrooveTransport + FeelTransport (declared musical timing)
          |       macro pocket, band/phrase/player lean
          v
MeatsackProjection(PerformerRig, InstrumentRig, performer state)
          |       physical feasibility, assignment, motion, articulation
          v
PhysicalExecution {
    excitations, continuous controls, assignments, residuals, receipt
}
          |
          v
Instrument-specific acoustic realization or existing synth patch (as supported)
          |
          v
PCM -> downstream observer
```

**Source authority:** Meatsack does NOT invent form, harmony, melody, motif, narrative carrier, chord identity, rhythmic intention, cadence, or an extra independent swing clock. Those belong upstream. A narrative carrier can change instruments without changing the musical motif.

**World authority:** `MusicWorld` and `instrument::Patch` select *sonic patches/production*, not bodies. `Role::{Lead,Bass,Keys,Pad}` is a musical role, not a guitar string, pianist hand, or brass embouchure. A separate, explicitly assigned performer/instrument rig is necessary. Never infer human physical identity from a patch name or role.

**Acoustic authority:** A gesture-planning receipt is not a physically accurate string, horn or drum synthesizer. The first experiments may reschedule existing synth note events and use bounded attack/velocity controls; call that **gesture-realization evidence**, not a full physical instrument model. Later continuous controls need explicit synthesizer support.

## 3. Formal candidate: a partial realization, not a mandatory postprocessing filter

For a declared physical configuration `K` (performer effectors, instrument targets and technique constraints), take the space `G` of **intended gestures** and the space `E` of physical excitation/control events.

    P_K : G ⇀ E

This map is **partial**: some intentions are infeasible for that specific rig. A six-string guitar, one guitarist with a pick, a four-limb kit drummer and a drum machine have different `P_K` even if they receive the same song-level intention.

More accurately for sequential gestures, implementation must consider finite performer state:

    (q_next, execution) = P_K(q_now, intended_gesture, declared_window)

where `q` contains **only sufficient physical state**: currently occupied resources, hand/string/fret positions, pedal states, technique, accumulated breath capacity, and recovery phase, depending on rig. There is no universal full-body `q`.

**Composition/musical identity law:** quotienting the output by its explicit source-gesture links should recover the **licensed** intended musical identities, except where a typed infeasibility/declared degradation says otherwise. The output must not secretly rewrite a chord, hit, motif, or performer assignment to make the receipt pass.

**No phantom resources law:** a human drummer does not suddenly acquire a third stick hand. The model may refuse, choose a different explicitly licensed technique, allocate a permitted near-simultaneous spread, or ask the upstream planner to revise. It may not silently invent a limb.

**No anatomical overclaim:** "feasible under this low-dimensional rig model" does not establish physical feasibility for every human, or that a given person could actually perform it at the declared tempo. Rig parameters require later calibration.

## 4. Intention and excitation are different object types

A musical chord may be *one* intended action and *six* realized string contacts. They must not become six independently composed events.

Candidate **internal** structures (not yet a frozen public API):

```rust
struct GestureIntent {
    id: GestureId,                 // stable identity, never inferred from changed onset
    source: MusicalSourceRef,      // SongMap / Action / Material / Score event(s)
    intended_anchor: MetricOrBeat, // original, immutable
    window: PerformanceWindow,
    required: Requirement,
    kind: MusicalGesture,          // Strum, PluckSet, Chord, Flam, Roll, ...
    participants: Vec<SourceEventId>,
}

struct PhysicalExecution {
    excitations: Vec<Excitation>, // note/sample attacks and strike/contact data
    controls: Vec<ControlEvent>,  // pedal, mute, bow, breath, bend, openness, ...
    mapping: Vec<GestureWitness>, // source gesture -> effectors/targets/events
    receipt: PhysicalReceipt,
}
```

Names and ownership here are proposals. **Do not introduce these exact types unless the first prototype earns them.** In particular, prefer a small sidecar/source mapping over changing historical `Note`/`DrumHit` struct literals everywhere.

Every generated excitation must carry a **stable source identity**. Existing `VoiceEventId` incorporates original onset/pitch/role/provenance and is intended for explicit voice continuity. Re-keying that identity by the *modified* attack time is dangerous: a physical reschedule must not orphan a continuity edge or make a reader mistake an event for a different composition.

**Grouping is explicit:** do not reconstruct Strum intent merely because four notes start at the same time. A piano chord, an arpeggio, a layered multi-track cue and an electronic stab can share a timestamp but demand different physical actions. If metadata is absent, the appropriate result is **Unknown / Unspecified**, not a fabricated guitar.

## 5. Hierarchical expressivity without global temporal chaos

For excitation `e` with authored beat `b`:

    t_e = T(b)
        + band_phrase_lean(b)
        + player_phrase_lean(b)
        + gesture_contact_offset(e, q)
        + optional_bounded_motor_residual(e, q)

Here `T` is the declared metric-to-time + existing groove transport. The band/player terms must be computed **once** in the musical feel family. Meatsack owns only the **gesture/physical** terms. If the upstream event is already feel-transformed, never add the same lean again.

**Distinct scales:**

1. **Section:** intended narrative force, player-role distribution and long-range energy movement (from SongMap/Discourse/Ensemble Narrative).
2. **Phrase:** correlated breath, arrival preparation and bounded shared/player phase bend (`FeelTransport` or equivalent).
3. **Beat/pocket:** a single common declared groove field; intentionally aligned notes remain possible.
4. **Gesture:** strum contact order, rebound strokes, finger attacks, key depression, tongue articulation.
5. **Acoustic:** excitation envelope, continuous pitch, resonance, damping and timbral variation.

The scales are **causally nested, not five independent randomizers**.

A possible phrase-time warp is `W(t)=t+φ(t)`, with selected structural anchors `a,b` satisfying `φ(a)=φ(b)=0` and `1+φ'(t)>0`. This is an **illustrative sufficient model**, not a mandate to snap every barline. It preserves temporal order and bounds net phrase drift. Actual physical strums can spread a logically simultaneous chord into sequential contacts; that distinction is part of gesture realization, not an unlawful phrase warp.

**No forced imperfections:** exactly-on-beat, repeated, intentional machine-like precision is permitted when the musician/musical style calls for it. Motor residual may be identically zero. If added, it should be small, seeded, conditional on persistent performer state, and demonstrably *not* independent white noise.

**Anti-self-healing law:** the physical projection must not iteratively rewrite the entire Score in response to a synthetic music-quality scalar. A bounded deterministic source-to-execution planning pass is allowed; irreversible/explicitly checkpointed local state advances forward. Infeasibility is surfaced as a witness or a new explicit upstream proposal, **not silently compensated**.

## 6. Local asymmetry, physical memory, and energy

Declare a persistent performer signature `θ_K` (e.g. dominant hand, preferred fingerings, stroke characteristics, comfortable reach) and a small evolving state `q(t)`. The same performer should sound consistently like the same *kind* of performer while changing gesture dynamics with context.

Correlations matter: a hard snare stroke can change rebound, feasible next stroke and subsequent attack character *through one causal transition*; it is not three unrelated random draws. The right and left hand need not have identical dynamics. Piano fingers need not depress keys identically. Guitar upstrokes and downstrokes need not yield the same contact ordering or attack spectrum.

The **narrative/performance plan** allocates musical energy and prominence among the musicians. Meatsack turns a local intended effort into physical control (pick speed/force, key attack, stick velocity, breath support), including possible energy-linked articulation and spectral changes. **Loudness/energy is not literally conserved across the band**; there is no invisible master feedback loop that automatically lowers every other player when one strikes harder. Any compensating response must be planned or causally modeled.

Physical controls can evolve **without a fresh note attack**: a guitarist bends or damps a vibrating string, a drummer changes hi-hat openness, a pianist depresses/relinquishes the sustain pedal, a horn player shapes a held tone with breath.

## 7. Instrument rig A — fretted strings (first-wave prototype)

Minimal necessary objects:

- **Strings:** finite strings with tunings and an explicit active vibration/contact state; alternative tunings permitted.
- **Fretting hand:** available fretting effectors (normally four fingers plus optional thumb-over-neck technique), fret/string reach, allowed barre contacts and hand-position changes. A barre is one finger performing multiple contacts under a declared feasibility rule.
- **Picking hand:** pick or fingerstyle; pluck resources, traversal direction, reachable string subset, attack force/speed and damping.
- **Technique:** downstroke, upstroke, fingerpicked simultaneous/near-simultaneous set, muted/percussive contact, slide/bend *if supported*.
- **Transition state:** finite movement from one shape to the next; notes tied to the same string cannot always sustain independently without explicit technique.

**One chord, three realizations:** same authored chord identity, same overall intended landing, under downstroke vs upstroke vs fingerpicked execution. Attack order and correlations differ; source identity does not.

A sweeping pick crosses strings **in a definite order over a finite time**, not all at the same beat and not each string with independent random delay. Muted/unstruck strings must be declared. Some fingerings are infeasible; no guitar fretboard SAT solver is required in v0.

**John Mayer thumb case:** optional thumb-over-neck constraint is a legitimate technique variant, not a universal 5th fretting finger. Do not assume either that every player can do it or that nobody can.

## 8. Instrument rig B — drummer (first-wave prototype)

A conventional acoustic player typically has two hands and two feet; **do not hardcode this into the universal model**. Some players have different ability/equipment, alternative grips, open-handed setups, auxiliary/double pedals, or a cooperating second drummer. A drum machine has no human-limb budget.

Minimal drummer rig:

- Configurable **effectors**: right/left stick hands; right/left pedal feet; each independently identified, but with coupled coordination constraints where earned.
- **Physical targets**: snare, hats, ride, toms, cymbals, kick and pedal controls; kit placement / travel-edge graph.
- **Technique families**: single strokes, alternating sticking, controlled double/rebound, flam, drag/roll if earned, cross-stick, choke/hat articulation.
- **State**: current surface, recovery interval, pedal/hat openness, feasible stroke path, handedness/preferences.
- **Required vs optional** strikes: required backbeat/structural anchor cannot silently vanish; optional ornaments may be withdrawn *only when the upstream gesture contract licenses it*.

Feasibility includes both **simultaneous resource allocation** and **movement/recovery**. A human with two stick hands cannot execute four distinct simultaneous stick strikes using standard one-stick-per-hand technique. Kick+snare+hat is ordinary and must pass. Foot-controlled hi-hat openness may occur **between** stick attacks and affects the subsequent sound.

A flam is **one coordinated gesture** producing a grace and primary stroke, not two unrelated perturbed snare notes. A double-stroke shares hand/rebound state. A stick crossing the kit has travel cost: no teleporting from ride to far floor tom.

The planner must distinguish a drum-machine groove from an acoustic-kit performance, rather than deciding one is "less musical" by fiat.

## 9. Instrument rig C — keyboard (second-wave)

Minimum sufficient model: two hands with independently declared available fingers/reach, keyboard regions, feasible hand repositioning, overlapping key presses, legato handoff, optional pedal controls. A chord played as a rolled chord, blocked chord or staggered hand attack has a **single harmonic intention but different excitation geometry**. Pedal controls change damping/sustain without requiring retrigger.

Do not assume a particular keyboard sound from the fact that the performer uses keys; physical access is independent of `MusicWorld.keys`.

## 10. Instrument rig D — wind and voice-like air control (second-wave)

Minimum sufficient model: breath/air-support reservoir, tongue/attack state, embouchure/valve/fingering transition cost, phrase-length limits, and continuous support/pressure trajectories. Distinguish tongued attacks, slurs and held swells; some sound changes **do not require another attack**.

This is a low-dimensional gesture constraint model, **not** a claim to full lung/tract acoustics or a physiological theory of singing. Instrument-specific response maps require separate evidence.

## 11. Future extensions and deliberate non-goals

Potential later families: bowed strings (bow direction/speed/pressure, string crossing), percussion mallets, fretted bass, organ pedals, multiple musicians playing one part, extended techniques, disability/adaptive rigs, expressive controllers and human/electronic hybrids.

**Non-goals for the first implementation:**

- skeletal/anatomical simulation; musculoskeletal inverse kinematics; an entire mathematical hand;
- universal "human quality" score or automatic aesthetic optimizer;
- arbitrary role->instrument inference from patch timbre;
- invisible post-Score repairs; silent loss of required notes; a second songwriting engine;
- independent onset/velocity jitter, five separately drifting clocks, forced off-grid attacks;
- complete physically modeled strings, piano soundboard, lips or brass bore in the same round;
- altering C137 narrative behavior, Source Evidence, v0.4 test pins or default audio paths;
- automatically cherry-picking/merging this research branch into the active C137 branch.

## 12. Engineering consequences of the actual v0.4 source

At this research branch's parent:

- `src/audio/human_music/score.rs`: `Note` and `DrumHit` encode musical notes/hits and provenance. `StrokeOrigin` records metric vs performed drum coordinate, but no human limb. This precedent should inform physical receipts, not be conflated with them.
- `src/audio/human_music/voice.rs`: the source explicitly states that `Role` cannot tell us whether two notes use the same string. `VoiceContinuation` is a source-owned edge; physical re-timing must not corrupt it.
- `src/audio/human_music/rhythm.rs`: `MetricPosition -> GrooveTransport -> FeelTransport -> PerformedPosition` is already named; `FeelTransport::Identity` was the base state. Re-inspect after C137 completes.
- `src/audio/human_music/synth.rs`: `HumanMusicSynth` maps Score note/hit times through the tempo map to sample-accurate scheduling and allocates synthetic voices. Use its existing patch renderer as a **control**; no body can be deduced from a synth voice.
- `src/audio/human_music/world.rs` and `instrument.rs`: `MusicWorld`/`Patch` are timbre and production choices, not performer rigs.
- `src/audio/time.rs`: integer sample positions are the authoritative render clock. Physical gesture timings should be converted to `SampleTime` at the declared boundary, not accumulated with frame-by-frame floating-point wall time.
- `src/audio/perception/` on C137: the diagnostic auditory observer is downstream of PCM, never the source of physical intentionality.

**Important:** these are provenance observations as of the recorded parent/inspection, not a claim that future C137 integration code has the same public API.

## 13. The actual research contract

The first project is not "humanize every instrument." It is:

1. **Find the smallest shared partial-projection contract** that supports explicit source gestures, finite effectors, controlled local physical memory, excitations, continuous controls, typed infeasibility and independent receipts.
2. Implement a **sequencer identity control** (no physical processing) and two instrument-specific falsifiers: drummer and fretted strings.
3. Compare the same intended material under rigid electronic, independent-jitter negative control, physical time-only and physical time+control realization.
4. Verify structural identities, resource feasibility, exact clock conversion, determinism, boundedness, provenance and archived byte equality.
5. **Listen blind**. Machine checks can reject; only listener judgment can accept a useful physical realization.
6. Add keyboard/breath control **only after** the two first-wave models prove that the shared contract generalizes.

Formal, implementation and listening evidence gates are in [MEATSACK_PROJECTION_VALIDATION.md](MEATSACK_PROJECTION_VALIDATION.md). The deliberate future branch isolation and Claude handoff are in [MEATSACK_PROJECTION_HANDOFF.md](MEATSACK_PROJECTION_HANDOFF.md).

**Standing principle:** *musical meaning is preserved, but physically embodied musical execution need not be quantized. Variance is correlated consequence, not stochastic decoration. The instrument is allowed to say "I cannot perform that."*
