# Cinematic machine presentation: validation record

Status: **READY FOR REVIEW**. No merge to main.

Starting SHA: `eb34baabc9ce406a5689e90a33bf0bc67e978f4b`.
Branch: `codex/acid-vs-crash-cinematic`.
Production checkpoint: `9ebab17c03dc952529ee0ca2b06f13c84b88b796`.
The final documentation commit is the ending SHA reported in the delivery.
Main remains `1b4358e91df108cdf0e0dbb92e886f010a8b7eb9`.

## Research and actual visual contact

[Research notes](acid-vs-crash-cinematic-research.md) link the director's and
Richard Morrison's interviews, archived production notes, and inspected film
stills. The useful influences were physical luminous architecture, dark space,
strong perspective rails, layered color separation and movement through the
same environment. No film images/dialogue/soundtrack ship in the demo.

Actual release PTY captures were reconstructed and visually inspected at
120×32 and 160×40 TrueColor and 56×24 Mono. A complete autonomous release
playthrough was captured with `--auto --deterministic --speed=10 --debug-shot`;
it exited normally and restored termios. Captured frames show trace, evasion,
DISPLAY assault and Crash containment. This is direct frame inspection by the
implementing agents, not an independent human audience review or a claim to
have watched the film itself during this run.

Two observed art defects were fixed: the pre-arrival route could already be
magenta, and a subsystem label could concatenate with Acid's dialogue. The
new regressions cover both. The floor was dimmed after inspection; fine
structural Braille rails were added over filled RGB faces. The capture helper
also needed correction: incremental byte concatenation caused backpressure
on long recordings. A chunked drain followed by offline reconstruction completed
the movie. The unsuccessful capture was not counted as a completed playthrough.

## Architecture and preservation

Before: ordinary dashboard → foothold dissolve → mostly fixed cyber camera →
headline and bottom controls → dashboard reconstruction.

Now: one machine from the opening frame. `presentation.rs` derives a shot plan;
`world_geom.rs` draws distinct architecture; `cyber.rs` realizes the light,
geometry, topology and trails; `Encounter::cinematic_frame` adds sparse anchored
UI and three stable control rows.

No core renderer/API, BattleGraph, EncounterModel reducer, AcidPlanner,
CrashController, story graph, action costs or outcome rules changed. Existing
RGB/3D/RasterFx/SurfaceFx/Scene/Story machinery is reused. The new camera history
is demo-local, bounded and replay-derived. Legacy code and flat visual goldens
remain: `--presentation=legacy`, `--visual=flat`, `--visual=cyber`.

Removed from the default identity: permanent bordered map/session/log/action
panels, automatic UI-versus-cyber screen distinction, permanent full dashboard
aftermath. These remain comparison tools rather than deleted implementations.

`ShotKind` covers Establishing, Arrival, RouteContest, NodeCloseup, Trace,
Isolation, Decoy, Evasion, DisplayAssault, FinalDuel, CrashWin, AcidWin,
Stalemate and Aftermath. `ShotPlan` contains camera, focal node, field/light
strength, label policy, phase and transition. Accepted actions receive bounded
attention; other tactical shots have minimum holds. Camera and exposure blend
from recorded prior values over 1.25 seconds. Trace attention is temporary;
high trace confidence alone does not permanently force a chase shot.

Stable world anchors carry distinct identities:

| Subsystem | Realization |
|---|---|
| MODEM | Double gateway ring and antenna |
| ROUTE | Switching prism above connected paths |
| AUTH | Nested depth vault, outside-in influence and inner core |
| SHELL | Sloped terminal prism and fine console rails |
| FILES | Separated data slabs, missing pieces when damaged |
| DECOY | Mirrored FILES geometry, materializing frame/cage |
| DISPLAY | Large framed portal with semantic topology/trace engraving |

Acid's actor uses planner progress along valid edges, distinct from phased
traffic dots. Cyan trace travels back over the current legal path. Feints draw
incomplete fading branches; they do not invent legal graph connections.
Isolation exposes broken edges, boundary waves and sparks; KILL shows a local
lease collapse. Integrity removes geometry independently of ownership.

DISPLAY contains one level of semantic UI geometry and a projected ordinary
text echo of the actual input. Its existing Scene entity SurfaceFx may corrupt
that echo, never the real input buffer. There is no general texture mapper or
recursive framebuffer engine. Uppercase T/I/D/K and final 1/2/3/4 shortcuts act
only on empty input; lowercase typed commands still work.

## Five distinct hero compositions

1. Quiet architecture: dim wide establishment, MODEM gateway, negative space.
2. AUTH breach: camera approaches the nested vault; foreign influence reaches
   outer layers while the core and other structures remain recognizable.
3. TRACE: camera rides the return route, cyan trail against magenta pressure;
   actual evasion produces incomplete branching paths.
4. DISPLAY assault: a portal occupies the view; local UI echo bridges geometry
   and ordinary terminal text, while the real prompt stays outside possession.
5. Final duel/formation: broad architectural battle and DISPLAY lattice; Acid's
   title transports illuminated raster samples of the current machine into
   lettering rather than adding an unrelated random fragment cloud.

Crash resolution pulls outward toward damaged structures. Stalemate holds a
stationary interference boundary. Acid holds a fragment formation, then
releases. All paths end with compact scars, trace receipt and replay access
above the same architecture. This is choreographed raster assembly, not a
claim that arbitrary 3D meshes morph into typography.

Narrow terminals favor focal framing and abbreviated final actions; wide
terminals reveal more topology. TrueColor is the hero realization; ANSI256/16
use existing quantization. Mono uses density plus bright structural Braille
and reverse Acid captions. No graphics protocol, raw ANSI visual hack,
networking or host-shell behavior was introduced.

## Correctness and replay

`VisualHistory` retains 48 world-space light samples plus deterministic shot
history. Every exact update, including zero-duration commands, rebuilds the
same camera/action cues. Resize only reprojects. Tests compare the entire
shot/camera sequence over irregular dt, final raster, existing world/planner/
facts/bundles replay, and continuity across instantaneous defensive commands.

Presentation tests cover semantic shot/focal selection, finite responsive
cameras, nonempty depth-tested geometry, RGB diversity, controls at four sizes,
all ending aftermaths, and repeated frozen output. Existing semantic and
renderer goldens remain unchanged. Two old flat-specific rendering tests now
explicitly select Flat; the legacy reassembly test explicitly selects legacy.
The new default receives its own rendering, replay and actual PTY tests.

## Damage and performance observations

Release probes at 160×40, 17 ms recorded input advance, TrueColor:

| Shot | Exact semantic delta | Affected footprint | Emitted bytes |
|---|---:|---:|---:|
| AUTH closeup | 22 | 22 | 855 |
| Trace camera | 1,720 | 1,720 | 48,368 |
| Final duel | 638 | 638 | 20,641 |
| Repeated frozen frame | 0 | 0 | 0 |

Raster-only generation for five 160×36-cell probes measured 3,279–6,873 μs on
this host, with 112–276 triangles writing pixels and 340–2,771 distinct RGB
colors. These are single-run observations under concurrent work, not timing
assertions or sustained FPS benchmarks. Layout, ANSI compilation, terminal
transport and emulator drawing are additional costs. A moving camera naturally
changes broad regions; the renderer is exactly as sparse as the visual change
allows. No core renderer bug was discovered or fixed in this round.

## Commands and results

Baseline: fmt, strict Clippy, cargo test **515 passed**, release build all exit 0.
Final: **535 tests = 226 library unit + 309 integration**, no failures/ignored.
Twenty added tests: 3 architecture, 12 presentation, 4 demo PTY, 1 resize PTY.

All of the following exited 0:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
cargo build --examples
cargo build --release --example acid_vs_crash

cargo test --test raster3d --test raster_fx --test acid_graphics \
  --test acid_architecture --test acid_presentation --test acid_battle \
  --test acid_story --test acid_render --test acid_battlefield \
  --test acid_battlefield_goldens --test whole_renderer_vt100 --test safety_api
cargo test --test pty_demos acid_ -- --test-threads=1
cargo test --test pty_resize_torture -- --test-threads=1
DUMP_ACID_SHOTS=/tmp/libgibson-shots/rasters \
  cargo test --release --test acid_presentation -- --nocapture
```

Targeted combined suite: **131 passed**. Acid PTY suite: **16 passed**. Resize
PTY suite: **3 passed**. Release presentation probes: **12 passed**. The full
suite includes existing demos, scene/reaction/effect/visual goldens, safety,
whole-renderer VT100 and resize torture.

Run-local helpers and receipts are under `/tmp/libgibson-shots`:

```sh
python3 /tmp/libgibson-shots/stage_matrix.py
python3 /tmp/libgibson-shots/mode_matrix.py
python3 /tmp/libgibson-shots/playthrough.py
```

The matrix contains 72 TrueColor stage/size captures (all 18 stages ×
56×24/80×24/120×32/160×40), 21 capability/mode starts and 3 extra Mono captures:
**96 actual PTY sessions**, all exit 0 and termios restored. Modes include
default, manual, auto, deterministic and debug; capability checks include all
four depths. The 21 startup smokes also check prompt and absent image protocol.
The matrix precedes the two caption/arrival-light fixes; focused fresh captures
and regression tests check those fixes. Full-play tests are separate from
startup smokes. The new resize test covers seven cinematic stages × four sizes,
including input preservation.

`bash scripts/dev/bindings_smoke.sh --asan` exited 0: C, C++, Python and
C/C++ ASan/UBSan passed. Go was explicitly skipped because its toolchain is
unavailable. Bindings and core source are unchanged.

Eight additional final-release captures rechecked quiet, AUTH, trace, DISPLAY,
climax and all endings after the caption/arrival fixes; all exited 0 and
restored termios. Images are under `/tmp/libgibson-shots/final-captures/`.
These run-local images are deliberately not committed as a generated corpus.

## Limits and review classification

- **EXPERIMENTAL:** demo-local shot/camera choreography and Rust cinematic APIs.
- **PARTIALLY TESTED:** perceptual pacing/readability across fonts and emulators;
  long interactive sessions, sustained FPS, SSH and multiplexers. Inspection
  captures approximate terminal cells and do not certify every emulator.
- **UNVERIFIED:** independent human aesthetic acceptance, especially the
  "mistaken for Sixel" reaction; Windows/macOS/Go execution.
- Remote CI: **BLOCKED / ENVIRONMENTAL**. Fresh
  [run 35938022709](https://github.com/femboy2112/libgibson/actions/runs/35938022709)
  has zero Rust job steps and an account-payment/spending-limit annotation;
  dependent jobs were skipped. This is not a code-failure diagnosis or green CI.

**READY FOR REVIEW**, with aesthetic acceptance reserved for the user.
