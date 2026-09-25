# Go bindings (engineering alpha)

This cgo wrapper exposes a subset of LibGibson's C ABI. It requires a Go toolchain
with cgo, a C compiler, and an installed **native LibGibson SDK** discoverable via
`pkg-config`. It no longer resolves the native library from a source-tree `target/`
directory.

Module path: `github.com/femboy2112/libgibson/bindings/go`.

## Native library (pkg-config)

The cgo directives use `#cgo pkg-config: libgibson` and `#include <gibson.h>`, so a
`libgibson.pc` must be on your `PKG_CONFIG_PATH`. Stage the native SDK from a
LibGibson checkout:

```sh
scripts/release/stage-sdk.sh --prefix /tmp/libgibson-sdk
export PKG_CONFIG_PATH=/tmp/libgibson-sdk/lib/pkgconfig
export LD_LIBRARY_PATH=/tmp/libgibson-sdk/lib   # so the built binary finds libgibson.so
```

Then, from `bindings/go`:

```sh
go vet ./...
go test ./...
go build ./...
go run ./cmd/example
```

## ABI compatibility

The wrapper is compiled against `GIBSON_ABI_VERSION` from the header it built with,
exposed as `gibson.ExpectedABIVersion`. `gibson.CheckABI()` compares it to the
loaded library's `gibson_abi_version()` and returns a clear error on mismatch;
`NewContext` calls it, so an incompatible native library fails cleanly rather than
corrupting memory.

## Release tag

Because this module lives in a subdirectory, a Go module release is tagged with the
module-path prefix — `bindings/go/vX.Y.Z` (e.g. `bindings/go/v0.1.0`), **not** a bare
`vX.Y.Z`. The tag is created only as part of a coordinated release (see
`../../docs/RELEASING.md`). No such tag exists yet.

## Notes

The module has no external Go dependencies and therefore no go.sum. The example is a
normal command package. There are currently no Go unit tests; example execution is a
smoke check, not exhaustive wrapper coverage. Linux execution was verified locally
and in public Go 1.27.1 CI; other platforms remain unverified. The `go 1.18` module
language directive is not a cross-platform support or Rust MSRV promise.
