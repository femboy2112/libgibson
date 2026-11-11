"""Event push/poll through the Python binding (v0.8 parity slice, finding F5).

Run with PYTHONPATH=bindings/python and LIBGIBSON_LIBRARY pointing at a built libgibson.so.
Exit 0 iff every check holds. Needs no terminal: events only exist on headless contexts here.
The header (include/gibson.h next to this checkout, or LIBGIBSON_INCLUDE) is parsed so a drift
between the C ABI and the Python constants/struct fails loudly instead of silently.
"""

import ctypes
import os
import re
import shutil
import subprocess
import sys
import tempfile

import gibson
from gibson import (Context, Event, EventType, GibsonError, GibsonEvent, Key, KeyKind, Mod,
                    QueueFullError, RenderMode, Status, GIBSON_EVENT_QUEUE_CAPACITY)

failures = []


def check(ok, what):
    if not ok:
        failures.append(what)
        print("FAIL", what)


def raises(exc, fn, what):
    try:
        fn()
    except exc as e:
        return e
    except Exception as e:  # wrong type
        check(False, f"{what}: raised {type(e).__name__}, wanted {exc.__name__}")
        return None
    check(False, f"{what}: did not raise")
    return None


def headless(cols=40, rows=12, mode=RenderMode.FULLSCREEN):
    return Context.headless(mode, cols, rows)


# --- layout and constants vs the header --------------------------------------------------------
header_path = os.path.join(os.environ.get("LIBGIBSON_INCLUDE", os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "..", "..", "include")), "gibson.h")
header = open(header_path, encoding="utf-8").read()

if ctypes.sizeof(ctypes.c_void_p) == 8:
    check(ctypes.sizeof(GibsonEvent) == 24, "sizeof(GibsonEvent) is 24 on LP64")
want_offsets = {"event_type": 0, "key_code": 4, "modifiers": 8, "_pad": 9, "cols": 12, "rows": 14,
                "paste_str": 16}
if ctypes.sizeof(ctypes.c_void_p) == 8:
    for name, off in want_offsets.items():
        check(getattr(GibsonEvent, name).offset == off, f"GibsonEvent.{name} offset {off}")

cc = shutil.which("cc") or shutil.which("gcc")
if cc:
    fields = ["event_type", "key_code", "modifiers", "_pad", "cols", "rows", "paste_str"]
    prog = ('#include <stddef.h>\n#include <stdio.h>\n#include "gibson.h"\n'
            'int main(void){printf("' + " ".join(["%zu"] * 8) + '\\n", sizeof(gibson_event_t)'
            + "".join(f", offsetof(gibson_event_t, {f})" for f in fields) + "); return 0;}\n")
    with tempfile.TemporaryDirectory() as d:
        c_file, exe = os.path.join(d, "layout.c"), os.path.join(d, "layout")
        open(c_file, "w").write(prog)
        r = subprocess.run([cc, "-I", os.path.dirname(header_path), c_file, "-o", exe],
                           capture_output=True, text=True)
        if r.returncode == 0:
            nums = [int(x) for x in subprocess.run([exe], capture_output=True, text=True).stdout.split()]
            check(nums[0] == ctypes.sizeof(GibsonEvent), f"sizeof(GibsonEvent) == C sizeof ({nums[0]})")
            for f, n in zip(fields, nums[1:]):
                check(getattr(GibsonEvent, f).offset == n, f"GibsonEvent.{f} offset == C offsetof ({n})")
        else:
            check(False, "C layout probe compiled: " + r.stderr[:200])
else:
    print("SKIP C layout probe (no cc); the LP64 offsets above still ran")


def defined(name):
    return int(re.search(r"#define\s+%s\s+(0x[0-9A-Fa-f]+|\d+)u?\b" % name, header).group(1), 0)


for n in ("ENTER", "LEFT", "RIGHT", "UP", "DOWN", "HOME", "END", "BACKSPACE", "DELETE", "ESC", "TAB",
          "PAGE_UP", "PAGE_DOWN"):
    check(getattr(Key, n) == defined("GIBSON_KEY_" + n), f"Key.{n} matches the header")
for n in ("NONE", "CHAR", "SPECIAL"):
    check(getattr(KeyKind, n) == defined("GIBSON_KEY_KIND_" + n), f"KeyKind.{n} matches the header")
for n in ("SHIFT", "CONTROL", "ALT"):
    check(getattr(Mod, n) == defined("GIBSON_MOD_" + n), f"Mod.{n} matches the header")
check(GIBSON_EVENT_QUEUE_CAPACITY == defined("GIBSON_EVENT_QUEUE_CAPACITY"), "queue capacity matches")
for n in ("NONE", "KEY", "PASTE", "RESIZE", "FOCUS_GAINED", "FOCUS_LOST"):
    m = re.search(r"GIBSON_EVENT_%s\s*=\s*(\d+)" % n, header)
    check(m and getattr(EventType, n) == int(m.group(1)), f"EventType.{n} matches the header")
for n, hn in (("OK", "GIBSON_OK"), ("ERR_INVALID_PARAM", "GIBSON_ERR_INVALID_PARAM"),
              ("ERR_QUEUE_FULL", "GIBSON_ERR_QUEUE_FULL"), ("ERR_PANIC", "GIBSON_ERR_PANIC")):
    m = re.search(r"%s\s*=\s*(-?\d+)" % hn, header)
    check(m and getattr(Status, n) == int(m.group(1)), f"Status.{n} matches the header")

# --- empty queue ------------------------------------------------------------------------------
with headless() as ctx:
    ev = ctx.poll_event()
    check(ev.event_type == EventType.NONE and ev.key_kind == KeyKind.NONE, "empty queue polls NONE")
    check((ev.key_code, ev.modifiers, ev.cols, ev.rows, ev.paste) == (0, 0, 0, 0, None),
          "an empty poll writes every field as zero")
    ev = ctx.poll_event(kind=False)
    check(ev.event_type == EventType.NONE and ev.key_kind is None, "kind-less empty poll")

# --- KEY: CHAR vs SPECIAL, and the U+1001 identity --------------------------------------------
with headless() as ctx:
    ctx.push_event(Event.char("a", Mod.SHIFT | Mod.CONTROL))
    ctx.push_event(Event.char(0x1001))           # a typed U+1001 (Myanmar letter)
    ctx.push_event(Event.special(Key.ENTER))     # the constant with the same value
    ctx.push_event(Event.char("\U0001F600", Mod.ALT))
    ctx.push_event(Event(EventType.KEY, 0x1001, 0))          # kind-less push: always the special key
    ctx.push_event(Event(EventType.KEY, ord("z"), 0))        # kind-less char
    got = [ctx.poll_event() for _ in range(6)]
    check(got[0] == Event(EventType.KEY, ord("a"), Mod.SHIFT | Mod.CONTROL, key_kind=KeyKind.CHAR), "char a")
    check(got[1] == Event(EventType.KEY, 0x1001, 0, key_kind=KeyKind.CHAR), "typed U+1001 stays CHAR")
    check(got[2] == Event(EventType.KEY, 0x1001, 0, key_kind=KeyKind.SPECIAL), "Key.ENTER stays SPECIAL")
    check(got[1].key_code == got[2].key_code and got[1].key_kind != got[2].key_kind,
          "same key_code, different identity")
    check(got[3].key_code == 0x1F600 and got[3].modifiers == Mod.ALT, "astral char and ALT")
    check(got[4].key_kind == KeyKind.SPECIAL and got[4].key_code == Key.ENTER, "kind-less 0x1001 is ENTER")
    check(got[5].key_kind == KeyKind.CHAR, "kind-less 'z' is CHAR")
    check(ctx.poll_event().event_type == EventType.NONE, "queue is empty again")

    ctx.push_event(Event.char(0x1001))
    ctx.push_event(Event.special(Key.ENTER))
    k1, k2 = ctx.poll_event(kind=False), ctx.poll_event(kind=False)
    check(k1.key_code == k2.key_code == 0x1001 and k1.key_kind is None and k2.key_kind is None,
          "kind-less poll conflates them, as the header says")

# --- PASTE ------------------------------------------------------------------------------------
with headless() as ctx:
    text = "héllo 世界\n\U0001F600"
    ctx.push_event(Event.paste_text(text))
    ev = ctx.poll_event()
    check(ev.event_type == EventType.PASTE and ev.paste == text and ev.key_kind == KeyKind.NONE,
          "paste round trips as str")
    ctx.push_event(Event.paste_text(b"bytes ok"))
    check(ctx.poll_event().paste == "bytes ok", "bytes paste accepted")
    ctx.push_event(Event.paste_text(""))
    check(ctx.poll_event().paste == "", "empty paste is an empty str, not None")
    e = raises(GibsonError, lambda: ctx.push_event(Event.paste_text(b"\xc3\x28")), "invalid UTF-8 paste")
    check(e is not None and e.status == Status.ERR_INVALID_PARAM and e.last_error != "",
          "invalid UTF-8 paste: INVALID_PARAM with last-error text")
    raises(GibsonError, lambda: ctx.push_event(Event(EventType.PASTE)), "paste without text")
    raises(ValueError, lambda: ctx.push_event(Event.paste_text("a\0b")), "paste with NUL")
    check(ctx.poll_event().event_type == EventType.NONE, "refused pastes were not queued")

# --- RESIZE -----------------------------------------------------------------------------------
with headless(40, 12) as ctx:
    root = gibson.Node.col().add_child(gibson.Node.text("x", None, gibson.WrapMode.NONE))
    ctx.set_root(root)
    ctx.render()
    check(ctx.frame_size() == (40, 12), "fullscreen frame is 40x12")
    ctx.push_event(Event.resize(60, 16))
    # frame_size reports the last rendered frame, so render before looking: a push that applied
    # the geometry itself would show 60x16 here.
    ctx.render()
    check(ctx.frame_size() == (40, 12), "a queued resize does nothing until polled (re-rendered)")
    ev = ctx.poll_event()
    check(ev == Event(EventType.RESIZE, cols=60, rows=16, key_kind=KeyKind.NONE), "RESIZE polled")
    ctx.render()
    check(ctx.frame_size() == (60, 16), "the polled RESIZE applied the geometry")
    for c, r in ((0, 12), (12, 0), (0, 0)):
        e = raises(GibsonError, lambda: ctx.push_event(Event.resize(c, r)), f"resize {c}x{r}")
        check(e is not None and e.status == Status.ERR_INVALID_PARAM, f"resize {c}x{r} is INVALID_PARAM")
    check(ctx.poll_event().event_type == EventType.NONE, "refused resizes were not queued")
    raises(ValueError, lambda: ctx.push_event(Event.resize(70000, 5)), "cols beyond u16")

# --- QUEUE_FULL -------------------------------------------------------------------------------
with headless() as ctx:
    # Distinguishable events (CJK code points U+4E00 + i), so a displaced, duplicated, reordered
    # or inserted event shows up in the drained sequence; the overflow is a distinct sentinel.
    queued = [Event.char(0x4E00 + i) for i in range(GIBSON_EVENT_QUEUE_CAPACITY)]
    for ev in queued:
        ctx.push_event(ev)
    e = raises(QueueFullError, lambda: ctx.push_event(Event.char("Z")), "push past capacity")
    check(e is not None and e.status == Status.ERR_QUEUE_FULL and isinstance(e, RuntimeError)
          and "full" in e.last_error, "QueueFullError carries status and last-error text")
    drained = []
    while True:
        ev = ctx.poll_event()
        if ev.event_type == EventType.NONE:
            break
        drained.append(ev)
    check(drained == queued, "the refusal displaced nothing: every queued event, in order, no sentinel")
    ctx.push_event(Event.char("r"))
    check(ctx.poll_event().key_code == ord("r"), "the queue accepts again once drained")

# --- invalid parameters -----------------------------------------------------------------------
with headless() as ctx:
    for what, ev in (
        ("surrogate as CHAR", Event.char(0xD800)),
        ("a non-special SPECIAL", Event.special(ord("a"))),
        ("unknown kind", Event(EventType.KEY, ord("a"), 0, key_kind=9)),
        ("reserved modifier bit", Event.char("a", 0x80)),
    ):
        e = raises(GibsonError, lambda: ctx.push_event(ev), what)
        check(e is not None and e.status == Status.ERR_INVALID_PARAM and not isinstance(e, QueueFullError),
              f"{what}: INVALID_PARAM")
    check(ctx.poll_event().event_type == EventType.NONE, "refused keys were not queued")
    raises(ValueError, lambda: ctx.push_event(Event.char("a", 256)), "modifiers beyond u8")
    raises(ValueError, lambda: ctx.push_event(Event(EventType.KEY, -1)), "negative key_code")
    # NONE events carry nothing to queue; the ABI's verdict is the wrapper's.
    try:
        ctx.push_event(Event())
        check(ctx.poll_event().event_type in (EventType.NONE,), "a pushed NONE polls back as NONE")
    except GibsonError as e:
        check(e.status == Status.ERR_INVALID_PARAM, "a pushed NONE is refused as INVALID_PARAM")

# A terminal (non-headless) context refuses pushes. Skipped when no TTY can be opened.
try:
    term = Context(RenderMode.INLINE)
except RuntimeError:
    term = None
    print("SKIP terminal-context push refusal (no terminal)")
if term is not None:
    with term:
        e = raises(GibsonError, lambda: term.push_event(Event.char("a")), "push on a terminal context")
        check(e is not None and e.status == Status.ERR_INVALID_PARAM, "terminal context: INVALID_PARAM")

# --- a polled event pushes back unchanged -----------------------------------------------------
with headless() as ctx:
    originals = [
        Event.char("a", Mod.SHIFT), Event.char(0x1001), Event.special(Key.TAB, Mod.CONTROL),
        Event.paste_text("round\ntrip 世"), Event.resize(33, 9),
    ]
    for ev in originals:
        ctx.push_event(ev)
    first = [ctx.poll_event() for _ in originals]
    for ev in first:
        ctx.push_event(ev)  # as polled, nothing rebuilt
    second = [ctx.poll_event() for _ in originals]
    check(first == second, "poll -> push -> poll is the identity")
    check(first[1].key_kind == KeyKind.CHAR and first[2].key_kind == KeyKind.SPECIAL, "kinds survive the loop")
    # ...and the kind-less loop too.
    ctx.push_event(Event.char(0x1001))
    a = ctx.poll_event(kind=False)
    ctx.push_event(a)
    check(ctx.poll_event(kind=False) == a, "kind-less poll -> push -> poll is the identity")

# Focus and geometry: before rendering, around a queued resize, and event round trip.
with headless() as ctx:
    check(ctx.terminal_size() == (40, 12), "terminal size before render")
    ctx.set_focus_reporting(True)
    ctx.push_event(Event(EventType.FOCUS_GAINED))
    ctx.push_event(Event.resize(33, 9))
    ctx.push_event(Event(EventType.FOCUS_LOST))
    check(ctx.terminal_size() == (40, 12), "queued resize does not apply before poll")
    check(ctx.poll_event().event_type == EventType.FOCUS_GAINED, "focus gained")
    check(ctx.poll_event().event_type == EventType.RESIZE, "resize")
    check(ctx.terminal_size() == (33, 9), "size after poll")
    e = ctx.poll_event()
    check(e.event_type == EventType.FOCUS_LOST, "focus lost")
    ctx.push_event(e)
    check(ctx.poll_event() == e, "focus event round trip")
    ctx.set_focus_reporting(False)
ctx.close()
e = raises(GibsonError, lambda: ctx.terminal_size(), "closed terminal size")
check(e is not None and e.status == Status.ERR_INVALID_PARAM, "closed context error")
e = raises(GibsonError, lambda: ctx.set_focus_reporting(True), "closed focus reporting")
check(e is not None and e.status == Status.ERR_INVALID_PARAM and not e.last_error,
      "focus error does not reuse stale terminal-size error")

# --- misc -------------------------------------------------------------------------------------
raises(Exception, lambda: setattr(Event(), "key_code", 1), "Event is immutable")

if failures:
    print(f"{len(failures)} check(s) failed")
    sys.exit(1)
print("test_events: all checks passed")
