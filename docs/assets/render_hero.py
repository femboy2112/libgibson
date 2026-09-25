#!/usr/bin/env python3
"""Capture a real libgibson_intro frame in a sized PTY and font-render it to PNG.

Pipeline: PTY (so the engine emits real truecolor ANSI) -> pyte (reconstruct the
exact final cell grid) -> Pillow (block glyphs as exact rects, text/Braille via
DejaVu Sans Mono). Deterministic: --at + --freeze + --deterministic.
"""
import argparse, os, pty, select, sys, time, fcntl, termios, struct, signal
import pyte
from PIL import Image, ImageDraw, ImageFont

BIN = "target/release/examples/libgibson_intro"
FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"

def capture(cols, rows, at, extra_env=None):
    argv = [BIN, f"--at={at}", "--freeze", "--deterministic",
            "--no-prologue", "--color=truecolor"]
    env = dict(os.environ)
    env["TERM"] = "xterm-256color"
    env["COLORTERM"] = "truecolor"
    if extra_env:
        env.update(extra_env)
    pid, fd = pty.fork()
    if pid == 0:  # child
        # set window size
        winsz = struct.pack("HHHH", rows, cols, 0, 0)
        fcntl.ioctl(sys.stdout.fileno(), termios.TIOCSWINSZ, winsz)
        os.execvpe(argv[0], argv, env)
        os._exit(127)
    # parent
    buf = bytearray()
    last = time.time()
    deadline = time.time() + 8.0
    while True:
        r, _, _ = select.select([fd], [], [], 0.3)
        now = time.time()
        if r:
            try:
                data = os.read(fd, 65536)
            except OSError:
                break
            if not data:
                break
            buf.extend(data)
            last = now
        else:
            # quiescent for 0.5s after we have data -> frame is fully painted
            if buf and now - last > 0.5:
                break
        if now > deadline:
            break
    # tear the child down
    try:
        os.write(fd, b"\x03")  # Ctrl-C
        time.sleep(0.1)
    except OSError:
        pass
    try:
        os.kill(pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    try:
        os.waitpid(pid, 0)
    except ChildProcessError:
        pass
    os.close(fd)
    return bytes(buf)

def to_rgb(val, default):
    if val == "default" or val is None:
        return default
    if isinstance(val, str) and len(val) == 6:
        try:
            return (int(val[0:2],16), int(val[2:4],16), int(val[4:6],16))
        except ValueError:
            pass
    named = {"black":(0,0,0),"red":(205,0,0),"green":(0,205,0),"brown":(205,205,0),
             "yellow":(255,255,0),"blue":(0,0,238),"magenta":(205,0,205),
             "cyan":(0,205,205),"white":(229,229,229)}
    return named.get(val, default)

DEF_FG=(205,210,215); DEF_BG=(0,0,0); CROP_BOTTOM=0

def render(screen, cols, rows, cw, ch, font_size, out):
    img = Image.new("RGB", (cols*cw, rows*ch), DEF_BG)
    d = ImageDraw.Draw(img)
    font = ImageFont.truetype(FONT, font_size)
    for y in range(rows):
        line = screen.buffer[y]
        for x in range(cols):
            c = line[x]
            ch_ = c.data or " "
            fg = to_rgb(c.fg, DEF_FG)
            bg = to_rgb(c.bg, DEF_BG)
            if c.reverse:
                fg, bg = bg, fg
            px, py = x*cw, y*ch
            # background
            d.rectangle([px, py, px+cw-1, py+ch-1], fill=bg)
            if ch_ == " " or ch_ == "\x00":
                continue
            # exact block glyphs as rectangles (seamless tiling)
            if ch_ == "█":  # full
                d.rectangle([px,py,px+cw-1,py+ch-1], fill=fg); continue
            if ch_ == "▀":  # upper half
                d.rectangle([px,py,px+cw-1,py+ch//2-1], fill=fg); continue
            if ch_ == "▄":  # lower half
                d.rectangle([px,py+ch//2,px+cw-1,py+ch-1], fill=fg); continue
            if ch_ == "▌":  # left half
                d.rectangle([px,py,px+cw//2-1,py+ch-1], fill=fg); continue
            if ch_ == "▐":  # right half
                d.rectangle([px+cw//2,py,px+cw-1,py+ch-1], fill=fg); continue
            if ch_ in "░▒▓":  # light/med/dark shade -> blend
                a = {"░":0.25,"▒":0.5,"▓":0.75}[ch_]
                col = tuple(int(bg[i]+(fg[i]-bg[i])*a) for i in range(3))
                d.rectangle([px,py,px+cw-1,py+ch-1], fill=col); continue
            lower8 = "▁▂▃▄▅▆▇█"
            if ch_ in lower8:
                frac = (lower8.index(ch_)+1)/8.0
                top = py + int(ch*(1-frac))
                d.rectangle([px,top,px+cw-1,py+ch-1], fill=fg); continue
            # text / Braille / everything else via the font
            try:
                bb = d.textbbox((0,0), ch_, font=font)
                gw = bb[2]-bb[0]; gh = bb[3]-bb[1]
                ox = px + (cw-gw)//2 - bb[0]
                oy = py + (ch-gh)//2 - bb[1]
                d.text((ox,oy), ch_, font=font, fill=fg)
            except Exception:
                pass
    if CROP_BOTTOM:
        w,h = img.size
        img = img.crop((0,0,w,h-CROP_BOTTOM*ch))
    img.save(out)
    return img.size

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--at", default="71")
    ap.add_argument("--width", type=int, default=120)
    ap.add_argument("--height", type=int, default=40)
    ap.add_argument("--cw", type=int, default=12)
    ap.add_argument("--ch", type=int, default=24)
    ap.add_argument("--font-size", type=int, default=22)
    ap.add_argument("--out", default="hero.png")
    ap.add_argument("--raw-out", default=None)
    ap.add_argument("--crop-bottom", type=int, default=0)
    a = ap.parse_args()
    global CROP_BOTTOM
    CROP_BOTTOM = a.crop_bottom
    data = capture(a.width, a.height, a.at)
    if a.raw_out:
        open(a.raw_out,"wb").write(data)
    scr = pyte.Screen(a.width, a.height)
    st = pyte.Stream(scr)
    st.feed(data.decode("utf-8","replace"))
    size = render(scr, a.width, a.height, a.cw, a.ch, a.font_size, a.out)
    nonblank = sum(1 for y in range(a.height) for x in range(a.width)
                   if (scr.buffer[y][x].data or " ") != " ")
    print(f"captured {len(data)} bytes; nonblank cells={nonblank}; image={size} -> {a.out}")

if __name__ == "__main__":
    main()
