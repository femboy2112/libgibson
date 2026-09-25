# Glyph realization and the sub-cell fidelity ladder

LibGibson squeezes extra resolution out of ordinary terminal cells with
**sub-cell graphics**: a `BrailleCanvas` addresses a 2×4 dot grid per cell and
renders each cell as a Unicode Braille glyph `U+2800 + bits`. This is the
highest-resolution single-cell realization and the engine's hero path.

But *whether a terminal displays that glyph* is a different question from
*whether the terminal protocol supports it*, and this document is about that
distinction, the fallback ladder, and how to control it.

## 1. Two capability axes

| axis | examples | detectable? |
|------|----------|-------------|
| **Terminal protocol capability** | color depth, synchronized updates, `CSI L` insert-line, OSC 8 hyperlinks | partially, via env/queries (`TerminalCapabilities`) |
| **Glyph realization capability** | does the loaded font actually contain Braille? half blocks? box drawing? | **no reliable universal query** |

These are **orthogonal**. A terminal may have TrueColor + a font with no Braille,
or ANSI-16 + a full Unicode font. LibGibson keeps them compositional:
glyph realization (`SubcellGlyphMode`, this document) never touches color, and
color quantization (`TerminalCapabilities`/`ColorDepth`) never touches glyphs.

## 2. The Linux virtual-console observation

The full `libgibson_intro` film was run on the **Linux kernel virtual console**
(TTY1, reached via Ctrl+Alt+F1). Ordinary text and layout rendered correctly, but
the Braille sub-cell graphics came out as **unrelated / corrupt glyphs** — the
wireframes looked blocky and wrong.

This is **font/glyph-repertoire realization**, not a framebuffer, diff, ANSI, or
encoding bug. The Linux kernel console accepts UTF-8 input but renders through its
loaded console bitmap font plus a Unicode map; the default fonts (e.g.
`default8x16`) do not carry the `U+2800..=U+28FF` Braille block. So:

> **"UTF-8 terminal" does not imply "the Braille block is displayable."**

Block elements (`U+2580..=U+259F`) descend from the VGA/CP437 repertoire and *are*
reliably present in Linux console fonts, which is why they make a safe fallback.

There is **no reliable universal way to query a font's glyph repertoire**, so
LibGibson does not pretend to detect it. It offers an explicit override and a
conservative `Auto` policy, and leaves visual confirmation to a human:

```
cargo run --example glyph_capability_lab
```

That probe renders each glyph family and the same sub-cell field in every mode,
side by side, and states plainly that **visual inspection is required**.

## 3. The realization ladder — `SubcellGlyphMode`

`SubcellGlyphMode::subcell_glyph(mask: u8) -> Option<char>` is the single pure
realization function. It returns `None` **iff** the mask is empty (`bits == 0`),
so an empty logical sample stays empty in every mode and any lit sample stays
visible in every mode (fallback never silently drops a dot).

| mode | sub-cells/cell | glyphs | how the 2×4 mask is aggregated | availability |
|------|----------------|--------|--------------------------------|--------------|
| `Braille2x4`   | 2×4 = 8 | `U+2800..=U+28FF` | none — full resolution (`0x2800 + bits`) | modern UTF-8 fonts |
| `HalfBlock1x2` | 1×2 = 2 | ` ▀▄█` | top rows (mask&`0x1B`) / bottom rows (mask&`0xE4`) → upper/lower/full | ~universal (CP437) |
| `Block`        | 1 (density) | ` ░▒▓█` | dot popcount → shade level | ~universal (CP437) |
| `Ascii`        | 1 (density) | ` .:-=+*#@` | dot popcount → ink ramp | universal |

The half-block masks `0x1B` (rows 0–1) and `0xE4` (rows 2–3) are disjoint and
cover all eight dots, matching `BrailleCanvas::dot_bit`. Lower-resolution modes
preserve topology (a thin line stays a connected run of light glyphs; a fill
stays solid) rather than faking resolution.

## 4. Detection and override

Precedence (highest first):

1. an explicit CLI value, e.g. `--glyphs=halfblock`;
2. the `LIBGIBSON_GLYPHS` environment variable;
3. `Auto` — resolved from `TERM`.

Accepted values: `auto | braille | halfblock | block | ascii` (plus a few
aliases: `half`/`half-block`, `blocks`/`shade`, `braille2x4`, `text`).

`Auto` policy:

* `TERM=linux` → `HalfBlock1x2` — a conservative, structure-preserving,
  CP437-safe default for the kernel VT. This is a **policy** default, **not** a
  claim that Braille is unavailable: a custom console font may contain it, and
  `--glyphs=braille` forces it.
* anything else → `Braille2x4` — the current behavior on graphical terminals is
  preserved exactly.

`ASCII` is the ultimate fallback, realizable on any font.

The resolver is `gibson::detect_glyph_mode(cli, get_env)` with an injectable
environment getter (mirroring `TerminalCapabilities::from_env`) for testability;
`detect_glyph_mode_from_env(cli)` uses the real process environment.

## 5. The generic realization chokepoint

A Braille glyph **losslessly encodes** its mask: `bits = glyph - 0x2800`. So any
surface produced by the Braille path can be re-realized into another family with
a single pass — no need to thread a mode parameter through every renderer:

```rust
gibson::transcode_surface_glyphs(&mut surface, mode);
```

`transcode_surface_glyphs` walks the surface, and for every cell whose glyph is a
single Braille code point it recovers the mask and re-realizes it via
`subcell_glyph`. It **does not** touch non-Braille content (text, box drawing, RGB
half-blocks) or any cell's **style** (the axis stays orthogonal to color).
`Braille2x4` returns immediately, so the hero path is **byte-identical** — every
existing cinematic golden is unaffected by construction.

For code that holds a `BrailleCanvas` directly (rather than a finished surface),
`BrailleCanvas` exposes `mask_at`, `glyph_at_mode`, `to_lines_mode`, and
`to_surface_mode`; `glyph_at` is exactly `glyph_at_mode(.., Braille2x4)`.

## 6. Where it is wired

| surface | control | behavior |
|---------|---------|----------|
| `libgibson_intro` (the film) | `--glyphs=` / `LIBGIBSON_GLYPHS` | transcodes the frame at both render sinks (interactive loop + `--dump`) |
| `glyph_capability_lab` | `--glyphs=` | the visual probe itself |
| `fx_lab` | scene **"Glyph ladder"** (#28) | one source field realized four ways side by side |
| `runtime_observatory` (live) | `--glyphs VALUE` / `LIBGIBSON_GLYPHS` | realizes the live sparklines; deterministic `--dump` frames stay Braille |

## 7. Braille synthesis audit

Every place that synthesizes `U+2800 + bits` (so every place a fallback matters):

* **`src/canvas.rs`** — `BrailleCanvas::glyph_at` (the one library synthesis
  site; now routes through `SubcellGlyphMode`).
* **`src/raster.rs`** — `RgbRaster::to_mono_surface` (Bayer-dithered Braille).
* **`examples/libgibson_intro/world.rs`** — `wire_surface` (the city wireframe).
* **`examples/libgibson_intro/membrane.rs`** — the harness→city refraction
  (decode + re-encode of Braille dots).
* **`examples/libgibson_intro/facade.rs`** — `chart` (OR-unions two Braille
  canvases).
* Consumers that read `glyph_at` and overlay it: `fx_lab/rgb.rs`,
  `acid_vs_crash/cyber.rs`.
* `braille_oscilloscope` (used by the observatory and `fx_lab`) and the
  `SPINNER_BRAILLE` frames in `src/node.rs` (literal spinner glyphs, not
  arithmetic).

Because every one of these emits Braille **into a `Surface`**, they are all
covered by the single `transcode_surface_glyphs` chokepoint wherever a caller
chooses to apply it. There are **no** scattered `if TERM == linux` checks.

## 8. Extension points

* **Add a glyph family:** add a `SubcellGlyphMode` variant and its arm in
  `subcell_glyph`, keeping the `None`-iff-empty invariant; add a `GlyphChoice`
  alias in `parse`. `transcode_surface_glyphs`, every `*_mode` canvas method, and
  every wired surface pick it up for free. Add exact-pattern unit tests.
* **Make a new renderer mode-aware:** if it produces a `Surface`, call
  `transcode_surface_glyphs(&mut surface, mode)` at its display sink; if it holds
  a `BrailleCanvas`, use `to_surface_mode`/`to_lines_mode`.
* **Not yet wired (by design):** the maximalist Node-composed demos
  `hack_the_gibson`, `acid_vs_crash`, and `polished_agent` render at full Braille
  fidelity (appropriate for graphical terminals). They compose `Node` trees
  rather than exposing a single surface sink, so — per the mandate to avoid
  patching each site — they are left as extension points: they can adopt fallback
  through the same `SubcellGlyphMode` primitives without any new mechanism.

## 9. What is *not* claimed

* CI cannot prove what a physical console font displays. The logical realization
  is exhaustively tested (`src/glyph.rs`, `tests/glyph_realization.rs`); font
  presence is a matter for `glyph_capability_lab` and human eyes.
* `Auto`'s `TERM=linux → HalfBlock` choice is a **policy**, not a font detection.
  Explicit `--glyphs=braille` overrides it.
* The glyph axis is Rust-only and additive. It adds **no** C ABI surface, and
  `GIBSON_ABI_VERSION` is unchanged.
