# HumanMusic Round XIII — temporal mass

## Scope and authority

Started at `d72b72f1d4d4c670f3ac2f2a5a3ccd808051d911` on `feat/v0.4-humanmusic-audio`;
main/base `7b4efdc4c7fc6523d890a28f2aa8e27a6ba59286`. PR #70 stays draft. No package, ABI
(`GIBSON_ABI_VERSION = 1`), MSRV (1.85) or release change. Audio stays Rust-only.

The one obligation: **pitch justification depends on temporal-experiential mass.** A fleeting
chromatic event and the same pitch held long enough to become a statement are not equivalent.
Greater mass demands stronger ownership. Receipts are in
[fixtures/humanmusic-r13](fixtures/humanmusic-r13).

## What the signal said before any model

The maintainer located two remaining SWISS defects at ~15–17 s and ~43–45 s. The Round XII
audit rows there are keys "holds" of D5 over C6: 1.94 beats at beat 32 and 3.90 beats at beat 88.
Their written lengths are 0.99 s and 1.98 s, but written length is not what the ear gets.

- Both worlds' keys patches have zero sustain (SWISS `(0.002, 0.18, 0.0, 0.1)`).
- Round VIII's `audible_end` puts every SWISS keys note at 0.137 s above the 30 dB floor,
  whatever its written length.
- An independent solo-stem Goertzel measurement agrees: the "hold" at 16.30 s falls from
  −7.1 dB to −55.7 dB in 0.20 s, the same decay as the 0.45-beat stab at 15.65 s.
- Velocities do not separate them either: hold 0.2008 / 0.2362 vs stab 0.2231.

What *does* separate the two events is timing. They are the only keys D5 attacks struck with
the harmony's own articulation (the C6 arrivals), fused with the pad's re-attacked `E4 A4 C5 G5`
and the bass root. The pad's C5 sits a whole step below the D5. All 29 other keys D5 attacks
are syncopated between arrivals. Onset synchrony is how the auditory system groups simultaneous
attacks into one object, so a colour struck with the chord's attack is part of the chord that
attack declares. The model measures that, not written duration. Whether these are the exact
events the maintainer heard is still a listening question: see the other events in the same
windows below.

## TemporalMass

For each pitched note:

| Field | Unit | Definition |
|---|---|---|
| `written_beats` / `written_secs` | beats / s | written length at `score.tempo_bpm` |
| `audible_secs` | s | `sonority::audible_end` on the role's world patch; the tail is cut at the same role's next attack at or after the written end |
| `recurrence_secs` | s | audible time of earlier same-role, same-pitch-class attacks within 2 s |
| `accent` | Weak / MidBar / Downbeat | 4/4 metric position |
| `fused` | bool | pad/keys/bass onset within 30 ms of its harmony's articulation |
| `mass` | weighted s | `(audible + recurrence) × salience` (1.0 / 1.25 / 1.5) |
| `class` | Fleeting / Gestural / Asserted | Asserted if fused or mass ≥ 0.75; Fleeting if mass < 0.20 |

**Mathematical invariants** (tested):

- Mass is non-decreasing in audible time, recurrence and accent.
- Class is non-decreasing in mass and in fusion.
- Audible time is non-decreasing in written time on a sustaining patch.
- A percussive patch's audible time is invariant under tempo and written length.
- The bands are ordered (a compile-time assertion).

**Calibrated heuristics** (named, each with a witness test):

| Constant | Value | Meaning |
|---|---:|---|
| `FLEETING_MAX_SECS` | 0.20 | five or more events a second: a gesture member |
| `ASSERTED_MIN_SECS` | 0.75 | heard as a harmonic statement; a weak-beat quarter at 88 BPM (0.68 s) stays gestural |
| `RECALL_WINDOW_SECS` | 2.0 | an earlier attack of the same colour is still in the ear |
| `FUSION_ONSET_SECS` | 0.03 | attacked with the harmony |
| `DOWNBEAT_SALIENCE`, `MIDBAR_SALIENCE` | 1.5, 1.25 | 4/4 metric hierarchy |

Velocity is recorded, not weighted. No sample simulation. No enjoyment score. Mass is evidence,
never a quality to minimise: tonics and suspensions are heavy too.

**Ownership vs demand.** The overlay (`mass::MassDiagnostics`) reads the unchanged Round XII
observer's reconstructed support. A fleeting note is carried by palette licence or gesture
membership. A gestural note needs a local destination: a connector into a written chord tone,
or a colour kept as a common tone or resolving by step. An asserted note needs harmonic
ownership: written in the sounding harmony, a physical suspension or retardation, an
appoggiatura, an anticipation, a pedal, or a selected colour.

The failures have names:

- `UnownedAssertedColor`: an asserted colour nobody wrote. In a support role it is counted as
  an accidental reharmonization.
- `OverdrawnGesture`: a non-harmonic tone exposed beyond what its slide or connector can carry.
- `SoftOrphan`: a gestural orphan colour, still Round XII's soft suspect.

## Tempo proof

The same beat geometry, masked by a following attack, on SWISS's bass patch:

| Event | 160 BPM | 118 BPM | 88 BPM |
|---|---|---|---|
| half-beat, weak | 0.188 s → 0.188 Fleeting | 0.254 → 0.254 Gestural | 0.341 → 0.341 Gestural |
| one beat, mid-bar | 0.375 → 0.469 Gestural | 0.508 → 0.636 Gestural | 0.682 → 0.852 **Asserted** |
| one beat, weak | 0.375 → 0.375 Gestural | 0.508 → 0.508 Gestural | 0.682 → 0.682 Gestural |

Exposure rises strictly as the tempo falls, and two marginal events change class. A keys ping
at the same three tempos, or held for 3.9 beats, stays 0.137 s. That is the physical control:
tempo matters only where the sound actually lasts.

## Fast slide vs slow staircase

The same C–C#–D–D#–E into a written E over C major, on the bass:

- **Fast** (1/4 beat, 118 BPM): every intermediate is Fleeting and carried by the gesture.
- **Slow** (1 beat, 118 BPM):
  - C# is `OverdrawnGesture`: it is heard for 0.508 s and aimed at a D that is no structural
    destination.
  - D loses the slide and is only a `SoftOrphan`.
  - D# is carried by its own semitone resolution into E.
- **Slow at 88 BPM**: the D becomes an `UnownedAssertedColor`.

The Round XII observer returns one verdict for both forms. That is still pinned as a regression.

## Acceptance witnesses

**SWISS ~15–17 s.** The keys hold `E4 A4 D5` at beat 32 (16.271 s), over C6 with Cmaj7 next.

- Mass 0.617: 0.137 s audible, 0.274 s recurrence from the D5 stabs at 28.5 and 30.75,
  downbeat ×1.5.
- Fused with the C6 arrival, so Asserted. Ownership is palette licence only, so the verdict is
  `UnownedAssertedColor`.
- The offbeat stab at 30.75 has mass 0.274 and stays Gestural.
- Change: **D5 omitted**. `E4 A4` still states C6's guide tones.
- The next stab at 34.25 falls from 0.411 to 0.274, because the hold it echoed is gone.

**SWISS ~43–45 s.** The same shell at beat 88 (44.746 s): mass 0.602, fused, `UnownedAssertedColor`.

- Change: **D5 omitted**.
- "Heavier still than beat 32" is **not** reproduced. Written seconds order that way (1.983 vs
  0.986), but both are the same ping, and beat 88 has 0.010 s less recurrence.

The same windows also hold, all unchanged and all carried:

- a bass/keys/lead unison C# at beat 84.5 (42.966 s) into C: keys 0.137 s Fleeting, bass
  0.254 s Gestural, lead 0.093 s Fleeting, all Local ownership;
- the lead F4 neighbours;
- the pad's own unwritten D5 over Cmaj7 at beat 92 (46.780 s), which is flagged and omitted.

## Before / after (StablePropulsion, seed 2112, FusionConversation, default options)

| Receipt | SWISS A → B | BLACK_ICE A → B |
|---|---:|---:|
| Accidental reharmonizations (unowned asserted support colours) | 6 → 0 | 21 → 0 |
| Unowned asserted colours, all roles | 6 → 0 | 21 → 0 |
| Overdrawn gestures (slow linear events) | 0 → 0 | 2 → 2 (lead, parked) |
| Soft orphans | 20 → 20 | 15 → 13 |
| Round XII false claims / suspensions / anticipations / arrivals / tendencies | 0 → 0 each | 0 → 0 each |

| Perturbation | SWISS | BLACK_ICE |
|---|---:|---:|
| Pitched notes | 449 | 449 |
| Pitch changed | 0 | 4 |
| Duration changed | 0 | 0 |
| Omitted | 6 | 17 |
| Added | 0 | 0 |
| Changed events | 6 (1.336%) | 21 (4.677%) |

**SWISS edits:**

- keys D5 at 32, 88 and 112;
- pad D6 at 20 over Fmaj9;
- pad D5 at 92 over Cmaj7;
- pad A4 at 116 over Cmaj7.

**BLACK_ICE edits:**

- the keys B4 over Am6 at 32, 88 and 112;
- pad G over Dm9 (16, 48, 80);
- pad E/G over Dm7: E omitted, G moved to F (20, 52), E omitted (84);
- pad B over Am6: omitted (24, 56, 64); at 72 and 104 B omitted and D moved to E;
- pad F# over Am9 (36, 92, 116).

No voicing loses more than one voice; that is the Round VIII guard. The deflected controls
move 20 (SWISS, 4.96%) and 25 (BLACK_ICE, 6.23%) events.

Lead, bass, drums, SFX, chords, performance plan and SongMap are unchanged. A test asserts
all of these. SongMap fingerprints stay `0xaf5360442b9b6729` / `0x8d901e3de29190a2`, and
SongMapConformance passes with 6 identity sites and 15 landmarks. The Round XI phenomenal
trajectories are byte-identical to the Round XII receipts.

## Generation

`functor::perform_mass` is Round XII's `perform_temporal` plus `comp::gate_support_mass` on the
pad/keys voicings. The gate's rule is authorship of the pitch class. An asserted voicing colour
that is not written in the sounding harmony gets the first of these that works:

1. shortened in grid steps, if that alone ends the assertion;
2. omitted, at most once per voicing, while a guide tone remains;
3. moved to the nearest unsounded written chord tone.

The gate returns its own ledger; the lab checks that its counts equal an independent score
diff. `perform` (R11) and `perform_temporal` (R12) are byte-identical: four R12 score hashes are
pinned, and the A WAVs equal the Round XII WAVs byte for byte.

**Bass (Commit 4) was not implicated, so it was not changed.** Every bass approach in both
scores is a single 0.45-beat semitone approach into a written root, not a staircase. It sounds
for 0.254 s (SWISS) or 0.341 s (BLACK_ICE): Gestural, Local ownership, Carried. Zero bass rows
are non-carried in all four scores. The slow-staircase control proves the audit would catch one.

**Lead: nothing high-mass.** BLACK_ICE's lead C#5 over Am6 at beats 33 and 41 is 0.341 s,
Gestural and overdrawn. It is the thesis surfacing on a sustaining lead: in SWISS the same
melodic slot is a ping. It is medium mass, and the mission reserves lead edits for high-mass
defects, so it is reported and parked.

## Preserved good wrong notes (B arm, unchanged, `Carried`)

- **Fast chromatic approach:**
  - SWISS unison C# (beat 84.5) into C, above;
  - BLACK_ICE bass G#2 → A2 at beat 3.5 and 35.5: 0.341 s, Local.
- **Authored extension:** SWISS pad G5 over Fmaj9 at beat 20, the written ninth. It is
  2.034 s, fused and Asserted, owned as a written chord tone, and kept. The unwritten D6 in
  the same voicing was omitted.
- **Anticipation:** BLACK_ICE lead G5 at beat 35.5 (0.440 s, Gestural, Harmonic) and SWISS
  lead E4 at 19.5.
- **Passing colour:** SWISS keys D5 at beat 115.5, between C5 and E5: Gestural, Local. Its
  mass fell from 0.514 to 0.377 once the fused D5 at 112 was gone.
- **Suspension:** only a synthetic control. A prepared, physically held D over G7 → C on
  BLACK_ICE's sustaining lead at 88 BPM is Asserted and carried. No authentic held suspension
  is claimed in the acceptance audio.

## Audio

`cargo +1.98.1 run --release --example temporal_mass_lab -- --compare --render
--out=target/humanmusic-r13/final --around-seconds=16 --around-seconds=44`

- `target/humanmusic-r13/final/swiss_r12.wav` (byte-identical to Round XII `swiss_temporal.wav`)
- `target/humanmusic-r13/final/swiss_mass.wav`
- `target/humanmusic-r13/final/black_ice_r12.wav` (byte-identical to Round XII `black_ice_temporal.wav`)
- `target/humanmusic-r13/final/black_ice_mass.wav`

All four are 48 kHz stereo PCM16, with no nonfinite samples.

BLACK_ICE B peaks at the world's 0.97 master ceiling at 60.00 s (beat 88), against A's 0.865.
That is the downbeat whose keys B4 was omitted, and at that instant it is the only note that
changes. Mechanism (Conjectured): the B4 ping had been partially cancelling the peak sample, so
the limiter now clamps that transient. RMS is 0.1196 → 0.1193.

The `--around-seconds` receipts show every note within 1.5 s of 16 s and 44 s before and after.

## PARKED

Everything deliberately not touched:

- lead generation, including the two gestural BLACK_ICE lead C#5 overdrawn gestures;
- bass realization;
- SongMap, phenomenal regimes, MeaningPlan, CompositionGrammar, MusicWorld patches,
  FusionConversation, Stage, interaction planning, the global voicing architecture, Surgical,
  CoupledR8, synth and mix;
- the Round XII observer and its beat-domain windows (`RESOLUTION_GAP`, the slide horizon, the
  anticipation window). The mass overlay supplies time instead of rewriting them;
- `LINEAR_MAX_BEATS` and `RESOLVE_WINDOW_BEATS` in the Round VIII vertical layer;
- region-level (prolongation) ownership, and explicit colour-selection plumbing for support
  voicings;
- the limiter transient above;
- default promotion, merges, tags, releases, Round XIV.

## Human-unverified

The machine now distinguishes pitch-path ownership by perceptual exposure. Whether that
calibration matches the maintainer's ear remains a listening question. Specifically unproven:

- that the omitted D5s are what was heard at 15–17 s and 43–45 s;
- that the four other SWISS edits the maintainer did not report (10.2 s, 46.8 s, 57.0 s,
  59.0 s) are improvements rather than losses;
- that BLACK_ICE's thinner and recoloured pad is better.
