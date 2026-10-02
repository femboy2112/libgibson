# HumanMusic Round XIV — heard objects

## Scope and authority

Started at `2820ea96b7a847c20f172fa3f27dd61da2f70094` on `feat/v0.4-humanmusic-audio`; main/base
`7b4efdc4c7fc6523d890a28f2aa8e27a6ba59286`. PR #70 stays draft. No package, ABI
(`GIBSON_ABI_VERSION = 1`), MSRV (1.85) or release change. Audio stays Rust-only. Receipts are in
[fixtures/humanmusic-r14](fixtures/humanmusic-r14).

Round XII (`perform_temporal`) is the listening baseline. Rounds XIII (`perform_mass`) and XIIIb
(`perform_tension`) are kept as historical experimental arms, and their gates are not promoted.
The maintainer's listen of the Round XIIIb renders found three things:

- the ~16 s chord was not fixed;
- BLACK_ICE's wrong notes were not fixed;
- SWISS had a new regression: at ~16 s the band no longer sounds the chart's Cmaj7.

This round has two obligations:

1. A realized gesture must be the gesture whose semantics justify it.
2. The band sounding at every moment must still realize the chord the chart says is there.

## Why Rounds XIII and XIIIb failed

| Round | What it measured | What it missed |
|---|---|---|
| XIII | each colour's ownership against the chord symbol | the 16 s chord has no colour in it; the keys D5 it removed was not the defect |
| XIIIb | pairs of sounding notes a semitone apart | the chord those notes belong to |

**Round XIIIb deleted the chord along with the clash.** It removed the pad's C5 from Cmaj7
`E4 B4 C5 G5` at beat 28 as "covered", because the bass sounded C at the pad's onset. The bass
then walks `E2 G2 B2` under `E4 B4 G5`. So for 15.25–16.02 s, and again at 23.72–24.41 s, the band
is E minor over its own bass while the chart says Cmaj7. The coverage proof read one instant.

**Round XIIIb rewrote the band after the band had reacted.** It shortened or re-timed 18
BLACK_ICE lead notes after the keys, the bass and the drums had realized against them.

**The Round XII observer labelled geometry as a slide.** It supported `SlidePath` for any
monotone chain of Notes landing on a chord tone. `synth.rs` triggers a fresh voice for every
Note: a new frequency, envelopes gated on, the filter reset. Nothing glides. In both acceptance
songs no note depended on that label: 49 SWISS and 52 BLACK_ICE notes carried it, each beside
another supported function. BLACK_ICE's chromatic moves are all single approach notes. The
"slowly articulated slide" the round was asked to remove does not exist in these scores.

## Two new audits (diagnostics; they drive no generation)

### Heard identity — `identity.rs`

The heard band is sliced wherever anything starts or stops. Each note's audible lifetime is
used, cut at its role's next attack. Every slice carries explicit evidence:

- the chart chord and the bass (the lowest heard pitch);
- whether the chart's root is heard, and when it was last heard (the identity memory);
- the chart's guide tones;
- a **rival**: a complete major or minor triad on another root that contains the bass, sounding
  over a pad or keys voice while no voice sounds the chart's root.

A rival held for **`IDENTITY_HOLD_SECS` = 0.5 s** flips the chart's identity. That is the only
new constant. What calibrates it:

- It flags both chords the maintainer reported:
  - Round XIIIb SWISS at 16 s: Em held 0.763 s over Cmaj7;
  - Round XII SWISS at 43 s: 1.78 s of Fmaj9 heard as A and then C, inside the 43–45 s report.
- One eighth of a walk through a rival at 118 BPM (0.25 s) stays **passing**.

The memory is the root's own audible tail, measured in seconds. The Round VIII law can't be
reused here. Its two-beat memory is clamped to the harmony's start, so a downbeat root certifies
the whole bar, and it reads the same flip beats for Round XII and Round XIIIb.

These pass:

- inversions (Cmaj7/E, Cmaj7/G);
- a root in an upper voice while the bass rests;
- a rootless voicing over the root in the bass (its release tail ringing alone after the bass
  stops is passing, not a flip);
- a pedal.

These fail:

- bass B under E G B with no C;
- the Round XIIIb root deletion under the bass walk.

`keeps_identity(before, after, a, b)` is the safety property for any support edit: the status is
no worse at any instant.

### Physical gesture — `gesture.rs`

The audit uses three words:

- `Slide`: one attack, every later pitch glided into.
- `ChromaticApproach`: two attacked pitches a semitone apart.
- `DiscreteLine`: anything else.

`entry_of(note)` is always `Attacked`, so `Slide` exists only as an abstract witness until a
renderer can play one. `GestureDiagnostics` lists every note the analysis calls a slide, on its
physical route: pitches, onsets, inter-onset intervals, lengths, attacks, and declared and
supported functions. A claim that stands on the slide alone is a **fake slide**.

### Frozen arms, as heard (`fixtures/humanmusic-r14/frozen/`)

| | R12 | R13 | R13b |
|---|---|---|---|
| SWISS identity flips | 1 (42.97–44.75 s, Fmaj9 heard as A then C) | 1 (same) | 3: the same, plus 15.25–16.02 and 23.72–24.41 s Cmaj7 heard as Em |
| BLACK_ICE identity flips | 10 (9.84 s) | 10 (9.76 s) | 10 (9.76 s) |
| fake slides / declared slides / chromatic staircases | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 |
| stale hearings (BLACK_ICE) | 0 | 0 | keys, bass and drums each heard 18 lead notes not in the final score |

BLACK_ICE's ten flips are rootless pad voicings over a bass on the 5th or 3rd:

- Am9 voiced `G4 B4 C5 E5` over E or C is heard as C or Em;
- Dm7 voiced `F4 C5 E5 G5` over A is heard as Am, for 2.05 s at 57.95 s.

## Round XIV: `perform_coherent`

This is Round XII's realization with one change of order. The pad, which no player ever reads,
is realized last among the pitched players and **hears** the lead, keys and bass. Those three,
and the drums who hear the bass, are Round XII's bytes. No post-hoc gate runs.

The pad plays Round XII's voice path with two changes, both made at the source, before a note
is emitted:

- **Spacing.** Two held members a minor 2nd or 9th apart move by octaves, the less structural
  first. Pitch classes are unchanged, so the chord is unchanged. SWISS Cmaj7 `E4 B4 C5 G5`
  becomes `E4 C5 G5 B5`, and the C stays.
- **Rooting.** Where the heard band would flip the chart's identity over a harmony the pad
  voices, the pad sounds the chart's root in place of an unwritten colour or the 5th.
  - It never gives up a guide tone, a written extension, or an upper-structure top voice (the
    witness of a pad thickening).
  - An edit is kept only if it removes flipped time there and `keeps_identity` holds over the
    whole song.

Nothing is ever removed because another player "covers" it. The `Contract` enum lost its
ordering, so `Coherent` is neither `Mass` nor `Tension`, by explicit predicate.

### SWISS ~16 s (`fixtures/humanmusic-r14/final/swiss.at-seconds.txt`)

| t | chart | R12 | R13b | R14 |
|---|---|---|---|---|
| 15.0 s | Cmaj7 | bass C2, pad E4 B4 C5 G5: rooted | bass C2, pad E4 B4 G5: rooted | bass C2, pad **E4 C5 G5 B5**: rooted |
| 15.5 s | Cmaj7 | bass E2, pad E4 B4 C5 G5: rooted | bass E2, pad E4 B4 G5: **FLIPPED (Em)** | bass E2, pad E4 C5 G5 B5: rooted |
| 16.0 s | Cmaj7 | bass B2, pad E4 B4 C5 G5: rooted | bass B2, pad E4 B4 G5: **FLIPPED (Em)** | bass B2, pad E4 C5 G5 B5: rooted |
| 16.5 s | C6 | bass C2, pad E4 A4 C5 G5: rooted | same | same |
| 17.0 s | C6 | same | same | same |

No keys or lead note is heard at these five instants. The keys pings (audible about 0.137 s)
at beats 30.5, 30.75, 31.5, 31.75 and 32 fall between them, and they are Round XII's in both arms.
The full slices are in the receipt.

The other Cmaj7 spans are in `swiss.chart-spans.txt`:

- beats 36 and 44 are the same voicing and get the same spacing;
- beat 116 `B3 A4 C5 E5` (a minor 9th) becomes `A4 C5 E5 B5`;
- beats 4, 12, 60, 68, 76, 100 and 108 (`C4 G4 B4 E5`) and beat 92 are unchanged;
- every Cmaj7 span stays rooted or passing in Round XIV.

The Fmaj9 at beat 84 becomes `F4 A4 E5 G5`, which ends the 42.97–44.75 s flip.

### Perturbation against Round XII (`*.perturbation.txt`, an independent diff checked against the ledger)

| | SWISS | BLACK_ICE |
|---|---|---|
| notes pitch-changed | 5 | 24 |
| notes duration-changed | 0 | 0 |
| notes omitted / added | 0 / 0 | 0 / 0 |
| note count | 449 → 449 | 449 → 449 |
| pitched events affected | 1.114% | 5.345% |
| pad voicings changed | 5 (4 spacings, 1 rooting) | 20 (14 spacings, 10 rootings) |
| lead, keys, bass, drums, SFX | identical | identical |
| identity flips | 1 → 0 | 10 → 0 |

Also unchanged from Round XII: the plan fingerprint, SongMap conformance, every interaction
receipt, and the Round XII false-claim counts.

BLACK_ICE changes more than SWISS only through these two pad mechanisms. No lead, keys or bass
note moved.

### Gesture semantics

The Round XII observer now also requires every later note of a slide chain to be glided. No
Note is, so pitch geometry alone never certifies a slide. The falsifier (five attacked notes
certified as a slide) now passes. What that change moves:

- no acceptance verdict;
- no Round XIII mass count on the Round XII or XIII arms. SWISS unowned stays 6/0 and overdrawn
  0/0; BLACK_ICE unowned stays 21/0 and overdrawn 2/2.

The bass needed no change. Every bass chromatic move in both songs is one attacked approach note
a semitone into its target on the next onset: 18 per song. BLACK_ICE's are 15 G#2 → A2,
2 D#2 → D2 and 1 A#2 → A2, each 0.307 s at 88 BPM. The target is a chord tone, and the observer
supports the approach independently. The route receipt is `black_ice.lines.txt`.

### Causal architecture

`Score.hearings` records what each dependent player consumed, at the moment it was consumed:

- keys, bass and drums ← lead;
- drums ← bass;
- the Round XIV pad ← lead, keys and bass.

`Score::stale_hearings()` compares each hearing with the final score's notes of that role. It
reports none for Round XII or Round XIV in either song or anywhere in the sweep. For Round XIIIb
BLACK_ICE it reports keys, bass and drums each holding 18 lead notes that are not in the final
score. The field is not part of the fingerprint.

### Sweep (ignored; 240 performances, 3 worlds)

`fuzz_the_coherent_arm_keeps_every_chart_chord` asserts, against Round XII:

- identity is no worse at any instant;
- plan, conformance, lead, keys, bass, drums and SFX are unchanged;
- no receipt is lost and there are no new false claims;
- no stale hearing, and no declared slide.

It prints what remains. Flips go from 1225 (1347.2 s) to 96 (102.8 s), with 703 spacings and
1065 rootings. The 96 left are where the pad is silent or no admissible voicing helps (for
example BLACK_ICE C#m7b5 and VAPOR95 Cmaj7 in the 120-beat demo story). They are printed, not
hidden.

## Machine acceptance

- Round IX SongMap conformance, Round X meaning controls, Round XI phenomenal controls and Round
  XII temporal pitch controls pass (see `engineering.txt`).
- No harmonic identity failure in the SWISS acceptance song under Round XIV, at any slice.
- No fake `SlidePath` claim; no realizer declares one; the observer no longer grants one to
  attacked notes.
- No coverage deletion of any kind in Round XIV.
- No post-hoc mutation: the hearing ledger is clean. The bass the drums heard is the final bass,
  and the lead the keys, bass and drums heard is the final lead.
- Rounds XIII and XIIIb are retained unchanged as historical arms.

## Renders

`cargo run --release --example heard_object_lab -- --render --out=target/humanmusic-r14/final`

Same SongMap (StablePropulsion, seed 2112, `deflected_lift_trace(120)`), language, production and
performance options for both arms:

- `swiss_r12.wav`, `black_ice_r12.wav`: Round XII. These are byte-identical to the Round XII
  renders in the Round XIII manifest.
- `swiss_r14.wav`, `black_ice_r14.wav`: Round XIV.
- `*.pad.wav`: the pad alone, the only stem that differs.

## Parked

- BLACK_ICE's lead chromatic notes, e.g. C#5 → C5 and C#5 → D5, 0.31–0.34 s at 88 BPM. They are
  single attacked approach or neighbour notes with independent support. Round XIII's mass audit
  still calls two of them overdrawn gestures. If they are what the maintainer hears as wrong, the
  question is an approach note against a held pad member a semitone away, which is the question
  Round XIIIb answered post-hoc.
- BLACK_ICE's bass G#2 approaches against a pad still holding G.
- The 96 sweep flips the pad cannot reach. The keys could hear the bass too; that is not done.
- Real portamento or a `Slide` articulation in the synth. Nothing needs it yet.
- Walking-bass passages under rootless voicings. A quarter-note walk away from the root for a
  whole bar is a flip under the 0.5 s hold, which jazz practice may not share. It is not seen in
  the acceptance songs.
- Promoting the identity audit into any gate other than the pad's own voicing choice.
- The PR #70 description, which is still Round IX.

## Human-unverified

The machine now checks the identity of heard musical objects rather than only individual-note
legality. Whether the new realization eliminates the remaining audible defects is still
determined by listening.
