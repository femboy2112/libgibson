#!/usr/bin/env python3
"""Seal Round XVII lab outputs without touching scores or synthesized samples."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

from humanmusic_r17_witnesses import articulation, connective_blip_witness, lattice_witness, parse_note, read_notes


def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as reader:
        for block in iter(lambda: reader.read(1024 * 1024), b''):
            value.update(block)
    return value.hexdigest()


def mixed_contrast(values, mask):
    return sum((-1) ** (mask.bit_count()-bits.bit_count()) * values[bits]
               for bits in range(16) if bits & mask == bits)


def calibrate_factorial():
    points = {bits: 5 + 3 * bool(bits & 1) + 2 * bool(bits & 2)
              - 7 * bool(bits & 1) * bool(bits & 2)
              + bool(bits & 2) * bool(bits & 8) for bits in range(16)}
    expected = {0: 5, 1: 3, 2: 2, 3: -7, 10: 1}
    assert all(mixed_contrast(points, mask) == expected.get(mask, 0) for mask in range(16))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--copy-receipts', type=Path)
    parser.add_argument('--source-head', help='committed production authority; verified against current src/audio before sealing')
    parser.add_argument('--require-audio', action='store_true', help='require all82 planned WAVs, all32 historical controls and both all-off PCM equalities')
    args = parser.parse_args()
    calibrate_factorial()
    directory = args.directory
    root = Path(__file__).resolve().parents[2]
    historical = root / 'docs/fixtures/humanmusic-r16/final'
    if args.require_audio:
        stems = ['full', 'lead', 'bass', 'keys', 'pad', 'lead_bass', 'support', 'rhythm']
        expected = {f'{w}_{arm}.{stem}.wav' for w in ['black_ice', 'swiss']
                    for arm in ['r14', 'r16', 'r17'] for stem in stems}
        expected.update(f'{w}_f{bits:04b}.full.wav' for w in ['black_ice', 'swiss'] for bits in range(16))
        expected.update(f'{w}_stable_precursors.full.wav' for w in ['black_ice', 'swiss'])
        actual = {p.name for p in directory.glob('*.wav')}
        assert actual == expected, dict(missing=sorted(expected-actual), unexpected=sorted(actual-expected))
        assert len(expected) == 82

    controls = []
    for world in ['black_ice', 'swiss']:
        for arm in ['r14', 'r16']:
            for suffix in ['notes.txt', 'drums.txt']:
                filename = f'{world}_{arm}.{suffix}'
                assert (directory / filename).read_bytes() == (historical / filename).read_bytes(), filename
                controls.append(filename + ' byte-exact historical input')
        for suffix in ['notes.txt', 'drums.txt']:
            assert (directory / f'{world}_f0000.{suffix}').read_bytes() == (directory / f'{world}_r16.{suffix}').read_bytes()
        full = directory / f'{world}_f0000.full.wav'
        if full.exists():
            assert digest(full) == digest(directory / f'{world}_r16.full.wav')
            controls.append(full.name + ' byte-exact R16 PCM')
    if args.require_audio:
        for stem in stems:
            assert digest(directory / f'swiss_r17.{stem}.wav') == digest(directory / f'swiss_r16.{stem}.wav'), stem
        controls.append('SWISS support-only R17 all8 stems byte-exact R16 (NO-GO preserved)')
        for world in ['black_ice', 'swiss']:
            for bits in range(8):
                assert digest(directory / f'{world}_f{bits:04b}.full.wav') == digest(directory / f'{world}_f{bits | 8:04b}.full.wav'), (world,bits)
        controls.append('support-factor paired full mixes byte-exact for all16 world/configuration pairs')
    old_wav_hashes = dict((line.split('  ', 1)[1], line.split('  ', 1)[0])
                          for line in (historical / 'SHA256SUMS').read_text().splitlines())
    historical_wavs = 0
    for path in directory.glob('*.wav'):
        if path.name in old_wav_hashes:
            historical_wavs += 1
            assert digest(path) == old_wav_hashes[path.name], path.name
            controls.append(path.name + ' historical SHA256 verified')
    if args.require_audio:
        assert historical_wavs == 32, historical_wavs
    (directory / 'historical-controls.txt').write_text('\n'.join(controls) + '\n')
    lattice = {path.name.removesuffix('.notes.txt'): len(lattice_witness(read_notes(path)))
               for path in sorted(directory.glob('*.notes.txt'))}
    assert lattice['black_ice_r17'] == 0
    assert lattice['swiss_r17'] == 0
    (directory / 'lattice-counts.json').write_text(json.dumps(lattice, indent=2) + '\n')
    sums = [f'{digest(path)}  {path.name}' for path in sorted(directory.glob('*.wav'))]
    (directory / 'SHA256SUMS').write_text('\n'.join(sums) + '\n')
    rows = []
    for path in sorted(directory.glob('*.perturbation.txt')):
        for line in path.read_text().splitlines():
            match = re.fullmatch(r'(pad|keys|bass|lead) old=(\d+) new=(\d+) retimed=(\d+) duration=(\d+) velocity=(\d+) pitch=(\d+) omitted=(\d+) added=(\d+)', line)
            if match:
                keys = ['old', 'new', 'retimed', 'gate', 'velocity', 'pitch', 'omitted', 'added']
                rows.append(dict(comparison=path.stem.removesuffix('.perturbation'), role=match[1], **dict(zip(keys, map(int, match.groups()[1:])))))
    connectives = []
    for path in sorted(directory.glob('*.r14-*.perturbation.txt')):
        world, comparison = path.name.split('.r14-', 1)
        arm = comparison.removesuffix('.perturbation.txt')
        bpm = 88 if world == 'black_ice' else 118
        old_notes = read_notes(directory / f'{world}_r14.notes.txt')
        new_notes = read_notes(directory / f'{world}_{arm}.notes.txt')
        before = None
        for line in path.read_text().splitlines():
            if line.startswith('before='):
                before = parse_note(line)
            elif line.startswith('after='):
                after = parse_note(line)
                if before.role not in ('Lead', 'Bass') or abs(before.start - after.start) < 1e-6:
                    continue
                old = articulation(before, old_notes, bpm)
                new = articulation(after, new_notes, bpm)
                connective = before.function in ('ChromaticApproach', 'Neighbor', 'ChromaticPassing', 'DiatonicPassing', 'Enclosure')
                connectives.append(dict(world=world, arm=arm, role=after.role, pitch=after.pitch,
                    source_function=before.function, source_onset=before.start, performed_onset=after.start,
                    before=old, after=new, source_connective=connective,
                    blip=connective_blip_witness(connective, new)))
    for world in ['black_ice', 'swiss']:
        assert not any(r['blip'] for r in connectives if r['world'] == world and r['arm'] == 'r17'), world
    (directory / 'connective-comparison.json').write_text(json.dumps(dict(
        boundary='Written gate silence, not PCM silence; source-ledger optional retiming cohort, source connective function retained.',
        rows=connectives), indent=2) + '\n')
    (directory / 'perturbation-summary.json').write_text(json.dumps(rows, indent=2) + '\n')
    # Complete Boolean-factorial Mobius contrasts, with all factors-off as reference.
    # These are signed mixed count differences, not interaction mechanisms or quality scores.
    retained = {}
    for line in (directory / 'summary.txt').read_text().splitlines():
        match = re.search(r'^(\w+)_f([01]{4}) .*retained_unresolved=(\d+)', line)
        if match:
            retained[(match[1], int(match[2], 2))] = int(match[3])
    factorial = []
    for world in ['black_ice', 'swiss']:
        values = {}
        for bits in range(16):
            arm = f'f{bits:04b}'
            point = {'off_lattice': lattice[f'{world}_{arm}'],
                     'retimed_source_connective_blips': sum(r['blip'] for r in connectives if r['world'] == world and r['arm'] == arm),
                     'retained_unresolved': retained[(world, bits)]}
            selected = [r for r in rows if r['comparison'] == f'{world}.r16-{arm}']
            assert len(selected) == 4, (world, arm, len(selected))
            for row in selected:
                for key in ['old', 'new', 'retimed', 'gate', 'velocity', 'pitch', 'omitted', 'added']:
                    point[f'{row["role"]}.{key}'] = row[key]
            values[bits] = point
        contrasts = {}
        for mask in range(16):
            contrasts[f'{mask:04b}'] = {key: mixed_contrast({bits: values[bits][key] for bits in range(16)}, mask) for key in values[0]}
        for bits in range(16):
            for key in values[0]:
                assert sum(contrasts[f'{mask:04b}'][key] for mask in range(16) if mask & bits == mask) == values[bits][key]
        support_noop = all(values[bits] == values[bits ^ 8] for bits in range(8))
        assert support_noop, 'unadmitted support factor unexpectedly changed measurements'
        factorial.append(dict(world=world, factor_bits={'0': 'lattice_positions', '1': 'legato_connectives', '2': 'mono_voice', '3': 'support_top_voice'},
                              measurements={f'{k:04b}': v for k,v in values.items()},
                              mobius_contrasts=contrasts, support_factor_is_noop=support_noop))
    (directory / 'factorial-interactions.json').write_text(json.dumps(dict(
        definition='Delta_S f(0) = sum over T subset S of (-1)^(|S|-|T|) f(T). Empty S is baseline; singleton is main change; order>=2 is mixed difference.',
        boundary='Complete finite factorial count contrasts. Non-additivity does not identify mechanism; all receipts share implementation provenance, no sound-quality score.',
        rows=factorial), indent=2) + '\n')
    columns = ['comparison', 'role', 'old', 'new', 'retimed', 'gate', 'velocity', 'pitch', 'omitted', 'added']
    table = ['| ' + ' | '.join(columns) + ' |', '|' + '|'.join(['---'] * len(columns)) + '|']
    table.extend('| ' + ' | '.join(str(row[key]) for key in columns) + ' |' for row in rows)
    (directory / 'perturbation-summary.md').write_text('\n'.join(table) + '\n')
    tracked = subprocess.check_output(['git', 'ls-files', 'src/audio', 'examples/pocket_music_lab.rs', 'tests/audio_pocket_integration.rs'], cwd=root, text=True).splitlines()
    # Include newly created Rust production files before their first commit as well.
    tracked = sorted(set(tracked) | {str(p.relative_to(root)) for p in (root / 'src/audio/human_music').glob('*.rs')})
    source_head = subprocess.check_output(['git', 'rev-parse', args.source_head or 'HEAD'], cwd=root, text=True).strip()
    subprocess.run(['git', 'diff', '--quiet', source_head, '--', 'src/audio'], cwd=root, check=True)
    provenance = dict(
        production_head=source_head,
        production_source_sha256={name: digest(root / name) for name in tracked},
        artifact_tools_sha256={name: digest(root / name) for name in ['examples/pocket_music_lab.rs', 'scripts/dev/humanmusic_r17_receipts.py']},
        render_directory=str(directory.resolve()),
        factor_order='high-to-low bits: support_top_voice, mono_voice, legato_connectives, lattice_positions; stable_precursors separately gated',
        human_acceptance={'arm': 'BLACK_ICE R17 full', 'status': 'Observed maintainer audition', 'quote': 'in the pocket, much better, but still a little grid/robotic. this is a good place to call it for this round, but make note. good work', 'boundary': 'Flagship timing accepted with residual grid/robotic feel; does not override failed fresh sweep or SWISS source NO-GO.'},
        support_factor_status='Maintainer accepted C4 knockout and rejected G5 knockout; C4 implicated, source selector NO-GO under unchanged ruler',
        boundary='Machine-generated score, envelope and PCM receipts share implementation provenance. No audio audition by the agent. Zero lattice violations is not timing acceptance; full design does not establish sound quality or mechanism.')
    (directory / 'provenance.json').write_text(json.dumps(provenance, indent=2) + '\n')
    if args.copy_receipts:
        args.copy_receipts.mkdir(parents=True, exist_ok=True)
        for path in directory.iterdir():
            if path.is_file() and path.suffix != '.wav':
                (args.copy_receipts / path.name).write_bytes(path.read_bytes())
    print(f'{len(sums)} WAV hashes; {len(rows)} perturbation role rows; receipts sealed')


if __name__ == '__main__':
    main()
