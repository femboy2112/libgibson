#!/usr/bin/env python3
"""Frozen R17 score witnesses; standard library, no production imports or score edits.

The primary blip witness is retimed optional connective AND written gate silence
before the next same-role onset >40 ms. It is not a claim of acoustic silence.
R14's unretimed authored rests are controls, not grounds to relax the threshold.
"""
import argparse
from collections import Counter
from dataclasses import dataclass
import hashlib
import json
import math
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'docs/fixtures/humanmusic-r16/final'
sys.path.insert(0, str(ROOT / 'scripts/fixtures'))
import humanmusic_archive  # noqa: E402  (compacted fixtures are restored on demand)


def expand_r16_source():
    """R16 dumps are stored compacted; restore any missing ones in place (see docs/fixtures/HUMANMUSIC_FIXTURE_MANIFEST.md)."""
    return humanmusic_archive.ensure_expanded(ROOT, 'docs/fixtures/humanmusic-r16/final/')
FROZEN_HEAD = 'c9242ff154fc60d9321995126af12f18574dd88e'
EPS = 1e-6
SILENCE_LIMIT_MS = 40.0


@dataclass(frozen=True)
class Note:
    start: float
    gate: float
    pitch: int
    role: str
    function: str
    raw: str


def parse_note(line):
    match = re.search(r'Note \{ start_beat: ([^,]+), dur_beats: ([^,]+), pitch: (\d+), velocity: [^,]+, role: (\w+),', line)
    if not match:
        raise ValueError(f'unrecognized note: {line[:100]}')
    function = re.search(r'function: Some\((\w+)\)', line)
    return Note(float(match[1]), float(match[2]), int(match[3]), match[4],
                function[1] if function else 'None', line[line.index('Note {'):])


def read_notes(path):
    return [parse_note(line) for line in path.read_text().splitlines() if line]


def groove_position(beat, swing):
    # Independent direct transcription of the frozen world's operational transport.
    eighth = math.floor(beat * 2 + 0.5)
    if swing > 0 and abs(beat * 2 - eighth) < EPS and eighth % 2:
        return beat + swing * 0.25
    return beat


def on_lattice(beat, subdiv=4, swing=0.0, surface_subdivision=4):
    divisions = {subdiv}
    if subdiv == 3 or surface_subdivision == 3:
        divisions.add(3)
    for division in divisions:
        nearest = math.floor(beat * division + 0.5)
        # Swing can move the nominal closest point across a rounding boundary.
        for slot in range(nearest - 2, nearest + 3):
            if abs(beat - groove_position(slot / division, swing)) < EPS:
                return True
    return False


def lattice_witness(notes, subdiv=4, swing=0.0, surface_subdivision=4):
    return [n for n in notes if not on_lattice(n.start, subdiv, swing, surface_subdivision)]


def successor(note, notes):
    later = [n for n in notes if n.role == note.role and n.start > note.start + EPS]
    return min(later, key=lambda n: n.start) if later else None


def articulation(note, notes, bpm):
    target = successor(note, notes)
    if target is None:
        return dict(destination=None, gate_ms=note.gate * 60000 / bpm,
                    silence_ms=None, legato_ratio=None)
    ioi = target.start - note.start
    return dict(destination=target.start, gate_ms=note.gate * 60000 / bpm,
                silence_ms=max(0.0, ioi - note.gate) * 60000 / bpm,
                legato_ratio=note.gate / ioi)


def connective_blip_witness(retimed, metrics):
    return retimed and metrics['silence_ms'] is not None and metrics['silence_ms'] > SILENCE_LIMIT_MS + EPS


def calibration():
    assert on_lattice(7.75) and not on_lattice(7.625)
    assert not on_lattice(7 + 1/3) and on_lattice(7 + 1/3, 3)
    assert on_lattice(7 + 1/3, surface_subdivision=3)
    assert on_lattice(7.625, swing=0.5) and not on_lattice(7.5, swing=0.5)
    assert on_lattice(7.25, swing=0.5)
    assert not on_lattice(7.7501), 'pitched onsets have no drum-jitter tolerance'
    assert not connective_blip_witness(False, {'silence_ms': 340.0})
    assert connective_blip_witness(True, {'silence_ms': 41.0})
    assert not connective_blip_witness(True, {'silence_ms': 40.0})
    assert not connective_blip_witness(True, {'silence_ms': 8.5})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'docs/fixtures/humanmusic-r17/baseline')
    parser.add_argument('--wav-directory', type=Path, default=ROOT / 'target/humanmusic-r16/final')
    args = parser.parse_args()
    calibration()
    expand_r16_source()
    args.output.mkdir(parents=True, exist_ok=True)
    inputs = sorted(SOURCE.glob('*.txt')) + [SOURCE / 'SHA256SUMS', SOURCE / 'provenance.json']
    manifest = []
    for path in inputs:
        relative = str(path.relative_to(ROOT))
        frozen = subprocess.check_output(['git', 'show', f'{FROZEN_HEAD}:{relative}'], cwd=ROOT)
        assert path.read_bytes() == frozen, f'historical input modified: {relative}'
        manifest.append(f'{hashlib.sha256(frozen).hexdigest()}  {relative}')
    (args.output / 'SOURCE_SHA256SUMS').write_text('\n'.join(manifest) + '\n')
    for name in ['summary.txt', 'SHA256SUMS', 'provenance.json']:
        (args.output / ('r16-' + name)).write_bytes((SOURCE / name).read_bytes())
    wav_checked = 0
    for line in (SOURCE / 'SHA256SUMS').read_text().splitlines():
        expected, filename = line.split('  ', 1)
        path = args.wav_directory / filename
        if not path.exists():
            continue
        assert hashlib.sha256(path.read_bytes()).hexdigest() == expected, filename
        wav_checked += 1
    expected = {'black_ice_r14': 0, 'black_ice_r15': 34, 'black_ice_r16': 15,
                'swiss_r14': 0, 'swiss_r15': 0, 'swiss_r16': 0}
    lattice = {}
    note_sets = {}
    for arm, count in expected.items():
        notes = read_notes(SOURCE / f'{arm}.notes.txt')
        note_sets[arm] = notes
        offenders = lattice_witness(notes)
        assert len(offenders) == count, (arm, len(offenders), count)
        lattice[arm] = dict(count=len(offenders), by_role=dict(sorted(Counter(n.role for n in offenders).items())),
                            offenders=[dict(role=n.role, onset=n.start, pitch=n.pitch) for n in offenders])
    assert lattice['black_ice_r15']['by_role'] == {'Bass': 16, 'Keys': 1, 'Lead': 17}
    assert lattice['black_ice_r16']['by_role'] == {'Bass': 7, 'Keys': 1, 'Lead': 7}
    assert all(abs(n['onset'] % 1 - 0.625) < EPS for n in lattice['black_ice_r16']['offenders'])
    # R14 and SWISS finite controls show exact pitched positions, without any jitter allowance.
    # Drum accent followers may inherit an R16 off-lattice lead; drums are not this gate's subject.
    drum_controls = {}
    for arm in ['black_ice_r14', 'swiss_r14', 'swiss_r16']:
        starts = [float(re.search(r'start_beat: ([^,]+)', line)[1])
                  for line in (SOURCE / f'{arm}.drums.txt').read_text().splitlines()]
        residuals = [abs(b - round(b * 4) / 4) for b in starts]
        assert max(residuals) <= 0.008 + EPS, arm
        assert any(r > EPS for r in residuals), arm
        drum_controls[arm] = dict(count=len(starts), max_abs_lattice_residual=max(residuals))
    rows = []
    before = None
    for line in (SOURCE / 'black_ice.r14-r16.perturbation.txt').read_text().splitlines():
        if line.startswith('before='):
            before = parse_note(line)
        elif line.startswith('after='):
            after = parse_note(line)
            assert before is not None
            if before.role not in ('Lead', 'Bass') or abs(before.start - after.start) < EPS:
                continue
            old = articulation(before, note_sets['black_ice_r14'], 88)
            new = articulation(after, note_sets['black_ice_r16'], 88)
            rows.append(dict(role=after.role, pitch=after.pitch, function=after.function,
                             before_onset=before.start, after_onset=after.start,
                             before=old, after=new,
                             r14_fails=connective_blip_witness(False, old),
                             connective=after.function in ('ChromaticApproach', 'Neighbor', 'ChromaticPassing', 'DiatonicPassing', 'Enclosure'),
                             all_retimed_gap=connective_blip_witness(True, new),
                             r16_fails=connective_blip_witness(after.function in ('ChromaticApproach', 'Neighbor', 'ChromaticPassing', 'DiatonicPassing', 'Enclosure'), new)))
    assert len(rows) == 35, len(rows)
    assert not any(row['r14_fails'] for row in rows)
    assert any(row['r16_fails'] for row in rows)
    receipt = dict(status='Verified finite frozen score measurements; human acceptance not inferred',
                   source_head=FROZEN_HEAD, source_family='R16 committed score/debug receipts',
                   lattice=lattice, drums=drum_controls, wav_hashes_checked=wav_checked,
                   blip_definition='Retimed optional connective AND written gate silence to next same-role onset >40 ms; no PCM-silence claim.',
                   r14_blips=sum(row['r14_fails'] for row in rows),
                   r16_blips=sum(row['r16_fails'] for row in rows),
                   all_retimed_gap_failures=sum(row['all_retimed_gap'] for row in rows),
                   connective_rows=rows)
    (args.output / 'witnesses.json').write_text(json.dumps(receipt, indent=2) + '\n')
    lines = ['Round XVII frozen witnesses', f'Source: {FROZEN_HEAD}',
             'Verified: finite committed-score measurements; no listening inference.',
             'Lattice: world.subdiv transported by groove_position; triplets only if world.subdiv==3 or language.surface_subdivision==3.',
             'Straight BLACK/SWISS: subdiv=4, swing=0; pitched tolerance 1e-6 beat (numerical only), no stochastic jitter.',
             'Drums: seeded +/-0.008 beat retained and excluded from pitched lattice gate.',
             f'WAV hashes verified against available historical files: {wav_checked}/84.', '']
    for arm, result in lattice.items():
        lines.append(f'{arm}: off_lattice={result["count"]} by_role={result["by_role"]}')
        for n in result['offenders']:
            lines.append(f'  {n["role"]} onset={n["onset"]:.9f} pitch={n["pitch"]}')
    lines += ['', 'Blip primary: retimed optional connective AND written gate silence >40 ms.',
              'R14 authored rests are not retimed; table retains their measured gaps.',
              'Primary connective cohort: ChromaticApproach/Neighbor/ChromaticPassing/DiatonicPassing/Enclosure; stable fragment precursors remain in table and all-retimed gap count.',
              'Destination = immediate later onset in same role; articulation/gate proxy only, not release-tail or PCM silence.',
              f'R14 failures=0; R16 failures={receipt["r16_blips"]}; retimed cohort={len(rows)}; all-retimed gaps={receipt["all_retimed_gap_failures"]}.', '',
              '| Role | Pitch | Function | Onset R14→R16 | Destination R14→R16 | Gate ms R14→R16 | Silence ms R14→R16 | Legato ratio R14→R16 | R16 fail |',
              '|---|---:|---|---|---|---|---|---|---|']
    for row in rows:
        old, new = row['before'], row['after']
        lines.append(f'| {row["role"]} | {row["pitch"]} | {row["function"]} | {row["before_onset"]:g}→{row["after_onset"]:g} | {old["destination"]:g}→{new["destination"]:g} | {old["gate_ms"]:.3f}→{new["gate_ms"]:.3f} | {old["silence_ms"]:.3f}→{new["silence_ms"]:.3f} | {old["legato_ratio"]:.4f}→{new["legato_ratio"]:.4f} | {row["r16_fails"]} |')
    lines += ['', 'Calibration passes: off-lattice 7.625 rejected in straight world, accepted under swing=0.5; legal triplets conditional; 0.0001 pitched jitter rejected; 40/41-ms blip boundary; unretimed authored rest preserved.',
              'Boundary: source-level optionality inferred from the existing source-ledger retimed cohort; no structural-note classifier or audio detector is fabricated here.',
              'Reproduce: python3 scripts/dev/humanmusic_r17_witnesses.py']
    (args.output / 'witnesses.txt').write_text('\n'.join(lines) + '\n')
    print(f'frozen lattice counts verified; retimed={len(rows)} R14_blips=0 R16_blips={receipt["r16_blips"]}; historical WAV hashes={wav_checked}/84')


if __name__ == '__main__':
    main()
