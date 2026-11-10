# First five minutes: Python and the released SDK

This Linux x86_64 walkthrough uses the published **v0.6.0** GitHub prerelease.
You need `curl`, `tar`, `sha256sum`, Python 3 with `venv` and a network connection.
It installs into a new directory, without changing your system Python or requiring
an interactive terminal.

```sh
(
set -eu
mkdir libgibson-first-run
cd libgibson-first-run
base=https://github.com/femboy2112/libgibson/releases/download/v0.6.0
curl -fLO "$base/libgibson-0.6.0-linux-x86_64.tar.gz"
curl -fLO "$base/libgibson-0.6.0-linux-x86_64.tar.gz.sha256"
curl -fLO "$base/libgibson-0.6.0-py3-none-any.whl"
sha256sum -c libgibson-0.6.0-linux-x86_64.tar.gz.sha256
tar xzf libgibson-0.6.0-linux-x86_64.tar.gz
python3 -m venv .venv
.venv/bin/pip install --no-index --no-deps ./libgibson-0.6.0-py3-none-any.whl
export LIBGIBSON_LIBRARY="$PWD/libgibson-0.6.0-linux-x86_64/lib/libgibson.so.1"
.venv/bin/python - <<'PY'
import gibson

with gibson.Context.headless(gibson.RenderMode.FULLSCREEN, 40, 8) as ctx:
    root = gibson.Node.col()
    root.add_child(gibson.Node.text("Hello, LibGibson"))
    ctx.set_root(root)
    ctx.render()
    assert ctx.frame_size() == (40, 8)
    assert ctx.frame_cell_glyph(0, 0) == b"H"
    print("Headless frame 40x8 OK")
PY
)
```

The block runs in a subshell. To continue experimenting afterward, run
`cd libgibson-first-run` and export `LIBGIBSON_LIBRARY="$PWD/libgibson-0.6.0-linux-x86_64/lib/libgibson.so.1"`
again in your shell.

The two assertions form a small automated headless test: change the expected
`b"H"` to another byte to check that a failing test is detected. Headless success
does not establish your terminal's interactive capabilities.

If Python cannot load the library, check that `LIBGIBSON_LIBRARY` names the unpacked
`.so.1` file and that the archive matches your architecture. An ABI mismatch means
the wrapper and native library do not agree: install matching release artifacts.
A missing `venv`, `curl` or compiler is a missing local tool, not a LibGibson test
failure. For interactive applications, run in a real TTY with a supported `TERM`;
`TERM=dumb` is rejected by the example apps. Headless execution works without a TTY.

For C, C++, Go, Rust, development SDKs and the interactive applications, see
[the five-language onboarding kit](../onboarding/README.md). The development
examples include APIs newer than v0.6.0; use the version specified by each example.
