# Temporal projector GPU research

This directory defines the benchmark contract for an **optional** GPU backend in
the Temporal Glyph Synthesis research program. It intentionally adds no `wgpu`
dependency to the LibGibson core crate.

## What the GPU should *not* do

The runtime sigma-delta update in `gibson::temporal` is tiny. A 120x40 terminal
with eight logical Braille subcells has only 38,400 scalar residual accumulators
per frame. CPU scalar/SIMD work is expected to beat GPU dispatch,
synchronization, and readback for that stage.

The terminal still needs CPU-visible glyph/style choices for ANSI compilation, so
a GPU-only runtime representation does not remove the readback boundary.

## Where GPU compute may help

The expensive operation is the **static projector/calibration** that should run
*before* temporal residual modulation:

1. rasterize or ingest a calibrated glyph basis;
2. evaluate many `(glyph, foreground, background)` candidates per target cell;
3. fit colors in linear-light or perceptual space;
4. build/cache per-font codebooks;
5. batch-project large image rasters into best static terminal cell states;
6. optionally search a small residual temporal codebook.

These operations are massively parallel and amortizable.

## Required architecture

Implement the CPU reference first behind a projector interface conceptually like:

```text
TargetTile -> StaticCellProjection {
    glyph,
    foreground,
    background,
    residual[physical/logical sample],
    error
}
```

A later GPU experiment must produce the same semantic output and use the same
error metric.

Keep the GPU experiment either:

- as a sibling research crate under this directory, or
- behind an explicitly optional feature only after the dependency/compile-size
  cost is justified.

Do not make `wgpu` a mandatory LibGibson dependency merely because a GPU exists.

## Adapter feasibility on a developer machine

Do not infer support from laptop marketing names. Probe the actual runtime:

```sh
lspci -nnk | grep -A3 -Ei 'vga|3d|display'
vulkaninfo --summary
```

A future `wgpu` spike should print:

- adapter name;
- backend (Vulkan/GL/etc.);
- device type (integrated/discrete/software);
- limits/features used;
- whether timestamp queries are available.

Integrated GPUs can be attractive here because unified/shared memory reduces the
host-device transfer penalty, but this must be measured.

## Benchmark matrix

Compare **end-to-end**, not kernel-only time:

| case | CPU scalar | CPU parallel/SIMD | GPU compute |
|---|---:|---:|---:|
| 80x24 target, small basis | | | |
| 120x40 target, small basis | | | |
| 160x50 target, small basis | | | |
| 120x40, large calibrated basis | | | |
| batch of 60 source frames | | | |

Include:

- target upload/conversion;
- dispatch;
- GPU execution;
- readback/map;
- conversion into ordinary LibGibson cell choices;
- total wall time.

The GPU backend earns a place only if it wins a realistic workload after all of
those costs.

## First codebook to test

Do **not** begin with every Unicode character.

The supplied feasibility experiment found most practical image value in compact
structured bases:

1. Braille;
2. block / half-block / fractional block families;
3. box/legacy-computing geometry if calibration shows useful extra directions.

Arbitrary alphabetic glyphs substantially increase mathematical rank but gave
little natural-image benefit in the supplied experiment and create ugly
font-specific noise.

## Calibration boundary

A calibrated basis is terminal/font/size/rasterizer specific. Store its identity
with at least:

- font family;
- font size / cell pixel geometry;
- terminal emulator;
- glyph set;
- antialiasing/rasterization mode where knowable;
- calibration hash/version.

Never silently apply a glyph-pixel codebook to an unknown font.

## Success criterion

The GPU experiment succeeds only if:

1. its projection error matches the CPU reference within a documented tolerance;
2. total latency is lower for a realistic workload;
3. the output remains ordinary LibGibson cell/surface data;
4. CPU fallback is complete;
5. core LibGibson remains usable without GPU libraries.

The joke is welcome — a GPU-accelerated terminal rasterizer is absolutely a valid
thing to build. The measurement decides whether it is also a sensible thing to
ship.
