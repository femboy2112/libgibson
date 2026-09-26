#!/usr/bin/env python3
"""Font-render actual ui_showcase headless ANSI output; no UI is mocked here.

Requires Pillow, pyte and DejaVu Sans Mono. Build the example first. This is a
documentation capture tool, not part of LibGibson's rendering/runtime path.
"""
import argparse
from collections import namedtuple
from pathlib import Path
import subprocess

import pyte
from PIL import Image, ImageDraw, ImageFont


FONT = Path("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf")
SKINS = ("vapor95", "black-ice", "swiss-signal")
Char = namedtuple("CaptureChar", (*pyte.screens.Char._fields, "dim"))


class CaptureScreen(pyte.Screen):
    """Retain SGR 2 (faint), which base pyte does not model."""

    def reset(self):
        super().reset()
        self.cursor.attrs = self.default_char

    @property
    def default_char(self):
        return Char(*super().default_char, False)

    def select_graphic_rendition(self, *attrs):
        dim = getattr(self.cursor.attrs, "dim", False)
        super().select_graphic_rendition(*attrs)
        if not hasattr(self.cursor.attrs, "dim"):
            self.cursor.attrs = Char(*self.cursor.attrs, dim)
        i = 0
        while i < len(attrs):
            attr = attrs[i]
            if attr in (0, 22):
                dim = False
            elif attr == 2:
                dim = True
            elif attr in (38, 48) and i + 1 < len(attrs):
                i += 4 if attrs[i + 1] == 2 else 2
            i += 1
        self.cursor.attrs = self.cursor.attrs._replace(dim=dim if attrs else False)


def rgb(value, default):
    names = {
        "black": (0, 0, 0), "red": (205, 49, 49),
        "green": (13, 188, 121), "brown": (229, 229, 16),
        "blue": (36, 114, 200), "magenta": (188, 63, 188),
        "cyan": (17, 168, 205), "white": (229, 229, 229),
        "brightblack": (102, 102, 102), "brightred": (241, 76, 76),
        "brightgreen": (35, 209, 139), "brightbrown": (245, 245, 67),
        "brightblue": (59, 142, 234), "brightmagenta": (214, 112, 214),
        "brightcyan": (41, 184, 219), "brightwhite": (255, 255, 255),
    }
    if value in names:
        return names[value]
    if isinstance(value, str) and len(value) == 6:
        try:
            return tuple(int(value[i:i + 2], 16) for i in (0, 2, 4))
        except ValueError:
            pass
    return default


def colors(cell):
    fg, bg = rgb(cell.fg, (220, 224, 229)), rgb(cell.bg, (12, 15, 20))
    if cell.reverse:
        fg, bg = bg, fg
    if cell.dim:
        fg = tuple((a + b) // 2 for a, b in zip(fg, bg))
    return fg, bg


def render(data, width, height, cw=10, ch=20):
    screen = CaptureScreen(width, height)
    pyte.Stream(screen).feed(data.decode("utf-8", "strict"))
    canvas = Image.new("RGB", (width * cw, height * ch))
    draw = ImageDraw.Draw(canvas)
    fonts = [ImageFont.truetype(str(FONT), ch - 4),
             ImageFont.truetype(str(FONT.with_name("DejaVuSansMono-Bold.ttf")), ch - 4)]
    # Backgrounds first so a continuation cell cannot erase its wide lead glyph.
    for y in range(height):
        for x in range(width):
            _, bg = colors(screen.buffer[y][x])
            draw.rectangle((x * cw, y * ch, (x + 1) * cw - 1, (y + 1) * ch - 1), fill=bg)
    for y in range(height):
        for x in range(width):
            cell = screen.buffer[y][x]
            glyph = cell.data
            if not glyph or glyph == " ":
                continue
            fg, bg = colors(cell)
            px, py = x * cw, y * ch
            if len(glyph) == 1 and 0x2800 <= ord(glyph) <= 0x28ff:
                # Braille is a 2x4 bit grid; draw its actual dots because the
                # selected text font may lack this Unicode block.
                mask = ord(glyph) - 0x2800
                for bit, (dx, dy) in enumerate(((0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (1, 2), (0, 3), (1, 3))):
                    if mask & (1 << bit):
                        dot_x = px + cw * (1 + 2 * dx) // 4
                        dot_y = py + ch * (1 + 2 * dy) // 8
                        draw.ellipse((dot_x - 1, dot_y - 1, dot_x + 1, dot_y + 1), fill=fg)
            elif glyph in "▁▂▃▄▅▆▇█":
                top = py + ch - round(ch * ("▁▂▃▄▅▆▇█".index(glyph) + 1) / 8)
                draw.rectangle((px, top, px + cw - 1, py + ch - 1), fill=fg)
            elif glyph in "▀▌▐":
                bounds = {"▀": (px, py, px + cw - 1, py + ch // 2 - 1),
                          "▌": (px, py, px + cw // 2 - 1, py + ch - 1),
                          "▐": (px + cw // 2, py, px + cw - 1, py + ch - 1)}
                draw.rectangle(bounds[glyph], fill=fg)
            elif glyph in "░▒▓":
                coverage = {"░": 1, "▒": 2, "▓": 3}[glyph]
                for dy in range(ch):
                    for dx in range(cw):
                        if ((dx & 1) + 2 * (dy & 1)) < coverage:
                            draw.point((px + dx, py + dy), fill=fg)
            else:
                draw.text((px, py + ch - 5), glyph, font=fonts[int(cell.bold)], fill=fg, anchor="ls")
            if cell.underscore:
                draw.line((px, py + ch - 2, px + cw - 1, py + ch - 2), fill=fg)
            if cell.strikethrough:
                draw.line((px, py + ch // 2, px + cw - 1, py + ch // 2), fill=fg)
    return canvas


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default="target/debug/examples/ui_showcase")
    parser.add_argument("--skin", choices=SKINS, action="append")
    parser.add_argument("--width", type=int, default=120)
    parser.add_argument("--height", type=int, default=40)
    parser.add_argument("--depth", choices=("truecolor", "ansi256", "ansi16", "mono"), default="truecolor")
    parser.add_argument("--transition", default="settled")
    parser.add_argument("--at-ms", type=int, default=1000)
    parser.add_argument("--out", type=Path, default=Path("docs/assets/ui-skins-truecolor.png"))
    parser.add_argument("--raw-dir", type=Path)
    args = parser.parse_args()
    shots = []
    for skin in args.skin or SKINS:
        command = [args.binary, "--dump-ansi", "--fullscreen", f"--skin={skin}",
                   f"--width={args.width}", f"--height={args.height}", f"--{args.depth}",
                   f"--transition={args.transition}", f"--at-ms={args.at_ms}"]
        data = subprocess.run(command, check=True, capture_output=True, timeout=30).stdout
        if args.raw_dir:
            args.raw_dir.mkdir(parents=True, exist_ok=True)
            (args.raw_dir / f"{skin}-{args.width}x{args.height}-{args.depth}-{args.transition}-{args.at_ms}.ansi").write_bytes(data)
        shots.append((skin, render(data, args.width, args.height)))
    gap, header = 16, 36
    width = sum(shot.width for _, shot in shots) + gap * (len(shots) + 1)
    result = Image.new("RGB", (width, shots[0][1].height + header + gap), (23, 26, 32))
    draw = ImageDraw.Draw(result)
    label_font = ImageFont.truetype(str(FONT), 15)
    left = gap
    for skin, shot in shots:
        draw.text((left, 9), f"{skin.upper()} / {args.width} x {args.height} / {args.depth}", font=label_font, fill=(228, 232, 240))
        result.paste(shot, (left, header))
        left += shot.width + gap
    args.out.parent.mkdir(parents=True, exist_ok=True)
    result.save(args.out)
    print(f"{args.out}: {len(shots)} actual headless ANSI captures, {result.size}")


if __name__ == "__main__":
    main()
