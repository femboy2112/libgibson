# Generic reference qualification

The 2026-11-27 replay used a fresh checkout of libgibson
`7d1996425cd2ce4904f55c634a4da5d3bbb647ce` before the living-matter implementation.
The adapter implementation comes from `79a0b0eb7e2305396cc2d67151e714ccf8f1a2b0`.
The original receipt and inputs remain under
`~/.local/state/first-performance/codex-fp`.
New artifacts and complete command output are under
`~/.local/state/second-performance/baseline`.

## Render and export

Blender 4.5.17 LTS, build `2f5ec98689b8`, rendered both 1920×1080 Ex1 stills with
EEVEE, 48 samples, the generic neutral preset, bone joint frames, and tape frame zero.
The GOD class-s lease ran 06:02:31–06:03:48 UTC. All 16 adapter checks passed.
I viewed both images. The front has evenly extended arms and a dark band under the hem.
The close-up shows seams in the crown hair cards and a fixed, brightly lit gaze.

| Shot | Original PNG SHA-256 | Replay PNG SHA-256 | Both concatenated PNG IDAT SHA-256 |
|---|---|---|---|
| Front | `c987e6de90c491db930497842586039691714dac1506b7ff04f62537366c25cb` | `f747e85b08e374b6c9d9847b87525b2231c6d94aed0679801894bb8939ff634e` | `57b67c87db65b664cf1b5f79facac1b767e9866ef0841631bb23c2de651c0593` |
| Face | `ec395c84f2223128cbde85b3711b9059729875bf2697648fa8995441b311ff0a` | `326d0e415fd6538fadc870e2480144a4d3a05f13386845898a5db37961f59577` | `451fbd7539e545b049d7dc48ce4570df8513eee18f9d965548d7a27192518071` |

The replay reads the original tape, fitted positions and export. The image data is
byte-identical. Blender writes a new timestamp and render duration into each PNG.
A separate, pinned source archive repeated the MPFB export, licence audit,
conversion, Rust recipe compilation and Python tape read. Original and rebuilt
inputs have the following identical SHA-256 values.

| Input | SHA-256 |
|---|---|
| Export manifest | `c96b2ec952aedcec907413a324520792d33f0668ddbd4146119ec93b0189a2cb` |
| Converted body | `e1f467e2cd69d8b8654bb0af4af0e1a5c4b02d8a49c7a945b9a709eb9fa1edd4` |
| Tape header | `8d0fbe84bc23c1ecea221b9c8b95928d05cd350a42ceaaabfad56b877e763722` |
| Tape binary | `60b86858f659c6287632cc9f555465df81a31247c2aee3a9b5a7643488b92da8` |
| Fitted coordinates | `916e2a92bb9f1fe05749883e9fc3b666a349d75adf0f63e09a3cd248f4b56153` |

The independent Python shape and joint oracle measured a maximum absolute error of
zero against the Blender export (tolerance 0.00001). The compiler reported Fitted
on the 2,197-vertex suit and 3,456-vertex skirt with zero counted penetration.
The suit adaptation moved 128 vertices by up to 0.009731651 m and the skirt
adaptation moved zero. These values use the runtime vertex-normal fit rule and
the asset's hidden-body indices. The 30 fps tape contains 17 frames.
Both the independent tape format checker and the adapter reader passed.

## Reproduction

From this repository on the GOD host with the installed MPFB data, choose a fresh
output directory. The runner supports the original output location by default.

```bash
export FP_OUTPUT_ROOT="$HOME/.local/state/second-performance/baseline-replay"
export CARGO_TARGET_DIR=/media/leah/SPANKBANK/temp/codex-iii/lg-target
for phase in export tape render controls unmasked; do
  /media/leah/SPANKBANK/GOD/bin/heavy run --who Codex/III-A \
    --why "generic baseline $phase" --class s --max-min 15 -- \
    bash tools/figure_assets/lease_fp_stills.sh "$phase" || exit
done
```

## Retained incoming failure

The pinned Cargo suite ran 194 tests across ten integration targets. Exactly one
failed, `the_fit_id_and_presentation_id_are_rebuilt_by_hand`. Its independent
byte fixture wrote the body-only algorithm identifier for a two-layer recipe.
The compiler hashed the versioned layered algorithm. The complete failing log
is `baseline/qual/clip-cargo.log` and the aggregate returned 1.

The corrected two-layer byte expectation and three independent forward-eye, face-identity
and garment-order controls passed all 36 studio tests against the archived implementation.
The nine other retained targets passed all 161 tests. They cover inverse binds, DQS
twist and bend behavior, IK reach, stance, motion continuity, preview lighting and tape
round trips. The design makes pose-space correctives conditional on measured DQS failure.
The retained source has no separate corrective morph driver to qualify.

The frame-zero Blender probe disables every visibility mask on the evaluated render
surfaces. It retains the actual armatures, subdivision and skirt lining. Counts are
crossing polygon pairs, which measure surface intersection and do not measure depth.

| Frame zero | Body/suit | Body/skirt | Suit/skirt |
|---|---:|---:|---:|
| Render masks on | 29 | 0 | 0 |
| Masks removed | 2,448 | 0 | 0 |
| Injected hidden patch, masks on | 29 | 0 | 0 |
| Same patch, masks removed | 2,387 | 0 | 230 |

Pulling 33 actual hidden cage vertices on the right side of the suit outward caused
the unmasked suit/skirt test to fail with exit code 5 while the displayed suit/skirt
still measured zero. Raw native Blender outputs are `qual/unmasked.log` and
`qual/injected-unmasked.log`. The incoming suit crosses the underlying body even
after the compiler's Fitted verdict. The baseline qualifies the reference replay
and the suit/skirt measurement. Whole-body contact remains open.

Each of the three new studio tests also rejected its predicted defect on the
retained implementation. The isolated mutation run replaced only the named
`cfg!` expression with true, saved its one-line source diff, and required the
single corresponding test to build and fail its assertion. The three raw diffs
and logs are in `baseline/qual` under `face_iris_backward`, `face_unhashed` and
`no_layers`. The full integration runner also registers the ordinary Rust cfg
mutants for all three.
