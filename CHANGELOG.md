# Changelog

All notable changes to LibGibson are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project follows
the 0.x Semantic Versioning rules defined in
[`docs/RELEASE_CONTRACT.md`](docs/RELEASE_CONTRACT.md) — pre-1.0, a breaking change
bumps the minor slot (`0.y`) and a compatible change bumps the patch slot (`0.y.z`).
The project may also deliberately elect a minor bump to mark a substantial additive
public subsystem or milestone (see the contract's "deliberate minor milestones"); 0.2.0
is such a milestone.

LibGibson is **engineering alpha**. Version 0.1.0 was released on GitHub on
2026-09-25 (Linux x86_64: source plus a native SDK archive). It is **not** published
to any package registry — no crates.io, PyPI, or Go module proxy upload.

## [Unreleased]

Development toward **0.7.0**: a playable terminal software-graphics engine
([`docs/V07_EXECUTION_GRAPH.md`](docs/V07_EXECUTION_GRAPH.md)).

## [0.7.0-alpha.6] — 2026-11-13 · GitHub prerelease from `main`

A sixth **consumer alpha** of the 0.7 line, cut from `main`. It is **not** a qualified 0.7.0; the
open v0.7 gates listed under 0.7.0-alpha.1 are unchanged. Linux x86_64 only, GitHub-only (no
registry upload, no Go module tag). `GIBSON_ABI_VERSION` stays 1 (67 symbols, unchanged); MSRV
stays 1.85. No public API change.

- **#375**: screen, cursor and renderer recovery after a caught owner-thread panic (#328).
- **#374**: termination-signal docs (#268). The host owns SIGTERM and SIGINT.

### Fixed — screen and cursor after a caught owner-thread panic (#328)

- The panic hook leaves the alternate screen, shows the cursor and resets SGR. If the owner
  catches the panic and keeps going, the next `Context` render, commit, insert or clear now:
  - re-enters the alternate screen (fullscreen);
  - hides the cursor again;
  - repaints the frame in full instead of diffing against the pre-panic screen.

  Before this, the session kept its old screen and cursor flags, so the frame was diffed onto
  the main screen with the cursor showing. Input recovery after the same panic is #329. No
  public API change. PTY test: `tests/caught_panic_screen_pty.rs`.

### Documentation — termination signals are the host's (#268)

- [`docs/TERMINAL_LEASE_CONTRACT.md`](docs/TERMINAL_LEASE_CONTRACT.md) now states that LibGibson
  installs no SIGTERM/SIGINT/SIGQUIT/SIGHUP handler, and gives the recommended host pattern:
  - set an atomic flag in the handler;
  - check it every iteration of a bounded `run_once` wait (a signal does not shorten the wait);
  - restore from the loop;
  - optionally re-raise.

  It keeps SIGHUP at its default action and leaves the loop on any poll error. The crate docs
  point there. No code change.

## [0.7.0-alpha.5] — 2026-11-13 · GitHub prerelease from `main`

A fifth **consumer alpha** of the 0.7 line, cut from `main`. It is **not** a qualified 0.7.0; the
open v0.7 gates listed under 0.7.0-alpha.1 are unchanged. Linux x86_64 only, GitHub-only (no
registry upload, no Go module tag). `GIBSON_ABI_VERSION` stays 1 (67 symbols, unchanged); MSRV
stays 1.85. The Blades styling below is Rust-only and experimental. The #371 fix changes Linux
behaviour for managed sessions: managed output and size follow the fd acquired at lease time.

### Fixed — a managed session writes, and measures, the terminal it acquired (Linux, #371)

- A `TerminalSession` that wins the terminal lease holds its own close-on-exec duplicate of
  fd 1. `Context` frames, mode changes, restore bytes and the panic hook all write it, so
  redirecting stdout during a session no longer splits frames between two destinations or
  leaves the acquired terminal unrestored. The descriptor closes exactly once on restore/Drop,
  and children spawned during a lease do not inherit it.
- `TerminalSession::terminal_size()` and delivered resize events read that descriptor's size,
  not crossterm's `/dev/tty`-else-stdout, so both describe the acquired terminal.
- **Behaviour change:** an application's own `io::stdout()` writes during a lease follow fd 1
  as it is now, while LibGibson's stay on the acquired terminal. See
  [`docs/TERMINAL_LEASE_CONTRACT.md`](docs/TERMINAL_LEASE_CONTRACT.md).
- No public API change; non-TTY stdout behaves as before. The mechanism and tests come from
  the draft #338 → #339 → #343 stack and #348, without #339's `bind_target` API change.

### Added — Blades in a consumer's colours (experimental, Rust only)

- **`ui::experience::BladesStyle`**, **`Blades::with_style`** and **`Blades::with_skin`** (#369).
  - `with_skin` derives the colours every frame at the environment's depth, so a skin's
    explicit `ansi16`/`ansi256` palette applies there.
  - `BladesStyle::from_palette` / `from_skin` tint plane mid-tones from the skin's `surface`
    toward `accent` and `rail`, and draw text, title face, rim and selection from `text`,
    `text_muted` and `accent`. Every field can be overridden.
  - `Blades::new()` keeps the procedural look byte-for-byte (golden hashes recorded before
    the change). Stack position, numbered strip labels, per-blade patterns and the rim
    still identify each blade under any palette.

## [0.7.0-alpha.4] — 2026-11-13 · GitHub prerelease from `main`

A fourth **consumer alpha** of the 0.7 line, cut from `main`. It is **not** a qualified 0.7.0; the
open v0.7 gates listed under 0.7.0-alpha.1 are unchanged. Linux x86_64 only, GitHub-only (no
registry upload, no Go module tag). `GIBSON_ABI_VERSION` stays 1 (67 symbols, unchanged); MSRV
stays 1.85. The additions below are Rust-only and experimental.

### Added — experience-tier consumer contract (experimental, Rust only)

- **`ui::experience::Destination::unavailable(reason)`**: the destination stays reachable but is
  never live in any grammar. Its content becomes `Unavailable: <reason>`, with no items and no
  actions. The reason is reported in the new `RequiredSemantics::unavailable`.
- **`ui::experience::handle_key_outcome`** and **`ExperienceRuntime::handle_key_outcome`** return
  `KeyOutcome::{Action, Consumed, Unhandled}`.
  - Keys a grammar does not bind, and `Intent::Back` (Esc and Backspace, which the experience,
    having no history, never acts on), come back to the application as `Unhandled`.
  - `handle_key` is unchanged.
- **`PresentationState::select(key)`**: select an item by its stable id.
- **`ui::experience::Fallback`**: presents a spatial grammar (for example Blades) only with more
  than 40 columns, colour and full motion, and otherwise presents `Standard`.
- **Consumer acceptance E1–E8**: [`docs/EXPERIENCE_ACCEPTANCE.md`](docs/EXPERIENCE_ACCEPTANCE.md)
  and `tests/experience_minerva.rs`.

### Added — RGBA rasters and area filtering (experimental, Rust only)

- **`raster::RgbaRaster`** (straight alpha) and `raster::Rgba`.
  - Constructors: `from_rgba_bytes` (validated like `RgbRaster::from_rgb_bytes`), `from_pixels`
    (takes ownership, no copy) and `from_rgb`.
- **`raster::composite_over(&mut RgbRaster, &RgbaRaster, x, y)`**: exact integer "over"
  compositing.
  - Negative and out-of-range offsets are clipped, and only the overlap is touched.
  - **`raster::flatten`** composites onto a solid background.
- **`raster::downscale_area(&RgbaRaster, w, h)`**: an exact area (box) filter with fractional edge
  coverage.
  - Colour is averaged premultiplied by alpha, so soft alpha edges never fringe toward the colour
    of transparent pixels. A fully transparent result is `[0, 0, 0, 0]`.
  - All integer arithmetic, so the result is identical on every host.
- **`ui::image_area`**: an opt-in `ui::image` that fits with the area filter instead of
  nearest-pixel sampling. Painted art doesn't alias; the cost is reading every source pixel once.

### Added — camera rigs and city helpers (experimental, Rust only)

- **`gibson::rig`**: `Dolly` (from → to, looking at a fixed target), `Orbit` (yaw swing and lift
  around a target) and `Transition` (camera → camera, all fields), with `Ease::{Linear,
  CubicInOut}`.
  - Each rig is a pure function of a caller-supplied `t_ms` and `reduced_motion`: no wall clock and
    no state.
  - A settled rig returns its final pose exactly, and reduced motion returns that same pose at
    every time. `settled(t_ms, reduced_motion)` tells a caller when to stop requesting frames.
  - Easing and orbit phase are evaluated in `f64`; orbit phase uses an integer modulo before
    conversion.
- **`gibson::city`**: wireframe helpers that draw lines and return a `Drawn { lines, triangles }`
  count of submitted primitives:
  - `grid` (lines placed by index, no drift);
  - `wire_box`;
  - `floor_bands`.
  It also places geometry without drawing:
  - `facade` (a −z-facing quad for `textured_quad`) and `faces_camera`, a half-space test against
    the quad's plane;
  - slot placement: `row_slot`, `alternating_slot`, `fan_slot`, plus `fan_wrap_index`, the first
    fan index whose angle reaches π. There is no non-overlap guarantee.
- **`raster3d::Camera::basis()` / `CameraBasis`**: the look-at basis (`right`, `up`, `forward`) that
  `project` and the rasterizer use, so callers no longer need to mirror private view maths.

### Added — experience grammars (experimental, Rust only)

- `gibson::ui::experience`: describe an app once as a semantic `Experience`
  (destinations, collections, items, actions) and realize it through
  interchangeable grammars — `Standard`, `MediaShelf`, `CrossMedia`, `Panorama`,
  `Orbital`, `Blades` — with a `PresentationReceipt` checking that required
  semantics survive, and `ExperienceRuntime` for live style switching that
  keeps the selection. Not in the C ABI or wrappers. See
  [`docs/EXPERIENCE_GRAMMARS.md`](docs/EXPERIENCE_GRAMMARS.md).
- Examples `album_flow` and `experience_lab`.

### Fixed — `ui::modal` height

- **A `modal` without an explicit `.height(n)` now sizes to its content** (still capped at 90% of
  the viewport) instead of a fixed 12 rows.
  - **The bug:** the fixed 12 rows left about 9 visible content rows. Longer content was clipped
    silently, with no scroll and no warning: 12 children showed 9, and 30 children on a 40-row
    screen still showed 9.
  - **Unchanged:** an explicit `.height(n)` is still authoritative.
  - **Beyond the cap:** content taller than the 90% bound is clipped deterministically.
  - **Effect on existing layouts:** a modal with fewer than about 9 content rows is now shorter
    than before. Set `.height(12)` to keep the old size.
  - Tests: `tests/modal_sizing.rs` (12 cases).

## [0.7.0-alpha.3] — 2026-11-12 · GitHub prerelease from `main`

A third **consumer alpha** of the 0.7 line, cut from `main`. It is **not** a qualified 0.7.0; the
open v0.7 gates listed under 0.7.0-alpha.1 are unchanged. Linux x86_64 only, GitHub-only (no
registry upload, no Go module tag). `GIBSON_ABI_VERSION` stays 1 (67 symbols, unchanged); MSRV
stays 1.85. The additions below are Rust-only and additive.

### Added — consumer surfaces requested by an external application (Rust only)

- **`ui::progress_steps(label, done, total)`**: discrete progress that prints `label done/total`
  rather than a percentage. The meter is rounded *down* to the nearest eighth of a cell, so it is
  never ahead of `done / total`. It has no animation of its own. It is text-only (one row) when the
  colour depth is monochrome or the element is 40 columns wide or less. `done` is clamped to
  `total`; a `total` of 0 paints `0/0`.
- **`ui::toned_label([tone_span(..).tone(Tone::Warning).strong(), ..])`** and **`ui::ToneSpan`**:
  a one-row label whose spans take a semantic `Tone`/`Emphasis` from the active skin, so one word
  can be warning-coloured inside a muted row. Rows, clipping and sanitising are the same as
  `rich_label`: clipped on the right, so the first span is the last to be cut.
- **`ui::image(&RgbRaster, (columns, rows), alt)`**: a static half-block picture.
  - It is fitted inside the box with its aspect ratio preserved and is never enlarged.
  - Fitting happens once, at construction, by nearest-pixel sampling: at most `2 × columns × rows`
    source pixels are read and stored.
  - The element is always exactly `rows` tall.
  - Only `alt` (one muted row) is painted when the colour depth is monochrome, when the box is
    under 20 × 6, or when the raster is empty.
- **`show::progress_spans_eighths`**: a progress meter built from an exact length in eighths of a
  cell.
- **`examples/status_states`**: six service-state words with tones and markers, rendered at
  40×15 mono and 80×24 ANSI-16 for every built-in skin. It reports which states share a colour at
  each depth. The light built-in skins paint Warning and Danger alike at ANSI-16; there the marker
  (`!` vs `×`) and the word still differ. No new API.
- Acceptance tests: `tests/minerva_surfaces.rs`.

### Changed

- **`ui::progress`** no longer reads `100%` before the fraction actually reaches 1.0 (0.996 now
  reads `99%`). Other percentages and the meter are unchanged.

## [0.7.0-alpha.2] — 2026-11-12 · GitHub prerelease from `main`

A second **consumer alpha** of the 0.7 line, cut from `main`. It is **not** a qualified 0.7.0; the
open v0.7 gates listed under 0.7.0-alpha.1 are unchanged. Linux x86_64 only, GitHub-only (no
registry upload, no Go module tag). `GIBSON_ABI_VERSION` stays 1 (67 symbols, unchanged); MSRV
stays 1.85. Everything below is additive Rust API marked EXPERIMENTAL in
[`docs/RELEASE_CONTRACT.md`](docs/RELEASE_CONTRACT.md).

### Added — textured facades and a grid raycaster (experimental, Rust only)

- **`raster3d::Rasterizer::textured_quad(corners, &RgbRaster, &Camera) -> bool`**: draws an
  existing raster as a perspective-correct, frustum-clipped, depth-tested quad that shares the
  z-buffer with meshes and lines, so wireframe edges and textured facades occlude each other
  correctly in either draw order. Not backface-culled. Work is bounded by the destination: at
  most `14 × width × height` pixel visits per call (a convex planar quad stays near
  `2 × width × height`), independent of texture size. (Ported from the unreleased v0.5 spatial
  branch, with its tests.)
- **`gibson::raycast`**: `GridMap` (including a seeded, closed, fully reachable `GridMap::maze`),
  `RayView`, the `Texels` trait and `render` into an `RgbRaster`. Wall textures read left to
  right on every face, with `u`, `v` in `[0, 1)`. The raycaster core is deterministic across
  hosts (quantised angle, fixed-polynomial trig) and bounded to `2 × w × h` pixel writes plus
  `w × max_steps` grid steps, independent of map size; positions beyond `RayView::MAX_COORD`
  and a `plane` outside `[MIN_PLANE, MAX_PLANE]` are refused. Extracted from the Deep Signal
  example's renderer (whose own copy is unchanged).
- **`examples/facade_signal`**: a raycaster walking a maze, rendered live onto the facades of a 3D
  wireframe city as the camera orbits. Text fallbacks for monochrome and small (<60×20)
  terminals; reduced motion renders one settled frame and waits on input; refuses to run live
  when stdout is not a terminal. `--frames N` prints receipts.
- Consumer acceptance cases: [`docs/FACADE_SIGNAL_ACCEPTANCE.md`](docs/FACADE_SIGNAL_ACCEPTANCE.md).

## [0.7.0-alpha.1] — 2026-11-12 · GitHub prerelease from `main`

A **consumer alpha** of the 0.7 line, cut from `main` so an external application can pin a tag
for the APIs it asked for. It is **not** a qualified 0.7.0. The v0.7 release gates are still open:
- output-endpoint identity and resize geometry after a stdout redirect (#348; renderer stack
  #338 → #339 → #343);
- the downstream canary on this tag;
- the separately published GPL adapter (node J);
- SDK parity.

Linux x86_64 only. GitHub-only: no crates.io, PyPI or Go module proxy upload, and no Go module
tag. `GIBSON_ABI_VERSION` stays 1 (67 symbols); MSRV stays 1.85. Everything below landed on
`main` after v0.6.0, which was released 2026-11-08 from `release/v0.6.0-rc` (tag `v0.6.0` →
`492d0fe`, source baseline `9a899e1`).

### Added — consumer APIs requested by an external application (Rust only)

- **`gibson::ui::rich_label(impl Into<Line>)`**: a single-row label whose spans each carry their
  own style (a gutter that differs from its code, one coloured word in a heading). Exactly one row,
  never wraps, clips to its width; the element's tone/emphasis is the base each span's style is
  laid over. Text from the first line break on is dropped and control characters are removed, so
  an unstyled rich label paints and lays out like `label`.
- **`Context::resize(cols, rows) -> io::Result<()>`** for headless contexts: resizes the virtual
  terminal in place, keeping the retained renderer state, so one context can be driven through a
  sequence of geometries in tests (a `UiRuntime` frame built afterwards sees the new size). Zero
  dimensions return `InvalidInput`; a terminal-backed context returns `Unsupported`.
- **`gibson::text::{display_width, char_width}`**: the number of columns the painter advances for
  text on one row (per grapheme, capped at 2; controls add nothing; a line break ends the row).
- **Per-colour-depth skin palettes**: `Skin` gains `ansi16` and `ansi256: Option<DepthPalette>`
  (`DepthPalette { palette, surface, well }`), used at that depth instead of nearest-colour
  mapping of the truecolor palette, so a skin can keep, e.g., a gold accent and an amber warning
  distinct at ANSI16 (they otherwise both map to yellow). `Skin::palette_for(depth)` exposes the
  choice. Built-in skins carry `None` and resolve exactly as before (existing goldens unchanged).
  The override covers palette, surface and well; title/selection/focus styles are still mapped.
- **`ResolvedSkin::tone_collisions()`**: tone pairs whose resolved foreground and background are
  identical at the resolved depth, for skin tests that must keep tones apart.
- **Breaking (experimental `gibson::ui`, 0.7 minor slot):** a `Skin` written as a complete struct
  literal must add `ansi16: None, ansi256: None`, and an exhaustive `let Skin { .. }` pattern
  without `..` must name them; a literal using `..skins::BLACK_ICE` (or any built-in) needs no
  change. The new inherent `Context::resize` takes precedence over a consumer extension-trait
  method of the same name.
- **Fixed:** a multi-span rich text row no longer paints a later span into the columns an earlier
  span gave up when it was cut (a wide glyph that did not fit, or a line break): the row now ends
  at the cut. Single-span text is unchanged.
- Known, not fixed here: layout measures some joined scripts (Arabic lam-alef, ZWJ-joined Hebrew)
  narrower than the painter paints them, for `label` and `rich_label` alike.
- No C ABI change.

### Fixed — raw-mode restore targets the acquired terminal (#345)

- A managed session now restores raw mode on the terminal it acquired, not on whatever stdin
  names at restore time. Previously, replacing stdin with another terminal after
  `enter_interactive` made `restore` report success while the acquired terminal stayed raw and
  the replacement received its saved settings. Input is read from the acquired terminal too, and
  a poll already blocked before acquisition no longer delivers its event to the new owner. A
  host's own crossterm raw mode is still disabled on restore, as documented; with a replaced
  stdin that restore now returns an error and changes nothing. Supported and excluded descriptor
  changes: [`docs/TERMINAL_LEASE_CONTRACT.md`](docs/TERMINAL_LEASE_CONTRACT.md). No public API
  or C ABI change.

### Added — C ABI: terminal-size query and focus events (#298)

- **`gibson_get_terminal_size(ctx, *out_cols, *out_rows)`** returns the live terminal geometry
  (as opposed to `gibson_get_frame_size`, which is the last rendered frame's), so a C consumer can
  size up before the first render. Both outputs are always non-zero.
- **`gibson_set_focus_reporting(ctx, enabled)`** opts into focus-change reporting; the terminal
  gaining or losing focus then polls out as the new fieldless event types
  **`GIBSON_EVENT_FOCUS_GAINED` (7)** / **`GIBSON_EVENT_FOCUS_LOST` (8)**. A polled focus event
  round-trips back through `gibson_push_event`. A no-op on a headless/non-TTY context.
- Purely additive: the exported C ABI symbol set grows **65 → 67**, `GIBSON_ABI_VERSION` stays 1,
  and no existing symbol, constant, or struct layout changes. The Rust machinery (`Event::Focus`,
  `TerminalSession::{terminal_size, set_focus_reporting}`) already existed; this is the C projection.
### Known limitation — Windows: every key is delivered twice, UNVERIFIED ([#292](https://github.com/femboy2112/libgibson/issues/292))
- **On Windows a key press arrives as two `Event::Key`s.** crossterm 0.29 reports a console key-up
  as `KeyEventKind::Release` (`parse.rs:286-290`); on Windows LibGibson passes every kind through
  (`delivers_key_kind`) and `decode_key` ignores the kind, so the release is delivered as a second
  press. This is read from the source: there is no Windows runtime or CI job, so it has not been
  observed, and **Windows remains unsupported and unverified** (see `docs/RELEASE_CONTRACT.md` §6).
- **Not changed here, deliberately.** Dropping releases on Windows would delete Alt+numpad
  characters (Alt+0233 gives an e-acute), which the console delivers only as a `Release` carrying
  `KeyCode::Char`. The fix is a LibGibson-owned Windows console source reading `KEY_EVENT_RECORD`s,
  future work that needs a native Windows host. This entry is disclosure only; no runtime behavior
  changed. A host-independent unit test pins today's behavior.

### Fixed — Go binding (`bindings/go`): a failed `NewContext` reports its own error message
- **`NewContext` now reads the failure message on the same OS thread as the failing call.**
  The native last error is thread-local, and the wrapper fetched it with a second cgo call; Go may
  move a goroutine between the two. Reproduced on gccgo 1.18 by `TestNewContextErrorIsFromItsOwnCall`
  (64 goroutines, each with its own invalid mode, so its own expected message; GOMAXPROCS 8;
  unlocked polluter goroutines writing a third text): on the old wrapper **12 of 12 runs** failed,
  with **4,601 foreign messages of 1,536,000 calls** (another goroutine's mode or the polluters'
  text). The fixed wrapper: 0 of 1,536,000. The gc toolchain was not run locally. Create and read
  now happen in one C helper, one cgo call. Go API unchanged.
  ([`docs/V08_PARITY_SLICE.md`](docs/V08_PARITY_SLICE.md) F6)

### Known limitation — a shifted key in the screen-switch window ([#312](https://github.com/femboy2112/libgibson/issues/312))
- **A shifted *symbol* (e.g. `!`) pressed during the brief legacy-mode window of a screen switch is
  delivered correctly as a press, but kitty reports its release under the base key (`1`); shift does
  not lowercase a symbol, so the release cannot be matched to the press layout-agnostically and is
  delivered as `1`.** A held-key model therefore keeps the symbol held until a focus loss, the key's
  next kitty press/release, or reporting is disabled. A legacy `W` pairs when the terminal reports
  `w` as its base. Not all Unicode letters pair this way: the default lowercase of `İ` is two code
  points, so it does not match
  a single `i` base key. The `İ` case is covered by a decoder model, not a real-layout terminal test.
  Deliberately not renamed: mapping `1`→`!` assumes a
  keyboard layout, and pairing the unmatched release with the newest orphan would mis-identify an
  unrelated key's release. None of the recovery events is guaranteed, so this hold may persist
  indefinitely while reporting remains active. Tests in `src/tty_source.rs` (`issue_312_*`) pin
  the failure and conditional recovery; no finite deadline is claimed.

### Release engineering — maintenance-line releases approved ([#287](https://github.com/femboy2112/libgibson/issues/287), #291)
- **Release Contract §1 gains an approved maintenance-line exception** (owner decision 2026-11-08,
  conditional on its gates): a `v<x.y.z>` tag may name a qualified commit on `release/v<x.y.z>-rc`
  or `release/<x.y>` that descends from the version's source baseline and adds only listed
  backports and release engineering, with the C ABI symbol set, header declarations, MSRV and
  frozen PCM golden unchanged. Procedure: `docs/RELEASING.md` §7. Decision record:
  `docs/decisions/0001-v0.6.0-release-line.md` (option A, 0.6.0 from `release/v0.6.0-rc`).
- **`scripts/release/check-maintenance-candidate.sh`** checks a candidate structurally: ancestry
  (and that the candidate is not the baseline itself), version, the C ABI, MSRV, the PCM golden, the
  C ABI layout oracle (`tests/ffi_abi_layout_freeze.rs`, which CI runs, unchanged), an exactly named
  release branch, tags and allow-listed commits. For the C ABI it compares:
  - the exported names, from source, from `abi/`, and from every `gibson_*` symbol of the built
    library, data symbols included;
  - the Rust C-ABI surface of the files that export (`scripts/release/maintenance_surface.py`):
    - every `extern "C"` function's signature, generics and non-doc attributes (`no_mangle`,
      `export_name`, `cfg`);
    - exported statics;
    - `#[repr]` struct, enum and union bodies;
    - `GIBSON_*` constant values;
    - type aliases, imports and module declarations;
    - the macros that define exported functions.

  A header change passes only if it is comment-only. Comments are removed by the compiler's own
  lexer (`cc -fpreprocessed -dD -E -P`) and whitespace is collapsed outside string and character
  literals. These fail closed: a header with a line splice or a trigraph introducer, a raw string,
  no compiler, and any failure of a tool feeding a check (git, nm, python3), including a tool that
  dies after printing part of its output. Parsing involves no pipes into early-exiting readers and
  no here-strings, so it has no IO failure mode of its own.
- **`scripts/release/tests/test-maintenance-checker.sh`** calibrates the checker with 47 cases in a
  throwaway clone. Every `GIT_*` variable is cleared first, and the clone's git directory is
  verified before any ref is touched.
  - **9 must pass:**
    - a valid candidate;
    - a comment-only header change;
    - both tags on the candidate;
    - a doc-only `src/ffi.rs` change;
    - a CRLF two-column allow-list;
    - a synthetic library with exactly the baseline's names;
    - a new non-exporting `#[cfg(unix)]` module, the shape of the #269 backport;
    - a `#[cfg(test)]` module with imports in `src/ffi.rs`;
    - a `Cargo.toml` with 20,000 comment lines.
  - **38 must fail, each for its intended reason and no other:**
    - candidate identity: a 0.7 development commit; a non-descendant; the baseline itself;
    - ABI baseline and library: a changed ABI baseline; a library from another tree or with an
      extra data symbol;
    - allow-list: a missing, extra or misclassified entry;
    - version and tags: a manifest or release-version mismatch; either tag on another commit;
    - headers: a declaration change; a macro hidden by line splices, by a trigraph or behind 84 KB
      of padding; whitespace inside a string literal;
    - Rust surface: swapped `#[repr(C)]` fields; a changed parameter type or constant; `cfg` or
      generics on an export; an `export_name` export; a new type alias; a new export or exported
      static;
    - frozen files and branches: an edited layout oracle; an MSRV or PCM producer change; no
      release branch, or a wrongly named one;
    - unreadable input: an unreadable `src/` file;
    - fault injection: four tools that die after printing.

  Raw run: `docs/decisions/0001/calibration-572e551.txt`. Mutation controls (each guard reverted
  turns exactly its cases wrong): `docs/decisions/0001/checker-mutation-controls.txt`.

  Three rounds of independent (Codex) review found the bypasses these cases now cover. An earlier
  count in this PR's description ("20/20") was wrong. **The checker's boundary:** its Rust surface
  parses the constructs this crate's C ABI uses. The export set is settled by `--so` (required to
  qualify), struct layouts by the frozen layout oracle in CI, and anything else by the independent
  review.
- **The 0.6 limitations in Release Contract §9** now state that a headless C context's capture grows
  for its lifetime (#297) and how a C program must bound it, and that 0.6.0's `gibson_poll_event`
  on a headless context reports no input (#259).
- **v0.6.0 published (state 2); post-release records.** v0.6.0 was released on 2026-11-08 as a
  GitHub prerelease from `release/v0.6.0-rc` (tag `v0.6.0` → `492d0fe`; `bindings/go/v0.6.0` names
  the same commit). `main` stays `0.7.0-dev` and was not merged with the maintenance branch; nothing
  was uploaded to crates.io, PyPI or the Go proxy. Receipts for the released commit:
  `docs/decisions/0001/allow-commits-492d0fe.txt`, `docs/decisions/0001/checker-492d0fe.txt`
  (RESULT: PASS with tags present) and `docs/decisions/0001/calibration-492d0fe.txt` (47 of 47).
  Records: [qualification](https://github.com/femboy2112/libgibson/issues/287#issuecomment-6721675144),
  [publication](https://github.com/femboy2112/libgibson/issues/287#issuecomment-6721728874);
  [decision 0001](docs/decisions/0001-v0.6.0-release-line.md), "Outcome".

### Added — Python wrapper: events (v0.8 parity slice, finding F5)
- **`bindings/python/gibson.py` can push and poll events.** The v0.8 parity consumer
  no longer needs raw ctypes for `gibson_event_t`. New: the `GibsonEvent` struct (layout checked
  against the header), the `Event` value type (`Event.char`, `Event.special`, `Event.paste_text`,
  `Event.resize`), `Context.push_event(event)` and `Context.poll_event(timeout_ms=0, kind=True)`,
  and the constants `EventType`, `KeyKind`, `Key`, `Mod`, `Status` and `GIBSON_EVENT_QUEUE_CAPACITY`.
  - A polled `Event` carries the key kind from `gibson_poll_event_kind`, so a typed U+1001 stays
    distinct from `Key.ENTER`, and a polled KEY, PASTE or RESIZE can be pushed back unchanged
    (a NONE from an empty queue cannot; the C ABI refuses NONE). `kind=False` calls the kind-less
    `gibson_poll_event` (`key_kind` is `None`); a `None` key kind on push calls `gibson_push_event`.
  - A non-OK status raises `GibsonError` (a `RuntimeError`, with `.status` and the thread-local
    `.last_error` read right after the call); a full queue raises `QueueFullError`. The C ABI, not
    Python, decides which events are invalid. Python raises `ValueError` (`UnicodeEncodeError` is one)
    only when it cannot build the C struct, for example a value that does not fit its C field, or a
    paste `str` that cannot be encoded as UTF-8 or contains a NUL byte.
  - `bindings/python/test_events.py` (new CI step in the Bindings job) covers CHAR/SPECIAL, paste,
    resize, empty queue, queue full, invalid parameters and the poll-then-push round trip.
  - `bindings/parity/v08/parity_v08.py` now uses these calls for events (transcript unchanged,
    byte-identical to C); Python's "Event identity & ordering" and "Geometry" capability labels move
    PARTIAL to SUPPORTED. No C ABI or Rust change.

### Added — Rust library (`gibson::Context`): `Context::enter_interactive` ([#296](https://github.com/femboy2112/libgibson/issues/296))
- `pub fn enter_interactive(&mut self) -> io::Result<()>` enables raw mode and bracketed paste now,
  exactly as the first `poll_event` / `run_once` does. `Context::inline()` and `fullscreen()` still
  leave it to the first poll, so output shown before that poll (a READY line, a first frame) was on
  a cooked tty: the kernel echoed type-ahead, and ICRNL turned Enter into `\n`, which raw decoding
  reads as Ctrl+J, so a line typed at "ready" was never submitted. Call it before the first output
  and "the screen says ready" implies "the terminal is interactive".
- Idempotent (a second call and later polls write nothing more); a no-op returning `Ok` on a
  headless context and on a non-TTY stdout. `restore()`, `Drop` and the panic hook undo it as they
  undo the first poll's. After `restore()` it returns an error (`ErrorKind::NotConnected`) instead
  of re-entering raw mode on a terminal the context no longer owns.
- The downstream workaround, `poll_event(Duration::ZERO)` before the first output, is no longer
  needed.
- `tests/enter_interactive_pty.rs` is a deterministic real-PTY probe: with the call, `hello\r`
  typed at READY is five characters and one Enter with no kernel echo; without it (the old
  behaviour, held so the race is not a race) the same probe fails with an echo and Ctrl+J.

### Added — Go binding: headless context, events, readback and capture (v0.8 parity slice, finding F5)
- **`bindings/go/gibson` covers the headless path without cgo in the caller.** Everything below is
  new; no existing exported name or signature changed:
  - `NewHeadlessContext(mode, cols, rows)`, `Context.UnsafeHandle()`, `Context.SetRootNode(node)`.
  - `Event` (`CharKey`, `SpecialKey`, `LegacyKey`, `PasteText`, `ResizeEvent`), `Context.PushEvent`,
    `Context.PollEventKind` and the kind-less `Context.PollEvent`; `KeyKind*`, `Key*`, `Mod*`,
    `EventQueueCapacity`. A polled event keeps the CHAR/SPECIAL key kind and can be pushed back.
  - `Context.FrameSize`, `FrameCell`, `FrameCells` (the C ABI judges every call, including one that
    returns no cells), `FrameCellGlyph`, `FullGlyph`, `RenderedOutput`, `TakeRenderedOutput`,
    `ClearRenderedOutput`.
  - A cgo-free `Style` / `Color`, `NewStyledTextNode(text, style, wrap)` and `Line.AddStyledSpan`.
  - `*gibson.Error` carries the status and the last-error text, read in the same cgo call as the
    failing ABI call (Go moves goroutines between OS threads); `errors.Is(err, ErrQueueFull)`.
  - `headless_test.go` covers the same ground as the Python and C++ event tests, plus the
    last-error-under-pollution test for `NewHeadlessContext`.
    `bindings/parity/v08/go/main.go` now uses these calls (transcript unchanged, byte-identical to C),
    moving the Go Event, Geometry, Cells and Text capability labels from PARTIAL to SUPPORTED. No C ABI
    or Rust change.

### Added — C++ header: headless context, events, readback and capture (v0.8 parity slice, finding F5)
- **`include/gibson.hpp` (and so the `bindings/cpp/gibson.hpp` shim) covers the headless path.**
  New, all additive; existing members keep their signatures and behavior:
  - `Context::headless(mode, cols, rows)`, `close()` (idempotent), `raw()`.
  - `Event` (`char_key`, `special_key`, `legacy_key`, `paste_text`, `resize`), `Context::push_event`,
    `Context::poll_event_kind(timeout_ms = 0)` and a kind-less `Context::poll_event_kindless(timeout_ms = 0)`
    (a separate name, so `&Context::poll_event` stays unambiguous; the older
    `poll_event(timeout_ms, gibson_event_t&)` is unchanged). A polled `Event` keeps the CHAR/SPECIAL
    key kind, so a typed U+1001 stays distinct from `GIBSON_KEY_ENTER`; a polled KEY, PASTE or
    RESIZE can be pushed back unchanged (an empty-queue NONE cannot: the C ABI refuses NONE).
  - `frame_cells` throws `Error` for no frame or out-of-bounds coordinates, as `frame_cell` does,
    even when it would return no cells (the C ABI validates every call).
  - `frame_size`, `frame_cell`, `frame_cells`, `frame_cell_glyph`, `full_glyph`, `rendered_output`,
    `take_rendered_output`, `clear_rendered_output`.
  - Errors are exceptions: `gibson::Error` (a `std::runtime_error` with `status()` and the thread-local
    `last_error()`, read right after the failing call) and `QueueFullError`.
  - `bindings/cpp/test_headless.cpp` (new CI steps, plain and ASan + UBSan) mirrors the Python
    event test; `bindings/parity/v08/parity_v08.cpp` now uses these calls (transcript unchanged,
    byte-identical to C), moving the C++ Event, Geometry, Cells and Text capability labels from
    PARTIAL to SUPPORTED. No C ABI or Rust change.

### Fixed — Rust library (`gibson::Context`): a delivered resize resizes a headless context ([#293](https://github.com/femboy2112/libgibson/issues/293))
- `Context::poll_from` on a **headless** context now applies a delivered `Event::Resize(cols, rows)`
  before it returns, so the next render lays out against the new terminal size (a fullscreen frame
  is `cols`×`rows`; an inline frame is `cols` wide and as tall as its content, at most
  `max(rows - 1, 1)` rows, as on a terminal). It used to only schedule a
  render, which then re-read the old size, while the C ABI's `gibson_poll_event` applied it: a
  Rust consumer had to call `ctx.session.set_terminal_size` itself to match C. Both now call one
  implementation in `Context`; the copy in `src/ffi.rs` is gone.
- **Zero sizes:** a resize with a zero width or height is still delivered but is not applied (the
  geometry stays). The C ABI already refuses to queue one (`GIBSON_ERR_INVALID_PARAM`), so neither
  side ever gives a headless context a zero geometry through a resize.
- A terminal (non-headless) context is unchanged: its geometry is observed from the terminal.
- The v0.8 parity slice's Rust consumer now uses `poll_from` alone; it produces the C records
  r02/r03/r04 byte-identically without the manual resize step.

### Added — key-release reporting on kitty-protocol terminals ([#275](https://github.com/femboy2112/libgibson/issues/275))
- **Opt-in, Rust only for now:** `TerminalSession::set_key_release_reporting(true)` (reached as
  `ctx.session`) asks the terminal to report key releases through the kitty keyboard protocol.
  Tested in kitty 0.32.2 (and xterm 390, which answers `Unsupported`); other terminals that
  implement the protocol (foot, Ghostty, WezTerm with its kitty-keyboard option) are expected to
  behave the same but are not tested. It pushes keyboard flags 15 on the current screen
  and asks whether the terminal took them. The answer arrives in input order as the new
  `Event::KeyReleaseReporting(KeyReleaseReporting::{Active, Unsupported})`; once `Active`, each key
  that goes up is the new `Event::KeyRelease(KeyEvent)`. Both variants are additions to the
  `#[non_exhaustive]` `Event`. Off by default: an application that does not opt in writes and
  receives exactly what it did before, and releases stay undelivered (#285).
- **A release is named exactly as its press was delivered.** kitty names a key differently on the
  way up when shift goes up first (a `W` pressed with shift is released as `w`, a `!` as `1`), so
  the Unix input source records each press by the key's base code and names its repeats and release
  the same way: a release matches its press by equality.
- **Restored on every exit path the session owns.** `restore`, `Drop` and the panic hook pop the
  flags before leaving the alternate screen (the kitty flag stacks are per screen, and a push left
  on the alternate screen is inherited by the next full-screen program). Moving between the main
  and alternate screen while enabled pops on the screen being left and asks again on the new one.
  A process killed outright (SIGKILL) leaves the flags pushed; kitty's reset (Ctrl+Shift+Delete)
  clears them. While enabled, text typed through an input method may not arrive: the feature is
  for direct key control, not text entry.
- **`gibson::hold` uses reported releases.** `HoldTracker::set_releases_reported` (or the answer
  event through `observe_event`) switches holds observed from then on to "held until released, at
  least one tic"; `observe_release` and `ReleaseCause::KeyReleased` end them. Keys held together
  stay held together, which the inferred model cannot do (a terminal autorepeats only the last key
  pressed). If reporting stops, every hold falls back to the inferred deadline.
- **DEEP SIGNAL opts in** (`--no-key-releases` opts out), shows `KEYS EXACT` on the HUD once the
  terminal confirms, and records releases and the answer in its `--record` trace. Its self-check
  adds a reported-release segment (`exact_keys=519616832bd513f1`); the existing values are
  unchanged.
- **Fixed in the ported decoder:** the kitty flags reply `CSI ? flags u` was read from the byte
  of its first digit (as in crossterm 0.29.0), so kitty's `CSI ? 15 u` read as flags 1 and a
  confirmed request would have looked unsupported.
- Evidence: `tests/key_release_reporting_pty.rs` (the bytes written, in order, on a normal exit, a
  panic, inline and across a screen switch; answers; release naming; no opt-in, no change), replaying
  bytes captured from kitty 0.32.2; `tests/input_hold_released.rs` (the hold model against an
  independent reference on 300 seeded traces); and `scripts/dev/kitty_key_release_e2e.sh`, which
  runs DEEP SIGNAL in real kitty 0.32.2 and xterm 390 on a private Xvfb display with xdotool key
  events (not in CI).
- **Review hardening (Codex, 7 P2 + 3 P3, all fixed with a test that fails without the fix):** the
  pop is claimed once, so a panic followed by the unwinding restore pops once; a restored session
  cannot enable; a stray device-attributes reply (somebody else's earlier query) cannot be told
  from a terminal without the protocol and reads as `Unsupported` (final, see below); after a
  disable, a late answer or release is dropped at delivery; a keypad key never takes over the
  record of the arrow with the same code; a name recorded before a disable/enable loses to the key's newer press;
  DEEP SIGNAL keeps holds per physical key, so releasing W while Up is down keeps moving forward;
  enabling enters raw mode first, so the answer is never echoed. A second pass (4 partial, 2 new
  P2) moved the toggle handling to delivery: an answer decoded before a disable/enable is dropped
  when handed out, a press already queued keeps its release name across a toggle (a repeat or
  release takes the most recent matching record, the legacy fallback only for text keys), and
  after a caught panic the session reports reporting off and can enable it again. A third pass:
  a release decoded before a toggle is dropped too (as disabling promises); a legacy record of any
  key recorded in a later epoch supersedes a stale record of it (ctrl+Enter released unreported,
  then plain Enter); and re-enabling after a caught panic restores raw mode. A further review: a
  key already down when reporting began is named by its first kitty autorepeat, and kitty's
  release matches a legacy capital (`W`, released as `w`) or a legacy symbol kitty autorepeated
  (`!`, released as `1`), so each such release ends its hold (known limit: a shifted symbol pressed
  just before the flags take effect, as during a screen switch, and let go without an autorepeat is
  still released as `1`, so that release does not end its hold,
  [#312](https://github.com/femboy2112/libgibson/issues/312)); `Active` needs every flag pushed (a
  terminal that took only some, such as 3, 7 or 11, answers `Unsupported`); **`Unsupported` is
  final:** once any poll hands it out (a `Context` poll, `input::poll_event`, a `TerminalSource`,
  whichever context reads it), no release is delivered from then on, including any queued behind the
  answer, while an answer to an earlier request (one decoded before reporting was last enabled or
  disabled) is dropped by whichever poll reads it and changes nothing, the check and the end of
  delivery being one atomic step; the owning session withdraws its push with one pop (never repeated
  at restore, `Drop` or in the panic hook): at once if its own `Context` poll handed the answer out,
  otherwise on its next `Context` poll, enable, disable or screen switch, or at restore. The
  terminal then goes back to the flags in force before the push (legacy keys, one `\r` per Enter,
  unless something else, such as the host of an inline session, had pushed its own). Once the panic
  hook has popped the flags, even for a caught panic, no release or terminal answer is delivered and
  `Unsupported` is delivered once, by the next `Context` poll or, if the terminal answers first,
  just before that answer by whichever poll hands it out, so it comes before the new answer even if
  reporting was enabled again meanwhile, and a client still waiting for releases switches back to
  inferred holds; if a `Context` poll fails to re-enter raw mode, the notice waits for the next.
  While reporting is confirmed, or its answer is still pending, a read ending in a lone ESC waits up
  to 50 ms for the rest of its sequence (reading what is already waiting before giving up), then is
  the Esc key. The rustdoc now lists what flags 15 change in delivered keys (a shifted letter has no
  `SHIFT`, Caps Lock does not change a letter, Ctrl+I/M/[ are Ctrl chords).
- **C ABI v1, additive (64 → 65 symbols):** `gibson_set_key_release_reporting(ctx, enabled)` and
  the event types `GIBSON_EVENT_KEY_RELEASE` (4; `key_code`, `modifiers` and the key kind exactly
  as the press was delivered, also through `gibson_poll_event_kind`),
  `GIBSON_EVENT_KEY_RELEASES_ACTIVE` (5) and `GIBSON_EVENT_KEY_RELEASES_UNSUPPORTED` (6), all
  pushable on a headless context. A consumer that never calls the new function never sees the new
  types; existing values are unchanged. `tests/ffi_key_release.rs`; the real-PTY test drives the
  same kitty bytes through the C ABI (role `c`), and checks that a C client that never opts in
  polls presses only and nothing is written for it, that disabling pops at once and drops a late
  answer and releases, and that a panic on the owning thread reaches C as
  `KEY_RELEASES_UNSUPPORTED` before the answer to a new request. `gibson.h` states what enabling
  and disabling do, what restore and the panic hook do, and the shifted-symbol limit
  ([#312](https://github.com/femboy2112/libgibson/issues/312)).
- **`fb_adapter` (node J) can use reported releases, opt-in:** `fb_adapter_config.key_releases` is
  0 by default (the adapter is embedded in consumers, and this is an opt-in input contract), and
  then the adapter asks nothing and ignores the release and answer events, keeping inferred holds.
  Set to 1, it requests them on a terminal; once confirmed a key is held exactly while it is down,
  at least one tic, and several keys that `map_key` sends to one producer key keep it held until
  the last of them goes up (up to 8 per producer key; inferred holds stay per producer key: there
  an alias re-arms the hold like an autorepeat), a hold begun before the confirmation keeps its
  inferred deadline unless every key that made it is reported up, and an exact hold has no grace
  fallback (a release that never arrives keeps it until reporting is withdrawn); a terminal
  without the protocol keeps the grace. `fb_adapter_releases_reported`; releases and the answer
  are recorded in the input trace (`FB_OBSERVED_*`), so a session replays exactly; the demo
  (`--key-releases`) shows `keys exact` on its status row. The selfcheck's P3 adds a
  reported-release leg (w held across a d chord, a one-tic tap) against the rule written out by
  hand, a full key queue (a release waiting for room is never lost to a re-press, which is dropped
  and counted) and a withdrawal (only holds waiting for a release are re-timed, as in
  `gibson::hold`), w and Up mapped to one producer key, held until both are up (and re-armed as
  one key with inferred holds), two holds mixing inferred and reported parts, each press and
  release of the producer key counted, with the controls `p3-ignore-releases`,
  `p3-rearm-loses-release`, `p3-shared-alias-hold`, `p3-unmatched-release`, `p3-wrap-deadline` and
  `p3-rewrite-ended` (29 controls; hold deadlines saturate at the last tic, and a hold that has
  ended while its release waits for queue room is not rewritten by a later release), and a leg
  where the same script through an adapter left at the default gives the grace model's whole
  transcript, written out by hand. `scripts/dev/kitty_key_release_e2e.sh` with `FB_DEMO=` runs the
  demo in real kitty: the trace of the live session replays headless to the same hash, and without
  `--key-releases` the demo records no answer and no release.

### Fixed — a key release is no longer delivered as a key press ([#285](https://github.com/femboy2112/libgibson/issues/285))
- The public input model is press-only. A terminal speaking the kitty keyboard protocol reports an
  event type with each key (`CSI 97u` press, `CSI 97;1:2u` repeat, `CSI 97;1:3u` release), and
  every **release** reached the application as a second press: through `gibson::input::poll_event`,
  `Context::poll_event` / `run_once`, `gibson_poll_event` and `gibson_poll_event_kind`. A press and
  a repeat are still delivered as `Event::Key`; a release is not delivered. LibGibson never enables
  the protocol itself, so this matters when something else left it enabled (for example a
  kitty-protocol program that exited without popping its flags).
- **A dropped event no longer ends a terminal poll early.** On a terminal context, an event that
  is not delivered (a release, or a key code LibGibson does not model, such as F5) used to make
  the poll return "no event" at once, before its timeout; the poll now carries on for the rest of
  the same deadline, and the events queued behind the dropped one are delivered in order. Once the
  deadline has passed, at most 64 more already-available undelivered events are consumed before
  the poll returns, so a stream of them cannot hold a poll past its timeout. (Headless and non-TTY
  contexts still return "no event" at once, and the legacy C poll still reports focus changes and
  other events it does not model as `GIBSON_EVENT_NONE` at once, as before.)
- **Windows key-kind decoding is unchanged** (the continued polling above does apply there).
  crossterm 0.29 reports a release there for each ordinary key-up, which LibGibson has always
  delivered as a second press ([#292](https://github.com/femboy2112/libgibson/issues/292)); but
  it also delivers an Alt+numpad character only as the release of Alt, and a decoded key event
  does not say which of the two a character release is. Dropping releases on Windows would lose
  those characters, so that fix is left to #292, which needs that distinction and a Windows test.
- `tests/key_release_kind_pty.rs` injects the sequences through a real PTY: press, repeat and
  release; a release followed by a legacy press; repeated cycles; fragmented sequences; U+1001
  (the #274 range) through both C polls; focus changes and a resize between releases; a release
  queued before a hangup; and DEEP SIGNAL recording one `w` per press/release pair (it recorded
  two). All six failed on `5790f1a`.

### Fixed — Rust library (`gibson::input`, `gibson::Context`) and C ABI input on a Unix terminal ([#269](https://github.com/femboy2112/libgibson/issues/269), [#15](https://github.com/femboy2112/libgibson/issues/15))
- **A terminal hangup during a poll no longer spins forever.** When stdin is the controlling
  terminal, input is now read by a library-owned source (`src/tty_source.rs`) instead of
  crossterm's event reader, whose read loop retries a hung-up terminal forever (#269). The hangup
  is detected from the read itself (`EIO` or `0` bytes while the descriptor reports a hangup), so
  it is reported as `ErrorKind::BrokenPipe` wherever it lands, not only when a check before the read
  catches it. The terminal read is the one crossterm chooses (stdin when it is a terminal, else
  `/dev/tty`), latched when the session acquires the terminal and read through LibGibson's own
  non-blocking description where its identity can be proven. A background read refused with
  `EIO` (job control) is retried, not reported as a hangup.
  This affected applications that ignore or handle SIGHUP; under the default disposition the
  signal ends the process first.
- **Events decoded before a hangup are delivered** before the `BrokenPipe`. They used to be
  stranded behind the hangup check.
- **A key that coincides with a resize is delivered** without waiting for further input (#15).
  The Event Pressure Lab's delivery acceptance for the `Context` path is enabled and passes.
- **A canonical-mode EOF character (^D) no longer blocks a poll past its timeout**. It also is not
  mistaken for a hangup.
- **A key arriving during a wait now ends it at once.** The old hangup-aware loop slept a 25 ms
  slice whenever input became readable during a wait.
- The decoder is crossterm 0.29.0's (MIT), ported with mechanical edits only; see
  `THIRD-PARTY-NOTICES.md`. `signal-hook` (already a dependency through crossterm, same version)
  is now a direct Unix dependency for `SIGWINCH`. Off Unix, and when stdin is not a terminal,
  input still goes through crossterm.
### Added — C ABI v1 (additive, 62 → 64 symbols): drain a headless capture ([#297](https://github.com/femboy2112/libgibson/issues/297))
- **`gibson_take_rendered_output(ctx, buf, cap, out_len)`** copies the whole headless ANSI capture
  into `buf` and, only then, empties it. `*out_len` is always the size before the call; a size
  query (`buf == NULL`) or an undersized buffer copies nothing and removes nothing (still
  `GIBSON_OK`, so check `*out_len <= cap`). The take is all-or-nothing, so a drain never splits an
  escape sequence. Until now a headless context's capture grew by every frame's bytes for the
  context's lifetime, because `gibson_get_rendered_output` only copies: a long-running headless C
  consumer grew without bound (+636 MB after 2000 frames at 320x100 in the node J adapter).
- **`gibson_clear_rendered_output(ctx, out_discarded)`** discards the capture without copying it,
  for a consumer that reads cells, not ANSI; `out_discarded` (may be `NULL`) receives the count.
- **`gibson_get_rendered_output` is unchanged**: it copies and never drains. Rust gains
  `Context::take_rendered_bytes`, the binary-safe drain (`take_output` stays lossy UTF-8). The
  Python binding adds `take_rendered_output()` and `clear_rendered_output()`.
- `bindings/sdk_forcing/headless_capture_drain.c`, an installed-SDK consumer run by the release
  cleanroom, drains (and, in a second variant, clears) every frame of one context at 40x15, 80x24
  and 240x80 and checks the capture and the resident memory stay bounded; its no-drain control
  fails.
- The framebuffer adapter now clears its headless capture at `headless_capture_limit` instead of
  replacing the context; `capture_rotations` counts those clears.

### Added — C ABI v1 (additive, 60 → 62 symbols): key kinds ([#274](https://github.com/femboy2112/libgibson/issues/274))
- **`gibson_poll_event_kind(ctx, timeout_ms, out_event, out_key_kind)`** polls like
  `gibson_poll_event` and also reports `GIBSON_KEY_KIND_CHAR` or `GIBSON_KEY_KIND_SPECIAL` for a key
  (`GIBSON_KEY_KIND_NONE` otherwise). `GIBSON_KEY_*` occupy 0x1001–0x100D, which are also Unicode
  scalar values (Myanmar letters U+1001–U+100D), so `key_code` alone cannot tell a typed `ခ` from
  Enter; the kind can. `*out_event` is written exactly as `gibson_poll_event` writes it.
- **`gibson_push_event_kind(ctx, event, key_kind)`** pushes a key with its kind, so a headless
  context can be given a typed U+1001–U+100D. `GIBSON_KEY_KIND_SPECIAL` with a non-`GIBSON_KEY_*`
  code, and any other kind, are `GIBSON_ERR_INVALID_PARAM`.
- **The legacy functions are unchanged**, ambiguity included: `gibson_push_event` reads
  0x1001–0x100D as the special keys, and `gibson_poll_event` reports both with the same code. No
  constant changed value and `gibson_event_t`'s layout is untouched; the kind is a separate
  out-parameter, not the reserved `_pad` bytes. `gibson_push_event` lets callers leave `_pad`
  uninitialised, and v1 libraries before #273 do not write it, so a flag there could not be trusted
  in either direction. A client that uses the new pair gets a link error, never a misread key, on an
  older `libgibson.so.1`. Python and Go bindings expose neither event function.

### Added — C ABI v1 (additive, 59 → 60 symbols): injected input for a headless context
- **`gibson_push_event(ctx, const gibson_event_t *event)`** queues a key, paste or resize event on
  a headless context; `gibson_poll_event` delivers queued events oldest first, each exactly once,
  without waiting. The encoding is exactly what `gibson_poll_event` writes, so a pushed event polls
  back unchanged. The queue is bounded at `GIBSON_EVENT_QUEUE_CAPACITY` (1024): a push beyond it
  returns the new **`GIBSON_ERR_QUEUE_FULL`** (-5) and displaces nothing. Unrepresentable events
  (unknown type or modifier bits, a non-scalar `key_code`, a null or non-UTF-8 paste, a zero-sized
  resize) and terminal contexts are refused with `GIBSON_ERR_INVALID_PARAM`. A delivered resize
  sets the headless geometry. Existing clients see no change: a headless poll with nothing pushed
  is still `GIBSON_EVENT_NONE`, and the new status is returned only by the new function.
- `gibson_event_t`'s layout is now frozen by test (it becomes caller-built input).
- The installed-SDK forcing consumer moves its marker only by pushed-and-polled keys; its frames
  and checksum are unchanged (`15734824182601092804`).

### Changed — Rust library (`gibson::capability`), performance ([#260](https://github.com/femboy2112/libgibson/issues/260))
- **256-colour quantization is O(1) and exactly equal to the old scan.** `nearest256` no longer
  scans 240 palette entries per colour: the cube's nearest entry is the per-channel nearest level
  (lowest on ties) and the grey ramp contributes only the two levels around the channel mean, the
  grey winning only on a strictly smaller distance — the old scan's exact rule. Proven equal over
  all 2²⁴ colours (`nearest256_exhaustive`, run in release). On the v0.7 raster-ingest bench the
  256-colour render stage drops from 8.2 / 14.7 / 35.1 ms to 0.37 / 0.85 / 1.96 ms (p50 at
  80×25 / 120×40 / 200×60) with byte-identical frames. No output change.

### Fixed — Rust library (`gibson::context`) and C ABI behaviour ([#259](https://github.com/femboy2112/libgibson/issues/259))
- **A headless `Context` no longer reads the real terminal.** `TerminalSession::headless` presents
  as a TTY so rendering behaves as on a terminal, and `Context::poll_event` / `run_once` (and
  `gibson_poll_event`, which calls `poll_event`) gated only on that flag — so a headless context
  read the process's controlling terminal: with no terminal it failed (`"Failed to initialize
  input reader"`), and under a real PTY it **consumed the user's keystrokes**. `poll_event` now
  answers `Ok(None)` at once on a headless context and `run_once` takes the off-terminal branch;
  headless input arrives only through an injected source (`Context::poll_from`). Non-headless TTY
  contexts are unchanged. `tests/headless_input_boundary.rs` (5) includes a real-PTY leg that typed
  `x` + Enter and saw both reach the headless context before the fix.

### Added — Rust library, experimental (v0.7 nodes C, D)
- **`gibson::hold::HoldTracker`** — held-key inference for press-only terminals as an explicit
  model on a caller-supplied simulation tic: the first observation holds for `initial_grace`
  tics (bridging the autorepeat delay), each later one re-arms `repeat_timeout`; a hold ends by
  `Timeout` or by a declared release (focus lost, disconnected, explicit) and is returned as an
  `EndedHold` record — **never** a synthetic key-release `Event`. Pure (no clock, no I/O); ended
  holds are returned, not retained; a regressing tic is an error that changes nothing.
  `tests/input_hold.rs` (18) checks it against an independent per-tic simulator on 300 seeded
  traces, proves the ended records independent of advance cadence, and runs a 35 Hz
  `FixedTimestep` simulation under 15/35/60/144 fps presentation with identical results.
- **`gibson::event_source::InjectedQueue`** (+ `InjectionReceipts`, `QueueFull`) — a bounded FIFO
  `EventSource` for drivers that inject input while the consumer runs: fixed capacity, a push into
  a full queue is refused with the event handed back, polls never wait, and receipts conserve
  `accepted == delivered + len`. `tests/input_injection.rs` (10): identical headless frame digests
  across runs, across injection batching and against the `ScriptedSource` route; dropping,
  delaying or order-sensitively reordering one event changes them.

### Changed — examples: DEEP SIGNAL's held-key grace, measured ([#275](https://github.com/femboy2112/libgibson/issues/275))
- The default initial hold grace is now **25 tics (714 ms)**, up from 9 (257 ms). Key timelines
  recorded by the game itself in xterm, with keys driven through X/XTEST, put the first autorepeat
  18 tics after the press at 500 ms / 30 Hz and 23 tics at X's default 660 ms / 25 Hz. The old
  grace stalled every hold for 9 or 14 tics; the new one holds both without interruption
  (`tests/deep_signal_hold_timeline.rs`, which also binds the game's recorded trace header to the
  checked configuration). A press-only terminal cannot tell a tap from the start of a hold, so a
  tap now moves for 25 tics; `--hold-grace-ms` sets another value. Recorded traces replay with the
  grace in their header, so older traces are unaffected.

### Added — examples (v0.7 node E; not library API)
- **`examples/deep_signal` — DEEP SIGNAL**, an original playable first-person terminal maze built
  only on the public API (no copyrighted assets or engine code): seeded sectors of growing size,
  signal shards (depth-tested diamonds), hatched hazard floors that pulse live, an uplink that
  opens when every shard is recovered. A fixed-point 35 Hz simulation (`FixedTimestep::exact`)
  that presentation can fall behind but never changes; held keys via `HoldTracker`; inputs stamped
  with their simulation tic (`--record`); DDA raycasting straight into an `RgbRaster` at the
  canvas's sub-cell resolution; colour half-blocks or Braille / block / ASCII monochrome with a
  width-aware HUD (40 columns); every transition effect reads world state. `--selfcheck` injects a
  scripted trace through `InjectedQueue` into a headless context and requires identical replays,
  the same world digest at presentation cadences 1/2/3/7 and at 40×15 ASCII, and a painted view.
  Tests: `deep_signal_sim` (6) and a real-PTY playthrough `deep_signal_pty` (2: 80×24 colour,
  40×15 ASCII); `deep_signal_selfcheck` runs the self-check in the suite.
- **Every sector is winnable.** Shards are placed only on tiles reachable from the start while the
  uplink is still shut (in a maze the exit cell can be the only link to part of the map; the first
  version stranded shards behind it in roughly 1 sector in 6, found by an independent review that
  ported the generator). The solvability test now uses its own BFS that treats the shut exit as a
  wall, over 402 seeds × 3 sectors — red against the old generator (seed 5, sector 3).
- **Record and replay.** `--record FILE` writes a versioned trace (header with seed, sectors and
  hold timeouts; one line per simulation-relevant input stamped with its tic; the end tic) and
  `--replay FILE` reproduces the run headlessly. `deep_signal_record_replay` plays a real-PTY
  session, quits with Ctrl-C, replays the trace and requires the same tic count, world digest
  **and trajectory hash** (every tic's position, angle and integrity) — a replay applying inputs
  one tic late kept the final digest but is caught by the trajectory. Ctrl-C now quits and no
  Ctrl- or Alt-chord moves the player.
- **Pacing under a slow terminal (v0.7 node F):** the loop keeps the simulation at wall-clock pace
  and drops presentations — after a blocked frame write up to 70 owed tics (2 s) run before the
  next frame, the rest deferred, never discarded. `tests/deep_signal_pacing_pty.rs` drives a real
  PTY with a held turn: a full-speed reader presents ~165 of 175 tics in 5 s; a ~50 KB/s reader
  still runs 172–175 tics (≥ 94% of those owed) while presenting ~15 frames. With the previous
  4-tic catch-up cap the same slow reader dilated the simulation to 45 of 178 tics — the test is
  red for that. A PTY and a throttled reader, not a physical terminal's display rate; a terminal
  that stops reading entirely stalls the (synchronous) loop.
- **Resize during play (v0.7 node G, game level):** `tests/deep_signal_resize_pty.rs` resizes a real
  PTY mid-play (80×24 → 120×40 → 40×15 → 100×30 → 80×24) with a turn held, feeding one terminal
  model resized at the exact byte offset; after each resize every view row is painted, the HUD sits
  on the last two rows in the right form, nothing exceeds the width, and quitting restores the
  screen. It exposed a flaw: the HUD chose its wide/compact form from `Event::Resize`, which can
  arrive late (here alongside held keys), so at 100×30 the compact HUD lingered; the HUD is now a
  `Node::canvas` that chooses from its laid-out width, so it cannot lag the real geometry.

### Added — examples / tooling (v0.7 nodes A, B; not library API)
- **`docs/V07_BASELINE.md`**: the merged v0.6 engine measured before any v0.7 library change — the
  installed-SDK forcing consumer at `f6cd8c7` and `7f57047` (identical output, checksum
  `15734824182601092804`), `check-abi` 59 = 59, and the first raster-ingest numbers.
- **`examples/raster_ingest_bench`**: a deterministic moving 320×200 software framebuffer fitted
  onto 80×25 / 120×40 / 200×60 (consumer-side integer box filter — LibGibson has no public fit
  primitive, #128/#129), lowered in colour and four monochrome glyph families, rendered by the one
  renderer, written into an OS pipe (not a terminal); per-stage timings, bytes, dirty cells and
  allocations; `--selfcheck`.
- **`tests/raster_ingest_oracle.rs`** (8): exact pattern fits, `fit_box` partitions the source
  (each source pixel reaches exactly one sample, driven through `fit_box`), averaging equal to a
  float route, a measured aspect bound, and — for all 21 timed configurations — the emitted ANSI
  bytes parsed by a terminal model match the lowered cells (glyph family checked independently;
  exact RGB in truecolor). One frame per configuration, not every timed frame.
- The bench's first finding — 256-colour render 4.7–7.2× truecolor on dense frames — is filed as
  #260.

## [0.6.0] — 2026-11-08 · GitHub prerelease from release/v0.6.0-rc (tag v0.6.0 → 492d0fe)

**Release state 2 of 3** ([Release Contract §1](docs/RELEASE_CONTRACT.md#1-versioning-model)): a
GitHub prerelease ([v0.6.0](https://github.com/femboy2112/libgibson/releases/tag/v0.6.0)); no registry
upload. The source version was prepared on 2026-11-05 at `9a899e1`, which is never tagged. "Operation
Instrumental Renaissance": the v0.6 integration merged through #253 (merge `f6cd8c7`, independent
audit approval of exactly `eaa0d1c`). `GIBSON_ABI_VERSION` stays 1 — the C ABI grew only by
**additive** functions (52 → 59, compatible, no version bump); MSRV stays 1.85.

The released tree is `9a899e1` plus reviewed backports of the `main` fixes for #264, #269/#15, #259
and #285, and the release engineering (the #297 disclosure and the approved policy text and
checker). The tagged tree's own CHANGELOG lists them under "Release candidate". The backported fixes
also appear in `[Unreleased]` above, because `main` carries them for 0.7.

### Release engineering — source version 0.6.0, RXVII
- **Version prepared in source:** `Cargo.toml`, `Cargo.lock` (the `libgibson` entry only),
  `bindings/python/pyproject.toml` and `THIRD-PARTY-NOTICES.md` move from `0.6.0-dev` to `0.6.0`.
  The packaged file list (`cargo package --list`, 183 files) and the resolved dependency tree are
  identical to `0.6.0-dev` apart from the crate's own version string.
- **Release states defined ([#257](https://github.com/femboy2112/libgibson/issues/257)):** the
  Release Contract §1 now distinguishes *prepared in source*, *tagged and released on GitHub* and
  *published through registries*, and states which one each milestone has reached. It had still
  called **0.5.0** "a prepared release candidate, not yet tagged" — v0.5.0 was tagged and published
  as a GitHub prerelease on 2026-10-24 (`v0.5.0` → `d8c7b99`). README, ROADMAP and
  STATE_OF_LIBGIBSON then said v0.6.0 was prepared in source, not released (it was released
  on 2026-11-08; see the heading above).

### Fixed — test harness (`event_pressure_lab`), RXVII ([#255](https://github.com/femboy2112/libgibson/issues/255))
- **Parallel event-pressure trials no longer collide on their scratch directory.** The trial
  directory was named `gibson-pressure-{pid}-{CLOCK_MONOTONIC µs}`; parallel test threads share the
  pid, so two trials starting in the same microsecond built the same path and the second failed with
  `EEXIST` — a false failure of a test whose code under test was fine. The name now carries a
  process-wide sequence (`gibson-pressure-{pid}-{µs}-{seq}`), creation stays exclusive
  (`create_dir`, never adopting an existing path), an existing name is skipped for a fresh one within
  a bounded number of attempts, and every other creation error propagates. The guard still removes
  the directory on drop; event-pressure expectations, deadlines and the ignored #15 acceptance are
  unchanged.
- `tests/event_pressure_trial_dir.rs` reproduces the legacy name verbatim as a negative control:
  with a frozen clock and barrier-synchronized starts it collides deterministically (1 of 16 creators
  wins), while the sequenced name gives 16 distinct directories. Test harness only — no library,
  ABI or behaviour change.

### Documentation — the terminal-write contract, RXVI ([#254](https://github.com/femboy2112/libgibson/issues/254))
- **No current document promises an atomic terminal write any more.** README, DESIGN §8,
  `docs/FEATURES.md` and the Release Contract (§2, §6) still described `TerminalTransaction` as
  "a single atomic write" / "atomic from the terminal's perspective", said the renderer "always
  knows" its live-region anchor, named the pre-RXIV `TerminalTransaction::commit`, and called
  `?2026` a "wire-atomic write"; the Go binding called sync updates "atomic terminal updates". The
  implementation (`commit_counted`: a counted write loop + one flush) and Release Contract §9 already
  said otherwise. They now state one contract: batching, not atomic I/O; a zero-byte failure is a
  certain no-op retried in place; a torn prefix or a failed flush voids the renderer's physical
  belief (`Renderer::physical_state_uncertain()`), fullscreen recovery repaints exactly and inline
  recovery abandons the torn region to scrollback; synchronized-update framing is a presentation
  request. `Context::set_sync_updates` and `gibson_set_sync_updates` gained doc comments saying
  so. Historical entries (ROADMAP Phase 1.75/1.85, the v0.5 roadmap, the acid-vs-crash note) are
  kept and marked superseded. Filesystem atomicity of capture/trace files (`atomic_write`) is a
  separate, tested contract and is unchanged.
- `tests/doc_terminal_write_contract.rs` guards the current documentation: it fails on the
  overclaims #254 found (14 hits at `4e33631`) and on the documents no longer stating the
  counted-write contract. No code, ABI or behaviour change.

### Fixed — Rust library (`gibson::input`, text-input painting and layout) and C ABI behaviour, RXV ([#252](https://github.com/femboy2112/libgibson/issues/252))
- **Text-input content can no longer reach the terminal as a control, and the editor's columns
  agree with the terminal's.** `paint_text_input` placed graphemes with `Surface::set_cell`,
  bypassing the control guard `Surface::print_str` applies, so a pasted TAB, a raw ESC and the
  CSI / OSC / DCS sequences it introduces, BEL, BS, DEL, NUL and C1 controls were emitted verbatim:
  on the untouched candidate, 19/19 hostile payloads leaked through `Node::text_input`, every
  representable one through `gibson_node_text_input`, and on a real PTY a pasted `ESC[2J` erased
  the screen. Widths also disagreed — after pasting `foo\tbar` the cursor said column 7, the total
  said 6, the compiler advanced 0 and the terminal 8.
  - **Insertion is single-line and control-free.** Every `TextInputState` path —
    `handle_event` (keys and bracketed paste), `insert_str`, `insert_char`, `with_text` — turns a
    run of line breaks (CR, LF, VT, FF, NEL, U+2028, U+2029) into one space and a TAB into one
    space, and drops every other control (`char::is_control`: C0, DEL, C1). `normalize_paste` is
    that normalization (a behaviour change for its direct callers).
  - **Painting is total.** For any string (the public `text` field, `Node::text_input`, the C ABI),
    a grapheme holding a control character or a line/paragraph separator, or of zero display width
    standing alone (a combining mark with no base, ZWSP, BOM, a bidi format character, a lone
    variation selector), is painted as **one `U+FFFD` cell**: still a visible cursor stop that
    Backspace removes, never a control.
  - **One width contract** (`TextInputState` docs): a grapheme of width 1–2 is one cell; a grapheme
    **wider than two cells** (a Devanagari conjunct with a spacing vowel sign such as `क्षि`, a run
    of Hangul leading jamo — which GB6 joins into one grapheme of any length) is laid codepoint by
    codepoint over consecutive cells, so the cells advance exactly as `unicode-width` and a
    per-codepoint terminal do; a substituted grapheme is one cell. `cursor_display_column`,
    `total_display_width`, `update_scroll`, the painter and the layout's intrinsic input width all
    use it. (Before, a 40-jamo paste was counted as 2 columns and overwrote the widget beside the
    input.)
  - **Scrolling keeps the whole cursor grapheme visible**: a wide grapheme at the right edge was
    left half outside the viewport and not drawn (and the hardware cursor was not placed).
  - `NumericInput::set_text` keeps its text exactly as given, so a control character stays
    `Unparseable` instead of being normalized into a different valid number.
  - Behaviour changes, no API change: `with_text` and `insert_str` now normalize;
    `total_display_width` sums per-grapheme cells instead of measuring the whole string;
    `handle_event` returns `false` for a key or paste that changed nothing (e.g. a paste of only
    controls), so a UI `on_edit` no longer fires for a no-op.
  - Evidence: `tests/text_input_glyph_boundary.rs` (issue reproduction, 21 hostile payloads × 6
    entry paths incl. the C ABI, a byte-grammar oracle, a vt100 screen/cursor/side-effect oracle,
    every cursor position, wide clusters beside a neighbouring widget, scrolling, deletion, 40×15
    mono, numeric text) and `tests/pty_text_input_paste.rs` (a hostile bracketed paste on a real PTY
    matches its sanitized control); painter-guard and sanitizer mutations each turn their own tests
    red.

### Fixed — Rust library (`gibson::renderer`), RXIV
- **Partial terminal writes no longer leave the renderer believing a torn frame never happened.**
  `TerminalTransaction` is not atomic I/O (synchronized-update framing is a display hint), and a
  failed commit could leave a prefix of a frame on the terminal while the renderer kept diffing
  against its old belief — after a torn frame the screen failed to converge for 119/143 cut points
  (fullscreen) and 120/157 (inline). Commits now count delivered bytes: a failure that sent
  **nothing** is certain (the emission state is restored and a retry diffs in place); a delivered
  prefix or a failed flush voids the physical belief, so the next fullscreen frame resets SGR,
  clears and repaints, and an inline region is abandoned to scrollback and re-allocated. Additive:
  `Renderer::physical_state_uncertain()`. (`tests/renderer_partial_write.rs`.)

### Added — examples / tooling, RXIV (experimental, not library API)
- **Interactive live instrument-film:** `field_cinema --live --interactive` (probe, target,
  tracked command, correlated verdict, verified-difference composition; `--step`,
  `--reduced-motion`, `--suppress`, `--shape ring`, `--mask-every N`, `--record DIR`,
  `--replay-film DIR`) and `field_cinema --envelope` (a measured resource/latency envelope).
- **`wire_instrument --trace PATH`:** commits the session's causal trace atomically.
- **Reference producer:** opt-in `--shape ring` (non-Gaussian holdout) and `--mask-every N`;
  defaults and the golden vector are byte-identical.
- **CI / release:** the installed-SDK forcing consumer runs in the Bindings job and the release
  cleanroom; a real-TTY input→state→render loop through ABI v1 (`ffi_realtty_probe abi-loop`).
- **Research:** a real-PTY stock-vs-patched crossterm A/B for #15
  (`docs/research/issue15_crossterm_ab/`). #15 is **not** fixed — blocked upstream.

### Changed — examples (experimental), RXIV
- **Cinematic TRANSFORM is gated by a typed effect contract** (`field_cinema/live.rs`): the RXIII
  `peak >= 0.8·target` rule transformed on an unchanged measurement (incl. a real no-op edit).
- **The live film runs on the instrument's certified transport/command channel** (`LiveSession`):
  id- and source-correlated verdicts, typed fail-closed errors, supervised producer lifetime.
- **Capture files:** writer and reader share one acceptance contract; an aggregate payload budget
  (256 MiB default); strict readers reject trailing bytes (`read_capture_prefix_with` is the explicit
  tolerant form); a different format version is `UnsupportedVersion`; files commit atomically.
- **Causal trace format v2 (`WIRETRC2`):** commands record their arguments; `Failed` verdicts and
  `TransportFailed` admission; v1 streams are `UnsupportedVersion`.
- **`wire_instrument`:** the ROI window is computed against the physical terminal and the probe is
  painted on the field; status clipping walks grapheme clusters.
- **Docs:** `SCENE_ALGEBRA_LAWS.md` — interchange holds under S ∧ C (aligned first tile AND
  commuting crossed writes); duration alignment alone is refuted. `scene.rs` docs stop calling
  `Parallel` a monoidal product and `to_node` a functor.

### Added — C ABI (`GIBSON_ABI_VERSION` stays 1; additive)
- **`gibson_create_headless_context(mode, cols, rows, **out)`** + **`gibson_get_rendered_output(ctx, buf, cap, *out_len)`:**
  a deterministic in-memory headless context plus a binary-safe readback of its ANSI frame
  stream, so C and Python consumers can reach byte-exact ANSI *frame* parity with the native
  renderer (#142, v0.6 Gate I). The v1 surface grows from 52 to 54 exported symbols; adding
  functions is ABI-compatible, so `GIBSON_ABI_VERSION` is unchanged. Python binding:
  `Context.headless(mode, cols, rows)` + `Context.rendered_output() -> bytes`.
- **`gibson_set_color_depth(ctx, depth)`:** overrides a context's color depth
  (`GIBSON_COLOR_DEPTH_{MONO,ANSI16,ANSI256,TRUECOLOR}`), so C and Python consumers can exercise
  the 256-color / 16-color / mono SGR *downgrade* across the ABI — the capability-fallback path
  the Gate I headless pair forced to truecolor and ceded (#142, v0.6 Gate J). The v1 surface grows
  from 54 to 55 exported symbols; still additive, so `GIBSON_ABI_VERSION` is unchanged. Python
  binding: `Context.set_color_depth(depth)` + the `ColorDepth` constants.
- **`gibson_get_frame_size(ctx, *cols, *rows)`** + **`gibson_get_frame_cell(ctx, x, y, *cell)`:**
  read back the resolved *styled cell grid* of the most recent frame — per cell, the grapheme, its
  display width, continuation/transparency flags, and the full style (fg/bg/attrs) quantized to the
  context's color depth, via the new `gibson_cell_t` (closing the styled-cell-grid half of the
  Gate I cede, #142, v0.6 Gate J). Also adds the Rust seam `Context::frame_size` / `Context::frame_cell`
  + the `FrameCell` view, and the inverse FFI converters `style_to_ffi` / `color_to_ffi`. The v1
  surface grows from 55 to 57 exported symbols; still additive, so `GIBSON_ABI_VERSION` is
  unchanged. Python binding: `Context.frame_size()` + `Context.frame_cell(x, y)` + `GibsonCell`.
- **`gibson_get_frame_cells(ctx, x, y, *out, max, *written)`** + **`gibson_get_frame_cell_glyph(ctx, x, y, buf, cap, *len)`** + the **`glyph_truncated`** flag (`gibson_cell_t._reserved[0]`):
  make the styled cell grid *honest* about grapheme clusters larger than the 16-byte inline cap (a
  ZWJ family emoji is 25 bytes). `gibson_get_frame_cell` now sets `glyph_truncated` when it cut a
  cluster; `gibson_get_frame_cell_glyph` returns the full (untruncated) bytes (size-query-first); and
  `gibson_get_frame_cells` reads a whole row or frame in one call with the exact same per-cell
  encoding. Corrects three docs that wrongly claimed "every grapheme fits" (#142, v0.6 Gate K). The
  v1 surface grows from 57 to 59 exported symbols; still additive, so `GIBSON_ABI_VERSION` is
  unchanged. Python binding: `Context.frame_cells()` + `Context.frame_cell_glyph(x, y)` +
  `GibsonCell.glyph_truncated`.

### Fixed — Rust library (`gibson::scene`)
- **`Scene::rename` now repairs a stale label index even when the new label equals the one
  already stored.** The fast-path early return fired before the `by_label` index was
  reconciled, so renaming an entity to a label a prior direct `entity_mut(id).label = …`
  write had already set left the index desynchronised. The early return now also requires
  the index to already resolve that label to this id. (#192)

### Changed — Rust library (`gibson::scene`)
- **`SceneEntity.id` is documented as unsupported to mutate.** Writing it through
  `entity_mut` corrupts label/position lookups; the never-recycled identity guarantee is now
  scoped to "absent a direct id write." Read-only enforcement is deferred to 0.7. (#192)

### Added — experimental (`instrument_workbench` example)
- **Keyboard-driven shape selection** in the workbench (issue #140). (#193)
- **Accountable edit outcomes (`EditOutcome`):** applied / no-op / refused / rollback-failed
  are now distinct; undo and redo no longer swallow their result; an accepted-command digest
  is kept separate from the attempted-intent digest. (#193)

### Fixed — experimental (`instrument_workbench` example)
- **Tick arithmetic saturates** over the full `u64` domain: `Step(n)` no longer rewinds at
  `u64::MAX` or overflows near `i64::MAX`, and `tick_wall` no longer overflows. (#193)

## [0.5.0] - 2026-10-24

**Interactive instruments** — a deliberate minor milestone that turns LibGibson's green
primitives into an interactive, observable instrument platform. All of the new surface is
**experimental** unless a release contract says otherwise. **No C ABI change**
(`GIBSON_ABI_VERSION` stays 1; none of the new surface is exposed through C, C++, Python or
Go — these are Rust additions); MSRV stays 1.85; existing terminal/TUI behaviour is
unchanged. One Rust CORE source-break (`Node.tag`) is documented below.

### Added — experimental (Rust)
- **Spatial input + painted-frame hit-map:** pointer/keyboard events resolve to semantic
  targets derived from the *same* resolved paint layout as the surface, honoring z-order,
  modal barriers, clipping, and disabled/hidden eligibility; stale pointer captures cancel
  with a typed reason (#127, integrated in the instrument platform #181).
- **Interactive authoring:** reversible edit sessions and drag gestures; compound undo with
  an explicit `RollbackFailed` on failed compensation (no silent "recovered" claim);
  identity-keyed selection; immutable shapes with narrow-phase hit testing; validated numeric
  editing (typed schema errors, no silent clamping).
- **Scientific observation:** `gibson::plot` observable geometry, a domain-neutral field view
  with point probes, explicit masked/non-finite sample states, and a disclosed auto-scale
  policy; world→raster→cell/field transforms report clipping and quantization rather than
  faking invertibility.
- **Bounded transport + telemetry:** bounded frame/command retention with a typed
  offered/accepted/refused/pending/delivered/acknowledged command state machine; a Python↔Rust
  wire conformance path; and deterministic, source-relative timing/retention telemetry
  (#182, #183, #184, #185).
- **Runnable demonstrations:** the flagship `instrument_workbench` and an independent
  `scope_editor`, each deterministic with PTY walkthroughs.

### Added — cinematic finale (example only; no third-party media)
- `examples/libgibson_intro_reaction` — the accepted v0.5 reaction cut: a keyed reaction
  composited into the LibGibson intro film on one authoritative 132 s edit clock, with a
  declared edit→narrative time map (`intro_seconds(t) = clamp(t/EDIT_END · INTRO_LAST)`,
  `EDIT_END = 132 s`, `INTRO_LAST = 71.5 s`), nine keyed reaction windows, deterministic
  `--sequence`/`--dump` frame export, and an opt-in original synthesized music bed (`--music`).
- **Rights-safe by construction:** the self-contained `embedded-media` build synthesizes an
  **original** chroma-key performer at runtime; **no third-party media ships** in the crate or
  the repository, and the build compiles from a fresh clone with no external assets. The real
  greenscreen reaction is an optional, local-only `--clip=PATH` input the user downloads
  themselves — see `examples/libgibson_intro_reaction/MEDIA_PROVENANCE.md`.
- The instrument-driven film that drives the picture from a live `Workbench`/`FieldView`/`plot`/
  producer causal chain is **deferred to 0.6.0** (maintainer scope decision); for 0.5 the new
  instruments ship as the separate executable demos above.

### Changed — CORE (source-breaking for raw struct-literal `Node` users)
- `gibson::Node` gained a public field `tag: Option<u32>` (semantic hit-target
  identity for the painted-frame hit-map, #127). External code that constructs a
  `Node` with a struct literal
  (`Node { kind, layout_style, children, computed_rect, surface_fx }`) must now add
  `tag: None`, or spread `..Node::new(kind)`. Code that uses `Node::new(..)` or the
  builders is unaffected. This is an intentional 0.5 CORE migration and is **not**
  advertised as source-compatible with every 0.4 struct-literal pattern. **No C ABI
  change** (`GIBSON_ABI_VERSION` stays 1; `Node` is not exposed through C/C++/Python/Go);
  MSRV stays 1.85.

### Release engineering
- The release preflight now executes the four `#[ignore]`d terminal-hangup liveness tests
  explicitly, in a serial `--include-ignored` PTY step on the Linux release host (they are
  otherwise skipped by `cargo test`), and refuses a non-Linux preflight rather than silently
  passing without that witness.

## [0.4.0] - 2026-10-02

**HumanMusic & Audio** — a deliberate minor milestone that adds a new **experimental,
Rust-only** realization axis: deterministic audio synthesis, and **HumanMusic**, a
procedural composer and band driven by LibGibson's semantic style/story state. All work
is additive. **No C ABI change** (`GIBSON_ABI_VERSION` stays 1; audio is not exposed
through C, C++, Python or Go); MSRV stays 1.85; existing terminal/TUI behaviour is
unchanged. HumanMusic's default realization is the historical `WRITTEN` profile; the
hardened `BAND` profile is opt-in. All music is synthesized at render time; no
prerecorded musical assets ship.

### Added — audio substrate (`gibson::audio`, experimental)
- A deterministic, device-free **offline renderer** (`OfflineRenderer`) on an integer
  sample clock with beat↔sample tempo mapping, so the same score renders the same PCM
  with no audio hardware.
- **DSP** primitives: oscillators including an alias-bounded FM oscillator, ADSR and
  exponential envelopes, a state-variable filter, synthesized drums, and production
  effects (chorus, damped delay, reverb, saturation, compressor, limiter), mixed through
  a bounded bus graph (`Dialogue` / `Music` / `Sfx` / `Master`).
- Pure-Rust **WAV** output (`i16` / `f32`).
- Media-clip audio for edit timelines: decodes a span of a **local, host-provided** clip
  via `ffmpeg` and plays it monotonically with a dialogue-ducking sidechain (used by the
  `reaction_soundtrack` example; no recorded audio is committed or shipped).
- Optional real-device output behind the **`audio-cpal`** feature (`cpal` 0.18.2 +
  `rtrb`; building it on Linux needs `libasound2-dev`). Off by default: the pure-Rust
  core adds **no mandatory dependency**, so default builds, `cargo test` and CI need no
  system audio libraries.

### Added — HumanMusic (`gibson::audio::human_music`, experimental)
- **Semantic trace → song → performance.** A `SemanticTrace` composes a `SongMap` (intent
  timeline, form, discourse plan and obligation ledger, theme, harmonic chart, charted
  rhythm, and recorded `composed_by` provenance). A performance is a separate fiber over
  that map, so one song can be played in any world, language, tempo or profile without
  becoming a different song.
- **Three sonic worlds**: BLACK_ICE, SWISS_SIGNAL and VAPOR95, each a dialect with its
  own instruments, harmonic vocabulary and production.
- **Performance profiles** (`PerformanceProfile`): `WRITTEN` (the default, historical
  realization), `POCKET` (the accepted pocket arm, kept byte-exact), and **`BAND`** (opt-in:
  rehearsed verb admission, an arbitrated *Balanced* drummer, the world's harmonic
  vocabulary as a source law, and pitch functions earned where they sound).
- **Checked performances**: `perform_checked` returns
  `Result<Composition, PerformanceRejection>`. A `PerformanceReceipt` measures the realized
  `Score` against the general performance laws, then admits, rejects or refuses it. Release
  invariants are typed errors, not debug assertions.
- **Causal ensemble behaviour**: players hear each other in a fixed causal order (lead →
  keys → bass → pad → drums). Calls own their material and answers derive from it. Voice
  lifetimes are shared by synthesis and hearing, and the sounding support answers for the
  chart's harmonic identity.
- **Harmonic and function laws**: every realized note carries a `PitchFunction` that must
  be earned where the note actually sounds (chord tone, passing, neighbour, an appoggiatura
  that resolves onto a core tone, …). Each world × language has a harmonic vocabulary.
  Planned settlements count only when a performed action discharges them.
- **Groove and expression**: a charted groove on indexed pocket positions, with timing
  transported metric → groove → feel → performed. Feel is a typed seam that is the
  identity in this release. Phrasing is expressive.
- **Percussion restraint**: a single arbitrated percussion surface, so drum ornaments yield
  where the band speaks (`with_drum_restraint`).
- **Identity by material**: what makes a song recognizable is declared anchors →
  identity-bearing material → realized evidence (`AnchorReport`), not which instrument
  played it.
- Diagnostics (witness audits, sonority, voicing, percussion and conformance reports) and
  listening labs under `examples/` (`human_music_lab`, `pocket_music_lab`,
  `cover_music_lab`, `cover_fidelity_lab`, `drum_restraint_lab`, `vapor95_style_lab`, …)
  that render WAVs with their receipts printed beside them.
- `reaction_soundtrack` example: an original HumanMusic score for the keyed reaction cut,
  ducking under the clip's dialogue, rendered on the same edit clock as the visuals.

### Added — Cover Mode (experimental)
- **`CoverMap`**: extraction is an identity projection of a reference (a `Composition`, a
  `ReferenceSong`, or an ordered chart). A cover is a fresh performance constrained only by
  that map and a `CoverTarget`.
- **Per-axis fidelity**: the `Loose`, `Interpretive`, `Faithful` and `Strict` presets select
  exact relations per axis, bounded by what the reference actually established
  (`FidelityReport`).
- **Noninterference**: after extraction the generator never reads the discarded reference.
  `CoverAdmission` = `PerformanceReceipt` + `CoverConformance`, measured on the realized cover
  rather than by re-extracting it.
- **Symbolic reference ingestion**: a `ReferenceSong` is built from attributed symbolic
  observations produced offline by a pinned parser (LilyPond and Standard MIDI File routes,
  checked in CI). The calibration reference is *Ode to Joy* (Mutopia #528, Public Domain).
- **Partial references**: axes a source never established stay unknown, and the cover
  writes new material inside the known scaffold. An impossible lift is a typed, lawful
  refusal (outside the target vocabulary, unplayable pinned attacks, a theme with nowhere to
  be stated) rather than best-effort output.

### Added — worlds and production
- **`WorldProduction`**: production is a typed world law, declared by the world and
  executed by the synth. It covers a memory-bus chorus, a tempo echo timed in note values,
  and a room with an input low-cut. A world that declares no stage computes none.
- **VAPOR95** is a revised world: a glassy FM keys source object, a supportive pad, a
  defined bass and a softer machine kit, with chorus, echo and space as memory, at 84 BPM.
- No prerecorded musical assets. The only musical source file in the repository is the
  public-domain symbolic score behind the Ode ingestion fixture (under `docs/fixtures/`,
  not shipped in the crate or the SDK).

### CI
- The `--all-features` build installs the ALSA development headers for `audio-cpal`.
- A new **Symbolic source parser laws** job checks the pinned offline parser.

### Unchanged
- C ABI (`GIBSON_ABI_VERSION` = 1) and the C / C++ / Python / Go wrappers, MSRV 1.85
  (with and without `audio-cpal`), the single rendering pipeline, and terminal/TUI
  behaviour. Outside `gibson::audio`, the only library change is the new module
  declaration.
- Distribution: GitHub source plus the native SDK archive. LibGibson is still not
  published to crates.io, PyPI or the Go module proxy.

### Known limitations
- `gibson::audio` and HumanMusic are **experimental, Rust-only APIs**. They may change
  incompatibly between releases.
- **One classified checked-performance rejection.** A 1,824,768-performance `BAND` search
  produced 12 rejections, all from one song in VAPOR95 at ≥ 96 BPM across drummer
  settings. `perform_checked` rejects it (a held-identity flip) instead of admitting it.
  It is tracked as known debt.
- Production effects (chorus, echo, room tails) are acoustic treatment outside the
  `Score`, so performance receipts do not hear them. The fixed 2.5 s render tail can end
  while the last echo repeats are still decaying.
- Receipts certify the stated musical laws, not musical quality. Quality is judged by
  listening.
- Linux x86_64 is the only release target.

Design record and evidence:
[docs/HUMAN_MUSIC_ARCHITECTURE.md](docs/HUMAN_MUSIC_ARCHITECTURE.md),
[docs/HUMAN_MUSIC_MERGE_READINESS.md](docs/HUMAN_MUSIC_MERGE_READINESS.md),
[docs/HUMAN_MUSIC_COVER.md](docs/HUMAN_MUSIC_COVER.md),
[docs/HUMAN_MUSIC_VAPOR95.md](docs/HUMAN_MUSIC_VAPOR95.md).

## [0.3.1] - 2026-09-28

**Temporal hardening + keyed compositing** — a compatible patch release. All work
is additive: the **experimental, Rust-only** `gibson::temporal` axis is extended and
hardened, and two research examples demonstrate keyed-video compositing through the
one existing renderer. **No C ABI change** (`GIBSON_ABI_VERSION` stays 1); MSRV stays
1.85. The temporal feature remains **off by default** and its perceptual benefit is
still setup-dependent and **unproven** (see
[docs/TEMPORAL_DISPLAY_VALIDATION.md](docs/TEMPORAL_DISPLAY_VALIDATION.md)). No new
spatial resolution, no video-playback claim, and no source media is shipped.

### Added (experimental, Rust-only `gibson::temporal`)
- **Regional motion gating** (`set_motion_region`/`clear_motion`): a moving region is
  held static while settled regions may refine; re-activated cells reseed so no stale
  residual leaks in.
- **Incremental regional reprojection** (`set_target_region`): reprojects only the
  overlapping cells and, as of this release, maintains eligibility and every
  diagnostic aggregate **incrementally** — work proportional to the touched region,
  not the whole grid (fixes #68).
- **`advance_into(&mut Surface)`**: reuses the caller's `Surface` allocation, bit-for-bit
  identical to `advance`.
- **Opt-in transport-aware rate-distortion** (`set_transport`): a cell-level
  keep/change decision by a λ benefit with an optional per-frame dirty budget;
  mean-preserving. The default remains the per-dot residual.
- **Capability-resolved emitted-swing safety**, a **jitter gate**, **nondestructive**
  target eligibility, and **live reclassification** on policy/color-depth change;
  Mono is always static.
- Exact sRGB8→linear **LUT** (bit-identical to the transfer function).

### Added (research examples)
- **`temporal_observatory`**: a procedurally-lensed black-hole flagship whose acquire-
  lock state machine drives region motion gating.
- **`temporal_projector_bench`**: projector/runtime microbenchmarks.
- **`temporal_video_compositor`**: chroma-keys a greenscreen clip and composites the
  keyed subject over a designed, temporally-stabilized LibGibson background, exploiting
  the greenscreen's ~80% transparency to reproject only the subject's bbox. Offline
  ffmpeg bake into a cached raw-RGB film with a **source-identity cache fingerprint**
  and atomic writes; the runtime never touches the codec.
- **`libgibson_intro_reaction`**: a directed "reaction cut" that composites a keyed
  subject **into** the existing `libgibson_intro` film via a **surface-local**
  compositor — the base film is reused verbatim and stays byte-identical outside the
  subject — with a deterministic two-clock (edit vs. narrative) director. See
  [docs/TEMPORAL_VIDEO_COMPOSITING.md](docs/TEMPORAL_VIDEO_COMPOSITING.md) and
  [docs/FRANK_REACTION_CUT_PLAN.md](docs/FRANK_REACTION_CUT_PLAN.md).

### Fixed
- **#68**: `set_target_region` no longer performs an O(total-cells) reclassification on
  every call; the accounting is incremental with a bounded worst-swing fallback, proven
  cell-for-cell against the retained full-recompute oracle.

### Unchanged
- C ABI (`GIBSON_ABI_VERSION` = 1, 52/52 symbols), MSRV 1.85, the single rendering
  pipeline (all temporal output is an ordinary `Surface`). The `libgibson_intro`
  example is behaviorally unchanged.

## [0.3.0] - 2026-09-27

**Temporal Rendering Milestone** — a deliberate minor bump (see the release
contract's "deliberate minor milestones"): high-cadence frame-pacing and
observability infrastructure plus an **experimental, Rust-only** temporal cell
realization axis, all flowing through the one existing renderer. **No C ABI change**
(`GIBSON_ABI_VERSION` stays 1); MSRV stays 1.85. The temporal feature is **off by
default** and its perceptual benefit is setup-dependent and **unproven** — see
[docs/TEMPORAL_DISPLAY_VALIDATION.md](docs/TEMPORAL_DISPLAY_VALIDATION.md).

### Added (CORE)
- **`FramePacing`** (`#[non_exhaustive]`, #62): opt-in `PhaseLocked` frame pacing
  that anchors deadlines to the scheduled cadence and skips missed deadlines without
  drifting; `CompletionRelative` remains the default (`Context::set_frame_pacing`).
  Phase-locked missed-deadline observability via
  `FrameScheduler::missed_periods_last_frame`/`missed_periods_total` and the matching
  `Context` accessors.
- **`FrameReport`** (`#[non_exhaustive]`, #64): a Rust-side per-frame snapshot
  separating generation (layout/paint/diff/ANSI construction) from the blocking
  write/flush, with exact/affected/total cells, bytes and full-repaint
  (`Context::last_frame_report`). The ABI-v1 `RenderStats` layout is unchanged.

### Added (EXPERIMENTAL, Rust-only — `gibson::temporal`, #63)
- **`TemporalDisplayProcessor`**: static-first, safety-gated, cadence-aware temporal
  realization emitting ordinary `Surface`s (no second renderer). Default static;
  residual temporal luminance requires a caller-supplied measured `PresentationProfile`
  and passes a per-cell **emitted luminance-swing** check that freezes any cell whose
  real `|Y(fg)−Y(bg)|` exceeds the depth cap, with cadence hysteresis, reduced-motion
  and content-motion overrides.
- **Exact two-color subcell projector**: `project_rgb_subcells` (127 complement-pair
  representatives, bit-identical to a full scan) and the batch `project_braille_image`
  / `BrailleImageProjection`, with ideal and **emitted (post-quantization)** error.
- **`TemporalBrailleField`**: a sigma-delta subcell modulator with a bounded
  residual-over-static path, optional coherent-subsampling-robust dither, explicit
  reset/invalidation, and an authoritative static fallback.
- **`temporal_cadence_beacon`** and **`temporal_display_lab`** examples; the beacon
  logs emission telemetry (`--csv`, `--photodiode`) for the external presentation
  experiment.

### Changed
- `#61`: remaining single-character glyph realizations use `Glyph::from_char`
  (behaviour-preserving allocation cleanup).
- `FrameScheduler::max_fps` is now private; use `set_max_fps` (which resets the timing
  epoch) and the `max_fps()` getter. Breaking for direct field access; no in-tree
  consumer used the field.
- New public 0.3 types are sealed `#[non_exhaustive]` (`FramePacing`, `FrameReport`,
  `PresentationProfile`, `TemporalSafetyPolicy`, `TemporalGate`, `TemporalCellProjection`,
  `TemporalDiagnostics`) so future fields/variants are additive; `ResetPolicy` stays
  exhaustive.

### Documented
- Temporal display processor design; the GPU research contract with a probe + measured
  CPU projector baseline (no `wgpu` in core; GPU left honestly unmeasured); the
  validation report (evidence classes, the one physical experiment, the
  accessibility/safety contract). Reconciled the synchronized-updates policy in the
  release contract (requested, not negotiated; the detected capability is unused).

## [0.2.5] - 2026-09-27

Umbrella-cleanup patch — the sixth release in the v0.2.x train, from the Europa
v0.2.0 consumer campaign. Resolves the last open findings of the Palimpsest
ergonomics umbrella (#48): a `&self` render-stats read (E-09), documentation of the
remaining small sharp edges (E-09), and the experimental-layer pointer for
raw-`Node` consumers (E-04). **No C ABI change** (`GIBSON_ABI_VERSION` stays 1); the
one code change is an additive, source-compatible receiver relaxation
(`&mut self` → `&self`) with no CORE break. `gibson::ui` stays EXPERIMENTAL and
Rust-only.

### Added
- **`Context::stats()` reads through `&self`** (#48 E-09): render statistics can now
  be read through a shared borrow rather than requiring `&mut`. The renderer's
  absolute counters are already mirrored into the scheduler on every render/commit
  path, so the read overlays them onto a copy of the accumulated stats and returns by
  value without mutating the context. Source-compatible — existing `&mut`-receiver
  call sites continue to compile unchanged.

### Documented
- **Kind-specific builders** (#48 E-09): `Node::background` and `Node::scroll_offset`
  now document that they apply only to their supported node kinds (box/bordered, and
  text-input, respectively) and are a silent no-op on any other kind, with the
  supported alternative for giving text a background. A mis-attached doc comment on
  `scroll_offset` (it described `rich_text`) is corrected.
- **Headless color depth** (#48 E-09): `Context::headless` documents that it defaults
  to truecolor — unlike an env-detected real terminal — and points at
  `set_color_depth` / `set_capabilities` to reproduce a lower-depth terminal in a
  capture. The default is intentionally left unchanged to keep captures stable.
- **Narrow panel titles** (#48 E-09): `Node::panel` documents the ≥5-column threshold
  below which no title is drawn, and that an over-long title is truncated without an
  ellipsis above it.
- **Experimental ergonomic layer** (#48 E-04): `docs/UI_LAYER.md` adds a pointer for
  raw-`Node` consumers — keyed focus, lists/tables, design skins and finite motion
  already exist ergonomically in the experimental `gibson::ui` layer — with the
  "experimental, may change" framing and guidance on composing the two paths.

## [0.2.4] - 2026-09-27

Viewport and focus foundations patch — the fifth release in the v0.2.x train, from
the Europa v0.2.0 consumer campaign (#39, #40; #42 resolved by documentation). Adds
the item-oriented viewport windowing and incremental focus-ring eviction that
scrollable, filterable list UIs otherwise re-derive by hand. **No C ABI change**
(`GIBSON_ABI_VERSION` stays 1); every change is an additive Rust method — no CORE
break, no signature change, no new struct fields — and `ViewportState` / `FocusRing`
are Rust-only (never on the C-ABI surface).

### Added
- **Viewport windowing helpers** (#39): `ViewportState::visible_range(total_items,
  item_height, view_height) -> Range<usize>` returns the item indices at least
  partially on screen (edge-clipped items included); `ensure_visible(index,
  item_height, view_height)` scrolls only as far as needed to reveal an item (no-op
  when already visible); `scroll_to_item(index, item_height)` pins an item to the
  top. Zero-height inputs are guarded (no divide-by-zero); like the existing scroll
  methods these do not self-clamp against the content end, so follow with `clamp`
  where that invariant is needed.
- **Incremental focus eviction** (#40): `FocusRing::remove(index) -> Option<FocusId>`
  removes a member (e.g. a list item filtered out), shifts later members down, and
  transfers focus to a surviving neighbour (the next survivor, or the new last), so
  focus is never silently lost. Outstanding `capture` entries are reindexed the same
  way, so a modal opened before the removal still restores the correct widget on
  `release`. Replaces the rebuild-the-whole-ring workaround.

### Documented
- **Responsive layout across size classes** (#42): `docs/UI_LAYER.md` now documents
  that size classes are a view concern — branch on `BuildCx.environment.width` and
  share `.key(...)` across variants; keyed `UiRuntime` reconciliation preserves focus
  and in-flight motion across the per-frame rebuild, so a resize is not special-cased
  and not expensive. `row().responsive(below)` remains the shortcut for the common
  row→column collapse. No "breakpoint framework" is added: how many size classes
  exist and where they break are application policy, not layout mechanics.

## [0.2.3] - 2026-09-27

Layout and custom-graphics-seam patch — the fourth release in the v0.2.x train,
from the Europa v0.2.0 consumer campaign (#36, #43, #48 E-05/E-06). Closes the
"size-blind Surface" gap: custom graphics can now be built at the size the layout
engine assigns, composited through the one existing rendering pipeline. **No C ABI
change** (`GIBSON_ABI_VERSION` stays 1); every change is additive with no CORE
break. `gibson::ui` stays EXPERIMENTAL and Rust-only.

### Added
- **Size-aware deferred canvas** (#43): `Node::canvas(|rect| -> Surface)` builds
  its surface **after** layout, receiving the node's resolved cell rectangle, so
  consumers no longer replicate Taffy's math to guess a size before layout. Sized
  by ordinary layout builders (`flex_grow` / `width` / `percent_width`); composited
  and clipped exactly like a raster. The callback never runs during layout, so
  there is no layout↔paint feedback loop and no second renderer.
- **Percentage sizing on `Element<A>`** (#36): `.percent_width(f32)` /
  `.percent_height(f32)`, lowering to Taffy `Dimension::Percent` — the semantic
  counterpart of `Node::percent_width`, for proportional split panes.
- **Clipped sub-surface view** (#48 E-06): `Surface::clip(Rect) -> SurfaceClip`,
  a translated `&mut` handle whose `set_cell` / `print_str` use clip-local
  coordinates and drop out-of-bounds writes — no per-call bounds guard, wide-glyph
  invariants preserved.
- **Braille compositing helpers** (#43): `BrailleCanvas::blit_to_surface(dst,
  style)` overlays dots onto a surface while preserving each cell's background
  (vector geometry over a shaded raster), and `Glyph::from_char(char)` builds a
  glyph without the `String` allocation of `Glyph::new(&c.to_string())`.

### Documented
- **`Node::surface` flex behavior** (#48 E-05): documented that the preset
  `Dimension::Length` overrides flex allocation (a full-width raster can silently
  push a sibling off-screen) and how to make a raster elastic; `Node::canvas` is
  the size-aware alternative.
- **Dense / canvas UIs** (#48 E-03): a crate-doc note on the "`Node` for chrome,
  `Surface` / `canvas` for the drawing" composition seam.

## [0.2.2] - 2026-09-27

Output, testability, and observability patch — the third release in the v0.2.x
train, from the Europa v0.2.0 consumer campaign (#45, #47, #48 E-02, E-07). Makes
finalized scrollback output intentional, the composed frame inspectable, and the
renderer's exact changed-cell count observable. **No C ABI change**
(`GIBSON_ABI_VERSION` stays 1); every change is additive with no CORE break.
`gibson::ui` remains EXPERIMENTAL and Rust-only.

### Added
- **Wrap-policy-aware scrollback insertion** (#45):
  `Context::commit_text_with_mode(text, WrapMode)` and
  `Context::insert_text_before_live_with_mode(text, WrapMode)` (plus the
  `Renderer` equivalents). `WrapMode::NoWrap` preserves preformatted content —
  aligned tables, diffs, ledgers — at its natural column width instead of
  re-flowing (`WordWrap`, the previous hard-coded behavior) or silently clipping
  wide rows. A preformatted line wider than the terminal is inserted via the
  always-correct repaint path so the live-region anchor never drifts. Control
  characters stay neutralized at the cell-model boundary, so untrusted text still
  cannot inject terminal controls. The existing `commit_text` /
  `insert_text_before_live` are unchanged (word-wrap) and source-compatible.
- **Visible-frame snapshot** (#48 E-02): `Context::last_frame_lines() -> Vec<String>`
  (with `Renderer::last_frame_lines` and `Surface::to_visible_lines`) returns the
  composed live frame as visible text — wide-glyph continuation cells collapsed,
  trailing blanks trimmed — so headless tests can assert on what the screen says
  without building a private terminal emulator. Raw wire bytes remain available
  via `rendered_bytes` / `take_output`.
- **Exact changed-cell observability** (#47): `Context::last_exact_changed_cells()
  -> usize` exposes the renderer's precomputed
  `SurfaceDiff::exact_changed_cell_count()` for the last frame — the exact
  semantic delta, distinct from the logical affected footprint
  (`RenderStats::dirty_cells`) and emitted bytes — with no second
  layout/paint/diff pass in the consumer. Rust-only; the ABI-1 stats struct is
  unchanged.

### Documented
- **Canonical interactive loop** (#48 E-07): `Context::run_once` now carries a
  copy-paste canonical loop (poll → rebuild-on-change → frame-paced render, with
  the non-TTY guard from #46), and `Event::Tick` documents that it is **not**
  emitted by the core input-driven loop — animation is paced via
  `animation_interval` + `render_if_due`, or the `gibson::ui::App` helper delivers
  per-iteration `AppEvent::Tick`.

## [0.2.1] - 2026-09-27

Consumer-correctness and honest-failure patch, driven by the Europa v0.2.0
external-consumer campaign (issues #35, #37, #44, #46, #48 E-01). Fixes narrow
`gibson::ui` interaction defects and makes three silent failure modes
observable. **No C ABI change** (`GIBSON_ABI_VERSION` stays 1); every change is a
bug fix or additive with no CORE source break. `gibson::ui` remains EXPERIMENTAL
and Rust-only.

### Fixed
- **`Alt`+`<char>` chords are no longer consumed as text input** (#35). A focused
  `text_input` / `TextInputState` treated `Alt`-modified characters as printable
  text — inserting a literal character and marking the event consumed — so an
  application's global `Alt` accelerators never fired while an input was focused.
  `Alt` chords are now left uninserted and unconsumed at both the
  `TextInputState` and `UiRuntime::handle_event` layers, so outer hotkeys receive
  them. Plain and `Shift`-modified text and the supported `Ctrl-a/e/u/k` line
  editors are unchanged; unsupported chords remain routable.
- **A stable identity key no longer forces an entrance animation** (#37).
  `UiRuntime` attached `MotionRole::Enter` to every `Key::Named` element, which
  under the `BLACK_ICE` acquisition grammar dissolved non-focused controls to
  blank cells on their first frame — making keyed controls invisible in headless
  captures, tests, and at `t = 0`. Entrance and reconciliation motion is now
  opt-in via `.motion(role)` (or a modal); an identity key no longer implies
  animation. Explicitly requested motion is unchanged.

### Added
- **`Node::separator(style)`** (#44): an untitled horizontal rule that needs no
  turbofish, unlike `Node::rule(None::<String>, style)`, whose generic
  `Option<impl Into<String>>` parameter cannot infer its payload type from a bare
  `None` (E0283). `Node::rule` is unchanged and remains source-compatible.
- **`Context::is_interactive(&self) -> bool`** (#46): reports whether the context
  drives a live TTY that is not a headless capture. An interactive loop built on
  `render` + `poll_event` renders nothing and receives no input on a non-TTY
  stdout; consumers can now detect that and fall back to a headless/`--dump` path
  (or exit with a diagnostic) instead of idling on a blank screen.
- **`Context::empty_root_renders(&self) -> u64`** (#48 E-01): counts `render` /
  `render_now` calls issued with no root set — the common "forgot `set_root`"
  mistake, otherwise a silent no-op indistinguishable from a legitimate empty
  frame. A root-backed render (even of an empty tree) never increments it.
  Rust-only; the ABI-1 stats struct is unchanged.

## [0.2.0] - 2026-09-26

The **experimental semantic UI milestone**. Adds `gibson::ui`, a Rust-only
composition layer above the existing framebuffer / renderer / terminal substrate. The
lower-level architecture is unchanged: UI trees lower to ordinary
`Node`/Taffy/`Surface`/`SurfaceFx`/`Focus`/`Context` objects through the one existing
rendering pipeline. **No C ABI change** (`GIBSON_ABI_VERSION` stays 1); no language
wrapper, renderer, layout engine, or terminal owner was added. The minor bump marks
the size of the new public Rust surface and the project milestone, not an ABI break or
a stability promise (see [`docs/RELEASE_CONTRACT.md`](docs/RELEASE_CONTRACT.md) §1).

### Added
- **Experimental `gibson::ui` semantic layer** (Rust-only, EXPERIMENTAL): an
  `Element<A>` component tree with an ordinary-Rust builder API (`panel`, `status`,
  `progress`, `button`, `choice`, `text_input`, `modal`, `toast`, `table`, `viewport`,
  `section`, `sparkline`, …), lowering to ordinary `Node` trees through an inspectable
  `Compiled<A>` boundary (`compile`, `compile_presented`, `UiRuntime::frame`).
- **Three built-in design grammars** (`Skin`): `VAPOR95` (windowed workstation),
  `BLACK_ICE` (compact instrument rails), `SWISS_SIGNAL` (editorial / Bauhaus). Each is
  a structural grammar (`Chrome::Window | Rail | Editorial`) that stays distinct in
  Mono and low color depths, not merely a repalette; all tokens are public for local
  customization, and the existing `Theme` gains no new semantics.
- **Typed interaction sidecar** (`InteractionMap<A>`): typed actions routed alongside
  the lowered `Node`, with an `EventOutcome<A>` consumed/actions dispatch contract.
- **Stable-key focus and presentation continuity** (`UiRuntime`): explicit named keys
  survive reorder / responsive reflow / same-key replacement / modal transitions /
  skin switching; focus capture and restoration across nested and sibling modals;
  duplicate keys are rejected transactionally before any state changes. Retained state
  is bounded by live semantic state — no frame history, no exit ghosts.
- **`BuildCx` / `PresentationCx` phase separation**: the construction-time environment
  (`BuildCx`, no focus/motion) is distinct from reconciled current-frame presentation
  (`PresentationCx`); `presented(...)` custom nodes run after reconciliation and observe
  current-frame focus/motion while returning ordinary `Node`s.
- **Finite semantic motion**: deterministic `elapsed → SurfaceFx` plans that settle to
  exactly nothing (settled frames emit zero bytes), with `MotionPreference`
  Full / Reduced / None, lowered onto the existing `SurfaceFx` operations.
- **Controlled text-input routing**: a focused editor owns editor-class input
  (printable / paste / Backspace / Delete / Left / Right / Home / End and the
  Ctrl-a/e/u/k line editors) even without an `on_edit` callback; the application owns
  all editor and domain state; unsupported chords remain routable.
- **Escape hatches** into the substrate remain local and explicit — `raw(Node)`,
  `surface(Arc<Surface>)`, `raster(Surface)`, `presented(...)`, a custom `Component`,
  and Scene / `SurfaceFx` integration — with no second rendering pipeline.
- **`ui_showcase` and headless visual proving grounds**: a deterministic operations
  workspace under all skins with `--dump` / `--transition` / `--at-ms` / `--profile`,
  plus automated skin × size × color-depth matrices and structural Mono goldens.

### Boundaries
- `gibson::ui` is **EXPERIMENTAL** and **Rust-only**: no C ABI surface and no
  C/C++/Python/Go bindings; it may change or be removed between minor releases and is
  not promoted to CORE by the move to 0.2.0.
- No mouse / hover router, no list virtualization, no exit-ghost retention, no async
  framework machinery.
- **Not a production-stability, API-finality, or universal-portability claim.** Existing
  platform limitations are unchanged: the core engine is implemented and tested on
  Linux x86_64 only, and the crossterm 0.29 resize/input-readiness collision (issue #15)
  still applies. Visual and hostile review to date is implementation-agent evidence, not
  independent human usability research.

## [0.1.1] - 2026-09-26

Additive, backwards-compatible fixes surfaced by external consumers testing the
v0.1.0 SDK. **No C ABI change** (`GIBSON_ABI_VERSION` stays 1); a consumer pinned
`libgibson = "0.1"` (`>=0.1.0, <0.2.0`) upgrades to this automatically.

### Added
- **Headless output capture** (issue #27): a context created with
  `Context::headless(...)` now renders into an in-memory buffer instead of the
  process stdout, so callers can read the exact rendered bytes back for
  snapshot / assertion testing without escape codes reaching a real terminal.
  New `Context::rendered_bytes() -> &[u8]` and `Context::take_output() -> String`;
  interactive contexts are unchanged.
- **`RenderMode` re-exported from the `context` module** (issue #28): so
  `use gibson::context::RenderMode;` resolves next to the `Context` methods that
  consume it, in addition to the existing crate-root `gibson::RenderMode`.

### Fixed
- `Context::headless` advertised itself for "automation, snapshotting and tests"
  but wrote rendered frames straight to the real process stdout, making output
  assertions impossible (issue #27).

## [0.1.0] - 2026-09-25

The first tagged release — **Engineering Alpha**, Linux x86_64. LibGibson is a
cell-framebuffer terminal UI engine: a declarative `Node`/Flexbox layout over a
grapheme-aware 2D cell surface, a differential ANSI compiler, and the flagship
**immutable-scrollback / mutable-live-region** model with an explicit physical
anchor. It exposes a language-neutral **C ABI v1** (ELF SONAME `libgibson.so.1`)
with C/C++/Python/Go wrappers, a color-capability ladder
(TrueColor → ANSI256 → ANSI16 → Mono) and an orthogonal glyph-realization ladder
(Braille → HalfBlock → Block → ASCII), plus the experimental Scene/Story and
software-graphics stack and the `libgibson_intro` short film (with its
first-contact prologue). **MSRV Rust 1.85**; dual-licensed **MIT OR Apache-2.0**.

Distribution is GitHub source plus a Linux x86_64 native SDK archive attached to
the release; it is **not** published to crates.io, PyPI, or the Go module proxy.

Known limitation: under a resize/input-readiness collision in crossterm 0.29, a
key can be briefly queued until a later key releases it (issue #15; upstream fix
filed as [crossterm#1128](https://github.com/crossterm-rs/crossterm/pull/1128),
not vendored). This is documented, not a claim of lossless delivery under that
collision.

### Added
- Versioned native shared object: the Linux `cdylib` now carries an ELF **SONAME
  `libgibson.so.1`** (stamped by a new `build.rs` via `-Wl,-soname`, guarded to
  Linux and scoped to the cdylib), whose major tracks `GIBSON_ABI_VERSION`. The
  staged SDK installs `libgibson.so.1` plus a `libgibson.so → libgibson.so.1`
  development symlink, so a dynamic consumer linked through `-lgibson` records
  `NEEDED = libgibson.so.1`. `libgibson.a` and pkg-config are unaffected.
- Pre-1.0 release contract (`docs/RELEASE_CONTRACT.md`): Rust API stability tiers
  (CORE vs experimental modules), the C ABI v1 policy and its independence from the
  package version, the wrapper ABI-compatibility policy, the MSRV floor with its
  evidence, and the supported-platform matrix.
- Declared MSRV: **Rust 1.85** (`rust-version` in `Cargo.toml`), established
  empirically and enforced by a scoped CI check.
- Relocatable native SDK staging (`scripts/release/stage-sdk.sh`): stages headers,
  shared/static libraries, relocatable `pkg-config` metadata, and license/doc files
  into a prefix with a deterministic checksum manifest.
- Canonical installable C++ header at `include/gibson.hpp`.
- C ABI v1 exported-symbol baseline (`abi/gibson-abi-v1.symbols`) and a regression
  check that guards against symbols silently disappearing or changing.
- C++ ABI gate: `gibson::Context`'s constructor now verifies
  `gibson_abi_version() == GIBSON_ABI_VERSION` before any other ABI call and throws
  `std::runtime_error` on mismatch (matching the Python/Go load-time checks); a new
  `gibson::Context::abi_compatible()` exposes the predicate.
- Declared-range consumer MSRV experiment (`scripts/release/msrv-consumer.sh`): a
  fresh, unlocked resolution of the declared dependency ranges on Rust 1.85, wired
  into the CI MSRV job and the release preflight so a dependency upgrade cannot
  silently raise the effective MSRV.
- Distributable dual-license payloads: `LICENSE`, `LICENSE-MIT`, and `LICENSE-APACHE`
  now ship in the Go module root and in the Python wheel/sdist; a
  `check-versions.sh` guard keeps them byte-identical to the repository-root
  originals.
- Sub-cell **glyph realization** layer (`src/glyph.rs`): `SubcellGlyphMode`
  (`Braille2x4`/`HalfBlock1x2`/`Block`/`Ascii`) realizes a 2×4 dot mask through
  progressively more portable glyph families; `transcode_surface_glyphs` is the
  generic chokepoint (a Braille glyph losslessly encodes its mask, so one pass
  re-realizes a whole surface and `Braille2x4` is a no-op — the hero path is
  byte-identical). `detect_glyph_mode` resolves `--glyphs=` / `LIBGIBSON_GLYPHS`
  / `Auto(TERM)` with a console-safe default on `TERM=linux`. Glyph realization
  is a distinct capability axis from terminal protocol/color and adds no C ABI
  surface (`GIBSON_ABI_VERSION` unchanged). See [`docs/GLYPHS.md`](docs/GLYPHS.md).
- `BrailleCanvas` gains `mask_at`, `glyph_at_mode`, `to_lines_mode`, and
  `to_surface_mode`; `glyph_at` now delegates to `glyph_at_mode(.., Braille2x4)`
  (behavior identical for all 256 masks).
- `glyph_capability_lab` example: a visual probe rendering each glyph family and
  one sub-cell field realized four ways, stating that font-repertoire realization
  requires visual inspection (documents the Linux VT observation).
- `libgibson_intro`, `fx_lab` (new "Glyph ladder" scene), and `runtime_observatory`
  (live sparklines) honor the glyph mode; deterministic dumps stay Braille.
- **First-contact prologue** for `libgibson_intro`: the default experience opens
  in an ordinary terminal and boots a credible — explicitly *simulated* — agent
  harness into immutable scrollback, then that information visibly *acquires
  structure*. A live region attaches while the boot log is still streaming (the
  two coexist), and the Harness framing assembles progressively: a header rail,
  the objective, the orchestration frame, then the four worker slots arriving one
  at a time in correspondence with their boot receipts, each resolving from a
  skeletal placeholder into a full agent record. ARCHITECT ignites to "active"
  only when the plan begins executing, its progress bar settling at exactly the
  film's `t = 0` value (an ignited head, no work done). After a short readable
  hold, terminal ownership hands cleanly to the fullscreen film at that same
  `t = 0` — a match cut into the existing 72-second timeline (unchanged; the boot
  log persists in scrollback). Layout is height-aware (compact on short
  terminals) and the assembly is monotonic (the live region never shrinks).
  `--no-prologue`/`--stage`/`--at`/`--dump` bypass it. Deterministic prelude clock;
  choreography witnesses plus a Unix PTY test cover the boot → assembly → handoff
  → film → restore arc.

### Changed
- The `transaction` module (`TerminalTransaction`, internal renderer plumbing) is
  now `pub(crate)` — it is no longer part of the public Rust API. It had no external
  consumers and was never re-exported; privatizing it before the first tag avoids a
  later `0.y` break. See `docs/RELEASE_CONTRACT.md` §2.
- The extension-point public enums are now `#[non_exhaustive]` (decided before the
  first release, since adding it later is itself source-breaking): `node::NodeKind`,
  `input::Event`, `input::KeyCode`, and `glyph::SubcellGlyphMode`. Adding a variant
  to these is a non-breaking `0.1.z` change; downstream `match`es must carry a `_`
  arm. `capability::Capability`/`ColorDepth` and `glyph::GlyphChoice` are left
  exhaustive (closed value sets). No `#[repr]`/C-ABI change (`GIBSON_ABI_VERSION`
  unchanged). See `docs/RELEASE_CONTRACT.md` §2.
- The published Rust crate now **excludes** the engineering-demo corpus (examples,
  integration tests and goldens, internal `docs/`, dev scripts, and the language
  bindings). It ships the library, the C header (for native-library builders), and
  the license/readme/design docs. Packaged manifest: 53 files (the demo corpus is
  excluded).
- The C++ wrapper header moved to `include/gibson.hpp`; `bindings/cpp/gibson.hpp` is
  now a forwarding shim so existing in-repository consumers keep working.
- The release preflight now **fails** (was a warning) when `THIRD-PARTY-NOTICES.md`
  differs from freshly generated output — a release candidate must not ship stale
  dependency notices.
- The Go clean-room consumer is now built from a module copy staged **outside** the
  checkout (no repository-relative `replace`), with `GOPROXY=off`, asserting no
  checkout path in `go.mod` and scanning the built Go binary for the checkout path —
  proving the Go wrapper has no dependency on repository-relative layout.
- `stage-sdk.sh --force` path guard hardened: it now normalizes the prefix and
  refuses catastrophic recursive-remove targets including system-path subpaths
  (`/etc/*`, `/lib/*`, `/bin/*`, `/boot/*`, …) and `$HOME` itself.

### Removed
- Unused `thiserror` dependency (it was declared but never used in `src/`).

---

The contents and exact cut procedure for a release are documented in
[`docs/RELEASE_CONTRACT.md`](docs/RELEASE_CONTRACT.md) and
[`docs/RELEASING.md`](docs/RELEASING.md). Ecosystem-registry publication
(crates.io, PyPI, Go module proxy) remains a separate, later decision.

[Unreleased]: https://github.com/femboy2112/libgibson/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/femboy2112/libgibson/releases/tag/v0.4.0
[0.3.1]: https://github.com/femboy2112/libgibson/releases/tag/v0.3.1
[0.3.0]: https://github.com/femboy2112/libgibson/releases/tag/v0.3.0
[0.2.5]: https://github.com/femboy2112/libgibson/releases/tag/v0.2.5
[0.2.4]: https://github.com/femboy2112/libgibson/releases/tag/v0.2.4
[0.2.3]: https://github.com/femboy2112/libgibson/releases/tag/v0.2.3
[0.2.2]: https://github.com/femboy2112/libgibson/releases/tag/v0.2.2
[0.2.1]: https://github.com/femboy2112/libgibson/releases/tag/v0.2.1
[0.2.0]: https://github.com/femboy2112/libgibson/releases/tag/v0.2.0
[0.1.1]: https://github.com/femboy2112/libgibson/releases/tag/v0.1.1
[0.1.0]: https://github.com/femboy2112/libgibson/releases/tag/v0.1.0
