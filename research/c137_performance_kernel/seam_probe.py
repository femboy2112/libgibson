#!/usr/bin/env python3
"""Pinned source-boundary audit and isolated projection counterexamples.

THIS DOES NOT COMPILE/EXECUTE RUST OR RENDER AUDIO. --repo pins the source
files to the audited commit before examining the program's authority order.
A positive finding means an architectural risk is PRESENT, not a sound-quality fix.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import unittest
from dataclasses import dataclass, replace
from pathlib import Path

BASE = '29527a891ed7bfed9657a99046884a61670897e3'
BLOBS = {
    'src/audio/human_music/functor.rs': '42e5f61612826b5cf2ac9c46f02edf44919935c4',
    'src/audio/human_music/score.rs': '1cf29e7cfe2195670d975193eccb5c0a098897b9',
    'src/audio/human_music/performance.rs': '597ab7a1cec080b3d0d3ce070e9aa7ea1d95fcb5',
    'src/audio/human_music/argument.rs': '2e56d03402f7084013cf1f2817cfd8f528a94664',
    'src/audio/human_music/comp.rs': '1a9a35e13b3d075258a16c0c980bdf6c0907abb9',
    'src/audio/human_music/bass.rs': 'b578510c6e283500a9511ee5b35a79c1ff3fed92',
}

@dataclass(frozen=True)
class Note:
    beat: float
    dur: float
    pitch: int
    function: str
    velocity: float
    source: str = 'germ'


def legacy_hearing_projection(n: Note) -> tuple:
    """Exact fields of current Rust score.rs::heard_note, transcribed."""
    return (n.beat, n.dur, n.pitch, n.function)


def effective_observation(n: Note) -> tuple:
    """One candidate richer view, NOT an already-implemented Rust feature."""
    return (n.beat, n.dur, n.pitch, n.function, n.velocity, n.source)


def inspect_repo(root: Path) -> dict:
    rows = {}
    for name, expected in BLOBS.items():
        p = root / name
        if not p.is_file():
            rows[name] = dict(status='MISSING', expected=expected)
            continue
        b = p.read_bytes()
        digest = hashlib.sha1(b'blob ' + str(len(b)).encode() + b'\0' + b).hexdigest()
        rows[name] = dict(status='MATCH' if digest == expected else 'DRIFT', expected=expected, actual=digest)
    if not all(r['status'] == 'MATCH' for r in rows.values()):
        return dict(status='SOURCE_DRIFT_REVIEW_REQUIRED', files=rows)
    f = (root / 'src/audio/human_music/functor.rs').read_text()
    score = (root / 'src/audio/human_music/score.rs').read_text()
    perf = (root / 'src/audio/human_music/performance.rs').read_text()
    comp = (root / 'src/audio/human_music/comp.rs').read_text()
    bass = (root / 'src/audio/human_music/bass.rs').read_text()
    heard_fn = score.split('fn heard_note(n: &Note) -> HeardNote {', 1)[1].split('\n}', 1)[0]
    # These are narrow source-characterization conditions, not quality PASS gates.
    facts = {
        'legacy_hearing_projection_omits_velocity': 'n.velocity' not in heard_fn and '(n.start_beat, n.dur_beats, n.pitch, n.function)' in heard_fn,
        'drummer_hears_keys_before_late_counter': f.index('Hearing::of("drums", Role::Keys, &keys)') < f.index('super::comp::realize_band_weave'),
        'final_gain_postdates_drum_hearing': f.index('Hearing::of("drums", Role::Keys, &keys)') < f.index('scale_backing_by_section_energy(&mut keys, argument)'),
        'argument_final_chord_after_general_plan': f.index('PerformancePlan::from_song_admitted(') < f.index('super::argument::prepare_performance('),
        'driver_enabled_after_general_plan': f.index('PerformancePlan::from_song_admitted(') < f.index('perf.drive = true;'),
        'bass_weave_explicitly_held': 'const BASS_WEAVE: bool = false;' in comp,
        'keys_listen_by_onset_count': 'KEYS_LISTEN_DENSE' in comp and 'lead_onsets' in comp,
        'bass_drive_fixed_eighth_grid': 'onsets.extend((0..STEPS).step_by(2));' in bass,
        'admit_take_only_tests_stage': 'if orchestration_violations(perf, score).is_empty()' in f,
        'old_coupledR8_is_a_rejected_control': 'the rejected negative' in perf,
    }
    return dict(status='PINNED_SOURCE_CHARACTERIZED' if all(facts.values()) else 'STATIC_ASSERTION_MISMATCH', files=rows, facts=facts)


class Counterexamples(unittest.TestCase):
    def setUp(self):
        self.n = Note(4.0, 0.5, 48, 'ChordTone', 0.4, 'verse')

    def test_legacy_hearing_cannot_see_velocity_change(self):
        louder = replace(self.n, velocity=0.95)
        self.assertEqual(legacy_hearing_projection(self.n), legacy_hearing_projection(louder))
        self.assertNotEqual(effective_observation(self.n), effective_observation(louder))

    def test_existing_hearing_detects_note_insertion_but_only_if_checked(self):
        original = [self.n]
        changed = original + [replace(self.n, beat=4.25, pitch=52)]
        self.assertNotEqual([legacy_hearing_projection(n) for n in original],
                            [legacy_hearing_projection(n) for n in changed])

    def test_same_energy_and_density_do_not_determine_semantic_response(self):
        # Two different source-labeled calls with identical pitch/rhythm and
        # loudness can warrant distinct related answers; energy-only policy cannot.
        a = self.n
        b = replace(self.n, source='different_question')
        coarse = lambda n: (n.beat, n.velocity, n.dur)
        self.assertEqual(coarse(a), coarse(b))
        self.assertNotEqual(effective_observation(a), effective_observation(b))

    def test_noop_and_causal_edit_are_different_operations(self):
        self.assertEqual(effective_observation(self.n), effective_observation(replace(self.n)))
        self.assertNotEqual(effective_observation(self.n), effective_observation(replace(self.n, beat=4.125)))

    def test_empty_checkout_refuses_source_assertion(self):
        from tempfile import TemporaryDirectory
        with TemporaryDirectory() as p:
            self.assertEqual(inspect_repo(Path(p))['status'], 'SOURCE_DRIFT_REVIEW_REQUIRED')


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, help='verify exact Git blob IDs before source assertions')
    parser.add_argument('--out', type=Path)
    args = parser.parse_args()
    import io
    stream=io.StringIO()
    result=unittest.TextTestRunner(stream=stream,verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(Counterexamples))
    source = inspect_repo(args.repo) if args.repo else {'status':'NOT_RUN_NO_CHECKOUT'}
    report = {'pinned_head':BASE, 'scope':'isolated transcribed counterexamples; NOT Rust execution or listening',
              'tests':result.testsRun,'failures':len(result.failures),'errors':len(result.errors),
              'source':source,'test_log':stream.getvalue()}
    s=json.dumps(report,indent=2)+'\n'
    if args.out:
        args.out.parent.mkdir(parents=True,exist_ok=True)
        args.out.write_text(s)
    print(s,end='')
    if not result.wasSuccessful():return 1
    return 0 if source['status'] in {'PINNED_SOURCE_CHARACTERIZED','NOT_RUN_NO_CHECKOUT'} else 2

if __name__ == '__main__':
    raise SystemExit(main())
