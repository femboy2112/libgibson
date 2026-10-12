# C137 bass capacity and teacher provenance

This scout inspected runtime source at `29527a891ed7bfed9657a99046884a61670897e3`
in the isolated `research/c137-kernel-bass-audit` worktree. The research audit read
in full was PR #103 head `12eb9f2b72c80ff116bcf584aacfe0751bde286a`.
No synthesis defaults, bass realization, or `BASS_WEAVE` setting were changed.

## Findings and their boundaries

| Claim | Evidence | Status and limit |
|---|---|---|
| Valid continuity edges imply a monophonic bassist | Three executable Rust counterexamples below | **Refuted** as a general inference. Edge validity has a narrower intended contract. |
| A rapid repeated bass pitch is equivalent to one sustained pitch once continuity is attached | Reference-production PCM differs on a controlled one-second pair | **Refuted**. This says nothing by itself about whether repetition is musically useful. |
| The weave rollback followed distinct scheduling and acoustic complaints | Four historical commits and exact source changes | **Source-verified history**; listening verdicts are maintainer reports, not this scout's audition. |
| No local teacher audio exists | Local WAV/MP3/Score found; fresh source build reproduces WAV byte-exactly | **Refuted** for one narrative control. Membership in the accepted seven-file set is still unverified. |
| Low pitch alone makes bass melody impossible | Teacher bass also reaches MIDI 36, while extending up to 59 | **Unsupported**. Register, duty, rhythm, articulation, and masking must be tested jointly. |
| Full-mix level proves inter-role masking or musical recognition | No isolated perceptual masking experiment or human verdict here | **Unverified**. Mix RMS and spectral overlap would not establish either claim. |

These bearings share the repository implementation; they are not independent
listeners. The separately parsed Score dump, Rust PCM controls, and file hash
comparison are different instruments for narrower claims.

## The bass failure sequence

| Commit | Actual new responsibility or mutation | What it did not establish |
|---|---|---|
| `31511bf7e42706ed9ac16f8b8f664e9bf8b68d34` | `weave_voice` projects the germ into MIDI 35–50; late functor insertion deletes foundation onsets inside answer windows and rebuilds voice links | A lawful prior transfer of harmonic-floor responsibility, final hearings, or acoustic clarity |
| `2cff8796e63212aae43a9a505907bee35b581f02` | Clips each bass gate to the following attack after inserting the weave | Source gate preservation or removal of repeated articulation |
| `2c945b5840d2523189229f76d19dbaf98d41e800` | Ties same-pitch neighbors within half a beat before clipping | A source-authorized sustain, harmonic destination, or useful low-register phrase |
| `29527a891ed7bfed9657a99046884a61670897e3` | `BASS_WEAVE = false`; keys answer the open windows and foundation bass remains | Universal impossibility of melodic bass; this is the required containment control |

`comp.rs::weave_voice` tags generated material as `role_note = "counter"` and
snaps each projected pitch to current stable harmony. The independent argument
witness concentrates on the `"argument"` carrier. Thus chord-tone labeling and
an argument witness do not establish ownership or correctness of the extra
counterline. The nearest lead pitch is selected by absolute temporal distance,
which can reference a future lead event: it is usable as declared source context,
but it must not be described as prior hearing of actual performance.

`bass.rs` itself already trims competing realized gates, then constructs pedal
restrikes and runs further support handling. Those historical adapters remain
compatibility code. A new kernel should emit one capacity-valid duty stream
before observation rather than borrow these repairs as its admission proof.

## What mono means in this implementation

`synth.rs::with_production` allocates **three** bass voice slots. This is a DSP
pool allowing overlap and choke tails, not three authorized musical bassists.
`trigger_note` chokes only explicit predecessor identities in `choke_from`.
It does not infer a unique performer from `Role::Bass` or `Score::mono_voice`.

`voice.rs::continuity_violations` checks link endpoints, uniqueness of predecessor,
direction, and role. It does not demand that all bass events belong to a single
nonbranching lane; simultaneous bass pitches cannot be connected by
`VoiceContinuation::new` because that constructor requires increasing onsets.
An unlinked extra pitch still renders. The 15 ms choke ramp is intentional direct
voice behavior; downstream shared reverb is a different lifetime.

The new `tests/audio_c137_bass_capacity.rs` fixes three predeclared negative
controls against the legacy contract:

1. Insert a disconnected E2 event inside a C2-to-G2 foundation line. The original
   continuity edge stays valid; the new event is not choked by that edge and
   changes reference-production PCM.
2. Insert a simultaneous C2/E2 bass dyad. No legal edge joins the equal onsets,
   yet the empty edge set passes and both contributions reach PCM.
3. Retrigger C2 after a quarter beat with a legal explicit edge. Edge validation
   passes, while PCM differs from a one-beat held C2.

Executed successfully (3 passed, 0 failed):

```bash
CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  CARGO_INCREMENTAL=0 cargo test --test audio_c137_bass_capacity -- --nocapture
```

The tests deliberately remain true statements of a legacy API's limits. They
are not failing CI placeholders, and their success is not new-path admission.

## Minimal bass duty recommendation

Use the existing `PerformancePlan`, `Stage`, `ActionPlan`, `BassMode`, and
source-bound interaction structures. No second duty ontology is needed for
the first conversation.

- The authoritative harmonic context chooses each required foundation landing.
  A source-linked `Resolve`/`ReEntry` or an allowed stage seat owns the event.
- Keep one chronological bassist stream. Generate root/third/fifth/seventh
  motion inside that stream; do not mix a second melody over it and then erase
  colliding foundation notes.
- Optional `Pickup` material can anticipate the next committed harmonic
  destination only when that bass landing is actually authorized. The existing
  earned-function `own_downbeat` calculation already checks the stage, holes,
  quoted windows, piece end, pedal duty, and the planned next harmony.
- An `Answer` using the bass seat must reserve a compatible interval before
  foundation realization. If required foundation and answer cannot coexist in
  the same lane, refuse the answer or route it to keys. Do not silently delete
  a carrier, shorten a required source gate, or repair its function label.
- Build explicit continuity from the accepted single stream. Before commit,
  reject simultaneous attacks, crossed source gates, competing successors,
  unauthorized stage windows, and anticipations without their promised target.
- Purposeful withholding is an existing source/interaction silence or stage
  decision. A free slot or lead rest alone does not authorize a bass melody.

The new contract should check source identity and feasible action composition,
not demand universal vertical consonance. A licensed chromatic approach needs
its destination and timing; it does not need to be snapped to the current chord.
Likewise, no universal lower-register pitch ban follows from the rejected weave.
`CoupledR8` remains an independent warning against coercing the whole band into
one harmonically tidy object.

## Teacher media: what was actually found

Narrow local searches covered the `LibGibson*` worktrees and relevant Music,
Downloads, temp, and ANAL_MUSIC locations. One teacher-named media group was
found at `/home/leah/LibGibson.resurrection/target/teacher-dump`:

| File | SHA-256 |
|---|---|
| `black_ice_narrative.wav` | `79cc0b73846c2b6e3658e6cf980e1a1415bc2b9b27c7834dbbdf8a22296b0a36` |
| `narrative_score.txt` | `f1ee1170bb0adb04700d7c18b4fcbee6567cb40683e2f3389750aa7730624fa8` |
| `teacher_golden7_narrative.mp3` | `0011fe6faad2ecb1b2894898c7c259ae47cfb01a762afe0ce535ec83578c466f` |

WAV: stereo signed 16-bit PCM, 48 kHz, 4,794,783 frames, 99.8913125 seconds.
The accompanying Score declares 224 beats at 138 BPM; it has no chord records,
hearing records, or explicit continuity edges, and `mono_voice = false`.
The MP3 reports 99.936 seconds and a `Lavf60.16.100` encoder tag. Its name is not
proof of original custody or a seven-file acceptance record.

The local resurrection checkout is currently
`c6a69dc57e75cd4063adbeb813f0429141ea00ca`; its handwritten `rick_probe.rs` and
`band_story_probe.rs` are unchanged through baseline `29527a89`. The exact
baseline source build was run independently:

```bash
CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 \
  cargo run --example band_story_probe -- \
  --cast=narrative --world=black_ice --out=target/reconstructed-teacher
sha256sum target/reconstructed-teacher/black_ice_narrative.wav
cmp target/reconstructed-teacher/black_ice_narrative.wav \
  /home/leah/LibGibson.resurrection/target/teacher-dump/black_ice_narrative.wav
```

**Result:** `cmp` exit 0 and the same SHA-256 as the table. This establishes a
reproducible handwritten narrative reference. The newly generated copy remains
labeled **reconstructed**; byte identity does not invent a listener annotation
or establish that all seven accepted references have been recovered.

Source blob IDs at baseline:

```text
examples/rick_probe.rs       715513071f3944e38e5d168c14e5de238e786cd3
examples/band_story_probe.rs 8a43ab393e185279b4c8e2a36d24650458c97858
```

The handwritten narrative Score has 1,046 pitched notes and 1,379 drum hits.
Independent parsing of the saved Score gives:

| Role | Notes | MIDI range | Distinct stored velocities | Stored velocity range |
|---|---:|---:|---:|---:|
| Lead | 269 | 62–80 | 81 | 0.468–1.000 |
| Keys | 334 | 50–71 | 70 | 0.380–0.860 |
| Bass | 359 | 36–59 | 60 | 0.541–0.860 |
| Pad | 84 | 50–67 | 1 | 0.400–0.400 |

After sorting bass by onset there are **zero written-gate overlaps** and **zero
same-pitch adjacent retriggers within a quarter beat**. Forty-two bass notes are
below MIDI 40; low register is not absent. These counts describe the file,
not an ideal density objective. Pad uses one velocity while other parts vary:
independent musical behavior does not require maximizing variation everywhere.

`rick_probe::bass_dense` states one contour through the chord's root, third,
fifth, seventh, fifth and root, then approaches the next root. Its keys stabs
occupy complementary sixteenth positions. The narrative example plans carrier
transfers and withheld lead statements from form. Their bass melodies are not
independently overlaid streams competing with a still-active foundation duty.

## Acoustic limits and follow-up controls

An additional local harness copied the unchanged `band_story_probe.rs`, renamed
its CLI `main`, and appended a driver which calls its original
`build_song(Cast::Narrative, 138.0, Stem::full())` **once**. It rendered that same
Score under normal, nocomp, reference, and five solo `StemMask` configurations.
No teacher notes, velocities, or onsets were reauthored. The generated harness
and binary remain local at
`/home/leah/LibGibson.kernel-bass/target/teacher_same_score_probe.{rs,}`;
the source SHA-256 is
`4043997bcf194b013f36b0b940cc2bcc442d3f07fd4d808014b6b9d3202446ac`.
It was compiled against the just-tested baseline library with:

```bash
rustc --edition=2021 -Awarnings target/teacher_same_score_probe.rs \
  --extern gibson=target/debug/deps/libgibson.rlib \
  -L dependency=target/debug/deps -o target/teacher_same_score_probe
./target/teacher_same_score_probe
```

Actual output directory:
`/home/leah/LibGibson.kernel-bass/target/reconstructed-teacher`.
Score fingerprint: `245caa1eca036f3d`. The generated `fresh_score.txt` is also
byte-identical to the original Score dump, with the same SHA-256 from the table.
All eight renders have 4,794,783 frames. The normal mix is again byte-identical
to the recovered teacher WAV.

| WAV | Stereo RMS (float render) | Peak (float render) | SHA-256 of written WAV |
|---|---:|---:|---|
| `narrative_normal.wav` | 0.173425 | 0.830086 | `79cc0b73846c2b6e3658e6cf980e1a1415bc2b9b27c7834dbbdf8a22296b0a36` |
| `narrative_nocomp.wav` | 0.188921 | 0.786706 | `9f042cbd65bbaaf15e6f6b6ce0f528762f7efcbbf87e24913a2bf28d908f6d93` |
| `narrative_reference.wav` | 0.133416 | 0.970000 | `9a5de803e78bd3cafe0e1c44661143f4972810dcb7106625f76ff8d4f3afb878` |
| `narrative_bass.wav` | 0.121718 | 0.512795 | `4b74918be0fab19777149115633e5f9ba78d67fdacd566c544fa215ac56b9ddf` |
| `narrative_keys.wav` | 0.065087 | 0.680594 | `a5ab5765ce14289c2920089c76ce142ec9ec368da37b4cfa1cf33f2ba5ef2641` |
| `narrative_lead.wav` | 0.067441 | 0.480079 | `bb3694085952836a3c55549c0ae4e5c30c311ddc55cfe4b893b4adba55baa5a6` |
| `narrative_drums.wav` | 0.149542 | 0.787794 | `ec32b5b4bee6eb81ebcc52d1d6f018be0ef175d01dc7a991f049fb15282c7dc5` |
| `narrative_pad.wav` | 0.023969 | 0.167439 | `1dd804d35cd9e7e02d545c646cd6206b68cdd4d880468a191be8a4d46402332d` |

The unmasked pre-master bus meter reports RMS bass `0.14886343`, keys
`0.079734266`, lead `0.081789605`, drums `0.21794415`, pad `0.029511843`.
This is actual acoustic contribution, but it is not a masking threshold or a
verdict on bass prominence. The logs are `same_score_probe.log` and
`bus_levels.txt` in the output directory.

The original narrative full mix measures mono-mixdown RMS `0.1722238222` and
maximum channel magnitude `0.8300476074`. Over complete one-second windows,
RMS-level p10/p90 are `-15.9047/-14.6444 dBFS`. These are unweighted PCM statistics,
not perceived loudness, mastering certification, or a role-correlation result.

For a fair comparison, keep the exact same Score and use existing
`HumanMusicSynth::with_production` and `StemMask`. `nocomp` isolates the compressor
toggle but leaves limiting and the other production stages; `reference` changes
multiple timbre, release, and production factors, so a difference there alone
does not localize a mechanism. Solo stems run through the output chain and need
not sum back to a nonlinear mastered full mix. Use the synth's per-bus meter for
the pre-master role contribution.

The existing baseline fusion harness can render matched production variants:

```bash
cargo run --example fusion_gates -- --seed=2112 --beats=128 --tempo=144 \
  --wav=target/c137-normal --production=normal
cargo run --example fusion_gates -- --seed=2112 --beats=128 --tempo=144 \
  --wav=target/c137-nocomp --production=nocomp
cargo run --example fusion_gates -- --seed=2112 --beats=128 --tempo=144 \
  --wav=target/c137-reference --production=reference
```

These commands are follow-up recipes, not runs performed by this scout. The
bass-held baseline remains mandatory. Comparing a future feasible bass answer
requires same-source mute/replace controls, bass and masker stems, written and
direct-audible occupancy, arrival preservation, and the maintainer's listening
verdict. No acceptance against `teacher_golden7` is claimed here.
