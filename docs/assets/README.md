# README assets

## `libgibson-intro-finale.png`

The README hero image is a **real frame of the `libgibson_intro` short film** — the
`t ≈ 70s` chrome finale (the "libGibson" wordmark over the network Earth, with the
"HACK THE PLANET!" end card). It is not a hand-drawn banner or a mockup: it is the
engine's own output, captured and reconstructed cell-for-cell.

### How it was derived (reproducible)

1. The film is played to the finale in a fixed-size PTY so the engine emits its real
   truecolor ANSI (a redirected, non-TTY run would degrade to plain text):

   ```sh
   cargo build --release --example libgibson_intro
   # rendered in a 120x40 PTY:
   target/release/examples/libgibson_intro --at=70 --freeze --deterministic \
       --no-prologue --color=truecolor
   ```

   `--at`/`--freeze`/`--deterministic` fix the frame, so the capture is reproducible.

2. The captured byte stream is replayed through a VT emulator to reconstruct the exact
   final cell grid (grapheme + truecolor fg/bg per cell), then each cell is drawn to
   PNG: half/full block glyphs (`▀ ▄ █ ▌ ▐ ░▒▓ ▁–▇`) as exact rectangles for seamless
   tiling, and text/Braille via **DejaVu Sans Mono** (which carries the Braille and
   block repertoires). No image protocol, no sub-cell upscaling tricks.

3. `render_hero.py` in this directory is the helper that does steps 1–2. It is a
   one-off asset tool, **not** part of the build or a project dependency; it needs
   `pyte` and `Pillow` (`pip install pyte Pillow`). Regenerate with:

   ```sh
   python3 docs/assets/render_hero.py --at=70 --width=120 --height=40 \
       --crop-bottom=1 --out=docs/assets/libgibson-intro-finale.png
   ```

   (`--crop-bottom=1` trims the paused-mode HUD hint row.)

The colours, geometry, and glyphs are exactly what the engine produces; the only
post-processing is font rasterization of the reconstructed grid and the one-row HUD
crop above.

## `ui-skins-truecolor.png`

The experimental UI guide's comparison shows **the same deterministic
`ui_showcase` semantic state** at 120×40 under Vapor95, Black Ice and Swiss
Signal. Each panel comes from the example's real `Context::headless` ANSI output.
No UI labels, colors, chrome or graphics are manually placed in the image.
The outer skin captions and gutters are the capture tool's comparison frame.

```sh
cargo build --example ui_showcase
python3 -m pip install pyte Pillow
python3 docs/assets/render_ui_skins.py --width=120 --height=40 \
    --depth=truecolor --transition=settled --at-ms=1000 \
    --out=docs/assets/ui-skins-truecolor.png
```

`render_ui_skins.py` replays the ANSI stream with pyte, retains SGR faint/bold/
reverse/underline, draws block/shade/Braille glyphs from their actual cell values,
and uses DejaVu Sans Mono for text. Defaults are 10×20 pixels per terminal cell.
These are virtual-terminal captures with an explicit default palette/font, not
a guarantee about every real terminal's fonts, ANSI16 palette or cursor shape.
The capture tool does not modify LibGibson's renderer or participate in runtime.
DejaVu Sans Mono lacks some CJK glyphs; those may appear as missing-glyph boxes
in PNGs even when the source ANSI stream and cell-width invariants are correct.

For review, substitute `--depth=ansi16|mono`, `--width=80 --height=24`, or
`--width=36 --height=18`. `--skin=black-ice` captures just one skin. Motion samples
use the example's `--transition=modal|focus|activate|success|error` and `--at-ms`;
`--transition=input|toast` supplies the corresponding inspection state.
`--raw-dir=/tmp/ui-captures` also preserves source ANSI streams. Text goldens
and rendered behavioral tests remain separate from this documentation asset.
