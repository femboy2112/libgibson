#!/usr/bin/env python3
"""M5-A visual checkpoint orchestrator: real xterm screenshots of examples/stage_motion_kit.

Needs on PATH: xterm, xdotool, ImageMagick `import`, ffmpeg, Pillow. Runs xterm on an
existing X display (see run_kit.sh, which starts a private Xvfb). Deterministic: fixed
geometry/font/settle and fixed example arguments; the manifest records PNG sha256s and a
repeat-capture check.

  make_kit.py --example-bin PATH --out DIR --display :97 [--only=stills|strip|all]
"""
import argparse, hashlib, json, os, shutil, signal, subprocess, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
GEOM, FONT, FS = "80x30+0+0", "DejaVu Sans Mono", "12"
SETTLE = 1.2          # seconds from xterm launch to capture
HOLD_MS = 3000        # the example keeps the frame this long (> SETTLE)
SEED, M = 0x5EED, (1 << 64) - 1
SEQ_STARTS, SEQ_TOTAL = [0, 300, 420, 920], 1120  # mirrors sequence() in the example
STRIP_STEP, TILE_W = 50, 240
LIVE = []             # xterm process groups to kill on any exit


def mix(x):
    z = (x + 0x9E3779B97F4A7C15) & M
    z ^= z >> 30; z = (z * 0xBF58476D1CE4E5B9) & M
    z ^= z >> 27; z = (z * 0x94D049BB133111EB) & M
    return z ^ (z >> 31)


def cue_ms(k):  # CueSchedule::new(SEED, 1, 300ms, 1200ms, 500ms).nth(k), whole ms
    h = mix(mix(mix(SEED) ^ 1) ^ k)
    return (300_000_000 + k * 1_200_000_000 + h % 700_000_001) // 1_000_000


def expect_frame(at):
    if at >= SEQ_TOTAL:
        return 0
    return max(i for i, s in enumerate(SEQ_STARTS) if s <= at)


def killall(*_):
    for p in LIVE:
        try:
            os.killpg(p.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
    LIVE.clear()


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def run1(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, **kw)


def shot(args, name, example_args, png, hold=HOLD_MS, settle=SETTLE, after_exit=False):
    """Launch xterm running the example, capture only its window, then reap it."""
    env = dict(os.environ, TERM="xterm-256color", COLORTERM="truecolor", DISPLAY=args.display)
    err = png[:-4] + ".stderr"
    exe = [args.example_bin] + example_args + [f"--hold-ms={hold}"]
    quoted = " ".join("'%s'" % a for a in exe)
    inner = f"{quoted} 2>'{err}'"
    if after_exit:
        inner += "; echo RESTORED-OK; sleep 10"
    xcmd = ["xterm", "-display", args.display, "-geometry", GEOM, "-fa", FONT, "-fs", FS,
            "-bg", "black", "-fg", "white", "-e", "sh", "-c", inner]
    p = subprocess.Popen(xcmd, env=env, start_new_session=True,
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    LIVE.append(p)
    try:
        time.sleep(settle)
        wid = None
        for flags in (["--onlyvisible"], []):
            r = run1(["xdotool", "search", *flags, "--pid", str(p.pid)], env=env)
            ids = r.stdout.split()
            if ids:
                wid = ids[-1]
                break
        if wid is None:
            raise RuntimeError(f"no xterm window for pid {p.pid} ({name})")
        c = run1(["import", "-display", args.display, "-window", wid, png], env=env)
        if c.returncode != 0:
            raise RuntimeError(f"import failed: {c.stderr}")
    finally:
        try:
            os.killpg(p.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        p.wait()
        if p in LIVE:
            LIVE.remove(p)
    stderr = open(err).read() if os.path.exists(err) else ""
    js = []
    for line in stderr.splitlines():
        if line.startswith("{"):
            try:
                js.append(json.loads(line))
            except ValueError:
                pass
    return {"name": name, "png": os.path.relpath(png, args.out), "command": " ".join(exe),
            "xterm": " ".join(xcmd[:11]), "stderr": stderr.strip().splitlines(),
            "json": js, "sha256": sha(png), "bytes": os.path.getsize(png)}


def stills(args, man):
    d = os.path.join(args.out, "stills"); os.makedirs(d, exist_ok=True)
    out = []

    def go(name, extra, **kw):
        r = shot(args, name, extra, os.path.join(d, name + ".png"), **kw)
        out.append(r); print("shot", name, r["sha256"][:12], flush=True)
        return r

    for at, tag in [(0, "start-f0"), (300, "start-f1"), (420, "start-f2"), (920, "start-f3"),
                    (650, "mid-hold-f2"), (1120, "past-total"), (1500, "late")]:
        r = go(f"g1-{at:04d}-{tag}", ["--scenario=g1", f"--at-ms={at}"])
        r["expect_frame"] = expect_frame(at)
        r["frame_ok"] = bool(r["json"]) and r["json"][0].get("frame") == expect_frame(at)
    r = go("g1-reduced-0420", ["--scenario=g1-reduced", "--at-ms=420"])
    r["expect_frame"] = 0; r["frame_ok"] = bool(r["json"]) and r["json"][0].get("frame") == 0
    go("g2-slots", ["--scenario=g2-slots"])
    go("g2-unfilled", ["--scenario=g2-unfilled"])
    cues = [cue_ms(k) for k in range(8)]
    for at, tag in [(cues[2], "at-cue2-blink"), (cues[2] + 400, "cue2-plus-400-open"), (0, "origin")]:
        r = go(f"g3-{at:05d}-{tag}", ["--scenario=g3-timeline", f"--at-ms={at}"])
        got = [j["cues_ms"] for j in r["json"] if "cues_ms" in j]
        r["cues_match_python"] = bool(got) and got[0] == cues
    go("g4-hidden", ["--scenario=g4-hidden", "--at-ms=500"])
    go("after-exit", ["--scenario=g2-slots"], hold=300, settle=2.0, after_exit=True)
    # Determinism: capture one shot a second time.
    again = shot(args, "determinism-repeat", ["--scenario=g1", "--at-ms=420"],
                 os.path.join(d, "_repeat.png"))
    first = next(s for s in out if s["name"].startswith("g1-0420"))
    man["determinism"] = {"shot": first["name"], "sha_a": first["sha256"], "sha_b": again["sha256"],
                          "identical": first["sha256"] == again["sha256"]}
    os.remove(os.path.join(d, "_repeat.png"))
    man["stills"] = out
    man["still_png_total_bytes"] = sum(s["bytes"] for s in out)
    man["cues_ms_python"] = cues


def strip(args, man):
    from PIL import Image, ImageDraw
    d = os.path.join(args.out, "strip"); os.makedirs(d, exist_ok=True)
    times = list(range(0, SEQ_TOTAL + 200 + 1, STRIP_STEP))
    rows, frames = [], []
    for i, at in enumerate(times):
        png = os.path.join(d, f"strip_{i:03d}.png")
        r = shot(args, f"strip-{at}", ["--scenario=g1", f"--at-ms={at}"], png, hold=2500, settle=1.0)
        r["at_ms"] = at; r["expect_frame"] = expect_frame(at)
        r["frame_ok"] = bool(r["json"]) and r["json"][0].get("frame") == expect_frame(at)
        rows.append({k: r[k] for k in ("png", "at_ms", "expect_frame", "frame_ok", "sha256", "command")})
        frames.append(png)
        os.remove(png[:-4] + ".stderr")
        print("strip", at, flush=True)
    ims = [Image.open(f).convert("RGB") for f in frames]
    th = round(ims[0].height * TILE_W / ims[0].width)
    cols = 6; lab = 16
    sheet = Image.new("RGB", (cols * TILE_W, ((len(ims) + cols - 1) // cols) * (th + lab)), (0, 0, 0))
    dr = ImageDraw.Draw(sheet)
    for i, im in enumerate(ims):
        x, y = (i % cols) * TILE_W, (i // cols) * (th + lab)
        sheet.paste(im.resize((TILE_W, th)), (x, y + lab))
        dr.text((x + 3, y + 2), f"{times[i]} ms  (exp f{expect_frame(times[i])})", fill=(255, 255, 0))
    sheet_p = os.path.join(args.out, "g1_contact_sheet.png"); sheet.save(sheet_p)
    picks = {"first": 0, "mid": len(frames) // 2, "last": len(frames) - 1}
    for k, i in picks.items():
        shutil.copy(frames[i], os.path.join(args.out, f"g1_strip_{k}.png"))
    mp4 = os.path.join(args.out, "g1_strip.mp4")
    f = run1(["ffmpeg", "-y", "-loglevel", "error", "-framerate", "20", "-i",
              os.path.join(d, "strip_%03d.png"), "-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2",
              "-pix_fmt", "yuv420p", "-c:v", "libx264", mp4])
    man["strip"] = {"fps": 20, "step_ms": STRIP_STEP, "times_ms": times, "shots": rows,
                    "contact_sheet": os.path.basename(sheet_p), "contact_sheet_sha256": sha(sheet_p),
                    "mp4": os.path.basename(mp4) if f.returncode == 0 else None,
                    "ffmpeg_stderr": f.stderr.strip()[:500],
                    "strip_png_total_bytes": sum(os.path.getsize(p) for p in frames)}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--example-bin", required=True)
    ap.add_argument("--out", default=os.path.join(HERE, "out"))
    ap.add_argument("--display", required=True)
    ap.add_argument("--only", choices=["stills", "strip", "all"], default="all")
    args = ap.parse_args()
    args.example_bin = os.path.abspath(args.example_bin)
    os.makedirs(args.out, exist_ok=True)
    mpath = os.path.join(args.out, "manifest.json")
    man = json.load(open(mpath)) if os.path.exists(mpath) and args.only != "all" else {}
    first = lambda c: (run1(c).stdout or run1(c).stderr).splitlines()[:1]
    man.update({"git_sha": run1(["git", "rev-parse", "HEAD"], cwd=REPO).stdout.strip(),
                "seed": SEED, "cue": {"cue": 1, "origin_ms": 300, "period_ms": 1200, "min_gap_ms": 500},
                "seq_holds_ms": [300, 120, 500, 200], "seq_starts_ms": SEQ_STARTS,
                "xterm": {"geometry": GEOM, "font": FONT, "size": FS, "settle_s": SETTLE,
                          "hold_ms": HOLD_MS, "term": "xterm-256color", "colorterm": "truecolor"},
                "tools": {"xterm": first(["xterm", "-version"]), "ffmpeg": first(["ffmpeg", "-version"]),
                          "import": first(["import", "-version"])}})
    signal.signal(signal.SIGTERM, lambda *a: (killall(), sys.exit(143)))
    t0 = time.time()
    try:
        if args.only in ("stills", "all"):
            stills(args, man)
        if args.only in ("strip", "all"):
            strip(args, man)
    finally:
        killall()
        man["wall_seconds_last_run"] = round(time.time() - t0, 1)
        total = man.get("still_png_total_bytes", 0)
        man["still_png_total_kb"] = round(total / 1024, 1)
        json.dump(man, open(mpath, "w"), indent=1, sort_keys=True)
    print("manifest", mpath)


if __name__ == "__main__":
    main()
