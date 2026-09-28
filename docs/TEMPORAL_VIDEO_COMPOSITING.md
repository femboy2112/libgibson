# Keyed video compositing (experimental)

This documents the `temporal_video_compositor` example: chroma-keying a greenscreen
clip and compositing the keyed foreground over a LibGibson-rendered background,
using the temporal subsystem's dirty-region machinery. It is a **research example**,
not a library feature — the `gibson` public API and C ABI are unchanged. Nothing
here is a claim of general "video playback".

## Why greenscreen changes the economics

A naive "play a video in the terminal" projects every pixel of every frame. At the
subcell projector's cost that is the whole grid reprojected ~29 times a second — the
worst case, and mostly wasted on a background nobody asked to preserve.

A greenscreen clip is different: the bright green field is **intentional
transparency**, not content. On the reference clip it is a measured **~81% of every
frame** (green-ness `G - max(R,B)` ≈ 250 on the clean field, < 40 on the subject).
So the strategy is not "project harder", it is:

1. **Key** the green to alpha (`key.rs`): a soft matte edge between two green-ness
   thresholds, plus green-spill suppression. Deterministic, unit-tested, no I/O.
2. **Composite** the keyed foreground over a separately rendered LibGibson
   background — *per subpixel, inside the projector's sample closure*. The composited
   RGB is fed to the **same** subcell projector every other example uses. There is
   one renderer; the output is an ordinary `Surface`.
3. **Exploit the sparsity**: only the subject's bounding box changes each source
   frame, so only the union of the previous and current bbox is reprojected with
   `TemporalDisplayProcessor::set_target_region`. The union clears the subject's
   trail; the ~70% pure-key region is never touched.

### Measured (release, 174-frame segment, 120×36 cells / 240×144 subpixels)

```
key coverage: 81.1% background keyed away, subject bbox avg 28.5% of frame
full-frame reproject : 16736.9 µs/frame
sparse (bbox union)  :  5120.7 µs/frame   (-69% vs full)   avg dirty 1257 cells (29% of grid)
temporal advance     :     63.3 µs/frame
source cadence budget: 34483 µs/frame @ 29 fps
sparse reproject + one advance = 5184 µs -> ~7x headroom under source cadence
```

Reproduce with `--profile` (headless, no TTY):

```
cargo run --release --example temporal_video_compositor -- --profile --region=120x36
```

The greenscreen sparsity is what makes this real on CPU. A GPU was reconsidered for
the moving workload (the settled-image case was already dismissed) and remains
**unjustified and research-only**: CPU sparse updates sit ~7× under the source-frame
budget, so there is no end-to-end win to chase and `wgpu` stays out of the graph.

## Temporal logic for moving content (honest)

Moving content cannot benefit from deep temporal accumulation — a first-order
sigma-delta integrates error over time, and that history is stale the instant the
content moves. So the compositor routes around it exactly as the science demands:

- The **moving subject** is marked a motion region (`set_motion_region`) and rendered
  **static** every frame. Its bbox tracks it; vacated cells are released.
- The **static designed background** is what may acquire a temporal lock and refine,
  under the usual gate (`--mode=temporal --measured-hz=<observed Hz>`), with sharp
  edges still frozen by the emitted-swing safety controller.

This is the truthful win: **a keyed composite over a temporally-stabilized
background**, not "temporal video".

## Running the demo

Default (auto-discovers the greenscreen clip in `~/Downloads`, BLACK_ICE scene):

```
cargo run --release --example temporal_video_compositor
```

Point it at any greenscreen clip and pick a scene:

```
cargo run --release --example temporal_video_compositor -- \
    --clip=/path/to/greenscreen.webm --scene=vapor --start=2 --dur=6
```

Modes (`--mode=`): `raw` (composite over black — the honest baseline), `keyed`
(designed scene, static — the hero default), `temporal` (background refinement; needs
`--measured-hz`), `diag` (bbox overlay + verbose telemetry). Scenes (`--scene=`):
`blackice` (cyan instrument field), `vapor` (vaporwave sunset + neon grid), `void`.

Ingest is an offline ffmpeg bake into a cached raw-RGB film (`bake.rs`); the runtime
never touches the codec. `--animate-bg` animates the background and (honestly)
reprojects the whole frame each source frame — the contrast case for the sparse win.

## What terminal conditions matter

- **Truecolor** (`COLORTERM=truecolor`) makes the composite look like the source.
  ANSI256/16 degrade gracefully (the projector quantizes to the wire, and safety is
  evaluated on the quantized colour). **Mono** has no colour and is forced static.
- A font that realizes **Braille U+2800–U+28FF** (e.g. DejaVu Sans Mono). The Linux
  VT console cannot realize Braille; use a graphical terminal.
- Temporal refinement additionally requires a **measured** high-refresh presentation
  profile — cadence cannot be observed through the terminal, so measure it with
  `temporal_cadence_beacon` and pass `--measured-hz`. Default output is static.

## What this is not

- Not "perfect video playback" — it is a keyed composite of a short segment, sharp on
  the subject, with terminal-cell resolution.
- Not spatial super-resolution — the projector's subcell resolution is fixed.
- Not universal — it needs Braille-capable, ideally truecolor, terminals.
- The perceptual value of temporal refinement remains **unproven**; only the physical
  experiment in `TEMPORAL_DISPLAY_VALIDATION.md` can settle it.

## Strongest truthful claim

> LibGibson can chroma-key a greenscreen clip and composite the keyed subject over a
> designed, temporally-stabilized background as ordinary two-colour Braille surfaces —
> exploiting the greenscreen's ~80% transparency so only the subject's bounding box is
> reprojected each frame, holding source cadence with ~7× CPU headroom and no GPU.

## Reaction cut: compositing into an existing terminal-native Surface

`temporal_video_compositor` keys a subject over a *designed* background. The second
flagship, `libgibson_intro_reaction`, goes further: it composites the keyed subject
**into the existing `libgibson_intro` short film** as though the subject is reacting
to it while it unfolds — a directed edit, not a video in a box.

The load-bearing property is that the original film is not degraded. The base intro
`Surface` is generated exactly as `libgibson_intro` does (its `frame()` is reused
verbatim — one renderer), and only the cells the transformed keyed subject actually
covers are patched. Every other cell is byte-identical to the intro, so the crisp
terminal-native text, wireframes and title survive untouched right up to the
subject's silhouette, and a moving subject's vacated trail is the original film again
the instant it leaves. Per touched cell the compositor reconstructs the underlying
cell to a logical 2×4 RGB tile (Braille mask, half-block and space decoded exactly;
ordinary text a documented conservative fallback used only *under* the subject),
inverse-samples and keys the foreground, alpha-composites per subpixel (byte-space by
default; opt-in linear-light and bilinear-on-premultiplied to suppress green edge
halos when a beat zooms), and re-projects that one tile through the same subcell
projector.

Two clocks stay independent so the original film's determinism is preserved: an
**edit clock** (what `--at`/`--stage` seek) and the **intro narrative clock**, which a
cue may `Continue`, `Hold` or `Slow` — letting a reaction run past the film's 72 s
over a held final frame without pretending the film became longer. Given an edit
time, the active cue, base-intro time, source window and transform are all computed
deterministically; nothing reads wall-clock time on the frame path.

### Measured (release, `--profile`, base-intro generation vs the local patch)

```
120x36-ish grid (120x32 cells):
  normal (city reveal) : base 3756 us + patch 1220 us = 4976 us   composited 257/3840 cells (6.7%)
  punch  (escalate)    : base 6794 us + patch 3572 us = 10365 us  composited 831/3840 cells (21.6%)
  quiet  (ascent)      : base 5310 us + patch  501 us = 5811 us   composited 111/3840 cells (2.9%)
  full-grid reproject (avoided)                        = 13194 us/frame
```

The reaction layer adds a small local patch (~0.5–3.6 ms) on top of the intro's own
native render; reprojecting the whole composited frame would cost ~13 ms. Because the
subject touches only 3–22% of the grid, the projector runs on a small minority of
cells — the same sparsity lever as the pure keyed-video path, here exploited against
a *live* rendered Surface rather than a designed background.

### Running it

```
cargo run --release --example libgibson_intro_reaction              # live, plays the edit
cargo run --release --example libgibson_intro_reaction -- --at=14   # seek the edit clock
cargo run --release --example libgibson_intro_reaction -- --stage=city_reveal --freeze
cargo run --release --example libgibson_intro_reaction -- --profile # headless receipts
cargo run --release --example libgibson_intro_reaction -- --no-reaction   # base intro only
```

It bakes only the ten short cue windows it uses (never the full clip) and discovers a
greenscreen clip in `~/Downloads`, or takes `--clip=PATH`. `--smooth` (bilinear) and
`--linear` (linear-light matte) trade cost for edge quality.

### What it is and is not

This is a genuine hardened capability — a keyed live-action reaction coherently edited
into an existing terminal-native cinematic Surface without degrading it — not
"terminal video playback". At terminal-cell resolution a smooth photographic region
(a cheek, a flat shirt) collapses to near-uniform Braille cells; detail survives at
edges and high-contrast features (hair, sunglasses, folds). The subject reads as a
person built from terminal cells, which is the point. The source clip is a local
stress-test asset and is **not** shipped.
