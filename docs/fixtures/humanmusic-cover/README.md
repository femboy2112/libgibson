# External symbolic source evidence

This directory separates downloaded source artifacts, normalized observations, and
later generated performances. No commercial recording or lyric was downloaded.

## Ode to Joy: exact edition and provenance

The [Mutopia catalog #528](https://www.mutopiaproject.org/cgibin/piece-info.cgi?id=528)
identifies the SATB arrangement as **Public Domain**, edition
`Mutopia-2009/08/05-528`. The source is attributed to L. V. Beethoven and the
edition's maintainer is Peter Chubb. The catalog's `8 7 8 7 D` is hymn metre;
the musical time signature explicitly encoded in the symbolic source is 4/4.

Downloaded artifacts, URLs, retrieval UTC and SHA256 are in
[sources/mutopia-528/provenance.json](sources/mutopia-528/provenance.json).
The exact LilyPond source, MIDI and catalog/license snapshots are retained.
The parser is a general restricted absolute-voice LilyPond adapter, with a separate
Mido 1.3.3 Standard MIDI File observation route. It does not contain this tune's notes.

Source-derived observations: G major, quarter note = 100, 4/4, 64 quarter-note
beats, four named voices. The explicitly selected `sop` voice has 62 note events.
All four voices contain 245 symbolic events. The MIDI has 238 events: its two
staff tracks coalesce seven exact simultaneous unisons. The first strict event
multiset mismatch is retained in `ode-import/first-cross-check.json`.

A second, explicitly declared projection groups `sop,alto` into `upper` and
`tenor,bass` into `lower`, following the source staff definitions, then coalesces
only identical onset/duration/pitch events within each group. Every projected
event agrees with the MIDI (`ode-import/cross-check.json`). Raw voice identity
and all 245 source events remain in `observations.json`; they are not deleted to
make the counts agree. These two parser/encoding routes share one edition's
provenance, so they are not independent historical sources.

No chord symbols, phrase-family labels, cadence function or performance groove
were encoded as source facts. Those coordinates remain unknown unless a separate
analysis explicitly declares them. Selecting the soprano is an explicit ingestion
choice based on a named source voice, not an inference from highest MIDI pitch.
The normalized `reference.tsv` is profile-independent: cover extraction chooses
which of its observations become invariants later.

## Reproduction

```sh
python3 -m venv target/humanmusic-consolidation/ingest-venv
target/humanmusic-consolidation/ingest-venv/bin/python -m pip install -r scripts/audio/requirements-symbolic.txt
target/humanmusic-consolidation/ingest-venv/bin/python -m unittest discover -s scripts/audio -p 'test_*.py' -v
target/humanmusic-consolidation/ingest-venv/bin/python scripts/audio/import_symbolic.py \
  --lily docs/fixtures/humanmusic-cover/sources/mutopia-528/ode.ly \
  --midi docs/fixtures/humanmusic-cover/sources/mutopia-528/ode.mid \
  --voice sop --voice alto --voice tenor --voice bass \
  --midi-group upper=sop,alto --midi-group lower=tenor,bass \
  --out target/humanmusic-consolidation/ode-import-replay
```

The importer supports absolute pitches, an outer transpose, inherited/dotted
power-of-two durations, rests, ties, slurs and barlines. It rejects relative
pitch, repeats, tuplets, chords, grace syntax, includes, unknown commands and
metadata changes that need a timeline. It does not execute LilyPond input. MIDI
parsing admits metrical synchronous SMF 0/1 and records source message origins;
encoded gates are not acoustical lifetimes. MIDI track/channel is not logical
voice identity. The seven parser controls include malformed syntax, missing
metadata, source pitch/rhythm mutations, ties/rests, tempo/meter/key changes and
MIDI note pairing. Future parsers should emit the same observations rather than
choosing cover identity themselves.

## Swing & A Miss: partial declaration

The maintainer's later provisional skeleton is the current source declaration:
about 179 seconds, approximately 95 BPM, 4/4, A major/Ionian, provisional straight
feel. Verse/pre-chorus E–Bm–F#m–A; chorus E–Bm–D–A; post-chorus Bm–D–A–E;
bridge F#m–D–A. The named section order is known provisionally; exact section
lengths, melody, bass, drums, rhythmic cells, voicings and production are absent.

The earlier supplied chart used D–Am–Em–G and D–Am–C–G, with a different
post-chorus ordering. The later declaration supersedes that working skeleton;
this is a recorded source discrepancy, not independently verified key/capo evidence.
Neither chart's lyrics are retained. Approximate total duration does not establish
exact bar counts. A partial-cover lift may choose a target schedule but must label
that schedule as generated, never imported source identity.

Questions parked for the maintainer: confirm the key/capo/chart discrepancy;
supply lawful exact melody and canonical rhythmic/rest data; confirm section
lengths and harmonic change positions, tempo and groove; identify which riff,
bass figure and orchestration details a listener recognizes. No current output
can establish a recognizable full Swing & A Miss cover without those bearings.
