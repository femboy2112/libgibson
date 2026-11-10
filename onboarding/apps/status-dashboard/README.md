# Simulated status dashboard

A live dashboard using the public Python wrapper. Values are explicitly labelled
as **simulated**, not readings from a producer or machine. `p` pauses or resumes,
`r` resets, and `q`, Escape or Ctrl-C quits. The layout clips at small widths
and responds to terminal resize.

Requires Python 3.9+, the separately built development `libgibson` Python wheel
listed in `requirements.txt`, and a staged Linux native SDK (ABI v1) containing
`libgibson.so.1`. No packages are downloaded from PyPI. For example, after
copying this directory outside the checkout and creating a virtual environment:

```sh
python -m pip install --no-index --find-links "$GIBSON_WHEELS" -r requirements.txt
export LIBGIBSON_LIBRARY="$GIBSON_SDK/lib/libgibson.so.1"
python app.py --self-test   # in-memory rendering and injected input
python app.py               # interactive terminal
```

`GIBSON_WHEELS` is a directory containing the matching locally built wheel; see
`bindings/python/README.md` in the repository for building it. Interactive mode
requires TTY stdin and stdout and rejects `TERM=dumb`; it exits with a diagnostic
when redirected. Terminal support depends on the host and is not implied by the
headless test. The public wrapper and native library must come from a compatible
same-development build; the v0.6 release does not provide every API used here.
