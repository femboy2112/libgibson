#!/usr/bin/env python3
"""Standalone contract check of a `gibson.figure.tape.v1` tape (docs/design/m5c-studio-api.md section 5).

    python3 -I tools/figure_assets/check_tape.py PATH/name.tape.json [--require-fa-tape]

Pure python, no bpy, no numpy, no import of the repository.  It parses the tape the way the Blender
adapter does (`fa_tape.read_tape`: format, channels, sha256 of the .bin, float32 little-endian body,
stride = 3 + 10 * joints + face, finite values, then the header keys `render_tape.py` / `fa_stage.py` read)
and additionally checks what a writer must get right and a lenient reader would hide: the exact top-level
key list in contract order, no duplicate keys, strict JSON (no NaN/Infinity), per-record key sets, event and
camera-cut frames inside the tape, fit_status keys equal to garments, unit-length rotation quaternions.

If `fa_tape.py` sits next to this script (the assets branch), the tape is also read with its real
`read_tape` and the two results are compared; `--require-fa-tape` makes its absence an error.

Exit status 0 = every check passed.  Output is `PASS ...` / `FAIL ...` lines and a final `CHECK_TAPE_FAILS n`.
"""
import array
import hashlib
import importlib.util
import json
import math
import os
import sys

FORMAT = "gibson.figure.tape.v1"
CHANNELS = ["root_t3", "local_t3", "local_r4", "local_s3"]
KEYS = [
    "format", "body_rev_id", "fit_id", "present_id", "asset_manifest_sha256", "controls", "garments",
    "fit_status", "materials", "style", "fps", "frame_count", "joints", "channels", "face", "events",
    "camera_cuts", "bin_sha256",
]
STYLE_KEYS = ["preset", "amplitude", "tempo", "settle_ms"]
EVENT_KEYS = ["frame", "kind", "label"]
CUT_KEYS = ["frame", "camera_id"]
HEX = set("0123456789abcdef")

FAILS = []


def check(ok, msg):
    print(("PASS " if ok else "FAIL ") + msg)
    if not ok:
        FAILS.append(msg)
    return ok


class _Dup(Exception):
    pass


def _pairs(pairs):
    seen = set()
    for k, _ in pairs:
        if k in seen:
            raise _Dup(k)
        seen.add(k)
    return _Ordered(pairs)


class _Ordered(dict):
    """A dict that remembers the key order the file had (dicts already do; this keeps it explicit)."""

    def __init__(self, pairs):
        super().__init__(pairs)
        self.order = [k for k, _ in pairs]


def _no_const(name):
    raise ValueError("non-JSON constant %s" % name)


def is_int(v):
    return isinstance(v, int) and not isinstance(v, bool)


def is_num(v):
    return isinstance(v, (int, float)) and not isinstance(v, bool) and math.isfinite(v)


def is_hex(s):
    return isinstance(s, str) and len(s) > 0 and set(s) <= HEX


def read_like_fa_tape(json_path):
    """The body of fa_tape.read_tape, reproduced so this file needs nothing else."""
    with open(json_path, "r", encoding="utf-8") as f:
        text = f.read()
    header = json.loads(text, object_pairs_hook=_pairs, parse_constant=_no_const)
    bin_path = json_path[:-len(".tape.json")] + ".tape.bin"
    with open(bin_path, "rb") as f:
        raw = f.read()
    return text, header, raw


def load_fa_tape(here):
    p = os.path.join(here, "fa_tape.py")
    if not os.path.isfile(p):
        return None
    sys.path.insert(0, here)
    spec = importlib.util.spec_from_file_location("fa_tape", p)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def main(argv):
    args = [a for a in argv[1:] if not a.startswith("--")]
    require_fa = "--require-fa-tape" in argv
    if len(args) != 1 or not args[0].endswith(".tape.json"):
        print("usage: check_tape.py PATH/name.tape.json [--require-fa-tape]")
        return 2
    path = args[0]
    try:
        text, h, raw = read_like_fa_tape(path)
    except _Dup as e:
        check(False, "header has a duplicate key %r" % (e.args[0],))
        print("CHECK_TAPE_FAILS %d" % len(FAILS))
        return 1
    except (OSError, ValueError) as e:
        check(False, "tape files are readable strict JSON + bin: %s" % e)
        print("CHECK_TAPE_FAILS %d" % len(FAILS))
        return 1

    check(isinstance(h, dict), "header is a JSON object")
    if not isinstance(h, dict):
        print("CHECK_TAPE_FAILS %d" % len(FAILS))
        return 1
    check(text.endswith("\n"), "header file ends with a newline")
    check(h.order == KEYS, "top-level keys are exactly the section 5 list in contract order%s" %
          ("" if h.order == KEYS else " (got %r)" % (h.order,)))
    check(h.get("format") == FORMAT, "format is %s" % FORMAT)
    check(h.get("channels") == CHANNELS, "channels are %s" % CHANNELS)
    for k in ("body_rev_id", "fit_id", "present_id"):
        check(isinstance(h.get(k), str), "%s is a string" % k)
    check(isinstance(h.get("asset_manifest_sha256"), str), "asset_manifest_sha256 is a string")

    joints, face = h.get("joints"), h.get("face")
    ok_names = (isinstance(joints, list) and all(isinstance(x, str) for x in joints) and len(set(joints)) == len(joints)
                and isinstance(face, list) and all(isinstance(x, str) for x in face) and len(set(face)) == len(face))
    check(ok_names, "joints and face are lists of distinct strings")
    check(is_num(h.get("fps")) and h["fps"] > 0, "fps is a positive finite number")
    check(is_int(h.get("frame_count")) and h["frame_count"] >= 1, "frame_count is an integer >= 1")
    if not (ok_names and is_num(h.get("fps")) and is_int(h.get("frame_count"))):
        print("CHECK_TAPE_FAILS %d" % len(FAILS))
        return 1

    ctl = h.get("controls")
    check(isinstance(ctl, dict) and all(is_num(v) for v in ctl.values()),
          "controls is an object of finite numbers (render_tape: dict(header['controls']))")
    gar, fs = h.get("garments"), h.get("fit_status")
    check(isinstance(gar, list) and all(isinstance(x, str) for x in gar), "garments is a list of ids")
    check(isinstance(fs, dict) and isinstance(gar, list) and sorted(fs) == sorted(gar),
          "fit_status keys equal garments (a missing verdict means not drawn)")
    check(isinstance(fs, dict) and all(isinstance(v, (str, dict)) for v in fs.values()),
          "fit_status values are strings or objects (fa_stage.verdict_of)")
    mats = h.get("materials")
    check(isinstance(mats, dict), "materials is an object (render_tape: .items())")
    st = h.get("style")
    check(isinstance(st, dict) and getattr(st, "order", None) == STYLE_KEYS, "style keys are %s in order" % STYLE_KEYS)
    if isinstance(st, dict):
        check(isinstance(st.get("preset"), str) and is_num(st.get("amplitude")) and is_num(st.get("tempo"))
              and is_int(st.get("settle_ms")), "style value types")

    n = h["frame_count"]
    ev, cuts = h.get("events"), h.get("camera_cuts")
    check(isinstance(ev, list) and all(getattr(e, "order", None) == EVENT_KEYS and is_int(e["frame"])
                                       and isinstance(e["kind"], str) and isinstance(e["label"], str) for e in ev),
          "every event is {frame, kind, label}")
    check(isinstance(ev, list) and all(0 <= e["frame"] < n for e in ev if isinstance(e, dict) and is_int(e.get("frame"))),
          "every event frame is inside [0, frame_count)")
    check(isinstance(cuts, list) and all(getattr(c, "order", None) == CUT_KEYS and is_int(c["frame"])
                                         and isinstance(c["camera_id"], str) for c in cuts),
          "every camera cut is {frame, camera_id}")
    check(isinstance(cuts, list) and all(0 <= c["frame"] < n for c in cuts if isinstance(c, dict) and is_int(c.get("frame"))),
          "every camera cut frame is inside [0, frame_count)")

    stride = 3 + 10 * len(joints) + len(face)
    check(len(raw) == 4 * stride * n, "bin is frame_count * (3 + 10 * joints + face) * 4 = %d bytes (got %d)" % (4 * stride * n, len(raw)))
    got = hashlib.sha256(raw).hexdigest()
    check(is_hex(h.get("bin_sha256")) and len(h["bin_sha256"]) == 64 and got == h["bin_sha256"],
          "bin_sha256 is the lowercase hex sha256 of the .bin")
    if len(raw) % 4 == 0:
        a = array.array("f")
        a.frombytes(raw)
        if sys.byteorder == "big":
            a.byteswap()
        check(len(a) == stride * n, "float count is stride * frame_count")
        check(all(math.isfinite(v) for v in a), "every body value is finite")
        worst = 0.0
        if len(a) == stride * n:
            for f in range(n):
                o = f * stride + 3
                for j in range(len(joints)):
                    q = a[o + 10 * j + 3:o + 10 * j + 7]
                    worst = max(worst, abs(math.sqrt(sum(x * x for x in q)) - 1.0))
        check(worst < 1e-3, "rotation quaternions are unit length (worst |q| - 1 = %.2e)" % worst)
    else:
        check(False, "bin size is a multiple of 4")

    here = os.path.dirname(os.path.abspath(__file__))
    ft = None
    try:
        ft = load_fa_tape(here)
    except Exception as e:  # fa_tape needs fa_common, bpy-free; a failure here is a real finding
        check(False, "fa_tape.py is present but does not load: %s" % e)
    if ft is None:
        if require_fa:
            check(False, "fa_tape.py is not next to this script (--require-fa-tape)")
        else:
            print("NOTE fa_tape.py not found next to this script; the cross-read with the adapter reader was skipped")
    else:
        try:
            t = ft.read_tape(path)
            check(t.frame_count == n and t.n_joints == len(joints) and t.n_face == len(face) and t.stride == stride,
                  "fa_tape.read_tape agrees on frame_count, joints, face and stride")
            root, js, fc = t.frame(n - 1)
            check(len(js) == len(joints) and len(fc) == len(face), "fa_tape frame() has one local per joint and one value per face channel")
        except Exception as e:
            check(False, "fa_tape.read_tape accepts the tape: %s" % e)

    print("CHECK_TAPE_FAILS %d" % len(FAILS))
    return 1 if FAILS else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
