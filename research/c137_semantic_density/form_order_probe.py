#!/usr/bin/env python3
"""Isolated reconstruction of the pinned C137-fusion form→step scheduling.

This is NOT Rust execution, audio rendering or listening. Before applying a
conclusion to changed source, rerun with --repo to enforce pinned Git blob IDs.

Transcribed from pinned 2465ad06:
  semantic::deflected_lift_trace, timeline::quantize_event_beat,
  plan::FormGraph::build_for_beats, argument::MusicalArgument::fusion.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import math
import unittest
from pathlib import Path

SOURCE_HEAD = '2465ad06a5424d77ca92f14fffbdab568000c540'
BLOBS = {
    'src/audio/human_music/argument.rs': 'bedfdb6e4c0753d233f700b59218ab96ba82130c',
    'src/audio/human_music/semantic.rs': '7a55f5a9484b564a7c840eea7e748b16306b6ca1',
    'src/audio/human_music/plan.rs': '385753d08cc8a08ee8c66bbd43bcdc088eb2cf8a',
    'src/audio/human_music/timeline.rs': '99b413f5c5fd5142a46bec7690226bf5e0e32845',
}
# Every EventKind::ModalEntered/Confirmation/SectionResolved in
# deflected_lift_trace. FocusAcquired is nonsalient; t=0 ActChanged is a bound.
SALIENT = (0.267, 0.40, 0.533, 0.733, 0.833, 0.933)


def rust_round_nonnegative(x: float) -> int:
    """Rust f64::round, for the nonnegative time domain used in this fixture."""
    return math.floor(x + 0.5)


def form_phrases(beats: float) -> list[tuple[int, int]]:
    """(bar index, bar length) under the actual DeflectedLift phrase_bars=4."""
    if not math.isfinite(beats) or beats <= 0:
        raise ValueError('positive finite song duration required')
    total_bars = max(1, math.ceil(beats / 4.0 - 1e-9))
    bounds = set(range(0, total_bars, 4)) | {total_bars}
    for fraction in SALIENT:
        raw = fraction * beats
        barline = rust_round_nonnegative(raw / 4.0) * 4.0
        # timeline.rs: event quantization prefers a bar line within 1 beat,
        # otherwise the nearest eighth note.
        musical = barline if abs(raw - barline) <= 1.0 + 1e-9 else rust_round_nonnegative(raw * 2.0) / 2.0
        bar = rust_round_nonnegative(musical / 4.0)
        snapped = (bar // 2) * 2
        if 0 < snapped < total_bars:
            bounds.add(snapped)
    sorted_bounds = sorted(bounds)
    return [(a, b-a) for a, b in zip(sorted_bounds, sorted_bounds[1:])]


def fusion_steps(phrases: list[tuple[int, int]]) -> list[dict]:
    """Literal control-flow reconstruction of MusicalArgument::fusion step policy."""
    spans = [length for _, length in phrases]
    if not spans:
        raise ValueError('no phrases')
    full = max(spans)
    n_full = sum(length >= full for length in spans)
    full_seen = 0
    last_verse = None
    last_hook = None
    result = []
    for ix, (start_bar, length) in enumerate(phrases):
        if length < full:
            role = 'Depart'
            source = last_hook if last_hook is not None else (last_verse or 0)
            referent = 'bridge'
        else:
            if full_seen + 1 == n_full and last_hook is not None:
                role, source, referent = 'Return', last_hook, 'hook'
            elif full_seen % 2 == 0:
                role, source, referent = 'Establish', None, 'verse'
                last_verse = ix
            else:
                role, source, referent = 'Consequent', last_verse, 'hook'
                last_hook = ix
            full_seen += 1
        result.append({'id': ix, 'beat': start_bar * 4, 'length_beats': length * 4,
                       'relation': role, 'source_step': source, 'referent': referent})
    return result


def verdict(steps: list[dict]) -> dict:
    departures = [s for s in steps if s['relation'] == 'Depart']
    returns = [s for s in steps if s['relation'] == 'Return']
    questions = [s for s in steps if s['relation'] == 'Question']
    answers = [s for s in steps if s['relation'] == 'Answer']
    return {
        'has_departure': bool(departures), 'has_return': bool(returns),
        'last_relation': steps[-1]['relation'],
        'return_after_departure': bool(departures and returns and returns[-1]['beat'] > departures[-1]['beat']),
        'source_question_count': len(questions), 'source_answer_count': len(answers),
        # Actual validate()/verify() only accrue debt on Question, then remove on Answer.
        # This emulates ONLY the open-debt set, not the entire validation or Score witness.
        'resolved_debt_gate_vacuous': not questions and not answers,
    }


class PinnedCharacterization(unittest.TestCase):
    def test_128_form_is_ten_phrases_with_four_short_terminal_phrases(self):
        self.assertEqual(form_phrases(128), [(0,4),(4,4),(8,4),(12,4),(16,4),(20,4),
                                            (24,2),(26,2),(28,2),(30,2)])

    def test_128_return_precedes_bridge_and_final_phrase_is_departure(self):
        steps = fusion_steps(form_phrases(128))
        self.assertEqual([(x['beat'], x['relation']) for x in steps],
                         [(0,'Establish'),(16,'Consequent'),(32,'Establish'),
                          (48,'Consequent'),(64,'Establish'),(80,'Return'),
                          (96,'Depart'),(104,'Depart'),(112,'Depart'),(120,'Depart')])
        v = verdict(steps)
        self.assertEqual(v['last_relation'], 'Depart')
        self.assertFalse(v['return_after_departure'])
        self.assertTrue(v['resolved_debt_gate_vacuous'])

    def test_160_fixture_differs_and_can_mask_default_failure(self):
        steps = fusion_steps(form_phrases(160))
        self.assertTrue(verdict(steps)['return_after_departure'])
        self.assertEqual(verdict(steps)['last_relation'], 'Return')

    def test_failure_extends_beyond_one_seed_or_one_length(self):
        for beats in (96,112,120,128,144):
            with self.subTest(beats=beats):
                v = verdict(fusion_steps(form_phrases(beats)))
                self.assertEqual(v['last_relation'], 'Depart')
                self.assertFalse(v['return_after_departure'])

    def test_counterfactual_ordering_is_observably_distinct(self):
        bad = fusion_steps(form_phrases(128))
        good = [dict(s) for s in bad]
        # Conceptual fix only: a final actual Return after the bridge; no claim that
        # an 8-beat return can currently transport the full 16-beat hook.
        good[-1]['relation'] = 'Return'
        good[-1]['referent'] = 'hook'
        good[-1]['source_step'] = 3
        self.assertFalse(verdict(bad)['return_after_departure'])
        self.assertTrue(verdict(good)['return_after_departure'])
        # The musical length-compatibility problem is left for the Rust compiler.


def source_guard(repo: Path) -> dict:
    checks = {}
    for path, expected in BLOBS.items():
        file = repo/path
        if not file.is_file():
            checks[path] = 'MISSING'
            continue
        b = file.read_bytes()
        actual = hashlib.sha1(b'blob ' + str(len(b)).encode() + b'\0' + b).hexdigest()
        checks[path] = 'MATCH' if actual == expected else 'DRIFT:' + actual
    return {'status': 'PINNED_MATCH' if all(s == 'MATCH' for s in checks.values()) else 'SOURCE_DRIFT_REVIEW_REQUIRED',
            'checks': checks}


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--beats',type=float,default=128.0)
    p.add_argument('--repo',type=Path,help='optional real checkout; verify pinned source Git blob IDs')
    p.add_argument('--out',type=Path)
    args = p.parse_args()
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(PinnedCharacterization)
    import io
    log = io.StringIO()
    res = unittest.TextTestRunner(stream=log,verbosity=2).run(suite)
    phrases = form_phrases(args.beats)
    steps = fusion_steps(phrases)
    guard = source_guard(args.repo) if args.repo else {'status':'NOT_RUN_NO_CHECKOUT'}
    report = {'source_head':SOURCE_HEAD, 'scope':'isolated transcribed source control flow, not Rust runtime or audio',
              'beats':args.beats,'phrases':phrases,'steps':steps,'verdict':verdict(steps),
              'tests':res.testsRun, 'failures':len(res.failures), 'errors':len(res.errors),
              'checkout':guard, 'test_log':log.getvalue()}
    report_json = json.dumps(report,indent=2)+'\n'
    if args.out:
        args.out.write_text(report_json)
    print(report_json,end='')
    if not res.wasSuccessful():return 1
    return 0 if guard['status'] in ('PINNED_MATCH','NOT_RUN_NO_CHECKOUT') else 2

if __name__=='__main__':
    raise SystemExit(main())
