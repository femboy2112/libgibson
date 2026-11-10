#!/usr/bin/env python3
"""Empirical controls for the trusted-fixture M0; no general math input is accepted."""
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from PIL import Image
from matplotlib.mathtext import MathTextParser

ROOT = Path(__file__).resolve().parents[2]
RENDER = ROOT/'experiments/math_m0/render.py'
BIN = ROOT/'target/debug/examples/math_m0_capture'
ORACLE = ROOT/'experiments/math_m0/screen_oracle.py'
ENV = dict(os.environ, PYTHONNOUSERSITE='1')
SCREEN_ENV = dict(ENV)
SCREEN_ENV.pop('PYTHONNOUSERSITE', None)


def cmd(*args, ok=True, env=ENV):
    p = subprocess.run(args, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env, timeout=30)
    if (p.returncode == 0) != ok:
        raise AssertionError((args, p.returncode, p.stderr[-2000:]))
    return p


def order_is_valid(top, bottom):
    return top < bottom


def main():
    cmd(sys.executable, str(ROOT/'experiments/math_m0/check_kitty_cleanup.py'))
    m = MathTextParser('path')
    p = m.parse(r'$\frac{a+\frac{b}{c}}{d+e}$', dpi=72)
    glyphs = {chr(g[2]): g[4] for g in p.glyphs if chr(g[2]) in 'abcde'}
    assert len(p.rects) == 2 and glyphs['b'] > glyphs['c'] > glyphs['d']
    assert not glyphs['c'] > glyphs['b']  # swapped numerator/denominator mutation
    assert m.parse(r'$\int_0^\infty e^{-x^2}dx$', dpi=72).height > 0
    for unsupported in (r'$\begin{matrix}a&b\\c&d\end{matrix}$', r'$\begin{aligned}a&=b\\&=c\end{aligned}$'):
        try:
            m.parse(unsupported, dpi=72)
            raise AssertionError('installed mathtext unexpectedly accepted this fixture')
        except ValueError:
            pass
    with tempfile.TemporaryDirectory(prefix='gibson-math-check-') as tmp:
        d = Path(tmp)
        for fixture in ('integral', 'nested', 'matrix', 'aligned'):
            fixture_dir = d/fixture
            cmd(sys.executable, str(RENDER), fixture, str(fixture_dir))
            raw = (fixture_dir/(fixture+'.rgb')).read_bytes()
            assert len(raw) == 9600 and min(raw) < 200
            for mode in ('half', 'mono'):
                out = fixture_dir/(fixture+'-'+mode+'.ansi')
                cmd(str(BIN), str(fixture_dir/(fixture+'.rgb')), str(out), mode)
                data = out.read_bytes()
                assert len(data) > 100
                cmd(sys.executable, str(ORACLE), str(fixture_dir/(fixture+'.rgb')), str(out), mode, env=SCREEN_ENV)
            assert (fixture_dir/(fixture+'-half.ansi')).read_bytes() != (fixture_dir/(fixture+'-mono.ansi')).read_bytes()
        # Discriminating terminal-style controls: a reverse-video mutant swaps
        # the half-block samples; a black-on-black mono mutant hides the dots.
        pattern = bytes([255, 0, 0])*80 + bytes([0, 0, 255])*80
        (d/'pattern.rgb').write_bytes(pattern*20)
        rows = ''.join(f'\x1b[{row};1H' + '▀'*80 for row in range(1, 21))
        ansi = '\x1b[38;2;255;0;0;48;2;0;0;255m'
        (d/'style-good.ansi').write_bytes((ansi+rows).encode())
        (d/'style-reverse.ansi').write_bytes((ansi+'\x1b[7m'+rows).encode())
        (d/'style-hidden.ansi').write_bytes((ansi+'\x1b[8m'+rows).encode())
        cmd(sys.executable, str(ORACLE), str(d/'pattern.rgb'), str(d/'style-good.ansi'), 'half', env=SCREEN_ENV)
        cmd(sys.executable, str(ORACLE), str(d/'pattern.rgb'), str(d/'style-reverse.ansi'), 'half', ok=False, env=SCREEN_ENV)
        cmd(sys.executable, str(ORACLE), str(d/'pattern.rgb'), str(d/'style-hidden.ansi'), 'half', ok=False, env=SCREEN_ENV)
        (d/'white.rgb').write_bytes(bytes([255])*9600)
        braille = ''.join(f'\x1b[{row};1H' + '⣿'*80 for row in range(1, 21))
        (d/'mono-good.ansi').write_bytes(('\x1b[37;40m'+braille).encode())
        (d/'mono-hidden.ansi').write_bytes(('\x1b[30;40m'+braille).encode())
        cmd(sys.executable, str(ORACLE), str(d/'white.rgb'), str(d/'mono-good.ansi'), 'mono', env=SCREEN_ENV)
        cmd(sys.executable, str(ORACLE), str(d/'white.rgb'), str(d/'mono-hidden.ansi'), 'mono', ok=False, env=SCREEN_ENV)
        # Blank/ignored-input mutant: terminal output is valid, but it must not
        # match the real nested-fraction samples under either screen oracle.
        (d/'blank.rgb').write_bytes(bytes([255])*9600)
        for mode in ('half', 'mono'):
            cmd(str(BIN), str(d/'blank.rgb'), str(d/('blank-'+mode+'.ansi')), mode)
            cmd(sys.executable, str(ORACLE), str(d/'blank.rgb'), str(d/('blank-'+mode+'.ansi')), mode, env=SCREEN_ENV)
            cmd(sys.executable, str(ORACLE), str(d/'nested'/'nested.rgb'), str(d/('blank-'+mode+'.ansi')), mode, ok=False, env=SCREEN_ENV)
        old = hashlib.sha256((d/'nested'/'nested.rgb').read_bytes()).digest()
        cmd(sys.executable, str(RENDER), 'nested', str(d/'repeat'))
        assert hashlib.sha256((d/'repeat'/'nested.rgb').read_bytes()).digest() == old
        cmd(sys.executable, str(RENDER), 'nested', str(d/'nested'), ok=False)
        assert hashlib.sha256((d/'nested'/'nested.rgb').read_bytes()).digest() == old
        cmd('pdftotext','-bbox',str(d/'nested'/'nested.pdf'),str(d/'nested.html'))
        doc = ET.parse(d/'nested.html')
        words = {e.text: e.attrib for e in doc.iter() if e.tag.endswith('word')}
        assert order_is_valid(float(words['b']['yMax']), float(words['c']['yMin']))
        assert not order_is_valid(float(words['c']['yMax']), float(words['b']['yMin']))
        # Independent PDF text geometry plus a pixel witness of its inner rule.
        lo = int(float(words['b']['yMax'])) + 1
        hi = int(float(words['c']['yMin']))
        image = Image.open(d/'nested'/'nested-full.png').convert('L')
        x0, x1 = int(float(words['b']['xMin'])), int(float(words['b']['xMax'])) + 1
        assert any(sum(image.getpixel((x,y)) < 128 for x in range(x0,x1)) >= 4 for y in range(lo,hi+1))
        cmd(sys.executable, str(RENDER), r'\\input{/etc/passwd}', str(d), ok=False)
        cmd(sys.executable, str(RENDER), r'\\frac{', str(d), ok=False)
        (d/'short.rgb').write_bytes(b'\0')
        (d/'long.rgb').write_bytes(b'\0'*9601)
        for name in ('short.rgb','long.rgb'):
            cmd(str(BIN), str(d/name), str(d/'bad.ansi'), 'half', ok=False)
        # A terminated/failing backend never produces a successful fixture.
        fake = d/'bin'; fake.mkdir()
        (fake/'pdflatex').write_text('#!/bin/sh\nexit 137\n')
        (fake/'pdflatex').chmod(0o755)
        env = dict(ENV, PATH=str(fake)+':'+ENV['PATH'])
        cmd(sys.executable, str(RENDER), 'nested', str(d/'failed'), ok=False, env=env)
        assert not (d/'failed/nested.rgb').exists()
    print('PASS: four fixtures, both terminal-cell oracles, blank mutant, repeat hash, fraction geometry, invalid input and failing backend controls')
    print('MathText fraction/integral supported; matrix/aligned rejected in installed version')

if __name__ == '__main__': main()
