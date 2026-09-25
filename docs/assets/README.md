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
