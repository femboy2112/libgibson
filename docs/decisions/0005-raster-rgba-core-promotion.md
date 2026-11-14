# Decision 0005: promote the `raster` RGBA compositing subset to CORE in 0.7.2

| | |
|---|---|
| Status | **Proposed 2026-11-14.** Release coordinator approval to prepare it (ASC-6). It takes effect only if 0.7.2 ships with it, after the consumer gate below. In 0.7.0 and 0.7.1 every RGBA item was EXPERIMENTAL ([decision 0003](0003-raster-rgb-core-promotion.md)). |
| Contract | [Release Contract §2](../RELEASE_CONTRACT.md#2-rust-api-stability-pre-10) |
| Guard | [`tests/raster_rgba_core_surface.rs`](../../tests/raster_rgba_core_surface.rs) |
| Follows | Decision 0003 kept `Rgba`, `RgbaRaster`, `composite_over`, `downscale_area` and `flatten` EXPERIMENTAL because they had "no external consumer yet", the `Rgba` array shape was undecided, and straight versus premultiplied alpha was "not settled". This decision answers all three for the items a consumer now calls. |
| Gate | Ships only after MINERVA confirms the call set (done, 2026-11-14) **and** its PTY suite passes against the 0.7.2 candidate. The release then follows [decision 0004](0004-release-naming.md) and the roadmap completion audit. |
| Recorded | 2026-11-14 |

## Question

MINERVA's avatar work (E-AV) will layer straight-alpha sprite art over an opaque panel and lower
one raster per frame. Which RGBA items can carry the CORE promise ("no breaking change within a
`0.y` patch line"), what exactly does that promise cover, and does the `Rgba` shape have to change
first?

## What the consumer uses

MINERVA supplied this list and confirmed it on 2026-11-14 after a read-only grep of its `main`
against the v0.7.1 source. No ProjectGHOST source is reproduced here.

- **Today:** no RGBA API at all. Its seven avatar and citadel files use only the CORE
  `Rgb`/`RgbRaster` subset of decision 0003. It hand-rolls a straight-alpha "over" that it
  verified is bit-identical to `composite_over`, including both fast paths.
- **E-AV will call:**
  - `Rgba`;
  - `RgbaRaster::from_rgba_bytes(w, h, w·4, bytes)` to ingest sprite bytes. Its synthetic test
    layers use the same path;
  - `RgbaRaster::new(w, h)`, only as a fully transparent "no layer" placeholder (nothing writes
    into it);
  - `width`, `height`, `get`, `pixels`;
  - `composite_over(&mut RgbRaster, &RgbaRaster, x, y)`, its core call. Layers are stacked base →
    blink → mouth → gaze → outfit over a panel cleared to the background, sometimes at a negative
    offset (a nod);
  - `flatten(&RgbaRaster, Rgb)`, optionally, for a single-layer tier.
- **Not called:** `downscale_area`, since its art ships pre-baked sizes and is never resampled at
  runtime, and `RgbaRaster::{set, pixels_mut, from_pixels, from_rgb}`.
- **What it relies on:** straight alpha end to end; `composite_over`'s exact formula, rounding
  and fast paths; and that only the overlap is touched. Its own test pins those byte for byte.

## API review

The surface as it stands at main `44721fa` (`src/raster/rgba.rs`, re-exported from `raster`).

| Item | Shape | Review |
|---|---|---|
| `Rgba` | `pub type Rgba = [u8; 4]`, `[r, g, b, a]` | Through the array it is `Copy`, `Eq`, `Ord`, `Hash`, `Default`, `Debug`, `Send` and `Sync`. Like `Rgb`, it is an alias, so it cannot change shape without breaking every literal, index and pattern. **Decision: promote it as it is.** See "Shape" below. |
| `RgbaRaster` | struct with private `width`, `height`, `pixels: Vec<Rgba>` | It derives `Debug`, `Clone`, `PartialEq` and `Eq`, and it is `Send` and `Sync`. Its fields are private, so it does not need `#[non_exhaustive]`. The invariant is `pixels.len() == width × height`. |
| `RgbaRaster::new(w, h)` | clamps each axis to `MAX_RASTER_DIMENSION`, every pixel `[0, 0, 0, 0]` | It allows zero (an empty raster) and never panics. It clamps, like `RgbRaster::new`. |
| `from_rgba_bytes(w, h, stride, bytes)` | copies a padded packed straight-RGBA buffer | It **rejects** bad input with `FrameError` and never clamps, exactly as `RgbRaster::from_rgb_bytes` does: `ZeroDimension`, `TooLarge`, `StrideTooSmall` (minimum `w × 4`), `DimensionsOverflow` (`stride × h`), `ShortBuffer`. It copies row-major, and bytes past `w × 4` in a row are ignored. It indexes only below `stride × (h − 1) + w × 4 ≤ bytes.len()`, so it cannot panic. |
| `width`, `height`, `pixels` | accessors | **Pixel order is row-major** (`y × width + x`). The promise includes it, because `pixels` exposes it. |
| `get(x, y)` | `i32` coordinates | Out of bounds it returns `None`, for every `i32`, including `i32::MIN` and `i32::MAX`. It cannot panic. |
| `composite_over(dst, src, x, y)` | straight-alpha "over" of `src` onto an opaque `RgbRaster` at `(x, y)` | **The formula is the promise:** `out = (src·a + dst·(255 − a) + 127) / 255` per channel, in integers, which is exact round-to-nearest of `(src·a + dst·(255 − a)) / 255`. 255 is odd, so no input lands exactly halfway, and the numerator (at most 255·255 + 127) cannot overflow. **Fast paths:** `a = 0` leaves `dst` untouched and `a = 255` replaces it. Both agree with the formula, and both are promised. **Clipping:** only the overlap is touched. Negative and past-edge offsets are legal, a fully outside offset (including `i32` extremes) is a no-op, and an empty source or destination is a no-op. The offset maths is in `i64`, so it cannot overflow. |
| `flatten(src, bg)` | a new `RgbRaster` the size of `src`, cleared to `bg`, then `composite_over` at `(0, 0)` | It is defined as that composition, and the guard pins the equivalence. |

**Panics.** No promoted item panics on any input. The largest allocation is
2048 × 2048 × 4 bytes (16 MiB) for an `RgbaRaster`, plus 12 MiB for a `flatten` result.
Allocation failure aborts, as everywhere in Rust.

**Determinism.** Every promoted operation is integer-only, so results are identical on every
host.

**Not promised.** The `Debug` text, which prints every pixel.

## Alpha semantics: settled

Decision 0003 said straight versus premultiplied was "not settled". In behaviour it already is,
and this decision records it:

- **Every promoted item is straight (non-premultiplied) alpha in and out.** `Rgba` is documented
  that way, `from_rgba_bytes` stores what it is given, and `composite_over` treats `src` as
  straight and writes an opaque result.
- `downscale_area` premultiplies **internally** so soft edges don't fringe, and returns straight
  alpha. It is not promoted (no consumer), so its internals stay free to change. Promoting it
  later would mean pinning its filter and its rule that a fully transparent pixel is
  `[0, 0, 0, 0]`.

## Shape: keep `[u8; 4]`

`Rgba` is an array and `Rgb` is a tuple. That inconsistency is real, but changing either one is a
breaking change:
- **Keeping the current shape** makes this a promise with no code change. It is a compatible,
  patch-level change under §1 and can ship in **0.7.2**.
- **Switching `Rgba` to a tuple** (or `Rgb` to an array) breaks every literal, index and pattern.
  It would need **0.8.0**, and it would change the consumer's code (MINERVA's ingest).

Changing the shape would buy consistency and nothing else. No consumer asked for it (MINERVA is
"indifferent"), and correctness doesn't need it, since both shapes carry the same bytes. The
release coordinator's rule for this decision is that a shape change needs a concrete consumer or
correctness reason, not tidiness. **Decision: keep `Rgba = [u8; 4]`.** A later unified colour type
would be a new item, not a change to either alias.

## Promotion in 0.7.2

Promote to CORE:
- `Rgba`;
- `RgbaRaster` with `new`, `from_rgba_bytes`, `width`, `height`, `get` and `pixels`;
- `composite_over`;
- `flatten`.

The promise covers the signatures, the trait sets and the behaviour in the table above. These
stay EXPERIMENTAL:

| Item | Why it stays EXPERIMENTAL |
|---|---|
| `RgbaRaster::set`, `pixels_mut` | No consumer writes into an `RgbaRaster`; MINERVA builds layers through `from_rgba_bytes`. Promoting them later is additive. |
| `RgbaRaster::from_pixels`, `from_rgb` | No consumer. `from_pixels` also returns its own error shape (`(FrameError, Vec<Rgba>)`), which hasn't been reviewed as a contract. |
| `downscale_area` | No consumer: MINERVA's art is never resampled at runtime. Its filter and premultiplied internals have not been reviewed as a contract. |
| `ui::image_area` | It belongs to `ui`, which stays EXPERIMENTAL as a whole. |

**Why 0.7.2 and not a minor.** The promotion adds a promise and changes no code, so it is a
compatible patch-level change under §1. A later breaking change to the promoted subset would
need the next `0.y` minor and a CHANGELOG entry, like any CORE change.

## Guard

`tests/raster_rgba_core_surface.rs` fails CI when the promoted subset changes:
- a changed parameter or return type stops the function-pointer coercions compiling;
- a dropped trait stops the trait-bound checks compiling;
- changing `Rgba` from `[u8; 4]` stops the array literal, index and pattern compiling;
- the behaviour pins:
  - `new` being transparent and clamped;
  - row-major ingest that ignores stride padding;
  - every `FrameError` rejection;
  - out-of-bounds `get`;
  - `composite_over`'s exact values, including the rounding boundary (`1·128/255 → 1`,
    `1·127/255 → 0`), independent channels and both fast paths;
  - clipping for negative, past-edge, fully outside, `i32`-extreme and empty cases;
  - `flatten` ≡ clear + `composite_over`.

A red check proves the guard bites. A mutant that truncates instead of rounding (`v / 255` in
place of `(v + 127) / 255`) must fail the guard in CI before this decision is accepted.

## Not decided here

- Whether to cut 0.7.2, and when. That needs the release coordinator's GO-TAG, like every tag.
- `downscale_area`, `set`, `pixels_mut`, `from_pixels` and `from_rgb`. Each needs a consumer first.
- A unified colour type.
- An RGBA-onto-RGBA composite. No consumer needs one: MINERVA composites onto an opaque base.
