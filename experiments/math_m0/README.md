# Math M0: fixed-fixture forcing experiment

**Research only, not a public math API or a sandbox.** Four hard-coded,
trusted formulas (integral, nested fraction, matrix and aligned equations)
go through local pdfTeX → Poppler rasterization → an 80×40 RGB image → public
`RgbRaster` → `Node::raster` → headless `Context` capture. The source is
retained in each JSON receipt. Rendering uses the ordinary LibGibson renderer,
not a separate terminal renderer. The result is **cell fallback**, not a pixel
protocol or paper-quality terminal output.

The command accepts only one of four fixture names; it does not accept math
source, TeX, SVG, font names, dimensions or commands from a user. pdfTeX is
powerful and is **not sandboxed** here: `-no-shell-escape` and process resource
limits are not a filesystem or network security boundary. Never extend this
script to process untrusted content without a separately tested isolation and
input-validation design. The local trusted toolchain, PATH, working directory,
output directory parent and fonts are assumed trustworthy. No fonts are vendored.
The renderer refuses an existing output directory, but this is not protection
against an attacker racing path components or replacing binaries. The capture
consumer follows file paths and may overwrite its output; use trusted paths.
Do not feed it FIFOs, devices or other untrusted file types. The subprocess
limits do not bound total parent memory or escaped child processes, and the
subprocess output pipes are buffered in memory. They do not constrain a
malicious replacement executable or a compromised toolchain.

## Run

Requires Rust/Cargo and local `pdflatex`, `pdftoppm`, `pdftotext`, Pillow,
NumPy and Matplotlib. There is no new core dependency. On the evaluated host,
`PYTHONNOUSERSITE=1` avoids an incompatible user-site NumPy installation.

```sh
cargo build --locked --example math_m0_capture -j 1
PYTHONNOUSERSITE=1 /usr/bin/python3 experiments/math_m0/check.py
out=$(mktemp -d)
PYTHONNOUSERSITE=1 /usr/bin/python3 experiments/math_m0/render.py nested "$out/nested"
target/debug/examples/math_m0_capture "$out/nested/nested.rgb" "$out/nested/nested-half.ansi" half
target/debug/examples/math_m0_capture "$out/nested/nested.rgb" "$out/nested/nested-mono.ansi" mono
```

The check exercises all fixtures, both cell encodings, same-host repeated RGB
hashes, malformed/unknown fixture rejection, short and oversized raw buffers,
and a failed renderer. It compares mathtext's independent glyph/rule geometry
against order expectations and separately checks Poppler PDF text boxes plus
the fraction rule in the TeX raster. Reversing numerator and denominator is a
negative ordering control. These are **partial geometry checks**, not a font
metric or complete clipping certification. The capture check verifies nonempty
cell output and distinct representations; it is not a reconstructed-screen
visual oracle.

## Findings on the observed host

- pdfTeX 3.141592653-2.6-1.40.25 (TeX Live 2023), Poppler 24.02.0,
  Pillow 10.2.0, and Matplotlib 3.6.3/NumPy 1.26.4 were used. The nested
  PDF reports embedded CMMI12 and CMR17 font subsets; check font licenses
  before any font distribution. Results and hashes are host/toolchain specific.
- Matplotlib mathtext is a distinct, offline math layout implementation. It
  produced glyphs/rules for fraction and integral, but rejected the matrix and
  aligned-environment fixtures in the installed version. This is a limited
  local probe, not a selection or a general statement about other versions.
  Typst and the MathJax Node package were unavailable and were not tested.
- The 240×120 TeX page is downsampled to 80×40 pixels and realized as 80×20
  half-block or monochrome cells. Fine glyph details are visibly lost; content
  near the origin occupies a small part of this fixed viewport. Neither path
  establishes legibility at 40×15, full-page layout, clipping behavior or an
  equivalent mathematical fallback. Retaining source is necessary because
  visual identity can be lost.
- No connected graphical terminal was available to observe pixel delivery;
  support remains **Unknown**. A Kitty executable or TERM value alone does
  not prove a working graphics transport. No paper-quality terminal output is
  claimed.
- Arbitrary input, SVG parsing, hostile SVG, filesystem/network isolation,
  fonts unavailable, recursion, full resource envelopes, Rust 1.85, long
  documents, resize and graphics protocol lifecycle remain unqualified. The
  backend-failure test is a deliberate exit status, not evidence about all
  process-termination races.

Next experiment: use a restricted semantic AST and genuinely isolated worker,
compare independently sourced vector metrics and clipping, and feed its bounded
pixels through a renderer-owned image protocol observed on a real terminal.
Keep semantic source available even when the pixel transport is unsupported.
