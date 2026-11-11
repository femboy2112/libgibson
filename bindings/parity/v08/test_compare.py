"""Focused controls for the committed parity oracle (no native SDK required)."""
from contextlib import redirect_stdout
import copy
import io
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
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

    def test_committed_geometry_oracle_rejects_common_mode_wrong_sizes(self):
        expected = json.loads((Path(__file__).parent / "expected.json").read_text(encoding="utf-8"))
        for rid, cols, rows in (("t01", 40, 12), ("t02", 60, 16)):
            with self.subTest(record=rid):
                self.assertEqual(
                    json.loads(expected["absolute"][rid]),
                    {"cols": cols, "id": rid, "k": "terminal_size", "rows": rows, "st": "OK"},
                )
                good = expected["absolute"][rid]
                wrong = json.loads(good)
                wrong.update(cols=1, rows=1)
                wrong = json.dumps(wrong, sort_keys=True, separators=(",", ":"))
                fixture = {
                    "record_ids": [rid],
                    "differs_from_c": {"c": []},
                    "absolute": {rid: good},
                }
                self.assertFalse(compare.check_against("c", good + "\n", good + "\n", fixture, {}, [])[1])
                self.assertTrue(compare.check_against("c", wrong + "\n", wrong + "\n", fixture, {}, [])[1])

    def test_common_mode_wrong_focus_is_rejected(self):
        wrong = self.text(self.gained, line("f05.2", "FOCUS_GAINED"), self.c_gap)
        self.assertIn("f05.2", self.check("c", wrong, wrong)[0])

    def test_common_mode_reordered_records_are_rejected(self):
        swapped = self.text(self.lost, self.gained, self.c_gap)
        self.assertFalse(self.check("c", swapped, swapped)[2])
        self.assertTrue(self.check("c", swapped, swapped)[1])

    def test_healed_gap_requires_expectation_update(self):
        self.assertIn("p02", self.check("rust", self.c, self.c)[0])

    def test_reference_changed_to_the_pinned_gap_requires_expectation_update(self):
        # Rust is unchanged and still exactly pinned. A changed C reference can
        # erase the difference just as easily as a change to the Rust client.
        bad, problems, _ = self.check("rust", self.rust, self.rust)
        self.assertIn("p02", bad)
        self.assertTrue(problems)

    def matrix(self, transcripts, c_gaps=None, rust_gaps=None):
        expected = copy.deepcopy(self.expect)
        if c_gaps is not None:
            expected["differs_from_c"] = c_gaps
        expected["differs_from_rust"] = (
            {lang: ([] if lang == "rust" else ["p02"]) for lang in compare.LANGS}
            if rust_gaps is None else rust_gaps
        )
        expected["labels"] = {
            dim: {lang: ["SUPPORTED", "fixture"] for lang in compare.LANGS}
            for dim in compare.DIMS
        }
        expected["resource"] = {"max_rss_second_half_growth_kb": 4096}
        with tempfile.TemporaryDirectory(prefix="parity-matrix-control-") as tmp:
            root = Path(tmp)
            (root / "expected.json").write_text(json.dumps(expected), encoding="utf-8")
            (root / "script.txt").write_text("f05 EVT poll 2\np02 ERR fixture\n", encoding="utf-8")
            for lang, text in transcripts.items():
                for suffix in ("a", "b", "rep"):
                    (root / f"{lang}.{suffix}.jsonl").write_text(text, encoding="utf-8")
                resource = {
                    "k": "resource", "lang": lang, "reps": 500,
                    "wall_ms": 1, "per_rep_us": 1, "maxrss_kb_1": 1,
                    "maxrss_kb_mid": 1, "maxrss_kb_n": 1, "iter_identical": 1,
                }
                (root / f"{lang}.rep.err").write_text(json.dumps(resource) + "\n", encoding="utf-8")
            output = io.StringIO()
            with redirect_stdout(output):
                status = compare.cmd_matrix(SimpleNamespace(
                    expected=str(root / "expected.json"), script=str(root / "script.txt"), dir=str(root)
                ))
        return status, output.getvalue()

    def test_matrix_accepts_the_declared_gap(self):
        transcripts = {lang: self.rust if lang == "rust" else self.c for lang in compare.LANGS}
        self.assertEqual(self.matrix(transcripts)[0], 0)

    def test_matrix_rejects_reference_change_to_the_pinned_gap(self):
        # Every C-style client changes to Rust's pinned result. Agreement among
        # all five must not silently heal the committed difference expectation.
        transcripts = {lang: self.rust for lang in compare.LANGS}
        self.assertEqual(self.matrix(transcripts)[0], 1)

    def test_matrix_enforces_the_rust_comparison_leg(self):
        # The C leg agrees with its expectation; only the Rust-leg declaration
        # is stale. This prevents the other leg from masking a missing check.
        transcripts = {lang: self.c for lang in compare.LANGS}
        status, output = self.matrix(transcripts, c_gaps={}, rust_gaps={"c": ["p02"]})
        self.assertEqual(status, 1)
        self.assertIn("vs rust", output)

    def test_changed_gap_requires_expectation_update(self):
        changed = self.text(self.gained, self.lost, line("p02", "RUST_OTHER"))
        self.assertIn("p02", self.check("rust", changed, self.c)[0])

    def test_missing_and_duplicate_records_are_rejected(self):
        self.assertTrue(self.check("c", self.text(self.gained, self.c_gap), self.c)[1])
        self.assertTrue(self.check("c", self.text(self.gained, self.lost, self.lost, self.c_gap), self.c)[1])


if __name__ == "__main__":
    unittest.main()
