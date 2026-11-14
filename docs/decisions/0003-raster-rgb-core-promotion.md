# Decision 0003: promote the `raster` RGB subset to CORE in 0.7.1

| | |
|---|---|
| Status | **Accepted 2026-11-13; in effect from 0.7.1.** Release coordinator review PASS of #386 (the owner delegates to it). In 0.7.0 all of `raster` was EXPERIMENTAL. |
| Contract | [Release Contract §2](../RELEASE_CONTRACT.md#2-rust-api-stability-pre-10) |
| Guard | [`tests/raster_core_surface.rs`](../../tests/raster_core_surface.rs) |
| Follows | [`V0_7_QUALIFICATION.md`](../V0_7_QUALIFICATION.md) G4: if MINERVA needs more than best-effort stability, promote what it uses to CORE. The 0.7.0 tier review made no promotion and named `raster::{Rgb, RgbRaster}` as the first candidate. |
| Recorded | 2026-11-13 |

## Question

MINERVA (ProjectGHOST) depends on EXPERIMENTAL `raster`, `raster3d` and `ui` for its 1.0.0-rc.1.
Which part of `raster` can carry the CORE promise in 0.7.1, which is "no breaking change within a
`0.y` patch line", and what exactly does that promise cover?

## What the consumer uses

This was checked read-only against ProjectGHOST's `main`, and it agrees with the release
coordinator's earlier grep for decision 0002. No ProjectGHOST source is reproduced here.

- **Imports.** Its avatar and citadel modules (7 files) import only `gibson::raster::Rgb` and
  `gibson::raster::RgbRaster`. They use no glob import of `raster` and none of `FrameError`,
  `MAX_RASTER_DIMENSION`, `Rgba`, `RgbaRaster`, `composite_over`, `downscale_area` or `flatten`.
- **Calls.** Those files call `RgbRaster::new`, `width`, `height`, `pixels`, `pixels_mut`, `get`,
  `set`, `clear`, `line` and `to_surface`. They do not call `blend`, `disc`, `to_mono_surface`,
  `write_ppm` or `from_rgb_bytes`. These are name matches, so some `get`/`set`/`width` hits may be
  on other types. Either way, they are the methods the consumer's code depends on.
- **`Rgb` as a tuple.** The files build and destructure `Rgb` as a plain tuple many times. This
  decides the shape question below.

## API review

The surface as it stands at `v0.7.0` (`src/raster.rs`).

| Item | Shape | Review |
|---|---|---|
| `Rgb` | `pub type Rgb = (u8, u8, u8)` | Through the tuple it is `Copy`, `Eq`, `Ord`, `Hash`, `Default`, `Debug`, `Send` and `Sync`. Because it is an alias, it **cannot** gain named fields, an alpha channel or `#[non_exhaustive]` without breaking every tuple literal and destructuring pattern, and the consumer uses those heavily. **Decision: promote the alias as it is.** A future colour type would be a new item, not a change to `Rgb`. |
| `MAX_RASTER_DIMENSION` | `pub const: u16 = 2048` | Bounds both `new`, which clamps to it, and `from_rgb_bytes`, which rejects past it. Changing the value changes what both return, so **the value is part of the promise** within a `0.y` line. Raising it is a minor-version change. |
| `RgbRaster` | struct with private `width`, `height` and `pixels: Vec<Rgb>` | It derives `Debug`, `Clone`, `PartialEq` and `Eq`, and it is `Send` and `Sync`. Its fields are private, so it does **not** need `#[non_exhaustive]`. It has no `Hash` and no `Default`. Adding either later is additive, and neither is needed now. The invariant is `pixels.len() == width × height`. `pixels_mut` returns a slice, which cannot change that length, so the invariant holds. |
| `RgbRaster::new(w, h)` | clamps each axis to `MAX_RASTER_DIMENSION` and fills black | It allows zero (an empty raster) and never panics. Clamping differs from `from_rgb_bytes`, which rejects. Both behaviours are documented in the code, and the guard pins both. |
| `width`, `height`, `pixels`, `pixels_mut` | accessors | **Pixel order is row-major** (`y × width + x`). The promise includes it, because the slices expose it. |
| `get(x, y)` / `set(x, y, c)` | `i32` coordinates | Out of bounds, `get` returns `None` and `set` does nothing, for every `i32`, including `i32::MIN` and `i32::MAX`. Neither can panic. |
| `clear(c)` | fills every pixel | |
| `blend(x, y, c, amount)` | per-channel `old + (c − old)·t`, rounded | `t` is `amount` clamped to [0, 1]. A non-finite `amount` is ignored, and an out-of-bounds pixel does nothing. This is interpolation of the stored 8-bit values, not linear-light colour. The rounding (half away from zero) is pinned. |
| `line(x0, y0, x1, y1, c)` | clipped Bresenham, endpoints included | The line is clipped to the raster before stepping, so work is bounded by the raster size even for `i32` extremes. A zero-size raster is a no-op. |
| `disc(cx, cy, r, c)` | filled closed disc (distance ≤ r) | The iteration box is clipped. A non-finite centre or radius, or a negative radius, does nothing. |
| `to_surface()` | one `▀` cell per pixel column and pixel-row pair | The foreground is the top pixel and the background is the bottom one. With an odd height, the last row's background is black. Colour quantization stays with the renderer. The promise covers this mapping. |
| `write_ppm(writer)` | binary PPM: `P6\n{w} {h}\n255\n` followed by RGB bytes | It is a development dump and plays no part in terminal output. The format is standard, so promoting it costs nothing. |
| `from_rgb_bytes(w, h, stride, bytes)` | copies a padded packed-RGB buffer | It rejects bad input with `FrameError` and never clamps. It indexes only below `stride × (h − 1) + w × 3 ≤ bytes.len()`, so it cannot panic. |
| `FrameError` | `#[non_exhaustive]` enum, `Debug + Clone + PartialEq + Eq + Display + Error` | It is already non-exhaustive, so adding a variant is allowed. The variant names and fields (`ZeroDimension`, `TooLarge`, `StrideTooSmall`, `DimensionsOverflow`, `ShortBuffer`) are promised. It is promoted because `from_rgb_bytes` returns it. |

**Panics.** No public item in this subset panics on any input. The largest allocation is
2048 × 2048 pixels, about 12 MiB. Allocation failure aborts, as everywhere in Rust.

**Not promised.**
- The `Debug` text: deriving it prints every pixel.
- The exact float route inside `blend` and `disc`, beyond the pinned results.

**Determinism.** `blend` uses `f32` and `disc` uses `f64`. Both are basic IEEE operations, so the
results are identical on the supported Linux x86_64 target.

## Promotion in 0.7.1

Promote this subset of `raster` to CORE:
- `Rgb`;
- `MAX_RASTER_DIMENSION` (value 2048);
- `FrameError`;
- `RgbRaster` and all of its methods **except `to_mono_surface`**.

The promise covers the signatures, the trait set and the behaviour in the table above. The rest of
`raster` and its neighbours stay EXPERIMENTAL:

| Item | Why it stays EXPERIMENTAL |
|---|---|
| `RgbRaster::to_mono_surface` | Its output is Braille dot density. Glyph realization is a separate capability axis: the Linux VT cannot draw U+2800–U+28FF. Its Bayer threshold pattern has never been reviewed as a contract. No consumer uses it. |
| `Rgba`, `RgbaRaster`, `composite_over`, `downscale_area`, `flatten` | New in 0.7.0 with no external consumer yet. `Rgba` is an array alias (`[u8; 4]`) while `Rgb` is a tuple. That inconsistency should be decided before either shape is frozen. Straight versus premultiplied alpha is not settled either: `downscale_area` premultiplies internally. |
| `raster3d`, `rig`, `city` | Camera, projection and depth APIs are still changing (`Camera::basis`/`CameraBasis` arrived in 0.7.0). MINERVA imports `raster3d` in 2 files, so it is the next candidate, but it needs its own review. |
| `raster_fx` | Effect passes whose parameters are presentation tuning, not a contract. |
| `raycast` | Single-purpose for the engine path (node J). It has no general consumer. |
| `ui::image_area` | It belongs to `ui`, which stays EXPERIMENTAL as a whole. |

**Why 0.7.1 and not a minor.** The promotion adds a promise and changes no code, so it is a
compatible patch-level change under §1. A later breaking change to the promoted subset would
need the next `0.y` minor and a CHANGELOG entry, like any CORE change.

## Guard

`tests/raster_core_surface.rs` fails CI when the promoted subset changes:
- A changed parameter or return type stops the function-pointer coercions compiling.
- A dropped trait stops the trait-bound checks compiling.
- Changing `Rgb` from a tuple stops the tuple literal and destructuring compiling.
- The behaviour pins cover clamping, row-major layout, out-of-bounds no-ops, blend rounding and
  clamping, invalid-input no-ops, line clipping, the disc's closed boundary, half-block
  realization, the PPM bytes and every `FrameError` variant.

`write_ppm` takes `impl Write`, so it cannot coerce to a function pointer. A generic
call-shape helper pins it instead: any `W: Write` passed by value, returning
`io::Result<()>`. Its output bytes are pinned by behaviour. The guard does not pin
`to_mono_surface` or the RGBA items.

## Not decided here

- Whether to cut 0.7.1 at all, and when. That needs the owner's go, like every tag.
- Whether to add `Hash` or `Default` to `RgbRaster`. Either would be additive, and there is no
  current need.
- `raster3d`'s tier, which needs a separate review.
