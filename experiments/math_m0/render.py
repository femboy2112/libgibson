#!/usr/bin/env python3
"""Trusted-fixture research only. Never accepts or executes a caller's math source."""
import argparse
import hashlib
import io
import json
import os
import resource
import shutil
import subprocess
import tempfile
from pathlib import Path

# Literal documents, not templates into which user-provided text can flow.
FIXTURES = {
    'integral': r'\displaystyle \int_0^\infty e^{-x^2}\,dx=\frac{\sqrt{\pi}}{2}',
    'nested': r'\displaystyle \frac{a+\frac{b}{c}}{d+e}',
    'matrix': r'\displaystyle A=\begin{pmatrix}a&b&c\\d&e&f\\g&h&i\end{pmatrix}',
    'aligned': r'\displaystyle \begin{aligned} a+b&=c\\ c-d&=e \end{aligned}',
}
PREAMBLE = r'''\documentclass{article}
\usepackage{amsmath}
\usepackage[paperwidth=240bp,paperheight=120bp,margin=8bp]{geometry}
\pagestyle{empty}
\begin{document}
\fontsize{20}{24}\selectfont\noindent $'''
END = r'$\end{document}'


def limits():
    resource.setrlimit(resource.RLIMIT_CPU, (5, 5))
    resource.setrlimit(resource.RLIMIT_AS, (512*1024*1024, 512*1024*1024))
    resource.setrlimit(resource.RLIMIT_FSIZE, (8*1024*1024, 8*1024*1024))


def run(argv, cwd, env):
    # Limits reduce accidental resource use; they do not sandbox file/network access.
    return subprocess.run(argv, cwd=cwd, env=env, check=True, timeout=12,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          preexec_fn=limits)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('fixture', choices=sorted(FIXTURES))
    p.add_argument('output', type=Path, help='output directory controlled by caller')
    a = p.parse_args()
    # All formula inputs above are developer-owned literals. Do not add text inputs.
    from PIL import Image
    # Refuse an existing destination: do not overwrite a previous result.
    # The caller must also ensure its parent cannot be modified by an attacker.
    a.output.mkdir()
    env = {k: os.environ[k] for k in ('PATH', 'HOME', 'LANG') if k in os.environ}
    env['SOURCE_DATE_EPOCH'] = '946684800'
    with tempfile.TemporaryDirectory(prefix='gibson-math-trusted-') as t:
        d = Path(t)
        (d/'formula.tex').write_text(PREAMBLE + FIXTURES[a.fixture] + END, encoding='ascii')
        run(['pdflatex','-no-shell-escape','-interaction=nonstopmode','-halt-on-error','formula.tex'], d, env)
        run(['pdftoppm','-f','1','-singlefile','-r','72','-png','formula.pdf','page'],d,env)
        shutil.copyfile(d/'formula.pdf', a.output/(a.fixture+'.pdf'))
        shutil.copyfile(d/'page.png', a.output/(a.fixture+'-full.png'))
        with Image.open(d/'page.png') as image:
            if image.size != (240, 120):
                raise ValueError(f'unexpected source image size {image.size}')
            # Preserve aspect ratio; the capture is deliberately only 80×40 pixels.
            image = image.convert('RGB').resize((80, 40), Image.Resampling.LANCZOS)
            raw = image.tobytes()
            image.save(a.output/(a.fixture+'.png'))
        (a.output/(a.fixture+'.rgb')).write_bytes(raw)
        (a.output/(a.fixture+'.json')).write_text(json.dumps({
            'trusted_fixture': a.fixture, 'semantic_source': FIXTURES[a.fixture],
            'pixel_dimensions': [80,40], 'rgb_sha256': hashlib.sha256(raw).hexdigest(),
            'fidelity': 'cell fallback; no graphical terminal transport claimed'}, indent=2)+'\n')
    print(a.output/(a.fixture+'.rgb'))

if __name__ == '__main__':
    main()
