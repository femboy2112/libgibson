#!/usr/bin/env python3
"""Compare an ANSI capture's terminal cells with the original RGB samples."""
from pathlib import Path
import re
import sys
import pyte

BAYER = ((0, 4), (6, 2), (1, 5), (7, 3))
BITS = ((1, 8), (2, 16), (4, 32), (64, 128))
NAMED = {'black': '000000', 'red': 'cd0000', 'green': '00cd00',
         'brown': 'cdcd00', 'blue': '0000ee', 'magenta': 'cd00cd',
         'cyan': '00cdcd', 'white': 'e5e5e5'}


def luminance(color):
    color = NAMED.get(color, color)
    if len(color) != 6:
        raise ValueError(f'unsupported color for mono contrast: {color!r}')
    try:
        r, g, b = (int(color[i:i+2], 16) for i in (0, 2, 4))
    except ValueError as e:
        raise ValueError(f'unsupported color for mono contrast: {color!r}') from e
    return (54*r + 183*g + 19*b)//256


def validate_sgr(data):
    # Pyte does not model every terminal SGR (notably conceal). Fail closed on
    # SGR we did not calibrate instead of treating an invisible cell as visible.
    for match in re.finditer(rb'\x1b\[([0-?]*[ -/]*)([@-~])', data):
        if match.group(2) != b'm':
            continue
        body = match.group(1)
        if not re.fullmatch(rb'[0-9;]*', body):
            raise ValueError('unsupported SGR syntax')
        values = [int(n or b'0') for n in body.split(b';')]
        i = 0
        while i < len(values):
            n = values[i]
            if n in (38, 48):
                if i + 1 >= len(values) or values[i+1] not in (2, 5):
                    raise ValueError('unsupported extended color')
                count = 3 if values[i+1] == 2 else 1
                if i + 2 + count > len(values) or any(x > 255 for x in values[i+2:i+2+count]):
                    raise ValueError('invalid extended color')
                i += 2 + count
            elif n in (0, 7, 27, 39, 49) or 30 <= n <= 37 or 40 <= n <= 47 or 90 <= n <= 97 or 100 <= n <= 107:
                i += 1
            else:
                raise ValueError(f'unsupported SGR {n}')


def main():
    if len(sys.argv) != 4 or sys.argv[3] not in ('half', 'mono'):
        raise SystemExit('usage: screen_oracle.py INPUT.rgb CAPTURE.ansi half|mono')
    raw = Path(sys.argv[1]).read_bytes()
    if len(raw) != 9600:
        raise ValueError('expected exactly 9600 RGB bytes')
    capture = Path(sys.argv[2]).read_bytes()
    validate_sgr(capture)
    screen = pyte.Screen(80, 20)
    pyte.Stream(screen).feed(capture.decode('utf-8'))

    def pixel(x, y):
        i = (y * 80 + x) * 3
        return tuple(raw[i:i+3])

    for y in range(20):
        for x in range(80):
            cell = screen.buffer[y][x]
            if cell.bold or cell.italics or cell.underscore or cell.strikethrough or cell.blink:
                raise ValueError(f'unsupported text decoration at {(x,y)}')
            if sys.argv[3] == 'half':
                if cell.data == '▀':
                    got = (cell.bg, cell.fg) if cell.reverse else (cell.fg, cell.bg)
                elif cell.data == ' ':  # visual background fills both half samples
                    bg = cell.fg if cell.reverse else cell.bg
                    got = (bg, bg)
                else:
                    raise ValueError(f'unexpected half-block glyph at {(x,y)}: {cell.data!r}')
                want = tuple(bytes(pixel(x, 2*y+dy)).hex() for dy in (0, 1))
                if got != want:
                    raise ValueError(f'pixel mismatch at {(x,y)}: got {got}, expected {want}')
            else:
                if cell.reverse:
                    raise ValueError(f'reverse video changes mono density at {(x,y)}')
                if (cell.fg, cell.bg) != ('default', 'default'):
                    # A single default color has unknown luminance. For both
                    # explicit colors require a bright dot on a dark ground.
                    if 'default' in (cell.fg, cell.bg) or luminance(cell.fg) - luminance(cell.bg) < 80:
                        raise ValueError(f'insufficient or unknown mono contrast at {(x,y)}')
                bits = 0
                for dy in range(4):
                    r, g, b = pixel(x, 2*y+dy//2)
                    lum = (54*r + 183*g + 19*b)//256
                    for dx in range(2):
                        if lum*8 > BAYER[dy][dx]*255 + 127:
                            bits |= BITS[dy][dx]
                want = chr(0x2800 + bits)
                if cell.data != want:
                    raise ValueError(f'braille mismatch at {(x,y)}: got {cell.data!r}, expected {want!r}')
    print('PASS: every reconstructed terminal cell matches input samples')

if __name__ == '__main__':
    main()
