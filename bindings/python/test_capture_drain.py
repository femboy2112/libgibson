"""Capture drain through the Python binding (#297).

Run with PYTHONPATH=bindings/python and LIBGIBSON_LIBRARY pointing at a built libgibson.so.
Exit 0 iff every check holds. The MemoryError check is the regression for a reviewed defect: the
first wrapper drained natively and then built its result, so a failed allocation lost the capture.
"""

import ctypes
import sys

import gibson
from gibson import Context

failures = []


def check(ok, what):
    if not ok:
        failures.append(what)
        print("FAIL", what)


def rendered_context(frames=3):
    ctx = Context.headless(gibson.RenderMode.FULLSCREEN, 20, 4)
    for i in range(frames):
        ctx.commit_text(f"frame {i} " + "x" * 10)
        ctx.render()
    return ctx


ctx = rendered_context()
full = ctx.rendered_output()
check(len(full) > 0, "a headless context captured bytes")
taken = ctx.take_rendered_output()
check(taken == full, "take returns exactly what get returned")
check(ctx.rendered_output() == b"", "the capture is empty after take")
check(ctx.take_rendered_output() == b"", "a second take returns nothing")

ctx = rendered_context()
n = len(ctx.rendered_output())
check(ctx.clear_rendered_output() == n, "clear returns the number of bytes removed")
check(ctx.clear_rendered_output() == 0, "a second clear removes nothing")

# A failure while building the result must not lose the capture.
ctx = rendered_context()
before = ctx.rendered_output()
real_string_at = ctypes.string_at


def failing_string_at(*args, **kwargs):
    raise MemoryError("injected")


ctypes.string_at = failing_string_at
try:
    try:
        ctx.take_rendered_output()
        check(False, "the injected MemoryError propagates")
    except MemoryError:
        pass
finally:
    ctypes.string_at = real_string_at
check(ctx.rendered_output() == before, "a MemoryError while building the result leaves the capture intact")

# Control: the order the first wrapper used (drain natively, then build) loses it.
ctx = rendered_context()
need = ctypes.c_size_t(0)
gibson._lib.gibson_take_rendered_output(ctx.handle, None, 0, ctypes.byref(need))
buf = (ctypes.c_uint8 * need.value)()
got = ctypes.c_size_t(0)
gibson._lib.gibson_take_rendered_output(ctx.handle, buf, need.value, ctypes.byref(got))
# ...a MemoryError here would leave the caller with nothing, and the capture is already gone:
check(ctx.rendered_output() == b"", "control: a native take empties the capture before Python builds anything")

if failures:
    print(f"test_capture_drain: {len(failures)} failure(s)")
    sys.exit(1)
print("test_capture_drain: OK")
