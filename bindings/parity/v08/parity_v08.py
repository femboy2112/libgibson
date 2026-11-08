"""LibGibson v0.8 parity slice -- Python consumer.

Interprets bindings/parity/v08/script.txt against the INSTALLED native library through the
Python wrapper (``import gibson``; the library is located by the wrapper's documented
``LIBGIBSON_LIBRARY`` / loader search) and writes the canonical transcript to stdout.
Format: docs/V08_PARITY_SLICE.md.

    python3 parity_v08.py SCRIPT [REPS]

Surface used: the wrapper's public API (Context.headless, Node, set_root, render, frame_size,
frame_cells, frame_cell, frame_cell_glyph, rendered_output, push_event, poll_event, Event, Key,
KeyKind, close) wherever it offers the operation. What the wrapper cannot express -- a NULL
event/out pointer, a NULL kind pointer, a NULL context, calls on a context it never created --
goes through raw ctypes against the SAME shared object the wrapper loaded (``gibson._lib._name``);
that residual split is itself a measured result.

Negative controls (never on by default): PARITY_V08_PERTURB=cell | events (see parity_v08.c).
"""

import ctypes
import os
import resource
import sys
import time

import gibson

FNV_OFFSET = 0xCBF29CE484222325
FNV_PRIME = 0x100000001B3
MASK = (1 << 64) - 1

ST = {0: "OK", -1: "INVALID_PARAM", -2: "IO", -3: "PANIC", -4: "NOT_INITIALIZED", -5: "QUEUE_FULL"}
KIND = {0: "NONE", 1: "CHAR", 2: "SPECIAL"}
TYPE = {1: "KEY", 2: "PASTE", 3: "RESIZE"}
EV_KEY, EV_PASTE, EV_RESIZE = gibson.EventType.KEY, gibson.EventType.PASTE, gibson.EventType.RESIZE
KIND_CHAR, KIND_SPECIAL = gibson.KeyKind.CHAR, gibson.KeyKind.SPECIAL
QUEUE_CAPACITY = gibson.GIBSON_EVENT_QUEUE_CAPACITY
SPECIAL = {n: getattr(gibson.Key, n) for n in (
    "ENTER", "LEFT", "RIGHT", "UP", "DOWN", "HOME", "END", "BACKSPACE", "DELETE", "ESC", "TAB",
    "PAGE_UP", "PAGE_DOWN")}
WRAP = {"NONE": gibson.WrapMode.NONE, "CHAR": gibson.WrapMode.CHAR, "WORD": gibson.WrapMode.WORD}


# Raw prototypes on our own handle to the library the wrapper loaded (wrapper state untouched),
# for the probes the wrapper cannot express (NULL pointers, foreign contexts).
raw = ctypes.CDLL(gibson._lib._name)
_vp, _u16, _u32, _i32, _sz = ctypes.c_void_p, ctypes.c_uint16, ctypes.c_uint32, ctypes.c_int32, ctypes.c_size_t
for name, args in {
    "gibson_create_headless_context": [_i32, _u16, _u16, _vp],
    "gibson_render": [_vp],
    "gibson_get_frame_cell": [_vp, _u16, _u16, _vp],
    "gibson_get_frame_cells": [_vp, _u16, _u16, _vp, _sz, ctypes.POINTER(_sz)],
    "gibson_node_text": [ctypes.c_char_p, _vp, _i32, _vp],
    "gibson_poll_event_kind": [_vp, _u32, _vp, _vp],
    "gibson_set_color_depth": [_vp, _i32],
    "gibson_get_frame_size": [_vp, ctypes.POINTER(_u16), ctypes.POINTER(_u16)],
}.items():
    fn = getattr(raw, name)
    fn.argtypes = args
    fn.restype = _i32
for name in ("gibson_destroy_context", "gibson_node_free"):
    getattr(raw, name).argtypes = [_vp]
    getattr(raw, name).restype = None
raw.gibson_last_error_message.argtypes = [ctypes.c_char_p, _sz]
raw.gibson_last_error_message.restype = _i32


def fnv(h, data):
    for b in data:
        h = ((h ^ b) * FNV_PRIME) & MASK
    return h


def safe(s):
    if all(0x20 <= ord(c) <= 0x7E and c not in '"\\' for c in s):
        return s
    return "hex:" + s.encode("utf-8").hex()


def emit(out, rec):
    parts = []
    for k in sorted(rec):
        v = rec[k]
        if isinstance(v, str):
            parts.append('"%s":"%s"' % (k, safe(v)))
        elif isinstance(v, list):
            parts.append('"%s":[%s]' % (k, ",".join(str(int(x)) for x in v)))
        else:
            parts.append('"%s":%d' % (k, int(v)))
    out.append("{" + ",".join(parts) + "}\n")


def st_name(s):
    return ST.get(s, "UNKNOWN")


def last_err():
    buf = ctypes.create_string_buffer(512)
    raw.gibson_last_error_message(buf, 512)
    return buf.value.decode("utf-8", "replace")


def load_script(path):
    steps = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\r\n")
            if not line or line.startswith("#"):
                continue
            steps.append(line.split(" "))
    return steps


def apply_perturbation(steps):
    mode = os.environ.get("PARITY_V08_PERTURB", "")
    if mode == "cell":
        for s in steps:
            if s[0] == "s06":
                s[-1] = s[-1][:-2] + "3f"
    elif mode == "events":
        ids = [s[0] for s in steps]
        if "e01" in ids and "e02" in ids:
            a, b = ids.index("e01"), ids.index("e02")
            steps[a], steps[b] = steps[b], steps[a]


def parse_color(spec):
    if spec == "reset":
        return gibson.GibsonColor.named(gibson.ColorType.RESET)
    if spec.startswith("rgb:"):
        r, g, b = (int(x) for x in spec[4:].split(","))
        return gibson.GibsonColor.rgb(r, g, b)
    if spec.startswith("ansi:"):
        return gibson.GibsonColor(color_type=gibson.ColorType.ANSI256, r=int(spec[5:]), g=0, b=0, _pad=0)
    return gibson.GibsonColor.named(int(spec[6:]))


def attrs(st):
    return "".join("1" if v else "0" for v in (st.bold, st.dim, st.italic, st.underline, st.reverse))


def color4(c):
    return [c.color_type, c.r, c.g, c.b]


class Interp:
    def __init__(self, out):
        self.out = out
        self.ctx = None
        self.root = None
        self.styles = {}

    def full_glyph(self, cell, x, y):
        if cell.glyph_truncated:
            return self.ctx.frame_cell_glyph(x, y)
        return bytes(cell.glyph[: cell.glyph_len])

    def cell_line(self, c, x, y):
        s = c.style
        return "%s;%d;%d;%d;%d,%d,%d,%d;%d,%d,%d,%d;%s\n" % (
            self.full_glyph(c, x, y).hex(), c.width, c.is_continuation, c.transparent,
            s.fg.color_type, s.fg.r, s.fg.g, s.fg.b, s.bg.color_type, s.bg.r, s.bg.g, s.bg.b, attrs(s),
        )

    def push(self, ev):
        """Push through the wrapper; (C status, last-error text the wrapper captured on failure)."""
        try:
            self.ctx.push_event(ev)
            return 0, ""
        except gibson.GibsonError as e:
            return e.status, e.last_error

    def emit_event(self, sid, st, ev, kind):
        key, rs = ev.event_type == EV_KEY, ev.event_type == EV_RESIZE
        paste = ev.paste.encode("utf-8").hex() if ev.event_type == EV_PASTE and ev.paste else ""
        emit(self.out, {
            "cols": ev.cols if rs else 0, "id": sid, "k": "event",
            "key_code": ev.key_code if key else 0, "kind": kind, "mods": ev.modifiers if key else 0,
            "paste": paste, "rows": ev.rows if rs else 0, "st": st_name(st),
            "type": TYPE.get(ev.event_type, "NONE"),
        })

    def probe(self, sid, name):
        h = self.ctx.handle
        rec = {"id": sid, "k": "probe", "name": name}
        cols, rows = self.ctx.frame_size()
        out = ctypes.c_void_p()
        cell = gibson.GibsonCell()
        with_err = False
        st = None
        if name == "create_bad_mode":
            st, with_err = raw.gibson_create_headless_context(7, 40, 12, ctypes.byref(out)), True
        elif name == "create_zero_cols":
            st, with_err = raw.gibson_create_headless_context(1, 0, 12, ctypes.byref(out)), True
        elif name == "create_null_out":
            st = raw.gibson_create_headless_context(1, 40, 12, None)
        elif name == "render_null_ctx":
            st = raw.gibson_render(None)
        elif name == "cell_oob_x":
            st = raw.gibson_get_frame_cell(h, cols, 0, ctypes.byref(cell))
        elif name == "cell_oob_y":
            st = raw.gibson_get_frame_cell(h, 0, rows, ctypes.byref(cell))
        elif name == "cells_oob":
            written = ctypes.c_size_t(99)
            st = raw.gibson_get_frame_cells(h, 0, rows, ctypes.byref(cell), 1, ctypes.byref(written))
            rec["written"] = written.value
        elif name == "cell_null_out":
            st = raw.gibson_get_frame_cell(h, 0, 0, None)
        elif name == "text_bad_utf8":
            node = ctypes.c_void_p()
            st, with_err = raw.gibson_node_text(b"\xff\xfe", None, 0, ctypes.byref(node)), True
            if st == 0:
                raw.gibson_node_free(node)
        elif name == "push_null_event":
            st = raw.gibson_push_event(h, None)  # raw: the wrapper always passes a struct
        elif name == "push_surrogate":
            st, rec["err"] = self.push(gibson.Event.char(0xD800))
        elif name == "push_special_not_key":
            st = self.push(gibson.Event.special(ord("a")))[0]
        elif name == "push_bad_kind":
            st = self.push(gibson.Event(EV_KEY, ord("a"), key_kind=9))[0]
        elif name == "push_bad_mods":
            st = self.push(gibson.Event.char("a", 0x80))[0]
        elif name == "push_resize_zero":
            st = self.push(gibson.Event.resize(0, 12))[0]
        elif name == "paste_bad_utf8":
            st, rec["err"] = self.push(gibson.Event.paste_text(b"\xc3\x28"))
        elif name == "paste_null":
            st = self.push(gibson.Event(EV_PASTE, paste=None))[0]
        elif name == "poll_null_out":
            st = raw.gibson_poll_event(h, 0, None)  # raw: the wrapper always passes a struct
        elif name == "poll_kind_null":
            st = raw.gibson_poll_event_kind(h, 0, ctypes.byref(gibson.GibsonEvent()), None)  # raw: NULL kind
        elif name == "color_depth_bad":
            st = raw.gibson_set_color_depth(h, 9)
        elif name == "queue_full":
            q = gibson.Event.char("q")
            accepted = sum(1 for _ in range(QUEUE_CAPACITY) if self.push(q)[0] == 0)
            st, err = self.push(q)
            drained = 0
            for _ in range(2 * QUEUE_CAPACITY):
                if self.ctx.poll_event().event_type == 0:
                    break
                drained += 1
            rec.update(accepted=accepted, drained=drained, err=err)
        elif name == "frame_before_render":
            c2 = ctypes.c_void_p()
            raw.gibson_create_headless_context(1, 8, 2, ctypes.byref(c2))
            c, r = ctypes.c_uint16(7), ctypes.c_uint16(7)
            sst = raw.gibson_get_frame_size(c2, ctypes.byref(c), ctypes.byref(r))
            st = raw.gibson_get_frame_cell(c2, 0, 0, ctypes.byref(cell))
            raw.gibson_destroy_context(c2)
            rec.update(cols=c.value, rows=r.value, size_st=st_name(sst))
        if with_err:
            rec["err"] = last_err()
        if out.value:
            raw.gibson_destroy_context(out)
        rec["st"] = "UNKNOWN_PROBE" if st is None else st_name(st)
        emit(self.out, rec)

    def step(self, s):
        sid, op, a = s[0], s[2], s[3:]
        out = self.out
        if op == "create":
            mode = gibson.RenderMode.INLINE if a[0] == "INLINE" else gibson.RenderMode.FULLSCREEN
            try:
                self.ctx = gibson.Context.headless(mode, int(a[1]), int(a[2]))
                st = "OK"
            except RuntimeError:
                st = "WRAPPER_RAISED"
            emit(out, {"cols": int(a[1]), "id": sid, "k": "create", "mode": a[0], "rows": int(a[2]), "st": st})
        elif op == "style":
            f = a[3]
            self.styles[a[0]] = gibson.GibsonStyle.make(
                fg=parse_color(a[1]), bg=parse_color(a[2]), bold=f[0] == "1", dim=f[1] == "1",
                italic=f[2] == "1", underline=f[3] == "1", reverse=f[4] == "1",
            )
        elif op == "col":
            self.root = gibson.Node.col()
            emit(out, {"id": sid, "k": "node", "st": "OK" if self.root.handle else "WRAPPER_NULL"})
        elif op == "text":
            # Node.text drops the C status: a null handle is the only failure signal.
            node = gibson.Node.text(bytes.fromhex(a[2]).decode("utf-8"), self.styles.get(a[0]), WRAP[a[1]])
            ok = bool(node.handle)
            self.root.add_child(node)
            emit(out, {"id": sid, "k": "node", "st": "OK" if ok else "WRAPPER_NULL"})
        elif op == "mount":
            try:
                self.ctx.set_root(self.root)  # consumes the tree
                st = "OK"
            except RuntimeError:
                st = "WRAPPER_RAISED"
            emit(out, {"id": sid, "k": "mount", "st": st})
        elif op == "render":
            try:
                self.ctx.render()
                st = "OK"
            except RuntimeError:
                st = "WRAPPER_RAISED"
            emit(out, {"id": sid, "k": "render", "st": st})
        elif op == "geom":
            c, r = self.ctx.frame_size()
            emit(out, {"cols": c, "id": sid, "k": "geom", "rows": r, "st": "OK"})
        elif op == "frame":
            c, r = self.ctx.frame_size()
            cells = self.ctx.frame_cells()
            h = FNV_OFFSET
            for i, cell in enumerate(cells):
                h = fnv(h, self.cell_line(cell, i % c, i // c).encode("ascii"))
            wire = self.ctx.rendered_output()
            emit(out, {
                "cells": len(cells), "digest": "%016x" % h, "id": sid, "k": "frame", "st": "OK",
                "wire_digest": "%016x" % fnv(FNV_OFFSET, wire), "wire_len": len(wire),
            })
        elif op == "cell":
            x, y = int(a[0]), int(a[1])
            try:
                c = self.ctx.frame_cell(x, y)
                g, st = self.full_glyph(c, x, y), "OK"
            except RuntimeError:
                c, g, st = gibson.GibsonCell(), b"", "WRAPPER_RAISED"
            emit(out, {
                "attrs": attrs(c.style), "bg": color4(c.style.bg), "cont": c.is_continuation,
                "fg": color4(c.style.fg), "glyph": g.hex(), "id": sid, "k": "cell", "st": st,
                "transp": c.transparent, "w": c.width, "x": x, "y": y,
            })
        elif op == "glyph":
            x, y = int(a[0]), int(a[1])
            g = self.ctx.frame_cell_glyph(x, y)
            emit(out, {"full_hex": g.hex(), "full_len": len(g), "id": sid, "k": "glyph", "st": "OK", "x": x, "y": y})
        elif op == "glyphcap":
            x, y = int(a[0]), int(a[1])
            c = self.ctx.frame_cell(x, y)
            emit(out, {
                "id": sid, "inline_len": c.glyph_len, "k": "glyphcap", "st": "OK",
                "trunc": int(c.glyph_truncated), "x": x, "y": y,
            })
        elif op.startswith("push_"):
            if op in ("push_char", "push_legacy"):
                code, mods = int(a[0][2:], 16), int(a[1])
                # push_legacy is the kind-less call: key_kind None selects gibson_push_event.
                kind = KIND_CHAR if op == "push_char" else None
                ev = gibson.Event(EV_KEY, code, mods, key_kind=kind)
            elif op == "push_special":
                ev = gibson.Event.special(SPECIAL[a[0]], int(a[1]))
            elif op == "push_paste":
                ev = gibson.Event.paste_text(bytes.fromhex(a[0]))
            else:
                ev = gibson.Event.resize(int(a[0]), int(a[1]))
            emit(out, {"id": sid, "k": "push", "st": st_name(self.push(ev)[0])})
        elif op == "poll":
            for k in range(1, int(a[0]) + 1):
                ev = self.ctx.poll_event()
                self.emit_event("%s.%d" % (sid, k), 0, ev, KIND.get(ev.key_kind, "NONE"))
        elif op == "poll_legacy":
            ev = self.ctx.poll_event(kind=False)
            self.emit_event(sid, 0, ev, "ABSENT")
        elif op == "probe":
            self.probe(sid, a[0])
        elif op == "lifecycle":
            st = "OK"
            if a[0] == "node_free_unattached":
                n = gibson.Node.text("unattached", None, gibson.WrapMode.NONE)
                if not n.handle:
                    st = "WRAPPER_NULL"
                del n  # Node.__del__ frees it
            elif a[0] == "destroy_null":
                raw.gibson_destroy_context(None)
            emit(out, {"id": sid, "k": "lifecycle", "name": a[0], "st": st})
        elif op == "destroy":
            self.ctx.close()
            self.ctx = None
            emit(out, {"id": sid, "k": "destroy", "st": "OK"})
        else:
            emit(out, {"id": sid, "k": "unknown_op", "st": "UNSUPPORTED"})

    def run(self, steps):
        for s in steps:
            self.step(s)
        if self.ctx is not None:
            self.ctx.close()


def wrapper_observations():
    """Runtime facts about the wrapper surface (stderr, not part of the comparable transcript)."""
    try:
        gibson.Node.text("\udc80")
        rejects = 0
    except UnicodeEncodeError:
        rejects = 1
    ctx = gibson.Context.headless(gibson.RenderMode.FULLSCREEN, 4, 2)
    ctx.close()
    ctx.close()  # idempotent at the wrapper level (handle set to None)
    obs = {
        "close_twice_safe": 1,
        **{
            "ctx_has_" + m: int(hasattr(gibson.Context, m))
            for m in ("headless", "poll_event", "push_event", "frame_size", "frame_cells", "frame_cell_glyph", "rendered_output")
        },
        "k": "wrapper",
        "lang": "py",
        "module_has_event_struct": int(any(hasattr(gibson, n) for n in ("GibsonEvent", "Event"))),
        "node_text_rejects_lone_surrogate": rejects,
        "node_text_status_observable": 0,
    }
    lines = []
    emit(lines, obs)
    sys.stderr.write(lines[0])


def main():
    if len(sys.argv) < 2:
        sys.stderr.write("usage: parity_v08.py SCRIPT [REPS]\n")
        return 2
    reps = max(1, int(sys.argv[2])) if len(sys.argv) > 2 else 1
    steps = load_script(sys.argv[1])
    apply_perturbation(steps)
    wrapper_observations()

    t0 = time.monotonic_ns()
    first = []
    Interp(first).run(steps)
    first_s = "".join(first)
    rss1 = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    rssmid = rss1
    identical = 1
    for rep in range(2, reps + 1):
        b = []
        Interp(b).run(steps)
        if "".join(b) != first_s:
            identical = 0
        if rep == (reps + 1) // 2:
            rssmid = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    wall_us = (time.monotonic_ns() - t0) // 1000
    sys.stdout.write(first_s)
    if reps > 1:
        res = []
        emit(res, {
            "iter_identical": identical, "k": "resource", "lang": "py", "maxrss_kb_1": rss1, "maxrss_kb_mid": rssmid,
            "maxrss_kb_n": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
            "per_rep_us": wall_us // reps, "reps": reps, "wall_ms": wall_us // 1000,
        })
        sys.stderr.write(res[0])
    return 0


if __name__ == "__main__":
    sys.exit(main())
