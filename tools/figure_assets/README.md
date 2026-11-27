# tools/figure_assets: offline body authoring for M5-C

Offline, dependency-only tooling. It never ships inside the crate and the crate never learns about it.
Blender 4.5 and the MPFB 2.0.18 extension (with the MakeHuman system asset pack in MPFB user data) are
installed outside this repo; Blender runs headless and only under a heavy `s` lease.

Status of every script in this directory when it was committed:

| file | status |
|---|---|
| `licence_audit.py`, `fa_common.py` (`--selftest`) | run with `python3 -I`, passing |
| `export_mpfb.py` | **never run in Blender.** Its pure-python assembly path was run with a stub Blender layer. |
| `render_probe.py` | first version ran in the MEASURE lease (defects 1-5 viewed); the defect fixes + self-checks of this revision are **UNRUN** |
| `render_tape.py`, `fa_stage.py` | **never run in Blender** (the offline render adapter, see below) |
| `fa_tape.py`, `fa_png.py`, `fa_checks.py`, `fa_skirt.py`, `fa_hair.py`, `make_test_tape.py`, `fa_selftest.py` | pure python, `--selftest` passing under `python3 -I` |
| `make_kit.sh`, `lease_m5c_adapter_smoke.sh` | `bash -n` clean; `make_kit.sh` run on synthetic 64x36 PNGs; the smoke lease control flow run with a fake `BLENDER` stub only |
| `lease_m5c_measure.sh` | `bash -n` clean; control flow exercised with a fake `BLENDER` stub only |

Do not read any of the Blender-side output claims below as observed; the first lease is the observation.

## Commands

Paths come from the environment, never from the repo:

```
export BLENDER=/path/to/blender                       # Blender 4.5.x
export MPFB_DATA=<blender config>/extensions/user_default/mpfb/data
export MPFB_USER_DATA=<blender config>/extensions/.user/user_default/mpfb/data
```

Pure python (no Blender):

```
python3 -I tools/figure_assets/licence_audit.py --out-md docs/evidence/m5c/LICENCE_MANIFEST.md \
        --out-json docs/evidence/m5c/LICENCE_MANIFEST.json
python3 -I tools/figure_assets/fa_common.py --selftest
python3 -I tools/figure_assets/licence_audit.py --check-export OUT/export/manifest.json
```

Blender (do NOT pass `--factory-startup`: the MPFB extension is enabled in the user preferences):

```
"$BLENDER" -b --python tools/figure_assets/export_mpfb.py -- --out OUT/export \
        [--preset tools/figure_assets/presets/default_female.json] [--rig game_engine] \
        [--licence-json OUT/licence.json] [--no-oracle] [--skip-gltf]
"$BLENDER" -b --python tools/figure_assets/render_probe.py -- --export OUT/export --out OUT/render \
        [--engine auto|eevee|cycles] [--spp 32] [--threads 4]
```

The MEASURE lease (one command, runs both under `/usr/bin/time -v`, Tctl every 5 s, 18 minute budget):

```
BLENDER=$BLENDER heavy run --class s --max-min 20 -- bash tools/figure_assets/lease_m5c_measure.sh OUT
```

It prints `STEP <name> rc=… seconds=… max_rss_kb=…`, the `PROBE …` lines, `TCTL_MAX_C=…`, and
`LEASE_FAILS=<n>`; it exits 1 when `n > 0`. Each Blender script prints its own result line
(`EXPORT_RC n`, `PROBE_RC n`); the lease trusts that line over the process exit status and counts a missing
line as a failure (`rc=97`). `export_mpfb.py` exits 4 when it wrote everything but a built-in cross-check
(an `XCHECK FAIL` line) failed.

## Frame, units, conventions

Every array is in the **Blender frame**: right-handed, **+Z up**, the character faces **-Y**, metres. The neutral
body is lifted so its lowest body vertex is at z = 0 (`ground_shift_z_neutral` in `figure_asset.json`); control
deltas are translation-free. After applying deltas the runtime must re-ground (lowest body vertex to z = 0).
UVs use the OBJ convention (v up). The base mesh has **19158** vertices in one index space: indices `0..13379`
are the body, `13380..19157` are helper geometry (joint cubes, tights/skirt/hair fitting helpers, eye/teeth/
tongue helpers). Triangles only reference body vertices; the helper vertices still carry positions and deltas
because **joints are the centroids of helper joint cubes and garments are bound to helper vertices**.

## Output files (`OUT/export`)

JSON header plus little-endian blobs. Every output file is listed with its sha256 in `manifest.json`, together
with `sources` (every third-party input file with sha256, for the licence cross-check), tool versions and the
failed XCHECK lines. No timestamps, fixed ordering: two exports of the same inputs give the same manifest.

| file | content |
|---|---|
| `figure_asset.json` | header: counts, frame, preset, rig summary, blob table for `base.*`, `skin.*`, `skel.*`, `oracle.*` |
| `blobs/base.positions.f32.bin` | (a) neutral positions, f32 x [19158, 3]: macro baseline only, **no control applied** |
| `blobs/base.normals.f32.bin` | smooth normals of the body, f32 x [13380, 3] (area-weighted, computed from the triangles) |
| `blobs/base.tris.u32.bin`, `base.tri_uv.u32.bin`, `base.uv.f32.bin` | body triangles (quads fan-split), per-corner UV index, UV table |
| `controls_export.json` + `blobs/delta.<control>.<neg\|pos>.{idx.u32,xyz.f32}.bin` | (b) sparse per-control deltas at \|c\| = 1, see below |
| `skeleton.json`, `blobs/skel.{rest_matrix,head,tail}.f32.bin` | (c) rest skeleton, bones parents-first; `parent` is an index (-1 root) |
| `blobs/skin.idx.u16.bin`, `blobs/skin.w.f32.bin` | (d) top-4 skin weights per body vertex, normalised, [13380, 4]; bone index into `skeleton.json` order |
| `landmarks.json` | (e) all `joint-*` vertex sets, (f) face landmark sets and the rules that produced them, helper groups |
| `attachments/<name>.json` + `blobs/att.<name>.*` | (g)(h) garment, hair, eyes, brows, lashes, teeth, and the self-authored skirt; see below |
| `body_shaped.glb` | (i) glTF 2.0 of the shaped, rigged body (+ preset's glTF attachments), Y-up, flat materials |
| `oracle_shaped.json`, `blobs/oracle.shaped_positions.f32.bin` | positions of the preset's shaped body (for a runtime equivalence test) |
| `manifest.json` | sha256 + size of every file above, sources, tools, checks |

### Controls and deltas

`controls.json` is the single definition (read by the export and by the licence audit). A control value
`c` in [-1, 1] applies the `neg` side scaled by `|c|` when `c < 0`, the `pos` side scaled by `c` when `c > 0`:
`P = P_neutral + sum_c |c| * delta[c][side(c)]` over the full index space. A side with several targets sums
them with the listed weights. A blob pair `idx` (u32, sorted) and `xyz` (f32 x 3) is sparse.

| control | negative side | positive side |
|---|---|---|
| `stature` (macro) | MakeHuman macro `height` 0.5 -> 0.0 | `height` 0.5 -> 1.0 (evaluated through MPFB, then differenced) |
| `leg_length` | `legs/upperlegs-height-decr` + `legs/lowerlegs-height-decr` | the `-incr` pair |
| `torso_length` | `torso/torso-scale-vert-decr` | `torso/torso-scale-vert-incr` |
| `shoulder_breadth` | `torso/measure-shoulder-dist-decr` | `-incr` |
| `ribcage` | `torso/measure-underbust-circ-decr` | `-incr` |
| `bust_projection` | `breast/breast-point-decr` | `-incr` |
| `bust_fullness` | `torso/measure-bust-circ-decr` | `-incr` |
| `waist` | `torso/measure-waist-circ-decr` | `-incr` |
| `hip_breadth` | `hip/hip-scale-horiz-decr` | `-incr` |
| `glute_projection` | `buttocks/buttocks-volume-decr` | `-incr` |
| `thigh_fullness` | `legs/measure-thigh-circ-decr` | `-incr` |
| `jaw_width` (the face parameter) | `chin/chin-width-decr` | `-incr` |
| `posture_lean` (the posture parameter) | `torso/torso-trans-backward` + `neck/neck-trans-backward` | the `forward` pair |

Notes: MakeHuman ships no kyphosis/lordosis target, so the posture parameter is a torso+neck lean, not a spine
curve. Target names are MakeHuman's file names under the MPFB `targets` directory. Torso/leg proportion is the
pair `torso_length` x `leg_length`. The macro control is nonlinear in the MakeHuman blend, so it is exact only
at the baseline and at the two endpoints; between them it is piecewise linear, which matches how MPFB blends
height itself. Target files carry vertex deltas in MakeHuman decimetres with +Y up; `fa_common.parse_target`
maps them to the Blender frame in metres (x, -z, y) * 0.1, and `export_mpfb.py` checks one target against MPFB's
own shape-key result (`XCHECK own .target parser …`).

### Skeleton and landmarks

Each bone endpoint carries a binding: a list of base-mesh vertex indices whose **mean** is the endpoint (every
MPFB rig strategy used here, CUBE (mean of a `joint-*` group), VERTEX and MEAN, reduces to that). After a shape
change the runtime recomputes joints from the deformed vertices, then bone lengths and inverse binds. The export
checks this against the Blender armature (`XCHECK bone endpoints re-derived …`, tolerance 2 mm). Orientation:
`rest_matrix` (row-major 4x4, bone Y axis head to tail) and the actual `roll`; the file's own `file_roll` is kept
for reference. The runtime may re-derive orientation from head/tail/roll and compare with `rest_matrix`.

Face landmarks (`landmarks.json` `face`): the base mesh has no named nose/brow/mouth groups, so those sets are
**derived by geometric rules** relative to joint centroids (rules are quoted in `face_rules`). They are frozen index
sets, not MakeHuman data, and their quality is a viewing question (RENDER, then look).

### Attachments (garment binding)

Per attached vertex i: three base-mesh vertex indices `bind_idx[i]`, weights `bind_w[i]` (sum 1), an offset
`bind_off[i]`, and three offset scales from `offset_spec`:

```
position[i] = w0*P[v0] + w1*P[v1] + w2*P[v2] + (ox*sx, oy*sy, oz*sz)
sk          = |P[a][axis] - P[b][axis]| / ref        for offset_spec[k] = {a, b, axis, ref}
```

This is the MakeHuman `.mhclo` rule re-expressed in the Blender frame (`fa_common.parse_mhclo`); the offset is
already axis-converted. `export_mpfb.py` checks the formula against MPFB's own fit of `female_casualsuit01`
(`XCHECK own mhclo fit …`). `positions` are the neutral fit. `skin_idx`/`skin_w` are transferred from the bound body
vertices (helper-bound vertices borrow the nearest body vertex). `delete_verts` are the body vertices hidden under
a garment (used for the glTF only).

## Rig choice

Default `game_engine` (MPFB `rig.game_engine`): **53 bones, 30 finger bones (15 per hand: five chains of 3)**, no
toes beyond one `ball` bone per foot, no face bones. Every endpoint is a pure joint-cube mean, so the whole skeleton
re-derives from landmark centroids with no vertex special-casing, which is exactly what B1 needs. It is far below
`MAX_JOINTS` (256). The cost: no jaw and no eye bones, so `Look` needs the eye meshes rotated about the eye joint
landmarks (`joint-l-eye`, `joint-r-eye`) and mouth `Express` needs morph targets or a self-authored jaw. The alternative `--rig default_no_toes`
(137 bones with jaw, eyes, twist and face-muscle bones, still all mean-of-vertices endpoints) is selectable; the
bone count and finger count are recorded in `skeleton.json`. Which to ship is decided after viewing, not here.

## Self-authored (our geometry, repo licence)

* the **loose skirt**: the pack has no skirt or dress, so `fa_common.make_skirt` builds an A-line skirt from rings of
  vertices around the neutral body, bound to nearby hip/waist body vertices with the same rule as above. Its binding
  weights make the hem follow the hips and upper thighs, not the legs separately; legs will poke through a high
  step. That is a known limitation, not a result.
* the face landmark sets, the control table, all scripts, the flat glTF materials (no pack texture is read).

Everything else is read from MakeHuman system assets under the per-file evidence in
`docs/evidence/m5c/LICENCE_MANIFEST.md`. **Licence state when committed:** the CC0 statement is shown on disk for the
base mesh, the garment, hair, eyes, brows, lashes, teeth and the weights files; it is **not shown on disk** for the
control/macro targets, the rig definitions, the vertex-group json and `macro.json`, so those rows are DROP until the
coordinator records the page URL and the exact quoted text in `licence_evidence_external.json`. The lease
prints `LICENCE_CHECK ok=… bad=…` (informational; `LEASE_STRICT_LICENCE=1` makes it a failure). Assets above 2 MB
are never committed: only this directory and the manifests live in the repo; regenerate with the commands above.

## Known limits and unverified items

* Everything Blender-side is unrun. Likely first-run failures are API-level (MPFB service signatures, headless
  `bpy.ops` context, glTF exporter arguments); every such failure prints a traceback and a result line.
* The garment cross-check compares our fit with MPFB's on the macro-applied neutral body; the offline selftest can
  only show gross plausibility (a few cm on the raw base mesh, which differs from the default human).
* Eyes are one flat colour (no iris/pupil texture); hair is a flat material; the skirt is single-sided cloth.
* `render_probe.py` frames the body from its bounding box and uses an opaque ground plane that receives the key
  light shadow; light powers are first guesses. Its arm-raise converts armature-space rotations to bone-local
  quaternions, so it does not depend on the bone roll; the result still has to be viewed.

## Offline render adapter (`render_tape.py`)

Renders a runtime-evaluated performance tape (`gibson.figure.tape.v1`, `docs/design/m5c-studio-api.md` section 5) with the exported body.
It is labelled "offline reference render of a runtime-evaluated performance"; no Blender-authored animation stands in for the tape.

```
"$BLENDER" -b --python tools/figure_assets/render_tape.py -- --export OUT/export --tape T.tape.json \
    --preset tools/figure_assets/presets/generic_neutral.json --shots tools/figure_assets/shots/ex1_character.json --out OUT/render \
    [--spp 32] [--engine auto|eevee|cycles] [--checks-only] [--mutant bad_bounds] [--clip-from A --clip-frames N] [--joint-frame world_aligned|bone]
bash tools/figure_assets/make_kit.sh OUT/render <kit id>          # MP4 (H.264 yuv420p crf 18) + contact sheet per clip, frame sha256 in MANIFEST.md
```

The export must be made with the SAME preset (`export_mpfb.py --preset presets/generic_neutral.json`): the preset's macro and its attachment lists
(hair, garment) define what the export holds. Presets: `generic_neutral.json`, `generic_exaggerated_feminine.json` (control values at the +/-1 ends,
never clamped or averaged by the adapter; a tape may carry values beyond +/-1, they are applied as given and reported as `ControlOutOfDomain`),
`style_restrained.json`, `style_expressive.json`. The preset `render` section holds only what the tape does not carry (skin, hair, brows, eyes) and the
defaults `make_test_tape.py` writes into a test tape; **the tape header wins** for controls, garments, fit_status and materials.

What the adapter does: (1) rebuilds the MPFB human (same macro as the export) and puts the shaped positions on it (export neutral + the tape's control
values through the export's sparse deltas, unclamped, re-grounded), adds the rig exactly like `export_mpfb.py`, deletes the helper geometry; (2) draws a
garment only when `fit_status[id]` starts with `Fitted`, otherwise a yellow label bottom-left on every image says `<id>: NOT DRAWN: <why>` (missing verdict, failed
fit, not in the export); (3) skirt `skirt_a_line`: smooth super-ellipse panels (Solidify + Subsurf), skinned to pelvis/thigh bones with smoothed weights, plus a
bounded deterministic lag toward the previous 6 frames (`fa_skirt.swing`, a pure function of tape frames f-6..f; no cloth simulation); (4) sets every pose bone
from the tape locals with `matrix_basis = M^-1 S_parent^-1 S M` (no keyframes); (5) materials: Principled BSDF slots `base panel trim ornament gem_emissive mask iris
accessory_emissive` (api section 1; garment -> `base`, skirt -> `panel`, eyes -> `iris`), skin with subsurface, key/fill/rim area lights plus a soft top light.

Self-checks (printed as `RT_CHECK PASS|FAIL <name> ...`, recorded in `render.json`, exit 5 when any fails; thresholds are FIRST GUESSES in `fa_checks.py`):
`contact` (min foot-vertex z within 5 mm of the ground OBJECT, rest pose), `shadow` (shadowed share of visible ground pixels around the feet in a check render
vs a Workbench figure mask, read back with `fa_png.py`), `framing:*` (body spans 60..98% of the frame height and is inside it; face close-up >= 45% for the face),
`eye_clearance:*` (rays from the camera to each eye hit no hair/garment/skirt), `brows_lashes_visible`, `brow_vs_skin_contrast`, `pose_math` (Blender skinning vs a
pure top-4 LBS from the same skin matrices on 3 frames), `garments_drawn_or_labelled`, `camera_cuts:*`, `duration:*`.
`--mutant bad_bounds` reproduces the probe bug (bounds over every mesh object including a hidden 2.80 m helper box 0.40 m below the feet, camera from that
height): `contact`, `shadow` and `framing` must then FAIL. `lease_m5c_adapter_smoke.sh` requires exactly that.

Outputs: `stills/<shots id>_<name>_<w>x<h>.png`, `frames/<clip>/frame_<NNNNN>.png`, `render.json`, `oracle_posed.json` + `oracle_posed.f32.bin` (body vertices
0..13379, f32 [frames][vertex][xyz], every `--oracle-every` frames, Blender frame, for the Blender-vs-libgibson oracle).

Tape conventions the format section does not state (assumptions, see `fa_tape.py`): a joint's tape local is its FULL local transform relative to the parent
(replaces the rest local; `W[i] = W[parent] * L[i]`), `root_t3` is an extra model-space translation on every root joint, joint frames at bind are world-aligned
(`--joint-frame bone` uses the exported bone rotations). The oracle is what can falsify them.

Smoke lease: `BLENDER=$BLENDER heavy run --class s --max-min 12 -- bash tools/figure_assets/lease_m5c_adapter_smoke.sh OUT` (10 minute budget).
`SMOKE_PROBE=1` also runs `render_probe.py --checks-only`, normal and mutant.
