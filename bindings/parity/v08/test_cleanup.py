"""Cleanup controls; run without a compiler or an installed SDK."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[3]
HELPER = ROOT / "scripts/dev/parity_v08_cleanup.sh"
HARNESS = ROOT / "scripts/dev/parity_v08.sh"

SETUP = r'''
set -euo pipefail
source "$HELPER"
GO_CACHE=$(mktemp -d "$WORK/go-cache.XXXXXX")
GO_CACHE_TOKEN=$(parity_directory_token "$GO_CACHE")
SDK_ALIAS_DIR=$(mktemp -d "$WORK/alias.XXXXXX")
SDK_ALIAS_TOKEN=$(parity_directory_token "$SDK_ALIAS_DIR")
trap parity_cleanup EXIT
printf '%s' "$GO_CACHE" > "$WORK/cache-path"
printf '%s' "$SDK_ALIAS_DIR" > "$WORK/alias-path"
printf 'cache' > "$GO_CACHE/cache-file"
ln -s "$WORK/keep" "$GO_CACHE/do-not-follow"
ln -s "$WORK/keep" "$SDK_ALIAS_DIR/sdk"
if [ "${REPLACE:-0}" = 1 ]; then
    mv "$GO_CACHE" "$WORK/moved-original"
    mkdir "$GO_CACHE"
    printf 'replacement' > "$GO_CACHE/user-data"
fi
if [ "${REPLACE_ALIAS:-0}" = 1 ]; then
    mv "$SDK_ALIAS_DIR" "$WORK/moved-alias"
    mkdir "$SDK_ALIAS_DIR"
    printf 'replacement' > "$SDK_ALIAS_DIR/user-data"
fi
if [ "${TERMINATE:-0}" = 1 ]; then
    kill -TERM "$$"
fi
exit "$RESULT"
'''


class CleanupControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="parity cleanup test ")
        self.addCleanup(self.temp.cleanup)
        self.work = Path(self.temp.name) / "work space"
        self.work.mkdir()
        (self.work / "keep").write_text("keep", encoding="utf-8")

    def run_cleanup(self, result, replace=False):
        env = dict(os.environ, HELPER=str(HELPER), WORK=str(self.work),
                   RESULT=str(result), REPLACE=str(int(replace)), REPLACE_ALIAS="0", TERMINATE="0")
        child = subprocess.run(["bash", "-c", SETUP], env=env, capture_output=True, text=True)
        cache = Path((self.work / "cache-path").read_text(encoding="utf-8"))
        alias = Path((self.work / "alias-path").read_text(encoding="utf-8"))
        self.assertEqual((self.work / "keep").read_text(encoding="utf-8"), "keep")
        self.assertFalse(alias.exists())
        return child, cache

    def test_success_and_failure_both_clean_owned_paths(self):
        for result in (0, 42):
            with self.subTest(result=result):
                child, cache = self.run_cleanup(result)
                self.assertEqual(child.returncode, result, child.stderr)
                self.assertFalse(cache.exists())
                self.assertTrue(self.work.exists())

    def test_replaced_directory_is_preserved_and_reported(self):
        child, cache = self.run_cleanup(0, replace=True)
        self.assertNotEqual(child.returncode, 0)
        self.assertIn("identity changed", child.stderr)
        self.assertEqual((cache / "user-data").read_text(encoding="utf-8"), "replacement")
        self.assertEqual((self.work / "moved-original/cache-file").read_text(encoding="utf-8"), "cache")

    def test_replaced_alias_is_preserved_and_reported(self):
        env = dict(os.environ, HELPER=str(HELPER), WORK=str(self.work), RESULT="0", REPLACE="0",
                   REPLACE_ALIAS="1", TERMINATE="0")
        child = subprocess.run(["bash", "-c", SETUP], env=env, capture_output=True, text=True)
        cache = Path((self.work / "cache-path").read_text(encoding="utf-8"))
        alias = Path((self.work / "alias-path").read_text(encoding="utf-8"))
        self.assertNotEqual(child.returncode, 0)
        self.assertIn("alias identity changed", child.stderr)
        self.assertFalse(cache.exists())
        self.assertEqual((alias / "user-data").read_text(encoding="utf-8"), "replacement")
        self.assertTrue((self.work / "moved-alias/sdk").is_symlink())

    def test_sigterm_removes_owned_paths_and_stays_nonzero(self):
        env = dict(os.environ, HELPER=str(HELPER), WORK=str(self.work), RESULT="0", REPLACE="0",
                   REPLACE_ALIAS="0", TERMINATE="1")
        child = subprocess.run(["bash", "-c", SETUP], env=env, capture_output=True, text=True)
        cache = Path((self.work / "cache-path").read_text(encoding="utf-8"))
        alias = Path((self.work / "alias-path").read_text(encoding="utf-8"))
        self.assertNotEqual(child.returncode, 0)
        self.assertFalse(cache.exists())
        self.assertFalse(alias.exists())
        self.assertEqual((self.work / "keep").read_text(encoding="utf-8"), "keep")

    def test_harness_cleans_after_alias_creation_failure(self):
        sdk = Path(self.temp.name) / "sdk path"
        (sdk / "lib/pkgconfig").mkdir(parents=True)
        (sdk / "lib/pkgconfig/libgibson.pc").touch()
        fake = Path(self.temp.name) / "fake-bin"
        fake.mkdir()
        fake_ln = fake / "ln"
        fake_ln.write_text('#!/bin/sh\nprintf "%s" "$4" > "$ALIAS_RECORD"\nexit 42\n', encoding="utf-8")
        fake_ln.chmod(0o755)
        record = self.work / "failed-alias"
        env = dict(os.environ, PARITY_V08_WORK=str(self.work), ALIAS_RECORD=str(record),
                   PATH=str(fake) + os.pathsep + os.environ["PATH"])
        child = subprocess.run([str(HARNESS), str(sdk)], env=env, capture_output=True, text=True)
        self.assertEqual(child.returncode, 42, child.stderr)
        self.assertFalse(Path(record.read_text(encoding="utf-8")).parent.exists())
        self.assertEqual(list(self.work.glob("go-cache.*")), [])
        self.assertEqual((self.work / "keep").read_text(encoding="utf-8"), "keep")


if __name__ == "__main__":
    unittest.main()
