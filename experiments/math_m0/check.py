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
ENV = dict(os.environ, PYTHONNOUSERSITE='1')


def cmd(*args, ok=True, env=ENV):
    p = subprocess.run(args, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env, timeout=30)
    if (p.returncode == 0) != ok:
        raise AssertionError((args, p.returncode, p.stderr[-2000:]))
    return p


def order_is_valid(top, bottom):
    return top < bottom


def main():
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
            cmd(sys.executable, str(RENDER), fixture, str(d))
            raw = (d/(fixture+'.rgb')).read_bytes()
            assert len(raw) == 9600 and len(set(raw)) > 2
            for mode in ('half', 'mono'):
                out = d/(fixture+'-'+mode+'.ansi')
                cmd(str(BIN), str(d/(fixture+'.rgb')), str(out), mode)
                data = out.read_bytes()
                assert len(data) > 100 and (('▀' if mode == 'half' else '⣿').encode() in data)
            assert (d/(fixture+'-half.ansi')).read_bytes() != (d/(fixture+'-mono.ansi')).read_bytes()
        old = hashlib.sha256((d/'nested.rgb').read_bytes()).digest()
        cmd(sys.executable, str(RENDER), 'nested', str(d))
        assert hashlib.sha256((d/'nested.rgb').read_bytes()).digest() == old
        cmd('pdftotext','-bbox',str(d/'nested.pdf'),str(d/'nested.html'))
        doc = ET.parse(d/'nested.html')
        words = {e.text: e.attrib for e in doc.iter() if e.tag.endswith('word')}
        assert order_is_valid(float(words['b']['yMax']), float(words['c']['yMin']))
        assert not order_is_valid(float(words['c']['yMax']), float(words['b']['yMin']))
        # Independent PDF text geometry plus a pixel witness of its inner rule.
        lo = int(float(words['b']['yMax'])) + 1
        hi = int(float(words['c']['yMin']))
        image = Image.open(d/'nested-full.png').convert('L')
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
    print('PASS: four fixed fixtures, both cell modes, repeat hash, independent fraction geometry, malformed/oversize/failing backend controls')
    print('MathText fraction/integral supported; matrix/aligned rejected in installed version')

if __name__ == '__main__': main()
