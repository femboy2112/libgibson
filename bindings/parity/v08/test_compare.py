"""Focused controls for the committed parity oracle (no native SDK required)."""
import json
from pathlib import Path
import unittest

import compare


def line(rid, value):
    return json.dumps({"id": rid, "k": "event", "type": value}, sort_keys=True, separators=(",", ":"))


class ComparatorControls(unittest.TestCase):
    def setUp(self):
        self.gained = line("f05.1", "FOCUS_GAINED")
        self.lost = line("f05.2", "FOCUS_LOST")
        self.c_gap = line("p02", "INVALID_PARAM")
        self.rust_gap = line("p02", "RUST_ACCEPTED")
        self.expect = {
            "record_ids": ["f05.1", "f05.2", "p02"],
            "differs_from_c": {"c": [], "rust": ["p02"]},
            "pinned": {"rust": {"p02": self.rust_gap}},
            "absolute": {"f05.1": self.gained, "f05.2": self.lost},
        }
        self.dims = {"f05": "EVT", "p02": "ERR"}
        self.c = self.text(self.gained, self.lost, self.c_gap)
        self.rust = self.text(self.gained, self.lost, self.rust_gap)

    @staticmethod
    def text(*records):
        return "\n".join(records) + "\n"

    def check(self, lang, transcript, reference):
        return compare.check_against(lang, transcript, reference, self.expect, self.dims, [])

    def test_correct_transcripts_pass(self):
        self.assertFalse(self.check("c", self.c, self.c)[1])
        self.assertFalse(self.check("rust", self.rust, self.c)[1])

    def test_committed_focus_oracle_is_not_just_relative(self):
        expected = json.loads((Path(__file__).parent / "expected.json").read_text(encoding="utf-8"))
        self.assertEqual(json.loads(expected["absolute"]["f05.1"])["type"], "FOCUS_GAINED")
        self.assertEqual(json.loads(expected["absolute"]["f05.2"])["type"], "FOCUS_LOST")
        self.assertLess(expected["record_ids"].index("f05.1"), expected["record_ids"].index("f05.2"))

    def test_common_mode_wrong_focus_is_rejected(self):
        wrong = self.text(self.gained, line("f05.2", "FOCUS_GAINED"), self.c_gap)
        self.assertIn("f05.2", self.check("c", wrong, wrong)[0])

    def test_common_mode_reordered_records_are_rejected(self):
        swapped = self.text(self.lost, self.gained, self.c_gap)
        self.assertFalse(self.check("c", swapped, swapped)[2])
        self.assertTrue(self.check("c", swapped, swapped)[1])

    def test_healed_gap_requires_expectation_update(self):
        self.assertIn("p02", self.check("rust", self.c, self.c)[0])

    def test_changed_gap_requires_expectation_update(self):
        changed = self.text(self.gained, self.lost, line("p02", "RUST_OTHER"))
        self.assertIn("p02", self.check("rust", changed, self.c)[0])

    def test_missing_and_duplicate_records_are_rejected(self):
        self.assertTrue(self.check("c", self.text(self.gained, self.c_gap), self.c)[1])
        self.assertTrue(self.check("c", self.text(self.gained, self.lost, self.lost, self.c_gap), self.c)[1])


if __name__ == "__main__":
    unittest.main()
