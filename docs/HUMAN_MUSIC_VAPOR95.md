# VAPOR95 — palette audit and revision record

**Why this exists.** The maintainer's listen called the VAPOR95 palette "kinda trash": generic lush
retro/synthwave, not convincing classic vaporwave. The perceptual north star is the *production and
form language* of MACINTOSH PLUS — リサフランク420 / 現代のコンピュー: euphoric old-pop/R&B memory,
slowed and heavy without becoming sludge, obsessive repetition, bright 80s digital source material
submerged in haze, chorus smear, feedback echoes, spatial motion, a drum machine behind the remembered
song, the mix as an instrument. It is a **reference, not material**: no audio of it is fetched or
stored, and none of its melody, lyrics, vocals, harmony, form or tempo is reproduced. HumanMusic stays
100% synthesized.

The semantic architecture is closed (final source before this intervention: `abc3f9f`; holdout v6
28/28, 503 checks; 485,568/485,568 fresh BAND performances admitted). This revision changes the
VAPOR95 **world** — its dialect — and the production physics every world may declare. It does not
change what a song is.

## 1. The audit (before any change)

**Observed** at `abc3f9f`. Two independent measurement passes:

- the style baseline (`docs/fixtures/humanmusic-vaporize/v1-baseline/`): five BAND takes under the
  archival v1 world, measured by `vapor95_style_lab`;
- a read-only spectral audit (fork A): seed 2112, three grammars, solo stems for VAPOR95 and
  BLACK_ICE, plus VAPOR95 `nosat` and `dry` renders (numpy; stems are solo renders through the full
  master chain).

| measure (full mix unless noted) | VAPOR95 v1 | BLACK_ICE |
| --- | --- | --- |
| spectral centroid | 962–1040 Hz | 1994–2074 Hz |
| energy above 6 kHz | 4.1–4.9 % | 11.1–11.7 % |
| low band (< 150 Hz) L/R correlation | 0.70–0.83 (`dry`: 1.000) | 0.98 |
| low band side vs mid (style baseline, 5 takes) | −7.0 … −9.5 dB | −19 dB |
| tail decay | ≈ −26 dB/s (RT60 ≈ 2.3 s) | ≈ −66 dB/s (≈ 0.9 s) |
| crest factor | 16.2–17.3 dB (`nosat`: 20.7) | 18.1–19.3 dB |
| bass stem: energy < 120 Hz / 120–500 Hz | 92–98 % / 2–8 % (centroid 65–73 Hz) | 76–79 % / 18–21 % |
| drums stem centroid | 1.34–1.55 kHz (70 % below 120 Hz) | 3.5–3.9 kHz |
| pad sounding (50 ms frames) | 93–96 % of the song | 91–94 % |
| co-sounding pad − keys | −2.1 … +0.8 dB | −1.1 … −3.7 dB |
| co-sounding keys − lead | −5.2 … −6.8 dB | −4.3 … −5.6 dB |
| clipped samples | 0 | 0 |

Synthesis facts that constrain any patch (from the code): an FM patch sums its 2-operator FM voice
with a plain sine at the fundamental; the FM index is static and in **cycles** (β = 2π·index); the
only attack-to-sustain brightness motion is the filter envelope; `FmOsc::alias_free_index` removes FM
from high ratios (ratio 14 → 0 at C6). Patch envelopes are part of the musical model — attack sets the
expression minimum gate, and release tails enter the BAND sounding-tail/identity laws — so a palette
change legitimately re-performs BAND takes (the laws must still admit them).

**Inference — why v1 reads as generic synthwave:**

1. **A puddle instead of a pocket.** The whole music bus — bass and kick included — feeds a bright
   (damp 0.25), 34 %-wet, ~2.3 s FDN, which decorrelates the low end (the `dry` render is perfectly
   mono). The bass itself has almost no definition: a triangle plus a sine an octave *below* the note.
2. **No source object.** The keys — the voice that should read as the remembered 80s workstation
   performance — are `Fm{ratio 2, index 1.0}` (β ≈ 6.3 rad): odd harmonics only, a hollow reed, sitting
   level with the pad and 5–7 dB under a buzzy `Fm{1, 1.4}` lead.
3. **Wash instead of memory.** A ±16-cent three-saw pad sounds through ~95 % of the song with a 1.2 s
   release under a 2.3 s room; there is no chorus and no echo, so "memory" exists only as continuous
   wash.
4. **Squash.** `tanh` drive 2.2 on the summed bus (drums included) removes ~4.4 dB of crest and
   intermodulates every chord.
5. **A slow pulse without bounce.** Dark, boomy drums (kick decays to −60 dB in ~1.24 s; hats at
   6.5 kHz) under 71 BPM eighth-note hats: a 0.42 s grid that drags.

## 2. The production law (new vocabulary)

`MusicWorld` always claimed to own its production treatment; its vocabulary was `saturation` and three
reverb numbers. The revision makes production one typed world law, `WorldProduction`, executed by the
synthesizer and reusing the existing DSP primitives (`Chorus`, `Delay`, `Reverb`, `Svf`):

```text
pad + keys + lead ──► MemoryChorus ──┐
bass ────────────────────────────────┼──► + drums ──► tanh saturation ──► + TempoEcho returns ──► Space ──► bus comp ──► limiter
keys + lead (send) ──► tone LP ──► L/R damped delays (note values) ─────────┘        (input low-cut)
```

- `MemoryChorus { rate_hz, depth_ms, mix }` — the unstable-memory smear, on the memory bus only.
- `TempoEcho { left, right: EchoTime, feedback, send, tone_hz }` — echo times are **note values**
  (`Eighth`, `DottedEighth`, `Quarter`, `DottedQuarter`); the synth converts them with the Score's
  tempo at the DSP boundary. Fed by keys + lead (articulated foreground); each repeat passes a
  one-pole low-pass in the feedback loop (`Delay::set_damping`), so memory decays darker. Returns join
  after the `tanh`, so repeats never intermodulate with the chord.
- `Space { size, damp, mix, low_cut_hz }` — the existing reverb plus an optional input low-cut
  (`Reverb::process_stereo_send`): the room answers everything above the corner, the low end stays dry
  and mono.
- Bass and drums never enter the chorus or the echo.

A `None` stage is not computed at all. BLACK_ICE and SWISS_SIGNAL declare `None` for every new stage
and their historical saturation/space, so their audio is byte-identical; so is the archival v1 world.
`ProductionControl` grows the matching single-factor ablations — `nochorus`, `noecho`, `fullband` —
and `HARMONIC_REFERENCE` removes them too.

Production is acoustic treatment, like the reverb always was: the BAND receipt reasons over the
Score's sounding notes and patch tails, not over echo repeats or room tails.

## 3. Candidate ranges (inference, from the audit)

| knob | v1 | range examined |
| --- | --- | --- |
| tempo / swing | 71 BPM / 0.16 (odd eighths +0.04 beat = 54 %) | cells around 78–90 BPM; swing 0 / 0.16 / 0.32 (50 / 54 / 58 %) |
| pad | Saw×3 ±16 c, 2200/+700 Hz, R 1.2 s, gain×mix .6×.8 | Saw×2, 5–9 c, 1100–1700 Hz, R .5–.9 s, gain×mix ≈ .5×.5 (4–7 dB under keys) |
| keys | `Fm{2, 1.0}` | `Fm{3, .2–.35}` (glass, harmonics 1 2 4 5 7 8) or `Fm{1, .25–.4}`; bright filter envelope; level ≈ lead |
| lead | `Fm{1, 1.4}` | `Fm{2, .12–.3}` or triangle; 1.8–3 kHz; attack ≤ 25 ms |
| bass | triangle + sub-octave, 900 Hz | saw/pulse, 400–700 Hz, no sub-octave; mono, dry |
| drums | kick amp τ .18, hats 6.5 kHz | kick τ .11–.15, snare HP 1.5–2.2 kHz, hats 7–8.5 kHz |
| space | 1.3 / damp .25 / mix .34, full band | 1.0–1.4 / .45–.65 / .22–.32, low-cut 150–200 Hz |
| saturation | 2.2 | 1.2–1.6 |
| chorus | — | .15–.5 Hz, 2–6 ms, mix .25–.5 (≈ 5–15 cents of wow) |
| echo | — | L {½, ¾} beat / R {¾, 1} beat, feedback .3–.5, send .12–.25, tone 2–3.5 kHz |

Unchanged on purpose: the SongMap (a world is a dialect, not a composer — no duplicated motifs to
fake sample chops), the eighth-note groove family (`subdiv`), drum density, `base_dynamic`, the master
ceiling and bus compressor, and VAPOR95's pre-existing clap and colour policy.

## 4. Revision record — VAPOR95 v1 palette → VAPOR95 current palette

An intentional sonic-world revision. **Observed** numbers below are the `vapor95_style_lab` records
committed in `docs/fixtures/humanmusic-vaporize/` (`v1-baseline/`, `corpus/`); every musical judgement
is the maintainer's and is **UNVERIFIED**.

### Parameters

| field | v1 (archival) | current |
| --- | --- | --- |
| tempo / swing / subdiv | 71 BPM / 0.16 / 8ths | **84 BPM** / 0.16 / 8ths |
| key, mode, vocabulary, voicing spread | F Ionian, full extended vocabulary, 0.6 | unchanged |
| pad | Saw×3 ±16 c, 2200 Hz +700, res .15, ADSR .4/.8/.7/1.2, gain .6 | Saw×2 ±7 c, 1400 Hz +250, res .06, ADSR .4/1.0/.65/.7, gain .55 |
| keys (source object) | `Fm{2, 1.0}`, 4000 Hz +500, res .1, ADSR .005/.5/.25/.5, gain .58 | `Fm{3, 0.25}`, 2200 Hz **+4200**, res .12, ADSR .003/.6/.22/.45, gain .72 |
| lead | `Fm{1, 1.4}`, 3500 Hz +800, ADSR .01/.6/.35/.6, gain .6 | `Fm{2, 0.15}`, 2600 Hz +700, res .08, ADSR .018/.5/.62/.5, gain .56 |
| bass | Triangle + sub-octave, 900 Hz +300, ADSR .01/.2/.7/.25, gain .8 | Saw, no sub, 560 Hz +360, res .18, ADSR .006/.22/.72/.16, gain .9 |
| kick | 52 Hz, sweep 140, τp .04, τa .18, click .15 | 54 Hz, sweep 150, τp .03, τa .13, click .22 |
| snare | HP 1200, τn .14, τt .10, 170/300, noise .6 | HP 1700, τn .12, τt .08, 190/320, noise .68 |
| hats / ghosts / density | 6500 Hz / .2 / .5 | 7800 Hz / .2 / .5 |
| saturation | 2.2 | 1.4 |
| space | 1.3 / damp .25 / mix .34, full band | 1.2 / damp .55 / mix .28, **low-cut 180 Hz** |
| memory chorus | — | 0.32 Hz, 3.5 ms, mix .4 (≈ 12 cents peak wow) |
| tempo echo | — | L dotted eighth / R quarter, feedback .38, send .2, tone 2.6 kHz |
| mix pad / keys / lead / bass | .8 / .9 / .62 / .82 | 1.4 / 2.1 / .56 / 1.4 (level-matched to v1 within ~1 dB RMS) |
| ceiling, `base_dynamic` | .95, .7 | unchanged |

Attacks stay ≤ 25 ms (pad 0.4 s unchanged), so the expression minimum gate does not move; the shorter
pad/bass releases do enter the BAND sounding-tail laws (legitimate world-relative re-performance).

**Tempo/swing disposition.** 71 BPM was the sludge: a 0.42 s eighth grid under boomy drums. The
default moves only the tempo (84 BPM) and keeps the world's light 54 % swing, so the A/B attributes
cleanly. Three other cells are rendered for the listen (`corpus/candidates/`): 78 BPM / 54 %,
84 BPM / 58 % (swing 0.32), 90 BPM straight. Machine screens pass for all four; none is preferred by
any machine check.

**DSP primitives.** Reused unchanged: `Chorus`, `Svf` (echo tone low-pass, space low-cut high-pass),
`soft_saturate`, `Compressor`, `Limiter`. Extended without changing their historical arithmetic (bit
tests): `Delay::set_damping` (one-pole in the loop), `Reverb::process_stereo_send` (room input given
separately). New glue: the `WorldProduction` types and the synth's `TempoEchoUnit`.

### Evidence

- **Same-Score palette A/B (A).** `corpus/palette/` renders each v1 take's Score through the current
  palette; its five `score=` fingerprints equal the v1 record's (`9c2168ff…`, `bf074393…`, `67ed22ec…`,
  `061efbd8…`, `ac74ede2…`). Same notes, onsets and dynamics; only timbre, mix and production differ.
- **Full world (B).** `corpus/current/` performs the same five SongMaps under the current world (BAND,
  checked, all admitted): SongMap and PerformancePlan fingerprints equal v1's; Score fingerprints
  differ (tempo, and the laws' response to the new tails).
- **Measured (full mixes, five takes, v1 → current):**

| measure | v1 | current (B) |
| --- | --- | --- |
| RMS / peak | −21.8 … −17.7 / −4.0 … −4.5 dBFS | −22.5 … −18.1 / −4.7 … −5.4 dBFS |
| low band (< 150 Hz) L/R correlation | 0.66 – 0.80 | 0.97 – 0.99 |
| low band side vs mid | −7.0 … −9.5 dB | −18.7 … −20.7 dB |
| crest | 12.4 – 16.9 dB | 12.0 – 17.3 dB |
| co-sounding pad − keys | −0.1 … −1.3 dB | −5.6 … −7.0 dB |
| co-sounding keys − lead | +0.1 … −18.8 dB (hookarc −8.2) | −0.4 … −10.1 dB (hookarc −0.4) |
| co-sounding drums − keys | +2.0 … +9.3 dB | −1.7 … +4.4 dB |
| bass energy 120–500 Hz | 1.2 – 2.7 % | 23 – 27 % |
| drums centroid | 1.0 – 1.7 kHz | 1.9 – 3.0 kHz |
| tail decay | ≈ −27 dB/s | −13 … −55 dB/s (echo memory) |
| ceiling contact | 0 | 0 |

- **Ablations** (`corpus/ablate/`, hookarc): `fullband` puts the low-band side back to −11.8 dB (the
  low-cut is what keeps the low end mono); `noecho` drops the end-of-render tail from −38 to −74 dB
  (the echo carries the memory); `nochorus` is ~2 dB louder (the decorrelated wet path costs level);
  `nosat`, `dry` as named. Every ablation keeps the Score fingerprint.
- **Natural transformation** (`tests/audio_vapor95_palette.rs`): for each representative SongMap, the
  BAND take under v1, current VAPOR95, BLACK_ICE and SWISS_SIGNAL (or SWISS's lawful vocabulary
  refusal) passes `SongMapConformance` with identical song coordinates — song fingerprint, form
  sections, identity sites, harmonic landmarks, chord changes, open debts, length.

### Historical compatibility

- **Exact:** BLACK_ICE and SWISS_SIGNAL (no new stage declared; R17 pocket 48/48 WAV + 757/757
  receipts byte-identical); every semantic receipt independent of VAPOR95.
- **Archival VAPOR95 v1** stays reproducible through the fixture (`tests/common/vapor95_v1.rs`; the
  in-crate test copy and the fixture are both checked against `v1-baseline/world-v1.txt`):
  `vapor95_style_lab --record --world=v1` reproduces the 30 baseline WAVs; `cover_music_lab
  --vapor95=v1` reproduces the 16-WAV cover corpus; `cover_fidelity_lab` and `drum_restraint_lab`
  take the same switch; tests whose VAPOR95 cases were derived under v1 (R7b/R8 pins, the closeout
  falsifiers, the support-lifetime and landed-function falsifiers, the consolidation flip witnesses,
  the R15 expression window, the closeout listening device) perform them under the fixture.
- **Intentionally replaced:** every render of the default VAPOR95 world — PCM and the VAPOR95 rows of
  listening hashes produced without `--vapor95=v1`. Holdouts v1–v6 executed under v1; re-running their
  harnesses at a later head is not their evidence (each is preserved at its declaration commit).
