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
`../../docs/RELEASING.md`); `bindings/go/v0.6.0` is the latest released module tag.

## Notes

The module has no external Go dependencies and therefore no go.sum. The example is a
normal command package. Go unit tests cover headless event and readback paths; example
execution is a smoke check, not exhaustive wrapper coverage. Linux execution was verified locally
and in public Go 1.27.1 CI; other platforms remain unverified. The `go 1.18` module
language directive is not a cross-platform support or Rust MSRV promise.

## Geometry and focus

`ctx.TerminalSize()` returns live columns and rows before rendering. A headless
context applies an injected resize when it is polled; a failed terminal measurement
uses the ABI's `80x24` fallback. `ctx.SetFocusReporting(true)` requests notifications;
poll for `gibson.EventFocusGained` and `gibson.EventFocusLost`. Disable with `false`;
`Close` restores the terminal. The request is a headless no-op and does not guarantee
terminal support. These methods require the additive v0.7 native symbols even though
the ABI version remains 1.
