# Temporal display validation

**Status:** experimental 0.3.0 research. This document records what has and has
**not** been validated for the temporal cell realization feature, with every
claim labeled by evidence class. It is deliberately conservative.

## Evidence classes

Claims below are tagged:

- **MEASURED** — deterministic software measurement on the machine named here.
- **SIMULATION** — a deterministic model of presentation, not a real panel.
- **MODEL** — analytic/first-principles inference (incl. the external Fable study).
- **HUMAN** — a physical/perceptual observation. **None collected yet** (see the
  experiment below); no perceptual gain is claimed by this milestone.

## Environment (development machine)

- CPU: AMD Ryzen 5 (8 threads). OS: Linux 7.0.0-28-generic.
- GPU: AMD Radeon 610M integrated (`radeonsi`/`amdgpu`), Vulkan 1.3.275 available,
  OpenGL 4.6. [MEASURED — probe]
- Rust: stable toolchain; MSRV 1.85. C ABI: `GIBSON_ABI_VERSION = 1` (unchanged).
- Terminal emulator: **setup-specific — the physical experiment below records it.**

## What is validated

### Static projector [MEASURED]
- Exact two-color fit over 127 complement-pair representatives; a slow 254-mask
  oracle test confirms the fit is bit-identical over 2000 random tiles.
- Reported `emitted_static_rmse` uses the actual 8-bit sRGB colors emitted and is
  provably `>= static_rmse` (the ideal, pre-quantization floor).
- Throughput: a full 160×50 projection (8000 cells) is ~28.6 ms release (~507 ms
  debug); ~7 ms for 80×24. This is a one-time per-target cost; the per-frame
  residual step is much cheaper.

### Residual modulation & robustness [MEASURED + SIMULATION]
- Residual sigma-delta over the static baseline converges to the target duty and
  holds each dot at its static value most frames (bounded correction). [MEASURED]
- **SIMULATION:** under a coherent 2:1 presentation subsample at 50% duty, plain
  first-order sigma-delta aliases to a ~0.5 DC bias; the dithered scheme stays
  within 0.1 of the true level. This is a deterministic model of a dropped-frame
  presentation, **not** a measurement of any real terminal.

### Safety controller [MEASURED]
- The processor computes the emitted linear-light luminance swing `|Y(fg)−Y(bg)|`
  (Rec.709) from the actual cell colors and freezes any cell exceeding the depth
  cap (default 10%) or whose colors have unknown luminance. A white/black cell
  (ΔY≈1.0) is frozen and verified to never flip across 200 frames, dither on.
- Default is static: unmeasured profile, cadence < 100 Hz, survival < 90%,
  reduced-motion, or content-motion all render the static fallback. A missed
  phase-locked deadline trips a 30-frame hysteresis hold.

### Composition [MEASURED]
- The processor emits ordinary `Surface` frames that compose through the existing
  layout/paint pipeline; a viewport clips them with no leak past the window
  (exact-cell test). There is one renderer.

### GPU [UNMEASURED]
- A Vulkan-capable iGPU exists, but no `wgpu` backend was built or benchmarked.
  No GPU-vs-CPU claim is made. See `research/temporal_gpu/README.md`.

## What is NOT validated [HUMAN — pending]

- Whether temporal residual modulation is **perceptually** better than the best
  static frame on any real terminal/display. Software cannot measure this.
- Any specific invisible/acceptable modulation depth on a real panel.
- Spatial super-resolution: **not claimed.** Time yields only convex combinations
  of glyph rasters (MODEL); it cannot synthesize spatial detail no glyph contains.

## The physical experiment (to complete the perceptual gate)

LibGibson cannot observe presentation cadence through a PTY. To gather the missing
HUMAN/physical evidence, run this **one** bounded experiment and report the result.

**Stage A — cadence.** In your target terminal (record its name/version and
whether a compositor/tmux/SSH is involved):

```sh
cargo run --release --example temporal_cadence_beacon -- \
    --fps=120 --seconds=8 --photodiode --csv=/tmp/beacon.csv
```

Film the on-screen region at ≥240 fps (phone slow-motion is fine) and/or aim a
photodiode at the inverting patch. From the capture, recover: the number of
**distinct** presented frames per second (presentation Hz), the fraction of the
120 emitted frames that appear as distinct presentations (survival), and the
inter-presentation jitter. Correlate against `/tmp/beacon.csv` (intended deadline
vs actual emit vs missed periods). Repeat with `--no-sync` to compare synchronized
updates on/off.

**Stage B — perceptual comparison** (only if Stage A shows a stable cadence
≥ ~100 Hz with ≥ 90% survival):

```sh
cargo run --release --example temporal_display_lab -- \
    --temporal --measured-hz=<measured presentation Hz> --survival=<measured>
```

Compare the STATIC and TEMPORAL panels at equal mean luminance. Report: is the
temporal panel visibly higher-fidelity, and is any flicker/shimmer objectionable?
A forced-choice depth staircase (vary `--depth-cap`) gives a threshold.

**Report back:** terminal + version, sync on/off, measured presentation Hz /
survival / jitter, and the A/B perceptual verdict with comfort notes. Development
continues from that receipt; no cadence or perceptual number will be fabricated in
its absence.

Decision heuristics (from the Fable study, MODEL): invisible depth ≥ ~10% at a
measured stable high cadence warrants deeper work; < ~5% means the luminance path
is not worth pursuing on that setup; 5–10% is niche gradient/portrait territory.

## Accessibility & safety contract

These are **engineering guardrails, not medical certification.**

- Temporal modulation is **off by default**; the default and every fallback is the
  static realization.
- Static is forced on: unmeasured presentation, cadence below the policy minimum,
  frame survival below the policy minimum, reduced-motion preference, known content
  motion, a per-cell emitted luminance swing above the depth cap, or unknown cell
  luminance.
- A missed phase-locked deadline trips a hysteresis hold before temporal may resume,
  so timing degradation cannot cause rapid temporal/static toggling.
- No saturated-red temporal modulation is used in these experiments.
- Callers integrating this must keep the region bounded and provide their own
  user-facing warning; the static fallback is always available and equal in mean.

## Honest conclusion

LibGibson 0.3.0 can truthfully claim: high-cadence phase-locked emission pacing,
split generation/write frame diagnostics, an exact static two-color subcell
projector, an experimental residual temporal modulator with a conservative
emitted-swing safety controller and automatic static fallback, and external
cadence-measurement tooling — all flowing through the one existing renderer.

It does **not** claim a proven perceptual improvement or spatial super-resolution.
The perceptual value of temporal modulation remains setup-dependent and unproven
pending the experiment above. On a typical 60 Hz path the feature automatically
renders static.
