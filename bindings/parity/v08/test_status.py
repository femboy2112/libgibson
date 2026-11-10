"""Qualification summaries must expose omitted evidence."""
from pathlib import Path
import subprocess
import unittest


HELPER = Path(__file__).resolve().parents[3] / "scripts/dev/parity_v08_status.sh"


class QualificationControls(unittest.TestCase):
    def run_summary(self, controls, rust, pty):
        return subprocess.check_output(
            ["bash", "-c", 'source "$1"; parity_qualification "$2" "$3" "$4"',
             "status-test", str(HELPER), controls, rust, pty], text=True)

    def test_skipped_controls_are_partial(self):
        self.assertIn("PARTIAL", self.run_summary("SKIPPED", "WORKTREE_BUILD", "PASS"))

    def test_prebuilt_rust_is_partial(self):
        self.assertIn("PARTIAL", self.run_summary("PASS", "PREBUILT_UNVERIFIED", "PASS"))

    def test_missing_pty_is_partial(self):
        self.assertIn("PARTIAL", self.run_summary("PASS", "WORKTREE_BUILD", "UNVERIFIED"))

    def test_full_local_relative_checks_still_disclose_provenance_gap(self):
        output = self.run_summary("PASS", "WORKTREE_BUILD", "PASS")
        self.assertIn("OK", output)
        self.assertIn("SDK provenance and release gates unverified", output)


if __name__ == "__main__":
    unittest.main()
