# OPERATION TRUE FORM: handoff (2026-11-27)

Author: LibGibson (Claude session under GOD). This file is public-safe. It contains no images, masks, meshes, recipes or private-kit content. Every pixel, mask, mesh and render named here lives only in the owner-local private kit, which is never in git. This file refers to that kit as `$C`.

## 1. Problem

The owner rejected candidate IV's 3D likeness of the Mk.0 character ("Shrek proportions", masculine read). The mandate is that the 3D body's physical form should match the pinned, approved Mk.0 2D sprite sheet (front, right profile, back and three-quarter views).

Root causes, verified from the candidate review JSONs:

1. **The shape space is too small.** The body is the MakeHuman base plus 13 linear MH target controls (`tools/figure_assets/controls.json` in libgibson). The IV fit saturated the box: bust 1.97, waist 1.95, hip 1.85, glute 1.85, torso 1.53 and shoulder 1.58. The sheet's hourglass is outside the linear span.
2. **The target was contaminated.** The old reference masks were full alpha silhouettes that included hair, laurel and heels. Hair fills the hip band in the front view. As a result the fitter drove thigh_fullness to −1.95 and waist to +1.95, both in the wrong direction. Shoulder +1.58 produced the masculine read.
3. **The pose did not match.** Candidates were rendered from a generic tape (hands on hips, bent knees, flat feet). The sheet shows a straight stance, arms down and heels.
4. **The metric was blind.** Whole-outline IoU (~0.50–0.62) cannot see the defining contours: waist pinch, hip/thigh flare and small shoulders.

## 2. Method (S0–S5)

- **S0: body-only targets.** Per view, alpha minus hair, laurel, veil and loose props. The skin-tight suit, panels, trim, armbands, jewels and shoes count as body. Hair-occluded stretches are reconstructed (colour key, mirror symmetry for front/back, hand polygons for the skull) and flagged as RECONSTRUCTED per pixel. The original reference masks stay byte-identical.
- **S1: pose match.** The candidate is rendered in a sheet-matched rest pose using the existing registration (uniform scale to a common height, centred x, common baseline). The pose is presentation-only: tape header ids are unchanged and only the per-frame joint poses change.
- **S2: profile instrument.** Per view, 48 height-normalised bands give left/right edges and widths (front/back/three-quarter) or front/back depth (side). The instrument separates arms from the torso, finds landmarks as extrema in documented y windows, computes ratios, and compares candidate vs target with relative error per landmark plus registered IoU.
- **S3: "canon-loft" shape layer.** One new smooth displacement target on the existing topology, with weights, UV, rig and helpers preserved.
  - **Stage A:** de-saturate the 13 MH controls, bounded at |c| ≤ 0.85, starting from neutral.
  - **Stage B:** per band and per body segment, scale the cross-section about the segment centroid, separately in width and front/back depth. This is smoothed by a second-difference penalty and blended across segments by skin weights.
  - **Bumps:** separate per-breast and per-glute bump terms.
  - **Solver:** Gauss–Newton against the instrument band edges in all four views.
  - **Output:** a new private export with a 14th control `canon_loft` (pos side, c = 1), which gives a new body_rev. Old ids stay byte-identical.
- **S4 (Minerva/ProjectGHOST):** garment refit through the real fit gates, face-basis re-cook for the new body_rev, and new physics bakes only after the owner accepts the shape. **Not started.**
- **S5:** an owner review sheet (sheet | candidate | body-only overlay plus the landmark table). The owner's eye is final. **Not started.**

### Acceptance (as amended by GOD's rulings)

- Body-only aligned IoU ≥ 0.85 in every view.
- Each landmark within ±6%, height-normalised.
- waist:hip and bust:waist within ±5%, with the waist judged on **back and side only**. Front is compared only like-for-like against a pose-matched render.
- **No thigh gap:** the sheet's thighs touch in front, back and three-quarter.
- Shoulder ≤ sheet in every view.
- Fit gates stay honest: a refused fit is reported and never forced.
- Then the owner's visual verdict. Unseen anatomy stays labelled uncertain.

## 3. Observability limits (measured, not assumed)

- **The front-view arms are fused to the torso in the sheet's own alpha.** Every row from the shoulder to the hip is a single run, so the front waist, underbust and high-hip widths are arm-inclusive and the front waist cannot be measured from the silhouette. Waist truth comes from the back view (arms separate from y_n 0.30 to 0.53) and the side view.
- **There is no thigh gap in the sheet:** the gap is ≤ 0.005 H in front, back and three-quarter.
- **Some regions are reconstructed in S0:** the back skull (fully hair-covered, so it is a polygon), the side and three-quarter nape and lower back, and the three-quarter rear hip edge. Reconstructed pixel counts are front 957, side 2554, back 3887 and three-quarter 1079, out of 44k–71k body pixels per view. These are flagged for the owner's eye in S5.
- **The sheet's three-quarter view is not at the render's −35° azimuth.** A sweep against the solved body peaks flat around −45° (IoU −35° .823, −45° .836, −55° .809, −65° .767).

## 4. Numbers

### Sheet body-only landmarks (width or side depth / figure height, @ y_n)

| landmark | front | back | side | three_quarter |
|---|---|---|---|---|
| shoulder | 0.244 @0.22 | 0.234 @0.20 | 0.183 @0.22 | 0.229 @0.22 |
| bust | 0.301 @0.32* | 0.249 @0.26 | 0.196 @0.27 | 0.247 @0.26 |
| underbust | 0.301 @0.32* | 0.161 @0.29 | 0.126 @0.33 | 0.221 @0.30 |
| waist | 0.299 @0.32* | 0.122 @0.31 | 0.126 @0.33 | 0.197 @0.32 |
| high_hip | 0.331 @0.41* | 0.236 @0.35 | 0.181 @0.38 | 0.226 @0.41 |
| hip | 0.364 @0.49 | 0.344 @0.49 | 0.231 @0.48 | 0.253 @0.47 |
| thigh (per leg) | 0.115 | 0.116 | 0.158 | 0.099 |
| knee | 0.071 | 0.078 | 0.109 | 0.053 |
| calf | 0.088 | 0.093 | 0.113 | 0.062 |
| ankle | 0.044 | 0.043 | 0.058 | 0.034 |

\* Front torso values are arm-fused (see §3).

Ratios:

| view | waist:hip | bust:waist | shoulder:hip |
|---|---|---|---|
| back | 0.354 | 2.041 | 0.680 |
| side | 0.547 | 1.553 | 0.795 |

Side bust projection is 0.065 H and glute projection 0.096 H.

### S2 instrument calibration (synthetic fixtures, in `test_profile.py`)

- **Self-test:** compare(X, X) gives error 0.0 and IoU 1.0 on 12 synthetic cases and on the 4 real sheet alphas.
- **Analytic figure** at 450, 600 and 900 px, with different aspects and offsets:
  - Band error ≤ 0.0035 H (1.9 px).
  - Extremum landmarks ≤ 0.0017 H.
  - Shoulder ≤ 0.0046 H (2.7 px), read on the steep neck/shoulder flank.
  - The 0.020 H thigh gap is recovered exactly.
  - Scaled or offset copies: band diff ≤ 0.0067 H, landmarks ≤ 2.4%, IoU ≥ 0.98.
- **Sensitivity:** widening the analytic waist by +5% reads +4.4% / +6.0% / +4.4% across the three resolutions (the 1 px quantum). Other extrema read 0.

### Candidate vs sheet (relative error, body-only)

Three snapshots of the candidate against the sheet:
- **Baseline IV:** the rejected body, rendered in its old pose with hair.
- **S1:** the same body in the sheet pose.
- **S3b:** canon-loft iteration 2, rendered by Blender.

| landmark | IV f / b / s / 3q | S1 sheet-pose f / b / s / 3q | S3b (Blender) f / b / s / 3q |
|---|---|---|---|
| shoulder | +18 / +27 / +2 / +10 | +16 / +26 / +1 / +8 | −1 / +4 / −17 / −3 |
| bust | −1 / +25 / +11 / +14 | 0 / +24 / +8 / +11 | −15 / +3 / −2 / +2 |
| waist | −32* / +70 / +29 / +2 | −34* / +62 / +21 / −2 | −47* / +28 / −1 / +1 |
| high_hip | −17 / +20 / −7 / +6 | −26 / +3 / −12 / +10 | −35 / −7 / −9 / −1 |
| hip | −18 / −11 / −26 / +6 | −15 / −7 / −30 / 0 | −27 / −21 / −13 / −2 |
| thigh | −8 / −5 / −8 / −1 | +7 / +12 / −11 / −18 | +1 / +1 / −34 / −13 |
| knee | −14 / −22 / −32 / +28 | −16 / −22 / −38 / +5 | +2 / −5 / −34 / +14 |
| calf | −18 / −22 / −16 / +17 | −23 / −25 / −40 / −3 | −13 / −13 / −34 / +7 |
| ankle | −30 / −27 / −5 / +9 | −31 / −29 / −22 / 0 | −26 / −21 / −12 / +12 |
| **IoU** | .590 / .595 / .513 / .624 | .800 / .774 / .648 / .752 | .822 / .837 / .774 / .839 |

\* Front waist is not like-for-like.

**S3 iteration 1** (numpy raster, arms tucked, 3q at −35°):
- IoU: front .845, back .834, side .793, three-quarter .737.
- Back waist −4.5%, back hip −1.4% and back waist:hip −3.1% all **pass**.
- Shoulder is +14% (front) and +22% (back); Stage A had pushed shoulder_breadth to +0.60.
- Side waist depth is +30%; bust and glute projection are −37% and −38%.
- The bust read as conical.

**S3 iteration 2:**
- Shoulder is fixed (shoulder_breadth −0.79 with a hard hinge).
- Bust projection is +7% and glute −13%.
- **Front and back hips regressed:** back waist went to +23% and hip to −22% (see §7).
- The numpy raster agrees with Blender to within ≤ 0.02 IoU per view.

**S3 iteration 3 (wrap; numpy raster only, not Blender-confirmed, not release quality):**
- IoU: front .865, back .881 (best yet), side .736, three-quarter .692.
- The hourglass is restored: back waist −6.9%, hip −3.6%, high_hip +3.7%, waist:hip −3.5%.
- Shoulder: front −0.7%, back +4.8% (over the sheet).
- Side view regressed: bust projection −48%, glute projection −98%, waist depth +54%.
- Stage A saturated: torso_length −0.82, with waist, hip and glute at the 0.85 bound.
- Mesh: 0 flips, but the face-area ratio is 0.38–2.86 (over the 2.2 limit) and 289 faces rotated > 30°.

**Pipeline smoke (real):**
- The native recipe accepted the 14-control export, and all 6 garments came back "Fitted" after adaptive pushes of ≤ 6.6 mm.
- The Blender render matched the numpy prediction.

## 5. Branch / file / sha map

**ProjectGHOST, branch `true-form/loft-20261127`** (off `claude/incarnate-iv-art-20261127` @501ac3be). All files are in `scripts/second_performance_art/`.

| sha | content |
|---|---|
| c5f58552 | `body_masks.py`: S0 body-only masks from the approved sprite views (colour key, mirror, correction polygons, per-pixel reconstructed flags, owner mask sheet). |
| 3cb55bc8 | `profile.py`, `test_profile.py`: S2 instrument. Subcommands `profile`, `profile-set`, `compare`, `compare-set`, `calibrate`. |
| b10f9f91 | `pose_tape.py`: sheet-pose tape writer. Keeps header ids byte-identical, rewrites frames from a JSON per-joint rotation spec, has a `--selftest`. Also adds `perform.py --body-only-masks` (opt-in, writes `masks/body/*.png` with body + shell + panels + trim only). |
| da53ac4d | `loft.py`, `test_loft.py`: numpy silhouette raster (perspective, LBS), Stage A, Stage B band loft, export writer. |
| b0e510b7 | Iteration 2: shoulder hinge, per-breast/glute bumps, bust roundness prior, arm tuck, report. |
| 72ea42e3 | Iteration 3 (wrap): back-view arm-fused rows no longer hidden; only sheet-intrinsic arm rows are one-sided (front/3q y_n 0.22–0.42, back 0.22–0.40). `test_loft.py` 24 OK. |

**libgibson, Codex branch `codex/incarnate-iii-living-matter-20261127` @47e4a7d5:** read only, unchanged. This handoff is mirrored at `docs/handoffs/TRUE_FORM_HANDOFF_20261127.md` on libgibson branch `true-form/handoff-20261127`.

### Running the tests (synthetic only; no private data needed)

```bash
cd scripts/second_performance_art
python3 -BI -m unittest discover -p 'test_profile.py'   # 14 tests, ~19 s
python3 -BI -m unittest discover -p 'test_loft.py'      # 24 tests
python3 -BI pose_tape.py --selftest
```

The only dependencies are numpy, PIL and scipy (1.18). There is no cv2 or skimage.

## 6. Where canon-loft plugs in (libgibson @47e4a7d5)

- **Shaping law:** `P = P_neutral + Σ|c|·delta[c][side(c)]`, then re-ground. See `tools/figure_assets/README.md` (§Controls and deltas, ~l.93) and `tools/figure_assets/fa_tape.py:218` `shape_positions`.
- **Export control list:** `tools/figure_assets/export_mpfb.py:760` writes `controls_export.json`, and the blobs are `blobs/delta.<control>.<neg|pos>.{idx.u32,xyz.f32}.bin`. canon_loft is one more entry here: pos side only, with an empty neg side.
- **Compile to GFA:** `tools/figure_assets/convert_to_gibson.py:396` reads `controls_export.json` generically.
- **Native load:** `src/figure/studio.rs:519` reads the control count, capped by `MAX_ASSET_CONTROLS = 128` (`studio.rs:131`). Each control's min/max/default and morphs follow, so a 14th control needs **no runtime code change**.
- **Identity:** `src/figure/studio.rs:3068` `body_rev_id = sha256(canonical body bytes)` covers the shaped positions and morphs. A new body therefore gets a new body_rev automatically, and old ids are untouched.
- **Fit/tape gate:** `tools/figure_assets/render_tape.py:93` `load_fitted` refuses a tape or fit receipt from another export manifest, body_rev, fit or present.
  - So the correct path for a new body is: convert → native still-recipe (which itself refuses unless every garment is Fitted) → `pose_tape.py` on the native tape.
  - Never edit header ids.

## 7. Open questions (where the work stopped)

1. **The side view fights front/back in the joint fit. This is the main blocker.** Iteration 3 fixed front and back (IoU .865/.881, hourglass back) but lost side bust and glute projection and side waist depth.
   - **Unknown cause.** It could be side-view arm/hand contamination, one shared Stage A body that saturates, or the regularisation strength.
   - **Untried next step:** a decoupled fit, taking depth from the side view only and width from front/back, possibly with depth-only bust/glute bump terms.
   - **Iteration 2's hip regression is now explained.** It was not the arm-margin rule, which marked all hip rows definite. Iteration 2 hid the back view's instrument `arm_fused` rows, and those cover y_n 0.35–0.67: the whole hip, glute and upper-thigh range. This is fixed in 72ea42e3.
2. **Bust and glute projection vs volume sanity.** The sheet's bust projection (0.065 H) and glute projection (0.096 H) need depth scales near or above the 1.6 bound, or dedicated bump terms. A radial-inflation bump crumpled the chest; a normal-direction bump works but left glute at −13%. What is the right parameterisation for a round, high bust that stays well-shaped under skinning?
3. **Front-view comparability.** Is a pose where the arms rest on the torso (as the sheet does) achievable without interpenetration? The sheet's waist is ~20 cm narrower than its hips, so a straight forearm cannot touch both. Or should the front-view torso bands be dropped from the objective, as GOD's ruling allows?
4. **Mesh quality limits.** Iteration 3 breaks the area-ratio limit (2.86 > 2.2). Iteration 2 has 0 flips, a face-area ratio of 0.60–1.93 and 174 faces rotated > 30°. Is that acceptable for skinning and garment adaptation at S4? Garment pushes were ≤ 6.6 mm at iteration 2. The iteration-1 body was larger and has not been run through the recipe.
5. **Side view** is the weakest view: IoU .77 in Blender, with thigh, knee and calf −34% in depth. The leg depth needs its own segment scaling; the band loft currently under-drives the lower legs in side depth.
6. **Owner-eye items for S5:** the back skull width (a guess), the side nape and lower back under the hair, and the three-quarter rear hip edge.

No lease is running. The private kit holds all S0–S3b outputs with sha256 receipts, for any later continuation under GOD.
