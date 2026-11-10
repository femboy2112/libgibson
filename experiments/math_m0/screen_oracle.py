#!/usr/bin/env python3
"""Compare an ANSI capture's terminal cells with the original RGB samples."""
from pathlib import Path
import sys
import pyte

BAYER = ((0, 4), (6, 2), (1, 5), (7, 3))
BITS = ((1, 8), (2, 16), (4, 32), (64, 128))


def main():
    if len(sys.argv) != 4 or sys.argv[3] not in ('half', 'mono'):
        raise SystemExit('usage: screen_oracle.py INPUT.rgb CAPTURE.ansi half|mono')
    raw = Path(sys.argv[1]).read_bytes()
    if len(raw) != 9600:
        raise ValueError('expected exactly 9600 RGB bytes')
    screen = pyte.Screen(80, 20)
    pyte.Stream(screen).feed(Path(sys.argv[2]).read_bytes().decode('utf-8'))

    def pixel(x, y):
        i = (y * 80 + x) * 3
        return tuple(raw[i:i+3])

    for y in range(20):
        for x in range(80):
            cell = screen.buffer[y][x]
            if sys.argv[3] == 'half':
                if cell.data == '▀':
                    got = (cell.fg, cell.bg)
                elif cell.data == ' ':  # a blank with the same background for both samples
                    got = (cell.bg, cell.bg)
                else:
                    raise ValueError(f'unexpected half-block glyph at {(x,y)}: {cell.data!r}')
                want = tuple(bytes(pixel(x, 2*y+dy)).hex() for dy in (0, 1))
                if got != want:
                    raise ValueError(f'pixel mismatch at {(x,y)}: got {got}, expected {want}')
            else:
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
