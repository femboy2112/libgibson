# v0.7 baseline — node A (and the first node-B measurement)

**Status: measured.** This records what the merged v0.6 engine does before any v0.7 library
change, so every later v0.7 claim has an unchanged baseline to compare against. Numbers are
for the stated host and SHA only; none is a universal FPS claim.

## Code under measurement

| | |
|---|---|
| v0.6 integration merge | `f6cd8c7` (#253; parents `2ffdd92` + approved `eaa0d1c`) |
| #255 harness fix | `7f57047` (#258) — test harness only; library identical to `f6cd8c7` |
| v0.6.0 source bump | #261 — version strings and documents only |
| Library code measured | identical across all three (no `src/` change since `f6cd8c7`) |

## Installed-SDK forcing consumer (hosted CI, exact SHAs)

`bindings/sdk_forcing/doom_forcing_consumer.c`, built against a staged prefix through
`pkg-config` only (no private interface), in the CI Bindings job:

| SHA | Result |
|---|---|
| `f6cd8c7` ([42129557477](https://github.com/femboy2112/libgibson/actions/runs/42129557477)) | `doom_forcing_consumer: OK  frames=240 grid=64x8 cells_read=122880 long_glyph=25B ansi=8825B checksum=15734824182601092804` |
| `7f57047` ([42134572298](https://github.com/femboy2112/libgibson/actions/runs/42134572298)) | identical line, identical checksum |

`check-abi`: 59 baseline symbols = 59 built, at both SHAs. The consumer is unchanged, so its
output is unchanged; any later delta must name its commit.

## Raster ingest (node B) — `examples/raster_ingest_bench`

A deterministic moving 320×200 software framebuffer (gradient, drifting checker band, bouncing
disc, a hard edge sweeping 3 px/frame) is fitted onto each grid by a **consumer-side**
integer box filter, lowered to cells, rendered by the one renderer into a headless context, and
written to an **OS pipe drained by a thread — a pipe, not a terminal**: write times say nothing
about a terminal's parse/display cost or physical FPS.

- **Host:** AMD Ryzen 5 (8 cores), 7.2 GB RAM, Linux 7.0 x86_64, shared with other work (load
  average 3.3–3.7 during the run).
- **Build:** `--release`, rustc 1.97.0-nightly (2026-04-14), library at `f6cd8c7`.
- **Command:** `systemd-run --user --scope -p MemoryMax=512M taskset -c 0 raster_ingest_bench --frames 120`
  (one core, 512 MB cap). Wall time 11.5 s for all 21 configurations; peak RSS **6.7 MB**.
- **Correctness first** (`tests/raster_ingest_oracle.rs`, 8 tests): uniform colour fits to itself;
  an aligned checkerboard lands exactly at 80×25; a single lit corner pixel stays in its corner
  sample; **`fit_box` partitions the source** at every grid (lighting any one source pixel lights
  exactly one sample — driven through `fit_box` itself); its averaging equals a float route bit for
  bit (the rectangle bounds are shared, so that route checks the averaging only); a source circle
  stays round at 80×25 (±1 sample) and the stretch distortion at 120×40 / 200×60 is measured and
  bounded at 8%; and for **all 21 timed configurations** the ANSI bytes the renderer emits, parsed
  by a terminal model (`vt100`), show exactly the lowered cells — every glyph within its family's
  declared repertoire, and exact RGB in truecolor. One frame (42) per configuration is checked, not
  every timed frame.

Per frame, microseconds (p50 unless two values = p50/p95); bytes, dirty cells and allocations
are means over 120 frames:

| grid | lowering | depth | gen | fit | lower | render | write | bytes | dirty | allocs |
|---|---|---|---|---|---|---|---|---|---|---|
| 80x25 | halfblock-color | truecolor | 241 | 193 | 59 | 1166/1273 | 62/84 | 64251 | 1832 | 8537 |
| 80x25 | halfblock-color | 256 | 247 | 193 | 60 | 8217/8389 | 36/58 | 12823 | 1832 | 2127 |
| 80x25 | halfblock-color | 16 | 232 | 190 | 58 | 366/412 | 19/31 | 7289 | 1832 | 1122 |
| 80x25 | braille-mono | mono | 223 | 188 | 75 | 166/198 | 16/27 | 1759 | 262 | 301 |
| 80x25 | halfblock-mono | mono | 226 | 190 | 155 | 139/177 | 15/36 | 816 | 109 | 2034 |
| 80x25 | block-mono | mono | 225 | 189 | 158 | 152/181 | 15/26 | 1282 | 178 | 2112 |
| 80x25 | ascii-mono | mono | 231 | 191 | 152 | 149/175 | 13/28 | 1236 | 263 | 2184 |
| 120x40 | halfblock-color | truecolor | 251 | 351 | 148 | 3119/3396 | 110/126 | 153975 | 4444 | 20324 |
| 120x40 | halfblock-color | 256 | 190 | 311 | 111 | 14706/20952 | 48/93 | 26462 | 4444 | 3675 |
| 120x40 | halfblock-color | 16 | 167 | 305 | 104 | 661/868 | 27/50 | 17797 | 4444 | 2584 |
| 120x40 | braille-mono | mono | 158 | 301 | 138 | 264/338 | 10/25 | 3619 | 579 | 580 |
| 120x40 | halfblock-mono | mono | 160 | 301 | 266 | 240/362 | 12/31 | 1726 | 265 | 4800 |
| 120x40 | block-mono | mono | 159 | 302 | 268 | 244/332 | 10/27 | 2457 | 376 | 4916 |
| 120x40 | ascii-mono | mono | 165 | 303 | 260 | 254/409 | 14/36 | 2446 | 582 | 5079 |
| 200x60 | halfblock-color | truecolor | 170 | 669 | 267 | 4896/5672 | 162/228 | 337849 | 11015 | 45727 |
| 200x60 | halfblock-color | 256 | 176 | 662 | 262 | 35097/38484 | 67/115 | 52521 | 11015 | 5682 |
| 200x60 | halfblock-color | 16 | 174 | 682 | 272 | 1690/2228 | 47/77 | 41055 | 11015 | 4676 |
| 200x60 | braille-mono | mono | 173 | 685 | 363 | 695/915 | 23/48 | 7072 | 1229 | 1096 |
| 200x60 | halfblock-mono | mono | 181 | 695 | 715 | 682/884 | 28/51 | 3976 | 718 | 11826 |
| 200x60 | block-mono | mono | 176 | 692 | 703 | 679/937 | 26/47 | 5069 | 939 | 11993 |
| 200x60 | ascii-mono | mono | 162 | 642 | 625 | 597/848 | 22/43 | 4564 | 1235 | 12248 |

Stages: **gen** = producing the 320×200 frame; **fit** = consumer box filter; **lower** =
`to_surface` / `to_mono_surface` (+ glyph transcode); **render** = `Context::render` (layout,
paint, diff, ANSI); **write** = `write_all` + `flush` into the pipe. **Write is an upper bound,
not a clean split:** pinned to one core, the drain thread shares the CPU — a frame larger than the
64 KiB pipe buffer (truecolor at 120×40 and 200×60) blocks until the reader runs, so its copy lands
in `write`, and a smaller frame's drain cost can land in the next frame's stages. **Noise floor:**
every configuration does identical `gen` work, yet its p50 ranges 158–251 µs across rows — a
single run on this shared host resolves differences of roughly 1.6× at best.

### What the numbers say

1. **256-colour quantization dominates.** Render is 4.7–7.2× truecolor and 20–22× 16-colour on
   these dense, changing frames (35.1 ms p50 at 200×60 — more than a 35 Hz tic by itself) while
   emitting *fewer* bytes than truecolor. `nearest256` (`src/capability.rs:185`) scans 240
   palette entries per colour. Filed as **#260** with an exact-equivalence acceptance (an O(1)
   quantizer proven equal to the scan over all 2²⁴ inputs). Tier B; not changed here.
2. **Truecolor at 200×60 is 338 KB/frame** (≈11.8 MB/s at 35 Hz). The pipe absorbs it in
   0.16 ms; whether a real terminal can is node F's slow-reader PTY measurement, not this one.
3. **Allocation rate** is high on colour frames (≈46 k allocations/frame at 200×60 truecolor)
   and on the transcoded monochrome fallbacks (≈12 k vs ≈1.1 k for Braille, because the transcode
   allocates per cell). Recorded as baseline, not yet acted on.
4. **Every source pixel is re-fitted every frame** because the source moves everywhere; dirty
   cells ≈ 92% of the grid in colour. Dirty-rectangle ingestion (#129) only pays when a source
   changes locally.
5. **Aspect:** the stretch fit keeps square samples only where the grid divides the source
   evenly (80×25 here). A letterbox / aspect-preserving fit is the open #128 question; the bench
   does not pretend to answer it.

### Library question this answers (red-first rule)

No new public raster API is proposed yet. The consumer-side fit is 0.2–0.7 ms and exact; the
measured cost that matters is quantization (#260). An ABI raster-ingest symbol is gated on the
C forcing consumer actually needing to push a framebuffer (node I) — not on this bench.

## Reproduce

```text
cargo run --example raster_ingest_bench -- --selfcheck
cargo build --release --example raster_ingest_bench
taskset -c 0 target/release/examples/raster_ingest_bench --frames 120
cargo test --test raster_ingest_oracle
```
