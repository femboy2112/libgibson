# Go bindings (engineering alpha)

This cgo wrapper exposes a subset of LibGibson's C ABI. It requires a repository
checkout, a Go toolchain with cgo, a C compiler, and the native Rust library.
It is not a standalone `go get` distribution: include/library paths resolve from
the `gibson` package into this checkout.

From the repository root on Linux:

```sh
cargo build --release
cd bindings/go
export LD_LIBRARY_PATH="$PWD/../../target/release${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
go vet ./...
go test ./...
go build ./...
go run ./cmd/example
```

The module has no external Go dependencies and therefore no go.sum. The example
is a normal command package, included in vet/test/build. There are currently no
Go unit tests; example execution is a smoke check, not exhaustive wrapper coverage.
Linux execution was verified locally; public GitHub CI and other platforms remain
pending. The `go 1.18` module language directive is not a cross-platform support or
Rust MSRV promise. See the current evidence in
[State of LibGibson](../../docs/STATE_OF_LIBGIBSON.md#public-repository-readiness).
