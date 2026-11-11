#!/usr/bin/env python3
"""No-compiler test: release preflight must read an isolated captured commit."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

root = Path(__file__).resolve().parents[3]
work = Path(tempfile.mkdtemp(prefix="libgibson-isolation-test."))
print(f"isolated preflight fixture retained: {work}", flush=True)
repo = work / "repo"
repo.mkdir()


def run(*args, **kwargs):
    return subprocess.run(args, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, **kwargs)


def executable(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    path.chmod(0o755)


def git(*args):
    return run("git", "-C", str(repo), *args)


run("git", "init", "-q", str(repo))
preflight = root / "scripts/release/preflight.sh"
executable(repo / "scripts/release/preflight.sh", preflight.read_text())
inner = root / "scripts/release/preflight-inner.sh"
if inner.exists():
    executable(repo / "scripts/release/preflight-inner.sh", inner.read_text())
executable(repo / "scripts/release/check-clean-index.py", (root / "scripts/release/check-clean-index.py").read_text())
for script in ("check-versions.sh", "msrv-consumer.sh", "check-candidate-abi.sh", "cleanroom.sh",
               "tests/test-check-abi.sh", "tests/test-check-clean-index.sh", "tests/test-stage-sdk-target.sh",
               "tests/test-cleanroom-package.sh", "tests/test-cleanroom-go.sh",
               "tests/test-cleanroom-scratch.sh"):
    executable(repo / "scripts/release" / script, "#!/bin/sh\nexit 0\n")
executable(repo / "scripts/release/gen-third-party-notices.sh", '#!/bin/sh\nprintf notice > "$1"\n')
for name in ("LICENSE-MIT", "LICENSE-APACHE", "LICENSES-THIRD-PARTY.md"):
    (repo / name).write_text("license")
(repo / "THIRD-PARTY-NOTICES.md").write_text("notice")
(repo / ".gitignore").write_text("ignored\n")
tracked = repo / "tracked"
tracked.write_text("committed\n")
os.utime(tracked, (time.time() - 60, time.time() - 60))  # avoid Git's racily-clean index guard
git("add", ".")
git("-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false",
    "-c", "core.hooksPath=/dev/null", "commit", "-qm", "fixture")
sha = git("rev-parse", "HEAD").stdout.decode().strip()
git("config", "core.trustctime", "false")
git("config", "core.checkstat", "minimal")
git("update-index", "--refresh")
stamp = tracked.stat()
tracked.write_text("altered!!\n")  # same byte length
os.utime(tracked, ns=(stamp.st_atime_ns, stamp.st_mtime_ns))
(repo / "ignored").write_text("should never enter the checkout")
assert not git("status", "--porcelain", "--untracked-files=all").stdout, "stat-cache control did not reproduce"

bin_dir = work / "bin"
executable(bin_dir / "cargo", '''#!/bin/sh
set -eu
[ "$(cat tracked)" = committed ]
[ ! -e ignored ]
[ "$(git rev-parse HEAD)" = "$EXPECTED_SHA" ]
[ "$CARGO_TARGET_DIR" != "$FORBIDDEN_TARGET" ]
printf x >> "$PROBE_MARKER"
''')
marker = work / "cargo-calls"
env = dict(os.environ, PATH=str(bin_dir) + ":" + os.environ["PATH"],
           CARGO_TARGET_DIR=str(work / "foreign target"), FORBIDDEN_TARGET=str(work / "foreign target"),
           EXPECTED_SHA=sha, PROBE_MARKER=str(marker))
result = subprocess.run(["bash", "scripts/release/preflight.sh"], cwd=repo, env=env,
                        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=30, text=True)
(work / "preflight.log").write_text(result.stdout)
assert result.returncode == 0, f"isolated preflight failed: {result.returncode}; see {work}/preflight.log"
assert marker.exists() and marker.stat().st_size >= 5, "fake compiler did not exercise the checks"
assert tracked.read_text() == "altered!!\n", "original working tree changed"
assert (repo / "ignored").read_text() == "should never enter the checkout"
assert git("rev-parse", "HEAD").stdout.decode().strip() == sha
shallow = work / "shallow checkout"
run("git", "clone", "--quiet", "--depth=1", repo.as_uri(), str(shallow))
assert run("git", "-C", str(shallow), "rev-parse", "--is-shallow-repository").stdout.strip() == b"true"
result = subprocess.run(["bash", "scripts/release/preflight.sh"], cwd=shallow, env=env,
                        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=30, text=True)
(work / "shallow-preflight.log").write_text(result.stdout)
assert result.returncode == 0, f"shallow checkout preflight failed; see {work}/shallow-preflight.log"
run("git", "-C", str(shallow), "checkout", "--quiet", "--detach", sha)
local_refs = run("git", "-C", str(shallow), "for-each-ref", "--format=%(refname)", "refs/heads", "refs/remotes").stdout.decode().splitlines()
for ref in local_refs:
    run("git", "-C", str(shallow), "update-ref", "-d", ref)
result = subprocess.run(["bash", "scripts/release/preflight.sh"], cwd=shallow, env=env,
                        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=30, text=True)
(work / "detached-preflight.log").write_text(result.stdout)
assert result.returncode == 0, f"detached checkout preflight failed; see {work}/detached-preflight.log"
print("PASS: dirty stat-cache and ignored source excluded; captured SHA and fresh target used; shallow/detached checkouts supported")
