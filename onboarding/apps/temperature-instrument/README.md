# Simulated temperature instrument

A small monitor using the public Python wrapper. The sample is a deterministic
**simulated** Celsius value, not a sensor measurement. The setpoint is an
independent **user request**, not a producer observation or an actuator command.
Status compares the sample and setpoint. `+`/`-` adjust the requested setpoint
by 0.5 °C; edits outside 10–40 °C are refused. `p` pauses/resumes the simulated
producer, and `q`, Escape or Ctrl-C quits. The display adapts to resize by
clipping long lines. Connect a real producer and hardware control only after
establishing their own units, authority, validation and failure handling.

Requires Python 3.9+, the separately built development `libgibson` Python wheel
listed in `requirements.txt`, and a staged Linux native SDK (ABI v1) containing
`libgibson.so.1`. No packages are downloaded from PyPI. After copying this
directory outside the checkout and creating a virtual environment:

```sh
python -m pip install --no-index --find-links "$GIBSON_WHEELS" -r requirements.txt
export LIBGIBSON_LIBRARY="$GIBSON_SDK/lib/libgibson.so.1"
python app.py --self-test   # test samples, edits, rejection and resize in memory
python app.py               # interactive terminal
```

`GIBSON_WHEELS` points to a directory with the matching local development wheel;
see `bindings/python/README.md` in the repository for building it. Interactive
mode requires TTY stdin and stdout and rejects `TERM=dumb`; it exits with a
diagnostic when redirected. Terminal support depends on the host and is not
implied by the headless test. Use a compatible development wrapper and native
library; the v0.6 release does not provide every API used here.
