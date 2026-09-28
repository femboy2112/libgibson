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
