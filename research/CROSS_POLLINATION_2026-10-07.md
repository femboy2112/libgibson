# Cross-pollination note — 2026-10-07

**Status:** design advice for the v0.5/HumanMusic research line; no implementation claim.  
**Branch basis:** `feat/v0.5-auditory-observer`.

This note imports three sibling ideas because they attack current HumanMusic failure modes at different layers. They should remain separate.

## 1. Figurative Engine: a “licensed violation” needs a recovery witness

Figurative Engine's strongest transferable idea is structural, not comedic: an apparent violation is only acceptable when there is an explicit witness showing what invariant was preserved and how the deviation resolves.

HumanMusic has the exact musical analogue.

A chromatic note, delayed attack, displaced accent, anticipatory bass note, or reharmonizing color tone should not be judged solely by “is this note in the chord?” The useful question is:

> **What licenses the deviation, and where is the musical debt discharged?**

A future structure might look like:

`TensionWitness`
- stable context / expected relation;
- deviating event;
- role: passing, neighbor, anticipation, suspension, approach, enclosure, alteration, etc.;
- temporal span over which it is allowed;
- target/resolution event;
- voice-leading distance;
- whether the resolution is actually audible under the current observer.

This is deliberately narrower than “jazz says there are no wrong notes.” A sustained chromatic pitch at slow tempo without a lawful role should fail. The same pitch as a short approach note may pass.

### Critical control

Use the existing slow-BPM failure regime. Stretch a legal passing figure until the temporal relation is no longer perceived as a passing event. The witness should cease to validate even though the pitch classes are unchanged.

That would make timing **load-bearing** rather than cosmetic.

## 2. ProjectMarcus: keep score, actuation, physics, and hearing as different objects

ProjectMarcus v2 has adopted a useful decomposition:

[
	ext{symbolic score}
	o
	ext{coordinated gesture}
	o
	ext{physical actuator state}
	o
	ext{acoustic signal}.
]

HumanMusic should use the same discipline for instruments without importing Marcus's speech-specific machinery.

For a guitar note, for example, “note = E4 at velocity 80” is under-specified. The audible event also depends on a primitive layer above the abstract note:

- pick/finger mechanism;
- attack location;
- attack speed and angle;
- string displacement;
- muting/damping;
- slide/bend trajectory;
- fret/finger contact;
- sympathetic excitation;
- amplifier/instrument state.

The key architectural move is to retain a **PerformanceGesture** or equivalent between musical intent and synthesis. Instrument identity should constrain lawful realizations of that gesture; it should not be a late timbre preset.

This is especially important for bass slides and attacks, where temporal mass is part of the phrase.

## 3. SmartLang: the auditory observer must preserve rival heard-objects

The new auditory-observer design correctly separates PCM from heard music. SmartLang adds one more useful rule:

> **A preferred interpretation must not erase structurally compatible rivals.**

Instead of producing only one “detected onset/pitch/object”, an early observer should be able to expose an `AuditoryHypothesisSet` when the evidence is genuinely ambiguous.

Examples:

- one pitched event vs two partially masked events;
- note onset vs noise transient;
- harmonic partial vs independent upper voice;
- bass slide target vs continuous pitch motion.

A downstream musical prior may rank the hypotheses. It must not rewrite ambiguity into certainty.

This is likely important for Reverse HumanMusic later, but it also matters now: HumanMusic synthesis should not optimize against an observer that collapses difficult percepts by construction.

## 4. Cross-layer rule

The three imports line up without collapsing:

[
	ext{musical intent}
	o
	ext{performance gesture}
	o
	ext{physical synthesis}
	o
	ext{auditory hypothesis set}
	o
	ext{musical interpretation}.
]

- Figurative Engine contributes **violation/recovery witnesses**.
- ProjectMarcus contributes **mechanistic actuation layers**.
- SmartLang contributes **ambiguity preservation**.
- HumanMusic keeps ownership of musical grammar, arrangement, pocket, and style.

## 5. Suggested first implementation slice

Do not build a general instrument ontology first.

Take one known failure class—slow exposed chromatic bass motion—and add only enough machinery to distinguish:

1. diatonic stable tone;
2. brief chromatic passing tone with explicit resolution;
3. sustained unlicensed chromatic tone;
4. slide whose physical pitch path crosses chromatic territory but whose heard object is one continuous gesture.

Predeclare expected outcomes and add mutation controls that preserve notes but alter duration/gesture.

If this battery cannot distinguish those four cases, more musical vocabulary will not fix the abstraction.
