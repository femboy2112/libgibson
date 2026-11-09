# Five headless starters

Each small program creates a 40×8 in-memory fullscreen frame. They work without a
TTY and use the public Rust or wrapper interfaces. A successful run prints its
language and `headless frame 40x8 OK`. These are starting points for automated
headless testing; they do not test real terminal behavior.

## Prepare artifacts outside the checkout

The native SDK does not include the Python, Go or Rust packages. From a checkout,
with Rust, C/C++, pkg-config, Python (venv, pip and setuptools), Go and Git installed:

```sh
# Choose a new directory outside the checkout.
export LAB="$(mktemp -d)"
scripts/release/stage-sdk.sh --prefix "$LAB/sdk"
python3 -m venv "$LAB/venv"
"$LAB/venv/bin/pip" install ./bindings/python
cp -R bindings/go "$LAB/go-module"
cargo package --allow-dirty
# Replace this version if the checkout version differs.
tar xzf target/package/libgibson-0.7.0-dev.crate -C "$LAB"
export GIBSON_SDK="$LAB/sdk"
export GIBSON_GO_MODULE="$LAB/go-module"
export GIBSON_RUST_CRATE="$LAB/libgibson-0.7.0-dev"
export PYTHON="$LAB/venv/bin/python"
./onboarding/smoke.sh
```

The script copies the consumers to a new temporary directory before compiling.
Its SDK probe checks header/library ABI agreement before building the examples;
linker, loader, missing toolchain and example failures are surfaced with context.
It sets `PKG_CONFIG_PATH` and `LD_LIBRARY_PATH` to the selected SDK. It requires
cached Rust dependencies or network access. The Go module copy allows the check
to run without the Go module proxy. Set `TMPDIR` if `/tmp` lacks space.
To check C and C++ against a copied SDK path containing spaces, including a
missing-package control, run `./onboarding/test-sdk-paths.sh` with `GIBSON_SDK`
set. This smaller check does not require Cargo or exercise the other languages.

## Run one starter

Copy the desired directory outside the checkout. For C and C++, export
`PKG_CONFIG_PATH="$GIBSON_SDK/lib/pkgconfig"` and
`LD_LIBRARY_PATH="$GIBSON_SDK/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"`, then:

```sh
cc -std=c11 -I"$GIBSON_SDK/include" main.c -L"$GIBSON_SDK/lib" -lgibson -o app && ./app
c++ -std=c++17 -I"$GIBSON_SDK/include" main.cpp -L"$GIBSON_SDK/lib" -lgibson -o app && ./app
```

Run the line matching the copied language. Python needs the separately installed
wrapper and `LIBGIBSON_LIBRARY="$GIBSON_SDK/lib/libgibson.so.1"`.
The Go example requires cgo and the SDK pkg-config/loader settings. Its `go.mod`
uses a placeholder: run `go mod edit -replace=github.com/femboy2112/libgibson/bindings/go="$GIBSON_GO_MODULE"` in the copied directory before `go run .`. These headless wrapper APIs are from development; the published v0.6.0 Go module does not provide them. The Rust example selects the published
`v0.6.0` source tag and its dependencies with Cargo. To test development artifacts,
use the smoke script's staged-module and unpacked-crate inputs instead.

Interactive terminals, resize/input, dashboard, chat, scientific-instrument, and
framebuffer examples remain separate work; passing this smoke does not certify them.
