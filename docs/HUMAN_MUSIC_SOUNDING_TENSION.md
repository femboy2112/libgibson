# HumanMusic Round XIIIb — sounding tension

## Scope and authority

Continues Round XIII (`a7fde22..a8514cc`) on `feat/v0.4-humanmusic-audio`; main/base
`7b4efdc4c7fc6523d890a28f2aa8e27a6ba59286`. PR #70 stays draft. No package, ABI
(`GIBSON_ABI_VERSION = 1`), MSRV (1.85) or release change. Audio stays Rust-only. Receipts are in
[fixtures/humanmusic-r13b](fixtures/humanmusic-r13b).

The maintainer's listen of the Round XIII renders:

> i still hear the same shitty chord at 16s in swiss, and the wrong notes are still in black
> ice. the point of temporal-chromatic mass, is that notes that sound weird given the context of
> the song need to be either quickly transient, or foreshadowing something that justifies the
> tension/dissonance.

## What Round XIII got wrong

Round XIII measured *ownership against the chord symbol*: a colour the harmony did not write,
weighted by exposure. It removed the keys D5 at beats 32 and 88. The chord at 16 s is not a
colour at all. Every pitch in it is written:

| | |
|---|---|
| beats | 28–32 (14.24–16.27 s, audible to 16.53 s) |
| harmony | Cmaj7 |
| pad | `E4 B4 C5 G5`, 3.92 beats each |
| bass | `C2` |

The major seventh B4 sits a semitone under the root C5, both at the same level (−13 dB in the
pitched stems), for 2.03 s. The B moves on only when the whole chord does. The same voicing
returns at beats 36 and 44; the pad voices every other Cmaj7 in the song `C4 G4 B4 E5`, with no
semitone. An ownership audit cannot see this chord, because nothing in it is unowned.

BLACK_ICE shows the same failure twice more:

- **The Am9 pads** (`G4 B4 C5 E5`, or `B3 … C5`, or `G4 A4 B4 C5`) hold the ninth a semitone
  under the minor third for 3.04 s, nine times.
- **The lead** strikes C#5 (and B4, and once A#4) against the pad's sustained C5 (A4) for
  0.3–0.4 s, then falls back onto the C that was sounding all along.

So Round XIIIb measures what **sounds**.

## The law

`src/audio/human_music/tension.rs`. A **clash** is two pitched notes heard at the same time a
minor second / augmented unison or a minor ninth apart (`CLASH_SEMITONES = [1, 13]`), in any
roles. Heard windows are Round XIII's (the patch envelope via `audible_end`, a release tail cut
at the same role's next attack). One voice's own legato tail into its next attack is not a clash.

- **Exposure** = overlap in seconds × metric salience where the clash begins (1.0 / 1.25 / 1.5),
  classed by the Round XIII bands. Fleeting is < 0.20 weighted s; Asserted is ≥ 0.75.
- **The note that owes the justification** is the less structural one: root < 3rd < 5th <
  7th/6th < written extension < diatonic non-chord tone < chromatic tone. A tie goes to the later
  attack, then the upper note.

That note is justified when the clash is:

| Verdict | Condition |
|---|---|
| Transient | Fleeting exposure |
| Foreshadowing | not Asserted, and its voice steps (1–2 semitones) to a new pitch within `RESOLVE_WITHIN_SECS` (0.35 s) of its release, unless that pitch is the partner a semitone away, already sounding |
| Suspension | prepared (sounding before its partner arrived) and stepping as above, at any exposure |
| Anticipation | not Asserted, a tone of the next harmony, sounding into it |
| **Unjustified** | anything else |

The "already sounding" clause is what separates the BLACK_ICE lead's C5–C#5–D5 from its
C5–C#5–C5. The passing tone points at a D the ear has not heard; the neighbour "resolves" onto
the C the pad has been holding under it the whole time, and the ear hears two adjacent pitches.

Invariants vs heuristics:

- **Invariants (tested):**
  - exposure is non-decreasing in overlap and in accent;
  - a prepared suspension stays justified at Asserted exposure, so mass is evidence, not a
    verdict;
  - the gate never adds a note, never plays one earlier, and never costs an interaction
    receipt.
- **Calibrated heuristics (named, each with a witness):**
  - the clash intervals: the major second (C6's G–A, Am9's A–B) and the tritone (Am6's C–F#)
    are deliberately not clashes;
  - `RESOLVE_WITHIN_SECS`;
  - `ORNAMENT_GRID_BEATS` (a 32nd);
  - `ORNAMENT_REACH_BEATS` (1 beat);
  - `PAD_REGISTER` 52–91 / `KEYS_REGISTER` 55–88.

## Tempo is load-bearing

| Case | 160 BPM | 118 BPM | 88 BPM |
|---|---|---|---|
| Half-beat lead C#5 neighbour over a held pad C5 | Transient (0.19 s) | Unjustified | Unjustified |

The same C#5 walking on to D5 is Foreshadowing at 88 BPM. The same pitches, the same beats, and
a different verdict for each clock or continuation (`tension_witnesses.rs`).

## The gate (`perform_tension`, opt-in)

`perform_tension` is `perform_mass`, then `gate_sounding_tension` over the whole band. It runs
after the drums have heard the band, so the groove is unchanged. It takes one unjustified clash
at a time, earliest first, and accepts the first candidate that:

- leaves fewer unjustified clashes;
- leaves none on the edited note;
- keeps every interaction receipt the band witnessed (the real `witness::audit`, not a guess).

Candidates by role of the responsible note:

- **Support voicing** (pad; keys comp/hold) makes room:
  1. shorten, when a line arrived against it;
  2. omit a member whose pitch class still sounds (doubled, or the root in the bass);
  3. move it by whole octaves, pitch class kept, so an authored seventh or ninth survives;
  4. the same for its support partner;
  5. omit, once per voicing, while the instrument still sounds two other voices;
  6. last, move to the nearest written chord tone its voicing lacks.
- **Line** (lead, bass, keys answers/figures) becomes transient:
  1. re-time it as an ornament leaning into its voice's next note: a later onset *inside its own
     written slot*, a 32nd, Fleeting with its whole release tail;
  2. else shorten it in place;
  3. else let a support partner make room;
  4. last, step to a neighbouring scale pitch.

## Acceptance (StablePropulsion, seed 2112, default options)

A = Round XII temporal, B = Round XIII mass (the renders the maintainer heard, byte-identical),
C = Round XIIIb tension.

| | A unjustified | B unjustified | C unjustified | C edits | affected |
|---|---:|---:|---:|---:|---:|
| SWISS | 4 | 4 | 0 | 4 omitted | 0.903% of 443 |
| BLACK_ICE | 38 | 33 | 0 | 9 re-registered, 9 shortened, 11 ornaments | 6.713% of 432 |
| SWISS (DeflectedLift control) | 4 | 3 | 0 | 2 re-registered, 1 omitted | 0.775% |
| BLACK_ICE (DeflectedLift control) | 2 | 2 | 0 | 1 re-registered, 1 omitted | 0.524% |

**SWISS, the whole change:**
- the pad's C5 is dropped from `E4 B4 C5 G5` at beats 28, 36 and 44 (14.24 s, 18.31 s,
  22.37 s), and from `B3 C5 E5` at beat 116;
- bass `C2` states the root, so the written maj7 keeps its B;
- nothing else moves: lead, keys, bass, pad elsewhere, and the 43–45 s window.

**BLACK_ICE:**
- every Am9 pad's B goes up to B5, so the ninth survives;
- two Dm9 pad E5s release after 1.5 beats so the melody's F5 arrives alone;
- eleven lead C#5/B4/A#4s become 32nd-note graces into their next note;
- seven C#5s followed by a rest are cut to a 32nd in place.

The lab (`examples/sounding_tension_lab.rs`) asserts, per score:
- B's fingerprint equals the Round XIII receipt;
- plan, drums, SFX and chart are identical;
- SongMap conformance passes;
- Round XII claims stay at 0;
- no interaction receipt is lost;
- 0 unjustified clashes remain;
- an independent diff of B→C counts exactly the gate's ledger.

**Sweep** (`fuzz_the_tension_gate_leaves_no_unjustified_clash`): 240 performances (two stories,
two lengths, ten seeds, both composers, all three worlds).
- Unjustified clashes: 2609 → 0; edits on 3.11% of 65161 pitched notes.
- Interaction receipts: 0 lost, 8 gained.
- No new Round XII false claim; SongMap conformance report identical to B's.

## Independent graders (`fixtures/humanmusic-r13b/independent`)

- **`tension_audit.py`** is a second implementation. It is stricter: release tails are never
  masked. Result: SWISS 4 → 0, BLACK_ICE 33 → 0 unjustified.
- **`peaks.py`**: at 14.6, 15.0 and 15.9 s the SWISS C5 partial (−13 dB) is gone; `E4 B4 G5`
  remain.
- **`roughness.py`** (Sethares roughness of the pitched stems, one absolute peak threshold per
  file), mass → tension:
  - SWISS 14.3–16.2 s: −22.3%;
  - the other two SWISS Cmaj7 bars: −22.6% and −28.8%;
  - BLACK_ICE Am9 bars: −19.6%, −11.6% and −13.1%;
  - whole songs: −3.1% (SWISS) and −6.0% (BLACK_ICE).
- **The BLACK_ICE lead fixes don't register at window scale.** The Am6 window with the C#5
  fixes reads +0.4%: a 2.5 s window is dominated by the bass harmonics and the pad.
- **`tone_energy.py` measures those fixes directly.** At four spots, time the lead's C#5 spends
  above −24 dB falls from 0.34–0.38 s to 0.12–0.14 s.
- **A calibration error, found and fixed.** The first roughness calibration used a per-frame
  threshold. That admitted quiet partials exactly where a loud note had been removed, and read
  the removal as +2.1% roughness. Fixed, and the fix is documented in the script.

## Audio

`cargo run --release --example sounding_tension_lab -- --render --pitched --dump-notes
--around-seconds=16 --around-seconds=44 --out=target/humanmusic-r13b/final`:

- `swiss_mass.wav` / `swiss_tension.wav`
- `black_ice_mass.wav` / `black_ice_tension.wav`

Levels and hashes are in `fixtures/humanmusic-r13b/final-manifest.json`. The mass renders are
byte-identical to Round XIII's. BLACK_ICE tension peaks at 0.953, below the 0.97 limiter ceiling
Round XIII's mass arm hit at 60 s.

## Not done / PARKED

- The chart itself (SongMap harmony) is frozen and untouched.
- Out-of-key notes with no semitone partner are not audited by the gate. Measured in the receipts
  under the same rule, every one is already transient or foreshadowing in both acceptance songs
  (SWISS 4 + 2, BLACK_ICE 14 + 22).
- Pad release tails into the next chord (0.26–0.32 s, one voice stepping F→F#, C→B) are legato
  by rule. If they are heard as clashes, that is the next witness.
- Human listening is the open gate.

## Human-unverified

The machine now holds every sounding semitone clash to "transient, or foreshadowing what
justifies it," measured on the texture that sounds rather than the chord symbol. Whether the 16 s
chord and the BLACK_ICE wrong notes the maintainer hears are these clashes remains a listening
question.
