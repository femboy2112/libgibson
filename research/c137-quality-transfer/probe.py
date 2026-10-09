#!/usr/bin/env python3
"""Dependency-free counterexamples for the C137 source-transfer audit.

These run isolated equations/fixtures transcribed from the pinned Rust sources.
They do NOT compile Rust, render audio, prove end-to-end behavior, or judge taste.
Optional --repo verifies Git blob IDs and the narrowly scoped profile-wiring facts.
"""
from __future__ import annotations
import argparse
import hashlib
import io
import json
import re
import unittest
from dataclasses import asdict, dataclass, replace
from pathlib import Path

BASE = "229b397b46a800342a58ac5c38c8bbad3d2cfb87"
BLOBS = {
    "examples/rick_story.rs": "74a874e3233f80ac156970f46e531f3e4d9d04a3",
    "examples/rick_gen3.rs": "7b33c919729aa8c897ebc65e4b6396612cfe36ba",
    "examples/band_story_probe.rs": "8a43ab393e185279b4c8e2a36d24650458c97858",
    "src/audio/human_music/policy.rs": "033f9e5dfc9d99160aeae0fdc60de8b883cc6836",
    "src/audio/human_music/composer.rs": "b0bcd2e48de0adaf993df6bc7429ebb7f008ad36",
    "src/audio/human_music/motif.rs": "f83c5562e1abd1eb7517bcf413c3e8f60b56a77b",
    "src/audio/human_music/material.rs": "7566480645b973bb3da7a07ba6e25343f833bfca",
    "src/audio/human_music/bass.rs": "6a0f83706d6c9b64365833b54d902658bdc61601",
}

@dataclass(frozen=True)
class Event:
    onset: float
    pitch: int
    duration: float
    velocity: float

# VERSE, examples/band_story_probe.rs at BASE; BEFORE the separate 0.022-beat lean.
VERSE = tuple(Event(*row) for row in [
    (0,64,.5,.60),(.5,67,.5,.62),(1,69,.5,.68),(1.5,71,1,.72),
    (3,69,.5,.62),(3.5,67,.5,.58),(4,65,.5,.60),(4.5,69,.5,.66),
    (5,72,.5,.70),(5.5,71,.25,.64),(5.75,69,.25,.62),(6,65,1,.58),
    (8,67,.5,.60),(8.5,72,.5,.66),(9,76,.25,.72),(9.25,74,.25,.66),
    (9.5,72,.25,.62),(9.75,71,.25,.60),(10,72,1,.58),
    (12,71,.5,.60),(12.5,69,.5,.58),(13,68,.5,.56),(13.5,64,.5,.54),
    (14,62,1.5,.52),
])
DIAGNOSTIC_DEGREES = (0,4,7,6,4,4)


def prefix_onsets(spacing: list[float]) -> list[float]:
    at = 0.0
    result = []
    for gap in spacing:
        result.append(at)
        at += gap
    return result


def modulo_pitches(degrees: tuple[int, ...], tones: tuple[int, ...]) -> tuple[int, ...]:
    """Isolated germ_tone_index + chord-tone selection, not the whole bass realizer."""
    if not tones:
        raise ValueError("empty tone domain")
    return tuple(tones[d % len(tones)] for d in degrees)


def quote_tail(events: tuple[Event, ...]) -> tuple[Event, ...]:
    """Isolated Transform::Quote's final-four crop, before room clipping."""
    if not events:
        return ()
    tail = events[-4:]
    t0, p0 = tail[0].onset, tail[0].pitch
    return tuple(replace(e, onset=e.onset-t0, pitch=e.pitch-p0) for e in tail)


class TransferCounterexamples(unittest.TestCase):
    def test_teacher_has_explicit_rests_and_accents(self):
        gaps = [b.onset-a.onset-a.duration for a,b in zip(VERSE, VERSE[1:])]
        self.assertEqual(sum(g for g in gaps if g > 0), 2.5)
        self.assertEqual(sum(g > 0 for g in gaps), 3)
        self.assertEqual(len(set(e.velocity for e in VERSE)), 11)

    def test_duration_chain_loses_teacher_onsets(self):
        encoded = prefix_onsets([e.duration for e in VERSE])
        residual = max(abs(e.onset-t) for e,t in zip(VERSE, encoded))
        self.assertEqual(residual, 2.5)
        self.assertNotEqual(encoded, [e.onset for e in VERSE])

    def test_using_onset_gaps_instead_changes_gates(self):
        spacing = [b.onset-a.onset for a,b in zip(VERSE, VERSE[1:])]
        spacing.append(VERSE[-1].duration)
        self.assertEqual(prefix_onsets(spacing), [e.onset for e in VERSE])
        self.assertEqual(sum(s-e.duration for s,e in zip(spacing,VERSE)), 2.5)
        # Positive control: contiguous equal-duration events need no richer encoding.
        self.assertEqual(prefix_onsets([.5,.5,.5]), [0,.5,1])

    def test_explicit_event_payload_roundtrips_teacher(self):
        # Existence proof for a timed/voiced event representation, not a Rust API test.
        recovered = tuple(Event(**d) for d in json.loads(json.dumps([asdict(e) for e in VERSE])))
        self.assertEqual(recovered, VERSE)

    def test_legacy_quote_is_not_full_carriage(self):
        result = quote_tail(VERSE)
        self.assertEqual(len(result), 4)
        self.assertEqual(len(VERSE), 24)
        self.assertEqual(result[0].onset, 0)
        self.assertEqual(result[1].pitch-result[0].pitch, VERSE[-3].pitch-VERSE[-4].pitch)

    def test_ordered_modulo_is_still_noninjective(self):
        a, b = (0,1,2,3), (0,5,2,7)
        self.assertNotEqual(tuple(y-x for x,y in zip(a,a[1:])), tuple(y-x for x,y in zip(b,b[1:])))
        self.assertEqual(modulo_pitches(a,(0,4,7,11)), modulo_pitches(b,(0,4,7,11)))
        self.assertNotEqual(modulo_pitches((0,1),(0,4,7,11)), modulo_pitches((0,2),(0,4,7,11)))

    def test_diagnostic_leap_collapses_on_tetrad(self):
        mapped = modulo_pitches(DIAGNOSTIC_DEGREES,(0,4,7,11))
        self.assertEqual(mapped, (0,0,11,7,0,0))
        self.assertEqual(mapped[0], mapped[1])
        self.assertNotEqual(DIAGNOSTIC_DEGREES[0], DIAGNOSTIC_DEGREES[1])

    def test_pitch_only_pass_cannot_read_germ_rhythm(self):
        # Hold upstream notes/chords fixed: the inspected bass post-pass never reads rhythm.
        rhythm_a, rhythm_b = (.5,.5,1,.5,.5,3), (1,.5,.5,1,1,2)
        self.assertNotEqual(rhythm_a, rhythm_b)
        existing = [Event(float(i),48,0.4,.7) for i in range(6)]
        mapped = modulo_pitches(DIAGNOSTIC_DEGREES,(0,4,7,11))
        def pass_on_fixed_backing(_rhythm):
            return [replace(e,pitch=48+p) for e,p in zip(existing,mapped)]
        self.assertEqual(pass_on_fixed_backing(rhythm_a), pass_on_fixed_backing(rhythm_b))

    def test_count_delta_does_not_detect_valid_replacement(self):
        backing = (48,48,48,48)
        source_phrase = (48,52,55,52)
        realized = source_phrase
        self.assertEqual(len(realized)-len(backing), 0)
        self.assertEqual(realized, source_phrase)
        self.assertNotEqual(realized, backing)


def inspect_checkout(repo: Path) -> dict:
    """Pin-check the source before making source-specific static assertions."""
    files = {}
    for name, expected in BLOBS.items():
        path = repo / name
        if not path.is_file():
            files[name] = {"status":"missing", "expected":expected}
            continue
        raw = path.read_bytes()
        actual = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
        files[name] = {"status":"matches" if actual == expected else "drift", "expected":expected,"actual":actual}
    if not all(row["status"] == "matches" for row in files.values()):
        return {"status":"SOURCE_DRIFT_REVIEW_REQUIRED", "files":files}
    story = (repo/"examples/rick_story.rs").read_text()
    policy = (repo/"src/audio/human_music/policy.rs").read_text()
    gen3 = (repo/"examples/rick_gen3.rs").read_text()
    checks = {
        "story_has_no_lead_life_builder": ".with_lead_life(" not in story,
        "story_enables_narrative_on_BAND": "PerformanceProfile::BAND.with_narrative(NarrativePolicy::Ensemble)" in story,
        "gen3_explicitly_builds_lead_life": ".with_lead_life(" in gen3,
        "written_lead_life_flags_off": bool(re.search(r"lead_life: LeadLifePolicy\s*\{\s*development: false,\s*spacing: false,\s*dynamics: false",policy)),
    }
    return {"status":"PINNED_STATIC_CHECKS_PASS" if all(checks.values()) else "STATIC_CHECK_FAILED", "files":files,"checks":checks}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo",type=Path,help="optional checkout for exact-blob and static wiring checks")
    parser.add_argument("--out",type=Path,help="write complete JSON report")
    args = parser.parse_args()
    stream = io.StringIO()
    result = unittest.TextTestRunner(stream=stream,verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(TransferCounterexamples))
    report = {
        "audited_commit":BASE,
        "scope":"isolated transcribed equations and teacher fixture; NOT Rust execution or listening",
        "tests_run":result.testsRun,"failures":len(result.failures),"errors":len(result.errors),
        "teacher_verse":{"notes":24,"internal_rest_beats":2.5,"distinct_velocities":11,"duration_chain_max_onset_error_beats":2.5,"legacy_quote_max_retained_events":4},
        "diagnostic_germ_on_c_major_tetrad":list(modulo_pitches(DIAGNOSTIC_DEGREES,(0,4,7,11))),
        "checkout":inspect_checkout(args.repo) if args.repo else {"status":"NOT_RUN_NO_CHECKOUT"},
        "unittest_output":stream.getvalue(),
    }
    payload = json.dumps(report,indent=2)+"\n"
    if args.out:
        args.out.parent.mkdir(parents=True,exist_ok=True)
        args.out.write_text(payload)
    print(payload,end="")
    if not result.wasSuccessful():
        return 1
    return 0 if report["checkout"]["status"] in {"PINNED_STATIC_CHECKS_PASS","NOT_RUN_NO_CHECKOUT"} else 2

if __name__ == "__main__":
    raise SystemExit(main())
