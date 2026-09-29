# Round XIII receipts — temporal mass

Starting source: `d72b72f1d4d4c670f3ac2f2a5a3ccd808051d911`.
Branch: `feat/v0.4-humanmusic-audio`; main/base `7b4efdc4c7fc6523d890a28f2aa8e27a6ba59286`.
PR #70 remains draft. No package, ABI, MSRV or release change.

## Commit 1 — failing witnesses only

`baseline-failures.txt` is the literal pre-intervention run of the four ignored falsifiers in
`src/audio/human_music/mass_witnesses.rs`. No production source changed. Each witness fails
because the Round XII observer returns an **identical** verdict for two events that differ
only in time:

| Witness | Pair judged identical by Round XII |
|---|---|
| SWISS 15–17 s / 43–45 s | keys D5 struck at beats 32 and 88 vs the offbeat D5 comp stab at 30.75 (all `Orphaned OrphanColor`) |
| Tempo dilation | the same one-beat pad D over C6 at 160 and 88 BPM |
| Fast slide vs slow staircase | C# inside C–C#–D–D#–E at 1/4 beat vs 1 beat per step |
| Long orphan ninth | a D over C6 held from the harmony's arrival vs a 0.2-beat weak D stab (keys and pad) |

The authored control inside the last witness (the same held D over `Cadd9`, whose written
chord contains the ninth) already passes and must keep passing.

The staircase witness first asserted that Round XII lets the fast C# inherit the slide. That
assertion failed: Round XII marks the fast C# suspect too, because the D it moves through is
a palette tension, which the audit labels `OrphanColor` at any speed, and the suspicion is
propagated to the connector. The witness now asserts the actual defect (the same verdict for
both parameterizations). The first form was not committed; this note preserves it.

## Source observation that changes the witness reading

The SWISS witnesses are written as a 1.94-beat and a 3.90-beat keys "hold". Written duration
is not what the listener receives on these patches:

- SWISS keys patch `adsr = (0.002, 0.18, 0.0, 0.1)` and BLACK_ICE keys `(0.003, 0.14, 0.0, 0.1)`
  have zero sustain. `sonority::audible_end` (Round VIII) puts every keys note at
  `a + (30/40)·d` = 0.137 s (SWISS) / 0.108 s (BLACK_ICE) above the 30 dB floor, whatever its
  written length. SWISS lead `(0.003, 0.12, 0.0, 0.08)` is also percussive; BLACK_ICE lead
  (sustain 0.4), both pads and both basses sustain.
- A second, independent route: the keys stem of the Round XII SWISS StablePropulsion score
  (seed 2112, normal production with reverb) was rendered solo with a temporary uncommitted
  probe, and D5 (587.33 Hz) energy was measured with a 1024-sample Hann Goertzel window,
  dB relative to D5's song peak:

  | Time (s) | Beat | Event | dB |
  |---:|---:|---|---:|
  | 15.65 | 30.78 | offbeat stab onset | −2.8 |
  | 15.75 | 30.98 | | −24.4 |
  | 15.85 | 31.17 | | −55.3 |
  | 16.30 | 32.06 | "hold" onset | −7.1 |
  | 16.40 | 32.25 | | −30.8 |
  | 16.50 | 32.45 | | −55.7 |

  The 1.94-beat hold and the 0.45-beat stab decay identically. Hold velocities are
  0.2008 (beat 32) and 0.2362 (beat 88); the neighbouring stabs are 0.2231.

What does distinguish the two witnesses from the other 29 keys D5 attacks in SWISS: they are
the only keys D5 attacks at the harmony's own articulation (beat 32 = C6 arrival, beat 88 = C6
arrival). They fuse with the pad's re-attacked C6 voicing `E4 A4 C5 G5`, whose C5 lies a whole
step below, and with the bass root. The other D5 attacks are syncopated between harmonic
articulations. Round XIII therefore measures onset fusion with the harmonic articulation as
temporal evidence alongside dwell. It does not use written duration as a proxy for exposure.

These are source and signal observations. Whether the maintainer's 15–17 s and 43–45 s
reports are caused by these exact events remains a listening question. The 43–45 s window
also contains a bass/keys/lead unison C# (beat 84.5, 0.229 s) against the pad's sustained C5,
and the pad's own unwritten D5 over Cmaj7 begins at beat 92 (46.78 s).

## Commit 2 — the TemporalMass contract and the diagnostic-first baseline

`src/audio/human_music/mass.rs` adds one evidence object and one overlay audit. It does not
change the Round XII observer, and generation is unchanged in this commit.

**`TemporalMass`** (per pitched note):

| Field | Unit | Source |
|---|---|---|
| `written_beats`, `written_secs` | beats, s | the note; `60 / score.tempo_bpm` (the score is the clock) |
| `audible_secs` | s | Round VIII `sonority::audible_end` for the role's world patch (30 dB floor), cut at the same role's next attack at/after the written end |
| `recurrence_secs` | s | audible time of earlier same-role, same-pitch-class attacks inside `RECALL_WINDOW_SECS` |
| `accent` | Weak / MidBar / Downbeat | bar position (`score.beats_per_bar`) |
| `fused` | bool | a pad/keys/bass onset within `FUSION_ONSET_SECS` of its harmony's articulation |
| `mass` | weighted s | `(audible + recurrence) × salience(accent)` |
| `class` | Fleeting / Gestural / Asserted | `Asserted` if fused or mass ≥ 0.75; `Fleeting` if mass < 0.20 |

Velocity is recorded but not weighted (the acceptance holds are no louder than their stabs;
cross-role loudness depends on mix gains the model does not simulate).

**Named constants** (each has a witness test in `mass_witnesses.rs`):

| Constant | Value | Meaning | Witness |
|---|---:|---|---|
| `FLEETING_MAX_SECS` | 0.20 weighted s | ≥ 5 events/s: a gesture member | half-beat weak event: Fleeting at 160, Gestural at 118 |
| `ASSERTED_MIN_SECS` | 0.75 weighted s | heard as a harmonic statement | one-beat mid-bar event: Gestural at 118, Asserted at 88 |
| `RECALL_WINDOW_SECS` | 2.0 s | same colour still in the ear | four pings weigh ≥ 3.5× one |
| `FUSION_ONSET_SECS` | 0.03 s | attacked with the harmony | SWISS keys D5 at 32 fuses; the stab at 30.75 does not |
| `DOWNBEAT_SALIENCE`, `MIDBAR_SALIENCE` | 1.5, 1.25 | 4/4 metric hierarchy | tempo proof, monotonicity |

**Invariants vs heuristics.**

- Mathematical invariants, tested:
  - mass is non-decreasing in audible time, recurrence, accent and fusion;
  - audible time is non-decreasing in written time on a sustaining patch;
  - class is monotone in mass and in fusion;
  - `FLEETING_MAX_SECS < ASSERTED_MIN_SECS` (a compile-time assertion);
  - a percussive patch's audible time does not dilate with tempo or written length.
- Calibrated heuristics: every value in the constants table. The orders of magnitude are
  defensible (run rate, echoic memory, onset-asynchrony fusion). The exact values are
  choices, not measurements of this listener.

**Ownership vs demand** (`mass::verdict`), from the Round XII observer's reconstructed support:

- Fleeting: needs `Licensed` (palette membership) or `Path` (slide, or a connector into a non-structural destination).
- Gestural: needs `Local` (a connector into a written chord tone, or a colour kept as a common tone or resolving by step). A colour holding only `Licensed` is a `SoftOrphan`; anything else is an `OverdrawnGesture`.
- Asserted: needs `Harmonic` (written in the sounding harmony, a physical suspension or retardation, an appoggiatura, an anticipation, a pedal, or a selected colour). A colour without it is an `UnownedAssertedColor`; in a support role it is counted as an accidental reharmonization.

**Baseline on the frozen Round XII scores** (fingerprints asserted by the lab; `baseline/`):

| Score | unowned asserted colours | accidental reharmonizations | overdrawn gestures | soft orphans | Round XII orphans |
|---|---:|---:|---:|---:|---:|
| SWISS stable | 6 | 6 | 0 | 20 | 38 |
| BLACK_ICE stable | 21 | 21 | 2 | 15 | 39 |
| SWISS deflected | 20 | 20 | 0 | 8 | 35 |
| BLACK_ICE deflected | 25 | 25 | 0 | 11 | 46 |

**Hard pre-generation acceptance:**

- PASS: SWISS keys D5 at beat 32 (16.271 s) and at beat 88 (44.746 s) are Asserted (fused),
  `UnownedAssertedColor`, mass 0.617 and 0.602 weighted s. That is at least 2× the offbeat D5
  stab at 30.75, which stays Gestural or Fleeting.
- NOT MET: "beat 88 higher still". The written seconds do order that way (1.983 s vs 0.986 s).
  But both events are the same 0.137 s keys ping. Beat 88 carries slightly *less* recent
  recurrence (0.264 s vs 0.274 s), so its mass is 0.602 against 0.617. Written length is
  inaudible on this patch. The model does not invent an ordering the signal does not contain.
- PASS: every BLACK_ICE 3.92-beat orphan pad colour has more written and audible real time
  (≥ 2.673 s / 2.727 s) than any SWISS one (1.993 s / ≤ 2.293 s).
- PASS: the fast slide's intermediates are Fleeting and carried. In the slow staircase at 118
  BPM the C# is overdrawn, the D is a soft orphan and the D# is carried. At 88 BPM the D
  becomes an unowned asserted colour.

**Also flagged that the maintainer did not report** (SWISS stable):

- the keys D5 fused at beat 112 (56.95 s), the same physics as the two witnesses;
- the pad D6 over Fmaj9 at beat 20 (10.17 s);
- the pad D5 over Cmaj7 at beat 92 (46.78 s), right after the 43–45 s window;
- the pad A4 over Cmaj7 at beat 116 (58.98 s).

These are model predictions, not listening evidence.

The Round XII "preserved good" pad B4 over Am6 at 24 is reclassified: it is held 2.67 s
under a harmony that does not write the ninth, and is owned only as a common tone into Am9.

The two BLACK_ICE lead C#5 overdrawn gestures (beats 33 and 41) come from the same mechanism.
C#5 is heard for 0.34 s over Am6 on a sustaining lead and approaches D5, which is itself an
unowned colour. In SWISS the lead patch is a ping.
