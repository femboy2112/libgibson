# The Doom horizon — installed-SDK capability map for a bulk-raster / input consumer

RXIII Checkpoint C / Gate E. Reviewed 2026-11-05 against the v0.6 ABI v1 (59 C symbols,
`abi/gibson-abi-v1.symbols`, `GIBSON_ABI_VERSION 1`).

**Scope and boundary.** This is a capability map, **not an engine**. The question it answers before the
ABI freezes: *how far can a third party who installed the native SDK get toward a "Doom-class"
consumer* — a program that, every frame, reacts to input, renders a full raster, and pulls the whole
frame back out to blit it — and **where does the public C ABI stop?** Nothing here ports Doom; it
measures the ABI against that class of consumer and records the boundaries as deliberate, so they are
a reviewed contract rather than a surprise after promotion.

**RXIV status.** The forcing consumer is now a repeatable check — it runs in the CI Bindings job
(`scripts/dev/sdk_forcing_consumer.sh`) and in the release cleanroom (`scripts/release/cleanroom.sh`,
hence Release Preflight) — and a real-TTY closed loop through the ABI is demonstrated (table row
"real-terminal input → state → render"). Neither changes the two boundaries below.

Every claim below is grounded in a consumer built against the SDK **staged into a prefix outside the
checkout** (`scripts/dev/sdk_forcing_consumer.sh` → `scripts/release/stage-sdk.sh`, compiled with
`cc $(pkg-config --cflags --libs libgibson)`, no repo include/lib paths) —
`bindings/sdk_forcing/doom_forcing_consumer.c`. Its measured run:

```
doom_forcing_consumer: OK  frames=240 grid=64x8 cells_read=122880 long_glyph=25B ansi=8825B
```

## What the installed SDK proves (the output / readback path)

The forcing consumer ran a 240-frame loop and, each frame, pulled the **entire** 64×8 resolved cell
grid out through the bulk path (122,880 cells total), self-checking invariants a smoke test never
would. All held, from the installed prefix:

| Doom-class need | ABI it rests on | Status | Evidence (in the forcing consumer) |
|---|---|---|---|
| a framebuffer you can pull every frame | `gibson_create_headless_context` (FULLSCREEN = fixed `cols×rows`), `gibson_render`, `gibson_get_frame_cells` | **SUPPORTED** | one bulk call returns all `cols×rows` cells; `written == grid` every frame |
| authoritative per-cell readback | `gibson_get_frame_cell`, `gibson_cell_t` (glyph, width, continuation, transparent, fg/bg, bold/dim/italic/underline/reverse) | **SUPPORTED** | bulk cells are **byte-identical** to single-cell readback on a stride sample |
| input → render → readback closes | `gibson_set_root_node` + `gibson_render` + readback | **SUPPORTED** | a marker driven by the frame index lands at exactly the demanded raster column |
| full grapheme for complex glyphs | `gibson_get_frame_cell_glyph` (size-then-copy), `GIBSON_CELL_GLYPH_CAP` | **SUPPORTED** | a 25-byte ZWJ family emoji (one cell) recovered in full past the 16-byte inline cap |
| mid-session color-depth adaptation | `gibson_set_color_depth` + re-render + readback | **SUPPORTED** | the same styled cell resolves to a **different** style at TRUECOLOR vs MONO |
| ANSI frame bytes to blit to a terminal | `gibson_get_rendered_output` | **SUPPORTED** | 8825 bytes of ANSI at truecolor; nonzero at every depth |
| real-terminal input → state → render (a game loop) | `gibson_create_context` + `gibson_poll_event` + `gibson_set_root_node` + `gibson_render` | **SUPPORTED on a TTY context** (RXIV) | `ffi_realtty_probe abi-loop`: Right arrows decoded by the ABI advance state that is re-rendered; the painted frame is verified on a vt100 parse (`tests/ffi_realtty_pty.rs`) |
| deterministic synthetic input into a headless frame | — | **NOT SUPPORTED** | boundary 1 below |
| sub-cell pixel raster | — | **NOT SUPPORTED** | boundary 2 below |

The output path of a text-mode (cell-grid) Doom-class consumer is therefore **viable on the shipped
SDK, from an installed prefix** — not merely on the repo-relative parity harnesses.

## The two real boundaries

### 1. No input into the deterministic framebuffer (headless is output-only)

`gibson_poll_event(ctx, timeout_ms, &event)` exists (events `KEY` / `PASTE` / `RESIZE`, with
`GIBSON_KEY_*` codes), but it reads a **terminal**. On a **headless** context — the deterministic,
TTY-independent framebuffer the raster path needs — it returns `GIBSON_ERR_IO` (−2), because there is
no terminal lease to read from. Verified directly: `headless poll_event → status=-2 event_type=0`.

There is **no ABI function to inject a synthetic event into a headless context** (no
`gibson_push_event` / feed). So a consumer that wants a *deterministic, testable* game loop must
synthesize input **outside** the engine and translate it to scene changes itself — which is exactly
what the forcing consumer does (frame index → marker column → node tree). Live terminal input
requires a **TTY** context (`gibson_create_context`), where `poll_event` works but output goes to the
live terminal rather than an in-memory frame the consumer owns.

**This is a boundary, not a defect.** The split is coherent: *headless = output you control; TTY =
input from a real terminal.* It is recorded here so a consumer author knows to own their input loop,
and so a future ABI decision is made deliberately.

### 2. No sub-cell pixel raster (the cell grid is the finest unit)

The readback unit is a **styled cell** (`gibson_cell_t`): one grapheme cluster plus style, at a
`(col, row)` grid position. There is no RGB-pixel framebuffer and no sub-cell addressing. A
**text-mode** Doom (ASCII/half-block raster over the cell grid) is in scope; a **pixel-accurate**
software renderer is not. This is the standing, deliberate cede — LibGibson is a cell/glyph
compositor, not a pixel framebuffer — restated here for the ABI-freeze record.

## Recommendations before the ABI v1 freeze (consumer-gated, no change now)

- **Keep the 59-symbol surface as is.** No Doom-class consumer exists today, so none of the below is
  justified yet (same consumer-gate as the deferred media work, `V06_CARRIED_FORWARD_PR_REVIEW.md`).
- If an interactive, deterministically-testable consumer is ever funded, the one additive symbol
  worth it is a **headless event-injection** entry point (e.g. `gibson_push_event`), which would let
  a game loop feed synthetic input through the ABI instead of mapping it by hand — and make such a
  consumer testable end-to-end without a real TTY. Additive, so ABI-v1-compatible.
- **Document the headless/TTY input split** in `gibson.h` at `gibson_poll_event` so the `GIBSON_ERR_IO`
  on a headless context is a contract, not a surprise.
- `gibson_cell_t`'s layout is already a committed v1 struct (reserved bytes + truncation flag); the
  bulk readback and long-glyph escape hatch are proven and should freeze as-is.

## Verdict

The public SDK is a **viable output target for a text-mode bulk-raster consumer**, proven by an
installed-prefix forcing consumer that drove the full Gate I/J/K readback surface (headless render →
whole-frame bulk readback → per-cell style → long-glyph recovery → mid-session depth restyle → ANSI
blit) over 240 frames. The two boundaries — **no input into a headless context** and **no sub-cell
pixel raster** — are named, evidenced, and deliberate. No ABI change is recommended for v0.6; this
record exists so the freeze is taken with the Doom horizon mapped rather than assumed.

> **v0.7 addendum (2026-11-06).** The first boundary is closed on the v0.7 line:
> `gibson_push_event` queues key, paste and resize events on a headless context, and
> `gibson_poll_event` delivers them (ABI v1, additive, 59 → 60 symbols). The installed-SDK forcing
> consumer now moves its marker only by pushed-and-polled input, with identical frames. The record
> above stays as written for v0.6.0. *Update 2026-11-07:* 62 symbols on `main` since #283
> (`da2be76`) added `gibson_poll_event_kind` / `gibson_push_event_kind` (#274).

> **Node J design note (2026-11-06, RXVIII). Investigation only: nothing is built, and no
> repository is created.** v0.7 node J is a Doom-compatible adapter. It **cannot live in this
> repository**: the engines are GPL-2.0 and LibGibson is MIT OR Apache-2.0. It must be a separate
> GPL-2.0 repository that consumes only the **installed** SDK through `pkg-config libgibson`, the
> way the node I forcing consumer does. Creating and publishing that repository needs the owner's
> authorization, so it is not started.
>
> The architecture the shipped ABI already supports:
>
> | Engine hook (doomgeneric-style) | LibGibson ABI | Status |
> |---|---|---|
> | Draw a 320×200 frame | Lower the frame to half-block cells and set them as styled spans on a headless or terminal context, as node I does for a 48×16 raster (`gibson_set_root_node`, `gibson_render`) | Proven shape (node I, 23,040 cells at ~225 µs/frame). A full 320×200 frame at the terminal's cell resolution has not been measured. |
> | Read keys | `gibson_poll_event_kind` (#274, after #283 merges) on a terminal context; the adapter maps `GIBSON_KEY_*` and typed characters to engine key codes | ABI exists. A press-only terminal sends no key-up, so the adapter must synthesize releases with a hold grace, the trade-off measured in #275 (25 tics bridged both measured autorepeat delays). |
> | Ticks / sleep | the engine's own clock (libc) | No LibGibson surface needed |
> | Resize | `GIBSON_EVENT_RESIZE` | ABI exists |
> | Terminal hangup | `gibson_poll_event*` returns `GIBSON_ERR_IO` once the terminal is gone (#269, after #282 merges) | The adapter must exit on it rather than retry |
>
> **Data:** game data is never vendored. The adapter must refuse to start without a user-supplied
> IWAD (Freedoom is a free choice). **Licensing:** the adapter repository carries GPL-2.0 and the
> engine's notices. Linking LibGibson (MIT OR Apache-2.0) into a GPL-2.0 program is compatible in
> that direction; LibGibson's own license is unaffected.
>
> **The first open question** is whether a 320×200 frame lowered to half-blocks fits a 35 Hz frame
> budget at common terminal sizes. That measurement belongs to the adapter's own CI.

> *Update 2026-11-07 (RXIX).* #283 (`da2be76`) and #282 (`04c0ad2`) are merged. Plain
> `gibson_poll_event` still reports a typed U+1001–U+100D and the `GIBSON_KEY_*` of equal value
> identically (a documented compatibility property; #274 is addressed by the `_kind` pair, not
> removed). #275's 25 tics is a compromise measured on xterm under Xvfb/XTEST, not key-release
> knowledge. A kitty-protocol key release is delivered as a press (#285, open; fix in review,
> #289). Node J is not started in this repository as of `5790f1a`.
