# LibGibson — Python wrapper

A `ctypes` wrapper for the [LibGibson](https://github.com/femboy2112/libgibson)
terminal UI engine. It calls into the native `libgibson` shared library, which you
install **separately** — this package does not bundle a native binary.

## Requirements

- Python >= 3.8
- A native `libgibson` compatible with **C ABI v1**. The wrapper calls
  `gibson_abi_version()` on import and raises a clear error on mismatch, rather than
  crashing later.

## Installing the native library

Stage the native SDK from a LibGibson checkout:

```
scripts/release/stage-sdk.sh --prefix ~/.local/libgibson-sdk
```

Then tell the wrapper where the library is — either an explicit path:

```
export LIBGIBSON_LIBRARY=~/.local/libgibson-sdk/lib/libgibson.so
```

or via the dynamic loader:

```
export LD_LIBRARY_PATH=~/.local/libgibson-sdk/lib
```

The wrapper's library search order is: `LIBGIBSON_LIBRARY` → the system loader
(`ctypes.util.find_library("gibson")`) → a bare `libgibson.so` on the loader path.
It never silently searches a source-tree `target/` directory.

## Using

```python
import gibson

with gibson.Context(gibson.RenderMode.INLINE) as ctx:
    ctx.commit("hello from python")
    ctx.render()
```

## Development (from a source checkout)

```
cargo build --release
export LIBGIBSON_LIBRARY="$PWD/target/release/libgibson.so"
python bindings/python/example.py
```
