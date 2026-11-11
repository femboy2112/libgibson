"""Preservation controls; run without a compiler or an installed SDK."""
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
SDK_ALIAS_DIR=$(mktemp -d "$WORK/alias.XXXXXX")
trap parity_cleanup EXIT
printf '%s' "$GO_CACHE" > "$WORK/cache-path"
printf '%s' "$SDK_ALIAS_DIR" > "$WORK/alias-path"
printf 'cache' > "$GO_CACHE/cache-file"
ln -s "$WORK/keep" "$SDK_ALIAS_DIR/sdk"
if [ "${REPLACE:-0}" = 1 ]; then
    mv "$GO_CACHE" "$WORK/moved-original"
    mkdir "$GO_CACHE"
    printf 'replacement' > "$GO_CACHE/user-data"
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

    def run_exit(self, result=0, replace=False, terminate=False, path=None):
        env = dict(os.environ, HELPER=str(HELPER), WORK=str(self.work), RESULT=str(result),
                   REPLACE=str(int(replace)), TERMINATE=str(int(terminate)))
        if path:
            env["PATH"] = str(path) + os.pathsep + env["PATH"]
        child = subprocess.run(["bash", "-c", SETUP], env=env, capture_output=True, text=True)
        cache = Path((self.work / "cache-path").read_text(encoding="utf-8"))
        alias = Path((self.work / "alias-path").read_text(encoding="utf-8"))
        self.assertEqual((self.work / "keep").read_text(encoding="utf-8"), "keep")
        self.assertIn(str(cache), child.stderr)
        self.assertIn(str(alias), child.stderr)
        return child, cache, alias

    def test_success_and_failure_preserve_and_report_temporary_paths(self):
        for result in (0, 42):
            with self.subTest(result=result):
                child, cache, alias = self.run_exit(result)
                self.assertEqual(child.returncode, result, child.stderr)
                self.assertEqual((cache / "cache-file").read_text(encoding="utf-8"), "cache")
                self.assertTrue((alias / "sdk").is_symlink())

    def test_replaced_roots_are_preserved(self):
        child, cache, alias = self.run_exit(replace=True)
        self.assertEqual(child.returncode, 0, child.stderr)
        self.assertEqual((cache / "user-data").read_text(encoding="utf-8"), "replacement")
        self.assertEqual((alias / "user-data").read_text(encoding="utf-8"), "replacement")
        self.assertEqual((self.work / "moved-original/cache-file").read_text(encoding="utf-8"), "cache")
        self.assertTrue((self.work / "moved-alias/sdk").is_symlink())

    def test_sigterm_preserves_paths_and_stays_nonzero(self):
        child, cache, alias = self.run_exit(terminate=True)
        self.assertNotEqual(child.returncode, 0)
        self.assertTrue((cache / "cache-file").is_file())
        self.assertTrue((alias / "sdk").is_symlink())

    def test_harness_preserves_after_alias_creation_failure(self):
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
        alias = Path(record.read_text(encoding="utf-8")).parent
        self.assertTrue(alias.is_dir())
        self.assertIn(str(alias), child.stderr)
        self.assertEqual(len(list(self.work.glob("go-cache.*"))), 1)
        # The alias is outside TemporaryDirectory; only remove the empty directory this test made.
        alias.rmdir()

    def test_directory_swap_between_identity_check_and_delete(self):
        fake = Path(self.temp.name) / "find-hook"
        fake.mkdir()
        hook = fake / "find"
        hook.write_text('''#!/bin/sh
printf triggered > "$WORK/interleaved"
mv "$2" "$WORK/moved-during-find"
mkdir "$2"
printf user-data > "$2/user-data"
exec /usr/bin/find "$@"
''', encoding="utf-8")
        hook.chmod(0o755)
        _, cache, _ = self.run_exit(path=fake)
        if (self.work / "interleaved").exists():
            self.assertEqual((cache / "user-data").read_text(encoding="utf-8"), "user-data")
        else:
            self.assertEqual((cache / "cache-file").read_text(encoding="utf-8"), "cache")

    def test_alias_swap_between_identity_check_and_unlink(self):
        fake = Path(self.temp.name) / "rm-hook"
        fake.mkdir()
        hook = fake / "rm"
        hook.write_text('''#!/bin/sh
for arg do last=$arg; done
root=${last%/sdk}
printf triggered > "$WORK/alias-interleaved"
mv "$root" "$WORK/moved-during-unlink"
mkdir "$root"
printf user-data > "$root/sdk"
exec /usr/bin/rm "$@"
''', encoding="utf-8")
        hook.chmod(0o755)
        _, _, alias = self.run_exit(path=fake)
        if (self.work / "alias-interleaved").exists():
            self.assertEqual((alias / "sdk").read_text(encoding="utf-8"), "user-data")
        else:
            self.assertTrue((alias / "sdk").is_symlink())


if __name__ == "__main__":
    unittest.main()
