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
NumPy, Matplotlib and pyte. There is no new core dependency. On the evaluated host,
`PYTHONNOUSERSITE=1` avoids an incompatible user-site NumPy installation.
The checker honors `CARGO_TARGET_DIR`, including a path containing spaces; use
the same setting for the example build and checker invocation.

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
the fraction rule in the TeX raster. Matrix glyph boxes must form three ordered
rows and columns, and the two equation rows must align at their equals signs.
Substituting another rendered fixture and altering the tested positions are
negative controls; these PDF checks do not prove that its raster is faithful.
Reversing numerator and denominator is a negative ordering control. These are
**partial geometry checks**, not an independent proof of integral shape, font
metrics, delimiter geometry, or complete clipping. Pyte independently interprets the
actual capture into 80×20 terminal cells; the oracle compares every half-block
foreground/background sample, including reverse-video state, and every
monochrome Braille glyph with the RGB input. Explicit mono colors must have
bright foreground and a contrast threshold using assumed standard palette
values; if both colors are `default`, the test assumes a light foreground on
a dark background. Actual terminal palette and visual contrast are not
measured. White/ignored-input, reversed-video, concealed text and black-on-black
mutants are rejected against their controls. Uncalibrated SGR sequences are
rejected before parsing, since Pyte does not model every SGR. The capture
validator accepts only the specific calibrated control sequences used by this
experiment, including balanced synchronized updates. It rejects alternate
screen switches, OSC, incomplete sequences and other unsupported controls.
This checks the modelled cell result, not
visual readability or the quality of typesetting; Pyte is not a complete
terminal emulator and cannot validate effects it does not model.

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
- A separate, real Kitty 0.32.2 X11 probe returned `OK` for an RGB image
  query and an error for deliberately invalid image format 99. A screenshot
  of that Kitty window contained 23,108 red pixels from a 12×12 red image
  placed at 20×8 cells. This is a controlled protocol probe, **not** a
  LibGibson rendering path, a formula display, or proof of image teardown,
  clipping, resize or long-session behavior. No paper-quality terminal output
  is claimed. Other terminals remain unqualified.
- The capture example was freshly built on Rust 1.85 and 1.98.1; the complete
  four-fixture headless checker passed with each executable. This qualifies the
  capture path, not optional future typesetter dependencies or a release tree.
- Arbitrary input, SVG parsing, hostile SVG, filesystem/network isolation,
  fonts unavailable, recursion, full resource envelopes, long
  documents, resize and graphics protocol lifecycle remain unqualified. The
  backend-failure test is a deliberate exit status, not evidence about all
  process-termination races.

Next experiment: use a restricted semantic AST and genuinely isolated worker,
compare independently sourced vector metrics and clipping, and feed its bounded
pixels through a renderer-owned image protocol observed on a real terminal.
Keep semantic source available even when the pixel transport is unsupported.

## Separate real Kitty experiment

This optional command opens a temporary terminal window and sends only fixed
probe data. It is not a general rendering backend. Run it on a trusted local
X11 desktop, then capture the window with a separate screenshot tool while it
is open; inspecting the `OK` response alone does not prove visible pixels.
The deliberate invalid-format query should return an error on Kitty, not be
misreported as an unsupported-terminal test. An injected delete-write failure
checks that termios restoration is attempted; it cannot guarantee restoration
when the device itself fails or the process is forcibly terminated. `--config NONE` avoids changing or
loading user Kitty settings. The result file must not already exist.

```sh
out=$(mktemp -d)
kitty --config NONE --title LIBGIBSON-M0-CAPABILITY-PROBE \
  /usr/bin/python3 experiments/math_m0/kitty_probe.py "$out/responses.bin"
```

A window screenshot was separately measured using ImageMagick and Pillow.
This observation does not upgrade LibGibson's capability status: the library
does not yet own this image protocol path.
