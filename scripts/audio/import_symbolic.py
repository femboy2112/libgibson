#!/usr/bin/env python3
"""Narrow symbolic observation importer, independent of CoverMap profile selection.

Supports absolute-pitch LilyPond named voices with inherited/dotted durations,
rests, ties, slurs, barlines, and an outer transposition. Rejects relative pitch,
repeats, tuplets, chords, grace syntax, includes and unknown commands. SMF parsing
uses pinned mido; it corroborates encoded note gates, never acoustical lifetimes.
No harmony analysis, lyric ingestion, MIDI playback, or source-specific melody.
"""
import argparse
from collections import Counter, defaultdict, deque
from fractions import Fraction
import hashlib
import importlib.metadata
import json
from pathlib import Path
import re

PITCH = r"[a-g](?:isis|eses|is|es)?[,']*"
NOTE = re.compile(rf"(?P<pitch>{PITCH}|r)(?P<duration>[0-9]*)(?P<dots>\.*)")


def pitch_number(text):
    match = re.fullmatch(r"([a-g])(isis|eses|is|es)?([,']*)", text)
    if not match:
        raise ValueError(f"unsupported pitch: {text!r}")
    name, accidental, octave = match.groups()
    return 48 + {'c': 0, 'd': 2, 'e': 4, 'f': 5, 'g': 7, 'a': 9, 'b': 11}[name] + {
        None: 0, 'is': 1, 'isis': 2, 'es': -1, 'eses': -2
    }[accidental] + 12 * (octave.count("'") - octave.count(','))


def rational(value):
    return [value.numerator, value.denominator]


def assignment(text, name):
    text = re.sub(r'%[^\n]*', '', text)
    pattern = rf"(?m)^\s*{re.escape(name)}\s*=\s*(?:\\transpose\s+({PITCH})\s+({PITCH})\s*)?\{{"
    match = re.search(pattern, text)
    if not match:
        raise ValueError(f"missing or unsupported absolute voice assignment {name!r}")
    start = match.end()
    end = text.find('}', start)
    if end < 0 or '{' in text[start:end]:
        raise ValueError(f"nested/unterminated voice {name!r}")
    transposition = 0 if match[1] is None else pitch_number(match[2]) - pitch_number(match[1])
    return text[start:end], transposition


def lily_voice(text, name, artifact):
    body, transpose = assignment(text, name)
    body = re.sub(r'\\bar\s+"[^"\n]*"', ' ', body)
    body = re.sub(r'\\voice(?:One|Two|Three|Four)\b', ' ', body)
    body = re.sub(r'[\[\]()|]', ' ', body)  # grouping/slurs do not change encoded gates
    at, duration = Fraction(0), Fraction(1)
    events, token, offset, tie = [], 0, 0, False
    while offset < len(body):
        if body[offset].isspace():
            offset += 1
            continue
        if body[offset] == '~':
            if tie or not events or Fraction(*events[-1]['onset']) + Fraction(*events[-1]['duration']) != at:
                raise ValueError('tie without adjacent note')
            tie = True
            offset += 1
            continue
        match = NOTE.match(body, offset)
        if not match:
            raise ValueError(f"unsupported LilyPond syntax in {name}: {body[offset:offset+30]!r}")
        offset = match.end()
        if match['duration']:
            denominator = int(match['duration'])
            if denominator == 0 or denominator & (denominator - 1):
                raise ValueError('duration must be a positive power of two')
            duration = Fraction(4, denominator) * (2 - Fraction(1, 2 ** len(match['dots'])))
        elif match['dots']:
            raise ValueError('dots without explicit duration are outside this importer subset')
        pitch = match['pitch']
        origin = {'artifact_sha256': artifact, 'voice': name, 'note_token': token}
        if pitch == 'r':
            if tie:
                raise ValueError('tie into rest')
        else:
            midi = pitch_number(pitch) + transpose
            if not 0 <= midi <= 127:
                raise ValueError('pitch outside MIDI range')
            if tie:
                if events[-1]['pitch'] != midi:
                    raise ValueError('tie changes pitch')
                events[-1]['duration'] = rational(Fraction(*events[-1]['duration']) + duration)
                events[-1]['origins'].append(origin)
                tie = False
            else:
                events.append({'onset': rational(at), 'duration': rational(duration),
                               'pitch': midi, 'origins': [origin]})
        at += duration
        token += 1
    if tie:
        raise ValueError('unterminated tie')
    return {'name': name, 'length': rational(at), 'notes': events,
            'relation': 'source named voice; absolute pitches plus declared transposition'}


def lily_observations(path, voices):
    data = path.read_bytes()
    artifact = hashlib.sha256(data).hexdigest()
    text = data.decode('utf-8')
    clean = re.sub(r'%[^\n]*', '', text)
    meters = re.findall(r'\\time\s+(\d+)/(\d+)', clean)
    if len(set(meters)) > 1:
        raise ValueError('meter changes require a timeline-capable importer')
    meter = re.search(r'\\time\s+(\d+)/(\d+)', clean)
    keys = re.findall(rf'\\key\s+({PITCH})\s+\\(major|minor)\b', clean)
    if len(set(keys)) > 1:
        raise ValueError('key changes require a timeline-capable importer')
    key = re.search(rf'\\key\s+({PITCH})\s+\\(major|minor)\b', clean)
    tempos = re.findall(r'\\tempo\s+(\d+)\s*=\s*(\d+)', clean)
    if len(set(tempos)) > 1:
        raise ValueError('tempo changes require a timeline-capable importer')
    tempo = re.search(r'\\tempo\s+(\d+)\s*=\s*(\d+)', clean)
    return {'schema': 'humanmusic-symbolic-observations/v1', 'source_sha256': artifact,
            'parser': 'import_symbolic.py absolute-lily/v1',
            'meter': None if meter is None else [int(meter[1]), int(meter[2])],
            'key': None if key is None else {'tonic_pc': pitch_number(key[1]) % 12, 'mode': key[2]},
            'quarter_bpm': None if tempo is None else rational(Fraction(int(tempo[2]) * 4, int(tempo[1]))),
            'voices': [lily_voice(text, name, artifact) for name in voices],
            'harmony': None, 'phrase_families': None,
            'limits': ['No inferred chord labels, phrase families, expressive timing, or lyric data.',
                       'Bar positions follow meter; cadence meaning is not imported as a score fact.']}


def midi_observations(path):
    import mido
    f = mido.MidiFile(path)
    if f.type not in (0, 1) or f.ticks_per_beat <= 0:
        raise ValueError('only synchronous metrical SMF 0/1 is supported')
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    tracks, metadata = [], []
    for ti, track in enumerate(f.tracks):
        tick, active, notes = 0, defaultdict(deque), []
        for ei, msg in enumerate(track):
            tick += msg.time
            origin = {'artifact_sha256': digest, 'track': ti, 'event': ei}
            if msg.type in ('time_signature', 'key_signature', 'set_tempo'):
                metadata.append({'tick': tick, 'origin': origin, 'event': msg.dict()})
            if msg.type == 'note_on' and msg.velocity > 0:
                active[(msg.channel, msg.note)].append((tick, msg.velocity, origin))
            elif msg.type == 'note_off' or (msg.type == 'note_on' and msg.velocity == 0):
                queue = active[(msg.channel, msg.note)]
                if not queue:
                    raise ValueError('unmatched note off')
                start, velocity, beginning = queue.popleft()
                if tick <= start:
                    raise ValueError('nonpositive note duration')
                notes.append({'onset': rational(Fraction(start, f.ticks_per_beat)),
                              'duration': rational(Fraction(tick-start, f.ticks_per_beat)),
                              'pitch': msg.note, 'velocity': velocity, 'channel': msg.channel,
                              'origins': [beginning, origin]})
        if any(active.values()):
            raise ValueError('unterminated MIDI note')
        tracks.append({'track': ti, 'name': track.name, 'notes': notes})
    return {'schema': 'humanmusic-midi-observations/v1', 'source_sha256': digest,
            'parser': 'mido ' + importlib.metadata.version('mido'), 'format': f.type,
            'ticks_per_quarter': f.ticks_per_beat, 'metadata': metadata, 'tracks': tracks,
            'limits': ['Tracks/channels do not establish logical voice identity.',
                       'Durations are encoded note gates; pedal and synthesis are not inferred.',
                       'Overlapping same-pitch events pair FIFO; no expressive voice inference.']}


def signature(notes):
    return Counter((tuple(n['onset']), tuple(n['duration']), n['pitch']) for n in notes)


def write_tsv(observations, path):
    """A normalized observation exchange, before cover/profile selection."""
    obs = observations
    rows = []
    if obs['meter'] is not None:
        rows.append(['meter', *map(str, obs['meter'])])
    if obs['key'] is not None:
        rows.append(['key', str(obs['key']['tonic_pc']), obs['key']['mode']])
    if obs['quarter_bpm'] is not None:
        rows.append(['tempo', str(float(Fraction(*obs['quarter_bpm'])))])
    length = max(Fraction(*v['length']) for v in obs['voices'])
    rows.append(['length', f'{length.numerator}/{length.denominator}'])
    for voice in obs['voices']:
        for n in voice['notes']:
            rows.append(['note', voice['name'], '/'.join(map(str,n['onset'])),
                         '/'.join(map(str,n['duration'])), str(n['pitch'])])
    path.write_text(''.join('\t'.join(row)+'\n' for row in rows))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--lily', required=True, type=Path)
    parser.add_argument('--voice', required=True, action='append')
    parser.add_argument('--midi', type=Path)
    parser.add_argument('--midi-group', action='append', default=[],
                        help='Explicit source staff projection: MIDI_TRACK=VOICE,VOICE; coalesces exact unisons only')
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    obs = lily_observations(args.lily, args.voice)
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out/'observations.json').write_text(json.dumps(obs, indent=2)+'\n')
    write_tsv(obs, args.out/'reference.tsv')
    if args.midi:
        midi = midi_observations(args.midi)
        (args.out/'midi-observations.json').write_text(json.dumps(midi, indent=2)+'\n')
        expected = signature(n for v in obs['voices'] for n in v['notes'])
        actual = signature(n for t in midi['tracks'] for n in t['notes'])
        receipt = {'schema': 'symbolic-cross-check/v1', 'match': expected == actual,
                   'lily_notes': sum(expected.values()), 'midi_notes': sum(actual.values()),
                   'missing_in_midi': [list(k)+[v] for k,v in (expected-actual).items()],
                   'extra_in_midi': [list(k)+[v] for k,v in (actual-expected).items()],
                   'boundary': 'Different parser/encoding; same Mutopia edition provenance, not independent historical sources.'}
        groups = []
        used_voices, used_tracks = set(), set()
        for group in args.midi_group:
            track, names = group.split('=')
            names = names.split(',')
            if used_voices.intersection(names) or track in used_tracks:
                raise ValueError('voice/track reused in cross-check projection')
            used_voices.update(names)
            used_tracks.add(track)
            source_voices = [v for v in obs['voices'] if v['name'] in names]
            target_tracks = [t for t in midi['tracks'] if t['name'] == track]
            if len(source_voices) != len(names) or len(target_tracks) != 1:
                raise ValueError('missing/ambiguous declared MIDI group')
            source = signature(n for v in source_voices for n in v['notes'])
            target = signature(target_tracks[0]['notes'])
            projected = Counter({k: 1 for k in source})
            groups.append({'track':track,'source_voices':names,
                           'source_notes':sum(source.values()),'midi_notes':sum(target.values()),
                           'coalesced_exact_unisons':sum(source.values())-len(projected),
                           'match':projected==target})
        if groups:
            if used_voices != {v['name'] for v in obs['voices']} or used_tracks != {t['name'] for t in midi['tracks'] if t['notes']}:
                raise ValueError('cross-check projection does not cover every note-bearing source/track')
            receipt['declared_projection'] = 'Within each explicitly named staff, exact simultaneous unison gates coalesce; Lily voice identity remains in observations.'
            receipt['groups'] = groups
            receipt['projected_match'] = all(g['match'] for g in groups)
        (args.out/'cross-check.json').write_text(json.dumps(receipt,indent=2)+'\n')
        if not receipt['match'] and not receipt.get('projected_match', False):
            raise SystemExit('Lily/MIDI disagreement preserved; do not promote ambiguous observation')
    print(json.dumps({'voices':[(v['name'],len(v['notes']),v['length']) for v in obs['voices']],
                      'meter':obs['meter'],'key':obs['key'],'quarter_bpm':obs['quarter_bpm']}))


if __name__ == '__main__':
    main()
