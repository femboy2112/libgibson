#!/usr/bin/env python3
"""Pure-python render self-checks (no bpy): contact, contact-shadow pixels, framing, hair/eye clearance, contrast.

    python3 -I tools/figure_assets/fa_checks.py --selftest

Each check returns a dict {"name", "ok", ...numbers...}.  The thresholds are FIRST GUESSES (never calibrated on
a render yet): the adapter prints them next to the measured values so the first lease can tune them, and the
`bad_bounds` mutant of render_tape.py is the negative control that proves the contact and shadow checks can fail.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)
import fa_png  # noqa: E402

CONTACT_TOL_M = 0.005          # min foot z within 5 mm of the ground (rest pose), ruling #102
SHADOW_MIN_COVERAGE = 0.15     # fraction of visible ground pixels near the feet that must be shadowed
SHADOW_DARK_RATIO = 0.80       # a ground pixel is "shadowed" below this fraction of the reference ground luminance
FRAME_MIN_FRACTION = 0.60      # the body must span at least this fraction of the frame height (ruling #102)
FRAME_MAX_FRACTION = 0.98
CONTRAST_MIN_RATIO = 1.8       # brow/lash vs skin luminance ratio (legibility proxy)


def contact_check(foot_min_z, ground_z, tol=CONTACT_TOL_M):
    gap = foot_min_z - ground_z
    return {"name": "contact", "ok": abs(gap) <= tol, "gap_m": gap, "tol_m": tol, "foot_min_z": foot_min_z, "ground_z": ground_z}


def framing_check(ndc_points, min_frac=FRAME_MIN_FRACTION, max_frac=FRAME_MAX_FRACTION):
    """ndc_points: [(x, y)] in camera NDC (0..1, y up) of the body vertices.  The figure must be inside the frame and span
    min_frac..max_frac of its height."""
    ys = [p[1] for p in ndc_points]
    xs = [p[0] for p in ndc_points]
    lo, hi = min(ys), max(ys)
    frac = hi - lo
    inside = lo >= 0.0 and hi <= 1.0 and min(xs) >= 0.0 and max(xs) <= 1.0
    return {"name": "framing", "ok": bool(inside and min_frac <= frac <= max_frac), "height_fraction": frac, "inside_frame": bool(inside),
            "min": min_frac, "max": max_frac, "x_range": [min(xs), max(xs)], "y_range": [lo, hi]}


def _median(v):
    s = sorted(v)
    n = len(s)
    return s[n // 2] if n % 2 else 0.5 * (s[n // 2 - 1] + s[n // 2])


def shadow_check(lum, mask, inner, outer, min_cov=SHADOW_MIN_COVERAGE, dark_ratio=SHADOW_DARK_RATIO):
    """Contact-shadow pixel test.

    lum, mask: per-row lists of luminance (0..255) of the beauty check render and of the figure mask render
    (figure = white > 128, everything else black).  inner / outer: pixel rects (x0, y0, x1, y1), x1/y1 exclusive;
    inner is the ground just around the feet, outer is a larger rect around it.  Reference = median luminance of the
    unmasked ground pixels in outer minus inner.  Shadowed = unmasked inner pixels below dark_ratio * reference.
    Coverage = shadowed / unmasked inner pixels."""
    h = len(lum)
    w = len(lum[0])

    def clip(r):
        return (max(0, r[0]), max(0, r[1]), min(w, r[2]), min(h, r[3]))

    ix0, iy0, ix1, iy1 = clip(inner)
    ox0, oy0, ox1, oy1 = clip(outer)
    ref = []
    for y in range(oy0, oy1):
        for x in range(ox0, ox1):
            if ix0 <= x < ix1 and iy0 <= y < iy1:
                continue
            if mask[y][x] <= 128:
                ref.append(lum[y][x])
    ins = []
    for y in range(iy0, iy1):
        for x in range(ix0, ix1):
            if mask[y][x] <= 128:
                ins.append(lum[y][x])
    if len(ref) < 50 or len(ins) < 50:
        return {"name": "shadow", "ok": False, "reason": "too few unmasked ground pixels (ref=%d inner=%d)" % (len(ref), len(ins)),
                "coverage": 0.0, "n_ref": len(ref), "n_inner": len(ins)}
    r = _median(ref)
    dark = sum(1 for v in ins if v < dark_ratio * r)
    cov = dark / float(len(ins))
    return {"name": "shadow", "ok": cov >= min_cov, "coverage": cov, "min_coverage": min_cov, "ref_luminance": r,
            "dark_ratio": dark_ratio, "n_inner": len(ins), "n_ref": len(ref)}


def shadow_check_files(beauty_png, mask_png, inner, outer, **kw):
    w, h, c, rows = fa_png.read(beauty_png)
    lum = fa_png.luminance_rows(w, h, c, rows)
    w2, h2, c2, rows2 = fa_png.read(mask_png)
    if (w, h) != (w2, h2):
        raise ValueError("beauty and mask PNG sizes differ")
    mk = fa_png.luminance_rows(w2, h2, c2, rows2)
    return shadow_check(lum, mk, inner, outer, **kw)


def ground_clip_check(lum, mask, rect, max_median=0.90, max_clipped_frac=0.25):
    """Exposure check: the ground pixels AWAY from the figure (unmasked, inside `rect`) must not be blown out.
    median luminance < max_median * 255 and fewer than max_clipped_frac of the pixels >= 250 (a white ground hides every shadow)."""
    h, w = len(lum), len(lum[0])
    x0, y0, x1, y1 = max(0, rect[0]), max(0, rect[1]), min(w, rect[2]), min(h, rect[3])
    v = [lum[y][x] for y in range(y0, y1) for x in range(x0, x1) if mask[y][x] <= 128]
    if len(v) < 50:
        return {"name": "ground_not_clipped", "ok": False, "reason": "too few ground pixels (%d)" % len(v), "n": len(v)}
    med = _median(v) / 255.0
    clipped = sum(1 for p in v if p >= 250.0) / float(len(v))
    return {"name": "ground_not_clipped", "ok": med < max_median and clipped < max_clipped_frac, "median": med, "clipped_fraction": clipped,
            "max_median": max_median, "max_clipped_fraction": max_clipped_frac, "n": len(v)}


def ground_clip_check_files(beauty_png, mask_png, rect, **kw):
    w, h, c, rows = fa_png.read(beauty_png)
    lum = fa_png.luminance_rows(w, h, c, rows)
    w2, h2, c2, rows2 = fa_png.read(mask_png)
    return ground_clip_check(lum, fa_png.luminance_rows(w2, h2, c2, rows2), rect, **kw)


def clearance_check(blocked, total, name="eye_clearance"):
    """blocked: number of probe rays whose first hit is NOT the intended surface (e.g. hair in front of an eye)."""
    return {"name": name, "ok": blocked == 0, "blocked": blocked, "rays": total}


def visibility_check(visible, total, min_frac=0.6, name="visibility"):
    f = visible / float(total) if total else 0.0
    return {"name": name, "ok": total > 0 and f >= min_frac, "visible": visible, "rays": total, "fraction": f, "min": min_frac}


def contrast_check(lum_a, lum_b, min_ratio=CONTRAST_MIN_RATIO, name="contrast"):
    hi, lo = max(lum_a, lum_b), min(lum_a, lum_b)
    ratio = (hi + 0.02) / (lo + 0.02)
    return {"name": name, "ok": ratio >= min_ratio, "ratio": ratio, "min": min_ratio}


# --------------------------------------------------------------------------- selftest

def _synthetic(w, h, shadow):
    """Ground luminance 150; figure (feet) as a white mask block; optionally a dark halo around/behind the feet."""
    lum = [[150.0] * w for _ in range(h)]
    mask = [[0.0] * w for _ in range(h)]
    for y in range(40, 60):
        for x in range(45, 55):
            mask[y][x] = 255.0
            lum[y][x] = 30.0
    if shadow:
        for y in range(50, 70):
            for x in range(40, 66):
                if mask[y][x] <= 128:
                    lum[y][x] = 70.0
    return lum, mask


def _selftest():
    fails = []

    def check(ok, msg):
        print(("PASS " if ok else "FAIL ") + msg)
        if not ok:
            fails.append(msg)

    inner, outer = (35, 35, 70, 75), (5, 5, 95, 95)
    lum, mask = _synthetic(100, 100, True)
    r = shadow_check(lum, mask, inner, outer)
    check(r["ok"] and r["coverage"] > 0.3, "shadow present -> ok (coverage %.2f)" % r["coverage"])
    lum, mask = _synthetic(100, 100, False)
    r = shadow_check(lum, mask, inner, outer)
    check((not r["ok"]) and r["coverage"] == 0.0, "no shadow (floating figure) -> FAIL (coverage %.2f)" % r["coverage"])
    # the dark feet themselves must not count as shadow
    lum, mask = _synthetic(100, 100, False)
    r = shadow_check(lum, mask, (45, 40, 55, 60), outer)
    check(r["n_inner"] < 50 and not r["ok"], "a rect that is all figure has too few ground pixels -> FAIL, never a pass")
    # PNG round trip drives the same decision through files
    import tempfile
    d = tempfile.mkdtemp(prefix="fa_checks_")
    for shadow in (True, False):
        lum, mask = _synthetic(100, 100, shadow)
        for nm, plane in (("b", lum), ("m", mask)):
            fa_png.write(os.path.join(d, "%s%d.png" % (nm, shadow)), 100, 100, 3, [bytearray(int(v) for v in row for _ in range(3)) for row in plane])
        rr = shadow_check_files(os.path.join(d, "b%d.png" % shadow), os.path.join(d, "m%d.png" % shadow), inner, outer)
        check(rr["ok"] == shadow, "shadow_check_files shadow=%s -> ok=%s" % (shadow, rr["ok"]))
    for fn in os.listdir(d):
        os.remove(os.path.join(d, fn))
    os.rmdir(d)
    lum, mask = _synthetic(100, 100, True)
    check(ground_clip_check(lum, mask, outer)["ok"], "ground at luminance 150 is not clipped")
    white = [[255.0] * 100 for _ in range(100)]
    check(not ground_clip_check(white, mask, outer)["ok"], "a pure-white ground FAILS ground_not_clipped (the viewed blow-out)")
    check(contact_check(0.002, 0.0)["ok"], "contact 2 mm ok")
    check(not contact_check(0.40, 0.0)["ok"], "contact 0.4 m (the viewed floating bug) FAILS")
    check(not contact_check(0.0, 0.0057)["ok"], "contact 5.7 mm FAILS (tol 5 mm)")
    good = [(0.45, 0.08), (0.55, 0.90), (0.5, 0.5)]
    check(framing_check(good)["ok"], "framing 82% of frame height ok")
    small = [(0.45, 0.25), (0.55, 0.75)]
    check(not framing_check(small)["ok"], "framing 50% of frame height FAILS (the viewed small-figure bug)")
    clipped = [(0.45, -0.02), (0.55, 0.9)]
    check(not framing_check(clipped)["ok"], "framing with feet outside the frame FAILS")
    check(not clearance_check(2, 6)["ok"] and clearance_check(0, 6)["ok"], "eye clearance: any blocked ray fails")
    check(contrast_check(0.05, 0.40)["ok"] and not contrast_check(0.30, 0.40)["ok"], "brow/skin contrast ratio")
    print("SELFTEST_FAILS %d" % len(fails))
    return 1 if fails else 0


if __name__ == "__main__":
    if "--selftest" in sys.argv:
        sys.exit(_selftest())
    print(__doc__)
